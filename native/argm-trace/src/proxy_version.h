// argm-trace: version.dll proxy resolver.
//
// This DLL ships named "version.dll" and sits next to Against_Rome.exe. Windows
// loads it in preference to the system copy, so it is a dependency-free way to
// get code running inside the game without an external injector. Every real
// version.dll export is forwarded to the genuine system DLL.
#pragma once

namespace argm {

// Loads the genuine %SystemRoot%\System32\version.dll and resolves every
// forwarded export. Returns false if the system DLL cannot be loaded (in which
// case version APIs would fail, but the game rarely calls them).
bool LoadRealVersionDll();

}  // namespace argm
