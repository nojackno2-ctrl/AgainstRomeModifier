# Decompilation Workflow

This project keeps reverse-engineering output reproducible instead of treating
one conversation or one decompiler view as the source of truth.

## Repository And Local Toolchain

- Authoritative repository: `C:\離線儲存\程式設計\Against_Rome_Modifier`
- Ghidra: `C:\Users\nojac\AppData\Local\Temp\AgainstRome_RE\ghidra`
- JDK: `C:\Users\nojac\AppData\Local\Temp\AgainstRome_RE\jdk21`
- Ghidra project: `C:\Users\nojac\AppData\Local\Temp\AgainstRome_RE\AgainstRomeVillageBuildArea`
- Imported program: `Against_Rome.exe`

The Ghidra, JDK, and project paths are machine-local observations, not stable
repository contracts. Verify them with `Test-Path` before use. Older OneDrive
repository paths are historical only and must never receive new output.

`java`, `ghidraRun`, and `analyzeHeadless` do not need to be on PATH. Set
`JAVA_HOME` to the local JDK and invoke `analyzeHeadless.bat` directly.

## Full EXE Inventory

```powershell
$repo = 'C:\離線儲存\程式設計\Against_Rome_Modifier'
$root = 'C:\Users\nojac\AppData\Local\Temp\AgainstRome_RE'
$out = Join-Path $repo 're_workspace\ghidra_inventory'
$env:JAVA_HOME = Join-Path $root 'jdk21'
$env:PATH = (Join-Path $env:JAVA_HOME 'bin') + ';' + $env:PATH

& (Join-Path $root 'ghidra\support\analyzeHeadless.bat') `
  $root 'AgainstRomeVillageBuildArea' `
  -process 'Against_Rome.exe' `
  -scriptPath (Join-Path $repo 'tools\re') `
  -postScript 'GhidraFunctionInventory.java' $out decompile
```

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

1. `docs/AI_AGENT_HANDOFF.md`
2. `docs/reverse-engineering/`
3. `data/game_schema.json`
4. Local function index
5. Local pseudocode inventory
6. Focused Ghidra script
7. Controlled runtime test on a backed-up installation
