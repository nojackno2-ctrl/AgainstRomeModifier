//! Whole-map transactional save API (`MapSaveTransaction`, `MapSaveReport`, `MapSaveError`).
//!
//! Provides an atomic, staged, failure-isolated save coordinator for Against Rome maps:
//! 1. **Staging**: Stages arbitrary map files in memory or from disk (terrain, ambience, objects, scenario, SDL, caches, markers).
//! 2. **Preflight Validation**: Validates inputs, relative paths, custom-map boundary guards, collision checks,
//!    and domain preflights without touching any destination file.
//! 3. **Rollback Snapshotting**: Snapshots all destination targets in an isolated backup workspace (`FileRollbackScope`).
//! 4. **Atomic Replacement**: Replaces destination files via atomic file renames with retry logic.
//! 5. **Failure Isolation & Rollback**: On any injected failure or I/O error at any phase, all modified files
//!    are cleanly restored byte-for-byte to their exact original bytes, newly created files are deleted,
//!    and temporary files are purged.
//! 6. **Commit & Verification**: Verifies file integrity, commits the transaction, and returns a comprehensive report.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

use crate::custom_map::{parse_endl_slot, CustomMapAccess, CustomMapEntry, CustomMapManifest};
use crate::rollback::{has_reparse_point_component, is_reparse_point_or_link, strip_unc_prefix, FileRollbackScope};
use crate::text_docs::{BodenIniDocument, BodenTexturesDocument, PutTextDocument};
use against_rome_scenario::model::ScenarioDocument;

/// Verifies that no path component of `rel_path` within `target_dir` is a symlink,
/// directory junction, or reparse point escaping the map root or linking outside.
pub fn verify_path_components_safe(target_dir: &Path, rel_path: &Path) -> Result<(), String> {
    let mut current = target_dir.to_path_buf();
    let canonical_root = target_dir.canonicalize().ok().map(strip_unc_prefix);

    for comp in rel_path.components() {
        match comp {
            std::path::Component::Normal(c) => {
                current.push(c);
                if is_reparse_point_or_link(&current) {
                    if let Ok(target_canon) = current.canonicalize() {
                        let target_clean = strip_unc_prefix(target_canon);
                        if let Some(root) = &canonical_root {
                            if !target_clean.starts_with(root) {
                                return Err(format!(
                                    "路徑安全違規: 目標路徑元件 '{}' 為符號連結或重解析點 (Junction/Symlink)，指向地圖目錄外部 ({})，禁止脫離地圖目錄寫入。",
                                    current.display(),
                                    target_clean.display()
                                ));
                            }
                        }
                    }
                    return Err(format!(
                        "路徑安全違規: 目標路徑元件 '{}' 為符號連結或重解析點 (Junction/Symlink)，禁止於地圖目錄內使用連結檔案或目錄。",
                        current.display()
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Normalizes a relative path within a map directory, rejecting traversal (`..`) or absolute paths.
pub fn normalize_relative_path(path: &Path) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() {
        return Err("相對檔案路徑不可為空。".into());
    }
    if path.is_absolute() {
        return Err(format!("檔案路徑必須是相對路徑，不可為絕對路徑: {}", path.display()));
    }

    let mut components = Vec::new();
    for comp in path.components() {
        match comp {
            std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                return Err(format!("檔案路徑不可包含根目錄或磁碟前綴: {}", path.display()));
            }
            std::path::Component::ParentDir => {
                return Err(format!("檔案路徑禁止使用 '..' 脫離地圖目錄: {}", path.display()));
            }
            std::path::Component::CurDir => continue,
            std::path::Component::Normal(c) => {
                let s = c.to_string_lossy();
                if s.chars().any(|ch| "/\\:*?\"<>|\0".contains(ch)) {
                    return Err(format!("檔案路徑包含非法字元: {}", path.display()));
                }
                components.push(s.to_string());
            }
        }
    }

    if components.is_empty() {
        return Err("相對檔案路徑不可僅為當前目錄 '.'。".into());
    }

    let mut normalized = PathBuf::new();
    for c in components {
        normalized.push(c);
    }
    Ok(normalized)
}

/// Identifies whether the specified path belongs to an identifiable Against Rome game installation directory.
/// Returns the canonical or resolved root directory of the game if identified.
pub fn detect_game_root(path: &Path, explicit_game_path: Option<&Path>) -> Option<PathBuf> {
    if let Some(gp) = explicit_game_path {
        if !gp.as_os_str().is_empty() {
            let clean_gp = strip_unc_prefix(gp.canonicalize().unwrap_or_else(|_| gp.to_path_buf()));
            let clean_path = strip_unc_prefix(path.canonicalize().unwrap_or_else(|_| path.to_path_buf()));
            if clean_path.starts_with(&clean_gp) {
                return Some(clean_gp);
            }
        }
    }

    let resolved = strip_unc_prefix(path.canonicalize().unwrap_or_else(|_| {
        let cur = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(path)
        };
        let mut components = Vec::new();
        for comp in cur.components() {
            match comp {
                std::path::Component::ParentDir => {
                    components.pop();
                }
                std::path::Component::CurDir => {}
                c => components.push(c.as_os_str().to_os_string()),
            }
        }
        let mut p = PathBuf::new();
        for c in components {
            p.push(c);
        }
        p
    }));

    for ancestor in resolved.ancestors() {
        if ancestor.as_os_str().is_empty() {
            continue;
        }

        // Signature 1: Key executable or distinctive game archive present
        let has_exe = ancestor.join("Against_Rome.exe").is_file()
            || ancestor.join("Against_Rome.EXE").is_file()
            || ancestor.join("against_rome.exe").is_file()
            || ancestor.join("Against_Rome.icd").is_file()
            || resolved.components().any(|c| {
                let s = c.as_os_str().to_string_lossy();
                s.eq_ignore_ascii_case("Against_Rome.exe") || s.eq_ignore_ascii_case("Against_Rome.icd")
            });
        let has_floortex = ancestor.join("floortex.dat").is_file() && ancestor.join("MAPS").is_dir();
        if has_exe || has_floortex {
            return Some(ancestor.to_path_buf());
        }

        // Signature 2: Directory named "Against Rome" (case-insensitive)
        if let Some(name) = ancestor.file_name().and_then(|s| s.to_str()) {
            if name.eq_ignore_ascii_case("Against Rome") {
                let parent_is_pf = ancestor
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|s| s.to_str())
                    .map_or(false, |p| {
                        p.eq_ignore_ascii_case("Program Files")
                            || p.eq_ignore_ascii_case("Program Files (x86)")
                    });
                let has_maps = ancestor.join("MAPS").is_dir()
                    || resolved.components().any(|c| c.as_os_str().to_string_lossy().eq_ignore_ascii_case("MAPS"));
                if parent_is_pf || has_maps || has_exe || has_floortex {
                    return Some(ancestor.to_path_buf());
                }
            }
        }
    }

    None
}

/// Validates the tiered path policy for whole-map save operations:
/// 1. External standalone map projects can be saved anywhere without naming or marker constraints.
/// 2. If the map or target resides inside an identifiable game installation root:
///    - Must be within the `MAPS` subdirectory. Target cannot be game root, SYSTEM, DATA, or other non-map files.
///    - Within `MAPS`, retail maps (`ENDL_000..=004`, `TUTOR_*`, `CAMP_*`, etc.) are strictly forbidden.
///    - Only custom maps (`ENDL_005..=999`) backed by `.arm_custom_map` (on disk or staged in transaction) are allowed.
pub fn validate_path_policy(
    target_dir: &Path,
    game_path: Option<&Path>,
    staged_marker: bool,
) -> Result<(), String> {
    let Some(game_root) = detect_game_root(target_dir, game_path) else {
        // Tier 1: External independent map project. Allowed freely.
        return Ok(());
    };

    // Tier 2: Inside an identifiable Against Rome game installation directory.
    let clean_target = strip_unc_prefix(target_dir.canonicalize().unwrap_or_else(|_| target_dir.to_path_buf()));
    let clean_root = strip_unc_prefix(game_root.canonicalize().unwrap_or_else(|_| game_root));

    let Ok(rel_to_root) = clean_target.strip_prefix(&clean_root) else {
        return Err(format!(
            "安全拒絕：目標路徑位於遊戲安裝目錄中，無法解析相對路徑: {}",
            target_dir.display()
        ));
    };

    let mut comps = rel_to_root.components();
    let first = comps.next();
    let Some(std::path::Component::Normal(first_name)) = first else {
        return Err(format!(
            "安全拒絕：禁止將地圖直接儲存於遊戲安裝根目錄: {}",
            target_dir.display()
        ));
    };

    let first_str = first_name.to_string_lossy();
    if !first_str.eq_ignore_ascii_case("MAPS") {
        return Err(format!(
            "安全拒絕：禁止寫入遊戲安裝目錄非 MAPS 自製地圖的檔案/目錄 ({}): {}",
            first_str,
            target_dir.display()
        ));
    }

    let second = comps.next();
    let Some(std::path::Component::Normal(map_folder)) = second else {
        return Err(format!(
            "安全拒絕：目標路徑為遊戲 MAPS 根目錄，並非具體地圖目錄: {}",
            target_dir.display()
        ));
    };

    let folder_name = map_folder.to_string_lossy();
    let Some(slot) = parse_endl_slot(&folder_name) else {
        return Err(format!(
            "安全拒絕：遊戲 MAPS 目錄下僅允許自製地圖 (ENDL_005..999)，拒絕修改零售地圖或未授權目錄: {}",
            folder_name
        ));
    };

    if slot < 5 {
        return Err(format!(
            "安全拒絕：遊戲 MAPS 目錄下拒絕修改零售無盡地圖 (ENDL_000..ENDL_004): {}",
            folder_name
        ));
    }

    if slot > 999 {
        return Err(format!(
            "安全拒絕：自製地圖槽位超出允許範圍 (5..=999): {}",
            slot
        ));
    }

    let has_disk_marker = clean_target.join(CustomMapManifest::MARKER_FILE_NAME).is_file()
        || clean_root.join("MAPS").join(&*folder_name).join(CustomMapManifest::MARKER_FILE_NAME).is_file();

    if !has_disk_marker && !staged_marker {
        return Err(format!(
            "安全拒絕：遊戲 MAPS 目錄下的自製地圖 '{}' 缺少 .arm_custom_map 標記，禁止無標記覆寫（新建自製地圖請在交易中暫存標記）。",
            folder_name
        ));
    }

    Ok(())
}

/// Validates target directory for custom map constraints (when explicitly required).
/// Prevents marker deadlock during new map creation if `staged_marker` is true.
pub fn validate_custom_map_target(
    target_dir: &Path,
    game_path: Option<&Path>,
    staged_marker: bool,
) -> Result<(), String> {
    let full_path = strip_unc_prefix(target_dir.canonicalize().unwrap_or_else(|_| target_dir.to_path_buf()));
    if let Some(gp) = game_path {
        let expected_maps = strip_unc_prefix(gp.join("MAPS").canonicalize().unwrap_or_else(|_| gp.join("MAPS")));
        let parent = full_path.parent().map(|p| strip_unc_prefix(p.canonicalize().unwrap_or_else(|_| p.to_path_buf())));
        if parent.as_ref() != Some(&expected_maps) {
            return Err("地圖資料夾不在選取的遊戲 MAPS 目錄，已取消儲存。".into());
        }
    }

    let Some(file_name) = full_path.file_name().and_then(|s| s.to_str()) else {
        return Err("無效的地圖目錄名稱。".into());
    };
    let Some(slot) = parse_endl_slot(file_name) else {
        return Err("編輯只允許 marker-backed 的 ENDL_005–999 自製地圖。".into());
    };
    if !(5..=999).contains(&slot) {
        return Err("編輯只允許 marker-backed 的 ENDL_005–999 自製地圖。".into());
    }

    let Some(parent) = full_path.parent() else {
        return Err("無法取得地圖目錄的父目錄。".into());
    };
    let Some(parent_name) = parent.file_name().and_then(|s| s.to_str()) else {
        return Err("無法取得父目錄名稱。".into());
    };
    if !parent_name.eq_ignore_ascii_case("MAPS") {
        return Err("編輯只允許 marker-backed 的 ENDL_005–999 自製地圖。".into());
    }

    let has_marker = staged_marker || CustomMapManifest::is_custom_map_directory(&full_path);
    if !has_marker {
        return Err("編輯只允許 marker-backed 的 ENDL_005–999 自製地圖。".into());
    }

    Ok(())
}

/// The action to be applied to a map file within the transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StagedAction {
    /// Write in-memory bytes to the destination file.
    WriteBytes(Vec<u8>),
    /// Copy content from an existing file on disk to the destination file.
    CopyFromPath(PathBuf),
    /// Delete the destination file if it exists.
    Delete,
}

/// A staged operation on a file relative to the target map directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StagedEntry {
    relative_path: PathBuf,
    action: StagedAction,
    tag: Option<String>,
}

impl StagedEntry {
    pub fn new(relative_path: PathBuf, action: StagedAction, tag: Option<String>) -> Self {
        Self {
            relative_path,
            action,
            tag,
        }
    }

    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    pub fn action(&self) -> &StagedAction {
        &self.action
    }

    pub fn tag(&self) -> Option<&str> {
        self.tag.as_deref()
    }

    pub fn is_deletion(&self) -> bool {
        matches!(self.action, StagedAction::Delete)
    }

    pub fn byte_size(&self) -> Option<usize> {
        match &self.action {
            StagedAction::WriteBytes(b) => Some(b.len()),
            StagedAction::CopyFromPath(p) => fs::metadata(p).ok().map(|m| m.len() as usize),
            StagedAction::Delete => None,
        }
    }
}

/// Phase of the transaction lifecycle.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TransactionStage {
    PreflightValidation,
    Staging,
    RollbackTracking,
    FileReplacement,
    PostVerification,
    FinalCommit,
}

impl std::fmt::Display for TransactionStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PreflightValidation => write!(f, "PreflightValidation"),
            Self::Staging => write!(f, "Staging"),
            Self::RollbackTracking => write!(f, "RollbackTracking"),
            Self::FileReplacement => write!(f, "FileReplacement"),
            Self::PostVerification => write!(f, "PostVerification"),
            Self::FinalCommit => write!(f, "FinalCommit"),
        }
    }
}

/// Injection points for testing rollback guarantees under simulated failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransactionFailurePoint {
    /// Injects an error before validating inputs or running preflight checks.
    BeforeValidation,
    /// Injects an error after validation succeeds, before disk staging or rollback tracking starts.
    AfterValidation,
    /// Injects an error during staging of the n-th entry (0-indexed).
    DuringStaging(usize),
    /// Injects an error before any destination files are modified.
    BeforeAnyReplacement,
    /// Injects an error before replacing the n-th staged entry (0-indexed).
    BeforeReplacement(usize),
    /// Injects an error immediately after replacing the n-th staged entry (0-indexed).
    DuringReplacement(usize),
    /// Injects an error before replacing a specific relative path.
    BeforePath(PathBuf),
    /// Injects an error after replacing a specific relative path.
    AfterPath(PathBuf),
    /// Injects an error during post-replacement verification.
    DuringPostVerification,
    /// Injects an error after all files have been replaced, but before committing the rollback scope.
    BeforeFinalCommit,
}

/// Context passed to failure injectors and lifecycle hooks.
#[derive(Clone, Debug)]
pub struct TransactionStageContext<'a> {
    pub target_directory: &'a Path,
    pub stage: TransactionStage,
    pub step_index: usize,
    pub total_steps: usize,
    pub current_entry: Option<&'a StagedEntry>,
}

/// Dynamic failure injection callback for tests.
pub type FailureInjector = Arc<dyn Fn(&TransactionStageContext) -> Result<(), String> + Send + Sync>;

/// Preflight validator callback.
pub type PreflightValidator = Arc<dyn Fn(&Path, &[StagedEntry]) -> Result<(), String> + Send + Sync>;

/// Status of the rollback operation when a transaction fails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RollbackStatus {
    /// Rollback succeeded completely; all modified files were restored byte-for-byte,
    /// new files deleted, and temp files cleaned up.
    CleanlyRestored,
    /// Rollback was not needed (no disk modifications occurred before failure).
    NoModificationsToRevert,
    /// Rollback encountered errors (details included).
    PartialError(String),
}

/// Error returned when a whole-map save transaction fails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapSaveError {
    pub stage: TransactionStage,
    pub message: String,
    pub rollback_status: RollbackStatus,
}

impl std::fmt::Display for MapSaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "地圖存檔交易失敗 [{}] (回滾狀態: {:?}): {}",
            self.stage, self.rollback_status, self.message
        )
    }
}

impl std::error::Error for MapSaveError {}

/// Summary report of a successfully committed map save transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapSaveReport {
    pub target_directory: PathBuf,
    pub written_files: Vec<PathBuf>,
    pub deleted_files: Vec<PathBuf>,
    pub total_bytes_written: u64,
    pub duration_millis: u64,
}

/// Whole-map transactional save coordinator (`MapSaveTransaction`).
pub struct MapSaveTransaction {
    target_directory: PathBuf,
    staged_entries: Vec<StagedEntry>,
    require_custom_map: bool,
    game_path: Option<PathBuf>,
    create_if_missing: bool,
    verify_written_bytes: bool,
    preflight_validators: Vec<PreflightValidator>,
    injected_failure: Option<(TransactionFailurePoint, String)>,
    failure_injector: Option<FailureInjector>,
}

impl MapSaveTransaction {
    /// Creates a new whole-map transactional save coordinator for the target map directory.
    pub fn new(target_directory: impl Into<PathBuf>) -> Self {
        Self {
            target_directory: target_directory.into(),
            staged_entries: Vec::new(),
            require_custom_map: false,
            game_path: None,
            create_if_missing: false,
            verify_written_bytes: true,
            preflight_validators: Vec::new(),
            injected_failure: None,
            failure_injector: None,
        }
    }

    /// Creates a transaction configured to require an editable custom map (ENDL_005..999 with marker).
    pub fn for_custom_map(target_directory: impl Into<PathBuf>, game_path: Option<&Path>) -> Result<Self, String> {
        let dir = target_directory.into();
        CustomMapAccess::require_editable_directory(&dir, game_path)?;
        let mut tx = Self::new(dir).with_custom_map_guard(true);
        if let Some(gp) = game_path {
            tx = tx.with_game_path(gp);
        }
        Ok(tx)
    }

    /// Enforces or relaxes custom-map validation (`ENDL_005`..`999` with `.arm_custom_map`).
    pub fn with_custom_map_guard(mut self, require: bool) -> Self {
        self.require_custom_map = require;
        self
    }

    /// Sets the expected game root directory for map path validation.
    pub fn with_game_path(mut self, game_path: impl Into<PathBuf>) -> Self {
        self.game_path = Some(game_path.into());
        self
    }

    /// Sets whether the target directory may be created if it does not yet exist.
    pub fn with_create_if_missing(mut self, allow: bool) -> Self {
        self.create_if_missing = allow;
        self
    }

    /// Enables or disables post-write byte-for-byte readback verification.
    pub fn with_byte_verification(mut self, verify: bool) -> Self {
        self.verify_written_bytes = verify;
        self
    }

    /// Injects a simulated failure point for deterministic rollback testing.
    pub fn with_injected_failure(mut self, point: TransactionFailurePoint, message: impl Into<String>) -> Self {
        self.injected_failure = Some((point, message.into()));
        self
    }

    /// Injects a custom failure predicate callback for fine-grained testing.
    pub fn with_failure_injector<F>(mut self, injector: F) -> Self
    where
        F: Fn(&TransactionStageContext) -> Result<(), String> + Send + Sync + 'static,
    {
        self.failure_injector = Some(Arc::new(injector));
        self
    }

    /// Registers a custom preflight validator to inspect target directory and staged entries before any disk write.
    pub fn add_preflight_validator<F>(&mut self, validator: F) -> &mut Self
    where
        F: Fn(&Path, &[StagedEntry]) -> Result<(), String> + Send + Sync + 'static,
    {
        self.preflight_validators.push(Arc::new(validator));
        self
    }

    // ==========================================
    // Staging APIs
    // ==========================================

    /// Stages raw bytes for a file relative to the target map directory.
    pub fn stage_file(
        &mut self,
        relative_path: impl AsRef<Path>,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<&mut Self, String> {
        self.stage_file_tagged(relative_path, bytes, "")
    }

    /// Stages raw bytes with an informative tag for diagnostics.
    pub fn stage_file_tagged(
        &mut self,
        relative_path: impl AsRef<Path>,
        bytes: impl Into<Vec<u8>>,
        tag: impl Into<String>,
    ) -> Result<&mut Self, String> {
        let norm = normalize_relative_path(relative_path.as_ref())?;
        let tag_str = tag.into();
        let opt_tag = if tag_str.trim().is_empty() { None } else { Some(tag_str) };

        self.staged_entries.retain(|e| e.relative_path != norm);
        self.staged_entries.push(StagedEntry::new(norm, StagedAction::WriteBytes(bytes.into()), opt_tag));
        Ok(self)
    }

    /// Stages a file copy from an existing source file on disk.
    pub fn stage_file_from_disk(
        &mut self,
        relative_path: impl AsRef<Path>,
        source_path: impl AsRef<Path>,
    ) -> Result<&mut Self, String> {
        self.stage_file_from_disk_tagged(relative_path, source_path, "")
    }

    /// Stages a file copy from an existing source file on disk with a tag.
    pub fn stage_file_from_disk_tagged(
        &mut self,
        relative_path: impl AsRef<Path>,
        source_path: impl AsRef<Path>,
        tag: impl Into<String>,
    ) -> Result<&mut Self, String> {
        let norm = normalize_relative_path(relative_path.as_ref())?;
        let src = source_path.as_ref().to_path_buf();
        if !src.is_file() {
            return Err(format!("欲暫存複製的來源檔案不存在: {}", src.display()));
        }

        let tag_str = tag.into();
        let opt_tag = if tag_str.trim().is_empty() { None } else { Some(tag_str) };

        self.staged_entries.retain(|e| e.relative_path != norm);
        self.staged_entries.push(StagedEntry::new(norm, StagedAction::CopyFromPath(src), opt_tag));
        Ok(self)
    }

    /// Stages a file for safe deletion on commit.
    pub fn stage_delete(&mut self, relative_path: impl AsRef<Path>) -> Result<&mut Self, String> {
        self.stage_delete_tagged(relative_path, "")
    }

    /// Stages a file for safe deletion on commit with a tag.
    pub fn stage_delete_tagged(
        &mut self,
        relative_path: impl AsRef<Path>,
        tag: impl Into<String>,
    ) -> Result<&mut Self, String> {
        let norm = normalize_relative_path(relative_path.as_ref())?;
        let tag_str = tag.into();
        let opt_tag = if tag_str.trim().is_empty() { None } else { Some(tag_str) };

        self.staged_entries.retain(|e| e.relative_path != norm);
        self.staged_entries.push(StagedEntry::new(norm, StagedAction::Delete, opt_tag));
        Ok(self)
    }

    /// Stages custom map marker metadata (`.arm_custom_map`).
    pub fn stage_custom_map_marker(&mut self, entry: &CustomMapEntry) -> Result<&mut Self, String> {
        if !entry.is_valid() {
            return Err("無效的自製地圖標記資料。".into());
        }
        let json = serde_json::to_string_pretty(entry)
            .map_err(|e| format!("序列化自製地圖標記失敗: {e}"))?;
        self.stage_file_tagged(CustomMapManifest::MARKER_FILE_NAME, json.into_bytes(), "Custom Map Marker")
    }

    /// Stages scenario document as `arm_scenario.json`.
    pub fn stage_scenario_document(&mut self, scenario: &ScenarioDocument) -> Result<&mut Self, String> {
        let json = serde_json::to_string_pretty(scenario)
            .map_err(|e| format!("序列化劇本檔案失敗: {e}"))?;
        self.stage_file_tagged("arm_scenario.json", json.into_bytes(), "Scenario Document")
    }

    /// Stages a `PutTextDocument` to the given relative path.
    pub fn stage_put_document(
        &mut self,
        relative_path: impl AsRef<Path>,
        doc: &PutTextDocument,
    ) -> Result<&mut Self, String> {
        let bytes = doc.to_bytes()?;
        self.stage_file_tagged(relative_path, bytes, "Put Text Document")
    }

    /// Stages a `BodenIniDocument` to the given relative path.
    pub fn stage_boden_ini(
        &mut self,
        relative_path: impl AsRef<Path>,
        doc: &BodenIniDocument,
    ) -> Result<&mut Self, String> {
        let bytes = doc.to_bytes()?;
        self.stage_file_tagged(relative_path, bytes, "Boden INI Document")
    }

    /// Stages a `BodenTexturesDocument` to the given relative path.
    pub fn stage_boden_textures(
        &mut self,
        relative_path: impl AsRef<Path>,
        doc: &BodenTexturesDocument,
    ) -> Result<&mut Self, String> {
        let bytes = doc.to_bytes()?;
        self.stage_file_tagged(relative_path, bytes, "Boden Textures Document")
    }

    /// Stages the deletion of standard height-dependent runtime caches (`skydens.dat`, `visible.dat`, `cliprect.dat`, `shadows.dat`).
    pub fn stage_height_cache_invalidation(&mut self) -> Result<&mut Self, String> {
        for cache in crate::blank_builder::HEIGHT_DEPENDENT_CACHES {
            self.stage_delete_tagged(*cache, "Height Cache Invalidation")?;
        }
        Ok(self)
    }

    // ==========================================
    // Query & State APIs
    // ==========================================

    pub fn target_directory(&self) -> &Path {
        &self.target_directory
    }

    pub fn staged_entries(&self) -> &[StagedEntry] {
        &self.staged_entries
    }

    pub fn staged_count(&self) -> usize {
        self.staged_entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.staged_entries.is_empty()
    }

    pub fn has_staged(&self, relative_path: impl AsRef<Path>) -> bool {
        if let Ok(norm) = normalize_relative_path(relative_path.as_ref()) {
            self.staged_entries.iter().any(|e| e.relative_path == norm)
        } else {
            false
        }
    }

    pub fn find_entry(&self, relative_path: impl AsRef<Path>) -> Option<&StagedEntry> {
        let norm = normalize_relative_path(relative_path.as_ref()).ok()?;
        self.staged_entries.iter().find(|e| e.relative_path == norm)
    }

    pub fn unstage(&mut self, relative_path: impl AsRef<Path>) -> bool {
        if let Ok(norm) = normalize_relative_path(relative_path.as_ref()) {
            let before = self.staged_entries.len();
            self.staged_entries.retain(|e| e.relative_path != norm);
            self.staged_entries.len() < before
        } else {
            false
        }
    }

    pub fn clear(&mut self) {
        self.staged_entries.clear();
    }

    // ==========================================
    // Preflight Validation
    // ==========================================

    /// Performs preflight validation of all inputs, directory constraints, and staged entries
    /// without making any modifications on disk.
    pub fn validate(&self) -> Result<(), MapSaveError> {
        let stage = TransactionStage::PreflightValidation;

        self.check_failure_injection(&TransactionStageContext {
            target_directory: &self.target_directory,
            stage,
            step_index: 0,
            total_steps: self.staged_entries.len(),
            current_entry: None,
        }, TransactionFailurePoint::BeforeValidation)?;

        // Target directory validation
        if self.target_directory.as_os_str().is_empty() {
            return Err(MapSaveError {
                stage,
                message: "未指定目標地圖目錄路徑。".into(),
                rollback_status: RollbackStatus::NoModificationsToRevert,
            });
        }

        if self.target_directory.is_file() {
            return Err(MapSaveError {
                stage,
                message: format!("目標路徑為檔案而非地圖目錄: {}", self.target_directory.display()),
                rollback_status: RollbackStatus::NoModificationsToRevert,
            });
        }

        let staged_marker = self.has_staged(CustomMapManifest::MARKER_FILE_NAME)
            || self.staged_entries.iter().any(|e| {
                e.relative_path() == Path::new(CustomMapManifest::MARKER_FILE_NAME) && !e.is_deletion()
            });

        // 1. Enforce global tiered path safety policy
        if let Err(e) = validate_path_policy(&self.target_directory, self.game_path.as_deref(), staged_marker) {
            return Err(MapSaveError {
                stage,
                message: e,
                rollback_status: RollbackStatus::NoModificationsToRevert,
            });
        }

        // 2. Custom map guard when explicitly enabled
        if self.require_custom_map {
            if let Err(e) = validate_custom_map_target(&self.target_directory, self.game_path.as_deref(), staged_marker) {
                return Err(MapSaveError {
                    stage,
                    message: format!("目標目錄非合法的自製地圖: {e}"),
                    rollback_status: RollbackStatus::NoModificationsToRevert,
                });
            }
        }

        if !self.create_if_missing && !self.target_directory.is_dir() {
            return Err(MapSaveError {
                stage,
                message: format!("目標地圖目錄不存在: {}", self.target_directory.display()),
                rollback_status: RollbackStatus::NoModificationsToRevert,
            });
        }

        if is_reparse_point_or_link(&self.target_directory) {
            return Err(MapSaveError {
                stage,
                message: format!(
                    "目標地圖目錄為符號連結或重解析點 (Junction/Symlink)，拒絕作為存檔目錄: {}",
                    self.target_directory.display()
                ),
                rollback_status: RollbackStatus::NoModificationsToRevert,
            });
        }

        // Staged entries validation
        let mut seen_paths = HashSet::new();
        for (idx, entry) in self.staged_entries.iter().enumerate() {
            if !seen_paths.insert(&entry.relative_path) {
                return Err(MapSaveError {
                    stage,
                    message: format!("重複的暫存檔案路徑: {}", entry.relative_path.display()),
                    rollback_status: RollbackStatus::NoModificationsToRevert,
                });
            }

            // Verify path components within target_directory are not symlinks/junctions
            if let Err(msg) = verify_path_components_safe(&self.target_directory, &entry.relative_path) {
                return Err(MapSaveError {
                    stage,
                    message: msg,
                    rollback_status: RollbackStatus::NoModificationsToRevert,
                });
            }

            match &entry.action {
                StagedAction::CopyFromPath(src) => {
                    if !src.is_file() {
                        return Err(MapSaveError {
                            stage,
                            message: format!("暫存檔案的來源檔案不存在: {}", src.display()),
                            rollback_status: RollbackStatus::NoModificationsToRevert,
                        });
                    }
                }
                StagedAction::WriteBytes(_) | StagedAction::Delete => {}
            }

            // Check ancestor collisions (e.g. staging "DATA" as file while "DATA/objects.dat" is also staged)
            for other in &self.staged_entries[..idx] {
                if entry.relative_path.starts_with(&other.relative_path) && entry.relative_path != other.relative_path {
                    return Err(MapSaveError {
                        stage,
                        message: format!(
                            "路徑衝突: '{}' 是 '{}' 的父路徑，兩者不可同時作為檔案操作。",
                            other.relative_path.display(),
                            entry.relative_path.display()
                        ),
                        rollback_status: RollbackStatus::NoModificationsToRevert,
                    });
                }
                if other.relative_path.starts_with(&entry.relative_path) && entry.relative_path != other.relative_path {
                    return Err(MapSaveError {
                        stage,
                        message: format!(
                            "路徑衝突: '{}' 是 '{}' 的父路徑，兩者不可同時作為檔案操作。",
                            entry.relative_path.display(),
                            other.relative_path.display()
                        ),
                        rollback_status: RollbackStatus::NoModificationsToRevert,
                    });
                }
            }
        }

        // Run user-registered preflight validators
        for validator in &self.preflight_validators {
            if let Err(e) = validator(&self.target_directory, &self.staged_entries) {
                return Err(MapSaveError {
                    stage,
                    message: format!("前置驗證失敗: {e}"),
                    rollback_status: RollbackStatus::NoModificationsToRevert,
                });
            }
        }

        self.check_failure_injection(&TransactionStageContext {
            target_directory: &self.target_directory,
            stage,
            step_index: self.staged_entries.len(),
            total_steps: self.staged_entries.len(),
            current_entry: None,
        }, TransactionFailurePoint::AfterValidation)?;

        Ok(())
    }

    // ==========================================
    // Transaction Execution & Commit
    // ==========================================

    /// Commits all staged operations atomically with automatic full rollback on partial failure.
    pub fn commit(&mut self) -> Result<MapSaveReport, MapSaveError> {
        let start_time = Instant::now();

        // Phase 1: Preflight validation
        self.validate()?;

        // Phase 2: Failure point before any disk replacement
        self.check_failure_injection(&TransactionStageContext {
            target_directory: &self.target_directory,
            stage: TransactionStage::Staging,
            step_index: 0,
            total_steps: self.staged_entries.len(),
            current_entry: None,
        }, TransactionFailurePoint::BeforeAnyReplacement)?;

        // Ensure target directory exists if create_if_missing is true
        if self.create_if_missing && !self.target_directory.exists() {
            if let Err(e) = fs::create_dir_all(&self.target_directory) {
                return Err(MapSaveError {
                    stage: TransactionStage::Staging,
                    message: format!("建立目標地圖目錄失敗 ({}): {e}", self.target_directory.display()),
                    rollback_status: RollbackStatus::NoModificationsToRevert,
                });
            }
        }

        // Phase 3: Stage content buffers in memory and check staging injection points
        let mut resolved_buffers: Vec<Option<Vec<u8>>> = Vec::with_capacity(self.staged_entries.len());
        for (i, entry) in self.staged_entries.iter().enumerate() {
            self.check_failure_injection(&TransactionStageContext {
                target_directory: &self.target_directory,
                stage: TransactionStage::Staging,
                step_index: i,
                total_steps: self.staged_entries.len(),
                current_entry: Some(entry),
            }, TransactionFailurePoint::DuringStaging(i))?;

            match &entry.action {
                StagedAction::WriteBytes(bytes) => {
                    resolved_buffers.push(Some(bytes.clone()));
                }
                StagedAction::CopyFromPath(src) => {
                    let bytes = fs::read(src).map_err(|e| MapSaveError {
                        stage: TransactionStage::Staging,
                        message: format!("讀取暫存複製來源失敗 ({}): {e}", src.display()),
                        rollback_status: RollbackStatus::NoModificationsToRevert,
                    })?;
                    resolved_buffers.push(Some(bytes));
                }
                StagedAction::Delete => {
                    resolved_buffers.push(None);
                }
            }
        }

        // Phase 4: Initialize rollback tracking scope
        let mut rollback = FileRollbackScope::new().map_err(|e| MapSaveError {
            stage: TransactionStage::RollbackTracking,
            message: format!("初始化回滾暫存環境失敗: {e}"),
            rollback_status: RollbackStatus::NoModificationsToRevert,
        })?;

        // Track every target destination in the rollback scope BEFORE mutating any file.
        for entry in &self.staged_entries {
            let dest_path = self.target_directory.join(&entry.relative_path);
            if let Err(e) = rollback.track_file(&dest_path) {
                let _ = rollback.restore_all();
                return Err(MapSaveError {
                    stage: TransactionStage::RollbackTracking,
                    message: format!("追蹤目標檔案回滾備份失敗 ({}): {e}", dest_path.display()),
                    rollback_status: RollbackStatus::NoModificationsToRevert,
                });
            }
        }

        // Phase 5: Execute atomic replacements
        let mut written_files = Vec::new();
        let mut deleted_files = Vec::new();
        let mut total_bytes_written: u64 = 0;

        for (i, entry) in self.staged_entries.iter().enumerate() {
            let dest_path = self.target_directory.join(&entry.relative_path);
            let ctx = TransactionStageContext {
                target_directory: &self.target_directory,
                stage: TransactionStage::FileReplacement,
                step_index: i,
                total_steps: self.staged_entries.len(),
                current_entry: Some(entry),
            };

            // Injected failure before replacing this file
            if let Err(e) = self.check_failure_injection(&ctx, TransactionFailurePoint::BeforeReplacement(i))
                .and_then(|()| self.check_failure_injection(&ctx, TransactionFailurePoint::BeforePath(entry.relative_path.clone())))
            {
                let rb_res = rollback.restore_all();
                clean_temp_files(&self.target_directory);
                return Err(MapSaveError {
                    stage: TransactionStage::FileReplacement,
                    message: e.message,
                    rollback_status: match rb_res {
                        Ok(()) => RollbackStatus::CleanlyRestored,
                        Err(rbe) => RollbackStatus::PartialError(rbe),
                    },
                });
            }

            // Perform replacement
            match &resolved_buffers[i] {
                Some(bytes) => {
                    if let Err(e) = write_atomic_destination(&dest_path, bytes) {
                        let rb_res = rollback.restore_all();
                        clean_temp_files(&self.target_directory);
                        return Err(MapSaveError {
                            stage: TransactionStage::FileReplacement,
                            message: format!("寫入目標檔案失敗 ({}): {e}", dest_path.display()),
                            rollback_status: match rb_res {
                                Ok(()) => RollbackStatus::CleanlyRestored,
                                Err(rbe) => RollbackStatus::PartialError(rbe),
                            },
                        });
                    }
                    total_bytes_written += bytes.len() as u64;
                    written_files.push(dest_path.clone());
                }
                None => {
                    // StagedAction::Delete
                    if dest_path.exists() {
                        clear_readonly(&dest_path);
                        if let Err(e) = fs::remove_file(&dest_path) {
                            let rb_res = rollback.restore_all();
                            clean_temp_files(&self.target_directory);
                            return Err(MapSaveError {
                                stage: TransactionStage::FileReplacement,
                                message: format!("刪除目標檔案失敗 ({}): {e}", dest_path.display()),
                                rollback_status: match rb_res {
                                    Ok(()) => RollbackStatus::CleanlyRestored,
                                    Err(rbe) => RollbackStatus::PartialError(rbe),
                                },
                            });
                        }
                        deleted_files.push(dest_path.clone());
                    }
                }
            }

            // Injected failure after replacing this file
            if let Err(e) = self.check_failure_injection(&ctx, TransactionFailurePoint::DuringReplacement(i))
                .and_then(|()| self.check_failure_injection(&ctx, TransactionFailurePoint::AfterPath(entry.relative_path.clone())))
            {
                let rb_res = rollback.restore_all();
                clean_temp_files(&self.target_directory);
                return Err(MapSaveError {
                    stage: TransactionStage::FileReplacement,
                    message: e.message,
                    rollback_status: match rb_res {
                        Ok(()) => RollbackStatus::CleanlyRestored,
                        Err(rbe) => RollbackStatus::PartialError(rbe),
                    },
                });
            }
        }

        // Phase 6: Post-replacement verification
        for (i, entry) in self.staged_entries.iter().enumerate() {
            let dest_path = self.target_directory.join(&entry.relative_path);
            match &resolved_buffers[i] {
                Some(expected_bytes) => {
                    if !dest_path.is_file() {
                        let rb_res = rollback.restore_all();
                        clean_temp_files(&self.target_directory);
                        return Err(MapSaveError {
                            stage: TransactionStage::PostVerification,
                            message: format!("寫入後驗證失敗: 目標檔案未建立 ({})", dest_path.display()),
                            rollback_status: match rb_res {
                                Ok(()) => RollbackStatus::CleanlyRestored,
                                Err(rbe) => RollbackStatus::PartialError(rbe),
                            },
                        });
                    }

                    if self.verify_written_bytes {
                        match fs::read(&dest_path) {
                            Ok(actual) => {
                                if actual != *expected_bytes {
                                    let rb_res = rollback.restore_all();
                                    clean_temp_files(&self.target_directory);
                                    return Err(MapSaveError {
                                        stage: TransactionStage::PostVerification,
                                        message: format!(
                                            "寫入後位元組校驗失敗 ({}): 預期 {} 位元組，實際讀取 {} 位元組",
                                            dest_path.display(),
                                            expected_bytes.len(),
                                            actual.len()
                                        ),
                                        rollback_status: match rb_res {
                                            Ok(()) => RollbackStatus::CleanlyRestored,
                                            Err(rbe) => RollbackStatus::PartialError(rbe),
                                        },
                                    });
                                }
                            }
                            Err(e) => {
                                let rb_res = rollback.restore_all();
                                clean_temp_files(&self.target_directory);
                                return Err(MapSaveError {
                                    stage: TransactionStage::PostVerification,
                                    message: format!("讀回校驗檔案失敗 ({}): {e}", dest_path.display()),
                                    rollback_status: match rb_res {
                                        Ok(()) => RollbackStatus::CleanlyRestored,
                                        Err(rbe) => RollbackStatus::PartialError(rbe),
                                    },
                                });
                            }
                        }
                    }
                }
                None => {
                    if dest_path.exists() {
                        let rb_res = rollback.restore_all();
                        clean_temp_files(&self.target_directory);
                        return Err(MapSaveError {
                            stage: TransactionStage::PostVerification,
                            message: format!("刪除後驗證失敗: 檔案依然存在 ({})", dest_path.display()),
                            rollback_status: match rb_res {
                                Ok(()) => RollbackStatus::CleanlyRestored,
                                Err(rbe) => RollbackStatus::PartialError(rbe),
                            },
                        });
                    }
                }
            }
        }

        if let Err(e) = self.check_failure_injection(&TransactionStageContext {
            target_directory: &self.target_directory,
            stage: TransactionStage::PostVerification,
            step_index: self.staged_entries.len(),
            total_steps: self.staged_entries.len(),
            current_entry: None,
        }, TransactionFailurePoint::DuringPostVerification) {
            let rb_res = rollback.restore_all();
            clean_temp_files(&self.target_directory);
            return Err(MapSaveError {
                stage: TransactionStage::PostVerification,
                message: e.message,
                rollback_status: match rb_res {
                    Ok(()) => RollbackStatus::CleanlyRestored,
                    Err(rbe) => RollbackStatus::PartialError(rbe),
                },
            });
        }

        // Phase 7: Injected failure before final commit
        if let Err(e) = self.check_failure_injection(&TransactionStageContext {
            target_directory: &self.target_directory,
            stage: TransactionStage::FinalCommit,
            step_index: self.staged_entries.len(),
            total_steps: self.staged_entries.len(),
            current_entry: None,
        }, TransactionFailurePoint::BeforeFinalCommit) {
            let rb_res = rollback.restore_all();
            clean_temp_files(&self.target_directory);
            return Err(MapSaveError {
                stage: TransactionStage::FinalCommit,
                message: e.message,
                rollback_status: match rb_res {
                    Ok(()) => RollbackStatus::CleanlyRestored,
                    Err(rbe) => RollbackStatus::PartialError(rbe),
                },
            });
        }

        // Phase 8: Final commit
        rollback.commit();
        clean_temp_files(&self.target_directory);

        let duration_millis = start_time.elapsed().as_millis() as u64;
        Ok(MapSaveReport {
            target_directory: self.target_directory.clone(),
            written_files,
            deleted_files,
            total_bytes_written,
            duration_millis,
        })
    }

    fn check_failure_injection(
        &self,
        ctx: &TransactionStageContext,
        point: TransactionFailurePoint,
    ) -> Result<(), MapSaveError> {
        if let Some((target_point, msg)) = &self.injected_failure {
            if *target_point == point {
                return Err(MapSaveError {
                    stage: ctx.stage,
                    message: format!("模擬注入失敗: {msg}"),
                    rollback_status: RollbackStatus::NoModificationsToRevert,
                });
            }
        }

        if let Some(injector) = &self.failure_injector {
            if let Err(msg) = injector(ctx) {
                return Err(MapSaveError {
                    stage: ctx.stage,
                    message: format!("自定義模擬注入失敗: {msg}"),
                    rollback_status: RollbackStatus::NoModificationsToRevert,
                });
            }
        }

        Ok(())
    }
}

// ==========================================
// File replacement and cleanup helpers
// ==========================================

const ATOMIC_REPLACE_RETRIES: u32 = 4;
const ATOMIC_REPLACE_RETRY_MS: u64 = 50;

fn write_atomic_destination(dest: &Path, bytes: &[u8]) -> Result<(), String> {
    if has_reparse_point_component(dest) {
        return Err(format!("目標路徑包含符號連結或重解析點: {}", dest.display()));
    }

    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    if !parent.exists() {
        fs::create_dir_all(parent).map_err(|e| format!("建立目標目錄失敗 ({}): {e}", parent.display()))?;
    }

    let file_name = dest.file_name().and_then(|s| s.to_str()).unwrap_or("file");
    let temp_file = parent.join(format!("{}.{}.tmp_tx", file_name, Uuid::new_v4().simple()));

    fs::write(&temp_file, bytes).map_err(|e| {
        let _ = fs::remove_file(&temp_file);
        format!("寫入暫存檔案失敗 ({}): {e}", temp_file.display())
    })?;

    clear_readonly(dest);

    let mut last_err = None;
    for attempt in 0..ATOMIC_REPLACE_RETRIES {
        match fs::rename(&temp_file, dest) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                if attempt + 1 < ATOMIC_REPLACE_RETRIES {
                    std::thread::sleep(std::time::Duration::from_millis(ATOMIC_REPLACE_RETRY_MS));
                }
            }
        }
    }

    let _ = fs::remove_file(&temp_file);
    Err(format!(
        "替換目標檔案失敗，檔案可能被佔用 ({}): {}",
        dest.display(),
        last_err.map(|e| e.to_string()).unwrap_or_default()
    ))
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

fn clean_temp_files(dir: &Path) {
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
                clean_temp_files(&path);
            } else if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                if name.contains(".tmp_tx") {
                    clear_readonly(&path);
                    let _ = fs::remove_file(&path);
                }
            }
        }
    }
}
