# Native file lookup and archive backends

Status: static-verified call paths, 2026-10-09 (Codex).
Binary SHA-256: `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`,
matching the installed `Against_Rome.exe`. No runtime probe or game-file write.

## General open path

`FUN_005801e0(name, mode)` is the general file-open path. Its native flow is:

1. Normalize path separators via `FUN_00572cf0`; optionally apply a registered
   path callback at `DAT_02a223e4`. Build the disk path using `DAT_029fd644`.
2. Mode characters `r+`, `w`, or `a` select write-capable behavior. The `p`
   modifier controls the wrapper branch; this is separate from archive lookup.
3. For ordinary reads, consult `FUN_0057fa30(diskPath)`. If allowed, attempt
   disk open at `FUN_005c673c` before searching archives.
4. If disk open fails or is skipped, and the mode is read-only, visit mounted
   archive slots in ascending order, invoking each backend's member-open
   callback. Stop at the first nonzero result.
5. Write-capable opens do not fall back to archive member lookup.

REA evidence: `ev_61c5fc7f95f042626f5d9963105d828d4230c5ba97ebf5c4207cf26309d34b16`.
The `p` modifier and mode literals were independently read at `0x608110`.

Thus loose-first is conditional, not an unconditional rule. The directory
policy gate at `0x57fa30` searches configured directory entries:

| Matching directory policy byte | Observed read behavior |
| --- | --- |
| 2 | Skip disk attempt |
| 1 | Permit disk attempt only if name appears in the startup directory cache |
| other / no match | Permit disk attempt |

REA evidence: `ev_8cc5ff8bb6458748fdebc35a08aac0b06cdc846d413efb4bb71998d76f60e508`.
The current runtime directory-policy table, callback, working root and active
configuration have not been observed. This static analysis therefore does not
prove which source a particular running game loaded.

## Mount order and backend evidence

`FUN_0057fb40` registers backend tables, then walks configured archive names
from index zero upward. For each name it tries registered backends in ascending
order and appends the first successful mount to `DAT_02a221e4/1e8`.
Backend registration at `0x57f950` has a four-table bound. This is distinct from
the other file-backend registration at `0x57f9a0`.

The embedded default configuration at `0x615e40` contains this archive order:
`output.dat`, `gui.dat`, `cl.pua`, `floortex.dat`, `alr.dat`, `apt.dat`,
`shad.dat`, `mp.dat`, `sfx.dat`, `voice.dat`.
The inspected installation has no `output.dat`; the default text is not proof
of the active configuration or a successful mount of every listed name.

| Backend | Registration / table | Observed implementation |
| --- | --- | --- |
| PUA | 0x582940 / 0x63300c | Calls ARCP loader 0x5645f0 via entry 0x582750 |
| ZIP | 0x552d20 / 0x62ca8c | Mount 0x552820, member open 0x552a20, read 0x552b90 |

The ZIP mount enumerates the central directory and builds a sorted name index.
Member lookup performs a binary search, selects the ZIP entry and opens its
decompression stream. The path comparator at `0x565180` folds bytes through a
lookup table and equates slash/backslash; no locale-wide Unicode case-folding
claim is made. `0x572cf0` changes slash/backslash to backslash for general paths.

Default REA analysis did not recover several indirect ZIP targets. A focused
Ghidra script creates those candidate procedures in a disposable database.
The initial attempted PUA adapter address `0x582740` had no procedure; the
actual table points to `0x582750`, and the corrected final run succeeds.
Evidence: ignored `re_workspace/virtual-file-final-20261009.txt` and
`virtual-file-final-20261009-evidence.json` (45 REA records).

## Installed overlap evidence

- ZIP versus loose: only two matching paths, both in `shad.dat`:
  `SYSTEM/DATA/SHADOWTEXTURE/KarNadA_shadow.bmp` and
  `LaBrLaubbaum_shadow.bmp`. Both loose files equal their ZIP payloads, so
  their contents cannot distinguish loaded source.
- PUA versus loose: 24 matching paths, including `SYSTEM/cl_detai.ini` and
  `SYSTEM/TEXT/US/*.put`. All 24 loose payloads differ from their decoded PUA
  counterparts (PFIL decompressed before comparison). These are better passive
  runtime attribution candidates; no interpretation of the differences as a
  translation or intended override is asserted here.

## Reproduce and remaining scope

```powershell
$env:JAVA_HOME = 'C:\Users\nojac\AppData\Local\Programs\REA\jdk-21.0.12.1+1'
& 'C:\Users\nojac\AppData\Local\Programs\REA\ghidra_12.1.4_PUBLIC\support\analyzeHeadless.bat' `
  $env:TEMP 'ArmVirtualFileProbe' `
  -import 'D:\Github\AgainstRomeModifier\re_workspace\Against_Rome.exe' `
  -scriptPath 'D:\Github\AgainstRomeModifier\tools\re' `
  -postScript GhidraVirtualFileAnalysis.java 'D:\Github\AgainstRomeModifier\re_workspace\virtual-file-new.txt' `
  -deleteProject
```

Use fresh project/output names; verify the machine-local toolchain paths.
The final run completed with Analysis/Post-analysis/Import succeeded and
exit 0. Generated pseudocode stays local. Pending work: configuration parsing
and overrides, full locale/path callbacks, PFIL wrapper and write-mode behavior,
passive runtime source attribution, and map-loader failure diagnosis.
