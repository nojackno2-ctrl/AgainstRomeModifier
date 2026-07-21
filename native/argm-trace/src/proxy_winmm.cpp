#include "proxy_winmm.h"

#include <windows.h>

// Each genuine winmm export the game imports is resolved at load time; the
// exported names below are naked thunks that jump straight to the resolved
// pointer, forwarding transparently regardless of calling convention or arity.
//
// The set is exactly what Against_Rome.exe imports from WINMM.dll (17 by name +
// one by ordinal, Ordinal 2 which is a NONAME export in winmm). Other processes
// rarely import winmm, and dgVoodoo is not used alongside this proxy, so this
// set is sufficient; a missing forward would only matter if some other loaded
// module imported a winmm function not listed here.
namespace {

#define WINMM_EXPORTS(X)             \
    X(auxGetDevCapsA)                \
    X(auxGetNumDevs)                 \
    X(auxSetVolume)                  \
    X(joyGetDevCapsA)                \
    X(joyGetNumDevs)                 \
    X(joyGetPosEx)                   \
    X(mciGetErrorStringA)            \
    X(mciSendCommandA)               \
    X(mixerClose)                    \
    X(mixerGetControlDetailsA)       \
    X(mixerGetDevCapsA)              \
    X(mixerGetLineControlsA)         \
    X(mixerGetLineInfoA)             \
    X(mixerGetNumDevs)               \
    X(mixerOpen)                     \
    X(mixerSetControlDetails)        \
    X(timeGetTime)

#define DECLARE_PTR(name) void* g_real_##name = nullptr;
WINMM_EXPORTS(DECLARE_PTR)
#undef DECLARE_PTR

// The game also imports winmm Ordinal 2 (a NONAME export). Resolved by ordinal.
void* g_real_Ordinal2 = nullptr;

}  // namespace

namespace argm {

bool LoadRealWinmmDll() {
    wchar_t sys[MAX_PATH];
    // A 32-bit process gets the SysWOW64 path here under WOW64, i.e. the genuine
    // 32-bit winmm.dll -- exactly what we must forward to.
    UINT n = GetSystemDirectoryW(sys, MAX_PATH);
    if (n == 0 || n >= MAX_PATH) return false;
    wcscat_s(sys, MAX_PATH, L"\\winmm.dll");

    HMODULE real = LoadLibraryW(sys);
    if (!real) return false;

#define RESOLVE(name) g_real_##name = reinterpret_cast<void*>(GetProcAddress(real, #name));
    WINMM_EXPORTS(RESOLVE)
#undef RESOLVE
    g_real_Ordinal2 = reinterpret_cast<void*>(GetProcAddress(real, MAKEINTRESOURCEA(2)));
    return true;
}

}  // namespace argm

// Naked forwarding thunks. The `Thunk_` prefix keeps them from colliding with
// any SDK winmm prototypes; argm_trace.def maps each real winmm export name (and
// Ordinal 2) back to its thunk, so the DLL's export table matches genuine winmm
// for the imports the game uses.
extern "C" {
#define THUNK(name)                               \
    __declspec(naked) void Thunk_##name() {       \
        __asm { jmp dword ptr [g_real_##name] }   \
    }
WINMM_EXPORTS(THUNK)
#undef THUNK

__declspec(naked) void Thunk_Ordinal2() {
    __asm { jmp dword ptr [g_real_Ordinal2] }
}
}
