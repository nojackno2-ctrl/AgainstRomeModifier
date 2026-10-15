use super::types::*;

pub const COLOR_ADDED: u32 = 0xA000E676;
pub const COLOR_DELETED: u32 = 0xA0FF1744;
pub const COLOR_MODIFIED: u32 = 0xB0FFD600;
pub const COLOR_MOVED: u32 = 0xC0FFA000;
pub const COLOR_BLOCKED: u32 = 0xA0D50000;
pub const COLOR_CLEARED: u32 = 0xA000B0FF;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffMarkerType {
    Added,
    Deleted,
    Modified,
    Moved,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DiffOverlayMarker {
    pub marker_type: DiffMarkerType,
    pub world_x: f32,
    pub world_z: f32,
    pub target_world_x: f32,
    pub target_world_z: f32,
    pub color_argb: u32,
    pub label: String,
    pub sub_label: String,
}

#[derive(Clone, Debug)]
pub struct DiffOverlayOptions {
    pub show_height_heatmap: bool,
    pub show_texture_diff: bool,
    pub show_collision_diff: bool,
    pub show_objects: bool,
    pub global_opacity: f32,
    pub height_sensitivity: f32,
    pub show_movement_vectors: bool,
}

impl Default for DiffOverlayOptions {
    fn default() -> Self {
        Self {
            show_height_heatmap: true,
            show_texture_diff: true,
            show_collision_diff: false,
            show_objects: true,
            global_opacity: 0.7,
            height_sensitivity: 1.0,
            show_movement_vectors: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct HeatmapMaskBuffer {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
}

impl HeatmapMaskBuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width * height],
        }
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, argb: u32) {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x] = argb;
        }
    }

    pub fn get_pixel(&self, x: usize, y: usize) -> u32 {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x]
        } else {
            0
        }
    }
}

pub struct MapVisualDiffOverlay;

impl MapVisualDiffOverlay {
    pub fn blend_argb(dst: u32, src: u32) -> u32 {
        let sa = ((src >> 24) & 0xFF) as f32 / 255.0;
        let sr = ((src >> 16) & 0xFF) as f32;
        let sg = ((src >> 8) & 0xFF) as f32;
        let sb = (src & 0xFF) as f32;

        let da = ((dst >> 24) & 0xFF) as f32 / 255.0;
        let dr = ((dst >> 16) & 0xFF) as f32;
        let dg = ((dst >> 8) & 0xFF) as f32;
        let db = (dst & 0xFF) as f32;

        let out_a = sa + da * (1.0 - sa);
        if out_a <= 0.001 {
            return 0;
        }

        let out_r = ((sr * sa + dr * da * (1.0 - sa)) / out_a).clamp(0.0, 255.0) as u32;
        let out_g = ((sg * sa + dg * da * (1.0 - sa)) / out_a).clamp(0.0, 255.0) as u32;
        let out_b = ((sb * sa + db * da * (1.0 - sa)) / out_a).clamp(0.0, 255.0) as u32;
        let a_byte = (out_a * 255.0).clamp(0.0, 255.0) as u32;

        (a_byte << 24) | (out_r << 16) | (out_g << 8) | out_b
    }

    pub fn generate_heatmap_mask(
        diff: &MapDiffReport,
        output_width: usize,
        output_height: usize,
        options: Option<&DiffOverlayOptions>,
    ) -> HeatmapMaskBuffer {
        let default_opt = DiffOverlayOptions::default();
        let options = options.unwrap_or(&default_opt);

        let mut buffer = HeatmapMaskBuffer::new(output_width, output_height);
        let opacity = options.global_opacity.clamp(0.0, 1.0);
        if opacity <= 0.001 {
            return buffer;
        }

        // 1. 疊加地形高度差異熱力
        if options.show_height_heatmap && diff.height.dimension > 0 && !diff.height.delta_grid.is_empty() {
            let h_dim = diff.height.dimension;
            let deltas = &diff.height.delta_grid;
            let mut max_delta = diff.height.max_raise.max(diff.height.max_lower);
            if max_delta <= 0.001 {
                max_delta = 1.0;
            }

            for py in 0..output_height {
                for px in 0..output_width {
                    let gx = (px as f32 / (output_width - 1).max(1) as f32) * (h_dim - 1) as f32;
                    let gy = (py as f32 / (output_height - 1).max(1) as f32) * (h_dim - 1) as f32;

                    let x0 = (gx.floor() as usize).min(h_dim - 1);
                    let y0 = (gy.floor() as usize).min(h_dim - 1);
                    let x1 = (x0 + 1).min(h_dim - 1);
                    let y1 = (y0 + 1).min(h_dim - 1);

                    let fx = gx - x0 as f32;
                    let fy = gy - y0 as f32;

                    let d00 = deltas[y0 * h_dim + x0];
                    let d10 = deltas[y0 * h_dim + x1];
                    let d01 = deltas[y1 * h_dim + x0];
                    let d11 = deltas[y1 * h_dim + x1];

                    let delta = (1.0 - fx) * (1.0 - fy) * d00
                        + fx * (1.0 - fy) * d10
                        + (1.0 - fx) * fy * d01
                        + fx * fy * d11;

                    if delta.abs() > 0.1 {
                        let norm = (delta.abs() / max_delta * options.height_sensitivity).clamp(0.0, 1.0);
                        let alpha = (norm * 180.0 * opacity).clamp(0.0, 255.0) as u32;

                        let color = if delta > 0.0 {
                            (alpha << 24) | 0x0000E676
                        } else {
                            (alpha << 24) | 0x00FF1744
                        };
                        buffer.set_pixel(px, py, color);
                    }
                }
            }
        }

        // 2. 疊加地表材質變更網格
        if options.show_texture_diff && diff.textures.dimension > 0 && !diff.textures.changed_tiles.is_empty() {
            let t_dim = diff.textures.dimension;
            let tile_w = output_width as f32 / t_dim as f32;
            let tile_h = output_height as f32 / t_dim as f32;

            let tex_alpha = (160.0 * opacity).clamp(0.0, 255.0) as u32;
            let tex_color = (tex_alpha << 24) | (COLOR_MODIFIED & 0x00FFFFFF);

            for change in &diff.textures.changed_tiles {
                let start_x = (change.tile_x as f32 * tile_w).floor() as usize;
                let end_x = ((change.tile_x + 1) as f32 * tile_w).ceil() as usize;
                let start_y = (change.tile_y as f32 * tile_h).floor() as usize;
                let end_y = ((change.tile_y + 1) as f32 * tile_h).ceil() as usize;

                for y in start_y..end_y.min(output_height) {
                    for x in start_x..end_x.min(output_width) {
                        let blended = Self::blend_argb(buffer.get_pixel(x, y), tex_color);
                        buffer.set_pixel(x, y, blended);
                    }
                }
            }
        }

        // 3. 疊加通行碰撞變更
        if options.show_collision_diff && !diff.collision.changes.is_empty() && diff.collision.dimension > 0 {
            let c_dim = diff.collision.dimension;
            let c_w = output_width as f32 / c_dim as f32;
            let c_h = output_height as f32 / c_dim as f32;

            for col in &diff.collision.changes {
                let base_color = if col.old_value == 0 { COLOR_BLOCKED } else { COLOR_CLEARED };
                let a = (((base_color >> 24) & 0xFF) as f32 * opacity).clamp(0.0, 255.0) as u32;
                let col_color = (a << 24) | (base_color & 0x00FFFFFF);

                let start_x = (col.tile_x as f32 * c_w).floor() as usize;
                let end_x = ((col.tile_x + 1) as f32 * c_w).ceil() as usize;
                let start_y = (col.tile_y as f32 * c_h).floor() as usize;
                let end_y = ((col.tile_y + 1) as f32 * c_h).ceil() as usize;

                for y in start_y..end_y.min(output_height) {
                    for x in start_x..end_x.min(output_width) {
                        let blended = Self::blend_argb(buffer.get_pixel(x, y), col_color);
                        buffer.set_pixel(x, y, blended);
                    }
                }
            }
        }

        buffer
    }

    pub fn generate_minimap_overlay(
        diff: &MapDiffReport,
        minimap_width: usize,
        minimap_height: usize,
        options: Option<&DiffOverlayOptions>,
    ) -> HeatmapMaskBuffer {
        let mut buffer = Self::generate_heatmap_mask(diff, minimap_width, minimap_height, options);

        let default_opt = DiffOverlayOptions::default();
        let options = options.unwrap_or(&default_opt);

        if options.show_objects && diff.objects.total_changes() > 0 {
            let scale_x = minimap_width as f32 / 16384.0;
            let scale_z = minimap_height as f32 / 16384.0;

            for obj in &diff.objects.added {
                let mx = (obj.x * scale_x).round() as i32;
                let my = (obj.z * scale_z).round() as i32;
                Self::draw_block(&mut buffer, mx, my, 2, COLOR_ADDED);
            }

            for obj in &diff.objects.deleted {
                let mx = (obj.x * scale_x).round() as i32;
                let my = (obj.z * scale_z).round() as i32;
                Self::draw_block(&mut buffer, mx, my, 2, COLOR_DELETED);
            }

            for m in &diff.objects.modified {
                let mx = (m.current.x * scale_x).round() as i32;
                let my = (m.current.z * scale_z).round() as i32;
                Self::draw_block(&mut buffer, mx, my, 2, COLOR_MODIFIED);
            }
        }

        buffer
    }

    fn draw_block(buffer: &mut HeatmapMaskBuffer, center_x: i32, center_y: i32, size: i32, argb: u32) {
        let half = size / 2;
        for dy in -half..=half {
            for dx in -half..=half {
                let px = center_x + dx;
                let py = center_y + dy;
                if px >= 0 && (px as usize) < buffer.width && py >= 0 && (py as usize) < buffer.height {
                    buffer.set_pixel(px as usize, py as usize, argb);
                }
            }
        }
    }

    pub fn generate_vector_markers(
        diff: &MapDiffReport,
        options: Option<&DiffOverlayOptions>,
    ) -> Vec<DiffOverlayMarker> {
        let default_opt = DiffOverlayOptions::default();
        let options = options.unwrap_or(&default_opt);

        let mut markers = Vec::new();
        if !options.show_objects {
            return markers;
        }

        for obj in &diff.objects.added {
            let label = format!("+ {}", Self::format_object_title(obj));
            let sub_label = format!("坐標: ({:.0}, {:.0}) 隊伍: {}", obj.x, obj.z, obj.team);
            markers.push(DiffOverlayMarker {
                marker_type: DiffMarkerType::Added,
                world_x: obj.x,
                world_z: obj.z,
                target_world_x: obj.x,
                target_world_z: obj.z,
                color_argb: COLOR_ADDED,
                label,
                sub_label,
            });
        }

        for obj in &diff.objects.deleted {
            let label = format!("- {} [已刪除]", Self::format_object_title(obj));
            let sub_label = format!("原坐標: ({:.0}, {:.0})", obj.x, obj.z);
            markers.push(DiffOverlayMarker {
                marker_type: DiffMarkerType::Deleted,
                world_x: obj.x,
                world_z: obj.z,
                target_world_x: obj.x,
                target_world_z: obj.z,
                color_argb: COLOR_DELETED,
                label,
                sub_label,
            });
        }

        for m in &diff.objects.modified {
            if m.distance_moved > 0.5 && options.show_movement_vectors {
                let label = format!("-> {} [移動 {:.1}m]", Self::format_object_title(&m.current), m.distance_moved);
                let sub_label = m.changed_attributes.join("; ");
                markers.push(DiffOverlayMarker {
                    marker_type: DiffMarkerType::Moved,
                    world_x: m.baseline.x,
                    world_z: m.baseline.z,
                    target_world_x: m.current.x,
                    target_world_z: m.current.z,
                    color_argb: COLOR_MOVED,
                    label,
                    sub_label,
                });
            } else {
                let label = format!("* {} [修改]", Self::format_object_title(&m.current));
                let sub_label = m.changed_attributes.join("; ");
                markers.push(DiffOverlayMarker {
                    marker_type: DiffMarkerType::Modified,
                    world_x: m.current.x,
                    world_z: m.current.z,
                    target_world_x: m.current.x,
                    target_world_z: m.current.z,
                    color_argb: COLOR_MODIFIED,
                    label,
                    sub_label,
                });
            }
        }

        markers
    }

    fn format_object_title(obj: &DiffObjectItem) -> String {
        if !obj.alias.is_empty() {
            obj.alias.clone()
        } else {
            format!("Object #{}", obj.uid)
        }
    }
}
