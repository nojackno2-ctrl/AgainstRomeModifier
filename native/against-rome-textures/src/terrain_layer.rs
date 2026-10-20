//! Terrain grayscale layer files (`boden` / `emboss` / `collision` BMP) and the
//! height-dependent cache invalidation, ported from the C# `TerrainLayerFiles`.

use crate::error::TextureError;
use std::fs;
use std::path::Path;

/// Caches keyed by the sum of all heights; any height edit must delete them.
pub const HEIGHT_DEPENDENT_CACHES: [&str; 4] =
    ["skydens.dat", "visible.dat", "cliprect.dat", "shadows.dat"];

/// Decoded layer: ARGB pixels (top-down, row-major) plus the green channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrainLayer {
    pub width: usize,
    pub height: usize,
    pub argb: Vec<u32>,
    pub green: Vec<u8>,
}

fn bad(msg: &str) -> TextureError {
    TextureError::InvalidFormat(msg.to_string())
}

fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

impl TerrainLayer {
    /// Reads a layer file; `Ok(None)` when the file does not exist (as in C#).
    pub fn read(path: impl AsRef<Path>) -> Result<Option<Self>, TextureError> {
        let path = path.as_ref();
        if !path.is_file() {
            return Ok(None);
        }
        Self::decode(&fs::read(path)?).map(Some)
    }

    /// Decodes uncompressed 8/24/32-bit BMP data (bottom-up or top-down).
    pub fn decode(bytes: &[u8]) -> Result<Self, TextureError> {
        if bytes.len() < 54 || &bytes[0..2] != b"BM" {
            return Err(bad("not a BMP file"));
        }
        let data_off = u32_at(bytes, 10).unwrap_or(0) as usize;
        let hdr = u32_at(bytes, 14).unwrap_or(0) as usize;
        if hdr < 40 {
            return Err(bad("unsupported BMP header"));
        }
        let w = u32_at(bytes, 18).unwrap_or(0) as i32;
        let h = u32_at(bytes, 22).unwrap_or(0) as i32;
        let bpp = u16_at(bytes, 28).unwrap_or(0) as usize;
        if u32_at(bytes, 30).unwrap_or(0) != 0 {
            return Err(bad("compressed BMP is not supported"));
        }
        if w <= 0 || h == 0 || !matches!(bpp, 8 | 24 | 32) {
            return Err(bad("unsupported BMP dimensions or bit depth"));
        }
        let top_down = h < 0;
        let (width, height) = (w as usize, h.unsigned_abs() as usize);
        let stride = (width * bpp + 31) / 32 * 4;
        let needed = stride
            .checked_mul(height)
            .and_then(|n| n.checked_add(data_off))
            .ok_or_else(|| bad("BMP size overflow"))?;
        if needed > bytes.len() {
            return Err(bad("truncated BMP pixel data"));
        }
        let mut palette: Vec<u32> = Vec::new();
        if bpp == 8 {
            let used = match u32_at(bytes, 46).unwrap_or(0) as usize {
                0 => 256,
                n => n.min(256),
            };
            let base = 14 + hdr;
            for i in 0..used {
                let p = bytes
                    .get(base + i * 4..base + i * 4 + 3)
                    .ok_or_else(|| bad("truncated BMP palette"))?;
                palette.push(0xFF00_0000 | (p[2] as u32) << 16 | (p[1] as u32) << 8 | p[0] as u32);
            }
        }
        let mut argb = Vec::with_capacity(width * height);
        for y in 0..height {
            let src_row = if top_down { y } else { height - 1 - y };
            let row = &bytes[data_off + src_row * stride..];
            for x in 0..width {
                let px = match bpp {
                    8 => *palette
                        .get(row[x] as usize)
                        .ok_or_else(|| bad("BMP palette index out of range"))?,
                    24 => {
                        0xFF00_0000
                            | (row[x * 3 + 2] as u32) << 16
                            | (row[x * 3 + 1] as u32) << 8
                            | row[x * 3] as u32
                    }
                    _ => {
                        (row[x * 4 + 3] as u32) << 24
                            | (row[x * 4 + 2] as u32) << 16
                            | (row[x * 4 + 1] as u32) << 8
                            | row[x * 4] as u32
                    }
                };
                argb.push(px);
            }
        }
        let green = argb.iter().map(|p| (p >> 8) as u8).collect();
        Ok(Self { width, height, argb, green })
    }

    /// Re-encodes the layer as a 24-bit bottom-up BMP. Only pixels whose value
    /// changed are rewritten as R=G=B gray; all others keep their original
    /// three channels bit-for-bit.
    pub fn encode_with_green(&self, values: &[u8]) -> Result<Vec<u8>, TextureError> {
        if values.len() != self.width * self.height {
            return Err(TextureError::DimensionMismatch {
                expected: self.width * self.height,
                actual: values.len(),
            });
        }
        let (w, h) = (self.width, self.height);
        let stride = (w * 3 + 3) & !3;
        let image = stride * h;
        let mut out = vec![0u8; 54 + image];
        out[0] = b'B';
        out[1] = b'M';
        out[2..6].copy_from_slice(&((54 + image) as u32).to_le_bytes());
        out[10..14].copy_from_slice(&54u32.to_le_bytes());
        out[14..18].copy_from_slice(&40u32.to_le_bytes());
        out[18..22].copy_from_slice(&(w as i32).to_le_bytes());
        out[22..26].copy_from_slice(&(h as i32).to_le_bytes());
        out[26..28].copy_from_slice(&1u16.to_le_bytes());
        out[28..30].copy_from_slice(&24u16.to_le_bytes());
        out[34..38].copy_from_slice(&(image as u32).to_le_bytes());
        for y in 0..h {
            let row = 54 + (h - 1 - y) * stride;
            for x in 0..w {
                let i = y * w + x;
                let o = row + x * 3;
                if values[i] == self.green[i] {
                    let p = self.argb[i];
                    out[o] = p as u8;
                    out[o + 1] = (p >> 8) as u8;
                    out[o + 2] = (p >> 16) as u8;
                } else {
                    out[o..o + 3].fill(values[i]);
                }
            }
        }
        Ok(out)
    }

    /// Writes the edited layer atomically (temp file + rename).
    pub fn write(&self, path: impl AsRef<Path>, values: &[u8]) -> Result<(), TextureError> {
        let path = path.as_ref();
        let bytes = self.encode_with_green(values)?;
        let tmp = path.with_extension("bmp.tmp");
        fs::write(&tmp, &bytes)?;
        if let Err(e) = fs::rename(&tmp, path) {
            let _ = fs::remove_file(&tmp);
            return Err(e.into());
        }
        Ok(())
    }
}

/// Deletes the height-dependent caches in `map_dir`; returns the removed names.
/// If any deletion fails, already-deleted files are restored before the error returns.
pub fn invalidate_height_caches(map_dir: impl AsRef<Path>) -> Result<Vec<String>, TextureError> {
    let dir = map_dir.as_ref();
    let mut backups: Vec<(String, Vec<u8>)> = Vec::new();
    for name in HEIGHT_DEPENDENT_CACHES {
        let path = dir.join(name);
        if !path.is_file() {
            continue;
        }
        let step = (|| -> std::io::Result<Vec<u8>> {
            let data = fs::read(&path)?;
            let mut perms = fs::metadata(&path)?.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            perms.set_readonly(false);
            fs::set_permissions(&path, perms)?;
            fs::remove_file(&path)?;
            Ok(data)
        })();
        match step {
            Ok(data) => backups.push((name.to_string(), data)),
            Err(e) => {
                for (n, d) in &backups {
                    let _ = fs::write(dir.join(n), d);
                }
                return Err(e.into());
            }
        }
    }
    Ok(backups.into_iter().map(|(n, _)| n).collect())
}
