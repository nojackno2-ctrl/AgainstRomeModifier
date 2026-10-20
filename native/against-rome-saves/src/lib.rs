//! Real ZIP save backup/restore. Directory replacement is staged on the same filesystem.
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use zip::{write::FileOptions, ZipArchive, ZipWriter};
pub mod ai_repair;
pub mod backup_health;
mod pfil;
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
const LIMIT: u64 = 512 * 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);
fn fail<T>(message: impl Into<String>) -> Result<T> {
    Err(std::io::Error::new(std::io::ErrorKind::InvalidData, message.into()).into())
}
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct Manifest {
    #[serde(default)]
    pub orig_folder: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub level: String,
    #[serde(default)]
    pub backup_time: String,
}
// C# accepts a null manifest and nullable/missing string fields. Normalize them
// to empty strings so catalog reads can still fill Title/Level from save.ini.
impl<'de> Deserialize<'de> for Manifest {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "PascalCase")]
        struct Fields {
            orig_folder: Option<String>,
            title: Option<String>,
            level: Option<String>,
            backup_time: Option<String>,
        }
        Ok(match Option::<Fields>::deserialize(deserializer)? {
            Some(fields) => Self {
                orig_folder: fields.orig_folder.unwrap_or_default(),
                title: fields.title.unwrap_or_default(),
                level: fields.level.unwrap_or_default(),
                backup_time: fields.backup_time.unwrap_or_default(),
            },
            None => Self::default(),
        })
    }
}
#[derive(Debug, Serialize)]
pub struct SaveInfo {
    pub folder: String,
    pub title: String,
    pub level: String,
    pub parsed: bool,
}
#[derive(Debug, Serialize)]
pub struct BackupInfo {
    pub file_name: String,
    pub manifest: Manifest,
    pub parsed: bool,
}
#[derive(Debug, Serialize)]
pub struct Catalog {
    pub saves: Vec<SaveInfo>,
    pub backups: Vec<BackupInfo>,
}
#[derive(Debug, Default, Serialize)]
pub struct RestoreResult {
    pub cleanup_warnings: Vec<String>,
}
/// Outcome of a read-only integrity check of a backup ZIP or a live save folder.
#[derive(Debug, Default, Serialize)]
pub struct VerifyReport {
    pub entries: usize,
    pub total_bytes: u64,
    pub title: String,
    pub level: String,
    /// Blocking problems: restoring or trusting this item is unsafe.
    pub errors: Vec<String>,
    /// Non-blocking oddities (unreadable manifest, odd folder name, ...).
    pub warnings: Vec<String>,
}
impl VerifyReport {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}
#[derive(Debug, Serialize, PartialEq)]
pub enum AiRepairResult {
    AlreadyRepaired,
    Changed { backup: String },
}
pub(crate) fn remove_dir_all_force(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if let Err(_) = fs::remove_dir_all(path) {
        fn make_writable_and_remove(p: &Path) -> std::io::Result<()> {
            if let Ok(m) = fs::symlink_metadata(p) {
                let mut perm = m.permissions();
                if perm.readonly() {
                    perm.set_readonly(false);
                    let _ = fs::set_permissions(p, perm);
                }
            }
            if p.is_dir() {
                for entry in fs::read_dir(p)? {
                    let entry = entry?;
                    make_writable_and_remove(&entry.path())?;
                }
                fs::remove_dir(p)?;
            } else {
                fs::remove_file(p)?;
            }
            Ok(())
        }
        make_writable_and_remove(path)?;
    }
    Ok(())
}
pub fn simple_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name != "."
        && name != ".."
        && !name.starts_with(' ')
        && !name.ends_with(['.', ' '])
        && !name.chars().any(|c| c < ' ' || "<>:\"/\\|?*".contains(c))
        && {
            let base = name.split('.').next().unwrap().to_ascii_uppercase();
            !matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
                && !(base.len() == 4
                    && (base.starts_with("COM") || base.starts_with("LPT"))
                    && matches!(base.as_bytes()[3], b'0'..=b'9'))
        }
}
fn child(root: &Path, name: &str) -> Result<PathBuf> {
    if !simple_name(name) {
        return fail("invalid simple name");
    }
    Ok(root.join(name))
}
fn reject_links(path: &Path) -> Result<()> {
    // Check every existing ancestor, including junction/reparse points on Windows.
    for p in path.ancestors() {
        match fs::symlink_metadata(p) {
            Ok(m) => {
                if m.file_type().is_symlink() {
                    return fail(format!("symbolic link rejected: {}", p.display()));
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if m.file_attributes() & 0x400 != 0 {
                        return fail(format!("reparse point rejected: {}", p.display()));
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
fn absolute(path: &Path) -> Result<PathBuf> {
    let p = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    if p.components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return fail("parent path components rejected");
    }
    reject_links(&p)?;
    Ok(p)
}
fn create_dir(path: &Path) -> Result<()> {
    reject_links(path)?;
    fs::create_dir_all(path)?;
    reject_links(path)
}
fn unique(root: &Path, prefix: &str) -> PathBuf {
    root.join(format!(
        ".{prefix}_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}
pub fn read_save_info(bytes: &[u8]) -> Result<(String, String)> {
    let decoded = if bytes.starts_with(b"PFIL") {
        against_rome_data::lzss::decompress_pfil(bytes, None)?
    } else {
        bytes.to_vec()
    };
    let (text, _, _) = encoding_rs::WINDOWS_1251.decode(&decoded);
    let lines: Vec<_> = text
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let mut title = String::new();
    let mut level = String::new();
    for pair in lines.windows(2) {
        if pair[0].contains("[titel]") {
            title = pair[1].into();
        }
        if pair[0].contains("[orglevelname]") {
            level = pair[1].into();
        }
    }
    Ok((title, level))
}
pub struct SaveService {
    game: PathBuf,
    backups: PathBuf,
}
impl SaveService {
    /// Explicit migration; creates a full save ZIP before publishing changed scr.dat.
    pub fn repair_endless_ai(&self, folder: &str) -> Result<AiRepairResult> {
        let source = child(&self.root(), folder)?;
        reject_tree(&source)?;
        let scr = source.join("CLAK/scr.dat");
        let raw = fs::read(&scr)?;
        let Some(repaired) = ai_repair::repair_payload(&raw)? else {
            return Ok(AiRepairResult::AlreadyRepaired);
        };
        let permissions = fs::metadata(&scr)?.permissions();
        if permissions.readonly() {
            return fail("scr.dat is read-only");
        }
        // Match the C# UI contract: the safety backup carries the save's Title/Level.
        let (title, level) = fs::read(source.join("save.ini"))
            .ok()
            .and_then(|b| read_save_info(&b).ok())
            .unwrap_or_default();
        let backup = self.backup(folder, &title, &level)?;
        // Never overwrite scr.dat unless the safety backup itself is provably restorable.
        let health = self.verify_backup(&backup)?;
        if !health.is_ok() {
            let _ = self.delete_backup(&backup);
            return fail(format!(
                "safety backup failed verification, scr.dat untouched: {}",
                health.errors.join("; ")
            ));
        }
        let stage = unique(scr.parent().unwrap(), "ai_repair_tmp");
        let result = (|| -> Result<()> {
            let mut out = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&stage)?;
            out.write_all(&repaired)?;
            out.sync_all()?;
            drop(out);
            fs::set_permissions(&stage, permissions)?;
            reject_links(&scr)?;
            if fs::read(&scr)? != raw {
                return fail("scr.dat changed during repair; backup retained");
            }
            fs::rename(&stage, &scr)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&stage);
        }
        result?;
        Ok(AiRepairResult::Changed { backup })
    }
    pub fn new(game: impl AsRef<Path>, backups: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            game: absolute(game.as_ref())?,
            backups: absolute(backups.as_ref())?,
        })
    }
    fn root(&self) -> PathBuf {
        self.game.join("SAVE")
    }
    pub fn scan(&self) -> Result<Catalog> {
        let root = self.root();
        reject_links(&root)?;
        let mut saves = vec![];
        if root.exists() {
            for entry in fs::read_dir(&root)? {
                // A single linked/unreadable entry is skipped, never followed, and must not abort the catalog.
                let Ok(entry) = entry else { continue };
                if reject_links(&entry.path()).is_err()
                    || !entry.file_type().is_ok_and(|t| t.is_dir())
                {
                    continue;
                }
                let ini = entry.path().join("save.ini");
                if reject_links(&ini).is_err() || !ini.is_file() {
                    continue;
                }
                let parsed = fs::read(ini).ok().and_then(|b| read_save_info(&b).ok());
                let (title, level) = parsed.clone().unwrap_or_default();
                saves.push(SaveInfo {
                    folder: entry.file_name().to_string_lossy().into(),
                    title,
                    level,
                    parsed: parsed.is_some(),
                });
            }
        }
        create_dir(&self.backups)?;
        let mut backups = vec![];
        for entry in fs::read_dir(&self.backups)? {
            let Ok(entry) = entry else { continue };
            if entry
                .path()
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
            {
                if reject_links(&entry.path()).is_err() {
                    continue;
                }
                let (mut manifest, parsed) = self.backup_info(&entry.path());
                legacy(&entry.file_name().to_string_lossy(), &mut manifest);
                backups.push(BackupInfo {
                    file_name: entry.file_name().to_string_lossy().into(),
                    manifest,
                    parsed,
                });
            }
        }
        saves.sort_by(|a, b| a.folder.cmp(&b.folder));
        backups.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        Ok(Catalog { saves, backups })
    }
    /// Mirrors C# GetOrReadBackup: metadata read before a failure is kept and the failure
    /// only clears `parsed`. This never gates restore; restore re-validates the archive itself.
    fn backup_info(&self, path: &Path) -> (Manifest, bool) {
        let mut m = Manifest::default();
        let mut parsed = true;
        let Ok(file) = File::open(path) else {
            return (m, false);
        };
        let Ok(mut zip) = ZipArchive::new(file) else {
            return (m, false);
        };
        let manifest = match zip.by_name("manifest.json") {
            Ok(mut e) => read_limited(&mut e, 4 * 1024 * 1024).and_then(|b| {
                Ok(serde_json::from_slice::<Manifest>(
                    b.strip_prefix(&[239, 187, 191]).unwrap_or(&b),
                )?)
            }),
            Err(zip::result::ZipError::FileNotFound) => Ok(Manifest::default()),
            Err(e) => Err(e.into()),
        };
        match manifest {
            Ok(found) => m = found,
            Err(_) => parsed = false,
        }
        if parsed && (m.title.is_empty() || m.level.is_empty()) {
            if let Ok(mut e) = zip.by_name("save.ini") {
                match read_limited(&mut e, LIMIT).and_then(|b| read_save_info(&b)) {
                    Ok((t, l)) => {
                        if m.title.is_empty() {
                            m.title = t;
                        }
                        if m.level.is_empty() {
                            m.level = l;
                        }
                    }
                    Err(_) => parsed = false,
                }
            }
        }
        (m, parsed)
    }
    pub fn backup(&self, folder: &str, title: &str, level: &str) -> Result<String> {
        let now = chrono::Local::now();
        let name = format!("Backup_{folder}_{}.zip", now.format("%Y%m%d_%H%M%S"));
        self.backup_as(folder, &name, title, level, &now.to_rfc3339())?;
        Ok(name)
    }
    pub fn backup_as(
        &self,
        folder: &str,
        file_name: &str,
        title: &str,
        level: &str,
        time: &str,
    ) -> Result<()> {
        let source = child(&self.root(), folder)?;
        reject_links(&source)?;
        if self.backups.starts_with(&source) {
            return fail("backup directory is inside save source");
        }
        if !source.is_dir() {
            return fail("save directory missing");
        }
        create_dir(&self.backups)?;
        let dest = child(&self.backups, file_name)?;
        reject_links(&dest)?;
        if dest.exists() {
            return fail("backup already exists");
        }
        let temp = unique(&self.backups, "backup_tmp");
        let result = (|| -> Result<()> {
            let f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            let mut zip = ZipWriter::new(f);
            let options =
                FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            let mut files = vec![];
            collect_files(&source, &source, &mut files)?;
            for (path, name) in files {
                if name.eq_ignore_ascii_case("manifest.json") {
                    continue;
                }
                zip.start_file(name, options)?;
                std::io::copy(&mut File::open(path)?, &mut zip)?;
            }
            zip.start_file("manifest.json", options)?;
            zip.write_all(&serde_json::to_vec_pretty(&Manifest {
                orig_folder: folder.into(),
                title: title.into(),
                level: level.into(),
                backup_time: time.into(),
            })?)?;
            zip.finish()?.sync_all()?;
            // Hard-link publication fails if a competing writer already published the name.
            if let Err(link_err) = fs::hard_link(&temp, &dest) {
                if !dest.exists() {
                    fs::rename(&temp, &dest)?;
                } else {
                    return Err(link_err.into());
                }
            } else {
                let _ = fs::remove_file(&temp);
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
    pub fn restore(&self, file_name: &str, folder: &str) -> Result<RestoreResult> {
        self.restore_impl(file_name, folder, || Ok(()))
    }
    fn restore_impl(
        &self,
        file_name: &str,
        folder: &str,
        before_publish: impl FnOnce() -> Result<()>,
    ) -> Result<RestoreResult> {
        let archive = child(&self.backups, file_name)?;
        reject_links(&archive)?;
        let root = self.root();
        create_dir(&root)?;
        let dest = child(&root, folder)?;
        reject_links(&dest)?;
        if dest.exists() && !dest.is_dir() {
            return fail("save destination is not a directory");
        }
        reject_tree(&dest)?;
        let temp = unique(&root, "restore_tmp");
        let old = unique(&root, "restore_old");
        fs::create_dir(&temp)?;
        let mut moved_old = false;
        let mut published = false;
        let result = (|| -> Result<()> {
            extract(&archive, &temp)?;
            if !temp.join("save.ini").is_file() {
                return fail("archive missing save.ini");
            }
            reject_links(&dest)?;
            if dest.exists() {
                fs::rename(&dest, &old)?;
                moved_old = true;
            }
            before_publish()?;
            fs::rename(&temp, &dest)?;
            published = true;
            Ok(())
        })();
        if let Err(error) = result {
            let rollback = (|| -> Result<()> {
                if published {
                    remove_dir_all_force(&dest)?;
                }
                if moved_old {
                    // Like C# RestoreBackup, never delete a destination we did not
                    // publish. Preserve both saves if another writer recreated it.
                    match fs::symlink_metadata(&dest) {
                        Ok(_) => return fail("save destination reappeared during rollback"),
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                        Err(e) => return Err(e.into()),
                    }
                    fs::rename(&old, &dest)?;
                }
                Ok(())
            })();
            let _ = remove_dir_all_force(&temp);
            if let Err(r) = rollback {
                return fail(format!(
                    "{error}; rollback failed: {r}; preserved original: {}",
                    old.display()
                ));
            }
            return Err(error);
        }
        let mut report = RestoreResult::default();
        if moved_old {
            if let Err(e) = remove_dir_all_force(&old) {
                report
                    .cleanup_warnings
                    .push(format!("{}: {e}", old.display()));
            }
        }
        Ok(report)
    }
    /// Reads every entry of a backup (forcing CRC checks) and applies the same path, size,
    /// duplicate and link rules as restore, without extracting anything.
    pub fn verify_backup(&self, file_name: &str) -> Result<VerifyReport> {
        let archive = child(&self.backups, file_name)?;
        reject_links(&archive)?;
        let mut report = VerifyReport::default();
        let mut zip = match ZipArchive::new(File::open(&archive)?) {
            Ok(z) => z,
            Err(e) => {
                report.errors.push(format!("not a readable ZIP: {e}"));
                return Ok(report);
            }
        };
        if zip.len() > 100000 {
            report.errors.push("too many entries".into());
            return Ok(report);
        }
        let mut seen = HashSet::new();
        let mut save_ini = None;
        let mut manifest = None;
        for i in 0..zip.len() {
            let mut e = match zip.by_index(i) {
                Ok(e) => e,
                Err(err) => {
                    report.errors.push(format!("entry {i}: {err}"));
                    continue;
                }
            };
            let name = e.name().to_string();
            if let Err(err) = validate_entry(&name) {
                report.errors.push(format!("{name}: {err}"));
                continue;
            }
            if !seen.insert(name.trim_end_matches('/').to_ascii_lowercase()) {
                report.errors.push(format!("{name}: duplicate archive path"));
                continue;
            }
            if let Some(mode) = e.unix_mode() {
                let ty = mode & 0o170000;
                if ty != 0 && ty != 0o100000 && ty != 0o040000 {
                    report.errors.push(format!("{name}: symbolic/special link"));
                    continue;
                }
            }
            report.entries += 1;
            if e.is_dir() {
                continue;
            }
            let declared = e.size();
            report.total_bytes = report.total_bytes.saturating_add(declared);
            if declared > LIMIT || report.total_bytes > LIMIT {
                report.errors.push("archive exceeds size limit".into());
                break;
            }
            let keep = matches!(name.as_str(), "save.ini" | "manifest.json");
            let mut data = vec![];
            let max_read = declared.saturating_add(1);
            let read = if keep {
                e.by_ref().take(max_read).read_to_end(&mut data)
            } else {
                std::io::copy(&mut e.by_ref().take(max_read), &mut std::io::sink())
                    .map(|n| n as usize)
            };
            match read {
                Ok(n) if n as u64 == declared => {}
                Ok(_) => {
                    report.errors.push(format!("{name}: size does not match header"));
                    continue;
                }
                Err(err) => {
                    report.errors.push(format!("{name}: {err}"));
                    continue;
                }
            }
            match name.as_str() {
                "save.ini" => save_ini = Some(data),
                "manifest.json" => manifest = Some(data),
                _ => {}
            }
        }
        match save_ini {
            None => report.errors.push("archive missing save.ini".into()),
            Some(b) => match read_save_info(&b) {
                Ok((t, l)) => {
                    report.title = t;
                    report.level = l;
                }
                Err(e) => report.warnings.push(format!("save.ini unreadable: {e}")),
            },
        }
        if let Some(b) = manifest {
            match serde_json::from_slice::<Manifest>(b.strip_prefix(&[239, 187, 191]).unwrap_or(&b)) {
                Ok(m) => {
                    if !m.orig_folder.is_empty() && !simple_name(&m.orig_folder) {
                        report
                            .warnings
                            .push("manifest OrigFolder is not a valid folder name".into());
                    }
                }
                Err(e) => report.warnings.push(format!("manifest.json unreadable: {e}")),
            }
        }
        Ok(report)
    }
    /// Read-only check of a live save folder: link-free, regular files, save.ini decodes.
    pub fn verify_save(&self, folder: &str) -> Result<VerifyReport> {
        let dir = child(&self.root(), folder)?;
        reject_tree(&dir)?;
        let mut report = VerifyReport::default();
        if !dir.is_dir() {
            report.errors.push("save directory missing".into());
            return Ok(report);
        }
        let mut files = vec![];
        if let Err(e) = collect_files(&dir, &dir, &mut files) {
            report.errors.push(e.to_string());
            return Ok(report);
        }
        report.entries = files.len();
        for (p, _) in &files {
            match fs::metadata(p) {
                Ok(m) => {
                    report.total_bytes = report.total_bytes.saturating_add(m.len());
                    if report.total_bytes > LIMIT {
                        report.errors.push("save directory exceeds size limit".into());
                        break;
                    }
                }
                Err(e) => {
                    report.errors.push(format!("{}: {e}", p.display()));
                }
            }
        }
        match fs::read(dir.join("save.ini")) {
            Err(_) => report.errors.push("save.ini missing".into()),
            Ok(b) => match read_save_info(&b) {
                Ok((t, l)) => {
                    report.title = t;
                    report.level = l;
                }
                Err(e) => report.errors.push(format!("save.ini unreadable: {e}")),
            },
        }
        if !dir.join("CLAK/scr.dat").is_file() {
            report.warnings.push("CLAK/scr.dat missing".into());
        }
        Ok(report)
    }
    pub fn save_preview(&self, folder: &str) -> Result<Option<Vec<u8>>> {
        let p = child(&self.root(), folder)?.join("savepic.tga");
        reject_links(&p)?;
        if p.exists() {
            Ok(Some(fs::read(p)?))
        } else {
            Ok(None)
        }
    }
    pub fn backup_preview(&self, file_name: &str) -> Result<Option<Vec<u8>>> {
        let p = child(&self.backups, file_name)?;
        reject_links(&p)?;
        if !p.is_file() {
            return Ok(None);
        }
        let mut zip = ZipArchive::new(File::open(p)?)?;
        let result = match zip.by_name("savepic.tga") {
            Ok(mut e) => Ok(Some(read_limited(&mut e, LIMIT)?)),
            Err(zip::result::ZipError::FileNotFound) => Ok(None),
            Err(e) => Err(e.into()),
        };
        result
    }
    pub fn delete_save(&self, folder: &str) -> Result<()> {
        let p = child(&self.root(), folder)?;
        reject_tree(&p)?;
        if p.exists() {
            remove_dir_all_force(&p)?;
        }
        Ok(())
    }
    pub fn delete_backup(&self, name: &str) -> Result<()> {
        let p = child(&self.backups, name)?;
        reject_links(&p)?;
        if p.exists() {
            fs::remove_file(p)?;
        }
        Ok(())
    }
}
fn read_limited(reader: &mut impl Read, limit: u64) -> Result<Vec<u8>> {
    let mut b = vec![];
    reader.take(limit.saturating_add(1)).read_to_end(&mut b)?;
    if b.len() as u64 > limit {
        return fail("entry exceeds size limit");
    }
    Ok(b)
}
fn collect_files(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, String)>) -> Result<()> {
    reject_links(dir)?;
    for e in fs::read_dir(dir)? {
        let e = e?;
        let p = e.path();
        reject_links(&p)?;
        if e.file_type()?.is_dir() {
            collect_files(root, &p, out)?;
        } else if e.file_type()?.is_file() {
            let name = p
                .strip_prefix(root)?
                .to_str()
                .ok_or("non UTF-8 file name")?
                .replace('\\', "/");
            validate_entry(&name)?;
            out.push((p, name));
        } else {
            return fail("special files rejected");
        }
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(())
}
fn reject_tree(path: &Path) -> Result<()> {
    reject_links(path)?;
    if path.is_dir() {
        for e in fs::read_dir(path)? {
            reject_tree(&e?.path())?;
        }
    }
    Ok(())
}
fn validate_entry(name: &str) -> Result<PathBuf> {
    if name.contains('\\') || name.starts_with('/') || name.is_empty() {
        return fail("unsafe archive path");
    }
    let trim = name.strip_suffix('/').unwrap_or(name);
    let mut path = PathBuf::new();
    for part in trim.split('/') {
        if !simple_name(part) {
            return fail("unsafe archive component");
        }
        path.push(part);
    }
    Ok(path)
}
fn extract(archive: &Path, dest: &Path) -> Result<()> {
    let mut zip = ZipArchive::new(File::open(archive)?)?;
    if zip.len() > 100000 {
        return fail("too many entries");
    }
    let mut seen = HashSet::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut e = zip.by_index(i)?;
        let path = validate_entry(e.name())?;
        if !seen.insert(e.name().trim_end_matches('/').to_ascii_lowercase()) {
            return fail("duplicate archive path");
        }
        if let Some(mode) = e.unix_mode() {
            let ty = mode & 0o170000;
            if ty != 0 && ty != 0o100000 && ty != 0o040000 {
                return fail("archive symbolic/special link rejected");
            }
        }
        total = total.checked_add(e.size()).ok_or("archive size overflow")?;
        if total > LIMIT {
            return fail("archive exceeds size limit");
        }
        let target = dest.join(path);
        reject_links(&target)?;
        if e.is_dir() {
            fs::create_dir_all(target)?;
        } else {
            if let Some(p) = target.parent() {
                fs::create_dir_all(p)?;
            }
            let mut out = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(target)?;
            let max_read = e.size().saturating_add(1);
            let copied = std::io::copy(&mut e.by_ref().take(max_read), &mut out)?;
            if copied != e.size() {
                return fail("entry size mismatch");
            }
            out.sync_all()?;
        }
    }
    Ok(())
}
fn legacy(name: &str, m: &mut Manifest) {
    let Some(stem) = name.strip_suffix(".zip") else {
        return;
    };
    if !stem.to_ascii_lowercase().starts_with("backup_") {
        return;
    }
    let Some((prefix, time)) = stem.rsplit_once('_') else {
        return;
    };
    let Some((prefix, date)) = prefix.rsplit_once('_') else {
        return;
    };
    if date.len() != 8
        || time.len() != 6
        || !date.bytes().chain(time.bytes()).all(|b| b.is_ascii_digit())
    {
        return;
    }
    if m.orig_folder.is_empty() {
        m.orig_folder = prefix[7..].into();
    }
    if m.backup_time.is_empty() {
        m.backup_time = format!(
            "{}-{}-{} {}:{}:{}",
            &date[..4],
            &date[4..6],
            &date[6..],
            &time[..2],
            &time[2..4],
            &time[4..]
        );
    }
}

#[cfg(test)]
mod tests;
