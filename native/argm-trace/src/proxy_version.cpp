#include "proxy_version.h"

#include <windows.h>

// Each real version.dll entry point is resolved at load time; the exported
// names below are naked thunks that jump straight to the resolved pointer, so
// they forward transparently regardless of calling convention or arity.
namespace {

#define VERSION_EXPORTS(X)          \
    X(GetFileVersionInfoA)          \
    X(GetFileVersionInfoByHandle)   \
    X(GetFileVersionInfoExA)        \
    X(GetFileVersionInfoExW)        \
    X(GetFileVersionInfoSizeA)      \
    X(GetFileVersionInfoSizeExA)    \
    X(GetFileVersionInfoSizeExW)    \
    X(GetFileVersionInfoSizeW)      \
    X(GetFileVersionInfoW)          \
    X(VerFindFileA)                 \
    X(VerFindFileW)                 \
    X(VerInstallFileA)              \
    X(VerInstallFileW)              \
    X(VerQueryValueA)               \
    X(VerQueryValueW)

#define DECLARE_PTR(name) void* g_real_##name = nullptr;
VERSION_EXPORTS(DECLARE_PTR)
#undef DECLARE_PTR

}  // namespace

namespace argm {

bool LoadRealVersionDll() {
    wchar_t sys[MAX_PATH];
    UINT n = GetSystemDirectoryW(sys, MAX_PATH);
    if (n == 0 || n >= MAX_PATH) return false;
    wcscat_s(sys, MAX_PATH, L"\\version.dll");

    HMODULE real = LoadLibraryW(sys);
    if (!real) return false;

#define RESOLVE(name) g_real_##name = reinterpret_cast<void*>(GetProcAddress(real, #name));
    VERSION_EXPORTS(RESOLVE)
#undef RESOLVE
    return true;
}

}  // namespace argm

// Naked forwarding thunks. The export names are supplied by argm_trace.def so
// they match the genuine version.dll exactly (undecorated), independent of the
// compiler's default name decoration.
extern "C" {
#define THUNK(name)                               \
    __declspec(naked) void name() {               \
        __asm { jmp dword ptr [g_real_##name] }   \
    }
VERSION_EXPORTS(THUNK)
#undef THUNK
}
