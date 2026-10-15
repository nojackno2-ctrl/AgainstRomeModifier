//! Windows-1251 (CP1251) encoding utilities and PFIL compression helpers.

/// Checks whether the string can be encoded into CP1251 without replacement.
pub fn can_encode_game_text(value: &str) -> bool {
    let (_, _, had_errors) = encoding_rs::WINDOWS_1251.encode(value);
    !had_errors
}

/// Returns the byte count of the string encoded in CP1251.
pub fn game_text_byte_count(value: &str) -> usize {
    let (cow, _, _) = encoding_rs::WINDOWS_1251.encode(value);
    cow.len()
}

/// Encodes a string into CP1251 bytes, returning an error if any characters cannot be mapped.
pub fn encode_cp1251(value: &str) -> Result<Vec<u8>, String> {
    let (cow, _, had_errors) = encoding_rs::WINDOWS_1251.encode(value);
    if had_errors {
        Err("地圖文字需使用遊戲支援的 CP1251 字元（拉丁／西里爾字母）；中文或 emoji 無法儲存。".into())
    } else {
        Ok(cow.into_owned())
    }
}

/// Decodes CP1251 bytes into a UTF-8 String, returning an error if invalid.
pub fn decode_cp1251(bytes: &[u8]) -> Result<String, String> {
    let (cow, had_errors) = encoding_rs::WINDOWS_1251.decode_without_bom_handling(bytes);
    if had_errors {
        Err("無法以 Windows-1251 解碼字元。".into())
    } else {
        Ok(cow.into_owned())
    }
}

/// Checks if bytes start with the 4-byte ASCII magic "PFIL" and have at least 64 bytes header.
pub fn is_pfil(bytes: &[u8]) -> bool {
    bytes.len() >= 64 && &bytes[0..4] == b"PFIL"
}

/// Decompresses bytes if they are PFIL compressed, otherwise returns the slice as a Vec.
pub fn decompress_pfil_if_needed(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if is_pfil(bytes) {
        against_rome_data::lzss::decompress_pfil(bytes, None).map_err(|e| e.to_string())
    } else {
        Ok(bytes.to_vec())
    }
}

/// Compresses plain bytes using the game's LZSS format (4096-byte ring buffer).
pub fn compress_lzss(input: &[u8]) -> Vec<u8> {
    const N: usize = 4096;
    const F: usize = 18;
    const THRESHOLD: usize = 2;

    let mut win = [0x20u8; N];
    win[N - F..].fill(0x00);
    let mut r = N - F; // 4078

    let mut output = Vec::with_capacity(input.len());
    let mut code_buf = [0u8; 17];
    code_buf[0] = 0;
    let mut code_ptr = 1usize;
    let mut mask = 1u16;

    let mut src = 0usize;
    let len = input.len();

    while src < len {
        let mut match_key = 0usize;
        let mut match_len = 0usize;
        let limit = F.min(len - src);

        if limit >= 3 {
            // Find longest match in the window
            for i in 0..N {
                if win[i] == input[src]
                    && win[(i + 1) & 4095] == input[src + 1]
                    && win[(i + 2) & 4095] == input[src + 2]
                {
                    let dist_to_r = (r.wrapping_sub(i)) & 4095;
                    let dist_to_r = if dist_to_r == 0 { 4096 } else { dist_to_r };
                    if dist_to_r >= 3 {
                        let mut m = 3usize;
                        while m < limit {
                            let expected = if m < dist_to_r {
                                win[(i + m) & 4095]
                            } else {
                                input[src + (m % dist_to_r)]
                            };
                            if expected != input[src + m] {
                                break;
                            }
                            m += 1;
                        }
                        if m > match_len {
                            match_key = i;
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
            win[r] = input[src];
            r = (r + 1) & 4095;
            src += 1;
        } else {
            let offset = match_key;
            let length = match_len;
            let b1 = (offset & 0xFF) as u8;
            let b2 = (((offset >> 4) & 0xF0) | ((length - 3) & 0x0F)) as u8;
            code_buf[code_ptr] = b1;
            code_ptr += 1;
            code_buf[code_ptr] = b2;
            code_ptr += 1;
            for k in 0..length {
                win[r] = input[src + k];
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

/// Compresses plain bytes into PFIL using a 64-byte template header (e.g. from original file).
pub fn compress_pfil(plain: &[u8], template_header: &[u8]) -> Result<Vec<u8>, String> {
    if template_header.len() < 64 {
        return Err("PFIL 標頭長度必須至少為 64 位元組。".into());
    }
    let compressed = compress_lzss(plain);
    let mut header = [0u8; 64];
    header.copy_from_slice(&template_header[..64]);
    let size_bytes = (plain.len() as u32).to_le_bytes();
    header[16..20].copy_from_slice(&size_bytes);

    let mut out = Vec::with_capacity(64 + compressed.len());
    out.extend_from_slice(&header);
    out.extend_from_slice(&compressed);
    Ok(out)
}
