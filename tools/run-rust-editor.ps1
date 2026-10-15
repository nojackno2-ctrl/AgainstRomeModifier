[CmdletBinding()]
param([string]$Map, [switch]$Release, [switch]$SmokeTest, [switch]$ValidateMap)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if ($ValidateMap -and (!$Map -or $SmokeTest)) {
    throw '-ValidateMap requires -Map and cannot combine with -SmokeTest.'
}
$repo = Split-Path $PSScriptRoot -Parent
$runtime = Join-Path $repo 're_workspace/rust-runtime'
$cargo = Join-Path $runtime 'cargo/bin/cargo.exe'
if (Test-Path -LiteralPath $cargo) {
    $env:CARGO_HOME = Join-Path $runtime 'cargo'
    $env:RUSTUP_HOME = Join-Path $runtime 'rustup'
    $env:PATH = (Join-Path $env:CARGO_HOME 'bin') + [IO.Path]::PathSeparator + $env:PATH
} else {
    $cargo = (Get-Command cargo -ErrorAction Stop).Source
}
$target = Join-Path $repo 're_workspace/rust-editor-app-target'
$arguments = @('build', '--locked', '--manifest-path',
    (Join-Path $repo 'apps/against-rome-editor/Cargo.toml'), '--target-dir', $target)
$profile = 'debug'
if ($Release) { $arguments += '--release'; $profile = 'release' }
& $cargo @arguments
if ($LASTEXITCODE -ne 0) { throw "Rust desktop build failed (exit $LASTEXITCODE)." }
$application = Join-Path $target "$profile/against-rome-editor.exe"
$applicationArgs = @()
if ($Map) { $applicationArgs += @('--map', $Map) }
if ($SmokeTest) { $applicationArgs += '--smoke-test' }
if ($ValidateMap) { $applicationArgs += '--validate-map' }
& $application @applicationArgs
if ($LASTEXITCODE -ne 0) { throw "Rust desktop failed (exit $LASTEXITCODE)." }
