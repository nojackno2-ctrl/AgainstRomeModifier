$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$env:CARGO_HOME = Join-Path $repo 're_workspace/rust-runtime/cargo'
$env:RUSTUP_HOME = Join-Path $repo 're_workspace/rust-runtime/rustup'
$env:PATH = "$env:CARGO_HOME\bin;$env:PATH"
# Process-local x86 setting only; retain the original static-CRT deployment model.
$env:CARGO_TARGET_I686_PC_WINDOWS_MSVC_RUSTFLAGS = '-C target-feature=+crt-static'
$manifest = Join-Path $PSScriptRoot 'Cargo.toml'
$target = Join-Path $repo 're_workspace/target-against-rome-trace'
& cargo test --offline --manifest-path $manifest --target-dir $target --target i686-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { throw "x86 tests failed (exit $LASTEXITCODE). Do not bypass app-control error 4551." }
# Workspace profiles own panic strategy; scope abort to the shipped DLL, never cargo test.
$previousReleasePanic = $env:CARGO_PROFILE_RELEASE_PANIC
try {
    $env:CARGO_PROFILE_RELEASE_PANIC = 'abort'
    $cfg = @(& cargo rustc --offline --release --manifest-path $manifest --target-dir $target --target i686-pc-windows-msvc --lib -- --print cfg)
    if ($LASTEXITCODE -ne 0) { throw "x86 DLL cfg probe failed (exit $LASTEXITCODE)." }
    if ($cfg -notcontains 'panic="abort"') { throw 'Trace release DLL requires panic="abort".' }
    & cargo build --offline --release --manifest-path $manifest --target-dir $target --target i686-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw "x86 DLL build failed (exit $LASTEXITCODE)." }
} finally {
    $env:CARGO_PROFILE_RELEASE_PANIC = $previousReleasePanic
}
$dll = Join-Path $target 'i686-pc-windows-msvc/release/winmm.dll'
& cargo run --offline --manifest-path $manifest --target-dir $target --example audit_exports -- $dll
if ($LASTEXITCODE -ne 0) { throw "DLL export audit failed (exit $LASTEXITCODE)." }
Write-Output "Verified DLL: $dll"
