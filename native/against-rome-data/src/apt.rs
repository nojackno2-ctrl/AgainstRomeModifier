//! APT (APAT v2/v3) indexed diamond patches document parser and raster decoder.
//!
//! Follows native loader `0x4E5710` and row helper `0x4E6CD0`.
//! Raster patches are decoded from owned bytes; frame light/smoke points and markers
//! are parsed and exposed via safe public APIs.

use std::fmt;

/// Maximum canvas dimension allowed for APT assets.
pub const MAX_CANVAS_DIMENSION: u32 = 2048;
/// Maximum palette color count.
pub const MAX_PALETTE_COLORS: u32 = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AptError {
    MissingSignature(u32),
    UnsupportedVersion(u32),
    UnsupportedFormat {
        width: u32,
        height: u32,
        depth: u32,
    },
    UnsupportedGroupVariants(u32),
    CountOutOfBounds(u32),
    InvalidCanvasDimensions {
        width: u32,
        height: u32,
    },
    InvalidPaletteDimensions {
        colors: u32,
        variants: u32,
    },
    HeaderOverlapsTables(u32),
    RowWidthExceedsDiamond {
        row: usize,
        width: u32,
    },
    TruncatedData {
        needed: usize,
        available: usize,
    },
    FrameReferencesOutOfRange {
        first: u32,
        count: u32,
        tile_count: u32,
    },
    PalettesTooLarge,
    MissingPdatBlock,
    TileOffsetOutsidePixelBlock {
        offset: usize,
        pixel_len: usize,
    },
    LayoutZeroAxis,
    AttachmentGroupMissing {
        index: usize,
        count: usize,
    },
    TileIndexOutOfRange {
        index: usize,
        count: usize,
    },
    FrameIndexOutOfRange {
        index: usize,
        count: usize,
    },
    PaletteVariantOutOfRange {
        variant: usize,
        count: usize,
    },
    CompressedTileMissingRowTable,
    CompressedPixelsOverlapRowTable,
    InvalidPixelRow {
        row: usize,
    },
    MissingPaletteColor {
        color: usize,
        color_count: usize,
    },
    VisiblePixelOutsideCanvas {
        x: i32,
        y: i32,
        width: usize,
        height: usize,
    },
    ParentCoordinateNotFinite,
    ParentCoordinateOutOfBounds,
}

impl fmt::Display for AptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AptError::MissingSignature(sig) => write!(f, "Missing APAT signature (got 0x{sig:08X})"),
            AptError::UnsupportedVersion(ver) => write!(f, "APT version {ver} is not supported; expected 2..3"),
            AptError::UnsupportedFormat { width, height, depth } => write!(
                f,
                "Only 8-bit APT 64x31 diamond patches are supported (got {width}x{height}, depth {depth})"
            ),
            AptError::UnsupportedGroupVariants(v) => write!(f, "APT group variants {v} exceed the native three slots"),
            AptError::CountOutOfBounds(cnt) => write!(f, "APT count {cnt} exceeds supported bounds"),
            AptError::InvalidCanvasDimensions { width, height } => {
                write!(f, "Invalid APT canvas dimensions ({width}x{height})")
            }
            AptError::InvalidPaletteDimensions { colors, variants } => {
                write!(f, "Invalid APT palette dimensions ({colors} colors, {variants} variants)")
            }
            AptError::HeaderOverlapsTables(offset) => {
                write!(f, "APT header overlaps its tables (offset {offset} < 112)")
            }
            AptError::RowWidthExceedsDiamond { row, width } => {
                write!(f, "APT row {row} width {width} exceeds its diamond (max 64)")
            }
            AptError::TruncatedData { needed, available } => {
                write!(f, "Truncated APT data (needed {needed} bytes, available {available})")
            }
            AptError::FrameReferencesOutOfRange { first, count, tile_count } => write!(
                f,
                "APT frame references tiles outside its table (first {first} + count {count} > {tile_count})"
            ),
            AptError::PalettesTooLarge => write!(f, "APT palettes are too large"),
            AptError::MissingPdatBlock => write!(f, "Missing APT PDAT block"),
            AptError::TileOffsetOutsidePixelBlock { offset, pixel_len } => write!(
                f,
                "APT tile points outside its pixel block (offset {offset} + 8 > {pixel_len})"
            ),
            AptError::LayoutZeroAxis => write!(f, "APT attachment layout has a zero axis"),
            AptError::AttachmentGroupMissing { index, count } => {
                write!(f, "APT attachment group is missing (index {index} >= {count})")
            }
            AptError::TileIndexOutOfRange { index, count } => {
                write!(f, "Tile index {index} out of range (count {count})")
            }
            AptError::FrameIndexOutOfRange { index, count } => {
                write!(f, "Frame index {index} out of range (count {count})")
            }
            AptError::PaletteVariantOutOfRange { variant, count } => {
                write!(f, "Palette variant {variant} out of range (count {count})")
            }
            AptError::CompressedTileMissingRowTable => write!(f, "APT compressed tile is missing its row table"),
            AptError::CompressedPixelsOverlapRowTable => {
                write!(f, "APT pixels overlap the compressed row table")
            }
            AptError::InvalidPixelRow { row } => {
                write!(f, "APT row {row} has invalid pixel offsets or width")
            }
            AptError::MissingPaletteColor { color, color_count } => {
                write!(f, "APT pixel references a missing palette color ({color} >= {color_count})")
            }
            AptError::VisiblePixelOutsideCanvas { x, y, width, height } => write!(
                f,
                "APT visible pixel extends outside its canvas at ({x}, {y}) for {width}x{height}"
            ),
            AptError::ParentCoordinateNotFinite => write!(f, "APT parent coordinate is not finite"),
            AptError::ParentCoordinateOutOfBounds => {
                write!(f, "APT parent coordinate exceeds signed native bounds")
            }
        }
    }
}

impl std::error::Error for AptError {}

/// A 2D integer point used in APT markers, attachments, and projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct AptPoint {
    pub x: i32,
    pub y: i32,
}

impl AptPoint {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// A range of consecutive tiles that compose an animation frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AptFrameRange {
    pub first_tile: usize,
    pub tile_count: usize,
}

impl AptFrameRange {
    pub const fn new(first_tile: usize, tile_count: usize) -> Self {
        Self { first_tile, tile_count }
    }
}

/// Information and layout placement for one 64x31 diamond tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AptTileInfo {
    pub data_offset: usize,
    pub packed_position: u32,
    pub auxiliary_bytes: u32,
}

impl AptTileInfo {
    pub const fn new(data_offset: usize, packed_position: u32, auxiliary_bytes: u32) -> Self {
        Self {
            data_offset,
            packed_position,
            auxiliary_bytes,
        }
    }

    #[inline]
    pub fn x(&self) -> i32 {
        (self.packed_position & 0x7FF) as i32
    }

    #[inline]
    pub fn y(&self) -> i32 {
        ((self.packed_position >> 11) & 0x7FF) as i32
    }
}

/// Attachment group containing optional marker, variant light points, and smoke points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AptAttachmentGroup {
    pub marker: AptPoint,
    pub light_variants: Vec<Vec<AptPoint>>,
    pub smoke_points: Vec<AptPoint>,
}

/// Result of an attachment query for a specific state / variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AptAttachments<'a> {
    pub lights: &'a [AptPoint],
    pub smoke: &'a [AptPoint],
    pub marker: Option<AptPoint>,
}

/// Decoded 32-bit ARGB raster image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AptIndexedImage {
    pub width: usize,
    pub height: usize,
    pub argb_pixels: Vec<u32>,
}

/// Converts native palette DWORD `0x00BBGGRR` to 32-bit ARGB `0xFFAARRGGBB`.
#[inline]
pub const fn palette_to_argb(stored: u32) -> u32 {
    0xFF00_0000 | ((stored & 0xFF) << 16) | (stored & 0xFF00) | ((stored >> 16) & 0xFF)
}

/// APAT v2/v3 indexed diamond patch document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AptDocument {
    pub version: u32,
    pub width: usize,
    pub height: usize,
    pub anchor_x: i32,
    pub anchor_y: i32,
    pub projection_origin: AptPoint,
    pub layout: [u32; 4],
    pub palette_color_count: usize,
    pub palette_variant_count: usize,
    pub frames: Vec<AptFrameRange>,
    pub tiles: Vec<AptTileInfo>,
    pub attachment_groups: Vec<AptAttachmentGroup>,

    row_offsets: [u32; 31],
    row_widths: [u32; 31],
    palettes: Vec<u32>,
    data: Vec<u8>,
}

impl AptDocument {
    /// Parses an APT document from raw file bytes.
    pub fn parse(source: &[u8]) -> Result<Self, AptError> {
        let mut reader = Reader::new(source);
        let header = reader.words(28)?;

        if header[0] != 0x54415041 {
            return Err(AptError::MissingSignature(header[0]));
        }

        let version = header[1];
        if version < 2 || version > 3 {
            return Err(AptError::UnsupportedVersion(version));
        }

        if header[6] != 64 || header[7] != 31 || header[13] != 8 {
            return Err(AptError::UnsupportedFormat {
                width: header[6],
                height: header[7],
                depth: header[13],
            });
        }

        let frame_count = validate_count(header[2])?;
        let tile_count = validate_count(header[4])?;
        let group_variants = validate_count(header[9])?;
        if group_variants < 1 || group_variants > 3 {
            return Err(AptError::UnsupportedGroupVariants(header[9]));
        }

        let width = validate_count(header[23])?;
        let height = validate_count(header[24])?;
        if width < 1 || width > MAX_CANVAS_DIMENSION as usize || height < 1 || height > MAX_CANVAS_DIMENSION as usize {
            return Err(AptError::InvalidCanvasDimensions {
                width: header[23],
                height: header[24],
            });
        }

        let colors = validate_count(header[20])?;
        let variants = validate_count(header[21])?;
        if colors < 1 || colors > MAX_PALETTE_COLORS as usize || variants < 1 {
            return Err(AptError::InvalidPaletteDimensions {
                colors: header[20],
                variants: header[21],
            });
        }

        if header[3] < 112 {
            return Err(AptError::HeaderOverlapsTables(header[3]));
        }
        reader.skip((header[3] - 112) as usize)?;

        let mut row_offsets = [0u32; 31];
        let mut row_widths = [0u32; 31];
        for y in 0..31 {
            row_offsets[y] = reader.word()?;
        }
        for y in 0..31 {
            let w = reader.word()?;
            if w > 64 {
                return Err(AptError::RowWidthExceedsDiamond { row: y, width: w });
            }
            row_widths[y] = w;
        }

        let projection_origin = AptPoint::new(reader.word()? as i32, reader.word()? as i32);
        let anchor_x = reader.word()? as i32;
        let anchor_y = reader.word()? as i32;
        let extra_a = reader.word()?;
        let extra_b = reader.word()?;
        let groups = validate_count(reader.word()?)?;

        let group_stride = if version == 3 { 6 + group_variants } else { 7 };
        let group_header_bytes = (groups as u64)
            .checked_mul(group_stride as u64)
            .and_then(|w| w.checked_mul(4))
            .ok_or(AptError::CountOutOfBounds(u32::MAX))?;
        reader.require_u64(group_header_bytes)?;

        let mut group_counts = Vec::with_capacity(groups);
        let mut markers = Vec::with_capacity(groups);
        for _ in 0..groups {
            markers.push(AptPoint::new(reader.word()? as i32, reader.word()? as i32));
            let first = reader.word()?;
            let second = reader.word()?;
            let variant_count = if version == 3 { group_variants } else { 1 };
            let per_variant = reader.words(variant_count)?;
            let tail1 = reader.word()?;
            let tail2 = reader.word()?;

            let mut counts = Vec::with_capacity(4 + per_variant.len());
            counts.push(first);
            counts.push(second);
            counts.extend(per_variant);
            counts.push(tail1);
            counts.push(tail2);
            group_counts.push(counts);
        }

        let extra_skip = (extra_a as u64)
            .checked_add(extra_b as u64)
            .and_then(|sum| sum.checked_mul(8))
            .ok_or(AptError::CountOutOfBounds(u32::MAX))?;
        reader.skip_u64(extra_skip)?;

        let mut attachments = Vec::with_capacity(groups);
        for group in 0..groups {
            let counts = &group_counts[group];
            let skip1 = (counts[0] as u64)
                .checked_mul(12)
                .and_then(|v| v.checked_add((counts[1] as u64).checked_mul(8)?))
                .ok_or(AptError::CountOutOfBounds(u32::MAX))?;
            reader.skip_u64(skip1)?;

            let mut light_variants: Vec<Vec<AptPoint>> = Vec::with_capacity(group_variants);
            for i in 0..group_variants {
                if version == 2 && i > 0 {
                    light_variants.push(light_variants[0].clone());
                } else {
                    let pts_count = validate_count(counts[2 + i])?;
                    light_variants.push(reader.points(pts_count)?);
                }
            }

            let smoke_count = validate_count(counts[counts.len() - 2])?;
            let smoke = reader.points(smoke_count)?;

            let skip2 = (counts[counts.len() - 1] as u64)
                .checked_mul(16)
                .ok_or(AptError::CountOutOfBounds(u32::MAX))?;
            reader.skip_u64(skip2)?;

            attachments.push(AptAttachmentGroup {
                marker: markers[group],
                light_variants,
                smoke_points: smoke,
            });
        }

        let frame_bytes = (frame_count as u64)
            .checked_mul(8)
            .ok_or(AptError::CountOutOfBounds(u32::MAX))?;
        reader.require_u64(frame_bytes)?;

        let mut frames = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            let first = validate_count(reader.word()?)?;
            let count = validate_count(reader.word()?)?;
            if (first as u64) + (count as u64) > tile_count as u64 {
                return Err(AptError::FrameReferencesOutOfRange {
                    first: first as u32,
                    count: count as u32,
                    tile_count: tile_count as u32,
                });
            }
            frames.push(AptFrameRange::new(first, count));
        }

        let palette_words = (colors as u64) * (variants as u64);
        if palette_words > i32::MAX as u64 {
            return Err(AptError::PalettesTooLarge);
        }
        let palettes = reader.words(palette_words as usize)?;

        let tile_bytes = (tile_count as u64)
            .checked_mul(12)
            .ok_or(AptError::CountOutOfBounds(u32::MAX))?;
        reader.require_u64(tile_bytes)?;

        let mut tiles = Vec::with_capacity(tile_count);
        for _ in 0..tile_count {
            let data_offset = validate_count(reader.word()?)?;
            let packed_position = reader.word()?;
            let auxiliary_bytes = reader.word()?;
            tiles.push(AptTileInfo::new(data_offset, packed_position, auxiliary_bytes));
        }

        if reader.word()? != 0x54414450 {
            return Err(AptError::MissingPdatBlock);
        }

        let data_len = validate_count(header[27])?;
        let data = reader.bytes(data_len)?.to_vec();

        for tile in &tiles {
            if (tile.data_offset as u64) + 8 > data.len() as u64 {
                return Err(AptError::TileOffsetOutsidePixelBlock {
                    offset: tile.data_offset,
                    pixel_len: data.len(),
                });
            }
        }

        let layout = [header[8], header[9], header[10], header[11]];

        Ok(Self {
            version,
            width,
            height,
            anchor_x,
            anchor_y,
            projection_origin,
            layout,
            palette_color_count: colors,
            palette_variant_count: variants,
            frames,
            tiles,
            attachment_groups: attachments,
            row_offsets,
            row_widths,
            palettes,
            data,
        })
    }

    /// Native 0x4E6EC0 selects group by axis0 and axis2, then chooses the axis1 light list.
    pub fn get_attachments(
        &self,
        axis0: u32,
        axis1: u32,
        axis2: u32,
    ) -> Result<AptAttachments<'_>, AptError> {
        if self.layout[0] == 0 || self.layout[1] == 0 || self.layout[2] == 0 {
            return Err(AptError::LayoutZeroAxis);
        }

        let a0 = (axis0.min(self.layout[0] - 1)) as u64;
        let a1 = (axis1.min(self.layout[1] - 1)) as usize;
        let a2 = (axis2.min(self.layout[2] - 1)) as u64;

        let index = a0 * (self.layout[2] as u64) + a2;
        if index >= self.attachment_groups.len() as u64 {
            return Err(AptError::AttachmentGroupMissing {
                index: index as usize,
                count: self.attachment_groups.len(),
            });
        }

        let group = &self.attachment_groups[index as usize];
        let lights = &group.light_variants[a1];
        let smoke = &group.smoke_points;
        let marker = if group.marker.x == -1_000_000 {
            None
        } else {
            Some(group.marker)
        };

        Ok(AptAttachments {
            lights,
            smoke,
            marker,
        })
    }

    /// Decodes a single 64x31 diamond tile into an ARGB image.
    pub fn decode_tile(&self, tile_index: usize, palette_variant: usize) -> Result<AptIndexedImage, AptError> {
        self.validate_palette(palette_variant)?;
        if tile_index >= self.tiles.len() {
            return Err(AptError::TileIndexOutOfRange {
                index: tile_index,
                count: self.tiles.len(),
            });
        }

        let mut pixels = vec![0u32; 64 * 31];
        self.write_tile(tile_index, palette_variant, &mut pixels, 64, 31, 30, 0)?;
        Ok(AptIndexedImage {
            width: 64,
            height: 31,
            argb_pixels: pixels,
        })
    }

    /// Decodes a complete canvas frame composited from its referenced diamond tiles.
    pub fn decode_frame(&self, frame_index: usize, palette_variant: usize) -> Result<AptIndexedImage, AptError> {
        self.validate_palette(palette_variant)?;
        if frame_index >= self.frames.len() {
            return Err(AptError::FrameIndexOutOfRange {
                index: frame_index,
                count: self.frames.len(),
            });
        }

        let mut pixels = vec![0u32; self.width * self.height];
        let frame = self.frames[frame_index];
        for index in frame.first_tile..frame.first_tile + frame.tile_count {
            let tile = &self.tiles[index];
            self.write_tile(
                index,
                palette_variant,
                &mut pixels,
                self.width,
                self.height,
                tile.x(),
                tile.y(),
            )?;
        }

        Ok(AptIndexedImage {
            width: self.width,
            height: self.height,
            argb_pixels: pixels,
        })
    }

    fn write_tile(
        &self,
        tile_index: usize,
        variant: usize,
        pixels: &mut [u32],
        width: usize,
        height: usize,
        center_x: i32,
        top_y: i32,
    ) -> Result<(), AptError> {
        let offset = self.tiles[tile_index].data_offset;
        let data = &self.data[offset..];
        if data.len() < 8 {
            return Err(AptError::TileOffsetOutsidePixelBlock {
                offset,
                pixel_len: self.data.len(),
            });
        }

        let flags = u32::from_le_bytes(data[0..4].try_into().unwrap());
        let opaque = u32::from_le_bytes(data[4..8].try_into().unwrap());
        let compressed = (flags & 0x8000_0000) != 0;

        if compressed && data.len() < 72 {
            return Err(AptError::CompressedTileMissingRowTable);
        }

        for y in 0..31 {
            if (flags & (1u32 << y)) != 0 {
                continue;
            }

            let mut start = self.row_offsets[y] as usize;
            let mut length = self.row_widths[y] as i32;
            let mut gap = 0i32;

            if compressed {
                let row = u16::from_le_bytes(data[8 + y * 2..10 + y * 2].try_into().unwrap());
                let next = u16::from_le_bytes(data[10 + y * 2..12 + y * 2].try_into().unwrap());
                start = (row & 0x7FF) as usize;
                length = ((next & 0x7FF) as i32) - (start as i32);
                gap = ((row >> 9) & 0x3C) as i32;

                if start < 64 {
                    return Err(AptError::CompressedPixelsOverlapRowTable);
                }
            }

            if length < 0
                || (gap + length) > (self.row_widths[y] as i32)
                || (start as u64) + (length as u64) + 8 > data.len() as u64
            {
                return Err(AptError::InvalidPixelRow { row: y });
            }

            let x = center_x + gap + 2 - (self.row_widths[y] as i32) / 2;
            let zero_opaque = (opaque & (1u32 << y)) != 0;

            for index in 0..length as usize {
                let color = data[8 + start + index] as usize;
                if color >= self.palette_color_count {
                    return Err(AptError::MissingPaletteColor {
                        color,
                        color_count: self.palette_color_count,
                    });
                }
                if color == 0 && !zero_opaque {
                    continue;
                }

                let dx = x + index as i32;
                let dy = top_y + y as i32;

                if dx < 0 || dy < 0 || (dx as usize) >= width || (dy as usize) >= height {
                    return Err(AptError::VisiblePixelOutsideCanvas {
                        x: dx,
                        y: dy,
                        width,
                        height,
                    });
                }

                let palette_color = self.palettes[variant * self.palette_color_count + color];
                pixels[dy as usize * width + dx as usize] = palette_to_argb(palette_color);
            }
        }

        Ok(())
    }

    fn validate_palette(&self, variant: usize) -> Result<(), AptError> {
        if variant >= self.palette_variant_count {
            Err(AptError::PaletteVariantOutOfRange {
                variant,
                count: self.palette_variant_count,
            })
        } else {
            Ok(())
        }
    }
}

fn validate_count(count: u32) -> Result<usize, AptError> {
    if count <= i32::MAX as u32 {
        Ok(count as usize)
    } else {
        Err(AptError::CountOutOfBounds(count))
    }
}

struct Reader<'a> {
    source: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn new(source: &'a [u8]) -> Self {
        Self { source, position: 0 }
    }

    fn remaining(&self) -> usize {
        self.source.len().saturating_sub(self.position)
    }

    fn require(&self, count: usize) -> Result<(), AptError> {
        if count > self.remaining() {
            Err(AptError::TruncatedData {
                needed: count,
                available: self.remaining(),
            })
        } else {
            Ok(())
        }
    }

    fn require_u64(&self, count: u64) -> Result<(), AptError> {
        let count_usize: usize = count.try_into().map_err(|_| AptError::TruncatedData {
            needed: usize::MAX,
            available: self.remaining(),
        })?;
        self.require(count_usize)
    }

    fn word(&mut self) -> Result<u32, AptError> {
        self.require(4)?;
        let bytes: [u8; 4] = self.source[self.position..self.position + 4]
            .try_into()
            .unwrap();
        self.position += 4;
        Ok(u32::from_le_bytes(bytes))
    }

    fn words(&mut self, count: usize) -> Result<Vec<u32>, AptError> {
        let byte_len = count.checked_mul(4).ok_or(AptError::PalettesTooLarge)?;
        self.require(byte_len)?;
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            out.push(self.word()?);
        }
        Ok(out)
    }

    fn points(&mut self, count: usize) -> Result<Vec<AptPoint>, AptError> {
        let byte_len = count.checked_mul(8).ok_or(AptError::PalettesTooLarge)?;
        self.require(byte_len)?;
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            let x = self.word()? as i32;
            let y = self.word()? as i32;
            out.push(AptPoint::new(x, y));
        }
        Ok(out)
    }

    fn skip(&mut self, count: usize) -> Result<(), AptError> {
        self.require(count)?;
        self.position += count;
        Ok(())
    }

    fn skip_u64(&mut self, count: u64) -> Result<(), AptError> {
        let count_usize: usize = count.try_into().map_err(|_| AptError::TruncatedData {
            needed: usize::MAX,
            available: self.remaining(),
        })?;
        self.skip(count_usize)
    }

    fn bytes(&mut self, count: usize) -> Result<&'a [u8], AptError> {
        self.require(count)?;
        let slice = &self.source[self.position..self.position + count];
        self.position += count;
        Ok(slice)
    }
}
