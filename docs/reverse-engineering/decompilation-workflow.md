# Decompilation Workflow

> Reviewed 2026-07-11. Generated decompiler output is local evidence; reproducible scripts and these notes are the publishable record.

This project keeps reverse-engineering output reproducible instead of treating
one conversation or one decompiler view as the source of truth.

## Repository And Local Toolchain

- Authoritative repository: `C:\離線儲存\程式設計\Against_Rome_Modifier`
- Ghidra: `C:\Users\nojac\AppData\Local\Temp\AgainstRome_RE\ghidra-12.1.2-clean\ghidra_12.1.2_PUBLIC`
- JDK: `C:\Users\nojac\AppData\Local\Temp\AgainstRome_RE\jdk21-full`
- Preferred project mode: one-shot headless import under `%TEMP%`, followed by
  `-deleteProject`; stale persistent project markers are not authoritative.
- Imported program: repository-local `re_workspace\Against_Rome.exe`

The Ghidra, JDK, and project paths are machine-local observations, not stable
repository contracts. Verify them with `Test-Path` before use. Older OneDrive
repository paths are historical only and must never receive new output.

Repair status (2026-07-17): the former `AgainstRome_RE\ghidra` tree was an
incomplete/mixed 12.1.2 installation. It is retained for forensic comparison
but must not be used. A clean official 12.1.2 distribution was downloaded from
the NSA GitHub release, verified against SHA-256
`b62e81a0390618466c019c60d8c2f796ced2509c4c1aea4a37644a77272cf99d`, and
extracted to the path above. Fresh one-shot imports now complete successfully.

`java`, `ghidraRun`, and `analyzeHeadless` do not need to be on PATH. Set
`JAVA_HOME` to the local JDK and invoke `analyzeHeadless.bat` directly.

## Full EXE Inventory

```powershell
$repo = 'C:\離線儲存\程式設計\Against_Rome_Modifier'
$root = 'C:\Users\nojac\AppData\Local\Temp\AgainstRome_RE'
$ghidra = Join-Path $root 'ghidra-12.1.2-clean\ghidra_12.1.2_PUBLIC'
$out = Join-Path $repo 're_workspace\ghidra_inventory'
$env:JAVA_HOME = Join-Path $root 'jdk21-full'
$env:PATH = (Join-Path $env:JAVA_HOME 'bin') + ';' + $env:PATH

& (Join-Path $ghidra 'support\analyzeHeadless.bat') `
  $env:TEMP 'AgainstRomeInventory' `
  -import (Join-Path $repo 're_workspace\Against_Rome.exe') `
  -scriptPath (Join-Path $repo 'tools\re') `
  -postScript 'GhidraFunctionInventory.java' $out decompile `
  -deleteProject
```

Repair verification used the same one-shot form and completed three focused
scripts with exit code 0: `GhidraUnitRangeSpeedAnalysis.java` resolved the
`moves`, `w1_rad1`, and `w1_rad2` consumers; `GhidraScriptIniAnalysis.java`
resolved the `[TribeData]`, `[Spells]`, and `[SpecialAbilities]` parser at
`0x41BCE0`; and `GhidraRessAnalysis.java` resolved `SYSTEM/ress.ini`,
`[objres]`, `[volkres]`, and their stored cost groups. Both runs reported
`Analysis succeeded`, `Post-analysis succeeded`, and `Import succeeded`.

Current generated artifacts:

- `re_workspace/ghidra_inventory/against_rome_function_index.csv`
  - approximately 7381 functions with names, sizes, parameter counts,
    references, first callers, and unknown-name status.
- `re_workspace/ghidra_inventory/against_rome_decompiled_functions.c`
  - generated Ghidra pseudocode for the same inventory.

Both files are local evidence under ignored `re_workspace/`. Do not publish
original game files or generated full-program pseudocode.

## Focused Scripts

Reusable scripts are kept under `tools/re/`, including analyses for:

- `objdef.dau` and `ress.ini` consumers;
- script INI and UI limits;
- endless-mode jobs, active limits, and runtime job semantics;
- village bounds, setter flow, and red-frame candidates;
- function inventory generation.

Prefer the smallest script that answers the current question. Do not regenerate
the entire inventory when a known function address and focused xrefs suffice.

## Evidence Rules

- Decompiler pseudocode is a starting point, not original source.
- A field is confirmed only when the game consumption path or runtime behavior
  establishes its meaning.
- Keep Ghidra-generated names as `FUN_*` until evidence supports a semantic
  alias.
- Separate file offsets from virtual addresses.
- Record callers, callees, argument order, original bytes, and runtime result.
- A writable patch also needs signatures, rollback, restore, migration, and
  Unknown-state refusal.
- Runtime regressions override plausible static reasoning.

## Reuse Order

1. `TechDoc.md` (includes integrated handoff)
2. `docs/reverse-engineering/`
3. `data/game_schema.json`
4. Local function index
5. Local pseudocode inventory
6. Focused Ghidra script
7. Controlled runtime test on a backed-up installation
