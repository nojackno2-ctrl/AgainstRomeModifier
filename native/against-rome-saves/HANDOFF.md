# WP-04 handoff — completed

2026-10-15, Codex. Owned paths: native/against-rome-saves/** only. No git commit/reset/clean; no installed-game writes, game launch, global environment changes or app-control bypass. Unrelated dirty work preserved. Shared AI_HANDOFF.md deliberately untouched under the explicit owned-path restriction.

## Implemented

- Read AGENTS.md, AI_HANDOFF.md, inventory section 5, existing crate and original C# EndlessSaveAiRepairService, BackupAutoHealer, BackupValidator, P6/P8/P9 CustomPatches, BciPattern, OriginalFileValidator, CustomMapManifest, PatchText and GameLZSS.
- Source finding: C# AI repair migrates embedded ak_level BCI, rather than guessing arbitrary serialized party arrays. ai_repair.rs ports enabled P6 deadline rollback/delays, P8 threshold/canonical gates, P9 spawn budget/quota/type filter/release-before-remark helper, including known legacy states and broken opcode160 migration. Preflight all three before mutation; reject ambiguous paths or unknown signatures.
- pfil.rs ports exact C# GameLZSS.Compress hash-chain ordering, overlap and ring behavior. Decode repaired PFIL again and compare exact decoded bytes. Preserve opaque header bytes except decoded length, BCI constants and serialized tail. Already-repaired output is None; file API makes no write or extra backup.
- SaveService::repair_endless_ai and CLI repair-ai GAME BACKUPS FOLDER create a complete save ZIP before staged scr.dat replacement. Preserve permissions, reject read-only/link paths, compare source before publication. API returns AlreadyRepaired or Changed with backup filename.
- backup_health.rs provides BackupFiles, find_missing, validate and heal with HealReport. Eight required entries plus any MAPS/.../team.dat; partgeo optional. Existing entries preserved case-insensitively, .bak preferred, epara supplied only by clean factory, custom map markers excluded, original partgeo validated at Pfeil00 column 12 = 5832704. Source is read-only: explicitly omit C# side-effect creating physical partgeo.dau.bak. This is an original-file dictionary contract, not corrupted ZIP reconstruction or hash certification.
- README documents usage, semantics, intentional stricter PFIL input validation, test commands and limits. Tests/oracle links unchanged C# sources; .NET is used only by the explicit test gate, never production Rust APIs.

## Verification

Process-local toolchain and exact final command, from D:\Github\AgainstRomeModifier:

```powershell
$env:CARGO_HOME='D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\cargo'
$env:RUSTUP_HOME='D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\rustup'
$env:PATH="$env:CARGO_HOME\bin;$env:PATH"
cargo test --locked --offline --manifest-path native/against-rome-saves/Cargo.toml --target-dir re_workspace/target-against-rome-saves -- --include-ignored --nocapture
```

Final result: 24 passed / 0 failed / 0 ignored (8 existing + 15 new ordinary tests + 1 explicit C# differential gate). Binary and doc-test targets each have 0 tests, all successful. C# differential: 19 cases, 46,761 decoded bytes and byte-exact PFIL compressed output. Covers original, three shipped P8 gates x five recognized limits, current no-op, broken opcode160 and long opaque tail for compressor wrap/overlap. Other tests cover complete save backup/restore, no-op SHA256, unknown input rejection without mutation, read-only refusal, baseline healing, custom-map exclusion, source preservation and error reporting. cargo fmt completed; git diff --check -- native/against-rome-saves passed. No dependencies/lockfile changes.

## Resolved attempts / limitations

- Initial compile while adding the test module failed E0583; resolved by creating src/ai_repair/tests.rs, then 23 ordinary tests passed.
- Initial compression emitted legal literal packets; superseded by exact C# compressor port and byte parity tests.
- Oracle initially failed due denied access to user NuGet.Config. A package-free local NuGet.Config alone still failed. Process-local APPDATA under the test TEMP directory resolved that unrelated settings dependency. No 4551 encountered, no policy changes or renamed binaries.
- Original source PFIL decoder tolerates some malformed input; reused Rust decoder intentionally fails closed. Backup healer returns errors/recovered entries rather than localized ILogger strings and intentionally omits source .bak creation per read-only instruction.
- No installed-game fixture or live gameplay acceptance; evidence is synthetic C# parity and real TEMP filesystem/ZIP tests. Existing app GUI has not been wired to this new repair operation (outside owned scope). Concurrent hostile source-tree mutation remains outside the crate's documented threat model.

## Integration next steps

Dispatcher can expose SaveService::repair_endless_ai in the app's explicit opt-in workflow and use backup_health for original-file backup dictionaries. Keep installed directory read-only; any later real-save evaluation must run on a TEMP copy. No remaining WP-04 implementation/test work identified in owned scope.
