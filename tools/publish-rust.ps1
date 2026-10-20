#Requires -Version 7
<#
.SYNOPSIS
    Stages a reproducible Windows x64 portable folder for the Rust desktop suite (map editor, modifier GUI, launcher, save manager, modifier CLI, and packaging CLI; no .NET involved).
.DESCRIPTION
    Builds apps/against-rome-editor (against-rome-editor, against-rome-modifier-gui, against-rome-launcher, and save-manager),
    native/against-rome-modifier (against-rome-modifier), and native/against-rome-packaging (ArmMapPackage) with `cargo build --release --locked --offline`,
    after running `cargo fetch --locked --manifest-path Cargo.toml` to populate/bootstrap CARGO_HOME dependencies for clean environments (skipped with -Offline),
    using an explicit x86_64-pc-windows-msvc target and process-local static CRT flags,
    then copies all six executables (against-rome-editor.exe, against-rome-modifier-gui.exe, against-rome-launcher.exe, save-manager.exe,
    against-rome-modifier.exe, and ArmMapPackage.exe) plus the runtime assets it actually probes (ThirdParty/dgVoodoo2 and ThirdParty/argm-trace, read by
    InstallResources::discover() next to the exe). ThirdParty/argm-trace/winmm.dll is the 32-bit Rust cdylib built from the workspace crate
    native/against-rome-trace (i686-pc-windows-msvc, Release, --locked --offline, panic=abort, static CRT) into a repo-local target dir; the legacy C++
    ThirdParty/argm-trace/winmm.dll is never copied into the package into a NEW staging directory and writes provenance.json. It then creates a sibling ZIP
    containing the staging contents and a `.zip.sha256` checksum file; existing destinations are never overwritten.
    Existing output is never overwritten. Cargo and git run from the repo root.
    Source-state provenance: before the build, after the build and again after staging, the script fingerprints the
    working tree (HEAD, `git status`, full `git diff HEAD --binary` SHA256, and a SHA256 manifest of every untracked
    non-ignored file). If the fingerprint changes during the run (a build race with another editor/agent), the run is
    refused and the staging directory is removed. Ignored files (target dirs, artifacts) are not fingerprinted.
    With -SkipBuild the existing exes are NOT tied to the current commit: they are marked unverifiable in provenance.json
    and in the staging folder name.
    If anything fails, or the pipeline is stopped, only files and directories created by this run are removed (the
    staging folder and partial archive/checksum, plus parent directories this run created, deepest first and only while empty).
    This is a staging script only; it does not prove portability on a clean Windows.
    The legacy .NET flow lives in tools/publish.ps1 and is untouched.
.PARAMETER OutputDirectory
    Parent directory for the staging folder (default: <repo>/artifacts/rust-editor). The staging folder is
    <OutputDirectory>/against-rome-editor-<version>-win-x64-<shortcommit>[-dirty][-skipbuild-unverified] and must not
    already exist.
.PARAMETER SkipBuild
    Reuse existing release exes instead of invoking cargo. The exe provenance is recorded as unverifiable.
.PARAMETER Offline
    Skip network bootstrap and build using only the existing Cargo cache. Missing dependencies fail closed.
.EXAMPLE
    ./tools/publish-rust.ps1 -WhatIf
#>
[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Medium')]
param(
    [string]$OutputDirectory,
    [switch]$SkipBuild,
    [switch]$Offline
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repo = Split-Path $PSScriptRoot -Parent
$targetTriple = 'x86_64-pc-windows-msvc'
$rootManifest = Join-Path $repo 'Cargo.toml'
$editorManifest = Join-Path $repo 'apps/against-rome-editor/Cargo.toml'
$editorTargetDir = Join-Path $repo 're_workspace/rust-editor-app-target'
$editorExeName = 'against-rome-editor.exe'
$editorExe = Join-Path $editorTargetDir "release/$editorExeName"

$modifierGuiExeName = 'against-rome-modifier-gui.exe'
$modifierGuiExe = Join-Path $editorTargetDir "release/$modifierGuiExeName"

$launcherExeName = 'against-rome-launcher.exe'
$launcherExe = Join-Path $editorTargetDir "release/$launcherExeName"

$saveManagerExeName = 'save-manager.exe'
$saveManagerExe = Join-Path $editorTargetDir "release/$saveManagerExeName"

$modifierManifest = Join-Path $repo 'native/against-rome-modifier/Cargo.toml'
$modifierTargetDir = Join-Path $repo 're_workspace/rust-rewrite-targets/native-against-rome-modifier'
$modifierExeName = 'against-rome-modifier.exe'
$modifierExe = Join-Path $modifierTargetDir "release/$modifierExeName"

$packagingManifest = Join-Path $repo 'native/against-rome-packaging/Cargo.toml'
$packagingTargetDir = Join-Path $repo 're_workspace/rust-rewrite-targets/native-against-rome-packaging'
$packagingExeName = 'ArmMapPackage.exe'
$packagingExe = Join-Path $packagingTargetDir "release/$packagingExeName"

# Rust runtime trace proxy (workspace crate, 32-bit because the game is 32-bit). Built by this script and staged as
# ThirdParty/argm-trace/winmm.dll; the legacy C++ ThirdParty/argm-trace/winmm.dll source-tree binary is never copied.
$traceManifest = Join-Path $repo 'native/against-rome-trace/Cargo.toml'
$traceTargetTriple = 'i686-pc-windows-msvc'
$traceTargetDir = Join-Path $repo 're_workspace/rust-rewrite-targets/native-against-rome-trace'
$traceDllName = 'winmm.dll'
$traceDll = Join-Path $traceTargetDir "$traceTargetTriple/release/$traceDllName"
$traceStagedRelative = 'ThirdParty/argm-trace/winmm.dll'

# Runtime assets the app really probes (install.rs: InstallResources::from_third_party). Everything else is optional docs.
# The trace winmm.dll is deliberately NOT listed here: it is a build output (see $traceDll), not a repository asset.
$assets = @(
    'ThirdParty/dgVoodoo2/D3D8.dll', 'ThirdParty/dgVoodoo2/DDraw.dll',
    'ThirdParty/dgVoodoo2/dgVoodooCpl.exe', 'ThirdParty/dgVoodoo2/dgVoodoo.conf',
    'ThirdParty/dgVoodoo2/REDISTRIBUTION.md',
    'ThirdParty/argm-trace/README.md')
if (!$OutputDirectory) { $OutputDirectory = Join-Path $repo 'artifacts/rust-editor' }
$outputRoot = [IO.Path]::GetFullPath($OutputDirectory)

function Get-Sha256([string]$Path) { (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() }
function Get-BytesSha256([byte[]]$Bytes) { [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($Bytes)).ToLowerInvariant() }

# Validate reused outputs too: a filename alone does not prove architecture or native GUI/CLI kind.
function Assert-NativeWindowsExe([string]$Path, [int[]]$Subsystem) {
    $stream = [IO.File]::OpenRead($Path)
    $reader = [IO.BinaryReader]::new($stream)
    try {
        if ($stream.Length -lt 64 -or $reader.ReadUInt16() -ne 0x5a4d) { throw 'missing DOS header' }
        $stream.Position = 60
        $pe = $reader.ReadUInt32()
        if ($pe -lt 64 -or ([long]$pe + 264) -gt $stream.Length) { throw 'invalid PE offset' }
        $stream.Position = $pe
        if ($reader.ReadUInt32() -ne 0x4550 -or $reader.ReadUInt16() -ne 0x8664) { throw 'expected AMD64 PE' }
        $stream.Position = $pe + 24
        if ($reader.ReadUInt16() -ne 0x20b) { throw 'expected PE32+' }
        $stream.Position = $pe + 92
        if ($reader.ReadUInt16() -notin $Subsystem) { throw "expected subsystem $($Subsystem -join ' or ')" }
        $stream.Position = $pe + 132
        if ($reader.ReadUInt32() -lt 15) { throw 'missing PE data directories' }
        $stream.Position = $pe + 248
        if ($reader.ReadUInt64() -ne 0) { throw 'managed CLR image is not a Rust-only executable' }
    } catch { throw "Invalid native Windows x64 executable '$Path': $($_.Exception.Message)" }
    finally { $reader.Dispose() }
}

# The game is a 32-bit process, so the trace proxy must be a native PE32 (i386) DLL with exports and no CLR header.
function Assert-NativeWindowsDll32([string]$Path) {
    $stream = [IO.File]::OpenRead($Path)
    $reader = [IO.BinaryReader]::new($stream)
    try {
        if ($stream.Length -lt 64 -or $reader.ReadUInt16() -ne 0x5a4d) { throw 'missing DOS header' }
        $stream.Position = 60
        $pe = $reader.ReadUInt32()
        if ($pe -lt 64 -or ([long]$pe + 248) -gt $stream.Length) { throw 'invalid PE offset' }
        $stream.Position = $pe
        if ($reader.ReadUInt32() -ne 0x4550 -or $reader.ReadUInt16() -ne 0x14c) { throw 'expected i386 PE' }
        $stream.Position = $pe + 22
        if (($reader.ReadUInt16() -band 0x2000) -eq 0) { throw 'not a DLL image' }
        $stream.Position = $pe + 24
        if ($reader.ReadUInt16() -ne 0x10b) { throw 'expected PE32' }
        $stream.Position = $pe + 116
        if ($reader.ReadUInt32() -lt 15) { throw 'missing PE data directories' }
        $stream.Position = $pe + 120
        if ($reader.ReadUInt32() -eq 0) { throw 'missing export directory' }
        $stream.Position = $pe + 232
        if ($reader.ReadUInt32() -ne 0) { throw 'managed CLR image is not a Rust-only DLL' }
    } catch { throw "Invalid native Windows 32-bit DLL '$Path': $($_.Exception.Message)" }
    finally { $reader.Dispose() }
}

# Runs git (read-only, no optional index locks) from the repo root and returns raw stdout bytes.
function Invoke-GitBytes([string[]]$GitArgs) {
    $psi = [Diagnostics.ProcessStartInfo]::new($script:gitExe)
    $psi.WorkingDirectory = $script:repo
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    foreach ($a in @('--no-optional-locks') + $GitArgs) { $psi.ArgumentList.Add($a) }
    $p = [Diagnostics.Process]::Start($psi)
    $errTask = $p.StandardError.ReadToEndAsync()   # drained concurrently (avoids pipe deadlock); only surfaced on failure
    $ms = [IO.MemoryStream]::new()
    $p.StandardOutput.BaseStream.CopyTo($ms)
    $p.WaitForExit()
    if ($p.ExitCode -ne 0) { throw "git $($GitArgs -join ' ') failed (exit $($p.ExitCode)): $($errTask.Result)" }
    return , $ms.ToArray()
}

# SHA256 of a file that may be open elsewhere; unverifiable source files fail closed.
function Get-SharedFileSha256([string]$Path) {
    try {
        $fs = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete)
        try { return [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($fs)).ToLowerInvariant() }
        finally { $fs.Dispose() }
    } catch { throw "Cannot fingerprint source file '$Path': $($_.Exception.Message)" }
}

# Fingerprint of the full source state: HEAD + status + complete tracked diff + untracked file manifest.
function Get-SourceState {
    $head = ([Text.Encoding]::UTF8.GetString((Invoke-GitBytes @('rev-parse', 'HEAD')))).Trim()
    if ($head -notmatch '^[0-9a-f]{40}$') { throw 'git rev-parse HEAD failed.' }
    $statusBytes = Invoke-GitBytes @('status', '--porcelain=v1', '-z', '--untracked-files=all', '--no-renames')
    $diffBytes = Invoke-GitBytes @('diff', 'HEAD', '--binary', '--no-ext-diff', '--no-textconv', '--no-renames')
    $untrackedBytes = Invoke-GitBytes @('ls-files', '--others', '--exclude-standard', '-z')
    $changed = @([Text.Encoding]::UTF8.GetString($statusBytes).Split([char]0, [StringSplitOptions]::RemoveEmptyEntries)).Count
    $untracked = [Text.Encoding]::UTF8.GetString($untrackedBytes).Split([char]0, [StringSplitOptions]::RemoveEmptyEntries)
    [Array]::Sort($untracked, [StringComparer]::Ordinal)
    $manifestText = [Text.StringBuilder]::new()
    foreach ($u in $untracked) {
        [void]$manifestText.Append((Get-SharedFileSha256 (Join-Path $script:repo $u))).Append('  ').Append($u).Append("`n")
    }
    $statusSha = Get-BytesSha256 $statusBytes
    $diffSha = Get-BytesSha256 $diffBytes
    $untrackedSha = Get-BytesSha256 ([Text.Encoding]::UTF8.GetBytes($manifestText.ToString()))
    $combined = Get-BytesSha256 ([Text.Encoding]::UTF8.GetBytes("$head|$statusSha|$diffSha|$untrackedSha"))
    [pscustomobject]@{
        Head = $head; ChangedPaths = $changed; Dirty = ($changed -gt 0); UntrackedCount = $untracked.Count
        StatusSha256 = $statusSha; TrackedDiffSha256 = $diffSha; UntrackedManifestSha256 = $untrackedSha
        SourceStateSha256 = $combined
    }
}

function Assert-SourceStateUnchanged($Before, [string]$Phase) {
    $now = Get-SourceState
    if ($now.SourceStateSha256 -ne $Before.SourceStateSha256) {
        throw ("Source tree changed $Phase (build race): HEAD $($Before.Head.Substring(0, 8))->$($now.Head.Substring(0, 8)), " +
            "state $($Before.SourceStateSha256)->$($now.SourceStateSha256). Refusing to publish; re-run once the tree is stable.")
    }
}

Push-Location -LiteralPath $repo
$origCargoHome = $env:CARGO_HOME
$origRustupHome = $env:RUSTUP_HOME
$origPath = $env:PATH
$origRustFlags = $env:RUSTFLAGS
$origEncodedRustFlags = $env:CARGO_ENCODED_RUSTFLAGS
try {
    $gitExe = (Get-Command git -ErrorAction Stop).Source

    # Same toolchain resolution convention as tools/run-rust-editor.ps1 (repo-local runtime first, else PATH).
    $runtime = Join-Path $repo 're_workspace/rust-runtime'
    $localCargo = Join-Path $runtime 'cargo/bin/cargo.exe'
    if (Test-Path -LiteralPath $localCargo) {
        if (!$env:CARGO_HOME) { $env:CARGO_HOME = Join-Path $runtime 'cargo' }
        if (!$env:RUSTUP_HOME) { $env:RUSTUP_HOME = Join-Path $runtime 'rustup' }
        $env:PATH = (Join-Path $runtime 'cargo/bin') + [IO.Path]::PathSeparator + $env:PATH
        $cargo = $localCargo
    } else {
        $cargo = (Get-Command cargo -ErrorAction Stop).Source
    }

    if (!(Test-Path -LiteralPath $rootManifest)) { throw "Missing root manifest: $rootManifest" }
    if (!(Test-Path -LiteralPath $editorManifest)) { throw "Missing manifest: $editorManifest" }
    if (!(Test-Path -LiteralPath $modifierManifest)) { throw "Missing manifest: $modifierManifest" }
    if (!(Test-Path -LiteralPath $packagingManifest)) { throw "Missing manifest: $packagingManifest" }
    if (!(Test-Path -LiteralPath $traceManifest)) { throw "Missing manifest: $traceManifest" }
    foreach ($a in $assets) {
        if (!(Test-Path -LiteralPath (Join-Path $repo $a) -PathType Leaf)) { throw "Missing runtime asset: $a" }
    }

    # --- provenance inputs (read-only) ---
    $version = [regex]::Match((Get-Content -LiteralPath $editorManifest -Raw), '(?m)^\s*version\s*=\s*"([^"]+)"').Groups[1].Value
    if (!$version) { throw 'Cannot read package version from editor Cargo.toml.' }
    $modifierVersion = [regex]::Match((Get-Content -LiteralPath $modifierManifest -Raw), '(?m)^\s*version\s*=\s*"([^"]+)"').Groups[1].Value
    if (!$modifierVersion) { throw 'Cannot read package version from modifier Cargo.toml.' }
    $packagingVersion = [regex]::Match((Get-Content -LiteralPath $packagingManifest -Raw), '(?m)^\s*version\s*=\s*"([^"]+)"').Groups[1].Value
    if (!$packagingVersion) { throw 'Cannot read package version from packaging Cargo.toml.' }

    $state = Get-SourceState
    $commit = $state.Head
    $dirty = $state.Dirty
    $rustcVv = @(& rustc -vV)
    if ($LASTEXITCODE -ne 0) { throw "rustc -vV failed (exit $LASTEXITCODE)." }
    $cargoVersion = & $cargo -V
    if ($LASTEXITCODE -ne 0) { throw "cargo -V failed (exit $LASTEXITCODE)." }
    $hostTriple = ($rustcVv | Where-Object { $_ -like 'host:*' } | Select-Object -First 1) -replace '^host:\s*', ''
    if ($hostTriple -ne 'x86_64-pc-windows-msvc') { throw "Expected host x86_64-pc-windows-msvc, got '$hostTriple'." }

    $fetchArgs = @('fetch', '--locked', '--manifest-path', $rootManifest)
    $editorBuildArgs = @('build', '--release', '--locked', '--offline', '--manifest-path', $editorManifest, '--bin', 'against-rome-editor', '--target-dir', $editorTargetDir)
    $modifierGuiBuildArgs = @('build', '--release', '--locked', '--offline', '--manifest-path', $editorManifest, '--bin', 'against-rome-modifier-gui', '--target-dir', $editorTargetDir)
    $launcherBuildArgs = @('build', '--release', '--locked', '--offline', '--manifest-path', $editorManifest, '--bin', 'against-rome-launcher', '--target-dir', $editorTargetDir)
    $saveManagerBuildArgs = @('build', '--release', '--locked', '--offline', '--manifest-path', $editorManifest, '--bin', 'save-manager', '--target-dir', $editorTargetDir)
    $modifierBuildArgs = @('build', '--release', '--locked', '--offline', '--manifest-path', $modifierManifest, '--bin', 'against-rome-modifier', '--target-dir', $modifierTargetDir)
    $packagingBuildArgs = @('build', '--release', '--locked', '--offline', '--manifest-path', $packagingManifest, '--bin', 'ArmMapPackage', '--target-dir', $packagingTargetDir)

    $traceBuildArgs = @('build', '--release', '--locked', '--offline', '--manifest-path', $traceManifest, '--lib', '--target-dir', $traceTargetDir, '--target', $traceTargetTriple)

    # Explicit targets override CARGO_BUILD_TARGET and Cargo config; use the corresponding output layout.
    $editorBuildArgs += @('--target', $targetTriple)
    $modifierGuiBuildArgs += @('--target', $targetTriple)
    $launcherBuildArgs += @('--target', $targetTriple)
    $saveManagerBuildArgs += @('--target', $targetTriple)
    $modifierBuildArgs += @('--target', $targetTriple)
    $packagingBuildArgs += @('--target', $targetTriple)
    if (!$SkipBuild) {
        $editorExe = Join-Path $editorTargetDir "$targetTriple/release/$editorExeName"
        $modifierGuiExe = Join-Path $editorTargetDir "$targetTriple/release/$modifierGuiExeName"
        $launcherExe = Join-Path $editorTargetDir "$targetTriple/release/$launcherExeName"
        $saveManagerExe = Join-Path $editorTargetDir "$targetTriple/release/$saveManagerExeName"
        $modifierExe = Join-Path $modifierTargetDir "$targetTriple/release/$modifierExeName"
        $packagingExe = Join-Path $packagingTargetDir "$targetTriple/release/$packagingExeName"
    }

    $stagingName = "against-rome-editor-$version-win-x64-$($commit.Substring(0, 8))" +
        $(if ($dirty) { '-dirty' } else { '' }) + $(if ($SkipBuild) { '-skipbuild-unverified' } else { '' })
    $staging = Join-Path $outputRoot $stagingName
    $archivePath = Join-Path $outputRoot "$stagingName.zip"
    $checksumPath = "$archivePath.sha256"
    if (Test-Path -LiteralPath $staging) { throw "Staging directory already exists; refusing to overwrite: $staging" }
    if (Test-Path -LiteralPath $archivePath) { throw "Archive already exists; refusing to overwrite: $archivePath" }
    if (Test-Path -LiteralPath $checksumPath) { throw "Archive checksum already exists; refusing to overwrite: $checksumPath" }

    Write-Host "Repo commit : $commit (dirty=$dirty, $($state.ChangedPaths) changed paths, $($state.UntrackedCount) untracked)"
    Write-Host "Source state: $($state.SourceStateSha256)"
    Write-Host "Toolchain   : $($rustcVv[0]); host $hostTriple"
    Write-Host "Cargo Home  : $env:CARGO_HOME"
    Write-Host "CRT linkage : $(if ($SkipBuild) { 'unverifiable (-SkipBuild)' } else { 'static (-C target-feature=+crt-static; caller flags retained)' })"
    Write-Host "Targets     : $editorExeName (desktop-gui), $modifierGuiExeName (desktop-gui), $launcherExeName (desktop-gui), $saveManagerExeName (desktop-gui), $modifierExeName (cli), $packagingExeName (cli)"
    Write-Host "Fetch       : cargo $($fetchArgs -join ' ')$(if ($SkipBuild) { ' [skipped; exe provenance UNVERIFIABLE]' } elseif ($Offline) { ' [skipped; -Offline]' })"
    Write-Host "Build       : cargo $($editorBuildArgs -join ' ')$(if ($SkipBuild) { ' [skipped; exe provenance UNVERIFIABLE]' })"
    Write-Host "              cargo $($modifierGuiBuildArgs -join ' ')$(if ($SkipBuild) { ' [skipped; exe provenance UNVERIFIABLE]' })"
    Write-Host "              cargo $($launcherBuildArgs -join ' ')$(if ($SkipBuild) { ' [skipped; exe provenance UNVERIFIABLE]' })"
    Write-Host "              cargo $($saveManagerBuildArgs -join ' ')$(if ($SkipBuild) { ' [skipped; exe provenance UNVERIFIABLE]' })"
    Write-Host "              cargo $($modifierBuildArgs -join ' ')$(if ($SkipBuild) { ' [skipped; exe provenance UNVERIFIABLE]' })"
    Write-Host "              cargo $($packagingBuildArgs -join ' ')$(if ($SkipBuild) { ' [skipped; exe provenance UNVERIFIABLE]' })"
    Write-Host "              CARGO_PROFILE_RELEASE_PANIC=abort cargo $($traceBuildArgs -join ' ')$(if ($SkipBuild) { ' [skipped; exe provenance UNVERIFIABLE]' })"
    Write-Host "Staging     : $staging"
    Write-Host "ZIP         : $archivePath"
    Write-Host "Checksum    : $checksumPath"

    if (!$PSCmdlet.ShouldProcess("$staging; $archivePath; $checksumPath", 'Build, stage, and package portable Rust distribution (six executables + runtime assets)')) {
        Write-Host 'Dry run: nothing built, nothing written.'
        return
    }
    if ($SkipBuild) {
        Write-Warning '-SkipBuild: the existing exes are not verified to correspond to the current commit/working tree.'
    }
    # Cargo gives encoded flags precedence. Append our CRT requirement without dropping caller flags,
    # and restore both variables even on failure. Windows 10/11 system DLLs remain OS prerequisites.
    if (!$SkipBuild) {
        if ($env:CARGO_ENCODED_RUSTFLAGS) {
            $env:CARGO_ENCODED_RUSTFLAGS += [char]31 + '-C' + [char]31 + 'target-feature=+crt-static'
        } else {
            $env:RUSTFLAGS = ($env:RUSTFLAGS + ' -C target-feature=+crt-static').Trim()
        }
    }

    $buildStart = Get-Date
    $createdDirs = [Collections.Generic.List[string]]::new()   # directories created by THIS run, in creation order
    $createdFiles = [Collections.Generic.List[object]]::new()  # archive/checksum files created by THIS run
    $completed = $false
    $archiveWorkDir = $null
    try {
        if (!$SkipBuild) {
            if (!$Offline) {
                & $cargo @fetchArgs
                if ($LASTEXITCODE -ne 0) { throw "cargo fetch failed (exit $LASTEXITCODE)." }
            }
            & $cargo @editorBuildArgs
            if ($LASTEXITCODE -ne 0) { throw "cargo build ($editorExeName) failed (exit $LASTEXITCODE)." }
            & $cargo @modifierGuiBuildArgs
            if ($LASTEXITCODE -ne 0) { throw "cargo build ($modifierGuiExeName) failed (exit $LASTEXITCODE)." }
            & $cargo @launcherBuildArgs
            if ($LASTEXITCODE -ne 0) { throw "cargo build ($launcherExeName) failed (exit $LASTEXITCODE)." }
            & $cargo @saveManagerBuildArgs
            if ($LASTEXITCODE -ne 0) { throw "cargo build ($saveManagerExeName) failed (exit $LASTEXITCODE)." }
            & $cargo @modifierBuildArgs
            if ($LASTEXITCODE -ne 0) { throw "cargo build ($modifierExeName) failed (exit $LASTEXITCODE)." }
            & $cargo @packagingBuildArgs
            if ($LASTEXITCODE -ne 0) { throw "cargo build ($packagingExeName) failed (exit $LASTEXITCODE)." }
            # Workspace i686 cdylib uses the root Cargo.lock. Abort applies only to this DLL build.
            $previousReleasePanic = $env:CARGO_PROFILE_RELEASE_PANIC
            try {
                $env:CARGO_PROFILE_RELEASE_PANIC = 'abort'
                $traceCfgArgs = @('rustc') + $traceBuildArgs[1..($traceBuildArgs.Count - 1)] + @('--', '--print', 'cfg')
                $cfg = @(& $cargo @traceCfgArgs)
                if ($LASTEXITCODE -ne 0) { throw "Rust trace cfg probe failed (exit $LASTEXITCODE)." }
                if ($cfg -notcontains 'panic="abort"') { throw 'Trace release DLL requires panic="abort".' }
                & $cargo @traceBuildArgs
                if ($LASTEXITCODE -ne 0) { throw "cargo build (Rust trace $traceDllName, $traceTargetTriple) failed (exit $LASTEXITCODE)." }
            } finally {
                $env:CARGO_PROFILE_RELEASE_PANIC = $previousReleasePanic
            }
        } else {
            # Prefer explicit-target outputs produced by this publisher, retaining legacy lookup compatibility.
            foreach ($entry in @(
                @('editorExe', $editorTargetDir, $editorExeName),
                @('modifierGuiExe', $editorTargetDir, $modifierGuiExeName),
                @('launcherExe', $editorTargetDir, $launcherExeName),
                @('saveManagerExe', $editorTargetDir, $saveManagerExeName),
                @('modifierExe', $modifierTargetDir, $modifierExeName),
                @('packagingExe', $packagingTargetDir, $packagingExeName))) {
                $candidate = Join-Path $entry[1] "$targetTriple/release/$($entry[2])"
                if (Test-Path -LiteralPath $candidate -PathType Leaf) { Set-Variable -Name $entry[0] -Value $candidate }
            }
            # In -SkipBuild, allow discovering the binary if pre-built in another known target directory
            if (!(Test-Path -LiteralPath $editorExe -PathType Leaf)) {
                foreach ($cand in @(
                    (Join-Path $repo "re_workspace/target-against-rome-desktop/release/$editorExeName"),
                    (Join-Path $repo "re_workspace/target-portable-suite-check/release/$editorExeName")
                )) {
                    if (Test-Path -LiteralPath $cand -PathType Leaf) { $editorExe = $cand; break }
                }
            }
            if (!(Test-Path -LiteralPath $modifierGuiExe -PathType Leaf)) {
                foreach ($cand in @(
                    (Join-Path $repo "re_workspace/target-against-rome-desktop/release/$modifierGuiExeName"),
                    (Join-Path $repo "re_workspace/target-portable-suite-check/release/$modifierGuiExeName")
                )) {
                    if (Test-Path -LiteralPath $cand -PathType Leaf) { $modifierGuiExe = $cand; break }
                }
            }
            if (!(Test-Path -LiteralPath $launcherExe -PathType Leaf)) {
                foreach ($cand in @(
                    (Join-Path $repo "re_workspace/target-against-rome-desktop/release/$launcherExeName"),
                    (Join-Path $repo "re_workspace/target-portable-suite-check/release/$launcherExeName")
                )) {
                    if (Test-Path -LiteralPath $cand -PathType Leaf) { $launcherExe = $cand; break }
                }
            }
            if (!(Test-Path -LiteralPath $saveManagerExe -PathType Leaf)) {
                foreach ($cand in @(
                    (Join-Path $repo "re_workspace/target-against-rome-desktop/release/$saveManagerExeName"),
                    (Join-Path $repo "re_workspace/target-portable-suite-check/release/$saveManagerExeName")
                )) {
                    if (Test-Path -LiteralPath $cand -PathType Leaf) { $saveManagerExe = $cand; break }
                }
            }
            if (!(Test-Path -LiteralPath $modifierExe -PathType Leaf)) {
                foreach ($cand in @(
                    (Join-Path $repo "re_workspace/rust-rewrite-targets/native-against-rome-modifier/release/$modifierExeName"),
                    (Join-Path $repo "re_workspace/target-against-rome-modifier/release/$modifierExeName")
                )) {
                    if (Test-Path -LiteralPath $cand -PathType Leaf) { $modifierExe = $cand; break }
                }
            }
            if (!(Test-Path -LiteralPath $packagingExe -PathType Leaf)) {
                foreach ($cand in @(
                    (Join-Path $repo "re_workspace/rust-rewrite-targets/native-against-rome-packaging/release/$packagingExeName"),
                    (Join-Path $repo "re_workspace/target-against-rome-packaging/release/$packagingExeName")
                )) {
                    if (Test-Path -LiteralPath $cand -PathType Leaf) { $packagingExe = $cand; break }
                }
            }
            if (!(Test-Path -LiteralPath $traceDll -PathType Leaf)) {
                foreach ($cand in @(
                    (Join-Path $repo "re_workspace/target-against-rome-trace/$traceTargetTriple/release/$traceDllName")
                )) {
                    if (Test-Path -LiteralPath $cand -PathType Leaf) { $traceDll = $cand; break }
                }
            }
        }
        if (!(Test-Path -LiteralPath $editorExe -PathType Leaf)) { throw "Release editor exe not found: $editorExe" }
        if (!(Test-Path -LiteralPath $modifierGuiExe -PathType Leaf)) { throw "Release modifier GUI exe not found: $modifierGuiExe" }
        if (!(Test-Path -LiteralPath $launcherExe -PathType Leaf)) { throw "Release launcher exe not found: $launcherExe" }
        if (!(Test-Path -LiteralPath $saveManagerExe -PathType Leaf)) { throw "Release save manager exe not found: $saveManagerExe" }
        if (!(Test-Path -LiteralPath $modifierExe -PathType Leaf)) { throw "Release modifier exe not found: $modifierExe" }
        if (!(Test-Path -LiteralPath $packagingExe -PathType Leaf)) { throw "Release packaging exe not found: $packagingExe" }
        if (!(Test-Path -LiteralPath $traceDll -PathType Leaf)) { throw "Rust trace DLL not found (build output required, no C++ fallback): $traceDll" }
        if (!$SkipBuild) {
            if ((Get-Item -LiteralPath $traceDll).LastWriteTime -lt $buildStart.AddSeconds(-1)) {
                # Cargo left an older dll untouched (up to date) -> still valid, but make that visible.
                Write-Warning 'Rust trace DLL predates this build (cargo reported it up to date).'
            }
            if ((Get-Item -LiteralPath $editorExe).LastWriteTime -lt $buildStart.AddSeconds(-1)) {
                # Cargo left an older exe untouched (up to date) -> still valid, but make that visible.
                Write-Warning 'Release editor exe predates this build (cargo reported it up to date).'
            }
            if ((Get-Item -LiteralPath $modifierGuiExe).LastWriteTime -lt $buildStart.AddSeconds(-1)) {
                Write-Warning 'Release modifier GUI exe predates this build (cargo reported it up to date).'
            }
            if ((Get-Item -LiteralPath $launcherExe).LastWriteTime -lt $buildStart.AddSeconds(-1)) {
                Write-Warning 'Release launcher exe predates this build (cargo reported it up to date).'
            }
            if ((Get-Item -LiteralPath $saveManagerExe).LastWriteTime -lt $buildStart.AddSeconds(-1)) {
                Write-Warning 'Release save manager exe predates this build (cargo reported it up to date).'
            }
            if ((Get-Item -LiteralPath $modifierExe).LastWriteTime -lt $buildStart.AddSeconds(-1)) {
                Write-Warning 'Release modifier exe predates this build (cargo reported it up to date).'
            }
            if ((Get-Item -LiteralPath $packagingExe).LastWriteTime -lt $buildStart.AddSeconds(-1)) {
                Write-Warning 'Release packaging exe predates this build (cargo reported it up to date).'
            }
        }
        Assert-SourceStateUnchanged $state 'during build'
        # These desktop apps currently retain a console subsystem for help/headless smoke output.
        foreach ($exe in @($editorExe, $modifierGuiExe, $launcherExe, $saveManagerExe)) { Assert-NativeWindowsExe $exe @(2, 3) }
        foreach ($exe in @($modifierExe, $packagingExe)) { Assert-NativeWindowsExe $exe 3 }
        Assert-NativeWindowsDll32 $traceDll
        $traceDllHash = Get-Sha256 $traceDll
        $editorExeHash = Get-Sha256 $editorExe
        $modifierGuiExeHash = Get-Sha256 $modifierGuiExe
        $launcherExeHash = Get-Sha256 $launcherExe
        $saveManagerExeHash = Get-Sha256 $saveManagerExe
        $modifierExeHash = Get-Sha256 $modifierExe
        $packagingExeHash = Get-Sha256 $packagingExe

        # Create each missing ancestor of the staging dir one level at a time (top-down), recording only what we create.
        $missing = [Collections.Generic.Stack[string]]::new()
        $probe = $staging
        while (!(Test-Path -LiteralPath $probe)) {
            $missing.Push($probe)
            $probe = Split-Path $probe -Parent
            if (!$probe) { throw "Cannot resolve an existing ancestor for: $staging" }
        }
        while ($missing.Count -gt 0) {
            $next = $missing.Pop()
            New-Item -ItemType Directory -Path $next | Out-Null   # no -Force: fails if it appeared meanwhile
            $createdDirs.Add($next)
        }

        Copy-Item -LiteralPath $editorExe -Destination (Join-Path $staging $editorExeName)
        Copy-Item -LiteralPath $modifierGuiExe -Destination (Join-Path $staging $modifierGuiExeName)
        Copy-Item -LiteralPath $launcherExe -Destination (Join-Path $staging $launcherExeName)
        Copy-Item -LiteralPath $saveManagerExe -Destination (Join-Path $staging $saveManagerExeName)
        Copy-Item -LiteralPath $modifierExe -Destination (Join-Path $staging $modifierExeName)
        Copy-Item -LiteralPath $packagingExe -Destination (Join-Path $staging $packagingExeName)
        $files = [ordered]@{
            $editorExeName      = $editorExeHash
            $modifierGuiExeName = $modifierGuiExeHash
            $launcherExeName    = $launcherExeHash
            $saveManagerExeName = $saveManagerExeHash
            $modifierExeName    = $modifierExeHash
            $packagingExeName   = $packagingExeHash
        }
        $traceDest = Join-Path $staging $traceStagedRelative
        New-Item -ItemType Directory -Path (Split-Path $traceDest -Parent) -Force | Out-Null   # inside our own staging dir
        Copy-Item -LiteralPath $traceDll -Destination $traceDest
        $files[$traceStagedRelative] = $traceDllHash
        foreach ($a in $assets) {
            $dest = Join-Path $staging $a
            New-Item -ItemType Directory -Path (Split-Path $dest -Parent) -Force | Out-Null   # inside our own staging dir
            Copy-Item -LiteralPath (Join-Path $repo $a) -Destination $dest
            $files[$a] = Get-Sha256 (Join-Path $repo $a)
        }

        $editorExeInfo = Get-Item -LiteralPath $editorExe
        $modifierGuiExeInfo = Get-Item -LiteralPath $modifierGuiExe
        $launcherExeInfo = Get-Item -LiteralPath $launcherExe
        $saveManagerExeInfo = Get-Item -LiteralPath $saveManagerExe
        $modifierExeInfo = Get-Item -LiteralPath $modifierExe
        $packagingExeInfo = Get-Item -LiteralPath $packagingExe
        $provenance = [ordered]@{
            schema          = 2
            product         = 'against-rome-suite'
            version         = $version
            platform        = 'win-x64'
            builtAtUtc      = [DateTime]::UtcNow.ToString('o')
            git             = [ordered]@{
                commit = $commit; dirty = $dirty; changedPaths = $state.ChangedPaths; untrackedCount = $state.UntrackedCount
                statusSha256 = $state.StatusSha256
                trackedDiffSha256 = $state.TrackedDiffSha256
                trackedDiffCommand = 'git diff HEAD --binary --no-ext-diff --no-textconv --no-renames'
                untrackedManifestSha256 = $state.UntrackedManifestSha256
                untrackedManifestFormat = 'per untracked non-ignored file sorted ordinal: "<sha256>  <path>\n"'
                sourceStateSha256 = $state.SourceStateSha256
                stableDuringRun = if ($SkipBuild) { 'checked-only-around-staging' } else { 'verified-before-and-after-build' }
            }
            exeProvenance   = if ($SkipBuild) { 'unverifiable' } else { 'built-by-this-run-from-recorded-source-state' }
            binaries        = [ordered]@{
                $editorExeName = [ordered]@{
                    source       = $editorExe
                    sha256       = $editorExeHash
                    lastWriteUtc = $editorExeInfo.LastWriteTimeUtc.ToString('o')
                    length       = $editorExeInfo.Length
                    cargoPackage = 'against-rome-editor-app'
                    cargoTarget  = 'against-rome-editor'
                    kind         = 'desktop-gui'
                }
                $modifierGuiExeName = [ordered]@{
                    source       = $modifierGuiExe
                    sha256       = $modifierGuiExeHash
                    lastWriteUtc = $modifierGuiExeInfo.LastWriteTimeUtc.ToString('o')
                    length       = $modifierGuiExeInfo.Length
                    cargoPackage = 'against-rome-editor-app'
                    cargoTarget  = 'against-rome-modifier-gui'
                    kind         = 'desktop-gui'
                }
                $launcherExeName = [ordered]@{
                    source       = $launcherExe
                    sha256       = $launcherExeHash
                    lastWriteUtc = $launcherExeInfo.LastWriteTimeUtc.ToString('o')
                    length       = $launcherExeInfo.Length
                    cargoPackage = 'against-rome-editor-app'
                    cargoTarget  = 'against-rome-launcher'
                    kind         = 'desktop-gui'
                }
                $saveManagerExeName = [ordered]@{
                    source       = $saveManagerExe
                    sha256       = $saveManagerExeHash
                    lastWriteUtc = $saveManagerExeInfo.LastWriteTimeUtc.ToString('o')
                    length       = $saveManagerExeInfo.Length
                    cargoPackage = 'against-rome-editor-app'
                    cargoTarget  = 'save-manager'
                    kind         = 'desktop-gui'
                }
                $modifierExeName = [ordered]@{
                    source       = $modifierExe
                    sha256       = $modifierExeHash
                    lastWriteUtc = $modifierExeInfo.LastWriteTimeUtc.ToString('o')
                    length       = $modifierExeInfo.Length
                    cargoPackage = 'against-rome-modifier'
                    cargoTarget  = 'against-rome-modifier'
                    kind         = 'cli'
                }
                $packagingExeName = [ordered]@{
                    source       = $packagingExe
                    sha256       = $packagingExeHash
                    lastWriteUtc = $packagingExeInfo.LastWriteTimeUtc.ToString('o')
                    length       = $packagingExeInfo.Length
                    cargoPackage = 'against-rome-packaging'
                    cargoTarget  = 'ArmMapPackage'
                    kind         = 'cli'
                }
            }
            traceDll        = [ordered]@{
                staged       = $traceStagedRelative
                source       = $traceDll
                sha256       = $traceDllHash
                cargoPackage = 'against-rome-trace'
                cargoManifest = 'native/against-rome-trace/Cargo.toml'
                target       = $traceTargetTriple
                profile      = 'release'
                panicStrategy = if ($SkipBuild) { 'unverifiable' } else { 'abort' }
                origin       = if ($SkipBuild) { 'existing-rust-build-output-unverifiable' } else { 'built-by-this-run-from-rust-crate' }
                buildCommand = if ($SkipBuild) { 'skipped (-SkipBuild)' } else { "CARGO_PROFILE_RELEASE_PANIC=abort cargo $($traceBuildArgs -join ' ')" }
                crtLinkage   = if ($SkipBuild) { 'unverifiable' } else { 'static' }
                cppSourceBinaryCopied = $false
            }
            exeFile         = [ordered]@{ lastWriteUtc = $editorExeInfo.LastWriteTimeUtc.ToString('o'); length = $editorExeInfo.Length }
            rust            = [ordered]@{
                cargo = $cargoVersion; rustc = $rustcVv[0]; host = $hostTriple; target = $targetTriple
                profile = 'release'; cargoHome = $env:CARGO_HOME
                crtLinkage = if ($SkipBuild) { 'unverifiable' } else { 'static' }
                rustFlags = if ($SkipBuild) { $null } else { $env:RUSTFLAGS }
                encodedRustFlags = if ($SkipBuild) { $null } else { $env:CARGO_ENCODED_RUSTFLAGS }
            }
            fetchCommand    = if ($SkipBuild) { 'skipped (-SkipBuild; existing release exes reused)' } elseif ($Offline) { 'skipped (-Offline; existing Cargo cache only)' } else { "cargo $($fetchArgs -join ' ')" }
            buildCommands   = if ($SkipBuild) { @('skipped (-SkipBuild; existing release exes reused)') } else { @(
                "cargo $($editorBuildArgs -join ' ')",
                "cargo $($modifierGuiBuildArgs -join ' ')",
                "cargo $($launcherBuildArgs -join ' ')",
                "cargo $($saveManagerBuildArgs -join ' ')",
                "cargo $($modifierBuildArgs -join ' ')",
                "cargo $($packagingBuildArgs -join ' ')"
            ) }
            buildCommand    = if ($SkipBuild) { 'skipped (-SkipBuild; existing release exes reused)' } else {
                "cargo $($editorBuildArgs -join ' '); cargo $($modifierGuiBuildArgs -join ' '); cargo $($launcherBuildArgs -join ' '); cargo $($saveManagerBuildArgs -join ' '); cargo $($modifierBuildArgs -join ' '); cargo $($packagingBuildArgs -join ' ')"
            }
            sourceExe       = $editorExe
            exeSha256       = $editorExeHash
            modifierGuiExeSha256 = $modifierGuiExeHash
            launcherExeSha256 = $launcherExeHash
            saveManagerExeSha256 = $saveManagerExeHash
            modifierExeSha256 = $modifierExeHash
            packagingExeSha256 = $packagingExeHash
            files           = $files
            notes           = $(if ($SkipBuild) { 'UNVERIFIABLE: -SkipBuild reused existing release exes; the git fields describe the source tree at staging time and do NOT claim to match the exes. ' } else { '' }) +
                'Staging only; not verified on a clean Windows machine. ThirdParty is discovered next to the exe by InstallResources::discover(). Contains against-rome-editor.exe (desktop UI), against-rome-modifier-gui.exe (standalone Modifier GUI), against-rome-launcher.exe (suite launcher), save-manager.exe (standalone Save Manager GUI), against-rome-modifier.exe (standalone CLI), and ArmMapPackage.exe (standalone map packaging CLI).'
        }
        $provenance | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $staging 'provenance.json') -Encoding utf8

        # Verify every copied file against the recorded hash (exes first).
        foreach ($name in $files.Keys) {
            $copied = Join-Path $staging $name
            if (!(Test-Path -LiteralPath $copied -PathType Leaf)) { throw "Staged file missing: $name" }
            if ((Get-Sha256 $copied) -ne $files[$name]) { throw "SHA256 mismatch after copy: $name" }
        }
        Assert-SourceStateUnchanged $state 'during staging'

        Import-Module (Join-Path $PSScriptRoot 'RustPortableArchive.psm1') -Force
        # Isolate the helper's partial files too (including failures inside CreateFromDirectory).
        $archiveWorkDir = Join-Path $outputRoot ('.rust-publish-' + [Guid]::NewGuid().ToString('N'))
        New-Item -ItemType Directory -Path $archiveWorkDir | Out-Null
        $createdDirs.Add($archiveWorkDir)
        $workArchive = Join-Path $archiveWorkDir ([IO.Path]::GetFileName($archivePath))
        $archiveResult = New-RustPortableArchive -SourceDirectory $staging -DestinationPath $workArchive -ChecksumPath "$workArchive.sha256"
        [IO.File]::Move($archiveResult.ArchivePath, $archivePath) # no replacement if a destination appeared meanwhile
        $createdFiles.Add([pscustomobject]@{ Path = $archivePath; Sha256 = $archiveResult.Sha256 })
        $checksumHash = Get-Sha256 $archiveResult.ChecksumPath
        [IO.File]::Move($archiveResult.ChecksumPath, $checksumPath)
        $createdFiles.Add([pscustomobject]@{ Path = $checksumPath; Sha256 = $checksumHash })
        $archiveResult.ArchivePath = $archivePath
        $archiveResult.ChecksumPath = $checksumPath
        Remove-Item -LiteralPath $archiveWorkDir
        Assert-SourceStateUnchanged $state 'during archive creation'

        $completed = $true
        Write-Host "Staged $($files.Count) files; editor SHA256 $editorExeHash; modifier GUI SHA256 $modifierGuiExeHash; launcher SHA256 $launcherExeHash; save manager SHA256 $saveManagerExeHash; modifier CLI SHA256 $modifierExeHash; packaging CLI SHA256 $packagingExeHash$(if ($SkipBuild) { ' (exe provenance UNVERIFIABLE)' })"
        Write-Host "Output: $staging"
        Write-Host "ZIP: $($archiveResult.ArchivePath) SHA256=$($archiveResult.Sha256) ($($archiveResult.FileCount) verified entries)"
        Write-Host "SHA256 file: $($archiveResult.ChecksumPath)"
    } finally {
        if (!$completed) {
            # Failure or pipeline stop: remove only unchanged files and directories created by this run.
            for ($i = $createdFiles.Count - 1; $i -ge 0; $i--) {
                $file = $createdFiles[$i]
                try {
                    if (!(Test-Path -LiteralPath $file.Path -PathType Leaf)) { continue }
                    if ((Get-Sha256 $file.Path) -eq $file.Sha256) {
                        Remove-Item -LiteralPath $file.Path -Force
                    } else {
                        Write-Warning "Leaving changed output file untouched during cleanup: $($file.Path)"
                    }
                } catch { Write-Warning "Cleanup of '$($file.Path)' failed: $($_.Exception.Message)" }
            }
            # Remove created directories deepest first; never recurse into pre-existing data.
            for ($i = $createdDirs.Count - 1; $i -ge 0; $i--) {
                $d = $createdDirs[$i]
                try {
                    if (!(Test-Path -LiteralPath $d -PathType Container)) { continue }
                    if ($d -eq $staging -or $d -eq $archiveWorkDir) {
                        Remove-Item -LiteralPath $d -Recurse -Force
                        Write-Warning "Removed incomplete staging directory created by this run: $d"
                    } elseif (!(Get-ChildItem -LiteralPath $d -Force | Select-Object -First 1)) {
                        Remove-Item -LiteralPath $d -Force
                    } else { break }   # non-empty: someone else wrote here; leave it and all its parents
                } catch { Write-Warning "Cleanup of '$d' failed: $($_.Exception.Message)" }
            }
        }
    }
} finally {
    if ($null -ne $origCargoHome) { $env:CARGO_HOME = $origCargoHome } else { Remove-Item Env:\CARGO_HOME -ErrorAction SilentlyContinue }
    if ($null -ne $origRustupHome) { $env:RUSTUP_HOME = $origRustupHome } else { Remove-Item Env:\RUSTUP_HOME -ErrorAction SilentlyContinue }
    $env:PATH = $origPath
    $env:RUSTFLAGS = $origRustFlags
    $env:CARGO_ENCODED_RUSTFLAGS = $origEncodedRustFlags
    Pop-Location
}
