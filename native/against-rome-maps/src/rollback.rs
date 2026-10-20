//! Atomic file and directory rollback scope (`FileRollbackScope`) and safe file writer (`SafeFileWriter`).
//! Backs up files before mutation or deletion, and automatically restores them
//! if the scope is dropped without calling `commit()`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Entry tracking original file state for rollback.
#[derive(Clone, Debug)]
struct RollbackFileEntry {
    path: PathBuf,
    backup_path: Option<PathBuf>,
    existed: bool,
}

/// Transactional scope tracking file and directory modifications.
/// If `commit()` is not invoked before `drop`, all tracked changes are reverted.
#[derive(Debug)]
pub struct FileRollbackScope {
    root_path: PathBuf,
    entries: HashMap<PathBuf, RollbackFileEntry>,
    directories: HashMap<PathBuf, PathBuf>,
    committed: bool,
    restored: bool,
    directory_restore_failed: bool,
    restore_failed: bool,
}

impl FileRollbackScope {
    /// Creates a new rollback scope with an isolated temporary backup directory.
    pub fn new() -> Result<Self, String> {
        let temp_dir = std::env::temp_dir();
        let root_path = temp_dir.join(format!("AgainstRomeModifierRollback_{}", Uuid::new_v4().simple()));
        fs::create_dir_all(&root_path)
            .map_err(|e| format!("建立回滾暫存目錄失敗 ({}): {e}", root_path.display()))?;

        Ok(Self {
            root_path,
            entries: HashMap::new(),
            directories: HashMap::new(),
            committed: false,
            restored: false,
            directory_restore_failed: false,
            restore_failed: false,
        })
    }

    /// Records a file path before mutation or deletion.
    /// If the file currently exists, a full copy is made in the rollback root.
    /// If it does not exist, it is recorded so that any file created later at this path can be removed on rollback.
    pub fn track_file(&mut self, path: &Path) -> Result<(), String> {
        if has_reparse_point_component(path) {
            return Err(format!("無法追蹤符號連結或重解析點路徑: {}", path.display()));
        }

        let canonical_or_full = normalize_path(path);
        if self.entries.contains_key(&canonical_or_full) {
            return Ok(());
        }

        let existed = canonical_or_full.is_file();
        let backup_path = if existed {
            let backup = self.root_path.join(format!("{}.bak", Uuid::new_v4().simple()));
            fs::copy(&canonical_or_full, &backup).map_err(|e| {
                format!(
                    "備份檔案至回滾暫存失敗 ({} -> {}): {e}",
                    canonical_or_full.display(),
                    backup.display()
                )
            })?;
            Some(backup)
        } else {
            None
        };

        self.entries.insert(
            canonical_or_full.clone(),
            RollbackFileEntry {
                path: canonical_or_full,
                backup_path,
                existed,
            },
        );
        Ok(())
    }

    /// Snapshots an existing directory, including unknown files and subdirectories.
    pub fn track_directory(&mut self, path: &Path) -> Result<(), String> {
        if has_reparse_point_component(path) {
            return Err(format!("無法快照連結目錄: {}", path.display()));
        }
        let canonical = normalize_path(path);
        if self.directories.contains_key(&canonical) {
            return Ok(());
        }
        if !canonical.is_dir() {
            return Err(format!("欲備份之目錄不存在: {}", canonical.display()));
        }

        let backup = self.root_path.join(format!("dir_{}", Uuid::new_v4().simple()));
        copy_directory_recursive(&canonical, &backup)?;
        self.directories.insert(canonical, backup);
        Ok(())
    }

    /// Restores a single tracked directory without ending the surrounding transaction.
    pub fn restore_directory(&mut self, path: &Path) -> Result<(), String> {
        if has_reparse_point_component(path) {
            return Err(format!("無法還原至連結目錄: {}", path.display()));
        }
        let canonical = normalize_path(path);
        let backup = self
            .directories
            .get(&canonical)
            .ok_or_else(|| format!("目錄未納入回滾追蹤: {}", canonical.display()))?;

        let backup = backup.clone();
        if canonical.exists() {
            clear_readonly_recursive(&canonical);
            if is_reparse_point_or_link(&canonical) {
                fs::remove_dir(&canonical)
                    .map_err(|e| format!("清除連結目錄失敗 ({}): {e}", canonical.display()))?;
            } else {
                fs::remove_dir_all(&canonical)
                    .map_err(|e| format!("清除目錄失敗 ({}): {e}", canonical.display()))?;
            }
        }
        copy_directory_recursive(&backup, &canonical)?;
        Ok(())
    }

    /// Marks the transaction as successfully committed.
    /// Subsequent drop will not revert any changes.
    pub fn commit(&mut self) {
        self.committed = true;
        let _ = fs::remove_dir_all(&self.root_path);
    }

    /// Explicitly restores all tracked files and directories to their pre-transaction state.
    pub fn restore_all(&mut self) -> Result<(), String> {
        if self.restored || self.committed {
            return Ok(());
        }

        let mut errors = Vec::new();

        // Restore files
        for entry in self.entries.values() {
            if entry.existed {
                if let Some(backup) = &entry.backup_path {
                    if let Some(parent) = entry.path.parent() {
                        if has_reparse_point_component(parent) {
                            errors.push(format!("還原檔案失敗: 父目錄包含符號連結或重解析點 ({})", parent.display()));
                            continue;
                        }
                        let _ = fs::create_dir_all(parent);
                    }
                    if is_reparse_point_or_link(&entry.path) {
                        let _ = fs::remove_file(&entry.path);
                    } else {
                        clear_readonly(&entry.path);
                    }

                    let mut restored_ok = false;
                    for attempt in 0..RENAME_RETRIES {
                        match fs::copy(backup, &entry.path) {
                            Ok(_) => {
                                restored_ok = true;
                                break;
                            }
                            Err(_) if attempt + 1 < RENAME_RETRIES => {
                                clear_readonly(&entry.path);
                                std::thread::sleep(std::time::Duration::from_millis(RENAME_RETRY_DELAY_MS));
                            }
                            Err(e) => {
                                errors.push(format!("還原檔案失敗 ({}): {e}", entry.path.display()));
                            }
                        }
                    }
                    if !restored_ok && !errors.iter().any(|s| s.contains(&entry.path.to_string_lossy().to_string())) {
                        errors.push(format!("還原檔案多次重試失敗 ({})", entry.path.display()));
                    }
                }
            } else if entry.path.exists() {
                if is_reparse_point_or_link(&entry.path) {
                    let _ = fs::remove_file(&entry.path);
                } else {
                    clear_readonly(&entry.path);
                    if let Err(e) = fs::remove_file(&entry.path) {
                        errors.push(format!("移除新建檔案失敗 ({}): {e}", entry.path.display()));
                    }
                }
            }
        }

        // Restore directories
        for (original, backup) in &self.directories {
            let result = (|| -> Result<(), String> {
                if original.exists() {
                    clear_readonly_recursive(original);
                    if is_reparse_point_or_link(original) {
                        fs::remove_dir(original).map_err(|e| format!("清除連結目錄失敗: {e}"))?;
                    } else {
                        fs::remove_dir_all(original)
                            .map_err(|e| format!("清除目錄失敗: {e}"))?;
                    }
                }
                copy_directory_recursive(backup, original)
            })();
            if let Err(e) = result {
                self.directory_restore_failed = true;
                errors.push(format!(
                    "還原目錄失敗 ({}): {e}。備份保留於 {}",
                    original.display(),
                    self.root_path.display()
                ));
            }
        }

        self.restored = true;
        if !errors.is_empty() {
            self.restore_failed = true;
        }
        // Keep the snapshot when any restore failed so the user can recover manually.
        if !self.directory_restore_failed && !self.restore_failed {
            let _ = fs::remove_dir_all(&self.root_path);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "回滾部分失敗 (備份快照已保留於 {}):\n{}",
                self.root_path.display(),
                errors.join("\n")
            ))
        }
    }

    pub fn is_committed(&self) -> bool {
        self.committed
    }

    pub fn is_restored(&self) -> bool {
        self.restored
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    pub fn tracked_file_count(&self) -> usize {
        self.entries.len()
    }

    pub fn tracked_directory_count(&self) -> usize {
        self.directories.len()
    }

    pub fn has_tracked_file(&self, path: &Path) -> bool {
        self.entries.contains_key(&normalize_path(path))
    }

    pub fn has_tracked_directory(&self, path: &Path) -> bool {
        self.directories.contains_key(&normalize_path(path))
    }
}

impl Drop for FileRollbackScope {
    fn drop(&mut self) {
        if !self.committed && !self.restored {
            let _ = self.restore_all();
        }
        if !self.directory_restore_failed && !self.restore_failed && (self.committed || self.restored) {
            let _ = fs::remove_dir_all(&self.root_path);
        }
    }
}

const RENAME_RETRIES: u32 = 3;
const RENAME_RETRY_DELAY_MS: u64 = 100;

/// Transactional file writing helpers (`SafeFileWriter`).
pub struct SafeFileWriter;

impl SafeFileWriter {
    /// Writes content to destination atomically via a temporary file,
    /// optionally registering with a `FileRollbackScope`.
    /// If content already matches byte-for-byte, writing is skipped.
    pub fn write_all_bytes(
        dest: &Path,
        bytes: &[u8],
        mut rollback: Option<&mut FileRollbackScope>,
    ) -> Result<(), String> {
        if has_reparse_point_component(dest) {
            return Err(format!("目標路徑包含符號連結或重解析點: {}", dest.display()));
        }

        if let Some(parent) = dest.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| format!("建立目標目錄失敗: {e}"))?;
            }
        }

        // Check if content already matches
        if dest.is_file() {
            if let Ok(existing) = fs::read(dest) {
                if existing == bytes {
                    return Ok(());
                }
            }
        }

        if let Some(rb) = rollback.as_deref_mut() {
            rb.track_file(dest)?;
        }

        let parent = dest.parent().unwrap_or_else(|| Path::new("."));
        let file_name = dest.file_name().and_then(|s| s.to_str()).unwrap_or("file");
        let temp_file = parent.join(format!("{}.{}.tmp", file_name, Uuid::new_v4().simple()));

        fs::write(&temp_file, bytes).map_err(|e| {
            let _ = fs::remove_file(&temp_file);
            format!("寫入暫存檔案失敗 ({}): {e}", temp_file.display())
        })?;

        // Rename replaces an existing destination atomically (MoveFileEx REPLACE_EXISTING on Windows),
        // so the original is never deleted first; retry for transient sharing violations.
        clear_readonly(dest);
        let mut last_err = None;
        for attempt in 0..RENAME_RETRIES {
            match fs::rename(&temp_file, dest) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    last_err = Some(e);
                    if attempt + 1 < RENAME_RETRIES {
                        std::thread::sleep(std::time::Duration::from_millis(RENAME_RETRY_DELAY_MS));
                    }
                }
            }
        }
        let _ = fs::remove_file(&temp_file);
        Err(format!(
            "替換目標檔案失敗，檔案可能被佔用或權限不足 ({}): {}",
            dest.display(),
            last_err.map(|e| e.to_string()).unwrap_or_default()
        ))
    }

    /// Deletes a file safely, registering with `FileRollbackScope` first if provided.
    pub fn delete_file(
        path: &Path,
        mut rollback: Option<&mut FileRollbackScope>,
    ) -> Result<(), String> {
        if has_reparse_point_component(path) {
            return Err(format!("目標路徑包含符號連結或重解析點: {}", path.display()));
        }

        if !path.exists() {
            return Ok(());
        }

        if let Some(rb) = rollback.as_deref_mut() {
            rb.track_file(path)?;
        }

        clear_readonly(path);
        fs::remove_file(path).map_err(|e| format!("刪除檔案失敗 ({}): {e}", path.display()))?;
        Ok(())
    }
}

pub fn is_reparse_point_or_link(path: &Path) -> bool {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(_) => return false,
    };
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if (meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
            return true;
        }
    }
    false
}

pub fn has_reparse_point_component(path: &Path) -> bool {
    if is_reparse_point_or_link(path) {
        return true;
    }
    let mut current = PathBuf::new();
    for comp in path.components() {
        current.push(comp);
        if is_reparse_point_or_link(&current) {
            return true;
        }
    }
    false
}

fn clear_readonly(path: &Path) {
    if is_reparse_point_or_link(path) {
        return;
    }
    if let Ok(meta) = fs::symlink_metadata(path) {
        let mut perms = meta.permissions();
        if perms.readonly() {
            #[allow(clippy::permissions_set_readonly_false)]
            perms.set_readonly(false);
            let _ = fs::set_permissions(path, perms);
        }
    }
}

fn clear_readonly_recursive(dir: &Path) {
    if is_reparse_point_or_link(dir) {
        return;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if is_reparse_point_or_link(&path) {
                continue;
            }
            if path.is_dir() {
                clear_readonly_recursive(&path);
            } else {
                clear_readonly(&path);
            }
        }
    }
}

fn copy_directory_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    if is_reparse_point_or_link(src) {
        return Err(format!("無法快照連結目錄: {}", src.display()));
    }
    if is_reparse_point_or_link(dst) {
        return Err(format!("目標路徑為符號連結或重解析點: {}", dst.display()));
    }
    if !dst.exists() {
        fs::create_dir_all(dst).map_err(|e| format!("建立目錄失敗: {e}"))?;
    }
    let entries = fs::read_dir(src).map_err(|e| format!("讀取來源目錄失敗: {e}"))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if is_reparse_point_or_link(&path) {
            return Err(format!("無法快照連結檔案或目錄: {}", path.display()));
        }
        if is_reparse_point_or_link(&target) {
            return Err(format!("目標路徑為符號連結或重解析點: {}", target.display()));
        }
        if path.is_dir() {
            copy_directory_recursive(&path, &target)?;
        } else {
            clear_readonly(&target);
            fs::copy(&path, &target).map_err(|e| format!("複製檔案失敗 ({}): {e}", path.display()))?;
        }
    }
    Ok(())
}

pub(crate) fn strip_unc_prefix(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let p = if let Ok(canon) = path.canonicalize() {
        canon
    } else if path.is_absolute() {
        path.to_path_buf()
    } else if let Ok(cwd) = std::env::current_dir() {
        cwd.join(path)
    } else {
        path.to_path_buf()
    };
    strip_unc_prefix(p)
}

