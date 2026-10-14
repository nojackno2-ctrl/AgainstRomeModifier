use serde_json::{json, Value};
fn word(b: &[u8], p: &mut usize) -> Result<u32, String> {
    let s = b
        .get(*p..p.checked_add(4).ok_or("offset overflow")?)
        .ok_or_else(|| format!("truncated ALR at {p}"))?;
    *p += 4;
    Ok(u32::from_le_bytes(s.try_into().unwrap()))
}
pub fn parse(b: &[u8]) -> Result<Value, String> {
    let mut p = 0;
    let mut h = Vec::new();
    for _ in 0..23 {
        h.push(word(b, &mut p)?);
    }
    if h[0] != 0x41524c41 {
        return Err("invalid ALRA signature".into());
    }
    if !(4..=6).contains(&h[1]) || h[4] != 8 {
        return Err(format!("unsupported ALR version {} depth {}", h[1], h[4]));
    }
    let count = h[3] as usize;
    let variants = h[8] as usize;
    if count == 0
        || count > 100_000
        || count > (b.len() - p) / 4
        || variants == 0
        || variants > 4096
    {
        return Err("invalid/truncated ALR count or palette variants".into());
    }
    let mut frames: Vec<Value> = Vec::new();
    let mut first_palette_colors = 0;
    for i in 0..count {
        let record = p;
        let r = word(b, &mut p)? as i32;
        if r >= 0 {
            if r as usize >= i {
                return Err("shared frame reference out of range".into());
            }
            let mut f = frames[r as usize].clone();
            f["shared_reference"] = json!(r);
            f["record_offset"] = json!(record);
            frames.push(f);
            continue;
        }
        let table = word(b, &mut p)? as usize;
        let pos = word(b, &mut p)?;
        let size = word(b, &mut p)?;
        let stored = word(b, &mut p)? as usize;
        let w = size & 2047;
        let height = (size >> 11) & 2047;
        let colors = (size >> 22) & 511;
        if i == 0 {
            first_palette_colors = colors;
        }
        let palette = variants
            .checked_mul(colors as usize)
            .and_then(|v| v.checked_mul(4))
            .ok_or("palette overflow")?;
        let payload = palette
            .checked_add(stored.checked_add(3).ok_or("pixel overflow")? & !3)
            .ok_or("payload overflow")?;
        if table != payload {
            return Err("invalid row table offset".into());
        }
        let end = p.checked_add(payload).ok_or("payload overflow")?;
        let data = b.get(p + palette..end).ok_or("truncated frame payload")?;
        p = end;
        let mut descriptors = Vec::new();
        for _ in 0..=height {
            descriptors.push(word(b, &mut p)?);
        }
        // Validate offsets and row runs without rendering or selecting an animation frame.
        for y in 0..height as usize {
            let row = descriptors[y];
            let start = (row & 0xfffff) as usize;
            let end = (descriptors[y + 1] & 0xfffff) as usize;
            if end < start || end > stored {
                return Err("row offset out of range".into());
            }
            let run = &data[start..end];
            let x = (row >> 21) & 1023;
            if row & 0x100000 == 0 {
                check_run(run, x, w, first_palette_colors)?;
            } else {
                if run.len() < 2 || run[0] as usize > run.len() - 2 {
                    return Err("truncated/invalid two-run prefix".into());
                }
                let first = run[0] as usize;
                check_run(&run[2..2 + first], x, w, first_palette_colors)?;
                check_run(
                    &run[2 + first..],
                    x + first as u32 + (run[1] & 127) as u32,
                    w,
                    first_palette_colors,
                )?;
            }
        }
        frames.push(json!({"record_offset":record,"shared_reference":null,"reference_word_raw":r,"position_word_raw":pos,"offset_x_raw":pos&65535,"offset_y_raw":pos>>16,"packed_size_raw":size,"width":w,"height":height,"palette_colors":colors,"stored_pixel_bytes":stored,"row_table_offset":table,"row_descriptors_raw":descriptors}));
    }
    Ok(
        json!({"header_words_raw":h,"version":h[1],"frame_count":count,"layout_columns":h[6],"layout_rows":h[7],"layout_product_matches_count":h[6] as u64*h[7] as u64==count as u64,"palette_variants":variants,"anchor_width":h[15],"anchor_height":h[16],"frames":frames,"parsed_bytes":p,"trailing_bytes":b.len()-p,"trailing_sha256":crate::digest(&b[p..]),"frame_selection":"unresolved; no frame selected, no frame0 fallback"}),
    )
}
fn check_run(run: &[u8], x: u32, w: u32, colors: u32) -> Result<(), String> {
    if x > w || run.len() as u32 > w - x {
        return Err("scanline exceeds width".into());
    }
    if run.iter().any(|v| *v as u32 >= colors) {
        return Err("palette index out of range".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut h = [0u32; 23];
        h[0] = 0x41524c41;
        h[1] = 4;
        h[3] = 1;
        h[4] = 8;
        h[6] = 1;
        h[7] = 1;
        h[8] = 1;
        let mut b: Vec<u8> = h.iter().flat_map(|w| w.to_le_bytes()).collect();
        for w in [u32::MAX, 8, 0, 1 | (1 << 11) | (1 << 22), 1, 0, 0, 0, 1] {
            b.extend(w.to_le_bytes());
        }
        b
    }
    #[test]
    fn valid_and_all_truncations() {
        let b = fixture();
        assert!(parse(&b).is_ok());
        for n in 0..b.len() {
            assert!(parse(&b[..n]).is_err(), "{n}");
        }
    }
    #[test]
    fn invalid_header_shared_and_offsets() {
        for (offset, value) in [
            (0, 0),
            (4, 7),
            (16, 32),
            (32, 0),
            (92, 0),
            (96, 7),
            (124, 5),
        ] {
            let mut b = fixture();
            b[offset..offset + 4].copy_from_slice(&u32::to_le_bytes(value));
            assert!(parse(&b).is_err(), "{offset}");
        }
    }
    #[test]
    fn row_bounds_and_palette() {
        assert!(check_run(&[1], 0, 1, 1).is_err());
        assert!(check_run(&[0], 1, 1, 1).is_err());
        assert!(check_run(&[], 2, 1, 1).is_err());
        let mut b = fixture();
        b[108..112].copy_from_slice(&2u32.to_le_bytes());
        b[116] = 1;
        assert!(parse(&b).is_err());
    }
    #[test]
    fn later_frame_uses_first_palette_and_shared_record() {
        let mut b = fixture();
        b[12..16].copy_from_slice(&3u32.to_le_bytes());
        // Later unique frame has no local palette; index zero uses first palette.
        for w in [u32::MAX, 4, 0, 1 | (1 << 11), 1, 0, 0, 1, 1] {
            b.extend(w.to_le_bytes());
        }
        let v = parse(&b).unwrap();
        assert_eq!(v["frame_count"], 3);
        assert_eq!(v["frames"][1]["palette_colors"], 0);
        assert_eq!(v["frames"][2]["shared_reference"], 1);
        // Index 1 is invalid against first palette count 1.
        b[148] = 1;
        assert!(parse(&b).is_err());
    }
}
