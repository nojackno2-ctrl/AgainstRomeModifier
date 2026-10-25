//! 目前編輯圖層的唯讀診斷，不依地形推導或改寫 collision。
//! 未指定單位通行能力時，保守視非零 collision 為阻擋；不代表原生逐單位尋路。
use crate::terrain::TerrainSession;
use against_rome_navigation::{
    analyze_connectivity, find_path, Coordinate, IsolatedRegion, PassabilityGrid,
};
use eframe::egui;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum BuildingMode { #[default] Off, Approximate, Apt }

#[derive(Default)]
pub struct NavigationUi {
    pub open: bool,
    building_mode: BuildingMode,
    apt_cells: Vec<[i64; 2]>,
    apt_fallbacks: Vec<[f32; 4]>,
    fallback_count: usize,
    game: Option<std::path::PathBuf>,
    resolver: Option<crate::building_footprint::FootprintResolver>,
    buildings: Vec<[f32; 4]>,
    placement_snapshot: Vec<u32>,
    pick: Option<bool>,
    holding: bool,
    seed: [i32; 2],
    target: [i32; 2],
    report: Option<NavigationReport>,
    error: Option<String>,
    snapshot: Option<(Vec<u8>, Vec<u8>, u32)>,
}
pub struct NavigationReport {
    pub passable: usize,
    pub seed_passable: bool,
    pub isolated: Vec<IsolatedRegion>,
    pub path_cells: Option<usize>,
    pub path: Option<Vec<Coordinate>>,
}
pub fn diagnose(
    session: &TerrainSession,
    water_surface: f32,
    seed: [i32; 2],
    target: [i32; 2],
) -> Result<NavigationReport, String> {
    diagnose_with_buildings(session, water_surface, seed, target, &[])
}
fn diagnose_with_buildings(
    session: &TerrainSession, water_surface: f32, seed: [i32; 2], target: [i32; 2],
    buildings: &[[f32; 4]],
) -> Result<NavigationReport, String> {
    diagnose_with_obstacles(session, water_surface, seed, target, buildings, &[])
}
fn diagnose_with_obstacles(
    session: &TerrainSession, water_surface: f32, seed: [i32; 2], target: [i32; 2],
    buildings: &[[f32; 4]], cells: &[[i64; 2]],
) -> Result<NavigationReport, String> {
    // 僅建立暫存遮罩，不改動地形 session 或原始 collision。
    let mut collision = session.collision().to_vec();
    let size = session.collision_size;
    let scale = 16384.0 / size as f32;
    for &[left, right, top, bottom] in buildings {
        if ![left, right, top, bottom].iter().all(|v| v.is_finite()) { continue; }
        for z in 0..size {
            for x in 0..size {
                if (x as f32) * scale <= right && ((x + 1) as f32) * scale > left
                    && (z as f32) * scale <= bottom && ((z + 1) as f32) * scale > top {
                    collision[z * size + x] = 255;
                }
            }
        }
    }
    for &[cx, cz] in cells {
        // 原生占用格固定 256×256；非標準 collision 尺寸採相交格保守映射。
        if !(0..256).contains(&cx) || !(0..256).contains(&cz) { continue; }
        let start_x = cx as usize * size / 256;
        let end_x = ((cx as usize + 1) * size).div_ceil(256).min(size);
        let start_z = cz as usize * size / 256;
        let end_z = ((cz as usize + 1) * size).div_ceil(256).min(size);
        for z in start_z..end_z { for x in start_x..end_x { collision[z * size + x] = 255; } }
    }
    if !water_surface.is_finite() {
        return Err("Water surface must be finite".into());
    }
    let grid = PassabilityGrid::new(
        session.collision_size,
        Some(&collision),
        Some((session.vertex_size, session.heights(), 1.0, water_surface)),
        16384.0,
    )
    .map_err(str::to_owned)?;
    if !grid.in_bounds(seed[0], seed[1]) || !grid.in_bounds(target[0], target[1]) {
        return Err("Seed and destination must lie inside the collision grid".into());
    }
    let passable = (0..grid.size)
        .flat_map(|z| (0..grid.size).map(move |x| (x, z)))
        .filter(|&(x, z)| grid.passable(x as i32, z as i32))
        .count();
    let path = find_path(&grid, (seed[0], seed[1]), (target[0], target[1]));
    Ok(NavigationReport {
        passable,
        seed_passable: grid.passable(seed[0], seed[1]),
        isolated: analyze_connectivity(&grid, &[(seed[0], seed[1])], &[(target[0], target[1])], 1),
        path_cells: path.as_ref().map(Vec::len),
        path: path.map(|p| p.into_iter().map(|(x, z)| grid.coordinate(x, z)).collect()),
    })
}
impl NavigationUi {
    pub(crate) fn update_placement(&mut self, objects: &[against_rome_placement::WorldObject]) {
        let snapshot = objects.iter().flat_map(|o| [
            o.slot as u32, o.uid, u32::from(o.type_id), u32::from(o.team),
            o.position[0].to_bits(), o.position[1].to_bits(), o.position[2].to_bits(),
            o.rotation_degrees.to_bits(), o.parent.map_or(u32::MAX, |p| p as u32),
        ]).collect::<Vec<_>>();
        if self.placement_snapshot != snapshot {
            self.invalidate();
            self.placement_snapshot = snapshot;
        }
    }
    pub(crate) fn update_buildings(&mut self, buildings: Vec<[f32; 4]>) {
        if self.buildings != buildings {
            self.invalidate();
            self.buildings = buildings;
        }
    }
    #[cfg(test)]
    fn set_approximate_buildings(&mut self, enabled: bool) {
        let mode = if enabled { BuildingMode::Approximate } else { BuildingMode::Off };
        if mode != self.building_mode { self.building_mode = mode; self.invalidate(); }
    }
    fn analyze(&self, session: &TerrainSession, water: f32, seed: [i32; 2], target: [i32; 2]) -> Result<NavigationReport, String> {
        match self.building_mode {
            BuildingMode::Off => diagnose(session, water, seed, target),
            BuildingMode::Approximate => diagnose_with_buildings(session, water, seed, target, &self.buildings),
            BuildingMode::Apt => diagnose_with_obstacles(session, water, seed, target, &self.apt_fallbacks, &self.apt_cells),
        }
    }
    pub(crate) fn update_apt(
        &mut self, game: Option<std::path::PathBuf>, objects: &[against_rome_placement::WorldObject],
        names: &std::collections::HashMap<i32, String>,
    ) {
        if self.game != game {
            self.resolver = game.as_deref().and_then(|g| crate::building_footprint::FootprintResolver::load(g).ok());
            self.game = game;
            self.invalidate();
        }
        let roots = objects.iter().filter(|o| o.parent.is_none()).filter(|o| {
            names.get(&i32::from(o.type_id)).is_some_and(|name|
                against_rome_maps::SdlObjectCatalog::categorize(name) == against_rome_maps::SdlObjectCategory::Building)
        });
        let mut cells = Vec::new();
        let mut fallbacks = Vec::new();
        for (object, bounds) in roots.zip(&self.buildings) {
            if !object.position[0].is_finite() || !object.position[2].is_finite() {
                fallbacks.push(*bounds); continue;
            }
            let stencil = self.resolver.as_mut().and_then(|r| r.resolve(i32::from(object.type_id)).ok());
            if let Some(stencil) = stencil {
                let anchor = [(object.position[0] as f64 / 64.0).floor() as i64,
                    (object.position[2] as f64 / 64.0).floor() as i64];
                cells.extend(stencil.cells.iter().map(|c| [anchor[0].saturating_add(c.relative[0]), anchor[1].saturating_add(c.relative[1])]));
            } else { fallbacks.push(*bounds); }
        }
        if self.apt_cells != cells || self.apt_fallbacks != fallbacks {
            self.invalidate();
            self.apt_cells = cells;
            self.apt_fallbacks = fallbacks;
        }
        self.fallback_count = self.apt_fallbacks.len();
    }    pub(crate) fn owns_canvas(&self) -> bool {
        self.pick.is_some() || self.holding
    }
    pub(crate) fn refresh(&mut self, session: Option<&TerrainSession>, water_surface: f32) {
        if let Some((heights, collision, water)) = &self.snapshot {
            if session.is_none_or(|s| s.heights() != heights || s.collision() != collision)
                || *water != water_surface.to_bits()
            {
                self.invalidate();
            }
        }
    }
    /// 取消及完成取點後，直到按鍵放開都獨占畫布，避免塗畫地形。
    pub(crate) fn cancel_pick(&mut self, ctx: &egui::Context) {
        if self.pick.is_some()
            && (!self.open
                || ctx.input(|i| i.key_pressed(egui::Key::Escape) || i.pointer.secondary_pressed()))
        {
            self.pick = None;
            self.holding = true;
        }
    }
    pub(crate) fn canvas_input(
        &mut self,
        ctx: &egui::Context,
        response: &egui::Response,
        rect: egui::Rect,
        size: usize,
    ) -> bool {
        let owns = self.owns_canvas();
        if self.holding {
            self.holding = ctx.input(|i| i.pointer.primary_down() || i.pointer.secondary_down());
        } else if self.pick.is_some()
            && response.hovered()
            && ctx.input(|i| i.pointer.primary_pressed())
        {
            if let Some(pos) = ctx
                .input(|i| i.pointer.interact_pos())
                .filter(|p| rect.contains(*p))
            {
                let (x, z) = crate::scenario_canvas::world_from_screen(rect, pos);
                let c = Coordinate::from_world(x, z, size, 16384.0);
                if self.pick.take() == Some(true) {
                    self.target = [c.x, c.z];
                } else {
                    self.seed = [c.x, c.z];
                }
                self.invalidate();
                self.holding = true;
            }
        }
        owns
    }
    pub(crate) fn paint_canvas(&self, painter: &egui::Painter, rect: egui::Rect, size: usize) {
        let Some(report) = &self.report else {
            return;
        };
        let screen =
            |c: &Coordinate| crate::scenario_canvas::screen_from_world(rect, c.world_x, c.world_z);
        if let Some(path) = &report.path {
            let points: Vec<_> = path.iter().map(screen).collect();
            if points.len() > 1 {
                painter.add(egui::Shape::line(
                    points,
                    egui::Stroke::new(3.0_f32, egui::Color32::YELLOW),
                ));
            }
        }
        let start = Coordinate::from_tile(self.seed[0], self.seed[1], size, 16384.0);
        let end = Coordinate::from_tile(self.target[0], self.target[1], size, 16384.0);
        let color = if report.path.is_none() {
            egui::Color32::RED
        } else {
            egui::Color32::LIGHT_BLUE
        };
        if report.path.is_none() {
            let cell = egui::Rect::from_center_size(
                screen(&end),
                egui::vec2(rect.width() / size as f32, rect.height() / size as f32),
            );
            painter.rect_filled(
                cell,
                0.0,
                egui::Color32::from_rgba_unmultiplied(255, 0, 0, 100),
            );
            painter.rect_stroke(cell, 0.0, egui::Stroke::new(2.0_f32, color));
        }
        for (c, color, label) in [
            (&start, egui::Color32::GREEN, "起點 / Start"),
            (
                &end,
                color,
                if report.path.is_none() {
                    "終點無法抵達 / End unreachable"
                } else {
                    "終點 / End"
                },
            ),
        ] {
            painter.circle_filled(screen(c), 5.0, color);
            painter.text(
                screen(c) + egui::vec2(8.0, -8.0),
                egui::Align2::LEFT_BOTTOM,
                label,
                egui::FontId::proportional(13.0),
                color,
            );
        }
    }
    pub fn invalidate(&mut self) {
        self.report = None;
        self.error = None;
        self.snapshot = None;
    }
    /// 交換診斷方向並取消取點，不修改地圖或復原紀錄。
    fn swap_endpoints(&mut self, allowed: bool) {
        if !allowed { return; }
        std::mem::swap(&mut self.seed, &mut self.target);
        self.clear_diagnostics(true);
    }

    /// 清除路徑覆蓋與錯誤，保留端點與障礙設定。
    fn clear_diagnostics(&mut self, allowed: bool) {
        if !allowed { return; }
        self.invalidate();
        if self.pick.take().is_some() { self.holding = true; }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        session: Option<&TerrainSession>,
        water_surface: f32,
        interaction_allowed: bool,
    ) {
        self.refresh(session, water_surface);
        if !self.open {
            return;
        }
        let mut mode = self.building_mode;
        let mut open = self.open;
        egui::Window::new("通行診斷 / Navigation").open(&mut open).show(ctx,|ui| {
            ui.label("編輯器連通模型估計，非遊戲精確尋路器。 / Editor connectivity estimate; not the exact game pathfinder.");
            let Some(session)=session else {ui.label("請先載入地圖。");return;};
            ui.label(format!("阻擋網格 {}×{}；水位高度取樣值 {:.2}",session.collision_size,session.collision_size,water_surface));
            ui.horizontal(|ui| {
                ui.label("建築障礙估計 / Building obstacle estimate");
                ui.selectable_value(&mut mode, BuildingMode::Off, "關閉 / Off");
                ui.selectable_value(&mut mode, BuildingMode::Approximate, "近似 384/64 / Approximate");
                ui.selectable_value(&mut mode, BuildingMode::Apt, "APT footprint（遊戲資料 / game data）");
            });
            if mode != self.building_mode { self.building_mode = mode; self.invalidate(); }
            if mode == BuildingMode::Apt {
                ui.label(crate::building_footprint::GROUP_LABEL);
                ui.label(format!("估計：全部列出格皆阻擋；{} 個建築無法解析，改用近似邊距。 / Estimate: all listed cells blocked; {} unresolved buildings use approximate margins.", self.fallback_count, self.fallback_count));
            }            let max=(session.collision_size-1) as i32;
            let mut changed=false;
            for (label,point) in [("起點 / Seed",&mut self.seed),("終點 / Destination",&mut self.target)] {
                ui.horizontal(|ui|{ui.label(label);for value in point {changed |= ui.add(egui::DragValue::new(value).clamp_range(0..=max)).changed();}});
            }
            ui.horizontal(|ui| {
                if ui.button("地圖選起點 / Pick start on map").clicked() { self.pick=Some(false); }
                if ui.button("地圖選終點 / Pick end on map").clicked() { self.pick=Some(true); }
            });
            ui.horizontal(|ui| {
                if ui.add_enabled(interaction_allowed, egui::Button::new("交換起終點 / Swap endpoints")).clicked() {
                    self.swap_endpoints(interaction_allowed);
                }
                if ui.add_enabled(interaction_allowed, egui::Button::new("清除診斷覆蓋 / Clear diagnostics")).clicked() {
                    self.clear_diagnostics(interaction_allowed);
                }
            });
            if self.pick.is_some() { ui.label("左鍵點選 2D 地圖；Esc／右鍵取消。 / Click 2D map; Esc/right-click cancels."); }
            if changed {self.report=None;}
            if ui.button("重新檢查 / Analyze current layers").clicked() {
                match self.analyze(session,water_surface,self.seed,self.target) {
                    Ok(report)=>{self.report=Some(report);self.error=None;self.snapshot=Some((session.heights().to_vec(),session.collision().to_vec(),water_surface.to_bits()));},
                    Err(e)=>{self.report=None;self.error=Some(e);},
                }
            }
            if let Some(error)=&self.error {ui.colored_label(egui::Color32::RED,error);}
            if let Some(report)=&self.report {
                if mode != BuildingMode::Off { ui.label("包含建築障礙的估計 / Estimate including building obstacles"); } ui.label(format!("可通行：{}；與起點隔離的區域：{}",report.passable,report.isolated.len()));
                if !report.seed_passable {ui.label("起點在阻擋格或水下，請選擇其他起點。");}
                ui.label(match report.path_cells {Some(n)=>format!("路徑：{n} 個網格"),None=>"終點無法抵達 / End unreachable".into()});
                egui::ScrollArea::vertical().max_height(240.0).show(ui,|ui| {
                    for region in &report.isolated {
                        ui.label(format!("{} 個網格；中心（{}，{}）；橋接建議：{:?}",region.tile_count,region.centroid.x,region.centroid.z,region.bridge.map(|p|(p.x,p.z))));
                    }
                });
            }
        });
        self.open = open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::CollisionBrushOp;
    #[test]
    fn swap_and_clear_preserve_layers_history_and_obstacle_settings() {
        let s = TerrainSession::create_blank(6, 32);
        let token = s.history_token();
        let mut nav = NavigationUi::default();
        nav.seed = [1, 2]; nav.target = [4, 3];
        nav.building_mode = BuildingMode::Approximate;
        nav.report = Some(diagnose(&s, 0.0, nav.seed, nav.target).unwrap());
        nav.pick = Some(true);
        nav.swap_endpoints(false);
        assert_eq!(nav.seed, [1, 2]);
        assert!(nav.report.is_some());
        nav.swap_endpoints(true);
        assert_eq!((nav.seed, nav.target), ([4, 3], [1, 2]));
        assert!(nav.report.is_none() && nav.pick.is_none() && nav.holding);
        nav.report = Some(diagnose(&s, 0.0, nav.seed, nav.target).unwrap());
        nav.error = Some("舊診斷".into());
        nav.clear_diagnostics(false);
        assert!(nav.report.is_some());
        nav.clear_diagnostics(true);
        assert!(nav.report.is_none() && nav.error.is_none() && nav.snapshot.is_none());
        assert_eq!((nav.seed, nav.target), ([4, 3], [1, 2]));
        assert_eq!(nav.building_mode, BuildingMode::Approximate);
        assert_eq!(s.history_token(), token);
        assert!(!s.is_dirty() && !s.can_undo());
    }
    #[test]
    fn live_diagnostics_follow_edits_and_undo_without_modifying_layers() {
        let mut s = TerrainSession::create_blank(6, 32);
        let initial = s.collision().to_vec();
        let r = diagnose(&s, 0.0, [0, 0], [4, 4]).unwrap();
        assert_eq!(r.passable, 25);
        assert!(r.path_cells.is_some());
        for z in 0..5 {
            s.paint_collision(2.5, z as f32 + 0.5, 0.6, CollisionBrushOp::Block)
                .unwrap();
        }
        s.commit_stroke();
        let blocked = s.collision().to_vec();
        let r = diagnose(&s, 0.0, [0, 0], [4, 4]).unwrap();
        assert_eq!(r.passable, 20);
        assert!(r.path_cells.is_none());
        assert_eq!(r.isolated.len(), 1);
        assert_eq!(s.collision(), blocked);
        s.undo();
        assert_eq!(s.collision(), initial);
        assert!(diagnose(&s, 0.0, [0, 0], [4, 4])
            .unwrap()
            .path_cells
            .is_some());
        assert!(diagnose(&s, f32::NAN, [0, 0], [4, 4]).is_err());
        assert!(diagnose(&s, 0.0, [-1, 0], [4, 4]).is_err());
    }
}

#[cfg(test)]
mod overlay_tests {
    use super::*;
    use crate::terrain::CollisionBrushOp;
    use egui::{pos2, vec2, Rect};

    #[test]
    fn world_grid_screen_round_trip_uses_navigation_scale() {
        let rect = Rect::from_min_size(pos2(-37.0, 81.0), vec2(743.0, 591.0));
        for size in [5, 192, 256] {
            let grid = PassabilityGrid::new(size, None, None, 16384.0).unwrap();
            for (x, z) in [
                (0, 0),
                (size as i32 / 2, size as i32 / 3),
                (size as i32 - 1, size as i32 - 1),
            ] {
                let c = grid.coordinate(x, z);
                let p = crate::scenario_canvas::screen_from_world(rect, c.world_x, c.world_z);
                let (wx, wz) = crate::scenario_canvas::world_from_screen(rect, p);
                let picked = Coordinate::from_world(wx, wz, size, 16384.0);
                assert_eq!((picked.x, picked.z), grid.world_to_tile(wx, wz));
                assert_eq!((picked.x, picked.z), (x, z));
            }
        }
    }

    fn barrier(gap: bool) -> TerrainSession {
        let mut s = TerrainSession::create_blank(6, 32);
        for z in 0..5 {
            if gap && z == 3 {
                continue;
            }
            s.paint_collision(2.5, z as f32 + 0.5, 0.4, CollisionBrushOp::Block)
                .unwrap();
        }
        s.commit_stroke();
        s
    }

    fn shapes(nav: &NavigationUi, size: usize) -> Vec<egui::epaint::ClippedShape> {
        let ctx = egui::Context::default();
        ctx.run(egui::RawInput::default(), |ctx| {
            let painter = ctx.layer_painter(egui::LayerId::background());
            nav.paint_canvas(
                &painter,
                Rect::from_min_size(pos2(0.0, 0.0), vec2(16384.0, 16384.0)),
                size,
            );
        })
        .shapes
    }

    #[test]
    fn drawn_polyline_avoids_barrier_cells() {
        let s = barrier(true);
        let report = diagnose(&s, 0.0, [0, 0], [4, 4]).unwrap();
        let path = report.path.as_ref().unwrap();
        assert_eq!(report.path_cells, Some(path.len()));
        let expected: Vec<_> = path.iter().map(|c| pos2(c.world_x, c.world_z)).collect();
        let nav = NavigationUi {
            target: [4, 4],
            report: Some(report),
            ..Default::default()
        };
        let line = shapes(&nav, 5)
            .into_iter()
            .find_map(|s| match s.shape {
                egui::Shape::Path(p) => Some(p.points),
                _ => None,
            })
            .unwrap();
        assert_eq!(line, expected);
        for point in line {
            let c = Coordinate::from_world(point.x, point.y, 5, 16384.0);
            assert_eq!(s.collision()[c.z as usize * 5 + c.x as usize], 0);
        }
    }

    #[test]
    fn unreachable_draws_end_cell_without_path() {
        let s = barrier(false);
        let report = diagnose(&s, 0.0, [0, 0], [4, 4]).unwrap();
        assert!(report.path.is_none());
        let nav = NavigationUi {
            target: [4, 4],
            report: Some(report),
            ..Default::default()
        };
        let shapes = shapes(&nav, 5);
        assert!(!shapes
            .iter()
            .any(|s| matches!(s.shape, egui::Shape::Path(_))));
        assert!(shapes.iter().any(
            |s| matches!(&s.shape,egui::Shape::Rect(r) if r.rect.contains(pos2(14745.6,14745.6)))
        ));
    }

    #[test]
    fn collision_edit_invalidates_overlay_even_with_window_closed() {
        let mut s = TerrainSession::create_blank(6, 32);
        let mut nav = NavigationUi {
            report: Some(diagnose(&s, 0.0, [0, 0], [4, 4]).unwrap()),
            snapshot: Some((
                s.heights().to_vec(),
                s.collision().to_vec(),
                0.0_f32.to_bits(),
            )),
            ..Default::default()
        };
        nav.refresh(Some(&s), 0.0);
        assert!(!shapes(&nav, 5).is_empty());
        s.paint_collision(1.5, 1.5, 0.4, CollisionBrushOp::Block)
            .unwrap();
        nav.refresh(Some(&s), 0.0);
        assert!(nav.report.is_none());
        assert!(shapes(&nav, 5).is_empty());
    }

    #[test]
    fn escape_and_right_click_cancel_without_changing_endpoints_or_report() {
        for event in [
            egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: pos2(30.0, 30.0),
                button: egui::PointerButton::Secondary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ] {
            let s = TerrainSession::create_blank(6, 32);
            let mut nav = NavigationUi {
                open: true,
                pick: Some(true),
                seed: [1, 1],
                target: [4, 4],
                report: Some(diagnose(&s, 0.0, [1, 1], [4, 4]).unwrap()),
                ..Default::default()
            };
            let ctx = egui::Context::default();
            let _ = ctx.run(
                egui::RawInput {
                    events: vec![event],
                    ..Default::default()
                },
                |ctx| {
                    nav.cancel_pick(ctx);
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let (rect, response) = ui
                            .allocate_exact_size(vec2(100.0, 100.0), egui::Sense::click_and_drag());
                        assert!(nav.canvas_input(ctx, &response, rect, 5));
                    });
                },
            );
            assert!(nav.pick.is_none());
            assert_eq!(nav.seed, [1, 1]);
            assert_eq!(nav.target, [4, 4]);
            assert!(nav.report.is_some());
            assert_eq!(s.collision(), vec![0; 25]);
        }
    }
}
#[cfg(test)]
mod pick_tests {
    use super::*;
    #[test]
    fn pick_uses_collision_coordinates_and_holds_canvas_until_release() {
        for end in [false, true] {
            let ctx = egui::Context::default();
            let mut nav = NavigationUi {
                open: true,
                pick: Some(end),
                seed: [0, 0],
                target: [1, 1],
                ..Default::default()
            };
            let mut rect = egui::Rect::NOTHING;
            let input = |events| egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400.0, 400.0),
                )),
                events,
                ..Default::default()
            };
            let _ = ctx.run(input(vec![]), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let (r, response) = ui.allocate_exact_size(
                        egui::vec2(200.0, 200.0),
                        egui::Sense::click_and_drag(),
                    );
                    rect = r;
                    assert!(nav.canvas_input(ctx, &response, r, 5));
                });
            });
            let pos = rect.min + egui::vec2(140.0, 100.0);
            for pressed in [true, false] {
                let _ = ctx.run(
                    input(vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ]),
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            let (r, response) = ui.allocate_exact_size(
                                egui::vec2(200.0, 200.0),
                                egui::Sense::click_and_drag(),
                            );
                            assert!(nav.canvas_input(ctx, &response, r, 5));
                        });
                    },
                );
                assert_eq!(if end { nav.target } else { nav.seed }, [3, 2]);
                assert!(nav.pick.is_none());
            }
            assert!(!nav.owns_canvas());
            assert_eq!(
                if end { nav.seed } else { nav.target },
                if end { [0, 0] } else { [1, 1] }
            );
        }
    }
}

#[cfg(test)]
mod building_tests {
    use super::*;
    use crate::terrain::CollisionBrushOp;

    #[test]
    fn building_closes_only_corridor_without_changing_collision() {
        let mut s = TerrainSession::create_blank(6, 32);
        for z in 0..5 {
            if z != 2 {
                s.paint_collision(2.5, z as f32 + 0.5, 0.4, CollisionBrushOp::Block).unwrap();
            }
        }
        s.commit_stroke();
        let collision = s.collision().to_vec();
        let heights = s.heights().to_vec();
        let margin = against_rome_maps::placement_edit::spawn_data::approximate_family_margin(true);
        let center = 16384.0 / 2.0;
        let buildings = vec![[center - margin, center + margin, center - margin, center + margin]];
        assert!(diagnose(&s, 0.0, [0, 2], [4, 2]).unwrap().path.is_some());
        assert!(diagnose_with_buildings(&s, 0.0, [0, 2], [4, 2], &buildings).unwrap().path.is_none());
        assert_eq!(s.collision(), collision);
        assert_eq!(s.heights(), heights);
        let mut nav = NavigationUi::default();
        assert_eq!(nav.building_mode, BuildingMode::Off);
        nav.report = Some(diagnose(&s, 0.0, [0, 2], [4, 2]).unwrap());
        nav.set_approximate_buildings(true);
        assert!(nav.report.is_none());
        nav.update_buildings(buildings.clone());
        nav.report = Some(diagnose_with_buildings(&s, 0.0, [0, 2], [4, 2], &buildings).unwrap());
        nav.update_buildings(buildings);
        assert!(nav.report.is_some());
        nav.update_buildings(vec![]);
        assert!(nav.report.is_none());
        nav.report = Some(diagnose(&s, 0.0, [0, 2], [4, 2]).unwrap());
        nav.set_approximate_buildings(false);
        assert!(nav.report.is_none());
    }
}

#[cfg(test)]
mod apt_tests {
    use super::*;
    use against_rome_data::{apt::AptPoint, apt_footprint::{AptFootprint, FootprintCell}};
    fn object(type_id: u16) -> against_rome_placement::WorldObject {
        against_rome_placement::WorldObject { slot: 0, type_id, team: 0, uid: 1,
            position: [8192.0, 0.0, 8192.0], rotation_degrees: 0.0, parent: None, vitals: None }
    }
    fn session() -> TerrainSession {
        let mut s = TerrainSession::create_blank(6, 32);
        for z in [0, 1, 3, 4] {
            s.paint_collision(2.5, z as f32 + 0.5, 0.4, crate::terrain::CollisionBrushOp::Block).unwrap();
        }
        s.commit_stroke(); s
    }
    #[test]
    fn synthetic_apt_mode_closes_corridor_without_collision_changes() {
        let mut b = vec![0u8; 428];
        b[..4].copy_from_slice(b"APAT");
        for (at, value) in [(4, 3u32), (12, 112), (36, 1), (384, 1), (396, 1), (416, 1)] {
            b[at..at+4].copy_from_slice(&value.to_le_bytes());
        }
        let stencil = AptFootprint::parse(&b).unwrap();
        assert_eq!(stencil.groups[0], vec![FootprintCell { value: 1, point: AptPoint::default(), relative: [0,0] }]);
        let mut nav = NavigationUi { building_mode: BuildingMode::Apt,
            resolver: Some(crate::building_footprint::FootprintResolver::synthetic(34, stencil.groups[0].clone())), ..Default::default() };
        nav.update_buildings(vec![[8000.0, 8400.0, 8000.0, 8400.0]]);
        nav.update_apt(None, &[object(34)], &std::collections::HashMap::from([(34, "BauGerBau00".into())]));
        assert_eq!(nav.apt_cells, vec![[128,128]]);
        assert_eq!(nav.fallback_count, 0);
        let s = session(); let collision = s.collision().to_vec();
        assert!(diagnose(&s, 0.0, [0,2], [4,2]).unwrap().path.is_some());
        assert!(nav.analyze(&s, 0.0, [0,2], [4,2]).unwrap().path.is_none());
        nav.building_mode = BuildingMode::Off;
        assert!(nav.analyze(&s, 0.0, [0,2], [4,2]).unwrap().path.is_some());
        assert_eq!(s.collision(), collision);
    }
    #[test]
    fn unresolved_type_falls_back_and_counts_placed_buildings() {
        let mut nav = NavigationUi { building_mode: BuildingMode::Apt, ..Default::default() };
        let bounds = [8192.0-64.0,8192.0+64.0,8192.0-64.0,8192.0+64.0];
        nav.update_buildings(vec![bounds]);
        nav.update_apt(None, &[object(999)], &std::collections::HashMap::from([(999, "BauUnknown".into())]));
        assert_eq!(nav.fallback_count, 1);
        assert_eq!(nav.apt_fallbacks, vec![bounds]);
        assert!(nav.apt_cells.is_empty());
        let s = session(); let bytes = s.collision().to_vec();
        assert!(nav.analyze(&s,0.0,[0,2],[4,2]).unwrap().path.is_none());
        assert_eq!(s.collision(),bytes);
    }
}
