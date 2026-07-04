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

The decompressed payload is plain INI text: a `[settlement]` header followed
by `[objectNNNN]` sections, one per placed object. Confirmed fields include
`namedef` (object definition name), `def` (numeric id), `pos`, `team`,
`nation`, `anzv` (unit counts on `Ver*Icon` formation entries), and `resv`
(six comma-separated stored-resource values). Original campaign templates
prove `resv` is the engine-native way to grant AI starting stockpiles, e.g.
`KAMP_006\Team_5.sdl` main house: `resv=614,300,372,250,460,140`. Endless
templates ship with all-zero `resv`.

### Settlement main-house starting-resources patch (implemented)

`AI終極模式` additionally rewrites the main-building `resv` line in every
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
- The modifier option `AI終極模式` changes this military count range to
  `20..20`, changes the military reinforcement wait from `180000` ms to
  `5000` ms, shortens four non-settlement party retreat deadlines from
  `600000` ms to `5000` ms while preserving both settlement-cleanup fallbacks
  at `600000` ms (see "Party lifecycle" below), cuts the dead-party
  confirmation counter from `20` to `3` ticks, and raises the reinforcement
  unit-count threshold at `0x195F8` from `4` to `40`.
  It also changes the last `s_addNPCJob_createUnit` argument at `0x17B1C` from
  `0` to `1`. EXE runtime analysis shows that this flag removes a job after its
  status leaves the running state, allowing the 20 per-team NPC-job slots to be
  reused by later reinforcement waves instead of retaining completed jobs.
  All six scheduler delay sites are changed to `5000..10000` ms. The first
  three are inner raider timers; the final three are the outer scheduler's
  initial and refresh ranges, which gate the dispatcher that calls the
  settlement and military-reinforcement spawners. Accelerating only the first
  three therefore still allowed 60-240 second arrival gaps. Older builds also
  bypassed the gate at `0x1960C` with `112,272`; that gate bypass, rather than
  the bounded scheduler acceleration, could exhaust the 20 job slots available
  to each team and remains disabled.
  A later `1000..2000` ms experiment was also rejected after runtime testing
  showed computer respawns could stop; it is recognized only for migration back
  to `5000..10000` ms.
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
- The RETREAT chain reaches DELETE_PARTY (256) when its cleanup condition
  completes or its `v61` fallback deadline expires. In the settled-party
  handlers, states 51/52 wait on village/palisade teardown state; the deadline
  is not the normal fast path.
- The six retreat/cleanup deadline literals share the BCI word signature
  `[81,61, 90,-3, 128,83, 86, 66, <ms>, 32, 44, 164]` at decompressed value
  offsets `0x10700`, `0x119C0`, `0x12FFC`, `0x13FE8`, `0x160EC`, `0x17F38`.
  Two same-shaped sites are deliberately excluded: the initial-arrival timeout
  at `0x7F24` (no `44` word; must stay `600000` or arrivals would retreat
  before settling) and the military reinforcement wait at `0x178E0` (followed
  by `pushlit 34` = CIVRECREATE_WAIT; patched separately).
- AI Ultimate accelerates only `0x119C0`, `0x12FFC`, `0x13FE8`, and `0x17F38`
  to `5000`. Settled-handler sites `0x10700` and `0x160EC` remain `600000`.
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

The `ak_haupthaus.bci` edit was re-examined in isolation and re-enabled under
the AI Ultimate toggle:

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

**UPDATE 2026-07-03 (later session): RE-ENABLED with the missing piece.** The
root cause of both rejected attempts below is identified as the spawner
threshold at `0x195F8`: it was still `8`, so permanently donated units pushed
`s_searchTeamUnits(team)` past the spawn condition after about one wave and
reinforcements stopped. AI Ultimate now applies the `v56 <- pushlit 0` quota
patch TOGETHER with raising the threshold `4 -> 40` (legacy `8` migrated).
Decoding state 49 (`0x17F60..0x183D8`) confirms handed-over units and the
leader are only re-marked via `s_setObjMark` (to `32 + type4Party`, or `-1`
when no type-4 party exists) and are NOT inserted into the type-4 party's
`v52` object array, so the settled handler's dead-party check (leader +
civilians + tracked members, debounce at `0x1600C` region) is not blocked by
them and team recycling still works. The army is naturally bounded: the
spawner stops sending waves once the team holds ~40 battle units (800 members
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
  only together with the corrected threshold of 40.
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
  40. Disabling restores `[90,15]` and threshold 4.** Correction (2026-07-03): an
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
  and `s_searchTeamUnits(team) < <0x195F8 literal>`. The old `112,272`
  bypass at `0x1960C` skipped this whole condition block, which is why it
  exhausted job slots.
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
  engine-legal. The AI Ultimate toggle now patches both literals at all four
  sites (signature
  `[66,0, 66,1, 66,?, 66,?, 66,0, 66,0, 66,?, 66,1, 90,8, 128,157, 73,-9, 86]`,
  exactly four hits expected; count words at signature word indexes 5 and 7).
- For contrast, `s_createCiviUnitsMax` (`FUN_00524d70`) hardcodes batches of
  4 civilians (`mov esi, 4` at `00524ee0`) and takes no count argument.
- Runtime verified 2026-07-03: with the Dorfverteidigung patch applied, the
  village AI converts 20 villagers into a single squad in-game, confirming
  both the root-cause analysis and the members-per-unit interpretation.

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
