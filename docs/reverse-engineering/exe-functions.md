# Executable Functions And Offsets

Addresses are from the currently analyzed `Against_Rome.exe`. Function names are
provisional unless manually named in a Ghidra project.

## Full Local Inventory

The current local Ghidra project identifies 7381 functions in `Against_Rome.exe`.
Generated lookup artifacts are under ignored local workspace files:

- `re_workspace/ghidra_inventory/against_rome_function_index.csv`
- `re_workspace/ghidra_inventory/against_rome_decompiled_functions.c`

See `decompilation-workflow.md` for the exact headless command. The generated
pseudocode is local reverse-engineering material, not original source, and must
not be treated as proof of a function's gameplay meaning without call-path or
runtime evidence.

## Ress Parser

- `0046c1c0`: loads `SYSTEM/ress.ini`.
- `0042a230`: parser used by `0046c1c0` for named sections.
- `0046bd00`: `[objres]` callback.
- `0046b200`: `[volkres]` callback.
- `0073d36c`: referenced as volkres row/table storage.
- `0073d070`, `0073d078`: referenced as formation-related tables.

Known strings:

- `SYSTEM/ress.ini` at `005eedb9`
- `[maxskills]` at `005eedc9`
- `[objres]` at `005eeddb`
- `[volkres]` at `005eede6`
- `resv_res%ld_bau` at `005ee9b9`
- `resv_res%ld_upg` at `005ee9d9`

## Team Data And Banner Version

- `00469370`: loads/parses map `DATA/team.dat` sections.
- `00468fcc`: `[teamdata]` callback candidate. The parser call passes 6
  columns and 8 rows for `[teamdata]`.
- `0073ca88`: team structure base. Rows are 0x84 bytes each.
- `team + 0x00`: faction id parsed from `[teamdata]` column 1.
- `team + 0x04`: banner version (`bver`) parsed from `[teamdata]` column 5.
- `team + 0x80`: population limit parsed from `[teamdata]` column 4.
- `0046a850`: setter candidate for `team + 0x04`; validates
  `0 <= bver < 10`.
- `0046a890`: getter candidate for `team + 0x04`.
- `00469ab0`: loads `SYSTEM/banner.ini`.
- `0073b3f0`: banner lookup table base used by faction plus `bver`.

Known strings:

- `SYSTEM/banner.ini` at `005ee661`
- `[volk%02ld_vicon_bver%02ld]` at `005ee673`
- `[volk%02ld_obdef_bver%02ld]` at `005ee699`
- `SYSTEM/CLMK/DLG/BANNER/%s` at `005ee6ba`
- `[teamdata]` at `005ee4c1` and `005ee4ce`

## Unit Creation And UI Limits

- `00529f90`: script callback `s_createUnit`
- `0052a020`: script callback `s_createUnitAndMems`
- `0052a110`: script callback `s_createBattleUnitsMax`
- `0052a140`: script callback `s_createCiviUnitsMax`
- `0052a170`: script callback `s_unitMemsWeaponMax`
- `0052a3e0`: script callback `s_getNotHorseUnitMems`
- `005249d0`: create battle units max implementation candidate
- `00524d70`: create civilian units max implementation candidate
- `005251d0`: unit members weapon max implementation candidate
- `00527110`: get non-horse unit members implementation candidate
- `00523a00`: unit creation from gathered members candidate
- `00538320`: member gather/filter candidate

## Endless Mode Script

Findings are from decompressed `MAPS/ENDL_000/SCRIPT/ak_level.bci`.

- `0x08DEC`: BCI reference to state name `INIT_UNITSCIV`.
- `0x08E1C`: BCI reference to state name `INIT_UNITSMIL`.
- `0x0E584`: BCI external call to `s_setVillageTemplate`.
- `0x0A5D8`, `0x0A784`, `0x1A554`: BCI references to
  `s_createUnitAndMems`.
- `0x17B60`: BCI external call to `s_addNPCJob_createUnit` with 9 arguments.
- Observed external-call bytecode pattern:
  `0x80 <symbol-id> 0x49 <negative-argument-count> 0x56`.
- `0054aa80`: EXE script callback for `s_addNPCJob_createUnit`; forwards
  9 integer arguments to `00547f50`.
- `00547f50`: implementation for NPC create-unit jobs. Argument 1 is team
  `0..7`; argument 2 is restricted to `0`, `1`, or `3`; argument 3 is
  restricted to `0..9`; arguments 6 and 7 become the clamped unit-count range.
  If argument 2 is `0`, max count is 4; otherwise max count is 20.

## Village Bounds And Red Overlay Candidates

- `0054af80`: script callback `s_setVillageTemplate`.
- `00549500`: village-template implementation; resolves template name and loads
  village building/palisade data.
- `005363e0`: returns current team village position through `FUN_0050f750`.
- `00536580`: returns village half-size candidates from object data at
  `DAT_021460a8` and `DAT_021460aa`.
- `00536630`: calculates logical village bounds as
  `center +/- (halfSize * 0x40 + 0x20)`.
- `00536820`: tests whether a world coordinate is inside the village bounds,
  with an optional `param_4 * 0x40` margin.
- `005368c0`: clamps or projects a coordinate to the village bounds.
- `00421c00 -> 00520320 -> 005203a0`: overlay creation path. `00520320`
  converts two world coordinates through `00518d30`, then calls `005203a0`.
- `0044e990`: selected-object/build interaction candidate; creates overlay
  type `0x28` through `FUN_00421c00(DAT_00736a4c, DAT_00736a50, x, z, 0x28,
  param_1 == 1, 0, 0, 0, 5)`.

Rejected candidate:

- `0x1366c4` and `0x1366cd` changed the `00536630` multipliers from shift `6`
  to shift `7`, but this did not enlarge the visible selected-village red
  dashed frame. The modifier now only restores those bytes if an old build
  already wrote them.

## Implemented EXE Patch

Focus-loss pause patch:

- File offset: `0x161a88`
- Original bytes: `89 15 C4 7D 9E 02`
- Patched bytes: `90 90 90 90 90 90`

The modifier verifies the expected original bytes before writing.
