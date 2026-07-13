# Against Rome Reverse Engineering Notes

> Index reviewed 2026-07-11. This is evidence documentation, not permission to write original game files directly. Product changes must be made in the modifier and verified through its fixture/apply/restore workflow.

This directory is the project-local knowledge base for original game data
formats, confirmed field indexes, executable entry points, and patch safety
notes. Addresses and indexes belong here instead of being scattered across UI
and patch code.

## Files

- `../../TechDoc.md`: current Chinese technical specification, integrating the cross-feature maintenance chronology, failure cases, and safety contracts.
- `file-formats.md`: compressed game files, wrappers, and encodings.
- `map-formats.md`: static BMP-grid facts, explicitly unverified map-layer semantics, and the controlled experiments required before any terrain or collision write feature.
- `endless-mode-ai.md`: endless AI spawn, timing, gate, job-slot, party
  lifecycle, and defeat-recovery findings.
- `bci0-opcodes.md`: `BCI0` script container layout and the partial bytecode
  instruction reference used to trace the endless party state machine.
- `objdef-fields.csv`: known `objdef.dau` indexes.
- `ress-fields.csv`: known `ress.ini` indexes.
- `exe-functions.md`: executable functions and anchors found with Ghidra.
- `../feature-spec-roman-endless.md`: runtime-verified Roman Endless design, its ENDL `team.dat` default layer, and the `dlg_volk` EXE setter patch.
- `priest-spells.md`: priest spell effect values, summon/resurrect unit
  types (`SpellODef`), and the decoded resurrection logic.
- `glory-upgrade-combat.md`: glory (`Ruhm`) leader-only per-level stat scaling
  (`aw_stuf`/`vw_stuf`/`dam_stu`/`maxruhm`), active combat skills
  (Berserker/marksmanship/shield/thunder/charge) and their `cl_epara.ini`
  factors, glory earn/loss rules, and the combat upgrade icons.
- `known-patches.md`: implemented, legacy, candidate, and rejected patches.
- `projectile-ballistics.md` and `priest-spells.md`: experimental projectile,
  ranged-distance, spell-radius, and priest casting-distance evidence.
- `decompilation-workflow.md`: local Ghidra/JDK workflow and generated EXE
  inventory.
- `../../data/game_schema.json`: machine-readable fields and patch metadata.

## Current Coverage

- `SYSTEM/DATA_MP/DEFAULTS/objdef.dau`: unit stats, weapons, sight, movement,
  spell range, housing capacity, and building speed (construction/upgrade/repair).
- `SYSTEM/ress.ini`: construction, production, upgrade, refund, and priest
  spell costs.
- `SYSTEM/banner.ini`: banner/icon/object variants referenced by `team.dat`.
- `SYSTEM/cl_script.ini`: villager delay, spell radius, morale parameters,
  per-spell effect values (`Value`/`Value2`/`Duration`/`NumObjects`), and the
  `[GlobalData]` glory earn/loss rules and `[SpecialAbilities]` tribe abilities.
- `SYSTEM/cl_epara.ini`: active combat skill factors (Berserker, marksmanship,
  shield, thunder strike) — read-only research, not yet managed by the modifier.
- `SYSTEM/CLAK/cl_scint.ini`: ODef alias tables and summon/resurrect unit
  types (`SpellODef`/`SpellODef2`) — read-only research, not yet managed by
  the modifier.
- `MAPS/**/team.dat`: map population limits, faction defaults, and banner versions; `RomanEndless` writes only ENDL team 0 to `ROM`.
- `MAPS/ENDL_*/SCRIPT/ak_level.bci`: bounded AI Ultimate Mode patch.
- `MAPS/ENDL_*/Endlos_*_Siedlung*.sdl`: read-only endless settlement templates.
- `Against_Rome.exe`: focus-loss patch, synchronized village-range setter,
  runtime-verified Roman Endless `dlg_volk` setter patch, restore-only legacy signatures, and local full-function inventory.
- `apt.dat`: ZIP-like candidate, not integrated into the modifier.

## Evidence Rules

1. Mark every finding `stable`, `static-verified`, `candidate`, `experimental`,
   `legacy`, or `rejected`.
2. A Ghidra `FUN_*` body is not semantically understood without call-path,
   data-flow, string-registration, or runtime evidence.
3. Separate stored values from runtime meaning.
4. Add field indexes to the CSV and schema before exposing new UI.
5. Record original, patched, legacy, and restored values for every write patch.
6. Unknown signatures remain untouched.
7. Runtime regressions override a plausible static hypothesis.

## Public Repository Policy

`Backup.zip` and original game files are intentionally excluded from GitHub.
Public builds load a user-supplied local backup or build an in-memory restore
baseline from the user's installed game. Localized original-game folders,
complete decompiler inventories, saves, downloaded toolchains, and internal AI
audit artifacts remain local-only under `.gitignore`.

Reproducible scripts under `tools/re/`, structured notes, offsets, and patch
metadata are publishable. Generated pseudocode under `re_workspace/` is local
evidence and is not original source code.

## Workflow For New Findings

1. Search this directory and `data/game_schema.json` first.
2. Search the existing local function index and pseudocode inventory.
3. Add a focused script under `tools/re/` only when current evidence is not
   enough.
4. Prove read behavior before designing a write patch.
5. Design signature detection, rollback, restore, and migration together.
6. Validate bytes, PFIL round-trip, and runtime behavior as appropriate.
7. Synchronize `known-patches.md`, schema, technical docs, and code.
