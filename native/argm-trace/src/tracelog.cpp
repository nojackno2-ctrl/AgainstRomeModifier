#include "tracelog.h"

#include <windows.h>

#include <cstdio>
#include <cstring>

namespace argm {
namespace {

HANDLE g_file = INVALID_HANDLE_VALUE;
CRITICAL_SECTION g_lock;
bool g_ready = false;
LARGE_INTEGER g_qpcFreq{};
LARGE_INTEGER g_qpcStart{};

// One shared scratch buffer, only ever touched while g_lock is held.
char g_scratch[4096];

void WriteAll(const char* data, size_t len) {
    if (g_file == INVALID_HANDLE_VALUE || len == 0) return;
    DWORD written = 0;
    // A single WriteFile per line keeps interleaving from other processes'
    // handles impossible and is fast enough for this volume.
    WriteFile(g_file, data, static_cast<DWORD>(len), &written, nullptr);
}

// Milliseconds since LogOpen, monotonic, independent of wall-clock changes.
double ElapsedMs() {
    LARGE_INTEGER now;
    QueryPerformanceCounter(&now);
    if (g_qpcFreq.QuadPart == 0) return 0.0;
    return (now.QuadPart - g_qpcStart.QuadPart) * 1000.0 /
           static_cast<double>(g_qpcFreq.QuadPart);
}

}  // namespace

bool LogOpen(const wchar_t* directory) {
    if (g_ready) return true;

    wchar_t path[MAX_PATH];
    if (directory && directory[0]) {
        _snwprintf_s(path, MAX_PATH, _TRUNCATE, L"%s\\argm_trace.log", directory);
    } else {
        wcscpy_s(path, MAX_PATH, L"argm_trace.log");
    }

    g_file = CreateFileW(path, GENERIC_WRITE, FILE_SHARE_READ, nullptr,
                         CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (g_file == INVALID_HANDLE_VALUE) return false;

    // UTF-8 BOM so editors show the file as UTF-8 (log content is ASCII, but
    // this keeps it consistent with the repo's UTF-8 conventions).
    const unsigned char bom[] = {0xEF, 0xBB, 0xBF};
    DWORD w = 0;
    WriteFile(g_file, bom, sizeof(bom), &w, nullptr);

    InitializeCriticalSection(&g_lock);
    QueryPerformanceFrequency(&g_qpcFreq);
    QueryPerformanceCounter(&g_qpcStart);
    g_ready = true;
    return true;
}

void LogClose() {
    if (!g_ready) return;
    EnterCriticalSection(&g_lock);
    if (g_file != INVALID_HANDLE_VALUE) {
        FlushFileBuffers(g_file);
        CloseHandle(g_file);
        g_file = INVALID_HANDLE_VALUE;
    }
    g_ready = false;
    LeaveCriticalSection(&g_lock);
    DeleteCriticalSection(&g_lock);
}

void LogLineV(const char* category, const char* fmt, va_list args) {
    if (!g_ready) return;

    EnterCriticalSection(&g_lock);

    int n = _snprintf_s(g_scratch, sizeof(g_scratch), _TRUNCATE,
                        "[%11.3f][t%04lx][%-10s] ", ElapsedMs(),
                        GetCurrentThreadId(), category ? category : "");
    if (n < 0) n = static_cast<int>(strlen(g_scratch));

    int m = _vsnprintf_s(g_scratch + n, sizeof(g_scratch) - n, _TRUNCATE, fmt, args);
    if (m < 0) m = static_cast<int>(strlen(g_scratch + n));

    size_t len = static_cast<size_t>(n + m);
    if (len < sizeof(g_scratch) - 1) {
        g_scratch[len++] = '\n';
    } else {
        g_scratch[sizeof(g_scratch) - 2] = '\n';
        len = sizeof(g_scratch) - 1;
    }
    WriteAll(g_scratch, len);

    LeaveCriticalSection(&g_lock);
}

void LogLine(const char* category, const char* fmt, ...) {
    va_list args;
    va_start(args, fmt);
    LogLineV(category, fmt, args);
    va_end(args);
}

void LogRaw(const char* fmt, ...) {
    if (!g_ready) return;
    va_list args;
    va_start(args, fmt);
    EnterCriticalSection(&g_lock);
    int n = _vsnprintf_s(g_scratch, sizeof(g_scratch), _TRUNCATE, fmt, args);
    if (n < 0) n = static_cast<int>(strlen(g_scratch));
    size_t len = static_cast<size_t>(n);
    if (len < sizeof(g_scratch) - 1) g_scratch[len++] = '\n';
    WriteAll(g_scratch, len);
    LeaveCriticalSection(&g_lock);
    va_end(args);
}

}  // namespace argm
