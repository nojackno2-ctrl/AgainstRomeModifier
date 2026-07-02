# Known Patches

## Stable

### Village Build Range

- Hook `005364c1` (file `0x1364c1`) jumps to executable zero padding at
  `0056258f` (file `0x16258f`).
- The trampoline preserves both negative-value checks, scales `ESI`/`EDI` with
  `(value * 5) >> 1`, calls `004c0900`, and returns at `005364d1`. Both the
  type-definition and per-object village-state copies therefore receive the
  same 2.5x values.
- Runtime result: the same setter path was verified at 2x for both the
  player-usable village construction range and the red dashed frame. The current
  2.5x factor still needs a fresh in-game verification; the 2x result must not be
  reported as proof of the new multiplier.
- Hook original: `85 F6 7C A6 85 FF 7C A2`.
- Hook patched: `E9 C9 C0 02 00 90 90 90`.
- Cave original: 39 zero bytes.
- Legacy 2x cave: `85 F6 0F 8C D4 3E FD FF 85 FF 0F 8C CC 3E FD FF D1 E6 D1 E7 57 56 50 E8 55 E3 F5 FF E9 21 3F FD FF`, followed by six zero bytes. It is recognized for migration and restore.
- Cave patched: `85 F6 0F 8C D4 3E FD FF 85 FF 0F 8C CC 3E FD FF 8D 34 B6 D1 EE 8D 3C BF D1 EF 57 56 50 E8 4F E3 F5 FF E9 1B 3F FD FF`.

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

- Runtime status: verified working by the user.
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
- Respawn cooldown: `180000 -> 5000` ms.
- The first three military reinforcement polling loops change from
  `480000..960000`, `480000..960000`, and `240000..360000` ms to
  `5000..10000` ms. The remaining AI action loops retain their original pacing.
- Active-party comparison literal at decompressed `0x195F8`: `4 -> 8`.
- The gate at `0x1960C` remains `66,0`.
- Older builds wrote `112,272` at `0x1960C` and shortened every action loop to
  `5000..10000` ms. Applying this version restores the gate and all unrelated
  loops; only the three bounded reinforcement polling loops remain accelerated.
- The signature is present in `ENDL_000` through `ENDL_004`.
- Disabling or compatibility restore returns the recycling flag, counts, delays,
  limits, and gate words to their exact original values.
- Rejected global economy edits are always restored: `ak_npc.bci` free-civilian
  reserve `20 -> 0`, `ak_produktion.bci` production branch `112 -> 117`, and
  `ak_haupthaus.bci` formation argument `[66,20] -> [81,59]`. Runtime testing
  showed that these scripts are not safely NPC-scoped and stop staffed player
  buildings from producing resources, including in a new game.

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
