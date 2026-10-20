//! Save-preview / UI sprite TGA decoder, ported from src.Core/Core/Services/TgaDecoder.cs.

/// Tightly packed top-down BGRA pixels; `bgra.len() == width * height * 4`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TgaImage {
    pub width: usize,
    pub height: usize,
    pub bgra: Vec<u8>,
}

/// Supports 8-bit paletted (24-bit palette) and 24/32-bit true colour; anything else or truncated data yields `None`.
pub fn decode(tga: &[u8]) -> Option<TgaImage> {
    if tga.len() < 18 {
        return None;
    }
    let id_length = tga[0] as usize;
    let image_type = tga[2];
    let width = u16::from_le_bytes([tga[12], tga[13]]) as usize;
    let height = u16::from_le_bytes([tga[14], tga[15]]) as usize;
    let depth = tga[16];
    let top_to_bottom = tga[17] & 0x20 != 0;
    let pixels = width.checked_mul(height)?;
    let output_len = pixels.checked_mul(4)?;
    if width == 0 || height == 0 || output_len > i32::MAX as usize {
        return None;
    }
    // Validate the complete payload before allocating from untrusted thumbnail dimensions.
    let start = 18usize.checked_add(id_length)?;
    let required = match image_type {
        1 if tga[1] == 1 && depth == 8 && tga[7] == 24 => {
            let palette_len = u16::from_le_bytes([tga[5], tga[6]]) as usize;
            start
                .checked_add(palette_len.checked_mul(3)?)?
                .checked_add(pixels)?
        }
        2 if depth == 24 || depth == 32 => {
            start.checked_add(pixels.checked_mul(depth as usize / 8)?)?
        }
        _ => return None,
    };
    if required > tga.len() {
        return None;
    }
    let row_bytes = width * 4;
    let mut bgra = Vec::new();
    bgra.try_reserve_exact(output_len).ok()?;
    bgra.resize(output_len, 0);
    let row_of = |y: usize| if top_to_bottom { y } else { height - 1 - y } * row_bytes;
    match image_type {
        1 => {
            if tga[1] != 1 || depth != 8 || tga[7] != 24 {
                return None;
            }
            let map_len = u16::from_le_bytes([tga[5], tga[6]]) as usize;
            let map_offset = 18 + id_length;
            let data_offset = map_offset + map_len * 3;
            if data_offset + width * height > tga.len() {
                return None;
            }
            let mut palette = vec![0u8; map_len * 4];
            for i in 0..map_len {
                let e = map_offset + i * 3;
                if e + 2 >= tga.len() {
                    break;
                }
                let (b, g, r) = (tga[e], tga[e + 1], tga[e + 2]);
                palette[i * 4..i * 4 + 4].copy_from_slice(&[
                    b,
                    g,
                    r,
                    if r == 0 && g == 0 && b == 0 { 0 } else { 255 },
                ]);
            }
            for y in 0..height {
                let target = row_of(y);
                for x in 0..width {
                    let index = tga[data_offset + y * width + x] as usize;
                    let p = target + x * 4;
                    if index < map_len {
                        bgra[p..p + 4].copy_from_slice(&palette[index * 4..index * 4 + 4]);
                    } else {
                        bgra[p..p + 4].copy_from_slice(&[255, 255, 255, 0]);
                    }
                }
            }
            Some(TgaImage {
                width,
                height,
                bgra,
            })
        }
        2 => {
            if depth != 24 && depth != 32 {
                return None;
            }
            let bpp = depth as usize / 8;
            let data_offset = 18 + id_length;
            if data_offset + width * height * bpp > tga.len() {
                return None;
            }
            for y in 0..height {
                let target = row_of(y);
                for x in 0..width {
                    let s = data_offset + (y * width + x) * bpp;
                    let p = target + x * 4;
                    bgra[p..p + 3].copy_from_slice(&tga[s..s + 3]);
                    bgra[p + 3] = if bpp == 4 { tga[s + 3] } else { 255 };
                }
            }
            Some(TgaImage {
                width,
                height,
                bgra,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn header(kind: u8, w: u16, h: u16, depth: u8, desc: u8) -> Vec<u8> {
        let mut v = vec![0u8; 18];
        v[2] = kind;
        v[12..14].copy_from_slice(&w.to_le_bytes());
        v[14..16].copy_from_slice(&h.to_le_bytes());
        v[16] = depth;
        v[17] = desc;
        v
    }
    #[test]
    fn truecolor_bottom_up_flips_rows_and_24bit_is_opaque() {
        let mut t = header(2, 1, 2, 24, 0);
        t.extend([1, 2, 3, 4, 5, 6]); // file row 0 is the bottom row
        assert_eq!(decode(&t).unwrap().bgra, vec![4, 5, 6, 255, 1, 2, 3, 255]);
    }
    #[test]
    fn paletted_black_is_transparent_and_out_of_range_is_white_transparent() {
        let mut t = header(1, 2, 1, 8, 0x20);
        t[1] = 1;
        t[5] = 2;
        t[7] = 24;
        t.extend([0, 0, 0, 9, 8, 7]); // palette: black, (b9 g8 r7)
        t.extend([0, 1]);
        assert_eq!(decode(&t).unwrap().bgra, vec![0, 0, 0, 0, 9, 8, 7, 255]);
        let n = t.len();
        t[n - 1] = 5;
        assert_eq!(&decode(&t).unwrap().bgra[4..], &[255, 255, 255, 0]);
    }
    #[test]
    fn short_truncated_and_unsupported_inputs_are_rejected() {
        assert!(decode(&[0; 17]).is_none());
        assert!(decode(&header(2, 2, 2, 24, 0)).is_none());
        assert!(decode(&header(3, 1, 1, 8, 0)).is_none());
        assert!(decode(&header(2, 0, 1, 24, 0)).is_none());
    }
}
