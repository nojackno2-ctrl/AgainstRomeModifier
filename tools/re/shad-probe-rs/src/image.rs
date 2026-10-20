use against_rome_environment::shadow::{darken_argb, ShadowFrame};
use std::{fs::OpenOptions, io, path::Path};

pub const GROUND: u32 = 0xff94b870;

pub fn write_png(path: &Path, width: usize, height: usize, argb: &[u32]) -> io::Result<()> {
    if width == 0
        || height == 0
        || width > u32::MAX as usize
        || height > u32::MAX as usize
        || width.checked_mul(height) != Some(argb.len())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid PNG dimensions/pixel extent.",
        ));
    }
    let file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let mut encoder = png::Encoder::new(file, width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_filter(png::FilterType::NoFilter);
    let mut writer = encoder.write_header()?;
    let mut rgba = Vec::with_capacity(argb.len() * 4);
    for &p in argb {
        rgba.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8, (p >> 24) as u8]);
    }
    writer.write_image_data(&rgba)?;
    writer.finish()?;
    Ok(())
}

pub fn ground_preview(frame: &ShadowFrame, radius: i32, kind: i32, cx: i32, cz: i32) -> Vec<u32> {
    let (width, height) = (720usize, 400usize);
    let mut pixels = vec![GROUND; width * height];
    if radius > 0 {
        for y in 0..height {
            for x in 0..width {
                let dx = x as f64 - width as f64 / 2.0;
                let dy = y as f64 - height as f64 / 2.0;
                let u = (dx + 2.0 * dy - f64::from(cx)) / f64::from(radius);
                let v = (2.0 * dy - dx - f64::from(cz)) / f64::from(radius);
                if u.abs() >= 1.0 || v.abs() >= 1.0 || (kind == 1 && u * u + v * v >= 1.0) {
                    continue;
                }
                let tx = (((u + 1.0) * frame.width() as f64 / 2.0) as usize).min(frame.width() - 1);
                let ty =
                    (((v + 1.0) * frame.height() as f64 / 2.0) as usize).min(frame.height() - 1);
                pixels[y * width + x] = darken_argb(
                    pixels[y * width + x],
                    frame.alpha_mask()[ty * frame.width() + tx],
                );
            }
        }
    }
    let mut cross = |cx: i64, cy: i64, color| {
        for d in -5..=5 {
            if (0..width as i64).contains(&(cx + d)) && (0..height as i64).contains(&cy) {
                pixels[cy as usize * width + (cx + d) as usize] = color;
            }
            if (0..width as i64).contains(&cx) && (0..height as i64).contains(&(cy + d)) {
                pixels[(cy + d) as usize * width + cx as usize] = color;
            }
        }
    };
    cross(360, 200, 0xffff4040);
    cross(
        (360.0 + f64::from(cx.wrapping_sub(cz)) / 2.0).round_ties_even() as i64,
        (200.0 + f64::from(cx.wrapping_add(cz)) / 4.0).round_ties_even() as i64,
        0xff20ffff,
    );
    pixels
}
