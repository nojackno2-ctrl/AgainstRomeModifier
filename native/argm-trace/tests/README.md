# argm-trace host tests

These run on a normal dev host (Linux/x86) — no Windows, MSVC, or the game
needed. They cover the machine-code-critical parts of the hook engine so a
regression there is caught before the DLL ever touches the game.

## `lde_test.cpp` — instruction length decoder

Validates `src/lde.h` (used to size a prologue for relocation) against 46
prologue instructions incl. the faction-selector prologue.

```
g++ -std=c++17 -Wall -Wextra -o lde_test lde_test.cpp && ./lde_test
```

## `detour_mechanism_test.cpp` — full inline-hook mechanism

Compiles the **real** `src/detour.cpp` (with `shim/windows.h` mapping the few
Win32 calls onto POSIX `mmap`) and installs an actual detour over a 9-argument
cdecl function that mimics `s_addNPCJob_createUnit`. It proves the generated
stub reads the correct arguments off the caller stack, the trampoline preserves
the original function's behavior, per-call capture works, and `RemoveAllDetours`
cleanly restores the target. Requires `gcc-multilib` (32-bit).

```
g++ -m32 -std=c++17 -fno-pic -no-pie -fcf-protection=none -fno-stack-protector \
    -I../src -Ishim -O0 detour_mechanism_test.cpp ../src/detour.cpp \
    -o detour_mechanism_test && ./detour_mechanism_test
```

Both exit 0 on success.

## What these do NOT cover

The Windows-only pieces — the `version.dll` proxy forwarding, PE build
fingerprint gate, ASLR rebasing, and behavior against the live `Against_Rome.exe`
— can only be validated by building with MSVC (Win32) and running the game.
