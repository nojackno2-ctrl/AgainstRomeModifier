<#
.SYNOPSIS
  Builds native/against-rome-data (release, --locked) and deploys against_rome_data.dll to an explicit directory.

.DESCRIPTION
  Toolchain: the repo-local Rust toolchain (re_workspace/rust-runtime, CARGO_HOME/RUSTUP_HOME set process-locally
  only) when present, otherwise a normally installed `cargo` found on PATH. Nothing global is modified.
  After the build the DLL is copied to -OutputDir (default re_workspace/native-data-deploy), the deployed bytes are
  verified against the built bytes by SHA-256, required exports are resolved from the loaded library, light and
  particle ABI/size queries are checked, and a manifest is written next to it.

  The deployed DLL contains Rust light.dat and particle.dat readers plus editor ALR scanline decoding; it is not a game
  file and the script never touches the game directory or ISO. C ABI consumers can use
  ARM_NATIVE_DATA_DLL=<OutputDir>\against_rome_data.dll, or place it beside the host executable.
#>
[CmdletBinding()]
param(
    [string]$OutputDir,
    [string]$TargetDir
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$crateDir = Join-Path $repoRoot 'native/against-rome-data'
$manifestPath = Join-Path $crateDir 'Cargo.toml'
if (-not (Test-Path -LiteralPath $manifestPath)) { throw "找不到 $manifestPath" }
if (-not (Test-Path -LiteralPath (Join-Path $crateDir 'Cargo.lock'))) { throw '缺少 Cargo.lock，--locked 無法使用。' }

function Resolve-Full([string]$path) { $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($path) }
if (-not $OutputDir) { $OutputDir = Join-Path $repoRoot 're_workspace/native-data-deploy' }
if (-not $TargetDir) { $TargetDir = Join-Path $repoRoot 're_workspace/against-rome-data-target' }
$OutputDir = Resolve-Full $OutputDir
$TargetDir = Resolve-Full $TargetDir
$builtDll = Join-Path $TargetDir 'release/against_rome_data.dll'
if ([IO.Path]::GetFullPath($OutputDir).TrimEnd('\') -eq [IO.Path]::GetFullPath((Split-Path $builtDll)).TrimEnd('\')) {
    throw '-OutputDir 不可是 cargo release 目錄本身；請指定獨立的部署目錄。'
}

# --- toolchain (process-local only) ---
$localCargoHome = Join-Path $repoRoot 're_workspace/rust-runtime/cargo'
$localRustupHome = Join-Path $repoRoot 're_workspace/rust-runtime/rustup'
$localCargo = Join-Path $localCargoHome 'bin/cargo.exe'
if ((Test-Path -LiteralPath $localCargo) -and (Test-Path -LiteralPath $localRustupHome)) {
    $cargo = $localCargo
    $env:CARGO_HOME = $localCargoHome
    $env:RUSTUP_HOME = $localRustupHome
    $env:PATH = (Join-Path $localCargoHome 'bin') + [IO.Path]::PathSeparator + $env:PATH   # this process only
    $toolchain = 'repo-local re_workspace/rust-runtime'
} else {
    $found = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $found) { throw '找不到 repo-local Rust（re_workspace/rust-runtime）也找不到已安裝的 cargo。' }
    $cargo = $found.Source
    $toolchain = "installed cargo ($cargo)"
}
Write-Host "toolchain: $toolchain"
$cargoVersion = (& $cargo --version) -join ' '
Write-Host $cargoVersion

# --- build ---
$lockHashBefore = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $crateDir 'Cargo.lock')).Hash
& $cargo build --release --locked --manifest-path $manifestPath --target-dir $TargetDir
if ($LASTEXITCODE -ne 0) { throw "cargo build --release --locked 失敗（exit $LASTEXITCODE）。" }
if (-not (Test-Path -LiteralPath $builtDll)) { throw "建置後找不到 $builtDll" }
$lockHashAfter = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $crateDir 'Cargo.lock')).Hash
if ($lockHashBefore -ne $lockHashAfter) { throw 'Cargo.lock 在建置中被改動。' }

# --- verify the built artifact ---
$bytes = [IO.File]::ReadAllBytes($builtDll)
if ($bytes.Length -lt 0x100 -or $bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) { throw '輸出不是 PE 檔。' }
$peOffset = [BitConverter]::ToInt32($bytes, 0x3C)
if ([BitConverter]::ToUInt32($bytes, $peOffset) -ne 0x00004550) { throw 'PE signature 不符。' }
$machine = [BitConverter]::ToUInt16($bytes, $peOffset + 4)
$characteristics = [BitConverter]::ToUInt16($bytes, $peOffset + 22)
if ($machine -ne 0x8664) { throw ('DLL 不是 x64（machine=0x{0:X4}）。' -f $machine) }
if (($characteristics -band 0x2000) -eq 0) { throw 'PE 沒有 DLL 旗標。' }
$text = [Text.Encoding]::ASCII.GetString($bytes)
$exports = 'ar_light_abi_version', 'ar_light_snapshot_struct_size', 'ar_light_error_message', 'ar_light_pool_open',
    'ar_light_pool_free', 'ar_light_pool_active_count', 'ar_light_pool_snapshot', 'ar_light_pool_snapshot_any',
    'ar_light_pool_copy_payload', 'ar_light_pool_copy_pfil_header',
    'ar_particle_pool_abi_version', 'ar_particle_pool_snapshot_struct_size', 'ar_particle_pool_record_size',
    'ar_particle_pool_state_size', 'ar_particle_pool_child_size', 'ar_particle_pool_payload_size',
    'ar_particle_pool_error_message', 'ar_particle_pool_open', 'ar_particle_pool_free',
    'ar_particle_pool_active_count', 'ar_particle_pool_snapshot', 'ar_particle_pool_snapshot_any',
    'ar_particle_pool_child_any', 'ar_particle_pool_validate_renderable_count',
    'ar_particle_pool_copy_payload', 'ar_particle_pool_copy_pfil_header', 'ar_particle_pool_copy_input_sha256',
    'ar_particle_pixel_abi_version', 'ar_particle_pixel_row',
    'ar_alr_scanline_abi_version', 'ar_alr_decode_argb',
    'ar_particle_definition_abi_version', 'ar_particle_definition_struct_size',
    'ar_particle_definition_open', 'ar_particle_definition_free', 'ar_particle_definition_query',
    'ar_particle_definition_load_utf8', 'ar_particle_definition_source_count',
    'ar_particle_definition_lifetime_frame', 'ar_particle_definition_saved_frames',
    'ar_particle_frame_candidate_struct_size',
    'ar_particle_definition_saved_frames_translated',
    'ar_particle_saved_object_mode', 'ar_particle_saved_object_mode_struct_size',
    'ar_particle_definition_primary_mode'
foreach ($name in $exports) {
    if (-not $text.Contains($name + [char]0)) { throw "DLL 缺少匯出 $name" }
}

# --- deploy (temp file, then move) ---
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
$deployed = Join-Path $OutputDir 'against_rome_data.dll'
$temp = "$deployed.$PID.tmp"
Copy-Item -LiteralPath $builtDll -Destination $temp -Force
try {
    $builtHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $builtDll).Hash
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath $temp).Hash -ne $builtHash) { throw '複製後 SHA-256 與建置輸出不符。' }
    Move-Item -LiteralPath $temp -Destination $deployed -Force
} finally {
    if (Test-Path -LiteralPath $temp) { Remove-Item -LiteralPath $temp -Force }
}
if ((Get-FileHash -Algorithm SHA256 -LiteralPath $deployed).Hash -ne $builtHash) { throw '部署後 SHA-256 不符。' }
Copy-Item -LiteralPath (Join-Path $crateDir 'include/against_rome_data.h') -Destination (Join-Path $OutputDir 'against_rome_data.h') -Force

# --- Rust-only explicit-path ABI audit; no managed Add-Type probe ---
$probe = Join-Path $TargetDir 'release/ar-native-probe.exe'
if (-not (Test-Path -LiteralPath $probe)) { throw "建置後找不到 $probe" }
& $probe $deployed
if ($LASTEXITCODE -ne 0) { throw "Rust native ABI probe failed (exit $LASTEXITCODE)." }
# These manifest values are emitted only after the Rust executable verifies each query.
$abi = 1, 120, 1, 24, 2252, 16, 35, 2322444
# --- manifest ---
$manifest = [ordered]@{
    purpose       = 'against_rome_data: saved pools, particle pixels, ALR scanlines and base/effective definition snapshots (C ABI v1)'
    dll           = 'against_rome_data.dll'
    sha256        = $builtHash
    size          = (Get-Item -LiteralPath $deployed).Length
    abi_version   = $abi[0]
    snapshot_struct_size = $abi[1]
    particle_abi_version = $abi[2]
    particle_snapshot_struct_size = $abi[3]
    particle_record_size = $abi[4]
    particle_state_size = $abi[5]
    particle_child_size = $abi[6]
    particle_payload_size = $abi[7]
    particle_pixel_abi_version = 1
    alr_scanline_abi_version = 1
    particle_definition_abi_version = 1
    particle_definition_struct_size = 44
    particle_pixel_dll_smoke = 'half-average + skip-zero + capacity tail + atomic invalid index passed'
    built_utc     = [DateTime]::UtcNow.ToString('o')
    cargo         = $cargoVersion
    toolchain     = $toolchain
    cargo_lock_sha256 = $lockHashAfter
    command       = 'cargo build --release --locked'
    source_dll    = $builtDll
}
$manifest | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $OutputDir 'native-data-manifest.json') -Encoding UTF8

Write-Host "deployed: $deployed"
Write-Host "sha256:   $builtHash"
Write-Host "set ARM_NATIVE_DATA_DLL=$deployed"
