#include "targets.h"

#include <windows.h>

#include "detour.h"
#include "tracelog.h"

namespace argm {
namespace {

// Preferred image base the documented VAs were recorded against. Every address
// in the reverse-engineering docs is an absolute VA under the stock EXE, which
// links at 0x00400000. If the OS relocates the module, we shift by the delta.
constexpr uintptr_t kDocPreferredBase = 0x00400000;

uintptr_t g_actualBase = 0;
uintptr_t g_loadDelta = 0;
uintptr_t g_expectedTimeDateStamp = 0;

// Resolve a documented VA to its runtime address.
void* Va(uintptr_t documentedVa) {
    return reinterpret_cast<void*>(documentedVa + g_loadDelta);
}

// __try-guarded 32-bit read; returns fallback on access violation.
uint32_t SafeRead32(uintptr_t addr, uint32_t fallback) {
    __try {
        return *reinterpret_cast<volatile uint32_t*>(addr);
    } __except (EXCEPTION_EXECUTE_HANDLER) {
        return fallback;
    }
}

// ---- Per-target log formatters -----------------------------------------

// s_addNPCJob_createUnit implementation (00547F50): the AI reinforcement
// spawn. Arg1 team 0..7, arg2 mode (0/1/3), arg3 0..9, arg6/arg7 the clamped
// unit-count range. This is the single most useful "what did the AI do" event.
void FmtNpcJob(const HookDescriptor&, const RegBank&, const uint32_t* cs) {
    LogLine("ai.spawn",
            "addNPCJob_createUnit team=%u mode=%u a3=%u a4=%u a5=%u "
            "count=%u..%u a8=%u a9=%u ret=%08X",
            cs[1], cs[2], cs[3], cs[4], cs[5], cs[6], cs[7], cs[8], cs[9], cs[0]);
}

// s_setNPCActive implementation (00548CE0): team, activeFlag. Marks a defeated
// AI team as eligible to respawn.
void FmtNpcActive(const HookDescriptor&, const RegBank&, const uint32_t* cs) {
    LogLine("ai.active", "setNPCActive team=%u active=%u ret=%08X",
            cs[1], cs[2], cs[0]);
}

// s_setVillageTemplate implementation (00549500): settlement-style arrival.
void FmtVillage(const HookDescriptor& d, const RegBank&, const uint32_t* cs) {
    LogLine("ai.village", "setVillageTemplate a1=%08X a2=%08X a3=%08X ret=%08X",
            cs[1], cs[2], cs[3], cs[0]);
    (void)d;
}

// s_createUnitAndMems callback (0052A020).
void FmtCreateUnit(const HookDescriptor&, const RegBank&, const uint32_t* cs) {
    LogLine("ai.unit", "createUnitAndMems a1=%08X a2=%08X a3=%08X ret=%08X",
            cs[1], cs[2], cs[3], cs[0]);
}

// Endless Roman faction selector setter (0045BD60). Verified 21-byte
// signature. arg1 = requested faction id.
void FmtFaction(const HookDescriptor&, const RegBank&, const uint32_t* cs) {
    LogLine("game.faction", "factionSelect requested=%u ret=%08X", cs[1], cs[0]);
}

// BCI VM dispatcher (005B1C62). EBX = VM context; PC at +0x08, code base at
// +0x2C. Logs one line per interpreted instruction -- VERY high volume.
void FmtOpcode(const HookDescriptor&, const RegBank& rb, const uint32_t*) {
    uint32_t pc = SafeRead32(rb.ebx + 0x08, 0xFFFFFFFF);
    uint32_t codeBase = SafeRead32(rb.ebx + 0x2C, 0);
    uint32_t opcode = codeBase ? SafeRead32(codeBase + pc, 0xFFFFFFFF) : 0xFFFFFFFF;
    LogLine("bci.op", "ctx=%08X pc=%05X op=%u", rb.ebx, pc, opcode);
}

// Verified original bytes for the faction selector (exe-functions.md).
const uint8_t kFactionSig[] = {0x53, 0x8B, 0x5C, 0x24, 0x08, 0x53, 0xE8,
                               0x25, 0x3B, 0xFE, 0xFF, 0x83, 0xC4, 0x04,
                               0x53, 0x89, 0x1D, 0x78, 0x74, 0x73, 0x00};
const char kFactionMask[] = "xxxxxx?????xxxxxxx?xxx";  // wildcard the E8 rel32

HookDescriptor g_targets[8];
int g_targetCount = 0;

void Add(const char* name, uintptr_t va, HitFormatter fmt, int argc,
         const uint8_t* sig = nullptr, const char* mask = nullptr) {
    HookDescriptor& d = g_targets[g_targetCount++];
    d = HookDescriptor{};
    d.name = name;
    d.target = Va(va);
    d.formatter = fmt;
    d.argCount = argc;
    d.signature = sig;
    d.signatureMask = mask;
}

}  // namespace

// Called by the generated stub for every hook hit.
extern "C" void __cdecl ArgmOnHookHit(const HookDescriptor* desc, RegBank* rb) {
    if (!desc || !desc->formatter) return;
    const uint32_t* callerStack = reinterpret_cast<const uint32_t*>(
        reinterpret_cast<uint8_t*>(rb) + sizeof(RegBank) + 4 /* eflags */);
    __try {
        desc->formatter(*desc, *rb, callerStack);
    } __except (EXCEPTION_EXECUTE_HANDLER) {
        // A formatter fault must never take down the game.
    }
}

bool CheckBuildFingerprint(const Config& cfg) {
    g_expectedTimeDateStamp = cfg.expectedTimeDateStamp;

    HMODULE mod = GetModuleHandleW(nullptr);
    g_actualBase = reinterpret_cast<uintptr_t>(mod);

    auto* dos = reinterpret_cast<IMAGE_DOS_HEADER*>(mod);
    auto* nt = reinterpret_cast<IMAGE_NT_HEADERS*>(g_actualBase + dos->e_lfanew);
    uintptr_t preferred = nt->OptionalHeader.ImageBase;
    // The documented VAs are recorded relative to the stock 0x00400000 image
    // base, so rebase against that constant (not the PE's own ImageBase field).
    // For the stock EXE they are identical; the mismatch is logged if not.
    g_loadDelta = g_actualBase - kDocPreferredBase;
    if (preferred != kDocPreferredBase) {
        LogLine("build",
                "WARNING: PE ImageBase %08X != documented base %08X; documented "
                "addresses may not apply.",
                static_cast<uint32_t>(preferred),
                static_cast<uint32_t>(kDocPreferredBase));
    }

    uint32_t tds = nt->FileHeader.TimeDateStamp;
    uint32_t sizeOfImage = nt->OptionalHeader.SizeOfImage;
    uint32_t entry = nt->OptionalHeader.AddressOfEntryPoint;
    const uint8_t* ep = reinterpret_cast<const uint8_t*>(g_actualBase + entry);

    LogRaw("");
    LogLine("build",
            "module=%08X preferred=%08X delta=%08X TimeDateStamp=%08X "
            "SizeOfImage=%08X entry=%08X",
            static_cast<uint32_t>(g_actualBase), static_cast<uint32_t>(preferred),
            static_cast<uint32_t>(g_loadDelta), tds, sizeOfImage, entry);
    LogLine("build",
            "entry bytes: %02X %02X %02X %02X %02X %02X %02X %02X %02X %02X %02X %02X",
            ep[0], ep[1], ep[2], ep[3], ep[4], ep[5], ep[6], ep[7], ep[8], ep[9],
            ep[10], ep[11]);

    if (g_expectedTimeDateStamp == 0) {
        LogLine("build",
                "No expectedTimeDateStamp configured. Record the TimeDateStamp "
                "above into argm_trace.ini [build] to lock hooks to this build.");
        // No expected fingerprint set: only the individually signature-verified
        // hooks are installed (see InstallAllHooks). Address-only hooks require
        // an explicit build lock.
        return true;
    }

    if (tds != g_expectedTimeDateStamp) {
        LogLine("build",
                "BUILD MISMATCH: expected TimeDateStamp %08X, got %08X. "
                "Skipping all address-based hooks.",
                static_cast<uint32_t>(g_expectedTimeDateStamp), tds);
        return false;
    }
    LogLine("build", "Build fingerprint verified.");
    return true;
}

void InstallAllHooks(const Config& cfg) {
    // Whether unsigned (address-only) targets may be installed. They require an
    // explicit build lock via expectedTimeDateStamp; without it only the
    // signature-verified faction hook is installed.
    bool buildLocked = g_expectedTimeDateStamp != 0;

    g_targetCount = 0;

    // Signature-verified: always safe to attempt regardless of build lock,
    // because a wrong build fails the signature check and is skipped.
    Add("faction", 0x0045BD60, FmtFaction, 1, kFactionSig, kFactionMask);

    if (buildLocked) {
        if (cfg.traceNpcJobs) Add("npcjob", 0x00547F50, FmtNpcJob, 9);
        if (cfg.traceNpcActive) Add("npcactive", 0x00548CE0, FmtNpcActive, 2);
        if (cfg.traceVillage) Add("village", 0x00549500, FmtVillage, 3);
        if (cfg.traceCreateUnit) Add("createunit", 0x0052A020, FmtCreateUnit, 3);
        if (cfg.traceOpcodes) Add("bciop", 0x005B1C62, FmtOpcode, 0);
    } else {
        LogLine("hook",
                "Build not locked: installing only signature-verified hooks. "
                "Set [build] expectedTimeDateStamp to enable AI-event hooks.");
    }

    int ok = 0;
    for (int i = 0; i < g_targetCount; i++) {
        if (InstallDetour(g_targets[i])) ok++;
    }
    LogLine("hook", "installed %d of %d hooks", ok, g_targetCount);
}

}  // namespace argm
