//! Settlement SDL scene editing. Saving is staged into the whole-map `MapSaveTransaction`
//! by `EditorApp`; this module never writes files itself.
use against_rome_maps::{
    MapSceneObject, SdlSceneCatalog, SdlSceneEditService, SdlSceneObjectAddition,
    SdlSceneObjectRemoval, SdlSettlementTranslation, SdlVector3,
};
use against_rome_scene::{move_to_world_position, SceneObjectPosition, Vec3};
use eframe::egui;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SceneSnapshot {
    objects: Vec<MapSceneObject>,
    removals: Vec<SdlSceneObjectRemoval>,
    additions: Vec<SdlSceneObjectAddition>,
    offsets: HashMap<String, SdlVector3>,
}

/// World extent of a map in SDL units (C# `WorldUnitsPerMapPixel * MapPixelSize`).
pub const MAP_WORLD_SIZE: f32 =
    SdlSceneCatalog::WORLD_UNITS_PER_MAP_PIXEL * SdlSceneCatalog::MAP_PIXEL_SIZE;

/// A selectable scene entry: an existing SDL object or a pending addition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneRef {
    Object(usize),
    Addition(usize),
}

/// Display marker of one scene entry at its effective (settlement-shifted) world position.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneMarker {
    pub target: SceneRef,
    /// World units: x, y, z.
    pub world: [f32; 3],
    pub team: i32,
    pub selected: bool,
    pub removed: bool,
}

/// Pointer gestures forwarded from the 3D viewport while move mode is active.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScenePointer {
    /// Press; `hit` is the marker under the cursor (if any).
    Press { hit: Option<SceneRef> },
    /// Cursor moved to terrain world position (x, z) in world units while dragging.
    Drag { world: (f32, f32) },
    Release,
}

#[derive(Clone, Copy)]
struct DragState {
    target: SceneRef,
    pushed: bool,
}

struct TranslateDialog {
    file: String,
    offset: [f32; 3],
}

struct AddDialog {
    templates: Vec<usize>,
    type_index: usize,
    settlements: Vec<String>,
    settlement: usize,
    team: i32,
}

/// Nearest non-removed marker to `pointer` within `radius_px` screen pixels.
pub fn nearest_marker(
    markers: &[SceneMarker],
    pointer: egui::Pos2,
    project: &dyn Fn(&SceneMarker) -> Option<egui::Pos2>,
    radius_px: f32,
) -> Option<SceneRef> {
    markers
        .iter()
        .filter(|m| !m.removed)
        .filter_map(|m| project(m).map(|p| (m.target, p.distance(pointer))))
        .filter(|(_, d)| *d <= radius_px)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(t, _)| t)
}

#[derive(Default)]
pub struct SceneUi {
    pub(crate) history_request: Option<bool>,
    pub open: bool,
    pub status: String,
    map_dir: Option<PathBuf>,
    baseline: Vec<MapSceneObject>,
    objects: Vec<MapSceneObject>,
    removals: Vec<SdlSceneObjectRemoval>,
    additions: Vec<SdlSceneObjectAddition>,
    undo: Vec<SceneSnapshot>,
    redo: Vec<SceneSnapshot>,
    history: against_rome_textures::history::HistoryTokens,
    selected: Option<usize>,
    dup_team: i32,
    dup_pos: [f32; 3],
    dup_angle: Option<f32>,
    /// Move mode (C# `移動場景物件`): pointer drags in the 2D/3D view move the selection.
    pub move_mode: bool,
    /// UI language for the new dialogs/messages (set by the app each frame).
    pub english: bool,
    origins: HashMap<String, SdlVector3>,
    /// Pending absolute settlement shifts relative to the saved `refpos`.
    offsets: HashMap<String, SdlVector3>,
    opening: Vec<MapSceneObject>,
    selected_addition: Option<usize>,
    drag: Option<DragState>,
    translate_dialog: Option<TranslateDialog>,
    add_dialog: Option<AddDialog>,
    confirm_restore: bool,
    scene_filter: String,
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
        self.origins = self
            .map_dir
            .as_deref()
            .and_then(|d| SdlSceneCatalog::load_settlement_origins(d).ok())
            .unwrap_or_default();
        self.baseline = objects.clone();
        self.opening = objects.clone();
        self.objects = objects;
        self.removals.clear();
        self.additions.clear();
        self.offsets.clear();
        self.undo.clear();
        self.redo.clear();
        self.history = Default::default();
        self.selected = None;
        self.selected_addition = None;
        self.drag = None;
        self.translate_dialog = None;
        self.add_dialog = None;
        self.confirm_restore = false;
        self.scene_filter.clear();
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

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_depth(&self) -> usize { self.undo.len() }
    pub fn redo_depth(&self) -> usize { self.redo.len() }
    pub fn history_token(&self) -> u64 { self.history.token() }

    pub fn take_history_receipts(&mut self) -> Vec<crate::undo_journal::SubsystemStep> {
        self.history.take_receipts().into_iter().map(|(before_token, after_token)| {
            crate::undo_journal::SubsystemStep {
                subsystem: crate::undo_journal::SubsystemId::Scene,
                before_token,
                after_token,
            }
        }).collect()
    }

    /// 僅清除重做分支，保留目前內容、基準與復原紀錄。
    pub fn clear_redo(&mut self) {
        self.redo.clear();
        self.history.clear_redo();
    }

    pub fn is_dirty(&self) -> bool {
        !self.removals.is_empty()
            || !self.additions.is_empty()
            || !self.offsets.is_empty()
            || SdlSceneEditService::has_changes(&self.baseline, &self.objects)
    }

    pub fn select(&mut self, index: Option<usize>) {
        self.selected = index.filter(|i| *i < self.objects.len());
        self.selected_addition = None;
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

    pub(crate) fn snapshot(&self) -> SceneSnapshot {
        SceneSnapshot {
            objects: self.objects.clone(),
            removals: self.removals.clone(),
            additions: self.additions.clone(),
            offsets: self.offsets.clone(),
        }
    }

    fn push_undo(&mut self) {
        self.end_drag();
        self.undo.push(self.snapshot());
    }

    /// 所有驗證通過且內容發布後才提交；空操作保留原重做分支。
    fn finish_edit(&mut self) {
        if self.undo.last() == Some(&self.snapshot()) {
            self.undo.pop();
        } else {
            self.clear_redo();
            self.history.commit();
        }
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
        self.finish_edit();
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
        self.finish_edit();
        Ok(())
    }

    pub fn set_angle(&mut self, index: usize, angle: Option<f32>) -> Result<()> {
        self.editable(index)?;
        let o = &self.objects[index];
        Self::validate_fields(o.team, [o.local_x, o.local_y, o.local_z], angle)?;
        self.push_undo();
        self.objects[index].angle = angle;
        self.finish_edit();
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
        self.finish_edit();
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
        self.finish_edit();
        Ok(())
    }

    pub fn remove_addition(&mut self, index: usize) -> Result<()> {
        if index >= self.additions.len() {
            return Err("待新增物件選取無效".into());
        }
        self.push_undo();
        self.additions.remove(index);
        self.selected_addition = match self.selected_addition {
            Some(i) if i == index => None,
            Some(i) if i > index => Some(i - 1),
            other => other,
        };
        self.finish_edit();
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        self.end_drag();
        let Some(s) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.snapshot());
        self.restore_snapshot(s);
        self.history.undo();
        true
    }

    pub fn redo(&mut self) -> bool {
        self.end_drag();
        let Some(s) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.snapshot());
        self.restore_snapshot(s);
        self.history.redo();
        true
    }

    fn restore_snapshot(&mut self, s: SceneSnapshot) {
        self.objects = s.objects;
        self.removals = s.removals;
        self.additions = s.additions;
        self.offsets = s.offsets;
        self.selected = self.selected.filter(|i| *i < self.objects.len());
        self.selected_addition = self
            .selected_addition
            .filter(|i| *i < self.additions.len());
    }

    fn t(&self, zh: &str, en: &str) -> String {
        if self.english { en } else { zh }.to_string()
    }

    fn origin_key(&self, file: &str) -> Option<&String> {
        self.origins.keys().find(|k| k.eq_ignore_ascii_case(file))
    }

    fn origin_of(&self, file: &str) -> Option<SdlVector3> {
        self.origin_key(file).map(|k| self.origins[k])
    }

    /// Pending settlement shift of `file` (zero when none).
    pub fn offset_of(&self, file: &str) -> SdlVector3 {
        self.offsets
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(file))
            .map(|(_, v)| *v)
            .unwrap_or_default()
    }

    /// Saved-refpos world position of an object (`origin + local`; falls back to the loaded world).
    fn object_world(&self, o: &MapSceneObject) -> [f32; 3] {
        match self.origin_of(&o.source_file) {
            Some(r) => [r.x + o.local_x, r.y + o.local_y, r.z + o.local_z],
            None => [o.world_x, o.world_y, o.world_z],
        }
    }

    fn addition_world(&self, a: &SdlSceneObjectAddition) -> Option<[f32; 3]> {
        let r = self.origin_of(a.effective_target_file())?;
        Some([r.x + a.local_x, r.y + a.local_y, r.z + a.local_z])
    }

    fn effective(&self, file: &str, w: [f32; 3]) -> [f32; 3] {
        let d = self.offset_of(file);
        [w[0] + d.x, w[1] + d.y, w[2] + d.z]
    }

    pub fn selected_ref(&self) -> Option<SceneRef> {
        self.selected_addition
            .map(SceneRef::Addition)
            .or(self.selected.map(SceneRef::Object))
    }

    pub fn select_ref(&mut self, target: Option<SceneRef>) {
        match target {
            Some(SceneRef::Object(i)) => self.select(Some(i)),
            Some(SceneRef::Addition(n)) if n < self.additions.len() => {
                self.selected = None;
                self.selected_addition = Some(n);
            }
            _ => {
                self.selected = None;
                self.selected_addition = None;
            }
        }
    }

    /// Display markers (effective world positions) for every scene entry.
    pub fn markers(&self) -> Vec<SceneMarker> {
        let mut out = Vec::with_capacity(self.objects.len() + self.additions.len());
        for (i, o) in self.objects.iter().enumerate() {
            out.push(SceneMarker {
                target: SceneRef::Object(i),
                world: self.effective(&o.source_file, self.object_world(o)),
                team: o.team,
                selected: self.selected == Some(i),
                removed: self.is_removed(o),
            });
        }
        for (n, a) in self.additions.iter().enumerate() {
            if let Some(w) = self.addition_world(a) {
                out.push(SceneMarker {
                    target: SceneRef::Addition(n),
                    world: self.effective(a.effective_target_file(), w),
                    team: a.team,
                    selected: self.selected_addition == Some(n),
                    removed: false,
                });
            }
        }
        out
    }

    /// True while pointer gestures in the 2D/3D views should drive scene editing.
    pub fn scene_pick_active(&self) -> bool {
        self.open && self.move_mode && self.map_dir.is_some()
    }

    /// True while a marker drag is in progress (the brush must not paint).
    pub fn is_capturing(&self) -> bool {
        self.drag.is_some()
    }

    /// Display name of a scene entry.
    fn name_of(&self, target: SceneRef) -> String {
        match target {
            SceneRef::Object(i) => self.objects.get(i).map(|o| o.name.clone()),
            SceneRef::Addition(n) => self.additions.get(n).and_then(|a| {
                self.objects
                    .iter()
                    .find(|o| {
                        o.object_index == a.template_index
                            && o.source_file.eq_ignore_ascii_case(&a.source_file)
                    })
                    .map(|o| o.name.clone())
            }),
        }
        .unwrap_or_default()
    }

    /// Selects `target` and arms a drag (3D/2D press on a marker). No undo step yet.
    pub fn begin_drag(&mut self, target: SceneRef) {
        self.end_drag();
        self.select_ref(Some(target));
        let name = self.name_of(target);
        let movable = match target {
            SceneRef::Object(i) => self.editable(i).is_ok(),
            SceneRef::Addition(n) => {
                n < self.additions.len()
                    && self
                        .origin_of(self.additions[n].effective_target_file())
                        .is_some()
            }
        };
        if movable {
            self.drag = Some(DragState {
                target,
                pushed: false,
            });
            self.status = self.t(
                &format!("已選取 {name}；拖曳即可移動。"),
                &format!("Selected {name}; drag to move it."),
            );
        } else {
            self.drag = None;
            self.status = self.t(
                &format!("{name} 已標記移除，無法移動。"),
                &format!("{name} is marked for removal and cannot be moved."),
            );
        }
    }

    /// Drag update: moves the dragged entry's effective world position to `world` (x, z),
    /// clamped to the map. The first real movement records exactly one undo step.
    pub fn drag_to_world(&mut self, world: (f32, f32)) -> Result<bool> {
        let Some(mut drag) = self.drag else {
            return Ok(false);
        };
        let (file, old_world, local) = match drag.target {
            SceneRef::Object(i) => {
                let o = self.objects.get(i).ok_or("SDL 物件選取無效")?;
                (
                    o.source_file.clone(),
                    self.object_world(o),
                    [o.local_x, o.local_z],
                )
            }
            SceneRef::Addition(n) => {
                let a = self.additions.get(n).ok_or("待新增物件選取無效")?;
                let w = self
                    .addition_world(a)
                    .ok_or("找不到新增物件的目標聚落")?;
                (
                    a.effective_target_file().to_string(),
                    w,
                    [a.local_x, a.local_z],
                )
            }
        };
        let eff = self.effective(&file, old_world);
        let moved = move_to_world_position(
            SceneObjectPosition {
                world: Vec3::new(eff[0], eff[1], eff[2]),
                local_x: local[0],
                local_z: local[1],
                has_native_saved_position: false,
            },
            world.0,
            world.1,
        )
        .map_err(|e| e.to_string())?;
        if moved.local_x == local[0] && moved.local_z == local[1] {
            return Ok(false);
        }
        if !drag.pushed {
            self.push_undo();
            drag.pushed = true;
            self.drag = Some(drag);
        }
        let (dx, dz) = (moved.local_x - local[0], moved.local_z - local[1]);
        match drag.target {
            SceneRef::Object(i) => {
                let o = &mut self.objects[i];
                o.local_x = moved.local_x;
                o.local_z = moved.local_z;
                o.world_x += dx;
                o.world_z += dz;
            }
            SceneRef::Addition(n) => {
                let a = &mut self.additions[n];
                a.local_x = moved.local_x;
                a.local_z = moved.local_z;
            }
        }
        Ok(true)
    }

    pub fn end_drag(&mut self) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        if !drag.pushed {
            return;
        }
        self.finish_edit();
        let m = self.markers().into_iter().find(|m| m.target == drag.target);
        if let Some(m) = m {
            self.status = self.t(
                &format!(
                    "物件已移到世界座標 ({:.0}, {:.0})；按「儲存」才會寫入 SDL。",
                    m.world[0], m.world[2]
                ),
                &format!(
                    "Object moved to world ({:.0}, {:.0}); click Save to write the SDL change.",
                    m.world[0], m.world[2]
                ),
            );
        }
    }

    /// 3D viewport gesture entry point; returns true while the gesture is captured.
    pub fn scene_pointer(&mut self, event: ScenePointer) -> bool {
        if !self.scene_pick_active() {
            self.end_drag();
            return false;
        }
        match event {
            ScenePointer::Press { hit } => {
                if let Some(t) = hit {
                    self.begin_drag(t);
                }
            }
            ScenePointer::Drag { world } => {
                if self.drag.is_some() {
                    if let Err(e) = self.drag_to_world(world) {
                        self.status = e;
                    }
                }
            }
            ScenePointer::Release => self.end_drag(),
        }
        self.drag.is_some()
    }

    /// 2D canvas gestures: a press on a marker selects it and drags it (one undo step per
    /// drag); a press that misses every marker is left to the caller. Returns true when the
    /// gesture is captured by the scene (the brush must not act).
    pub fn scene_canvas_input(
        &mut self,
        response: &egui::Response,
        world_from_screen: &dyn Fn(egui::Pos2) -> (f32, f32),
        screen_from_world: &dyn Fn((f32, f32)) -> egui::Pos2,
    ) -> bool {
        if !self.scene_pick_active() {
            self.end_drag();
            return false;
        }
        let (pressed, down, pointer) = response.ctx.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.interact_pos(),
            )
        });
        if pressed && response.hovered() {
            if let Some(p) = pointer {
                let markers = self.markers();
                let hit = nearest_marker(
                    &markers,
                    p,
                    &|m| Some(screen_from_world((m.world[0], m.world[2]))),
                    10.0,
                );
                if let Some(t) = hit {
                    self.begin_drag(t);
                    return self.drag.is_some();
                }
            }
            false
        } else if self.drag.is_some() {
            if down {
                if let Some(p) = pointer {
                    if let Err(e) = self.drag_to_world(world_from_screen(p)) {
                        self.status = e;
                    }
                }
                true
            } else {
                self.end_drag();
                false
            }
        } else {
            false
        }
    }

    /// Paints every scene entry as a team-colored dot; selection is ringed, removals crossed.
    pub fn paint_scene_overlay(
        &self,
        painter: &egui::Painter,
        screen_from_world: &dyn Fn((f32, f32)) -> egui::Pos2,
    ) {
        if !self.open || self.map_dir.is_none() {
            return;
        }
        for m in self.markers() {
            let p = screen_from_world((m.world[0], m.world[2]));
            let c = against_rome_scene::draw_list::team_color(m.team);
            let fill = egui::Color32::from_rgb(
                (c[0] * 255.0) as u8,
                (c[1] * 255.0) as u8,
                (c[2] * 255.0) as u8,
            );
            painter.circle_filled(p, 4.0, fill);
            painter.circle_stroke(p, 4.0, egui::Stroke::new(1.0_f32, egui::Color32::BLACK));
            if m.selected {
                painter.circle_stroke(p, 8.0, egui::Stroke::new(2.0_f32, egui::Color32::YELLOW));
            }
            if m.removed {
                painter.line_segment(
                    [p - egui::vec2(5.0, 5.0), p + egui::vec2(5.0, 5.0)],
                    egui::Stroke::new(2.0_f32, egui::Color32::RED),
                );
                painter.line_segment(
                    [p + egui::vec2(-5.0, 5.0), p + egui::vec2(5.0, -5.0)],
                    egui::Stroke::new(2.0_f32, egui::Color32::RED),
                );
            }
        }
    }

    /// Delete key (C# `ToggleDeleteSelectedSceneObject`): marks the selected object for removal,
    /// cancels an existing removal, or drops a pending addition.
    pub fn toggle_delete_selected(&mut self) -> Result<()> {
        match self.selected_ref() {
            Some(SceneRef::Addition(n)) => self.remove_addition(n),
            Some(SceneRef::Object(i)) => {
                let o = self.objects.get(i).ok_or("SDL 物件選取無效")?;
                let existing = self.removals.iter().position(|r| {
                    r.object_index == o.object_index
                        && r.source_file.eq_ignore_ascii_case(&o.source_file)
                });
                if let Some(pos) = existing {
                    self.push_undo();
                    self.removals.remove(pos);
                    self.finish_edit();
                    Ok(())
                } else {
                    self.remove_object(i)
                }
            }
            None => Err(self.t("尚未選取場景物件", "No scene object selected")),
        }
    }

    // ----- Settlement shift (C# TranslateSelectedSettlement) -----

    fn selected_settlement_file(&self) -> Option<String> {
        let file = match self.selected_ref()? {
            SceneRef::Object(i) => self.objects.get(i)?.source_file.clone(),
            SceneRef::Addition(n) => self.additions.get(n)?.effective_target_file().to_string(),
        };
        self.origin_key(&file).cloned()
    }

    pub fn can_translate_settlement(&self) -> bool {
        self.selected_settlement_file().is_some()
    }

    fn open_translate_dialog(&mut self) {
        if let Some(file) = self.selected_settlement_file() {
            let d = self.offset_of(&file);
            self.translate_dialog = Some(TranslateDialog {
                file,
                offset: [d.x, d.y, d.z],
            });
        }
    }

    /// Whether shifting `file` by `offset` keeps every object and pending addition in the map
    /// (C# bound check on X/Z against `[0, mapSize]`, removed objects included).
    pub fn translation_in_bounds(&self, file: &str, offset: [f32; 3]) -> bool {
        let inside = |w: [f32; 3]| {
            let (x, z) = (w[0] + offset[0], w[2] + offset[2]);
            (0.0..=MAP_WORLD_SIZE).contains(&x) && (0.0..=MAP_WORLD_SIZE).contains(&z)
        };
        self.objects
            .iter()
            .filter(|o| o.source_file.eq_ignore_ascii_case(file))
            .all(|o| inside(self.object_world(o)))
            && self
                .additions
                .iter()
                .filter(|a| a.effective_target_file().eq_ignore_ascii_case(file))
                .filter_map(|a| self.addition_world(a))
                .all(inside)
    }

    /// Sets the absolute shift of settlement `file` (zero removes it). Rejects non-finite values
    /// and shifts that push any part of the settlement outside the map.
    pub fn translate_settlement(&mut self, file: &str, offset: [f32; 3]) -> Result<()> {
        let key = self
            .origin_key(file)
            .cloned()
            .ok_or_else(|| self.t("找不到聚落", "Settlement not found"))?;
        if offset.iter().any(|v| !v.is_finite()) {
            return Err(self.t("平移量必須是有限數值", "Offsets must be finite numbers"));
        }
        if !self.translation_in_bounds(&key, offset) {
            return Err(self.t(
                "此平移量會讓部分聚落物件超出地圖範圍，已取消。",
                "This offset would move part of the settlement outside the map.",
            ));
        }
        let next = SdlVector3::new(offset[0], offset[1], offset[2]);
        if next == self.offset_of(&key) {
            return Ok(());
        }
        self.push_undo();
        self.offsets.retain(|k, _| !k.eq_ignore_ascii_case(&key));
        if next != SdlVector3::default() {
            self.offsets.insert(key, next);
        }
        self.finish_edit();
        Ok(())
    }

    // ----- Add object by type (C# AddSceneObjectFromCatalog) -----

    /// Template objects: first non-removed object per name (case-insensitive), ordered by
    /// kind then name. Values are indices into `objects()`.
    pub fn catalog_templates(&self) -> Vec<usize> {
        let mut seen = std::collections::HashSet::new();
        let mut v: Vec<usize> = (0..self.objects.len())
            .filter(|&i| !self.is_removed(&self.objects[i]))
            .filter(|&i| seen.insert(self.objects[i].name.to_ascii_lowercase()))
            .collect();
        v.sort_by(|&a, &b| {
            let (x, y) = (&self.objects[a], &self.objects[b]);
            x.kind()
                .cmp(y.kind())
                .then_with(|| x.name.to_ascii_lowercase().cmp(&y.name.to_ascii_lowercase()))
        });
        v
    }

    /// Settlement file names, sorted case-insensitively.
    pub fn settlement_files(&self) -> Vec<String> {
        let mut v: Vec<String> = self.origins.keys().cloned().collect();
        v.sort_by_key(|s| s.to_ascii_lowercase());
        v
    }

    /// Most common team among the objects of `file` (default -1), clamped to -1..=15.
    pub fn dominant_team(&self, file: &str) -> i32 {
        let mut counts: Vec<(i32, usize)> = Vec::new();
        for o in self
            .objects
            .iter()
            .filter(|o| o.source_file.eq_ignore_ascii_case(file))
        {
            match counts.iter_mut().find(|(t, _)| *t == o.team) {
                Some(c) => c.1 += 1,
                None => counts.push((o.team, 1)),
            }
        }
        counts
            .iter()
            .max_by_key(|(_, n)| *n)
            .map(|(t, _)| (*t).clamp(-1, 15))
            .unwrap_or(-1)
    }

    pub fn can_add_object(&self) -> bool {
        !self.objects.is_empty() && !self.origins.is_empty()
    }

    fn open_add_dialog(&mut self) {
        if !self.can_add_object() {
            return;
        }
        let templates = self.catalog_templates();
        let settlements = self.settlement_files();
        let current = self.selected_settlement_file();
        let settlement = current
            .as_ref()
            .and_then(|f| settlements.iter().position(|s| s == f))
            .unwrap_or(0);
        let type_index = match self.selected {
            Some(i) => templates
                .iter()
                .position(|&t| {
                    self.objects[t]
                        .name
                        .eq_ignore_ascii_case(&self.objects[i].name)
                })
                .unwrap_or(0),
            None => 0,
        };
        let team = settlements
            .get(settlement)
            .map(|f| self.dominant_team(f))
            .unwrap_or(-1);
        self.add_dialog = Some(AddDialog {
            templates,
            type_index,
            settlements,
            settlement,
            team,
        });
    }

    /// Queues a new object cloned from `objects()[template]` into settlement `target_file`,
    /// 96 world units from the settlement origin (clamped to the map), selecting it.
    pub fn add_object_from_catalog(
        &mut self,
        template: usize,
        target_file: &str,
        team: i32,
    ) -> Result<usize> {
        let t = self
            .objects
            .get(template)
            .ok_or("SDL 物件選取無效")?
            .clone();
        if self.is_removed(&t) {
            return Err(self.t("模板物件已標記移除", "The template object is marked for removal"));
        }
        if !(-1..=15).contains(&team) {
            return Err("隊伍必須介於 -1 與 15".into());
        }
        let file = self
            .origin_key(target_file)
            .cloned()
            .ok_or_else(|| self.t("找不到目標聚落", "Target settlement not found"))?;
        let origin = self.origins[&file];
        const OFFSET: f32 = 96.0;
        let wx = (origin.x + OFFSET).clamp(0.0, MAP_WORLD_SIZE);
        let wz = (origin.z + OFFSET).clamp(0.0, MAP_WORLD_SIZE);
        self.push_undo();
        self.additions.push(SdlSceneObjectAddition {
            source_file: t.source_file.clone(),
            template_index: t.object_index,
            team,
            local_x: wx - origin.x,
            local_y: 0.0,
            local_z: wz - origin.z,
            angle: t.angle,
            target_file: Some(file.clone()),
        });
        self.finish_edit();
        let n = self.additions.len() - 1;
        self.select_ref(Some(SceneRef::Addition(n)));
        self.status = self.t(
            &format!(
                "已將 {} 暫存新增到 {file}；可拖曳或輸入座標，按「儲存」才會寫入。",
                t.name
            ),
            &format!(
                "Added {} to {file}; drag it or type a position, then Save.",
                t.name
            ),
        );
        Ok(n)
    }

    // ----- Restore (C# RestoreOpeningSceneObjects) -----

    /// True when there is anything for "restore to opening" to undo.
    pub fn can_restore(&self) -> bool {
        self.is_dirty()
    }

    /// Restores every SDL edit (object fields, removals, additions, settlement shifts) to the
    /// state captured when the map was opened (or last saved). Recorded as one undo step.
    pub fn restore_to_open(&mut self) -> bool {
        if !self.can_restore() {
            return false;
        }
        self.push_undo();
        let opening: HashMap<(String, i32), &MapSceneObject> = self
            .opening
            .iter()
            .map(|o| ((o.source_file.to_ascii_uppercase(), o.object_index), o))
            .collect();
        // Object set equals the baseline; restore each to its opening values when known.
        let restored: Vec<MapSceneObject> = self
            .baseline
            .iter()
            .map(|b| {
                opening
                    .get(&(b.source_file.to_ascii_uppercase(), b.object_index))
                    .map(|o| (*o).clone())
                    .unwrap_or_else(|| b.clone())
            })
            .collect();
        self.objects = restored;
        self.removals.clear();
        self.additions.clear();
        self.offsets.clear();
        self.drag = None;
        self.selected = self.selected.filter(|i| *i < self.objects.len());
        self.selected_addition = None;
        self.finish_edit();
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
        let mut translations: Vec<SdlSettlementTranslation> = self
            .offsets
            .iter()
            .map(|(file, d)| SdlSettlementTranslation {
                source_file: file.clone(),
                delta_x: d.x,
                delta_y: d.y,
                delta_z: d.z,
            })
            .collect();
        translations.sort_by(|a, b| a.source_file.cmp(&b.source_file));
        let translations = (!translations.is_empty()).then_some(translations.as_slice());
        Ok(SdlSceneEditService::stage_changes(
            target,
            &self.baseline,
            &self.objects,
            removals,
            additions,
            translations,
        )?
        .into_iter()
        .map(|f| (f.relative_path, f.bytes))
        .collect())
    }

    /// Reloads the baseline from disk; call only after the transaction committed.
    pub fn commit_baseline(&mut self, path: &Path) -> Result<()> {
        let objects = SdlSceneCatalog::load_directory(path)?;
        self.origins = SdlSceneCatalog::load_settlement_origins(path)?;
        self.baseline = objects.clone();
        self.opening = objects.clone();
        self.objects = objects;
        self.removals.clear();
        self.additions.clear();
        self.offsets.clear();
        self.undo.clear();
        self.redo.clear();
        self.history = Default::default();
        self.selected = None;
        self.selected_addition = None;
        self.drag = None;
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
        self.show_dialogs(ctx);
    }

    fn show_dialogs(&mut self, ctx: &egui::Context) {
        if self.translate_dialog.is_some() {
            let mut dlg = self.translate_dialog.take().unwrap();
            let mut keep = true;
            let mut apply = false;
            egui::Window::new(self.t(
                &format!("平移聚落 - {}", dlg.file),
                &format!("Move Settlement - {}", dlg.file),
            ))
            .id(egui::Id::new("scene_translate_dialog"))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                for (n, label) in [
                    self.t("平移 X", "Offset X"),
                    self.t("平移 Y", "Offset Y"),
                    self.t("平移 Z", "Offset Z"),
                ]
                .into_iter()
                .enumerate()
                {
                    ui.horizontal(|ui| {
                        ui.label(label);
                        ui.add(egui::DragValue::new(&mut dlg.offset[n]).speed(1.0));
                    });
                }
                ui.horizontal(|ui| {
                    if ui.button(self.t("確定", "OK")).clicked() {
                        apply = true;
                    }
                    if ui.button(self.t("取消", "Cancel")).clicked() {
                        keep = false;
                    }
                });
            });
            if apply {
                match self.translate_settlement(&dlg.file.clone(), dlg.offset) {
                    Ok(()) => keep = false,
                    Err(e) => {
                        self.status = e;
                        keep = false;
                    }
                }
            }
            if keep {
                self.translate_dialog = Some(dlg);
            }
        }
        if self.add_dialog.is_some() {
            let mut dlg = self.add_dialog.take().unwrap();
            let mut keep = true;
            let mut apply = false;
            let before = dlg.settlement;
            egui::Window::new(self.t("新增場景物件", "Add Scene Object"))
                .id(egui::Id::new("scene_add_dialog"))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    let type_label = |this: &Self, i: usize| {
                        this.objects
                            .get(i)
                            .map(|o| format!("[{}] {}", kind_text(o.kind(), this.english), o.name))
                            .unwrap_or_default()
                    };
                    ui.horizontal(|ui| {
                        ui.label(self.t("物件類型", "Type"));
                        let current = dlg
                            .templates
                            .get(dlg.type_index)
                            .map(|&i| type_label(self, i))
                            .unwrap_or_default();
                        egui::ComboBox::from_id_source("scene_add_type")
                            .selected_text(current)
                            .show_ui(ui, |ui| {
                                for (n, &i) in dlg.templates.iter().enumerate() {
                                    ui.selectable_value(&mut dlg.type_index, n, type_label(self, i));
                                }
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.label(self.t("目標聚落", "Settlement"));
                        egui::ComboBox::from_id_source("scene_add_settlement")
                            .selected_text(
                                dlg.settlements.get(dlg.settlement).cloned().unwrap_or_default(),
                            )
                            .show_ui(ui, |ui| {
                                for (n, f) in dlg.settlements.iter().enumerate() {
                                    ui.selectable_value(&mut dlg.settlement, n, f);
                                }
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.label(self.t("隊伍", "Team"));
                        ui.add(egui::DragValue::new(&mut dlg.team).clamp_range(-1..=15));
                    });
                    ui.horizontal(|ui| {
                        if ui.button(self.t("確定", "OK")).clicked() {
                            apply = true;
                        }
                        if ui.button(self.t("取消", "Cancel")).clicked() {
                            keep = false;
                        }
                    });
                });
            if dlg.settlement != before {
                if let Some(f) = dlg.settlements.get(dlg.settlement) {
                    dlg.team = self.dominant_team(f);
                }
            }
            if apply {
                keep = false;
                if let (Some(&tpl), Some(file)) = (
                    dlg.templates.get(dlg.type_index),
                    dlg.settlements.get(dlg.settlement).cloned(),
                ) {
                    if let Err(e) = self.add_object_from_catalog(tpl, &file, dlg.team) {
                        self.status = e;
                    }
                }
            }
            if keep {
                self.add_dialog = Some(dlg);
            }
        }
        if self.confirm_restore {
            let mut yes = false;
            let mut no = false;
            egui::Window::new(self.t("還原 SDL 驗證變更", "Restore SDL Verification Changes"))
                .id(egui::Id::new("scene_restore_dialog"))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(self.t(
                        "要將所有待儲存的 SDL 變更（隊伍、位置、角度、待新增、待刪除與聚落平移）還原到本次開啟地圖時的狀態嗎？\n還原後仍需按「儲存」才會寫回自製地圖。",
                        "Do you want to restore all unsaved SDL edits (teams, positions, angles, pending additions, deletions and settlement moves) to the state when this map was opened?\nYou still need to click \"Save\" to write them back to the custom map.",
                    ));
                    ui.horizontal(|ui| {
                        yes = ui.button(self.t("是", "Yes")).clicked();
                        no = ui.button(self.t("否", "No")).clicked();
                    });
                });
            if yes {
                self.restore_to_open();
            }
            if yes || no {
                self.confirm_restore = false;
            }
        }
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
                .add_enabled(self.can_undo(), egui::Button::new(self.t("復原", "Undo")))
                .clicked()
            {
                self.history_request = Some(false);
            }
            if ui
                .add_enabled(self.can_redo(), egui::Button::new(self.t("重做", "Redo")))
                .clicked()
            {
                self.history_request = Some(true);
            }
        });
        ui.horizontal_wrapped(|ui| {
            let move_label = self.t("移動場景物件", "Move scene objects");
            ui.checkbox(&mut self.move_mode, move_label)
                .on_hover_text(self.t(
                    "選取後在 2D / 3D 檢視拖曳；Delete 切換刪除。移動會暫存到 Safe Save。",
                    "Click and drag objects in the 2D/3D view; Delete toggles removal. Moves stay pending until Safe Save.",
                ));
            if ui
                .add_enabled(
                    self.can_translate_settlement(),
                    egui::Button::new(self.t("平移聚落…", "Move Settlement…")),
                )
                .clicked()
            {
                self.open_translate_dialog();
            }
            if ui
                .add_enabled(
                    self.can_add_object(),
                    egui::Button::new(self.t("新增場景物件…", "Add Object…")),
                )
                .clicked()
            {
                self.open_add_dialog();
            }
            if ui
                .add_enabled(
                    self.can_restore(),
                    egui::Button::new(self.t("還原到本次開啟時", "Restore to Initial")),
                )
                .clicked()
            {
                self.confirm_restore = true;
            }
        });
        let mut result: Result<()> = Ok(());
        let query = self.scene_filter.trim().to_lowercase();
        let visible_objects: Vec<_> = self
            .objects
            .iter()
            .enumerate()
            .filter_map(|(index, object)| {
                matches_scene_object_filter(object, self.is_removed(object), &query)
                    .then_some(index)
            })
            .collect();
        let filter_hint = self.t(
            "篩選檔案、名稱、類型、隊伍或編號",
            "Filter file, name, kind, team or index",
        );
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.scene_filter)
                    .hint_text(filter_hint),
            );
            ui.label(format!("{}/{}", visible_objects.len(), self.objects.len()));
        });
        if visible_objects.is_empty() {
            ui.label(self.t("沒有符合的 SDL 物件。", "No SDL objects match this filter."));
        } else {
            let row_height = ui.text_style_height(&egui::TextStyle::Body);
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .id_source("scene_objects")
                .show_rows(ui, row_height, visible_objects.len(), |ui, rows| {
                    for row in rows {
                        let i = visible_objects[row];
                        let label = {
                            let o = &self.objects[i];
                            format!(
                                "{}{} #{} {} [{}] T{}",
                                if self.is_removed(o) { "✖ " } else { "" },
                                o.source_file,
                                o.object_index,
                                o.name,
                                o.kind(),
                                o.team
                            )
                        };
                        if ui
                            .selectable_label(self.selected == Some(i), label)
                            .clicked()
                        {
                            self.select(Some(i));
                        }
                    }
                });
        }
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
            let mut pick = None;
            for (n, a) in self.additions.iter().enumerate() {
                ui.horizontal(|ui| {
                    let label = format!(
                        "{} ←#{} → {} T{} ({:.1}, {:.1}, {:.1})",
                        a.source_file,
                        a.template_index,
                        a.effective_target_file(),
                        a.team,
                        a.local_x,
                        a.local_y,
                        a.local_z
                    );
                    if ui
                        .selectable_label(self.selected_addition == Some(n), label)
                        .clicked()
                    {
                        pick = Some(n);
                    }
                    if ui.button("取消").clicked() {
                        drop_index = Some(n);
                    }
                });
            }
            if let Some(n) = pick {
                self.select_ref(Some(SceneRef::Addition(n)));
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

fn kind_text(kind: &str, english: bool) -> String {
    if !english {
        return kind.to_string();
    }
    match kind {
        "建築" => "Building",
        "單位" => "Unit",
        _ => "Other",
    }
    .to_string()
}

fn matches_scene_object_filter(object: &MapSceneObject, removed: bool, query: &str) -> bool {
    let normalized_query = query.to_lowercase();
    let terms: Vec<_> = normalized_query.split_whitespace().collect();
    if terms.is_empty() {
        return true;
    }
    let kind_english = match object.kind() {
        "建築" => "building",
        "單位" => "unit",
        _ => "object",
    };
    let searchable = format!(
        "{} #{} id{} {} {} {} {} T{} {}",
        object.source_file,
        object.object_index,
        object.object_index,
        object.name,
        object.kind(),
        kind_english,
        object.team,
        object.team,
        if removed { "removed 待移除" } else { "" },
    )
    .to_lowercase();
    terms.iter().all(|term| searchable.contains(term))
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
    fn step3b_exact_snapshots_and_tokens_round_trip() {
        let (mut ui, dir) = loaded("step3b_exact");
        let mut states = vec![(ui.snapshot(), ui.history_token(), ui.is_dirty())];
        macro_rules! edit {
            ($edit:expr) => {{
                $edit;
                states.push((ui.snapshot(), ui.history_token(), ui.is_dirty()));
            }};
        }
        edit!(ui.set_team(0, 3).unwrap());
        edit!(ui.set_position(0, [140.0, 12.0, 160.0]).unwrap());
        edit!(ui.set_angle(0, Some(90.0)).unwrap());
        edit!(ui.duplicate_object(0, 2, [200.0, 0.0, 200.0], None).unwrap());
        edit!({
            ui.begin_drag(SceneRef::Addition(0));
            ui.drag_to_world((1300.0, 2300.0)).unwrap();
            ui.drag_to_world((1400.0, 2400.0)).unwrap();
            ui.end_drag();
        });
        edit!(ui.translate_settlement(NAME, [20.0, 5.0, 30.0]).unwrap());
        edit!(ui.remove_addition(0).unwrap());
        edit!(ui.remove_object(1).unwrap());
        edit!({ ui.select(Some(1)); ui.toggle_delete_selected().unwrap(); });
        edit!(ui.add_object_from_catalog(0, NAME, 4).unwrap());
        edit!(assert!(ui.restore_to_open()));
        let receipts = ui.take_history_receipts();
        assert_eq!(receipts.len(), states.len() - 1);
        for (receipt, pair) in receipts.iter().zip(states.windows(2)) {
            assert_eq!(receipt.subsystem, crate::undo_journal::SubsystemId::Scene);
            assert_eq!((receipt.before_token, receipt.after_token), (pair[0].1, pair[1].1));
        }
        for (snapshot, token, dirty) in states.iter().rev().skip(1) {
            assert!(ui.undo());
            assert_eq!(&ui.snapshot(), snapshot);
            assert_eq!(ui.history_token(), *token);
            assert_eq!(ui.is_dirty(), *dirty);
        }
        assert!(!ui.undo());
        for (snapshot, token, dirty) in states.iter().skip(1) {
            assert!(ui.redo());
            assert_eq!(&ui.snapshot(), snapshot);
            assert_eq!(ui.history_token(), *token);
            assert_eq!(ui.is_dirty(), *dirty);
        }
        assert!(!ui.redo());
        assert!(ui.take_history_receipts().is_empty());
        cleanup(dir);
    }

    #[test]
    fn scene_object_filter_searches_file_name_kind_team_index_and_removal() {
        let (ui, dir) = loaded("scene_filter");
        let object = &ui.objects()[0];
        assert!(matches_scene_object_filter(object, false, "bauromhau00"));
        assert!(matches_scene_object_filter(object, false, "ENDLOS_01_SIEDLUNG01.SDL BUILDING"));
        assert!(matches_scene_object_filter(object, false, "T0 #0"));
        assert!(!matches_scene_object_filter(object, false, "figrom"));
        assert!(matches_scene_object_filter(object, true, "removed"));
        assert!(!matches_scene_object_filter(object, false, "removed"));
        assert!(matches_scene_object_filter(object, false, "   "));
        cleanup(dir);
    }

    #[test]
    fn step3b_drag_navigation_and_deactivation_finalize_one_receipt() {
        let (mut ui, dir) = loaded("step3b_drag_navigation");
        ui.begin_drag(SceneRef::Object(0));
        ui.drag_to_world((1300.0, 2300.0)).unwrap();
        let edited = ui.snapshot();
        // 拖曳中復原會先提交該手勢，再復原同一步。
        assert!(ui.undo() && !ui.is_dirty() && !ui.is_capturing());
        assert_eq!(ui.take_history_receipts().len(), 1);
        assert!(ui.redo());
        assert_eq!(ui.snapshot(), edited);
        assert!(ui.take_history_receipts().is_empty());
        ui.begin_drag(SceneRef::Object(0));
        ui.drag_to_world((1400.0, 2400.0)).unwrap();
        ui.open = false;
        assert!(!ui.scene_pointer(ScenePointer::Release));
        assert!(!ui.is_capturing());
        assert_eq!(ui.take_history_receipts().len(), 1);
        assert!(ui.undo());
        assert_eq!(ui.snapshot(), edited);
        cleanup(dir);
    }

    #[test]
    fn step3b_branch_and_redo_only_clear_preserve_undo() {
        let (mut ui, dir) = loaded("step3b_branch");
        ui.set_team(0, 1).unwrap();
        let first = (ui.snapshot(), ui.history_token());
        ui.set_team(0, 2).unwrap();
        let abandoned = ui.history_token();
        assert!(ui.undo());
        ui.set_angle(0, Some(45.0)).unwrap();
        assert_ne!(ui.history_token(), abandoned);
        assert!(!ui.can_redo() && !ui.redo());
        assert!(ui.undo());
        ui.take_history_receipts();
        ui.clear_redo();
        assert_eq!((ui.undo_depth(), ui.redo_depth()), (1, 0));
        assert_eq!((ui.snapshot(), ui.history_token()), first);
        assert!(ui.is_dirty());
        assert!(!ui.redo());
        assert!(ui.undo());
        assert!(!ui.is_dirty());
        assert!(ui.take_history_receipts().is_empty());
        cleanup(dir);
    }

    #[test]
    fn step3b_noop_and_rejected_edits_preserve_redo_without_receipts() {
        let (mut ui, dir) = loaded("step3b_noop");
        assert!(!ui.undo() && !ui.redo() && !ui.restore_to_open());
        ui.set_team(0, 2).unwrap();
        assert!(ui.undo());
        ui.take_history_receipts();
        let token = ui.history_token();
        let object = ui.objects[0].clone();
        ui.set_team(0, object.team).unwrap();
        ui.set_position(0, [object.local_x, object.local_y, object.local_z]).unwrap();
        ui.set_angle(0, object.angle).unwrap();
        ui.translate_settlement(NAME, [0.0; 3]).unwrap();
        assert!(ui.set_team(0, 16).is_err());
        assert!(ui.set_position(0, [f32::NAN; 3]).is_err());
        assert!(ui.remove_addition(0).is_err());
        assert!(ui.translate_settlement(NAME, [f32::INFINITY; 3]).is_err());
        ui.begin_drag(SceneRef::Object(0));
        ui.end_drag();
        ui.begin_drag(SceneRef::Object(0));
        let world = ui.markers()[0].world;
        assert!(!ui.drag_to_world((world[0], world[2])).unwrap());
        assert!(ui.drag_to_world((world[0] + 100.0, world[2])).unwrap());
        assert!(ui.take_history_receipts().is_empty());
        ui.drag_to_world((world[0], world[2])).unwrap();
        ui.end_drag();
        assert_eq!(ui.history_token(), token);
        assert_eq!((ui.undo_depth(), ui.redo_depth()), (0, 1));
        assert!(!ui.is_dirty());
        assert!(ui.take_history_receipts().is_empty());
        assert!(ui.redo() && ui.is_dirty());
        cleanup(dir);
    }

    #[test]
    fn step3b_dirty_baseline_save_failure_and_success() {
        let (mut ui, dir) = loaded("step3b_baseline");
        ui.set_team(0, 3).unwrap();
        let edited = ui.snapshot();
        let token = ui.history_token();
        assert!(ui.commit_baseline(&dir.join("missing")).is_err());
        assert_eq!((ui.snapshot(), ui.history_token()), (edited.clone(), token));
        assert!(ui.is_dirty() && ui.undo() && !ui.is_dirty());
        assert!(ui.redo() && ui.is_dirty());
        assert_eq!(ui.snapshot(), edited);
        let files = ui.stage_transaction_files(&dir).unwrap();
        commit(&dir, &files);
        ui.commit_baseline(&dir).unwrap();
        assert!(!ui.is_dirty() && !ui.can_undo() && !ui.can_redo());
        assert!(ui.take_history_receipts().is_empty());
        ui.set_team(0, 4).unwrap();
        assert!(ui.is_dirty() && ui.undo() && !ui.is_dirty());
        assert_eq!(ui.objects[0].team, 3);
        assert!(ui.redo() && ui.is_dirty());
        assert_eq!(ui.objects[0].team, 4);
        ui.reset_session(Some(&dir));
        assert!(!ui.is_dirty() && !ui.can_undo() && !ui.can_redo());
        assert!(ui.take_history_receipts().is_empty());
        cleanup(dir);
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

    fn loaded(tag: &str) -> (SceneUi, PathBuf) {
        let dir = fixture(tag);
        let mut ui = SceneUi::default();
        ui.english = true;
        ui.reset_session(Some(&dir));
        ui.open = true;
        ui.move_mode = true;
        (ui, dir)
    }

    #[test]
    fn nearest_marker_picks_closest_and_skips_removed() {
        let m = |i, x: f32, removed| SceneMarker {
            target: SceneRef::Object(i),
            world: [x, 0.0, 0.0],
            team: 0,
            selected: false,
            removed,
        };
        let markers = [m(0, 10.0, false), m(1, 14.0, false), m(2, 12.0, true)];
        let project = |mk: &SceneMarker| Some(egui::pos2(mk.world[0], 0.0));
        let pick = |x: f32| nearest_marker(&markers, egui::pos2(x, 0.0), &project, 5.0);
        assert_eq!(pick(11.0), Some(SceneRef::Object(0)));
        assert_eq!(pick(13.0), Some(SceneRef::Object(1)));
        assert_eq!(pick(12.0), Some(SceneRef::Object(0)), "removed marker is not pickable");
        assert_eq!(pick(100.0), None);
    }

    #[test]
    fn drag_is_one_undo_step_and_clamps_to_map() {
        let (mut ui, dir) = loaded("drag");
        let idx = ui.objects().iter().position(|o| o.object_index == 0).unwrap();
        let start = ui.markers()[idx].world;
        assert_eq!((start[0], start[2]), (1100.0, 2100.0));
        ui.begin_drag(SceneRef::Object(idx));
        assert!(!ui.can_undo(), "press alone records nothing");
        assert!(!ui.drag_to_world((1100.0, 2100.0)).unwrap(), "no movement, no step");
        assert!(ui.drag_to_world((1500.0, 2500.0)).unwrap());
        assert!(ui.drag_to_world((1600.0, 2600.0)).unwrap());
        assert!(ui.drag_to_world((-50.0, 99999.0)).unwrap());
        ui.end_drag();
        let o = &ui.objects()[idx];
        assert_eq!((o.local_x, o.local_z), (-1000.0, 16384.0 - 2000.0));
        assert!(ui.is_dirty() && ui.can_undo());
        assert!(ui.undo());
        assert!(!ui.can_undo() && !ui.is_dirty(), "the whole drag was one undo step");
        // Removed objects cannot be dragged.
        ui.remove_object(idx).unwrap();
        ui.begin_drag(SceneRef::Object(idx));
        assert!(!ui.is_capturing());
        cleanup(dir);
    }

    #[test]
    fn drag_moves_pending_addition_and_respects_settlement_shift() {
        let (mut ui, dir) = loaded("dragadd");
        ui.translate_settlement(NAME, [100.0, 0.0, 0.0]).unwrap();
        ui.duplicate_object(0, 1, [0.0, 0.0, 0.0], None).unwrap();
        ui.begin_drag(SceneRef::Addition(0));
        // Effective world is origin + shift + local; dragging to (1300, 2300) sets local 200,300.
        assert!(ui.drag_to_world((1300.0, 2300.0)).unwrap());
        ui.end_drag();
        let a = &ui.additions()[0];
        assert_eq!((a.local_x, a.local_z), (200.0, 300.0));
        cleanup(dir);
    }

    #[test]
    fn translate_settlement_validates_stages_and_commits() {
        let (mut ui, dir) = loaded("translate");
        let err = ui
            .translate_settlement(NAME, [20000.0, 0.0, 0.0])
            .unwrap_err();
        assert_eq!(
            err,
            "This offset would move part of the settlement outside the map."
        );
        assert!(ui.translate_settlement(NAME, [f32::NAN, 0.0, 0.0]).is_err());
        assert!(ui.translate_settlement("missing.sdl", [1.0, 0.0, 0.0]).is_err());
        assert!(!ui.is_dirty());
        ui.english = false;
        assert_eq!(
            ui.translate_settlement(NAME, [-5000.0, 0.0, 0.0]).unwrap_err(),
            "此平移量會讓部分聚落物件超出地圖範圍，已取消。"
        );
        ui.translate_settlement(NAME, [50.0, 5.0, -100.0]).unwrap();
        assert!(ui.is_dirty() && ui.can_undo());
        assert_eq!(ui.markers()[0].world[0], 1150.0);
        let files = ui.stage_transaction_files(&dir).unwrap();
        assert_eq!(files.len(), 1);
        commit(&dir, &files);
        ui.commit_baseline(&dir).unwrap();
        assert!(!ui.is_dirty());
        let objs = SdlSceneCatalog::load_directory(&dir).unwrap();
        let first = objs.iter().find(|o| o.object_index == 0).unwrap();
        assert_eq!((first.world_x, first.world_y, first.world_z), (1150.0, 65.0, 2000.0));
        assert_eq!((first.local_x, first.local_z), (100.0, 100.0), "local coordinates untouched");
        // Zero offset clears a pending shift; undo restores it.
        ui.translate_settlement(NAME, [10.0, 0.0, 0.0]).unwrap();
        ui.translate_settlement(NAME, [0.0, 0.0, 0.0]).unwrap();
        assert!(!ui.is_dirty());
        assert!(ui.undo() && ui.is_dirty());
        cleanup(dir);
    }

    #[test]
    fn add_object_by_type_stages_into_target_settlement() {
        let (mut ui, dir) = loaded("addtype");
        let templates = ui.catalog_templates();
        assert_eq!(templates.len(), 2);
        assert_eq!(ui.settlement_files(), vec![NAME.to_string()]);
        assert_eq!(ui.dominant_team(NAME), 0);
        let template = templates[0];
        assert!(ui.add_object_from_catalog(template, NAME, 99).is_err());
        assert!(ui.add_object_from_catalog(template, "x.sdl", 1).is_err());
        let n = ui.add_object_from_catalog(template, NAME, 2).unwrap();
        assert_eq!(ui.selected_ref(), Some(SceneRef::Addition(n)));
        let a = ui.additions()[n].clone();
        assert_eq!((a.local_x, a.local_y, a.local_z, a.team), (96.0, 0.0, 96.0, 2));
        let files = ui.stage_transaction_files(&dir).unwrap();
        commit(&dir, &files);
        ui.commit_baseline(&dir).unwrap();
        let objs = SdlSceneCatalog::load_directory(&dir).unwrap();
        assert_eq!(objs.len(), 3);
        let added = objs.iter().find(|o| o.team == 2).unwrap();
        assert_eq!((added.world_x, added.world_z), (1096.0, 2096.0));
        cleanup(dir);
    }

    #[test]
    fn restore_to_open_clears_every_edit_in_one_undoable_step() {
        let (mut ui, dir) = loaded("restore");
        assert!(!ui.restore_to_open(), "nothing to restore");
        ui.set_team(0, 7).unwrap();
        ui.set_position(1, [1.0, 2.0, 3.0]).unwrap();
        ui.remove_object(0).unwrap();
        ui.duplicate_object(1, 1, [5.0; 3], None).unwrap();
        ui.translate_settlement(NAME, [10.0, 0.0, 0.0]).unwrap();
        assert!(ui.is_dirty());
        assert!(ui.restore_to_open());
        assert!(!ui.is_dirty());
        assert!(ui.removals().is_empty() && ui.additions().is_empty());
        assert_eq!(ui.objects()[0].team, 0);
        assert!(ui.undo() && ui.is_dirty(), "restore can itself be undone");
        cleanup(dir);
    }

    #[test]
    fn delete_toggles_removal_and_scene_pointer_requires_move_mode() {
        let (mut ui, dir) = loaded("toggle");
        ui.select(Some(0));
        ui.toggle_delete_selected().unwrap();
        assert_eq!(ui.removals().len(), 1);
        ui.toggle_delete_selected().unwrap();
        assert!(ui.removals().is_empty() && !ui.is_dirty());
        ui.move_mode = false;
        assert!(!ui.scene_pointer(ScenePointer::Press { hit: Some(SceneRef::Object(0)) }));
        ui.move_mode = true;
        assert!(ui.scene_pointer(ScenePointer::Press { hit: Some(SceneRef::Object(0)) }));
        ui.scene_pointer(ScenePointer::Drag { world: (1200.0, 2200.0) });
        assert!(!ui.scene_pointer(ScenePointer::Release));
        assert!(ui.is_dirty());
        let files = ui.stage_transaction_files(&dir).unwrap();
        assert_eq!(files.len(), 1);
        cleanup(dir);
    }
}
