#include <windows.h>

#include "config.h"
#include "detour.h"
#include "proxy_winmm.h"
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

    argm::LogRaw("==== argm-trace: Against Rome runtime flight recorder (winmm proxy) ====");
    argm::LogLine("init", "log opened, verifySignatures=%d enableHooks=%d",
                  cfg.verifySignatures ? 1 : 0, cfg.enableHooks ? 1 : 0);

    // Always record the build fingerprint banner -- it is pure logging and never
    // touches game code.
    bool fingerprintOk = argm::CheckBuildFingerprint(cfg);

    // Master hook gate. Log-only mode (enableHooks=0) never installs any hook,
    // so it cannot destabilize the game -- this is the safe first bring-up that
    // proves the winmm proxy loads and the log works. Only once that is
    // confirmed does enabling hooks become worthwhile.
    if (!cfg.enableHooks) {
        argm::LogLine("init",
                      "LOG-ONLY MODE: hook installation disabled (enableHooks=0). "
                      "The proxy loaded and the log works; set [general] enableHooks=1 "
                      "to install AI-event hooks once startup is confirmed stable.");
        return 0;
    }

    if (fingerprintOk) {
        argm::InstallAllHooks(cfg);
    } else {
        argm::LogLine("init", "hooks disabled for this build (fingerprint mismatch)");
    }
    return 0;
}

}  // namespace

BOOL APIENTRY DllMain(HMODULE module, DWORD reason, LPVOID) {
    switch (reason) {
        case DLL_PROCESS_ATTACH:
            DisableThreadLibraryCalls(module);
            // Forward winmm immediately so the game's early audio/timer calls
            // work. Must happen before any thunk is hit.
            argm::LoadRealWinmmDll();
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
