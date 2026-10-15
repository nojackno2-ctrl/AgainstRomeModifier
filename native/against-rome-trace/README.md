# against-rome-trace (WP-08)

Rust port of `native/argm-trace`, with no C++ or third-party Rust dependencies.
The production target is **i686-pc-windows-msvc**, output `winmm.dll`.
The x64 library is only a host test artifact and must not be deployed.

Implemented: 193 winmm export thunks (192 named, ordinal 2 NONAME), INI config
using the original Win32 profile APIs, bounded x86 LDE, signature checks,
trampoline/stub generation, Win32 install/restore, PE32 build fingerprint and
ASLR rebasing, all eleven target signatures and event formatters, BOM trace log,
and DLL startup worker. Export thunks and DllMain use COFF assembly labels so
Rust helper names do not add entries to the public export table.

From the repository root, prepare the process-local toolchain:

```powershell
$env:CARGO_HOME='D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\cargo'
$env:RUSTUP_HOME='D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\rustup'
$env:PATH="$env:CARGO_HOME\bin;$env:PATH"
cargo test --offline --manifest-path native/against-rome-trace/Cargo.toml --target-dir re_workspace/target-against-rome-trace --features check-runtime
```

`check-runtime` type-checks Win32 integration on the x64 host and runs profile,
guarded-read and **on-disk SysWOW64 export parity** tests. It does not compile
the x86 assembler or execute an inline hook. SysWOW64 is required for that
Windows host test; absence is a failure rather than a silently skipped test.

When the i686 standard library and MSVC x86 tools are available:

```powershell
rustup target add i686-pc-windows-msvc
& native/against-rome-trace/build-win32.ps1
```

The script tests on x86, builds the release DLL with static CRT, then parses
the resulting PE export table and requires exact names, ordinals and NONAME
parity with the original definition. The x86-only mechanism test allocates a
synthetic function, installs a real detour, verifies two argument captures and
unchanged return values, then checks original bytes and execution after removal.
No script deploys into the installed game directory.

Behavior and deliberate safety differences from C++:

- `enableHooks=false` by default; `enabled=false` remains proxy-only. All
  address-based AI hooks require a nonzero matching timestamp. Both faction
  variants always require their original signatures.
- `verifySignatures`, `argDumpCount` and `maxLogMegabytes` are loaded exactly as
  in C++. The original installer always checks supplied signatures, formatters
  have fixed widths, and the original logger does not enforce the configured
  size cap. This port retains those behaviors; the size cap is not a guarantee.
- Bounded decoder rejects truncated input, instructions longer than the x86
  15-byte limit, and prefixed relative branches. C++ allowed up to 16 bytes and
  only checked unprefixed relative branches. All original target prologues and
  47 decoder fixtures remain accepted with identical lengths.
- `ReadProcessMemory` replaces MSVC SEH for fault-tolerant game-memory reads.
  Protection/allocation failures refuse installation before touching code;
  instruction caches are flushed for generated code as well as patched code.
  Proxy load fails if a required real export cannot be resolved.
- Log lines are bounded to 4095 bytes including newline without splitting a
  UTF-8 codepoint; ASCII event payloads retain the original layout.

Validation status and exact results are in [HANDOFF.md](HANDOFF.md). An x86
build/export audit, synthetic detour execution, and real-game acceptance are
still required before deployment. Like the C++ implementation, installation
does not suspend other game threads; runtime unloading while hooks or the
initializer are executing has not been made safe. Use process lifetime loading.
Do not change Windows app-control policy or rename/move binaries to bypass 4551.
