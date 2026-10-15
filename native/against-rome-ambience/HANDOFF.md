# Ambience port handoff

2026-10-15 (Claude, resuming Codex): crate compiles and tests pass. Ownership restricted to this crate; no commits.

- Done: atmosphere.rs (BodenIni*, EditSession, LightingBridge, Profile, WeatherPresets), soundscape.rs (all 6 C# files), localization.rs (Save transaction, Catalog, Studio, PutScriptAst, SyntaxValidator), app_localization.rs + app_tables.rs (root Loc).
- Fixes this session: borrow-checker errors in MultiLanguageTextStudio; `lzss::compress_pfil` does not exist in against-rome-data, so a local literal-only `pack_pfil` (same approach as campaign injector) is used for briefing.put.
- Added tests: tests/atmosphere.rs, soundscape.rs, localization.rs (behavioral smoke/round-trip; not byte-oracle against C#).
- src/localization/catalog.rs is a stray file not in the module tree (duplicate descriptor JSON); safe to delete.
- SNDZ is an editor prototype; native sound/script export stays unsupported (tests assert this).
- Test: cd native/against-rome-ambience; cargo test --offline --target-dir ../../re_workspace/target-against-rome-ambience
