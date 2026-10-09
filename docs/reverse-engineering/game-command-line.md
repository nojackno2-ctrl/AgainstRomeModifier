# Game configuration and command-line options

Status: static-verified, 2026-10-09, Codex/REA Ghidra 12.1.4.
EXE SHA256: `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`.
The game was not launched; examples below describe static token handling,
not verified invocations. No temporary INI was written to the installation.

## Startup and source precedence

`FUN_00557790` registers callback `0x401010` and the embedded engine defaults.
`FUN_0055c9d0` initializes engine configuration, then `FUN_0055e9b0`
tokenizes the command line into pointers at `0x29e91f8` and count `0x29e924c`.
`FUN_00571640` invokes the registered callback; the 24-byte thunk at
`0x401010` calls `FUN_00413630`. Ghidra default analysis did not create a
procedure for this thunk; its direct CALL bytes independently identify the
destination. `FUN_00413630` calls `FUN_00412750`, which runs:

1. `FUN_00416790`: zero a `0x1a4`-byte game parameter state, then set defaults.
2. `FUN_00415a70("USER/clparam.ini")`: read recognized configuration sections.
3. `FUN_00416d30(argc, argv)`: translate recognized options to INI text,
   write `tmpini.` in mode `wt`, close it, parse it through `FUN_00415a70`,
   and delete it through `FUN_00580720`.

`FUN_00415a70` opens the INI through `FUN_00572f50`/`FUN_00572f70`, which
uses virtual-file open `FUN_005801e0`. Therefore the earlier archive/loose-file
rules apply to `USER/clparam.ini`. Its existence in `cl.pua` does not prove
that PUA supplies the active contents.

Only recognized CLI sections are emitted; options can overwrite fields set by
the prior file parse. Unspecified fields generally retain their earlier state,
but per-case side effects and later normalization require separate analysis.
If the temporary file cannot be opened, it is not parsed and the function
still logs state. `FUN_00573240` searches from the first stored line and
returns the first exact case-insensitive section match; `FUN_005732b0`
returns its following line. Thus repeated recognized sections in one source
use the first occurrence, including repeated CLI options in the temporary INI.

## Token handling

- Outside double quotes, the engine splits at bytes below `0x21`.
  Within double quotes, only NUL or a double quote ends the token. Leading
  quote bytes are retained and skipped by the game option translator.
- The game translator processes each argv entry independently. A long option
  starts with `--`; a short alias starts with a single `-`.
- `FUN_00416cd0` reads the name until whitespace/NUL. `FUN_00416cf0`
  skips whitespace; the rest of that same token becomes the value.
  An empty/whitespace-only value becomes `1`.
- Long names compare without case using `FUN_005675f0(..., 0)`; short aliases
  compare exactly using `FUN_005675f0(..., 1)` → `FUN_005675a0`.
- Accepted entries become `[%s]\n%s\n`. Unknown names or missing prefixes
  are logged and omitted from the temporary INI.

For example, the raw Windows command-line text `"--logictime 5"` stays in
one token and produces `[logictime]` with value `5`. Unquoted
`--logictime 5` produces value `1` for `--logictime`, followed by a separate
invalid token `5`. `--logictime=5` is an unknown name because `=` does not end
the name. These predictions follow the native parsers; shell quoting and
actual gameplay effects were not tested. Do not assume modern CLI conventions.

## Recognized table

40 records at `0x61a444`, stride 12: uint32 parser case, long-name pointer,
short-alias pointer. Table order is also used by the file reader. Names are
native bytes; they do not prove all implied gameplay semantics.

| Long name | Short | Parser case | Long name | Short | Parser case |
|---|---|---:|---|---|---:|
| help | ? | 1 | host | O | 39 |
| asynclogicgfx | y | 10 | hostaddr | h | 40 |
| quickstart | q | 11 | hostport | p | 41 |
| runmode | r | 12 | addr | I | 42 |
| alrdelta | w | 20 | port | P | 43 |
| loadalrcb | 1 | 21 | gamename | g | 44 |
| loadaptcb | 2 | 22 | username | m | 45 |
| logictime | L | 23 | netdbg | d | 46 |
| rndinitonstart | R | 24 | netlevel | n | 47 |
| iprexitopcnum | X | 25 | mpguitest | u | 48 |
| usetimelocal | T | 26 | netstarthost | 7 | 49 |
| norndforposmp | M | 27 | netstartclient | 8 | 50 |
| useanfuehrer | F | 28 | numplr | z | 51 |
| alpoll | A | 29 | maxplr | x | 52 |
| deflevel | l | 30 | dbginputusers | i | 60 |
| dlgactive | a | 31 | dbgoutputusers | o | 61 |
| noscriptcheck | c | 32 | dbgmode | D | 62 |
| startpicdelay | t | 33 | defdetail | 3 | 70 |
| nobackgnd | b | 34 | defzoom | 4 | 71 |
| securemode | s | 35 | defpos | 5 | 72 |

Concrete field evidence from `FUN_00416790`, `FUN_00415a70` and the state logger
`FUN_00416960`: `logictime` parses one signed integer into state offset
`0x188`; `useanfuehrer` into `0x19c`; `deflevel` replaces a string pointer at
`0x08`. Defaults include `logictime = -1`, `useanfuehrer = 0`,
`deflevel = "level1"`, `runmode = 1`, and `noscriptcheck = 1` before file/CLI
overrides. `host` sets offset `0x17c` to 1 when its section exists, regardless
of the numeric text. These stores are confirmed, but full consumers remain
untraced. No recommended values or runtime support are inferred here.

## Reproduce and remaining scope

Run `tools/re/probe_engine_config.py` against the fingerprinted installed EXE,
with a new output path outside its directory, to reproduce section, resource,
and option-table metadata. Local full evidence is retained in ignored
`re_workspace/config-sources-20261009-evidence.json`.
Key pseudocode evidence: `ev_cbfcb63dc1a67d4e1dbb8958477c1ad5a56129a748e771b2ccd0a0be937f35bd`
(CLI translator), `ev_d6d88bb21d0694261925f4d20eccdf12656e2658e9e9e26bc1bd795e8a08b969`
(file reader), and `ev_4691c18527a3478a47d43822845fe326af199060484adc16e0d44f3ead6e2fc1`
(game initialization).

Remaining: later normalization and all final field consumers, live source
attribution, and parser/write failure behavior. The traced defaults and
first-section selection are static observations, not measured gameplay.
