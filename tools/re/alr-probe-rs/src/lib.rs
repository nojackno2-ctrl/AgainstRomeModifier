//! Rust port of the C# `tools/re/alr-probe` utility.
//!
//! Opens a read-only `alr.dat` ZIP, parses every entry as ALR with
//! `against-rome-data`, decodes every frame/palette variant, writes a few
//! representative RGBA samples using generated names only, and emits a
//! `report.json` compatible with the C# oracle.

use against_rome_data::alr::{AlrDocument, AlrFrameInfo};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Basenames (case-sensitive) that get a sample RGBA export.
pub const SAMPLE_BASENAMES: [&str; 4] = [
    "fialgesc00.alr",
    "lagestgr00.alr",
    "lagenabust05.alr",
    "gersch01.alr",
];

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct Failure {
    pub entry: String,
    pub error: String,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct FrameInfoReport {
    pub offset_x: i32,
    pub offset_y: i32,
    pub width: usize,
    pub height: usize,
    pub palette_color_count: usize,
    pub packed_size: u32,
}

impl From<&AlrFrameInfo> for FrameInfoReport {
    fn from(i: &AlrFrameInfo) -> Self {
        Self {
            offset_x: i.offset_x,
            offset_y: i.offset_y,
            width: i.width,
            height: i.height,
            palette_color_count: i.palette_color_count,
            packed_size: i.packed_size,
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct Sample {
    pub entry: String,
    pub file: String,
    #[serde(rename = "Width")]
    pub width: usize,
    #[serde(rename = "Height")]
    pub height: usize,
    pub info: FrameInfoReport,
    #[serde(rename = "AnchorWidth")]
    pub anchor_width: u32,
    #[serde(rename = "AnchorHeight")]
    pub anchor_height: u32,
}

/// Same fields and order as the C# anonymous report object.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct Report {
    pub hash: String,
    pub entries: usize,
    pub documents: usize,
    pub frames: usize,
    pub variants: usize,
    pub blanks: usize,
    pub failures: Vec<Failure>,
    pub samples: Vec<Sample>,
}

#[derive(Debug)]
pub enum ProbeError {
    /// Output path already exists (file or directory); nothing was touched.
    OutputExists(PathBuf),
    Io(String),
}

impl std::fmt::Display for ProbeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProbeError::OutputExists(_) => {
                f.write_str("Output must be a new directory; existing files are never overwritten.")
            }
            ProbeError::Io(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for ProbeError {}

fn io_err(context: &str, e: impl std::fmt::Display) -> ProbeError {
    ProbeError::Io(format!("{context}: {e}"))
}

/// Converts `0xAARRGGBB` pixels to RGBA bytes (R, G, B, A).
pub fn argb_to_rgba(pixels: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(pixels.len() * 4);
    for &c in pixels {
        out.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8]);
    }
    out
}

/// Equivalent of .NET `Path.GetFileName` on Windows (splits on `/` and `\`).
fn base_name(entry: &str) -> &str {
    entry.rsplit(['/', '\\']).next().unwrap_or(entry)
}

fn hash_file(file: &mut File) -> std::io::Result<String> {
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    file.seek(SeekFrom::Start(0))?;
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect())
}

/// Processes one entry. Counters are updated as they go (matching the C#
/// behaviour where earlier counts survive a mid-entry failure).
fn process_entry(
    name: &str,
    bytes: &[u8],
    output: &Path,
    report: &mut Report,
) -> Result<(), String> {
    let document = AlrDocument::parse(bytes).map_err(|e| e.to_string())?;
    report.documents += 1;
    let base = base_name(name);
    for frame_index in 0..document.frames.len() {
        for variant in 0..document.palette_variant_count {
            let pixels = document
                .decode_frame_argb(frame_index, variant)
                .map_err(|e| e.to_string())?;
            report.variants += 1;
            let info = &document.frames[frame_index].info;
            let (w, h) = (info.width, info.height);
            if variant == 0 {
                report.frames += 1;
                if w == 0 || h == 0 {
                    report.blanks += 1;
                }
            }
            if frame_index == 0
                && variant == 0
                && w > 0
                && h > 0
                && SAMPLE_BASENAMES.contains(&base)
            {
                // Generated name only; the archive entry path is never used for writes.
                let file = format!("sample-{}.rgba", report.samples.len());
                let mut out = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(output.join(&file))
                    .map_err(|e| format!("cannot write {file}: {e}"))?;
                out.write_all(&argb_to_rgba(&pixels))
                    .map_err(|e| format!("cannot write {file}: {e}"))?;
                report.samples.push(Sample {
                    entry: name.to_string(),
                    file,
                    width: w,
                    height: h,
                    info: FrameInfoReport::from(&document.frames[0].info),
                    anchor_width: document.anchor_width,
                    anchor_height: document.anchor_height,
                });
            }
        }
    }
    Ok(())
}

/// Runs the probe. Refuses to touch an existing `output`.
pub fn run_probe(input: &Path, output: &Path) -> Result<Report, ProbeError> {
    if output.exists() {
        return Err(ProbeError::OutputExists(output.to_path_buf()));
    }
    let mut file = File::open(input).map_err(|e| io_err("cannot open input", e))?;
    let hash = hash_file(&mut file).map_err(|e| io_err("cannot hash input", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| io_err("input is not a readable ZIP", e))?;
    fs::create_dir_all(output).map_err(|e| io_err("cannot create output directory", e))?;

    let mut report = Report {
        hash,
        entries: archive.len(),
        documents: 0,
        frames: 0,
        variants: 0,
        blanks: 0,
        failures: Vec::new(),
        samples: Vec::new(),
    };
    for index in 0..archive.len() {
        let name = match archive.by_index_raw(index) {
            Ok(e) => e.name().to_string(),
            Err(e) => {
                report.failures.push(Failure {
                    entry: format!("#{index}"),
                    error: e.to_string(),
                });
                continue;
            }
        };
        let mut data = Vec::new();
        let read = archive
            .by_index(index)
            .map_err(|e| e.to_string())
            .and_then(|mut e| {
                e.read_to_end(&mut data)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            });
        let result = read.and_then(|_| process_entry(&name, &data, output, &mut report));
        if let Err(error) = result {
            report.failures.push(Failure { entry: name, error });
        }
    }
    let json = serde_json::to_string_pretty(&report).map_err(|e| io_err("serialize report", e))?;
    fs::write(output.join("report.json"), json)
        .map_err(|e| io_err("cannot write report.json", e))?;
    Ok(report)
}

/// CLI driver: returns the process exit code (2 usage/output conflict, 1 failures or errors, 0 ok).
pub fn cli(args: &[OsString], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    if args.len() != 2 {
        let _ = writeln!(
            stderr,
            "Usage: alr-probe <readonly alr.dat> <new output directory>"
        );
        return 2;
    }
    let abs = |p: &OsString| std::path::absolute(p).unwrap_or_else(|_| PathBuf::from(p));
    let (input, output) = (abs(&args[0]), abs(&args[1]));
    match run_probe(&input, &output) {
        Ok(r) => {
            let _ = writeln!(
                stdout,
                "Documents={} Frames={} PaletteFrames={} Blanks={} Failures={}",
                r.documents,
                r.frames,
                r.variants,
                r.blanks,
                r.failures.len()
            );
            if r.failures.is_empty() {
                0
            } else {
                1
            }
        }
        Err(e @ ProbeError::OutputExists(_)) => {
            let _ = writeln!(stderr, "{e}");
            2
        }
        Err(e) => {
            let _ = writeln!(stderr, "{e}");
            1
        }
    }
}
