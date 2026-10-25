//! Age of Empires II 風格小地圖導航之純幾何計算邏輯與導航面板。
//! 移植自 C# `src.MapEditor/MinimapNavigator.cs`，提供保持長寬比的黑邊（Letterboxing）換算、
//! 滑鼠點與地圖 Tile 座標互轉、主 2D 畫布可見範圍矩形投影，以及視圖聚焦平移向量計算。

use against_rome_ambience::Language as AppLanguage;
use crate::app::tr;
use crate::terrain::TerrainSession;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, TextureHandle, Vec2};

/// 小地圖顯示之標記類型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinimapMarkerKind {
    /// 當前選取之物件（繪製為高亮黃點與白色瞄準外環）。
    SelectedObject,
    /// 劇本起始相機鏡頭（繪製為青藍色菱形標記）。
    Camera,
    /// 劇本事件生成點（繪製為隊伍代表色實心圓點）。
    Spawn,
}

/// 繪製於小地圖上之輕量標記資訊。
#[derive(Debug, Clone, PartialEq)]
pub struct MinimapMarker {
    pub world_x: f32,
    pub world_z: f32,
    pub color: Color32,
    pub kind: MinimapMarkerKind,
    pub label: Option<String>,
}

impl MinimapMarker {
    pub fn selected(world_x: f32, world_z: f32) -> Self {
        Self {
            world_x,
            world_z,
            color: Color32::from_rgb(255, 230, 40),
            kind: MinimapMarkerKind::SelectedObject,
            label: None,
        }
    }

    pub fn camera(world_x: f32, world_z: f32) -> Self {
        Self {
            world_x,
            world_z,
            color: Color32::from_rgb(80, 200, 255),
            kind: MinimapMarkerKind::Camera,
            label: None,
        }
    }

    pub fn spawn(world_x: f32, world_z: f32, color: Color32, label: Option<String>) -> Self {
        Self {
            world_x,
            world_z,
            color,
            kind: MinimapMarkerKind::Spawn,
            label,
        }
    }
}

/// 點擊或拖曳小地圖時產生之導航目標結果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinimapNavResult {
    /// 2D 畫布之目標平移向量。
    pub canvas_pan: Vec2,
    /// 目標世界 X 座標 (0..16384)。
    pub world_x: f32,
    /// 目標世界 Z 座標 (0..16384)。
    pub world_z: f32,
    /// 目標 Tile 座標。
    pub tile_pos: Pos2,
}

/// 小地圖純幾何計算輔助類別，嚴格對照 C# `MinimapNavigator` 數學契約。
pub struct MinimapNavigator;

impl MinimapNavigator {
    /// 計算 PictureBox 在 Zoom 模式（保持長寬比）下，影像在控制項客戶區中的實際繪製矩形。
    /// 若控制項或影像尺寸無效，回傳 `Rect::ZERO`。
    pub fn calculate_zoomed_image_bounds(client_size: Vec2, image_size: Vec2) -> Rect {
        if client_size.x <= 0.0 || client_size.y <= 0.0 || image_size.x <= 0.0 || image_size.y <= 0.0 {
            return Rect::ZERO;
        }

        let scale = (client_size.x / image_size.x).min(client_size.y / image_size.y);
        let width = (image_size.x * scale).round().max(1.0);
        let height = (image_size.y * scale).round().max(1.0);
        let x = ((client_size.x - width) / 2.0).floor();
        let y = ((client_size.y - height) / 2.0).floor();

        Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, height))
    }

    /// 將小地圖上的滑鼠點轉換為地圖 tile 座標（浮點數）。
    /// 若點位於影像矩形外部（黑邊區域），則回傳 `None`。
    pub fn try_point_to_tile(point: Pos2, image_bounds: Rect, map_dimension: u32) -> Option<Pos2> {
        if map_dimension == 0 || image_bounds.width() <= 0.0 || image_bounds.height() <= 0.0 {
            return None;
        }

        // C# Contains 語意：左閉右開 [min, max)
        if point.x < image_bounds.min.x
            || point.x >= image_bounds.max.x
            || point.y < image_bounds.min.y
            || point.y >= image_bounds.max.y
        {
            return None;
        }

        let u = (point.x - image_bounds.min.x) / image_bounds.width();
        let v = (point.y - image_bounds.min.y) / image_bounds.height();

        // 依 minimap.bmp 與 boden.bmp / MapCanvasControl 的座標系：
        // 影像左上角為 (0,0)，X 向右對應 tileX (0 -> map_dimension)，Y 向下對應 tileY (0 -> map_dimension)。
        let dim = map_dimension as f32;
        let tile_x = (u * dim).clamp(0.0, dim);
        let tile_y = (v * dim).clamp(0.0, dim);

        Some(Pos2::new(tile_x, tile_y))
    }

    /// 將地圖 tile 座標轉換為小地圖影像矩形內的客戶區點位。
    pub fn tile_to_point(tile_position: Pos2, image_bounds: Rect, map_dimension: u32) -> Pos2 {
        if map_dimension == 0 || image_bounds.width() <= 0.0 || image_bounds.height() <= 0.0 {
            return image_bounds.min;
        }

        let dim = map_dimension as f32;
        let u = tile_position.x / dim;
        let v = tile_position.y / dim;

        Pos2::new(
            image_bounds.min.x + u * image_bounds.width(),
            image_bounds.min.y + v * image_bounds.height(),
        )
    }

    /// 依據 2D 畫布之可見 SceneBounds 與視圖尺寸，計算 2D 畫布在小地圖上的可見區域矩形。
    /// `scene_bounds` 代表地圖內容相對於畫布客戶區 (0, 0) 的邊界矩形。
    pub fn calculate_canvas_visible_minimap_rect(
        scene_bounds: Rect,
        canvas_client_size: Vec2,
        minimap_image_bounds: Rect,
        map_dimension: u32,
    ) -> Rect {
        if map_dimension == 0
            || scene_bounds.width() <= 0.0
            || scene_bounds.height() <= 0.0
            || minimap_image_bounds.width() <= 0.0
            || minimap_image_bounds.height() <= 0.0
        {
            return Rect::ZERO;
        }

        // 畫布的可視視窗為 (0, 0, canvas_client_size.x, canvas_client_size.y)
        // 算出該視窗相對於 scene_bounds 的交集比例
        let left = (0.0f32).max(-scene_bounds.min.x) / scene_bounds.width();
        let top = (0.0f32).max(-scene_bounds.min.y) / scene_bounds.height();
        let right = (scene_bounds.width()).min(canvas_client_size.x - scene_bounds.min.x)
            / scene_bounds.width();
        let bottom = (scene_bounds.height()).min(canvas_client_size.y - scene_bounds.min.y)
            / scene_bounds.height();

        if right <= left || bottom <= top {
            return Rect::ZERO;
        }

        let u0 = left.clamp(0.0, 1.0);
        let v0 = top.clamp(0.0, 1.0);
        let u1 = right.clamp(0.0, 1.0);
        let v1 = bottom.clamp(0.0, 1.0);

        let w = (2.0f32).max((u1 - u0) * minimap_image_bounds.width());
        let h = (2.0f32).max((v1 - v0) * minimap_image_bounds.height());

        Rect::from_min_size(
            Pos2::new(
                minimap_image_bounds.min.x + u0 * minimap_image_bounds.width(),
                minimap_image_bounds.min.y + v0 * minimap_image_bounds.height(),
            ),
            Vec2::new(w, h),
        )
    }

    /// 根據目標 Tile 座標與 2D 畫布幾何，換算讓該 Tile 置中顯示於畫布時的 `pan` 向量。
    pub fn focus_tile_pan(
        tile_pos: Pos2,
        map_dimension: u32,
        canvas_size: Vec2,
        zoom: f32,
    ) -> Vec2 {
        if map_dimension == 0 || canvas_size.x <= 0.0 || canvas_size.y <= 0.0 {
            return Vec2::ZERO;
        }

        let map_aspect = 1.0;
        let base_size = if canvas_size.x / canvas_size.y > map_aspect {
            Vec2::new(canvas_size.y * map_aspect, canvas_size.y)
        } else {
            Vec2::new(canvas_size.x, canvas_size.x / map_aspect)
        };
        let display_size = base_size * zoom;

        // Tile 中心歸一化位置 (0.0 ~ 1.0)
        let dim = map_dimension as f32;
        let u = (tile_pos.x + 0.5) / dim;
        let v = (tile_pos.y + 0.5) / dim;

        // 當 pan 為 0 時，地圖中心點 (u=0.5, v=0.5) 位於 canvas 中心。
        // 將 (u, v) 置中所需的 pan 即為負的相對位移向量。
        Vec2::new(-(u - 0.5) * display_size.x, -(v - 0.5) * display_size.y)
    }

    /// 連續世界座標的視角置中，不加入 Tile 中心半格偏移。
    pub fn focus_world_pan(x: f32, z: f32, size: Vec2, zoom: f32) -> Vec2 {
        if !x.is_finite() || !z.is_finite() || !zoom.is_finite() || zoom <= 0.0 {
            return Vec2::ZERO;
        }
        Self::focus_tile_pan(Pos2::new(x / 16384.0 - 0.5, z / 16384.0 - 0.5), 1, size, zoom)
    }

    /// 將世界座標 (0..16384) 投影為小地圖繪製矩形內的客戶區點位。
    pub fn world_to_minimap_point(world_x: f32, world_z: f32, minimap_bounds: Rect) -> Pos2 {
        let u = (world_x / 16384.0).clamp(0.0, 1.0);
        let v = (world_z / 16384.0).clamp(0.0, 1.0);
        Pos2::new(
            minimap_bounds.min.x + u * minimap_bounds.width(),
            minimap_bounds.min.y + v * minimap_bounds.height(),
        )
    }

    /// 將小地圖上的客戶區點位轉換為世界座標 (0..16384)。
    pub fn point_to_world(point: Pos2, minimap_bounds: Rect) -> Option<(f32, f32)> {
        if !point.is_finite() || !minimap_bounds.is_finite() || minimap_bounds.width() <= 0.0 || minimap_bounds.height() <= 0.0 {
            return None;
        }
        if point.x < minimap_bounds.min.x
            || point.x >= minimap_bounds.max.x
            || point.y < minimap_bounds.min.y
            || point.y >= minimap_bounds.max.y
        {
            return None;
        }
        let u = ((point.x - minimap_bounds.min.x) / minimap_bounds.width()).clamp(0.0, 1.0);
        let v = ((point.y - minimap_bounds.min.y) / minimap_bounds.height()).clamp(0.0, 1.0);
        Some((u * 16384.0, v * 16384.0))
    }
}

/// 小地圖導航面板 UI 元件。
#[derive(Debug, Clone)]
pub struct MinimapUi {
    pub open: bool,
    pub show_selection: bool,
    pub show_markers: bool,
}

impl Default for MinimapUi {
    fn default() -> Self {
        Self {
            open: true,
            show_selection: true,
            show_markers: true,
        }
    }
}

impl MinimapUi {
    /// 顯示小地圖視窗，繪製縮圖、主 2D 畫布視野矩形、選取物件與劇本標記，並處理點擊/拖曳導航。
    /// 若觸發視野跳轉，回傳計算後之導航目標結構 `MinimapNavResult`。
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        session: Option<&TerrainSession>,
        texture: Option<&TextureHandle>,
        canvas_rect: Rect,
        map_rect: Rect,
        zoom: f32,
        lang: AppLanguage,
        markers: &[MinimapMarker],
    ) -> Option<MinimapNavResult> {
        if !self.open {
            return None;
        }

        let mut target_nav = None;
        let title = tr(lang, "Minimap Overview", "小地圖總覽");

        egui::Window::new(title)
            .id(egui::Id::new("minimap_overview_window"))
            .open(&mut self.open)
            .default_size(Vec2::new(240.0, 260.0))
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr(lang, "Click to navigate", "點擊導航視角"));
                    ui.separator();
                    ui.checkbox(&mut self.show_selection, tr(lang, "Selection", "選取物件"))
                        .on_hover_text(tr(lang, "Show selected objects indicator on minimap", "在小地圖上顯示當前選取物件標記"));
                    ui.checkbox(&mut self.show_markers, tr(lang, "Markers", "劇本標記"))
                        .on_hover_text(tr(lang, "Show scenario camera and spawn markers", "在小地圖上顯示劇本相機與部隊生成標記"));
                });

                let dimension = session.map_or(64, |s| s.vertex_size as u32);
                let available = Vec2::new(
                    ui.available_size().x.max(100.0),
                    ui.available_size().y.max(100.0),
                );
                let (response, painter) =
                    ui.allocate_painter(available, egui::Sense::click_and_drag());
                let client_rect = response.rect;

                // 保持 1:1 地圖比例之 letterbox 影像矩形
                let image_size = Vec2::new(dimension as f32, dimension as f32);
                let local_bounds =
                    MinimapNavigator::calculate_zoomed_image_bounds(client_rect.size(), image_size);
                let screen_bounds = local_bounds.translate(client_rect.min.to_vec2());

                // 底色與縮圖繪製
                painter.rect_filled(client_rect, 0.0, Color32::from_rgb(18, 22, 28));
                painter.rect_filled(screen_bounds, 0.0, Color32::from_rgb(28, 32, 40));

                if let Some(tex) = texture {
                    painter.image(
                        tex.id(),
                        screen_bounds,
                        Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }

                // 繪製劇本標記與選取物件
                if (self.show_markers || self.show_selection) && screen_bounds.width() > 0.0 {
                    for marker in markers {
                        if marker.kind == MinimapMarkerKind::SelectedObject && !self.show_selection {
                            continue;
                        }
                        if marker.kind != MinimapMarkerKind::SelectedObject && !self.show_markers {
                            continue;
                        }
                        let pt = MinimapNavigator::world_to_minimap_point(
                            marker.world_x,
                            marker.world_z,
                            screen_bounds,
                        );
                        match marker.kind {
                            MinimapMarkerKind::SelectedObject => {
                                // 亮白色外環瞄準線
                                painter.circle_stroke(
                                    pt,
                                    5.5,
                                    Stroke::new(1.5_f32, Color32::WHITE),
                                );
                                // 亮金色核心
                                painter.circle_filled(pt, 3.5, marker.color);
                            }
                            MinimapMarkerKind::Camera => {
                                let d = 4.0;
                                let diamond = vec![
                                    Pos2::new(pt.x, pt.y - d),
                                    Pos2::new(pt.x + d, pt.y),
                                    Pos2::new(pt.x, pt.y + d),
                                    Pos2::new(pt.x - d, pt.y),
                                ];
                                painter.add(egui::Shape::convex_polygon(
                                    diamond,
                                    marker.color,
                                    Stroke::new(1.0_f32, Color32::WHITE),
                                ));
                            }
                            MinimapMarkerKind::Spawn => {
                                painter.circle_filled(pt, 3.0, marker.color);
                                painter.circle_stroke(
                                    pt,
                                    3.0,
                                    Stroke::new(1.0_f32, Color32::from_rgb(20, 20, 20)),
                                );
                            }
                        }
                    }
                }

                // 計算主畫布可見區域在小地圖上的投影矩形
                let scene_bounds = Rect::from_min_size(
                    Pos2::new(
                        map_rect.min.x - canvas_rect.min.x,
                        map_rect.min.y - canvas_rect.min.y,
                    ),
                    map_rect.size(),
                );
                let local_view_rect = MinimapNavigator::calculate_canvas_visible_minimap_rect(
                    scene_bounds,
                    canvas_rect.size(),
                    local_bounds,
                    dimension,
                );

                if local_view_rect.width() > 0.0 && local_view_rect.height() > 0.0 {
                    let screen_view_rect = local_view_rect.translate(client_rect.min.to_vec2());

                    // 與 C# 契約一致：半透明填充 (35/255)、黑色外框陰影 (180/255)、金色指示框 (255, 230, 40)
                    painter.rect_filled(
                        screen_view_rect,
                        0.0,
                        Color32::from_rgba_unmultiplied(255, 255, 255, 35),
                    );
                    painter.rect_stroke(
                        screen_view_rect.expand(0.5),
                        0.0,
                        Stroke::new(3.0_f32, Color32::from_rgba_unmultiplied(0, 0, 0, 180)),
                    );
                    painter.rect_stroke(
                        screen_view_rect,
                        0.0,
                        Stroke::new(1.5_f32, Color32::from_rgb(255, 230, 40)),
                    );
                }

                // 處理左鍵點擊與拖曳導航
                if response.clicked() || response.dragged_by(egui::PointerButton::Primary)
                {
                    if let Some(hover_pos) = response.hover_pos() {
                        let local_pt = Pos2::new(
                            hover_pos.x - client_rect.min.x,
                            hover_pos.y - client_rect.min.y,
                        );
                        if let Some(tile_pos) =
                            MinimapNavigator::try_point_to_tile(local_pt, local_bounds, dimension)
                        {
                            let world_x = (tile_pos.x / dimension as f32) * 16384.0;
                            let world_z = (tile_pos.y / dimension as f32) * 16384.0;
                            let new_pan = MinimapNavigator::focus_world_pan(world_x, world_z, canvas_rect.size(), zoom);
                            target_nav = Some(MinimapNavResult {
                                canvas_pan: new_pan,
                                world_x,
                                world_z,
                                tile_pos,
                            });
                        }
                    }
                }
            });

        target_nav
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_click_release_navigates_without_dragging() {
        let ctx = egui::Context::default();
        let mut mini = MinimapUi::default();
        let canvas = Rect::from_min_size(Pos2::ZERO, Vec2::splat(500.0));
        let mut bounds = Rect::ZERO;
        for _ in 0..3 {
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                mini.show(ctx, None, None, canvas, canvas, 1.0, AppLanguage::English, &[]);
            });
            bounds = output.shapes.iter().find_map(|s| match &s.shape {
                egui::Shape::Rect(r) if r.fill == Color32::from_rgb(28, 32, 40) => Some(r.rect),
                _ => None,
            }).expect("小地圖影像矩形");
        }
        let p = bounds.center();
        let mut result = None;
        for pressed in [true, false] {
            let input = egui::RawInput {
                events: vec![egui::Event::PointerMoved(p), egui::Event::PointerButton {
                    pos: p, button: egui::PointerButton::Primary, pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                result = mini.show(ctx, None, None, canvas, canvas, 1.0, AppLanguage::English, &[]);
            });
        }
        let nav = result.expect("放開左鍵應導航");
        assert!((nav.world_x - 8192.0).abs() < 0.1);
        assert!((nav.world_z - 8192.0).abs() < 0.1);
        assert!(nav.canvas_pan.length() < 0.01);
    }

    #[test]
    fn test_minimap_zoom_bounds_letterbox_horizontal_and_vertical() {
        // 寬長比小於影像（水平裁切／上下黑邊）：例如 client 是 200x400，image 是 100x100
        let client_tall = Vec2::new(200.0, 400.0);
        let image_square = Vec2::new(100.0, 100.0);
        let bounds_tall = MinimapNavigator::calculate_zoomed_image_bounds(client_tall, image_square);
        assert_eq!(bounds_tall, Rect::from_min_size(Pos2::new(0.0, 100.0), Vec2::new(200.0, 200.0)));

        // 寬長比大於影像（左右黑邊）：例如 client 是 400x200，image 是 100x100
        let client_wide = Vec2::new(400.0, 200.0);
        let bounds_wide = MinimapNavigator::calculate_zoomed_image_bounds(client_wide, image_square);
        assert_eq!(bounds_wide, Rect::from_min_size(Pos2::new(100.0, 0.0), Vec2::new(200.0, 200.0)));

        // 非 1:1 的影像長寬比：例如 client 是 300x300，image 是 200x100（2:1）
        let image_rect = Vec2::new(200.0, 100.0);
        let bounds_rect = MinimapNavigator::calculate_zoomed_image_bounds(Vec2::new(300.0, 300.0), image_rect);
        assert_eq!(bounds_rect, Rect::from_min_size(Pos2::new(0.0, 75.0), Vec2::new(300.0, 150.0)));

        // 零尺寸與無效尺寸防護
        assert_eq!(MinimapNavigator::calculate_zoomed_image_bounds(Vec2::ZERO, image_square), Rect::ZERO);
        assert_eq!(MinimapNavigator::calculate_zoomed_image_bounds(client_tall, Vec2::ZERO), Rect::ZERO);
        assert_eq!(MinimapNavigator::calculate_zoomed_image_bounds(Vec2::new(-10.0, 20.0), image_square), Rect::ZERO);
    }

    #[test]
    fn test_minimap_point_to_tile_outside_clicks_are_ignored() {
        let image_bounds = Rect::from_min_size(Pos2::new(50.0, 50.0), Vec2::new(200.0, 200.0));
        let dimension = 64;

        // 影像左方黑邊
        assert_eq!(MinimapNavigator::try_point_to_tile(Pos2::new(20.0, 100.0), image_bounds, dimension), None);
        // 影像上方黑邊
        assert_eq!(MinimapNavigator::try_point_to_tile(Pos2::new(100.0, 10.0), image_bounds, dimension), None);
        // 影像右方黑邊
        assert_eq!(MinimapNavigator::try_point_to_tile(Pos2::new(260.0, 100.0), image_bounds, dimension), None);
        // 影像下方黑邊
        assert_eq!(MinimapNavigator::try_point_to_tile(Pos2::new(100.0, 260.0), image_bounds, dimension), None);
        // 負座標
        assert_eq!(MinimapNavigator::try_point_to_tile(Pos2::new(-5.0, -5.0), image_bounds, dimension), None);
        // 右界與下界邊緣 [min, max)
        assert_eq!(MinimapNavigator::try_point_to_tile(Pos2::new(250.0, 100.0), image_bounds, dimension), None);
        assert_eq!(MinimapNavigator::try_point_to_tile(Pos2::new(100.0, 250.0), image_bounds, dimension), None);
    }

    #[test]
    fn test_minimap_coordinates_round_trip_accurately() {
        let image_bounds = Rect::from_min_size(Pos2::new(20.0, 30.0), Vec2::new(200.0, 200.0));
        let dimension = 64;

        let test_tiles = [
            Pos2::new(0.0, 0.0),
            Pos2::new(32.0, 32.0),
            Pos2::new(16.5, 48.25),
            Pos2::new(63.0, 63.0),
        ];

        for tile in test_tiles {
            let pt = MinimapNavigator::tile_to_point(tile, image_bounds, dimension);
            let back = MinimapNavigator::try_point_to_tile(pt, image_bounds, dimension).expect("tile within bounds");
            assert!((tile.x - back.x).abs() < 0.5, "tile.x ({}) != back.x ({})", tile.x, back.x);
            assert!((tile.y - back.y).abs() < 0.5, "tile.y ({}) != back.y ({})", tile.y, back.y);
        }
    }

    #[test]
    fn test_minimap_canvas_visible_rect_computation_handles_zoom_and_pan() {
        let minimap_image = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(200.0, 200.0));
        let dimension = 64;
        let canvas_client = Vec2::new(400.0, 400.0);

        // 場景置中且無縮放（scene_bounds 正好等於 canvas_client）
        let scene_centered = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(400.0, 400.0));
        let visible_all = MinimapNavigator::calculate_canvas_visible_minimap_rect(
            scene_centered,
            canvas_client,
            minimap_image,
            dimension,
        );
        assert!((visible_all.min.x - 0.0).abs() < 0.01);
        assert!((visible_all.min.y - 0.0).abs() < 0.01);
        assert!((visible_all.width() - 200.0).abs() < 0.01);
        assert!((visible_all.height() - 200.0).abs() < 0.01);

        // 放大 2 倍且向右下方平移（可視區域只包含場景左上角）
        let scene_zoomed = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(800.0, 800.0));
        let visible_top_left = MinimapNavigator::calculate_canvas_visible_minimap_rect(
            scene_zoomed,
            canvas_client,
            minimap_image,
            dimension,
        );
        assert!((visible_top_left.min.x - 0.0).abs() < 0.01);
        assert!((visible_top_left.min.y - 0.0).abs() < 0.01);
        assert!((visible_top_left.width() - 100.0).abs() < 0.01);
        assert!((visible_top_left.height() - 100.0).abs() < 0.01);

        // 完全移出視窗的情況回傳 ZERO
        let scene_out_of_view = Rect::from_min_size(Pos2::new(600.0, 600.0), Vec2::new(400.0, 400.0));
        let visible_none = MinimapNavigator::calculate_canvas_visible_minimap_rect(
            scene_out_of_view,
            canvas_client,
            minimap_image,
            dimension,
        );
        assert_eq!(visible_none, Rect::ZERO);
    }

    #[test]
    fn test_minimap_focus_tile_pan_centering() {
        let dimension = 64;
        let canvas_size = Vec2::new(400.0, 400.0);
        let zoom = 1.0;

        // 當目標 tile 為中心 (31.5, 31.5) 時，tile 中心剛好在 (32.0, 32.0) -> u = 0.5, v = 0.5
        let center_tile = Pos2::new(31.5, 31.5);
        let center_pan = MinimapNavigator::focus_tile_pan(center_tile, dimension, canvas_size, zoom);
        assert!((center_pan.x).abs() < 0.001);
        assert!((center_pan.y).abs() < 0.001);

        // 放大 2 倍時，目標在中心依然保持 pan = 0
        let center_pan_zoom2 = MinimapNavigator::focus_tile_pan(center_tile, dimension, canvas_size, 2.0);
        assert!((center_pan_zoom2.x).abs() < 0.001);
        assert!((center_pan_zoom2.y).abs() < 0.001);

        // 驗證左上角 tile (0, 0)
        let corner_tile = Pos2::new(0.0, 0.0);
        let corner_pan = MinimapNavigator::focus_tile_pan(corner_tile, dimension, canvas_size, zoom);
        // u = 0.5 / 64 = 0.0078125
        // pan_x = -(u - 0.5) * 400 = -(0.0078125 - 0.5) * 400 = 196.875
        let expected_pan_x = -(0.5 / 64.0 - 0.5) * 400.0;
        assert!((corner_pan.x - expected_pan_x).abs() < 0.001);
        assert!((corner_pan.y - expected_pan_x).abs() < 0.001);
    }

    #[test]
    fn test_minimap_world_and_point_conversions() {
        let bounds = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(100.0, 100.0));

        // (0, 0) 世界座標對應 bounds.min
        let p_min = MinimapNavigator::world_to_minimap_point(0.0, 0.0, bounds);
        assert_eq!(p_min, Pos2::new(10.0, 20.0));

        // (16384, 16384) 世界座標對應 bounds.max
        let p_max = MinimapNavigator::world_to_minimap_point(16384.0, 16384.0, bounds);
        assert_eq!(p_max, Pos2::new(110.0, 120.0));

        // (8192, 8192) 世界座標對應 bounds 中心
        let p_mid = MinimapNavigator::world_to_minimap_point(8192.0, 8192.0, bounds);
        assert_eq!(p_mid, Pos2::new(60.0, 70.0));

        // 逆向轉換驗證
        let (wx, wz) = MinimapNavigator::point_to_world(Pos2::new(60.0, 70.0), bounds).expect("valid point");
        assert!((wx - 8192.0).abs() < 0.1);
        assert!((wz - 8192.0).abs() < 0.1);

        // 越界點回傳 None
        assert_eq!(MinimapNavigator::point_to_world(Pos2::new(5.0, 20.0), bounds), None);
        assert_eq!(MinimapNavigator::point_to_world(Pos2::new(120.0, 70.0), bounds), None);
    }

    #[test]
    fn test_minimap_markers_and_ui_defaults() {
        let marker_sel = MinimapMarker::selected(100.0, 200.0);
        assert_eq!(marker_sel.kind, MinimapMarkerKind::SelectedObject);
        assert_eq!(marker_sel.world_x, 100.0);
        assert_eq!(marker_sel.world_z, 200.0);

        let marker_cam = MinimapMarker::camera(500.0, 600.0);
        assert_eq!(marker_cam.kind, MinimapMarkerKind::Camera);

        let marker_spawn = MinimapMarker::spawn(1000.0, 2000.0, Color32::RED, Some("Wave 1".into()));
        assert_eq!(marker_spawn.kind, MinimapMarkerKind::Spawn);
        assert_eq!(marker_spawn.label.as_deref(), Some("Wave 1"));

        let ui = MinimapUi::default();
        assert!(ui.open);
        assert!(ui.show_selection);
        assert!(ui.show_markers);
    }
}
