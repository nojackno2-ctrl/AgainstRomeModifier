[CmdletBinding()]
<#
.SYNOPSIS
Runs the Rust rewrite test gates. Every cargo command is --locked; -Offline adds --offline.

.DESCRIPTION
Default: host `cargo test` for every native/against-rome-* workspace member plus the tools/re/*-rs probes.
-IncludeDesktop  also tests apps/against-rome-editor (the egui app; excluded by default as the slowest build).
-TraceWin32      additionally runs the i686 static-CRT panic=abort trace DLL release gate (Windows/MSVC only).
-TraceOnly       runs ONLY the i686 trace gate (always --release, implies -TraceWin32, -Release is redundant)
                 and cannot be combined with -IncludeDesktop/-Retail.
-Retail          adds the ignored installed-game texture roundtrip.
Workspace members are checked against `cargo metadata`, so a new member cannot silently skip this script.
#>
[CmdletBinding()]
param([switch]$Release, [switch]$IncludeDesktop, [switch]$Retail,
    [switch]$TraceWin32, [switch]$TraceOnly, [switch]$Offline)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$localRuntime = Join-Path $repo 're_workspace/rust-runtime'
$localCargo = Join-Path $localRuntime 'cargo/bin/cargo.exe'
if (Test-Path -LiteralPath $localCargo) {
    $env:CARGO_HOME = Join-Path $localRuntime 'cargo'
    $env:RUSTUP_HOME = Join-Path $localRuntime 'rustup'
    $env:PATH = (Join-Path $env:CARGO_HOME 'bin') + [IO.Path]::PathSeparator + $env:PATH
    $cargo = $localCargo
} else {
    $cargo = (Get-Command cargo -ErrorAction Stop).Source
}
$crates = @('native/against-rome-data', 'native/against-rome-editor',
    'native/against-rome-navigation', 'native/against-rome-modifier', 'native/against-rome-saves',
    'native/against-rome-placement', 'native/against-rome-scenario', 'native/against-rome-textures',
    'native/against-rome-environment', 'native/against-rome-landscape', 'native/against-rome-packaging',
    'native/against-rome-trace', 'native/against-rome-maps', 'native/against-rome-campaign',
    'native/against-rome-ai-planner', 'native/against-rome-ambience', 'native/against-rome-scene')
$probes = @('formation-probe-rs', 'shad-probe-rs', 'alr-probe-rs', 'apt-probe-rs', 'anim-probe-rs',
    'alr-scanline-differential-rs', 'lighting-probe-rs', 'roads-golden-rs', 'formation-golden-rs', 'particle-assets-rs',
    'particle-definition-probe-rs', 'particle-numeric-contract-rs', 'particle-render-contract-rs',
    'saved-light-contract-rs', 'ar-re-tools-rs') | ForEach-Object { "tools/re/$_" }
$desktop = 'apps/against-rome-editor'
# TraceOnly is a focused release/i686 gate; it never runs retail or C# oracles.
if ($TraceOnly) {
    if ($IncludeDesktop -or $Retail) { throw 'TraceOnly cannot be combined with desktop/retail gates.' }
    $TraceWin32 = $true
}
# CARGO_NET_OFFLINE is the env form of --offline and covers every cargo call below.
if ($Offline) { $env:CARGO_NET_OFFLINE = 'true' }
# Every workspace member must be a known gate; fails on drift in either direction.
$metadata = (& $cargo metadata --locked --no-deps --format-version 1 `
    --manifest-path (Join-Path $repo 'Cargo.toml')) -join "`n"
if ($LASTEXITCODE -ne 0) { throw "cargo metadata failed (exit $LASTEXITCODE)" }
$members = @{}
foreach ($package in ($metadata | ConvertFrom-Json).packages) {
    $dir = [IO.Path]::GetRelativePath($repo, (Split-Path $package.manifest_path -Parent)).Replace('\', '/')
    $members[$dir] = $package.name
}
$known = @($crates) + $probes + $desktop
$unlisted = @($members.Keys | Where-Object { $known -notcontains $_ })
$missing = @($known | Where-Object { !$members.ContainsKey($_) })
if ($unlisted.Count -or $missing.Count) {
    throw "Workspace members out of sync with test script. Unlisted: [$($unlisted -join ', ')] Missing: [$($missing -join ', ')]"
}
if ($IncludeDesktop) { $crates += $desktop }
if ($TraceOnly) { $crates = @() } else { $crates += $probes }
foreach ($crate in $crates) {
    $manifest = Join-Path $repo "$crate/Cargo.toml"
    if (!(Test-Path -LiteralPath $manifest)) { throw "Missing Rust rewrite crate: $crate" }
    $name = $crate.Replace('/', '-')
    $target = Join-Path $repo "re_workspace/rust-rewrite-targets/$name"
    if ($crate -eq 'apps/against-rome-editor') {
        $target = Join-Path $repo 're_workspace/rust-editor-app-target'
    }
    $arguments = @('test', '--locked', '--manifest-path', $manifest, '--target-dir', $target)
    if ($crate -eq 'native/against-rome-trace') {
        $arguments += @('--config', (Join-Path $repo 'native/against-rome-trace/.cargo/config.toml'))
    }
    if ($Release) { $arguments += '--release' }
    Write-Host "Testing $crate"
    & $cargo @arguments
    if ($LASTEXITCODE -ne 0) { throw "Rust tests failed: $crate (exit $LASTEXITCODE)" }
}
if ($TraceWin32) {
    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) { throw 'TraceWin32 requires Windows and MSVC build tools.' }
    $manifest = Join-Path $repo 'native/against-rome-trace/Cargo.toml'
    $config = Join-Path $repo 'native/against-rome-trace/.cargo/config.toml'
    $target = Join-Path $repo 're_workspace/target-against-rome-trace'
    # --manifest-path does NOT load the manifest directory's .cargo/config.toml.
    # Explicit --config makes this invocation independent of the caller's CWD.
    # https://doc.rust-lang.org/cargo/reference/config.html#hierarchical-structure
    $common = @('--locked', '--manifest-path', $manifest, '--config', $config,
        '--target-dir', $target, '--target', 'i686-pc-windows-msvc', '--release')

    $testOutput = @(& $cargo test @common)
    $testExit = $LASTEXITCODE
    $testOutput | ForEach-Object { Write-Host $_ }
    if ($testExit -ne 0) { throw "Trace i686 tests failed (exit $testExit). Do not bypass app-control error 4551." }
    if (!(($testOutput -join "`n") -match 'test result: ok\. [1-9][0-9]* passed;')) { throw 'Trace i686 ran no passing tests.' }
    # Keep the unwind-capable test harness outside the DLL-only profile override.
    $previousReleasePanic = $env:CARGO_PROFILE_RELEASE_PANIC
    try {
        $env:CARGO_PROFILE_RELEASE_PANIC = 'abort'
        $cfg = @(& $cargo rustc @common --lib -- --print cfg)
        if ($LASTEXITCODE -ne 0) { throw "Trace cfg probe failed (exit $LASTEXITCODE)." }
        if ($cfg -notcontains 'panic="abort"') { throw 'Trace release DLL requires panic="abort".' }
        if ($cfg -notcontains 'target_feature="crt-static"') {
            throw 'Trace requires crt-static; check RUSTFLAGS/CARGO_ENCODED_RUSTFLAGS overrides.'
        }
        Write-Host 'PASS: i686 effective panic="abort", target_feature="crt-static"'
        & $cargo build @common --lib
        if ($LASTEXITCODE -ne 0) { throw "Trace release DLL build failed (exit $LASTEXITCODE)." }
    } finally {
        $env:CARGO_PROFILE_RELEASE_PANIC = $previousReleasePanic
    }
    $dll = Join-Path $target 'i686-pc-windows-msvc/release/winmm.dll'
    if (!(Test-Path -LiteralPath $dll -PathType Leaf)) { throw "Missing trace DLL: $dll" }
    $dumpbinCommand = Get-Command dumpbin.exe -ErrorAction SilentlyContinue
    if ($dumpbinCommand) { $dumpbin = $dumpbinCommand.Source } else {
        $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
        if (!(Test-Path -LiteralPath $vswhere)) { throw 'dumpbin.exe/vswhere.exe required for CRT dependency audit.' }
        $vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if ($LASTEXITCODE -ne 0 -or !$vs) { throw 'MSVC build tools not found.' }
        $dumpbin = Get-ChildItem -Path (Join-Path $vs 'VC/Tools/MSVC/*/bin/Hostx64/x86/dumpbin.exe') |
            Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
        if (!$dumpbin) { throw 'MSVC x86 dumpbin.exe not found.' }
    }
    $headers = @(& $dumpbin /headers $dll)
    if ($LASTEXITCODE -ne 0) { throw 'Trace PE header audit failed.' }
    if (!(($headers -join "`n") -match '(?im)^\s*14C machine \(x86\)')) { throw 'Trace DLL is not i686.' }
    $dependencies = @(& $dumpbin /dependents $dll)
    if ($LASTEXITCODE -ne 0) { throw 'Trace CRT dependency audit failed.' }
    $dependencies | ForEach-Object { Write-Host $_ }
    if (($dependencies -join "`n") -match '(?im)^\s*(?:(?:msvcr|msvcp|vcruntime|concrt)[\w.-]*|ucrtbase[d]?|api-ms-win-crt-[\w.-]+)\.dll\s*$') {
        throw 'Trace DLL imports a dynamic Microsoft CRT.'
    }
    Write-Host 'PASS: i686 DLL has no dynamic Microsoft CRT dependencies'
    # Parse exports without loading the proxy DLL into this process or a game.
    & $cargo run --locked --manifest-path $manifest --config $config --target-dir $target --example audit_exports -- $dll
    if ($LASTEXITCODE -ne 0) { throw "Trace export audit failed (exit $LASTEXITCODE)." }
    Write-Host "Verified trace DLL: $dll SHA256=$((Get-FileHash -LiteralPath $dll -Algorithm SHA256).Hash)"
}
function Invoke-DifferentialTest {
    param([string[]]$CargoArguments, [string]$Label)
    # libtest returns success even when a renamed filter matches zero tests.
    $output = @(& $cargo @CargoArguments)
    $testExitCode = $LASTEXITCODE
    $output | ForEach-Object { Write-Host $_ }
    if ($testExitCode -ne 0) { throw "$Label failed (exit $testExitCode)." }
    if (!(($output -join "`n") -match 'test result: ok\. [1-9][0-9]* passed;')) {
        throw "$Label executed no passing tests; check the differential filter and ignored gate."
    }
}
if ($Retail) {
    $arguments = @('test', '--locked', '--manifest-path',
        (Join-Path $repo 'native/against-rome-textures/Cargo.toml'), '--target-dir',
        (Join-Path $repo 're_workspace/rust-rewrite-targets/native-against-rome-textures'),
        '--test', 'installed_maps_tests')
    if ($Release) { $arguments += '--release' }
    $arguments += @('--', '--ignored', '--nocapture')
    Invoke-DifferentialTest -CargoArguments $arguments -Label 'Retail textures TEMP roundtrip'
}
