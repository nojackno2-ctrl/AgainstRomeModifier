# WP-08 — Antigravity, 2026-10-18

## Parent re-verification after lifecycle fixes — Codex, 2026-10-18

- `cargo test --release --locked --offline --manifest-path native/against-rome-trace/Cargo.toml --target-dir re_workspace/target-against-rome-trace --features check-runtime`: x64 host **77 passed**.
- Same crate on `i686-pc-windows-msvc`: **80 passed, 0 failed, 0 ignored**, including two genuine x86 detour tests.
- `pwsh -NoProfile -ExecutionPolicy Bypass -File native/against-rome-trace/build-win32.ps1`: x86 suite passed, static-CRT release `winmm.dll` built, and export audit passed for **193 exact names/ordinals/NONAME**.
- No DLL deployment or real-game process/hook test was performed; installed game files remain unchanged.

- Ownership: strictly `native/against-rome-trace/**` and scoped `re_workspace` report; no commits/pushes, no AI_HANDOFF.md edits, and no installed-game writes.
- Installed standard `i686-pc-windows-msvc` target via process-local rustup (`rustup target add i686-pc-windows-msvc`) without altering system security policy or bypassing Code Integrity (no 4551 errors).
- Configured process-local static CRT via `native/against-rome-trace/.cargo/config.toml` (`-C target-feature=+crt-static`) ensuring clean standalone deployment without VC++ runtime dependency.
- Implemented substantive missing functionality:
  1. `maxLogMegabytes` log capping in `TraceLog`: actively tracks byte output against configured limit, emits a warning notice before threshold, and cleanly suppresses subsequent writes to prevent disk fill during high-volume opcode tracing.
  2. Gated signature validation in `install` with `cfg.verifySignatures`.
  3. Added multi-argument real x86 detour test `real_x86_multi_arg_detour_preserves_regs_captures_all_args_and_restores` mimicking `s_addNPCJob_createUnit` (9 stack arguments, return preservation, register preservation, multi-call capture, and clean restoration).
  4. Added `TEST_LOCK` serialization to eliminate concurrent test race conditions on shared static test hooks.
- Verification results:
  - x64 host tests: **74 passed, 0 failed, 0 ignored** (3 unit, 47 LDE, 24 logic).
  - i686-pc-windows-msvc tests: **77 passed, 0 failed, 0 ignored** (6 unit including 2 genuine x86 detour tests, 47 LDE, 24 logic).
  - Release DLL build (`i686-pc-windows-msvc`): static-CRT `winmm.dll` built successfully at `re_workspace/target-against-rome-trace/i686-pc-windows-msvc/release/winmm.dll`.
  - Export audit (`audit_exports`): **193 exports verified**, exact names, ordinals, and NONAME parity against `winmm.def` and genuine SysWOW64 `winmm.dll`.
  - Formatting: `cargo fmt --check` clean.

## Exact commands

```powershell
$env:CARGO_HOME='D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\cargo'
$env:RUSTUP_HOME='D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\rustup'
$env:PATH="$env:CARGO_HOME\bin;$env:PATH"

# 1. Host x64 tests
cargo test --offline --manifest-path native/against-rome-trace/Cargo.toml --target-dir re_workspace/target-against-rome-trace --features check-runtime

# 2. i686 Win32 build, test & audit via build-win32.ps1
pwsh -File native/against-rome-trace/build-win32.ps1

# 3. Direct i686 test and release build
cargo test --offline --manifest-path native/against-rome-trace/Cargo.toml --target-dir re_workspace/target-against-rome-trace --target i686-pc-windows-msvc
cargo build --offline --release --manifest-path native/against-rome-trace/Cargo.toml --target-dir re_workspace/target-against-rome-trace --target i686-pc-windows-msvc
cargo run --offline --manifest-path native/against-rome-trace/Cargo.toml --target-dir re_workspace/target-against-rome-trace --example audit_exports -- re_workspace/target-against-rome-trace/i686-pc-windows-msvc/release/winmm.dll
cargo fmt --manifest-path native/against-rome-trace/Cargo.toml -- --check
```

## Lifecycle safety audit (Claude, 2026-10-18)

- Failure modes found in `src/runtime.rs`:
  1. `remove_all` silently did nothing if `HOOKS` was poisoned, leaving detours patched into code that detach then unmaps/frees.
  2. The async `init` thread could call `install` after `DLL_PROCESS_DETACH` had run `remove_all`, re-patching the game with stubs in a dying module.
- Fix: `HOOKS` lock poison is recovered (`into_inner`) in `install`/`remove_all`; new `SHUTDOWN` flag set by `shutdown()` (called on detach) makes later `install` calls return false. `remove_all` stays repeatable.
- Regression tests (run on the x64 host, using real VirtualAlloc'd fake hooks): `remove_all_restores_bytes_even_when_hook_list_is_poisoned`, `install_after_shutdown_is_refused_and_shutdown_is_repeatable`. `TEST_LOCK` is now `cfg(test)` for all targets.
- Audited and left unchanged: install failure paths free tramp/stub; a failed `VirtualProtect` in `remove_all` keeps the entry for retry; re-installing an already patched address is rejected by the relative-jmp check.
- Superseded by the parent re-verification above: i686 release tests and DLL export audit now pass; real-game process/hook execution remains unverified.
