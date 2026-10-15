# Rust SaveManager core and CLI

Uses actual ZIP archives with `manifest.json` PascalCase fields (`OrigFolder`, `Title`, `Level`, `BackupTime`), compatible with C# `SaveBackupManifestCodec`. Handles UTF-8 BOM manifests, legacy backup filenames, and PFIL/LZSS `save.ini` metadata decoded as Windows-1251. The reused Rust PFIL decoder is stricter than the C# decoder on corrupt/trailing compressed input.

Commands (all paths explicit):

```text
against-rome-saves scan GAME_DIRECTORY BACKUP_DIRECTORY
against-rome-saves backup GAME_DIRECTORY BACKUP_DIRECTORY SAVE_FOLDER [TITLE LEVEL]
against-rome-saves restore GAME_DIRECTORY BACKUP_DIRECTORY BACKUP_ZIP SAVE_FOLDER
against-rome-saves repair-ai GAME_DIRECTORY BACKUP_DIRECTORY SAVE_FOLDER
```

`GAME_DIRECTORY/SAVE` contains save folders. Backup does not overwrite an existing archive. ZIP creation uses a temporary file and publishes via a hard link, which requires a filesystem supporting hard links. Restore extracts and validates in a sibling staging directory, moves the old save aside, then renames the staged directory into place. A publication failure restores the original directory; rollback errors retain and report the original path. This is recoverable directory replacement, not a crash-atomic filesystem transaction: process or machine failure between renames can leave `.restore_old_*` for recovery. No write is made to the installed game by tests.

Rejects symbolic links/reparse points, unsafe Windows names, archive traversal, case-insensitive duplicate entries, archive symlinks/special files, missing `save.ini`, and archives over 512 MiB expanded or 100,000 entries. Backup rejects a backup directory nested in its source. Concurrent hostile filesystem modifications after validation are outside the threat model; use trusted directories with no concurrent writers.

Library also provides preview bytes and explicit delete operations. Scan returns metadata without a cache (avoids stale timestamp-based data). Backup timestamps retain ISO text in the manifest; legacy names use the original display format. UI date formatting remains the caller's responsibility. GUI integration/replacing the existing C# SaveManager is still outstanding.

Validation: `cargo test --manifest-path native/against-rome-saves/Cargo.toml`. Tests use real files/ZIPs under the system TEMP directory, cover byte-preserving roundtrip and previews, manifest compatibility, Windows-1251 PFIL, fallback names, duplicate/traversal/archive symlinks, injected post-old-directory-move failure with verified rollback, fresh destination, deletion, and nested backup root protection.

WP-04 adds `ai_repair::repair_payload` and `SaveService::repair_endless_ai`. This is the opt-in C# `EndlessSaveAiRepairService` migration of embedded `MAPS/ENDL_ddd/SCRIPT/ak_level` BCI in `CLAK/scr.dat`: P6 scheduler rollback guard and 30-second delays, P8 threshold 70 with canonical gate, P9 spawn-budget restoration and release-before-remark helper. It does not guess arbitrary serialized party-state fields. Unknown/ambiguous signatures fail before writing. The file API creates a full ZIP backup before publishing a staged replacement, rejects read-only files and changed source bytes, and preserves the file permissions. Already-repaired saves produce `AlreadyRepaired` with no write or extra backup. PFIL header bytes other than decoded length, BCI constants and trailing serialized save bytes are preserved. The Rust decoder intentionally rejects malformed/truncated/trailing compressed data that C# may tolerate.

`backup_health::{find_missing, validate, heal}` implements the separate original-file backup dictionary contract, not ZIP damage reconstruction or cryptographic baseline certification. Validation requires eight named entries plus any `MAPS/.../team.dat`; partgeo remains optional. Healing preserves existing entries (case-insensitive), prefers `.bak`, obtains missing epara only from the supplied clean factory, excludes marked custom maps, and validates the `Pfeil00` partgeo baseline (column 12 = 5832704). Errors and recovered entries are returned in `HealReport`. Source directories remain read-only: the C# incidental creation of a physical `partgeo.dau.bak` is omitted under this work package's explicit read-only constraint. Path/reparse checks are retained. These APIs do not invoke .NET in production.

Exact validation from repository root (process-local toolchain):

```powershell
$env:CARGO_HOME='D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\cargo'
$env:RUSTUP_HOME='D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\rustup'
$env:PATH="$env:CARGO_HOME\bin;$env:PATH"
cargo test --locked --offline --manifest-path native/against-rome-saves/Cargo.toml --target-dir re_workspace/target-against-rome-saves -- --include-ignored --nocapture
```

2026-10-15: **24 passed / 0 failed / 0 ignored**, including an explicit C# differential gate (requires .NET 8 SDK). The gate builds unchanged C# patch/compressor sources in TEMP and compares 19 synthetic cases / 46,761 decoded bytes plus exact PFIL compressed output; cases include all shipped legacy P8 gates/thresholds, the broken opcode-160 helper, current no-op and a long trailer exercising compressor wrap/overlap. A package-free NuGet configuration and process-local TEMP APPDATA avoid depending on user NuGet settings. Ordinary `cargo test` runs 23 tests and ignores only this explicit oracle gate. No installed-game or live gameplay acceptance was performed. See `HANDOFF.md` for the work-package handoff.
