// Minimal Win32 shim over POSIX so the REAL native/argm-trace/src/detour.cpp
// can be compiled and executed on 32-bit Linux for a mechanism test.
// Only the symbols detour.cpp actually uses are provided.
#pragma once
#include <cstdint>
#include <cstddef>
#include <cstring>
#include <sys/mman.h>
#include <unistd.h>

// MSVC calling-convention keyword -> GCC attribute (32-bit only).
#ifndef __cdecl
#define __cdecl __attribute__((__cdecl__))
#endif

typedef uint32_t DWORD;
typedef int BOOL;
typedef void* LPVOID;
typedef size_t SIZE_T;
typedef void* HANDLE;

#define PAGE_EXECUTE_READWRITE 0x40
#define PAGE_READWRITE 0x04
#define MEM_COMMIT 0x1000
#define MEM_RESERVE 0x2000
#define MEM_RELEASE 0x8000

inline HANDLE GetCurrentProcess() { return (HANDLE)(intptr_t)-1; }

inline LPVOID VirtualAlloc(LPVOID, SIZE_T size, DWORD, DWORD) {
    void* p = mmap(nullptr, size, PROT_READ | PROT_WRITE | PROT_EXEC,
                   MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    return (p == MAP_FAILED) ? nullptr : p;
}

inline BOOL VirtualFree(LPVOID, SIZE_T, DWORD) { return 1; }  // leak: fine for a test

inline BOOL VirtualProtect(LPVOID addr, SIZE_T size, DWORD, DWORD* oldProt) {
    if (oldProt) *oldProt = PAGE_EXECUTE_READWRITE;
    long pg = sysconf(_SC_PAGESIZE);
    uintptr_t start = reinterpret_cast<uintptr_t>(addr);
    uintptr_t alignedStart = start & ~(uintptr_t)(pg - 1);
    uintptr_t end = start + size;
    size_t len = end - alignedStart;
    return mprotect(reinterpret_cast<void*>(alignedStart), len,
                    PROT_READ | PROT_WRITE | PROT_EXEC) == 0;
}

inline void FlushInstructionCache(HANDLE, LPVOID addr, SIZE_T size) {
    char* p = reinterpret_cast<char*>(addr);
    __builtin___clear_cache(p, p + size);
}
