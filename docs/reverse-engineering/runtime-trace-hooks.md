# Runtime Trace Hooks (`native/argm-trace`)

> Status 2026-07-17: implementation complete and the instruction-length decoder
> is host-unit-tested, but the DLL has **not** been compiled with MSVC nor run
> against the live game. Addresses below are version-specific evidence from the
> analyzed `Against_Rome.exe`; the tool refuses to install address-based hooks
> unless the running build's PE `TimeDateStamp` is confirmed via
> `argm_trace.ini`.

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
| `game.faction` | `0x0045BD60` | Endless Roman faction selector setter | The faction chosen in `dlg_volk`. Installed with the full verified 21-byte signature, so it is safe even on an unrecognized build. |
| `bci.op` | `0x005B1C62` | BCI VM dispatcher entry | Full opcode stream (`EBX` = VM context, PC at `+0x08`, code base at `+0x2C`). VERY high volume; off by default. |

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

## Open items before claiming it works

- Compile with MSVC (Win32) — never yet built on a real toolchain.
- Confirm the calling convention/arg order of `0x00547F50` and `0x00549500` on
  the live build; the generic stack-arg dump is convention-agnostic for logging
  but the column labels assume the documented argument order.
- Confirm `EBX` is the VM context at `0x005B1C62` at hook entry before trusting
  the `bci.op` decode (the dispatcher frame setup is at that address).
