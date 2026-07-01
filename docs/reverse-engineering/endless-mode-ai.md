# Endless Mode AI Spawn Notes

These notes cover the original endless-mode spawn logic found in
`MAPS/ENDL_*/SCRIPT/ak_level.bci` and the associated settlement templates.

## Confirmed Files

- `MAPS/ENDL_000` through `MAPS/ENDL_004` each contain
  `SCRIPT/ak_level.bci`.
- `ak_level.bci` is stored with the same `PFIL@` LZSS wrapper used by other
  game data files.
- After decompression, each endless script starts with `BCI0` and is
  `120657` bytes.
- The decompressed scripts differ by only 5 bytes between maps, so the endless
  AI logic is effectively shared across the five original endless maps.

## Spawn Modes

The script contains names for two separate setup states:

- `INIT_UNITSCIV`: civilian / settlement-style setup.
- `INIT_UNITSMIL`: military-unit setup.

This confirms the two original AI arrival modes are script-controlled states,
not `team.dat` column 5/6 behavior flags.

Related script symbols:

- `s_setVillageTemplate`
- `s_createUnitAndMems`
- `s_addNPCJob_createUnit`
- `s_addNPCJob_dissolveUnit`
- `placesSettle`
- `placesSpawn`
- `placesWaypoint`

## Settlement-Style Spawn

The civilian/settlement path references settlement template names:

- `Endlos_Ger_Siedlung1` through `Endlos_Ger_Siedlung5`
- `Endlos_Kel_Siedlung1` through `Endlos_Kel_Siedlung5`
- `Endlos_Hun_Siedlung1` through `Endlos_Hun_Siedlung5`
- `Endlos_Rom_Siedlung1` through `Endlos_Rom_Siedlung5`

The original `ENDL_000` data folder only contains `Siedlung1` and
`Siedlung2` files for each faction:

- `Endlos_Ger_Siedlung1.sdl`, `Endlos_Ger_Siedlung2.sdl`
- `Endlos_Kel_Siedlung1.sdl`, `Endlos_Kel_Siedlung2.sdl`
- `Endlos_Hun_Siedlung1.sdl`, `Endlos_Hun_Siedlung2.sdl`
- `Endlos_Rom_Siedlung1.sdl`, `Endlos_Rom_Siedlung2.sdl`

The `.sdl` files are also `PFIL@` compressed. They contain the prebuilt
settlement composition used by the settlement-style AI arrival. Editing these
templates is the most direct data-side way to change what the village-style AI
brings/builds.

## Military-Style Spawn

The military setup path references unit creation helpers rather than settlement
templates:

- `s_createUnitAndMems`
- `s_addNPCJob_createUnit`

Current BCI code offsets in the decompressed `ENDL_000` script:

- `0x08DEC`: reference to `INIT_UNITSCIV`
- `0x08E1C`: reference to `INIT_UNITSMIL`
- `0x0E584`: external call to `s_setVillageTemplate`
- `0x0A5D8`, `0x0A784`, `0x1A554`: references to `s_createUnitAndMems`
- `0x17B60`: external call to `s_addNPCJob_createUnit`

The external-call bytecode pattern observed so far is:

`0x80 <symbol-id> 0x49 <negative-argument-count> 0x56`

For the `s_addNPCJob_createUnit` call, the argument count is `-9`. The literal
arguments immediately before the call include `0`, `1`, `4`, `4`, `0`, `0`,
`8`, and `3`, plus one local variable argument. These are quantity candidates,
but the function signature still needs confirmation before exposing them as a
safe patch.

Ghidra decompilation of the EXE callback confirms:

- `s_addNPCJob_createUnit` is registered to script callback `0054aa80`.
- `0054aa80` forwards 9 integer arguments directly to `00547f50`.
- `00547f50` validates argument 1 as team `0..7`.
- Argument 2 must be `0`, `1`, or `3`.
- Argument 3 must be `0..9`.
- Argument 6 is clamped to `1..maxCount` and stored in the NPC job at
  offset `+0x11`.
- Argument 7 is clamped to `argument4..maxCount` and stored at offset `+0x12`.
- `maxCount` is `4` when argument 2 is `0`; otherwise it is `20`.

The BCI VM pushes call arguments in stack order and the callback receives them
reversed. Therefore the military job call at decompressed offset `0x17B60` is
currently interpreted as:

`s_addNPCJob_createUnit(local7, 3, 8, 0, 0, 4, 4, 1, 0)`

The two `4` values are the current military-job unit-count range. Because
argument 2 is `3`, the EXE clamps these count values to `1..20`. Changing both
values together should change the number of created military units/formations
for this job. This is still marked candidate until verified in-game, because
each "unit" may be a formation with multiple members rather than one individual
soldier.

The two count literals are stored as 32-bit BCI words at decompressed offsets:

- `0x17B2C`: first count literal, currently `4`
- `0x17B34`: second count literal, currently `4`

The bytecode sequence around `0x17B60` is identical in decompressed
`ENDL_000` through `ENDL_004`, so the same signature applies to all original
endless maps inspected.

## Current Interpretation

- AI arrival mode is controlled by `ak_level.bci` state logic.
- Settlement-style arrival uses `s_setVillageTemplate` plus faction-specific
  `Endlos_*_Siedlung*.sdl` templates.
- Military-style arrival uses script unit-creation jobs.
- The military create-unit job's current count range is `4..4`, clamped by the
  EXE to `1..20`.
- The modifier option `AI終極模式` changes this military count range to
  `20..20`, changes the respawn wait literal from `180000` ms to `5000` ms, and
  raises the active-party comparison literal at `0x195F8` from `4` to `8`.
  It also changes the last `s_addNPCJob_createUnit` argument at `0x17B1C` from
  `0` to `1`. EXE runtime analysis shows that this flag removes a job after its
  status leaves the running state, allowing the 20 per-team NPC-job slots to be
  reused by later reinforcement waves instead of retaining completed jobs.
  The first three military reinforcement polling loops are also changed to
  `5000..10000` ms so the 5-second cooldown is checked promptly. Other action
  loops retain their original pacing. Older builds changed every loop and
  bypassed the gate at `0x1960C` with `112,272`; the unrelated loop changes and
  gate bypass are restored because that unbounded combination could exhaust the
  20 job slots available to each team.
  Settlement/village-mode `.sdl` templates remain untouched.

### Rejected global village-production patch

Older builds changed `ak_npc.bci` (`0 -> 20` at `0x1EA0`),
`ak_produktion.bci` (`117 -> 112` at `0x3710`), and `ak_haupthaus.bci`
(`[81,59] -> [66,20]` at `0x3FCC`). Runtime testing proved these global paths
are not safely NPC-scoped: staffed player resource buildings remain at zero,
including in a new game. Current builds always restore the three original
values and keep AI Ultimate limited to endless-map reinforcement logic.
- `team.dat` still controls faction, population limit, and banner version, but
  it is not the source of the endless AI spawn-mode decision.

## Pending Work

- Decode enough of the `BCI0` bytecode instruction set to identify the branch
  that selects `INIT_UNITSCIV` versus `INIT_UNITSMIL`.
- Add a safe modifier patch that edits the decompressed BCI count literals and
  recompresses it as `PFIL@`.
