# Ambience port handoff

2026-10-18. Owned path: native/against-rome-ambience/** only. No commits made.
Status: ADVANCED & COMPLETED.

- Done:
  - atmosphere.rs: BodenIni*, AtmosphereEditSession, AtmosphereLightingBridge, AtmosphereProfile, WeatherPresets, AtmosphereShaderUniforms.
  - soundscape.rs: SoundscapeSnapshot, SoundscapeBinaryStorage, SoundscapeZoneCatalog, SoundscapeZonePlanner, SoundscapeEditorModule, SoundscapeMixEvaluator.
  - localization.rs: BriefingSaveTransaction, BriefingTextTable, BriefingTextTableCatalog, MultiLanguageTextStudio, PutScriptSyntaxValidator, TranslationGlossary.
  - app_localization.rs + app_tables.rs: Loc, Language, full application text tables and culture fallback.
  - lib.rs: clean crate-root re-exports for parent and editor integration.
- Tests (tests/*.rs): 24 passing, 0 failing, 0 warnings.
- Test run:
  cargo test --manifest-path native/against-rome-ambience/Cargo.toml --target-dir re_workspace/target-against-rome-ambience
