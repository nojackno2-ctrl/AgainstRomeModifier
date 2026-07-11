[English](README.md) | [繁體中文](README.zh-TW.md)

---

# Against Rome Modifier

> [!WARNING]
> This modifier is still in testing. Please backup your original files before using it.
> (這個修改器還在測試中，如果要使用請將原始的檔案進行備份。)

This is a Windows Forms modifier for the real-time strategy game *Against Rome*.
It is built with C# on .NET 8. Public builds do not contain original game data; the modifier builds its restore baseline from the user's own installation when an optional local `Backup.zip` is not present.

## Maintenance Documentation

- [`TechDoc.md`](TechDoc.md): Current Chinese technical specification (integrates the AI-agent handoff checklist, debugging history, failure cases, and verification steps at the end).
- [`TechDoc_EN.md`](TechDoc_EN.md): Current English technical specification.
- [`docs/reverse-engineering/`](docs/reverse-engineering/README.md): File formats, offsets, patch bytes, evidence, and the local Ghidra workflow.

## Project Origin & Author's Note

The author of this project is a devoted player who loved *Against Rome* many years ago. This tool was created with AI-agent assistance and is maintained as a research and personal modding project.

## Core Features

- **Maximum-Population Unlock**: Modifies map `team.dat` files, raising the population limit up to 1600 when enabled.
- **20x Housing Capacity**: Reversible switch to scale every positive population-building `wohnwer` value in `objdef.dau` by 20x.
- **10x Construction Speed**: Reversible switch to accelerate construction, upgrades, and repairs in `objdef.dau` by 10x (shortens building build/upgrade times by 10, which automatically boosts the per-second repair rate; successfully runtime-verified in-game).
- **10x Storage Capacity**: Reversible switch to scale storage capacities of Town Halls (`Hau` structures) and Warehouses (`Lag` structures) in `objdef.dau` by 10x (successfully runtime-verified in-game).
- **10x Town Hall HP**: Reversible switch to multiply hit-points (HP) of all Town Halls (`Hau` structures) in `objdef.dau` by 10x (successfully runtime-verified in-game).
- **Endless-Mode AI Ultimate Mode**: Split into six independently selectable modules:
  - Raises the mass-army spawn count to the vanilla script limit.
  - Recycles completed military reinforcement jobs for continuing waves.
  - Reduces the military reinforcement wait time to 5 seconds.
  - Cuts four non-settlement party retreat deadlines from 10 minutes to 5 seconds.
  - Preserves the two settlement cleanup fallbacks at 10 minutes so an old village and its palisades finish clearing before that team slot is reused.
  - Accelerates the confirmed one-object-at-a-time cleanup cadence from about 1.5 seconds to 0.1 seconds per object.
  - Raises the military-reinforcement unit threshold from 4 to 40.
  - Bypasses the main-house resource and transient leader/civilian checks that otherwise stop later waves below that limit.
  - Transfers the whole reinforcement party into the village instead of retreating, while retaining the type-4 settlement, building, one-party-at-a-time, and unit-count safety gates.
  - Endless settlement templates also receive a reversible starting-resource boost.
  - New games can fix the type-1 village-AI cap at 4 while keeping the separate type-4 military settlement available as the fifth settled opponent.
  - (Unsafe global CLAK production/training edits stay disabled).
- **Free Construction & Production**: Free construction, production, upgrades, and spell costs through `ress.ini` modification.
- **Unit Stat Editing**: Adjust HP, damage, VW, AW, movement speed, sight range, cooldown times, attack range, and spell radius through `objdef.dau` and `cl_script.ini`.
- **Troop Presets & One-Click Control**: Import/export troop presets via `.artroop` files, and provide one-click buttons to enable/disable all features.
- **Loss-Focus Background Execution**: Patches `Against_Rome.exe` to allow the game to continue running when it loses focus (is minimized or inactive).
- **Entire Map Construction Range**: Replaces the construction range limit via a synchronized `Against_Rome.exe` setter trampoline, allowing building anywhere on the entire map (successfully runtime-verified in-game, including the red dashed frame).
- **Embedded dgVoodoo2 Integration**: Optional integration that installs the bundled 32-bit D3D8/DirectDraw wrappers from dgVoodoo2 without overwriting existing unmanaged DLLs.
- **Path Auto-Detection**: Automatically detects game directories and supports launching the game with a single click.
- **Save Game Management**: Save backup, restore, and history management.
- **Embedded Technical Documentation**: Built-in viewer for easy documentation reading.
- **Local Reverse-Engineering Workflow**: Retains generated Ghidra function index and pseudocode inventory kept under the ignored `re_workspace/` folder.

## Technical Architecture

- [`src/Program.cs`](src/Program.cs): Application entry point, DPI setup, and UAC elevation.
- [`src/Core/GameLZSS.cs`](src/Core/GameLZSS.cs): Game-specific PFIL/LZSS compression and decompression.
- [`src/Core/TroopConfig.cs`](src/Core/TroopConfig.cs): Known unit IDs, unit categories, field indexes, and balance rules.
- [`src/UI/ModifierForm.cs`](src/UI/ModifierForm.cs): Main UI layout and embedded documentation view.
- [`src/UI/ModifierForm.Data.cs`](src/UI/ModifierForm.Data.cs): Backup loading, data inspection, TGA icon parsing, and display formatting.
- [`src/UI/ModifierForm.Patches.cs`](src/UI/ModifierForm.Patches.cs): Patch and restore logic for `objdef.dau`, `ress.ini`, `cl_script.ini`, `Against_Rome.exe`, and `team.dat`.
- [`src/UI/ModifierForm.DgVoodoo.cs`](src/UI/ModifierForm.DgVoodoo.cs): Embedded dgVoodoo2 extraction, managed installation, conflict detection, and removal.
- [`src/UI/ModifierForm.SaveManager.cs`](src/UI/ModifierForm.SaveManager.cs): Save backup, restore, and cache handling.
- [`src/UI/ModifierForm.Presets.cs`](src/UI/ModifierForm.Presets.cs): Actions to enable or disable all features at once.
- [`src/UI/TroopPresetForm.cs`](src/UI/TroopPresetForm.cs): Troop stat preset editor.
- [`tools/Repair-LanguageBackup.ps1`](tools/Repair-LanguageBackup.ps1): Validates and repairs a local language overlay backup after an interrupted or incomplete migration.
- [`docs/reverse-engineering/`](docs/reverse-engineering/): Structured reverse-engineering notes.
- [`data/game_schema.json`](data/game_schema.json): Tool-readable file format and patch metadata.

## Embedded Resources

- `Backup.zip` is optional and intentionally not committed to GitHub.
- If `Backup.zip` is embedded or placed next to the executable, it is loaded as the restore source.
- If no `Backup.zip` exists, the modifier builds an in-memory backup from the user's selected game installation directory.
- `TechDoc.md` is embedded as `TechDoc.md`.
- `TechDoc_EN.md` is embedded as `TechDoc_EN.md`.
- Game payloads are decoded as code page 1251 where required; project documentation is UTF-8.

## Reverse Engineering Data

The project keeps reverse-engineering notes in [`docs/reverse-engineering/`](docs/reverse-engineering/). The same facts are mirrored in machine-readable form in [`data/game_schema.json`](data/game_schema.json) so future UI and patch code can avoid hardcoded indexes.

Current coverage:

- `SYSTEM/DATA_MP/DEFAULTS/objdef.dau`: Unit stats and weapon fields.
- `SYSTEM/ress.ini`: Construction, production, upgrade, and spell costs.
- `SYSTEM/cl_script.ini`: Villager delay, spell radius, and morale parameters.
- `MAPS/**/team.dat`: Population limits and banner version semantics.
- `MAPS/ENDL_*/SCRIPT/ak_level.bci`: Bounded AI Ultimate Mode patch with byte/save-state verification; long-running late-wave regression remains.
- `Against_Rome.exe`: Focus-loss background execution patch, runtime-verified village construction-range expansion, restore-only handling for the rejected legacy four-site range/red-frame candidate, and a full local Ghidra function inventory.

The generated Ghidra output is local research material, not original source. Unknown `FUN_*` functions are not treated as understood until the call path or runtime evidence is documented.

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

The GitHub repository does not include original game files. Users must own and install *Against Rome*, then select the game folder in the modifier. The modifier uses those local files as the clean restore baseline before applying patches.

The following content is intentionally local-only and covered by `.gitignore`:

- `遊戲原始檔案/`, `Original game archives/`, `Backup.zip`, and extracted game trees such as `MAPS/`, `SYSTEM/`, `SAVE/`, and `ToEng/`.
- `.codex/`, `.agents/`, `re_workspace/`, build output, IDE state, dumps, logs, and private audit handoff files.
- Generated Ghidra inventories and downloaded analysis toolchains.

Small reproducible analysis scripts under `tools/re/` are source material and are intentionally published. The bundled dgVoodoo2 files are also intentional: the upstream redistribution terms permit individual files to ship with a game or game mod; see [`ThirdParty/dgVoodoo2/REDISTRIBUTION.md`](ThirdParty/dgVoodoo2/REDISTRIBUTION.md).

## Maintenance Tools

- [`tools/Repair-LanguageBackup.ps1`](tools/Repair-LanguageBackup.ps1) — Out-of-band repair for the English language overlay's backup baseline (`.against-rome-modifier-language-backup` inside the game folder). Use it only when the modifier reports a missing or corrupted language backup manifest: it rebuilds the baseline by hashing the active overlay against a clean original game tree. It writes directly into the game install directory, so read the script's validation steps before running. Defaults: `-GamePath 'C:\Program Files (x86)\Against Rome'`; the clean original tree is auto-detected under the repository root.
- `tools/bcitool.py` — Python reader/disassembler for `BCI0` script bytecode. Its PFIL LZSS decompressor is a port of `GameLZSS`; keep the two in sync if the C# algorithm ever changes.
- `tests/verify_split_patches/` — Manual golden-value verification harness for the Endless AI patches. Requires a local `遊戲原始檔案/` tree. Run it before and after changing any P1–P19 patch constant.

## dgVoodoo2 Integration

Enable the dgVoodoo2 switch and apply changes to extract the bundled v2.87.3 files directly from the modifier. No network connection or separate download is required. The modifier installs only the x86 `D3D8.dll`, `DDraw.dll`, `dgVoodooCpl.exe`, and `dgVoodoo.conf`. Uncheck the switch and apply, or restore compatibility/all settings, to remove files owned by the modifier. Existing unmanaged DLLs are never overwritten, and a user-edited configuration is kept.

Upstream source and redistribution terms:
- [dgVoodoo2 v2.87.3 release](https://github.com/dege-diosg/dgVoodoo2/releases/tag/v2.87.3)
- [Official redistribution terms](https://dege.fw.hu/dgVoodoo2/ReadmeGeneral/)

## Disclaimer

This modifier is developed for academic exchange and personal modding research. The intellectual property rights of the game belong to the original rights holders. Do not distribute original game assets or decompiled source code.
