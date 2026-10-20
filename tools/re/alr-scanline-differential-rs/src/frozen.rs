//! Independent port of the frozen pre-Rust C# oracle from e3a21d9.
//! Keep this parser and decoder independent of the production ALR implementation.

use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameInfo {
    pub offset_x: i32,
    pub offset_y: i32,
    pub width: usize,
    pub height: usize,
    pub palette_color_count: usize,
    pub packed_size: u32,
    pub stored_pixel_bytes: u32,
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub info: FrameInfo,
    payload: Arc<[u8]>,
    rows: Arc<[u32]>,
}

#[derive(Debug)]
pub struct Document {
    pub version: u32,
    pub layout_columns: u32,
    pub layout_rows: u32,
    pub palette_variant_count: usize,
    pub anchor_width: u32,
    pub anchor_height: u32,
    pub frames: Vec<Frame>,
}

impl Document {
    pub fn parse(source: &[u8]) -> Result<Self, String> {
        let mut reader = Reader {
            source,
            position: 0,
        };
        if reader.word()? != 0x4152_4c41 {
            return Err("Missing ALRA signature.".into());
        }
        let version = reader.word()?;
        if !(4..=6).contains(&version) {
            return Err(format!(
                "ALR version {version} is not supported; expected 4..6."
            ));
        }
        reader.word()?; // Native local +0x78, not semantic animation metadata.
        let count = reader.word()?;
        if reader.word()? != 8 {
            return Err("Only 8-bit indexed ALR frames are supported.".into());
        }
        reader.word()?;
        let layout_columns = reader.word()?;
        let layout_rows = reader.word()?;
        let variants = reader.word()?;
        if variants == 0 || variants > i32::MAX as u32 {
            return Err("Invalid ALR palette variant count.".into());
        }
        reader.word()?;
        for _ in 0..5 {
            reader.word()?;
        }
        let anchor_width = reader.word()?;
        let anchor_height = reader.word()?;
        for _ in 0..6 {
            reader.word()?;
        }
        if count == 0 || count > i32::MAX as u32 || u64::from(count) * 4 > reader.remaining() as u64
        {
            return Err("Invalid or truncated ALR frame count.".into());
        }
        let mut frames: Vec<Frame> = Vec::new();
        for index in 0..count as usize {
            let reference = reader.word()? as i32;
            if reference >= 0 {
                if reference as usize >= index {
                    return Err("ALR shared frame must reference an earlier record.".into());
                }
                // C# shares the record itself; Arc avoids copying its owned buffers.
                frames.push(frames[reference as usize].clone());
                continue;
            }
            let table_offset = reader.word()?;
            let position = reader.word()?;
            let packed_size = reader.word()?;
            let stored_pixel_bytes = reader.word()?;
            let width = (packed_size & 0x7ff) as usize;
            let height = ((packed_size >> 11) & 0x7ff) as usize;
            let palette_color_count = ((packed_size >> 22) & 0x1ff) as usize;
            let palette_bytes = u64::from(variants) * palette_color_count as u64 * 4;
            let payload_bytes = palette_bytes + ((u64::from(stored_pixel_bytes) + 3) & !3);
            let table_bytes = (height as u64 + 1) * 4;
            if u64::from(table_offset) != payload_bytes
                || payload_bytes > i32::MAX as u64
                || payload_bytes + table_bytes > reader.remaining() as u64
            {
                return Err(
                    "ALR frame has an invalid row table offset or truncated payload.".into(),
                );
            }
            let payload = Arc::from(reader.bytes(payload_bytes as usize)?);
            let mut rows = Vec::with_capacity(height + 1);
            for _ in 0..=height {
                rows.push(reader.word()?);
            }
            frames.push(Frame {
                info: FrameInfo {
                    // The frozen C# code does not sign-extend these coordinates.
                    offset_x: (position & 0xffff) as i32,
                    offset_y: (position >> 16) as i32,
                    width,
                    height,
                    palette_color_count,
                    packed_size,
                    stored_pixel_bytes,
                },
                payload,
                rows: rows.into(),
            });
        }
        // Optional trailing metadata is deliberately ignored, as in C#.
        Ok(Self {
            version,
            layout_columns,
            layout_rows,
            palette_variant_count: variants as usize,
            anchor_width,
            anchor_height,
            frames,
        })
    }

    pub fn decode_frame_argb(&self, frame: usize, palette: usize) -> Result<Vec<u32>, String> {
        let selected = self.frames.get(frame).ok_or("Frame index out of range.")?;
        if palette >= self.palette_variant_count {
            return Err("Palette variant out of range.".into());
        }
        let first = &self.frames[0];
        let colors = first.info.palette_color_count;
        if colors == 0 {
            return Err("Indexed ALR has no palette in its first frame.".into());
        }
        let start = palette
            .checked_mul(colors)
            .and_then(|n| n.checked_mul(4))
            .ok_or("Palette offset overflow.")?;
        let end = start
            .checked_add(colors * 4)
            .ok_or("Palette offset overflow.")?;
        let bytes = first
            .payload
            .get(start..end)
            .ok_or("Truncated first-frame palette.")?;
        let palette: Vec<u32> = bytes
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        let pixel_start = self
            .palette_variant_count
            .checked_mul(selected.info.palette_color_count)
            .and_then(|n| n.checked_mul(4))
            .ok_or("Pixel offset overflow.")?;
        let data = selected
            .payload
            .get(pixel_start..)
            .ok_or("Truncated frame palettes.")?;
        decode_scanlines(
            selected.info.width,
            selected.info.height,
            &selected.rows,
            data,
            &palette,
        )
    }
}

struct Reader<'a> {
    source: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn remaining(&self) -> usize {
        self.source.len() - self.position
    }

    fn bytes(&mut self, count: usize) -> Result<&'a [u8], String> {
        if count > self.remaining() {
            return Err(format!("Truncated ALR at offset 0x{:X}.", self.position));
        }
        let bytes = &self.source[self.position..self.position + count];
        self.position += count;
        Ok(bytes)
    }

    fn word(&mut self) -> Result<u32, String> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

fn decode_scanlines(
    width: usize,
    height: usize,
    rows: &[u32],
    data: &[u8],
    palette: &[u32],
) -> Result<Vec<u32>, String> {
    if width > 2047 || height > 2047 {
        return Err("ALR frame dimensions must be at most 2047.".into());
    }
    if rows.len() != height + 1 {
        return Err("ALR requires one descriptor per row plus an end descriptor.".into());
    }
    if palette.is_empty() || palette.len() > 511 {
        return Err("ALR palette size must fit its native 9-bit field.".into());
    }
    let mut pixels = vec![0; width * height];
    for y in 0..height {
        let row = rows[y];
        let start = (row & 0xfffff) as usize;
        let end = (rows[y + 1] & 0xfffff) as usize;
        if end < start || end > data.len() {
            return Err(format!("ALR row {y} has invalid data offsets."));
        }
        let x = ((row >> 21) & 0x3ff) as usize;
        let first_opaque = row & 0x8000_0000 != 0;
        let payload = &data[start..end];
        let destination = &mut pixels[y * width..(y + 1) * width];
        if row & 0x10_0000 == 0 {
            write_run(destination, x, payload, palette, first_opaque)?;
        } else {
            if payload.len() < 2 {
                return Err(format!("ALR row {y} is missing its two-run prefix."));
            }
            let first_length = payload[0] as usize;
            let gap = (payload[1] & 0x7f) as usize;
            let second_opaque = payload[1] & 0x80 != 0;
            if first_length > payload.len() - 2 {
                return Err(format!("ALR row {y} has an invalid first run length."));
            }
            write_run(
                destination,
                x,
                &payload[2..2 + first_length],
                palette,
                first_opaque,
            )?;
            write_run(
                destination,
                x + first_length + gap,
                &payload[2 + first_length..],
                palette,
                second_opaque,
            )?;
        }
    }
    Ok(pixels)
}

fn write_run(
    destination: &mut [u32],
    x: usize,
    indices: &[u8],
    palette: &[u32],
    zero_opaque: bool,
) -> Result<(), String> {
    if x > destination.len() || indices.len() > destination.len() - x {
        return Err("ALR scanline extends beyond its frame width.".into());
    }
    for (i, &index) in indices.iter().enumerate() {
        let stored = *palette
            .get(index as usize)
            .ok_or("ALR pixel references a missing palette color.")?;
        if index != 0 || zero_opaque {
            destination[x + i] =
                0xff00_0000 | ((stored & 0xff) << 16) | (stored & 0xff00) | ((stored >> 16) & 0xff);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(out: &mut Vec<u8>, value: u32) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    fn header(version: u32, count: u32, variants: u32) -> Vec<u8> {
        let mut out = Vec::new();
        for value in [
            0x4152_4c41,
            version,
            0,
            count,
            8,
            0,
            7,
            9,
            variants,
            0,
            0,
            0,
            0,
            0,
            0,
            123,
            456,
            0,
            0,
            0,
            0,
            0,
            0,
        ] {
            word(&mut out, value);
        }
        out
    }

    fn frame(
        out: &mut Vec<u8>,
        width: u32,
        height: u32,
        colors: u32,
        palettes: &[u32],
        data: &[u8],
        rows: &[u32],
    ) {
        let padded = (data.len() + 3) & !3;
        for value in [
            u32::MAX,
            (palettes.len() * 4 + padded) as u32,
            0xffff_8000,
            width | (height << 11) | (colors << 22),
            data.len() as u32,
        ] {
            word(out, value);
        }
        for &color in palettes {
            word(out, color);
        }
        out.extend_from_slice(data);
        out.resize(out.len() + padded - data.len(), 0);
        for &row in rows {
            word(out, row);
        }
    }

    fn sample() -> Vec<u8> {
        let mut out = header(6, 1, 1);
        frame(
            &mut out,
            3,
            1,
            2,
            &[0x1234_5678, 0x00ab_cdef],
            &[0, 1],
            &[1 << 21, 2],
        );
        out
    }

    #[test]
    fn frozen_single_run_metadata_and_argb_parity() {
        for version in 4u32..=6 {
            let mut bytes = sample();
            bytes[4..8].copy_from_slice(&version.to_le_bytes());
            bytes.extend_from_slice(b"optional trailing metadata");
            let doc = Document::parse(&bytes).unwrap();
            assert_eq!(doc.version, version);
            assert_eq!((doc.layout_columns, doc.layout_rows), (7, 9));
            assert_eq!((doc.anchor_width, doc.anchor_height), (123, 456));
            let info = &doc.frames[0].info;
            assert_eq!((info.offset_x, info.offset_y), (32768, 65535));
            assert_eq!(
                (info.width, info.height, info.palette_color_count),
                (3, 1, 2)
            );
            assert_eq!(info.packed_size, 3 | (1 << 11) | (2 << 22));
            assert_eq!(info.stored_pixel_bytes, 2);
            assert_eq!(doc.decode_frame_argb(0, 0).unwrap(), [0, 0, 0xffef_cdab]);
        }
    }

    #[test]
    fn frozen_first_frame_palettes_and_shared_chain_parity() {
        let mut bytes = header(4, 4, 2);
        frame(
            &mut bytes,
            1,
            1,
            2,
            &[0x112233, 0x445566, 0x778899, 0xaabbcc],
            &[1],
            &[0, 1],
        );
        // A selected frame may have no local palette; use the first frame's.
        frame(&mut bytes, 1, 1, 0, &[], &[0], &[0x8000_0000, 1]);
        word(&mut bytes, 1);
        word(&mut bytes, 2);
        let doc = Document::parse(&bytes).unwrap();
        assert_eq!(doc.decode_frame_argb(0, 1).unwrap(), [0xffcc_bbaa]);
        for index in 1..4 {
            assert_eq!(doc.decode_frame_argb(index, 0).unwrap(), [0xff33_2211]);
            assert_eq!(doc.decode_frame_argb(index, 1).unwrap(), [0xff99_8877]);
        }
        assert!(Arc::ptr_eq(&doc.frames[1].payload, &doc.frames[3].payload));
        assert!(doc.decode_frame_argb(4, 0).is_err());
        assert!(doc.decode_frame_argb(0, 2).is_err());
    }

    #[test]
    fn frozen_selected_local_palettes_are_skipped_not_used() {
        let mut bytes = header(6, 2, 2);
        frame(
            &mut bytes,
            1,
            1,
            2,
            &[0, 0x123456, 0, 0xabcdef],
            &[1],
            &[0, 1],
        );
        // Local color count determines the pixel-data start, but the first
        // frame's color count determines the index bounds and palette stride.
        frame(&mut bytes, 1, 1, 1, &[0xffffff, 0xffffff], &[1], &[0, 1]);
        let doc = Document::parse(&bytes).unwrap();
        assert_eq!(doc.decode_frame_argb(1, 0).unwrap(), [0xff56_3412]);
        assert_eq!(doc.decode_frame_argb(1, 1).unwrap(), [0xffef_cdab]);
    }

    #[test]
    fn frozen_two_run_opacity_gap_and_descriptor_masks_parity() {
        let palette = [0xaa12_3456, 0x0065_4321];
        for first in [false, true] {
            for second in [false, true] {
                let row = (1 << 21) | 0x10_0000 | if first { 0x8000_0000 } else { 0 };
                let data = [2, 2 | if second { 0x80 } else { 0 }, 0, 1, 0, 1];
                // Flags in the final descriptor must not affect its offset.
                let pixels = decode_scanlines(8, 1, &[row, 0xfff0_0006], &data, &palette).unwrap();
                assert_eq!(
                    pixels,
                    [
                        0,
                        if first { 0xff56_3412 } else { 0 },
                        0xff21_4365,
                        0,
                        0,
                        if second { 0xff56_3412 } else { 0 },
                        0xff21_4365,
                        0
                    ]
                );
            }
        }
    }

    #[test]
    fn frozen_padding_empty_frames_and_max_palette_parity() {
        let mut bytes = header(5, 1, 1);
        frame(&mut bytes, 4, 1, 1, &[0x010203], &[0], &[0x8000_0000, 4]);
        assert_eq!(
            Document::parse(&bytes)
                .unwrap()
                .decode_frame_argb(0, 0)
                .unwrap(),
            [0xff03_0201; 4]
        );
        assert!(decode_scanlines(0, 1, &[0, 0], &[], &[0])
            .unwrap()
            .is_empty());
        assert!(decode_scanlines(2047, 0, &[u32::MAX], &[], &[0])
            .unwrap()
            .is_empty());
        let mut palette = vec![0; 511];
        palette[255] = 0x123456;
        assert_eq!(
            decode_scanlines(1, 1, &[0, 1], &[255], &palette).unwrap(),
            [0xff56_3412]
        );
    }

    #[test]
    fn rejects_every_truncated_prefix_and_bad_header() {
        let bytes = sample();
        for end in 0..bytes.len() {
            assert!(Document::parse(&bytes[..end]).is_err(), "prefix {end}");
        }
        for (offset, values) in [
            (0, vec![0]),
            (4, vec![3, 7]),
            (12, vec![0, u32::MAX]),
            (16, vec![0, 16]),
            (32, vec![0, u32::MAX]),
            (92, vec![0, 1]),
            (96, vec![0, 13, u32::MAX]),
            (108, vec![u32::MAX]),
        ] {
            for value in values {
                let mut bad = bytes.clone();
                bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                assert!(
                    Document::parse(&bad).is_err(),
                    "offset {offset}, value {value}"
                );
            }
        }
    }

    #[test]
    fn invalid_rows_are_deferred_until_decode() {
        let mut bytes = sample();
        let len = bytes.len();
        bytes[len - 4..].copy_from_slice(&5u32.to_le_bytes());
        let doc = Document::parse(&bytes).unwrap();
        assert!(doc
            .decode_frame_argb(0, 0)
            .unwrap_err()
            .contains("invalid data offsets"));
        let mut no_palette = header(4, 1, 1);
        frame(&mut no_palette, 0, 0, 0, &[], &[], &[0]);
        assert!(Document::parse(&no_palette)
            .unwrap()
            .decode_frame_argb(0, 0)
            .unwrap_err()
            .contains("no palette"));
    }

    #[test]
    fn frozen_multiple_rows_nonzero_initial_offset_and_negative_record_marker() {
        let mut bytes = header(6, 1, 1);
        frame(
            &mut bytes,
            2,
            2,
            2,
            &[0xabcdef, 0x123456],
            &[255, 1, 0],
            &[1, 0x8000_0002, 3],
        );
        // Every negative signed reference denotes a new frame, not just -1.
        bytes[92..96].copy_from_slice(&0x8000_0000u32.to_le_bytes());
        assert_eq!(
            Document::parse(&bytes)
                .unwrap()
                .decode_frame_argb(0, 0)
                .unwrap(),
            [0xff56_3412, 0, 0xffef_cdab, 0]
        );
    }

    #[test]
    fn rejects_malformed_scanlines_and_invalid_decoder_arguments() {
        let palette = [0, 1];
        let cases: &[(&[u32], &[u8])] = &[
            (&[2, 1], &[0, 0]),            // Decreasing offsets.
            (&[0, 3], &[0, 0]),            // End outside payload.
            (&[0x10_0000, 1], &[0]),       // Short two-run prefix.
            (&[0x10_0000, 2], &[1, 0]),    // First run too long.
            (&[3 << 21, 0], &[]),          // Even an empty run cannot start past width.
            (&[1 << 21, 2], &[0, 1]),      // Single run overflow.
            (&[0x10_0000, 3], &[0, 3, 1]), // Second run overflow.
            (&[0, 1], &[2]),               // Missing palette index.
        ];
        for &(rows, data) in cases {
            assert!(decode_scanlines(2, 1, rows, data, &palette).is_err());
        }
        assert!(decode_scanlines(2048, 0, &[0], &[], &palette).is_err());
        assert!(decode_scanlines(0, 2048, &[], &[], &palette).is_err());
        assert!(decode_scanlines(1, 1, &[0], &[], &palette).is_err());
        assert!(decode_scanlines(0, 0, &[0], &[], &[]).is_err());
        assert!(decode_scanlines(0, 0, &[0], &[], &[0; 512]).is_err());
    }
}
