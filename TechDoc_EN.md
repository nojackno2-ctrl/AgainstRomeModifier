# Against Rome Modifier Complete Technical Document

Updated: 2026-07-05.

This document describes the current code, data formats, reverse-engineering evidence, enabled patches, candidates, and rejected approaches. It is not a version history. Each feature has one current description. Reproducible runtime behavior and the latest concrete decompiler evidence take precedence over an older interpretation.

For the detailed maintenance chronology, debugging failures, checklists, and workflow guidelines intended for future AI agents, refer to the integrated chapters at the end of the Chinese technical document `TechDoc.md`. This file remains the English current-state specification.


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
| `src/Program.cs` | WinForms entry, elevation, High DPI startup, global exception handling. |
| `src/Core/GameLZSS.cs` | LZSS and `PFIL@` wrapper decode/encode with bounds checks. |
| `src/Core/Bci/` | BCI signature matching, word writes, and PFIL script handling. |
| `src/Core/EndlessAi/` | AI Ultimate M1-M14 modules, state detection, and orchestration. |
| `src/Core/TroopConfig.cs` | Field enums, unit IDs, names, factions, tiers, types, and balance baselines. |
| `src/Core/Patches/` | Pure, WinForms-independent patch logic: `ObjdefPatcher`, `RessPatcher`, `ClScriptPatcher`, `ClEparaPatcher`, `ClScintPatcher`, `TeamDatPatcher`, `ExePatchModel`, `VerifiedBinaryWriter`. Byte computation and fixed-offset state detection/planning live here so they can be unit-tested without WinForms or copyrighted game files. |
| `src/UI/ModifierForm.cs` | Main UI, controls, backup cache, parsed unit cache, shared state. |
| `src/UI/ModifierForm.Data.cs` | Current-data reading, CSV-like parsing, comparisons, icons, EXE state detection (delegates to `ExePatchModel`). |
| `src/UI/ModifierForm.DataExt.cs` | Safe access to cached original unit rows. |
| `src/UI/ModifierForm.Patches.cs` | Transactional writes, restores; delegates EXE/INI/DAU/team/BCI byte computation to `src/Core/Patches/` and only converts UI state to Options / logs results. |
| `src/UI/ModifierForm.Presets.cs` | Actions to enable/disable all features at once. |
| `src/UI/ModifierForm.SaveManager.cs` | Save discovery, ZIP backup/restore/delete, metadata cache. |
| `src/UI/TroopPresetForm.cs` | Nine-property editing for 43 units and `.artroop` I/O. |
| `src/UI/UIElements.cs` | Owner-drawn toggles, dark menu renderer, GDI disposal. |
| `src/Core/Localization.cs` | Chinese/English UI and log strings. |

The application targets `.NET 8`, `net8.0-windows`, WinForms, x64, nullable reference types, and `PerMonitorV2` DPI. `Backup.zip` is embedded only when present; both embedded technical documents are mandatory resources.

## 3. Backup, Transactions, and Apply Order

Original data is loaded from embedded `Backup.zip` when available. Public repositories must not publish original game assets. Without the archive, required originals are read from the selected game installation and retained in memory. `backupFiles` uses case-insensitive keys.

`FileRollbackScope` records every target before its first write. `SafeWriteAllBytes` writes through a temporary file in the target directory. Any exception restores all files to their state at the start of that operation. A successful operation calls `Commit()`, disposes the scope, and only then refreshes UI data. This is a per-operation transaction, not a persistent `.bak` system.

Apply order:

1. Validate the directory and `Against_Rome.exe`.
2. Load originals and confirm with the user.
3. Snapshot UI values on the UI thread.
4. Within the same rollback transaction, run `RestoreOriginalFilesInternal` without changing the UI snapshot. This shares the Restore All file path and clears modifier-managed EXE, stats, team.dat, endless AI, food-healing, language, and dgVoodoo2 state left by older builds.
5. Apply EXE compatibility state.
6. Apply `cl_script.ini`.
7. Apply `ress.ini`.
8. Apply `objdef.dau`.
9. Restore original `team.dat` files, then apply population only.
10. Apply endless-mode BCI changes.
11. Apply language resources.
12. Commit, dispose rollback state, and reload current values.

## 4. PFIL/LZSS and Text Compatibility

Known `PFIL@` users include `ress.ini`, `cl_script.ini`, `banner.ini`, `objdef.dau`, all `team.dat`, endless `ak_level.bci`, and some endless settlement `.sdl` files.

The decompressor rejects negative or greater-than-50-MB output sizes. The 4096-byte ring uses `& 4095`. Compression uses a 16-bit hash table, bounded hash chains, and guards against matching not-yet-updated short-distance ring data. Every changed payload should pass `decompress(compress(payload)) == payload`.

**Ring-init contract (fixed 2026-07-02):** the game EXE decompressor (`FUN_00565c00`) fills only the first `0xFEE` ring positions with spaces (`0x20`); the last 18 positions (`0xFEE..0xFFF`) stay `0x00` from `memset`, and the write cursor starts at `0xFEE`. The compressor's window model must match exactly. The old `GameLZSS` modeled the whole ring as spaces, so space runs within the first 18 output bytes could be matched against the "phantom spaces" at `0xFEE..0xFFF`; the game then decoded `0x00` there, and NULs truncated text files at the INI tokenizer (symptom: `No Mem len=0 CLMK\mk_tcon.c [1138]`, `.sdl` parsed as zero objects). Binary payloads (BCI) never triggered it because they contain no early space runs.

The game does not implement RFC 4180 CSV. CSV-like rows use `Split(',')` and `string.Join`. Adding quotes or escaping can make old engine object IDs and paths invalid, causing missing building buttons. Preserve original line endings, trailing empty fields, and cp1251 encoding.

## 5. Language Resources

English mode overlays resources from the selected game's `ToEng` directory. Before the first managed overlay, all 332 destination states are preserved in `.against-rome-modifier-language-backup`; disabling restores that baseline, including deleting files that did not originally exist. Enabling aborts if `ToEng` is missing or empty, and restore aborts instead of reporting success when an older modifier already overlaid every file without creating a baseline. Coverage includes:

- `SYSTEM/TEXT/US/`: menus, briefing, campaign, object names, multiplayer dialogs, messages, and tutorials.
- `SYSTEM/CLMK/DLG/`: text-bearing TGA buttons, result screens, settings, and menus.
- `MAPS/`: multiplayer/tutorial `briefing.put`, `netgame.put`, `text.put`, and related resources.

Language restore is independent from stat and compatibility restore.

## 6. `SYSTEM/cl_script.ini`

This is cp1251 text inside `PFIL@`. Compiled regular expressions locate `Radius`, `Value`, `Value2`, `CiviDelay`, `MoralsDecLostMem`, `MoralsDecFlee`, `MoralsDecOverPop`, and `MoralsIncIdle` while preserving comments.

- Spell Value / Value2 parameters are modified by the Spell Enhancement toggle to scale damage by 5x, healing by 50x, and Celt Spell3 resurrection health/morale to 100%.
- The core villager-speed switch writes every `CiviDelay` as 500 ms (the fastest valid 10x setting). When disabled, original backup values are preserved. The executable clamps the delay to at least 500 ms.
- Infinite morale uses the executable's minimum accepted `MoralsIncIdle` value of 500 ms and is described as rapid rather than instant recovery.
- Infinite morale zeroes decay parameters and sets the required idle recovery behavior.
- Balance mode applies a default 2.5x faction spell-radius multiplier.
- A custom KEL/HUN priest's ninth property overrides the faction multiplier as `SpellRadius / 500.0`. GER has no original Radius record and remains non-editable at zero.
- If no relevant feature is enabled, the original file is written; values are never repeatedly multiplied from an already modified file.

## 6a. `SYSTEM/CLAK/cl_scint.ini`

This is cp1251 text inside `PFIL@`. It handles the unit mapping aliases for summon/resurrect spells (`SpellODef` / `SpellODef2` key entries).
- Under Celt Spell3 (Raise Dead), `SpellODef` is changed from `KEL_INF00` (Swordsman) to `KEL_INF01` (Spearman), and `SpellODef2` from `KEL_SCH00` (Archer) to `KEL_INF02` (Heavy Infantry/Double Swordsman) when Spell Enhancement is checked.
- On restore, the backup file is written back.

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
| 42 | Storage capacity (`maxre`) |
| 73 | BuildTime (`buildt`) |
| 74 | UpgradeTime (`upgrdt`) |
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
- The 10x storage-capacity switch multiplies every positive original `maxre` (Index 42)
  value by 10 for all town halls and warehouses (building rows whose names start with `Bau`
  and contain `Hau` or `Lag`). It remains fully reversible (successfully runtime-verified in-game).
- The 10x fast-build/upgrade/repair switch scales down the original `buildt` (Index 73)
  and `upgrdt` (Index 74) values by a factor of 10 for all building rows (names starting
  with `Bau`), with a lower bound of 1 ms to prevent divide-by-zero or timer errors.
  Because repair rate is inversely proportional to build time in Against Rome's data-driven
  rules, shortening the build time simultaneously boosts building, upgrading, and
  repair speeds. This switch is fully integrated into apply, restore, and preset actions (successfully runtime-verified in-game).

The built-in balance layer is now a complete 43-entry final-value table in `TroopConfig.BalancedUnitStats`. Every entry stores `HP,Dmg,VW,AW,Speed,Sight,Relt,Range,SpellRadius`; enabling balance does not apply a generic tier matrix or post-process shield, two-handed, or unit-type multipliers. Static initialization requires the table count to match `UnitMeta` and every `UnitOrder` key to contain exactly nine values. Explicit `.artroop` values still override this fallback layer.

The intended asymmetry is: Roman has the strongest overall roster; Teuton has the highest melee output; Celt has the strongest infantry defense and foot-ranged roster, with slingers using high per-hit damage; Hun has the strongest cavalry, with horse archers using lower per-hit damage and faster reload. Infantry, cavalry, and leaders retain equal movement speed. `BalancedUnitStats` is the single authoritative source for exact final values.

## 9. `team.dat`

Every `MAPS/**/team.dat` is restored from its original first. The core switch then writes 1600 when enabled or leaves the restored values unchanged when disabled.

- Preserve `[maxteamobjgenerell]`; the loader ignores it.
- `[teamdata]` column 4 is the runtime per-team limit and is capped by the executable's global limit of 1600.
- `[teamdata]` index 4: replace only when the original value is greater than zero, so disabled team slots stay disabled.
- Index 5 is `bver`, a banner version in range 0-9, not an AI switch. The EXE combines faction and `bver` to resolve `banner.ini` sections such as `[volk%02ld_vicon_bver%02ld]` and `[volk%02ld_obdef_bver%02ld]`.

## 10. Endless `ak_level.bci`

AI Ultimate is refactored into 8 independent user-facing experience modules: M1 reinforcement size (P1), M2 Town Hall conversion batch (P10), M3 Dorfverteidigung defense batch (P12), M4 reinforcement cooldown (P3), M5 scheduler loop delay (P6), M6 guaranteed settlement spawning (P7), M7 starting resource stockpile (P13), and M8 four settled-AI quota (P16).
To prevent game logic deadlocks and respawn-related crashes, the 6 technical safety patches: P2 (completed-job slot recycle), P4 (defeat cleanup timeline), P5 (team-death verification loops), P11 (camp demolish delay), P8 (active unit limit), and P9 (zero retreat quota) are merged into R0 as background safety fixes. They are applied automatically under the hood to ensure robust AI execution. `src/Core/EndlessAi/EndlessAiOrchestrator.cs` owns module detection and application.

P15 is now a mandatory R0 safety repair. The two settled-party terminal
transitions at decompressed offsets `0x109E8` and `0x16374` must remain on the
vanilla `DELETE_PARTY (256)` path. A previous build changed them to
`DELETE_TEAM (257)`, which can delete the team while `ak_haupthaus.bci` waits
for an individual teardown acknowledgement. The missing acknowledgement can
stall the simulation loop indefinitely. Detection classifies 257 and mixed
256/257 states as Legacy, and both apply/restore migrate them to 256. P15 is no
longer part of user-toggleable M3. P2/P4/P5/P11/P8/P9 are also applied as permanent safety valves to prevent NPC job slot exhaustion, overlapping respawn crashes, and reinforcement freezes.

`MAPS/ENDL_*/SCRIPT/ak_level.bci` is a `BCI0` compiled-script payload inside `PFIL@`. Patches search opcode/literal signatures and have been found with the same local sequence in `ENDL_000` through `ENDL_004`.

- Military create call around decompressed `0x17B60`: interpreted as `s_addNPCJob_createUnit(local7, 3, 8, 0, 0, 4, 4, 1, 0)` after reversing BCI stack order.
- Count literals near `0x17B2C` and `0x17B34`: `4 -> 20`.
- Completed-job recycling flag near `0x17B1C`: `0 -> 1`, allowing completed military reinforcement jobs to release their NPC-job slots for later waves.
- Older builds edited three global CLAK economy scripts. `ak_npc.bci` (free-civilian reserve) and `ak_produktion.bci` (production gate) proved not NPC-scoped in runtime testing — they stop staffed player resource buildings even in a new game — and are always restored by R0. The third edit, `ak_haupthaus.bci` conversion size `[81,59] -> [66,20]` at `0x3FCC`, is controlled by M1: Ghidra decompilation of the `s_createBattleUnitsMax` implementation (`FUN_005249d0`) confirms the argument is the members-per-battle-unit count, clamped by the EXE to 0..20, and each call already converts all gathered idle civilians (up to 100) in batches of that size. The original runtime value is 6, matching the observed 6-man AI conversion units. A player manual-conversion regression check is still pending.
- 2026-07-03 correction: with only the `ak_haupthaus` edit, in-game AI conversions stayed at 6. The main-house call sits in the `var57 == 34` (CIVRECREATE_WAIT) branch and only fires in the military-reinforcement recreate chain; the village AI's day-to-day conversion runs through `Dorfverteidigung.bci`'s four `s_addNPCJob_createUnit(team, 1, type∈{1,2,6,3}, 0, 0, 6, 6, 1, 0)` sites (pushsym at decompressed `0xF1BC/0xF264/0xF30C/0xF3B4`). Args 6/7 are the per-unit member min/max (job `+0x11/+0x12`; EXE clamp 1..20 since arg 2 is 1); the job executor (~`00548700`) gathers that many idle civilians and calls `FUN_00523a00` once — one job creates one N-member unit, which also confirms the `ak_level.bci` military job counts (`4..4 -> 20..20`) are members-per-unit. AI Ultimate now patches all eight literals `6 -> 20` via the signature `[66,0, 66,1, 66,?, 66,?, 66,0, 66,0, 66,?, 66,1, 90,8, 128,157, 73,-9, 86]` (exactly four hits enforced); disabling restores 6. Runtime verified 2026-07-03: with the patch applied, the village AI converts 20 villagers into a single squad in-game.
- EXE path `0054aa80 -> 00547f50` clamps this mode to 1..20.
- Military reinforcement wait at decompressed `0x178E0`: `180000 -> 5000 ms`.
- The reinforcement donation formula remains original. The `v56[party]` retreat quota changes from `[90,15]` to `[66,0]`, handing the whole type-5 reinforcement party to the village instead of retreating. This patch is applied and restored atomically with the threshold of 40; the earlier quota-only combination with threshold 8 stopped arrivals after roughly one wave.
- Party retreat/cleanup deadlines use a mixed target. The four non-settlement sites `0x119C0`, `0x12FFC`, `0x13FE8`, and `0x17F38` change from `600000 -> 5000 ms`. The settled-party sites `0x10700` and `0x160EC` stay at `600000 ms`: states 51/52 normally wait for the engine's old-village/palisade cleanup condition, and the deadline is only a fallback. The earlier all-six-at-5000 build forced DELETE_PARTY before cleanup completed, allowing the same team to resettle while NPC village records still pointed at the old location. Apply recognizes that build as legacy-enabled and migrates it to the mixed target. The initial-arrival timeout at `0x7F24` also remains `600000 ms`.
- `SYSTEM/CLAK/SCRIPT/ak_haupthaus.bci` old-village cleanup cadence: the unique initialization sequence at `0x3248` sets the dead-village pass to `1500 + rand(-25,25)` ms, then the leave-village loop removes one confirmed building/palisade per pass. AI Ultimate changes only the base literal `1500 -> 100`, yielding `75..125` ms per object while preserving confirmations. A 71-object village therefore drops from roughly 105 seconds to roughly 7 seconds. Disable restores 1500; an enabled install still holding 1500 is accepted for migration. This script path also applies to a player village after its main building dies, but does not affect normal live-village production cadence.
- Dead-party confirmation counter at decompressed `0x1068C`: `20 -> 3` consecutive ticks (settled-party handler; counts ticks with village, leader, civilians, and members all gone before entering RETREAT).
- `ak_npc.bci` needs no patch for reactivation: its per-team state machine already calls `s_setNPCActive(team, 1)` when a healthy village exists for an inactive team. Save files are never modified.
- All six AI scheduler delay sites use `5000..10000` ms. The first three are inner raider timers; the last three initialize and refresh the outer scheduler that gates the settlement/military dispatcher. Leaving the outer sites at their original 60-240 seconds made AI arrivals slow even when the inner timers were accelerated. A `1000..2000` ms interim build caused computer respawns to stall in runtime testing and is rejected; Apply recognizes and migrates that state back to 5-10 seconds.
- Settlement-spawner default and 0/1/2/3-live-party probabilities are all set to 101, so spawning always triggers while an eligible team exists. In single player the occupied mask protects player team 0 and `pickTeam` selects only unoccupied CPU teams 1-7, giving a hard result of one player plus at most seven simultaneous CPU opponents without duplicating occupied teams.
- M8 changes new-game initialization from `s_randRange(4, 2) -> v70` to `s_randRange(4, 4) -> v70`. Four type-1 village AIs plus the separate type-4 military settlement produce five settled opponents. Disabling restores the vanilla random 2-4 range, and the former `s_randRange(3, 3)` state is migrated automatically. Because `v70` is initialized when the game starts, existing saves are not rewritten.
- Military-reinforcement unit-count threshold at decompressed `0x195F8`: `4 -> 40`; this is not an AI-player limit. A 2026-07-05 `ESAVE_000` snapshot showed the active Roman military settlement had only about nine 20-member units when the original main-house resource conditions stopped later waves; the earlier bounded gate still let transient leader/civilian state suppress subsequent waves. P8 now replaces the condition tail beginning at `0x1960C` with three equivalent `teamUnits < 40` branches. Earlier type-4-settlement, building, and one-active-type-5-party checks remain intact. The old unconditional `jmp +272` remains rejected because it also skipped the 40-unit bound and could exhaust job slots. Threshold 8, threshold 40 with the original gate, both earlier bounded gates, and the old unconditional gate are migrated on Apply.
- Older `112,272` gate bypasses and blanket 5000..10000 ms action-loop patches are migrated; only the three bounded reinforcement polling loops remain accelerated.
- Earlier enabled builds with original spawner probabilities, the prior first-three-only scheduler state, Gemini's interim all-six-loops state, or all six retreat deadlines at 5000 ms are detected as legacy-enabled. Apply migrates them to guaranteed spawning, six bounded scheduler delays at 5-10 seconds, and the protected settlement-cleanup deadlines.
- Disable/compatibility restore reverses every count, delay, limit, and gate value.
- Settlement templates: in `MAPS/ENDL_*/Endlos_*_Siedlung*.sdl` (plain INI text after PFIL decompression), the main building's `resv` line (namedef containing `_Haupt`; `Hauptzelt` for Romans) changes from `0,0,0,0,0,0` to `614,300,372,250,460,288` — each slot is the maximum observed across original campaign AI settlements — giving village-style AI a starting stockpile. Restore returns all zeros. The identical templates under `MP_*` stay untouched to match the ENDL-only scope.

The count represents created military units/formations; the visible individual-soldier total also depends on formation contents. The EXE provides only 20 NPC-job slots per team, so removing the gate entirely is not safe for long-running endless games.

### 2026-07-03 respawn incident

Read-only inspection of the current `ESAVE_002` (`ENDL_002`) explains why a
defeated computer still appeared to take a long time to return. The save was
written at 11:04:54, before the five live scripts were written at about
11:18:13, and Apply does not replace the `ak_level` copy embedded in
`CLAK\scr.dat`. The saved script has the expected 5000-ms military delay, six
5000-ms retreat deadlines, debounce 3, recycle 1, counts 20, threshold 8, gate
`66,0`, and six spawner probabilities of 101, but its first three polling loops
remain at the rejected `1000..2000`-ms state. That state is already known from
runtime testing to stall computer returns; lower polling intervals are not
monotonically safer or faster.

The saved decompressed BCI hash was
`4dd021f2f86336e6fc61a269c677ef7403cad833494d467c8fe1cd5580f771a2`.
The live `ENDL_002` decompressed payload hash was
`49839eb76743893b879be201c729c8104c09415acccc29928fbcea29eee02429` and
differed in 42,885 bytes. The live payload no longer matched normal BCI
opcode/signature structure even though selected literal offsets still showed
target values. A successful self round-trip only reproduced that invalid
payload and is not game-compatibility evidence. Restore all five live scripts
from a known-clean baseline, re-apply, then verify decompressed signatures
before starting a fresh endless game. Never rewrite the saved `CLAK\scr.dat`.

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

The current patch hooks `005364c1` (file `0x1364c1`) into a 289-byte executable zero-padding region at `0056258f` (file `0x16258f`). The trampoline preserves both negative-value checks, scales `ESI`/`EDI` with `value * 5`, calls `004c0900`, and returns at `005364d1`, keeping the type-definition and per-object copies synchronized. Runtime testing previously confirmed this setter path and synchronized red frame at 3x. The 5x machine-code and state migration are statically tested but still require in-game confirmation.

The modifier never writes the four rejected `07` candidates. It only detects legacy two-site or four-site states and restores all four original shift-6 instructions. The option and preset field control only the runtime-verified setter trampoline. Unknown mixed bytes are left untouched with a warning.

Overlay type `0x28` at `00451650` is also rejected: callers are `igm_but_kampf_beserk` and `igm_but_kampf_normal` combat-mode controls.

Future work must start from the player build-order acceptance/rejection path and runtime breakpoints on the actual reported red-line drawing, not another search for similar shift instructions.

### 11.3 Priest Spell Altar-Count Requirements

- Faction-level spell buttons are checked in `FUN_0044a010` (VA `0x0044a010`).
- It requires both data-driven `spruch` thresholds (via `ress.ini`) and hardcoded altar counts.
- The altar count limits are 1, 2, 3, 4 for spells 1-4, hardcoded as `cmp esi, N` immediates (12 sites: 3 factions × 4 spells).
- The patch replaces the immediate byte (cmp offset + 2) with `0x00` for all 12 sites, eliminating the altar count requirement.
- **Sites (file offsets):**
  - Germans (`FigGerPri00`): `0x4A0E3` (Spell 4), `0x4A112` (Spell 1), `0x4A136` (Spell 2), `0x4A15A` (Spell 3)
  - Celts (`FigKelPri00`): `0x4A1CC` (Spell 1), `0x4A249` (Spell 4), `0x4A293` (Spell 2), `0x4A2B7` (Spell 3)
  - Huns (`FigHunPri00`): `0x4A329` (Spell 1), `0x4A3A6` (Spell 4), `0x4A3F0` (Spell 2), `0x4A414` (Spell 3)
- **Safety**: State detection validates all 12 original sequences. Mixed or unknown states are ignored.

### 11.4 Other EXE Attempts

The former altar-limit assembly attempt caused crashes and is not present (replaced by the verified immediate patching). Every future EXE patch requires a version signature, original/patched/restored bytes, a proven call path, and runtime verification.

## 12. `apt.dat`

`apt.dat` is identified as a ZIP-like container with `SYSTEM/DATA/APT/*.apt` binary entries. The modifier does not alter collision, UI layout, or repack this file. Former projectile collision expansion was restored. Keep it read-only until entry semantics, checksums, and runtime loading are proven.

## 13. Troop Files and Saves

Global preset files (`.arpreset`) have been removed in favor of one-click "Enable All" and "Disable All" buttons.

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

- `dotnet test .\tests\AgainstRomeModifier.Tests\AgainstRomeModifier.Tests.csproj -c Release` passes (33 xUnit tests as of 2026-07-05). All fixtures are synthetic; no copyrighted game files are required, so this also runs in the `.github/workflows/ci.yml` CI job on a clean checkout. The legacy `tests/verify_split_patches` console project still depends on a local `遊戲原始檔案/` tree and is manual-only, not part of CI.
- `ExePatchModelTests` specifically covers the EXE fixed-offset patches (focus-loss, spell-altar, legacy village-range restore, village setter 2x/2.5x/3x/5x): state detection, enable/disable round-trips, migration from any legacy setter state to 5x and back, and abort-without-corruption when expected bytes don't match.
- Build succeeds and JSON parses.
- The current Chinese and English documents are included by the project as the
  intended embedded resources.
- `git diff --check` passes.
- Every changed PFIL payload round-trips.
- `objdef.dau` decompressed length is unchanged.
- `ress.ini` preserves indexes 19-24, the pack-horse row, trailing fields, and line endings.
- `team.dat` changes only the general limit and index 4; `bver` remains unchanged.
- Every endless map matches a signature; mismatches are skipped and logged.
- EXE writes require exact known bytes.
- The village candidate never writes `07`; only known candidates may be restored to `06`.
- Runtime coverage includes map load, construction, upgrades, unit production/disarm, four factions, priest behavior, morale, villager speed, and the 24-villager equipment regression.
- AI Ultimate testing must cover all five endless maps, late reinforcement
  waves, respawn, action loops, completed-job recycling, restore, and old saves.
- The current village result remains: all four candidate changes produced no visible effect.
- The setter trampoline now targets 5x for both the player-usable village
  construction range and red dashed frame. The shared path was runtime-verified at
  3x; the 5x factor still needs an in-game check.

### 2026-07-05: Leader-death glory-retention feature withdrawn

- In-game testing confirmed that attempting to completely disable the leader-death glory-retention behavior causes the game to crash. The behavior is therefore treated as unsafe to modify.
- The modifier no longer exposes the option, embeds the patched BCI, applies or detects the patch, or restores its script state. Enable All, normal Apply, Restore All, and Restore Stats all exclude this feature.
- The modifier no longer exposes or applies leader-death glory retention. Because the retired experimental script is confirmed to cause an access-violation crash in combat, modifier startup and the food-healing apply/restore path now identify that exact legacy `ak_anfuehrer.bci` payload (with either healing literal 1 or 10), rebuild it from the embedded vanilla file, and then apply the still-supported healing-value patch. Unknown or third-party scripts are not overwritten by this migration.
- The root cause is not yet isolated. Any future implementation must pass in-game validation; a successful build and static validation do not establish safety.

## 17. Known Limits

Machine decompilation cannot recreate every original source line, identifier, comment, or build project. A function inventory is navigation, not 100% semantic truth. Some `ress.ini` fields, `apt.dat` entries, and BCI opcodes remain candidates. AI Ultimate's count, timing, active-limit, and completed-job recycling changes still require a long-running endless-mode regression test; global civilian production/training edits are disabled after causing player resource-production regression. The setter path now targets 5x for both construction range and red dashed frame; runtime confirmation of the new factor is pending.

Always separate a stored value from its runtime meaning. Proximity, naming similarity, or a plausible static formula is not sufficient proof.

## 18. Public Repository Boundary

- Publishable: C# source, `data/game_schema.json`, `docs/reverse-engineering/`, reproducible scripts under `tools/re/`, `tools/Repair-LanguageBackup.ps1`, and the redistributable `ThirdParty/dgVoodoo2/` integration files.
- Never publish: localized original-game folders covered by `.gitignore`,
  `Original game archives/`, `Backup.zip`, game executables/data/maps/language/save
  payloads, or whole-program decompiler output derived from the original executable.
- Local-only: `.codex/`, `.agents/`, `re_workspace/`, IDE/build output, dumps/logs, language-backup directories, and `CodeAuditReport.md`.
- Before publishing, run `git status --short --ignored` and verify that local assets are marked `!!`; also inspect `git ls-files` for accidental game payloads.
- The dgVoodoo2 v2.87.3 upstream terms permit individual files to ship with a game or game mod. This project is not a general-purpose launcher/framework; provenance and terms are recorded in `ThirdParty/dgVoodoo2/REDISTRIBUTION.md`.
