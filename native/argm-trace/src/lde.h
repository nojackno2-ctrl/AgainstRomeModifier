// argm-trace: header-only 32-bit instruction length decoder.
//
// Kept free of <windows.h> so it can be unit-tested on any host (see
// native/argm-trace/tests/lde_test.cpp). Used only to size a function prologue
// for trampoline relocation. Any opcode it does not recognize returns 0, which
// makes the hook installer abort that hook -- an unknown or mis-decoded
// instruction fails safe instead of corrupting code.
#pragma once

#include <cstddef>
#include <cstdint>

namespace argm {
namespace lde_detail {

enum : uint8_t {
    F_NONE = 0,
    F_MODRM = 1 << 0,   // has ModR/M byte
    F_IMM8 = 1 << 1,    // + 1 imm byte
    F_IMM16 = 1 << 2,   // + 2 imm bytes
    F_IMMZ = 1 << 3,    // + operand-size imm (2 with 0x66, else 4)
    F_MOFFS = 1 << 4,   // + address-size imm (2 with 0x67, else 4)
    F_FARPTR = 1 << 5,  // + operand-size imm + 2 (segment)
    F_ENTER = 1 << 6,   // + imm16 + imm8
    F_BAD = 1 << 7,     // undecodable -> abort
};

inline const uint8_t* PrimaryTable() {
    static uint8_t t[256];
    static bool init = false;
    if (init) return t;
    for (int i = 0; i < 256; i++) t[i] = F_NONE;

    for (int base = 0x00; base <= 0x38; base += 0x08) {
        t[base + 0] = F_MODRM;
        t[base + 1] = F_MODRM;
        t[base + 2] = F_MODRM;
        t[base + 3] = F_MODRM;
        t[base + 4] = F_IMM8;
        t[base + 5] = F_IMMZ;
    }
    t[0x0F] = F_BAD;  // two-byte escape, intercepted before table lookup

    t[0x62] = F_MODRM;
    t[0x63] = F_MODRM;
    t[0x68] = F_IMMZ;
    t[0x69] = F_MODRM | F_IMMZ;
    t[0x6A] = F_IMM8;
    t[0x6B] = F_MODRM | F_IMM8;

    for (int i = 0x70; i <= 0x7F; i++) t[i] = F_IMM8;  // jcc rel8

    t[0x80] = F_MODRM | F_IMM8;
    t[0x81] = F_MODRM | F_IMMZ;
    t[0x82] = F_MODRM | F_IMM8;
    t[0x83] = F_MODRM | F_IMM8;
    for (int i = 0x84; i <= 0x8F; i++) t[i] = F_MODRM;

    t[0x9A] = F_FARPTR;

    t[0xA0] = F_MOFFS; t[0xA1] = F_MOFFS; t[0xA2] = F_MOFFS; t[0xA3] = F_MOFFS;
    t[0xA8] = F_IMM8;
    t[0xA9] = F_IMMZ;

    for (int i = 0xB0; i <= 0xB7; i++) t[i] = F_IMM8;
    for (int i = 0xB8; i <= 0xBF; i++) t[i] = F_IMMZ;

    t[0xC0] = F_MODRM | F_IMM8;
    t[0xC1] = F_MODRM | F_IMM8;
    t[0xC2] = F_IMM16;
    t[0xC4] = F_MODRM;
    t[0xC5] = F_MODRM;
    t[0xC6] = F_MODRM | F_IMM8;
    t[0xC7] = F_MODRM | F_IMMZ;
    t[0xC8] = F_ENTER;
    t[0xCA] = F_IMM16;
    t[0xCD] = F_IMM8;

    t[0xD0] = F_MODRM; t[0xD1] = F_MODRM; t[0xD2] = F_MODRM; t[0xD3] = F_MODRM;
    t[0xD4] = F_IMM8; t[0xD5] = F_IMM8;
    for (int i = 0xD8; i <= 0xDF; i++) t[i] = F_MODRM;  // x87

    for (int i = 0xE0; i <= 0xE3; i++) t[i] = F_IMM8;   // loop/jecxz rel8
    for (int i = 0xE4; i <= 0xE7; i++) t[i] = F_IMM8;   // in/out imm8
    t[0xE8] = F_IMMZ;
    t[0xE9] = F_IMMZ;
    t[0xEA] = F_FARPTR;
    t[0xEB] = F_IMM8;

    t[0xF6] = F_MODRM;
    t[0xF7] = F_MODRM;
    t[0xFE] = F_MODRM;
    t[0xFF] = F_MODRM;

    init = true;
    return t;
}

inline bool IsPrefix(uint8_t b) {
    switch (b) {
        case 0xF0: case 0xF2: case 0xF3:
        case 0x2E: case 0x36: case 0x3E: case 0x26: case 0x64: case 0x65:
        case 0x66: case 0x67:
            return true;
        default:
            return false;
    }
}

// Size of ModR/M + SIB + displacement (32-bit addressing; addrPrefix = 0x67).
inline size_t ModRMSize(const uint8_t* p, bool addrPrefix) {
    uint8_t modrm = p[0];
    uint8_t mod = static_cast<uint8_t>(modrm >> 6);
    uint8_t rm = static_cast<uint8_t>(modrm & 0x07);
    size_t n = 1;

    if (addrPrefix) {  // 16-bit addressing: no SIB
        if (mod == 0) { if (rm == 6) n += 2; }
        else if (mod == 1) n += 1;
        else if (mod == 2) n += 2;
        return n;
    }

    if (mod == 3) return n;  // register-direct
    if (rm == 4) {           // SIB
        uint8_t sibByte = p[1];
        n += 1;
        if (mod == 0 && (sibByte & 0x07) == 0x05) n += 4;
    }
    if (mod == 0) { if (rm == 5) n += 4; }
    else if (mod == 1) n += 1;
    else if (mod == 2) n += 4;
    return n;
}

// Two-byte (0x0F) body length excluding the leading 0x0F. Returns 0 on any
// opcode not explicitly recognized (caller aborts the hook).
inline size_t TwoByteBody(const uint8_t* p, bool operandPrefix, bool addrPrefix) {
    uint8_t op = p[0];
    if (op >= 0x80 && op <= 0x8F) return 1 + (operandPrefix ? 2 : 4);  // jcc near
    if (op == 0xBA) return 1 + ModRMSize(p + 1, addrPrefix) + 1;       // grp8 imm8
    if (op >= 0x70 && op <= 0x73) return 1 + ModRMSize(p + 1, addrPrefix) + 1;
    if (op == 0xA4 || op == 0xAC) return 1 + ModRMSize(p + 1, addrPrefix) + 1;
    bool hasModrm =
        (op >= 0x40 && op <= 0x4F) ||   // cmov
        (op >= 0x90 && op <= 0x9F) ||   // setcc
        op == 0xAF || op == 0xA3 || op == 0xAB || op == 0xB3 || op == 0xBB ||
        op == 0xBC || op == 0xBD ||     // bsf/bsr
        op == 0xB6 || op == 0xB7 || op == 0xBE || op == 0xBF ||  // movzx/movsx
        op == 0x1F ||                   // multi-byte nop
        (op >= 0x10 && op <= 0x17) || (op >= 0x28 && op <= 0x2F) ||
        (op >= 0x54 && op <= 0x5F) ||
        op == 0x6E || op == 0x6F || op == 0x7E || op == 0x7F || op == 0xD6;
    if (hasModrm) return 1 + ModRMSize(p + 1, addrPrefix);
    if (op == 0x05 || op == 0x06 || op == 0x07 || op == 0x08 || op == 0x09 ||
        op == 0x0B || op == 0x0E || op == 0x31 || op == 0x77 || op == 0xA2 ||
        op == 0xAA)
        return 1;
    return 0;
}

}  // namespace lde_detail

// Returns the byte length of the single 32-bit instruction at `code`, or 0 if
// it cannot be decoded.
inline size_t DecodeLength(const uint8_t* code) {
    using namespace lde_detail;
    const uint8_t* p = code;
    bool operandPrefix = false, addrPrefix = false;
    int guard = 0;
    while (guard++ < 14 && IsPrefix(*p)) {
        if (*p == 0x66) operandPrefix = true;
        else if (*p == 0x67) addrPrefix = true;
        p++;
    }

    uint8_t op = *p;
    if (op == 0x0F) {
        size_t body = TwoByteBody(p + 1, operandPrefix, addrPrefix);
        if (body == 0) return 0;
        size_t len = static_cast<size_t>((p - code) + 1) + body;
        return (len > 16) ? 0 : len;
    }

    uint8_t flags = PrimaryTable()[op];
    if (flags & F_BAD) return 0;

    const uint8_t* q = p + 1;
    if (flags & F_MODRM) {
        size_t m = ModRMSize(q, addrPrefix);
        if (op == 0xF6 && ((q[0] >> 3) & 0x07) <= 1) {
            q += m + 1;
        } else if (op == 0xF7 && ((q[0] >> 3) & 0x07) <= 1) {
            q += m + (operandPrefix ? 2 : 4);
        } else {
            q += m;
        }
    }
    if (flags & F_IMM8) q += 1;
    if (flags & F_IMM16) q += 2;
    if (flags & F_IMMZ) q += operandPrefix ? 2 : 4;
    if (flags & F_MOFFS) q += addrPrefix ? 2 : 4;
    if (flags & F_FARPTR) q += (operandPrefix ? 2 : 4) + 2;
    if (flags & F_ENTER) q += 3;

    size_t len = static_cast<size_t>(q - code);
    return (len == 0 || len > 16) ? 0 : len;
}

}  // namespace argm
