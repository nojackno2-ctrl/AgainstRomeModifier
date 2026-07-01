# Against Rome Modifier Complete Technical Document

This document describes the current code, data formats, reverse-engineering evidence, enabled patches, candidates, and rejected approaches. It is not a version history. Each feature has one current description. Reproducible runtime behavior and the latest concrete decompiler evidence take precedence over an older interpretation.

## 1. Documentation and Evidence Rules

- Project documentation is UTF-8. Decompressed game text is Windows code page 1251.
- Field indexes are zero-based. Byte sequences and file offsets are hexadecimal.
- `TechDoc.md` is the Chinese technical document. `TechDoc_EN.md` is the English technical document; both are embedded in the application.
- **Stable** means implemented, signature/format checked, and runtime or round-trip verified.
- **Implemented candidate** means writable and restorable with strong static evidence, but incomplete runtime coverage.
- **Read-only candidate** must not be written automatically.
- **Rejected** means runtime evidence disproved the static hypothesis. Exact details remain documented to prevent the same failed patch from being reintroduced.
- A generated Ghidra `FUN_*` body is not understood merely because it decompiles. Meaning requires call-path, data-flow, registered string, or runtime evidence.

## 2. Architecture

| File | Responsibility |
|---|---|
| `Program.cs` | WinForms entry, elevation, High DPI startup, global exception handling. |
| `GameLZSS.cs` | LZSS and `PFIL@` wrapper decode/encode with bounds checks. |
| `TroopConfig.cs` | Field enums, unit IDs, names, factions, tiers, types, and balance baselines. |
| `ModifierForm.cs` | Main UI, controls, backup cache, parsed unit cache, shared state. |
| `ModifierForm.Data.cs` | Current-data reading, CSV-like parsing, comparisons, icons, EXE state detection. |
| `ModifierForm.DataExt.cs` | Safe access to cached original unit rows. |
| `ModifierForm.Patches.cs` | Transactional writes, restores, EXE/INI/DAU/team/BCI patches. |
| `ModifierForm.Presets.cs` | `.arpreset` import/export and compatibility. |
| `ModifierForm.SaveManager.cs` | Save discovery, ZIP backup/restore/delete, metadata cache. |
| `TroopPresetForm.cs` | Nine-property editing for 43 units and `.artroop` I/O. |
| `UIElements.cs` | Owner-drawn toggles, dark menu renderer, GDI disposal. |
| `Localization.cs` | Chinese/English UI and log strings. |

The application targets `.NET 8`, `net8.0-windows`, WinForms, x64, nullable reference types, and `PerMonitorV2` DPI. `Backup.zip` is embedded only when present; both embedded technical documents are mandatory resources.

## 3. Backup, Transactions, and Apply Order

Original data is loaded from embedded `Backup.zip` when available. Public repositories must not publish original game assets. Without the archive, required originals are read from the selected game installation and retained in memory. `backupFiles` uses case-insensitive keys.

`FileRollbackScope` records every target before its first write. `SafeWriteAllBytes` writes through a temporary file in the target directory. Any exception restores all files to their state at the start of that operation. A successful operation calls `Commit()`, disposes the scope, and only then refreshes UI data. This is a per-operation transaction, not a persistent `.bak` system.

Apply order:

1. Validate the directory and `Against_Rome.exe`.
2. Load originals and confirm with the user.
3. Snapshot UI values on the UI thread.
4. Apply EXE compatibility state.
5. Apply `cl_script.ini`.
6. Apply `ress.ini`.
7. Apply `objdef.dau`.
8. Restore original `team.dat` files, then apply population only.
9. Apply endless-mode BCI changes.
10. Apply language resources.
11. Commit, dispose rollback state, and reload current values.

## 4. PFIL/LZSS and Text Compatibility

Known `PFIL@` users include `ress.ini`, `cl_script.ini`, `banner.ini`, `objdef.dau`, all `team.dat`, endless `ak_level.bci`, and some endless settlement `.sdl` files.

The decompressor rejects negative or greater-than-50-MB output sizes. The 4096-byte ring uses `& 4095`. Compression uses a 16-bit hash table, bounded hash chains, and guards against matching not-yet-updated short-distance ring data. Every changed payload should pass `decompress(compress(payload)) == payload`.

The game does not implement RFC 4180 CSV. CSV-like rows use `Split(',')` and `string.Join`. Adding quotes or escaping can make old engine object IDs and paths invalid, causing missing building buttons. Preserve original line endings, trailing empty fields, and cp1251 encoding.

## 5. Language Resources

English mode overlays resources from the selected game's `ToEng` directory. Before the first managed overlay, all 332 destination states are preserved in `.against-rome-modifier-language-backup`; disabling restores that baseline, including deleting files that did not originally exist. Enabling aborts if `ToEng` is missing or empty, and restore aborts instead of reporting success when an older modifier already overlaid every file without creating a baseline. Coverage includes:

- `SYSTEM/TEXT/US/`: menus, briefing, campaign, object names, multiplayer dialogs, messages, and tutorials.
- `SYSTEM/CLMK/DLG/`: text-bearing TGA buttons, result screens, settings, and menus.
- `MAPS/`: multiplayer/tutorial `briefing.put`, `netgame.put`, `text.put`, and related resources.

Language restore is independent from stat and compatibility restore.

## 6. `SYSTEM/cl_script.ini`

This is cp1251 text inside `PFIL@`. Compiled regular expressions locate `Radius`, `CiviDelay`, `MoralsDecLostMem`, `MoralsDecFlee`, `MoralsDecOverPop`, and `MoralsIncIdle` while preserving comments.

- The core villager-speed switch writes every `CiviDelay` as 500 ms (the fastest valid 10x setting). When disabled, original backup values are preserved. The executable clamps the delay to at least 500 ms.
- Infinite morale uses the executable's minimum accepted `MoralsIncIdle` value of 500 ms and is described as rapid rather than instant recovery.
- Infinite morale zeroes decay parameters and sets the required idle recovery behavior.
- Balance mode applies a default 2.5x faction spell-radius multiplier.
- A custom KEL/HUN priest's ninth property overrides the faction multiplier as `SpellRadius / 500.0`. GER has no original Radius record and remains non-editable at zero.
- If no relevant feature is enabled, the original file is written; values are never repeatedly multiplied from an already modified file.

## 7. `SYSTEM/ress.ini`

### 7.1 EXE Evidence

- `0046c1c0` loads `SYSTEM/ress.ini`.
- `0042a230` parses sections and rows.
- `[objres]` is passed as up to 500 rows with field parameter `0x1f` to `0046bd00`.
- `[volkres]` is passed as 6 rows with field parameter `0x128` to `0046b200`.
- Callback writes and switches are the basis for field groups; unresolved meanings remain candidates.

### 7.2 `[objres]`

| Index | Meaning | Current behavior |
|---|---|---|
| 0 | Object ID | Never changed. |
| 1-6 | Engine `bau` six-resource construction-cost group | Zeroed for `Art`/`Bar`/`Fal` siege or trap rows under free production; decompilation confirms use by construction resource checks and deductions. |
| 1-6 | Complete building `bau` construction/repair cost group | Zeroed for `Bau*` under free construction. |
| 7-12 | Complete building `upg` cost group | Zeroed under free upgrades. |
| 13-18 | Engine `aus` six-resource unit-training group | Zeroed under free production; decompilation confirms use by production-count calculation, resource checks, and deductions. |
| 19-24 | Engine `auf` equipment/refund relationship | Preserved from the original. |
| 25-28 | Priest/druid MP costs | Zeroed under no spell cost. |
| 29/trailing | Empty/padding | Preserved. |

`FigTiePac00_Packpferd` is excluded as a complete row. Its original `18:1` and `24:1` values represent horse cost and related equipment data. Do not invent `Ver*ZivIco*` rows; those are UI/banner object relationships elsewhere.

Clearing indexes 19-24 was rejected. It can break equipment accounting and endless AI unit relationships. The preserved regression case is: 24 selected villagers, 4 mounted civilians, then 20 battle units, yet the UI still reports 4 unequipped villagers instead of 0. This is a shared reservation-count issue, not population. Disarm refunds require a separate EXE solution rather than destroying `auf` data.

### 7.3 `[volkres]`

- 0-7 are faction-wide skill/resource parameters. Index 2 is healing, not population.
- 8, 10, 12, and 14 are zeroed formation/base research costs.
- Even indexes in 24-263 are unlock costs; paired odd ID fields are preserved.
- 264-295 are four complete eight-level upgrade-cost groups (`befehl`, `motivieren`, `angriff`, and `verteidigung`) and are zeroed under free upgrades.
- Split index 296 is only the empty string created by the trailing comma; it is not an engine-loaded data field.

Population must never be written to `[volkres]`.

## 8. `objdef.dau`

The file is cp1251 CSV-like text inside `PFIL@`. Field 52 maps rows to `TroopConfig.UnitMeta`. Decompressed text length must remain exactly unchanged. `CheckLen` verifies each replacement fits the original field width and pads it; otherwise the entire unit update is skipped with a warning.

Stable indexes:

| Index | Field |
|---|---|
| 4 | Moves |
| 19 | HP |
| 23 | Movsf |
| 24 | Sirad |
| 52 | internal Name |
| 78/79/84 | Weapon 1 active/damage/reload |
| 80-82 | Priest spell ranges |
| 86/87/88/89/92 | Weapon 2 active/damage/min/max/reload |
| 94/95/96/97/100 | Weapon 3 active/damage/min/max/reload |
| 142 | AW |
| 146 | VW |
| 156 | Housing capacity (`wohnwer`) |
| 191 | Bmovs |
| 199 | Weapon 1 damage/type candidate |

Weapon slots use an eight-column stride and up to eight active slots are inspected. Building indexes 28-39 are not costs; they are production-building resource-storage slots and must remain original.

The fixed nine-property array is `HP,Dmg,VW,AW,Speed,Sight,Relt,Range,SpellRadius`. Old four-property presets are completed from original or active balance baselines.

- HP, VW, and AW use current baseline integers.
- Damage derives a scale from final/original primary damage and applies it to active weapon slots. Weapon 1 on ranged infantry/cavalry is treated as a melee backup.
- Speed derives one factor and applies it to Moves, Movsf, and Bmovs.
- Sight writes Sirad.
- Priest Range scales fields 80-82; other Range scales Weapon 2/3 min/max.
- Relt scales active weapon reload values; lower means faster.
- SpellRadius is implemented in `cl_script.ini`, not `objdef.dau`.
- The core 20x housing-capacity switch multiplies every positive original field
  156 value and remains reversible because each apply starts from the backup.

The balance direction includes 2x movement, 3x ranged/siege range, about 1.5x ranged rate, stronger priest sight/range, and 2.5x spell radius. The current exact four-property baseline from `TroopConfig.CalculateFactionBaseStats` follows.

Generic HP by tier is low 110, mid 130, high 150, ace 160, and leader 450. Priests and siege units return zero from this matrix function and retain their original four-property values unless explicitly customized.

| Faction | Tier | Type | HP | Damage | VW | AW |
|---|---|---|---:|---:|---:|---:|
| Roman | low | melee_inf | 110 | 20 | 8 | 12 |
| Roman | mid | melee_inf | 130 | 28 | 14 | 20 |
| Roman | mid | ranged_inf | 130 | 22 | 12 | 24 |
| Roman | high | melee_inf | 150 | 42 | 20 | 22 |
| Roman | high | ranged_inf | 150 | 30 | 16 | 26 |
| Roman | high | hybrid_inf | 150 | 38 | 18 | 22 |
| Roman | ace | cav | 160 | 50 | 24 | 26 |
| Roman | leader | leader_melee | 450 | 80 | 28 | 36 |
| Teuton | low | melee_inf | 110 | 25 | 10 | 12 |
| Teuton | low | ranged_inf | 110 | 20 | 6 | 12 |
| Teuton | mid | melee_inf | 130 | 32 | 16 | 22 |
| Teuton | mid | hybrid_inf | 130 | 28 | 14 | 20 |
| Teuton | high | melee_inf | 150 | 38 | 14 | 26 |
| Teuton | high | cav | 150 | 42 | 20 | 24 |
| Teuton | ace | melee_inf | 160 | 65 | 12 | 30 |
| Teuton | leader | leader_melee | 450 | 70 | 26 | 38 |
| Celt | low | melee_inf | 110 | 24 | 10 | 12 |
| Celt | low | ranged_inf | 110 | 20 | 8 | 12 |
| Celt | mid | melee_inf | 130 | 24 | 18 | 18 |
| Celt | mid | ranged_inf | 130 | 20 | 12 | 18 |
| Celt | high | melee_inf | 150 | 38 | 12 | 24 |
| Celt | high | cav | 150 | 38 | 22 | 22 |
| Celt | ace | ranged_inf | 160 | 65 | 18 | 25 |
| Celt | leader | leader_melee | 450 | 60 | 30 | 28 |
| Hun | low | melee_inf | 110 | 26 | 10 | 10 |
| Hun | low | ranged_inf | 110 | 20 | 8 | 12 |
| Hun | mid | melee_inf | 130 | 24 | 12 | 18 |
| Hun | mid | cav | 130 | 32 | 16 | 20 |
| Hun | high | melee_inf | 150 | 36 | 8 | 22 |
| Hun | high | ranged_inf | 150 | 32 | 16 | 24 |
| Hun | high | cav | 150 | 45 | 18 | 26 |
| Hun | high | ranged_cav | 150 | 36 | 16 | 24 |
| Hun | ace | cav | 160 | 52 | 22 | 26 |
| Hun | leader | leader_cav | 450 | 80 | 25 | 36 |

Specialized units return before the generic matrix and therefore take priority:

| Unit key | Role | HP | Damage | VW | AW |
|---|---|---:|---:|---:|---:|
| `FigKelInf01_Lanze` | Celt defensive spearman | 180 | 22 | 32 | 18 |
| `FigRomInf00_Lanze_Schild` | Roman light infantry | 130 | 24 | 22 | 18 |
| `FigRomSch00_Speer_Schild` | Roman armored ranged infantry | 140 | 25 | 26 | 24 |
| `FigRomInf01_Schwert_Schild` | Roman guard | 200 | 36 | 28 | 28 |
| `FigHunInf01_Schwert_Schild` | Hun sword-and-shield infantry | 140 | 24 | 24 | 20 |
| `FigKelInf02_Doppelschwert` | Celt dual-sword infantry | 130 | 40 | 15 | 28 |
| `FigGerInf03_Doppelhammer` | Teuton dual-hammer infantry | 150 | 60 | 16 | 34 |

These are pre-write baselines. Nine-property custom values may replace them, and in-game shield modifiers can make displayed final VW higher than the stored baseline.

## 9. `team.dat`

Every `MAPS/**/team.dat` is restored from its original first. The core switch then writes 1600 when enabled or leaves the restored values unchanged when disabled.

- Preserve `[maxteamobjgenerell]`; the loader ignores it.
- `[teamdata]` column 4 is the runtime per-team limit and is capped by the executable's global limit of 1600.
- `[teamdata]` index 4: replace only when the original value is greater than zero, so disabled team slots stay disabled.
- Index 5 is `bver`, a banner version in range 0-9, not an AI switch. The EXE combines faction and `bver` to resolve `banner.ini` sections such as `[volk%02ld_vicon_bver%02ld]` and `[volk%02ld_obdef_bver%02ld]`.

## 10. Endless `ak_level.bci`

`MAPS/ENDL_*/SCRIPT/ak_level.bci` is a `BCI0` compiled-script payload inside `PFIL@`. Patches search opcode/literal signatures and have been found with the same local sequence in `ENDL_000` through `ENDL_004`.

- Military create call around decompressed `0x17B60`: interpreted as `s_addNPCJob_createUnit(local7, 3, 8, 0, 0, 4, 4, 1, 0)` after reversing BCI stack order.
- Count literals near `0x17B2C` and `0x17B34`: `4 -> 20`.
- Completed-job recycling flag near `0x17B1C`: `0 -> 1`, allowing completed military reinforcement jobs to release their NPC-job slots for later waves.
- Older builds edited three global CLAK economy scripts: `ak_npc.bci`, `ak_produktion.bci`, and `ak_haupthaus.bci`. Runtime testing proved these paths are not safely NPC-scoped and stop all staffed player resource buildings even in a new game. Current builds always restore the three original values regardless of the AI Ultimate toggle.
- EXE path `0054aa80 -> 00547f50` clamps this mode to 1..20.
- Respawn wait: `180000 -> 5000 ms`.
- Action-loop ranges retain their original values to avoid overfilling the NPC-job table.
- Active-party comparison literal at decompressed `0x195F8`: `4 -> 8`; the gate at `0x1960C` remains `66,0`.
- Older `112,272` gate bypasses and 5000..10000 ms action-loop patches are automatically migrated back to safe values.
- Disable/compatibility restore reverses every count, delay, limit, and gate value.

The count represents created military units/formations; the visible individual-soldier total also depends on formation contents. The EXE provides only 20 NPC-job slots per team, so removing the gate entirely is not safe for long-running endless games.

## 11. `Against_Rome.exe`

### 11.1 Focus-Loss Execution

- File offset `0x161a88`.
- Original `89 15 C4 7D 9E 02`.
- Patched `90 90 90 90 90 90`.
- The original write sets a global pause state. The full six-byte signature must match before writing.

### 11.2 Village Build Range: Setter Patch Runtime-Verified, Old Sites Rejected

| Hypothesis | Function | File offset | Original | Tested candidate |
|---|---|---|---|---|
| Logical X | `00536630` | `0x1366c4` | `C1 E2 06` | `C1 E2 07` |
| Logical Z | `00536630` | `0x1366cd` | `C1 E1 06` | `C1 E1 07` |
| Display X candidate | `004d7160` | `0x0d722c` | `C1 E6 06` | `C1 E6 07` |
| Display Z candidate | `004d7160` | `0x0d723b` | `C1 E7 06` | `C1 E7 07` |

Static path:

- `s_setVillageAeraDeltas`: `0053ba20 -> 00536450 -> 004c0900`.
- `s_villageAeraDeltas`: `0053ba60 -> 00536510`.
- `s_getVillageAeraDeltas`: `0053ba80 -> 00536580`.
- `s_getTeamVillageAera`: `0053bad0 -> 00536630`.
- `s_inTeamVillage`: `0053bb40 -> 00536770`, reaching bounds test `00536820`.
- `s_setShowTeamVillageAera`: `0053c140 -> 00537d60`.
- `s_showTeamVillageAera`: `0053c170 -> 00537da0`.
- `00536450` calls `004c0900` to write X/Z into the village object's type-definition rectangle, then writes the same values into per-object village state.
- `004c0970` reads the type-definition rectangle for a generic point-in-object test and has ten UI callers; `004d7160` reads the same copy and calls `00495360` for four dashed sides.
- `00536630` reads the per-object copy. The two paths share setter inputs but use separate storage.

The static hypothesis changed `delta * 64 + 32` to `delta * 128 + 32`. The four sites changed only the final multipliers in `00536630` and `004d7160`; they omitted `004c0970`, another consumer of the type-definition rectangle, so the patch did not synchronize every consumer. Runtime testing also showed no change to the buildable area or the reported red dashed boundary. It is therefore rejected as a working patch.

`00539700` initializes pending-village state through `00536450`. The logical point test `00536820` is directly reached by script/AI wrapper `005367c0` and candidate-position search `00544fd0`; player previews `0044f4b0` and `0044f7b0` do not call it. This rules out `00536630` as the general player construction-range gate.

The current patch hooks `005364c1` (file `0x1364c1`) into a 289-byte executable zero-padding region at `0056258f` (file `0x16258f`). The trampoline preserves both negative-value checks, scales `ESI`/`EDI` with `(value * 5) >> 1`, calls `004c0900`, and returns at `005364d1`, keeping the type-definition and per-object copies synchronized. Runtime testing previously confirmed this setter path at 2x; the current 2.5x factor and its effect on the red dashed frame still require game testing.

The modifier never writes the four rejected `07` candidates. It only detects legacy two-site or four-site states and restores all four original shift-6 instructions. The option and preset field control only the runtime-verified setter trampoline. Unknown mixed bytes are left untouched with a warning.

Overlay type `0x28` at `00451650` is also rejected: callers are `igm_but_kampf_beserk` and `igm_but_kampf_normal` combat-mode controls.

Future work must start from the player build-order acceptance/rejection path and runtime breakpoints on the actual reported red-line drawing, not another search for similar shift instructions.

### 11.3 Other EXE Attempts

The former altar-limit assembly attempt caused crashes and is not present. Every future EXE patch requires a version signature, original/patched/restored bytes, a proven call path, and runtime verification.

## 12. `apt.dat`

`apt.dat` is identified as a ZIP-like container with `SYSTEM/DATA/APT/*.apt` binary entries. The modifier does not alter collision, UI layout, or repack this file. Former projectile collision expansion was restored. Keep it read-only until entry semantics, checksums, and runtime loading are proven.

## 13. Presets, Troop Files, and Saves

`.arpreset` is INI-like text using invariant numeric culture. It stores `MaxPopulation` and `FastCiviProduction` as switches plus the other global settings and nine-property troop rows. Legacy `PopLimit` and `CiviSpeed` fields remain import-compatible; only 1600 and 10x map to enabled. `VillageBuildRange` controls only the runtime-verified setter trampoline and cannot re-enable the rejected four-site patch.

`.artroop` rows use:

`UnitKey=HP,Dmg,VW,AW,Speed,Sight,Relt,Range,SpellRadius`

All 43 known units, including three priests and seven siege units, are supported. Imports validate keys, field counts, and numeric values before updating grids.

Save management scans live saves and ZIP backups. The only authoritative live-save root is `Path.Combine(GetGamePath(), "SAVE")`, which is `C:\Program Files (x86)\Against Rome\SAVE` for the default installation. The modifier must never treat `%LOCALAPPDATA%\VirtualStore` as a game-data source or patch target.

When this legacy game is started from `Program Files (x86)` without elevation, Windows UAC file virtualization can redirect relative writes to `%LOCALAPPDATA%\VirtualStore\Program Files (x86)\Against Rome\SAVE`, creating a separate set of saves. The modifier manifest uses `requireAdministrator`; its launcher points `FileName` at the selected `Against_Rome.exe` and sets `WorkingDirectory` to that same selected game directory. Direct game shortcuts must also run elevated to prevent VirtualStore from being recreated.

`BackupSaveCache` keys parsed `save.ini` metadata by archive path and last-write time. Deleting an archive also removes its cache entry. Restore must constrain all ZIP entries to the intended SAVE directory.

### 13.1 Local Save Consolidation on 2026-06-29

- The official `SAVE` tree and the complete VirtualStore game tree were backed up first under `%LOCALAPPDATA%\AgainstRomeModifier\MigrationBackups\20260629_155551`.
- The official tree already contained `ESAVE_000/001`. VirtualStore contained `ESAVE_000/001/002/003/004/009`; its conflicting `000/001` slots were preserved as official `005/006`, while the remaining slots retained their names.
- The newer official `game.cfg` remained active. The virtualized copy was retained as `SAVE\game.virtualstore-20260607.cfg`. Both `key.cfg` files had the same hash, so only the official copy was retained.
- Remaining virtualized crash dumps, compatibility logs, and `mod_info.md` were moved to the official game directory. After verification, the VirtualStore Against Rome directory was removed.
- `RUNASADMIN` was added to the per-user compatibility layer for `Against_Rome.exe`. The Start menu shortcut still targets the official executable and uses the official game directory as its working directory, so modifier and shortcut launches now share the authoritative data.

## 14. UI and Performance

- Borderless dark WinForms UI with sidebar-driven hidden-header tabs.
- Owner-drawn `ModernToggle`; every `CreateRoundRectRgn` handle is released with `DeleteObject`.
- Fixed table widths and vertical scrolling prevent column shifts.
- `_backupUnitRows` parses original `objdef.dau` once for default/current grids.
- Frequently used regular expressions are static, compiled instances.
- Physical log writes are locked; background work does not directly read UI controls.
- Error logs include exception messages and stack traces.

## 15. Reverse-Engineering Assets

The local Ghidra inventory contains 7,381 machine-generated functions:

- `re_workspace/ghidra_inventory/against_rome_function_index.csv`
- `re_workspace/ghidra_inventory/against_rome_decompiled_functions.c`

These are research artifacts, not original source and not proof that every function is understood.

Reusable sources:

- `tools/re/GhidraVillageRedFrameAnalysis.java`
- `docs/reverse-engineering/decompilation-workflow.md`
- `docs/reverse-engineering/exe-functions.md`
- `docs/reverse-engineering/known-patches.md`
- `docs/reverse-engineering/objdef-fields.csv`
- `docs/reverse-engineering/ress-fields.csv`
- `data/game_schema.json`

Use these before repeating whole-program analysis. Rebuild the inventory only for a changed EXE or missing analysis data.

## 16. Verification Checklist

- Build succeeds and JSON parses.
- Chinese and English canonical/embedded documents match.
- `git diff --check` passes.
- Every changed PFIL payload round-trips.
- `objdef.dau` decompressed length is unchanged.
- `ress.ini` preserves indexes 19-24, the pack-horse row, trailing fields, and line endings.
- `team.dat` changes only the general limit and index 4; `bver` remains unchanged.
- Every endless map matches a signature; mismatches are skipped and logged.
- EXE writes require exact known bytes.
- The village candidate never writes `07`; only known candidates may be restored to `06`.
- Runtime coverage includes map load, construction, upgrades, unit production/disarm, four factions, priest behavior, morale, villager speed, and the 24-villager equipment regression.
- AI Ultimate testing covers all five endless maps, respawn, action loops, and concurrent active forces.
- The current village result remains: all four candidate changes produced no visible effect.
- The setter trampoline was separately verified at 2x for the player-usable village construction range; the current 2.5x factor and red dashed frame remain verification items.

## 17. Known Limits

Machine decompilation cannot recreate every original source line, identifier, comment, or build project. A function inventory is navigation, not 100% semantic truth. Some `ress.ini` fields, `apt.dat` entries, and BCI opcodes remain candidates. AI Ultimate's count, timing, active-limit, and completed-job recycling changes still require a long-running endless-mode regression test; global civilian production/training edits are disabled after causing player resource-production regression. The setter path was runtime-verified at 2x for the construction range; the current 2.5x factor and its effect on the red dashed frame remain unverified.

Always separate a stored value from its runtime meaning. Proximity, naming similarity, or a plausible static formula is not sufficient proof.

## 18. Public Repository Boundary

- Publishable: C# source, `data/game_schema.json`, `docs/reverse-engineering/`, reproducible scripts under `tools/re/`, `tools/Repair-LanguageBackup.ps1`, and the redistributable `ThirdParty/dgVoodoo2/` integration files.
- Never publish: `遊戲原始檔案/`, `Original game archives/`, `Backup.zip`, game executables/data/maps/language/save payloads, or whole-program decompiler output derived from the original executable.
- Local-only: `.codex/`, `.agents/`, `re_workspace/`, IDE/build output, dumps/logs, language-backup directories, and `CodeAuditReport.md`.
- Before publishing, run `git status --short --ignored` and verify that local assets are marked `!!`; also inspect `git ls-files` for accidental game payloads.
- The dgVoodoo2 v2.87.3 upstream terms permit individual files to ship with a game or game mod. This project is not a general-purpose launcher/framework; provenance and terms are recorded in `ThirdParty/dgVoodoo2/REDISTRIBUTION.md`.
