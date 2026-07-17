# Endless Mode AI Spawn Notes

> Current safety status reviewed 2026-07-16: use the bounded `20..20` military count, `30000 ms` reinforcement wait and scheduler interval, active-party limit `8`, and the bounded save/load deadline repair. The runtime has 20 NPC-job slots; the unconditional gate bypass remains rejected.

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

The decompressed payload is plain INI text: a `[settlement]` header followed
by `[objectNNNN]` sections, one per placed object. Confirmed fields include
`namedef` (object definition name), `def` (numeric id), `pos`, `team`,
`nation`, `anzv` (unit counts on `Ver*Icon` formation entries), and `resv`
(six comma-separated stored-resource values). Original campaign templates
prove `resv` is the engine-native way to grant AI starting stockpiles, e.g.
`KAMP_006\Team_5.sdl` main house: `resv=614,300,372,250,460,140`. Endless
templates ship with all-zero `resv`.

### Settlement main-house starting-resources patch (implemented)

AI Ultimate M5 additionally rewrites the main-building `resv` line in every
`MAPS\ENDL_*\Endlos_*_Siedlung*.sdl`:

- Main building `namedef` contains `_Haupt`: `Haupthaus` for Germans, Celts,
  and Huns; `Hauptzelt` for Romans.
- Enabled: `resv=0,0,0,0,0,0 -> 614,300,372,250,460,288` (each slot is the
  maximum observed across original campaign AI settlement templates, so all
  values are in an engine-proven range).
- Disabled/restore: back to all zeros.
- Text is decoded/encoded as Latin-1 for byte-exact round-trips; the PFIL
  recompressor updates the header's uncompressed-size field, so the changed
  line length is safe (header offset 16 is the only size field).
- 42 templates across `ENDL_000..004` verified to round-trip with exactly one
  changed line each. The equivalent templates under `MP_000..004` are left
  untouched, matching the `ENDL_`-only scope of the script patch.

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
- AI Ultimate M1 changes this military count range to
  `20..20`, changes the military reinforcement wait from `180000` ms to
  `30000` ms, shortens four non-settlement party retreat deadlines from
  `600000` ms to `60000` ms while preserving both settlement-cleanup fallbacks
  at `600000` ms (see "Party lifecycle" below), cuts the dead-party
  confirmation counter from `20` to `3` ticks, and raises the reinforcement
  unit-count threshold at `0x195F8` from `4` to `70` (legacy `8`/`30`/`40`
  states migrate on apply).
  It also changes the last `s_addNPCJob_createUnit` argument at `0x17B1C` from
  `0` to `1`. EXE runtime analysis shows that this flag removes a job after its
  status leaves the running state, allowing the 20 per-team NPC-job slots to be
  reused by later reinforcement waves instead of retaining completed jobs.
  All six scheduler delay sites are changed to `30000..30000` ms. The first
  three are inner raider timers; the final three are the outer scheduler's
  initial and refresh ranges, which gate the dispatcher that calls the
  settlement and military-reinforcement spawners. Accelerating only the first
  three therefore still allowed 60-240 second arrival gaps. Older builds also
  bypassed the gate at `0x1960C` with `112,272`; that gate bypass, rather than
  the bounded scheduler acceleration, could exhaust the 20 job slots available
  to each team and remains disabled.
  A later `1000..2000` ms experiment was also rejected after runtime testing
  showed computer respawns could stop; it is recognized only for migration to
  the bounded 30-second scheduler.
  Settlement/village-mode `.sdl` templates get the main-house
  starting-resources rewrite described above; their building layout is
  otherwise untouched.

## Party Lifecycle And Defeat Recovery (decoded 2026-07-02)

`ak_level.bci` manages up to 16 "party" slots. Script arrays (indexed by party
slot): `v47` = party type, `v48` = state, `v49` = team, `v61` = deadline
timestamp, `v63[type]` = live count per type. State numbering (from the debug
name formatter at `0x8C8C`): 0 STOP, 1 IDLE, 16..23 INIT chain
(20 = INIT_UNITSCIV, 21 = INIT_UNITSMIL), 32..35 CIV states, 48..52 RETREAT
chain, 256 DELETE_PARTY, 257 DELETE_TEAM.

- Party creation (`0x7B94`): finds a free slot, sets `v49/v47`, state := 16,
  and `v63[type]++`. Only slot release clears `v47` and decrements `v63`.
- Occupied-team mask (`fn 0x81E8`): `base | (1 << v49[i])` for every slot with
  `v47[i] != 0`, where base is `v18` (net game) or `1` (single player, protects
  team 0). `pickTeam (0xC3DC)` chooses among `~occupied & tribeMask & v68`
  (`v68` = CPU-controlled teams, `255` in single player).
- Spawners: Siedler spawner (`0x18828`) rolls 80/60/40/20 % by `v63[1]`
  (count >= 4 -> 0 %); military-reinforcement spawner (`0x18E8C`) requires an
  existing settled team with >= 2 buildings; three additional raider spawners
  hang off the `v72/v74/v76` polling timers.
- New-game initialization stores `s_randRange(4, 2)` in `v70`, so vanilla
  chooses a type-1 settlement cap of 2, 3, or 4 for that game. The settlement
  spawner stops once `v63[1]` reaches `v70`. Type-4 military settlement parties
  are counted separately but consume the same finite CPU-team pool. M8 therefore
  changes both bounds to `4`, producing `s_randRange(4, 4)`: four type-1
  villages plus the separate type-4 military settlement keep five settled
  opponents while leaving team slots available for military/attack parties.
  The former M8 implementation `s_randRange(3, 3)` is recognized as legacy and
  migrated. Existing saves retain their already-initialized `v70` value.
- AI Ultimate changes the Siedler spawner's default and 0/1/2/3-live-party
  probabilities from `0,0,80,60,40,20` to six `101` literals. In single player
  the occupied mask reserves player team 0 and `pickTeam` can select only
  unoccupied CPU teams 1..7, so the effective bound is one player plus seven
  computer opponents. Once every eligible CPU team is occupied, `pickTeam`
  cannot create another opponent; after defeat cleanup frees a team, the same
  path immediately makes that team eligible to return.
- Settled parties (types 1 and 4; handlers at `0xF144` and `0x144AC`, covering
  both INIT_UNITSCIV and INIT_UNITSMIL) sit in state 1 (IDLE) and check
  `s_getVillageCenterObj` EVERY tick. When village, leader, and civilians are
  all gone, a consecutive-failure counter (literal `20` at decompressed
  `0x1068C`) sends the party into the RETREAT chain with
  `v61[party] := s_getTime() + 600000`.
- The vanilla RETREAT chain reaches DELETE_PARTY (256) when its cleanup
  condition completes or its `v61` fallback deadline expires. In the
  settled-party handlers, states 51/52 wait on village/palisade teardown state;
  the deadline is not the normal fast path. The two settled terminal writes are
  at `0x109E8` and `0x16374` and remain DELETE_PARTY. A previous P15 build
  changed them to DELETE_TEAM (257), but that can delete the team while
  `ak_haupthaus.bci` is waiting for a per-object cleanup acknowledgement. If the
  recipient disappears before acknowledging, the sequential teardown protocol
  can wait forever and stall the simulation loop. P15 is therefore now an R0
  migration repair: it detects 257 as legacy and restores both sites to 256.
  Other party families also retain state 256.
- The six retreat/cleanup deadline literals share the BCI word signature
  `[81,61, 90,-3, 128,83, 86, 66, <ms>, 32, 44, 164]` at decompressed value
  offsets `0x10700`, `0x119C0`, `0x12FFC`, `0x13FE8`, `0x160EC`, `0x17F38`.
  Two same-shaped sites are deliberately excluded: the initial-arrival timeout
  at `0x7F24` (no `44` word; must stay `600000` or arrivals would retreat
  before settling) and the military reinforcement wait at `0x178E0` (followed
  by `pushlit 34` = CIVRECREATE_WAIT; patched separately).
- AI Ultimate accelerates only `0x119C0`, `0x12FFC`, `0x13FE8`, and `0x17F38`
  to `60000`. Settled-handler sites `0x10700` and `0x160EC` remain `600000`.
  The previous all-six-at-5000 state forced DELETE_PARTY before the old village
  registry and palisades were cleared. A 2026-07-03 `ESAVE_000` snapshot showed
  team 3 active again while retaining 71 old village records and the old
  village base coordinate `(1632,5792)`; villagers then targeted that old site.
  Apply accepts the all-six state only for migration to the mixed target.
- Actual village teardown is driven by `ak_haupthaus.bci`, not by the party
  deadline. Its unique initialization sequence at `0x3248` sets the dead-house
  loop delay to `1500 + rand(-25,25)` ms; the leaving-village branch searches
  buildings inside the team village, sends one cleanup message, waits for its
  confirmation, and yields for that delay before the next object. AI Ultimate
  changes the base literal to `100`, preserving the one-at-a-time confirmation
  protocol while reducing a 71-object village from about 105 seconds to about
  7 seconds. The final fixed 2000-ms wait at `0x5000` is only a final-state
  confirmation and is deliberately left unchanged.
- `0x17F38` is therefore the type-5 handler's RETREAT_INIT deadline, NOT a
  "village defeat respawn timer". The 2026-07-02 save read (teams stuck at
  `npcActive=0` with all job slots free while `0x17F38` was already `5000`)
  disproved the old interpretation.
- `ak_npc.bci` needs no patch for reactivation: its per-team state machine
  already calls `s_setNPCActive(team, 1)` (call site `0x30F4`) whenever a
  healthy village exists for an inactive team, and `s_setNPCActive(team, 0)`
  (`0x3420`, `0x41BC`) when the village is lost. The EXE stores the flag in
  `DAT_029e6000[8]` (`FUN_00548ce0` setter / `FUN_00548d20` getter); no other
  EXE writer exists besides level init.
- The `s_netGame`-gated defeat handler at `0x1AA0C` (clears `v18/v41` team
  bits, plays `ENDL_ALL_%02i.wav`) recycles defeated HUMAN player slots in
  multiplayer; it is not part of the AI respawn path and is left untouched.

### Rejected global village-production patch

Older builds changed `ak_npc.bci` (`0 -> 20` at `0x1EA0`),
`ak_produktion.bci` (`117 -> 112` at `0x3710`), and `ak_haupthaus.bci`
(`[81,59] -> [66,20]` at `0x3FCC`). Runtime testing proved the first two
paths are not safely NPC-scoped: staffed player resource buildings remain at
zero, including in a new game. Current builds always restore those two
original values.

### Main-house conversion-size patch (re-enabled)

The `ak_haupthaus.bci` edit was re-examined in isolation and is controlled by
AI Ultimate M1:

- The site at decompressed `0x3FCC` (unique hit for signature
  `[?, ?, 81, 11, 81, 10, 81, 98, 128, 81, 73, -4, 86]`) pushes the last
  argument of the `s_createBattleUnitsMax` external call (symbol #81 in this
  script's `SYMBCONS` table).
- The EXE registers `s_createBattleUnitsMax` with signature `i_iiii` via the
  trampoline at `0052a110`, which forwards to `FUN_005249d0`.
- `FUN_005249d0` clamps the count argument to `0..20`, gathers up to 100 idle
  civilians from the village, and converts all of them in batches of that
  size — each batch becomes one battle unit via `FUN_00523a00`. The count is
  therefore members-per-unit, and conversion already continues until the idle
  civilian pool is exhausted.
- Original runtime value is 6 (the observed 6-man AI conversion units);
  `[81,59] -> [66,20]` raises it to the EXE maximum of 20.
- `var 59` has no `82,59` store anywhere in the script, so it is populated by
  the runtime/message context rather than script code.
- Pending: an in-game regression confirming the player's manual conversion UI
  is unaffected (it uses a separate UI path, and the documented player
  breakage came from the `ak_npc`/`ak_produktion` paths).
- Caveat (2026-07-03): this call sits inside the `var57 == 34`
  (CIVRECREATE_WAIT) branch, so it only fires in the military-reinforcement
  recreate chain. The village AI's *day-to-day* civilian-to-squad conversion
  does NOT go through it — see the Dorfverteidigung section below.
- `team.dat` still controls faction, population limit, and banner version, but
  it is not the source of the endless AI spawn-mode decision.

### Reinforcement-party retreat quota (v56) — units handed over instead of retreating

**RESOLVED & RUNTIME-CONFIRMED 2026-07-08.** The complete fix for "Roman
reinforcements" needs THREE P9 control points working together, established over
two sessions of disassembly and two rounds of in-game testing. Final confirmed
result: reinforcements arrive **with soldiers** (not villagers only), **stay in
the village as garrison instead of retreating**, military (type-4) AI still
spawns normally, and destroyed ordinary villages still respawn. The three points:

| Control point | Offset | Vanilla | Ultimate | Role |
| --- | --- | --- | --- | --- |
| Site 8 (spawn budget) | `0x16A44` | `[90,6]` | `[90,6]` (kept) | `v56` soldier spawn budget — must stay vanilla or reinforcements have no soldiers |
| Site 9 (retreat quota) | `0x17880` | `[90,15]` | `[66,0]` | zero retreat quota → over-quota units donated, not retreated |
| Type filter (state 49) | `0x1825C` | jz `92` | jnz `92` (opcode `117 -> 118`) | inverted condition, in-game confirmed 2026-07-17: squads (type != 1) enter the zeroed quota and are donated (garrison); type-1 units (pack horses / civilians) take the vanilla retreat path off-map. The former jz `0` fall-through donated EVERYTHING and made supply pack horses pile up every wave (ESAVE_001); it is Legacy and auto-migrates. See the 2026-07-17 update below |

The "donated squads may stand passively" caveat noted below during static
analysis **did not materialize** — in-game the garrison behaves correctly, so no
further release/dissolve rework was needed. Detailed decode of each point
follows.

**UPDATE 2026-07-08 (second session): the state-49 donation walk has a UNIT-TYPE
FILTER that exempts soldier squads from donation.** Runtime report after the
site-8 fix below: soldiers now spawn, but they still retreat even with the
site-9 quota verified in-place as `[66,0]` on all five installed maps. Full
decode of the retreat chain (with the corrected jump rule: bcitool `dis`
prints jump targets 8 bytes short; real target = printed + 8):

- State 48 (`0x17ce0..0x17f34`) does NOT issue movement orders — it collects
  all own-marked (`32+party`) team units into the party object array
  (`v52`/`v53`) via internal fn `0x94A8` (append + counters; flag 1 also does
  `s_setScriptMode(1)` — used at spawn).
- State 49 (`0x17f3c..0x18458`): finds a recipient via internal fn `0xABF8`
  (first party with `v47[i]==4`, else -1 → mark `-1`), re-marks the leader,
  then walks the array: **only units with `s_getUnitType(obj) == 1` enter the
  quota logic** (`local33 >= v56[party]` → donate via `s_setObjMark`); units
  with type != 1 — soldier SQUADS created by `s_createUnitAndMems` — take the
  else branch and are ALWAYS kept in the retreat array regardless of quota.
- State 50 walks the remaining array via `0xB0E8`: `s_sendMsg(8, exitX, exitY)`
  each tick and `s_destroyObj` within distance 100 of the exit; then
  DELETE_PARTY (`0x8358`), which destroys every team unit still marked
  `32+party` (donated/re-marked units survive).
- Also decoded: the REAL vanilla delivery mechanism is state 32
  (`0x16efc..0x174b0`): units within 400 of the village center are handed over
  one at a time via `s_sendMsg(6, …)` + `s_addNPCJob_dissolveUnit`; distant
  idle units are continuously ordered toward the village. State 32 only exits
  to 33 when the live array count reaches 0 or the village center is gone.
- Fix shipped: P9 third control point — the type-filter jz at `0x1825C`
  (signature `[128,214, 73,-2, 86, 66,1, 96,102, 117,92]`, the file's only
  `s_getUnitType` call) operand `92 -> 0` (fall-through), so squads are also
  subject to the zeroed quota and get donated via `s_setObjMark` instead of
  retreating. Static-analysis caveat (later DISPROVEN in-game): donated squads
  keep script mode 1 (no `s_setScriptMode(0)` on this path — the proper release
  helper `0xBBC0` does mode-0 + `sendMsg(6,…)` but is only used by settled-party
  death), so it was feared they might stand passively at the village. Runtime
  testing 2026-07-08 confirmed the garrison behaves correctly, so this path was
  left as-is.

**UPDATE 2026-07-17 (FINAL, in-game confirmed): the type filter changes from
"eat" (jz operand 0) to "INVERT" (jz -> jnz, operand kept 92).** Chronology:

1. In-game report (ESAVE_001): the shipped jz+0 (eat-the-condition) variant
   donated every unit of the retreating type-5 party — supply pack horses
   (`FigTiePac00_Packpferd`) included — so every reinforcement wave permanently
   added pack horses to the Roman camp and they piled up.
2. Fix: invert the branch (jz(117) -> jnz(118), operand kept 92). Consistent
   with the 2026-07-08 type semantics (soldier squads type != 1; pack
   horses/civilians type 1): squads fall INTO the zeroed-quota branch and are
   all donated via `s_setObjMark` (garrison, unchanged goal); type-1 units
   take the jump to the else branch (`local35 += 1`, no donate flag), stay in
   the retreat array, and leave via the vanilla state-50 exit walk
   (`s_sendMsg(8, exit)` + `s_destroyObj` within 100).
3. In-game confirmed on a new endless game: **soldiers garrison, pack horses
   retreat off-map**. (An initial user report claimed the mirror image —
   soldiers retreating, horses staying — which briefly led to a
   restore-the-vanilla-filter build the same day; the user then corrected the
   observation and the jnz inversion was reinstated as final.)

EXE decode of `s_getUnitType` for reference: callback `0x52a190` →
`0x5256d0` → `0x525700`, returns 0 if flags byte bit `0x10` is set, 1 if bit
`0x20`, else -1 (flags at `0x2146086 + 312*idx`, classifier at `~0x513d40`).

Trade-off: the vanilla behavior of donating 2–3 civilians per wave to the
village no longer happens (village population growth still comes from the
`Dorfverteidigung`/`ak_npc` conversion paths). The signature wildcards both
the jump opcode and operand words (`[128,214, 73,-2, 86, 66,1, 96,102, ?, ?]`).
Detection: filter `jz 92` = Original, `jnz 92` = Ultimate, anything else
(including the shipped `jz 0`) = Legacy, auto-migrated on the next apply.
Saves created with a legacy script keep the old behavior — the script lives
inside the save (`CLAK/scr.dat`), so a new game is required.

**UPDATE 2026-07-08: site-8 zeroing REVERTED — that write is the soldier SPAWN
BUDGET, not a retreat quota.** Runtime report: with both P9 sites at `[66,0]`,
type-5 reinforcements arrived with villagers only, no soldiers. Disassembly of
the type-5 INIT (state 16, `0x16984..0x16ab0`) shows `v57[party] =
s_randRange(2,3)` (civilian count) and `v56[party] = s_randRange(2,4)` (site 8,
`0x16A44`) — then state 20 (`callint -50000`) creates civilians from v57 and
state 21 (`callint -49732` -> internal fn `0xA964`) creates soldiers: `0xA964`
reads `v56[party]` at `0xA9CC`, compares it against `v54[party]` (spawned so
far), calls the unit-creation subroutine (`callint -1036`/`-1328`) and
decrements `v56[party]` by each spawned batch (`0xABA0..0xABD4`). Zero budget
=> zero soldiers. Zeroing site 8 also adds nothing against retreating: the
spawn loop drains v56 anyway, and site 9 (`0x17880`) rewrites it with the
donation remainder before the retreat chain. P9 therefore now patches ONLY
site 9 to `[66,0]`; site 8 is ALWAYS kept/restored to vanilla `[90,6]`, and
the legacy both-zeroed state is detected as Legacy and migrated on apply.
Other init-site v56 writes (`0x111C8` randRange(40,100), `0x121F8`
randRange(40,80), `0x136A8` clamp 20, `0x1471C` randRange(3,5)) are the same
spawn-budget pattern for other party types; the only v56 READS in the file are
`0xA9CC` (spawn loop) and `0x18168` (state-49 donation walk).

**UPDATE 2026-07-03 (later session): RE-ENABLED with the missing piece.** The
root cause of both rejected attempts below is identified as the spawner
  threshold at `0x195F8`: it was still `8`, so permanently donated units pushed
`s_searchTeamUnits(team)` past the spawn condition after about one wave and
reinforcements stopped. AI Ultimate now applies the `v56 <- pushlit 0` quota
patch TOGETHER with raising the threshold `4 -> 70` (legacy `8`/`30`/`40`
migrated).
Decoding state 49 (`0x17F60..0x183D8`) confirms handed-over units and the
leader are only re-marked via `s_setObjMark` (to `32 + type4Party`, or `-1`
when no type-4 party exists) and are NOT inserted into the type-4 party's
`v52` object array, so the settled handler's dead-party check (leader +
civilians + tracked members, debounce at `0x1600C` region) is not blocked by
them and team recycling still works. The army is naturally bounded: the
spawner stops sending waves once the team holds ~30 battle units (600 members
at 20 per unit), below the 1600 EXE population cap; the engine's population
check is the hard stop if a map's `team.dat` limit is lower. Historical
record of the two earlier single-sided attempts follows.

Decoded 2026-07-03 while investigating "reinforcement squads retreat after
delivering". The type-5 (military reinforcement) handler's flow in
`ak_level.bci`:

- Before entering the CIVRECREATE state (34), the handler computes how many
  of its own battle units to DONATE to the village:
  `donate = min(4 - teamUnits, 2, partyUnits / 2)` (sequence at decompressed
  `0x17750..0x17888`), then writes `v57[party] = civiQuota` and
  `v56[party] = partyUnits - donate` — v56 is the RETREAT quota.
- State 34 creates civilian-recreate NPC jobs; on quota/deadline it enters
  the retreat chain (48). State 48 orders party-marked units toward the exit;
  state 49 walks the party's object array (`v52`/`v53`): the first `v56`
  battle units stay in the retreat array, and every unit BEYOND the quota is
  released via `s_setObjMark` (reassigned to another type-4 party if one
  exists, else mark `-1`) — this is the vanilla donation path. State 50
  clears the arrays and enters DELETE_PARTY (256), freeing the slot.
- REJECTED (2026-07-03 runtime): an earlier build changed the v56 write at
  `0x17888` region (signature
  `[81,57, 90,-3, 90,14, 164, 81,56, 90,-3, 90,15, 164, 81,61]`, unique)
  from `pushloc 15` to `pushlit 0` so every unit stayed. In-game this
  stopped Roman reinforcements entirely and broke team re-arrival, because:
  (a) the military spawner's condition block counts the team's battle units
  via `s_searchTeamUnits` and only spawns below the literal at `0x195F8`
  (see below) — permanently donated units push the team over the threshold
  forever; (b) lingering handed-over units can keep a defeated team from
  passing the all-gone dead-party confirmation, blocking slot recycling.
  This quota-only configuration was rejected. Current builds use `[66,0]`
  only together with the corrected bounded threshold (currently 70).
- Second attempt (also reverted 2026-07-03, same session): widen the vanilla
  donation formula instead of zeroing the quota. The state-33 block at
  `0x17788` computes `donate = min(TH - teamUnits, CAP, partyUnits / DIV)`
  with vanilla `(TH,CAP,DIV) = (4,2,2)`. Raising this to `(8,4,1)` still
  broke reinforcement arrivals in-game (reported: "Roman reinforcements
  stopped entirely, and the AI Ultimate threshold change also
  seemed to have no effect"). That attempt was rejected; the formula remains
  vanilla. The later analysis isolated the quota-only failure to the low
  threshold described above.
  **Current state: the donation formula `(TH,CAP,DIV)` remains vanilla
  `(4,2,2)`, while AI Ultimate sets v56 to `[66,0]` atomically with threshold
  30. Disabling restores `[90,15]` and threshold 4.** Correction (2026-07-03): an
  earlier draft of this note claimed dead code named
  `EndlessAiRetainLegacyOpcode/Value`/`EndlessAiDonationUltimateValues`
  remained for migrating legacy-enabled scripts/saves back to vanilla;
  `git log -S` shows those identifiers were never committed, and no such
  migration/detection code existed at that point. Current code explicitly
  detects the v56 state and enforces the quota/threshold pair across all five
  `ENDL_*` maps.
- `0x195F8` correction: this literal was previously documented as an
  "active-party limit". Decoding the military spawner (`0x18E90..0x1974C`)
  shows it is the reinforcement unit-count threshold: the spawner requires
  `v63[5] == 0` (one reinforcement party at a time), a settled type-4 party
  whose team has >= 2 buildings, main-house storage checks, a leader check,
  and `s_searchTeamUnits(team) < <0x195F8 literal>`. A 2026-07-05 save proved
  the main-house resource checks can become false with only about nine Roman
  units present. Earlier experimental builds tried two bounded,
  unit-count-only condition tails, but current P8 does not install either one:
  it changes only the threshold literal at `0x195F8` to `70` and keeps the
  complete original condition tail at `0x1960C`. The original type-4
  settlement, buildings, resources, leader, civilian, and one-active-type-5
  reinforcement-party checks therefore remain. The old `112,272` bypass
  skipped this whole condition block and could exhaust job slots; it and both
  bounded experimental tails are recognized only for migration back to the
  original condition words.
  P8 migration is signature-closed: only the complete original condition tail,
  the exact old unbounded tail, and the two exact historical bounded tails are
  recognized, and only with thresholds `4`, `8`, `30`, `40`, or `70`. Any other
  gate/threshold combination is `Unknown` and remains byte-identical.
- A second `v56 <- pushloc 15` write exists at `0x111EC` but belongs to a
  different handler's INIT chain (value from `randRange(40,100)` context);
  the longer signature excludes it deliberately.

### Dorfverteidigung village-defense jobs (the actual 6-man conversion path)

Runtime testing showed the main-house patch alone leaves the observed AI
conversions at 6 members. Root cause: the village-style AI's routine
civilian-to-battle-unit conversion is driven by `Dorfverteidigung.bci`
(SYSTEM\CLAK\SCRIPT), not by `ak_haupthaus.bci`.

- Four `s_addNPCJob_createUnit` call sites (decompressed pushsym offsets
  `0xF1BC`, `0xF264`, `0xF30C`, `0xF3B4`; symbol #157 in this script) each
  push, in stack order: `0, 1, 6, 6, 0, 0, <type>, 1, <team local>`.
  With the reversed VM argument mapping this is
  `s_addNPCJob_createUnit(team, 1, type, 0, 0, 6, 6, 1, 0)` where `type` is
  `1`, `2`, `6`, or `3` per site.
- Args 6/7 (`6, 6`) are the min/max stored in the NPC job at `+0x11/+0x12`.
  Disassembly of the job executor (around `00548700`) shows it draws a count
  in that range (clamped by available idle civilians), gathers that many
  civilians into the shared selection array (`0064d65c`), and calls
  `FUN_00523a00` once — one job = ONE unit with N members. This also settles
  the old open question for the `ak_level.bci` military job counts (`4..4`):
  they are members-per-unit, not unit counts.
- Because arg 2 is `1`, the EXE clamp allows `1..20`, so `6..6 -> 20..20` is
  engine-legal. AI Ultimate M1 now patches both literals at all four
  sites (signature
  `[66,0, 66,1, 66,?, 66,?, 66,0, 66,0, 66,?, 66,1, 90,8, 128,157, 73,-9, 86]`,
  exactly four hits expected; count words at signature word indexes 5 and 7).
- For contrast, `s_createCiviUnitsMax` (`FUN_00524d70`) hardcodes batches of
  4 civilians (`mov esi, 4` at `00524ee0`) and takes no count argument.
- Runtime verified 2026-07-03: with the Dorfverteidigung patch applied, the
  village AI converts 20 villagers into a single squad in-game, confirming
  both the root-cause analysis and the members-per-unit interpretation.

### Village AI is defense-only — attacks come from separate party scripts

Question investigated 2026-07-04: "can an AI village-type Roman team sortie and
attack other teams like the other tribes do?" String-scan of the live CLAK
scripts (`SYSTEM\CLAK\SCRIPT`) and the decompressed `MAPS\ENDL_000` `ak_level.bci`
symbol/string table settles it: **no — and this is not Rome-specific. In the
vanilla endless AI, a settled village team of any faction never sorties; it only
defends.**

- Settled village teams (both `INIT_UNITSCIV` and `INIT_UNITSMIL` arrivals) run
  `Dorfverteidigung.bci`. Its only offensive-looking action is converting
  villagers into squads (the 6-man path above) and stationing them at the
  village's defensive important-positions — the script imports
  `s_searchImportantPos` and references `OD_IPOS01`..`OD_IPOS04`. There is no
  sortie/pursue-enemy-team logic in the settled-party handler; type-5 military
  reinforcements only DONATE units into the village garrison (see the v56 retreat
  quota section), they do not lead the garrison out to attack.
- Endless-mode aggression is generated by SEPARATE, short-lived party scripts
  that `ak_level.bci` names in its string table:
  `Mordbrenner` (arson/raiders), `Strafexpedition` and the Rome-specific
  `RoemischeStrafexpedition` (punitive expeditions), and `Tributgeher`
  (tribute collectors) — plus `Siedler` for settler arrivals. These are dispatched
  by the three inner raider timers (`v72`/`v74`/`v76` pollers) and the outer
  scheduler, hit the target, and leave on their retreat deadline.
- `Strafexpedition.bci` itself references `DorfAngriff` and `s_setVillageTemplate`;
  `DorfAngriff.bci` ("village attack") holds the actual assault behavior
  (`s_setTeamHostile`, unit-search/merge, morale, mission-result globals).
  These attack parties are the aggression path, distinct from the defensive
  `Dorfverteidigung` that a settled village runs.
- Consequence for team selection: per the `pickTeam (0xC3DC)` occupied-team mask
  (see Party Lifecycle), a team already holding a village is in the occupied mask,
  so attack/raider parties spawn on OTHER (unoccupied) CPU team slots. A settled
  village team therefore cannot double as an attacker in vanilla — the "Roman
  army marching on you" is an independent punitive-expedition party, not troops
  led out from a Roman village.
- To make village-type AI attack from its own stockpiled forces would be a new RE
  task: either add a sortie state to the settled-party handler in `ak_level.bci`,
  or give `Dorfverteidigung.bci` offensive job/target logic. Both are
  substantially more involved than the literal-value patches shipped so far;
  not attempted.

## Settle-Place Eligibility — the "defeated CPUs stop respawning" root cause (2026-07-08, RUNTIME-CONFIRMED)

**Status: fixed and confirmed working.** P17/P18/P19 (module M4) were applied to
the live install (all five `MAPS/ENDL_000..004/SCRIPT/ak_level.bci`) and the
user confirmed in-game that defeated CPU teams resume respawning. Independent
byte-level verification against the live install after apply: all five maps
show exactly 1 signature hit each for P17/P18/P19, every value matches the
Ultimate target (`P17`=100, `P18`=1, `P19`=800), the PFIL header's
uncompressed-size field matches the decompressed length, and each file's
decompress -> recompress -> decompress round-trip is byte-identical — the
patch applied cleanly with no corruption. `dotnet test --filter
CheckGameStatusTest` also reports all of M1-M6 as `Ultimate` with zero
`Legacy`/`Unknown` sites. As with every other endless AI fix, this only takes
effect on a NEW endless game (saves embed their own `ak_level` copy).

Diagnosed from a live `ESAVE_000` (ENDL_002) save exhibiting the long-standing
symptom: everything fine early, then defeated AI teams never return. Save-state
decode (`CLAK/scr.dat` task records: `TLCV` = the script's 87 int32 variables,
`TMEM`/`IARR` = script arrays by handle; array vars v45..v63 hold handles 8..25):

- All patch sites in the save-embedded `ak_level` matched the installed script
  (31 identical word diffs vs vanilla) — not a stale-script issue.
- Clock healthy: `s_getTime` ≈ 12.0M (weather task timestamp 12,000,919;
  engine sim time 35.99M = 3x speedhack), dispatcher `v16` = 12,017,058 pending.
- Party state: only slot0 (type-1 village, team 6) and slot4 (type-4 Roman,
  team 3) alive; slots 1,2,3,5,7 released cleanly (v47=0, v63[1]=1 < v70=4);
  5 CPU teams free; spawn probability 101 (M4). The Siedler spawner had been
  firing every 30 s for ~80 minutes with zero new villages.
- v61 note: retreat deadlines are stored NEGATED (`op 44` = negate) — the
  negative values are by design, not clock overflow.

The only silent-deterministic failure point left is the settle-place finder:

- Type-1 create wrapper (`0xE704`) and the type-4 founder create (caller at
  `0x142ac`) both call `fn 0x9904` (find-free-settle-place). On -1 the wrapper
  immediately DELETEs the fresh party and returns 0 — invisible churn, no
  deadline writes, exactly what the save shows.
- `fn 0x9904` iterates the 8 `placesSettle` entries (v7/v8) from a random
  start; a place is eligible iff `fn 0xA54(x, y, 2500)` reports it clear AND no
  live party claims it (`v47[i]!=0 && v59[i]==place`).
- `fn 0xA54` vetoes a place when `fn 0x690` finds ANY team's village center
  within the radius (`s_getVillageCenterObj` per team 0..7) or `fn 0x858`
  finds ANY team's units within the radius (`s_searchTeamUnits`, team loop
  starts at 0 = the player).
- Save evidence for accumulation: dead teams 2/5/7 retain NPC village records
  in `CLAK/npc.dat` (15/11/107 coordinate records; teams 2, 5 and live team 3
  share base (1632,5792) = the same settle place reused across generations,
  team 7 kept a full 107-record village). Team blocks are 19540 bytes at
  offset 36 + 8*i; the village base coordinate sits near block+0x400.
- Late game, the player's expansion (units within 2500 of a place veto it) plus
  dead-team leftovers permanently veto all remaining places → respawn stops.

Fix (AI Ultimate M4, 2026-07-08):

- `P18` — unit-scan comparand `0 -> 1` (value word after the file's only
  `callint -636`; signature `[120,-636, 73,-3, 86, 66,?, 96,101,117,20,
  66,0,87]`, decompressed site `0xAF0`). `fn 0x858` returns the lowest team
  index with units near the place, so `result >= 1` exempts the player
  (team 0) while CPU units still veto.
- `P19` — place veto radius `2500 -> 800` (signature `[66,?, 90,1, 90,0,
  120,-36800, 73,-3, 86]`, the file's only `callint -36800`, site `0x9A10`).
  Radius flows into both the village-center distance and unit search.
- `P17` — Roman founder 60% gate `60 -> 100` was previously designed/tested
  (RomanFounderGatePatchTests) but never wired into any module; now included
  in M4 as well.
- All three verified unique (1 hit) on all five vanilla AND currently-patched
  live maps, before and after value substitution.
- IMPORTANT: saves embed their own `ak_level`; patching the installed map scripts
  affects new endless games. Existing saves can now be migrated explicitly from
  Save Manager with an automatic full backup; normal Apply never rewrites saves.

### Paired-save follow-up: outer scheduler deadline ahead of saved script clock (2026-07-16)

A later `ESAVE_000` pair isolated a separate reason that AI can appear completely
stopped even when party slots, NPC jobs, teams, and settle places are available.
After loading the 2026-07-15 save, letting the game run, and overwriting it on
2026-07-16, the two type-1 parties remained unchanged. The authoritative saved
script clock is the third `scr.dat` callback scalar (`DAT_02662648`, decompressed
offset `0x80`), populated by `FUN_0052f860 -> FUN_005098d0`; it was `3,917,589`.
The embedded `ak_level` retained `v16 = 9,378,294`.

The outer dispatcher at `0x1BC64..0x1BD38` requires
`s_getTime() >= v16` before it calls the settlement and reinforcement spawners.
It was therefore closed by `5,460,705 ms` of game-clock time. Read-only EXE
verification found both master-clock constants at 10x, making the remaining wait
about 546 real seconds (9m06s); at 1x it would be about 91 minutes. This save's
embedded outer initial/refresh literals are both `30000`, so after the stale
deadline is crossed the normal 10x retry interval is roughly three real seconds.

This invalidates using `ak_wetter`'s persisted timestamp as an approximation of
the current clock: its `v8 = 9,395,235` is also in the future. The 2026-07-08
settle-place evidence remains valid as a second-stage late-game degradation, but
it was not the immediate reason this paired save produced no new party. The
cause of the clock rollback itself is still unproven; a controlled 1x versus 10x
save/load comparison is required before assigning it to the base game or the
game-speed patch.

The user continued that same loaded game and overwrote `ESAVE_000` again at
07:49, well beyond the predicted 9m06s minimum. The saved party table then grew
from two active slots to eight: types
`[1,1,1,1,3,4,5,3,0,0,0,0,0,0,0,0]`. The six new slots are 0, 3, 4, 5, 6,
and 7; `v63` now counts four type-1, two type-3, one type-4, and one type-5
party. In particular, new type-1 settlements use teams 5/6 at settle places
3/5, while the type-4/type-5 pair uses team 4 at place 6. This confirms from
the saved runtime state that the dispatcher did resume after crossing its old
deadline; the observed stop was delayed spawning, not a permanently dead
spawner.

The follow-up save has reached both configured bounds: `v63[1] = 4 == v70`,
and eight total active parties equals the bounded active-party limit. A ninth
party is therefore not expected until an existing party is released. The newly
serialized callback clock is `4,841,383`, but refreshed `v16` is
`14,161,135` (`+9,319,752 ms`), so a future deadline is again persisted relative
to the loaded script clock. This recurrence explains why a subsequent load can
again appear not to respawn for a long time, but it still does not identify
whether the underlying clock rollback comes from base-game save/load behavior
or the speed patch.

#### Implemented repair (2026-07-16; static/test verified, runtime pending)

P6 now replaces the fixed-size outer scheduler block at code-stream
`0x1BC44..0x1BD48` with a bounded guard:

`due := s_getTime() >= v16 || v16 > s_getTime() + 60000`

When due, it writes `v16 := s_getTime() + 30000` and calls the original
settlement spawner at its unchanged offset. A legitimate 30-second pending
deadline therefore keeps normal throttling, while a deadline more than 60
seconds ahead is treated as the save/load rollback observed in the paired
saves. The following dispatcher call, the bounded eight-party gate, and the
20-slot NPC-job protections remain unchanged. The old 30-second literal-only
state is detected as Legacy and migrated on Apply.

All five local `ENDL_000..004` decompressed RE samples uniquely matched and
completed byte-exact Original -> Ultimate -> Original round trips. Save Manager
also exposes an opt-in "Repair Endless AI Timer" action. It first creates a full
save backup, then locates the unique `MAPS/ENDL_###/SCRIPT/ak_level` BCI embedded
in `CLAK/scr.dat`, applies the same P6 repair, recompresses PFIL, verifies the
decompressed bytes, and writes atomically. No save is changed automatically.

## Integrated Endless Respawn Core (2026-07-16)

The modifier no longer exposes legacy M2, M3, and M4 as independent toggles.
They are one atomic `EndlessAi.Core` lifecycle:

- M2: reinforcement cooldown, scheduler/deadline repair, and completed-job recycling.
- M3: safe retreat/death detection and confirmed old-village cleanup acceleration.
- M4: settlement probability, Roman-founder gate, and settle-place eligibility.

Apply and restore always process all three modules together. Detection reports the
core enabled for Ultimate or Legacy state (including partial/mixed sites) so the next Apply migrates all
remaining sites to the complete safe core. Old profiles that request any of
M2/M3/M4 are normalized to the complete core, while the old feature keys remain
readable for compatibility. M1 reinforcement size and M5 settlement starting
resources remain independent difficulty choices. Save Manager repair stays opt-in
because existing saves embed their own `ak_level`.

## Pending Work

### 2026-07-03 stale-save and live-integrity incident

- Current `ESAVE_002` is `ENDL_002`; its payload timestamp (`11:04:54`) is
  earlier than the latest live-script writes (`~11:18:13`). Apply does not
  replace the BCI already embedded in `CLAK\scr.dat`.
- The embedded script has all current defeat-chain literals (six deadlines at
  5000, debounce 3, guaranteed settlement spawning, recycle 1, counts 20,
  threshold 8, gate `66,0`) but retains the rejected first-three-loop range
  `1000..2000` ms. This precisely matches the known runtime-stall state and
  makes the save unsuitable for accepting the current migration.
- Saved decompressed BCI SHA-256:
  `4dd021f2f86336e6fc61a269c677ef7403cad833494d467c8fe1cd5580f771a2`.
- Live decompressed `ENDL_002` SHA-256:
  `49839eb76743893b879be201c729c8104c09415acccc29928fbcea29eee02429`.
  The payloads differ in 42,885 bytes. The live payload fails normal opcode and
  signature matching; isolated literals at familiar offsets are insufficient
  to call it valid.
- Self round-trip equality is not an integrity oracle: it can faithfully
  recompress an already-invalid decompressed payload. Validation must compare
  against a clean baseline and assert the complete expected signature set.
- Required recovery/acceptance sequence: restore all five live scripts from a
  known-clean original baseline, re-apply, verify all five decompressed BCI
  structures and target values, then start a new endless game. Saved payloads
  remain read-only.

- Decode enough of the `BCI0` bytecode instruction set to identify the branch
  that selects `INIT_UNITSCIV` versus `INIT_UNITSMIL`.
- Run long-duration regression tests on all five `ENDL_000..ENDL_004` maps,
  including later reinforcement waves, completed-job recycling, disable/restore,
  and old-save behavior. The safe bounded patch is implemented; its remaining
  gap is long-run runtime coverage, not BCI write support.
