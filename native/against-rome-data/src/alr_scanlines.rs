//! ALR indexed scanlines for the editor's still-frame preview (not native blend dispatch).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanlineError {
    Dimensions,
    Descriptors,
    Palette,
    RowOffsets,
    Prefix,
    RunBounds,
    Index,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexedSample {
    pub index: u8,
    /// Segment-specific index0 write policy; nonzero indices always write.
    pub write_zero: bool,
}

/// None denotes an absent span; Some(index0,false) denotes a serialized sample
/// that this segment skips. These must remain distinct for native pixel consumers.
pub fn decode_indexed(
    width: usize,
    height: usize,
    rows: &[u32],
    data: &[u8],
    palette_len: usize,
) -> Result<Vec<Option<IndexedSample>>, ScanlineError> {
    if width > 2047 || height > 2047 {
        return Err(ScanlineError::Dimensions);
    }
    if rows.len() != height + 1 {
        return Err(ScanlineError::Descriptors);
    }
    if palette_len == 0 || palette_len > 511 {
        return Err(ScanlineError::Palette);
    }
    let mut pixels = vec![None; width * height];
    for y in 0..height {
        let row = rows[y];
        let start = (row & 0xfffff) as usize;
        let end = (rows[y + 1] & 0xfffff) as usize;
        let bytes = data.get(start..end).ok_or(ScanlineError::RowOffsets)?;
        let x = ((row >> 21) & 1023) as usize;
        let dst = &mut pixels[y * width..(y + 1) * width];
        let opaque = row & 0x80000000 != 0;
        if row & 0x100000 == 0 {
            write_run(dst, x, bytes, palette_len, opaque)?;
        } else {
            if bytes.len() < 2 {
                return Err(ScanlineError::Prefix);
            }
            let first = bytes[0] as usize;
            if first > bytes.len() - 2 {
                return Err(ScanlineError::Prefix);
            }
            write_run(dst, x, &bytes[2..2 + first], palette_len, opaque)?;
            write_run(
                dst,
                x + first + (bytes[1] & 127) as usize,
                &bytes[2 + first..],
                palette_len,
                bytes[1] & 128 != 0,
            )?;
        }
    }
    Ok(pixels)
}

fn write_run(
    dst: &mut [Option<IndexedSample>],
    x: usize,
    indices: &[u8],
    palette_len: usize,
    opaque: bool,
) -> Result<(), ScanlineError> {
    let end = x
        .checked_add(indices.len())
        .ok_or(ScanlineError::RunBounds)?;
    let target = dst.get_mut(x..end).ok_or(ScanlineError::RunBounds)?;
    for (pixel, &index) in target.iter_mut().zip(indices) {
        if index as usize >= palette_len { return Err(ScanlineError::Index); }
        *pixel = Some(IndexedSample { index, write_zero: opaque });
    }
    Ok(())
}

pub fn decode_argb(width: usize, height: usize, rows: &[u32], data: &[u8], palette: &[u32])
    -> Result<Vec<u32>, ScanlineError>
{
    Ok(decode_indexed(width, height, rows, data, palette.len())?.into_iter().map(|sample| {
        match sample {
            Some(sample) if sample.index != 0 || sample.write_zero => {
                let color = palette[sample.index as usize];
                0xff000000 | ((color & 255) << 16) | (color & 0xff00) | ((color >> 16) & 255)
            }
            _ => 0,
        }
    }).collect())
}
