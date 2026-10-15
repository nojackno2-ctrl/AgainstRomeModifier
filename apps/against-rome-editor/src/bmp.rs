//! Windows 24-bit BMP 格式編解碼器
//! 依據 Against Rome 逆向規則：
//! - boden.bmp 的 BMP 尺寸定義頂點網格，遊戲只讀綠通道；Heightmapstep 是世界高度縮放。
//! - collision.bmp 保留 (R+G+B)/3 灰階導航值；0 可通行、非零阻擋。
//! - 讀寫時支援非 257 尺寸（如 TUTOR_01 的 193x193 高度圖與 192x192 碰撞圖）。
//! - 編輯高度時保留原有 BMP 像素的 Red 與 Blue 通道，確保原始烘焙或光照資訊不被破壞。

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BmpImage {
    pub width: usize,
    pub height: usize,
    /// 原始 BGR 資料，每像素 3 位元組 [B, G, R]。
    /// 長度為 width * height * 3。
    /// 座標系已在記憶體中標準化為由頂向下 (y=0 為最頂端，y=height-1 為最底端)。
    pub bgr_pixels: Vec<u8>,
    source: Option<OriginalBmp>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OriginalBmp {
    bytes: Vec<u8>,
    width: usize,
    height: usize,
    pixel_offset: usize,
    stride: usize,
    top_down: bool,
}

#[derive(Debug)]
pub enum BmpError {
    InvalidMagic,
    TooSmall(usize),
    InvalidDimensions(i32, i32),
    UnsupportedBitsPerPixel(u16),
    UnsupportedCompression(u32),
    BufferTooShort { expected: usize, actual: usize },
    InvalidHeader,
    SizeOverflow,
}

impl fmt::Display for BmpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BmpError::InvalidMagic => write!(f, "Invalid BMP magic identifier (expected 'BM')"),
            BmpError::InvalidHeader => write!(f, "Invalid or overlapping BMP header/pixel layout"),
            BmpError::SizeOverflow => write!(
                f,
                "BMP dimensions exceed supported address space or size limit"
            ),
            BmpError::TooSmall(len) => write!(f, "BMP payload too small: {} bytes", len),
            BmpError::InvalidDimensions(w, h) => write!(f, "Invalid BMP dimensions: {}x{}", w, h),
            BmpError::UnsupportedBitsPerPixel(bpp) => {
                write!(
                    f,
                    "Unsupported BMP bit depth: {} (only 24-bit supported)",
                    bpp
                )
            }
            BmpError::UnsupportedCompression(c) => {
                write!(
                    f,
                    "Unsupported BMP compression: {} (only uncompressed supported)",
                    c
                )
            }
            BmpError::BufferTooShort { expected, actual } => write!(
                f,
                "BMP buffer truncated: expected at least {} bytes, got {}",
                expected, actual
            ),
        }
    }
}

impl std::error::Error for BmpError {}

impl BmpImage {
    /// 建立全新純色 24-bit 圖像
    pub fn new_solid(width: usize, height: usize, b: u8, g: u8, r: u8) -> Self {
        let total_pixels = width * height;
        let mut bgr_pixels = Vec::with_capacity(total_pixels * 3);
        for _ in 0..total_pixels {
            bgr_pixels.push(b);
            bgr_pixels.push(g);
            bgr_pixels.push(r);
        }
        Self {
            width,
            height,
            bgr_pixels,
            source: None,
        }
    }

    /// 解析 24-bit Windows BMP 位元組流
    pub fn parse(bytes: &[u8]) -> Result<Self, BmpError> {
        if bytes.len() < 54 {
            return Err(BmpError::TooSmall(bytes.len()));
        }

        if bytes[0] != b'B' || bytes[1] != b'M' {
            return Err(BmpError::InvalidMagic);
        }

        let pixel_offset =
            u32::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]) as usize;
        let header_size = u32::from_le_bytes([bytes[14], bytes[15], bytes[16], bytes[17]]);
        let header_end = 14usize
            .checked_add(header_size as usize)
            .ok_or(BmpError::SizeOverflow)?;
        if header_size < 40 || header_end > bytes.len() || pixel_offset < header_end {
            return Err(BmpError::InvalidHeader);
        }

        let width_raw = i32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]);
        let height_raw = i32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]);

        if width_raw <= 0 || height_raw == 0 || height_raw == i32::MIN {
            return Err(BmpError::InvalidDimensions(width_raw, height_raw));
        }

        let width = width_raw as usize;
        let is_top_down = height_raw < 0;
        let height = height_raw.unsigned_abs() as usize;

        let planes = u16::from_le_bytes([bytes[26], bytes[27]]);
        let bpp = u16::from_le_bytes([bytes[28], bytes[29]]);
        let compression = u32::from_le_bytes([bytes[30], bytes[31], bytes[32], bytes[33]]);

        if planes != 1 || bpp != 24 {
            return Err(BmpError::UnsupportedBitsPerPixel(bpp));
        }
        if compression != 0 {
            return Err(BmpError::UnsupportedCompression(compression));
        }

        // 4-byte 對齊 stride
        let row_bytes = width.checked_mul(3).ok_or(BmpError::SizeOverflow)?;
        let stride = row_bytes.checked_add(3).ok_or(BmpError::SizeOverflow)? & !3;
        let image_bytes = stride.checked_mul(height).ok_or(BmpError::SizeOverflow)?;
        let required_bytes = pixel_offset
            .checked_add(image_bytes)
            .ok_or(BmpError::SizeOverflow)?;
        if bytes.len() < required_bytes {
            return Err(BmpError::BufferTooShort {
                expected: required_bytes,
                actual: bytes.len(),
            });
        }

        let declared_size = u32::from_le_bytes(bytes[2..6].try_into().unwrap()) as usize;
        if declared_size != 0 && (declared_size < required_bytes || declared_size > bytes.len()) {
            return Err(BmpError::InvalidHeader);
        }
        let declared_image = u32::from_le_bytes(bytes[34..38].try_into().unwrap()) as usize;
        if declared_image != 0
            && (declared_image < image_bytes
                || pixel_offset
                    .checked_add(declared_image)
                    .is_none_or(|end| end > bytes.len()))
        {
            return Err(BmpError::InvalidHeader);
        }
        let pixel_count = width.checked_mul(height).ok_or(BmpError::SizeOverflow)?;
        // Editor assets are small grids. Refuse oversized untrusted allocations.
        if pixel_count > 64 * 1024 * 1024 {
            return Err(BmpError::SizeOverflow);
        }

        let mut bgr_pixels = vec![0u8; width * height * 3];

        for y in 0..height {
            // BMP 預設為 bottom-up (height > 0)
            let row_in_file = if is_top_down { y } else { height - 1 - y };
            let src_row_start = pixel_offset + row_in_file * stride;
            let dst_row_start = y * width * 3;

            for x in 0..width {
                let src_idx = src_row_start + x * 3;
                let dst_idx = dst_row_start + x * 3;
                bgr_pixels[dst_idx] = bytes[src_idx]; // Blue
                bgr_pixels[dst_idx + 1] = bytes[src_idx + 1]; // Green
                bgr_pixels[dst_idx + 2] = bytes[src_idx + 2]; // Red
            }
        }

        Ok(Self {
            width,
            height,
            bgr_pixels,
            source: Some(OriginalBmp {
                bytes: bytes.to_vec(),
                width,
                height,
                pixel_offset,
                stride,
                top_down: is_top_down,
            }),
        })
    }

    /// 編碼為標準 24-bit bottom-up Windows BMP 位元組流
    pub fn encode_bmp(&self) -> Vec<u8> {
        assert_eq!(
            self.width
                .checked_mul(self.height)
                .and_then(|v| v.checked_mul(3)),
            Some(self.bgr_pixels.len()),
            "invalid BMP pixel buffer"
        );
        if let Some(source) = &self.source {
            if source.width == self.width && source.height == self.height {
                let mut bytes = source.bytes.clone();
                for y in 0..self.height {
                    let row = if source.top_down {
                        y
                    } else {
                        self.height - 1 - y
                    };
                    let dest = source.pixel_offset + row * source.stride;
                    let src = y * self.width * 3;
                    bytes[dest..dest + self.width * 3]
                        .copy_from_slice(&self.bgr_pixels[src..src + self.width * 3]);
                }
                return bytes;
            }
        }
        let stride = (self.width * 3 + 3) & !3;
        let image_size = stride * self.height;
        let file_size = 54 + image_size;

        let mut output = vec![0u8; file_size];

        // BITMAPFILEHEADER (14 bytes)
        output[0] = b'B';
        output[1] = b'M';
        output[2..6].copy_from_slice(&(file_size as u32).to_le_bytes());
        // reserved [6..10] = 0
        output[10..14].copy_from_slice(&54u32.to_le_bytes()); // offset to pixel array

        // BITMAPINFOHEADER (40 bytes)
        output[14..18].copy_from_slice(&40u32.to_le_bytes()); // biSize
        output[18..22].copy_from_slice(&(self.width as i32).to_le_bytes()); // biWidth
        output[22..26].copy_from_slice(&(self.height as i32).to_le_bytes()); // biHeight (bottom-up)
        output[26..28].copy_from_slice(&1u16.to_le_bytes()); // biPlanes
        output[28..30].copy_from_slice(&24u16.to_le_bytes()); // biBitCount = 24
        output[30..34].copy_from_slice(&0u32.to_le_bytes()); // biCompression = BI_RGB
        output[34..38].copy_from_slice(&(image_size as u32).to_le_bytes()); // biSizeImage
                                                                            // biXPelsPerMeter, biYPelsPerMeter, biClrUsed, biClrImportant = 0

        // 寫入像素（Bottom-up）
        for y in 0..self.height {
            let row_in_file = self.height - 1 - y;
            let dst_row_start = 54 + row_in_file * stride;
            let src_row_start = y * self.width * 3;

            for x in 0..self.width {
                let src_idx = src_row_start + x * 3;
                let dst_idx = dst_row_start + x * 3;
                output[dst_idx] = self.bgr_pixels[src_idx]; // Blue
                output[dst_idx + 1] = self.bgr_pixels[src_idx + 1]; // Green
                output[dst_idx + 2] = self.bgr_pixels[src_idx + 2]; // Red
            }
            // 剩餘的 stride padding bytes 預設已為 0
        }

        output
    }

    /// 提取綠通道 (Green Channel) 作為高度網格
    pub fn extract_green_channel(&self) -> Vec<u8> {
        let total = self.width * self.height;
        let mut green = Vec::with_capacity(total);
        for i in 0..total {
            green.push(self.bgr_pixels[i * 3 + 1]);
        }
        green
    }

    /// 更新綠通道，同時嚴格保留原有的 Blue 與 Red 通道！
    pub fn update_green_channel(&mut self, new_green: &[u8]) {
        assert_eq!(
            new_green.len(),
            self.width * self.height,
            "Green channel buffer length must match image pixel count"
        );
        for (i, &g) in new_green.iter().enumerate() {
            self.bgr_pixels[i * 3 + 1] = g;
        }
    }

    /// 從單一綠通道高度資料建立灰階 BMP (若無原圖時使用)
    pub fn from_green_channel_grayscale(width: usize, height: usize, green: &[u8]) -> Self {
        assert_eq!(width * height, green.len());
        let mut bgr_pixels = Vec::with_capacity(width * height * 3);
        for &g in green {
            bgr_pixels.push(g); // Blue = g
            bgr_pixels.push(g); // Green = g
            bgr_pixels.push(g); // Red = g
        }
        Self {
            width,
            height,
            bgr_pixels,
            source: None,
        }
    }

    /// Match the existing editor's integer RGB average, retaining nonbinary values.
    pub fn extract_collision_channel(&self) -> Vec<u8> {
        let total = self.width * self.height;
        let mut collision = Vec::with_capacity(total);
        for i in 0..total {
            let b = self.bgr_pixels[i * 3] as u32;
            let g = self.bgr_pixels[i * 3 + 1] as u32;
            let r = self.bgr_pixels[i * 3 + 2] as u32;
            let avg = (b + g + r) / 3;
            collision.push(avg as u8);
        }
        collision
    }

    /// 從通行網格建立黑白 BMP (255 為白，0 為黑)
    pub fn from_collision_channel(width: usize, height: usize, collision: &[u8]) -> Self {
        assert_eq!(width * height, collision.len());
        let mut bgr_pixels = Vec::with_capacity(width * height * 3);
        for &val in collision {
            let color = val;
            bgr_pixels.push(color);
            bgr_pixels.push(color);
            bgr_pixels.push(color);
        }
        Self {
            width,
            height,
            bgr_pixels,
            source: None,
        }
    }

    /// Rewrite only pixels whose navigation value changed. Untouched colored
    /// pixels keep all three source channels rather than becoming grayscale.
    pub fn update_collision_channel(&mut self, collision: &[u8]) {
        assert_eq!(collision.len(), self.width * self.height);
        for (pixel, &value) in self.bgr_pixels.chunks_exact_mut(3).zip(collision) {
            let old = (pixel[0] as u16 + pixel[1] as u16 + pixel[2] as u16) / 3;
            if old != value as u16 {
                pixel.fill(value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bmp_roundtrip_standard_257() {
        let width = 257;
        let height = 257;
        let original = BmpImage::new_solid(width, height, 10, 20, 30);
        let encoded = original.encode_bmp();
        let decoded = BmpImage::parse(&encoded).expect("BMP parse failed");

        assert_eq!(decoded.width, 257);
        assert_eq!(decoded.height, 257);
        assert_eq!(decoded.bgr_pixels, original.bgr_pixels);
    }

    #[test]
    fn test_bmp_roundtrip_non_257_sizes() {
        // TUTOR_01 的 193x193 高度圖
        let original_193 = BmpImage::new_solid(193, 193, 40, 50, 60);
        let encoded_193 = original_193.encode_bmp();
        let decoded_193 = BmpImage::parse(&encoded_193).expect("193x193 BMP parse failed");
        assert_eq!(decoded_193.width, 193);
        assert_eq!(decoded_193.height, 193);
        assert_eq!(decoded_193.bgr_pixels, original_193.bgr_pixels);

        // 192x192 碰撞圖
        let original_192 = BmpImage::new_solid(192, 192, 0, 255, 0);
        let encoded_192 = original_192.encode_bmp();
        let decoded_192 = BmpImage::parse(&encoded_192).expect("192x192 BMP parse failed");
        assert_eq!(decoded_192.width, 192);
        assert_eq!(decoded_192.height, 192);
        assert_eq!(decoded_192.bgr_pixels, original_192.bgr_pixels);
    }

    #[test]
    fn test_green_channel_preservation_other_channels_intact() {
        let width = 4;
        let height = 4;
        // 初始像素具備獨特的 B 與 R 值
        let mut img = BmpImage::new_solid(width, height, 111, 50, 222);

        // 修改綠通道
        let mut new_green = vec![0u8; width * height];
        new_green[0] = 77;
        new_green[5] = 88;
        new_green[15] = 99;

        img.update_green_channel(&new_green);

        // 驗證綠通道已更新
        assert_eq!(img.bgr_pixels[0 * 3 + 1], 77);
        assert_eq!(img.bgr_pixels[5 * 3 + 1], 88);
        assert_eq!(img.bgr_pixels[15 * 3 + 1], 99);

        // 驗證 Blue 和 Red 通道完好如初！
        for i in 0..(width * height) {
            assert_eq!(img.bgr_pixels[i * 3], 111, "Blue channel preserved");
            assert_eq!(img.bgr_pixels[i * 3 + 2], 222, "Red channel preserved");
        }
    }

    #[test]
    fn test_collision_extraction() {
        let width = 2;
        let height = 2;
        let mut img = BmpImage::new_solid(width, height, 0, 0, 0); // 全黑 = 通行
        img.bgr_pixels[0 * 3] = 255; // 點 0 設為有顏色 (阻擋)

        let col = img.extract_collision_channel();
        assert_eq!(col[0], 85);
        assert_eq!(col[1], 0);
        assert_eq!(col[2], 0);
        assert_eq!(col[3], 0);
    }
}
