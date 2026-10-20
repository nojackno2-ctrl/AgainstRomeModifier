//! SaveBackupService catalog view, ported from src.Core/Core/Services/SaveBackupService.cs.
//! Backup/restore/repair mechanics live in `against-rome-saves`; this adds the catalog fields the C# service
//! exposed (last-write times, normalized backup time, creation-time fallback) for the CLI and GUI.
use against_rome_saves::{simple_name, AiRepairResult, SaveService};
use chrono::{DateTime, FixedOffset, Local, NaiveDateTime};
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Debug, Clone, PartialEq)]
pub struct GameSaveInfo {
    pub folder: String,
    pub title: String,
    pub level: String,
    pub last_write_time: String,
    pub parsed: bool,
}
#[derive(Debug, Clone, PartialEq)]
pub struct SaveBackupInfo {
    pub file_name: String,
    pub title: String,
    pub level: String,
    pub backup_time: String,
    pub orig_folder: String,
    pub last_write_time: String,
    pub parsed: bool,
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SaveBackupCatalog {
    pub saves: Vec<GameSaveInfo>,
    pub backups: Vec<SaveBackupInfo>,
}

fn io_err(e: Box<dyn std::error::Error + Send + Sync>) -> io::Error {
    match e.downcast::<io::Error>() {
        Ok(e) => *e,
        Err(e) => io::Error::new(io::ErrorKind::InvalidData, e.to_string()),
    }
}

/// .NET `DateTime` JSON shape for a local time: `yyyy-MM-ddTHH:mm:ss.fffffff+hh:mm`.
pub fn dotnet_local_iso(time: SystemTime) -> String {
    let t: DateTime<Local> = time.into();
    format!(
        "{}.{:07}{}",
        t.format("%Y-%m-%dT%H:%M:%S"),
        t.timestamp_subsec_nanos() / 100,
        t.format("%:z")
    )
}

/// C# SaveBackupManifestCodec.FormatTime: round-trip parse (converted to local time), else the value unchanged.
pub fn format_time(value: &str) -> String {
    if let Ok(t) = DateTime::<FixedOffset>::parse_from_rfc3339(value) {
        return t
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
    }
    if let Ok(t) = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S") {
        return t.format("%Y-%m-%d %H:%M:%S").to_string();
    }
    value.to_string()
}

pub struct SaveBackupService {
    game: PathBuf,
    backup_dir: PathBuf,
    inner: SaveService,
}

impl SaveBackupService {
    /// `backup_dir` is `<game>/SAVE_BACKUP` for the CLI.
    pub fn new(game: &Path, backup_dir: &Path) -> io::Result<Self> {
        Ok(Self {
            game: game.to_path_buf(),
            backup_dir: backup_dir.to_path_buf(),
            inner: SaveService::new(game, backup_dir).map_err(io_err)?,
        })
    }
    pub fn is_simple_name(value: &str) -> bool {
        simple_name(value)
    }

    pub fn scan(&self) -> io::Result<SaveBackupCatalog> {
        let raw = self.inner.scan().map_err(io_err)?;
        let mut catalog = SaveBackupCatalog::default();
        for s in raw.saves {
            let modified = fs::metadata(self.game.join("SAVE").join(&s.folder))
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            catalog.saves.push(GameSaveInfo {
                folder: s.folder,
                title: s.title,
                level: s.level,
                last_write_time: dotnet_local_iso(modified),
                parsed: s.parsed,
            });
        }
        for b in raw.backups {
            let meta = fs::metadata(self.backup_dir.join(&b.file_name)).ok();
            let modified = meta
                .as_ref()
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let mut backup_time = format_time(&b.manifest.backup_time);
            if backup_time.is_empty() {
                let created = meta
                    .as_ref()
                    .and_then(|m| m.created().ok())
                    .unwrap_or(modified);
                backup_time = DateTime::<Local>::from(created)
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string();
            }
            catalog.backups.push(SaveBackupInfo {
                file_name: b.file_name,
                title: b.manifest.title,
                level: b.manifest.level,
                backup_time,
                orig_folder: b.manifest.orig_folder,
                last_write_time: dotnet_local_iso(modified),
                parsed: b.parsed,
            });
        }
        Ok(catalog)
    }
    pub fn create_backup(&self, folder: &str, title: &str, level: &str) -> io::Result<String> {
        self.inner.backup(folder, title, level).map_err(io_err)
    }
    /// Returns the saves-crate result; unlike the C# CLI this also writes a full ZIP backup before changing scr.dat.
    pub fn repair_endless_ai(&self, folder: &str) -> io::Result<AiRepairResult> {
        self.inner.repair_endless_ai(folder).map_err(io_err)
    }
    pub fn restore_backup(&self, file_name: &str, folder: &str) -> io::Result<Vec<String>> {
        Ok(self
            .inner
            .restore(file_name, folder)
            .map_err(io_err)?
            .cleanup_warnings)
    }
    pub fn delete_save(&self, folder: &str) -> io::Result<()> {
        self.inner.delete_save(folder).map_err(io_err)
    }
    pub fn delete_backup(&self, name: &str) -> io::Result<()> {
        self.inner.delete_backup(name).map_err(io_err)
    }
    pub fn read_save_preview(&self, folder: &str) -> io::Result<Option<Vec<u8>>> {
        self.inner.save_preview(folder).map_err(io_err)
    }
    pub fn read_backup_preview(&self, name: &str) -> io::Result<Option<Vec<u8>>> {
        self.inner.backup_preview(name).map_err(io_err)
    }
    /// Decode for Rust UI callers; missing and unsupported thumbnails have no image.
    /// I/O and archive errors remain errors rather than masquerading as a missing preview.
    pub fn decode_save_preview(&self, folder: &str) -> io::Result<Option<crate::tga::TgaImage>> {
        Ok(self
            .read_save_preview(folder)?
            .as_deref()
            .and_then(crate::tga::decode))
    }
    pub fn decode_backup_preview(&self, name: &str) -> io::Result<Option<crate::tga::TgaImage>> {
        Ok(self
            .read_backup_preview(name)?
            .as_deref()
            .and_then(crate::tga::decode))
    }
}
