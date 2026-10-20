//! Port of tools/Repair-LanguageBackup.ps1: builds the managed language backup
//! (`.against-rome-modifier-language-backup`) from a pristine original install
//! when the ToEng overlay is already active but no backup exists. The backup
//! is staged in a sibling temp directory and moved into place atomically; the
//! game files themselves are never modified.
use crate::{
    create_synced,
    install::{safe_language_path, sha256, LANGUAGE_BACKUP},
};
use serde_json::{json, Value};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

fn invalid(s: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, s.into())
}

#[derive(Debug, Clone)]
pub struct RepairReport {
    pub backup_root: PathBuf,
    pub existing_files: Vec<String>,
    pub dry_run: bool,
}
impl RepairReport {
    pub fn to_json(&self) -> Value {
        json!({"backup": self.backup_root.display().to_string(),
            "existingFiles": self.existing_files, "existingCount": self.existing_files.len(),
            "missingCount": 0, "dryRun": self.dry_run})
    }
}

fn is_original_candidate(p: &Path) -> bool {
    p.is_dir() && p.join("Against_Rome.exe").is_file() && p.join("ToEng").is_dir()
}

/// Mirrors the script's auto-discovery: among the immediate subdirectories of
/// `root`, exactly one must contain `Against_Rome.exe` and a `ToEng` directory.
pub fn discover_original_in(root: &Path) -> io::Result<PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(root)? {
        let p = entry?.path();
        if is_original_candidate(&p) {
            found.push(p);
        }
    }
    if found.len() != 1 {
        return Err(invalid(format!(
            "Could not uniquely locate the original game directory under: {}",
            root.display()
        )));
    }
    Ok(found.remove(0))
}

/// The script searches the repository root (parent of tools/). Here: walk up
/// from the current directory, then the executable, and use the first
/// ancestor having any candidate (ambiguity there is an error).
pub fn discover_original() -> io::Result<PathBuf> {
    let mut starts = Vec::new();
    if let Ok(d) = std::env::current_dir() {
        starts.push(d);
    }
    if let Some(d) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        starts.push(d);
    }
    let mut last = None;
    for start in starts {
        for dir in start.ancestors().take(8) {
            let any = fs::read_dir(dir)
                .map(|rd| rd.flatten().any(|e| is_original_candidate(&e.path())))
                .unwrap_or(false);
            if any {
                return discover_original_in(dir);
            }
            last = Some(dir.to_path_buf());
        }
    }
    Err(invalid(format!(
        "Could not uniquely locate the original game directory under: {}",
        last.map(|p| p.display().to_string()).unwrap_or_default()
    )))
}

fn overlay_files(root: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for item in fs::read_dir(root)? {
        let item = item?;
        let kind = item.file_type()?;
        if kind.is_symlink() {
            return Err(invalid("symbolic links are not language assets"));
        }
        if kind.is_dir() {
            overlay_files(&item.path(), out)?;
        } else if kind.is_file() {
            out.push(item.path());
        }
    }
    Ok(())
}

struct TempGuard<'a> {
    path: PathBuf,
    game: &'a Path,
}
impl Drop for TempGuard<'_> {
    fn drop(&mut self) {
        // Refuse to clean anything outside the game directory.
        if self.path.exists() && self.path.starts_with(self.game) && self.path != self.game {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

/// Validates everything first; with `dry_run` nothing is written.
pub fn repair_language_backup(
    game: &Path,
    original: &Path,
    dry_run: bool,
) -> io::Result<RepairReport> {
    if !game.join("Against_Rome.exe").is_file() {
        return Err(invalid(format!("Invalid game directory: {}", game.display())));
    }
    if !original.join("Against_Rome.exe").is_file() {
        return Err(invalid(format!(
            "Invalid original game directory: {}",
            original.display()
        )));
    }
    let overlay_root = game.join("ToEng");
    let backup_root = game.join(LANGUAGE_BACKUP);
    if backup_root.exists() {
        return Err(invalid(if backup_root.join("manifest.json").is_file() {
            format!("A managed language backup already exists: {}", backup_root.display())
        } else {
            format!(
                "An incomplete managed language backup directory exists: {}",
                backup_root.display()
            )
        }));
    }
    let mut overlay = Vec::new();
    overlay_files(&overlay_root, &mut overlay)?;
    overlay.sort();
    if overlay.is_empty() {
        return Err(invalid(format!(
            "No language overlay files were found: {}",
            overlay_root.display()
        )));
    }
    let mut existing = Vec::new();
    for file in &overlay {
        let rel = file
            .strip_prefix(&overlay_root)
            .map_err(|_| invalid("invalid asset path"))?
            .to_string_lossy()
            .replace('\\', "/");
        let original_file = safe_language_path(original, &rel)?;
        let active_file = safe_language_path(game, &rel)?;
        if !original_file.is_file() {
            return Err(invalid(format!("The original source is incomplete; missing: {rel}")));
        }
        if !active_file.is_file() {
            return Err(invalid(format!(
                "The active language overlay is incomplete; missing: {rel}"
            )));
        }
        let overlay_hash = sha256(&fs::read(file)?);
        if sha256(&fs::read(&active_file)?) != overlay_hash {
            return Err(invalid(format!(
                "The active file does not match the ToEng overlay: {rel}"
            )));
        }
        if sha256(&fs::read(&original_file)?) == overlay_hash {
            return Err(invalid(format!(
                "The proposed original file is identical to the overlay: {rel}"
            )));
        }
        existing.push(rel);
    }
    let report = RepairReport {
        backup_root: backup_root.clone(),
        existing_files: existing.clone(),
        dry_run,
    };
    if dry_run {
        return Ok(report);
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temp = game.join(format!("{LANGUAGE_BACKUP}.tmp-{:x}{:x}", std::process::id(), nonce));
    let _guard = TempGuard { path: temp.clone(), game };
    let files_root = temp.join("files");
    fs::create_dir_all(&files_root)?;
    for rel in &existing {
        let dst = safe_language_path(&files_root, rel)?;
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        create_synced(&dst, &fs::read(safe_language_path(original, rel)?)?)?;
    }
    let manifest =
        serde_json::to_vec_pretty(&json!({"ExistingFiles": existing, "MissingFiles": []}))?;
    create_synced(&temp.join("manifest.json"), &manifest)?;
    if backup_root.exists() {
        return Err(invalid(format!(
            "A managed language backup already exists: {}",
            backup_root.display()
        )));
    }
    fs::rename(&temp, &backup_root)?;
    Ok(report)
}
