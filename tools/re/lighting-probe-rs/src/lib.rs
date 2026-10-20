pub mod base64;
pub mod bitmap;
pub mod daynight;
pub mod error;
pub mod model;

use std::ffi::OsString;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

pub use against_rome_environment::shadow::{LightingShadowMap, DECODED_LENGTH, SLICE_COUNT};
pub use bitmap::MapLightingBitmap;
pub use daynight::MapLightingDayNight;
pub use error::ProbeError;
pub use model::MapLightingModel;

use against_rome_data::apt::AptDocument;
use serde::{Deserialize, Serialize};
use zip::ZipArchive;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct AptSample {
    pub name: String,
    pub frame: usize,
    #[serde(rename = "Width")]
    pub width: usize,
    #[serde(rename = "Height")]
    pub height: usize,
    #[serde(rename = "AnchorX")]
    pub anchor_x: i32,
    #[serde(rename = "AnchorY")]
    pub anchor_y: i32,
    pub pixels: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct ValidationItem {
    name: String,
    bytes: String,
}

#[cfg(windows)]
pub const NEWLINE: &str = "\r\n";
#[cfg(not(windows))]
pub const NEWLINE: &str = "\n";

pub fn format_scalar(v: f32) -> String {
    if v.is_nan() {
        "NaN".to_string()
    } else if v.is_infinite() {
        if v > 0.0 {
            "Infinity".to_string()
        } else {
            "-Infinity".to_string()
        }
    } else if v == 0.0 {
        if v.is_sign_negative() {
            "-0".to_string()
        } else {
            "0".to_string()
        }
    } else {
        format!("{v}")
    }
}

pub fn format_vector3(v: [f32; 3]) -> String {
    format!(
        "<{}, {}, {}>",
        format_scalar(v[0]),
        format_scalar(v[1]),
        format_scalar(v[2])
    )
}

pub fn run_archive(archive_path: &Path, stdout: &mut impl Write) -> Result<(), ProbeError> {
    let file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut samples = Vec::new();

    for name in ["gerhau00", "gerwoh00"] {
        let suffix = format!("/{name}.apt");
        let exact = format!("{name}.apt");
        let mut matches = Vec::new();

        for i in 0..archive.len() {
            let entry = archive.by_index(i)?;
            let norm = entry.name().replace('\\', "/").to_ascii_lowercase();
            if norm == exact || norm.ends_with(&suffix) {
                matches.push(i);
            }
        }

        if matches.is_empty() {
            return Err(ProbeError::Zip(format!("Entry not found for {name}")));
        }
        if matches.len() > 1 {
            return Err(ProbeError::Zip(format!(
                "Multiple entries found for {name}"
            )));
        }

        let mut entry = archive.by_index(matches[0])?;
        let mut buffer = Vec::new();
        entry.read_to_end(&mut buffer)?;

        let document = AptDocument::parse(&buffer)?;
        let layout = document.layout;
        if layout[0] == 0 {
            return Err(ProbeError::InvalidData("Layout axis 0 is zero".into()));
        }
        let frame = ((layout[0] - 1) as usize)
            .checked_mul(layout[1] as usize)
            .and_then(|v| v.checked_mul(layout[2] as usize))
            .and_then(|v| v.checked_mul(layout[3] as usize))
            .ok_or_else(|| ProbeError::InvalidData("Frame index calculation overflow".into()))?;

        let image = document.decode_frame(frame, 0)?;
        samples.push(AptSample {
            name: name.to_string(),
            frame,
            width: image.width,
            height: image.height,
            anchor_x: document.anchor_x,
            anchor_y: document.anchor_y,
            pixels: image.argb_pixels,
        });
    }

    let json = serde_json::to_string(&samples)?;
    write!(stdout, "{json}{NEWLINE}")?;
    Ok(())
}

pub fn run_validate(mut stdin: impl Read, stdout: &mut impl Write) -> Result<(), ProbeError> {
    let mut input_str = String::new();
    stdin.read_to_string(&mut input_str)?;

    let items: Vec<ValidationItem> = serde_json::from_str(&input_str)?;
    for item in items {
        let bytes = base64::decode_base64(&item.bytes)?;
        let name = item.name;

        if name.ends_with("shadows.dat") {
            let map = LightingShadowMap::parse_decoded(&bytes)?;
            let header_str = map
                .header()
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",");
            write!(
                stdout,
                "VALID {} decoded={} slices={} header={}{}",
                name,
                bytes.len(),
                SLICE_COUNT,
                header_str,
                NEWLINE
            )?;
        } else if name.ends_with("daynight.bmp") {
            let table = MapLightingDayNight::parse(&bytes)?;
            let ambient = table.ambient_at(17, 55)?;
            let row = table.raw_color(12, 4)?;
            write!(
                stdout,
                "VALID {} ambient17:55={} row4hour12={}{}",
                name,
                format_vector3(ambient),
                format_vector3(row),
                NEWLINE
            )?;
        } else {
            let image = MapLightingBitmap::parse(&bytes)?;
            let color0 = image.color_at(0, 0)?;
            write!(
                stdout,
                "VALID {} {}x{} rgb0={}{}",
                name,
                image.width(),
                image.height(),
                format_vector3(color0),
                NEWLINE
            )?;
        }
    }

    Ok(())
}

pub fn cli(
    args: &[OsString],
    stdin: impl Read,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> i32 {
    if args.len() == 1 && args[0] == "validate" {
        match run_validate(stdin, stdout) {
            Ok(()) => 0,
            Err(e) => {
                let _ = write!(stderr, "Validation error: {e}{NEWLINE}");
                1
            }
        }
    } else if args.len() == 1 {
        let path = Path::new(&args[0]);
        match run_archive(path, stdout) {
            Ok(()) => 0,
            Err(e) => {
                let _ = write!(stderr, "Error: {e}{NEWLINE}");
                1
            }
        }
    } else {
        let _ = write!(
            stderr,
            "Usage: lighting-probe <readonly apt.dat copy>{NEWLINE}"
        );
        2
    }
}
