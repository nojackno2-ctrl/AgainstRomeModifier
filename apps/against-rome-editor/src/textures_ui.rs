//! Native material-layer authoring. Saving boden.txt is independent of terrain/DATA saves.
use against_rome_textures::{
    BodenTexturesDocument, FloortexLibrary, NativeTerrainMaterialResolver,
    StandardFloorMaterialCatalog, TerrainBlendAuthoringMap, TerrainBlendEditSession,
};
use eframe::egui;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

type Result<T> = std::result::Result<T, String>;

fn explicit_map(text: &str) -> Result<PathBuf> {
    let path = PathBuf::from(text.trim());
    if !path.is_absolute() || path.components().any(|c| c == Component::ParentDir) {
        return Err("請輸入 MAP 資料夾的完整絕對路徑，不可包含 ..".into());
    }
    if !path.is_dir() {
        return Err("MAP 資料夾不存在".into());
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "arm-textures-ui-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            fs::write(path.join("boden.txt"), b"; keep me\r\n[Dimension]\r\n2\r\n[Texturen]\r\n4BA___51\r\n4BA___51\r\n4BA___51\r\n4BB___51\r\n[Unknown]\r\nkeep=42\r\n").unwrap();
            Self(path)
        }
        fn load(&self) -> TexturesUi {
            let mut ui = TexturesUi {
                open: true,
                map_input: self.0.display().to_string(),
                ..Default::default()
            };
            ui.request_load().unwrap();
            ui
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn paint_all(ui: &mut TexturesUi) {
        ui.selected = "BB".into();
        ui.center = [1.0, 1.0];
        ui.radius = 4.0;
        assert!(ui.paint().unwrap() > 0);
    }

    #[test]
    fn paint_undo_redo_save_reload_and_exact_backup() {
        let temp = Temp::new();
        let original = fs::read(temp.0.join("boden.txt")).unwrap();
        let mut ui = temp.load();
        assert_eq!(ui.materials, ["BA", "BB"]);
        assert!(!ui.is_dirty());
        paint_all(&mut ui);
        assert!(ui.is_dirty());
        ui.session.as_mut().unwrap().undo().unwrap();
        assert!(!ui.is_dirty());
        ui.session.as_mut().unwrap().redo().unwrap();
        assert!(ui.is_dirty());
        // Editing the text box never redirects a loaded session's save.
        ui.map_input = temp.0.join("unloaded").display().to_string();
        let backup = ui.save().unwrap();
        assert_eq!(fs::read(backup).unwrap(), original);
        assert!(!ui.is_dirty());
        assert!(!Path::new(&ui.map_input).exists());
        let reload = temp.load();
        assert_eq!(
            reload.session.as_ref().unwrap().current_textures(),
            &["4BB___51"; 4]
        );
        let text = fs::read_to_string(temp.0.join("boden.txt")).unwrap();
        assert!(text.starts_with("; keep me\r\n"));
        assert!(text.ends_with("[Unknown]\r\nkeep=42\r\n"));
        assert!(!temp.0.join(".arm-textures-save.lock").exists());
        assert!(!temp.0.join(".arm-textures-stage").exists());
        ui.session.as_mut().unwrap().undo().unwrap();
        assert!(ui.is_dirty());
    }

    #[test]
    fn rejected_junction_and_invalid_brush_do_not_mutate_session() {
        let temp = Temp::new();
        let mut ui = temp.load();
        let before = ui.session.as_ref().unwrap().current_textures().to_vec();
        ui.selected = "BB".into();
        ui.radius = 0.1;
        assert!(ui.paint().is_err()); // No transition tiles in the fixture catalog.
        assert_eq!(ui.session.as_ref().unwrap().current_textures(), before);
        assert!(!ui.is_dirty());
        ui.radius = f32::NAN;
        assert!(ui.paint().is_err());
        ui.radius = 1.0;
        ui.center[0] = -1.0;
        assert!(ui.paint().is_err());
        assert!(!ui.is_dirty());
    }

    #[test]
    fn dirty_load_close_confirmation_and_failed_load_preserve_edits() {
        let temp = Temp::new();
        let mut ui = temp.load();
        paint_all(&mut ui);
        ui.library_input = temp.0.join("missing.dat").display().to_string();
        ui.request_load().unwrap();
        assert!(ui.pending.is_some());
        assert!(ui.confirm_pending().is_err());
        assert!(ui.is_dirty());
        ui.request_close();
        assert!(ui.open);
        assert!(matches!(ui.pending, Some(Pending::Close)));
        ui.pending = None;
        assert!(ui.is_dirty());
        ui.request_close();
        ui.confirm_pending().unwrap();
        assert!(!ui.open);
        assert!(!ui.is_dirty());
        assert!(ui.session.is_none());
    }

    #[test]
    fn external_change_and_foreign_lock_refuse_save() {
        let temp = Temp::new();
        let mut ui = temp.load();
        paint_all(&mut ui);
        let lock = temp.0.join(".arm-textures-save.lock");
        fs::write(&lock, b"foreign lock").unwrap();
        assert!(ui.save().is_err());
        assert_eq!(fs::read(&lock).unwrap(), b"foreign lock");
        fs::remove_file(lock).unwrap();
        let path = temp.0.join("boden.txt");
        fs::write(&path, b"external edit").unwrap();
        assert!(ui.save().unwrap_err().contains("外部修改"));
        assert!(ui.is_dirty());
        assert_eq!(fs::read(path).unwrap(), b"external edit");
    }

    #[test]
    fn readonly_and_installed_paths_are_refused() {
        assert!(allow_save(Path::new(
            "C:/Program Files (x86)/Against Rome/MAPS/TEST/boden.txt"
        ))
        .is_err());
        assert!(explicit_map("relative").is_err());
        let temp = Temp::new();
        let mut ui = temp.load();
        paint_all(&mut ui);
        let path = temp.0.join("boden.txt");
        let original = fs::read(&path).unwrap();
        let permissions = fs::metadata(&path).unwrap().permissions();
        let mut readonly = permissions.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&path, readonly).unwrap();
        let result = ui.save();
        fs::set_permissions(&path, permissions).unwrap();
        assert!(result.unwrap_err().contains("唯讀"));
        assert!(ui.is_dirty());
        assert_eq!(fs::read(path).unwrap(), original);
        assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 1);
    }

    #[test]
    fn egui_panel_runs_with_loaded_dirty_session() {
        let temp = Temp::new();
        let mut ui = temp.load();
        paint_all(&mut ui);
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| ui.show(ctx));
        assert!(ui.open);
        assert!(ui.is_dirty());
    }

    #[test]
    fn pfil_paint_save_reload_preserves_container_and_backup() {
        let temp = Temp::new();
        let path = temp.0.join("boden.txt");
        let plain = fs::read(&path).unwrap();
        let mut header = [0u8; 64];
        header[..4].copy_from_slice(b"PFIL");
        header[40..44].copy_from_slice(b"keep");
        let compressed = against_rome_textures::lzss::compress_pfil(&plain, &header).unwrap();
        fs::write(&path, &compressed).unwrap();
        let mut ui = temp.load();
        assert!(ui.document.as_ref().unwrap().is_pfil());
        paint_all(&mut ui);
        let backup = ui.save().unwrap();
        assert_eq!(fs::read(backup).unwrap(), compressed);
        let saved = fs::read(&path).unwrap();
        assert_eq!(&saved[..4], b"PFIL");
        assert_eq!(&saved[40..44], b"keep");
        let reload = temp.load();
        assert_eq!(
            reload.session.as_ref().unwrap().current_textures(),
            &["4BB___51"; 4]
        );
        assert!(!reload.is_dirty());
    }
}

fn allow_save(path: &Path) -> Result<()> {
    if crate::storage::is_forbidden_install_path(path) {
        return Err("遊戲安裝目錄唯讀；請先將 MAP 複製到工作資料夾".into());
    }
    for ancestor in path.ancestors() {
        let metadata = fs::symlink_metadata(ancestor).map_err(|e| e.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err("不可儲存至 symbolic link".into());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err("不可儲存至 junction/reparse point".into());
            }
        }
    }
    if fs::metadata(path)
        .map_err(|e| e.to_string())?
        .permissions()
        .readonly()
    {
        return Err("boden.txt 為唯讀，未寫入任何資料".into());
    }
    Ok(())
}

// Own only files successfully created with create_new; never remove a foreign lock/stage.
struct TemporaryFile {
    path: PathBuf,
    file: Option<fs::File>,
}
impl TemporaryFile {
    fn create(path: PathBuf) -> Result<Self> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Self {
            path,
            file: Some(file),
        })
    }
}
impl Drop for TemporaryFile {
    fn drop(&mut self) {
        drop(self.file.take());
        let _ = fs::remove_file(&self.path);
    }
}

enum Pending {
    Load { map: String, library: String },
    Close,
}

pub struct TexturesUi {
    pub open: bool,
    map_input: String,
    library_input: String,
    loaded_path: Option<PathBuf>,
    original: Vec<u8>,
    document: Option<BodenTexturesDocument>,
    session: Option<TerrainBlendEditSession>,
    materials: Vec<String>,
    selected: String,
    center: [f32; 2],
    radius: f32,
    pending: Option<Pending>,
    status: String,
}
impl Default for TexturesUi {
    fn default() -> Self {
        Self {
            open: false,
            map_input: String::new(),
            library_input: String::new(),
            loaded_path: None,
            original: Vec::new(),
            document: None,
            session: None,
            materials: Vec::new(),
            selected: String::new(),
            center: [0.0; 2],
            radius: 1.0,
            pending: None,
            status: String::new(),
        }
    }
}
impl TexturesUi {
    pub fn is_dirty(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(TerrainBlendEditSession::is_dirty)
    }

    fn load_now(&mut self, map: &str, library: &str) -> Result<()> {
        let map = explicit_map(map)?;
        let path = map.join("boden.txt");
        let original = fs::read(&path).map_err(|e| e.to_string())?;
        let document = BodenTexturesDocument::from_bytes(&original).map_err(|e| e.to_string())?;
        let textures = document.textures();
        let mut names = textures.clone();
        // Explicit library overrides discovery. A missing explicit library is an error.
        let library_path = if library.trim().is_empty() {
            map.ancestors()
                .map(|p| p.join("floortex.dat"))
                .find(|p| p.is_file())
        } else {
            Some(PathBuf::from(library.trim()))
        };
        if let Some(path) = library_path {
            let library = FloortexLibrary::open(path).map_err(|e| e.to_string())?;
            names.extend_from_slice(library.names());
        }
        let catalog = Arc::new(StandardFloorMaterialCatalog::from_texture_names(&names));
        let mut materials = catalog.material_ids();
        materials.sort();
        let fallback = materials.first().cloned().unwrap_or_else(|| "BB".into());
        let import = TerrainBlendAuthoringMap::import(
            document.dimension(),
            &textures,
            catalog.as_ref(),
            &fallback,
        )
        .map_err(|e| e.to_string())?;
        let session =
            TerrainBlendEditSession::new(import, &textures, catalog).map_err(|e| e.to_string())?;
        // Replace state only after the entire load succeeds.
        self.status = format!(
            "已載入 {}；{} 材質，{} 未辨識 tiles，{} corner conflicts",
            path.display(),
            materials.len(),
            session.unresolved_tile_indices().len(),
            session.initial_corner_conflicts().len()
        );
        self.loaded_path = Some(path);
        self.original = original;
        self.document = Some(document);
        self.session = Some(session);
        self.materials = materials;
        self.selected = fallback;
        self.center = [0.0; 2];
        self.pending = None;
        Ok(())
    }

    fn request_load(&mut self) -> Result<()> {
        explicit_map(&self.map_input)?;
        if self.is_dirty() {
            self.pending = Some(Pending::Load {
                map: self.map_input.clone(),
                library: self.library_input.clone(),
            });
            Ok(())
        } else {
            self.load_now(&self.map_input.clone(), &self.library_input.clone())
        }
    }

    fn request_close(&mut self) {
        if self.is_dirty() {
            self.open = true;
            self.pending = Some(Pending::Close);
        } else {
            self.open = false;
        }
    }

    fn confirm_pending(&mut self) -> Result<()> {
        match self.pending.take().ok_or("沒有待確認操作")? {
            Pending::Load { map, library } => self.load_now(&map, &library),
            Pending::Close => {
                self.session = None;
                self.document = None;
                self.loaded_path = None;
                self.original.clear();
                self.open = false;
                Ok(())
            }
        }
    }

    fn paint(&mut self) -> Result<usize> {
        if !self.materials.contains(&self.selected) {
            return Err("請選擇清單內的材質".into());
        }
        let session = self.session.as_mut().ok_or("請先載入 MAP")?;
        let dim = session.tile_dimension() as f32;
        if self
            .center
            .iter()
            .any(|v| !v.is_finite() || *v < 0.0 || *v > dim)
            || !self.radius.is_finite()
            || self.radius <= 0.0
            || self.radius > dim * 2.0
        {
            return Err("筆刷中心或半徑超出範圍".into());
        }
        // Preview on a fork so rejected junctions cannot mutate the active session.
        let mut candidate = session.fork();
        let result = candidate
            .paint_circle(
                self.center[0],
                self.center[1],
                self.radius,
                &self.selected,
                true,
                false,
                None,
            )
            .map_err(|e| e.to_string())?;
        if !result.succeeded {
            return Err(format!("不支援此材質接合，未套用筆刷：{:?}", result.issues));
        }
        let count = result.texture_changes.len();
        candidate.commit_stroke();
        *session = candidate;
        Ok(count)
    }

    fn save(&mut self) -> Result<PathBuf> {
        let path = self.loaded_path.as_ref().ok_or("請先載入 MAP")?;
        allow_save(path)?;
        let _lock = TemporaryFile::create(path.with_file_name(".arm-textures-save.lock"))?;
        if fs::read(path).map_err(|e| e.to_string())? != self.original {
            return Err("來源 boden.txt 已被外部修改；保留未儲存編輯，請另行處理衝突".into());
        }
        let mut document = self.document.as_ref().ok_or("請先載入 MAP")?.clone();
        let session = self.session.as_mut().ok_or("請先載入 MAP")?;
        document
            .set_textures(session.current_textures())
            .map_err(|e| e.to_string())?;
        let bytes = document.to_bytes();
        // Validate serialization before publishing, including PFIL round-trip.
        let verified = BodenTexturesDocument::from_bytes(&bytes).map_err(|e| e.to_string())?;
        if verified.textures() != session.current_textures() {
            return Err("材質序列化驗證失敗".into());
        }
        let mut stage = TemporaryFile::create(path.with_file_name(".arm-textures-stage"))?;
        let file = stage.file.as_mut().unwrap();
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        fs::set_permissions(
            &stage.path,
            fs::metadata(path).map_err(|e| e.to_string())?.permissions(),
        )
        .map_err(|e| e.to_string())?;
        drop(stage.file.take());
        let mut backup = None;
        for n in 0..10000 {
            let candidate = path.with_file_name(format!("boden.txt.arm_backup.{n}"));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
            {
                Ok(mut file) => {
                    file.write_all(&self.original)
                        .and_then(|_| file.sync_all())
                        .map_err(|e| e.to_string())?;
                    backup = Some(candidate);
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        let backup = backup.ok_or("備份檔名稱已用盡")?;
        allow_save(path)?;
        if fs::read(path).map_err(|e| e.to_string())? != self.original {
            return Err("儲存期間來源已變更，未覆寫 boden.txt".into());
        }
        fs::rename(&stage.path, path)
            .map_err(|e| format!("發布失敗：{e}；備份 {}", backup.display()))?;
        self.original = bytes;
        self.document = Some(verified);
        session.commit_baseline();
        Ok(backup)
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        let mut open = self.open;
        egui::Window::new("地圖材質 / Materials")
            .open(&mut open)
            .default_width(580.0)
            .show(ctx, |ui| {
                ui.label("MAP 資料夾（boden.txt）；材質保存與地形 / DATA 分開。");
                ui.text_edit_singleline(&mut self.map_input);
                ui.label("floortex.dat（留空則從 MAP 上層尋找，否則只使用圖內材質）");
                ui.text_edit_singleline(&mut self.library_input);
                if ui.button("載入材質層").clicked() {
                    if let Err(e) = self.request_load() {
                        self.status = e;
                    }
                }
                if let Some(path) = &self.loaded_path {
                    ui.label(format!("目前：{}", path.display()));
                }
                ui.label(if self.is_dirty() {
                    "有未儲存編輯 *"
                } else {
                    "沒有未儲存編輯"
                });
                if self.session.is_some() {
                    egui::ComboBox::from_label("材質")
                        .selected_text(&self.selected)
                        .show_ui(ui, |ui| {
                            for material in &self.materials {
                                ui.selectable_value(&mut self.selected, material.clone(), material);
                            }
                        });
                    let dim = self.session.as_ref().unwrap().tile_dimension() as f32;
                    ui.horizontal(|ui| {
                        ui.label("Corner X / Y");
                        for v in &mut self.center {
                            ui.add(egui::DragValue::new(v).clamp_range(0.0..=dim));
                        }
                        ui.label("半徑");
                        ui.add(egui::DragValue::new(&mut self.radius).clamp_range(0.1..=dim * 2.0));
                    });
                    ui.horizontal(|ui| {
                        if ui.button("套用圓形筆刷").clicked() {
                            self.status = match self.paint() {
                                Ok(n) => format!("已更新 {n} tiles"),
                                Err(e) => e,
                            };
                        }
                        let session = self.session.as_mut().unwrap();
                        if ui
                            .add_enabled(session.can_undo(), egui::Button::new("Undo"))
                            .clicked()
                        {
                            session.undo();
                        }
                        if ui
                            .add_enabled(session.can_redo(), egui::Button::new("Redo"))
                            .clicked()
                        {
                            session.redo();
                        }
                        if ui
                            .add_enabled(self.is_dirty(), egui::Button::new("儲存 boden.txt"))
                            .clicked()
                        {
                            self.status = match self.save() {
                                Ok(p) => format!("已儲存；備份 {}", p.display()),
                                Err(e) => e,
                            };
                        }
                    });
                }
                ui.label(&self.status);
            });
        if !open {
            self.request_close();
        }
        if self.pending.is_some() {
            egui::Window::new("捨棄未儲存材質？")
                .collapsible(false)
                .show(ctx, |ui| {
                    if ui.button("捨棄並繼續").clicked() {
                        if let Err(e) = self.confirm_pending() {
                            self.status = e;
                        }
                    }
                    if ui.button("保留編輯").clicked() {
                        self.pending = None;
                    }
                });
        }
    }
}
