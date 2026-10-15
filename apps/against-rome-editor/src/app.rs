//! Against Rome 原生 Rust 桌面地圖編輯器主介面 (egui/eframe)

use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2};
use std::path::PathBuf;

use crate::storage::{load_map, safe_save_map};
use crate::terrain::{BrushPath, CollisionBrushOp, HeightBrushOp, TerrainSession};
use crate::{map_settings::MapSettings, navigation::NavigationUi, toolbox::ToolboxUi};
use crate::{placement_ui::PlacementUi,scenario_ui::ScenarioUi};
use crate::textures_ui::TexturesUi;
use crate::{console::ConsoleUi, event_graph::EventGraphUi};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    Heightmap,
    Collision,
    Blended,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveTool {
    HeightRaise,
    HeightLower,
    HeightFlatten,
    HeightSmooth,
    HeightRoughen,
    Water,
    CollisionBlock,
    CollisionClear,
}

enum PendingAction {
    Load(PathBuf),
    Blank(usize),
    Close,
}

pub struct EditorApp {
    pub map_path_input: String,
    pub session: Option<TerrainSession>,
    pub status_message: String,
    pub status_is_error: bool,

    // 筆刷參數
    pub active_tool: ActiveTool,
    pub brush_radius: f32,
    pub brush_strength: u8,
    pub flatten_target: u8,
    pub roughness_seed: i32,
    pub sample_flatten_target: bool,
    pub water_surface: f32,
    pub water_bed: u8,

    // 視圖參數
    pub view_mode: ViewMode,
    pub zoom: f32,
    pub pan: Vec2,

    // 紋理快取
    texture: Option<TextureHandle>,
    texture_dirty: bool,
    is_painting: bool,
    stroke_path: BrushPath,
    pending_action: Option<PendingAction>,
    close_allowed: bool,
    toolbox: ToolboxUi,
    navigation: NavigationUi,
    height_step: f32,
    placement: PlacementUi,
    scenario: ScenarioUi,
    textures: TexturesUi,
    event_graph: EventGraphUi,
    console: ConsoleUi,
}

impl EditorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Load the user's installed CJK font, without redistributing font files.
        #[cfg(windows)]
        if let Some(windows) = std::env::var_os("WINDIR") {
            for name in ["msjh.ttc", "msyh.ttc"] {
                if let Ok(bytes) = std::fs::read(PathBuf::from(&windows).join("Fonts").join(name)) {
                    let mut fonts=egui::FontDefinitions::default();
                    fonts.font_data.insert("system-cjk".into(),egui::FontData::from_owned(bytes));
                    for family in [egui::FontFamily::Proportional,egui::FontFamily::Monospace] {
                        fonts.families.entry(family).or_default().push("system-cjk".into());
                    }
                    cc.egui_ctx.set_fonts(fonts);
                    break;
                }
            }
        }
        #[cfg(not(windows))]
        let _=cc;
        Self::initial_state()
    }

    fn initial_state() -> Self {
        // 預設建立一張空白 257x257 地圖以便立即可用
        let session = TerrainSession::create_blank(257, 32);

        Self {
            map_path_input: String::new(),
            session: Some(session),
            status_message: "Ready. Blank 257x257 map initialized.".to_string(),
            status_is_error: false,
            active_tool: ActiveTool::HeightRaise,
            brush_radius: 12.0,
            brush_strength: 8,
            flatten_target: 32,
            roughness_seed: 42,
            sample_flatten_target: true,
            water_surface: 48.0,
            water_bed: 24,
            view_mode: ViewMode::Blended,
            zoom: 1.0,
            pan: Vec2::ZERO,
            texture: None,
            texture_dirty: true,
            is_painting: false,
            stroke_path: BrushPath::default(),
            pending_action: None,
            close_allowed: false,
            toolbox: ToolboxUi::default(),
            navigation: NavigationUi::default(),
            height_step: 4.0,
            placement: PlacementUi::default(),
            scenario: ScenarioUi::default(),
            textures: TexturesUi::default(),
            event_graph: EventGraphUi::default(),
            console: ConsoleUi::default(),
        }
    }

    pub fn set_loaded_map(&mut self,path:PathBuf,session:TerrainSession) {
        self.session=Some(session);
        self.map_path_input=path.to_string_lossy().into_owned();
        self.status_is_error=false;
        self.status_message=format!("Loaded {}",path.display());
        let settings=match MapSettings::load(&path) {
            Ok(s)=>s,
            Err(e)=>{self.status_is_error=true;self.status_message=format!("Map loaded; water settings disabled: {e}");MapSettings::default()}
        };
        self.height_step=settings.height_step;
        self.water_surface=settings.water_surface();
        self.water_bed=settings.suggested_bed();
        self.navigation.invalidate();self.texture_dirty=true;
    }

    fn perform_action(&mut self, action: PendingAction, ctx: &egui::Context) {
        match action {
            PendingAction::Load(path) => match load_map(&path) {
                Ok(loaded) => {
                    self.status_message = format!(
                        "Loaded {}: {} height vertices, {} collision pixels per side",
                        path.display(),
                        loaded.vertex_size,
                        loaded.collision_size
                    );
                    self.set_loaded_map(path,loaded);
                }
                Err(error) => {
                    self.status_message = format!("Load failed: {error}");
                    self.status_is_error = true;
                    return;
                }
            },
            PendingAction::Blank(size) => {
                self.session = Some(TerrainSession::create_blank(size, 32));
                self.status_message = format!("Created blank {size}x{size} terrain");
                self.status_is_error = false;
                self.map_path_input.clear();
                self.height_step=4.0;self.water_surface=0.0;self.water_bed=0;
                self.navigation.invalidate();
            }
            PendingAction::Close => {
                self.close_allowed = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        }
        self.is_painting = false;
        self.stroke_path.clear();
        self.texture_dirty = true;
    }

    fn handle_pending_action(&mut self, ctx: &egui::Context) {
        if self.pending_action.is_none() {
            return;
        }
        let dirty=if matches!(self.pending_action,Some(PendingAction::Close)) {
            self.has_unsaved_changes()
        } else {self.session.as_ref().is_some_and(|s|s.is_dirty())};
        if !dirty {
            let action = self.pending_action.take().unwrap();
            self.perform_action(action, ctx);
            return;
        }
        let mut discard = false;
        let mut cancel = false;
        egui::Window::new("Unsaved changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label("Discard unsaved changes? Closing also discards pending DATA and scenario edits.");
                ui.horizontal(|ui| {
                    discard = ui.button("Discard unsaved changes").clicked();
                    cancel = ui.button("Keep editing").clicked();
                });
            });
        if discard {
            let action = self.pending_action.take().unwrap();
            self.perform_action(action, ctx);
        } else if cancel {
            self.pending_action = None;
        }
    }

    fn has_unsaved_changes(&self)->bool {
        self.session.as_ref().is_some_and(|s|s.is_dirty()) || self.placement.is_dirty() || self.scenario.dirty() || self.textures.is_dirty() || self.event_graph.is_dirty()
    }

    fn update_texture(&mut self, ctx: &egui::Context) {
        let Some(session) = &self.session else { return };
        let w = session.vertex_size;
        let h = session.vertex_size;
        let mut pixels = Vec::with_capacity(w * h);

        let col_scale = session.collision_size as f32 / (session.vertex_size.max(2) - 1) as f32;

        for y in 0..h {
            for x in 0..w {
                let height = session.heights()[y * w + x];
                let col_x =
                    ((x as f32 * col_scale).round() as usize).min(session.collision_size - 1);
                let col_y =
                    ((y as f32 * col_scale).round() as usize).min(session.collision_size - 1);
                let is_blocked = session.collision()[col_y * session.collision_size + col_x] > 0;

                let color = match self.view_mode {
                    ViewMode::Heightmap => {
                        // 地形高程調色盤：綠色平原 -> 土黃丘陵 -> 雪白峰頂
                        if height < 64 {
                            Color32::from_rgb(20 + height, 80 + height * 2, 20 + height)
                        } else if height < 160 {
                            Color32::from_rgb(100 + (height - 64), 140 + (height - 64) / 2, 50)
                        } else {
                            let light = 160 + ((height as u32 - 160) * 95 / 95) as u8;
                            Color32::from_rgb(light, light, light)
                        }
                    }
                    ViewMode::Collision => {
                        if is_blocked {
                            Color32::from_rgb(220, 40, 40) // 紅色阻擋
                        } else {
                            Color32::from_rgb(30, 30, 30) // 暗黑可通行
                        }
                    }
                    ViewMode::Blended => {
                        let base_g = (height as f32 * 0.7) as u8 + 40;
                        if is_blocked {
                            Color32::from_rgb(200, base_g / 2, base_g / 2) // 阻擋染紅
                        } else {
                            Color32::from_rgb(base_g / 2, base_g, base_g / 3)
                        }
                    }
                };
                pixels.push(color);
            }
        }

        let image = ColorImage {
            size: [w, h],
            pixels,
        };

        if let Some(tex) = &mut self.texture {
            tex.set(image, egui::TextureOptions::NEAREST);
        } else {
            self.texture =
                Some(ctx.load_texture("terrain-view", image, egui::TextureOptions::NEAREST));
        }
        self.texture_dirty = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::SampleChange;

    #[test]
    fn dirty_session_blocks_new_and_open_until_explicit_discard() {
        let mut app = EditorApp::initial_state();
        app.session
            .as_mut()
            .unwrap()
            .apply_height_changes(&[SampleChange {
                index: 0,
                before: 32,
                after: 99,
            }]);
        let ctx = egui::Context::default();
        for action in [
            PendingAction::Blank(193),
            PendingAction::Load(PathBuf::from("not-a-map")),
            PendingAction::Close,
        ] {
            app.pending_action = Some(action);
            let _ = ctx.run(Default::default(), |ctx| app.handle_pending_action(ctx));
            assert!(app.pending_action.is_some());
            assert_eq!(app.session.as_ref().unwrap().heights()[0], 99);
            assert!(!app.close_allowed);
        }
        // The exact action executed by the dialog's explicit discard button.
        app.pending_action = Some(PendingAction::Blank(193));
        let action = app.pending_action.take().unwrap();
        app.perform_action(action, &ctx);
        assert_eq!(app.session.as_ref().unwrap().vertex_size, 193);
    }
}

impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested())
            && !self.close_allowed
            && self.has_unsaved_changes()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending_action = Some(PendingAction::Close);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.is_painting {
            if let Some(session) = &mut self.session {
                session.cancel_stroke();
            }
            self.is_painting = false;
            self.stroke_path.clear();
            self.texture_dirty = true;
        }
        // 快捷鍵處理
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Z)) {
            if let Some(session) = &mut self.session {
                if session.undo() {
                    self.is_painting = false;
                    self.stroke_path.clear();
                    self.texture_dirty = true;
                    self.status_message = "Undo performed.".to_string();
                    self.status_is_error = false;
                }
            }
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Y)) {
            if let Some(session) = &mut self.session {
                if session.redo() {
                    self.is_painting = false;
                    self.stroke_path.clear();
                    self.texture_dirty = true;
                    self.status_message = "Redo performed.".to_string();
                    self.status_is_error = false;
                }
            }
        }

        // 頂部導覽列
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("Against Rome Map Editor (Rust)");
                ui.separator();
                if ui.button("Modifier / Saves").clicked() {self.toolbox.open=true;}
                if ui.button("Navigation").clicked() {self.navigation.open=true;}
                if ui.button("Objects").clicked() {self.placement.open=true;}
                if ui.button("Scenario / Camera").clicked() {self.scenario.open=true;}
                if ui.button("Materials").clicked() {self.textures.open=true;}
                if ui.button("Event Graph").clicked() {self.event_graph.open=true;}
                if ui.button("Console").clicked() {self.console.open=true;}

                ui.label("Map Folder:");
                ui.text_edit_singleline(&mut self.map_path_input);

                if ui.button("Load Folder").clicked() {
                    self.pending_action = Some(PendingAction::Load(PathBuf::from(
                        self.map_path_input.trim(),
                    )));
                }

                if ui.button("Safe Save").clicked() {
                    if let Some(session) = &mut self.session {
                        let path = PathBuf::from(self.map_path_input.trim());
                        if path.as_os_str().is_empty() {
                            self.status_message =
                                "Error: Please specify target map directory path before saving."
                                    .to_string();
                            self.status_is_error = true;
                        } else {
                            match safe_save_map(&path, session) {
                                Ok(report) => {
                                    self.status_message = format!(
                                        "Saved safely! Backup: {:?}",
                                        report.boden_backup.map(|p| p
                                            .file_name()
                                            .unwrap()
                                            .to_string_lossy()
                                            .into_owned())
                                    );
                                    self.status_is_error = false;
                                }
                                Err(err) => {
                                    self.status_message = format!("Safe Save Rejected: {}", err);
                                    self.status_is_error = true;
                                }
                            }
                        }
                    }
                }

                ui.separator();

                let can_undo = self.session.as_ref().map(|s| s.can_undo()).unwrap_or(false);
                if ui
                    .add_enabled(can_undo, egui::Button::new("Undo (Ctrl+Z)"))
                    .clicked()
                {
                    if let Some(s) = &mut self.session {
                        s.undo();
                        self.texture_dirty = true;
                    }
                }

                let can_redo = self.session.as_ref().map(|s| s.can_redo()).unwrap_or(false);
                if ui
                    .add_enabled(can_redo, egui::Button::new("Redo (Ctrl+Y)"))
                    .clicked()
                {
                    if let Some(s) = &mut self.session {
                        s.redo();
                        self.texture_dirty = true;
                    }
                }
            });

            // 狀態列
            ui.horizontal(|ui| {
                let color = if self.status_is_error {
                    Color32::from_rgb(240, 80, 80)
                } else {
                    Color32::from_rgb(100, 200, 100)
                };
                ui.colored_label(color, &self.status_message);

                if let Some(session) = &self.session {
                    if session.is_dirty() {
                        ui.colored_label(Color32::YELLOW, "[Unsaved Changes]");
                    }
                }
            });
        });

        // 左側工具面板
        egui::SidePanel::left("left_tools_panel")
            .min_width(220.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui,|ui| {
                ui.heading("Tools & Settings");
                ui.separator();

                ui.label("Display Mode:");
                ui.horizontal(|ui| {
                    if ui
                        .selectable_value(&mut self.view_mode, ViewMode::Blended, "Blended")
                        .clicked()
                    {
                        self.texture_dirty = true;
                    }
                    if ui
                        .selectable_value(&mut self.view_mode, ViewMode::Heightmap, "Height")
                        .clicked()
                    {
                        self.texture_dirty = true;
                    }
                    if ui
                        .selectable_value(&mut self.view_mode, ViewMode::Collision, "Collision")
                        .clicked()
                    {
                        self.texture_dirty = true;
                    }
                });

                ui.separator();
                ui.label("Brush Mode:");
                ui.selectable_value(
                    &mut self.active_tool,
                    ActiveTool::HeightRaise,
                    "Raise (Height)",
                );
                ui.selectable_value(
                    &mut self.active_tool,
                    ActiveTool::HeightLower,
                    "Lower (Height)",
                );
                ui.selectable_value(
                    &mut self.active_tool,
                    ActiveTool::HeightFlatten,
                    "Flatten (Height)",
                );
                ui.selectable_value(
                    &mut self.active_tool,
                    ActiveTool::HeightSmooth,
                    "Smooth (Height)",
                );
                ui.selectable_value(
                    &mut self.active_tool,
                    ActiveTool::HeightRoughen,
                    "Roughen (Noise)",
                );
                ui.selectable_value(
                    &mut self.active_tool,
                    ActiveTool::Water,
                    "Water (Height + Collision)",
                );
                ui.selectable_value(
                    &mut self.active_tool,
                    ActiveTool::CollisionBlock,
                    "Block (Collision)",
                );
                ui.selectable_value(
                    &mut self.active_tool,
                    ActiveTool::CollisionClear,
                    "Clear (Collision)",
                );

                ui.separator();
                ui.add(
                    egui::Slider::new(&mut self.brush_radius, 1.0..=64.0).text("Radius (vertices)"),
                );
                ui.add(egui::Slider::new(&mut self.brush_strength, 1..=64).text("Strength"));

                if self.active_tool == ActiveTool::HeightFlatten {
                    ui.checkbox(&mut self.sample_flatten_target, "Sample first dab height");
                    if !self.sample_flatten_target {
                        ui.add(
                            egui::Slider::new(&mut self.flatten_target, 0..=255)
                                .text("Flatten Target"),
                        );
                    }
                }
                if self.active_tool == ActiveTool::Water {
                    ui.add(
                        egui::Slider::new(&mut self.water_surface, 0.0..=255.0)
                            .text("Water surface (G units)"),
                    );
                    ui.add(egui::Slider::new(&mut self.water_bed, 0..=254).text("Bed height"));
                    ui.label("Local excavation/blocking; set game water level separately.");
                    ui.label(format!("World water: {:.2}; Heightmapstep: {}",self.water_surface*self.height_step,self.height_step));
                    if self.water_surface<4.0 {ui.label("Water surface too low for the water brush (minimum 4).");}
                }
                if self.active_tool == ActiveTool::HeightRoughen {
                    ui.add(
                        egui::Slider::new(&mut self.roughness_seed, 0..=1000).text("Noise Seed"),
                    );
                }

                ui.separator();
                ui.heading("Map Diagnostics");
                if let Some(session) = &self.session {
                    let v_size = session.vertex_size;
                    let is_standard_257 = v_size == 257;
                    if is_standard_257 {
                        ui.label(format!(
                            "Height Dimensions: {}x{} (Standard)",
                            v_size, v_size
                        ));
                    } else {
                        ui.colored_label(
                            Color32::from_rgb(255, 180, 50),
                            format!(
                                "Height Dimensions: {}x{} (Non-257 Compatible)",
                                v_size, v_size
                            ),
                        );
                    }
                    ui.label(format!(
                        "Collision Grid: {}x{}",
                        session.collision_size, session.collision_size
                    ));
                    ui.label("Channel: Green channel height decoded");
                    if session.raw_boden_bmp.is_some() {
                        ui.colored_label(Color32::LIGHT_GREEN, "Original R/B pixels preserved");
                    } else {
                        ui.label("Synthesized grayscale heightmap");
                    }
                }

                ui.separator();
                ui.heading("Quick Presets");
                if ui.button("New Blank 257x257 (Retail)").clicked() {
                    self.pending_action = Some(PendingAction::Blank(257));
                }
                if ui.button("New Blank 193x193 (TUTOR_01)").clicked() {
                    self.pending_action = Some(PendingAction::Blank(193));
                }
                });
            });

        // 中央畫布檢視區
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.texture_dirty || self.texture.is_none() {
                self.update_texture(ctx);
            }

            let Some(texture) = &self.texture else { return };
            let Some(session) = &self.session else { return };

            let (response, painter) =
                ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
            let canvas_rect = response.rect;

            // 計算畫布置中顯示
            let map_aspect = 1.0;
            let display_size = if canvas_rect.width() / canvas_rect.height() > map_aspect {
                Vec2::new(canvas_rect.height() * map_aspect, canvas_rect.height())
            } else {
                Vec2::new(canvas_rect.width(), canvas_rect.width() / map_aspect)
            } * self.zoom;

            let top_left = canvas_rect.min + (canvas_rect.size() - display_size) * 0.5 + self.pan;
            let map_rect = Rect::from_min_size(top_left, display_size);

            painter.image(
                texture.id(),
                map_rect,
                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );

            // 處理滑鼠操作繪製
            if let Some(mouse_pos) = response.hover_pos() {
                if map_rect.contains(mouse_pos) {
                    let norm_x = (mouse_pos.x - map_rect.min.x) / map_rect.width();
                    let norm_y = (mouse_pos.y - map_rect.min.y) / map_rect.height();

                    let grid_x = norm_x * (session.vertex_size - 1) as f32;
                    let grid_y = norm_y * (session.vertex_size - 1) as f32;

                    // 繪製筆刷圓圈指示器
                    let pixel_radius =
                        self.brush_radius * (map_rect.width() / (session.vertex_size - 1) as f32);
                    painter.circle_stroke(
                        mouse_pos,
                        pixel_radius,
                        Stroke::new(1.5_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 180)),
                    );

                    // 筆刷塗畫
                    if self.pending_action.is_none()
                        && (response.is_pointer_button_down_on()
                            && ctx.input(|i| i.pointer.primary_down())
                            || response.clicked_by(egui::PointerButton::Primary))
                    {
                        if !self.is_painting {
                            self.stroke_path.clear();
                        }
                        self.is_painting = true;
                        let tool = self.active_tool;
                        let radius = self.brush_radius;
                        let strength = self.brush_strength;
                        let seed = self.roughness_seed;

                        let points = self.stroke_path.advance((
                            grid_x.round_ties_even() as i32,
                            grid_y.round_ties_even() as i32,
                        ));

                        if let Some(session_mut) = &mut self.session {
                            for (x, y) in points {
                                let grid_x = x as f32;
                                let grid_y = y as f32;
                                if tool == ActiveTool::HeightFlatten
                                    && self.stroke_path.flatten_target.is_none()
                                {
                                    self.stroke_path.flatten_target =
                                        Some(if self.sample_flatten_target {
                                            session_mut.heights()
                                                [y as usize * session_mut.vertex_size + x as usize]
                                        } else {
                                            self.flatten_target
                                        });
                                }
                                let flat_target = self.stroke_path.flatten_target;
                                let result = match tool {
                                    ActiveTool::HeightRaise => session_mut.paint_height(
                                        grid_x,
                                        grid_y,
                                        radius,
                                        HeightBrushOp::Raise,
                                        strength,
                                        None,
                                        seed,
                                    ),
                                    ActiveTool::HeightLower => session_mut.paint_height(
                                        grid_x,
                                        grid_y,
                                        radius,
                                        HeightBrushOp::Lower,
                                        strength,
                                        None,
                                        seed,
                                    ),
                                    ActiveTool::HeightFlatten => session_mut.paint_height(
                                        grid_x,
                                        grid_y,
                                        radius,
                                        HeightBrushOp::Flatten,
                                        strength,
                                        flat_target,
                                        seed,
                                    ),
                                    ActiveTool::HeightSmooth => session_mut.paint_height(
                                        grid_x,
                                        grid_y,
                                        radius,
                                        HeightBrushOp::Smooth,
                                        strength,
                                        None,
                                        seed,
                                    ),
                                    ActiveTool::HeightRoughen => session_mut.paint_height(
                                        grid_x,
                                        grid_y,
                                        radius,
                                        HeightBrushOp::Roughen,
                                        strength,
                                        None,
                                        seed,
                                    ),
                                    ActiveTool::Water if self.water_surface < 4.0 => Err(against_rome_editor::terrain::Error::InvalidParameters),
                                    ActiveTool::Water => session_mut
                                        .paint_water(
                                            grid_x,
                                            grid_y,
                                            radius,
                                            strength,
                                            self.water_bed,
                                            self.water_surface,
                                        )
                                        .map(|s| s.heights),
                                    ActiveTool::CollisionBlock => {
                                        let col_scale = session_mut.collision_size as f32
                                            / (session_mut.vertex_size - 1) as f32;
                                        session_mut.paint_collision(
                                            grid_x * col_scale,
                                            grid_y * col_scale,
                                            radius * col_scale,
                                            CollisionBrushOp::Block,
                                        )
                                    }
                                    ActiveTool::CollisionClear => {
                                        let col_scale = session_mut.collision_size as f32
                                            / (session_mut.vertex_size - 1) as f32;
                                        session_mut.paint_collision(
                                            grid_x * col_scale,
                                            grid_y * col_scale,
                                            radius * col_scale,
                                            CollisionBrushOp::Clear,
                                        )
                                    }
                                };
                                if let Err(error) = result {
                                    self.status_message = format!("Brush rejected: {error}");
                                    self.status_is_error = true;
                                }
                            }
                            self.texture_dirty = true;
                        }
                    }
                }
            }

            // 放開拖曳時提交筆劃
            if !ctx.input(|i| i.pointer.primary_down()) && self.is_painting {
                self.is_painting = false;
                if let Some(session_mut) = &mut self.session {
                    session_mut.commit_stroke();
                }
                self.stroke_path.clear();
            }
        });
        self.handle_pending_action(ctx);
        self.toolbox.show(ctx);
        self.navigation.show(ctx,self.session.as_ref(),self.water_surface);
        self.placement.show(ctx);
        self.scenario.show(ctx);
        self.textures.show(ctx);
        self.event_graph.show(ctx);
        self.console.show(ctx);
    }
}
