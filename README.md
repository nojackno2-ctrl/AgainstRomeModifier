# Against Rome Modifier

This is a Windows Forms modifier for the real-time strategy game *Against Rome*.
It is built with C# on .NET 8. Public builds do not contain original game data;
the modifier builds its restore baseline from the user's own installation when
an optional local `Backup.zip` is not present.

## Maintenance Documentation

- [`TechDoc.md`](TechDoc.md): current Chinese technical specification.
- [`TechDoc_EN.md`](TechDoc_EN.md): current English technical specification.
- [`docs/AI_AGENT_HANDOFF.md`](docs/AI_AGENT_HANDOFF.md): detailed AI-agent
  handoff, debugging history, failure cases, safety contracts, and verification
  checklists. New maintenance agents should read this first.
- [`docs/reverse-engineering/`](docs/reverse-engineering/README.md): file
  formats, offsets, patch bytes, evidence, and the local Ghidra workflow.

## Project Origin & Author's Note

The author of this project is a devoted player who loved *Against Rome* many
years ago. This tool was created with AI-agent assistance and is maintained as a
research and personal modding project.

## Core Features

- Maximum-population switch for map `team.dat` files (1600 when enabled).
- Reversible 20x capacity switch for every positive population-building
  `wohnwer` value in `objdef.dau`.
- Endless-mode AI Ultimate Mode, which raises the mass-army spawn count to the
  vanilla script limit, recycles completed military reinforcement jobs for
  continuing waves, reduces AI respawn wait to 5 seconds, and raises the
  active-party limit to 8 while retaining safe original action-loop pacing.
  Global CLAK economy scripts and settlement templates remain untouched.
- Free construction, production, upgrades, and spell costs through `ress.ini`.
- Unit stat editing for HP, damage, VW, AW, movement, sight, cooldown, range, and spell radius through `objdef.dau` and `cl_script.ini`.
- Troop preset import/export through `.artroop` and global preset import/export through `.arpreset`.
- Background execution patch for `Against_Rome.exe` when the game loses focus.
- Option to scale the village construction/red-frame range to 2.5x through a
  synchronized `Against_Rome.exe` setter trampoline (the underlying setter path
  was runtime-verified at 2x, including the red dashed frame; the new 2.5x
  factor still requires a fresh game test).
- Optional embedded dgVoodoo2 integration that installs the bundled 32-bit
  D3D8/DirectDraw wrappers without overwriting unmanaged DLLs.
- Automatic game path detection and one-click launch.
- Save backup, restore, and history management.
- Embedded technical documentation.
- Local reverse-engineering workflow with a generated Ghidra function index and
  pseudocode inventory kept under ignored `re_workspace/`.

## Technical Architecture

- `Program.cs`: application entry point, DPI setup, and UAC elevation.
- `GameLZSS.cs`: game-specific PFIL/LZSS compression and decompression.
- `TroopConfig.cs`: known unit IDs, unit categories, field indexes, and balance rules.
- `ModifierForm.cs`: main UI layout and embedded documentation view.
- `ModifierForm.Data.cs`: backup loading, data inspection, TGA icon parsing, and display formatting.
- `ModifierForm.Patches.cs`: patch and restore logic for `objdef.dau`, `ress.ini`, `cl_script.ini`, `Against_Rome.exe`, and `team.dat`.
- `ModifierForm.DgVoodoo.cs`: embedded dgVoodoo2 extraction, managed
  installation, conflict detection, and removal.
- `ModifierForm.SaveManager.cs`: save backup, restore, and cache handling.
- `ModifierForm.Presets.cs`: preset import/export.
- `TroopPresetForm.cs`: troop stat preset editor.
- `tools/Repair-LanguageBackup.ps1`: validates and repairs a local language
  overlay backup after an interrupted or incomplete migration.
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
- `MAPS/**/team.dat`: population limits and banner version semantics.
- `MAPS/ENDL_*/SCRIPT/ak_level.bci`: bounded AI Ultimate Mode patch with
  byte/save-state verification; long-running late-wave regression remains.
- `Against_Rome.exe`: focus-loss background execution patch, runtime-verified
  village construction-range expansion, restore-only handling for the rejected
  legacy four-site range/red-frame candidate, and a full local Ghidra function
  inventory.

The generated Ghidra output is local research material, not original source.
Unknown `FUN_*` functions are not treated as understood until the call path or
runtime evidence is documented.

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

The following content is intentionally local-only and covered by `.gitignore`:

- `遊戲原始檔案/`, `Original game archives/`, `Backup.zip`, and extracted game
  trees such as `MAPS/`, `SYSTEM/`, `SAVE/`, and `ToEng/`;
- `.codex/`, `.agents/`, `re_workspace/`, build output, IDE state, dumps, logs,
  and private audit handoff files;
- generated Ghidra inventories and downloaded analysis toolchains.

Small reproducible analysis scripts under `tools/re/` are source material and
are intentionally published. The bundled dgVoodoo2 files are also intentional:
the upstream redistribution terms permit individual files to ship with a game
or game mod; see `ThirdParty/dgVoodoo2/REDISTRIBUTION.md`.

## dgVoodoo2 Integration

Enable the dgVoodoo2 switch and apply changes to extract the bundled v2.87.3
files directly from the modifier. No network connection or separate download is
required. The modifier installs only the x86 `D3D8.dll`, `DDraw.dll`,
`dgVoodooCpl.exe`, and `dgVoodoo.conf`. Uncheck the switch and apply, or restore
compatibility/all settings, to remove files owned by the modifier. Existing
unmanaged DLLs are never overwritten, and a user-edited configuration is kept.

Upstream source and redistribution terms:

- [dgVoodoo2 v2.87.3 release](https://github.com/dege-diosg/dgVoodoo2/releases/tag/v2.87.3)
- [Official redistribution terms](https://dege.fw.hu/dgVoodoo2/ReadmeGeneral/)

## Disclaimer

This modifier is developed for academic exchange and personal modding research.
The intellectual property rights of the game belong to the original rights
holders. Do not distribute original game assets or decompiled source code.
