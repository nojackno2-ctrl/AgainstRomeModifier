# Against Rome Modifier

This is a Windows Forms modifier for the real-time strategy game *Against Rome*.
It is built with C# on .NET 8 and packages the original backup data as an
embedded resource so the modifier can patch and restore game files from a single
application.

## Project Origin & Author's Note

The author of this project is a devoted player who loved *Against Rome* many
years ago. This tool was created with AI-agent assistance and is maintained as a
research and personal modding project.

## Core Features

- Population limit customization for map `team.dat` files.
- Free construction, production, upgrades, equipment, and spell costs through `ress.ini`.
- Unit stat editing for HP, damage, VW, AW, movement, sight, cooldown, range, and spell radius through `objdef.dau` and `cl_script.ini`.
- Troop preset import/export through `.artroop` and global preset import/export through `.arpreset`.
- Background execution patch for `Against_Rome.exe` when the game loses focus.
- Automatic game path detection and one-click launch.
- Save backup, restore, and history management.
- Embedded technical documentation.

## Technical Architecture

- `Program.cs`: application entry point, DPI setup, and UAC elevation.
- `GameLZSS.cs`: game-specific PFIL/LZSS compression and decompression.
- `TroopConfig.cs`: known unit IDs, unit categories, field indexes, and balance rules.
- `ModifierForm.cs`: main UI layout and embedded documentation view.
- `ModifierForm.Data.cs`: backup loading, data inspection, TGA icon parsing, and display formatting.
- `ModifierForm.Patches.cs`: patch and restore logic for `objdef.dau`, `ress.ini`, `cl_script.ini`, `Against_Rome.exe`, and `team.dat`.
- `ModifierForm.SaveManager.cs`: save backup, restore, and cache handling.
- `ModifierForm.Presets.cs`: preset import/export.
- `TroopPresetForm.cs`: troop stat preset editor.
- `docs/reverse-engineering/`: structured reverse-engineering notes.
- `data/game_schema.json`: tool-readable file format and patch metadata.

## Embedded Resources

- `Backup.zip` is optional and intentionally not committed to GitHub.
- If `Backup.zip` is embedded or placed next to the executable, it is loaded as the restore source.
- If no `Backup.zip` exists, the modifier builds an in-memory backup from the user's selected game installation directory.
- `TechDoc.md` is embedded as `TechDoc.md`.
- `TechDoc_EN.md` is embedded as `TechDoc_EN.md`.
- Game payloads are decoded as code page 1251 where required; project documentation is UTF-8.

## Reverse Engineering Data

The project keeps reverse-engineering notes in `docs/reverse-engineering/`. The
same facts are mirrored in machine-readable form in `data/game_schema.json` so
future UI and patch code can avoid hardcoded indexes.

Current coverage:

- `SYSTEM/DATA_MP/DEFAULTS/objdef.dau`: unit stats and weapon fields.
- `SYSTEM/ress.ini`: construction, production, upgrade, and spell costs.
- `SYSTEM/cl_script.ini`: villager delay, spell radius, and morale parameters.
- `MAPS/**/team.dat`: population limits.
- `Against_Rome.exe`: focus-loss background execution patch.

## Development Environment

- Language: C# 12
- Target framework: .NET 8.0 Windows
- UI: Windows Forms
- Platform target: x64

## Build Steps

1. Install .NET 8.0 SDK and Visual Studio 2022.
2. `Backup.zip` is optional for public builds. Keep it local only if you have one.
3. Open `AgainstRomeModifier.slnx` or `AgainstRomeModifier.csproj`.
4. Select `Release` and `x64`.
5. Build the solution. Output is under `bin/Release/net8.0-windows/`.

## Public Build Behavior

The GitHub repository does not include original game files. Users must own and
install *Against Rome*, then select the game folder in the modifier. The modifier
uses those local files as the clean restore baseline before applying patches.

## Disclaimer

This modifier is developed for academic exchange and personal modding research.
The intellectual property rights of the game belong to the original rights
holders. Do not distribute original game assets or decompiled source code.
