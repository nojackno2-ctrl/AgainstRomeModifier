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

// __try-guarded byte read; returns fallback on access violation.
uint8_t SafeRead8(uintptr_t addr, uint8_t fallback) {
    __try {
        return *reinterpret_cast<volatile uint8_t*>(addr);
    } __except (EXCEPTION_EXECUTE_HANDLER) {
        return fallback;
    }
}

// __try-guarded best-effort C-string copy out of game memory. Non-printable
// bytes become '.'; returns "?" if the pointer faults immediately.
const char* SafeReadStr(uint32_t addr, char* buf, size_t cap) {
    __try {
        const char* s = reinterpret_cast<const char*>(addr);
        size_t i = 0;
        for (; i + 1 < cap && s[i]; i++) {
            buf[i] = (s[i] >= 0x20 && s[i] < 0x7F) ? s[i] : '.';
        }
        buf[i] = 0;
        return buf;
    } __except (EXCEPTION_EXECUTE_HANDLER) {
        return "?";
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
// Byte-verified prologue: arg1 -> EBX is the team (validated 0..7), arg2 -> EBP
// is a pointer checked non-null (the template name per exe-functions.md).
void FmtVillage(const HookDescriptor&, const RegBank&, const uint32_t* cs) {
    char name[64];
    LogLine("ai.village",
            "setVillageTemplate team=%u tmpl=%08X \"%s\" a3=%08X ret=%08X",
            cs[1], cs[2], SafeReadStr(cs[2], name, sizeof(name)), cs[3], cs[0]);
}

// s_createUnitAndMems callback (0052A020).
void FmtCreateUnit(const HookDescriptor&, const RegBank&, const uint32_t* cs) {
    LogLine("ai.unit", "createUnitAndMems a1=%08X a2=%08X a3=%08X ret=%08X",
            cs[1], cs[2], cs[3], cs[0]);
}

// Per-team respawn-eligibility flag array DAT_029e6000 (8 bytes, one per team).
// Documented in exe-functions.md / endless-mode-ai.md; rebased for ASLR.
constexpr uintptr_t kNpcActiveArrayVa = 0x029E6000;

// s_NPCActive getter (00548D20): reads DAT_029e6000[team] and returns it. The
// getter has no side effects, so at hook entry the array already holds the
// value it will return -- we snapshot ALL eight teams at once. This is the
// single most useful "why isn't team N reinforcing" datum: the endless party
// state machine gates type-4 reinforcement on this flag.
void FmtNpcActiveGet(const HookDescriptor&, const RegBank&, const uint32_t* cs) {
    uint32_t team = cs[1];
    uintptr_t arr = kNpcActiveArrayVa + g_loadDelta;
    uint8_t s[8];
    for (int i = 0; i < 8; i++) s[i] = SafeRead8(arr + i, 0xFF);
    unsigned active = team < 8 ? s[team] : 0xFF;
    LogLine("ai.query",
            "NPCActive team=%u -> active=%u | all[t0=%u t1=%u t2=%u t3=%u "
            "t4=%u t5=%u t6=%u t7=%u] ret=%08X",
            team, active, s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7], cs[0]);
}

// Level-init sweep (0054A070): zeroes DAT_029e6000[0..7] and other per-team
// arrays once per level load. Logged as a fresh-session boundary -- every
// endless behavior report must come from a new session, so this line is the
// anchor that separates one run's events from the next in a single log file.
void FmtLevelInit(const HookDescriptor&, const RegBank&, const uint32_t* cs) {
    LogLine("ai.level",
            "===== level-init: per-team NPC state reset (NEW SESSION BOUNDARY) "
            "ret=%08X =====", cs[0]);
}

// s_createBattleUnitsMax impl (005249D0): the count argument is clamped to
// <=20. Records the requested count before the clamp so an "AI wanted N but got
// 20" or "requested 0" situation is visible.
void FmtBattleMax(const HookDescriptor&, const RegBank&, const uint32_t* cs) {
    LogLine("ai.unitmax",
            "createBattleUnitsMax req a1=%u a2=%u a3=%u (clamp<=20) ret=%08X",
            cs[1], cs[2], cs[3], cs[0]);
}

// s_createCiviUnitsMax impl (00524D70): civilian variant of the above.
void FmtCiviMax(const HookDescriptor&, const RegBank&, const uint32_t* cs) {
    LogLine("ai.unitmax",
            "createCiviUnitsMax req a1=%u a2=%u a3=%u (clamp<=20) ret=%08X",
            cs[1], cs[2], cs[3], cs[0]);
}

// Endless Roman faction selector setter (0045BD60). Verified 21-byte
// signature. arg1 = requested faction id.
void FmtFaction(const HookDescriptor&, const RegBank&, const uint32_t* cs) {
    LogLine("game.faction", "factionSelect requested=%u ret=%08X", cs[1], cs[0]);
}

// Same setter on an install carrying the modifier's force-Roman EXE patch
// (`8B5C2408` -> `6A035B90`): the function ignores its argument and uses 3.
// cs[1] still shows what dlg_volk actually requested.
void FmtFactionForced(const HookDescriptor&, const RegBank&, const uint32_t* cs) {
    LogLine("game.faction",
            "factionSelect requested=%u effective=3 (force-Roman patch) ret=%08X",
            cs[1], cs[0]);
}

// BCI VM dispatcher, true function entry 005B1C60 (`53 56 57 55 89 E5 ...`).
// Byte-verified: the VM context is stack ARGUMENT 1 (`mov ebx,[ebp+0x14]`
// happens only later, at 005B1C6F), so at entry EBX still holds the caller's
// value and must NOT be used. PC at ctx+0x08, code length at ctx+0x28, code
// base at ctx+0x2C. Logs one line per interpreted instruction -- VERY high
// volume.
void FmtOpcode(const HookDescriptor&, const RegBank&, const uint32_t* cs) {
    uint32_t ctx = cs[1];
    uint32_t pc = SafeRead32(ctx + 0x08, 0xFFFFFFFF);
    uint32_t codeBase = SafeRead32(ctx + 0x2C, 0);
    uint32_t opcode = codeBase ? SafeRead32(codeBase + pc, 0xFFFFFFFF) : 0xFFFFFFFF;
    LogLine("bci.op", "ctx=%08X pc=%05X op=%u", ctx, pc, opcode);
}

// ---- Byte signatures (dumped from the analyzed EXE, TimeDateStamp 404D1710,
// via the PE section table; see runtime-trace-hooks.md) --------------------

// Stock faction selector prologue (exe-functions.md). rel32 of the E8 and the
// abs32 of the `mov ds:[..],ebx` are wildcarded so the signature keys on the
// instruction skeleton, not on link-time addresses.
const uint8_t kFactionSig[] = {0x53, 0x8B, 0x5C, 0x24, 0x08, 0x53, 0xE8,
                               0x25, 0x3B, 0xFE, 0xFF, 0x83, 0xC4, 0x04,
                               0x53, 0x89, 0x1D, 0x78, 0x74, 0x73, 0x00};
const char kFactionMask[] = "xxxxxxx????xxxxxx????";

// Faction selector with the force-Roman patch applied (known-patches.md:
// `53 8B5C2408` -> `53 6A03 5B 90`). Tail is identical to stock.
const uint8_t kFactionForcedSig[] = {0x53, 0x6A, 0x03, 0x5B, 0x90, 0x53, 0xE8,
                                     0x25, 0x3B, 0xFE, 0xFF, 0x83, 0xC4, 0x04,
                                     0x53, 0x89, 0x1D, 0x78, 0x74, 0x73, 0x00};
const char kFactionForcedMask[] = "xxxxxxx????xxxxxx????";

// s_addNPCJob_createUnit impl 00547F50: push ebx/esi/edi/ebp then load
// arg1 (team), arg8, arg9 from the caller stack.
const uint8_t kNpcJobSig[] = {0x53, 0x56, 0x57, 0x55, 0x8B, 0x5C, 0x24, 0x14,
                              0x8B, 0x7C, 0x24, 0x30, 0x8B, 0x6C, 0x24, 0x34};
const char kNpcJobMask[] = "xxxxxxxxxxxxxxxx";

// s_setNPCActive impl 00548CE0: no pushed regs; validates team then writes
// DAT_029e6000[team]. Includes the early-out tail (fixed rel8s, same build).
const uint8_t kNpcActiveSig[] = {0x8B, 0x54, 0x24, 0x04, 0x85, 0xD2, 0x7C,
                                 0x05, 0x83, 0xFA, 0x08, 0x7C, 0x06, 0xB8,
                                 0xFF, 0xFF, 0xFF, 0xFF, 0xC3};
const char kNpcActiveMask[] = "xxxxxxxxxxxxxxxxxxx";

// s_setVillageTemplate impl 00549500: team -> EBX, template ptr -> EBP.
const uint8_t kVillageSig[] = {0x53, 0x56, 0x57, 0x55, 0x8B, 0x5C, 0x24, 0x14,
                               0x8B, 0x6C, 0x24, 0x18, 0x31, 0xFF, 0x85, 0xED};
const char kVillageMask[] = "xxxxxxxxxxxxxxxx";

// s_createUnitAndMems callback 0052A020: reads its 9-int arg block off arg1.
const uint8_t kCreateUnitSig[] = {0x53, 0x56, 0x57, 0x55, 0x83, 0xEC, 0x30,
                                  0x8B, 0x44, 0x24, 0x44, 0x8B, 0x70, 0x08};
const char kCreateUnitMask[] = "xxxxxxxxxxxxxx";

// BCI dispatcher entry 005B1C60: pushes, mov ebp,esp, sub esp,0x6A8,
// and esp,-8, then ctx/PC/limit loads.
const uint8_t kBciSig[] = {0x53, 0x56, 0x57, 0x55, 0x89, 0xE5, 0x81, 0xEC,
                           0xA8, 0x06, 0x00, 0x00, 0x83, 0xE4, 0xF8, 0x8B,
                           0x5D, 0x14, 0x8B, 0x53, 0x08, 0x8B, 0x4B, 0x28};
const char kBciMask[] = "xxxxxxxxxxxxxxxxxxxxxxxx";

// s_NPCActive getter 00548D20: arg1 team at [esp+4], reads DAT_029e6000[team].
const uint8_t kNpcQuerySig[] = {0x8B, 0x44, 0x24, 0x04, 0x85, 0xC0, 0x7C, 0x14,
                                0x83, 0xF8, 0x08, 0x7D, 0x0F, 0x80, 0xB8};
const char kNpcQueryMask[] = "xxxxxxxxxxxxxxx";

// Level-init sweep 0054A070: 4 pushes, sub esp,0x24, mov ebx,<npc array>.
const uint8_t kLevelInitSig[] = {0x53, 0x56, 0x57, 0x55, 0x83, 0xEC, 0x24,
                                 0xBB, 0x38, 0x91, 0x9C, 0x02};
const char kLevelInitMask[] = "xxxxxxxxxxxx";

// s_createBattleUnitsMax impl 005249D0: 4 pushes, sub esp,0x0C, arg loads.
const uint8_t kBattleMaxSig[] = {0x53, 0x56, 0x57, 0x55, 0x83, 0xEC, 0x0C,
                                 0x8B, 0x54, 0x24, 0x24};
const char kBattleMaxMask[] = "xxxxxxxxxxx";

// s_createCiviUnitsMax impl 00524D70: 4 pushes, sub esp,0x08, arg loads.
const uint8_t kCiviMaxSig[] = {0x53, 0x56, 0x57, 0x55, 0x83, 0xEC, 0x08,
                               0x8B, 0x6C, 0x24, 0x24};
const char kCiviMaxMask[] = "xxxxxxxxxxx";

HookDescriptor g_targets[16];
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
    // because a wrong build fails the signature check and is skipped. Two
    // variants cover both a stock install and one carrying the modifier's
    // force-Roman patch; at most one signature can match.
    Add("faction", 0x0045BD60, FmtFaction, 1, kFactionSig, kFactionMask);
    Add("factionF", 0x0045BD60, FmtFactionForced, 1, kFactionForcedSig,
        kFactionForcedMask);

    if (buildLocked) {
        // Build-locked AND byte-signature-verified: both checks must pass.
        if (cfg.traceNpcJobs)
            Add("npcjob", 0x00547F50, FmtNpcJob, 9, kNpcJobSig, kNpcJobMask);
        if (cfg.traceNpcActive)
            Add("npcactive", 0x00548CE0, FmtNpcActive, 2, kNpcActiveSig,
                kNpcActiveMask);
        if (cfg.traceVillage)
            Add("village", 0x00549500, FmtVillage, 3, kVillageSig, kVillageMask);
        if (cfg.traceCreateUnit)
            Add("createunit", 0x0052A020, FmtCreateUnit, 3, kCreateUnitSig,
                kCreateUnitMask);
        if (cfg.traceNpcQuery)
            Add("npcquery", 0x00548D20, FmtNpcActiveGet, 1, kNpcQuerySig,
                kNpcQueryMask);
        if (cfg.traceLevelInit)
            Add("levelinit", 0x0054A070, FmtLevelInit, 0, kLevelInitSig,
                kLevelInitMask);
        if (cfg.traceUnitMax) {
            Add("battlemax", 0x005249D0, FmtBattleMax, 3, kBattleMaxSig,
                kBattleMaxMask);
            Add("civimax", 0x00524D70, FmtCiviMax, 3, kCiviMaxSig, kCiviMaxMask);
        }
        if (cfg.traceOpcodes)
            Add("bciop", 0x005B1C60, FmtOpcode, 0, kBciSig, kBciMask);
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
