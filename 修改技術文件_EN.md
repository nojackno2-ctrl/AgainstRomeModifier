# Against Rome Modification Technical Document

## Document Record Rules
* **All modifications must be recorded** in this document.
* **Only correct and successful modifications are recorded**, unsuccessful attempts are excluded.
* Records must **specify the modified file name, exact modification location (e.g., line numbers or sections), and what changes were made**.
* **Value Modification Record Format**: When a value is modified, use the format `OriginalValue(ModifiedValue)` (e.g., `200(1600)`).
* **Encoding Rule**: This file must be saved and edited in **UTF-8 (without BOM)** to prevent character corruption.
* **No Truncation**: Under no circumstances write truncated text indicators into the file.
* **Precise Edits**: Use exact line range replacement tools to edit files to avoid accidental data loss.

## Modification Objectives
1. Convert the game interface language to English for reference and development.
2. Increase the population limit from 200 to 1600 for players and AI in Endless, Multiplayer, and Custom modes.
3. Make all building construction and unit production completely free.
4. Optimize and balance unit stats, movement speeds, spell casting range, and attack ranges for all factions.

## Modification Mechanisms
1. Utilize the built-in `ToEng` backup folder to overwrite the original game's system folder with the English text and graphics resources.
2. Unpack or decompress configuration and map files (such as `team.dat`, `ress.ini`, `cl_script.ini`, `objdef.dau`, `apt.dat`), apply the modifications, re-compress, and restore the custom headers.

## Backup File Precautions
* **Backup files are for extracting original data and comparing values only**.
* **Never overwrite files directly with original backups**, otherwise you will revert all subsequent changes.

## Anti-Overwrite Workflow & Architecture Integration (Mandatory)
To avoid the "applying change A reverts change B" issue, all standalone scripts (Python, PowerShell) have been fully integrated into the unified graphic modifier. The project is modularized into `Program.cs`, `GameLZSS.cs`, `TroopConfig.cs`, and `ModifierForm.*.cs`. Old scripts have been deleted. Modifying core files must follow:
1. **Single Entry Point**: All value adjustments and applications must go through the modifier UI.
2. **Read-Apply-Write Process**: On each application: **Read original backup from the Backup zip -> Apply custom values and stats from UI -> Compress and overwrite active game files**. This ensures changes accumulate correctly.
3. **No Independent Scripts**: Any new file modifications must be added directly into this modifier codebase (centered around `ModifierForm.Patches.cs`).

---

## Technical Details

### 1. UI & Language Resource Overwrite
* **Text Resources (`SYSTEM\TEXT\US\`)**: Overwrites `.put` files for main text:
  - Menu & UI: `opt.put`, `g_mscr.put`, `g_bann.put`
  - Quests & Briefings: `g_brief.put`, `g_kamp.put`, `debriefg.put`
  - Units & Objects: `objnames.put`, `g_volk.put`
  - Dialogs & Tips: `netdlg1.put`, `netdlg2.put`, `nettexts.put`, `msgbox.put`, `g_tut.put`
* **Graphic Resources (`SYSTEM\CLMK\DLG\`)**: Replaces UI TGA assets:
  - Stats screens (`STAT_01` to `STAT_05`, `ENDL`)
  - Settings (`OPTA`), Menu buttons (`VERT`, `INFO`)
* **Map Briefings (`MAPS\`)**: Overwrites `briefing.put`, `netgame.put`, `text.put` in multiplayer and tutorial folders.

### 2. cl_script.ini Changes
* **Details**: To fix AI respawn issues, all attributes, regeneration, and spawn delays are restored to original backups. Only the priest spell radius (`Radius`) is modified, scaling all spell radii by 2.5x.

### 3. ress.ini Changes
* **Format**: Uses LZSS compression with a 64-byte `PFIL@` header.
* **Modifications**:
  - **Free Construction & Upgrades**: Items starting with `Bau` in `[objres]` have costs set to `0` (including Index 8-11 upgrade materials). Fixed 11 missing tier-3/upgrade buildings at the end of the section to prevent engine fallback.
  - **Free Production & Healing Food Cost**: Unit costs in `[objres]` (cols 13-19) are set to `0`. Healing food cost is set to `10` for infantry/civilians and `20` for cavalry/leaders/priests (col 22), forcing food consumption when regenerating. Siege weapons/traps/barricades wild construction costs (cols 1-7) are cleared.
  - **Zero Spell Cost**: Set all priest spell MP costs in the last 4 columns to `0`.
  - **Free Honor Unlocks**: Honor costs for formations (cols 8,10,12,14), unit/skill/building research (cols 24-263 even columns), and attribute upgrades (cols 264-291) in `[volkres]` are set to `0`.
  - **Population Limit**: Max population limit (Index 2) in `[volkres]` is set to `1600`.

### 4. objdef.dau Changes
* **Format**: LZSS compression. Keep original size of `3,310,807` bytes to prevent crash.
* **Modifications**:
  - **Double Movement Speed**: Double `moves` (Index 4), `movsf` (Index 23), and `bmovs` (Index 191) for all mobile troops, excluding siege engines/projectiles/priests.
  - **Priest Range**: Set priest sight `sirad` (Index 24) to `30000` (original `1500`) to enable distant spell casting.
  - **3x Ranged Range & Sight**: Ranged unit sight (Index 24) and Weapon 2/3 range (Index 88-89, 96-97) are scaled by 3.0x.
  - **1.5x Ranged Fire Rate**: Ranged reload time (`relt`) scaled by `1/1.5`.
  - **Unit Balance & Faction Traits**: Units are categorized into low/mid/high tiers with balanced health, attack, defense, and combat stats. Faction bonuses:
    * Romans: 1.15x HP, +7 Defense (VW).
    * Teutons: 1.3x Melee Dmg, +7 Combat (AW), -2 VW (melee/leaders only).
    * Celts: +5 VW, 1.25x Ranged Dmg.
    * Huns: -7 VW, 1.35x Melee Dmg (cavalry/leaders only).

### 5. team.dat Changes
* **Details**: Unpacks map files and sets the population limit `[maxteamobjgenerell]` and each faction's limit in `[teamdata]` to the custom limit (e.g., 1600). Restores other stats to original backups.

### 6. apt.dat Changes
* **Details**: Restored collision sizes for projectiles. Directly copy `Backup\apt.dat`.

### 7. Main Executable Changes
* **Details**: At offset `0x161a88` in `Against_Rome.exe`, replace `89 15 C4 7D 9E 02` with `90 90 90 90 90 90` (`NOP`) to prevent auto-pause when focus is lost.

---

## Version History (Summary)
* **v1.0 - v14.1**: Implemented core LZSS compression algorithms, GUI styling, bulk settings, presets, safe file access, error handling, and fixed various game bugs.
* **v15.0 - v15.2**: Integrated movement speed, sight, range, reload speed, and spell radius multipliers directly into the balance toggle (`chkBalance`). Corrected grid alignment and column offsets.
* **v16.0**: Optimized memory allocations by replacing closures with static local functions in the LZSS compression module.
* **v17.0 - v17.1**: Implemented cache memory structures for `objdef.dau` and save files, drastically speeding up loading times. Resolved a population limit check bug.
* **v18.0**: Redesigned UI with a symmetric layout and added the `ModernToggle` control. Implemented custom dark rendering for menus, dynamic coloring for attribute comparison, and stable unit sorting.
* **v19.0 - v19.1**: Introduced strong-typed enums for CSV columns, added a thread-safe lock for logger files, and implemented parallel task processing for `team.dat` maps.
