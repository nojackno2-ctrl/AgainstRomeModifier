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

Build a dedicated map editor that can eventually provide an Age-of-Empires-II-like authored-map workflow, while preserving Against Rome's undocumented-format safety constraints. `docs/map-editor-spec.md` remains the staged implementation guide. Changes are intentionally uncommitted pending user direction.

## Map Editor Phase 1 Progress (2026-07-11)

- Added `src.Shared/AgainstRome.Shared.csproj`; `GameLZSS`, `FileRollbackScope`, and `SafeFileWriter` now have one shared implementation. The original modifier references it without changing their namespaces or call sites.
- Added `src.MapEditor/AgainstRomeMapEditor.csproj`, producing `AgainstRomeMapEditor.exe`. It accepts `--game <path>` and optional `--map ENDL_NNN`, lists maps, clones a selected map into the next available `ENDL_005..999` slot, and safely edits the Phase 1 briefing/boden.ini fields.
- Shared map core implements actual `ENDL_???` discovery, `.arm_custom_map` + `MAPS/arm_custom_maps.json` registration, temporary-directory cloning, unknown-file byte preservation, briefing title changes, and SDL `name` map-path rewrite. All PFIL rewrites retain their original 64-byte headers and write through `SafeFileWriter`; multi-file property saves use `FileRollbackScope`.
- Map text is strict Windows-1251. A title that cannot be represented by the game encoding fails safely rather than being written as `?`.
- `EndlessAiOrchestrator` now enumerates existing `ENDL_???` directories for `ak_level.bci`/SDL targets and calculates expected map-target counts dynamically. `BackupManager` excludes directories marked `.arm_custom_map` from original `team.dat` baseline / `.bak` handling; map population patching remains unchanged and still applies to all maps.
- Added `MapEditorPhase1Tests`; latest verification: `dotnet test tests/AgainstRomeModifier.Tests/AgainstRomeModifier.Tests.csproj -c Release --no-restore` passed **100/100**. A full Release solution build also passed before this handoff update.
- Manual smoke evidence (user, 2026-07-11): a copied new map appears usable and can enter the game. This validates the slot-discovery and basic load path. The user explicitly deferred AI long-run behavior and save/load verification; do not treat either as verified.
- Phase 2 viewer: `MapCanvasControl` now provides read-only minimap, collision bitmap, and deterministic `boden.txt` 64x64 texture-grid views. It deliberately does not overlay SDL objects: the world-coordinate-to-image calibration is still unverified and must not be guessed.
- First authored-map tool: the texture-grid view now has a material palette and a click-to-paint single-tile brush. It writes only the static-verified `boden.txt` 64×64 table, only after the user saves, only to a `.arm_custom_map` custom map, and through the same multi-file `FileRollbackScope` transaction as map properties. Original maps are now read-only in the editor. `BodenTexturesDocument` has exact-cell parsing/writes and a round-trip test; latest xUnit count is **101 passed**.
- Editor UX redesign after user feedback: the previous nested-tab prototype was replaced with a 1440×900 canvas-first workspace. Maps are always visible at left; the center is the active canvas; the right inspector exposes the material palette and map properties; top command/tool bars expose clone, save, undo/redo, material editing, minimap, and collision preview. The material brush supports click-drag painting, tile hover coordinates, palette search/color swatches, undo/redo, dirty-state indication, and save/discard/cancel prompts. Height, collision-write, and object tools are visible but disabled with explicit validation labels. Release build has 0 warnings/errors, xUnit passes 101/101, and a hidden runtime startup smoke test confirmed the form constructs and remains running.
- Stale-launch diagnosis: the user's screenshot showed the old nested-tab UI because `bin/Debug/net8.0-windows/AgainstRomeMapEditor.dll` predated the newly built `src.MapEditor/bin/Debug` DLL. Debug and Release outputs are now rebuilt and hash-matched. `ModifierForm.MapManager` also resolves the newest development DLL/EXE candidate by timestamp while published builds continue to use the sibling executable, preventing an old copied editor from being launched silently.
- Texture-view correction after visual feedback: the first canvas-first build still rendered every texture ID as a hash-derived random color, producing an unusable multicolor grid. The material editor now draws the real `minimap.bmp` as its base, overlays a restrained 64×64 tile grid, and colors only changed cells. Palette swatches are derived from the average minimap color of all cells using that texture ID. Workspace texture folders contain no matching texture images, so IDs such as `L5B09T1A` remain the only available source labels. The computer-use visual QA pass confirmed the new canvas is a recognizable terrain map rather than a random-color wall.
- Material-tool UX correction after a second user screenshot: raw IDs are no longer the primary workflow. The right inspector defaults to a current-brush swatch plus explicit instructions; right-clicking a map tile samples its material and left-drag paints it elsewhere. Status/brush labels use stable per-map names such as `地表樣式 001`. The raw-ID library is hidden behind an explicit `顯示進階材質庫（內部 ID）` checkbox. A second computer-use visual QA pass confirmed the default inspector is clean and no longer presents an unexplained ID wall.
- Real-render preview boundary: the user requires the preview to be the actual game-rendered scene. The editor now labels minimap/collision views as data/editing backgrounds, not game previews, and exposes `在遊戲中預覽`: it transactionally saves dirty custom-map data, launches `Against_Rome.exe` with the selected game root as working directory, and tells the user which `ENDL_NNN` slot to choose. No verified direct-map command-line argument exists. Static EXE evidence for unreachable mode 9 / `TextureEditor V0.1` is documented in `docs/reverse-engineering/map-formats.md`; it appears to be a procedural texture/debug tool, not yet a safe 3D map-editor entry point.
- Phase 3 static baseline: five original ENDL samples were measured read-only and documented in `docs/reverse-engineering/map-formats.md`. BMP dimensions/channel classes and the 64×64 / 256×256 / 257×257 grid relation are static-verified; `vertex.bmp` height semantics, collision black/white semantics, cache-file authority, and SDL-to-minimap calibration are explicitly unverified. Do not expose any corresponding write control until the document's one-variable-at-a-time runtime experiments succeed.

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
