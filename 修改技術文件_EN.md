# Against Rome Modifier Technical Document

This document records the modifier's current implementation, proven reverse
engineering evidence, and candidate/disabled work. It is not a version history.

## Game And Tool Paths

- The game path is selected in the UI. The original
  `C:\Program Files (x86)\Against Rome` installation has been verified against
  original EXE bytes.
- Local Ghidra: `C:\Users\nojac\AppData\Local\Temp\AgainstRome_RE\ghidra`
- Local JDK: `C:\Users\nojac\AppData\Local\Temp\AgainstRome_RE\jdk21`
- Ghidra project: `C:\Users\nojac\AppData\Local\Temp\AgainstRome_RE\AgainstRomeVillageBuildArea`
- Full EXE function index: `re_workspace/ghidra_inventory/against_rome_function_index.csv`
- Full Ghidra pseudocode: `re_workspace/ghidra_inventory/against_rome_decompiled_functions.c`

`re_workspace/` is local reverse-engineering evidence. It is not committed to
Git, and original game assets or generated decompiled output must not be
distributed.

## Modifier Architecture

- `Program.cs`: application entry point, DPI, and elevation setup.
- `GameLZSS.cs`: game-specific `PFIL@` LZSS compression and decompression.
- `TroopConfig.cs`: unit IDs, categories, field indexes, and balance rules.
- `ModifierForm.cs`: main UI and embedded technical document.
- `ModifierForm.Data.cs`: backup loading, current data reads, icon parsing, and state detection.
- `ModifierForm.Patches.cs`: all write and restore logic.
- `ModifierForm.Presets.cs`: `.arpreset` and `.artroop` import/export.
- `ModifierForm.SaveManager.cs`: save backup and restore.
- `docs/reverse-engineering/`: auditable reverse-engineering evidence.
- `data/game_schema.json`: tool-readable file format, field, and patch metadata.

## Supported Game Files

- `SYSTEM/DATA_MP/DEFAULTS/objdef.dau`
  - Unit HP, movement, sight, weapon damage, cooldown, range, priest spell
    ranges, VW, and AW.
  - Decompressed text length must be preserved.
- `SYSTEM/ress.ini`
  - Construction, production, upgrade, and spell costs.
  - `[objres]` indexes `19-24` are equipment/refund-related and are preserved
    to avoid breaking UI equipment accounting and AI/job behavior.
- `SYSTEM/cl_script.ini`
  - Civilian spawn speed, spell radius, and morale parameters.
- `MAPS/**/team.dat`
  - `[maxteamobjgenerell]` and `[teamdata]` population limits.
  - `[teamdata]` column 6 is `bannerVersion/bver`, not an AI switch.
- `MAPS/ENDL_*/SCRIPT/ak_level.bci`
  - Endless AI Ultimate Mode count, respawn wait, action-loop wait, and
    active-AI gate candidate patches.
- `Against_Rome.exe`
  - Background execution when focus is lost.
  - Legacy village build range candidate bytes are restore-only and are no
    longer applied.
- `apt.dat`
  - Confirmed as a ZIP-like UI/layout container. The modifier does not write it.

## Current Feature Status

- Stable: population limit, free construction/production/upgrades/spells, unit
  stats, civilian speed, spell range, morale, language copy, focus-loss
  background execution, save management, and presets.
- Candidate: AI Ultimate Mode's endless AI acceleration and active-AI gate
  bypass. The BCI/EXE path is located, but each effect still needs in-game
  validation.
- Disabled: 2x village red build-range frame. The old EXE offsets
  `0x1366c4`/`0x1366cd` affect `00536630` logical village bounds, but did not
  enlarge the visible red dashed frame. The modifier hides this option and only
  restores old patched bytes.

## Reverse Engineering Rules

- Start with `docs/reverse-engineering/`, `data/game_schema.json`, and
  `re_workspace/ghidra_inventory/`.
- Mark field meaning confirmed only when EXE or BCI runtime evidence shows how
  the game consumes it.
- Ghidra pseudocode is not original source. Unknown `FUN_*` functions remain
  unknown until call-path, string, or runtime evidence supports a name.
- EXE patches must verify original bytes and provide restore bytes.
- Every write path must have a backup or byte-level restore path.

## Village Red Frame Status

Verified original installed EXE bytes:

- `0x1366c4`: `C1 E2 06`
- `0x1366cd`: `C1 E1 06`
- `0x136867`: `C1 E0 06`

Changing the first two shifts to `7` did not enlarge the visible red frame, so
it is not the correct fix. The next evidence path is the overlay chain:

`0044e990 -> 00421c00 -> 00520320 -> 005203a0`

`0044e990` creates overlay type `0x28`. Until the visible frame size source is
proven, this modifier does not expose a village build-range feature.
