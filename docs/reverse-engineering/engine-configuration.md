# Native engine configuration

Status: static-verified, 2026-10-09, Codex/REA Ghidra 12.1.4.
Target: `Against_Rome.exe`, SHA256
`6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`.
No game process was running during inspection; effective runtime settings remain
unobserved. Installed files were read only.

## Startup sequence

`FUN_00572b70` calls the parser `FUN_00571c80` in this order:

1. Parse registered embedded defaults, with reset enabled, if supplied.
2. Read the filename held at `0x632eb0` directly through `FUN_00563e20`.
   Its initial string is `puse.ini`; the embedded `[ininame]` can change it
   before this read. If no embedded defaults exist, this call enables reset.
3. Parse the command configuration buffer returned by `FUN_0055e760`, if
   nonempty. That helper returns globals `0x29e89ec` and `0x29e89f0`.
   Conversion from the process command line to this buffer remains untraced.
4. If the current filename differs from the literal `puse.ini`, read that
   filename again, with reset disabled. This comparison is not a check that
   the command buffer changed the filename.

The embedded defaults select `clsys.ini`; therefore a second attempt to read
that file can occur even without a command filename override. A file that is
missing simply returns from the parser. Final scalar precedence depends on
which files and sections exist, and is not universally command-buffer-last.
The installed loose-file scan found no `clsys.ini`, `puse.ini`, or `edit.cfg`
outside the excluded SAVE/ToEng trees. `USER/edit.cfg` exists in `cl.pua`, but
that alone does not demonstrate that this direct-file parser reads it.

## Section recognition

`FUN_005719f0` reads lines, removes semicolon comments, changes control bytes
and commas to spaces, lowercases ASCII A-Z through `FUN_005c74b0`, and trims
trailing spaces. Leading whitespace is not explicitly trimmed in this helper.
It tests 36 records at `0x632b98`, stride 22, in order. `FUN_00571900`
compares each byte as `rol8(input, 3) XOR 0x39`; `FUN_005718b0` terminates
encoded text at `0x39`. A match must start at the beginning of the line.
These are prefix matches, not validated INI section names.

| Index | Exact decoded prefix | Index | Exact decoded prefix |
|---:|---|---:|---|
| 0 | `[dos_resolution]` | 18 | `[testmode]` |
| 1 | `[dbgdefault]` | 19 | `[pathprefix]` |
| 2 | `[winfullscreen]` | 20 | `[use3dhw]]` |
| 3 | `[language]` | 21 | `[usedinput]` |
| 4 | `[joystick]` | 22 | `[nohiperftimer]` |
| 5 | `[dos_sound]` | 23 | `[usemthr]` |
| 6 | `[archive]` | 24 | `[dbgwinpos]` |
| 7 | `[network]` | 25 | `[inputmode]` |
| 8 | `[winchars]` | 26 | `[allowscrsaver]` |
| 9 | `[nocdplay]` | 27 | `[noglobalexit]` |
| 10 | `[sysdebug]` | 28 | `[allowboot]` |
| 11 | `[ininame]` | 29 | `[disablesyskeys]` |
| 12 | `[usemmx]` | 30 | `[numinstances]` |
| 13 | `[noproctest]` | 31 | `[usek7]` |
| 14 | `[nodirectx]` | 32 | `[usesyskeys]` |
| 15 | `[mindxversion]` | 33 | `[nocrashdump]` |
| 16 | `[backgroundrun]` | 34 | `[specialdirs` |
| 17 | `[transparency]` | 35 | `[` |

The doubled closing bracket at index 20 and missing closing bracket at index
34 are source bytes, not transcription corrections. Index 35 catches other
opening-bracket lines. Names alone do not establish the effects of all options.

## Archive and directory lists

Encountering section 6 frees and clears both existing archive and directory
lists, including when processing another source. Archive lines use `%s%s`
to read a filename and optional mode; at most 32 archive entries are accepted.
A mode starting with `open` sets flag 1; other modes leave flag 0.
See [virtual-file-system.md](virtual-file-system.md) for mount and lookup rules.

Section 34 reads directory and optional mode. A mode beginning `noacc` sets
policy 2; `rdonly` sets policy 1; otherwise policy 0. Those policies govern
the loose-file gate described in the linked file-system note. Section 19 sets
a path prefix and adds a trailing separator when needed; section 11 sets the
configuration filename.

**Static boundary-check finding:** directory insertion at `0x57297c` checks
`[EBX + 0x50] < 32` (archive count), but the pointer store at `0x5729ce`
uses `[EBX + 0x158]` (directory count). Here EBX is `0x29fd448`.
The directory pointer array starts at `0x29fd5a4`; the policy byte array starts
at `0x29fd624`, leaving 32 pointer slots. The insertion branch does not check
directory count against that capacity. This instruction-level mismatch is
confirmed; out-of-range runtime consequences remain a candidate. No oversized
configuration was executed and no original game file was patched.

## Reproduce and coverage

```powershell
python tools/re/probe_engine_config.py `
  'C:\Program Files (x86)\Against Rome\Against_Rome.exe' `
  re_workspace/engine-config-new.json
```

The stdlib probe requires the exact fingerprint, reads file-backed PE bytes,
checks all 36 encode/decode roundtrips and recognizes the 10 embedded-default
headers. It exports metadata only, refuses existing output and output inside
the input directory, and does not execute the game or emulate the parser.
REA evidence stays local in ignored `re_workspace/config-final-20261009-evidence.json`.

Remaining: command-buffer construction, each option's consumer, game-specific
`USER/clparam.ini` parsing, current process configuration and actual file-source
attribution. Packaged configuration existence does not establish active use.
