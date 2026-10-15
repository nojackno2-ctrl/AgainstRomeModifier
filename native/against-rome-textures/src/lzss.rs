//! LZSS and PFIL compression / decompression routines matching Against Rome engine format.

use crate::error::TextureError;

pub const PFIL_HEADER_LEN: usize = 64;

pub fn is_pfil(data: &[u8]) -> bool {
    data.len() >= PFIL_HEADER_LEN && data.starts_with(b"PFIL")
}

/// Decompresses data if it begins with a PFIL header; otherwise returns the slice untouched.
pub fn decompress_if_pfil(data: &[u8]) -> Result<Vec<u8>, TextureError> {
    if data.starts_with(b"PFIL") {
        if data.len() < PFIL_HEADER_LEN {
            return Err(TextureError::InvalidFormat(format!(
                "Truncated PFIL header: expected at least {} bytes, got {}",
                PFIL_HEADER_LEN,
                data.len()
            )));
        }
        against_rome_data::lzss::decompress_pfil(data, None)
            .map_err(|e| TextureError::InvalidFormat(format!("Failed to decompress PFIL: {}", e)))
    } else {
        Ok(data.to_vec())
    }
}

/// Compresses raw input bytes using Against Rome LZSS specification.
///
/// Ring window: 4096 bytes.
/// Initial window: first 4078 bytes are 0x20 (' '), last 18 bytes are 0x00.
/// Write cursor starts at 4078 (4096 - 18).
pub fn compress(input: &[u8]) -> Vec<u8> {
    const N: usize = 4096;
    const F: usize = 18;
    const THRESHOLD: usize = 2;

    let mut win = [0x20u8; N];
    win[4078..].fill(0x00);
    let mut r = 4078usize;

    let mut head = [-1i32; 65536];
    let mut next = [-1i32; N];
    let mut prev = [-1i32; N];
    let mut current_hash = [-1i32; N];

    // Initialize hash table for initial window
    for i in 0..N {
        let h = (((win[i] as u32) << 8)
            ^ ((win[(i + 1) & 4095] as u32) << 4)
            ^ (win[(i + 2) & 4095] as u32)) as usize
            & 0xFFFF;
        next[i] = head[h];
        prev[i] = -1;
        if head[h] != -1 {
            prev[head[h] as usize] = i as i32;
        }
        head[h] = i as i32;
        current_hash[i] = h as i32;
    }

    let mut output = Vec::with_capacity(input.len());
    let mut src = 0usize;
    let len = input.len();

    let mut code_buf = [0u8; 17];
    code_buf[0] = 0;
    let mut code_ptr = 1usize;
    let mut mask = 1u16;

    let remove_node =
        |idx: usize,
         current_hash: &mut [i32; N],
         prev: &mut [i32; N],
         next: &mut [i32; N],
         head: &mut [i32; 65536]| {
            let old_h = current_hash[idx];
            if old_h != -1 {
                let p = prev[idx];
                let n = next[idx];
                if p != -1 {
                    next[p as usize] = n;
                } else {
                    head[old_h as usize] = n;
                }
                if n != -1 {
                    prev[n as usize] = p;
                }
                current_hash[idx] = -1;
                next[idx] = -1;
                prev[idx] = -1;
            }
        };

    let insert_node =
        |idx: usize,
         win: &[u8; N],
         next: &mut [i32; N],
         head: &mut [i32; 65536],
         prev: &mut [i32; N],
         current_hash: &mut [i32; N]| {
            let h = (((win[idx] as u32) << 8)
                ^ ((win[(idx + 1) & 4095] as u32) << 4)
                ^ (win[(idx + 2) & 4095] as u32)) as usize
                & 0xFFFF;
            next[idx] = head[h];
            prev[idx] = -1;
            if head[h] != -1 {
                prev[head[h] as usize] = idx as i32;
            }
            head[h] = idx as i32;
            current_hash[idx] = h as i32;
        };

    let update_window = |idx: usize,
                             val: u8,
                             win: &mut [u8; N],
                             current_hash: &mut [i32; N],
                             prev: &mut [i32; N],
                             next: &mut [i32; N],
                             head: &mut [i32; 65536]| {
        let p2 = (idx.wrapping_sub(2)) & 4095;
        let p1 = (idx.wrapping_sub(1)) & 4095;
        let p0 = idx;
        remove_node(p2, current_hash, prev, next, head);
        remove_node(p1, current_hash, prev, next, head);
        remove_node(p0, current_hash, prev, next, head);
        win[idx] = val;
        insert_node(p2, win, next, head, prev, current_hash);
        insert_node(p1, win, next, head, prev, current_hash);
        insert_node(p0, win, next, head, prev, current_hash);
    };

    while src < len {
        let mut match_key = 0usize;
        let mut match_len = 0usize;
        let limit = F.min(len - src);

        if limit >= 3 {
            let hash = (((input[src] as u32) << 8)
                ^ ((input[src + 1] as u32) << 4)
                ^ (input[src + 2] as u32)) as usize
                & 0xFFFF;
            let mut i = head[hash];
            let mut depth = 0;

            while i != -1 && depth < 256 {
                depth += 1;
                let u = i as usize;
                let mut dist_to_r = (r.wrapping_sub(u)) & 4095;
                if dist_to_r == 0 {
                    dist_to_r = 4096;
                }

                if dist_to_r >= 3
                    && win[u] == input[src]
                    && win[(u + 1) & 4095] == input[src + 1]
                    && win[(u + 2) & 4095] == input[src + 2]
                {
                    let mut m = 3;
                    while m < limit {
                        let expected = if m < dist_to_r {
                            win[(u + m) & 4095]
                        } else {
                            input[src + (m % dist_to_r)]
                        };
                        if expected != input[src + m] {
                            break;
                        }
                        m += 1;
                    }
                    if m > match_len {
                        match_key = u;
                        match_len = m;
                        if match_len == limit {
                            break;
                        }
                    }
                }
                i = next[u];
            }

            if match_len < limit {
                for d in 1..=2 {
                    let idx = (r.wrapping_sub(d)) & 4095;
                    if win[idx] == input[src] {
                        let mut m = 1;
                        let dist_to_r = d;
                        while m < limit {
                            let expected = if m < dist_to_r {
                                win[(idx + m) & 4095]
                            } else {
                                input[src + (m % dist_to_r)]
                            };
                            if expected != input[src + m] {
                                break;
                            }
                            m += 1;
                        }
                        if m > match_len {
                            match_key = idx;
                            match_len = m;
                            if match_len == limit {
                                break;
                            }
                        }
                    }
                }
            }
        }

        if match_len <= THRESHOLD {
            code_buf[0] |= mask as u8;
            code_buf[code_ptr] = input[src];
            code_ptr += 1;
            update_window(
                r,
                input[src],
                &mut win,
                &mut current_hash,
                &mut prev,
                &mut next,
                &mut head,
            );
            r = (r + 1) & 4095;
            src += 1;
        } else {
            let offset = match_key;
            let length = match_len;
            let b1 = (offset & 0xFF) as u8;
            let b2 = (((offset >> 4) & 0xF0) | ((length - 3) & 0x0F)) as u8;
            code_buf[code_ptr] = b1;
            code_buf[code_ptr + 1] = b2;
            code_ptr += 2;

            for k in 0..length {
                update_window(
                    r,
                    input[src + k],
                    &mut win,
                    &mut current_hash,
                    &mut prev,
                    &mut next,
                    &mut head,
                );
                r = (r + 1) & 4095;
            }
            src += length;
        }

        mask <<= 1;
        if mask == 256 {
            output.extend_from_slice(&code_buf[..code_ptr]);
            code_buf[0] = 0;
            code_ptr = 1;
            mask = 1;
        }
    }

    if mask != 1 {
        output.extend_from_slice(&code_buf[..code_ptr]);
    }

    output
}

/// Wraps compressed bytes into an Against Rome PFIL container, preserving the original 64-byte header.
pub fn compress_pfil(input: &[u8], orig_header: &[u8]) -> Result<Vec<u8>, TextureError> {
    if orig_header.len() < PFIL_HEADER_LEN {
        return Err(TextureError::InvalidFormat(
            "PFIL header must contain at least 64 bytes".to_string(),
        ));
    }

    let compressed = compress(input);
    let mut out = Vec::with_capacity(PFIL_HEADER_LEN + compressed.len());

    let mut header = [0u8; PFIL_HEADER_LEN];
    header.copy_from_slice(&orig_header[..PFIL_HEADER_LEN]);

    // Write uncompressed size as little-endian u32 at offset 16..20
    let size_bytes = (input.len() as u32).to_le_bytes();
    header[16..20].copy_from_slice(&size_bytes);

    out.extend_from_slice(&header);
    out.extend_from_slice(&compressed);

    Ok(out)
}
