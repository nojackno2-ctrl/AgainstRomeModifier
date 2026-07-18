#include "detour.h"

#include <windows.h>

#include "lde.h"
#include "tracelog.h"

namespace argm {
namespace {

constexpr size_t kJmpSize = 5;          // E9 rel32
constexpr size_t kMaxStolen = 32;       // upper bound on prologue we relocate
constexpr size_t kMaxHooks = 32;

struct Installed {
    void* target = nullptr;
    uint8_t original[kMaxStolen] = {};
    size_t originalLen = 0;
    void* trampoline = nullptr;
    void* stub = nullptr;
};

Installed g_installed[kMaxHooks];
size_t g_installedCount = 0;

}  // namespace

// DecodeLength is provided by lde.h (header-only, host-unit-tested).

bool VerifySignature(const HookDescriptor& desc) {
    if (!desc.signature || !desc.signatureMask) return true;
    const uint8_t* p = static_cast<const uint8_t*>(desc.target);
    for (size_t i = 0; desc.signatureMask[i]; i++) {
        if (desc.signatureMask[i] == 'x' && p[i] != desc.signature[i]) return false;
    }
    return true;
}

namespace {

// True if the instruction at `code` is IP-relative and therefore unsafe to
// copy verbatim into a trampoline.
bool IsRelative(const uint8_t* code) {
    uint8_t op = code[0];
    if (op == 0xE8 || op == 0xE9 || op == 0xEB) return true;      // call/jmp rel
    if (op >= 0x70 && op <= 0x7F) return true;                    // jcc rel8
    if (op >= 0xE0 && op <= 0xE3) return true;                    // loop/jecxz
    if (op == 0x0F && code[1] >= 0x80 && code[1] <= 0x8F) return true;  // jcc rel32
    return false;
}

void Protect(void* addr, size_t len, DWORD prot, DWORD* old) {
    VirtualProtect(addr, len, prot, old);
}

uint32_t Abs32(const void* p) {
    return static_cast<uint32_t>(reinterpret_cast<uintptr_t>(p));
}

// rel32 for an E8/E9 whose operand ends at `instrEnd`, targeting `dest`.
int32_t Rel32(const void* instrEnd, const void* dest) {
    return static_cast<int32_t>(reinterpret_cast<uintptr_t>(dest) -
                                reinterpret_cast<uintptr_t>(instrEnd));
}

}  // namespace

// Implemented in targets.cpp; the generated stub calls this.
extern "C" void __cdecl ArgmOnHookHit(const HookDescriptor* desc, RegBank* rb);

bool InstallDetour(HookDescriptor& desc) {
    if (!desc.target || g_installedCount >= kMaxHooks) return false;
    if (!VerifySignature(desc)) {
        LogLine("hook", "SKIP %s: signature mismatch at %p", desc.name, desc.target);
        return false;
    }

    uint8_t* target = static_cast<uint8_t*>(desc.target);

    // Measure a whole number of instructions >= kJmpSize; abort on anything
    // relative or undecodable.
    size_t stolen = 0;
    while (stolen < kJmpSize) {
        const uint8_t* here = target + stolen;
        if (IsRelative(here)) {
            LogLine("hook", "SKIP %s: relative instr in prologue at +%u",
                    desc.name, static_cast<unsigned>(stolen));
            return false;
        }
        size_t len = DecodeLength(here);
        if (len == 0 || stolen + len > kMaxStolen) {
            LogLine("hook", "SKIP %s: undecodable prologue at +%u", desc.name,
                    static_cast<unsigned>(stolen));
            return false;
        }
        stolen += len;
    }

    // Trampoline: original bytes + jmp back to (target + stolen).
    uint8_t* tramp = static_cast<uint8_t*>(
        VirtualAlloc(nullptr, stolen + kJmpSize, MEM_COMMIT | MEM_RESERVE,
                     PAGE_EXECUTE_READWRITE));
    if (!tramp) return false;
    memcpy(tramp, target, stolen);
    tramp[stolen] = 0xE9;
    *reinterpret_cast<int32_t*>(tramp + stolen + 1) =
        Rel32(tramp + stolen + kJmpSize, target + stolen);

    // Stub: pushfd; pushad; mov eax,esp; push eax; push desc; call OnHookHit;
    //       add esp,8; popad; popfd; jmp trampoline.
    uint8_t* stub = static_cast<uint8_t*>(
        VirtualAlloc(nullptr, 32, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE));
    if (!stub) {
        VirtualFree(tramp, 0, MEM_RELEASE);
        return false;
    }
    size_t o = 0;
    stub[o++] = 0x9C;                               // pushfd
    stub[o++] = 0x60;                               // pushad
    stub[o++] = 0x89; stub[o++] = 0xE0;             // mov eax,esp
    stub[o++] = 0x50;                               // push eax (arg2 = RegBank*)
    stub[o++] = 0x68;                               // push imm32 (arg1 = desc)
    *reinterpret_cast<uint32_t*>(stub + o) = Abs32(&desc);
    o += 4;
    stub[o++] = 0xE8;                               // call rel32
    *reinterpret_cast<int32_t*>(stub + o) =
        Rel32(stub + o + 4, reinterpret_cast<const void*>(&ArgmOnHookHit));
    o += 4;
    stub[o++] = 0x83; stub[o++] = 0xC4; stub[o++] = 0x08;  // add esp,8
    stub[o++] = 0x61;                               // popad
    stub[o++] = 0x9D;                               // popfd
    stub[o++] = 0xE9;                               // jmp rel32 -> trampoline
    *reinterpret_cast<int32_t*>(stub + o) = Rel32(stub + o + 4, tramp);
    o += 4;

    // Patch the target with a jmp to the stub, padding the tail with NOPs.
    DWORD oldProt = 0;
    Protect(target, stolen, PAGE_EXECUTE_READWRITE, &oldProt);

    Installed& rec = g_installed[g_installedCount];
    rec.target = target;
    rec.originalLen = stolen;
    memcpy(rec.original, target, stolen);
    rec.trampoline = tramp;
    rec.stub = stub;

    target[0] = 0xE9;
    *reinterpret_cast<int32_t*>(target + 1) = Rel32(target + kJmpSize, stub);
    for (size_t i = kJmpSize; i < stolen; i++) target[i] = 0x90;

    Protect(target, stolen, oldProt, &oldProt);
    FlushInstructionCache(GetCurrentProcess(), target, stolen);

    g_installedCount++;
    desc.trampoline = tramp;
    desc.installed = true;
    LogLine("hook", "OK   %s at %p (stole %u bytes)", desc.name, desc.target,
            static_cast<unsigned>(stolen));
    return true;
}

void RemoveAllDetours() {
    for (size_t i = 0; i < g_installedCount; i++) {
        Installed& rec = g_installed[i];
        if (!rec.target) continue;
        DWORD oldProt = 0;
        Protect(rec.target, rec.originalLen, PAGE_EXECUTE_READWRITE, &oldProt);
        memcpy(rec.target, rec.original, rec.originalLen);
        Protect(rec.target, rec.originalLen, oldProt, &oldProt);
        FlushInstructionCache(GetCurrentProcess(), rec.target, rec.originalLen);
        if (rec.trampoline) VirtualFree(rec.trampoline, 0, MEM_RELEASE);
        if (rec.stub) VirtualFree(rec.stub, 0, MEM_RELEASE);
        rec.target = nullptr;
    }
    g_installedCount = 0;
}

}  // namespace argm
