//! Full particle-inventory differential against an independent port of the frozen
//! C# editor decoder. This validates editor parity, not original game execution.
pub mod frozen;

use against_rome_data::AlrDocument;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    time::SystemTime,
};

pub const EXPECTED_MEMBERS: usize = 68;
pub const EXPECTED_FRAMES: u64 = 11_345;
pub type Result<T> = std::result::Result<T, String>;

#[derive(Debug, Serialize)]
pub struct Record {
    pub member: String,
    pub sha256: String,
    pub frames: usize,
    pub variants: usize,
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub scope: &'static str,
    pub oracle: &'static str,
    pub members: usize,
    pub frames: u64,
    pub decodes: u64,
    pub pixels: u64,
    pub failures: u32,
    pub source_unchanged: bool,
    pub archive_sha256: String,
    pub native_dll_sha256: Option<String>,
    pub records: Vec<Record>,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn hash_file(path: &Path) -> Result<String> {
    let mut input = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = input.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

#[derive(Debug, PartialEq, Eq)]
struct Identity {
    length: u64,
    modified: SystemTime,
    sha256: String,
}

fn identity(path: &Path) -> Result<Identity> {
    let before = fs::metadata(path).map_err(|e| e.to_string())?;
    let modified = before.modified().map_err(|e| e.to_string())?;
    let sha256 = hash_file(path)?;
    let after = fs::metadata(path).map_err(|e| e.to_string())?;
    if before.len() != after.len() || modified != after.modified().map_err(|e| e.to_string())? {
        return Err("Source archive changed while hashing.".into());
    }
    Ok(Identity {
        length: before.len(),
        modified,
        sha256,
    })
}

fn ensure_unchanged(path: &Path, before: &Identity) -> Result<()> {
    if *before != identity(path)? {
        return Err("Source archive changed during validation.".into());
    }
    Ok(())
}

fn is_within(path: &Path, root: &Path) -> bool {
    // canonicalize() returns consistent extended paths on Windows; comparison
    // remains component-boundary aware, including case-insensitive Windows paths.
    #[cfg(windows)]
    {
        let path = path.to_string_lossy().to_lowercase();
        let root = root.to_string_lossy().to_lowercase();
        Path::new(&path).starts_with(Path::new(&root))
    }
    #[cfg(not(windows))]
    {
        path.starts_with(root)
    }
}

/// Output is a fresh .json in TEMP or the repository's re_workspace. Canonical
/// parents block link escapes. No directory is created, nor any input overwritten.
pub fn validate_output(
    game_root: &Path,
    output: &Path,
    allowed_roots: &[PathBuf],
) -> Result<PathBuf> {
    if output.extension().and_then(|x| x.to_str()) != Some("json") {
        return Err("Output must have the .json extension.".into());
    }
    if output
        .components()
        .any(|x| matches!(x, Component::ParentDir))
    {
        return Err("Output must not contain parent traversal.".into());
    }
    // Reject dangling links as well as ordinary existing files/directories.
    if fs::symlink_metadata(output).is_ok() {
        return Err("Output already exists.".into());
    }
    let name = output.file_name().ok_or("Output requires a file name.")?;
    #[cfg(windows)]
    if name.to_string_lossy().contains(':') || name.to_string_lossy().ends_with([' ', '.']) {
        return Err("Invalid Windows output filename.".into());
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = fs::canonicalize(parent).map_err(|e| format!("Output parent: {e}"))?;
    let game = fs::canonicalize(game_root).map_err(|e| format!("Game root: {e}"))?;
    if is_within(&parent, &game) {
        return Err("Output must be outside the installed game root.".into());
    }
    if !allowed_roots
        .iter()
        .filter_map(|r| fs::canonicalize(r).ok())
        .any(|r| is_within(&parent, &r))
    {
        return Err("Output must be a new JSON under TEMP or re_workspace.".into());
    }
    let resolved = parent.join(name);
    if fs::symlink_metadata(&resolved).is_ok() {
        return Err("Output already exists.".into());
    }
    Ok(resolved)
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub frames: u64,
    pub decodes: u64,
    pub pixels: u64,
}

/// Neither the frozen parser nor its scanline/color code calls the current parser.
pub fn compare_member(source: &[u8], member: &str) -> Result<Counts> {
    let current = AlrDocument::parse(source).map_err(|e| format!("{member}: current: {e}"))?;
    let frozen = frozen::Document::parse(source).map_err(|e| format!("{member}: frozen: {e}"))?;
    if current.version != frozen.version
        || current.layout_columns != frozen.layout_columns
        || current.layout_rows != frozen.layout_rows
        || current.anchor_width != frozen.anchor_width
        || current.anchor_height != frozen.anchor_height
        || current.frames.len() != frozen.frames.len()
        || current.palette_variant_count != frozen.palette_variant_count
    {
        return Err(format!("Container mismatch: {member}"));
    }
    let mut counts = Counts {
        frames: current.frames.len() as u64,
        ..Counts::default()
    };
    for (index, frame) in current.frames.iter().enumerate() {
        let a = &frame.info;
        let b = &frozen.frames[index].info;
        if (
            a.offset_x,
            a.offset_y,
            a.width,
            a.height,
            a.palette_color_count,
            a.packed_size,
            a.stored_pixel_bytes,
        ) != (
            b.offset_x,
            b.offset_y,
            b.width,
            b.height,
            b.palette_color_count,
            b.packed_size,
            b.stored_pixel_bytes,
        ) {
            return Err(format!("Frame metadata mismatch: {member}, frame={index}"));
        }
        for variant in 0..current.palette_variant_count {
            let actual = current
                .decode_frame_argb(index, variant)
                .map_err(|e| format!("{member}, frame={index}, palette={variant}: current: {e}"))?;
            let expected = frozen
                .decode_frame_argb(index, variant)
                .map_err(|e| format!("{member}, frame={index}, palette={variant}: frozen: {e}"))?;
            if actual != expected {
                return Err(format!(
                    "Pixel mismatch: {member}, frame={index}, palette={variant}"
                ));
            }
            counts.decodes += 1;
            counts.pixels += actual.len() as u64;
        }
    }
    Ok(counts)
}

fn audit(archive_path: &Path, inventory_path: &Path, native_dll: Option<&Path>) -> Result<Summary> {
    let before = identity(archive_path)?;
    let inventory: serde_json::Value =
        serde_json::from_slice(&fs::read(inventory_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let assets = inventory
        .get("target_alr_assets")
        .and_then(|v| v.as_object())
        .ok_or("Inventory missing target_alr_assets object.")?;
    if assets.len() != EXPECTED_MEMBERS {
        return Err("Incomplete particle inventory scope: expected 68 members.".into());
    }
    let mut archive = zip::ZipArchive::new(File::open(archive_path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    // Match C# OrdinalIgnoreCase, but insist there is exactly one match. Member
    // bytes, including every palette, are subsequently checked against the SHA.
    let mut names = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        names.push(
            archive
                .by_index(index)
                .map_err(|e| e.to_string())?
                .name()
                .to_owned(),
        );
    }
    let mut records = Vec::new();
    let mut counts = Counts::default();
    for (name, asset) in assets {
        let matches: Vec<_> = names
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.eq_ignore_ascii_case(name))
            .map(|(index, _)| index)
            .collect();
        if matches.len() != 1 {
            return Err(format!("Missing/ambiguous archive member: {name}"));
        }
        let mut source = Vec::new();
        archive
            .by_index(matches[0])
            .map_err(|e| e.to_string())?
            .read_to_end(&mut source)
            .map_err(|e| e.to_string())?;
        let member_hash = digest(&source);
        if asset.get("sha256").and_then(|v| v.as_str()) != Some(member_hash.as_str()) {
            return Err(format!("Inventory identity changed: {name}"));
        }
        let member_counts = compare_member(&source, name)?;
        let current = AlrDocument::parse(&source).map_err(|e| e.to_string())?;
        records.push(Record {
            member: name.clone(),
            sha256: member_hash,
            frames: member_counts.frames as usize,
            variants: current.palette_variant_count,
        });
        counts.frames += member_counts.frames;
        counts.decodes += member_counts.decodes;
        counts.pixels += member_counts.pixels;
    }
    if counts.frames != EXPECTED_FRAMES {
        return Err(format!(
            "Incomplete particle inventory scope: expected 11345 frames, got {}.",
            counts.frames
        ));
    }
    ensure_unchanged(archive_path, &before)?;
    let native_dll_sha256 = native_dll.map(hash_file).transpose()?;
    Ok(Summary {
        scope: "Every frame and palette variant of the 68 identity-matched particle-linked ALRs",
        oracle: "Frozen C# editor decoder from e3a21d9; not original game runtime",
        members: records.len(),
        frames: counts.frames,
        decodes: counts.decodes,
        pixels: counts.pixels,
        failures: 0,
        source_unchanged: true,
        archive_sha256: before.sha256,
        native_dll_sha256,
        records,
    })
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    #[test]
    fn detects_hash_length_and_mtime_changes() {
        let path =
            std::env::temp_dir().join(format!("arm-scanline-identity-{}.bin", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(b"abcd").unwrap();
        drop(file);
        let before = identity(&path).unwrap();
        assert!(ensure_unchanged(&path, &before).is_ok());
        fs::write(&path, b"abce").unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(before.modified))
            .unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().modified().unwrap(),
            before.modified
        );
        assert!(ensure_unchanged(&path, &before).is_err());
        fs::write(&path, b"abcde").unwrap();
        assert!(ensure_unchanged(&path, &before).is_err());
        fs::write(&path, b"abcd").unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(
                fs::FileTimes::new()
                    .set_modified(before.modified + std::time::Duration::from_secs(10)),
            )
            .unwrap();
        assert!(ensure_unchanged(&path, &before).is_err());
        fs::remove_file(path).unwrap();
    }
}

pub fn run(
    game_root: &Path,
    inventory: &Path,
    output: &Path,
    native_dll: Option<&Path>,
    allowed_roots: &[PathBuf],
) -> Result<Summary> {
    let output = validate_output(game_root, output, allowed_roots)?;
    let result = audit(&game_root.join("alr.dat"), inventory, native_dll)?;
    let bytes = serde_json::to_vec(&result).map_err(|e| e.to_string())?;
    // Recheck canonical parent after validation; create_new is the final atomic
    // no-clobber guard, including a concurrent creator at the destination.
    if validate_output(game_root, &output, allowed_roots)? != output {
        return Err("Output parent changed during validation.".into());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .map_err(|e| format!("Output {}: {e}", output.display()))?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    Ok(result)
}

/// 0 = full-scope success, 1 = validation/I/O failure, 2 = invalid argument count.
pub fn cli(args: &[OsString], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    if args.len() != 3 {
        let _ = writeln!(
            stderr,
            "Usage: <game-root> <particle-assets-inventory.json> <new-output.json>"
        );
        return 2;
    }
    let game_root = Path::new(&args[0]);
    let inventory = Path::new(&args[1]);
    let output = Path::new(&args[2]);
    let native_dll = std::env::var_os("ARM_NATIVE_DATA_DLL").map(PathBuf::from);
    let roots = [
        std::env::temp_dir(),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../re_workspace"),
    ];
    match run(game_root, inventory, output, native_dll.as_deref(), &roots) {
        Ok(r) => {
            let _ = writeln!(stdout, "members={} frames={} decodes={} pixels={} failures=0 source_unchanged=true output={}", r.members, r.frames, r.decodes, r.pixels, output.display());
            0
        }
        Err(e) => {
            let _ = writeln!(stderr, "{e}");
            1
        }
    }
}
