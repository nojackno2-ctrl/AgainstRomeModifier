# ThirdParty/argm-trace

`winmm.dll` is the runtime flight-recorder proxy DLL (32-bit, because the game
is a 32-bit process). There are two implementations; only the Rust one is
packaged by the Rust portable publisher.

## Rust implementation (packaged)

- Source: `native/against-rome-trace` (independent Cargo crate, `[lib] name =
  "winmm"`, `cdylib`, own `Cargo.toml`/`Cargo.lock`, not a member of the root
  workspace).
- `tools/publish-rust.ps1` builds it itself on every publish and stages the
  **build output** as `ThirdParty/argm-trace/winmm.dll` inside the portable
  folder/ZIP. It does not read a pre-existing target artifact (except with
  `-SkipBuild`, which marks provenance as unverifiable) and never copies the
  C++ binary kept in this directory. `provenance.json` records the `traceDll`
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

## C++ implementation (retained oracle, not packaged by the Rust publisher)

`winmm.dll` in this directory is the **prebuilt binary of the legacy C++
oracle** `native/argm-trace` (MSVC/Win32). It stays checked in as the parity
reference and so the legacy .NET build can still embed it
(`src.Core/AgainstRome.Core.csproj` as `argm-trace.winmm.dll`, deployed by
`ArgmTraceFeature` when "Enable Gameplay Trace Logger" is applied), mirroring
how `ThirdParty/dgVoodoo2` ships prebuilt DLLs. Do not replace it with the Rust
build output. The Rust publisher never copies it.

Rebuild (VS C++ workload required), only when `native/argm-trace` changes:

```powershell
cmake -S native/argm-trace -B build/argm-trace -A Win32 -DARGM_BUILD_TESTS=ON
cmake --build build/argm-trace --config Release
copy build\argm-trace\Release\winmm.dll ThirdParty\argm-trace\winmm.dll
```

Then rebuild the .NET solution so the new binary is re-embedded. Keep this copy
in sync with `native/argm-trace` whenever that source changes.
