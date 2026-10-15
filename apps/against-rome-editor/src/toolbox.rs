//! Native desktop tools. Reviews bind exact paths and source bytes to explicit confirmation.
use against_rome_modifier::{self as modifier, State, FOCUS, ROMAN_ENDLESS, UNIT_RECRUIT_20};
use against_rome_saves::{Catalog, SaveService};
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
enum SaveAction {
    Backup,
    Restore,
    DeleteSave,
    DeleteBackup,
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
#[derive(Default)]
pub struct ToolboxUi {
    pub open: bool,
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
    status: String,
}
impl ToolboxUi {
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
    fn scan(&mut self) -> Result<()> {
        self.catalog = None;
        self.selected_save = None;
        self.selected_backup = None;
        self.save_review = None;
        self.title.clear();
        self.restore_folder.clear();
        writable(&explicit(&self.backups)?)?;
        self.catalog = Some(self.service()?.scan().map_err(err)?);
        Ok(())
    }
    fn review_save(&mut self, action: SaveAction) -> Result<()> {
        self.save_review = None;
        let game = explicit(&self.game)?;
        let backups = explicit(&self.backups)?;
        writable(&backups)?;
        let catalog = self.service()?.scan().map_err(err)?;
        let (source, folder, title, level, paths) = match action {
            SaveAction::Backup | SaveAction::DeleteSave => {
                let name = self.selected_save.as_ref().ok_or("請選擇存檔")?;
                let info = catalog
                    .saves
                    .iter()
                    .find(|s| &s.folder == name)
                    .ok_or("存檔已消失，請重新掃描")?;
                let p = game.join("SAVE").join(name);
                if action == SaveAction::Backup {
                    writable(&backups)?;
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
    fn apply_save(&mut self) -> Result<()> {
        let r = self.save_review.take().ok_or("請先 Review")?;
        if r.game != self.game || r.backups != self.backups {
            return Err("Root 已變更，請重新掃描".into());
        }
        if matches!(r.action, SaveAction::Backup | SaveAction::DeleteSave)
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
                return Ok(());
            }
            SaveAction::DeleteSave => {
                writable(&explicit(&r.game)?.join("SAVE").join(&r.source))?;
                service.delete_save(&r.source).map_err(err)?;
            }
            SaveAction::DeleteBackup => {
                writable(&explicit(&r.backups)?.join(&r.source))?;
                service.delete_backup(&r.source).map_err(err)?;
            }
        }
        self.scan()
    }
    fn report(&mut self, r: Result<()>) {
        self.status = match r {
            Ok(()) => "完成".into(),
            Err(e) => e,
        };
    }
    pub fn show(&mut self, ctx: &egui::Context) {
        let mut open = self.open;
        egui::Window::new("Rust 工具箱 — Modifier / Save Manager").open(&mut open).default_width(650.0).show(ctx,|ui|{
            egui::ScrollArea::vertical().show(ui,|ui|{
                ui.heading("EXE Modifier");ui.label("只操作工作副本；關閉遊戲。Program Files 寫入會拒絕。Inspect 為唯讀。");
                ui.label("EXE 完整路徑");let a=ui.text_edit_singleline(&mut self.exe).changed();ui.label("新備份檔案完整路徑（必填）");let b=ui.text_edit_singleline(&mut self.patch_backup).changed();
                if a||b {self.patch_review=None;if a {self.states=None;}}
                if ui.button("Inspect EXE").clicked(){let r=self.inspect();self.report(r);}
                if let Some(states)=self.states {for i in 0..3 {ui.horizontal(|ui|{ui.label(format!("{}: {:?}",PATCHES[i].name,states[i]));if ui.checkbox(&mut self.enabled[i],"Enable（取消 = Disable）").changed(){self.patch_review=None;}});}}
                if ui.button("Review patches").clicked(){let r=self.review_patch();self.report(r);}
                if let Some(r)=&self.patch_review {ui.label(format!("將修改 {}\n保留原始 bytes 到 {}\nEnable: {:?}",r.exe,r.backup,r.enabled));if ui.button("確認 Apply patches").clicked(){let result=self.apply_patch();self.report(result);}}
                ui.separator();ui.heading("Save Manager");ui.label("明確指定 game root（其下為 SAVE）與備份 root；還原與刪除只允許工作副本。");
                ui.label("Game root");let a=ui.text_edit_singleline(&mut self.game).changed();ui.label("Backups root");let b=ui.text_edit_singleline(&mut self.backups).changed();
                if a||b {self.catalog=None;self.selected_save=None;self.selected_backup=None;self.save_review=None;self.title.clear();self.restore_folder.clear();}
                if ui.button("Scan saves / backups").clicked(){let r=self.scan();self.report(r);}
                if let Some(c)=&self.catalog {
                    ui.label("存檔（folder / title / level / metadata）");
                    for s in &c.saves {if ui.selectable_label(self.selected_save.as_ref()==Some(&s.folder),format!("{} | {} | {} | parsed={}",s.folder,s.title,s.level,s.parsed)).clicked(){self.selected_save=Some(s.folder.clone());self.title=s.title.clone();self.save_review=None;}}
                    ui.label("備份（archive / original folder / title / level / time）");
                    for b in &c.backups {if ui.selectable_label(self.selected_backup.as_ref()==Some(&b.file_name),format!("{} | {} | {} | {} | {} | parsed={}",b.file_name,b.manifest.orig_folder,b.manifest.title,b.manifest.level,b.manifest.backup_time,b.parsed)).clicked(){self.selected_backup=Some(b.file_name.clone());self.restore_folder=b.manifest.orig_folder.clone();self.save_review=None;}}
                }
                ui.label("備份標題");if ui.text_edit_singleline(&mut self.title).changed(){self.save_review=None;}
                ui.label("還原目的資料夾名稱（SAVE 之下；會替換整個資料夾）");if ui.text_edit_singleline(&mut self.restore_folder).changed(){self.save_review=None;}
                ui.horizontal_wrapped(|ui|{for (label,action) in [("Review Backup",SaveAction::Backup),("Review Restore",SaveAction::Restore),("Review Delete save",SaveAction::DeleteSave),("Review Delete backup",SaveAction::DeleteBackup)] {if ui.button(label).clicked(){let r=self.review_save(action);self.report(r);}}});
                if let Some(r)=&self.save_review {ui.colored_label(egui::Color32::YELLOW,format!("確認 {:?}: {}\nGame root: {}\nBackups root: {}\n將替換/操作 folder: {}\nTitle: {}",r.action,r.source,r.game,r.backups,r.folder,r.title));for(p,_)in &r.paths {ui.label(p.display().to_string());}if ui.button("確認執行此操作").clicked(){let r=self.apply_save();self.report(r);}}
                ui.separator();ui.label(&self.status);
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
}
