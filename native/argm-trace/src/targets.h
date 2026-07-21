// argm-trace: reverse-engineered hook targets and their log formatters.
#pragma once

#include "config.h"

namespace argm {

// Reads the main module's PE fingerprint (image base, timestamp, size, entry
// bytes) and logs it. Returns false if the loaded build does not match the
// configured expected fingerprint (when one is set), meaning no address-based
// hook should be installed.
bool CheckBuildFingerprint(const Config& cfg);

// Installs every hook enabled in cfg, rebasing the documented addresses for the
// actual load address. Safe to call once after CheckBuildFingerprint.
void InstallAllHooks(const Config& cfg);

}  // namespace argm
