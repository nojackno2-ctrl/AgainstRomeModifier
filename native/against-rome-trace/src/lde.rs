//! Bounded port of lde.h, for 32-bit x86 (not an x64 decoder).
pub fn is_prefix(b: u8) -> bool {
    matches!(
        b,
        0xf0 | 0xf2 | 0xf3 | 0x2e | 0x36 | 0x3e | 0x26 | 0x64 | 0x65 | 0x66 | 0x67
    )
}
fn modrm(p: &[u8], addr: bool) -> Option<usize> {
    let b = *p.first()?;
    let (m, r) = (b >> 6, b & 7);
    let mut n = 1;
    if m != 3 {
        if addr {
            n += match m {
                0 if r == 6 => 2,
                1 => 1,
                2 => 2,
                _ => 0,
            };
        } else {
            if r == 4 {
                let sib = *p.get(n)?;
                n += 1;
                if m == 0 && sib & 7 == 5 {
                    n += 4;
                }
            }
            n += match m {
                0 if r == 5 => 4,
                1 => 1,
                2 => 4,
                _ => 0,
            };
        }
    }
    (p.len() >= n).then_some(n)
}
pub fn decode_length(code: &[u8]) -> Option<usize> {
    let (mut i, mut operand, mut addr) = (0, false, false);
    while i < 14 && is_prefix(*code.get(i)?) {
        operand |= code[i] == 0x66;
        addr |= code[i] == 0x67;
        i += 1;
    }
    let op = *code.get(i)?;
    if is_prefix(op) {
        return None;
    }
    i += 1;
    let z = if operand { 2 } else { 4 };
    if op == 0x0f {
        let b = *code.get(i)?;
        i += 1;
        match b {
            0x80..=0x8f => i += z,
            0xba | 0x70..=0x73 | 0xa4 | 0xac => i += modrm(code.get(i..)?, addr)? + 1,
            0x40..=0x4f
            | 0x90..=0x9f
            | 0xaf
            | 0xa3
            | 0xab
            | 0xb3
            | 0xbb
            | 0xbc
            | 0xbd
            | 0xb6
            | 0xb7
            | 0xbe
            | 0xbf
            | 0x1f
            | 0x10..=0x17
            | 0x28..=0x2f
            | 0x54..=0x5f
            | 0x6e
            | 0x6f
            | 0x7e
            | 0x7f
            | 0xd6 => i += modrm(code.get(i..)?, addr)?,
            0x05..=0x09 | 0x0b | 0x0e | 0x31 | 0x77 | 0xa2 | 0xaa => (),
            _ => return None,
        }
    } else {
        let arithmetic = op <= 0x3d && op & 7 <= 5;
        let has_modrm = (arithmetic && op & 7 <= 3)
            || matches!(op,
            0x62 | 0x63 | 0x69 | 0x6b | 0x80..=0x8f | 0xc0 | 0xc1 |
            0xc4..=0xc7 | 0xd0..=0xd3 | 0xd8..=0xdf | 0xf6 | 0xf7 | 0xfe | 0xff);
        if has_modrm {
            let b = *code.get(i)?;
            i += modrm(code.get(i..)?, addr)?;
            if (b >> 3) & 7 <= 1 {
                if op == 0xf6 {
                    i += 1;
                }
                if op == 0xf7 {
                    i += z;
                }
            }
        }
        if (arithmetic && op & 7 == 4)
            || matches!(op,
            0x6a | 0x6b | 0x70..=0x80 | 0x82 | 0x83 | 0xa8 | 0xb0..=0xb7 |
            0xc0 | 0xc1 | 0xc6 | 0xcd | 0xd4 | 0xd5 | 0xe0..=0xe7 | 0xeb)
        {
            i += 1;
        }
        if (arithmetic && op & 7 == 5)
            || matches!(
                op,
                0x68 | 0x69 | 0x81 | 0xa9 | 0xb8..=0xbf | 0xc7 | 0xe8 | 0xe9
            )
        {
            i += z;
        }
        if matches!(op, 0xc2 | 0xca) {
            i += 2;
        }
        if matches!(op, 0xa0..=0xa3) {
            i += if addr { 2 } else { 4 };
        }
        if matches!(op, 0x9a | 0xea) {
            i += z + 2;
        }
        if op == 0xc8 {
            i += 3;
        }
    }
    (i <= 15 && i <= code.len()).then_some(i)
}
