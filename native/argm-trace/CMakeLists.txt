# argm-trace: Against Rome runtime flight-recorder DLL.
#
# Target platform is 32-bit x86 (Win32) to match Against_Rome.exe. Configure and
# build with the MSVC toolchain, for example:
#
#   cmake -S native/argm-trace -B build/argm-trace -A Win32
#   cmake --build build/argm-trace --config Release
#
# The output is version.dll; drop it next to Against_Rome.exe. See README.md.
cmake_minimum_required(VERSION 3.20)
project(argm_trace CXX)

if(NOT CMAKE_SIZEOF_VOID_P EQUAL 4)
    message(FATAL_ERROR "argm-trace must be built as 32-bit (Win32). Pass -A Win32.")
endif()

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

add_library(argm_trace SHARED
    src/dllmain.cpp
    src/proxy_version.cpp
    src/config.cpp
    src/detour.cpp
    src/targets.cpp
    src/tracelog.cpp
)

# Ship as version.dll (drop-in proxy loader) using the export list in the .def.
set_target_properties(argm_trace PROPERTIES
    OUTPUT_NAME "version"
    PREFIX ""
)

if(MSVC)
    target_sources(argm_trace PRIVATE argm_trace.def)
    target_compile_definitions(argm_trace PRIVATE
        WIN32_LEAN_AND_MEAN
        _CRT_SECURE_NO_WARNINGS
        NOMINMAX
    )
    target_compile_options(argm_trace PRIVATE /W4 /permissive-)
    # /SAFESEH:NO so the hand-written __asm thunks and stubs link cleanly.
    target_link_options(argm_trace PRIVATE /SAFESEH:NO)
endif()

# Do NOT link version.lib: the genuine version.dll is loaded at runtime via
# LoadLibrary + GetProcAddress (see proxy_version.cpp). Linking the import
# library here would make this proxy depend on itself.
