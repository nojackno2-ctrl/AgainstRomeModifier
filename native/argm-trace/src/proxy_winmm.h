// argm-trace: winmm.dll proxy resolver.
//
// This DLL ships named "winmm.dll" and sits next to Against_Rome.exe. The game
// statically imports WINMM.dll (confirmed via its PE import table), and winmm is
// NOT a KnownDLL, so Windows loads our copy from the application directory in
// preference to the system copy -- a dependency-free way to get code running
// inside the game at startup without an external injector.
//
// (The earlier version.dll proxy was the wrong target: the game does not import
// version.dll, so it was never loaded on its own, and when dgVoodoo's version
// API calls pulled it in, it conflicted. winmm is imported directly by the game
// and loads before dgVoodoo touches anything.)
//
// Every winmm export the game (and its process) needs is forwarded to the
// genuine system winmm.dll.
#pragma once

namespace argm {

// Loads the genuine 32-bit system winmm.dll (SysWOW64 under WOW64) and resolves
// every forwarded export. Returns false if the system DLL cannot be loaded, in
// which case audio/timer APIs would fail -- so this must succeed for the game to
// run, and the proxy is careful to load it in DllMain before any thunk is hit.
bool LoadRealWinmmDll();

}  // namespace argm
