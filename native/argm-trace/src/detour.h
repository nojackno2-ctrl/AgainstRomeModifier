// argm-trace: minimal, self-contained inline-hook engine.
//
// Design goals, in priority order:
//   1. Never corrupt the game. Every install verifies a caller-supplied byte
//      signature first and refuses (returns false) on any mismatch, on any
//      relative branch inside the stolen prologue, or on any instruction the
//      bundled length decoder cannot classify. A refused hook is a no-op.
//   2. No third-party dependency and no network fetch: the length decoder and
//      trampoline builder are contained here.
//   3. Logging only. The generated stub preserves all registers and flags and
//      does not disturb the call stack, so it works for cdecl and stdcall
//      targets without knowing their exact signature.
#pragma once

#include <cstdint>
#include <cstddef>

namespace argm {

// Register block as pushed by PUSHAD (low address -> high address).
struct RegBank {
    uint32_t edi, esi, ebp, esp, ebx, edx, ecx, eax;
};

struct HookDescriptor;

// Formatter invoked for every hit. callerStack[0] is the return address into
// the caller; callerStack[1..] are the stack arguments (valid for cdecl and
// stdcall). rb exposes the register state at function entry.
using HitFormatter = void (*)(const HookDescriptor& desc, const RegBank& rb,
                              const uint32_t* callerStack);

struct HookDescriptor {
    const char* name = nullptr;      // human-readable, used as the log category
    void* target = nullptr;          // absolute address to hook
    const uint8_t* signature = nullptr;  // expected bytes at target (nullptr = skip)
    const char* signatureMask = nullptr; // 'x' match, '?' wildcard; len == sig len
    HitFormatter formatter = nullptr;    // per-target log formatter
    int argCount = 0;                    // generic arg dump width (formatter may ignore)

    // Filled in by InstallDetour.
    void* trampoline = nullptr;      // call this to reach the original function
    bool installed = false;
};

// The single-instruction length decoder used to size prologues for relocation
// lives in lde.h (argm::DecodeLength), kept header-only so it can be unit
// tested on the host without <windows.h>.

// Verifies desc.signature/desc.signatureMask against desc.target. Returns true
// when signature is null (verification disabled for that target).
bool VerifySignature(const HookDescriptor& desc);

// Installs the hook described by desc. Returns true and sets desc.installed /
// desc.trampoline on success; returns false and leaves the target untouched on
// any safety failure.
bool InstallDetour(HookDescriptor& desc);

// Removes all installed detours and restores original bytes. Best-effort; used
// on DLL unload.
void RemoveAllDetours();

}  // namespace argm
