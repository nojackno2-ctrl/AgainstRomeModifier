use sha2::{Digest, Sha256};

pub const PFIL_MAGIC: &[u8; 4] = b"PFIL";
pub const MAX_DECODED_LEN: usize = 50 * 1024 * 1024;

/// 解壓縮 PFIL 檔案。若輸入無 "PFIL" 標頭或長度小於 64 位元組，則視為未壓縮原始資料直接回傳。
pub fn decompress_pfil(input: &[u8]) -> Result<Vec<u8>, String> {
    if input.len() < 64 || !input.starts_with(PFIL_MAGIC) {
        return Ok(input.to_vec());
    }

    let declared = u32::from_le_bytes([input[16], input[17], input[18], input[19]]) as usize;
    if declared > MAX_DECODED_LEN {
        return Err("PFIL 宣告大小超出安全上限 (50MB)。".to_string());
    }

    // 優先嘗試 against-rome-data 的嚴格 LZSS 解壓縮
    if let Ok(strict) = against_rome_data::lzss::decompress_pfil(input, Some(declared)) {
        return Ok(strict);
    }

    // 若嚴格模式因尾部邊界填充或對齊微差返回錯誤，回退至相容解碼迴圈 (對齊 probe.py / GameLZSS.cs)
    let mut ring = [0x20u8; 4096];
    ring[4078..].fill(0);
    let mut r = 4078usize;
    let mut ip = 64usize;
    let mut out = Vec::with_capacity(declared);

    while out.len() < declared && ip < input.len() {
        let flags = input[ip];
        ip += 1;
        for bit in 0..8 {
            if out.len() == declared || ip >= input.len() {
                break;
            }
            if (flags & (1 << bit)) != 0 {
                let b = input[ip];
                ip += 1;
                out.push(b);
                ring[r] = b;
                r = (r + 1) & 4095;
            } else {
                if ip + 1 >= input.len() {
                    break;
                }
                let a = input[ip] as usize;
                let b = input[ip + 1] as usize;
                ip += 2;
                let offset = a | ((b & 0xF0) << 4);
                let len = (b & 0x0F) + 3;
                for k in 0..len {
                    if out.len() == declared {
                        break;
                    }
                    let val = ring[(offset + k) & 4095];
                    out.push(val);
                    ring[r] = val;
                    r = (r + 1) & 4095;
                }
            }
        }
    }

    if out.len() != declared {
        return Err(format!(
            "PFIL LZSS 解壓縮不完整：預期 {} 位元組，實際產生 {} 位元組。",
            declared,
            out.len()
        ));
    }

    Ok(out)
}

/// 建立簡單有效的合成 PFIL 資料（以全 literal flags 打包），供單元測試與安全驗證使用。
pub fn create_synthetic_pfil(raw_payload: &[u8]) -> Vec<u8> {
    let mut pfil = vec![0u8; 64];
    pfil[0..4].copy_from_slice(b"PFIL");
    let len_bytes = (raw_payload.len() as u32).to_le_bytes();
    pfil[16..20].copy_from_slice(&len_bytes);

    let mut ip = 0;
    while ip < raw_payload.len() {
        let chunk_size = std::cmp::min(8, raw_payload.len() - ip);
        let mut flag = 0u8;
        for i in 0..chunk_size {
            flag |= 1 << i;
        }
        pfil.push(flag);
        pfil.extend_from_slice(&raw_payload[ip..ip + chunk_size]);
        ip += chunk_size;
    }
    pfil
}

/// 計算資料的 SHA256 小寫十六進位字串。
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// 精確模擬 Python 的 `repr(bytes)` 輸出。
pub fn python_bytes_repr(bytes: &[u8]) -> String {
    let has_single = bytes.contains(&b'\'');
    let has_double = bytes.contains(&b'"');
    let quote = if has_single && !has_double { '"' } else { '\'' };

    let mut out = String::with_capacity(bytes.len() * 2 + 3);
    out.push('b');
    out.push(quote);

    for &b in bytes {
        match b {
            b'\\' => out.push_str("\\\\"),
            b'\t' => out.push_str("\\t"),
            b'\r' => out.push_str("\\r"),
            b'\n' => out.push_str("\\n"),
            q if q == quote as u8 => {
                out.push('\\');
                out.push(quote);
            }
            0x20..=0x7E => out.push(b as char),
            other => {
                use std::fmt::Write;
                let _ = write!(out, "\\x{other:02x}");
            }
        }
    }

    out.push(quote);
    out
}

/// 格式化 PFIL / 二進位傾印輸出，完全比照 probe.py 的 --dump 行格式。
pub fn format_dump(input: &[u8], start: usize, end: usize) -> Result<String, String> {
    let hash = sha256_hex(input);
    let raw = decompress_pfil(input)?;
    let mut out = String::new();

    out.push_str(&format!("SHA256 {hash}\n"));
    out.push_str(&format!("payload length {}\n", raw.len()));

    let limit = std::cmp::min(end, raw.len());
    let mut off = start;
    while off < limit {
        let chunk_end = std::cmp::min(off + 16, raw.len());
        let b = &raw[off..chunk_end];
        let hex_bytes = b
            .iter()
            .map(|c| format!("{c:02x}"))
            .collect::<Vec<_>>()
            .join(" ");
        let repr = python_bytes_repr(b);
        out.push_str(&format!("{off:06x} {hex_bytes} {repr}\n"));
        off += 16;
    }

    Ok(out)
}

/// 解析 "start:end" 區間字串，支援十進位與十六進位 (0x...)。
pub fn parse_dump_range(s: &str) -> Result<(usize, usize), String> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return Err(format!("無效的傾印區間格式 (預期 START:END)：'{s}'"));
    }

    let parse_num = |val: &str| -> Result<usize, String> {
        let trimmed = val.trim();
        if let Some(hex) = trimmed
            .strip_prefix("0x")
            .or_else(|| trimmed.strip_prefix("0X"))
        {
            usize::from_str_radix(hex, 16).map_err(|e| format!("無效的十六進位數值 '{val}': {e}"))
        } else {
            trimmed
                .parse::<usize>()
                .map_err(|e| format!("無效的數值 '{val}': {e}"))
        }
    };

    let start = parse_num(parts[0])?;
    let end = parse_num(parts[1])?;
    if start > end {
        return Err(format!("起始位移不能大於結束位移 ({start} > {end})。"));
    }
    Ok((start, end))
}
