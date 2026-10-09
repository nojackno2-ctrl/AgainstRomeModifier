# Cinematic camera scripting: evidence and limits

2026-10-08, Codex, Task B; isolated branch `wt/cinematic`, base `080ebc9`.

## Finding

**Camera control exists in the executable's script-native API.** Registration
and VM marshalling establish `s_lgcSetEnginePos v(ddd)` and
`s_lgcSetEngineZoom v(d)`. This is high-confidence static evidence, not an
in-game playback result. The supplied ENDL level scripts do **not** call them;
no real camera-call bytecode fixture is available in the authorized copies.
Do not conclude that scripting has no camera mechanism merely from that absence.

`CinematicBciCompiler.CompileCameraCalls` now emits verified, balanced native
statement CODE fragments using explicit engine parameters. It registers names
in the supplied `BciImage`; it does not append CODE, change main, inject a
scheduler, produce a standalone script, or write any file. Complete cinematic
sequence playback remains **experimental / unwired**. Planner interpolation and
catalog JSON are editor data, not evidence of native cinematic functionality.

## Inputs and scope

Read-only inputs (no installed game directory access or game launch):

| Input | SHA-256 of file as supplied |
|---|---|
| Repo `re_workspace/Against_Rome.exe`, 2,486,272 bytes | `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf` |
| `%TEMP%/ArmGameCompare_20261007/ENDL_000/SCRIPT/ak_level.bci` | `336e3a5cb808e7f2ef6aaa77ac2ef54a0fb00de04cb00d1abb4fbd5b937d5f27` |
| `%TEMP%/ArmGameCompare_20261007/ENDL_005/SCRIPT/ak_level.bci` | `faaba1e6653327c04610ac0ab4a728d1faadde4cf750eb9598f33e0504fd769e` |

The TEMP SYSTEM tree contains six configuration/data files and no `.bci`.
The native-assets copy supplies graphics/data, not additional level scripts.
The available script corpus is therefore exactly two ENDL scripts, not campaigns
or tutorials. `tools/bcitool.py` decompresses PFIL and decodes these samples.
Both contain `s_showTextBox` (#218), `s_forceShowTextBox` (#219), and
`s_lgcGetTeamCpuCtrl` (#106); neither contains camera/fade/cutscene symbols or
opcode 67. CODE lengths are 115244 and 115552 bytes, with 248 and 252 constants.

## Native registration and effects

All addresses below are **virtual addresses** in the hashed EXE, not BCI offsets.
`tools/re/cinematic_probe.py` scans registration calls to `0x5b1410`, including
optional `or ebx,eax` between pushes and call. It finds 719 registrations;
this pattern scan is not a proof of an exhaustive native inventory.

| Name | Verified signature | Wrapper | Registration pushes start |
|---|---|---|---|
| `s_lgcSetEnginePos` | `v(ddd)` | `0x54c400` | `0x54c8d1` |
| `s_lgcSetEngineZoom` | `v(d)` | `0x54c620` | `0x54c98e` |
| `s_lgcGetEngineZoom` | `d(v)` | `0x54c640` | `0x54c9a7` |
| `s_lgcSetFadeArea` | `v(iiii)` | `0x54c860` | `0x54cb21` |
| `s_lgcFade` | `v(iii)` | `0x54c890` | `0x54cb3c` |
| `s_showTextBox` | `i(ii)` | `0x521f10` | `0x52221e` |
| `s_conMoveTo` | `i(iiiiiiiii)` | `0x5345c0` | `0x534df1` |
| `s_conWaitTime` | `i(iiiiii)` | `0x534cb0` | `0x534fba` |

Position registration bytes:
`6a0068f32e60006800c4540068fa2e6000e8294b0600`.
They push zero, signature address `0x602ef3`, wrapper `0x54c400`, name address
`0x602efa`, then call the registrar. Signature/name strings are
`v(ddd)\0s_lgcSetEnginePos\0`.

Zoom registration bytes:
`6a0068a72f60006820c6540068ac2f6000e86c4a0600`.
Signature/name strings at `0x602fa7`/`0x602fac` are
`v(d)\0s_lgcSetEngineZoom\0`.

Position wrapper forwards three doubles to `0x54c010`; that function narrows
them to floats and calls `0x4989d0`, which stores them in argument order at
`0x7717e8`, `0x7717ec`, `0x7717f0`. Engine argument order is established;
the geometric mapping to editor focus coordinates is not established here.
Zoom wrapper forwards a double to `0x54c1f0`, narrows to float, and calls
`0x498a30`. The latter clamps negative input to zero and input above 9 to 9
(`0x41100000`), stores `0x771800`, and updates projection paths
`0x491d80`/`0x491bd0`. The planner's preview distance **82 is not native zoom**.
No conversion from that distance, Pitch or Yaw is established.

Fade's three integer arguments are verified; direction, color and duration
interpretation remain undetermined, so no fade or letterbox writer was added.
Exact names `s_setCamera` and `s_disableGUI` are absent as NUL-terminated EXE
strings. This does not rule out differently named APIs. There is no evidence
for the old preview's input-lock call or a named cutscene-trigger API.
`s_conWaitTime` is a six-argument controller command, not a one-argument script
sleep. Unit movement takes nine arguments, not the former preview's three.

## Exact native statement encoding

Use [the corrected VM ABI](scenario-events.md), not historical `pushsym/call`
labels still printed by bcitool. All code words are little-endian int32:

| Purpose | CODE words | Evidence |
|---|---|---|
| Double immediate | `67, low32(IEEE754), high32(IEEE754)` | handler `0x5b5b89`; reads 8 operand bytes, adds two stack cells, stores low then high |
| Integer immediate | `66, value` | real ENDL message call |
| String reference | `76, constantIndex` | real ENDL message call; string resolved by `0x5b1680` |
| Invoke native | `128, nativeNameConstantIndex` | handler `0x5b8aeb`, executes via `0x5b1700` at `0x5b8c05` |
| Discard arguments | `73, -wordCount` | handler `0x5b6590`; negative branch `0x5b669f` |
| Read integer return | `86` | corrected event compiler ABI; omitted for statements |

`0x5b1700` walks the signature from argument 1, reading the topmost argument
first. An `i` consumes one cell; a `d` copies the topmost **pair**, low then
high, and consumes two cells (`0x5b179b..0x5b17c1`). Push arguments in reverse
order, but keep the two words within each double in little-endian order.

Position statement:
`67 Zlo Zhi; 67 Ylo Yhi; 67 Xlo Xhi; 128 <position-name-index>; 73 -6`.
Zoom statement:
`67 Zoomlo Zoomhi; 128 <zoom-name-index>; 73 -2`.
These exact camera encodings derive from the EXE handlers and signatures;
**they were not observed in the two TEMP maps**. Native names are resolved
through each image's constants, never treated as fixed script indices or
embedded EXE addresses. Neither void call emits opcode 86.

Both original maps have this message statement at CODE offset `0x1b9f4`
(container offset adds `0x24`):
`76 240; 66 0; 128 218; 73 -2`.
At `0x1ba14`, opcode 86 reads the message handle used by the following
`128 219; 73 -1; 86; 71` (`s_forceShowTextBox`). Our message fragment matches
the first 32 bytes after relocating the string constant; it intentionally
omits forced display, handle use, subtitle duration and voice playback.

## Implementation boundary and verification

- `CompileCameraCalls`: explicit engine coordinates and native zoom 0..9 only;
  rejects NaN/infinity, float-overflow coordinates and preview distance 82.
- `CompileMessageCall`: real string-reference statement, strict game encoding,
  per-image native symbol resolution. No assets or bytecode containers bundled.
- `CompileToScenarioEvents`: subtitles become Message events with absolute
  startup delays, matching `ScenarioEventCompiler`. No invented opening or
  completion messages pretending to perform camera motion or trigger events.
- `GenerateBciScriptText`: comments-only planning preview, not executable IPR.
  Planner/catalog and full sequence remain experimental/unwired. No changes to
  host save integration or shared event compiler.
- Tests check double words/order/cleanup, relocated native indices, refusal
  before image mutation, preserved CODE/main, absolute subtitle timing and
  absence of invented executable preview calls. Optional evidence tests read
  both hashed TEMP samples and the hashed repo EXE. Camera-call checks use EXE
  evidence because no camera-call TEMP sample exists.

Reproduce inside the dedicated worktree (Python dependencies are local tools):

```powershell
python -m pip install --target artifacts/cinematic-python capstone pefile
$env:PYTHONPATH = "$PWD/artifacts/cinematic-python"
python tools/re/cinematic_probe.py --exe ../AgainstRomeModifier/re_workspace/Against_Rome.exe --samples "$env:TEMP/ArmGameCompare_20261007"
$env:DOTNET_ROLL_FORWARD = 'Major'
$env:ARM_CINEMATIC_SAMPLES = "$env:TEMP/ArmGameCompare_20261007"
$env:ARM_CINEMATIC_EXE = 'D:/Github/AgainstRomeModifier/re_workspace/Against_Rome.exe'
# Leave game-path / live acceptance switches unset; never point them at an install.
dotnet build AgainstRomeModifier.slnx -c Release -p:UseAppHost=false
dotnet test AgainstRomeModifier.slnx -c Release --no-build
```

Validation completed: the specified Release build succeeded with **0 errors**
(39 analyzer/compiler warnings). The specified full solution test succeeded:
Modules **556 passed**, Host **705 passed / 22 skipped**, **0 failures**.
Both cinematic TEMP/EXE evidence tests ran (not skipped). `git diff --check`
passed. Local logs stay under ignored `artifacts/`; they are not game assets.

In-game playback, scheduler units and integration, camera restore, multiplayer,
save/load behavior, exact world-axis/focus mapping, preview zoom conversion,
Pitch/Yaw, fades/letterbox and player-control behavior remain unverified.
