# Known Patches

## Stable

### Population Limit

- Files: `MAPS/**/team.dat`
- Sections: `[maxteamobjgenerell]`, `[teamdata]`
- Behavior: raises map population limits to the configured value.

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
- Behavior: zeroes known cost fields.
- Safety: preserves `[objres]` indexes `19-24` because those equipment/refund
  fields may be used by vanilla UI and AI spawn logic to infer unit equipment
  relationships.

### Unit Stat Editing

- File: `SYSTEM/DATA_MP/DEFAULTS/objdef.dau`
- Fields: see `objdef-fields.csv`.
- Behavior: adjusts HP, damage, cooldown, movement, sight, range, spell range, VW, and AW.
- Constraint: decompressed text length must remain unchanged.

### Villager Speed, Spell Radius, Morale

- File: `SYSTEM/cl_script.ini`
- Patterns include `CiviDelay`, `Radius`, and morale parameters.

### Focus-Loss Background Execution

- File: `Against_Rome.exe`
- Offset: `0x161a88`
- Safe only when byte signature matches.

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

### Endless Military Spawn Count

- File: `MAPS/ENDL_*/SCRIPT/ak_level.bci`
- Format: `PFIL@` compressed `BCI0` compiled script.
- Modifier UI: `AI終極模式` / `AI Ultimate Mode`.
- Candidate call: decompressed BCI offset `0x17B60`,
  `s_addNPCJob_createUnit`.
- Current interpreted call:
  `s_addNPCJob_createUnit(local7, 3, 8, 0, 0, 4, 4, 1, 0)`.
- Count fields: logical arguments 6 and 7, currently both `4`.
- Decompressed BCI literal offsets: `0x17B2C` and `0x17B34`.
- Verified same local bytecode sequence in `ENDL_000` through `ENDL_004`.
- EXE behavior: callback `0054aa80` forwards to `00547f50`, which clamps these
  count values to `1..20` because argument 2 is `3`.
- Risk: candidate until verified in-game. The count appears to represent
  created military units/formations, not necessarily individual soldiers.
- Implemented behavior: when enabled, writes both count literals to `20`; when
  disabled or compatibility restore is used, writes both back to original `4`.
  The patch searches for the bytecode signature instead of relying only on a
  fixed offset.
- Respawn wait: when enabled, writes the `CIVRECREATE_WAIT` delay literal from
  original `180000` ms to `5000` ms; restore writes it back to `180000` ms.
- Action-loop waits: when enabled, rewrites identified endless AI
  `s_randRange` delay pairs from original long waits such as `480000..960000`,
  `240000..360000`, `60000..120000`, and `120000..240000` ms to `5000..10000`
  ms. Restore writes the matched pairs back to their original values.
- Active-AI limit bypass: when enabled, patches the decompressed BCI words at
  `0x1960C` from `66,0` to `112,272`. This skips the active-party flag gate
  after the local active-count value is computed and jumps directly to the
  follow-up dispatch path. Restore writes the original `66,0` words back.

### APT UI Layout Editing

- File: `apt.dat`
- Container: ZIP-like.
- Risk: high until runtime behavior and checksums are confirmed.

## Disabled / Rejected Candidates

### Village Build Range Red Frame

- Candidate EXE offsets tested:
  - `0x1366c4`: `C1 E2 06` -> `C1 E2 07`
  - `0x1366cd`: `C1 E1 06` -> `C1 E1 07`
- Decompiled function: `00536630` / `GetVillageBounds`.
- What it affects: calculated village bounds used by village logic, including
  `00536820` tile-in-village checks.
- User-visible result: it did not enlarge the selected village/main-house red
  dashed frame.
- Current modifier behavior: the UI option is hidden and disabled. Apply and
  compatibility restore never write the patched `07` bytes. If an old build has
  already written those bytes, the modifier restores them to the original
  `06/06` bytes.
- Current installed-game verification after restoring the game folder:
  - `0x1366c4`: `C1 E2 06`
  - `0x1366cd`: `C1 E1 06`
  - `0x136867`: `C1 E0 06`

The visible red frame is still unresolved. Current evidence points at the
overlay path around `0044e990 -> 00421c00 -> 00520320 -> 005203a0` and overlay
type `0x28`, not the old `00536630` byte patch by itself.
