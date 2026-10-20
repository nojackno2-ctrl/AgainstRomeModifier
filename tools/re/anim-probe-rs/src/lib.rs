use against_rome_data::{AlrDocument, AptDocument};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub const PNG_NAMES: [&str; 5] = [
    "gersch01_anim0_idle_sheet.png",
    "gersch01_team_variants_sheet.png",
    "gerhau02_axis1_val0.png",
    "gerhau02_axis1_val1.png",
    "gerhau02_anim_torch_sheet.png",
];

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

/// Owns a newly created TEMP directory. Windows directory handles deny delete
/// sharing, pinning every ancestor (including the output) throughout all writes.
pub struct Output {
    path: PathBuf,
    #[cfg(windows)]
    _guards: Vec<File>,
}

#[cfg(windows)]
fn guard_directory(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    let handle = OpenOptions::new()
        .read(true)
        .share_mode(3) // FILE_SHARE_READ | FILE_SHARE_WRITE, deliberately no DELETE
        .custom_flags(0x02000000 | 0x00200000) // BACKUP_SEMANTICS | OPEN_REPARSE_POINT
        .open(path)?;
    let metadata = handle.metadata()?;
    if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
        return Err(invalid("directory is a reparse point or not a directory"));
    }
    Ok(handle)
}

fn plain_directory(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(invalid("symlink or non-directory ancestor"));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(invalid("reparse point ancestor"));
        }
    }
    Ok(())
}

impl Output {
    pub fn new(path: &Path) -> io::Result<Self> {
        Self::under(path, &std::env::temp_dir())
    }

    fn under(path: &Path, temp: &Path) -> io::Result<Self> {
        // Reject traversal before normalization and ambiguous Windows aliases.
        for part in path.components() {
            if matches!(part, Component::ParentDir | Component::CurDir) {
                return Err(invalid("traversal is forbidden"));
            }
            if let Component::Normal(name) = part {
                let text = name.to_string_lossy();
                if text.ends_with(['.', ' ']) || text.contains(':') {
                    return Err(invalid("ambiguous output component"));
                }
            }
        }
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };
        // Existing parents only: never create an unvalidated ancestor tree.
        let parent = path.parent().ok_or_else(|| invalid("missing parent"))?;
        #[cfg(windows)]
        let mut guards = Vec::new();
        let mut ancestors: Vec<_> = parent.ancestors().collect();
        ancestors.reverse();
        for ancestor in ancestors {
            plain_directory(ancestor)?;
            #[cfg(windows)]
            guards.push(guard_directory(ancestor)?);
        }
        let temp = fs::canonicalize(temp)?;
        let parent = fs::canonicalize(parent)?;
        let candidate = parent.join(path.file_name().ok_or_else(|| invalid("missing name"))?);
        if candidate == temp || !candidate.starts_with(&temp) {
            return Err(invalid("output is outside TEMP or equal to TEMP"));
        }
        fs::create_dir(&candidate)?;
        #[cfg(windows)]
        guards.push(guard_directory(&candidate)?);
        plain_directory(&candidate)?;
        Ok(Self {
            path: candidate,
            #[cfg(windows)]
            _guards: guards,
        })
    }

    fn create(&self, name: &str) -> io::Result<File> {
        if !PNG_NAMES.contains(&name) && name != "probe_summary.json" {
            return Err(invalid("unexpected generated filename"));
        }
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.path.join(name))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug, Clone)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
}

impl Image {
    fn new(width: usize, height: usize, color: u32) -> Result<Self> {
        let count = width
            .checked_mul(height)
            .filter(|&n| n <= 64 * 1024 * 1024)
            .ok_or_else(|| invalid("sheet dimensions exceed probe allocation limit"))?;
        if width == 0 || height == 0 {
            return Err(invalid("empty image").into());
        }
        Ok(Self {
            width,
            height,
            pixels: vec![color; count],
        })
    }

    // Straight-alpha replacement. Clip at sheet boundaries, never cell boundaries.
    fn blit(&mut self, source: &Image, x: i64, y: i64) {
        for py in 0..source.height {
            for px in 0..source.width {
                let color = source.pixels[py * source.width + px];
                let dx = x + px as i64;
                let dy = y + py as i64;
                if color >> 24 != 0
                    && dx >= 0
                    && dy >= 0
                    && dx < self.width as i64
                    && dy < self.height as i64
                {
                    self.pixels[dy as usize * self.width + dx as usize] = color;
                }
            }
        }
    }

    fn write(&self, output: &Output, name: &str) -> Result<()> {
        let mut encoder =
            png::Encoder::new(output.create(name)?, self.width as u32, self.height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        let rgba: Vec<u8> = self
            .pixels
            .iter()
            .flat_map(|&c| [(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8])
            .collect();
        writer.write_image_data(&rgba)?;
        writer.finish()?;
        Ok(())
    }
}

pub fn alr_sheets(doc: &AlrDocument) -> Result<(Image, Image)> {
    let cols = doc.layout_columns as usize;
    let rows = doc.layout_rows as usize;
    if cols == 0 || rows <= 14 || doc.palette_variant_count < 9 {
        return Err(invalid("ALR needs columns, direction 14 and 9 palettes").into());
    }
    let cw = doc.anchor_width as usize;
    let ch = doc.anchor_height as usize;
    let mut idle = Image::new(
        cw.checked_mul(6).ok_or_else(|| invalid("width overflow"))?,
        ch.checked_mul(4)
            .ok_or_else(|| invalid("height overflow"))?,
        0,
    )?;
    let mut teams = Image::new(
        cw.checked_mul(9).ok_or_else(|| invalid("width overflow"))?,
        ch,
        0,
    )?;
    let base = 14usize
        .checked_mul(cols)
        .ok_or_else(|| invalid("frame index overflow"))?;
    let decode = |index: usize, variant: usize| -> Result<(Image, i32, i32)> {
        let info = doc
            .frames
            .get(index)
            .ok_or_else(|| invalid("missing ALR frame"))?
            .info;
        Ok((
            Image {
                width: info.width,
                height: info.height,
                pixels: doc.decode_frame_argb(index, variant)?,
            },
            info.offset_x,
            info.offset_y,
        ))
    };
    for f in 0..cols {
        let (image, x, y) = decode(base + f, 0)?;
        idle.blit(
            &image,
            ((f % 6) * cw) as i64 + x as i64,
            ((f / 6) * ch) as i64 + y as i64,
        );
    }
    for v in 0..9 {
        let (image, x, y) = decode(base, v)?;
        teams.blit(&image, (v * cw) as i64 + x as i64, y as i64);
    }
    Ok((idle, teams))
}

fn apt_image(doc: &AptDocument, index: usize) -> Result<Image> {
    let frame = doc.decode_frame(index, 0)?;
    Ok(Image {
        width: frame.width,
        height: frame.height,
        pixels: frame.argb_pixels,
    })
}

fn apt_sheet(mut decode: impl FnMut(usize) -> Result<Image>) -> Result<Image> {
    let mut sheet = Image::new(1000, 1000, 0xff202020)?;
    for d in 0..25 {
        let frame = decode(1000 + d)?;
        for py in 0..200 {
            for px in 0..200 {
                let x = 280 + px;
                let y = 200 + py;
                if x < frame.width && y < frame.height {
                    let color = frame.pixels[y * frame.width + x];
                    if color >> 24 != 0 {
                        sheet.pixels[((d / 5) * 200 + py) * 1000 + (d % 5) * 200 + px] = color;
                    }
                }
            }
        }
    }
    Ok(sheet)
}

fn entry(path: &Path, name: &str) -> Result<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(File::open(path)?)?;
    // Exact case-sensitive ZIP lookup; never extract archive filenames.
    let mut entry = archive.by_name(name)?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[derive(serde::Serialize)]
pub struct Summary {
    pub status: &'static str,
    pub unit_anim: &'static str,
    pub team_variants: &'static str,
    pub apt_axis1_val0: &'static str,
    pub apt_axis1_val1: &'static str,
    pub apt_anim: &'static str,
}

pub fn summary() -> Summary {
    Summary {
        status: "SUCCESS",
        unit_anim: PNG_NAMES[0],
        team_variants: PNG_NAMES[1],
        apt_axis1_val0: PNG_NAMES[2],
        apt_axis1_val1: PNG_NAMES[3],
        apt_anim: PNG_NAMES[4],
    }
}

pub fn run(alr: &Path, apt: &Path, output: &Output) -> Result<()> {
    println!("Exporting probe artifacts to {}", output.path().display());
    let doc = AlrDocument::parse(&entry(alr, "SYSTEM/DATA/ALR/gersch01.alr")?)?;
    println!(
        "gersch01.alr: {} frames, {} cols, {} rows, {} variants",
        doc.frames.len(),
        doc.layout_columns,
        doc.layout_rows,
        doc.palette_variant_count
    );
    let (idle, teams) = alr_sheets(&doc)?;
    idle.write(output, PNG_NAMES[0])?;
    println!("Exported {}", PNG_NAMES[0]);
    teams.write(output, PNG_NAMES[1])?;
    println!("Exported {}", PNG_NAMES[1]);
    let doc = AptDocument::parse(&entry(apt, "SYSTEM/DATA/APT/gerhau02.apt")?)?;
    println!(
        "gerhau02.apt: {} frames, layout {}x{}x{}x{}",
        doc.frames.len(),
        doc.layout[0],
        doc.layout[1],
        doc.layout[2],
        doc.layout[3]
    );
    let b0 = apt_image(&doc, 1000)?;
    let b1 = apt_image(&doc, 1125)?;
    b0.write(output, PNG_NAMES[2])?;
    b1.write(output, PNG_NAMES[3])?;
    if b0.width != b1.width || b0.height != b1.height {
        return Err(invalid("APT comparison frame dimensions differ").into());
    }
    let differences = b0
        .pixels
        .iter()
        .zip(&b1.pixels)
        .filter(|(a, b)| a != b)
        .count();
    println!(
        "gerhau02 frame 1000 vs 1125 pixel diffs: {differences} / {}",
        b0.pixels.len()
    );
    apt_sheet(|index| apt_image(&doc, index))?.write(output, PNG_NAMES[4])?;
    println!("Exported {}", PNG_NAMES[4]);
    let summary_text = serde_json::to_string_pretty(&summary())?;
    #[cfg(windows)]
    let summary_text = summary_text.replace('\n', "\r\n");
    output
        .create("probe_summary.json")?
        .write_all(summary_text.as_bytes())?;
    println!("Probe complete successfully.");
    Ok(())
}

#[cfg(test)]
mod tests;
