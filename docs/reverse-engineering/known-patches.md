# Known Patches

## Stable

### Village Build Range

- Hook `005364c1` (file `0x1364c1`) jumps to executable zero padding at
  `0056258f` (file `0x16258f`).
- The trampoline preserves both negative-value checks, scales `ESI`/`EDI` with
  `value * 3`, calls `004c0900`, and returns at `005364d1`. Both the
  type-definition and per-object village-state copies therefore receive the
  same 3x values.
- Runtime result: both the player-usable village construction range and the red
  dashed frame have been successfully verified in-game at the 3x scale.
- Hook original: `85 F6 7C A6 85 FF 7C A2`.
- Hook patched: `E9 C9 C0 02 00 90 90 90`.
- Cave original: 39 zero bytes.
- Legacy 2x cave: `85 F6 0F 8C D4 3E FD FF 85 FF 0F 8C CC 3E FD FF D1 E6 D1 E7 57 56 50 E8 55 E3 F5 FF E9 21 3F FD FF`, followed by six zero bytes. It is recognized for migration and restore.
- Cave patched: `85 F6 0F 8C D4 3E FD FF 85 FF 0F 8C CC 3E FD FF 8D 34 76 90 90 8D 3C 7F 90 90 57 56 50 E8 4F E3 F5 FF E9 1B 3F FD FF`.

### Population Limit

- Files: `MAPS/**/team.dat`
- Section: `[teamdata]`, column 4.
- Behavior: raises each active team's population limit up to the executable's
  global limit of 1600. `[maxteamobjgenerell]` is emitted by the game's save
  writer but ignored by the loader, so the modifier preserves it.

### Team Banner Version

- Files: `MAPS/**/DATA/team.dat`
- Section: `[teamdata]`
- Field: column 5, zero-based, after the population limit column.
- Meaning: banner version (`bver`), not AI behavior.
- Data basis: the `[teamdata]` callback writes this value to the team structure
  at offset `+0x04`, validates it as `0 <= value < 10`, and later combines it
  with the faction to look up `SYSTEM/banner.ini` sections named
  `[volk%02ld_vicon_bver%02ld]` and `[volk%02ld_obdef_bver%02ld]`.
- Confirmed installed-game data: `SYSTEM/banner.ini` maps `volk00..03`
  (`GER`, `KEL`, `HUN`, `ROM`) and `bver00..09` to `Ver*ZivIco`,
  `Ver*KamIco`, and `Ver*Sta*` banner objects. `volk04..05` are `none`.
- Behavior: changing it should affect team banner/icon/object visual variants.
  It should not be used to enable computer players or tune AI behavior.

### Free Construction, Production, Upgrade, Spell Costs

- File: `SYSTEM/ress.ini`
- Sections: `[objres]`, `[volkres]`
- Behavior: zeroes `[objres]` build `1-6`, upgrade `7-12`, training `13-18`,
  and spell `25-28` fields where applicable, plus the confirmed `[volkres]`
  cost fields.
- Safety: preserves the independent `[objres]` `auf` group at indexes `19-24`.
  It is not part of the `aus` training-cost group.
- `[volkres]` indexes `264-295` are four complete eight-level groups named
  `befehl`, `motivieren`, `angriff`, and `verteidigung` by the executable.

### Unit Stat Editing

- File: `SYSTEM/DATA_MP/DEFAULTS/objdef.dau`
- Fields: see `objdef-fields.csv`.
- Behavior: adjusts HP, damage, cooldown, movement, sight, range, spell range, VW, and AW.
- Constraint: decompressed text length must remain unchanged.

### 20x Population-Building Capacity

- File: `SYSTEM/DATA_MP/DEFAULTS/objdef.dau`.
- Field: zero-based column `156` (`wohnwer`).
- Behavior: multiplies every positive original housing-capacity value by 20,
  including faction main buildings and residential buildings.
- Safety: rebuilds from the original in-memory backup, preserves each field's
  width, and treats a partial/non-20x state as disabled when loading settings.

### 10x Building Speed (Construction, Upgrade, Repair)

- File: `SYSTEM/DATA_MP/DEFAULTS/objdef.dau`.
- Fields: zero-based column `73` (`buildt` - build time) and column `74` (`upgrdt` - upgrade time).
- Behavior: divides original positive build and upgrade times by 10 for all building entries (names starting with `Bau`). Minimum limit is 1 ms to prevent game-engine timer divide-by-zero crashes.
- Repair Speed: repair speed in Against Rome is internally linked to build time (shorter build time results in faster repair per second). Thus, this single modification boosts build, upgrade, and repair rates by 10x.
- Safety: rebuilds from backup, preserves original column width using `PadLeft`, and maintains original decompressed file length.

### Villager Speed, Spell Radius, Morale

- File: `SYSTEM/cl_script.ini`
- Patterns include `CiviDelay`, `Radius`, and morale parameters.
- The core switch writes `CiviDelay` as 500 ms (the fastest valid 10x setting).
  When disabled, the original backup values are retained.
- Only HUN and KEL have `Radius` records in the original file; GER spell radius
  is not exposed as editable.

### Focus-Loss Background Execution

- File: `Against_Rome.exe`
- Offset: `0x161a88`
- Safe only when byte signature matches.

### AI Ultimate Mode

- Runtime status: the 2026-07-03 stale-save/invalid-live-script incident
  (see `endless-mode-ai.md`) is resolved. A full static re-verification the
  same day (145 automated checks reproducing every finder/signature against
  the installed game) confirms all five `MAPS/ENDL_*/SCRIPT/ak_level.bci`
  are structurally valid (`BCI0`, 120657 bytes) and every patch site — create-unit
  counts/recycle flag, military respawn delay, all six retreat deadlines,
  dead-party debounce, reinforcement threshold/gate, spawner probabilities,
  the six reinforcement/action loop delays, the zeroed v56 retreat quota, and vanilla donation
  values — is at a consistent, internally-coherent state across all five maps,
  along with `ak_npc.bci`, `ak_produktion.bci`, `ak_haupthaus.bci`,
  `Dorfverteidigung.bci`, and all 42 `Endlos_*_Siedlung*.sdl` templates.
  Long-duration in-game regression across multiple reinforcement waves is
  still the only remaining open item.
- File: `MAPS/ENDL_*/SCRIPT/ak_level.bci`.
- Format: `PFIL@` compressed `BCI0` compiled script.
- Modifier UI: `AI終極模式` / `AI Ultimate Mode`.
- Create-unit call: decompressed BCI offset `0x17B60`,
  `s_addNPCJob_createUnit(local7, 3, 8, 0, 0, 4, 4, 1, 0)` after reversing
  BCI stack argument order.
- Count literals: `0x17B2C` and `0x17B34`, both `4 -> 20`.
- Completed-job recycling flag: final logical argument and decompressed literal
  at `0x17B1C`, `0 -> 1`. This lets completed military reinforcement jobs free
  their per-team NPC-job slots for later waves.
- EXE path `0054aa80 -> 00547f50` clamps the count to `1..20`.
- Military reinforcement cooldown at `0x178E0`: `180000 -> 5000` ms.
- Party retreat/cleanup deadlines use a mixed target. Non-settlement sites
  `0x119C0`, `0x12FFC`, `0x13FE8`, and `0x17F38` change `600000 -> 5000` ms.
  Settled-handler sites `0x10700` and `0x160EC` stay at `600000` because states
  51/52 normally wait for old-village/palisade cleanup; the deadline is only a
  fallback. The previous all-six-at-5000 state could force DELETE_PARTY before
  cleanup, allowing a respawn while NPC village records still referenced the
  old location.
- Old-village object cleanup cadence in `SYSTEM/CLAK/SCRIPT/ak_haupthaus.bci`:
  the unique sequence at decompressed `0x3248` initializes the dead-village
  pass as `1500 + rand(-25,25)` ms. AI Ultimate changes `1500 -> 100`, so the
  existing one-object-at-a-time message/confirmation loop runs every 75..125 ms
  instead of 1475..1525 ms. Restore returns 1500. The final standalone 2000-ms
  confirmation delay remains original.
  `0x17F38` was previously misclassified as a "village defeat respawn timer";
  it is the type-5 handler's RETREAT_INIT deadline. The same-shaped
  initial-arrival timeout at `0x7F24` is deliberately NOT patched (a 5-second
  value there would retreat parties before they can settle).
- Dead-party confirmation counter at `0x1068C`: `20 -> 3` consecutive ticks
  (settled-party handler; counts ticks with village, leader, civilians, and
  members all gone before entering RETREAT).
- All six scheduler delay sites change to `5000..10000` ms. The first three
  are inner raider timers; the remaining `60000..120000`, `60000..120000`, and
  `120000..240000` sites initialize and refresh the outer action scheduler that
  gates the settlement/military dispatcher. Leaving those outer sites original
  caused AI arrival checks to occur only every 1-4 minutes.
  REJECTED runtime state: `1000..2000` ms caused computer respawns to stop;
  Apply accepts that interim state and migrates it back to `5000..10000` ms.
- Military-reinforcement unit-count threshold at decompressed `0x195F8`:
  `4 -> 40` (2026-07-03 update; the previous `8` is accepted as legacy-enabled
  and migrated on the next apply). The spawner only sends the next type-5
  reinforcement wave while `s_searchTeamUnits(team) < threshold`, so with the
  retreat quota zeroed (below) the team's army accumulates up to ~40 units
  (40 x 20 members = 800, under the EXE global population cap of 1600) and
  then stops growing — a natural upper bound below the population limit.
- Reinforcement no-retreat (RE-ENABLED 2026-07-03 with the missing piece):
  the type-5 retreat quota write `v56[party] <- pushloc 15` at decompressed
  `0x17888` region (unique signature
  `[81,57, 90,-3, 90,14, 164, 81,56, 90,-3, ?, ?, 164, 81,61]`, wildcard =
  the quota opcode/operand words) becomes `pushlit 0` when AI Ultimate is
  enabled. State 49 then hands EVERY battle unit plus the leader over to the
  village's type-4 party via `s_setObjMark` (the vanilla donation path,
  normally limited to `min(4-teamUnits, 2, partyUnits/2)` units) instead of
  retreating them. The 2026-07-03 failure of this exact edit is now explained:
  the threshold was still `8`, so donated units pushed `s_searchTeamUnits`
  over the spawner condition after roughly one wave and reinforcements
  stopped — the quota patch MUST ship together with the `40` threshold, and
  both are driven by the same toggle. Handed-over units are not inserted into
  the type-4 party's `v52` object array, so the settled handler's dead-party
  check (leader + civilians + tracked members) is not blocked by them. The
  donation formula literals at `0x17788` (`4,2,2`) stay vanilla — with the
  quota forced to 0 the formula only shapes the civilian-recreate quota
  (`v57`), not the retreat set.
- The gate at `0x1960C` remains `66,0`.
- The Siedler spawner's default and 0/1/2/3-live-party probability literals change from `0,0,80,60,40,20` to `101,101,101,101,101,101`. The single-player occupied mask reserves player team 0, and `pickTeam` selects only unoccupied CPU teams 1-7, so this fills at most seven simultaneous computer opponents and reuses a defeated team's slot after cleanup.
- REJECTED CONFIGURATION 2026-07-03 (same session, before any release): tried making
  reinforcement parties hand over all units instead of retreating, via the
  `v56[party]` retreat quota (`[90,15] -> [66,0]` at decompressed `0x17888`
  region) and separately via widening the donation formula at `0x17788`
  (`(TH,CAP,DIV)` `4,2,2 -> 8,4,1`). Both broke reinforcement arrivals in
  runtime testing (reported: Roman reinforcements stopped, active-party
  limit also appeared to have no effect). The root cause was the still-low
  threshold of 8. Current AI Ultimate uses quota 0 together with threshold 40;
  the widened donation formula remains rejected and vanilla. See
  `endless-mode-ai.md` for full detail.
- Older builds wrote `112,272` at `0x1960C` and shortened every action loop to
  `5000..10000` ms. Applying this version restores the gate and all unrelated
  loops; only the three bounded reinforcement polling loops remain accelerated.
- The signature is present in `ENDL_000` through `ENDL_004`.
- Earlier enabled states (including only `0x17F38` shortened, no retreat
  deadlines shortened, or all six deadlines shortened) are accepted as
  legacy-enabled and migrated to the mixed four-fast/two-protected deadline
  state with the 3-tick debounce on the next apply.
- 2026-07-03 incident: `ESAVE_002` predates the latest Apply and embeds the
  rejected first-three-loop state `1000..2000` ms despite already containing
  the six 5000-ms retreat deadlines, debounce 3, recycle 1, counts 20,
  threshold 8, gate `66,0`, and guaranteed settlement-spawner probabilities.
  This save is expected to exhibit the known respawn stall and cannot validate
  the 5–10-second migration. At incident time, the live `ENDL_002` decompressed payload
  also failed structural signature checks and differed from the saved payload
  in 42,885 bytes (save SHA-256 `4dd021f2...f771a2`, live
  `49839eb7...02429`). All five live scripts were subsequently restored from a
  known-clean baseline, re-applied, and statically verified. Runtime acceptance
  still requires a fresh game; never migrate by rewriting save payloads.
- Disabling or compatibility restore returns the recycling flag, counts, delays,
  limits, and gate words to their exact original values.
- Rejected global economy edits are always restored: `ak_npc.bci` free-civilian
  reserve `20 -> 0` and `ak_produktion.bci` production branch `112 -> 117`.
  Runtime testing showed these two scripts are not safely NPC-scoped and stop
  staffed player buildings from producing resources, including in a new game.
- `ak_haupthaus.bci` conversion-size argument `[81,59] -> [66,20]` (decompressed
  `0x3FCC`, unique signature hit) is re-enabled and follows the AI Ultimate
  toggle. It replaces "push var 59" with "push literal 20" as the last argument
  of the `s_createBattleUnitsMax` call. Ghidra decompilation of the callback
  implementation `FUN_005249d0` (registered via trampoline `LAB_0052a110`,
  signature `i_iiii`) shows the argument is the members-per-battle-unit count,
  clamped by the EXE to `0..20`; the function gathers up to 100 idle civilians
  per call and converts all of them in batches of that size (one batch = one
  battle unit via `FUN_00523a00`). The runtime value observed in play is 6,
  matching the reported 6-man AI conversion units. This edit was previously
  reverted together with the two production-path edits; the documented player
  breakage belongs to those paths, but a dedicated in-game regression for the
  player's manual conversion UI is still pending.
- 2026-07-03 correction: the main-house call only fires in the CIVRECREATE
  chain (`var57 == 34`). The 6-man units observed in normal play come from
  `Dorfverteidigung.bci`: four `s_addNPCJob_createUnit` sites
  (`s_addNPCJob_createUnit(team, 1, type∈{1,2,6,3}, 0, 0, 6, 6, 1, 0)`,
  pushsym offsets `0xF1BC/0xF264/0xF30C/0xF3B4`). The `6, 6` literals are the
  per-unit member min/max (job `+0x11/+0x12`; EXE clamp `1..20` because
  arg 2 is `1`). AI Ultimate now patches all eight literals `6 -> 20` via the
  word signature
  `[66,0, 66,1, 66,?, 66,?, 66,0, 66,0, 66,?, 66,1, 90,8, 128,157, 73,-9, 86]`
  (exactly four hits enforced); disable restores `6`. Job-executor
  disassembly (`00548700` region) confirms one job creates one unit whose
  member count is drawn from that range, which also confirms the
  `ak_level.bci` military job counts (`4..4 -> 20..20`) are members-per-unit.
  Runtime verified 2026-07-03: in-game the village AI now converts 20
  villagers per squad with the patch applied.

### 10x Idle HP Regeneration (amount, not interval)

- Files: 12 `SYSTEM/CLAK/SCRIPT/<name>.bci` unit AI scripts (`ak_anfuehrer`,
  `ak_artillerie`, `ak_geisterreiter`, `ak_kampfverband`, `ak_krieger`,
  `ak_kundschafterwolf`, `ak_landtier`, `ak_packpferd`, `ak_priester`,
  `ak_verbandswolf`, `ak_zivilist`, `ak_zivilverband`).
- Mechanism: each script's idle-regen tick calls `s_addLP(obj, 1)` — the
  modifier toggle `FoodHealing10x` rewrites the literal `1` argument to `10`
  (still one call per tick; the tick INTERVAL is untouched). An earlier
  design rewrote the tick interval (`LPIncIdle` in `cl_script.ini`,
  15000 -> 1500 ms) to the same effective 10x rate; the amount-based version
  replaced it because the user asked for "+10 per tick" specifically, and it
  reads more clearly on the HP bar (one visible jump instead of ten small
  ones). `cl_script.ini`'s `LPIncIdle` is now unconditionally restored to the
  original 15000 ms on every apply, migrating any install patched by the
  earlier interval-based build.
- Signature per script: `[66,1, 81,10, 81,98, 128,<addLpSym>, 73,-3, 86]`
  (`pushlit 1 / pushvar 10 / pushvar 98 / pushsym s_addLP / argc -3 /
  callext`), where `<addLpSym>` is that script's own `s_addLP` index in its
  `SYMBCONS` table (symbol tables are not stable across scripts — resolved
  per file: anfuehrer 87, artillerie 69, geisterreiter 68, kampfverband 89,
  krieger 73, kundschafterwolf 77, landtier 72, packpferd 57, priester 92,
  verbandswolf 68, zivilist 83, zivilverband 87). Verified 2026-07-03: every
  site hits exactly once against the installed game (offsets recorded in
  `docs/reverse-engineering/decompilation-workflow.md` session notes).
- Excluded on purpose: `geisterreiter`/`kundschafterwolf`/`verbandswolf` each
  have a SECOND `s_addLP` call with literal `-1` (an LP-decay tick, unrelated
  to healing) — the `pushlit 1` requirement in the signature naturally
  excludes it. Also excluded: `ak_haupthaus`/`ak_lager`/`ak_produktion`/
  `ak_wohnhaus`/`ak_opferstaette`/`ak_tor`/`ak_steinschlagfalle` — these
  `Bau*`-controlling scripts share the identical call shape for BUILDING
  self-repair, out of scope for a "unit healing" feature, and `ak_produktion`
  in particular already has documented history of a global-economy edit
  breaking staffed player buildings (see the AI Ultimate Mode section) — kept
  untouched as a deliberate safety margin.
- Restore: rewrites the literal back to `1` for all 12 scripts; unknown state
  (site count != 1, or value not in {1, 10}) aborts the whole apply, matching
  the project's "unknown-state refusal" convention.
- Full chain decoded 2026-07-03 (capstone disassembly of the EXE VM
  dispatcher, cross-checked against the local pseudocode function inventory
  — no working Ghidra install was available this session):
  `[TribeData]` parser `0041c600` resolves keys via the pointer table at
  `0061BB40` (`LPIncIdle` = key 15), stores into `DAT_029c50e8[tribe]` via
  `FUN_00540dc0` (EXE clamps 500..100000000, default 10000), exposed to
  scripts as `s_getTribeValue` (trampoline `00542250 -> FUN_00540fe0`). Each
  unit script schedules `deadline = time + getTribeValue(tribe, 15)` and on
  expiry calls `s_addLP(unit, N)`. **No village-bounds check and no food
  deduction exist anywhere in this chain** (verified through
  `s_addLP -> FUN_005129c0 -> FUN_00512a10 -> FUN_00512aa0/FUN_004ad1e0`, and
  by scanning all CLAK scripts for LP+store symbol pairs). The perceived
  "units heal in the village by eating main-house food" is this idle regen —
  units idle safely inside the village; food stores are drained by separate
  mechanisms. The objdef `reglp` column (index 72) is building/siege
  self-repair (+1 LP ticks for `Bau*` structures, driven by the SAME
  `s_addLP` call shape from the building scripts above), not unit healing.
- Exact heal conditions (fully decoded 2026-07-03 with the corrected VM
  opcode table — see `bci0-opcodes.md`): the regen tick function is only
  invoked from each script's activity dispatcher when
  `s_getObjActivity(obj) == 1002` (the STOP/idle activity set at spawn);
  moving/attack/other activities (8, 9, 1004, 1005, 1006, 32) route to other
  handlers, so combat and movement stop healing at the dispatcher level, not
  via per-tick checks. Inside the tick the gates are: `s_gameMode()` true,
  `s_dead(obj) == 0`, LPIncIdle deadline reached (self-re-arming via a
  var-reference argument, opcode 77), and `s_unitMember(obj) == 0` — the
  FORMATION object (`ak_kampfverband`, tick entry `0x6878`, dispatcher flags
  `(0,1,1,1)` enabling LP/morale/mana regen) performs the heal once and the
  EXE (`FUN_00525ac0`) loops it over every member; individual-figure scripts
  like `ak_krieger` call the same shared tick with the LP flag OFF. The
  moving/crew/loaded checks visible near the heal code belong to the
  step-away controller, not the heal gate.

### REJECTED: objdef `ptime` resource-production cycle

- Two runtime tests on 2026-07-03 (original/10 → 300-900 ms, then a fixed
  500 ms) produced **no observable change** in resource-building output
  (tested on the Celt Bauernhof with a fresh apply verified on disk).
- `ptime` (zero-based column `27`), `resb1-6` (28-33) and `resr1-6` (34-39)
  remain documented as the production-cycle/input/output columns by data
  correlation, but the engine's runtime production rate is evidently driven
  by something else (worker cycle timing is a candidate). Do not re-attempt a
  production-rate patch through `ptime` without new EXE-side evidence.
- The briefly-shipped toggle was replaced by the idle-regeneration amount
  patch above; any `ptime` values a previous apply wrote are automatically
  restored on the next apply because objdef patching always rebuilds from the
  original backup.

### Priest Spell Altar-Count

- File: `Against_Rome.exe`
- Target Offsets (imm8 offsets):
  - Germans (`FigGerPri00`): `0x4A114` (Spell 1), `0x4A138` (Spell 2), `0x4A15C` (Spell 3), `0x4A0E5` (Spell 4)
  - Celts (`FigKelPri00`): `0x4A1CE` (Spell 1), `0x4A295` (Spell 2), `0x4A2B7` (Spell 3), `0x4A24B` (Spell 4)
  - Huns (`FigHunPri00`): `0x4A32B` (Spell 1), `0x4A3F2` (Spell 2), `0x4A416` (Spell 3), `0x4A3A8` (Spell 4)
- Original bytes (9 bytes per site):
  - Germans:
    - Spell 1: `83 FE 01 0F 8C 54 FF FF FF` at `0x4A112`
    - Spell 2: `83 FE 02 0F 8C 58 FF FF FF` at `0x4A136`
    - Spell 3: `83 FE 03 0F 8C 5C FF FF FF` at `0x4A15A`
    - Spell 4: `83 FE 04 0F 8D 92 00 00 00` at `0x4A0E3`
  - Celts:
    - Spell 1: `83 FE 01 0F 8D A3 00 00 00` at `0x4A1CC`
    - Spell 2: `83 FE 02 0F 8C 61 FF FF FF` at `0x4A293`
    - Spell 3: `83 FE 03 0F 8C 65 FF FF FF` at `0x4A2B7`
    - Spell 4: `83 FE 04 0F 8D 89 00 00 00` at `0x4A249`
  - Huns:
    - Spell 1: `83 FE 01 0F 8D A3 00 00 00` at `0x4A329`
    - Spell 2: `83 FE 02 0F 8C 61 FF FF FF` at `0x4A3F0`
    - Spell 3: `83 FE 03 0F 8C 65 FF FF FF` at `0x4A414`
    - Spell 4: `83 FE 04 0F 8D 89 00 00 00` at `0x4A3A6`
- Behavior: Modifies the hardcoded altar count constants (1, 2, 3, 4) in the spell button logic in `Against_Rome.exe`. Setting these imm8 values to `00` removes the altar count requirement entirely.
- Safety: The modifier checks all 12 patterns before writing. Setting values from `0x00` to `0x7F` is safe.

## Candidate

### UI Unit Count Limits

Related script callbacks:

- `s_createBattleUnitsMax`
- `s_createCiviUnitsMax`
- `s_unitMemsWeaponMax`
- `s_getNotHorseUnitMems`

Observed case:

- With 24 villagers selected, choosing 4 mounted civilians and 20 battle units should leave 0 unequipped villagers, but the current free-production data patch can still display 4 unequipped villagers.
- This does not appear to be a population-limit issue. It is likely a shared UI reservation-count issue between mounted-civilian and battle-equipment paths.
- Original `ress.ini [objres]` `aus`/`auf` data may let the vanilla UI infer occupied equipment slots. Clearing non-exempt battle-unit `auf` fields (`Index 19-24`) may cause the UI to stop subtracting battle-unit reservations and may also interfere with endless AI spawn jobs.
- Current patch behavior preserves original battle-unit `auf` fields; find a separate fix if disband/disarm resource refunds need to be blocked without disturbing AI behavior.

### APT UI Layout Editing

- File: `apt.dat`
- Container: ZIP-like.
- Risk: high until runtime behavior and checksums are confirmed.

## Disabled / Rejected

### Legacy Village Range And Red-Frame Sites

- Logical bounds (`00536630`):
  - `0x1366c4`: `C1 E2 06` -> `C1 E2 07`
  - `0x1366cd`: `C1 E1 06` -> `C1 E1 07`
- Visible dashed frame (`004d7160`):
  - `0x0d722c`: `C1 E6 06` -> `C1 E6 07`
  - `0x0d723b`: `C1 E7 06` -> `C1 E7 07`
- `00536450` writes each X/Z value twice: `004c0900` stores it in the village
  object's type-definition rectangle, then `00536450` copies it into per-object
  village state. `004d7160` reads the type-definition copy; `00536630` reads the
  per-object copy. They share setter inputs but are separate storage paths.
- The rejected four-site patch changed the two final consumers, but omitted
  `004c0970`, which tests a point against the type-definition rectangle and has
  ten UI callers. Therefore it never synchronized every consumer involved in
  player-side UI/target handling.
- Runtime result: after all four sites were patched, the user observed no
  change to the buildable area or the selected-village red dashed boundary.
  The hypothesis is rejected as a working patch.
- Corrected interpretation: `004d7160` may draw another team-village display
  rectangle, or the reported boundary is produced by a separate data/render
  path. Static similarity is not sufficient proof of user-visible semantics.
- `00536820`, the point-inside-logical-village test, has only the script/AI
  wrapper `005367c0` and candidate-position search `00544fd0` as direct callers.
  Player order previews `0044f4b0` and `0044f7b0` do not call it. Therefore the
  `00536630` pair is not the general player construction-range gate.
- Modifier behavior: the modifier never writes any of the four rejected `07`
  candidates. If either the old two-site state or four-site state is detected,
  it restores all four original `06` instructions. The UI and preset field
  control only the stable setter trampoline documented above.
- `00451650` / overlay type `0x28` is also rejected: its callers are combat-mode
  controls `igm_but_kampf_beserk` and `igm_but_kampf_normal`, not the village
  range frame.
