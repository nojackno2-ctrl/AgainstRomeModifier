use crate::error::ProbeError;

/// Pure 24-bit BI_RGB decoder, top-down RGB storage; does not depend on system libraries or installed assets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapLightingBitmap {
    width: usize,
    height: usize,
    rgb: Vec<u8>,
}

impl MapLightingBitmap {
    pub fn parse(source: &[u8]) -> Result<Self, ProbeError> {
        if source.len() < 54 || u16::from_le_bytes([source[0], source[1]]) != 0x4D42 {
            return Err(ProbeError::InvalidData(
                "Missing or truncated lighting BMP header.".into(),
            ));
        }

        let declared_size = read_u32(source, 2);
        let reserved = read_u32(source, 6);
        let offset = read_u32(source, 10);

        if declared_size as usize != source.len() || reserved != 0 {
            return Err(ProbeError::InvalidData(
                "Invalid BMP size or reserved fields.".into(),
            ));
        }

        let header_size = read_u32(source, 14);
        let planes = u16::from_le_bytes([source[26], source[27]]);
        let bit_count = u16::from_le_bytes([source[28], source[29]]);
        let compression = read_u32(source, 30);

        if header_size != 40 || bit_count != 24 || compression != 0 {
            return Err(ProbeError::NotSupported(
                "Only 24-bit BITMAPINFOHEADER BI_RGB lighting BMPs are supported.".into(),
            ));
        }

        let width = i32::from_le_bytes([source[18], source[19], source[20], source[21]]);
        let signed_height = i32::from_le_bytes([source[22], source[23], source[24], source[25]]);

        if width <= 0 || signed_height == 0 || signed_height == i32::MIN || planes != 1 {
            return Err(ProbeError::InvalidData(
                "Invalid BMP dimensions or plane count.".into(),
            ));
        }

        let height = signed_height.abs() as usize;
        let w = width as usize;
        let stride = ((width as u64 * 3 + 3) & !3) as usize;
        let extent = stride as u64 * height as u64;
        let image_size = read_u32(source, 34) as u64;

        if (offset as usize) < 54
            || (offset as usize) > source.len()
            || extent > (source.len() - offset as usize) as u64
            || (image_size != 0 && image_size != extent)
            || (width as u64 * height as u64 * 3 > i32::MAX as u64)
        {
            return Err(ProbeError::InvalidData("Invalid BMP pixel extent.".into()));
        }

        let mut rgb = vec![0u8; w * height * 3];
        let offset_usize = offset as usize;

        for y in 0..height {
            let stored_y = if signed_height < 0 { y } else { height - 1 - y };
            let row_start = offset_usize + stored_y * stride;
            let row = &source[row_start..row_start + w * 3];
            for x in 0..w {
                let dest = (y * w + x) * 3;
                rgb[dest] = row[x * 3 + 2]; // R
                rgb[dest + 1] = row[x * 3 + 1]; // G
                rgb[dest + 2] = row[x * 3]; // B
            }
        }

        Ok(Self {
            width: w,
            height,
            rgb,
        })
    }

    #[inline]
    pub fn width(&self) -> usize {
        self.width
    }

    #[inline]
    pub fn height(&self) -> usize {
        self.height
    }

    #[inline]
    pub fn raw_rgb(&self) -> &[u8] {
        &self.rgb
    }

    /// Returns unnormalized RGB byte values as [r, g, b] floats (0.0..=255.0).
    pub fn color_at(&self, x: i32, y: i32) -> Result<[f32; 3], ProbeError> {
        if x < 0 || x >= self.width as i32 {
            return Err(ProbeError::OutOfRange("x".into()));
        }
        if y < 0 || y >= self.height as i32 {
            return Err(ProbeError::OutOfRange("y".into()));
        }
        let i = (y as usize * self.width + x as usize) * 3;
        Ok([
            self.rgb[i] as f32,
            self.rgb[i + 1] as f32,
            self.rgb[i + 2] as f32,
        ])
    }
}

#[inline]
fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}
