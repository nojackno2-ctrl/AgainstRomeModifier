# Executable Functions And Offsets

Addresses are from the currently analyzed `Against_Rome.exe`. Function names are
provisional unless manually named in a Ghidra project.

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

## Implemented EXE Patch

Focus-loss pause patch:

- File offset: `0x161a88`
- Original bytes: `89 15 C4 7D 9E 02`
- Patched bytes: `90 90 90 90 90 90`

The modifier verifies the expected original bytes before writing.
