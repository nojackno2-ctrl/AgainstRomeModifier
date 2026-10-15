# Rust executable modifier core

Port of the actual `ExePatchModel.cs` definitions for focus pause, Roman endless selection, and recruit 20. `Patch::detect`, `plan`, and `apply_file` support enabling/disabling known patches, idempotence, and reject unknown, truncated, malformed, or overlapping ranges before writing.

`apply_file` stages a complete sibling file, flushes data, creates a caller-chosen exclusive backup, compares the source again, and commits with Windows `ReplaceFileW` (Unix `rename`). Failed staging leaves the source intact. Windows replacement can fail after moving/removing the target; on error the implementation verifies the target and recreates a missing target from the verified original backup. An unexpected existing target is never overwritten: the backup and remaining replacement are preserved and their paths are reported for manual recovery. The original error remains a failure even after successful recovery. Existing backups, temporary files, and lock files are never overwritten. Stale lock/temp files require manual inspection after a crash. Callers must stop external writers such as the game; the lock coordinates this Rust API only. Atomic visibility is provided; crash/power-loss durability of directory metadata is not promised.

Tests exercise the Windows replacement on temporary synthetic EXEs, roundtrip restoration, untouched byte preservation, unknown bytes, overlap, overflow, existing backups, stale locks/temp files, and injected commit failures, including Win32 1176 after original deletion and 1177 after original rename, plus preservation of an unexpected partial target. CLI integration tests run the actual binary against synthetic files and reject invalid arguments. A separate parity test independently extracts offsets and bytes from current C# definitions. No installed game files are accessed. Remaining patch definitions, archive/data edits, profiles, and UI are not ported here.

The binary defaults to read-only inspection when passed an EXE path. Writes require the `apply` command and a fresh backup path. Unknown arguments/features and duplicate selections are rejected; it never discovers an installed game. CLI apply refuses targets beneath Windows Program Files directories (use an explicit working copy).

```text
against-rome-modifier inspect "D:\work\Against_Rome.exe"
against-rome-modifier "D:\work\Against_Rome.exe"
against-rome-modifier apply "D:\work\Against_Rome.exe" --backup "D:\work\before.bin" --enable focus --disable unit-recruit-20
```

```powershell
$env:CARGO_HOME = "$PWD/re_workspace/rust-runtime/cargo"
$env:RUSTUP_HOME = "$PWD/re_workspace/rust-runtime/rustup"
$env:CARGO_TARGET_DIR = "$PWD/re_workspace/against-rome-modifier-target"
& "$env:CARGO_HOME/bin/cargo.exe" test --locked --manifest-path native/against-rome-modifier/Cargo.toml
& "$env:CARGO_HOME/bin/cargo.exe" build --locked --release --manifest-path native/against-rome-modifier/Cargo.toml
```

Windows failure semantics follow [Microsoft ReplaceFileW documentation](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew): a zero return can describe partial filesystem changes. Recovery restores primary file bytes; recreation after a partial failure does not promise restoration of original ACLs, named streams or all metadata. Recovery itself can fail (permissions, storage failure, concurrent writers); keep reported evidence for manual recovery. Tests simulate these documented error states; they do not force the actual OS to produce 1176/1177.
