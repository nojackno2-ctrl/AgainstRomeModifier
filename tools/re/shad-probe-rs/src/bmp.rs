use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Audit {
    pub width: i32,
    pub height: i32,
    pub depth: u16,
    pub stride: u64,
    pub declared_image_bytes: u32,
    pub actual_image_bytes: u64,
    pub image_size_matches: bool,
    pub trailing_bytes: u64,
}

fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
fn half(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

/// General probe audit deliberately reports biSizeImage mismatch, rather than
/// rejecting it. ShadowDocument performs the additional shadow-only checks.
pub fn audit_bmp(bytes: &[u8]) -> Result<Audit, String> {
    if bytes.len() < 54 || half(bytes, 0) != 0x4d42 {
        return Err("Missing/truncated BMP.".into());
    }
    let width = word(bytes, 18) as i32;
    let height = word(bytes, 22) as i32;
    let depth = half(bytes, 28);
    let mut colors = word(bytes, 46);
    if depth == 8 && colors == 0 {
        colors = 256;
    }
    let offset = u64::from(word(bytes, 10));
    if u64::from(word(bytes, 2)) != bytes.len() as u64
        || word(bytes, 6) != 0
        || word(bytes, 14) != 40
        || width <= 0
        || height == 0
        || height == i32::MIN
        || !matches!(depth, 8 | 24)
        || word(bytes, 30) != 0
        || half(bytes, 26) != 1
        || (depth == 8 && colors > 256)
        || offset < 54 + u64::from(colors) * 4
    {
        return Err("Invalid BMP structural extent/layout.".into());
    }
    let stride = (width as u64 * u64::from(depth)).div_ceil(32) * 4;
    let extent = stride * u64::from(height.unsigned_abs());
    if offset
        .checked_add(extent)
        .is_none_or(|end| end > bytes.len() as u64)
    {
        return Err("Invalid BMP structural extent/layout.".into());
    }
    if depth == 8 {
        for y in 0..height.unsigned_abs() as usize {
            let start = offset as usize + y * stride as usize;
            if bytes[start..start + width as usize]
                .iter()
                .any(|&v| u32::from(v) >= colors)
            {
                return Err("BMP palette index out of range.".into());
            }
        }
    }
    let declared = word(bytes, 34);
    Ok(Audit {
        width,
        height,
        depth,
        stride,
        declared_image_bytes: declared,
        actual_image_bytes: extent,
        image_size_matches: declared == 0 || u64::from(declared) == extent,
        trailing_bytes: bytes.len() as u64 - offset - extent,
    })
}
