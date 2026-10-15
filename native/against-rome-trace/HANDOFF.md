# WP-08 — Codex, 2026-10-15

- Ownership: only `native/against-rome-trace/**`; no commits or installed-game writes.
- Read AGENTS.md, AI_HANDOFF.md, inventory section 5 and original C++ config,
  decoder, detour, targets, DLL lifecycle, logger, proxy and tests.
- Implemented dependency-free Rust config/profile reads, bounded x86 decoder,
  signature checks, trampoline/stub/patch emission, PE32 fingerprint/rebasing,
  all 11 target signatures/formatters, synchronized BOM logger, Win32 allocation/
  protection/restore, worker lifecycle, and generated winmm assembly forwarding.
- The copied export definition has 193 entries (192 named + ordinal 2 NONAME).
- Final host verification: **73 passed, 0 failed, 0 ignored** (47 original
  C++ LDE fixtures + 23 logic tests + 3 genuine Win32/file checks), no warnings.
  This checks FFI/runtime types, not x86 assembly/linker or executable detours.
- `rustup target add i686-pc-windows-msvc` failed downloading rust-std:
  network access denied, os error 10013. Only x86_64-pc-windows-msvc is installed.
  Use host tests and runtime compile checks; do not claim an x86 DLL build.
- Additional verification attempt: x64 system-winmm resolver test failed on
  `NotifyCallbackData`. Original generate_exports.py explicitly snapshots
  SysWOW64 (x86), so requiring every x86-only export from x64 winmm is invalid.
  Resolved the test's architecture error by parsing SysWOW64's PE export table:
  all 193 original names/ordinals/NONAME entries match. Full GetProcAddress
  resolution is retained as an x86-only test, alongside real detour execution.
- Explicit i686 cargo build fails E0463 (missing target std), as expected.
- Existing unrelated dirty work is preserved. Global handoff remains untouched
  because this work package permits writes only under this crate.

## Exact commands and remaining work

```powershell
$env:CARGO_HOME='D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\cargo'
$env:RUSTUP_HOME='D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\rustup'
$env:PATH="$env:CARGO_HOME\bin;$env:PATH"
cargo test --offline --manifest-path native/against-rome-trace/Cargo.toml --target-dir re_workspace/target-against-rome-trace --features check-runtime
cargo run --offline --manifest-path native/against-rome-trace/Cargo.toml --target-dir re_workspace/target-against-rome-trace --example audit_exports -- C:\Windows\SysWOW64\winmm.dll
cargo fmt --manifest-path native/against-rome-trace/Cargo.toml -- --check
```

- Export-auditor smoke: PASS, 193 exact x86 exports on genuine SysWOW64 DLL.
  This is not an audit of a built Rust DLL, which is still unavailable.
- Formatting check passes. No 4551 encountered in this crate; no policy changes.
- `cargo build --offline --manifest-path native/against-rome-trace/Cargo.toml
  --target-dir re_workspace/target-against-rome-trace --target
  i686-pc-windows-msvc`: FAIL E0463, missing target std; no production DLL.
- After target download is available, run `build-win32.ps1`: static-CRT x86
  tests/build, then exact export-table audit of the actual Rust DLL. The two
  x86-only tests (full resolver, executable detour/argument capture/restore)
  are not included in the 73 host passes and have not run here.
- x86 global assembly, CRT DllMain linkage and /DEF integration remain unverified.
  Run DLL proxy log-only smoke in a disposable TEMP game copy only, followed by
  explicitly enabled hooks with matching fingerprint; installed game stays read-only.
- See README for documented C++ safety corrections, inherited unused config
  options, and process-lifetime loading/unload/concurrent-patching limitations.
