# argm-trace host tests

These cover the machine-code-critical parts of the hook engine so a regression
there is caught before the DLL ever touches the game.

## Easiest: via CMake/CTest on the MSVC toolchain (Windows)

Configuring argm-trace with `-DARGM_BUILD_TESTS=ON` builds both tests with the
same MSVC x86 toolchain that builds the shipping `winmm.dll` and registers
them as CTest tests:

```powershell
cmake -S native/argm-trace -B build/argm-trace -A Win32 -DARGM_BUILD_TESTS=ON
cmake --build build/argm-trace --config Release
ctest --test-dir build/argm-trace -C Release --output-on-failure
```

Verified 2026-07-18: the native proxy builds clean (Win32, correct undecorated
export table) and both CTest tests pass. The mechanism test there uses
`detour_mechanism_test_win.cpp` (real `<windows.h>`); the plain
`detour_mechanism_test.cpp` below is the Linux/g++ equivalent.

## Manual builds

The lde test is pure and portable; the mechanism test has a Linux/g++ variant
(mmap shim) and a Windows/MSVC variant (genuine Win32 APIs).

## `lde_test.cpp` — instruction length decoder

Validates `src/lde.h` (used to size a prologue for relocation) against 46
prologue instructions incl. the faction-selector prologue.

```
g++ -std=c++17 -Wall -Wextra -o lde_test lde_test.cpp && ./lde_test
```

## `detour_mechanism_test.cpp` / `detour_mechanism_test_win.cpp` — full inline-hook mechanism

Both install an actual detour over a 9-argument cdecl function that mimics
`s_addNPCJob_createUnit` and prove the generated stub reads the correct
arguments off the caller stack, the trampoline preserves the original
function's behavior, per-call capture works, and `RemoveAllDetours` cleanly
restores the target. They exercise the **real** `src/detour.cpp`.

- `detour_mechanism_test_win.cpp` (Windows/MSVC): compiles against genuine
  `<windows.h>` — the most faithful test. Built by the CTest flow above, or
  manually from an x86 developer prompt:
  ```
  cl /std:c++17 /EHsc /I..\src detour_mechanism_test_win.cpp ..\src\detour.cpp
  ```
- `detour_mechanism_test.cpp` (Linux/g++): uses `shim/windows.h` to map the few
  Win32 calls onto POSIX `mmap`. Requires `gcc-multilib` (32-bit):
  ```
  g++ -m32 -std=c++17 -fno-pic -no-pie -fcf-protection=none -fno-stack-protector \
      -I../src -Ishim -O0 detour_mechanism_test.cpp ../src/detour.cpp \
      -o detour_mechanism_test && ./detour_mechanism_test
  ```

Both exit 0 on success.

## What these do NOT cover

The remaining Windows-only pieces — the `winmm.dll` proxy forwarding at
runtime, the PE build-fingerprint gate, ASLR rebasing, and behavior against the
live `Against_Rome.exe` — require a user-controlled modifier Apply/Restore runtime check
next to the game and playing. That deployment step is the user's, since it
writes into the game install directory.
