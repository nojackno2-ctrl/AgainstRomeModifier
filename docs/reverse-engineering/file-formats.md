# File Formats

## Compression

Most editable game data files are stored with the game's `PFIL@` LZSS wrapper.
The modifier uses `GameLZSS.DecompressPfil` and `GameLZSS.CompressPfil`.

Known compressed text files:

- `SYSTEM/ress.ini`
- `SYSTEM/cl_script.ini`
- `SYSTEM/DATA_MP/DEFAULTS/objdef.dau`
- `MAPS/**/team.dat`

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
