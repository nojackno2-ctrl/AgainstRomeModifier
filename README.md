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
- **Play as Romans in Endless Mode**: From Resource & Combat Upgrades, changes the five endless-map team-0 defaults to Romans and safely patches the endless-only `dlg_volk` selector so every faction flag starts as Roman. Runtime-verified; it affects new games only. Romans have no priest or glory skill tree by original design.
- **Hand Over All Roman Reinforcements**: Standalone feature that releases every Roman reinforcement object from reinforcement-party script mode and hands soldiers, pack horses, and civilians to the village instead of splitting them by unit type. It applies the P8 threshold, P9 zero quota/all-unit fall-through, and release helper atomically. Legacy `EndlessAi.M6`, the former `jnz+92` split, direct-mark states, and the broken opcode-160 helper state migrate automatically to `jz+0` plus the correct opcode-120 helper. Five-map static/integration checks and a fresh endless-game runtime test have passed: the complete reinforcement party is handed to the village and no longer retreats.
- **20x Housing Capacity**: Reversible switch to scale every positive population-building `wohnwer` value in `objdef.dau` by 20x.
- **20 Civilians Per Residential-Tent Click**: Reversible player-only EXE patch that queues up to 20 male or female civilians with one click; successfully runtime-verified in-game.
- **Fill Unit Conversion to 20 Per Click**: Reversible player-only EXE patch that sets the selected villager-to-unit conversion count to 20 with one click, subject to the original cap and available villagers; successfully runtime-verified in-game.
- **10x Construction Speed**: Reversible switch to accelerate construction, upgrades, and repairs in `objdef.dau` by 10x (shortens building build/upgrade times by 10, which automatically boosts the per-second repair rate; successfully runtime-verified in-game).
- **10x Storage Capacity**: Reversible switch to scale storage capacities of Town Halls (`Hau` structures) and Warehouses (`Lag` structures) in `objdef.dau` by 10x (successfully runtime-verified in-game).
- **10x Town Hall HP**: Reversible switch to multiply hit-points (HP) of all Town Halls (`Hau` structures) in `objdef.dau` by 10x (successfully runtime-verified in-game).
- **Endless-Mode AI Ultimate Mode**: Reversible, bounded BCI patches for the five `ENDL_*` maps. The UI applies reinforcement scheduling, defeat cleanup, and settlement respawn atomically as one **Endless Respawn Core**, preventing partial lifecycle configurations; reinforcement size and AI starting resources remain independent difficulty choices. It never uses the previously rejected unconditional gate bypass. See [`endless-mode-ai.md`](docs/reverse-engineering/endless-mode-ai.md) for evidence, legacy migration, and runtime limits.
- **Village Garrison Quota 3x (Experimental)**: A standalone reversible patch for `Dorfverteidigung.bci`. It multiplies the four dynamic ImportantPos-derived squad quotas by three while preserving their ratios and zero values. This is independent of AI Ultimate M1's 20 members per newly produced squad. Static signatures and exact restore are verified; fresh-game runtime and performance verification remain pending, so it is excluded from **Enable All**.
- **Free Construction & Production**: Free construction, production, upgrades, and spell costs through `ress.ini` modification.
- **5x Spell Damage**: Reversible `cl_script.ini` patch for six configured active-spell values; successfully runtime-verified in-game.
- **Unit Stat Editing**: Adjust only HP, damage, VW, AW, sight, and cooldown through `objdef.dau`. Movement speed, attack range, spell radius, and priest sight/casting distance are deliberately owned by the independent experimental modifiers, not by custom troop layers.
- **Entire-Map Vision for All Units**: Sets `Sirad=30000` for every supported unit across all factions; the entire-map vision effect is runtime-verified in game.
- **Other Stat Modifiers**: 3× ranged range (with a built-in accuracy fix — doubled impact radius and zeroed lead-aim scatter so shots still land at the longer range), 2× unit movement, entire-map priest casting distance, 3× spell effect radius, and higher projectile arcs.
- **Centered High Resolution (4:3)**: The native 1920×1080 EXE experiment is disabled because it retained a legacy top-left draw surface. This option restores every experimental EXE site and keeps stock 1600×1200. Centered fullscreen and correctly proportioned windowed output are runtime-verified; it does not stretch or claim 16:9 world-view expansion.
- **Camera Zoom Out +1 (Experimental)**: Uses the first visibly effective native zoom step to show more battlefield. Runtime testing found no visible effect at 0.5, so that state migrates back to +1; the tradeoff is that world units, health bars, and information also become smaller. Selection, edge scrolling, fog, bounds, and mission compatibility still need retesting.
- **Troop Presets & One-Click Control**: `.artroop` exports use the six-field `HP,Dmg,VW,AW,Sight,Relt` format. Legacy nine-field imports remain readable but their removed fields are discarded; Enable All includes the runtime-verified centered-high-resolution and entire-map-unit-vision features.
- **Loss-Focus Background Execution**: Patches `Against_Rome.exe` to allow the game to continue running when it loses focus (is minimized or inactive).
- **Entire Map Construction Range**: Replaces the construction range limit via a synchronized `Against_Rome.exe` setter trampoline, allowing building anywhere on the entire map (successfully runtime-verified in-game, including the red dashed frame).
- **Embedded dgVoodoo2 Integration**: Optional integration that installs the bundled 32-bit D3D8/DirectDraw wrappers from dgVoodoo2 without overwriting existing unmanaged DLLs.
- **Path Auto-Detection**: Automatically detects game directories and supports launching the game with a single click.
- **Save Game Management**: Save backup, restore, and history management.
- **Embedded Technical Documentation**: Built-in viewer for easy documentation reading.
- **Local Reverse-Engineering Workflow**: Retains generated Ghidra function index and pseudocode inventory kept under the ignored `re_workspace/` folder.

## Playing Romans in Endless Mode

1. Select the game root containing `Against_Rome.exe`.
2. Enable **Play as Romans in Endless Mode** in Resource & Combat Upgrades and apply changes. Enable All includes it.
3. Start a **new** endless game. Any of the three faction flags enters as Romans; no highlighted flag is an expected visual limitation.
4. Uncheck and apply again, or restore, to return new games to their original faction. Existing saves keep their embedded team data.

The original Options `swi_volk` control is a separate player-driven path and can still manually change the faction back to a barbarian one.

## Technical Architecture

The solution (`AgainstRomeModifier.slnx`) is split into decoupled projects, each producing its own executable or library:

- [`src.Launcher/`](src.Launcher/): `AgainstRomeLauncher.exe` — suite entry point that launches the other tools and hosts the technical documentation viewer (`TechDocForm`).
- [`src.Modifier/`](src.Modifier/): `AgainstRomeModifier.exe` — the modifier UI. `ModifierForm` partial classes cover layout, data inspection (backup loading, TGA icon parsing), patch application (feature-toggle map → `PatchProfile` → transactional apply/category restore), presets, and the troop stat preset editor (`TroopPresetForm`).
- [`src.SaveManager/`](src.SaveManager/): `AgainstRomeSaveManager.exe` — save backup, restore, and preview (`SaveManagerForm`).
- [`src.MapEditor/`](src.MapEditor/): `AgainstRomeMapEditor.exe` — map editor, including endless-map clone/delete management.
- [`src.Core/`](src.Core/): `AgainstRome.Core.dll` — the shared modifier core: registry-backed feature definitions (`Core/Features/`), file patchers (`Core/Patches/`), transactional services such as [`PatchEngine.cs`](src.Core/Core/Services/PatchEngine.cs) (FoodHealing and Endless AI share the BCI cache and are committed by one `SaveAll`), [`TroopConfig.cs`](src.Core/Core/TroopConfig.cs) (known unit IDs, categories, field indexes, balance rules), localization, and the embedded `Backup.zip`/dgVoodoo2 payloads.
- [`src.Shared/`](src.Shared/): `AgainstRome.Shared.dll` — lowest-level shared library: [`GameLZSS.cs`](src.Shared/Core/GameLZSS.cs) (game-specific PFIL/LZSS compression), `SafeFileWriter`, `FileRollbackScope`, and the map catalog/clone/delete services (`Maps/`).
- [`tools/publish.ps1`](tools/publish.ps1): Publishes all four executables and packages them into the release ZIP.
- [`tools/Repair-LanguageBackup.ps1`](tools/Repair-LanguageBackup.ps1): Validates and repairs a local language overlay backup after an interrupted or incomplete migration.
- [`docs/reverse-engineering/`](docs/reverse-engineering/): Structured reverse-engineering notes.
- [`data/game_schema.json`](data/game_schema.json): Tool-readable file format and patch metadata.

## Embedded Resources

- `Backup.zip` is optional and intentionally not committed to GitHub.
- If `Backup.zip` is embedded (in `AgainstRome.Core.dll`) or placed next to the executable, it is loaded as the restore source.
- If no `Backup.zip` exists, the modifier builds an in-memory backup from the user's selected game installation directory.
- `TechDoc.md` and `TechDoc_EN.md` are embedded in the launcher for the documentation viewer.
- Game payloads are decoded as code page 1251 where required; project documentation is UTF-8.

## Reverse Engineering Data

The project keeps reverse-engineering notes in [`docs/reverse-engineering/`](docs/reverse-engineering/). The same facts are mirrored in machine-readable form in [`data/game_schema.json`](data/game_schema.json) so future UI and patch code can avoid hardcoded indexes.

Current coverage:

- `SYSTEM/DATA_MP/DEFAULTS/objdef.dau`: Unit stats and weapon fields.
- `SYSTEM/ress.ini`: Construction, production, upgrade, and spell costs.
- `SYSTEM/cl_script.ini`: Villager delay, spell radius, and morale parameters.
- `MAPS/**/team.dat`: Population limits, player faction, and banner version semantics; Roman Endless changes only team 0 in `ENDL_*` maps to `ROM`.
- `MAPS/ENDL_*/SCRIPT/ak_level.bci`: Bounded AI Ultimate Mode patch; the active-party limit is 8 because the runtime has 20 NPC-job slots.
- `SYSTEM/CLAK/SCRIPT/Dorfverteidigung.bci`: AI Ultimate M1 squad-size patch and the independent Experimental village-garrison quota multiplier.
- `Against_Rome.exe`: Focus-loss patch, runtime-verified village construction-range expansion and Roman Endless selector patch, restore-only handling for the rejected legacy four-site range/red-frame candidate, and a full local Ghidra function inventory.

The generated Ghidra output is local research material, not original source. Unknown `FUN_*` functions are not treated as understood until the call path or runtime evidence is documented.

## Development Environment

- Language: C# 12
- Target framework: .NET 8.0 Windows
- UI: Windows Forms
- Platform target: x64

## Build Steps

1. Install .NET 8.0 SDK and Visual Studio 2022.
2. `Backup.zip` is optional for public builds. Keep it local only if you have one.
3. Open `AgainstRomeModifier.slnx`.
4. Select `Release` and `x64`.
5. Build the solution. Each app's output is under its own project folder, e.g. `src.Modifier/bin/Release/net8.0-windows/`. To produce a release package with all four executables, run `tools/publish.ps1`.

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
