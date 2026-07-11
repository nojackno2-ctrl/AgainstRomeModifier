# Against Rome Reverse Engineering Notes

This directory is the project-local knowledge base for original game data formats,
confirmed field indexes, executable entry points, and patch safety notes.

The goal is to keep reverse-engineering results in small, auditable files instead
of scattering addresses and CSV indexes across UI and patch code.

## Files

- `file-formats.md` documents known compressed game files and text encodings.
- `endless-mode-ai.md` records current endless-mode AI spawn findings.
- `objdef-fields.csv` lists known `objdef.dau` CSV indexes.
- `ress-fields.csv` lists known `ress.ini` CSV indexes.
- `exe-functions.md` lists executable functions and offsets found from Ghidra.
- `known-patches.md` records implemented patches and risk level.
- `decompilation-workflow.md` records the local Ghidra/JDK workflow and the
  generated full-EXE inventory.
- `../../data/game_schema.json` is the tool-readable version intended for future UI and patch logic.

## Current Coverage

- `SYSTEM/DATA_MP/DEFAULTS/objdef.dau`: unit stats, weapons, sight, movement, spell range.
- `SYSTEM/ress.ini`: construction, production, upgrade, and priest spell costs.
- `SYSTEM/banner.ini`: team banner/icon/object variants referenced by `team.dat`.
- `MAPS/**/team.dat`: map population limits and banner version references.
- `MAPS/ENDL_*/SCRIPT/ak_level.bci`: endless-mode AI spawn script.
- `MAPS/ENDL_*/Endlos_*_Siedlung*.sdl`: endless-mode settlement templates.
- `SYSTEM/cl_script.ini`: villager delay, spell radius, morale parameters.
- `Against_Rome.exe`: focus-loss pause patch.
- `Against_Rome.exe`: full local function inventory and Ghidra pseudocode under
  ignored `re_workspace/ghidra_inventory/`.
- `apt.dat`: identified as ZIP-like container, not yet integrated into the modifier.

## Public Repository Policy

`Backup.zip` and original game files are intentionally excluded from GitHub.
Public builds should either load a local `Backup.zip` supplied by the user or
build an in-memory restore baseline from the user's own installed game folder.

## Rules For New Findings

1. Add raw addresses and function names to `exe-functions.md`.
2. Add field indexes to the CSV files before wiring them into C#.
3. Mark each new field as `stable`, `candidate`, or `experimental`.
4. Prefer adding to `data/game_schema.json` before adding new UI controls.
5. Keep original game code and assets out of this repository; store only format notes, offsets, and patch metadata.
