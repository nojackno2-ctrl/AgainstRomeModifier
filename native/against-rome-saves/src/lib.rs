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
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
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
#[derive(Debug, Serialize, PartialEq)]
pub enum AiRepairResult {
    AlreadyRepaired,
    Changed { backup: String },
}
pub fn simple_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name != "."
        && name != ".."
        && !name.ends_with(['.', ' '])
        && !name.chars().any(|c| c < ' ' || "<>:\"/\\|?*".contains(c))
        && {
            let base = name.split('.').next().unwrap().to_ascii_uppercase();
            !matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                && !(base.len() == 4
                    && (base.starts_with("COM") || base.starts_with("LPT"))
                    && matches!(base.as_bytes()[3], b'1'..=b'9'))
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
        let backup = self.backup(folder, "", "")?;
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
                let entry = entry?;
                reject_links(&entry.path())?;
                if !entry.file_type()?.is_dir() {
                    continue;
                }
                let ini = entry.path().join("save.ini");
                reject_links(&ini)?;
                if !ini.is_file() {
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
            let entry = entry?;
            if entry
                .path()
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
            {
                reject_links(&entry.path())?;
                let info = self.backup_info(&entry.path());
                let parsed = info.is_ok();
                let mut manifest = info.unwrap_or_default();
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
    fn backup_info(&self, path: &Path) -> Result<Manifest> {
        let mut zip = ZipArchive::new(File::open(path)?)?;
        let mut m = match zip.by_name("manifest.json") {
            Ok(mut e) => {
                let b = read_limited(&mut e, 4 * 1024 * 1024)?;
                serde_json::from_slice::<Manifest>(b.strip_prefix(&[239, 187, 191]).unwrap_or(&b))?
            }
            Err(zip::result::ZipError::FileNotFound) => Manifest::default(),
            Err(e) => return Err(e.into()),
        };
        if m.title.is_empty() || m.level.is_empty() {
            if let Ok(mut e) = zip.by_name("save.ini") {
                let b = read_limited(&mut e, LIMIT)?;
                let (t, l) = read_save_info(&b)?;
                if m.title.is_empty() {
                    m.title = t;
                }
                if m.level.is_empty() {
                    m.level = l;
                }
            }
        }
        Ok(m)
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
            fs::hard_link(&temp, &dest)?;
            fs::remove_file(&temp)?;
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
                    fs::remove_dir_all(&dest)?;
                }
                if moved_old {
                    fs::rename(&old, &dest)?;
                }
                Ok(())
            })();
            let _ = fs::remove_dir_all(&temp);
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
            if let Err(e) = fs::remove_dir_all(&old) {
                report
                    .cleanup_warnings
                    .push(format!("{}: {e}", old.display()));
            }
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
            fs::remove_dir_all(p)?;
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
    reader.take(limit + 1).read_to_end(&mut b)?;
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
            let copied = std::io::copy(&mut e.by_ref().take(LIMIT + 1), &mut out)?;
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
