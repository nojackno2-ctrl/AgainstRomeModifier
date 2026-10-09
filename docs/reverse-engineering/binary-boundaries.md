# Installed executable and stream-DLL boundaries

Status: static-verified, 2026-10-09. No installed module was loaded or executed.
Parent analysis: Codex/REA Ghidra 12.1.4, disposable headless Ghidra, and
Capstone instruction checks. Agy was delegated a separate read-only `ar.exe`
investigation; its result must be reviewed separately before accepting claims.

| Carrier | SHA256 | Entry VA | Observed boundary |
|---|---|---|---|
| Against_Rome.exe | `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf` | `0x5cd8c0` | Main game, dynamic stream interface |
| ds_andll.dll | `dfdcdcf53a778cd5eeab5f53332748e77be97b29f480c36a7d1ca8d18cbbb17f` | `0x10002b9a` | 12 exported stream functions |
| ar.exe | `75afdd8ab3831a3eed4fea16984964c41b870e005db61d37c5d2d7cab5f5ccf5` | `0x445a07` | Separate PE32 image; relationship under investigation |

All three inspected images declare x86 machine `0x14c`. DLL image base is
`0x10000000`; both EXEs use `0x400000`. The `ar.exe` section layout includes
`.pgljft` and `.ksrwq`, and 51,977 bytes follow the final raw section extent.
These are observed layout facts, not proof of a loader or protection mechanism.
`UNWISE.EXE` was not analyzed in this pass.

## Main EXE loads ds_andll.dll

`FUN_005c54d0` calls `LoadLibraryA("ds_andll.dll")` when its module slot
`0x2ab6fb4` is zero, temporarily using `SetErrorMode(0x8000)` and restoring
the previous mode. It resolves the following names with `GetProcAddress`:

| Export | DLL VA | Main EXE function-pointer slot |
|---|---|---|
| w32_strmInit | `0x10001260` | `0x2ab6fb8` |
| w32_strmExit | `0x10001890` | `0x2ab6fbc` |
| w32_strmPlay | `0x10001bd0` | `0x2ab6fc0` |
| w32_strmEnd | `0x10002730` | `0x2ab6fc4` |
| w32_strmInfo | `0x100025a0` | `0x2ab6fd0` |
| w32_strmPlaySnd | `0x100028c0` | `0x2ab6fc8` |
| w32_strmEndSnd | `0x10002920` | `0x2ab6fcc` |
| w32_strmSetVol | `0x10002940` | `0x2ab6fd4` |
| w32_strmGetVol | `0x100029b0` | `0x2ab6fd8` |
| w32_strmRunning | `0x100029d0` | `0x2ab6fdc` |
| w32_strmExists | `0x10002a00` | `0x2ab6fe0` |
| w32_strmSetCB | `0x10002a20` | `0x2ab6fe4` |

DLL-load failure returns -1 and logs `No DS video possible`. Missing exports
return -2 through -13 in the resolution order above; the module is freed and
its module slot cleared. Successful resolution returns 0. `FUN_005c58a0`
obtains native window/module values via `FUN_0055d0d0` and calls the resolved
Init with three arguments. `FUN_005c5920` forwards 12 arguments to Play;
`FUN_005c59c0` forwards four to PlaySnd. An absent pointer or zero first
argument causes these wrappers to return 0. Whether this branch is reached
in a live game has not been observed.

## DLL behavior and ABI correction

The DLL imports `CoInitialize`, `CoCreateInstance`, `CoUninitialize`,
Quartz `AMGetErrorTextA`, window APIs and `timeGetTime`. Its Init registers
window class `PuseAnimW32`; the playback helper creates a child window titled
`PuseAnimW32 per DShow`. The Info helper creates a COM graph, renders the
given filename and queries a video interface. Together, imports, code and
native diagnostic strings establish a COM/DirectShow multimedia boundary.
Full COM interface typing and codec behavior remain unverified.

Default Ghidra analysis incorrectly inferred only one argument for Play and
PlaySnd and four for their common helper `0x10001c50`, yielding misleading
constant-zero returns. Direct instructions resolve the discrepancy:

- Play reads 12 incoming arguments, pushes 19 arguments to the helper, and
  supplies a local output-handle pointer as helper argument 17.
- PlaySnd reads four incoming arguments and also pushes 19 helper arguments,
  with constants supplying most window/video parameters.
- At `0x10001c30` / `0x10002907`, each calls `0x10001c50`; helper argument
  cleanup is 76 bytes, within the total `ADD ESP, 0x50` including the local slot.
- Following the call, `NEG EAX; SBB EAX,EAX; NOT EAX; AND EAX,ECX` returns
  the output handle only when helper status equals zero; otherwise it returns 0.

The analysis script pins these three cdecl argument counts (12, 4, 19) in the
disposable database. Corrected pseudocode now agrees with the instructions;
the earlier uncorrected export output must not be used as an ABI reference.
The remaining argument meanings are not guessed from their positions.

Other verified export behavior:

- Init stores instance/window/log callback globals, calls CoInitialize and
  RegisterClassA, and returns -1/-2 on those failures, 0 on success.
- End delegates to a helper with zero, which processes all tracked streams.
  EndSnd finds a stream by integer identifier before delegating.
- SetVol clamps its second argument to `[-10000, 0]`, stores it at stream
  offset `0x128`, and calls an audio-interface method when present.
  GetVol returns that stored value or -1 if the identifier is absent.
- Running is true only when a found stream's field at `0x120` equals 2.
  Exists only tests whether identifier lookup succeeds.
- SetCB stores its second and third arguments in globals `0x10010a44` and
  `0x10010a48`; its first argument does not influence those stores.
  Callback event meanings are not established here.

## Reproduce and limits

`tools/re/probe_pe_interfaces.py` reads PE32 imports/exports/section layout,
with no module loading. Output must be new and outside the source directory.
Local metadata: `re_workspace/ds-interfaces-20261009.json` and
`ar-interfaces-20261009.json`. Full REA evidence for the main EXE is
`stream-main-20261009-evidence.json` (10 records), including loader pseudocode
`ev_9a3a637138267a7c1de176fcaf7b74c7bfdca10c9fa5b3607bc6e5a39359f7f9`.

For DLL decompilation, copy it to the ignored workspace and verify its hash
first: the local Windows headless batch wrapper rejected the spaced installed
path with `\Against was unexpected at this time.`. The hash-identical local
copy imported successfully. Use a fresh project/output name:

```powershell
$env:JAVA_HOME = 'C:\Users\nojac\AppData\Local\Programs\REA\jdk-21.0.12.1+1'
& 'C:\Users\nojac\AppData\Local\Programs\REA\ghidra_12.1.4_PUBLIC\support\analyzeHeadless.bat' `
  $env:TEMP 'ArmStreamProbeNew' -import 'D:\Github\AgainstRomeModifier\re_workspace\ds_andll.dll' `
  -scriptPath 'D:\Github\AgainstRomeModifier\tools\re' `
  -postScript GhidraStreamBoundaryAnalysis.java 'D:\Github\AgainstRomeModifier\re_workspace\stream-new.txt' `
  -deleteProject
```

Optional final argument `helpers` restricts output to seven internal functions.
Corrected export evidence: `stream-dll-abi-20261009.txt`. No source bytes are
changed by the script. Pending: complete argument typing, callback lifecycle,
caller-to-media-path attribution, codec availability and live playback.
