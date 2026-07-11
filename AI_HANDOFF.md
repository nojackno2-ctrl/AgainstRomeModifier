# AI Handoff - Live Project Memory

## History Consolidation (2026-07-11)

- User explicitly approved consolidation of the 80-commit `主要開發` history into 8 linear milestone commits.
- The user requested detailed Traditional Chinese history entries; all eight milestone commit messages were rewritten accordingly without changing source or documentation content.
- Original tip `566ad4a` is retained locally at `codex/history-before-consolidation-20260711`; the rewritten history is the current `主要開發` tip.
- The code and documentation tree were verified equal before recording this handoff update; the handoff update is the only intentional content difference.
- The user approved publishing and `origin/主要開發` was force-updated successfully with `--force-with-lease`. Fresh verification on the rewritten history passed: Release build (0 warnings, 0 errors) and xUnit (98 passed, 0 failed, 0 skipped).

> Last refreshed: 2026-07-11 (UTC+08:00)
> Branch: `主要開發` tracking `origin/主要開發`
> Working tree at refresh: documentation-only changes in progress; preserve them and inspect `git diff` before editing.

## Current Objective

Keep the modifier maintainable and safe while documentation stays synchronized with the post-2026-07-10 codebase. The 2026-07-11 refresh and consolidation are complete in the current working tree: public README files, technical specifications, reverse-engineering notes, schema metadata, UI-maintenance rules, and this handoff are current. Completed-plan, historical-review, duplicate verification, and unimplemented-research documents were removed after their durable guidance was merged into canonical documents. Changes are intentionally uncommitted pending user direction.

## Verified Current State

- Architecture: `PatchOptions` is removed. `PatchProfile`, `FeatureRegistry`, `IFeatureModule`, `PatchContext`, and `DetectContext` are the source of truth for Apply/Detect/Restore. `PatchEngine` is the orchestrator; feature-specific changes belong under `src/Core/Features/` and pure transforms under `src/Core/Patches/`.
- Baseline and writes: `Backup.zip` is optional and intentionally untracked. The modifier prefers embedded/local `Backup.zip`, otherwise builds an in-memory baseline from the user-selected valid game root. Never edit installed game files directly during development or tests; exercise the modifier's own apply/restore paths with fixtures.
- BCI: FoodHealing and Endless AI use the shared `BciScriptFile` cache and flush once with `SaveAll`. Do not add direct BCI `File.Write` calls from a feature.
- Custom troops: new `.artroop` exports contain exactly `HP,Dmg,VW,AW,Sight,Relt`. Legacy nine-field files are accepted for import, but Speed, Range, SpellRadius, and priest Sight/casting-distance overlaps are normalized away. Independent experimental modifiers own those values.
- Experimental toggles: `RangedRange3x`, `UnitMovementSpeed2x`, `SpellEntireMap`, `SpellRange3x`, `ProjectileArcHeight`, and `RangedAccuracy` are registered feature IDs. Priest casting distance uses `Sirad` (objdef column 24) and `SpellEntireMap` writes 30000; spell effect radius is `cl_script.ini [Spells] Radius` and `SpellRange3x` scales it by 3. Projectile arc and accuracy are static-verified but still need focused runtime-regression observation.
- Endless military mode: safe values are party count `20..20`, wait `5000 ms`, bounded active-party limit `8`, and original loop pacing. The runtime has only 20 NPC-job slots. Do not restore the rejected unconditional gate bypass. Village/settlement mode remains separate unless explicitly requested.
- CI: `.github/workflows/ci.yml` builds all three projects on Windows and runs xUnit. `EnableWindowsTargeting` is set for cross-platform restore. The optional GitHub automatic dependency-submission setting may still emit an external opaque `HttpError`; this is not a project build/test failure.

## Latest Local Verification (2026-07-11)

```powershell
dotnet build AgainstRomeModifier.csproj -c Release --no-restore
# Result: 0 warnings, 0 errors

dotnet test tests/AgainstRomeModifier.Tests/AgainstRomeModifier.Tests.csproj -c Release --no-restore
# Result: 98 passed, 0 failed, 0 skipped
```

## Active Constraints

1. Read `AGENTS.md`, this file, Git status/diff, and recent commits before work.
2. Preserve all uncommitted changes; never reset, checkout, rebase, or commit without explicit user approval.
3. Unknown binary/BCI signatures are not writable. Preserve original/patched/legacy/unknown detection and rollback behavior.
4. Keep project documents UTF-8 with BOM when they contain Traditional Chinese. Check readability with `Get-Content -Encoding utf8` after a rewrite.
5. Keep public source free of original game files, `Backup.zip`, local saves, decompiler caches, and restore payloads.

## Useful Entry Points

| Topic | Primary code/doc surface |
|---|---|
| Feature contracts and categories | `src/Core/Features/FeatureRegistry.cs`, `PatchProfile.cs` |
| Apply/restore transaction | `src/Core/Services/PatchEngine.cs` |
| Backup, ZIP safety, baseline stats | `src/Core/Services/BackupManager.cs` |
| Experimental UI and profile map | `src/UI/ModifierForm.cs`, `ModifierForm.Patches.cs`, `ModifierForm.Data.cs` |
| Endless BCI evidence | `docs/reverse-engineering/endless-mode-ai.md`, `known-patches.md` |
| Projectile and experimental evidence | `docs/reverse-engineering/projectile-ballistics.md`, `priest-spells.md`, `known-patches.md` |
| Test fixtures | `tests/AgainstRomeModifier.Tests/` |

## Next Steps

1. Complete the current documentation-only refresh and verify UTF-8 readability, internal links, `git diff --check`, build, and test.
2. If validating experimental projectile features in-game, record the test map, enabled toggles, expected behavior, observed behavior, and restore result; do not upgrade their runtime status without that evidence.
3. Keep new proposals in the canonical technical document or a focused reverse-engineering note only when they contain actionable, current evidence; do not recreate completed-plan or historical-review documents.
