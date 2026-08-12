// MSVC/Win32 mechanism test for the REAL native/argm-trace/src/detour.cpp.
//
// Identical intent to detour_mechanism_test.cpp, but compiled natively against
// the genuine <windows.h> (VirtualAlloc/VirtualProtect/FlushInstructionCache)
// with the same MSVC x86 toolchain that builds the shipping winmm.dll. This
// is the most faithful possible test of the stub/trampoline/rel32/RegBank
// machine code the DLL runs inside Against_Rome.exe: no POSIX mmap shim stands
// in for the OS. It installs an actual inline detour over a 9-argument __cdecl
// function that mimics s_addNPCJob_createUnit and verifies:
//   (1) the generated stub reads the correct arguments off the caller stack,
//   (2) the trampoline runs the original prologue + body (behavior preserved),
//   (3) per-call capture works across repeated calls,
//   (4) RemoveAllDetours cleanly restores the original bytes.
//
// Build & run: configure argm-trace with -DARGM_BUILD_TESTS=ON (see
// CMakeLists.txt) which adds this and lde_test as CTest tests, or compile
// manually from this folder with the x86 developer prompt:
//   cl /std:c++17 /EHsc /I..\src detour_mechanism_test_win.cpp ..\src\detour.cpp
//
// Exit code 0 = all pass.
#include "detour.h"

#include <cstdarg>
#include <cstdint>
#include <cstdio>

// ---- Stubs for the symbols detour.cpp expects from tracelog.h ----
namespace argm {
void LogLine(const char*, const char* fmt, ...) {
    va_list a; va_start(a, fmt);
    printf("    [log] "); vprintf(fmt, a); printf("\n");
    va_end(a);
}
void LogLineV(const char*, const char*, va_list) {}
void LogRaw(const char*, ...) {}
bool LogOpen(const wchar_t*) { return true; }
void LogClose() {}
}  // namespace argm

// ---- Captured hit data ----
static uint32_t g_capturedArgs[10];
static uint32_t g_capturedRet;
static int g_hitCount = 0;

// Mirrors targets.cpp::ArgmOnHookHit exactly.
extern "C" void __cdecl ArgmOnHookHit(const argm::HookDescriptor* desc, argm::RegBank* rb) {
    if (!desc || !desc->formatter) return;
    const uint32_t* callerStack = reinterpret_cast<const uint32_t*>(
        reinterpret_cast<uint8_t*>(rb) + sizeof(argm::RegBank) + 4 /* eflags */);
    desc->formatter(*desc, *rb, callerStack);
}

static void CaptureFormatter(const argm::HookDescriptor&, const argm::RegBank&,
                             const uint32_t* cs) {
    g_capturedRet = cs[0];
    for (int i = 0; i < 9; i++) g_capturedArgs[i] = cs[i + 1];
    g_hitCount++;
}

// Target: mimic s_addNPCJob_createUnit(team, mode, a3..a9). noinline + a real
// body so it has a normal relocatable prologue; returns a value derived from
// its args so the trampoline's behavior preservation is observable.
extern "C" __declspec(noinline) int __cdecl addNpcJob(
    int team, int mode, int a3, int a4, int a5, int cntLo, int cntHi, int a8, int a9) {
    volatile int sink = a3 + a4 + a5 + a8 + a9;
    (void)sink; (void)mode; (void)cntHi;
    return team * 1000 + cntLo;
}

int main() {
    printf("== argm-trace detour mechanism test (MSVC/Win32) ==\n\n");

    int base = addNpcJob(5, 1, 4, 0, 0, 20, 20, 8, 3);
    printf("baseline addNpcJob(5,1,4,0,0,20,20,8,3) = %d (expect 5020)\n", base);

    argm::HookDescriptor desc{};
    desc.name = "npcjob-test";
    desc.target = reinterpret_cast<void*>(&addNpcJob);
    desc.formatter = CaptureFormatter;
    desc.argCount = 9;
    desc.signature = nullptr;   // skip signature check for the test
    desc.signatureMask = nullptr;

    bool ok = argm::InstallDetour(desc);
    printf("\nInstallDetour returned %s, installed=%d, trampoline=%p\n",
           ok ? "true" : "false", desc.installed ? 1 : 0, desc.trampoline);
    if (!ok) { printf("\nRESULT: FAIL (hook not installed)\n"); return 1; }

    int hooked = addNpcJob(5, 1, 4, 0, 0, 20, 20, 8, 3);
    printf("\nhooked call returned %d (expect 5020 -> trampoline preserved behavior)\n", hooked);
    printf("hit count = %d (expect 1)\n", g_hitCount);
    printf("captured args = [%u %u %u %u %u %u %u %u %u] (expect 5 1 4 0 0 20 20 8 3)\n",
           g_capturedArgs[0], g_capturedArgs[1], g_capturedArgs[2], g_capturedArgs[3],
           g_capturedArgs[4], g_capturedArgs[5], g_capturedArgs[6], g_capturedArgs[7],
           g_capturedArgs[8]);
    printf("captured return address = %08x (nonzero, into caller)\n", g_capturedRet);

    int hooked2 = addNpcJob(3, 3, 9, 1, 2, 15, 40, 0, 0);
    printf("\nsecond hooked call returned %d (expect 3015)\n", hooked2);
    printf("captured args = [%u %u %u %u %u %u %u %u %u] (expect 3 3 9 1 2 15 40 0 0)\n",
           g_capturedArgs[0], g_capturedArgs[1], g_capturedArgs[2], g_capturedArgs[3],
           g_capturedArgs[4], g_capturedArgs[5], g_capturedArgs[6], g_capturedArgs[7],
           g_capturedArgs[8]);

    int fail = 0;
    const uint32_t exp1[9] = {5, 1, 4, 0, 0, 20, 20, 8, 3};
    const uint32_t exp2[9] = {3, 3, 9, 1, 2, 15, 40, 0, 0};
    if (hooked != 5020) fail++;
    if (hooked2 != 3015) fail++;
    if (g_hitCount != 2) fail++;
    for (int i = 0; i < 9; i++) if (g_capturedArgs[i] != exp2[i]) fail++;
    g_hitCount = 0;
    int hooked3 = addNpcJob(5, 1, 4, 0, 0, 20, 20, 8, 3);
    if (hooked3 != 5020) fail++;
    for (int i = 0; i < 9; i++) if (g_capturedArgs[i] != exp1[i]) fail++;

    argm::RemoveAllDetours();
    int hcBefore = g_hitCount;
    int afterRemove = addNpcJob(7, 0, 0, 0, 0, 99, 0, 0, 0);
    printf("\nafter RemoveAllDetours: addNpcJob(7,...,99,...) = %d (expect 7099), "
           "hitCount %d->%d (expect unchanged)\n", afterRemove, hcBefore, g_hitCount);
    if (afterRemove != 7099) fail++;
    if (g_hitCount != hcBefore) fail++;  // no hit after removal

    printf("\nRESULT: %s\n", fail ? "FAIL" : "ALL PASS");
    return fail ? 1 : 0;
}
