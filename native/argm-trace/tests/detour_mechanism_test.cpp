// Mechanism test for the REAL native/argm-trace/src/detour.cpp on 32-bit Linux.
//
// It installs an actual inline detour over a 9-argument cdecl function that
// mimics s_addNPCJob_createUnit, calls it, and verifies:
//   (1) the generated stub reads the correct arguments off the caller stack,
//   (2) the trampoline runs the original prologue + body so the function still
//       returns the right value (behavior preserved),
//   (3) it survives repeated calls.
//
// This exercises the exact stub/trampoline/rel32/RegBank machine code the DLL
// will use inside the game. It does NOT test the Windows-only pieces
// (version.dll proxy, PE fingerprint, GetModuleHandle rebasing).
//
// Build & run (from native/argm-trace/tests, needs gcc-multilib):
//   g++ -m32 -std=c++17 -fno-pic -no-pie -fcf-protection=none \
//       -fno-stack-protector -I../src -Ishim -O0 \
//       detour_mechanism_test.cpp ../src/detour.cpp -o detour_mechanism_test
//   ./detour_mechanism_test
//
// Exit code 0 = all pass. shim/windows.h maps the few Win32 calls detour.cpp
// uses (VirtualAlloc/VirtualProtect/FlushInstructionCache) onto POSIX mmap.
#include "detour.h"

#include <cstdarg>
#include <cstdint>
#include <cstdio>
#include <cstring>

// ---- Shims for the symbols detour.cpp expects from tracelog.h ----
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

// This mirrors targets.cpp::ArgmOnHookHit exactly: compute the caller stack
// from the RegBank and invoke the descriptor's formatter.
extern "C" void ArgmOnHookHit(const argm::HookDescriptor* desc, argm::RegBank* rb) {
    if (!desc || !desc->formatter) return;
    const uint32_t* callerStack = reinterpret_cast<const uint32_t*>(
        reinterpret_cast<uint8_t*>(rb) + sizeof(argm::RegBank) + 4 /* eflags */);
    desc->formatter(*desc, *rb, callerStack);
}

// Formatter that records the args, exactly like FmtNpcJob would read them.
static void CaptureFormatter(const argm::HookDescriptor&, const argm::RegBank&,
                             const uint32_t* cs) {
    g_capturedRet = cs[0];
    for (int i = 0; i < 9; i++) g_capturedArgs[i] = cs[i + 1];
    g_hitCount++;
}

// Target: mimic s_addNPCJob_createUnit(team, mode, a3..a9). __attribute__ noinline
// + a real body so it has a normal relocatable prologue. Returns a value derived
// from its args so we can prove the trampoline preserved behavior.
extern "C" __attribute__((noinline, cdecl)) int addNpcJob(
    int team, int mode, int a3, int a4, int a5, int cntLo, int cntHi, int a8, int a9) {
    // A little arithmetic so the compiler emits a real body, not a stub.
    volatile int sink = a3 + a4 + a5 + a8 + a9;
    (void)sink; (void)mode; (void)cntHi;
    return team * 1000 + cntLo;
}

int main() {
    printf("== argm-trace detour mechanism test (32-bit) ==\n\n");

    // Baseline (no hook).
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

    // Call the hooked function.
    int hooked = addNpcJob(5, 1, 4, 0, 0, 20, 20, 8, 3);
    printf("\nhooked call returned %d (expect 5020 -> trampoline preserved behavior)\n",
           hooked);
    printf("hit count = %d (expect 1)\n", g_hitCount);
    printf("captured args = [%u %u %u %u %u %u %u %u %u] (expect 5 1 4 0 0 20 20 8 3)\n",
           g_capturedArgs[0], g_capturedArgs[1], g_capturedArgs[2], g_capturedArgs[3],
           g_capturedArgs[4], g_capturedArgs[5], g_capturedArgs[6], g_capturedArgs[7],
           g_capturedArgs[8]);
    printf("captured return address = %08x (nonzero, into caller)\n", g_capturedRet);

    // Second call with different args to prove per-call capture works.
    int hooked2 = addNpcJob(3, 3, 9, 1, 2, 15, 40, 0, 0);
    printf("\nsecond hooked call returned %d (expect 3015)\n", hooked2);
    printf("captured args = [%u %u %u %u %u %u %u %u %u] (expect 3 3 9 1 2 15 40 0 0)\n",
           g_capturedArgs[0], g_capturedArgs[1], g_capturedArgs[2], g_capturedArgs[3],
           g_capturedArgs[4], g_capturedArgs[5], g_capturedArgs[6], g_capturedArgs[7],
           g_capturedArgs[8]);

    // Verdict.
    int fail = 0;
    const uint32_t exp1[9] = {5, 1, 4, 0, 0, 20, 20, 8, 3};
    const uint32_t exp2[9] = {3, 3, 9, 1, 2, 15, 40, 0, 0};
    if (hooked != 5020) fail++;
    if (hooked2 != 3015) fail++;
    if (g_hitCount != 2) fail++;
    for (int i = 0; i < 9; i++) if (g_capturedArgs[i] != exp2[i]) fail++;
    // Re-run first to re-check exp1 capture in isolation.
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
