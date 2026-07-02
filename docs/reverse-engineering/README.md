# Against Rome Reverse Engineering Notes

This directory is the project-local knowledge base for original game data
formats, confirmed field indexes, executable entry points, and patch safety
notes. Addresses and indexes belong here instead of being scattered across UI
and patch code.

## Files

- `../AI_AGENT_HANDOFF.md`: cross-feature maintenance chronology, failure cases,
  safety contracts, and verification playbooks for future agents.
- `file-formats.md`: compressed game files, wrappers, and encodings.
- `endless-mode-ai.md`: endless AI spawn, timing, gate, job-slot, and save-state
  findings.
- `objdef-fields.csv`: known `objdef.dau` indexes.
- `ress-fields.csv`: known `ress.ini` indexes.
- `exe-functions.md`: executable functions and anchors found with Ghidra.
- `known-patches.md`: implemented, legacy, candidate, and rejected patches.
- `decompilation-workflow.md`: local Ghidra/JDK workflow and generated EXE
  inventory.
- `../../data/game_schema.json`: machine-readable fields and patch metadata.

## Current Coverage

- `SYSTEM/DATA_MP/DEFAULTS/objdef.dau`: unit stats, weapons, sight, movement,
  spell range, and housing capacity.
- `SYSTEM/ress.ini`: construction, production, upgrade, refund, and priest
  spell costs.
- `SYSTEM/banner.ini`: banner/icon/object variants referenced by `team.dat`.
- `SYSTEM/cl_script.ini`: villager delay, spell radius, and morale parameters.
- `MAPS/**/team.dat`: map population limits and banner versions.
- `MAPS/ENDL_*/SCRIPT/ak_level.bci`: bounded AI Ultimate Mode patch.
- `MAPS/ENDL_*/Endlos_*_Siedlung*.sdl`: read-only endless settlement templates.
- `Against_Rome.exe`: focus-loss patch, synchronized village-range setter,
  restore-only legacy signatures, and local full-function inventory.
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
