#include <windows.h>

#include "config.h"
#include "detour.h"
#include "proxy_version.h"
#include "targets.h"
#include "tracelog.h"

namespace {

// Directory containing the running executable (where the log and ini live).
void GetExeDirectory(wchar_t* out, size_t cap) {
    out[0] = 0;
    wchar_t path[MAX_PATH];
    if (GetModuleFileNameW(nullptr, path, MAX_PATH) == 0) return;
    wchar_t* slash = wcsrchr(path, L'\\');
    if (slash) *slash = 0;
    wcscpy_s(out, cap, path);
}

// Runs off the loader lock: safe place to LoadLibrary, read files and patch code.
DWORD WINAPI InitThread(LPVOID) {
    wchar_t dir[MAX_PATH];
    GetExeDirectory(dir, MAX_PATH);

    argm::Config cfg = argm::LoadConfig(dir);
    if (!cfg.enabled) return 0;

    if (!argm::LogOpen(dir)) return 0;

    argm::LogRaw("==== argm-trace: Against Rome runtime flight recorder ====");
    argm::LogLine("init", "log opened, verifySignatures=%d",
                  cfg.verifySignatures ? 1 : 0);

    // A wrong build must never be patched: fingerprint gate first.
    bool proceed = argm::CheckBuildFingerprint(cfg);
    if (proceed) {
        argm::InstallAllHooks(cfg);
    } else {
        argm::LogLine("init", "hooks disabled for this build");
    }
    return 0;
}

}  // namespace

BOOL APIENTRY DllMain(HMODULE module, DWORD reason, LPVOID) {
    switch (reason) {
        case DLL_PROCESS_ATTACH:
            DisableThreadLibraryCalls(module);
            // Forward version.dll immediately so early callers work.
            argm::LoadRealVersionDll();
            // Do the heavy lifting off the loader lock.
            CreateThread(nullptr, 0, InitThread, nullptr, 0, nullptr);
            break;
        case DLL_PROCESS_DETACH:
            argm::RemoveAllDetours();
            argm::LogLine("shutdown", "process detach");
            argm::LogClose();
            break;
    }
    return TRUE;
}
