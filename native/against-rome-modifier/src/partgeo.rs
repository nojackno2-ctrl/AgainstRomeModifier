//! partgeo.dau patcher ported faithfully from src.Core/Core/Patches/PartgeoPatcher.cs.

use crate::patch_text::{encode_cp1251, parse_csv_line, to_csv_string, TextDocument};
use std::io::{self, Error, ErrorKind};

pub const PROJECTILE_GEO_NAMES: &[&str] = &[
    "Wurfspeer00",
    "Wurfaxt00",
    "Katapultstein00",
    "Katapultstein01",
    "Pfeil00",
    "Spiess00",
    "Fackel",
];

pub const ARC_YSUB_MULTIPLIER: f64 = 2.0;
pub const YSUB_COLUMN: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PartgeoOptions {
    pub projectile_arc_height: bool,
}

pub struct PartgeoPatcher;

impl PartgeoPatcher {
    pub fn get_patched_bytes(original: &[u8], options: &PartgeoOptions) -> io::Result<Vec<u8>> {
        if !options.projectile_arc_height {
            return Ok(original.to_vec());
        }

        let doc = TextDocument::read(original)?;
        let mut lines = doc.lines.clone();
        let mut patched_count = 0;

        for line in &mut lines {
            let mut cols = parse_csv_line(line);
            if cols.len() <= YSUB_COLUMN {
                continue;
            }

            let name = cols[2].trim();
            if !PROJECTILE_GEO_NAMES.contains(&name) {
                continue;
            }

            let ysub: i64 = cols[YSUB_COLUMN].trim().parse().map_err(|_| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "partgeo.dau entry {} ysub value invalid or unparseable",
                        name
                    ),
                )
            })?;
            if ysub <= 0 {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("partgeo.dau entry {} ysub is not positive", name),
                ));
            }

            let new_value = (ysub as f64 * ARC_YSUB_MULTIPLIER).round() as i64;
            let new_val_str = new_value.to_string();
            let width = cols[YSUB_COLUMN].len();

            if new_val_str.len() > width {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "partgeo.dau entry {} ysub {} exceeds field width {}",
                        name, new_val_str, width
                    ),
                ));
            }

            let padded = format!("{:>width$}", new_val_str, width = width);
            cols[YSUB_COLUMN] = padded;
            *line = to_csv_string(cols.iter().map(|s| s.as_str()));
            patched_count += 1;
        }

        if patched_count != PROJECTILE_GEO_NAMES.len() {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "partgeo.dau projectile count mismatch: expected {}, found {}",
                    PROJECTILE_GEO_NAMES.len(),
                    patched_count
                ),
            ));
        }

        let mut text = lines.join(doc.line_ending);
        if doc.text.ends_with(doc.line_ending) && !text.ends_with(doc.line_ending) {
            text.push_str(doc.line_ending);
        }

        let encoded = encode_cp1251(&text);
        if doc.was_pfil {
            compress_pfil_exact(&encoded, doc.raw_header_template.as_deref())
        } else {
            Ok(encoded)
        }
    }
}

fn compress_pfil_exact(input_bytes: &[u8], header_template: Option<&[u8]>) -> io::Result<Vec<u8>> {
    let compressed = compress_lzss_csharp_exact(input_bytes);
    let mut header = [0u8; 64];
    if let Some(template) = header_template {
        let copy_len = template.len().min(64);
        header[..copy_len].copy_from_slice(&template[..copy_len]);
    } else {
        header[..4].copy_from_slice(b"PFIL");
    }

    let size_bytes = (input_bytes.len() as u32).to_le_bytes();
    header[16..20].copy_from_slice(&size_bytes);

    let mut out_bytes = Vec::with_capacity(64 + compressed.len());
    out_bytes.extend_from_slice(&header);
    out_bytes.extend_from_slice(&compressed);
    Ok(out_bytes)
}

fn compress_lzss_csharp_exact(input: &[u8]) -> Vec<u8> {
    const N: usize = 4096;
    const F: usize = 18;
    const THRESHOLD: usize = 2;

    let mut win = [0x20u8; N];
    win[N - F..N].fill(0x00);
    let mut r = N - F; // 4078

    let mut head = [-1i32; 65536];
    let mut next = [-1i32; N];
    let mut prev = [-1i32; N];
    let mut current_hash = [-1i32; N];

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

    let mut output = Vec::with_capacity(input.len());
    let mut src = 0usize;
    let len = input.len();

    let mut code_buf = [0u8; 17];
    code_buf[0] = 0;
    let mut code_ptr = 1usize;
    let mut mask = 1u16;

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

            if match_len < limit {
                for d in 1..=2 {
                    let idx = (r.wrapping_sub(d)) & 4095;
                    if win[idx] == input[src] {
                        let mut match_count = 1;
                        let dist_to_r = d;
                        while match_count < limit {
                            let expected = if match_count < dist_to_r {
                                win[(idx + match_count) & 4095]
                            } else {
                                input[src + (match_count % dist_to_r)]
                            };
                            if expected != input[src + match_count] {
                                break;
                            }
                            match_count += 1;
                        }
                        if match_count > match_len {
                            match_key = idx;
                            match_len = match_count;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partgeo_scales_all_7_projectiles() {
        let mut rows = vec!["header1".to_string(), "header2".to_string()];
        for name in PROJECTILE_GEO_NAMES {
            let mut cols = vec!["     0".to_string(); 20];
            cols[2] = format!("{:>20}", name);
            cols[12] = "   100".to_string();
            rows.push(cols.join(","));
        }
        let raw = rows.join("\r\n") + "\r\n";

        let options = PartgeoOptions {
            projectile_arc_height: true,
        };
        let patched = PartgeoPatcher::get_patched_bytes(raw.as_bytes(), &options).unwrap();
        let s = String::from_utf8(patched).unwrap();

        for line in s.lines().skip(2) {
            let cols = parse_csv_line(line);
            assert_eq!(cols[12].trim(), "200");
        }

        let untouched =
            PartgeoPatcher::get_patched_bytes(raw.as_bytes(), &PartgeoOptions::default()).unwrap();
        assert_eq!(untouched, raw.as_bytes());
    }

    #[test]
    fn partgeo_rejects_missing_and_malformed() {
        let options = PartgeoOptions {
            projectile_arc_height: true,
        };
        assert!(PartgeoPatcher::get_patched_bytes(b"", &options).is_err());
        assert!(PartgeoPatcher::get_patched_bytes(b"header\r\n", &options).is_err());

        // Field width overflow
        let row = "   20,    1,                 Wurfspeer00,0,0,0,0,0,0,0,0,0,50\r\n";
        assert!(PartgeoPatcher::get_patched_bytes(row.as_bytes(), &options).is_err());
    }
}
