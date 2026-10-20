use super::error::ShadowError;

/// Single 8-bit BI_RGB shadow BMP (not an ALRA animation container).
///
/// The mask is the raw palette *index*; RGBQUAD colors are ignored (native 0x40FE80 / 0x410380).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowDocument {
    width: usize,
    height: usize,
    stored_stride: usize,
    top_down: bool,
    palette_color_count: usize,
    trailing_byte_count: usize,
    mask: Vec<u8>,
}

fn word(source: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([source[offset], source[offset + 1], source[offset + 2], source[offset + 3]])
}

fn half(source: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([source[offset], source[offset + 1]])
}

impl ShadowDocument {
    /// Strictly parses an owned-bytes shadow BMP. Never panics on arbitrary input.
    pub fn parse(source: &[u8]) -> Result<Self, ShadowError> {
        if source.len() < 54 {
            return Err(ShadowError::invalid("Truncated shadow BMP header."));
        }
        if half(source, 0) != 0x4D42 {
            return Err(ShadowError::invalid("Missing BM signature."));
        }
        if u64::from(word(source, 2)) != source.len() as u64 || word(source, 6) != 0 {
            return Err(ShadowError::invalid("Invalid shadow BMP file size or reserved fields."));
        }
        if word(source, 14) != 40 {
            return Err(ShadowError::unsupported(
                "Only BITMAPINFOHEADER (40 bytes) shadow BMPs are supported.",
            ));
        }
        let width = word(source, 18) as i32;
        let signed_height = word(source, 22) as i32;
        if width <= 0 || signed_height == 0 || signed_height == i32::MIN {
            return Err(ShadowError::invalid("Invalid shadow BMP dimensions."));
        }
        if half(source, 26) != 1 {
            return Err(ShadowError::invalid("Invalid shadow BMP plane count."));
        }
        if half(source, 28) != 8 || word(source, 30) != 0 {
            return Err(ShadowError::unsupported(
                "Only uncompressed 8-bit BI_RGB shadow BMPs are supported.",
            ));
        }
        let height = i64::from(signed_height.unsigned_abs());
        let width = i64::from(width);
        let stride = (width + 3) & !3i64;
        let pixel_bytes = stride * height;
        let colors = match word(source, 46) {
            0 => 256u32,
            c => c,
        };
        let important = word(source, 50);
        let offset = i64::from(word(source, 10));
        let image_size = i64::from(word(source, 34));
        let len = source.len() as i64;
        if colors > 256
            || important > colors
            || offset < 54 + i64::from(colors) * 4
            || offset > len
            || pixel_bytes > len - offset
            || (image_size != 0 && image_size != pixel_bytes)
            || width * height > i64::from(i32::MAX)
        {
            return Err(ShadowError::invalid(
                "Invalid shadow BMP palette, pixel offset, or payload extent.",
            ));
        }
        let (w, h, stride_u, offset_u) =
            (width as usize, height as usize, stride as usize, offset as usize);
        let mut mask = vec![0u8; w * h];
        for y in 0..h {
            let stored_y = if signed_height < 0 { y } else { h - 1 - y };
            let start = offset_u + stored_y * stride_u;
            let row = &source[start..start + w];
            for (x, &index) in row.iter().enumerate() {
                if u32::from(index) >= colors {
                    return Err(ShadowError::invalid("Shadow BMP index exceeds its palette table."));
                }
                mask[y * w + x] = index;
            }
        }
        Ok(Self {
            width: w,
            height: h,
            stored_stride: stride_u,
            top_down: signed_height < 0,
            palette_color_count: colors as usize,
            trailing_byte_count: (len - offset - pixel_bytes) as usize,
            mask,
        })
    }

    pub fn width(&self) -> usize {
        self.width
    }
    pub fn height(&self) -> usize {
        self.height
    }
    pub fn stored_stride(&self) -> usize {
        self.stored_stride
    }
    pub fn is_top_down(&self) -> bool {
        self.top_down
    }
    pub fn palette_color_count(&self) -> usize {
        self.palette_color_count
    }
    pub fn trailing_byte_count(&self) -> usize {
        self.trailing_byte_count
    }
    pub fn frame_count(&self) -> usize {
        1
    }

    /// Top-down intensity mask (0..255) borrowed from the document.
    pub fn mask(&self) -> &[u8] {
        &self.mask
    }

    /// Native strength at pixel (x, y), top-down; `None` outside the image.
    pub fn strength_at(&self, x: usize, y: usize) -> Option<u8> {
        if x < self.width && y < self.height {
            self.mask.get(y * self.width + x).copied()
        } else {
            None
        }
    }

    pub fn decode_frame(&self, frame_index: i32) -> Result<ShadowFrame, ShadowError> {
        if frame_index != 0 {
            return Err(ShadowError::OutOfRange("frame_index"));
        }
        Ok(ShadowFrame::new(self.width, self.height, self.mask.clone()))
    }
}

/// Top-down intensity mask plus black-ARGB preview pixels; the native strength denominator is 256.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowFrame {
    width: usize,
    height: usize,
    mask: Vec<u8>,
    argb: Vec<u32>,
}

impl ShadowFrame {
    fn new(width: usize, height: usize, mask: Vec<u8>) -> Self {
        let argb = mask.iter().map(|&m| ((u32::from(m) * 255 + 128) >> 8) << 24).collect();
        Self { width, height, mask, argb }
    }

    pub fn width(&self) -> usize {
        self.width
    }
    pub fn height(&self) -> usize {
        self.height
    }
    /// Native strength 0..255; 0 keeps the ground color, 255 keeps 1/256 of it.
    pub fn alpha_mask(&self) -> &[u8] {
        &self.mask
    }
    /// Strength mapped to standard alpha/255 (rounded); preview only.
    pub fn argb_pixels(&self) -> &[u32] {
        &self.argb
    }
}

/// Reproduces 0x410380: per-channel `floor(channel * (256 - strength) / 256)`, alpha preserved.
pub fn darken_argb(destination: u32, strength: u8) -> u32 {
    let factor = 256 - u32::from(strength);
    let r = ((destination >> 16) & 255) * factor >> 8;
    let g = ((destination >> 8) & 255) * factor >> 8;
    let b = (destination & 255) * factor >> 8;
    (destination & 0xFF00_0000) | r << 16 | g << 8 | b
}
