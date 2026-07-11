# File Formats

> Reviewed 2026-07-11. Preserve the documented PFIL/LZSS ring initialization and Windows-1251 byte round-trip when changing text payload handling.

## Compression

Most editable game data files are stored with the game's `PFIL@` LZSS wrapper.
The modifier uses `GameLZSS.DecompressPfil` and `GameLZSS.CompressPfil`.

Known compressed text files:

- `SYSTEM/ress.ini`
- `SYSTEM/cl_script.ini`
- `SYSTEM/CLAK/cl_scint.ini` (ODef aliases, `SpellODef` unit types — see
  `priest-spells.md`)
- `SYSTEM/cl_epara.ini` (combat-state factors: Berserker/Schutzschild/
  Donnerschlag multipliers)
- `SYSTEM/banner.ini`
- `SYSTEM/DATA_MP/DEFAULTS/objdef.dau`
- `MAPS/**/team.dat`

Known compressed binary/script files:

- `MAPS/ENDL_*/SCRIPT/ak_level.bci`: `PFIL@` wrapper around a `BCI0`
  compiled script payload.
- `MAPS/ENDL_*/Endlos_*_Siedlung*.sdl`: settlement template payloads used by
  endless-mode village-style AI spawns.

## Text Encoding

Decompressed game payloads are handled as Windows code page 1251 in the current
modifier. Project documentation is UTF-8.

## CSV-Like Files

`objdef.dau` and parts of `ress.ini` are CSV-like text:

- Field indexes are zero-based.
- Rows are line-based.
- `objdef.dau` patching must preserve decompressed text length.
- New parser work should prefer structured CSV helpers over ad hoc splitting.

## ZIP-Like Files

`apt.dat` appears to be a ZIP-style container with files under
`SYSTEM/DATA/APT/*.apt`. It is identified but not currently patched.
