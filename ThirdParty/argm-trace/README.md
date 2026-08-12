# ThirdParty/argm-trace

`winmm.dll` here is a **prebuilt binary** of this repository's own
`native/argm-trace` runtime flight-recorder, checked in so the .NET build can
embed it without requiring the C++ toolchain (mirrors how `ThirdParty/dgVoodoo2`
ships prebuilt DLLs).

- Source: `native/argm-trace` (32-bit `winmm.dll` proxy, MSVC/Win32).
- Embedded by `src.Core/AgainstRome.Core.csproj` as `argm-trace.winmm.dll`
  and deployed to the game folder by `ArgmTraceFeature` when the "Enable
  Gameplay Trace Logger" option is applied.

## Rebuilding

From the repo root, with the VS C++ workload installed:

```powershell
cmake -S native/argm-trace -B build/argm-trace -A Win32 -DARGM_BUILD_TESTS=ON
cmake --build build/argm-trace --config Release
copy build\argm-trace\Release\winmm.dll ThirdParty\argm-trace\winmm.dll
```

Then rebuild the solution so the new binary is re-embedded. Keep this copy in
sync with `native/argm-trace` whenever that source changes.
