//! Explicit-input, read-only ZIP probe. No installed-game discovery or PFIL decode.
pub mod bmp;
pub mod image;
pub mod safety;

use against_rome_environment::shadow::{darken_argb, ShadowDocument, ShadowError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

pub const PREVIEW_LIMIT: &str =
    "Flat ground; assumed UV orientation. No game comparison or object sprite overlay.";
const SHADOW_PREFIX: &str = "system/data/shadowtexture/";
const SAMPLES: [&str; 5] = [
    "gerhau02_shadow.bmp",
    "gernadelbaum_shadow.bmp",
    "round_small_shadow.bmp",
    "lakadorn_shadow.bmp",
    "error.bmp",
];
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub hash: String,
    pub input_bytes: u64,
    pub entries: usize,
    pub documents: usize,
    pub frames: usize,
    pub directories: usize,
    pub other_bmps: usize,
    pub failures: Vec<Failure>,
    pub samples: Vec<Sample>,
    pub objects: Vec<Object>,
    pub inventory: Vec<Inventory>,
}
#[derive(Debug, Serialize)]
pub struct Failure {
    pub entry: String,
    pub error: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub entry: String,
    pub file: String,
    pub ground_file: String,
    #[serde(rename = "Width")]
    pub width: usize,
    #[serde(rename = "Height")]
    pub height: usize,
}
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Inventory {
    Directory {
        entry: String,
        kind: &'static str,
    },
    Other {
        entry: String,
        kind: &'static str,
        audit: bmp::Audit,
    },
    Shadow {
        entry: String,
        kind: &'static str,
        audit: bmp::Audit,
        #[serde(rename = "TrailingByteCount")]
        trailing_byte_count: usize,
        #[serde(rename = "minStrength")]
        min_strength: u8,
        #[serde(rename = "maxStrength")]
        max_strength: u8,
    },
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Object {
    pub id: i32,
    pub name: String,
    pub shidx: i32,
    pub shsiz: i32,
    pub shtyp: i32,
    pub shacx: i32,
    pub shacz: i32,
    pub correction_screen_x: f64,
    pub correction_screen_y: f64,
    pub mapped_entry: Option<String>,
    pub candidate_entry: Option<String>,
    pub status: String,
    pub found: bool,
    pub preview: Option<String>,
    pub frame: i32,
    pub preview_limit: String,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

// ShadowError's Display adds a Rust-specific prefix; preserve C# messages.
fn shadow_error(error: ShadowError) -> String {
    match error {
        ShadowError::InvalidData(message) | ShadowError::NotSupported(message) => {
            message.to_string()
        }
        other => other.to_string(),
    }
}

/// Strict probe grammar, deliberately not the catalog's permissive parser.
pub fn read_shadow_names(text: &str) -> std::result::Result<HashMap<i32, String>, String> {
    let mut result = HashMap::new();
    let mut section = false;
    for raw in text.trim_start_matches('\u{feff}').split(['\r', '\n']) {
        let line = raw.trim();
        if line.starts_with('[') {
            section = line.eq_ignore_ascii_case("[ShadowNames]");
            continue;
        }
        if !section || line.is_empty() || line.starts_with(';') {
            continue;
        }
        let (id, name) = line
            .split_once(',')
            .ok_or("Invalid decoded ShadowNames row.")?;
        let id: i32 = id
            .trim()
            .parse()
            .map_err(|_| "Invalid decoded ShadowNames row.")?;
        if !(0..2000).contains(&id) {
            return Err("Invalid decoded ShadowNames row.".into());
        }
        let name = name.trim();
        if safety::component(name).is_err() || result.insert(id, name.to_owned()).is_some() {
            return Err("Invalid/duplicate shadow name.".into());
        }
    }
    if result.is_empty() {
        return Err("Missing/empty ShadowNames section; supply decoded text, not PFIL.".into());
    }
    Ok(result)
}

fn read_text(path: &Path) -> Result<String> {
    // File.ReadAllLines detects UTF-8/UTF-16/UTF-32 BOMs. Keep the same decoded-text
    // input contract (never silently interpret PFIL or search for another file).
    let bytes = fs::read(safety::input(path)?)?;
    if bytes.starts_with(b"PFIL") {
        return Err(invalid("Supply decoded text, not PFIL.").into());
    }
    let text = if bytes.starts_with(&[0xff, 0xfe, 0, 0]) || bytes.starts_with(&[0, 0, 0xfe, 0xff]) {
        let little = bytes[0] == 0xff;
        if (bytes.len() - 4) % 4 != 0 {
            return Err(invalid("Truncated UTF-32 text.").into());
        }
        let mut text = String::new();
        for chunk in bytes[4..].chunks_exact(4) {
            let v = if little {
                u32::from_le_bytes(chunk.try_into().unwrap())
            } else {
                u32::from_be_bytes(chunk.try_into().unwrap())
            };
            text.push(char::from_u32(v).ok_or_else(|| invalid("Invalid UTF-32 text."))?);
        }
        text
    } else if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        let little = bytes[0] == 0xff;
        if (bytes.len() - 2) % 2 != 0 {
            return Err(invalid("Truncated UTF-16 text.").into());
        }
        let words: Vec<_> = bytes[2..]
            .chunks_exact(2)
            .map(|v| {
                if little {
                    u16::from_le_bytes([v[0], v[1]])
                } else {
                    u16::from_be_bytes([v[0], v[1]])
                }
            })
            .collect();
        String::from_utf16(&words)?
    } else {
        String::from_utf8(bytes)?
            .trim_start_matches('\u{feff}')
            .to_owned()
    };
    Ok(text)
}

fn objects(
    text: &str,
    names: &HashMap<i32, String>,
    shadows: &HashMap<String, Vec<u8>>,
    output: &Path,
) -> Result<Vec<Object>> {
    let header = text
        .split(['\r', '\n'])
        .find(|line| line.starts_with(";idx"))
        .ok_or_else(|| invalid("Missing objdef ;idx header."))?;
    let columns: Vec<_> = header
        .split(',')
        .map(|v| v.trim().trim_start_matches(';'))
        .collect();
    let mut indices = Vec::new();
    for column in ["shidx", "shsiz", "shtyp", "shacx", "shacz"] {
        indices.push(
            columns
                .iter()
                .position(|&c| c == column)
                .ok_or_else(|| invalid(format!("Missing objdef column {column}.")))?,
        );
    }
    let mut result = Vec::new();
    for line in text.split(['\r', '\n']) {
        let fields: Vec<_> = line.split(',').collect();
        let Ok(id) = fields[0].trim().parse::<i32>() else {
            continue;
        };
        if !matches!(id, 12 | 42 | 46) {
            continue;
        }
        let value = |idx: usize| -> Result<i32> {
            Ok(fields
                .get(idx)
                .ok_or_else(|| invalid("Truncated objdef row."))?
                .trim()
                .parse()?)
        };
        let shidx = value(indices[0])?;
        let shsiz = value(indices[1])?;
        let shtyp = value(indices[2])?;
        let shacx = value(indices[3])?;
        let shacz = value(indices[4])?;
        let name = fields
            .get(52)
            .ok_or_else(|| invalid("Truncated objdef name column."))?
            .trim()
            .to_owned();
        let mapped = names.get(&shidx).cloned();
        let candidate = match id {
            42 => "GerHau02_shadow.bmp",
            46 => "GerNadelbaum_shadow.bmp",
            _ => "round_small_shadow.bmp",
        };
        let selected = mapped.as_deref().unwrap_or(candidate);
        let source = shadows.get(&safety::case_key(selected));
        let preview = if let Some(source) = source {
            let file = format!("object-{id}-ground-assumption.png");
            let frame = ShadowDocument::parse(source)?.decode_frame(0)?;
            image::write_png(
                &output.join(&file),
                720,
                400,
                &image::ground_preview(&frame, shsiz, shtyp, shacx, shacz),
            )?;
            Some(file)
        } else {
            None
        };
        result.push(Object {
            id,
            name,
            shidx,
            shsiz,
            shtyp,
            shacx,
            shacz,
            correction_screen_x: f64::from(shacx.wrapping_sub(shacz)) / 2.0,
            correction_screen_y: f64::from(shacx.wrapping_add(shacz)) / 4.0,
            candidate_entry: if mapped.is_some() {
                None
            } else {
                Some(candidate.to_owned())
            },
            status: if mapped.is_some() {
                "ID from supplied ShadowNames"
            } else {
                "UNVERIFIED candidate; ShadowNames missing"
            }
            .into(),
            mapped_entry: mapped,
            found: source.is_some(),
            preview,
            frame: 0,
            preview_limit: PREVIEW_LIMIT.into(),
        });
    }
    Ok(result)
}

/// Process only the explicitly supplied ZIP; every non-directory entry is audited.
/// Entry failures remain in report.json and make the CLI return 1.
pub fn run(
    input: &Path,
    output: &Path,
    objdef: Option<&Path>,
    names: Option<&Path>,
) -> Result<Report> {
    let input = safety::input(input)?;
    // Validate optional inputs before creating an output directory.
    let objdef_text = objdef.map(read_text).transpose()?;
    let names = names
        .map(read_text)
        .transpose()?
        .map(|v| read_shadow_names(&v).map_err(invalid))
        .transpose()?
        .unwrap_or_default();
    let output = safety::create_output(output)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1); // FileShare.Read: keep the hashed archive stable.
    }
    let mut file = options.open(input)?;
    let input_bytes = file.metadata()?.len();
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    let hash = format!("{:X}", hash.finalize());
    file.seek(SeekFrom::Start(0))?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut report = Report {
        hash,
        input_bytes,
        entries: archive.len(),
        documents: 0,
        frames: 0,
        directories: 0,
        other_bmps: 0,
        failures: Vec::new(),
        samples: Vec::new(),
        objects: Vec::new(),
        inventory: Vec::new(),
    };
    let mut shadows: HashMap<String, Vec<u8>> = HashMap::new();
    let mut seen = HashSet::new();
    for index in 0..archive.len() {
        // by_index opens/decompresses the entry without extraction. Read to EOF to
        // ensure CRC and actual payload are checked, not just central metadata.
        let mut entry = match archive.by_index(index) {
            Ok(entry) => entry,
            Err(error) => {
                report.failures.push(Failure {
                    entry: format!("ZIP entry #{index}"),
                    error: error.to_string(),
                });
                continue;
            }
        };
        let name = entry.name().to_owned();
        let result = (|| -> std::result::Result<(), String> {
            safety::register_path(&name, &mut seen).map_err(|e| e.to_string())?;
            if entry.unix_mode().is_some_and(|mode| {
                matches!(
                    mode & 0o170000,
                    0o120000 | 0o020000 | 0o060000 | 0o010000 | 0o140000
                )
            }) {
                return Err("ZIP symlink/device entries are forbidden.".into());
            }
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            if bytes.len() as u64 != entry.size() {
                return Err("ZIP entry payload length mismatch.".into());
            }
            if name.ends_with('/') {
                if !bytes.is_empty() {
                    return Err("Directory contains data.".into());
                }
                report.directories += 1;
                report.inventory.push(Inventory::Directory {
                    entry: name.clone(),
                    kind: "directory",
                });
                return Ok(());
            }
            let audit = bmp::audit_bmp(&bytes)?;
            if !name.to_ascii_lowercase().starts_with(SHADOW_PREFIX) {
                report.other_bmps += 1;
                report.inventory.push(Inventory::Other {
                    entry: name.clone(),
                    kind: "other BMP",
                    audit,
                });
                return Ok(());
            }
            let document = ShadowDocument::parse(&bytes).map_err(shadow_error)?;
            let frame = document.decode_frame(0).map_err(shadow_error)?;
            report.documents += 1;
            report.frames += 1;
            let leaf_name = name.rsplit('/').next().unwrap();
            let leaf = safety::case_key(leaf_name);
            if shadows.contains_key(&leaf) {
                return Err("Duplicate/alias shadow leaf name.".into());
            }
            shadows.insert(leaf.clone(), bytes);
            report.inventory.push(Inventory::Shadow {
                entry: name.clone(),
                kind: "shadow",
                audit,
                trailing_byte_count: document.trailing_byte_count(),
                min_strength: *frame.alpha_mask().iter().min().unwrap(),
                max_strength: *frame.alpha_mask().iter().max().unwrap(),
            });
            if SAMPLES.contains(&leaf_name.to_ascii_lowercase().as_str()) {
                let file = format!("sample-{}.png", report.samples.len());
                image::write_png(
                    &output.join(&file),
                    frame.width(),
                    frame.height(),
                    frame.argb_pixels(),
                )
                .map_err(|e| e.to_string())?;
                let ground_file = format!("sample-{}-ground.png", report.samples.len());
                let ground: Vec<_> = frame
                    .alpha_mask()
                    .iter()
                    .map(|&v| darken_argb(image::GROUND, v))
                    .collect();
                image::write_png(
                    &output.join(&ground_file),
                    frame.width(),
                    frame.height(),
                    &ground,
                )
                .map_err(|e| e.to_string())?;
                report.samples.push(Sample {
                    entry: name.clone(),
                    file,
                    ground_file,
                    width: frame.width(),
                    height: frame.height(),
                });
            }
            Ok(())
        })();
        if let Err(error) = result {
            report.failures.push(Failure { entry: name, error });
        }
    }
    if let Some(text) = objdef_text {
        report.objects = objects(&text, &names, &shadows, &output)?;
    }
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("report.json"))?;
    serde_json::to_writer_pretty(file, &report)?;
    Ok(report)
}

pub fn cli(args: Vec<OsString>) -> i32 {
    if !(2..=4).contains(&args.len()) {
        eprintln!("Usage: shad-probe-rs <readonly shad.dat> <NEW TEMP directory> [decoded objdef.txt] [decoded cl_shado.txt]");
        return 2;
    }
    let paths: Vec<_> = args.into_iter().map(PathBuf::from).collect();
    if let Err(error) = safety::validate_output(&paths[1], &safety::temp_root()) {
        eprintln!("{error}");
        return 2;
    }
    match run(
        &paths[0],
        &paths[1],
        paths.get(2).map(PathBuf::as_path),
        paths.get(3).map(PathBuf::as_path),
    ) {
        Ok(report) => {
            println!(
                "Entries={} Documents={} Frames={} Directories={} OtherBMPs={} Failures={}",
                report.entries,
                report.documents,
                report.frames,
                report.directories,
                report.other_bmps,
                report.failures.len()
            );
            println!("Report: {}", paths[1].join("report.json").display());
            i32::from(!report.failures.is_empty())
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}
