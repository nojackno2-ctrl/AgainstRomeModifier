//! DATA object-family editor. Its Save is independent of terrain/image saving.
use against_rome_placement::{ObjectStore, WorldObject};
use eframe::egui;
use std::{
    fs,
    path::{Path, PathBuf},
};

type Result<T> = std::result::Result<T, String>;

/// World displacement rounded to the nearest signed 16.16 unit. Never saturates.
fn fixed_delta(values: &[String; 3]) -> Result<[i32; 3]> {
    let mut result = [0; 3];
    for axis in 0..3 {
        let world = values[axis]
            .trim()
            .parse::<f64>()
            .map_err(|_| format!("{} 位移不是有效數值", ["X", "Y", "Z"][axis]))?;
        let scaled = world * 65536.0;
        if !world.is_finite()
            || !scaled.is_finite()
            || scaled < i32::MIN as f64
            || scaled > i32::MAX as f64
        {
            return Err(format!(
                "{} 位移超出 signed 16.16 範圍 [-32768, 32767.99998474121]",
                ["X", "Y", "Z"][axis]
            ));
        }
        let rounded = scaled.round();
        if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
            return Err("16.16 四捨五入後超出範圍".into());
        }
        result[axis] = rounded as i32;
    }
    Ok(result)
}

fn explicit_map(text: &str) -> Result<PathBuf> {
    let path = PathBuf::from(text.trim());
    if text.trim().is_empty()
        || !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("請輸入 MAP 資料夾的完整絕對路徑，不接受 ..".into());
    }
    reject_links(&path)?;
    Ok(path)
}
fn reject_links(path: &Path) -> Result<()> {
    for p in path.ancestors() {
        match fs::symlink_metadata(p) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(format!("拒絕 symbolic link: {}", p.display()));
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if metadata.file_attributes() & 0x400 != 0 {
                        return Err(format!("拒絕 junction/reparse point: {}", p.display()));
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{}: {e}", p.display())),
        }
    }
    Ok(())
}
fn allow_save(map: &Path) -> Result<()> {
    reject_links(map)?;
    let text = map.to_string_lossy().replace('/', "\\").to_lowercase();
    if text
        .split('\\')
        .any(|p| p == "program files" || p == "program files (x86)")
    {
        return Err("禁止保存到 Program Files；請先複製 MAP 到工作資料夾".into());
    }
    let canonical = fs::canonicalize(map).map_err(|e| e.to_string())?;
    for variable in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Some(root) = std::env::var_os(variable) {
            if let Ok(root) = fs::canonicalize(root) {
                if canonical.starts_with(root) {
                    return Err("禁止保存到安裝目錄；請操作 MAP 工作副本".into());
                }
            }
        }
    }
    let data = map.join("DATA");
    reject_links(&data)?;
    // Validate every existing DATA entry before the core creates its save lock or stages.
    for entry in fs::read_dir(&data).map_err(|e| e.to_string())? {
        reject_links(&entry.map_err(|e| e.to_string())?.path())?;
    }
    Ok(())
}

#[derive(Clone)]
enum Pending {
    Load(String),
    Close,
    Remove { slot: usize, uid: u32 },
}

pub struct PlacementUi {
    pub open: bool,
    map_input: String,
    loaded_map: Option<PathBuf>,
    store: Option<ObjectStore>,
    selected: Option<(usize, u32)>,
    filter: String,
    delta: [String; 3],
    pending: Option<Pending>,
    status: String,
}
impl Default for PlacementUi {
    fn default() -> Self {
        Self {
            open: false,
            map_input: String::new(),
            loaded_map: None,
            store: None,
            selected: None,
            filter: String::new(),
            delta: ["0".into(), "0".into(), "0".into()],
            pending: None,
            status: String::new(),
        }
    }
}
impl PlacementUi {
    pub fn is_dirty(&self) -> bool {
        self.store.as_ref().is_some_and(ObjectStore::is_dirty)
    }
    fn load_now(&mut self, text: &str) -> Result<()> {
        let map = explicit_map(text)?;
        // Load completely before replacing the current unsaved session.
        let store = ObjectStore::load(&map)?;
        self.store = Some(store);
        self.loaded_map = Some(map);
        self.map_input = text.to_owned();
        self.selected = None;
        self.pending = None;
        Ok(())
    }
    fn request_load(&mut self) -> Result<()> {
        explicit_map(&self.map_input)?;
        if self.is_dirty() {
            self.pending = Some(Pending::Load(self.map_input.clone()));
            Ok(())
        } else {
            self.load_now(&self.map_input.clone())
        }
    }
    fn request_close(&mut self) {
        if self.is_dirty() {
            self.open = true;
            self.pending = Some(Pending::Close);
        } else {
            self.open = false;
            self.pending = None;
        }
    }
    fn confirm_pending(&mut self) -> Result<()> {
        match self.pending.take().ok_or("沒有待確認操作")? {
            Pending::Load(map) => self.load_now(&map),
            Pending::Close => {
                // Closing is an explicit discard; reopening starts with a fresh session.
                self.store = None;
                self.loaded_map = None;
                self.selected = None;
                self.open = false;
                Ok(())
            }
            Pending::Remove { slot, uid } => {
                if self.selected != Some((slot, uid)) {
                    return Err("選擇已變更；請重新確認移除".into());
                }
                self.store
                    .as_mut()
                    .ok_or("請先載入 MAP")?
                    .remove_family(slot, uid)?;
                self.selected = None;
                Ok(())
            }
        }
    }
    fn current(&self) -> Result<(usize, u32)> {
        let (slot, uid) = self.selected.ok_or("請選擇 family root")?;
        let store = self.store.as_ref().ok_or("請先載入 MAP")?;
        // UID identity is checked by the core again on mutation.
        if !store
            .objects()
            .iter()
            .any(|o| o.slot == slot && o.uid == uid)
        {
            return Err("stale object UID".into());
        }
        Ok((slot, uid))
    }
    fn move_family(&mut self) -> Result<()> {
        let (slot, uid) = self.current()?;
        let delta = fixed_delta(&self.delta)?;
        self.store
            .as_mut()
            .unwrap()
            .translate_family_fixed(slot, uid, delta)
    }
    fn clone_family(&mut self) -> Result<()> {
        let (slot, uid) = self.current()?;
        let delta = fixed_delta(&self.delta)?;
        let key = self
            .store
            .as_mut()
            .unwrap()
            .clone_family_fixed(slot, uid, delta)?;
        self.selected = Some(key);
        Ok(())
    }
    fn request_remove(&mut self) -> Result<()> {
        let (slot, uid) = self.current()?;
        self.store.as_ref().unwrap().family(slot, uid)?;
        self.pending = Some(Pending::Remove { slot, uid });
        Ok(())
    }
    fn undo(&mut self) -> Result<()> {
        if !self.store.as_mut().ok_or("請先載入 MAP")?.undo() {
            return Err("沒有可復原操作".into());
        }
        self.check_selection();
        self.pending = None;
        Ok(())
    }
    fn redo(&mut self) -> Result<()> {
        if !self.store.as_mut().ok_or("請先載入 MAP")?.redo() {
            return Err("沒有可重做操作".into());
        }
        self.check_selection();
        self.pending = None;
        Ok(())
    }
    fn check_selection(&mut self) {
        if let Some(key) = self.selected {
            if !self
                .store
                .as_ref()
                .is_some_and(|s| s.objects().iter().any(|o| (o.slot, o.uid) == key))
            {
                self.selected = None;
            }
        }
    }
    fn save(&mut self) -> Result<()> {
        let path = self.loaded_map.as_ref().ok_or("請先載入 MAP")?;
        allow_save(path)?;
        self.store.as_mut().ok_or("請先載入 MAP")?.save()?;
        self.pending = None;
        Ok(())
    }
    fn report(&mut self, result: Result<()>) {
        self.status = match result {
            Ok(()) => "完成（DATA 保存與 terrain 保存獨立）".into(),
            Err(e) => e,
        };
    }
    fn visible(&self, objects: Vec<WorldObject>) -> Vec<WorldObject> {
        let filter = self.filter.trim().to_lowercase();
        objects
            .into_iter()
            .filter(|o| {
                filter.is_empty()
                    || format!(
                        "slot={} uid={} type={} team={} parent={:?}",
                        o.slot, o.uid, o.type_id, o.team, o.parent
                    )
                    .to_lowercase()
                    .contains(&filter)
            })
            .collect()
    }
    pub fn show(&mut self, ctx: &egui::Context) {
        let mut visible = self.open;
        egui::Window::new("Rust 物件 / Family 工具").open(&mut visible)
            .default_width(760.0).show(ctx, |ui| {
            ui.label("DATA 獨立編輯階段：此處 Save 只保存 DATA 物件與關聯池，不保存 terrain、材質或劇本。");
            ui.label("只編輯明確指定的 MAP；安裝目錄允許唯讀載入，保存請使用工作副本。");
            ui.label("MAP 完整路徑（包含 DATA 的資料夾）");
            ui.text_edit_singleline(&mut self.map_input);
            ui.horizontal(|ui| {
                if ui.button("Load DATA").clicked() { let r = self.request_load(); self.report(r); }
                if ui.button("Close tool").clicked() { self.request_close(); }
            });
            if let Some(path) = &self.loaded_map {
                ui.label(format!("實際載入：{} {}", path.display(), if self.is_dirty() {"（未保存）"} else {"（已保存）"}));
            }
            if let Some(store) = &self.store {
                ui.label(format!("Active objects={} / capacity={}",store.objects().len(),store.capacity()));
            }
            ui.horizontal(|ui| {ui.label("Filter slot / UID / type / team / parent");ui.text_edit_singleline(&mut self.filter);});
            let objects = self.visible(self.store.as_ref().map(ObjectStore::objects).unwrap_or_default());
            egui::ScrollArea::vertical().id_source("placement-list").max_height(250.0).show(ui, |ui| {
                for o in objects {
                    let key = (o.slot,o.uid);
                    let label = format!("slot={} UID={} type={} team={} XYZ=({:.4}, {:.4}, {:.4}) parent={:?}",o.slot,o.uid,o.type_id,o.team,o.position[0],o.position[1],o.position[2],o.parent);
                    if ui.selectable_label(self.selected==Some(key),label).clicked() {self.selected=Some(key);if matches!(self.pending,Some(Pending::Remove{..})){self.pending=None;}}
                }
            });
            if let (Some(store),Some((slot,uid))) = (&self.store,self.selected) {
                match store.family(slot,uid) {
                    Ok(family) => {ui.label(format!("整個 family slots: {family:?}"));}
                    Err(error) => {ui.colored_label(egui::Color32::YELLOW,error);}
                }
            }
            ui.label("Move / Clone 的相對 world 位移；X/Y/Z → signed 16.16（四捨五入至 1/65536）。");
            ui.horizontal(|ui| {for axis in 0..3 {ui.label(["ΔX","ΔY","ΔZ"][axis]);ui.add(egui::TextEdit::singleline(&mut self.delta[axis]).desired_width(95.0));}});
            if let Ok(delta)=fixed_delta(&self.delta) {ui.label(format!("16.16 delta: {delta:?}"));}
            ui.label("地圖外座標沿用原有 family 契約，不自動 clamp。未知／不支援關聯由 core 拒絕，錯誤原樣顯示。");
            ui.horizontal_wrapped(|ui| {
                if ui.button("Move family").clicked() {let r=self.move_family();self.report(r);}
                if ui.button("Clone family").clicked() {let r=self.clone_family();self.report(r);}
                if ui.button("Remove family…").clicked() {let r=self.request_remove();self.report(r);}
                let can_undo=self.store.as_ref().is_some_and(ObjectStore::can_undo);
                let can_redo=self.store.as_ref().is_some_and(ObjectStore::can_redo);
                if ui.add_enabled(can_undo,egui::Button::new("Undo")).clicked() {let r=self.undo();self.report(r);}
                if ui.add_enabled(can_redo,egui::Button::new("Redo")).clicked() {let r=self.redo();self.report(r);}
                if ui.add_enabled(self.is_dirty(),egui::Button::new("Save DATA")).clicked() {let r=self.save();self.report(r);}
            });
            ui.separator();ui.label(&self.status);
        });
        // The title-bar close button follows the same dirty guard as the explicit Close tool.
        if self.open && !visible {
            self.request_close();
        } else if self.open {
            self.open = visible;
        }
        if let Some(pending) = self.pending.clone() {
            egui::Window::new("確認物件操作").collapsible(false).resizable(false).show(ctx,|ui| {
                let text=match &pending {
                    Pending::Load(path)=>format!("DATA 尚未保存。捨棄目前物件修改並載入：\n{path}？"),
                    Pending::Close=>"DATA 尚未保存。捨棄目前物件修改並關閉？".into(),
                    Pending::Remove{slot,uid}=>format!("移除 slot={slot} UID={uid} 的完整 family？\n只先變更記憶體，可 Undo；Save DATA 後才寫入工作副本。"),
                };
                ui.label(text);
                ui.horizontal(|ui| {
                    if ui.button("取消／繼續編輯").clicked(){self.pending=None;self.open=true;}
                    if ui.button("確認").clicked(){let r=self.confirm_pending();self.report(r);}
                });
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use against_rome_placement::{COLUMNS, SEGMENTS};
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "ar-placement-ui-{}-{}",
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
    fn fixture(map: &Path) {
        let data = map.join("DATA");
        fs::create_dir_all(&data).unwrap();
        let n = 3usize;
        let mut objects = vec![0u8; 16 + n * (79 + COLUMNS.iter().sum::<usize>())];
        objects[..4].copy_from_slice(&1u32.to_le_bytes());
        objects[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        objects[8..12].copy_from_slice(&30u32.to_le_bytes());
        objects[12..16].copy_from_slice(&30u32.to_le_bytes());
        for slot in 0..n {
            let r = 16 + slot * 79;
            objects[r] = 1;
            objects[r + 3..r + 7].copy_from_slice(&(slot as u32 + 1).to_le_bytes());
            for offset in [67, 69] {
                objects[r + offset..r + offset + 2].copy_from_slice(&(slot as u16).to_le_bytes());
            }
            for offset in [71, 73, 77] {
                objects[r + offset..r + offset + 2].fill(255);
            }
            for col in 0..18 {
                let offset =
                    16 + n * 79 + n * COLUMNS[..col].iter().sum::<usize>() + slot * COLUMNS[col];
                if COLUMNS[col] == 2 {
                    objects[offset..offset + 2].fill(255);
                }
                if col == 1 {
                    objects[offset..offset + 2].copy_from_slice(&(slot as u16).to_le_bytes());
                }
            }
        }
        let mut position = vec![0u8; 8 + n * 17];
        position[..4].copy_from_slice(&1u32.to_le_bytes());
        position[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        for slot in 0..n {
            position[8 + slot * 17] = 1;
            position[9 + slot * 17..13 + slot * 17]
                .copy_from_slice(&(slot as f32 * 20.0).to_le_bytes());
        }
        let mut objdata = vec![0u8; 8 + n * SEGMENTS.iter().sum::<usize>()];
        objdata[..4].copy_from_slice(&1u32.to_le_bytes());
        objdata[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        for (name, bytes) in [
            ("objects", objects),
            ("position", position),
            ("objdata", objdata),
        ] {
            fs::write(data.join(format!("{name}.dat")), bytes).unwrap();
        }
    }
    #[test]
    fn fixed_conversion_rejects_invalid_and_overflow() {
        assert_eq!(
            fixed_delta(&["1".into(), "-0.5".into(), "0.0000152587890625".into()]).unwrap(),
            [65536, -32768, 1]
        );
        for bad in [
            "NaN",
            "inf",
            "-inf",
            "32768",
            "-32768.00001",
            "32767.99999",
            "junk",
            "",
        ] {
            assert!(
                fixed_delta(&[bad.into(), "0".into(), "0".into()]).is_err(),
                "{bad}"
            );
        }
        assert_eq!(
            fixed_delta(&["-32768".into(), "0".into(), "32767.9999847412109375".into()]).unwrap(),
            [i32::MIN, 0, i32::MAX]
        );
    }
    #[test]
    fn temp_data_move_undo_redo_save_reload_and_stale_uid() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi {
            map_input: temp.0.display().to_string(),
            ..Default::default()
        };
        ui.request_load().unwrap();
        ui.selected = Some((1, 2));
        ui.delta = ["1".into(), "-0.5".into(), "0.25".into()];
        ui.move_family().unwrap();
        assert!(ui.is_dirty());
        assert_eq!(
            ui.store.as_ref().unwrap().objects()[1].position,
            [21.0, -0.5, 0.25]
        );
        ui.undo().unwrap();
        assert!(!ui.is_dirty());
        ui.redo().unwrap();
        assert!(ui.is_dirty());
        ui.save().unwrap();
        assert!(!ui.is_dirty());
        assert_eq!(
            ObjectStore::load(&temp.0).unwrap().objects()[1].position,
            [21.0, -0.5, 0.25]
        );
        ui.selected = Some((1, 99));
        assert!(ui.move_family().is_err());
        assert!(!ui.is_dirty());
    }
    #[test]
    fn dirty_load_and_close_require_confirmation_and_failed_load_retains_work() {
        let temp = Temp::new();
        let first = temp.0.join("first");
        let second = temp.0.join("second");
        fixture(&first);
        fixture(&second);
        let mut ui = PlacementUi {
            open: true,
            map_input: first.display().to_string(),
            ..Default::default()
        };
        ui.request_load().unwrap();
        ui.selected = Some((0, 1));
        ui.delta[0] = "2".into();
        ui.move_family().unwrap();
        ui.map_input = second.display().to_string();
        ui.request_load().unwrap();
        assert!(matches!(ui.pending, Some(Pending::Load(_))));
        assert_eq!(ui.loaded_map.as_ref(), Some(&first));
        assert!(ui.is_dirty());
        ui.pending = None;
        ui.request_close();
        assert!(ui.open);
        assert!(matches!(ui.pending, Some(Pending::Close)));
        ui.pending = None;
        ui.map_input = temp.0.join("missing").display().to_string();
        ui.request_load().unwrap();
        assert!(ui.confirm_pending().is_err());
        assert_eq!(ui.loaded_map.as_ref(), Some(&first));
        assert!(ui.is_dirty());
        ui.map_input = second.display().to_string();
        ui.request_load().unwrap();
        ui.confirm_pending().unwrap();
        assert_eq!(ui.loaded_map.as_ref(), Some(&second));
        assert!(!ui.is_dirty());
        ui.selected = Some((0, 1));
        ui.move_family().unwrap();
        ui.request_close();
        ui.confirm_pending().unwrap();
        assert!(!ui.open);
        assert!(ui.store.is_none());
    }
    #[test]
    fn removal_confirmation_binds_selection_and_is_undoable() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();
        ui.selected = Some((0, 1));
        ui.request_remove().unwrap();
        ui.selected = Some((1, 2));
        assert!(ui.confirm_pending().is_err());
        assert_eq!(ui.store.as_ref().unwrap().objects().len(), 3);
        ui.request_remove().unwrap();
        ui.confirm_pending().unwrap();
        assert_eq!(ui.store.as_ref().unwrap().objects().len(), 2);
        ui.undo().unwrap();
        assert_eq!(ui.store.as_ref().unwrap().objects().len(), 3);
    }
    #[test]
    fn save_path_is_bound_to_loaded_map_and_installed_path_refused() {
        assert!(explicit_map("").is_err());
        assert!(explicit_map("relative").is_err());
        assert!(allow_save(Path::new(
            "C:\\Program Files (x86)\\Against Rome\\MAPS\\TEST"
        ))
        .is_err());
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();
        ui.selected = Some((0, 1));
        ui.delta[0] = "1".into();
        ui.move_family().unwrap();
        ui.map_input = temp.0.join("unloaded").display().to_string();
        ui.save().unwrap();
        assert!(!Path::new(&ui.map_input).exists());
        assert_eq!(
            ObjectStore::load(&temp.0).unwrap().objects()[0].position[0],
            1.0
        );
    }

    #[test]
    fn clone_allocates_independent_object_and_core_errors_are_preserved() {
        let temp = Temp::new();
        fixture(&temp.0);
        let objects_path = temp.0.join("DATA/objects.dat");
        let position_path = temp.0.join("DATA/position.dat");
        let mut objects = fs::read(&objects_path).unwrap();
        objects[16 + 2 * 79] = 0;
        fs::write(objects_path, objects).unwrap();
        let mut positions = fs::read(&position_path).unwrap();
        positions[8 + 2 * 17] = 0;
        fs::write(position_path, positions).unwrap();
        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();
        ui.selected = Some((0, 1));
        ui.delta[0] = "7.5".into();
        ui.clone_family().unwrap();
        let (slot, uid) = ui.selected.unwrap();
        assert_eq!(slot, 2);
        let list = ui.store.as_ref().unwrap().objects();
        assert_eq!(
            list.iter().find(|o| o.slot == slot).unwrap().position[0],
            7.5
        );
        assert_eq!(list.iter().find(|o| o.slot == 0).unwrap().position[0], 0.0);
        assert_eq!(list.iter().find(|o| o.slot == slot).unwrap().uid, uid);
        ui.undo().unwrap();
        assert!(!ui.is_dirty());
        assert!(ui.selected.is_none());
        ui.redo().unwrap();
        ui.save().unwrap();
        assert_eq!(ObjectStore::load(&temp.0).unwrap().objects().len(), 3);
        ui.selected = Some((0, 1));
        let expected = ui
            .store
            .as_ref()
            .unwrap()
            .clone()
            .clone_family_fixed(0, 1, [491520, 0, 0])
            .unwrap_err();
        assert_eq!(ui.clone_family().unwrap_err(), expected);
        assert!(!ui.is_dirty());
    }

    #[test]
    fn external_data_change_refuses_save_and_keeps_unsaved_session() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();
        ui.selected = Some((0, 1));
        ui.delta[0] = "1".into();
        ui.move_family().unwrap();
        let path = temp.0.join("DATA/position.dat");
        let mut external = fs::read(&path).unwrap();
        external[9..13].copy_from_slice(&123.0f32.to_le_bytes());
        fs::write(&path, &external).unwrap();
        let error = ui.save().unwrap_err();
        assert!(error.contains("external file change"), "{error}");
        assert!(ui.is_dirty());
        assert_eq!(fs::read(path).unwrap(), external);
        assert_eq!(ui.store.as_ref().unwrap().objects()[0].position[0], 1.0);
    }
}
