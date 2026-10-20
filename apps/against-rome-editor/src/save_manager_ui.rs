//! Standalone Save Manager UI and Desktop Application.
//! Direct port of the existing C# `src.SaveManager/UI/SaveManagerForm.cs` workflow.
//! Uses native `against-rome-saves` and `against-rome-modifier::save_manager` services.
//! Features:
//! - Dual-view tables: Live in-game saves (`SAVE/*`) and backup ZIP archives (`SavesBackup/*`).
//! - Selected save details & TGA preview decoding.
//! - Safe backup creation, atomic transactional restore with overwrite protection.
//! - Permanent live save delete and backup archive delete with explicit modal confirmation.
//! - Endless AI repair flow (`SaveService::repair_endless_ai`) with automatic pre-repair backup.
//! - Bilingual support (Traditional Chinese and English) matching C# `Localization.cs`.
//! - Strict safety invariants: path traversal rejection, Program Files protection, simple name validation.

use against_rome_modifier::{
    save_manager::{SaveBackupCatalog, SaveBackupService},
    services::GameDirectoryLocator,
    tga::TgaImage,
};
use against_rome_saves::AiRepairResult;
use eframe::egui::{self, Color32, ColorImage, FontId, RichText, Sense, Stroke, TextureHandle, Vec2};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// UI Language matching C# `Loc.CurrentLanguage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    TraditionalChinese,
    English,
}

impl Language {
    pub fn is_zh(&self) -> bool {
        matches!(self, Language::TraditionalChinese)
    }

    pub fn window_title(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "Against Rome Modifier - 存檔管理器",
            Language::English => "Against Rome Modifier - Save Manager",
        }
    }

    pub fn main_title(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "AGAINST ROME 存檔管理器",
            Language::English => "AGAINST ROME SAVE MANAGER",
        }
    }

    pub fn game_path_label(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "遊戲目錄：",
            Language::English => "Game Path:",
        }
    }

    pub fn backup_path_label(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "備份目錄：",
            Language::English => "Backup Path:",
        }
    }

    pub fn game_saves_title(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "遊戲中存檔列表",
            Language::English => "In-Game Save List",
        }
    }

    pub fn backups_title(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "備份歷史列表",
            Language::English => "Backup History List",
        }
    }

    pub fn detail_title(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "存檔詳細與預覽",
            Language::English => "Save Details & Preview",
        }
    }

    pub fn btn_backup(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "備份此存檔",
            Language::English => "Backup Save",
        }
    }

    pub fn btn_delete_save(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "刪除此存檔",
            Language::English => "Delete Save",
        }
    }

    pub fn btn_refresh(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "重新整理",
            Language::English => "Refresh",
        }
    }

    pub fn btn_repair_ai(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "修復無盡 AI",
            Language::English => "Repair Endless AI",
        }
    }

    pub fn btn_restore(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "還原此備份",
            Language::English => "Restore Backup",
        }
    }

    pub fn btn_delete_backup(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "刪除此備份",
            Language::English => "Delete Backup",
        }
    }

    pub fn header_folder(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "資料夾",
            Language::English => "Folder",
        }
    }

    pub fn header_save_title(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "存檔標題",
            Language::English => "Save Title",
        }
    }

    pub fn header_level(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "關卡",
            Language::English => "Level",
        }
    }

    pub fn header_time(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "最後存檔時間",
            Language::English => "Time",
        }
    }

    pub fn header_backup_file(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "備份檔名",
            Language::English => "Backup File",
        }
    }

    pub fn header_backup_time(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "備份時間",
            Language::English => "Time",
        }
    }

    pub fn header_orig_folder(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "原資料夾",
            Language::English => "Original Folder",
        }
    }

    pub fn unparsable(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "無法解析",
            Language::English => "Unparsable",
        }
    }

    pub fn unparsed_note(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "save.ini 未完整解析",
            Language::English => "save.ini not fully parsed",
        }
    }

    /// Title/level cell text. Unlike the C# grid (which replaces unparsed rows with a
    /// placeholder), metadata that was still read stays visible and is flagged instead.
    pub fn metadata_display(&self, parsed: bool, value: &str, placeholder: &'static str) -> String {
        if parsed {
            value.to_string()
        } else if value.trim().is_empty() {
            placeholder.to_string()
        } else {
            format!("{value} ⚠{}", self.unparsed_note())
        }
    }

    pub fn title_display(&self, parsed: bool, title: &str) -> String {
        self.metadata_display(parsed, title, self.unparsable())
    }

    pub fn level_display(&self, parsed: bool, level: &str) -> String {
        self.metadata_display(parsed, level, self.unknown())
    }

    pub fn unknown(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "未知",
            Language::English => "Unknown",
        }
    }

    pub fn no_preview(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "[無預覽圖片]",
            Language::English => "[No Preview Image]",
        }
    }

    pub fn confirm(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "確定執行",
            Language::English => "Confirm",
        }
    }

    pub fn cancel(&self) -> &'static str {
        match self {
            Language::TraditionalChinese => "取消",
            Language::English => "Cancel",
        }
    }

    pub fn format_game_save_detail(
        &self,
        folder: &str,
        title: &str,
        level: &str,
        time: &str,
    ) -> String {
        match self {
            Language::TraditionalChinese => {
                format!(
                    "存檔類型: 遊戲存檔\n\n資料夾: {folder}\n\n存檔標題: {title}\n\n原版關卡: {level}\n\n存檔時間: {time}"
                )
            }
            Language::English => {
                format!(
                    "Save Type: Game Save\n\nFolder: {folder}\n\nSave Title: {title}\n\nOriginal Level: {level}\n\nSave Time: {time}"
                )
            }
        }
    }

    pub fn format_backup_detail(
        &self,
        file: &str,
        orig_folder: &str,
        title: &str,
        level: &str,
        time: &str,
    ) -> String {
        match self {
            Language::TraditionalChinese => {
                format!(
                    "存檔類型: 備份檔案\n\n備份檔名: {file}\n\n原資料夾: {orig_folder}\n\n存檔標題: {title}\n\n原版關卡: {level}\n\n備份時間: {time}"
                )
            }
            Language::English => {
                format!(
                    "Save Type: Backup File\n\nBackup File: {file}\n\nOriginal Folder: {orig_folder}\n\nSave Title: {title}\n\nOriginal Level: {level}\n\nBackup Time: {time}"
                )
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Safety guards matching toolbox.rs and SaveService invariants
// ---------------------------------------------------------------------------

pub fn check_explicit_path(s: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(s.trim());
    if s.trim().is_empty()
        || !p.is_absolute()
        || p.components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("請輸入完整絕對路徑，不接受空白或包含 '..' 的路徑".into());
    }
    for a in p.ancestors() {
        match fs::symlink_metadata(a) {
            Ok(m) => {
                if m.file_type().is_symlink() {
                    return Err(format!("拒絕 symbolic link: {}", a.display()));
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if m.file_attributes() & 0x400 != 0 {
                        return Err(format!("拒絕 junction/reparse point: {}", a.display()));
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(p)
}

pub fn check_writable_path(p: &Path) -> Result<(), String> {
    let text = p.to_string_lossy().replace('/', "\\").to_lowercase();
    if text
        .split('\\')
        .any(|c| c == "program files" || c == "program files (x86)")
    {
        return Err("禁止直接寫入 Program Files 安裝目錄；請在獨立測試副本上操作".into());
    }
    for key in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Some(root) = std::env::var_os(key) {
            let root = PathBuf::from(root);
            if p.starts_with(&root) {
                return Err("禁止直接寫入系統安裝目錄；請在獨立工作副本上操作".into());
            }
        }
    }
    Ok(())
}

/// Convert TgaImage (BGRA bytes) to egui ColorImage.
pub fn tga_to_color_image(image: &TgaImage) -> ColorImage {
    let mut pixels = Vec::with_capacity(image.width * image.height);
    for chunk in image.bgra.chunks_exact(4) {
        let (b, g, r, a) = (chunk[0], chunk[1], chunk[2], chunk[3]);
        pixels.push(Color32::from_rgba_unmultiplied(r, g, b, a));
    }
    ColorImage {
        size: [image.width, image.height],
        pixels,
    }
}

// ---------------------------------------------------------------------------
// Actions requiring explicit confirmation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmedAction {
    DeleteSave { folder: String },
    DeleteBackup { file_name: String },
    RestoreBackup { file_name: String, target_folder: String },
    RepairEndlessAi { folder: String },
}

#[derive(Debug, Clone)]
pub struct PendingConfirmation {
    pub action: ConfirmedAction,
    pub title: String,
    pub prompt: String,
}

// ---------------------------------------------------------------------------
// Standalone Save Manager Model
// ---------------------------------------------------------------------------

pub struct SaveManagerModel {
    pub game_path: String,
    pub backup_path: String,
    pub catalog: Option<SaveBackupCatalog>,
    pub selected_live_save: Option<String>,
    pub selected_backup: Option<String>,
    pub preview_image: Option<ColorImage>,
    pub detail_text: String,
    pub status_message: String,
    pub status_is_error: bool,
    pub pending_confirmation: Option<PendingConfirmation>,
    pub language: Language,
    last_preview_key: String,
}

impl SaveManagerModel {
    pub fn new(initial_game_path: Option<String>) -> Self {
        let resolved = if let Some(p) = initial_game_path.filter(|s| !s.trim().is_empty()) {
            p
        } else {
            let found = GameDirectoryLocator::resolve_initial_game_path(None);
            found.to_string_lossy().into_owned()
        };

        let default_backup = if !resolved.trim().is_empty() {
            PathBuf::from(&resolved).join("SavesBackup").to_string_lossy().into_owned()
        } else {
            std::env::current_dir()
                .map(|p| p.join("SavesBackup").to_string_lossy().into_owned())
                .unwrap_or_else(|_| "SavesBackup".to_string())
        };

        let mut model = Self {
            game_path: resolved,
            backup_path: default_backup,
            catalog: None,
            selected_live_save: None,
            selected_backup: None,
            preview_image: None,
            detail_text: String::new(),
            status_message: String::new(),
            status_is_error: false,
            pending_confirmation: None,
            language: Language::TraditionalChinese,
            last_preview_key: String::new(),
        };

        let _ = model.rescan();
        model
    }

    pub fn set_game_path(&mut self, path: String) {
        if self.game_path != path {
            self.game_path = path;
            if self.backup_path.is_empty() || self.backup_path.ends_with("SavesBackup") {
                if !self.game_path.trim().is_empty() {
                    self.backup_path = PathBuf::from(&self.game_path)
                        .join("SavesBackup")
                        .to_string_lossy()
                        .into_owned();
                }
            }
            self.clear_selection();
            let _ = self.rescan();
        }
    }

    pub fn set_backup_path(&mut self, path: String) {
        if self.backup_path != path {
            self.backup_path = path;
            self.clear_selection();
            let _ = self.rescan();
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected_live_save = None;
        self.selected_backup = None;
        self.preview_image = None;
        self.detail_text.clear();
        self.last_preview_key.clear();
    }

    fn service(&self) -> Result<SaveBackupService, String> {
        let game = check_explicit_path(&self.game_path)?;
        let backups = check_explicit_path(&self.backup_path)?;
        SaveBackupService::new(&game, &backups).map_err(|e| e.to_string())
    }

    pub fn rescan(&mut self) -> Result<(), String> {
        let g = self.game_path.trim();
        if g.is_empty() {
            self.catalog = None;
            return Ok(());
        }
        let service = match self.service() {
            Ok(s) => s,
            Err(e) => {
                self.status_message = e.clone();
                self.status_is_error = true;
                return Err(e);
            }
        };

        match service.scan() {
            Ok(cat) => {
                self.catalog = Some(cat);
                self.status_is_error = false;
                // Re-bind preview if a selection is still valid
                if let Some(folder) = self.selected_live_save.clone() {
                    self.select_live_save(&folder);
                } else if let Some(file) = self.selected_backup.clone() {
                    self.select_backup(&file);
                }
                Ok(())
            }
            Err(e) => {
                let err_msg = format!("掃描存檔失敗: {e}");
                self.status_message = err_msg.clone();
                self.status_is_error = true;
                Err(err_msg)
            }
        }
    }

    pub fn select_live_save(&mut self, folder: &str) {
        self.selected_live_save = Some(folder.to_string());
        self.selected_backup = None;

        let Some(cat) = &self.catalog else { return };
        let save_info = cat.saves.iter().find(|s| s.folder == folder);

        let title = save_info
            .map(|s| self.language.title_display(s.parsed, &s.title))
            .unwrap_or_else(|| self.language.unknown().to_string());
        let level = save_info
            .map(|s| self.language.level_display(s.parsed, &s.level))
            .unwrap_or_else(|| self.language.unknown().to_string());
        let unparsed = save_info.is_some_and(|s| !s.parsed);
        let time = save_info
            .map(|s| s.last_write_time.as_str())
            .unwrap_or("");

        self.detail_text = self.language.format_game_save_detail(folder, &title, &level, time);
        if unparsed {
            self.detail_text.push_str(&format!("\n\n⚠ {}", self.language.unparsed_note()));
        }

        let key = format!("live:{folder}");
        if self.last_preview_key != key {
            self.last_preview_key = key;
            self.preview_image = self
                .service()
                .ok()
                .and_then(|svc| svc.decode_save_preview(folder).ok().flatten())
                .as_ref()
                .map(tga_to_color_image);
        }
    }

    pub fn select_backup(&mut self, file_name: &str) {
        self.selected_backup = Some(file_name.to_string());
        self.selected_live_save = None;

        let Some(cat) = &self.catalog else { return };
        let backup_info = cat.backups.iter().find(|b| b.file_name == file_name);

        let orig_folder = backup_info
            .map(|b| if b.orig_folder.is_empty() { self.language.unknown() } else { b.orig_folder.as_str() })
            .unwrap_or_else(|| self.language.unknown());
        let title = backup_info
            .map(|b| self.language.title_display(b.parsed, &b.title))
            .unwrap_or_else(|| self.language.unknown().to_string());
        let level = backup_info
            .map(|b| self.language.level_display(b.parsed, &b.level))
            .unwrap_or_else(|| self.language.unknown().to_string());
        let unparsed = backup_info.is_some_and(|b| !b.parsed);
        let time = backup_info
            .map(|b| b.backup_time.as_str())
            .unwrap_or("");

        self.detail_text = self.language.format_backup_detail(file_name, orig_folder, &title, &level, time);
        if unparsed {
            self.detail_text.push_str(&format!("\n\n⚠ {}", self.language.unparsed_note()));
        }

        let key = format!("backup:{file_name}");
        if self.last_preview_key != key {
            self.last_preview_key = key;
            self.preview_image = self
                .service()
                .ok()
                .and_then(|svc| svc.decode_backup_preview(file_name).ok().flatten())
                .as_ref()
                .map(tga_to_color_image);
        }
    }

    // --- Action Requests ---

    /// Records `result`'s error in the visible status line; success passes through.
    fn report<T>(&mut self, result: Result<T, String>) -> Result<T, String> {
        if let Err(e) = &result {
            self.status_message = e.clone();
            self.status_is_error = true;
        }
        result
    }

    /// Rescans after a successful action; a rescan failure stays visible next to the
    /// success message instead of being overwritten by it.
    fn finish_action(&mut self, mut msg: String) -> String {
        let rescan = self.rescan();
        if let Err(e) = &rescan {
            msg.push_str(&format!(" ({e})"));
        }
        self.status_message = msg.clone();
        self.status_is_error = rescan.is_err();
        msg
    }

    pub fn request_backup_save(&mut self) -> Result<String, String> {
        let result = self.request_backup_save_inner();
        self.report(result)
    }

    fn request_backup_save_inner(&mut self) -> Result<String, String> {
        let Some(folder) = self.selected_live_save.clone() else {
            return Err(if self.language.is_zh() {
                "請先選擇要備份的存檔。".into()
            } else {
                "Please select a save to backup first.".into()
            });
        };

        if !SaveBackupService::is_simple_name(&folder) {
            return Err(if self.language.is_zh() {
                "無效的存檔目錄，操作已取消。".into()
            } else {
                "Invalid save directory, operation cancelled.".into()
            });
        }

        let (title, level) = if let Some(cat) = &self.catalog {
            cat.saves
                .iter()
                .find(|s| s.folder == folder)
                .map(|s| (s.title.clone(), s.level.clone()))
                .unwrap_or_default()
        } else {
            (String::new(), String::new())
        };

        let backups_dir = check_explicit_path(&self.backup_path)?;
        check_writable_path(&backups_dir)?;

        let svc = self.service()?;
        let created = svc
            .create_backup(&folder, &title, &level)
            .map_err(|e| format!("備份存檔失敗: {e}"))?;

        let msg = if self.language.is_zh() {
            format!("備份存檔成功！已建立備份: {created}")
        } else {
            format!("Save backed up successfully! Created: {created}")
        };
        Ok(self.finish_action(msg))
    }

    pub fn request_delete_save(&mut self) {
        let Some(folder) = self.selected_live_save.clone() else {
            self.status_message = if self.language.is_zh() {
                "請先選擇要刪除的存檔。".into()
            } else {
                "Please select a save to delete first.".into()
            };
            self.status_is_error = true;
            return;
        };

        if !SaveBackupService::is_simple_name(&folder) {
            self.status_message = "無效的存檔目錄".into();
            self.status_is_error = true;
            return;
        }

        let prompt = if self.language.is_zh() {
            format!("確定要永久刪除遊戲存檔 [{folder}] 嗎？此操作不可還原！")
        } else {
            format!("Are you sure you want to permanently delete game save [{folder}]? This action cannot be undone!")
        };

        self.pending_confirmation = Some(PendingConfirmation {
            action: ConfirmedAction::DeleteSave { folder },
            title: if self.language.is_zh() { "確認刪除存檔".into() } else { "Confirm Delete Save".into() },
            prompt,
        });
    }

    pub fn request_delete_backup(&mut self) {
        let Some(file_name) = self.selected_backup.clone() else {
            self.status_message = if self.language.is_zh() {
                "請先選擇要刪除的備份。".into()
            } else {
                "Please select a backup to delete first.".into()
            };
            self.status_is_error = true;
            return;
        };

        if !SaveBackupService::is_simple_name(&file_name) {
            self.status_message = "無效的備份檔名".into();
            self.status_is_error = true;
            return;
        }

        let prompt = if self.language.is_zh() {
            format!("確定要永久刪除備份檔案 [{file_name}] 嗎？")
        } else {
            format!("Are you sure you want to permanently delete backup file [{file_name}]?")
        };

        self.pending_confirmation = Some(PendingConfirmation {
            action: ConfirmedAction::DeleteBackup { file_name },
            title: if self.language.is_zh() { "確認刪除備份".into() } else { "Confirm Delete Backup".into() },
            prompt,
        });
    }

    pub fn request_restore_backup(&mut self) {
        let Some(file_name) = self.selected_backup.clone() else {
            self.status_message = if self.language.is_zh() {
                "請先選擇要還原的備份。".into()
            } else {
                "Please select a backup to restore first.".into()
            };
            self.status_is_error = true;
            return;
        };

        let target_folder = if let Some(cat) = &self.catalog {
            cat.backups
                .iter()
                .find(|b| b.file_name == file_name)
                .map(|b| b.orig_folder.clone())
                .unwrap_or_default()
        } else {
            String::new()
        };

        if target_folder.is_empty() || !SaveBackupService::is_simple_name(&target_folder) {
            self.status_message = if self.language.is_zh() {
                "無法取得有效的原資料夾名稱，無法還原。".into()
            } else {
                "Cannot determine valid original folder name, restore aborted.".into()
            };
            self.status_is_error = true;
            return;
        }

        let Ok(game_root) = check_explicit_path(&self.game_path) else {
            self.status_message = "遊戲路徑無效".into();
            self.status_is_error = true;
            return;
        };
        let dest = game_root.join("SAVE").join(&target_folder);

        let prompt = if dest.exists() {
            if self.language.is_zh() {
                format!("目標存檔資料夾 [{target_folder}] 已存在，還原將會覆蓋它。是否繼續？")
            } else {
                format!("Target save folder [{target_folder}] already exists. Overwrite?")
            }
        } else {
            if self.language.is_zh() {
                format!("確定要將備份 [{file_name}] 還原至 [{target_folder}] 嗎？")
            } else {
                format!("Are you sure you want to restore backup [{file_name}] to [{target_folder}]?")
            }
        };

        self.pending_confirmation = Some(PendingConfirmation {
            action: ConfirmedAction::RestoreBackup {
                file_name,
                target_folder,
            },
            title: if self.language.is_zh() { "確認還原備份".into() } else { "Confirm Restore Backup".into() },
            prompt,
        });
    }

    pub fn request_repair_endless_ai(&mut self) {
        let Some(folder) = self.selected_live_save.clone() else {
            self.status_message = if self.language.is_zh() {
                "請先選擇要修復的無盡模式存檔。".into()
            } else {
                "Please select an endless mode save to repair first.".into()
            };
            self.status_is_error = true;
            return;
        };

        if !SaveBackupService::is_simple_name(&folder) {
            self.status_message = "無效的存檔目錄".into();
            self.status_is_error = true;
            return;
        }

        let prompt = if self.language.is_zh() {
            "修復前會先建立完整備份，再更新存檔內嵌的無盡 AI 排程與增援規則。已堆積的馱馬不會自動移除。是否繼續？".into()
        } else {
            "A full backup will be created before repairing, then embedded endless AI scheduling and reinforcement rules will be updated. Packed mules will not be automatically removed. Continue?".into()
        };

        self.pending_confirmation = Some(PendingConfirmation {
            action: ConfirmedAction::RepairEndlessAi { folder },
            title: if self.language.is_zh() { "確認修復無盡 AI".into() } else { "Confirm Repair Endless AI".into() },
            prompt,
        });
    }

    pub fn cancel_pending_action(&mut self) {
        self.pending_confirmation = None;
    }

    pub fn confirm_pending_action(&mut self) -> Result<String, String> {
        let result = self.confirm_pending_action_inner();
        self.report(result)
    }

    fn confirm_pending_action_inner(&mut self) -> Result<String, String> {
        let Some(pending) = self.pending_confirmation.take() else {
            return Err("無待確認操作".into());
        };

        let svc = self.service()?;
        match pending.action {
            ConfirmedAction::DeleteSave { folder } => {
                let game_root = check_explicit_path(&self.game_path)?;
                let dest = game_root.join("SAVE").join(&folder);
                check_writable_path(&dest)?;

                svc.delete_save(&folder).map_err(|e| format!("刪除存檔失敗: {e}"))?;
                self.selected_live_save = None;
                self.preview_image = None;
                self.detail_text.clear();

                let msg = if self.language.is_zh() {
                    format!("已成功刪除存檔 [{folder}]。")
                } else {
                    format!("Successfully deleted save [{folder}].")
                };
                Ok(self.finish_action(msg))
            }
            ConfirmedAction::DeleteBackup { file_name } => {
                let backup_root = check_explicit_path(&self.backup_path)?;
                let dest = backup_root.join(&file_name);
                check_writable_path(&dest)?;

                svc.delete_backup(&file_name).map_err(|e| format!("刪除備份失敗: {e}"))?;
                self.selected_backup = None;
                self.preview_image = None;
                self.detail_text.clear();

                let msg = if self.language.is_zh() {
                    format!("已成功刪除備份 [{file_name}]。")
                } else {
                    format!("Successfully deleted backup [{file_name}].")
                };
                Ok(self.finish_action(msg))
            }
            ConfirmedAction::RestoreBackup { file_name, target_folder } => {
                let game_root = check_explicit_path(&self.game_path)?;
                let dest = game_root.join("SAVE").join(&target_folder);
                check_writable_path(&dest)?;

                let warnings = svc
                    .restore_backup(&file_name, &target_folder)
                    .map_err(|e| format!("還原備份失敗: {e}"))?;

                let mut msg = if self.language.is_zh() {
                    format!("還原備份 [{file_name}] 至 [{target_folder}] 成功！")
                } else {
                    format!("Backup [{file_name}] restored to [{target_folder}] successfully!")
                };
                if !warnings.is_empty() {
                    msg.push_str(&format!(" (清理警告: {})", warnings.join("; ")));
                }
                Ok(self.finish_action(msg))
            }
            ConfirmedAction::RepairEndlessAi { folder } => {
                let game_root = check_explicit_path(&self.game_path)?;
                let dest = game_root.join("SAVE").join(&folder);
                check_writable_path(&dest)?;
                let backups_root = check_explicit_path(&self.backup_path)?;
                check_writable_path(&backups_root)?;

                let res = svc
                    .repair_endless_ai(&folder)
                    .map_err(|e| format!("修復無盡 AI 失敗: {e}"))?;

                let msg = match res {
                    AiRepairResult::AlreadyRepaired => {
                        if self.language.is_zh() {
                            "這份存檔已包含目前的無盡 AI 排程與增援修復。".into()
                        } else {
                            "This save already contains current endless AI scheduling and reinforcement fixes.".into()
                        }
                    }
                    AiRepairResult::Changed { backup } => {
                        if self.language.is_zh() {
                            format!("無盡 AI 修復成功，並已建立安全備份: {backup}")
                        } else {
                            format!("Endless AI repaired successfully; safety backup created: {backup}")
                        }
                    }
                };
                Ok(self.finish_action(msg))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Standalone Save Manager Desktop App (eframe::App)
// ---------------------------------------------------------------------------

pub type GameFolderPicker = Box<dyn Fn() -> Option<PathBuf> + Send + Sync>;

fn native_game_folder_dialog(current_path: &str) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new().set_title("Select Against Rome Game Folder");
    let current = Path::new(current_path.trim());
    if current.is_dir() {
        dialog = dialog.set_directory(current);
    }
    dialog.pick_folder()
}

pub struct SaveManagerApp {
    pub model: SaveManagerModel,
    preview_texture: Option<TextureHandle>,
    last_rendered_key: String,
    pub frames_remaining: Option<u8>,
    /// Injectable for headless tests; the desktop default opens the native folder picker.
    pub game_folder_picker: Option<GameFolderPicker>,
}

impl SaveManagerApp {
    pub fn new(initial_game_path: Option<String>) -> Self {
        Self {
            model: SaveManagerModel::new(initial_game_path),
            preview_texture: None,
            last_rendered_key: String::new(),
            frames_remaining: None,
            game_folder_picker: None,
        }
    }

    pub fn with_frames(mut self, frames: u8) -> Self {
        self.frames_remaining = Some(frames);
        self
    }

    /// Opens the native folder picker (C# `btnBrowseGamePath.Click`). Cancel or an invalid
    /// selection preserves the current model state; a valid selection goes through
    /// `SaveManagerModel::set_game_path` (default backup root + rescan).
    pub fn browse_game_folder(&mut self) {
        let selected = if let Some(picker) = &self.game_folder_picker {
            picker()
        } else {
            native_game_folder_dialog(&self.model.game_path)
        };
        let Some(path) = selected else { return };
        if !path.is_dir() {
            self.model.status_message = if self.model.language.is_zh() {
                format!("所選遊戲資料夾不存在: {}", path.display())
            } else {
                format!("Selected game folder does not exist: {}", path.display())
            };
            self.model.status_is_error = true;
            return;
        }
        let new_path = path.to_string_lossy().into_owned();
        if self.model.game_path == new_path {
            // C# always calls RefreshSavesAndBackups; `set_game_path` is a no-op for an
            // unchanged path, so reset the selection and rescan explicitly (no double scan).
            self.model.clear_selection();
            let _ = self.model.rescan();
        } else {
            self.model.set_game_path(new_path);
        }
    }
    pub fn show(&mut self, ctx: &egui::Context) {
        // Texture caching: load texture when preview_image changes
        let cur_key = &self.model.last_preview_key;
        if cur_key != &self.last_rendered_key {
            self.last_rendered_key = cur_key.clone();
            if let Some(img) = &self.model.preview_image {
                self.preview_texture = Some(ctx.load_texture(
                    "save_manager_preview",
                    img.clone(),
                    egui::TextureOptions::LINEAR,
                ));
            } else {
                self.preview_texture = None;
            }
        }

        let lang = self.model.language;

        // Visual Theme: Against Rome Dark Surface Styling
        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = Some(Color32::from_rgb(230, 235, 240));
        visuals.panel_fill = Color32::from_rgb(10, 11, 16);
        visuals.window_fill = Color32::from_rgb(18, 22, 31);
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(18, 22, 31);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(45, 53, 69));
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(26, 30, 42);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(38, 44, 60);
        visuals.widgets.active.bg_fill = Color32::from_rgb(48, 56, 78);
        ctx.set_visuals(visuals);

        // --- Top Bar (Title + Language Selector + Path Controls) ---
        egui::TopBottomPanel::top("top_panel")
            .frame(egui::Frame::none().fill(Color32::from_rgb(14, 16, 24)).inner_margin(12.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(
                        RichText::new(lang.main_title())
                            .color(Color32::from_rgb(0, 220, 255))
                            .strong()
                            .size(17.0),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Language buttons matching C# WinForms btnLangZH / btnLangEN
                        let is_en = matches!(lang, Language::English);
                        if ui
                            .selectable_label(is_en, "English")
                            .clicked()
                            && !is_en
                        {
                            self.model.language = Language::English;
                            if let Some(folder) = self.model.selected_live_save.clone() {
                                self.model.select_live_save(&folder);
                            } else if let Some(file) = self.model.selected_backup.clone() {
                                self.model.select_backup(&file);
                            }
                        }
                        let is_zh = matches!(lang, Language::TraditionalChinese);
                        if ui
                            .selectable_label(is_zh, "繁體中文")
                            .clicked()
                            && !is_zh
                        {
                            self.model.language = Language::TraditionalChinese;
                            if let Some(folder) = self.model.selected_live_save.clone() {
                                self.model.select_live_save(&folder);
                            } else if let Some(file) = self.model.selected_backup.clone() {
                                self.model.select_backup(&file);
                            }
                        }
                    });
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                // Paths row: Game Path & Backup Path
                ui.horizontal(|ui| {
                    ui.label(RichText::new(lang.game_path_label()).strong().color(Color32::from_rgb(180, 190, 205)));
                    let g_edit = ui.add(
                        egui::TextEdit::singleline(&mut self.model.game_path)
                            .desired_width(320.0)
                            .hint_text("C:\\Program Files (x86)\\Against Rome or working dir"),
                    );
                    if g_edit.lost_focus() {
                        let _ = self.model.rescan();
                    }
                    if ui.button(if lang.is_zh() { "瀏覽..." } else { "Browse..." }).clicked() {
                        self.browse_game_folder();
                    }

                    ui.add_space(16.0);

                    ui.label(RichText::new(lang.backup_path_label()).strong().color(Color32::from_rgb(180, 190, 205)));
                    let b_edit = ui.add(
                        egui::TextEdit::singleline(&mut self.model.backup_path)
                            .desired_width(260.0)
                            .hint_text("SavesBackup directory"),
                    );
                    if b_edit.lost_focus() {
                        let _ = self.model.rescan();
                    }

                    ui.add_space(12.0);
                    if ui
                        .button(RichText::new(lang.btn_refresh()).color(Color32::from_rgb(0, 220, 255)).strong())
                        .clicked()
                    {
                        let _ = self.model.rescan();
                    }
                });
            });

        // --- Status Footer ---
        egui::TopBottomPanel::bottom("status_footer")
            .frame(egui::Frame::none().fill(Color32::from_rgb(14, 16, 24)).inner_margin(8.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if !self.model.status_message.is_empty() {
                        let color = if self.model.status_is_error {
                            Color32::from_rgb(255, 90, 90)
                        } else {
                            Color32::from_rgb(100, 240, 150)
                        };
                        ui.label(RichText::new(&self.model.status_message).color(color));
                    } else {
                        ui.label(RichText::new("就緒 (Ready)").color(Color32::from_rgb(140, 150, 165)));
                    }
                });
            });

        // --- Main Responsive Central Workspace (68% left lists, 32% right preview/details) ---
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Color32::from_rgb(10, 11, 16)).inner_margin(12.0))
            .show(ctx, |ui| {
                let total_width = ui.available_width();
                let left_width = (total_width * 0.68).max(520.0);
                let right_width = (total_width - left_width - 16.0).max(280.0);
                let total_height = ui.available_height();
                let card_height = ((total_height - 16.0) / 2.0).max(220.0);

                ui.horizontal(|ui| {
                    // LEFT COLUMN (Live Saves & Backups)
                    ui.allocate_ui(Vec2::new(left_width, total_height), |ui| {
                        // Card 1: In-Game Saves
                        egui::Frame::group(ui.style())
                            .fill(Color32::from_rgb(18, 22, 31))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(45, 53, 69)))
                            .inner_margin(10.0)
                            .show(ui, |ui| {
                                ui.set_min_height(card_height);
                                ui.set_max_height(card_height);

                                ui.label(
                                    RichText::new(lang.game_saves_title())
                                        .strong()
                                        .color(Color32::from_rgb(0, 220, 255))
                                        .size(14.0),
                                );
                                ui.add_space(4.0);

                                // Live Saves Table Header
                                egui::Grid::new("live_saves_header")
                                    .striped(true)
                                    .min_col_width(80.0)
                                    .show(ui, |ui| {
                                        ui.label(RichText::new(lang.header_folder()).strong().color(Color32::from_rgb(0, 220, 255)));
                                        ui.label(RichText::new(lang.header_save_title()).strong().color(Color32::from_rgb(0, 220, 255)));
                                        ui.label(RichText::new(lang.header_level()).strong().color(Color32::from_rgb(0, 220, 255)));
                                        ui.label(RichText::new(lang.header_time()).strong().color(Color32::from_rgb(0, 220, 255)));
                                        ui.end_row();
                                    });

                                ui.separator();

                                // Scrollable Table Rows
                                let scroll_h = card_height - 90.0;
                                let mut to_select_save = None;
                                egui::ScrollArea::vertical()
                                    .id_source("live_saves_scroll")
                                    .max_height(scroll_h)
                                    .show(ui, |ui| {
                                        if let Some(cat) = &self.model.catalog {
                                            if cat.saves.is_empty() {
                                                ui.label(RichText::new("(無存檔目錄 / Empty)").color(Color32::from_rgb(130, 140, 150)));
                                            } else {
                                                for save in &cat.saves {
                                                    let is_selected = self.model.selected_live_save.as_deref() == Some(&save.folder);
                                                    let title_display = lang.title_display(save.parsed, &save.title);
                                                    let level_display = lang.level_display(save.parsed, &save.level);

                                                    let row_text = format!(
                                                        "{:<14} | {:<22} | {:<12} | {}",
                                                        save.folder, title_display, level_display, save.last_write_time
                                                    );

                                                    let resp = ui.selectable_label(is_selected, row_text);
                                                    if resp.clicked() {
                                                        to_select_save = Some(save.folder.clone());
                                                    }
                                                }
                                            }
                                        } else {
                                            ui.label(RichText::new("請確認遊戲路徑後點擊重新整理").color(Color32::from_rgb(160, 170, 180)));
                                        }
                                    });

                                if let Some(folder) = to_select_save {
                                    self.model.select_live_save(&folder);
                                }

                                ui.separator();
                                ui.add_space(4.0);

                                // Actions for Live Saves
                                ui.horizontal(|ui| {
                                    // Backup Button
                                    let btn_backup = egui::Button::new(
                                        RichText::new(lang.btn_backup())
                                            .strong()
                                            .color(Color32::from_rgb(0, 220, 255)),
                                    );
                                    if ui.add_enabled(self.model.selected_live_save.is_some(), btn_backup).clicked() {
                                        let _ = self.model.request_backup_save();
                                    }

                                    // Delete Save Button
                                    let btn_delete = egui::Button::new(
                                        RichText::new(lang.btn_delete_save())
                                            .strong()
                                            .color(Color32::from_rgb(255, 90, 90)),
                                    );
                                    if ui.add_enabled(self.model.selected_live_save.is_some(), btn_delete).clicked() {
                                        self.model.request_delete_save();
                                    }

                                    // Refresh Button
                                    if ui.button(RichText::new(lang.btn_refresh()).color(Color32::WHITE)).clicked() {
                                        let _ = self.model.rescan();
                                    }

                                    // Endless AI Repair Button
                                    let btn_repair = egui::Button::new(
                                        RichText::new(lang.btn_repair_ai())
                                            .strong()
                                            .color(Color32::from_rgb(255, 214, 64)),
                                    );
                                    if ui.add_enabled(self.model.selected_live_save.is_some(), btn_repair).clicked() {
                                        self.model.request_repair_endless_ai();
                                    }
                                });
                            });

                        ui.add_space(8.0);

                        // Card 2: Backup History
                        egui::Frame::group(ui.style())
                            .fill(Color32::from_rgb(18, 22, 31))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(45, 53, 69)))
                            .inner_margin(10.0)
                            .show(ui, |ui| {
                                ui.set_min_height(card_height);
                                ui.set_max_height(card_height);

                                ui.label(
                                    RichText::new(lang.backups_title())
                                        .strong()
                                        .color(Color32::from_rgb(0, 220, 255))
                                        .size(14.0),
                                );
                                ui.add_space(4.0);

                                // Backups Table Header
                                egui::Grid::new("backups_header")
                                    .striped(true)
                                    .min_col_width(70.0)
                                    .show(ui, |ui| {
                                        ui.label(RichText::new(lang.header_backup_file()).strong().color(Color32::from_rgb(0, 220, 255)));
                                        ui.label(RichText::new(lang.header_save_title()).strong().color(Color32::from_rgb(0, 220, 255)));
                                        ui.label(RichText::new(lang.header_level()).strong().color(Color32::from_rgb(0, 220, 255)));
                                        ui.label(RichText::new(lang.header_backup_time()).strong().color(Color32::from_rgb(0, 220, 255)));
                                        ui.label(RichText::new(lang.header_orig_folder()).strong().color(Color32::from_rgb(0, 220, 255)));
                                        ui.end_row();
                                    });

                                ui.separator();

                                // Scrollable Table Rows
                                let scroll_h = card_height - 90.0;
                                let mut to_select_backup = None;
                                egui::ScrollArea::vertical()
                                    .id_source("backups_scroll")
                                    .max_height(scroll_h)
                                    .show(ui, |ui| {
                                        if let Some(cat) = &self.model.catalog {
                                            if cat.backups.is_empty() {
                                                ui.label(RichText::new("(無備份檔案 / Empty)").color(Color32::from_rgb(130, 140, 150)));
                                            } else {
                                                for b in &cat.backups {
                                                    let is_selected = self.model.selected_backup.as_deref() == Some(&b.file_name);
                                                    let title_display = lang.title_display(b.parsed, &b.title);
                                                    let level_display = lang.level_display(b.parsed, &b.level);
                                                    let orig_display = if b.orig_folder.is_empty() { lang.unknown() } else { &b.orig_folder };

                                                    let row_text = format!(
                                                        "{:<26} | {:<16} | {:<10} | {:<18} | {}",
                                                        b.file_name, title_display, level_display, b.backup_time, orig_display
                                                    );

                                                    let resp = ui.selectable_label(is_selected, row_text);
                                                    if resp.clicked() {
                                                        to_select_backup = Some(b.file_name.clone());
                                                    }
                                                }
                                            }
                                        } else {
                                            ui.label(RichText::new("請確認備份路徑後點擊重新整理").color(Color32::from_rgb(160, 170, 180)));
                                        }
                                    });

                                if let Some(file) = to_select_backup {
                                    self.model.select_backup(&file);
                                }

                                ui.separator();
                                ui.add_space(4.0);

                                // Actions for Backups
                                ui.horizontal(|ui| {
                                    // Restore Button
                                    let btn_restore = egui::Button::new(
                                        RichText::new(lang.btn_restore())
                                            .strong()
                                            .color(Color32::from_rgb(180, 100, 255)),
                                    );
                                    if ui.add_enabled(self.model.selected_backup.is_some(), btn_restore).clicked() {
                                        self.model.request_restore_backup();
                                    }

                                    // Delete Backup Button
                                    let btn_delete_b = egui::Button::new(
                                        RichText::new(lang.btn_delete_backup())
                                            .strong()
                                            .color(Color32::from_rgb(255, 90, 90)),
                                    );
                                    if ui.add_enabled(self.model.selected_backup.is_some(), btn_delete_b).clicked() {
                                        self.model.request_delete_backup();
                                    }
                                });
                            });
                    });

                    ui.add_space(8.0);

                    // RIGHT COLUMN (Details & TGA Preview Card)
                    ui.allocate_ui(Vec2::new(right_width, total_height), |ui| {
                        egui::Frame::group(ui.style())
                            .fill(Color32::from_rgb(18, 22, 31))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(45, 53, 69)))
                            .inner_margin(14.0)
                            .show(ui, |ui| {
                                ui.set_min_height(total_height - 8.0);
                                ui.label(
                                    RichText::new(lang.detail_title())
                                        .strong()
                                        .color(Color32::from_rgb(0, 220, 255))
                                        .size(15.0),
                                );
                                ui.add_space(8.0);

                                // Preview Picture Box (Zoom/Aspect Preserving)
                                let preview_box_size = Vec2::new(right_width - 28.0, 200.0);
                                if let Some(tex) = &self.preview_texture {
                                    ui.allocate_ui(preview_box_size, |ui| {
                                        ui.image((tex.id(), preview_box_size));
                                    });
                                } else {
                                    // Dark Placeholder Box
                                    let (rect, _response) = ui.allocate_exact_size(preview_box_size, Sense::hover());
                                    ui.painter().rect_filled(rect, 4.0, Color32::from_rgb(12, 12, 16));
                                    ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.0_f32, Color32::from_rgb(35, 40, 55)));
                                    ui.painter().text(
                                        rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        lang.no_preview(),
                                        FontId::proportional(13.0),
                                        Color32::from_rgb(110, 120, 135),
                                    );
                                }

                                ui.add_space(14.0);
                                ui.separator();
                                ui.add_space(10.0);

                                // Details info display
                                egui::ScrollArea::vertical()
                                    .id_source("details_scroll")
                                    .show(ui, |ui| {
                                        if !self.model.detail_text.is_empty() {
                                            ui.label(
                                                RichText::new(&self.model.detail_text)
                                                    .color(Color32::from_rgb(210, 215, 225))
                                                    .size(13.5),
                                            );
                                        } else {
                                            ui.label(
                                                RichText::new(if lang.is_zh() {
                                                    "請在左側點選存檔或備份項目以檢視詳細內容與縮圖。"
                                                } else {
                                                    "Select a game save or backup on the left to view details and thumbnail."
                                                })
                                                .color(Color32::from_rgb(130, 140, 155)),
                                            );
                                        }
                                    });
                            });
                    });
                });
            });

        // --- Explicit Modal Confirmation Window for Destructive Actions ---
        if let Some(pending) = self.model.pending_confirmation.clone() {
            egui::Window::new(RichText::new(&pending.title).strong().color(Color32::from_rgb(255, 214, 64)))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .fixed_size([460.0, 180.0])
                .show(ctx, |ui| {
                    ui.add_space(8.0);
                    ui.label(RichText::new(&pending.prompt).color(Color32::WHITE).size(14.0));
                    ui.add_space(18.0);
                    ui.separator();
                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        if ui
                            .button(RichText::new(lang.confirm()).color(Color32::from_rgb(255, 90, 90)).strong())
                            .clicked()
                        {
                            let _ = self.model.confirm_pending_action();
                        }
                        if ui
                            .button(RichText::new(lang.cancel()).color(Color32::WHITE))
                            .clicked()
                        {
                            self.model.cancel_pending_action();
                        }
                    });
                });
        }
    }
}

impl eframe::App for SaveManagerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(left) = &mut self.frames_remaining {
            *left = left.saturating_sub(1);
            if *left == 0 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            } else {
                ctx.request_repaint();
            }
        }
        self.show(ctx);
    }
}

/// Standalone CLI/desktop entry runner.
pub fn run_standalone_entry() -> Result<(), String> {
    let mut game_path = None;
    let mut backup_path = None;
    let mut language = Language::TraditionalChinese;
    let mut smoke = false;
    let mut args = std::env::args_os().skip(1);

    while let Some(arg) = args.next() {
        let text = arg.to_string_lossy();
        match text.as_ref() {
            "--help" | "-h" => {
                println!(
                    "save-manager [--game <PATH>] [--backups <PATH>] [--lang <zh|en>] [--smoke-test]\n\
                     Standalone Save Manager for Against Rome saves and backups.\n\
                     --smoke-test executes headless egui UI updates without opening a window."
                );
                return Ok(());
            }
            "--version" | "-v" => {
                println!("against-rome-save-manager {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--game" | "-g" => {
                let next = args.next().ok_or_else(|| "--game requires a directory path".to_string())?;
                game_path = Some(next.to_string_lossy().into_owned());
            }
            "--backups" | "-b" => {
                let next = args.next().ok_or_else(|| "--backups requires a directory path".to_string())?;
                backup_path = Some(next.to_string_lossy().into_owned());
            }
            "--lang" | "-l" => {
                let next = args.next().ok_or_else(|| "--lang requires 'zh' or 'en'".to_string())?;
                let val = next.to_string_lossy();
                if val.eq_ignore_ascii_case("en") {
                    language = Language::English;
                } else {
                    language = Language::TraditionalChinese;
                }
            }
            "--smoke-test" => {
                smoke = true;
            }
            other => {
                if !other.starts_with('-') && game_path.is_none() {
                    game_path = Some(other.to_string());
                } else {
                    return Err(format!("Unknown option: {other}"));
                }
            }
        }
    }

    if smoke {
        let mut app = SaveManagerApp::new(game_path);
        if let Some(b) = backup_path {
            app.model.set_backup_path(b);
        }
        app.model.language = language;
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let _ = ctx.run(Default::default(), |ctx| {
                app.show(ctx);
            });
        }
        println!("Native desktop save-manager smoke test completed: three UI updates executed.");
        return Ok(());
    }

    let title = language.window_title();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size([1180.0, 780.0])
            .with_min_inner_size([960.0, 640.0]),
        ..Default::default()
    };

    eframe::run_native(
        title,
        native_options,
        Box::new(move |cc| {
            #[cfg(windows)]
            if let Some(windows) = std::env::var_os("WINDIR") {
                for name in ["msjh.ttc", "msyh.ttc"] {
                    if let Ok(bytes) = std::fs::read(PathBuf::from(&windows).join("Fonts").join(name)) {
                        let mut fonts = egui::FontDefinitions::default();
                        fonts.font_data.insert("system-cjk".into(), egui::FontData::from_owned(bytes));
                        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                            fonts.families.entry(family).or_default().push("system-cjk".into());
                        }
                        cc.egui_ctx.set_fonts(fonts);
                        break;
                    }
                }
            }
            #[cfg(not(windows))]
            let _ = cc;

            let mut app = SaveManagerApp::new(game_path);
            if let Some(b) = backup_path {
                app.model.set_backup_path(b);
            }
            app.model.language = language;
            Box::new(app)
        }),
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(prefix: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "arm-save-mgr-test-{prefix}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Creates a minimal valid 16x16 24-bit uncompressed true-color TGA file.
    fn create_test_tga(width: u16, height: u16) -> Vec<u8> {
        let mut tga = vec![0u8; 18];
        tga[2] = 2; // Uncompressed true-color
        tga[12..14].copy_from_slice(&width.to_le_bytes());
        tga[14..16].copy_from_slice(&height.to_le_bytes());
        tga[16] = 24; // 24 bpp (BGR)
        tga[17] = 0x20; // top-to-bottom

        let pixels = width as usize * height as usize;
        for _ in 0..pixels {
            tga.push(0x50); // B
            tga.push(0xA0); // G
            tga.push(0xF0); // R
        }
        tga
    }

    /// Generates a synthetic endless AI repair fixture payload matching native/against-rome-saves.
    fn endless_ai_fixture() -> Vec<u8> {
        const ORIGINAL: &[i32] = &[
            66, 0, 128, 72, 86, 102, 117, 48, 128, 83, 86, 81, 16, 96, 101, 117, 12, 71, 66, 1,
            117, 172, 66, 0, 91, 19, 81, 71, 102, 117, 52, 66, 120000, 66, 60000, 128, 16, 73,
            -2, 86, 91, 19, 112, 44, 66, 240000, 66, 120000, 128, 16, 73, -2, 86, 91, 19, 128,
            83, 86, 90, 19, 32, 82, 16, 120, -9748,
        ];
        const GATE: &[i32] = &[66, 0, 66, 0, 66, 0, 66, 0, 66, 0, 90, 6, 102, 117, 32];
        fn bytes(w: &[i32]) -> Vec<u8> {
            w.iter().flat_map(|v| v.to_le_bytes()).collect()
        }
        let mut w = vec![];
        for (upper, lower) in [
            (960000, 480000),
            (960000, 480000),
            (360000, 240000),
            (120000, 60000),
        ] {
            w.extend([66, upper, 66, lower, 128, 16, 999]);
        }
        w.extend(ORIGINAL);
        w.push(999);
        w.extend([90, 0, 66, 4, 96, 98, 91, 11]);
        w.extend(GATE);
        w.push(999);
        for i in 0..10 {
            w.extend([81, 56, 90, -3, 90, if i == 8 { 6 } else { 15 }, 164, 999]);
        }
        w.extend([128, 214, 73, -2, 86, 66, 1, 96, 102, 117, 92, 999]);
        w.extend([
            90, 42, 117, 56, 90, 41, 90, 4, 90, 3, 128, 141, 73, -3, 86, 71, 112, 116,
        ]);
        let mut bci = vec![0; 36];
        bci[..4].copy_from_slice(b"BCI0");
        bci[8..12].copy_from_slice(&((w.len() * 4) as i32).to_le_bytes());
        bci.extend(bytes(&w));
        bci.extend(b"SYMBCONS\0unknown-save-tail\xff");
        let mut b = b"opaque prefix\0MAPS/ENDL_003/SCRIPT/ak_level.bci\0".to_vec();
        b.extend(bci);
        let mut pfil_bytes = vec![0x5a; 64];
        pfil_bytes[..4].copy_from_slice(b"PFIL");
        pfil_bytes[16..20].copy_from_slice(&(b.len() as u32).to_le_bytes());
        for chunk in b.chunks(8) {
            pfil_bytes.push(((1u16 << chunk.len()) - 1) as u8);
            pfil_bytes.extend(chunk);
        }
        pfil_bytes
    }

    #[test]
    fn test_save_manager_scan_and_list_parity() {
        let t = TempDir::new("scan");
        let game = t.0.join("game");
        let save_dir = game.join("SAVE/SAVE_001");
        fs::create_dir_all(&save_dir).unwrap();
        fs::write(
            save_dir.join("save.ini"),
            b"[titel]\nCampaign Victory\n[orglevelname]\nENDL_002\n",
        )
        .unwrap();
        fs::write(save_dir.join("data.bin"), b"test-save-data").unwrap();

        let mut model = SaveManagerModel::new(Some(game.display().to_string()));
        model.set_backup_path(t.0.join("SavesBackup").display().to_string());
        model.rescan().unwrap();

        let cat = model.catalog.as_ref().expect("Catalog should exist");
        assert_eq!(cat.saves.len(), 1);
        let s = &cat.saves[0];
        assert_eq!(s.folder, "SAVE_001");
        assert_eq!(s.title, "Campaign Victory");
        assert_eq!(s.level, "ENDL_002");
        assert!(s.parsed);
        assert_eq!(cat.backups.len(), 0);
    }

    #[test]
    fn test_save_manager_backup_creation_and_manifest() {
        let t = TempDir::new("backup");
        let game = t.0.join("game");
        let save_dir = game.join("SAVE/SAVE_001");
        fs::create_dir_all(&save_dir).unwrap();
        fs::write(
            save_dir.join("save.ini"),
            b"[titel]\nBattle of Cannae\n[orglevelname]\nENDL_001\n",
        )
        .unwrap();
        fs::write(save_dir.join("content.dat"), b"save-content").unwrap();

        let mut model = SaveManagerModel::new(Some(game.display().to_string()));
        let backup_dir = t.0.join("SavesBackup");
        model.set_backup_path(backup_dir.display().to_string());
        model.rescan().unwrap();

        // Select live save and request backup
        model.select_live_save("SAVE_001");
        let res = model.request_backup_save();
        assert!(res.is_ok());

        let cat = model.catalog.as_ref().unwrap();
        assert_eq!(cat.backups.len(), 1);
        let b = &cat.backups[0];
        assert!(b.file_name.starts_with("Backup_SAVE_001_"));
        assert_eq!(b.title, "Battle of Cannae");
        assert_eq!(b.level, "ENDL_001");
        assert_eq!(b.orig_folder, "SAVE_001");
        assert!(backup_dir.join(&b.file_name).is_file());
    }

    #[test]
    fn test_save_manager_tga_preview_decoding() {
        let t = TempDir::new("preview");
        let game = t.0.join("game");
        let save_dir = game.join("SAVE/SAVE_PREVIEW");
        fs::create_dir_all(&save_dir).unwrap();
        fs::write(
            save_dir.join("save.ini"),
            b"[titel]\nThumbnail Test\n[orglevelname]\nENDL_000\n",
        )
        .unwrap();
        let tga_bytes = create_test_tga(20, 15);
        fs::write(save_dir.join("savepic.tga"), &tga_bytes).unwrap();

        let mut model = SaveManagerModel::new(Some(game.display().to_string()));
        model.set_backup_path(t.0.join("SavesBackup").display().to_string());
        model.rescan().unwrap();

        model.select_live_save("SAVE_PREVIEW");
        assert!(model.preview_image.is_some());
        let img = model.preview_image.as_ref().unwrap();
        assert_eq!(img.size, [20, 15]);
        assert_eq!(img.pixels.len(), 300);

        // Verify detail string formatting
        assert!(model.detail_text.contains("Thumbnail Test"));
        assert!(model.detail_text.contains("SAVE_PREVIEW"));
        assert!(model.detail_text.contains("ENDL_000"));
    }

    #[test]
    fn test_save_manager_restore_backup_with_confirmation() {
        let t = TempDir::new("restore");
        let game = t.0.join("game");
        let save_dir = game.join("SAVE/SAVE_001");
        fs::create_dir_all(&save_dir).unwrap();
        fs::write(
            save_dir.join("save.ini"),
            b"[titel]\nOriginal Title\n[orglevelname]\nENDL_005\n",
        )
        .unwrap();
        fs::write(save_dir.join("state.dat"), b"original-state").unwrap();

        let mut model = SaveManagerModel::new(Some(game.display().to_string()));
        model.set_backup_path(t.0.join("SavesBackup").display().to_string());
        model.rescan().unwrap();

        // 1. Create backup
        model.select_live_save("SAVE_001");
        model.request_backup_save().unwrap();
        let backup_file = model.catalog.as_ref().unwrap().backups[0].file_name.clone();

        // 2. Modify live save to simulate corruption
        fs::write(save_dir.join("state.dat"), b"corrupted-tampered-data").unwrap();

        // 3. Request restore (must set confirmation)
        model.select_backup(&backup_file);
        model.request_restore_backup();
        assert!(model.pending_confirmation.is_some());
        let pending = model.pending_confirmation.as_ref().unwrap();
        assert!(matches!(pending.action, ConfirmedAction::RestoreBackup { .. }));
        assert!(pending.prompt.contains("覆蓋") || pending.prompt.contains("Overwrite"));

        // 4. Cancel pending action leaves corrupted state untouched
        model.cancel_pending_action();
        assert!(model.pending_confirmation.is_none());
        assert_eq!(fs::read(save_dir.join("state.dat")).unwrap(), b"corrupted-tampered-data");

        // 5. Confirm pending action restores original state
        model.request_restore_backup();
        let res = model.confirm_pending_action();
        assert!(res.is_ok());
        assert_eq!(fs::read(save_dir.join("state.dat")).unwrap(), b"original-state");
    }

    #[test]
    fn test_save_manager_delete_save_with_confirmation() {
        let t = TempDir::new("delete_save");
        let game = t.0.join("game");
        let save_dir = game.join("SAVE/SAVE_DEL");
        fs::create_dir_all(&save_dir).unwrap();
        fs::write(
            save_dir.join("save.ini"),
            b"[titel]\nTo Delete\n[orglevelname]\nENDL_000\n",
        )
        .unwrap();

        let mut model = SaveManagerModel::new(Some(game.display().to_string()));
        model.set_backup_path(t.0.join("SavesBackup").display().to_string());
        model.rescan().unwrap();

        model.select_live_save("SAVE_DEL");
        model.request_delete_save();
        assert!(model.pending_confirmation.is_some());
        let pending = model.pending_confirmation.as_ref().unwrap();
        assert_eq!(
            pending.action,
            ConfirmedAction::DeleteSave {
                folder: "SAVE_DEL".into()
            }
        );

        // Cancel preserves directory
        model.cancel_pending_action();
        assert!(save_dir.is_dir());

        // Confirm deletes directory
        model.request_delete_save();
        let res = model.confirm_pending_action();
        assert!(res.is_ok());
        assert!(!save_dir.exists());
        assert!(model.selected_live_save.is_none());
    }

    #[test]
    fn test_save_manager_delete_backup_with_confirmation() {
        let t = TempDir::new("delete_backup");
        let game = t.0.join("game");
        let save_dir = game.join("SAVE/SAVE_001");
        fs::create_dir_all(&save_dir).unwrap();
        fs::write(
            save_dir.join("save.ini"),
            b"[titel]\nTo Backup & Delete\n[orglevelname]\nENDL_000\n",
        )
        .unwrap();

        let mut model = SaveManagerModel::new(Some(game.display().to_string()));
        let backup_dir = t.0.join("SavesBackup");
        model.set_backup_path(backup_dir.display().to_string());
        model.rescan().unwrap();

        model.select_live_save("SAVE_001");
        model.request_backup_save().unwrap();
        let b_file = model.catalog.as_ref().unwrap().backups[0].file_name.clone();

        model.select_backup(&b_file);
        model.request_delete_backup();
        assert!(model.pending_confirmation.is_some());
        let pending = model.pending_confirmation.as_ref().unwrap();
        assert_eq!(
            pending.action,
            ConfirmedAction::DeleteBackup {
                file_name: b_file.clone()
            }
        );

        // Cancel preserves archive
        model.cancel_pending_action();
        assert!(backup_dir.join(&b_file).is_file());

        // Confirm deletes archive
        model.request_delete_backup();
        let res = model.confirm_pending_action();
        assert!(res.is_ok());
        assert!(!backup_dir.join(&b_file).exists());
        assert!(model.selected_backup.is_none());
    }

    #[test]
    fn test_save_manager_endless_ai_repair_flow() {
        let t = TempDir::new("repair_ai");
        let game = t.0.join("game");
        let save_dir = game.join("SAVE/SAVE_AI");
        fs::create_dir_all(save_dir.join("CLAK")).unwrap();
        fs::write(
            save_dir.join("save.ini"),
            b"[titel]\nEndless Match\n[orglevelname]\nENDL_003\n",
        )
        .unwrap();
        let raw = endless_ai_fixture();
        fs::write(save_dir.join("CLAK/scr.dat"), &raw).unwrap();

        let mut model = SaveManagerModel::new(Some(game.display().to_string()));
        let backup_dir = t.0.join("SavesBackup");
        model.set_backup_path(backup_dir.display().to_string());
        model.rescan().unwrap();

        model.select_live_save("SAVE_AI");
        model.request_repair_endless_ai();
        assert!(model.pending_confirmation.is_some());
        let pending = model.pending_confirmation.as_ref().unwrap();
        assert_eq!(
            pending.action,
            ConfirmedAction::RepairEndlessAi {
                folder: "SAVE_AI".into()
            }
        );

        // Cancel preserves original scr.dat
        model.cancel_pending_action();
        assert_eq!(fs::read(save_dir.join("CLAK/scr.dat")).unwrap(), raw);

        // Confirm executes repair and creates pre-repair safety backup
        model.request_repair_endless_ai();
        let res = model.confirm_pending_action().unwrap();
        assert!(res.contains("成功") || res.contains("successfully"));
        let repaired_scr = fs::read(save_dir.join("CLAK/scr.dat")).unwrap();
        assert_ne!(repaired_scr, raw);

        // Safety backup exists in backups list
        let cat = model.catalog.as_ref().unwrap();
        assert_eq!(cat.backups.len(), 1);

        // Running repair a second time reports AlreadyRepaired without creating additional backup
        model.select_live_save("SAVE_AI");
        model.request_repair_endless_ai();
        let res2 = model.confirm_pending_action().unwrap();
        assert!(res2.contains("已包含") || res2.contains("already contains"));
        assert_eq!(model.catalog.as_ref().unwrap().backups.len(), 1);
    }

    #[test]
    fn action_errors_are_visible_in_status() {
        let t = TempDir::new("action-errors");
        let game = t.0.join("game");
        make_game(&game, "SAVE_001");
        let mut model = SaveManagerModel::new(Some(game.display().to_string()));
        model.set_backup_path(t.0.join("SavesBackup").display().to_string());

        // Backup with nothing selected.
        model.status_message.clear();
        model.status_is_error = false;
        assert!(model.request_backup_save().is_err());
        assert!(model.status_is_error);
        assert!(!model.status_message.is_empty());

        // Confirm without a pending action.
        model.status_message.clear();
        model.status_is_error = false;
        assert!(model.confirm_pending_action().is_err());
        assert!(model.status_is_error);
        assert!(!model.status_message.is_empty());

        // Service failure: restore a backup that does not exist.
        model.status_message.clear();
        model.status_is_error = false;
        model.pending_confirmation = Some(PendingConfirmation {
            title: String::new(),
            prompt: String::new(),
            action: ConfirmedAction::RestoreBackup {
                file_name: "missing.zip".into(),
                target_folder: "SAVE_001".into(),
            },
        });
        let err = model.confirm_pending_action().unwrap_err();
        assert!(model.status_is_error);
        assert_eq!(model.status_message, err);
    }

    #[test]
    fn rescan_failure_after_successful_action_stays_visible() {
        let t = TempDir::new("rescan-after");
        let game = t.0.join("game");
        make_game(&game, "SAVE_001");
        let mut model = SaveManagerModel::new(Some(game.display().to_string()));
        model.set_backup_path(t.0.join("SavesBackup").display().to_string());
        model.select_live_save("SAVE_001");
        model.request_backup_save().unwrap();
        assert!(!model.status_is_error);

        let msg = model.finish_action("done".into());
        assert!(!model.status_is_error);
        assert_eq!(msg, "done");

        // A backup root that is a regular file makes the follow-up rescan fail.
        let bad_backup = t.0.join("backups-file");
        fs::write(&bad_backup, b"not a directory").unwrap();
        model.backup_path = bad_backup.display().to_string();
        let msg = model.finish_action("done".into());
        assert!(model.status_is_error);
        assert!(msg.starts_with("done ("));
        assert_eq!(model.status_message, msg);
    }

    #[test]
    fn test_save_manager_safety_guards() {
        assert!(check_explicit_path("").is_err());
        assert!(check_explicit_path("relative/path").is_err());
        assert!(check_explicit_path("C:\\game\\..\\escape").is_err());
        assert!(check_writable_path(Path::new("C:\\Program Files (x86)\\Against Rome")).is_err());
        assert!(check_writable_path(Path::new("C:\\Program Files\\Against Rome")).is_err());

        assert!(SaveBackupService::is_simple_name("SAVE_001"));
        assert!(!SaveBackupService::is_simple_name(""));
        assert!(!SaveBackupService::is_simple_name(".."));
        assert!(!SaveBackupService::is_simple_name("CON"));
        assert!(!SaveBackupService::is_simple_name("path/traversal"));
        assert!(!SaveBackupService::is_simple_name("path\\traversal"));
    }

    #[test]
    fn test_save_manager_bilingual_localization() {
        let zh = Language::TraditionalChinese;
        let en = Language::English;

        assert_eq!(zh.main_title(), "AGAINST ROME 存檔管理器");
        assert_eq!(en.main_title(), "AGAINST ROME SAVE MANAGER");
        assert_eq!(zh.game_saves_title(), "遊戲中存檔列表");
        assert_eq!(en.game_saves_title(), "In-Game Save List");
        assert_eq!(zh.backups_title(), "備份歷史列表");
        assert_eq!(en.backups_title(), "Backup History List");
        assert_eq!(zh.detail_title(), "存檔詳細與預覽");
        assert_eq!(en.detail_title(), "Save Details & Preview");

        let detail_zh = zh.format_game_save_detail("DIR", "標題", "關卡", "2026-10-18");
        assert!(detail_zh.contains("存檔類型: 遊戲存檔"));
        assert!(detail_zh.contains("DIR"));

        let detail_en = en.format_game_save_detail("DIR", "Title", "Level", "2026-10-18");
        assert!(detail_en.contains("Save Type: Game Save"));
        assert!(detail_en.contains("DIR"));
    }

    #[test]
    fn unparsed_metadata_stays_visible_with_warning() {
        use against_rome_modifier::save_manager::{GameSaveInfo, SaveBackupCatalog, SaveBackupInfo};

        let zh = Language::TraditionalChinese;
        assert_eq!(zh.title_display(true, "T"), "T");
        assert_eq!(zh.title_display(false, ""), "無法解析");
        assert_eq!(zh.level_display(false, " "), "未知");
        let shown = zh.title_display(false, "Cannae");
        assert!(shown.starts_with("Cannae") && shown.contains("save.ini 未完整解析"));

        let mut model = SaveManagerModel::new(None);
        model.catalog = Some(SaveBackupCatalog {
            saves: vec![GameSaveInfo {
                folder: "SAVE_001".into(),
                title: "Cannae".into(),
                level: "ENDL_001".into(),
                last_write_time: "2026-10-19T00:00:00".into(),
                parsed: false,
            }],
            backups: vec![SaveBackupInfo {
                file_name: "b.zip".into(),
                title: "Zama".into(),
                level: "ENDL_002".into(),
                backup_time: "2026-10-18 12:00:00".into(),
                orig_folder: "SAVE_007".into(),
                last_write_time: String::new(),
                parsed: false,
            }],
        });

        model.select_live_save("SAVE_001");
        for want in ["SAVE_001", "Cannae", "ENDL_001", "2026-10-19T00:00:00", "未完整解析"] {
            assert!(model.detail_text.contains(want), "{want} missing: {}", model.detail_text);
        }
        assert!(!model.detail_text.contains("無法解析"));

        model.select_backup("b.zip");
        for want in ["b.zip", "SAVE_007", "Zama", "ENDL_002", "2026-10-18 12:00:00", "未完整解析"] {
            assert!(model.detail_text.contains(want), "{want} missing: {}", model.detail_text);
        }
    }

    #[test]
    fn test_save_manager_headless_smoke_frames() {
        let t = TempDir::new("headless_smoke");
        let game = t.0.join("game");
        let save_dir = game.join("SAVE/SAVE_SMOKE");
        fs::create_dir_all(&save_dir).unwrap();
        fs::write(
            save_dir.join("save.ini"),
            b"[titel]\nSmoke Save\n[orglevelname]\nENDL_000\n",
        )
        .unwrap();

        let mut app = SaveManagerApp::new(Some(game.display().to_string()));
        app.model.set_backup_path(t.0.join("SavesBackup").display().to_string());
        app.model.language = Language::TraditionalChinese;

        let ctx = egui::Context::default();
        for _ in 0..3 {
            let _ = ctx.run(Default::default(), |ctx| {
                app.show(ctx);
            });
        }

        assert_eq!(app.model.catalog.as_ref().map(|c| c.saves.len()), Some(1));
    }

    fn make_game(root: &Path, folder: &str) {
        let f = root.join("SAVE").join(folder);
        fs::create_dir_all(&f).unwrap();
        fs::write(f.join("save.ini"), b"[titel]\nPicked\n[orglevelname]\nENDL_000\n").unwrap();
    }

    #[test]
    fn game_folder_picker_success_updates_path_backup_root_and_rescans() {
        let t = TempDir::new("pick-ok");
        let old_game = t.0.join("old");
        let new_game = t.0.join("new");
        make_game(&old_game, "SAVE_OLD");
        make_game(&new_game, "SAVE_NEW");
        let mut app = SaveManagerApp::new(Some(old_game.display().to_string()));
        app.model.select_live_save("SAVE_OLD");
        let picked = new_game.clone();
        app.game_folder_picker = Some(Box::new(move || Some(picked.clone())));
        app.browse_game_folder();
        assert_eq!(app.model.game_path, new_game.to_string_lossy());
        assert_eq!(
            app.model.backup_path,
            new_game.join("SavesBackup").to_string_lossy()
        );
        let cat = app.model.catalog.as_ref().expect("catalog refreshed");
        assert_eq!(cat.saves.len(), 1);
        assert_eq!(cat.saves[0].folder, "SAVE_NEW");
        assert!(app.model.selected_live_save.is_none());
    }

    #[test]
    fn game_folder_picker_cancel_and_invalid_selection_preserve_state() {
        let t = TempDir::new("pick-keep");
        let game = t.0.join("game");
        make_game(&game, "SAVE_001");
        let mut app = SaveManagerApp::new(Some(game.display().to_string()));
        app.model.select_live_save("SAVE_001");
        app.model.status_message = "prior".into();
        app.model.status_is_error = false;
        let backup = app.model.backup_path.clone();

        app.game_folder_picker = Some(Box::new(|| None));
        app.browse_game_folder();
        assert_eq!(app.model.game_path, game.to_string_lossy());
        assert_eq!(app.model.backup_path, backup);
        assert!(app.model.catalog.is_some());
        assert_eq!(app.model.selected_live_save.as_deref(), Some("SAVE_001"));
        assert_eq!(app.model.status_message, "prior");
        assert!(!app.model.status_is_error);

        let missing = t.0.join("missing-game-folder");
        app.game_folder_picker = Some(Box::new(move || Some(missing.clone())));
        app.browse_game_folder();
        assert_eq!(app.model.game_path, game.to_string_lossy());
        assert_eq!(app.model.backup_path, backup);
        assert_eq!(app.model.catalog.as_ref().unwrap().saves.len(), 1);
        assert_eq!(app.model.selected_live_save.as_deref(), Some("SAVE_001"));
        assert!(app.model.status_is_error);
        assert_ne!(app.model.status_message, "prior");
    }

    #[test]
    fn game_folder_picker_rescan_failure_is_reported_in_status() {
        let t = TempDir::new("pick-fail");
        let old_game = t.0.join("old");
        let game = t.0.join("game");
        make_game(&old_game, "SAVE_OLD");
        fs::create_dir_all(&game).unwrap();
        let mut app = SaveManagerApp::new(Some(old_game.display().to_string()));
        // Non-default backup root that is a regular file, so the rescan is rejected.
        let bad_backup = t.0.join("backups-file");
        fs::write(&bad_backup, b"not a directory").unwrap();
        app.model.backup_path = bad_backup.display().to_string();
        app.model.status_message.clear();
        app.model.status_is_error = false;
        let picked = game.clone();
        app.game_folder_picker = Some(Box::new(move || Some(picked.clone())));
        app.browse_game_folder();
        assert_eq!(app.model.game_path, game.to_string_lossy());
        assert!(app.model.status_is_error);
        assert!(!app.model.status_message.is_empty());
    }

    #[test]
    fn game_folder_picker_same_path_forces_rescan_and_clears_selection() {
        let t = TempDir::new("pick-same");
        let game = t.0.join("game");
        make_game(&game, "SAVE_001");
        let mut app = SaveManagerApp::new(Some(game.display().to_string()));
        app.model.select_live_save("SAVE_001");
        let bad_backup = t.0.join("backups-file");
        fs::write(&bad_backup, b"not a directory").unwrap();
        app.model.backup_path = bad_backup.display().to_string();
        app.model.status_message.clear();
        app.model.status_is_error = false;
        let picked = game.clone();
        app.game_folder_picker = Some(Box::new(move || Some(picked.clone())));
        app.browse_game_folder();
        assert_eq!(app.model.game_path, game.to_string_lossy());
        assert!(app.model.selected_live_save.is_none());
        assert!(app.model.status_is_error);
        assert!(!app.model.status_message.is_empty());
    }
}
