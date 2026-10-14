//! Strict port of `GameLZSS.Decompress`/`DecompressPfil` (src.Shared/Core/GameLZSS.cs).
//!
//! Ring window: 4096 bytes, first 4078 are 0x20, last 18 are 0x00, write cursor starts at 4078.
//! The C# decoder silently tolerates truncation, back-reference overshoot and trailing input;
//! this port treats each of those as an error so a decoded pool is always complete.

use crate::error::Error;

pub const PFIL_HEADER_LEN: usize = 64;
/// Same ceiling as the C# decoder (50 MiB).
pub const MAX_DECODED_LEN: usize = 50 * 1024 * 1024;

pub fn has_pfil_magic(input: &[u8]) -> bool {
    input.starts_with(b"PFIL")
}

/// Decodes a PFIL file. `expected_len` (when `Some`) must equal the u32 at header offset 16;
/// the declared size is checked *before* any allocation.
pub fn decompress_pfil(input: &[u8], expected_len: Option<usize>) -> Result<Vec<u8>, Error> {
    if input.len() < PFIL_HEADER_LEN {
        return Err(Error::PfilHeaderTruncated { len: input.len() });
    }
    if !has_pfil_magic(input) {
        return Err(Error::PfilBadMagic);
    }
    let declared = u32::from_le_bytes([input[16], input[17], input[18], input[19]]);
    let limit = expected_len.unwrap_or(MAX_DECODED_LEN);
    if declared as usize > MAX_DECODED_LEN || expected_len.is_some_and(|e| e != declared as usize) {
        return Err(Error::DeclaredSize { declared, expected: limit });
    }
    decode(&input[PFIL_HEADER_LEN..], declared as usize)
}

/// Decodes a headerless LZSS body into exactly `size` bytes.
pub fn decode(data: &[u8], size: usize) -> Result<Vec<u8>, Error> {
    let mut out = Vec::with_capacity(size);
    let mut ring = [0x20u8; 4096];
    ring[4078..].fill(0);
    let mut r = 4078usize;
    let mut ip = 0usize;
    while out.len() < size {
        let Some(&flags) = data.get(ip) else {
            return Err(Error::LzssTruncated { produced: out.len(), expected: size });
        };
        ip += 1;
        for bit in 0..8 {
            if out.len() == size {
                break;
            }
            if flags & (1 << bit) != 0 {
                let Some(&b) = data.get(ip) else {
                    return Err(Error::LzssTruncated { produced: out.len(), expected: size });
                };
                ip += 1;
                out.push(b);
                ring[r] = b;
                r = (r + 1) & 4095;
            } else {
                let (Some(&p1), Some(&p2)) = (data.get(ip), data.get(ip + 1)) else {
                    return Err(Error::LzssTruncated { produced: out.len(), expected: size });
                };
                ip += 2;
                let offset = p1 as usize | ((p2 as usize & 0xF0) << 4);
                let len = (p2 as usize & 0x0F) + 3;
                if out.len() + len > size {
                    return Err(Error::LzssOvershoot { produced: out.len(), copy_len: len, expected: size });
                }
                for k in 0..len {
                    let b = ring[(offset + k) & 4095];
                    out.push(b);
                    ring[r] = b;
                    r = (r + 1) & 4095;
                }
            }
        }
    }
    if ip != data.len() {
        return Err(Error::LzssTrailingInput { unused: data.len() - ip });
    }
    Ok(out)
}
