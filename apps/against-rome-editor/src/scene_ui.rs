//! Settlement SDL scene editing. Saving is staged into the whole-map `MapSaveTransaction`
//! by `EditorApp`; this module never writes files itself.
use against_rome_maps::{
    MapSceneObject, SdlSceneCatalog, SdlSceneEditService, SdlSceneObjectAddition,
    SdlSceneObjectRemoval,
};
use eframe::egui;
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, String>;

#[derive(Clone)]
struct SceneSnapshot {
    objects: Vec<MapSceneObject>,
    removals: Vec<SdlSceneObjectRemoval>,
    additions: Vec<SdlSceneObjectAddition>,
}

#[derive(Default)]
pub struct SceneUi {
    pub open: bool,
    pub status: String,
    map_dir: Option<PathBuf>,
    baseline: Vec<MapSceneObject>,
    objects: Vec<MapSceneObject>,
    removals: Vec<SdlSceneObjectRemoval>,
    additions: Vec<SdlSceneObjectAddition>,
    undo: Vec<SceneSnapshot>,
    selected: Option<usize>,
    dup_team: i32,
    dup_pos: [f32; 3],
    dup_angle: Option<f32>,
}

impl SceneUi {
    /// Loads the SDL scene of `path`, or resets to empty when `None` (blank map).
    /// A load failure leaves an empty clean scene and reports through `status`.
    pub fn reset_session(&mut self, path: Option<&Path>) {
        let (objects, status) = match path.map(SdlSceneCatalog::load_directory) {
            Some(Ok(objects)) => (objects, String::new()),
            Some(Err(e)) => (Vec::new(), format!("SDL 場景載入失敗：{e}")),
            None => (Vec::new(), String::new()),
        };
        self.map_dir = path.map(Path::to_path_buf).filter(|_| status.is_empty());
        self.baseline = objects.clone();
        self.objects = objects;
        self.removals.clear();
        self.additions.clear();
        self.undo.clear();
        self.selected = None;
        self.status = status;
    }

    pub fn objects(&self) -> &[MapSceneObject] {
        &self.objects
    }

    pub fn removals(&self) -> &[SdlSceneObjectRemoval] {
        &self.removals
    }

    pub fn additions(&self) -> &[SdlSceneObjectAddition] {
        &self.additions
    }

    pub fn map_dir(&self) -> Option<&Path> {
        self.map_dir.as_deref()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn is_dirty(&self) -> bool {
        !self.removals.is_empty()
            || !self.additions.is_empty()
            || SdlSceneEditService::has_changes(&self.baseline, &self.objects)
    }

    pub fn select(&mut self, index: Option<usize>) {
        self.selected = index.filter(|i| *i < self.objects.len());
        if let Some(o) = self.selected.map(|i| &self.objects[i]) {
            self.dup_team = o.team;
            self.dup_pos = [o.local_x, o.local_y, o.local_z];
            self.dup_angle = o.angle;
        }
    }

    fn is_removed(&self, o: &MapSceneObject) -> bool {
        self.removals.iter().any(|r| {
            r.object_index == o.object_index && r.source_file.eq_ignore_ascii_case(&o.source_file)
        })
    }

    fn push_undo(&mut self) {
        self.undo.push(SceneSnapshot {
            objects: self.objects.clone(),
            removals: self.removals.clone(),
            additions: self.additions.clone(),
        });
    }

    fn editable(&self, index: usize) -> Result<()> {
        let o = self.objects.get(index).ok_or("SDL 物件選取無效")?;
        if self.is_removed(o) {
            return Err("該物件已標記移除".into());
        }
        Ok(())
    }

    fn validate_fields(team: i32, pos: [f32; 3], angle: Option<f32>) -> Result<()> {
        if !(-1..=15).contains(&team) {
            return Err("隊伍必須介於 -1 與 15".into());
        }
        if pos.iter().any(|v| !v.is_finite()) {
            return Err("座標必須是有限數值".into());
        }
        if angle.is_some_and(|a| !a.is_finite() || !(-360.0..=360.0).contains(&a)) {
            return Err("角度必須介於 -360 與 360".into());
        }
        Ok(())
    }

    pub fn set_team(&mut self, index: usize, team: i32) -> Result<()> {
        self.editable(index)?;
        let o = &self.objects[index];
        Self::validate_fields(team, [o.local_x, o.local_y, o.local_z], o.angle)?;
        self.push_undo();
        self.objects[index].team = team;
        Ok(())
    }

    pub fn set_position(&mut self, index: usize, pos: [f32; 3]) -> Result<()> {
        self.editable(index)?;
        Self::validate_fields(self.objects[index].team, pos, self.objects[index].angle)?;
        self.push_undo();
        let o = &mut self.objects[index];
        o.local_x = pos[0];
        o.local_y = pos[1];
        o.local_z = pos[2];
        Ok(())
    }

    pub fn set_angle(&mut self, index: usize, angle: Option<f32>) -> Result<()> {
        self.editable(index)?;
        let o = &self.objects[index];
        Self::validate_fields(o.team, [o.local_x, o.local_y, o.local_z], angle)?;
        self.push_undo();
        self.objects[index].angle = angle;
        Ok(())
    }

    /// Marks an existing object for removal (applied at Safe Save).
    pub fn remove_object(&mut self, index: usize) -> Result<()> {
        self.editable(index)?;
        // Pending duplicates that use this object as template would dangle on save.
        let o = &self.objects[index];
        if self.additions.iter().any(|a| {
            a.template_index == o.object_index && a.source_file.eq_ignore_ascii_case(&o.source_file)
        }) {
            return Err("尚有以此物件為模板的待新增複製，請先移除該複製".into());
        }
        self.push_undo();
        let o = &self.objects[index];
        self.removals.push(SdlSceneObjectRemoval {
            source_file: o.source_file.clone(),
            object_index: o.object_index,
        });
        Ok(())
    }

    /// Queues a duplicate of the object at `index` with the given team/position/angle.
    pub fn duplicate_object(
        &mut self,
        index: usize,
        team: i32,
        pos: [f32; 3],
        angle: Option<f32>,
    ) -> Result<()> {
        self.editable(index)?;
        Self::validate_fields(team, pos, angle)?;
        self.push_undo();
        let o = &self.objects[index];
        self.additions.push(SdlSceneObjectAddition {
            source_file: o.source_file.clone(),
            template_index: o.object_index,
            team,
            local_x: pos[0],
            local_y: pos[1],
            local_z: pos[2],
            angle,
            target_file: None,
        });
        Ok(())
    }

    pub fn remove_addition(&mut self, index: usize) -> Result<()> {
        if index >= self.additions.len() {
            return Err("待新增物件選取無效".into());
        }
        self.push_undo();
        self.additions.remove(index);
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        let Some(s) = self.undo.pop() else {
            return false;
        };
        self.objects = s.objects;
        self.removals = s.removals;
        self.additions = s.additions;
        self.selected = self.selected.filter(|i| *i < self.objects.len());
        true
    }

    /// Side-effect-free candidate files (relative path, bytes) for the whole-map transaction.
    /// Fails closed when dirty edits target a directory other than the loaded one.
    pub fn stage_transaction_files(&self, target: &Path) -> Result<Vec<(String, Vec<u8>)>> {
        if !self.is_dirty() {
            return Ok(Vec::new());
        }
        let loaded = self
            .map_dir
            .as_deref()
            .ok_or("SDL scene has no loaded map")?;
        let same = std::fs::canonicalize(loaded)
            .ok()
            .zip(std::fs::canonicalize(target).ok())
            .is_some_and(|(a, b)| a == b);
        if !same {
            return Err("SDL scene save target differs from the loaded map".into());
        }
        let removals = (!self.removals.is_empty()).then_some(self.removals.as_slice());
        let additions = (!self.additions.is_empty()).then_some(self.additions.as_slice());
        Ok(SdlSceneEditService::stage_changes(
            target,
            &self.baseline,
            &self.objects,
            removals,
            additions,
            None,
        )?
        .into_iter()
        .map(|f| (f.relative_path, f.bytes))
        .collect())
    }

    /// Reloads the baseline from disk; call only after the transaction committed.
    pub fn commit_baseline(&mut self, path: &Path) -> Result<()> {
        let objects = SdlSceneCatalog::load_directory(path)?;
        self.baseline = objects.clone();
        self.objects = objects;
        self.removals.clear();
        self.additions.clear();
        self.undo.clear();
        self.selected = None;
        self.map_dir = Some(path.to_path_buf());
        Ok(())
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        let mut open = self.open;
        egui::Window::new("場景 / Scene (SDL)")
            .open(&mut open)
            .default_width(520.0)
            .show(ctx, |ui| self.ui(ui));
        self.open = open;
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        if self.map_dir.is_none() {
            ui.label("尚未載入含 SDL 場景的地圖。");
        }
        ui.horizontal(|ui| {
            ui.label(format!(
                "物件 {}，待移除 {}，待新增 {}{}",
                self.objects.len(),
                self.removals.len(),
                self.additions.len(),
                if self.is_dirty() { " *" } else { "" }
            ));
            if ui
                .add_enabled(self.can_undo(), egui::Button::new("復原"))
                .clicked()
            {
                self.undo();
            }
        });
        let mut result: Result<()> = Ok(());
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .id_source("scene_objects")
            .show(ui, |ui| {
                for i in 0..self.objects.len() {
                    let o = &self.objects[i];
                    let label = format!(
                        "{}{} #{} {} [{}] T{}",
                        if self.is_removed(o) { "✖ " } else { "" },
                        o.source_file,
                        o.object_index,
                        o.name,
                        o.kind(),
                        o.team
                    );
                    if ui
                        .selectable_label(self.selected == Some(i), label)
                        .clicked()
                    {
                        self.select(Some(i));
                    }
                }
            });
        if let Some(i) = self.selected {
            ui.separator();
            let o = self.objects[i].clone();
            let mut team = o.team;
            let mut pos = [o.local_x, o.local_y, o.local_z];
            let mut angle = o.angle;
            ui.horizontal(|ui| {
                ui.label("隊伍");
                ui.add(egui::DragValue::new(&mut team).clamp_range(-1..=15));
                ui.label("XYZ");
                for v in &mut pos {
                    ui.add(egui::DragValue::new(v).speed(0.5));
                }
                let mut has_angle = angle.is_some();
                ui.checkbox(&mut has_angle, "角度");
                if has_angle {
                    let a = angle.get_or_insert(0.0);
                    ui.add(egui::DragValue::new(a).speed(1.0));
                } else {
                    angle = None;
                }
            });
            if team != o.team {
                result = result.and(self.set_team(i, team));
            }
            if pos != [o.local_x, o.local_y, o.local_z] {
                result = result.and(self.set_position(i, pos));
            }
            if angle != o.angle {
                result = result.and(self.set_angle(i, angle));
            }
            if ui.button("移除物件").clicked() {
                result = result.and(self.remove_object(i));
            }
            ui.separator();
            ui.label("複製選取物件");
            ui.horizontal(|ui| {
                ui.label("隊伍");
                ui.add(egui::DragValue::new(&mut self.dup_team).clamp_range(-1..=15));
                ui.label("XYZ");
                for v in &mut self.dup_pos {
                    ui.add(egui::DragValue::new(v).speed(0.5));
                }
                let mut has_angle = self.dup_angle.is_some();
                ui.checkbox(&mut has_angle, "角度");
                if has_angle {
                    ui.add(egui::DragValue::new(self.dup_angle.get_or_insert(0.0)).speed(1.0));
                } else {
                    self.dup_angle = None;
                }
            });
            if ui.button("加入複製").clicked() {
                result = result.and(self.duplicate_object(
                    i,
                    self.dup_team,
                    self.dup_pos,
                    self.dup_angle,
                ));
            }
        }
        if !self.additions.is_empty() {
            ui.separator();
            ui.label("待新增");
            let mut drop_index = None;
            for (n, a) in self.additions.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{} ←#{} T{} ({:.1}, {:.1}, {:.1})",
                        a.source_file, a.template_index, a.team, a.local_x, a.local_y, a.local_z
                    ));
                    if ui.button("取消").clicked() {
                        drop_index = Some(n);
                    }
                });
            }
            if let Some(n) = drop_index {
                result = result.and(self.remove_addition(n));
            }
        }
        if let Err(e) = result {
            self.status = e;
        }
        if !self.status.is_empty() {
            ui.label(&self.status);
        }
        ui.label("儲存請使用 Safe Save；此面板不會直接寫檔。");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use against_rome_maps::{CustomMapManifest, MapSaveTransaction};
    use std::fs;

    const SDL: &str = "[settlement]\r\nrefpos=1000.0,50.0,2000.0\r\n\r\n[object0000]\r\ndef=1700\r\nnamedef=BauRomHau00_Haupthaus\r\npos=100.00,10.00,100.00\r\nteam=0\r\nangle=0.00\r\n\r\n[object0001]\r\ndef=1701\r\nnamedef=FigRomLeg00_Legionaer\r\npos=120.00,10.00,100.00\r\nteam=0\r\n";
    const NAME: &str = "Endlos_01_Siedlung01.sdl";

    fn fixture(tag: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("arm_scene_ui_{tag}_{}", uuid::Uuid::new_v4()));
        let dir = root.join("MAPS").join("ENDL_005");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(CustomMapManifest::MARKER_FILE_NAME), b"").unwrap();
        fs::write(dir.join(NAME), SDL).unwrap();
        dir
    }

    fn cleanup(dir: PathBuf) {
        fs::remove_dir_all(dir.parent().unwrap().parent().unwrap()).unwrap();
    }

    fn commit(dir: &Path, files: &[(String, Vec<u8>)]) {
        let mut tx = MapSaveTransaction::for_custom_map(dir, None).unwrap();
        for (p, b) in files {
            tx.stage_file(p, b.clone()).unwrap();
        }
        tx.commit().unwrap();
    }

    #[test]
    fn loads_and_blank_reset_are_clean() {
        let dir = fixture("load");
        let mut ui = SceneUi::default();
        ui.reset_session(Some(&dir));
        assert_eq!(ui.objects().len(), 2);
        assert!(!ui.is_dirty());
        ui.reset_session(None);
        assert!(ui.objects().is_empty() && !ui.is_dirty() && ui.map_dir().is_none());
        cleanup(dir);
    }

    #[test]
    fn edit_stage_commit_roundtrip_updates_baseline() {
        let dir = fixture("roundtrip");
        let mut ui = SceneUi::default();
        ui.reset_session(Some(&dir));
        let idx = ui
            .objects()
            .iter()
            .position(|o| o.object_index == 0)
            .unwrap();
        let other = 1 - idx;
        ui.set_team(idx, 3).unwrap();
        ui.set_position(idx, [150.5, 10.0, 100.0]).unwrap();
        ui.set_angle(idx, None).unwrap();
        ui.remove_object(other).unwrap();
        ui.duplicate_object(idx, 2, [200.0, 10.0, 200.0], Some(90.0))
            .unwrap();
        assert!(ui.is_dirty());
        assert_eq!(fs::read(dir.join(NAME)).unwrap(), SDL.as_bytes());

        let files = ui.stage_transaction_files(&dir).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(
            fs::read(dir.join(NAME)).unwrap(),
            SDL.as_bytes(),
            "staging is memory-only"
        );
        commit(&dir, &files);
        ui.commit_baseline(&dir).unwrap();

        assert!(!ui.is_dirty() && !ui.can_undo());
        let objs = SdlSceneCatalog::load_directory(&dir).unwrap();
        assert_eq!(objs.len(), 2, "one removed, one duplicated");
        let moved = objs.iter().find(|o| o.team == 3).unwrap();
        assert_eq!((moved.local_x, moved.angle), (150.5, None));
        let dup = objs.iter().find(|o| o.team == 2).unwrap();
        assert_eq!((dup.local_z, dup.angle), (200.0, Some(90.0)));
        assert!(objs.iter().all(|o| !o.name.starts_with("Fig")));
        cleanup(dir);
    }

    #[test]
    fn failed_transaction_keeps_files_dirty_and_undo() {
        let dir = fixture("fail");
        let mut ui = SceneUi::default();
        ui.reset_session(Some(&dir));
        ui.set_team(0, 5).unwrap();
        let files = ui.stage_transaction_files(&dir).unwrap();
        let mut tx = MapSaveTransaction::for_custom_map(&dir, None)
            .unwrap()
            .with_injected_failure(
                against_rome_maps::TransactionFailurePoint::BeforeFinalCommit,
                "scene failure",
            );
        for (p, b) in &files {
            tx.stage_file(p, b.clone()).unwrap();
        }
        assert!(tx.commit().is_err());
        assert_eq!(fs::read(dir.join(NAME)).unwrap(), SDL.as_bytes());
        assert!(ui.is_dirty() && ui.can_undo());
        assert!(ui.undo() && !ui.is_dirty());
        cleanup(dir);
    }

    #[test]
    fn dirty_scene_fails_closed_for_other_target_and_validates_input() {
        let dir = fixture("target");
        let other = fixture("other");
        let mut ui = SceneUi::default();
        ui.reset_session(Some(&dir));
        assert!(ui.stage_transaction_files(&other).unwrap().is_empty());
        assert!(ui.set_team(0, 99).is_err());
        assert!(ui.set_angle(0, Some(f32::NAN)).is_err());
        assert!(!ui.is_dirty());
        ui.set_team(0, 4).unwrap();
        assert!(ui
            .stage_transaction_files(&other)
            .unwrap_err()
            .contains("differs"));
        ui.duplicate_object(0, 1, [0.0; 3], None).unwrap();
        assert!(ui.remove_object(0).unwrap_err().contains("模板"));
        cleanup(dir);
        cleanup(other);
    }
}
