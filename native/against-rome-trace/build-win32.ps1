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
& cargo build --offline --release --manifest-path $manifest --target-dir $target --target i686-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { throw "x86 DLL build failed (exit $LASTEXITCODE)." }
$dll = Join-Path $target 'i686-pc-windows-msvc/release/winmm.dll'
& cargo run --offline --manifest-path $manifest --target-dir $target --example audit_exports -- $dll
if ($LASTEXITCODE -ne 0) { throw "DLL export audit failed (exit $LASTEXITCODE)." }
Write-Output "Verified DLL: $dll"
