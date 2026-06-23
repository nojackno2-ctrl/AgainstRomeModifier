# Known Patches

## Stable

### Population Limit

- Files: `MAPS/**/team.dat`
- Sections: `[maxteamobjgenerell]`, `[teamdata]`
- Behavior: raises map population limits to the configured value.

### Free Construction, Production, Upgrade, Spell Costs

- File: `SYSTEM/ress.ini`
- Sections: `[objres]`, `[volkres]`
- Behavior: zeroes known cost fields.

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

### APT UI Layout Editing

- File: `apt.dat`
- Container: ZIP-like.
- Risk: high until runtime behavior and checksums are confirmed.
