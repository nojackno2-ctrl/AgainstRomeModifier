// argm-trace: runtime configuration, read from argm_trace.ini next to the game
// executable. Every switch defaults to the safe value so a bare install with
// no .ini still produces a useful AI-event log without the high-volume or
// higher-risk hooks.
#pragma once

#include <cstdint>

namespace argm {

struct Config {
    // Master switch. When false the DLL loads, forwards version.dll, and does
    // nothing else.
    bool enabled = true;

    // Verify the documented byte signature at each target before hooking it.
    // Strongly recommended: leave on. When a signature does not match (wrong
    // game build) the individual hook is skipped instead of corrupting code.
    bool verifySignatures = true;

    // --- Event hooks (low frequency, safe defaults ON) ---
    bool traceNpcJobs = true;      // s_addNPCJob_createUnit implementation
    bool traceNpcActive = true;    // s_setNPCActive / s_NPCActive
    bool traceVillage = true;      // s_setVillageTemplate
    bool traceCreateUnit = true;   // s_createUnitAndMems

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
