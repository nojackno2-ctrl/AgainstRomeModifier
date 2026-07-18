# Runtime Trace Hooks (`native/argm-trace`)

> Status 2026-07-18: implementation complete and **compiled with MSVC**
> (Win32 `version.dll`, correct undecorated export table). Every hook target's
> prologue and argument convention has been byte-verified against the installed
> `Against_Rome.exe` (PE `TimeDateStamp = 0x404D1710`, sections parsed from the
> real binary), and all hooks carry byte signatures dumped from that binary.
> The host tests (instruction-length decoder + full inline-hook mechanism
> against genuine Win32 APIs) build and pass via CTest on the same toolchain.
> The DLL has **not** yet been run against the live game — dropping
> `version.dll` into the game folder and playing endless mode is the remaining
> user step. The tool refuses to install address-based hooks unless the running
> build's PE `TimeDateStamp` is confirmed via `argm_trace.ini`.

## Purpose

《Against Rome》 ships no logging. `native/argm-trace` is a 32-bit `version.dll`
proxy that loads into the game process and installs **log-only** inline hooks on
the reverse-engineered engine functions, producing a time-stamped
`argm_trace.log` of what the computer AI actually does at runtime. It is the
"flight recorder" side of the debugging story that complements the offline
save/BCI editing the rest of the repo does.

## Hooked functions

All addresses are absolute VAs under the stock image base `0x00400000` and are
rebased at load time for ASLR. Sources are `exe-functions.md` and
`endless-mode-ai.md`.

| Category | VA | Function | What it records |
|---|---|---|---|
| `ai.spawn` | `0x00547F50` | `s_addNPCJob_createUnit` impl | AI reinforcement job: team `0..7`, mode (`0/1/3`), the `0..9` selector, and the clamped unit-count range (args 6/7). The single most useful "what did the AI do" event. |
| `ai.active` | `0x00548CE0` | `s_setNPCActive` impl (`FUN_00548ce0`) | A defeated AI team marked eligible to respawn (`DAT_029e6000[team]`). |
| `ai.village` | `0x00549500` | `s_setVillageTemplate` impl | Settlement-style AI arrival. |
| `ai.unit` | `0x0052A020` | `s_createUnitAndMems` callback | Unit-and-members creation. |
| `ai.query` | `0x00548D20` | `s_NPCActive` getter (`FUN_00548d20`) | Snapshots **all eight** teams' respawn-eligibility flags (`DAT_029e6000[0..7]`) every time the AI queries one. The getter has no side effects, so at hook entry the array already holds the value it returns. The single most direct "why isn't team N reinforcing" datum — the party state machine gates type-4 reinforcement on this flag. |
| `ai.level` | `0x0054A070` | level-init sweep | Zeroes the per-team NPC arrays once per level load. Logged as a **NEW SESSION BOUNDARY** so one run's events are separable from the next inside a single log file (endless behavior reports must come from a fresh session). |
| `ai.unitmax` | `0x005249D0` / `0x00524D70` | `s_createBattleUnitsMax` / `s_createCiviUnitsMax` impls | Requested unit count before the `<=20` clamp, so "AI wanted N but got 20" or "requested 0" is visible. |
| `game.faction` | `0x0045BD60` | Endless Roman faction selector setter | The faction chosen in `dlg_volk`. Two 21-byte signatures are attempted: the stock prologue and the force-Roman-patched prologue (`53 6A 03 5B 90`, see known-patches.md), so tracing works on modified installs too. Signature-verified, so safe even on an unrecognized build. |
| `bci.op` | `0x005B1C60` | BCI VM dispatcher entry | Full opcode stream. VERY high volume; off by default. **Byte-verified correction:** the true function entry is `0x005B1C60` (`53 56 57 55 89 E5 ...`), and the VM context is stack **argument 1** — `mov ebx,[ebp+0x14]` only happens at `0x005B1C6F`, so at entry `EBX` still holds the caller's value. The hook reads the context from the caller stack. PC at ctx`+0x08`, code length at ctx`+0x28`, code base at ctx`+0x2C`. |

The `ai.spawn` argument meanings map directly onto the party state machine in
`endless-mode-ai.md`: cross-referencing the spawn log with the documented party
slots (`v47`/`v48`/`v49`/`v61`/`v63`) is how a "reinforcements stopped" or
"team slots exhausted" problem gets localized.

## Safety model

1. **Build fingerprint gate** — `CheckBuildFingerprint` logs the module's PE
   fingerprint (ImageBase, `TimeDateStamp`, `SizeOfImage`, entry bytes). Every
   address-based hook stays disabled until `[build] expectedTimeDateStamp` in
   `argm_trace.ini` matches the running build. Only the signature-verified
   faction hook is exempt.
2. **Signature verification** — targets with known original bytes are compared
   before patching; a mismatch skips that hook.
3. **Prologue relocatability check** — `argm::DecodeLength` (header-only,
   `src/lde.h`) sizes the stolen prologue; if it contains a relative branch or
   an instruction the decoder cannot classify, the hook is skipped. The decoder
   is unit-tested on the host (`tests/lde_test.cpp`, 46 cases including the
   faction prologue).
4. **Register/flag-preserving stub** — the generated detour saves and restores
   all registers and flags and reads arguments from the caller stack without
   disturbing it, so it is safe for cdecl and stdcall targets without knowing
   their exact signature. Formatter faults are swallowed by SEH.

## Build & capture workflow

See `native/argm-trace/README.md`. Summary:

1. `cmake -S native/argm-trace -B build/argm-trace -A Win32 && cmake --build build/argm-trace --config Release` → `version.dll`.
2. Drop `version.dll` beside `Against_Rome.exe` (co-exists with dgVoodoo2's `DDraw.dll`/`D3D8.dll`).
3. Run once, read the `[build]` banner in `argm_trace.log`, copy `TimeDateStamp`
   into `argm_trace.ini` `expectedTimeDateStamp` to unlock the AI-event hooks.
4. Reproduce the endless-mode problem, then analyze `argm_trace.log`.

## Byte-verified conventions (2026-07-18)

Resolved by dumping the hook targets straight out of the installed
`Against_Rome.exe` via its PE section table (`AUTO` VA `0x1000` → raw `0x1000`,
so raw offset = VA − 0x400000 for code):

- **`0x00547F50`** prologue `53 56 57 55 8B 5C 24 14 8B 7C 24 30 8B 6C 24 34`:
  pushes 4 registers, then reads arg1 (team, validated `0..7`) from
  `[esp+0x14]`, arg8/arg9 from `[esp+0x30]/[esp+0x34]`, arg3 from `[esp+0x1C]`
  (validated `0..9`). Args are plain dwords at `[esp+4+4·(n−1)]` at function
  entry — the hook's `cs[1..9]` column labels are correct.
- **`0x00549500`** prologue `53 56 57 55 8B 5C 24 14 8B 6C 24 18 31 FF 85 ED`:
  arg1 → `EBX` is the team (validated `0..7`), arg2 → `EBP` is a pointer
  checked non-null (template name). Early-out returns `-1` with a plain `C3`
  ret, i.e. caller-cleaned stack — the log-only stub is safe.
- **`0x005B1C60`** is the true dispatcher entry (the previously documented
  `0x005B1C62` is two one-byte pushes in). VM context = stack argument 1;
  `EBX` is only loaded from it at `0x005B1C6F`. Dispatcher reads PC
  `[ctx+0x08]`, code length `[ctx+0x28]`, code base `[ctx+0x2C]`, opcode
  `[base+pc]` — matching `bci0-opcodes.md`.
- **`0x00548CE0`** starts `8B 54 24 04 85 D2` (no saved registers; arg1 team at
  `[esp+4]`, arg2 flag at `[esp+8]`, writes `[edx+0x29E6000]`) — confirms the
  documented `DAT_029e6000` semantics and gives a 6-byte relocatable prologue.
- **`0x0045BD60`** on this install is already force-Roman-patched
  (`53 6A 03 5B 90 ...`), which is why the trace tool now ships both signature
  variants.
- **`0x00548D20`** (`s_NPCActive` getter) prologue
  `8B 44 24 04 85 C0 7C 14 83 F8 08 7D 0F 80 B8 00 60 9E 02`: arg1 team at
  `[esp+4]`, reads `byte [team+0x029E6000]`, returns 0/1. Confirms the
  `DAT_029e6000` array base and that the flag can be snapshotted read-only at
  entry. 6-byte relocatable prologue.
- **`0x0054A070`** (level-init sweep) prologue
  `53 56 57 55 83 EC 24 BB 38 91 9C 02 ...`: `mov ebx, 0x029C9138` is the first
  of several per-team arrays it zeroes; no arguments needed for the boundary
  marker. 7-byte relocatable prologue.
- **`0x005249D0`** / **`0x00524D70`** (`s_createBattleUnitsMax` /
  `s_createCiviUnitsMax` impls) prologues `53 56 57 55 83 EC 0C 8B 54 24 24` and
  `53 56 57 55 83 EC 08 8B 6C 24 24`: 4 pushes then a `sub esp` then argument
  loads; the count argument is read at the documented `<=20` clamp. Args are
  read from the caller stack at entry (`cs[1..3]`). 7-byte relocatable
  prologues.
- These four `ai.query`/`ai.level`/`ai.unitmax` targets are the endless-AI
  decision-context deepening (2026-07-18): the action hooks record what the AI
  *did*, and these record the *inputs and boundaries* so "why didn't it act"
  is answerable from the log alone.
- All six targets' prologues decode to ≥5 relocatable bytes with no relative
  branches (covered by `tests/lde_test.cpp` prologue cases).
- Installed build fingerprint: `TimeDateStamp = 0x404D1710` (also noted in
  `argm_trace.ini.sample`).

## Open items before claiming it works

- ~~Compile with MSVC (Win32).~~ Done 2026-07-18 with VS 18 Community's C++
  workload: `cmake -A Win32 && cmake --build --config Release` produces
  `version.dll` (20,992 bytes, machine 14C/x86, undecorated exports matching
  the genuine version.dll). Host tests pass via `-DARGM_BUILD_TESTS=ON` + CTest.
- **Live-game smoke run (remaining):** drop the built `version.dll` next to
  `Against_Rome.exe`, launch once, read the `[build]` banner in
  `argm_trace.log`, set `expectedTimeDateStamp=404D1710`, then reproduce an
  endless-mode session and confirm `ai.spawn` / `ai.village` / `ai.active`
  events appear and the game stays stable. This writes into the game install
  directory, so it is performed by the user, not the tooling.
