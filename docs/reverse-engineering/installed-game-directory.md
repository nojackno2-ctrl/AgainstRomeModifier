# Installed game directory investigation

Status: static-verified inventory, 2026-10-09 (Codex / REA Ghidra 12.1.4).
Source: `C:\Program Files (x86)\Against Rome`, read only.
This installation contains custom maps; it is not asserted to be a pristine release.

## Reproduce

```powershell
python tools/re/inventory_game_directory.py 'C:\Program Files (x86)\Against Rome' re_workspace/game-directory-20261009.json
```

Choose a new output path on subsequent runs. The script rejects existing output
and output inside the game directory. It uses the Python standard library,
executes no game programs and extracts no archive members to disk.
The generated manifest remains ignored local evidence, not bundled game data.

Scope: 4,342 files / 1,475,164,698 bytes; 808 files excluded (SAVE, SCRNSHOT,
ToEng, language backups, `.bak`, runtime logs/dumps and `.arm_original`).
No inspection errors. Excluded content was not analyzed. Files outside this
scope and archive entries are not included in the 4,342 file count.

## Executables and archives

The installed `Against_Rome.exe` is 2,486,272 bytes, SHA-256
`6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`.
It exactly matches `re_workspace/Against_Rome.exe`, so the REA analysis of that
immutable local copy applies to this installed EXE's bytes.
Other carriers are `ar.exe`, `UNWISE.EXE`, and `ds_andll.dll`; their identities
are recorded in the manifest, but their code and roles have not been traced.

All eight root `.dat` packages have ZIP local headers and readable ZIP central
directories. Counts below include directory entries where present.

| Package | Entries | Central-directory content | Sampled payload signature |
| --- | ---: | --- | --- |
| alr.dat | 2,075 | 2,075 `.alr` | ALRA (sample version 6) |
| apt.dat | 222 | 222 `.apt` | APAT (sample versions 2 and 3) |
| floortex.dat | 3,005 | 3,005 `.bmp` | BMP |
| gui.dat | 3,924 | 3,921 `.tga`, 3 `.kor` | TGA header bytes; `.kor` comment prefix |
| mp.dat | 237 | 230 `.tga`, 7 extensionless entries | TGA header bytes |
| sfx.dat | 918 | 918 `.wav` | RIFF/WAVE |
| shad.dat | 2,674 | 2,671 `.bmp`, 3 extensionless entries | BMP |
| voice.dat | 1,136 | 1,136 `.wav` | RIFF/WAVE |

The script reads at most three member prefixes per extension. Counts are full
central-directory observations; sampled headers do not establish every member's
format, full CRC validity, or runtime use. In particular `shad.dat` contains
icon paths such as `SYSTEM/DATA_MP/ICONGFX/16/StArblos.bmp`; its name alone does
not imply that every entry is an object shadow.

## Script and wrapper coverage

2,690 inspected loose files begin with `PFIL@`. A bounded LZSS prefix decode
finds 109 BCI0 payloads. There are 110 loose `.bci` files total: the remaining
`MAPS/KAMP_012/SCRIPT/ak_level.bci` starts directly with BCI0.

Additional verification loaded all 110 scripts with existing `tools/bcitool.py`
`Bci`, checked that the code stream has its declared length, and compared all
109 compressed script prefixes and decompressed lengths against that decoder.
All passed. This proves container parsing and agreement of the two decoders;
it does not prove opcode validity, complete wrapper integrity, or gameplay.
The other 2,581 PFIL prefixes remain unclassified rather than presumed text.

Grouping those prefixes exposes useful consumer-tracing targets. These are
observed bytes, not established version numbers or object schemas:

| First four decoded bytes | Files | Example paths |
| --- | ---: | --- |
| `01 00 00 00` | 1,782 | `MAPS/ENDL_000/DATA/action.dat`, `anim.dat` |
| `00 01 00 00` | 280 | `MAPS/ENDL_000/cliprect.dat`, `shadows.dat` |
| `[set` | 146 | settlement `.sdl` |
| `;Ver` | 80 | multiplayer settlement `.sdt` |
| `[Wat` | 74 | `boden.ini` |
| `[Dim` | 74 | `boden.txt` |

SDL and SDT prefixes differ in this installation; treating either as an
interchangeable opaque placement payload would need additional parsing and
consumer evidence. Binary map-pool families dominate the remaining files and
are a priority for investigating the empty-map loader stall.

## Investigation queue

1. Trace EXE file-open/lookup precedence between loose SYSTEM files and ZIP
   members, including locale-dependent paths and configuration registration.
2. Classify remaining PFIL payload families by their consumers; trace map DATA,
   SDL/SDT and script lifecycle together to diagnose the empty-map load stall.
3. Trace the `ar.exe` and `ds_andll.dll` boundaries without assuming their roles.
4. Reconcile clock capture/rebase with actual save/load and pause callers;
   see `script-clock.md`. Controlled runtime evidence is still required.
5. Preserve separate coverage for render assets, audio, scripts, AI, save state,
   GUI and network behavior. Directory inventory is not complete reverse engineering.
