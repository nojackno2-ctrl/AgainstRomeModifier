# ThirdParty/argm-trace

`winmm.dll` is the runtime flight-recorder proxy DLL (32-bit, because the game
is a 32-bit process). It is built from Rust.

## Rust implementation (packaged)

- Source: `native/against-rome-trace` (independent Cargo crate, `[lib] name =
  "winmm"`, `cdylib`, own `Cargo.toml`/`Cargo.lock`, not a member of the root
  workspace).
- `tools/publish-rust.ps1` builds it itself on every publish and stages the
  **build output** as `ThirdParty/argm-trace/winmm.dll` inside the portable
  folder/ZIP. It does not read a pre-existing target artifact (except with
  `-SkipBuild`, which marks provenance as unverifiable) and never copies
  any prebuilt binary. `provenance.json` records the `traceDll`
  SHA256, target, build command and `cppSourceBinaryCopied = false`.
- Rebuild by hand from the repo root (repo-local toolchain, static CRT,
  locked/offline, Release, 32-bit):

```powershell
$env:CARGO_HOME  = "$PWD\re_workspace\rust-runtime\cargo"
$env:RUSTUP_HOME = "$PWD\re_workspace\rust-runtime\rustup"
$env:PATH        = "$env:CARGO_HOME\bin;$env:PATH"
$env:RUSTFLAGS   = '-C target-feature=+crt-static'
cargo build --release --locked --offline --lib `
  --manifest-path native/against-rome-trace/Cargo.toml `
  --target-dir re_workspace/rust-rewrite-targets/native-against-rome-trace `
  --target i686-pc-windows-msvc
# output: re_workspace\rust-rewrite-targets\native-against-rome-trace\i686-pc-windows-msvc\release\winmm.dll
```

  Full tests plus export audit: `native/against-rome-trace/build-win32.ps1`.
- Verified by `tools/test-publish-rust.ps1`: Cargo builds the cdylib, the
  staged DLL is that build output, and it is a valid PE32 (i386, DLL, exports,
  no CLR header). Not verified there: behavior inside the real game or on a
  clean Windows machine.

## History

The original C++ recorder (`native/argm-trace`) and its prebuilt `winmm.dll`
were removed after the Rust crate reached parity (identical export table,
INI keys and LDE fixtures). This directory now only documents the packaged
Rust build output; no binary is checked in here.
