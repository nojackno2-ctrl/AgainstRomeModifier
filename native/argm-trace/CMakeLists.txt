# argm-trace: Against Rome runtime flight-recorder DLL.
#
# Target platform is 32-bit x86 (Win32) to match Against_Rome.exe. Configure and
# build with the MSVC toolchain, for example:
#
#   cmake -S native/argm-trace -B build/argm-trace -A Win32
#   cmake --build build/argm-trace --config Release
#
# The output is winmm.dll; see README.md for the user-controlled runtime step.
cmake_minimum_required(VERSION 3.20)
project(argm_trace CXX)

if(NOT CMAKE_SIZEOF_VOID_P EQUAL 4)
    message(FATAL_ERROR "argm-trace must be built as 32-bit (Win32). Pass -A Win32.")
endif()

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Statically link the CRT (/MT). winmm.dll is loaded into a 2004 game whose
# environment (and any end user's machine) may not have the VC++ redistributable
# / Universal CRT. A dynamic-CRT proxy DLL fails to load -- the loader can't
# resolve VCRUNTIME140.dll / api-ms-win-crt-*.dll -- and the game refuses to
# start. Static linking makes the DLL self-contained (only KERNEL32 remains).
cmake_policy(SET CMP0091 NEW)
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")

add_library(argm_trace SHARED
    src/dllmain.cpp
    src/proxy_winmm.cpp
    src/config.cpp
    src/detour.cpp
    src/targets.cpp
    src/tracelog.cpp
)

# Ship as winmm.dll (drop-in proxy loader) using the export list in the .def.
# The game statically imports WINMM.dll and winmm is not a KnownDLL, so our copy
# in the game folder is loaded at startup. (version.dll was the wrong target --
# the game never imports it.)
set_target_properties(argm_trace PROPERTIES
    OUTPUT_NAME "winmm"
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

# Do not link winmm.lib: the genuine system DLL is loaded at runtime via
# LoadLibrary + GetProcAddress (see proxy_winmm.cpp).

# --- Host tests (opt-in) --------------------------------------------------
# Configure with -DARGM_BUILD_TESTS=ON to build and register the host tests as
# CTest tests on the same MSVC x86 toolchain that builds winmm.dll:
#   cmake -S native/argm-trace -B build/argm-trace -A Win32 -DARGM_BUILD_TESTS=ON
#   cmake --build build/argm-trace --config Release
#   ctest --test-dir build/argm-trace -C Release --output-on-failure
option(ARGM_BUILD_TESTS "Build the argm-trace host tests" OFF)
if(ARGM_BUILD_TESTS)
    enable_testing()

    # Instruction-length decoder: pure, portable, no <windows.h>.
    add_executable(lde_test tests/lde_test.cpp)
    add_test(NAME lde_test COMMAND lde_test)

    # Full inline-hook mechanism against the real detour.cpp + genuine Win32
    # APIs (the *_win variant; the plain detour_mechanism_test.cpp targets
    # Linux/g++ with the mmap shim and is not built here).
    add_executable(detour_mechanism_test
        tests/detour_mechanism_test_win.cpp
        src/detour.cpp
    )
    target_include_directories(detour_mechanism_test PRIVATE src)
    if(MSVC)
        target_compile_definitions(detour_mechanism_test PRIVATE
            WIN32_LEAN_AND_MEAN _CRT_SECURE_NO_WARNINGS NOMINMAX)
        target_link_options(detour_mechanism_test PRIVATE /SAFESEH:NO)
    endif()
    add_test(NAME detour_mechanism_test COMMAND detour_mechanism_test)
endif()
