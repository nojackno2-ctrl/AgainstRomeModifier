[CmdletBinding()]
param([switch]$Release, [switch]$IncludeDesktop, [switch]$Differential)
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
    'native/against-rome-placement', 'native/against-rome-scenario', 'native/against-rome-textures')
if ($IncludeDesktop) { $crates += 'apps/against-rome-editor' }
foreach ($crate in $crates) {
    $manifest = Join-Path $repo "$crate/Cargo.toml"
    if (!(Test-Path -LiteralPath $manifest)) { throw "Missing Rust rewrite crate: $crate" }
    $name = $crate.Replace('/', '-')
    $target = Join-Path $repo "re_workspace/rust-rewrite-targets/$name"
    if ($crate -eq 'apps/against-rome-editor') {
        $target = Join-Path $repo 're_workspace/rust-editor-app-target'
    }
    $arguments = @('test', '--locked', '--manifest-path', $manifest, '--target-dir', $target)
    if ($Release) { $arguments += '--release' }
    Write-Host "Testing $crate"
    & $cargo @arguments
    if ($LASTEXITCODE -ne 0) { throw "Rust tests failed: $crate (exit $LASTEXITCODE)" }
}
if ($Differential) {
    $arguments = @('test', '--locked', '--manifest-path',
        (Join-Path $repo 'native/against-rome-editor/Cargo.toml'), '--target-dir',
        (Join-Path $repo 're_workspace/rust-rewrite-targets/native-against-rome-editor'),
        '--test', 'differential')
    if ($Release) { $arguments += '--release' }
    $arguments += @('--', '--ignored', '--nocapture')
    & $cargo @arguments
    if ($LASTEXITCODE -ne 0) { throw 'Live C# / Rust terrain differential failed.' }
    $gates = @(
        @{ Crate = 'against-rome-placement'; Filter = 'managed_family_light_auxiliary_differential'; Kind = '--lib' },
        @{ Crate = 'against-rome-scenario'; Filter = 'csharp_synthetic_differential'; Kind = '--test' }
    )
    foreach ($gate in $gates) {
        $arguments = @('test', '--locked', '--manifest-path',
            (Join-Path $repo "native/$($gate.Crate)/Cargo.toml"), '--target-dir',
            (Join-Path $repo "re_workspace/rust-rewrite-targets/native-$($gate.Crate)"), $gate.Kind)
        if ($gate.Kind -eq '--test') { $arguments += 'compiler' }
        if ($Release) { $arguments += '--release' }
        $arguments += @($gate.Filter, '--', '--ignored', '--nocapture')
        & $cargo @arguments
        if ($LASTEXITCODE -ne 0) { throw "C# / Rust differential failed: $($gate.Crate)" }
    }
}
