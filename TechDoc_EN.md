# Against Rome Modifier Technical Document

This is the embedded English UTF-8 technical document.

For the structured reverse-engineering database, see `docs/reverse-engineering/`.
For tool-readable metadata, see `data/game_schema.json`.

## Supported Game Files

- `SYSTEM/DATA_MP/DEFAULTS/objdef.dau`: unit statistics, movement, sight, weapon damage, cooldown, range, priest spell ranges, VW, and AW.
- `SYSTEM/ress.ini`: construction, production, upgrade, equipment, and priest spell costs.
- `SYSTEM/cl_script.ini`: villager delay, spell radius, and morale parameters.
- `MAPS/**/team.dat`: map population limits.
- `Against_Rome.exe`: compatibility patch for background execution when the game loses focus.
- `apt.dat`: identified as a ZIP-like UI/layout container; not yet patched by the modifier.

## Backup Source

`Backup.zip` is optional for public builds and should not be published to GitHub.
When it is absent, the modifier reads the required original files from the user's
selected game installation directory and keeps that backup in memory.

## Reverse Engineering Workflow

1. Decompress target game data with `GameLZSS.DecompressPfil`.
2. Identify fields in the decompressed text or CSV-like rows.
3. Record confirmed indexes in `docs/reverse-engineering/*.csv`.
4. Add tool-readable metadata to `data/game_schema.json`.
5. Add read-only UI inspection before enabling write patches.
6. Verify that recompressed files load in game.

## Patch Safety Rules

- Always keep an original backup before writing.
- Do not patch `Against_Rome.exe` unless the expected bytes match.
- Preserve decompressed `objdef.dau` text length when modifying fields.
- Treat newly discovered indexes as experimental until tested in game.
- Do not distribute original game assets or decompiled source code.
