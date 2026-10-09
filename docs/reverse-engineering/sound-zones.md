# Ambient and placed sounds: real evidence and export boundary

Reviewed 2026-10-08, Codex, branch `wt/sound`, baseline `080ebc9`.

**Conclusion:** native sound mechanisms exist, but a complete, sample-verifiable
ambient-zone writer cannot be established from the authorized copies. Soundscape
remains **experimental / unwired**. No native format or script emitter was added.
`DATA/sound.dat` and `SNDZ` are fabricated editor prototype contracts, not verified
game formats. Native file/script export now throws before writing anything;
private binary/JSON draft serialization and geometric preview remain available.

## Scope, provenance and reproducibility

Only these inputs were read: `%TEMP%/ArmGameCompare_20261007` (ENDL_000,
ENDL_005, SYSTEM), `%TEMP%/ArmNativeAssets_20261007/objdef.dau`, and the main
repository's **read-only** `re_workspace/Against_Rome.exe` copy. All edits,
analysis output, build/test output and commits are in `D:/Github/ARM_wt_sound`.
The installed game directory was never accessed and no game was launched.

| Input | SHA-256 of original file |
| --- | --- |
| EXE copy, 2,486,272 bytes | `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf` |
| TEMP objdef.dau, 439,006 bytes | `baba4840965ef93e3325eb4f3a1d274e59fdb91bb3e93b34a31369292da161f8` |
| ENDL_000/SCRIPT/ak_level.bci | `336e3a5cb808e7f2ef6aaa77ac2ef54a0fb00de04cb00d1abb4fbd5b937d5f27` |
| ENDL_005/SCRIPT/ak_level.bci | `faaba1e6653327c04610ac0ab4a728d1faadde4cf750eb9598f33e0504fd769e` |

Reproduce with the committed read-only probe (uses the existing bcitool PFIL/BCI
reader; no assets are committed). Run from the sound worktree:

```powershell
python tools/re/sound_probe.py --compare "$env:TEMP/ArmGameCompare_20261007" `
  --objdef "$env:TEMP/ArmNativeAssets_20261007/objdef.dau" `
  --exe D:/Github/AgainstRomeModifier/re_workspace/Against_Rome.exe
# Optional static disassembly; requires pefile and capstone on Python's path:
# append --disassembly
python tools/bcitool.py dis "$env:TEMP/ArmGameCompare_20261007/ENDL_000/SCRIPT/ak_level.bci" 0x3594 0x35a8
```

Objdef/pool offsets below are in **decompressed PFIL payloads**. BCI offsets are
relative to the code stream (add `0x24` for payload file offsets). EXE addresses
are PE virtual addresses, image base `0x400000`; quoted string file offsets are
also supplied. Static disassembly proves instructions, not game playback.

## 1. Actual map and system files

Both DATA directories contain `action, anim, biglager, engine, explos, flash,
formatio, fow, fowreq, gametime, gfxtype, hagel, hirarchy, hitex, lager, light,
objdata, objects, particle, position, rain, snow, stat, team, way` `.dat` files
(and `team.dat.bak`). **Neither contains sound.dat.** Neither entire level copy
contains a raw `SNDZ` occurrence. These observations cover two samples, not every
possible game version. The native pools are PFIL, not the prototype's SNDZ.

| Pool | Decoded bytes | Payload offset 0 hex excerpt |
| --- | ---: | --- |
| action.dat | 350012 | `01 00 00 00 b0 36 00 00 06 00 00 00 01 00 00 00` |
| flash.dat | 23312 | `01 00 00 00 64 00 00 00 10 00 00 00 00 00 00 00` |
| particle.dat | 2322444 | `01 00 00 00 00 04 00 00 40 00 00 00 00 00 00 00` |
| light.dat | 57352 | `01 00 00 00 00 04 00 00 00 00 00 00 00 00 00 00` |
| engine.dat | 94 | `01 00 00 00 fc 5f 15 46 00 00 00 00 29 45 28 46` |

Each of these five files has the same original-file SHA-256 in ENDL_000 and
ENDL_005 (probe prints full hashes). Their headers alone do not identify sound
zone fields. They must not be repurposed as a guessed audio pool. The authorized
SYSTEM copy contains only `cl_alr.ini`, `cl_apt.ini`, `cl_shado.ini`; neither TEMP
input set contains `sfxenv.dau`, `sfxobj.dau`, `sfxexp.dau`, `cl_sfx.ini` or
`voice.ini`. `lightdef.dau` is available in ArmNativeAssets, but is a light table,
not evidence of an analogous sound-zone structure.

## 2. Native sound catalogs and trigger tables in the EXE

These are **confirmed static loader references**, rather than filename guesses:

| File / section | EXE string file offset / VA | Consumer evidence |
| --- | --- | --- |
| sfxenv.dau | `0x1f32bf` / `0x5f32bf` | `0x480e9e` push; `0x480ea3` call `0x4e1260` |
| sfxobj.dau | `0x1f32e2` / `0x5f32e2` | `0x480ec9` push; call `0x4e1880` |
| sfxexp.dau | `0x1f3305` / `0x5f3305` | `0x480ef4` push; call `0x4e2790` |
| SYSTEM\cl_sfx.ini / [SfxNames] | `0x1f6cb5` / `0x5f6cb5`; `0x1f6cdc` / `0x5f6cdc` | `0x4deba3` pushes section name |
| [SfxEnviroment] (original spelling) | `0x1f72e7` / `0x5f72e7` | `0x4e12a3` push in loader `0x4e1260` |
| [SfxObjects] | `0x1f73e4` / `0x5f73e4` | `0x4e1906` push in loader `0x4e1880` |
| [SfxExplosion] | `0x1f7486` / `0x5f7486` | `0x4e27d3` push in loader `0x4e2790` |

The EXE also has variants `sfxenv.d%02ld`, `sfxobj.d%02ld`, `sfxexp.d%02ld`
(VAs `0x5f33dd`, `0x5f33eb`, `0x5f33f9`), and a separate `sfxobj.d44` /
`[SfxObjects_ver2]` writer. Version/variant selection must be understood before
editing these tables. The environment loader formats `%s%s` using a global path
at `0x770de8`; no claim is made that the files always reside directly under SYSTEM.

Writer header literals, starting around VA `0x5f71b7` / file `0x1f71b7`:

```text
[SfxEnviroment]
;p_rain,rain 1,rain 2,rain 3,rain 4
,p_gisc,gisc 1,gisc 2,gisc 3,gisc 4
,p_kamp,kamp 1,kamp 2,kamp 3,kamp 4
,p_flas,flas 1,flas 2,flas 3,flas 4
[SfxObjects]
; idx ,p_%04d,A1_%03d,A2_%03d,A3_%03d,A4_%03d
[SfxExplosion]
; idx ,propab,Alter1,Alter2,Alter3,Alter4
```

Environment loader `0x4e1260` supplies callback `0x4e0fe0`, one row and 20
columns to table reader `0x42a230`. Callback column 0 parses `%ld` and stores an
int16 at `0x84bfcc` (`0x4e100f`); column 1 stores at `0x84bfce` (`0x4e102d`).
At `0x4df349`, native code passes `0x84bfce` to selector `0x4e0f20`; a
nonnegative result is sent to sound playback/registration `0x4dfaa0` at
`0x4df36d`. This establishes environmental table selection in executable code.
Names strongly suggest rain, spray/surf, combat and lightning categories;
timing, audibility, probability interpretation and sample mapping remain unverified.

Object/action selector `0x4e1470` validates the object and action range 0..63,
extracts its definition index, checks another object predicate, obtains a random
value modulo 100, and compares it against an int16 probability. The per-definition
table is at `0x84bff4`, stride `0x280`; four int16 alternatives per action begin
at table offset `0x80`. The EXE enum table at `0x61cb04` pairs
`SFX_ACTION_PERMANENT` (string VA `0x5e167c`, file `0x1e167c`) with `0x23`.
Many object logic functions call `0x4e1470` (e.g. `0x4a9cee`, `0x4ad5b3`,
`0x4b6ff3`). This supports object/action-based sounds, **not** arbitrary
polygon-zone serialization. Continuous-loop semantics are not proven by the word
PERMANENT alone; scheduling and playback must still be traced.

## 3. Objdef and possible placed sound markers

The real decoded objdef is 3,310,807 bytes. Zero-based columns 155 `watesfx`
(header text offset `0x479`) and 159 `kampsfx` (`0x499`) exist:

| Row / decoded row offset | Name | watesfx | kampsfx |
| --- | --- | ---: | ---: |
| 12 / `0x4dde` | FigGerSch01_Axt_Schild | 0 | 1 |
| 184 / `0x45338` | FX_Gischt00 | 1 | 0 |
| 1753 / `0x29012e` | SFXmark00_Regen_1 | 0 | 0 |
| 1764 / `0x294302` | SFXmark11_Uhu | 0 | 0 |
| 1773 / `0x2978de` | SFXmark20_Wasser_1 | 0 | 0 |
| 1778 / `0x2996ca` | SFXmark25_Feuer_1 | 0 | 0 |

The definition has 68 `SFXmark` rows, IDs 1753..1820, covering rain, thunder,
wind, birds, crickets, water, surf, fire, frogs and animals. These are real object
names. Their `typus` is their definition index; **do not confuse that index with
an object category**. The EXE separately pairs `ODOBJ_SOUNDEVENT` with value 51
at enum-table VA `0x628510`, and displays `Soundevent` for category `0x33`
at `0x4e36d0`. The relationship of that category to SFXmark placement is a
candidate requiring loader/runtime tracing, not a proved mapping from objdef.sex.

Using the existing verified objects.dat layout (16-byte header, 79-byte records,
active byte at +0, definition ID u16 at +75), **both supplied levels have zero
active objects in IDs 1753..1820**. Therefore there is no real placed-SFX template
here to verify initialization of linked action/animation/position/runtime fields.
Names and definition rows alone cannot justify emitting a fabricated marker.

## 4. Real level scripts: voice is not an ambient zone API

Both BCI symbol tables contain index 76 `s_playVoiceSample` and index 77
`s_sendVoiceSampleMP`. Calls in both samples:

| BCI code offset | API | argc encoding |
| --- | --- | --- |
| `0x3594`, `0x36b0` | s_playVoiceSample | opcode 73, -1 (one argument) |
| `0x35c4`, `0x36e0` | s_sendVoiceSampleMP | opcode 73, -2 (two arguments) |

At code `0x3594`, exact little-endian words are:
`80 00 00 00 4c 00 00 00 49 00 00 00 ff ff ff ff 56 00 00 00`
(pushsym 76, argc -1, call). These calls occur in shared helper code; their
presence does not prove the helper runs at startup or emits spatial ambience.
No `s_playSound` symbol was found in either sample or the EXE ASCII strings.
No `s_playVoiceSampleMP` symbol was found; the existing prototype used that
incorrect name in a commented example.

EXE registration at `0x522294` associates `s_playVoiceSample` (VA `0x6002a1`)
with VM wrapper `0x521fb0`. The wrapper converts one VM string through
`0x5b1680`, calls `0x520e40`, then `0x43e860`; it takes no XYZ or radius.
`s_setVoiceSubGroup` (VA `0x6002d2`) registers wrapper `0x522000`, which reads
two arguments and calls `0x520ed0`. That function bounds the first integer to
0..3 and forwards both to `0x43d520`. Thus the prototype's
`s_setVoiceSubGroup("Amb_River_Gentle", volumePercent)` is unsupported.
`SYSTEM/voice.ini` is a real EXE path string at VA `0x5e5a52`, but no copy is
available to establish valid samples/groups. Neither voice API supplies evidence
for a positional emitter or listener-driven zone trigger.

## Confidence, changes and remaining work

**High confidence:** real table names/loaders, object/action selection code,
objdef sound flags and SFXmark names, BCI call arities, absence of sound.dat and
active SFXmark records in these two copies. **Candidate/incomplete:** environmental
category behavior, SFXmark-to-Soundevent category mapping, looping, attenuation,
listener selection, sample IDs, variant loading, object initialization and game
acceptance. No claim is made that Against Rome lacks placed environmental sound;
only that its complete authoring contract is not verified here.

Changes: prevent `SaveToSoundDat` and `ExportToScript` from producing false native
outputs; explicitly label existing drafts/design/IDs as experimental. No new
format, guessed DAU rows, marker writer, BCI compiler or map-save integration.
Tests cover draft JSON/binary round trips and native-export refusal, including
preservation of existing files and refusal before creating DATA directories.
These are editor safety tests, **not real-game compatibility fixtures**.

Next evidence needed: authorized TEMP copies of matching `cl_sfx.ini` and
sfxenv/sfxobj/sfxexp tables (including active variants), plus a map with active
SFXmark objects or a real spatial sound script call. Trace that sample through
native loader, action selection, playback and position handling; derive minimal
fixtures locally without committing game assets, then implement and validate an
export contract. In-game looping, attenuation, audible sample identity, trigger
timing and save/reload remain entirely unverified.

## Validation

Executed inside `D:/Github/ARM_wt_sound`:

- `DOTNET_ROLL_FORWARD=Major dotnet build AgainstRomeModifier.slnx -c Release -p:UseAppHost=false`: exit 0, **0 errors**, 40 analyzer warnings.
- `dotnet test AgainstRomeModifier.slnx -c Release --no-build` with the same roll-forward setting: exit 0; Modules **548 passed**, Host **705 passed / 22 skipped**; **0 failed**.
- Read-only probe, with and without optional disassembly: exit 0 against the input hashes above.
- `git diff --check`: passed. No game assets are included in the commit.
