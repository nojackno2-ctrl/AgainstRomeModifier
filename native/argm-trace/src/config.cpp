#include "config.h"

#include <windows.h>

#include <cstdio>
#include <cstdlib>
#include <cwchar>

namespace argm {
namespace {

bool ReadBool(const wchar_t* section, const wchar_t* key, bool fallback,
              const wchar_t* file) {
    int v = GetPrivateProfileIntW(section, key, fallback ? 1 : 0, file);
    return v != 0;
}

int ReadInt(const wchar_t* section, const wchar_t* key, int fallback,
            const wchar_t* file) {
    return GetPrivateProfileIntW(section, key, fallback, file);
}

}  // namespace

Config LoadConfig(const wchar_t* directory) {
    Config c;

    wchar_t file[MAX_PATH];
    if (directory && directory[0]) {
        _snwprintf_s(file, MAX_PATH, _TRUNCATE, L"%s\\argm_trace.ini", directory);
    } else {
        wcscpy_s(file, MAX_PATH, L"argm_trace.ini");
    }

    // No .ini present -> keep all defaults (produces the default AI-event log).
    if (GetFileAttributesW(file) == INVALID_FILE_ATTRIBUTES) return c;

    c.enabled = ReadBool(L"general", L"enabled", c.enabled, file);
    c.verifySignatures =
        ReadBool(L"general", L"verifySignatures", c.verifySignatures, file);
    c.maxLogMegabytes = ReadInt(L"general", L"maxLogMegabytes", c.maxLogMegabytes, file);
    c.argDumpCount = ReadInt(L"general", L"argDumpCount", c.argDumpCount, file);
    if (c.argDumpCount < 0) c.argDumpCount = 0;
    if (c.argDumpCount > 16) c.argDumpCount = 16;

    c.traceNpcJobs = ReadBool(L"hooks", L"traceNpcJobs", c.traceNpcJobs, file);
    c.traceNpcActive = ReadBool(L"hooks", L"traceNpcActive", c.traceNpcActive, file);
    c.traceVillage = ReadBool(L"hooks", L"traceVillage", c.traceVillage, file);
    c.traceCreateUnit = ReadBool(L"hooks", L"traceCreateUnit", c.traceCreateUnit, file);
    c.traceNpcQuery = ReadBool(L"hooks", L"traceNpcQuery", c.traceNpcQuery, file);
    c.traceLevelInit = ReadBool(L"hooks", L"traceLevelInit", c.traceLevelInit, file);
    c.traceUnitMax = ReadBool(L"hooks", L"traceUnitMax", c.traceUnitMax, file);
    c.traceOpcodes = ReadBool(L"hooks", L"traceOpcodes", c.traceOpcodes, file);

    // TimeDateStamp is stored as a hex string (e.g. "3F1A2B3C"); parse manually
    // because GetPrivateProfileInt only reads decimal.
    wchar_t tds[32] = {};
    GetPrivateProfileStringW(L"build", L"expectedTimeDateStamp", L"0", tds, 32, file);
    c.expectedTimeDateStamp = static_cast<uint32_t>(wcstoul(tds, nullptr, 16));

    return c;
}

}  // namespace argm
