//! Pure Rust LZSS and PFIL compression / decompression routines
//! matching the Against Rome engine format (FUN_00565c00 / GameLZSS.cs).

use std::io::{self, Error, ErrorKind};

pub const PFIL_HEADER_LEN: usize = 64;
pub const MAX_UNCOMPRESSED_SIZE: usize = 50 * 1024 * 1024; // 50 MiB ceiling

pub fn is_pfil(data: &[u8]) -> bool {
    data.len() >= PFIL_HEADER_LEN && data.starts_with(b"PFIL")
}

/// Decompress data if it has a PFIL header; otherwise returns data as-is.
pub fn decompress_if_pfil(data: &[u8]) -> io::Result<Vec<u8>> {
    if data.starts_with(b"PFIL") {
        decompress_pfil(data)
    } else {
        Ok(data.to_vec())
    }
}

/// Decompresses a PFIL-compressed byte slice.
pub fn decompress_pfil(input: &[u8]) -> io::Result<Vec<u8>> {
    if input.len() < PFIL_HEADER_LEN {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!(
                "truncated PFIL header: expected >= {} bytes, got {}",
                PFIL_HEADER_LEN,
                input.len()
            ),
        ));
    }
    if !input.starts_with(b"PFIL") {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "missing PFIL magic bytes",
        ));
    }
    let uncompressed_size =
        u32::from_le_bytes([input[16], input[17], input[18], input[19]]) as usize;
    if uncompressed_size > MAX_UNCOMPRESSED_SIZE {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!(
                "uncompressed size {} exceeds maximum allowable limit of 50 MB",
                uncompressed_size
            ),
        ));
    }

    decompress_body(&input[PFIL_HEADER_LEN..], uncompressed_size)
}

/// Decompresses headerless LZSS data into an exact output length.
pub fn decompress_body(data: &[u8], size: usize) -> io::Result<Vec<u8>> {
    let mut out = Vec::with_capacity(size);
    let mut ring = [0x20u8; 4096];
    ring[4078..].fill(0x00);
    let mut r = 4078usize;
    let mut ip = 0usize;

    while ip < data.len() && out.len() < size {
        let flags = data[ip];
        ip += 1;

        for bit in 0..8 {
            if ip >= data.len() || out.len() >= size {
                break;
            }
            if (flags & (1 << bit)) != 0 {
                let b = data[ip];
                ip += 1;
                out.push(b);
                ring[r] = b;
                r = (r + 1) & 4095;
            } else {
                if ip + 1 >= data.len() {
                    break;
                }
                let p1 = data[ip] as usize;
                let p2 = data[ip + 1] as usize;
                ip += 2;

                let offset = p1 | ((p2 & 0xF0) << 4);
                let length = (p2 & 0x0F) + 3;

                for k in 0..length {
                    let b = ring[(offset + k) & 4095];
                    if out.len() < size {
                        out.push(b);
                    }
                    ring[r] = b;
                    r = (r + 1) & 4095;
                }
            }
        }
    }

    if out.len() < size {
        return Err(Error::new(
            ErrorKind::UnexpectedEof,
            format!(
                "LZSS stream truncated: produced {} of {} expected bytes",
                out.len(),
                size
            ),
        ));
    }

    Ok(out)
}

/// Compresses raw input bytes using Against Rome LZSS specification.
pub fn compress(input: &[u8]) -> Vec<u8> {
    const N: usize = 4096;
    const F: usize = 18;

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

    let remove_node = |idx: usize,
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

    let insert_node = |idx: usize,
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
                    let mut match_count = 3;
                    while match_count < limit {
                        let expected = if match_count < dist_to_r {
                            win[(u + match_count) & 4095]
                        } else {
                            input[src + (match_count % dist_to_r)]
                        };
                        if expected != input[src + match_count] {
                            break;
                        }
                        match_count += 1;
                    }

                    if match_count > match_len {
                        match_key = u;
                        match_len = match_count;
                        if match_len == limit {
                            break;
                        }
                    }
                }
                i = next[u];
            }

            if match_len < 3 {
                for offset in 1..=2 {
                    let u = (r.wrapping_sub(offset)) & 4095;
                    if win[u] == input[src]
                        && win[(u + 1) & 4095] == input[src + 1]
                        && win[(u + 2) & 4095] == input[src + 2]
                    {
                        let mut match_count = 3;
                        let dist_to_r = offset;
                        while match_count < limit {
                            let expected = input[src + (match_count % dist_to_r)];
                            if expected != input[src + match_count] {
                                break;
                            }
                            match_count += 1;
                        }
                        if match_count > match_len {
                            match_key = u;
                            match_len = match_count;
                            if match_len == limit {
                                break;
                            }
                        }
                    }
                }
            }
        }

        if match_len < 3 {
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
            let p1 = (match_key & 0xFF) as u8;
            let p2 = (((match_key >> 4) & 0xF0) | ((match_len - 3) & 0x0F)) as u8;
            code_buf[code_ptr] = p1;
            code_buf[code_ptr + 1] = p2;
            code_ptr += 2;

            for k in 0..match_len {
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
            src += match_len;
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

/// Wraps compressed bytes into an Against Rome PFIL container.
/// If `header_template` has >= 64 bytes, its non-size fields are preserved.
pub fn compress_pfil(input: &[u8], header_template: Option<&[u8]>) -> io::Result<Vec<u8>> {
    let compressed = compress(input);
    let mut out = Vec::with_capacity(PFIL_HEADER_LEN + compressed.len());

    let mut header = [0u8; PFIL_HEADER_LEN];
    if let Some(template) = header_template {
        if template.len() >= PFIL_HEADER_LEN {
            header.copy_from_slice(&template[..PFIL_HEADER_LEN]);
        } else {
            header[..template.len()].copy_from_slice(template);
        }
    }
    // Always enforce magic at 0..4
    header[0..4].copy_from_slice(b"PFIL");
    // Write uncompressed size as little-endian u32 at 16..20
    let size_bytes = (input.len() as u32).to_le_bytes();
    header[16..20].copy_from_slice(&size_bytes);

    out.extend_from_slice(&header);
    out.extend_from_slice(&compressed);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_uncompressed_text_and_binary() {
        let texts = [
            b"Hello Against Rome World!\r\n[FormationRotationFaktor]\r\n500.0\r\n".to_vec(),
            (0..5000).map(|i| (i % 256) as u8).collect::<Vec<u8>>(),
            vec![b'A'; 1000],
        ];

        for original in texts {
            let compressed = compress_pfil(&original, None).unwrap();
            assert!(is_pfil(&compressed));
            assert_eq!(&compressed[..4], b"PFIL");
            let decompressed = decompress_pfil(&compressed).unwrap();
            assert_eq!(decompressed, original);
        }
    }

    #[test]
    fn template_header_bytes_preserved() {
        let mut template = [0x77u8; 64];
        template[..4].copy_from_slice(b"PFIL");
        let payload = b"Test payload";
        let compressed = compress_pfil(payload, Some(&template)).unwrap();
        assert_eq!(&compressed[4..16], &template[4..16]);
        assert_eq!(&compressed[20..64], &template[20..64]);
        let decompressed = decompress_pfil(&compressed).unwrap();
        assert_eq!(decompressed, payload);
    }
}
