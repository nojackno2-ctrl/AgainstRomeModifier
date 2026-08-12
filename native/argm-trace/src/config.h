// argm-trace: runtime configuration, read from argm_trace.ini next to the game
// executable. Every switch defaults to the safe value so a bare install with
// no .ini still produces a useful AI-event log without the high-volume or
// higher-risk hooks.
#pragma once

#include <cstdint>

namespace argm {

struct Config {
    // Master switch. When false the DLL loads, forwards winmm.dll, and does
    // nothing else.
    bool enabled = true;

    // Verify the documented byte signature at each target before hooking it.
    // Strongly recommended: leave on. When a signature does not match (wrong
    // game build) the individual hook is skipped instead of corrupting code.
    bool verifySignatures = true;

    // Master gate for ALL inline-hook installation. When false (the safe default
    // for a first bring-up), the DLL loads, forwards winmm, opens the log and
    // records the build banner, but installs NO hooks at all -- it never touches
    // game code, so it cannot destabilize startup. Set to true only after the
    // log-only mode is confirmed to load and run the game cleanly.
    bool enableHooks = false;

    // --- Event hooks (low frequency, safe defaults ON) ---
    bool traceNpcJobs = true;      // s_addNPCJob_createUnit implementation
    bool traceNpcActive = true;    // s_setNPCActive (respawn eligibility writes)
    bool traceVillage = true;      // s_setVillageTemplate
    bool traceCreateUnit = true;   // s_createUnitAndMems

    // --- Endless-AI decision-context hooks (low frequency, safe defaults ON) ---
    // These record the *inputs* and *boundaries* of the AI reinforcement logic,
    // complementing the action hooks above so a "why didn't the AI act" question
    // can be answered from the log alone.
    bool traceNpcQuery = true;     // s_NPCActive getter: snapshots all 8 teams' respawn flags
    bool traceLevelInit = true;    // level-init sweep: marks a fresh session boundary
    bool traceUnitMax = true;      // s_createBattleUnitsMax / s_createCiviUnitsMax count clamps

    // --- Full BCI opcode stream (VERY high volume, OFF by default) ---
    // Logs every interpreted script instruction. Only enable for short,
    // targeted captures; it can produce hundreds of MB per minute.
    bool traceOpcodes = false;

    // Number of caller-stack dwords to dump for generic event hooks.
    int argDumpCount = 9;

    // When > 0, stop appending after this many megabytes to protect the disk.
    int maxLogMegabytes = 256;

    // PE TimeDateStamp of the reverse-engineered game build. When 0 the address
    // based hooks (everything except the signature-verified faction hook) stay
    // disabled, because the documented addresses are only valid for the exact
    // analyzed build. Set this once, after reading the value argm-trace logs in
    // its [build] banner, to unlock the AI-event hooks.
    uint32_t expectedTimeDateStamp = 0;
};

// Loads argm_trace.ini from the given directory. Missing file or missing keys
// fall back to the defaults above.
Config LoadConfig(const wchar_t* directory);

}  // namespace argm
