//! Native desktop tools. Reviews bind exact paths and source bytes to explicit confirmation.
use against_rome_modifier::{self as modifier, State, FOCUS, ROMAN_ENDLESS, UNIT_RECRUIT_20};
use against_rome_saves::{Catalog, SaveService, VerifyReport};
use eframe::egui;
use std::{
    fs,
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, String>;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
const PATCHES: [modifier::Patch; 3] = [FOCUS, ROMAN_ENDLESS, UNIT_RECRUIT_20];
pub type GameFolderPicker = Box<dyn Fn() -> Option<PathBuf> + Send + Sync>;
fn native_game_folder_dialog(current_path: &str) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new().set_title("Select Against Rome Game Folder");
    let current = Path::new(current_path.trim());
    if current.is_dir() {
        dialog = dialog.set_directory(current);
    }
    dialog.pick_folder()
}
fn explicit(s: &str) -> Result<PathBuf> {
    let p = PathBuf::from(s.trim());
    if s.trim().is_empty()
        || !p.is_absolute()
        || p.components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("請輸入完整絕對路徑，不接受空白或 ..".into());
    }
    for a in p.ancestors() {
        match fs::symlink_metadata(a) {
            Ok(m) => {
                if m.file_type().is_symlink() {
                    return Err("拒絕 symbolic link".into());
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if m.file_attributes() & 0x400 != 0 {
                        return Err("拒絕 junction/reparse point".into());
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(err(e)),
        }
    }
    Ok(p)
}
fn writable(p: &Path) -> Result<()> {
    let text = p.to_string_lossy().replace('/', "\\").to_lowercase();
    if text
        .split('\\')
        .any(|c| c == "program files" || c == "program files (x86)")
    {
        return Err("禁止寫入 Program Files；請先複製到工作資料夾".into());
    }
    for key in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Some(root) = std::env::var_os(key) {
            let root = PathBuf::from(root);
            if p.starts_with(&root) {
                return Err("禁止寫入安裝目錄；請操作副本".into());
            }
        }
    }
    Ok(())
}
#[derive(Debug, PartialEq, Eq)]
struct Snapshot(Vec<(PathBuf, Vec<u8>)>);
fn snapshot(p: &Path) -> Result<Snapshot> {
    fn walk(p: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) -> Result<()> {
        explicit(&p.to_string_lossy())?;
        if !p.exists() {
            return Ok(());
        }
        if p.is_dir() {
            out.push((p.to_path_buf(), Vec::new()));
            let mut entries = fs::read_dir(p)
                .map_err(err)?
                .map(|e| e.map(|e| e.path()))
                .collect::<std::io::Result<Vec<_>>>()
                .map_err(err)?;
            entries.sort();
            for e in entries {
                walk(&e, out)?;
            }
        } else {
            out.push((p.to_path_buf(), fs::read(p).map_err(err)?));
        }
        Ok(())
    }
    let mut v = vec![];
    walk(p, &mut v)?;
    Ok(Snapshot(v))
}
struct PatchReview {
    exe: String,
    backup: String,
    enabled: [bool; 3],
    original: Vec<u8>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveAction {
    Backup,
    Restore,
    DeleteSave,
    DeleteBackup,
    RepairAi,
}
struct SaveReview {
    game: String,
    backups: String,
    source: String,
    folder: String,
    title: String,
    level: String,
    action: SaveAction,
    paths: Vec<(PathBuf, Snapshot)>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ToolboxTab {
    #[default]
    Modifier,
    SaveManager,
}

fn format_verify_report(target: &str, report: &VerifyReport) -> String {
    let mut s = format!(
        "驗證報告 [{}]: 狀態={}, 項目數(entries)={}, 總大小(total_bytes)={} bytes, 標題(title)='{}', 關卡(level)='{}', 錯誤(errors)={}, 警告(warnings)={}",
        target,
        if report.is_ok() { "通過 (OK)" } else { "異常 (FAIL)" },
        report.entries,
        report.total_bytes,
        report.title,
        report.level,
        report.errors.len(),
        report.warnings.len(),
    );
    if !report.errors.is_empty() {
        s.push_str("\n錯誤列表 (errors):");
        for (i, e) in report.errors.iter().enumerate() {
            s.push_str(&format!("\n  [{}] {}", i + 1, e));
        }
    }
    if !report.warnings.is_empty() {
        s.push_str("\n警告列表 (warnings):");
        for (i, w) in report.warnings.iter().enumerate() {
            s.push_str(&format!("\n  [{}] {}", i + 1, w));
        }
    }
    s
}

fn clone_verify_report(r: &VerifyReport) -> VerifyReport {
    VerifyReport {
        entries: r.entries,
        total_bytes: r.total_bytes,
        title: r.title.clone(),
        level: r.level.clone(),
        errors: r.errors.clone(),
        warnings: r.warnings.clone(),
    }
}
#[derive(Default)]
pub struct ToolboxUi {
    pub open: bool,
    pub current_tab: ToolboxTab,
    exe: String,
    patch_backup: String,
    states: Option<[State; 3]>,
    enabled: [bool; 3],
    patch_review: Option<PatchReview>,
    game: String,
    backups: String,
    catalog: Option<Catalog>,
    selected_save: Option<String>,
    selected_backup: Option<String>,
    title: String,
    restore_folder: String,
    save_review: Option<SaveReview>,
    pub last_verify_report: Option<(String, VerifyReport)>,
    pub status: String,
    /// Injectable for UI tests; the desktop default opens the native folder picker.
    pub game_folder_picker: Option<GameFolderPicker>,
}
impl ToolboxUi {
    pub fn status(&self) -> &str {
        &self.status
    }
    pub fn open_modifier(&mut self) {
        self.current_tab = ToolboxTab::Modifier;
        self.open = true;
    }
    pub fn open_save_manager(&mut self) {
        self.current_tab = ToolboxTab::SaveManager;
        self.open = true;
    }
    pub fn is_dirty(&self) -> bool {
        self.patch_review.is_some() || self.save_review.is_some()
    }
    fn inspect(&mut self) -> Result<()> {
        self.patch_review = None;
        self.states = None;
        let b = fs::read(explicit(&self.exe)?).map_err(err)?;
        let states = PATCHES.map(|p| p.detect(&b));
        self.enabled = states.map(|s| s == State::Patched);
        self.states = Some(states);
        Ok(())
    }
    fn review_patch(&mut self) -> Result<()> {
        self.patch_review = None;
        let p = explicit(&self.exe)?;
        let backup = explicit(&self.patch_backup)?;
        writable(&p)?;
        writable(&backup)?;
        if backup.exists() {
            return Err("備份路徑必須是尚未存在的新檔案".into());
        }
        let original = fs::read(p).map_err(err)?;
        modifier::plan(&original, &self.selections()).map_err(err)?;
        self.patch_review = Some(PatchReview {
            exe: self.exe.clone(),
            backup: self.patch_backup.clone(),
            enabled: self.enabled,
            original,
        });
        Ok(())
    }
    fn selections(&self) -> Vec<(modifier::Patch, bool)> {
        PATCHES.into_iter().zip(self.enabled).collect()
    }
    fn apply_patch(&mut self) -> Result<()> {
        let r = self.patch_review.take().ok_or("請先 Review")?;
        if r.exe != self.exe || r.backup != self.patch_backup || r.enabled != self.enabled {
            return Err("路徑或選項已變更，請重新 Review".into());
        }
        let p = explicit(&r.exe)?;
        let b = explicit(&r.backup)?;
        writable(&p)?;
        writable(&b)?;
        if fs::read(&p).map_err(err)? != r.original {
            return Err("EXE 已變更，拒絕套用；請重新 Inspect / Review".into());
        }
        modifier::apply_file(&p, &b, &self.selections()).map_err(err)?;
        self.inspect()
    }
    fn service(&self) -> Result<SaveService> {
        SaveService::new(explicit(&self.game)?, explicit(&self.backups)?).map_err(err)
    }
    /// Opens the native folder picker (C# `btnBrowseGamePath.Click`). Cancelling or an
    /// invalid selection preserves the current path and catalog; a valid selection
    /// updates the Game root and rescans saves and backups.
    pub fn browse_game_folder(&mut self) {
        let selected = if let Some(picker) = &self.game_folder_picker {
            picker()
        } else {
            native_game_folder_dialog(&self.game)
        };
        let Some(path) = selected else { return };
        if !path.is_dir() {
            self.status = format!("Selected game folder does not exist: {}", path.display());
            return;
        }
        self.game = path.to_string_lossy().into_owned();
        let r = self.scan();
        self.report(r);
    }
    fn scan(&mut self) -> Result<()> {
        self.catalog = None;
        self.selected_save = None;
        self.selected_backup = None;
        self.save_review = None;
        self.last_verify_report = None;
        self.title.clear();
        self.restore_folder.clear();
        writable(&explicit(&self.backups)?)?;
        self.catalog = Some(self.service()?.scan().map_err(err)?);
        Ok(())
    }
    pub fn review_save(&mut self, action: SaveAction) -> Result<()> {
        self.save_review = None;
        let game = explicit(&self.game)?;
        let backups = explicit(&self.backups)?;
        writable(&backups)?;
        let catalog = self.service()?.scan().map_err(err)?;
        let (source, folder, title, level, paths) = match action {
            SaveAction::Backup | SaveAction::DeleteSave | SaveAction::RepairAi => {
                let name = self.selected_save.as_ref().ok_or("請選擇存檔")?;
                let info = catalog
                    .saves
                    .iter()
                    .find(|s| &s.folder == name)
                    .ok_or("存檔已消失，請重新掃描")?;
                let p = game.join("SAVE").join(name);
                if action == SaveAction::Backup {
                    writable(&backups)?;
                } else if action == SaveAction::RepairAi {
                    writable(&backups)?;
                    writable(&p)?;
                    let scr = p.join("CLAK").join("scr.dat");
                    if !scr.is_file() {
                        return Err("存檔缺少 CLAK/scr.dat，無法進行 AI 修復".into());
                    }
                } else {
                    writable(&p)?;
                }
                (
                    name.clone(),
                    name.clone(),
                    self.title.clone(),
                    info.level.clone(),
                    vec![p],
                )
            }
            SaveAction::Restore | SaveAction::DeleteBackup => {
                let name = self.selected_backup.as_ref().ok_or("請選擇備份")?;
                let info = catalog
                    .backups
                    .iter()
                    .find(|s| &s.file_name == name)
                    .ok_or("備份已消失，請重新掃描")?;
                let mut paths = vec![backups.join(name)];
                if action == SaveAction::Restore {
                    if !info.parsed || !against_rome_saves::simple_name(&self.restore_folder) {
                        return Err("備份不支援或還原資料夾名稱無效".into());
                    }
                    let dest = game.join("SAVE").join(&self.restore_folder);
                    writable(&dest)?;
                    paths.push(dest);
                } else {
                    writable(&paths[0])?;
                }
                (
                    name.clone(),
                    self.restore_folder.clone(),
                    info.manifest.title.clone(),
                    info.manifest.level.clone(),
                    paths,
                )
            }
        };
        let paths = paths
            .into_iter()
            .map(|p| snapshot(&p).map(|s| (p, s)))
            .collect::<Result<Vec<_>>>()?;
        self.save_review = Some(SaveReview {
            game: self.game.clone(),
            backups: self.backups.clone(),
            source,
            folder,
            title,
            level,
            action,
            paths,
        });
        Ok(())
    }
    pub fn apply_save(&mut self) -> Result<String> {
        let r = self.save_review.take().ok_or("請先 Review")?;
        if r.game != self.game || r.backups != self.backups {
            return Err("Root 已變更，請重新掃描".into());
        }
        if matches!(r.action, SaveAction::Backup | SaveAction::DeleteSave | SaveAction::RepairAi)
            && self.selected_save.as_ref() != Some(&r.source)
            || matches!(r.action, SaveAction::Restore | SaveAction::DeleteBackup)
                && self.selected_backup.as_ref() != Some(&r.source)
            || r.action == SaveAction::Restore && r.folder != self.restore_folder
            || r.action == SaveAction::Backup && r.title != self.title
        {
            return Err("選擇或內容已變更，請重新 Review".into());
        }
        for (p, s) in &r.paths {
            if snapshot(p)? != *s {
                return Err("Review 後來源或目標已變更，拒絕執行".into());
            }
        }
        let service = self.service()?;
        match r.action {
            SaveAction::Backup => {
                writable(&explicit(&r.backups)?)?;
                service.backup(&r.source, &r.title, &r.level).map_err(err)?;
                self.scan()?;
                let msg = "完成".to_string();
                self.status = msg.clone();
                Ok(msg)
            }
            SaveAction::Restore => {
                writable(&explicit(&r.game)?.join("SAVE").join(&r.folder))?;
                let report = service.restore(&r.source, &r.folder).map_err(err)?;
                self.scan()?;
                if !report.cleanup_warnings.is_empty() {
                    return Err(format!(
                        "還原已完成，但清理需要注意：{:?}",
                        report.cleanup_warnings
                    ));
                }
                let msg = "完成".to_string();
                self.status = msg.clone();
                Ok(msg)
            }
            SaveAction::DeleteSave => {
                writable(&explicit(&r.game)?.join("SAVE").join(&r.source))?;
                service.delete_save(&r.source).map_err(err)?;
                self.scan()?;
                let msg = "完成".to_string();
                self.status = msg.clone();
                Ok(msg)
            }
            SaveAction::DeleteBackup => {
                writable(&explicit(&r.backups)?.join(&r.source))?;
                service.delete_backup(&r.source).map_err(err)?;
                self.scan()?;
                let msg = "完成".to_string();
                self.status = msg.clone();
                Ok(msg)
            }
            SaveAction::RepairAi => {
                writable(&explicit(&r.backups)?)?;
                writable(&explicit(&r.game)?.join("SAVE").join(&r.source))?;
                let outcome = service.repair_endless_ai(&r.source).map_err(err)?;
                self.scan()?;
                let msg = match outcome {
                    against_rome_saves::AiRepairResult::AlreadyRepaired => {
                        "無盡戰役 AI 腳本已處於修復狀態，無需變更 (AlreadyRepaired)".to_string()
                    }
                    against_rome_saves::AiRepairResult::Changed { backup } => {
                        format!(
                            "無盡戰役 AI 腳本修復成功 (Changed)，安全備份已建立：{}",
                            backup
                        )
                    }
                };
                self.status = msg.clone();
                Ok(msg)
            }
        }
    }
    pub fn verify_selected_save(&mut self) -> Result<VerifyReport> {
        let game = explicit(&self.game)?;
        let _backups = explicit(&self.backups)?;
        let name = self.selected_save.as_ref().ok_or("請選擇存檔")?.clone();
        let catalog = self.catalog.as_ref().ok_or("尚未掃描存檔目錄，請先掃描")?;
        if !catalog.saves.iter().any(|s| s.folder == name) {
            return Err("存檔已消失，請重新掃描".into());
        }
        let save_path = game.join("SAVE").join(&name);
        if !save_path.is_dir() {
            return Err("存檔已消失，請重新掃描".into());
        }
        let service = self.service()?;
        let report = service.verify_save(&name).map_err(err)?;
        let msg = format_verify_report(&format!("存檔 {}", name), &report);
        self.last_verify_report = Some((format!("存檔 {}", name), clone_verify_report(&report)));
        self.status = msg;
        Ok(report)
    }
    pub fn verify_selected_backup(&mut self) -> Result<VerifyReport> {
        let _game = explicit(&self.game)?;
        let backups = explicit(&self.backups)?;
        let name = self.selected_backup.as_ref().ok_or("請選擇備份")?.clone();
        let catalog = self.catalog.as_ref().ok_or("尚未掃描存檔目錄，請先掃描")?;
        if !catalog.backups.iter().any(|b| b.file_name == name) {
            return Err("備份已消失，請重新掃描".into());
        }
        let backup_path = backups.join(&name);
        if !backup_path.is_file() {
            return Err("備份已消失，請重新掃描".into());
        }
        let service = self.service()?;
        let report = service.verify_backup(&name).map_err(err)?;
        let msg = format_verify_report(&format!("備份 {}", name), &report);
        self.last_verify_report = Some((format!("備份 {}", name), clone_verify_report(&report)));
        self.status = msg;
        Ok(report)
    }
    fn report_verify(&mut self, r: Result<VerifyReport>) {
        if let Err(e) = r {
            self.last_verify_report = None;
            self.status = e;
        }
    }
    fn report(&mut self, r: Result<()>) {
        self.status = match r {
            Ok(()) => "完成".into(),
            Err(e) => e,
        };
    }
    fn report_msg(&mut self, r: Result<String>) {
        self.status = match r {
            Ok(msg) => if msg.is_empty() { "完成".into() } else { msg },
            Err(e) => e,
        };
    }
    pub fn show(&mut self, ctx: &egui::Context) {
        let mut open = self.open;
        let title = match self.current_tab {
            ToolboxTab::Modifier => "EXE Modifier (遊戲補丁修改工具)",
            ToolboxTab::SaveManager => "Save Game Manager (遊戲存檔管理工具)",
        };
        egui::Window::new(title).open(&mut open).default_width(680.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.current_tab, ToolboxTab::Modifier, "EXE Modifier (遊戲補丁)");
                ui.selectable_value(&mut self.current_tab, ToolboxTab::SaveManager, "Save Manager (存檔管理)");
            });
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                match self.current_tab {
                    ToolboxTab::Modifier => {
                        ui.heading("EXE Modifier (遊戲補丁)");
                        ui.label("只操作工作副本；關閉遊戲。Program Files 寫入會拒絕。Inspect 為唯讀。");
                        ui.label("EXE 完整路徑");
                        let a = ui.text_edit_singleline(&mut self.exe).changed();
                        ui.label("新備份檔案完整路徑（必填）");
                        let b = ui.text_edit_singleline(&mut self.patch_backup).changed();
                        if a || b {
                            self.patch_review = None;
                            if a { self.states = None; }
                        }
                        if ui.button("Inspect EXE").clicked() {
                            let r = self.inspect();
                            self.report(r);
                        }
                        if let Some(states) = self.states {
                            for i in 0..3 {
                                ui.horizontal(|ui| {
                                    ui.label(format!("{}: {:?}", PATCHES[i].name, states[i]));
                                    if ui.checkbox(&mut self.enabled[i], "Enable（取消 = Disable）").changed() {
                                        self.patch_review = None;
                                    }
                                });
                            }
                        }
                        if ui.button("Review patches").clicked() {
                            let r = self.review_patch();
                            self.report(r);
                        }
                        if let Some(r) = &self.patch_review {
                            ui.label(format!("將修改 {}\n保留原始 bytes 到 {}\nEnable: {:?}", r.exe, r.backup, r.enabled));
                            if ui.button("確認 Apply patches").clicked() {
                                let result = self.apply_patch();
                                self.report(result);
                            }
                        }
                    }
                    ToolboxTab::SaveManager => {
                        ui.heading("Save Manager (存檔管理)");
                        ui.label("明確指定 game root（其下為 SAVE）與備份 root；還原與刪除只允許工作副本。");
                        ui.label("Game root");
                        let a = ui
                            .horizontal(|ui| {
                                let changed = ui.text_edit_singleline(&mut self.game).changed();
                                if ui.button("Browse... (瀏覽)").clicked() {
                                    self.browse_game_folder();
                                }
                                changed
                            })
                            .inner;
                        ui.label("Backups root");
                        let b = ui.text_edit_singleline(&mut self.backups).changed();
                        if a || b {
                            self.catalog = None;
                            self.selected_save = None;
                            self.selected_backup = None;
                            self.save_review = None;
                            self.last_verify_report = None;
                            self.title.clear();
                            self.restore_folder.clear();
                        }
                        if ui.button("Scan saves / backups").clicked() {
                            let r = self.scan();
                            self.report(r);
                        }
                        if let Some(c) = &self.catalog {
                            ui.label("存檔（folder / title / level / metadata）");
                            for s in &c.saves {
                                if ui.selectable_label(self.selected_save.as_ref() == Some(&s.folder), format!("{} | {} | {} | parsed={}", s.folder, s.title, s.level, s.parsed)).clicked() {
                                    self.selected_save = Some(s.folder.clone());
                                    self.title = s.title.clone();
                                    self.save_review = None;
                                    self.last_verify_report = None;
                                }
                            }
                            ui.label("備份（archive / original folder / title / level / time）");
                            for b in &c.backups {
                                if ui.selectable_label(self.selected_backup.as_ref() == Some(&b.file_name), format!("{} | {} | {} | {} | {} | parsed={}", b.file_name, b.manifest.orig_folder, b.manifest.title, b.manifest.level, b.manifest.backup_time, b.parsed)).clicked() {
                                    self.selected_backup = Some(b.file_name.clone());
                                    self.restore_folder = b.manifest.orig_folder.clone();
                                    self.save_review = None;
                                    self.last_verify_report = None;
                                }
                            }
                        }
                        ui.label("備份標題");
                        if ui.text_edit_singleline(&mut self.title).changed() {
                            self.save_review = None;
                        }
                        ui.label("還原目的資料夾名稱（SAVE 之下；會替換整個資料夾）");
                        if ui.text_edit_singleline(&mut self.restore_folder).changed() {
                            self.save_review = None;
                        }
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Verify selected save (驗證選取存檔)").clicked() {
                                let r = self.verify_selected_save();
                                self.report_verify(r);
                            }
                            if ui.button("Verify selected backup (驗證選取備份)").clicked() {
                                let r = self.verify_selected_backup();
                                self.report_verify(r);
                            }
                        });
                        if let Some((target, rep)) = &self.last_verify_report {
                            ui.group(|ui| {
                                ui.heading(format!("驗證報告: {}", target));
                                ui.horizontal(|ui| {
                                    if rep.is_ok() {
                                        ui.colored_label(egui::Color32::GREEN, "狀態: 通過 (OK)");
                                    } else {
                                        ui.colored_label(egui::Color32::RED, "狀態: 異常 (FAIL)");
                                    }
                                    ui.label(format!("項目數 (entries): {}", rep.entries));
                                    ui.label(format!("總大小 (total_bytes): {} bytes", rep.total_bytes));
                                });
                                ui.label(format!("標題 (title): {}", rep.title));
                                ui.label(format!("關卡 (level): {}", rep.level));
                                if !rep.errors.is_empty() {
                                    ui.colored_label(
                                        egui::Color32::RED,
                                        format!("錯誤 (errors, {}):", rep.errors.len()),
                                    );
                                    for err in &rep.errors {
                                        ui.label(format!("  - {}", err));
                                    }
                                } else {
                                    ui.label("錯誤 (errors): 無");
                                }
                                if !rep.warnings.is_empty() {
                                    ui.colored_label(
                                        egui::Color32::YELLOW,
                                        format!("警告 (warnings, {}):", rep.warnings.len()),
                                    );
                                    for warn in &rep.warnings {
                                        ui.label(format!("  - {}", warn));
                                    }
                                } else {
                                    ui.label("警告 (warnings): 無");
                                }
                            });
                        }
                        ui.horizontal_wrapped(|ui| {
                            for (label, action) in [
                                ("Review Backup", SaveAction::Backup),
                                ("Review Restore", SaveAction::Restore),
                                ("Review Delete save", SaveAction::DeleteSave),
                                ("Review Delete backup", SaveAction::DeleteBackup),
                                ("Review Repair AI", SaveAction::RepairAi),
                            ] {
                                if ui.button(label).clicked() {
                                    let r = self.review_save(action);
                                    self.report(r);
                                }
                            }
                        });
                        if let Some(r) = &self.save_review {
                            let confirm_desc = match r.action {
                                SaveAction::RepairAi => format!(
                                    "確認 {:?}: {}\nGame root: {}\nBackups root: {}\n目標存檔 folder: {}\n將修復 CLAK/scr.dat 並在變更前建立安全備份",
                                    r.action, r.source, r.game, r.backups, r.folder
                                ),
                                _ => format!(
                                    "確認 {:?}: {}\nGame root: {}\nBackups root: {}\n將替換/操作 folder: {}\nTitle: {}",
                                    r.action, r.source, r.game, r.backups, r.folder, r.title
                                ),
                            };
                            ui.colored_label(egui::Color32::YELLOW, confirm_desc);
                            for (p, _) in &r.paths {
                                ui.label(p.display().to_string());
                            }
                            if ui.button("確認執行此操作").clicked() {
                                let r = self.apply_save();
                                self.report_msg(r);
                            }
                        }
                    }
                }
                ui.separator();
                ui.label(&self.status);
            });
        });
        self.open = open;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "arm-tools-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn modifier_review_apply_and_stale_guard() {
        let t = Temp::new();
        let p = t.0.join("copy.exe");
        let mut b = vec![0; FOCUS.offset + FOCUS.original.len()];
        for patch in PATCHES {
            b[patch.offset..patch.offset + patch.original.len()].copy_from_slice(patch.original);
        }
        fs::write(&p, &b).unwrap();
        let mut u = ToolboxUi {
            exe: p.display().to_string(),
            patch_backup: t.0.join("before.bin").display().to_string(),
            ..Default::default()
        };
        u.inspect().unwrap();
        assert_eq!(u.states, Some([State::Original; 3]));
        u.enabled = [true; 3];
        u.review_patch().unwrap();
        u.apply_patch().unwrap();
        assert_eq!(u.states, Some([State::Patched; 3]));
        assert_eq!(fs::read(&u.patch_backup).unwrap(), b);
        u.patch_backup = t.0.join("next.bin").display().to_string();
        u.enabled = [false; 3];
        u.review_patch().unwrap();
        fs::write(&p, b"changed").unwrap();
        assert!(u.apply_patch().is_err());
        assert!(!Path::new(&u.patch_backup).exists());
    }
    #[test]
    fn save_backup_restore_delete_and_stale_review() {
        let t = Temp::new();
        let game = t.0.join("game");
        let folder = game.join("SAVE/SAVE_001");
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join("save.ini"),
            b"[titel]\nTitle\n[orglevelname]\nENDL_000\n",
        )
        .unwrap();
        fs::write(folder.join("data.bin"), b"original").unwrap();
        let mut u = ToolboxUi {
            game: game.display().to_string(),
            backups: t.0.join("backups").display().to_string(),
            ..Default::default()
        };
        u.scan().unwrap();
        u.selected_save = Some("SAVE_001".into());
        u.title = "Custom title".into();
        u.review_save(SaveAction::Backup).unwrap();
        u.apply_save().unwrap();
        let archive = u.catalog.as_ref().unwrap().backups[0].file_name.clone();
        assert_eq!(
            u.catalog.as_ref().unwrap().backups[0].manifest.title,
            "Custom title"
        );
        fs::write(folder.join("data.bin"), b"changed").unwrap();
        u.selected_backup = Some(archive.clone());
        u.restore_folder = "SAVE_001".into();
        u.review_save(SaveAction::Restore).unwrap();
        u.apply_save().unwrap();
        assert_eq!(fs::read(folder.join("data.bin")).unwrap(), b"original");
        u.selected_save = Some("SAVE_001".into());
        u.review_save(SaveAction::DeleteSave).unwrap();
        fs::write(folder.join("new.bin"), b"external").unwrap();
        assert!(u.apply_save().is_err());
        assert!(folder.exists());
        u.review_save(SaveAction::DeleteSave).unwrap();
        u.apply_save().unwrap();
        assert!(!folder.exists());
        u.selected_backup = Some(archive.clone());
        u.review_save(SaveAction::DeleteBackup).unwrap();
        u.apply_save().unwrap();
        assert!(!Path::new(&u.backups).join(archive).exists());
    }
    #[test]
    fn explicit_paths_and_installed_writes_fail_closed() {
        assert!(explicit("").is_err());
        assert!(explicit("relative").is_err());
        assert!(writable(Path::new("C:\\Program Files (x86)\\Against Rome\\game.exe")).is_err());
    }

    #[test]
    fn review_root_selection_and_target_changes_fail_closed() {
        let t = Temp::new();
        let game = t.0.join("game");
        let source = game.join("SAVE/SAVE_001");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("save.ini"), b"[titel]\nA\n").unwrap();
        let mut u = ToolboxUi {
            game: game.display().to_string(),
            backups: t.0.join("backups").display().to_string(),
            ..Default::default()
        };
        u.scan().unwrap();
        u.selected_save = Some("SAVE_001".into());
        u.review_save(SaveAction::DeleteSave).unwrap();
        u.game = t.0.join("other-game").display().to_string();
        assert!(u.apply_save().is_err());
        assert!(source.exists());
        assert!(u.save_review.is_none());
        u.game = game.display().to_string();
        u.review_save(SaveAction::Backup).unwrap();
        u.selected_save = Some("SAVE_002".into());
        assert!(u.apply_save().is_err());
        assert!(u.catalog.as_ref().unwrap().backups.is_empty());
        u.selected_save = Some("SAVE_001".into());
        u.review_save(SaveAction::Backup).unwrap();
        u.apply_save().unwrap();
        u.selected_backup = Some(u.catalog.as_ref().unwrap().backups[0].file_name.clone());
        u.restore_folder = "SAVE_002".into();
        u.review_save(SaveAction::Restore).unwrap();
        let dest = game.join("SAVE/SAVE_002");
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("save.ini"), b"external work").unwrap();
        assert!(u.apply_save().is_err());
        assert_eq!(fs::read(dest.join("save.ini")).unwrap(), b"external work");
        u.scan().unwrap();
        assert!(u.selected_save.is_none());
        assert!(u.selected_backup.is_none());
        assert!(u.save_review.is_none());
    }

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
    fn save_ai_repair_success_creates_backup_and_subsequent_noop_reports_already_repaired() {
        let t = Temp::new();
        let game = t.0.join("game");
        let folder = game.join("SAVE/SAVE_001");
        fs::create_dir_all(folder.join("CLAK")).unwrap();
        fs::write(
            folder.join("save.ini"),
            b"[titel]\nTitle\n[orglevelname]\nENDL_003\n",
        )
        .unwrap();
        let raw = endless_ai_fixture();
        fs::write(folder.join("CLAK/scr.dat"), &raw).unwrap();
        fs::write(folder.join("other.dat"), b"preserve").unwrap();

        let mut u = ToolboxUi {
            game: game.display().to_string(),
            backups: t.0.join("backups").display().to_string(),
            ..Default::default()
        };
        u.scan().unwrap();
        u.selected_save = Some("SAVE_001".into());
        u.review_save(SaveAction::RepairAi).unwrap();
        assert!(u.save_review.is_some());

        let res = u.apply_save().unwrap();
        assert!(res.contains("Changed"));
        assert!(res.contains("安全備份已建立"));
        assert!(u.status.contains("Changed"));

        let catalog = u.catalog.as_ref().unwrap();
        assert_eq!(catalog.backups.len(), 1);
        let backup_name = &catalog.backups[0].file_name;
        assert!(res.contains(backup_name));
        assert!(Path::new(&u.backups).join(backup_name).is_file());

        let repaired_scr = fs::read(folder.join("CLAK/scr.dat")).unwrap();
        assert_ne!(repaired_scr, raw);
        assert_eq!(fs::read(folder.join("other.dat")).unwrap(), b"preserve");

        // Subsequent repair on already repaired save is a NO-OP reporting AlreadyRepaired without creating extra backup
        u.selected_save = Some("SAVE_001".into());
        u.review_save(SaveAction::RepairAi).unwrap();
        let res2 = u.apply_save().unwrap();
        assert!(res2.contains("AlreadyRepaired"));
        assert!(u.status.contains("AlreadyRepaired"));
        assert_eq!(u.catalog.as_ref().unwrap().backups.len(), 1);
        assert_eq!(fs::read(folder.join("CLAK/scr.dat")).unwrap(), repaired_scr);
    }

    #[test]
    fn save_ai_repair_stale_source_and_tampered_state_fail_closed() {
        let t = Temp::new();
        let game = t.0.join("game");
        let folder = game.join("SAVE/SAVE_001");
        fs::create_dir_all(folder.join("CLAK")).unwrap();
        fs::write(
            folder.join("save.ini"),
            b"[titel]\nTitle\n[orglevelname]\nENDL_003\n",
        )
        .unwrap();
        let raw = endless_ai_fixture();
        fs::write(folder.join("CLAK/scr.dat"), &raw).unwrap();

        let mut u = ToolboxUi {
            game: game.display().to_string(),
            backups: t.0.join("backups").display().to_string(),
            ..Default::default()
        };
        u.scan().unwrap();
        u.selected_save = Some("SAVE_001".into());
        u.review_save(SaveAction::RepairAi).unwrap();

        // 1. External file tampering in save folder before apply
        fs::write(folder.join("CLAK/scr.dat"), b"tampered").unwrap();
        let err = u.apply_save().unwrap_err();
        assert!(err.contains("Review 後來源或目標已變更，拒絕執行"));
        assert_eq!(u.catalog.as_ref().unwrap().backups.len(), 0);

        // 2. Selection changed after review
        fs::write(folder.join("CLAK/scr.dat"), &raw).unwrap();
        u.selected_save = Some("SAVE_001".into());
        u.review_save(SaveAction::RepairAi).unwrap();
        u.selected_save = Some("SAVE_002".into());
        let err2 = u.apply_save().unwrap_err();
        assert!(err2.contains("選擇或內容已變更，請重新 Review"));

        // 3. Root path changed after review
        u.selected_save = Some("SAVE_001".into());
        u.review_save(SaveAction::RepairAi).unwrap();
        u.game = t.0.join("other_game").display().to_string();
        let err3 = u.apply_save().unwrap_err();
        assert!(err3.contains("Root 已變更，請重新掃描"));
    }

    #[test]
    fn save_ai_repair_unsafe_paths_and_missing_scr_rejected() {
        let t = Temp::new();
        let game = t.0.join("game");
        let folder = game.join("SAVE/SAVE_001");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("save.ini"), b"[titel]\nNoScr\n").unwrap();

        let mut u = ToolboxUi {
            game: game.display().to_string(),
            backups: t.0.join("backups").display().to_string(),
            ..Default::default()
        };
        u.scan().unwrap();

        // 1. No save selected
        u.selected_save = None;
        assert!(u.review_save(SaveAction::RepairAi).is_err());

        // 2. Missing CLAK/scr.dat
        u.selected_save = Some("SAVE_001".into());
        let err = u.review_save(SaveAction::RepairAi).unwrap_err();
        assert!(err.contains("缺少 CLAK/scr.dat"));

        // 3. Unsafe path: Program Files rejection on game
        let mut u_prog = ToolboxUi {
            game: "C:\\Program Files (x86)\\Against Rome".into(),
            backups: t.0.join("backups").display().to_string(),
            ..Default::default()
        };
        assert!(u_prog.review_save(SaveAction::RepairAi).is_err());

        // 4. Unsafe path: Program Files rejection on backups
        let mut u_backups_prog = ToolboxUi {
            game: game.display().to_string(),
            backups: "C:\\Program Files\\Backups".into(),
            ..Default::default()
        };
        assert!(u_backups_prog.review_save(SaveAction::RepairAi).is_err());
    }

    #[test]
    fn save_and_backup_verification_success() {
        let t = Temp::new();
        let game = t.0.join("game");
        let folder = game.join("SAVE/SAVE_001");
        fs::create_dir_all(folder.join("CLAK")).unwrap();
        fs::write(
            folder.join("save.ini"),
            b"[titel]\nHeroic Victory\n[orglevelname]\nENDL_002\n",
        )
        .unwrap();
        fs::write(folder.join("CLAK/scr.dat"), b"script-content").unwrap();
        fs::write(folder.join("data.bin"), b"data-content").unwrap();

        let mut u = ToolboxUi {
            game: game.display().to_string(),
            backups: t.0.join("backups").display().to_string(),
            ..Default::default()
        };
        u.scan().unwrap();
        u.selected_save = Some("SAVE_001".into());

        // 1. Verify live save
        let rep = u.verify_selected_save().unwrap();
        assert!(rep.is_ok());
        assert_eq!(rep.entries, 3);
        assert!(rep.total_bytes > 0);
        assert_eq!(rep.title, "Heroic Victory");
        assert_eq!(rep.level, "ENDL_002");
        assert!(rep.errors.is_empty());
        assert!(u.status.contains("Heroic Victory"));
        assert!(u.status.contains("ENDL_002"));
        assert!(u.status.contains("通過 (OK)"));
        assert!(u.status.contains("entries"));
        assert!(u.status.contains("total_bytes"));
        assert!(u.last_verify_report.is_some());
        assert!(u.save_review.is_none());

        // Create backup
        u.title = "Custom Backup Title".into();
        u.review_save(SaveAction::Backup).unwrap();
        u.apply_save().unwrap();
        let archive = u.catalog.as_ref().unwrap().backups[0].file_name.clone();

        // 2. Verify backup
        u.selected_backup = Some(archive.clone());
        let rep_b = u.verify_selected_backup().unwrap();
        assert!(rep_b.is_ok());
        assert!(rep_b.entries >= 3);
        assert!(rep_b.total_bytes > 0);
        assert_eq!(rep_b.title, "Heroic Victory");
        assert_eq!(rep_b.level, "ENDL_002");
        assert!(rep_b.errors.is_empty());
        assert!(u.status.contains("Heroic Victory"));
        assert!(u.status.contains("ENDL_002"));
        assert!(u.status.contains("通過 (OK)"));
        assert!(u.last_verify_report.is_some());
        assert!(u.save_review.is_none());
    }

    #[test]
    fn save_and_backup_corrupt_error_reporting() {
        let t = Temp::new();
        let game = t.0.join("game");
        let folder = game.join("SAVE/SAVE_CORRUPT");
        fs::create_dir_all(&folder).unwrap();
        // Corrupt PFIL header that fails decompression
        fs::write(folder.join("save.ini"), b"PFIL\x00\x00\x00\x00corrupt_payload").unwrap();

        let mut u = ToolboxUi {
            game: game.display().to_string(),
            backups: t.0.join("backups").display().to_string(),
            ..Default::default()
        };
        u.scan().unwrap();
        u.selected_save = Some("SAVE_CORRUPT".into());

        // 1. Live save with corrupt save.ini reports error
        let rep = u.verify_selected_save().unwrap();
        assert!(!rep.is_ok());
        assert!(!rep.errors.is_empty());
        assert!(rep.errors.iter().any(|e| e.contains("save.ini unreadable")));
        assert!(u.status.contains("異常 (FAIL)"));
        assert!(u.status.contains("save.ini unreadable"));
        assert!(u.save_review.is_none());

        // 2. Corrupt backup file (invalid ZIP) reports error
        let corrupt_zip_path = Path::new(&u.backups).join("corrupt_backup.zip");
        fs::write(&corrupt_zip_path, b"NOT_A_VALID_ZIP_HEADER").unwrap();
        u.scan().unwrap();
        u.selected_backup = Some("corrupt_backup.zip".into());

        let rep_b = u.verify_selected_backup().unwrap();
        assert!(!rep_b.is_ok());
        assert!(!rep_b.errors.is_empty());
        assert!(rep_b.errors.iter().any(|e| e.contains("not a readable ZIP")));
        assert!(u.status.contains("異常 (FAIL)"));
        assert!(u.status.contains("not a readable ZIP"));
        assert!(u.save_review.is_none());
    }

    #[test]
    fn save_and_backup_verify_rejects_missing_and_stale_selection() {
        let t = Temp::new();
        let game = t.0.join("game");
        let folder = game.join("SAVE/SAVE_001");
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join("save.ini"),
            b"[titel]\nTitle\n[orglevelname]\nENDL_001\n",
        )
        .unwrap();

        let mut u = ToolboxUi {
            game: game.display().to_string(),
            backups: t.0.join("backups").display().to_string(),
            ..Default::default()
        };
        u.scan().unwrap();
        u.selected_save = Some("SAVE_001".into());
        u.review_save(SaveAction::Backup).unwrap();
        u.apply_save().unwrap();
        let archive = u.catalog.as_ref().unwrap().backups[0].file_name.clone();

        // 1. Missing selection
        u.selected_save = None;
        assert_eq!(u.verify_selected_save().unwrap_err(), "請選擇存檔");
        u.selected_backup = None;
        assert_eq!(u.verify_selected_backup().unwrap_err(), "請選擇備份");

        // 2. Stale selection (not present in catalog)
        u.selected_save = Some("SAVE_NONEXISTENT".into());
        assert_eq!(u.verify_selected_save().unwrap_err(), "存檔已消失，請重新掃描");
        u.selected_backup = Some("NONEXISTENT.zip".into());
        assert_eq!(u.verify_selected_backup().unwrap_err(), "備份已消失，請重新掃描");

        // 3. Stale selection (in catalog, but deleted from disk)
        u.selected_save = Some("SAVE_001".into());
        fs::remove_dir_all(&folder).unwrap();
        assert_eq!(u.verify_selected_save().unwrap_err(), "存檔已消失，請重新掃描");

        u.selected_backup = Some(archive.clone());
        fs::remove_file(Path::new(&u.backups).join(&archive)).unwrap();
        assert_eq!(u.verify_selected_backup().unwrap_err(), "備份已消失，請重新掃描");

        // 4. Missing catalog
        u.catalog = None;
        u.selected_save = Some("SAVE_001".into());
        u.selected_backup = Some(archive);
        assert_eq!(
            u.verify_selected_save().unwrap_err(),
            "尚未掃描存檔目錄，請先掃描"
        );
        assert_eq!(
            u.verify_selected_backup().unwrap_err(),
            "尚未掃描存檔目錄，請先掃描"
        );
    }

    #[test]
    fn save_and_backup_verification_does_not_mutate_or_create_dirs() {
        let t = Temp::new();
        let game = t.0.join("game");
        let folder = game.join("SAVE/SAVE_001");
        fs::create_dir_all(folder.join("CLAK")).unwrap();
        fs::write(
            folder.join("save.ini"),
            b"[titel]\nRead Only\n[orglevelname]\nENDL_001\n",
        )
        .unwrap();
        fs::write(folder.join("CLAK/scr.dat"), b"untouched script").unwrap();
        fs::write(folder.join("data.bin"), b"untouched data").unwrap();

        let initial_ini = fs::read(folder.join("save.ini")).unwrap();
        let initial_scr = fs::read(folder.join("CLAK/scr.dat")).unwrap();
        let initial_dat = fs::read(folder.join("data.bin")).unwrap();

        let mut u = ToolboxUi {
            game: game.display().to_string(),
            backups: t.0.join("backups").display().to_string(),
            ..Default::default()
        };
        u.scan().unwrap();
        u.selected_save = Some("SAVE_001".into());
        u.review_save(SaveAction::Backup).unwrap();
        u.apply_save().unwrap();
        let archive = u.catalog.as_ref().unwrap().backups[0].file_name.clone();
        let initial_zip = fs::read(Path::new(&u.backups).join(&archive)).unwrap();

        // Target directory that should never be created
        let never_create_dir = t.0.join("never_created_backups_dir");
        assert!(!never_create_dir.exists());

        // Verify live save pointing to a non-existent backups directory
        u.backups = never_create_dir.display().to_string();
        u.selected_save = Some("SAVE_001".into());
        let rep = u.verify_selected_save().unwrap();
        assert!(rep.is_ok());

        // Prove no directory creation and no file byte mutation
        assert!(!never_create_dir.exists(), "verification must not create directories");
        assert_eq!(fs::read(folder.join("save.ini")).unwrap(), initial_ini);
        assert_eq!(fs::read(folder.join("CLAK/scr.dat")).unwrap(), initial_scr);
        assert_eq!(fs::read(folder.join("data.bin")).unwrap(), initial_dat);
        assert!(u.save_review.is_none(), "verification must not create a review/apply mutation flow");

        // Verify backup pointing to a non-existent game directory
        let never_create_game = t.0.join("never_created_game_dir");
        assert!(!never_create_game.exists());
        u.game = never_create_game.display().to_string();
        u.backups = t.0.join("backups").display().to_string();
        u.selected_backup = Some(archive.clone());
        let rep_b = u.verify_selected_backup().unwrap();
        assert!(rep_b.is_ok());

        // Prove no directory creation and no backup byte mutation
        assert!(!never_create_game.exists(), "verification must not create directories");
        assert_eq!(
            fs::read(Path::new(&u.backups).join(&archive)).unwrap(),
            initial_zip,
            "verification must not mutate backup bytes"
        );
        assert!(u.save_review.is_none(), "verification must not create a review/apply mutation flow");
    }

    fn make_game(root: &Path, folder: &str) {
        let f = root.join("SAVE").join(folder);
        fs::create_dir_all(&f).unwrap();
        fs::write(f.join("save.ini"), b"[titel]\nPicked\n[orglevelname]\nENDL_000\n").unwrap();
    }

    #[test]
    fn game_folder_picker_success_updates_path_and_rescans() {
        let t = Temp::new();
        let old_game = t.0.join("old");
        let new_game = t.0.join("new");
        make_game(&old_game, "SAVE_OLD");
        make_game(&new_game, "SAVE_NEW");
        let mut u = ToolboxUi {
            game: old_game.display().to_string(),
            backups: t.0.join("backups").display().to_string(),
            ..Default::default()
        };
        u.scan().unwrap();
        u.selected_save = Some("SAVE_OLD".into());
        let picked = new_game.clone();
        u.game_folder_picker = Some(Box::new(move || Some(picked.clone())));
        u.browse_game_folder();
        assert_eq!(u.game, new_game.to_string_lossy());
        assert_eq!(u.status, "完成");
        let c = u.catalog.as_ref().expect("catalog refreshed");
        assert_eq!(c.saves.len(), 1);
        assert_eq!(c.saves[0].folder, "SAVE_NEW");
        assert!(u.selected_save.is_none());
    }

    #[test]
    fn game_folder_picker_cancel_and_invalid_selection_preserve_state() {
        let t = Temp::new();
        let game = t.0.join("game");
        make_game(&game, "SAVE_001");
        let mut u = ToolboxUi {
            game: game.display().to_string(),
            backups: t.0.join("backups").display().to_string(),
            ..Default::default()
        };
        u.scan().unwrap();
        u.selected_save = Some("SAVE_001".into());
        u.game_folder_picker = Some(Box::new(|| None));
        u.browse_game_folder();
        assert_eq!(u.game, game.to_string_lossy());
        assert!(u.catalog.is_some());
        assert_eq!(u.selected_save.as_deref(), Some("SAVE_001"));
        assert!(u.status.is_empty());

        let missing = t.0.join("missing-game-folder");
        u.game_folder_picker = Some(Box::new(move || Some(missing.clone())));
        u.browse_game_folder();
        assert_eq!(u.game, game.to_string_lossy());
        assert!(u.catalog.is_some());
        assert_eq!(u.selected_save.as_deref(), Some("SAVE_001"));
        assert!(u.status.contains("does not exist"));
    }

    #[test]
    fn game_folder_picker_scan_failure_is_reported_in_status() {
        let t = Temp::new();
        let game = t.0.join("game");
        fs::create_dir_all(&game).unwrap();
        let mut u = ToolboxUi::default(); // empty backups root -> scan rejects it
        let picked = game.clone();
        u.game_folder_picker = Some(Box::new(move || Some(picked.clone())));
        u.browse_game_folder();
        assert_eq!(u.game, game.to_string_lossy());
        assert!(u.catalog.is_none());
        assert!(!u.status.is_empty() && u.status != "完成");
    }
}
