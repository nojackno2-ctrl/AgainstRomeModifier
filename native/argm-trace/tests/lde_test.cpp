// Host unit test for argm::DecodeLength (native/argm-trace/src/lde.h).
//
// The trampoline builder relies on this decoder to size a function prologue
// correctly; a wrong length would relocate a partial instruction and crash the
// game. This test runs on any host (no <windows.h> needed):
//
//   g++ -std=c++17 -Wall -Wextra -o lde_test lde_test.cpp && ./lde_test
//
// Exit code 0 = all pass.
#include "../src/lde.h"

#include <cstdint>
#include <cstdio>

using argm::DecodeLength;

namespace {

struct Case {
    const char* name;
    uint8_t bytes[16];
    size_t expect;
};

// True if the opcode at `code` is IP-relative (mirror of detour.cpp::IsRelative).
bool IsRelative(const uint8_t* code) {
    uint8_t op = code[0];
    if (op == 0xE8 || op == 0xE9 || op == 0xEB) return true;
    if (op >= 0x70 && op <= 0x7F) return true;
    if (op >= 0xE0 && op <= 0xE3) return true;
    if (op == 0x0F && code[1] >= 0x80 && code[1] <= 0x8F) return true;
    return false;
}

}  // namespace

int main() {
    const Case cases[] = {
        {"push ebx", {0x53}, 1},
        {"push ebp", {0x55}, 1},
        {"mov ebx,[esp+8]", {0x8B, 0x5C, 0x24, 0x08}, 4},
        {"mov edi,edi", {0x8B, 0xFF}, 2},
        {"mov ebp,esp", {0x8B, 0xEC}, 2},
        {"mov eax,[esp+4]", {0x8B, 0x44, 0x24, 0x04}, 4},
        {"sub esp,0x10", {0x83, 0xEC, 0x10}, 3},
        {"sub esp,0x100", {0x81, 0xEC, 0x00, 0x01, 0x00, 0x00}, 6},
        {"mov eax,imm32", {0xB8, 0x44, 0x33, 0x22, 0x11}, 5},
        {"push imm32", {0x68, 0x44, 0x33, 0x22, 0x11}, 5},
        {"push imm8", {0x6A, 0x04}, 2},
        {"xor eax,eax", {0x33, 0xC0}, 2},
        {"mov [esp+4],ebx", {0x89, 0x5C, 0x24, 0x04}, 4},
        {"lea eax,[esp+8]", {0x8D, 0x44, 0x24, 0x08}, 4},
        {"mov ecx,ds:[imm32]", {0x8B, 0x0D, 0x78, 0x74, 0x73, 0x00}, 6},
        {"mov ds:[imm32],ebx", {0x89, 0x1D, 0x78, 0x74, 0x73, 0x00}, 6},
        {"ret", {0xC3}, 1},
        {"ret imm16", {0xC2, 0x08, 0x00}, 3},
        {"retf imm16", {0xCA, 0x08, 0x00}, 3},
        {"cmp eax,[ebx+0x28]", {0x3B, 0x43, 0x28}, 3},
        {"add esp,4", {0x83, 0xC4, 0x04}, 3},
        {"nop", {0x90}, 1},
        {"movzx eax,byte[edx]", {0x0F, 0xB6, 0x02}, 3},
        {"movzx eax,byte[edx+1]", {0x0F, 0xB6, 0x42, 0x01}, 4},
        {"test al,imm8", {0xA8, 0x01}, 2},
        {"test eax,imm32", {0xA9, 0x01, 0x00, 0x00, 0x00}, 5},
        {"call rel32", {0xE8, 0x00, 0x10, 0x00, 0x00}, 5},
        {"jmp rel32", {0xE9, 0x00, 0x10, 0x00, 0x00}, 5},
        {"jmp rel8", {0xEB, 0x10}, 2},
        {"jz rel8", {0x74, 0x10}, 2},
        {"jnz rel32", {0x0F, 0x85, 0x00, 0x10, 0x00, 0x00}, 6},
        {"mov eax,[eax*4+imm32]", {0x8B, 0x04, 0x85, 0x00, 0x00, 0x00, 0x00}, 7},
        {"mov r/m,imm32(C7)", {0xC7, 0x45, 0xFC, 0x00, 0x00, 0x00, 0x00}, 7},
        {"test r/m,imm32(F7/0)", {0xF7, 0xC0, 0x01, 0x00, 0x00, 0x00}, 6},
        {"neg eax (F7/3)", {0xF7, 0xD8}, 2},
        {"inc dword[eax](FF/0)", {0xFF, 0x00}, 2},
        {"call [imm32](FF/2)", {0xFF, 0x15, 0x00, 0x00, 0x00, 0x00}, 6},
        {"mov edi,edi(prefix66)", {0x66, 0x8B, 0xFF}, 3},
        {"push imm16(66 68)", {0x66, 0x68, 0x00, 0x10}, 4},
        {"enter", {0xC8, 0x10, 0x00, 0x00}, 4},
        {"setne al", {0x0F, 0x95, 0xC0}, 3},
        {"cmovz eax,ecx", {0x0F, 0x44, 0xC1}, 3},
        {"imul eax,ecx,imm8", {0x6B, 0xC1, 0x04}, 3},
        {"and esp,-16(83)", {0x83, 0xE4, 0xF0}, 3},
        {"mov ebp,esp(89E5)", {0x89, 0xE5}, 2},
        {"pop ebx", {0x5B}, 1},
        {"xor edi,edi(31FF)", {0x31, 0xFF}, 2},
    };

    int fail = 0;
    for (const auto& c : cases) {
        size_t got = DecodeLength(c.bytes);
        bool ok = got == c.expect;
        if (!ok) fail++;
        printf("[%s] %-26s expect=%zu got=%zu\n", ok ? "ok " : "FAIL", c.name,
               c.expect, got);
    }

    // Every hook-target prologue (bytes dumped from the analyzed EXE,
    // TimeDateStamp 404D1710) must yield >= 5 stolen bytes from whole
    // instructions with no relative branch in that region.
    struct Prologue {
        const char* name;
        uint8_t bytes[24];
    };
    const Prologue prologues[] = {
        {"faction@45BD60", {0x53, 0x8B, 0x5C, 0x24, 0x08, 0x53, 0xE8,
                            0x25, 0x3B, 0xFE, 0xFF}},
        {"factionForced@45BD60", {0x53, 0x6A, 0x03, 0x5B, 0x90, 0x53, 0xE8,
                                  0x25, 0x3B, 0xFE, 0xFF}},
        {"npcjob@547F50", {0x53, 0x56, 0x57, 0x55, 0x8B, 0x5C, 0x24, 0x14,
                           0x8B, 0x7C, 0x24, 0x30}},
        {"npcactive@548CE0", {0x8B, 0x54, 0x24, 0x04, 0x85, 0xD2, 0x7C, 0x05}},
        {"village@549500", {0x53, 0x56, 0x57, 0x55, 0x8B, 0x5C, 0x24, 0x14,
                            0x8B, 0x6C, 0x24, 0x18}},
        {"createunit@52A020", {0x53, 0x56, 0x57, 0x55, 0x83, 0xEC, 0x30,
                               0x8B, 0x44, 0x24, 0x44}},
        {"bciop@5B1C60", {0x53, 0x56, 0x57, 0x55, 0x89, 0xE5, 0x81, 0xEC,
                          0xA8, 0x06, 0x00, 0x00}},
    };
    for (const auto& p : prologues) {
        size_t stolen = 0;
        bool rel = false;
        bool undec = false;
        while (stolen < 5) {
            const uint8_t* h = p.bytes + stolen;
            if (IsRelative(h)) { rel = true; break; }
            size_t l = DecodeLength(h);
            if (l == 0) { undec = true; break; }
            stolen += l;
        }
        bool ok = stolen >= 5 && !rel && !undec;
        printf("[%s] prologue %-22s stolen=%zu rel=%d undec=%d\n",
               ok ? "ok " : "FAIL", p.name, stolen, rel ? 1 : 0, undec ? 1 : 0);
        if (!ok) fail++;
    }

    printf("\n%s (%d failures)\n", fail ? "FAILURES" : "ALL PASS", fail);
    return fail ? 1 : 0;
}
