//! ALR (ALRA v4..6) sprite animation container parser, decoder, and encoder.
//!
//! Follows native loader `0x4E3A20`, palette helper `0x4E48F0`, and scanline decoders
//! `0x4E49C0` / `0x4E4A60`. Supports versions 4..=6 and 8-bit indexed frames.
//! Frame sharing DAGs (`reference >= 0`), local/first-frame palettes, and trailing metadata
//! (such as IFOM blocks) are safely handled and preserved.

use std::fmt;

use crate::{
    alr_metadata::{MetadataError, SourceHeaderMetadata, HEADER_BYTES, HEADER_WORDS},
    alr_scanlines::{self, IndexedSample, ScanlineError},
};

/// Information header for an ALR frame record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlrFrameInfo {
    /// Horizontal offset in anchor canvas coordinates.
    pub offset_x: i32,
    /// Vertical offset in anchor canvas coordinates.
    pub offset_y: i32,
    /// Frame width in pixels (11 bits, 0..=2047).
    pub width: usize,
    /// Frame height in pixels (11 bits, 0..=2047).
    pub height: usize,
    /// Local palette color count (9 bits, 0..=511).
    pub palette_color_count: usize,
    /// Raw packed size word (width | height << 11 | colors << 22).
    pub packed_size: u32,
    /// Unpadded serialized pixel byte count.
    pub stored_pixel_bytes: u32,
}

impl AlrFrameInfo {
    /// Constructs a frame info record, computing `packed_size` from width, height, and colors.
    pub fn new(
        offset_x: i32,
        offset_y: i32,
        width: usize,
        height: usize,
        palette_color_count: usize,
        stored_pixel_bytes: u32,
    ) -> Self {
        let packed_size = (width as u32 & 0x7FF)
            | ((height as u32 & 0x7FF) << 11)
            | ((palette_color_count as u32 & 0x1FF) << 22);
        Self {
            offset_x,
            offset_y,
            width,
            height,
            palette_color_count,
            packed_size,
            stored_pixel_bytes,
        }
    }
}

/// A parsed frame within an ALR document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlrFrame {
    /// Frame dimensions, offsets, and palette count.
    pub info: AlrFrameInfo,
    /// Serialized payload (contains local palette variants followed by 4-byte aligned pixel bytes).
    pub payload: Vec<u8>,
    /// Row descriptor table (height + 1 words).
    pub rows: Vec<u32>,
    /// Index of the source frame if this frame shares data with an earlier record (`reference >= 0`).
    pub shared_from: Option<usize>,
}

impl AlrFrame {
    /// Returns true if this frame is a shared reference to another frame.
    pub fn is_shared(&self) -> bool {
        self.shared_from.is_some()
    }
}

/// Errors returned when parsing, decoding, or inspecting an ALR document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlrError {
    /// File is shorter than the 23-word (92-byte) header.
    TruncatedHeader { available: usize },
    /// Magic signature is not `0x41524C41` ("ALRA").
    InvalidSignature(u32),
    /// Version is outside 4..=6.
    UnsupportedVersion(u32),
    /// Bit depth is not 8.
    UnsupportedDepth(u32),
    /// Frame count is zero.
    InvalidFrameCount(u32),
    /// Palette variant count is zero.
    InvalidPaletteVariants(u32),
    /// Frame reference list is truncated.
    TruncatedFrameReferences { needed: usize, available: usize },
    /// Shared frame reference (`reference >= 0`) points to a forward or self index.
    SharedFrameInvalidReference { index: usize, reference: usize },
    /// Row table offset in the frame header does not equal the payload byte length.
    InvalidTableOffset {
        table_offset: u32,
        payload_bytes: usize,
    },
    /// Frame payload (palettes + aligned pixels) is truncated.
    TruncatedPayload { needed: usize, available: usize },
    /// Frame row descriptor table is truncated.
    TruncatedRowTable { needed: usize, available: usize },
    /// Requested frame index is out of bounds.
    FrameIndexOutOfRange { index: usize, count: usize },
    /// Requested palette variant index is out of bounds.
    PaletteVariantOutOfRange { variant: usize, count: usize },
    /// The first frame record has no palette colors (colors == 0).
    FirstFrameMissingPalette,
    /// Scanline decoding failed.
    Scanline(ScanlineError),
}

impl fmt::Display for AlrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AlrError::TruncatedHeader { available } => {
                write!(
                    f,
                    "Truncated ALR header: available {available} bytes, expected {HEADER_BYTES}"
                )
            }
            AlrError::InvalidSignature(sig) => {
                write!(
                    f,
                    "Missing ALRA signature (expected 0x41524C41, got 0x{sig:08X})"
                )
            }
            AlrError::UnsupportedVersion(ver) => {
                write!(f, "ALR version {ver} is not supported; expected 4..6")
            }
            AlrError::UnsupportedDepth(depth) => {
                write!(
                    f,
                    "Only 8-bit indexed ALR frames are supported (got depth {depth})"
                )
            }
            AlrError::InvalidFrameCount(cnt) => {
                write!(f, "Invalid ALR frame count ({cnt})")
            }
            AlrError::InvalidPaletteVariants(var) => {
                write!(f, "Invalid ALR palette variant count ({var})")
            }
            AlrError::TruncatedFrameReferences { needed, available } => {
                write!(
                    f,
                    "Truncated ALR frame references: needed {needed} bytes, available {available}"
                )
            }
            AlrError::SharedFrameInvalidReference { index, reference } => {
                write!(
                    f,
                    "ALR shared frame at index {index} must reference an earlier record (got {reference})"
                )
            }
            AlrError::InvalidTableOffset {
                table_offset,
                payload_bytes,
            } => {
                write!(
                    f,
                    "ALR frame row table offset mismatch: {table_offset} != payload {payload_bytes}"
                )
            }
            AlrError::TruncatedPayload { needed, available } => {
                write!(
                    f,
                    "Truncated ALR payload: needed {needed} bytes, available {available}"
                )
            }
            AlrError::TruncatedRowTable { needed, available } => {
                write!(
                    f,
                    "Truncated ALR row table: needed {needed} bytes, available {available}"
                )
            }
            AlrError::FrameIndexOutOfRange { index, count } => {
                write!(f, "Frame index {index} out of range (count {count})")
            }
            AlrError::PaletteVariantOutOfRange { variant, count } => {
                write!(f, "Palette variant {variant} out of range (count {count})")
            }
            AlrError::FirstFrameMissingPalette => {
                write!(f, "Indexed ALR has no palette in its first frame")
            }
            AlrError::Scanline(err) => {
                write!(f, "ALR scanline decode error: {err:?}")
            }
        }
    }
}

impl std::error::Error for AlrError {}

impl From<ScanlineError> for AlrError {
    fn from(err: ScanlineError) -> Self {
        AlrError::Scanline(err)
    }
}

/// An owned, parsed Against Rome ALRA sprite animation document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlrDocument {
    /// File format version (4..=6).
    pub version: u32,
    /// Layout grid columns.
    pub layout_columns: u32,
    /// Layout grid rows.
    pub layout_rows: u32,
    /// Number of palette variants.
    pub palette_variant_count: usize,
    /// Native anchor canvas width.
    pub anchor_width: u32,
    /// Native anchor canvas height.
    pub anchor_height: u32,
    /// All 23 original 32-bit header words.
    pub raw_header_words: [u32; HEADER_WORDS],
    /// All frames in the document, in sequential order.
    pub frames: Vec<AlrFrame>,
    /// Preserved trailing bytes (e.g. IFOM block).
    pub trailing_bytes: Vec<u8>,
}

impl AlrDocument {
    /// Constructs a document with default header words and the specified fields.
    pub fn new(
        version: u32,
        layout_columns: u32,
        layout_rows: u32,
        palette_variant_count: usize,
        anchor_width: u32,
        anchor_height: u32,
        frames: Vec<AlrFrame>,
    ) -> Self {
        let mut words = [0u32; HEADER_WORDS];
        words[0] = 0x4152_4C41;
        words[1] = version;
        words[3] = frames.len() as u32;
        words[4] = 8;
        words[6] = layout_columns;
        words[7] = layout_rows;
        words[8] = palette_variant_count as u32;
        words[15] = anchor_width;
        words[16] = anchor_height;
        Self {
            version,
            layout_columns,
            layout_rows,
            palette_variant_count,
            anchor_width,
            anchor_height,
            raw_header_words: words,
            frames,
            trailing_bytes: Vec::new(),
        }
    }

    /// Parses an ALRA container from raw bytes according to native loader `0x4E3A20`.
    pub fn parse(bytes: &[u8]) -> Result<Self, AlrError> {
        if bytes.len() < HEADER_BYTES {
            return Err(AlrError::TruncatedHeader {
                available: bytes.len(),
            });
        }
        let mut words = [0u32; HEADER_WORDS];
        for (i, chunk) in bytes[..HEADER_BYTES].chunks_exact(4).enumerate() {
            words[i] = u32::from_le_bytes(chunk.try_into().unwrap());
        }

        let sig = words[0];
        if sig != 0x4152_4C41 {
            return Err(AlrError::InvalidSignature(sig));
        }

        let version = words[1];
        if !(4..=6).contains(&version) {
            return Err(AlrError::UnsupportedVersion(version));
        }

        let count = words[3];
        if count == 0 || count > i32::MAX as u32 {
            return Err(AlrError::InvalidFrameCount(count));
        }

        let depth = words[4];
        if depth != 8 {
            return Err(AlrError::UnsupportedDepth(depth));
        }

        let columns = words[6];
        let rows = words[7];
        let variants = words[8];
        if variants == 0 || variants > i32::MAX as u32 {
            return Err(AlrError::InvalidPaletteVariants(variants));
        }

        let anchor_width = words[15];
        let anchor_height = words[16];

        let mut offset = HEADER_BYTES;
        let count_usize = count as usize;
        let ref_bytes = count_usize
            .checked_mul(4)
            .ok_or(AlrError::InvalidFrameCount(count))?;
        if bytes.len().saturating_sub(offset) < ref_bytes {
            return Err(AlrError::TruncatedFrameReferences {
                needed: ref_bytes,
                available: bytes.len().saturating_sub(offset),
            });
        }

        let mut frames: Vec<AlrFrame> = Vec::with_capacity(count_usize);

        for index in 0..count_usize {
            if offset + 4 > bytes.len() {
                return Err(AlrError::TruncatedFrameReferences {
                    needed: 4,
                    available: bytes.len() - offset,
                });
            }
            let reference = i32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
            offset += 4;

            if reference >= 0 {
                let ref_idx = reference as usize;
                if ref_idx >= index {
                    return Err(AlrError::SharedFrameInvalidReference {
                        index,
                        reference: ref_idx,
                    });
                }
                let target = &frames[ref_idx];
                frames.push(AlrFrame {
                    info: target.info,
                    payload: target.payload.clone(),
                    rows: target.rows.clone(),
                    shared_from: Some(ref_idx),
                });
                continue;
            }

            // Independent frame record: 4 words (16 bytes)
            if offset + 16 > bytes.len() {
                return Err(AlrError::TruncatedPayload {
                    needed: 16,
                    available: bytes.len() - offset,
                });
            }
            let table_offset = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
            let position = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap());
            let size = u32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap());
            let stored_pixel_bytes =
                u32::from_le_bytes(bytes[offset + 12..offset + 16].try_into().unwrap());
            offset += 16;

            let width = (size & 0x7FF) as usize;
            let height = ((size >> 11) & 0x7FF) as usize;
            let colors = ((size >> 22) & 0x1FF) as usize;

            let palette_bytes = (variants as usize)
                .checked_mul(colors)
                .and_then(|c| c.checked_mul(4))
                .ok_or(AlrError::InvalidPaletteVariants(variants))?;
            let aligned_pixel_bytes = (stored_pixel_bytes as usize)
                .checked_add(3)
                .map(|b| b & !3)
                .ok_or(AlrError::TruncatedPayload {
                    needed: usize::MAX,
                    available: bytes.len() - offset,
                })?;
            let payload_bytes = palette_bytes
                .checked_add(aligned_pixel_bytes)
                .ok_or(AlrError::TruncatedPayload {
                    needed: usize::MAX,
                    available: bytes.len() - offset,
                })?;
            let table_bytes = height
                .checked_add(1)
                .and_then(|h| h.checked_mul(4))
                .ok_or(AlrError::TruncatedRowTable {
                    needed: usize::MAX,
                    available: bytes.len() - offset,
                })?;

            if table_offset as usize != payload_bytes {
                return Err(AlrError::InvalidTableOffset {
                    table_offset,
                    payload_bytes,
                });
            }

            let total_frame_bytes = payload_bytes
                .checked_add(table_bytes)
                .ok_or(AlrError::TruncatedPayload {
                    needed: usize::MAX,
                    available: bytes.len() - offset,
                })?;

            if bytes.len() - offset < total_frame_bytes {
                return Err(AlrError::TruncatedPayload {
                    needed: total_frame_bytes,
                    available: bytes.len() - offset,
                });
            }

            let payload = bytes[offset..offset + payload_bytes].to_vec();
            offset += payload_bytes;

            let mut row_descriptors = Vec::with_capacity(height + 1);
            for chunk in bytes[offset..offset + table_bytes].chunks_exact(4) {
                row_descriptors.push(u32::from_le_bytes(chunk.try_into().unwrap()));
            }
            offset += table_bytes;

            let offset_x = (position & 0xFFFF) as i32;
            let offset_y = (position >> 16) as i32;

            let info = AlrFrameInfo {
                offset_x,
                offset_y,
                width,
                height,
                palette_color_count: colors,
                packed_size: size,
                stored_pixel_bytes,
            };

            frames.push(AlrFrame {
                info,
                payload,
                rows: row_descriptors,
                shared_from: None,
            });
        }

        let trailing_bytes = bytes[offset..].to_vec();

        Ok(Self {
            version,
            layout_columns: columns,
            layout_rows: rows,
            palette_variant_count: variants as usize,
            anchor_width,
            anchor_height,
            raw_header_words: words,
            frames,
            trailing_bytes,
        })
    }

    /// Serializes this ALR document back into byte-exact binary format.
    pub fn encode(&self) -> Vec<u8> {
        let mut words = self.raw_header_words;
        words[0] = 0x4152_4C41;
        words[1] = self.version;
        words[3] = self.frames.len() as u32;
        words[4] = 8;
        words[6] = self.layout_columns;
        words[7] = self.layout_rows;
        words[8] = self.palette_variant_count as u32;
        words[15] = self.anchor_width;
        words[16] = self.anchor_height;

        let mut buf = Vec::new();
        for w in &words {
            buf.extend_from_slice(&w.to_le_bytes());
        }

        for frame in &self.frames {
            if let Some(shared_idx) = frame.shared_from {
                buf.extend_from_slice(&(shared_idx as i32).to_le_bytes());
            } else {
                buf.extend_from_slice(&(-1i32).to_le_bytes());
                let table_offset = frame.payload.len() as u32;
                buf.extend_from_slice(&table_offset.to_le_bytes());
                let position = ((frame.info.offset_x as u32) & 0xFFFF)
                    | (((frame.info.offset_y as u32) & 0xFFFF) << 16);
                buf.extend_from_slice(&position.to_le_bytes());
                buf.extend_from_slice(&frame.info.packed_size.to_le_bytes());
                buf.extend_from_slice(&frame.info.stored_pixel_bytes.to_le_bytes());
                buf.extend_from_slice(&frame.payload);
                for row in &frame.rows {
                    buf.extend_from_slice(&row.to_le_bytes());
                }
            }
        }

        buf.extend_from_slice(&self.trailing_bytes);
        buf
    }

    /// Returns the number of frames.
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Reads raw detached palette words from the first frame record for the given variant.
    /// Follows native helper `0x4E48F0`, which obtains all frame palettes from the first frame.
    pub fn read_raw_palette(&self, palette_variant: usize) -> Result<Vec<u32>, AlrError> {
        if palette_variant >= self.palette_variant_count {
            return Err(AlrError::PaletteVariantOutOfRange {
                variant: palette_variant,
                count: self.palette_variant_count,
            });
        }
        if self.frames.is_empty() {
            return Err(AlrError::FirstFrameMissingPalette);
        }
        let first = &self.frames[0];
        let colors = first.info.palette_color_count;
        if colors == 0 {
            return Err(AlrError::FirstFrameMissingPalette);
        }
        let start = palette_variant * colors * 4;
        let end = start + colors * 4;
        if end > first.payload.len() {
            return Err(AlrError::TruncatedPayload {
                needed: end,
                available: first.payload.len(),
            });
        }
        let mut palette = Vec::with_capacity(colors);
        for chunk in first.payload[start..end].chunks_exact(4) {
            palette.push(u32::from_le_bytes(chunk.try_into().unwrap()));
        }
        Ok(palette)
    }

    /// Decodes a frame's scanlines into straight-alpha ARGB pixels using the primary palette.
    pub fn decode_frame_argb(
        &self,
        frame_index: usize,
        palette_variant: usize,
    ) -> Result<Vec<u32>, AlrError> {
        if frame_index >= self.frames.len() {
            return Err(AlrError::FrameIndexOutOfRange {
                index: frame_index,
                count: self.frames.len(),
            });
        }
        let palette = self.read_raw_palette(palette_variant)?;
        let frame = &self.frames[frame_index];
        let pixel_start = self.palette_variant_count * frame.info.palette_color_count * 4;
        let pixel_data = frame.payload.get(pixel_start..).unwrap_or(&[]);
        alr_scanlines::decode_argb(
            frame.info.width,
            frame.info.height,
            &frame.rows,
            pixel_data,
            &palette,
        )
        .map_err(AlrError::Scanline)
    }

    /// Decodes a frame's scanlines into structured indexed samples.
    pub fn decode_frame_indexed(
        &self,
        frame_index: usize,
    ) -> Result<Vec<Option<IndexedSample>>, AlrError> {
        if frame_index >= self.frames.len() {
            return Err(AlrError::FrameIndexOutOfRange {
                index: frame_index,
                count: self.frames.len(),
            });
        }
        let frame = &self.frames[frame_index];
        // Matches C# `ReadIndexedSamples`: always the first frame's palette count, so a
        // zero-color first frame is rejected by the scanline decoder rather than substituted.
        let palette_len = self.frames[0].info.palette_color_count;
        let pixel_start = self.palette_variant_count * frame.info.palette_color_count * 4;
        let pixel_data = frame.payload.get(pixel_start..).unwrap_or(&[]);
        alr_scanlines::decode_indexed(
            frame.info.width,
            frame.info.height,
            &frame.rows,
            pixel_data,
            palette_len,
        )
        .map_err(AlrError::Scanline)
    }

    /// Parses normalized metadata from the header using `SourceHeaderMetadata`.
    pub fn source_header(&self) -> Result<SourceHeaderMetadata, MetadataError> {
        SourceHeaderMetadata::from_header_words(
            self.raw_header_words,
            HEADER_BYTES as u64 + self.frames.len() as u64 * 4,
        )
    }
}
