//! Rust port of the C# `tools/re/apt-probe` utility.
//!
//! Opens a read-only `apt.dat` ZIP, parses every entry as APT with
//! `against-rome-data`, validates every attachment selection with the Rust
//! geometry (`apt_geometry::place`), optionally decodes every palette
//! tile/frame variant, writes a few representative RGBA samples using
//! generated names only, and emits a `report.json` shaped like the C# oracle.

use against_rome_data::apt::{AptDocument, AptError, AptPoint};
use against_rome_data::apt_geometry::{place, AptVector3};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Entry-name suffix (case-insensitive) that receives sample exports.
pub const SAMPLE_SUFFIX: &str = "/gerhau02.apt";
/// Frame indexes (variant 0) that are exported for the sample entry.
pub const SAMPLE_FRAMES: [usize; 2] = [0, 1000];

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct Failure {
    pub entry: String,
    pub error: String,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub struct XY {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct Sample {
    pub entry: String,
    #[serde(rename = "frameIndex")]
    pub frame_index: usize,
    pub file: String,
    #[serde(rename = "Width")]
    pub width: usize,
    #[serde(rename = "Height")]
    pub height: usize,
    #[serde(rename = "AnchorX")]
    pub anchor_x: i32,
    #[serde(rename = "AnchorY")]
    pub anchor_y: i32,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct AttachmentEntry {
    pub entry: String,
    #[serde(rename = "Version")]
    pub version: u32,
    #[serde(rename = "Layout")]
    pub layout: [u32; 4],
    #[serde(rename = "ProjectionOrigin")]
    pub projection_origin: XY,
    pub anchor: XY,
    pub groups: usize,
    pub selections: usize,
    #[serde(rename = "lightPoints")]
    pub light_points: usize,
    #[serde(rename = "smokePoints")]
    pub smoke_points: usize,
    pub markers: usize,
}

/// Same fields and order as the C# anonymous report object.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct Report {
    pub hash: String,
    #[serde(rename = "hashAfter")]
    pub hash_after: String,
    #[serde(rename = "attachmentsOnly")]
    pub attachments_only: bool,
    pub entries: usize,
    pub documents: usize,
    pub tiles: usize,
    pub frames: usize,
    pub failures: Vec<Failure>,
    pub samples: Vec<Sample>,
    pub attachments: Vec<AttachmentEntry>,
}

#[derive(Debug)]
pub enum ProbeError {
    /// Output path already exists (file or directory); nothing was touched.
    OutputExists(PathBuf),
    /// Output is inside the source archive directory.
    OutputInsideSource(PathBuf),
    /// The source archive changed between the two hashes.
    SourceChanged,
    Io(String),
}

impl std::fmt::Display for ProbeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProbeError::OutputExists(_) => {
                f.write_str("Output must be a new directory; existing files are never overwritten.")
            }
            ProbeError::OutputInsideSource(_) => {
                f.write_str("Output must be outside the source archive directory.")
            }
            ProbeError::SourceChanged => f.write_str("Source archive changed during audit."),
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

fn hash_file(file: &mut File) -> std::io::Result<String> {
    file.seek(SeekFrom::Start(0))?;
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

/// Per-document attachment totals.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AttachmentStats {
    pub selections: usize,
    pub light_points: usize,
    pub smoke_points: usize,
    pub markers: usize,
}

/// Walks every (axis0, axis2, axis1) selection like the C# probe, counting
/// lights/smoke/markers and placing every point at parent (0,0), height 0.
/// `on_place` receives each placement (lights, then smoke, then marker).
/// Counts of completed selections are returned even on error via `stats`.
pub fn scan_attachments(
    document: &AptDocument,
    stats: &mut AttachmentStats,
    mut on_place: impl FnMut(AptVector3),
) -> Result<(), AptError> {
    for a in 0..document.layout[0] {
        for c in 0..document.layout[2] {
            for b in 0..document.layout[1] {
                let selected = document.get_attachments(a, b, c)?;
                stats.selections += 1;
                stats.light_points += selected.lights.len();
                stats.smoke_points += selected.smoke.len();
                if selected.marker.is_some() {
                    stats.markers += 1;
                }
                for point in selected.lights.iter().chain(selected.smoke.iter()) {
                    on_place(place(document, *point, 0.0, 0.0, 0.0)?);
                }
                if let Some(marker) = selected.marker {
                    on_place(place(document, marker, 0.0, 0.0, 0.0)?);
                }
            }
        }
    }
    Ok(())
}

/// Processes one entry. Counters are updated as they go (matching the C#
/// behaviour where earlier counts survive a mid-entry failure).
fn process_entry(
    name: &str,
    bytes: &[u8],
    output: &Path,
    attachments_only: bool,
    report: &mut Report,
) -> Result<(), String> {
    let document = AptDocument::parse(bytes).map_err(|e| e.to_string())?;
    report.documents += 1;

    let mut stats = AttachmentStats::default();
    scan_attachments(&document, &mut stats, |_| {}).map_err(|e| e.to_string())?;
    let AptPoint { x: ox, y: oy } = document.projection_origin;
    report.attachments.push(AttachmentEntry {
        entry: name.to_string(),
        version: document.version,
        layout: document.layout,
        projection_origin: XY { x: ox, y: oy },
        anchor: XY {
            x: document.anchor_x,
            y: document.anchor_y,
        },
        groups: document.attachment_groups.len(),
        selections: stats.selections,
        light_points: stats.light_points,
        smoke_points: stats.smoke_points,
        markers: stats.markers,
    });
    if attachments_only {
        return Ok(());
    }

    let is_sample_entry = name.to_lowercase().ends_with(SAMPLE_SUFFIX);
    for variant in 0..document.palette_variant_count {
        for i in 0..document.tiles.len() {
            document
                .decode_tile(i, variant)
                .map_err(|e| e.to_string())?;
            report.tiles += 1;
        }
        for i in 0..document.frames.len() {
            let frame = document
                .decode_frame(i, variant)
                .map_err(|e| e.to_string())?;
            report.frames += 1;
            if is_sample_entry && variant == 0 && SAMPLE_FRAMES.contains(&i) {
                // Generated name only; the archive entry path is never used for writes.
                let file = format!("sample-{}.rgba", report.samples.len());
                let mut out = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(output.join(&file))
                    .map_err(|e| format!("cannot write {file}: {e}"))?;
                out.write_all(&argb_to_rgba(&frame.argb_pixels))
                    .map_err(|e| format!("cannot write {file}: {e}"))?;
                report.samples.push(Sample {
                    entry: name.to_string(),
                    frame_index: i,
                    file,
                    width: frame.width,
                    height: frame.height,
                    anchor_x: document.anchor_x,
                    anchor_y: document.anchor_y,
                });
            }
        }
    }
    Ok(())
}

/// Mirrors the C# check `output.StartsWith(GetDirectoryName(input) + sep, OrdinalIgnoreCase)`.
pub fn output_inside_source_dir(input: &Path, output: &Path) -> bool {
    let Some(dir) = input.parent() else {
        return false;
    };
    let mut prefix = dir.to_string_lossy().to_lowercase();
    while prefix.ends_with(['\\', '/']) {
        prefix.pop();
    }
    prefix.push(std::path::MAIN_SEPARATOR);
    output.to_string_lossy().to_lowercase().starts_with(&prefix)
}

/// Runs the probe. Refuses an output inside the source directory or an existing output.
pub fn run_probe(
    input: &Path,
    output: &Path,
    attachments_only: bool,
) -> Result<Report, ProbeError> {
    if output_inside_source_dir(input, output) {
        return Err(ProbeError::OutputInsideSource(output.to_path_buf()));
    }
    if output.exists() {
        return Err(ProbeError::OutputExists(output.to_path_buf()));
    }
    let mut file = File::open(input).map_err(|e| io_err("cannot open input", e))?;
    let hash = hash_file(&mut file).map_err(|e| io_err("cannot hash input", e))?;
    let mut archive = zip::ZipArchive::new(
        file.try_clone()
            .map_err(|e| io_err("cannot clone input", e))?,
    )
    .map_err(|e| io_err("input is not a readable ZIP", e))?;
    fs::create_dir_all(output).map_err(|e| io_err("cannot create output directory", e))?;

    let mut report = Report {
        hash: hash.clone(),
        hash_after: String::new(),
        attachments_only,
        entries: archive.len(),
        documents: 0,
        tiles: 0,
        frames: 0,
        failures: Vec::new(),
        samples: Vec::new(),
        attachments: Vec::new(),
    };
    for index in 0..archive.len() {
        let name = match archive.by_index_raw(index) {
            Ok(e) => e.name().to_string(),
            Err(e) => {
                let error = e.to_string();
                let entry = format!("#{index}");
                println!("FAIL {entry}: {error}");
                report.failures.push(Failure { entry, error });
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
        let result =
            read.and_then(|_| process_entry(&name, &data, output, attachments_only, &mut report));
        if let Err(error) = result {
            println!("FAIL {name}: {error}");
            report.failures.push(Failure { entry: name, error });
        }
    }
    drop(archive);
    let hash_after = hash_file(&mut file).map_err(|e| io_err("cannot rehash input", e))?;
    if hash != hash_after {
        return Err(ProbeError::SourceChanged);
    }
    report.hash_after = hash_after;
    let json = serde_json::to_string_pretty(&report).map_err(|e| io_err("serialize report", e))?;
    fs::write(output.join("report.json"), json)
        .map_err(|e| io_err("cannot write report.json", e))?;
    Ok(report)
}

/// CLI driver: returns the process exit code (2 usage/output conflict, 1 failures or errors, 0 ok).
pub fn cli(args: &[OsString], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let flag_ok = args.len() != 3 || args[2] == "--attachments-only";
    if !(2..=3).contains(&args.len()) || !flag_ok {
        let _ = writeln!(
            stderr,
            "Usage: apt-probe <readonly apt.dat> <new output directory> [--attachments-only]"
        );
        return 2;
    }
    let attachments_only = args.len() == 3;
    let abs = |p: &OsString| std::path::absolute(p).unwrap_or_else(|_| PathBuf::from(p));
    let (input, output) = (abs(&args[0]), abs(&args[1]));
    match run_probe(&input, &output, attachments_only) {
        Ok(r) => {
            let _ = writeln!(
                stdout,
                "Documents={} PaletteTiles={} PaletteFrames={} Failures={}",
                r.documents,
                r.tiles,
                r.frames,
                r.failures.len()
            );
            if r.failures.is_empty() {
                0
            } else {
                1
            }
        }
        Err(e @ (ProbeError::OutputExists(_) | ProbeError::OutputInsideSource(_))) => {
            let _ = writeln!(stderr, "{e}");
            2
        }
        Err(e) => {
            let _ = writeln!(stderr, "{e}");
            1
        }
    }
}
