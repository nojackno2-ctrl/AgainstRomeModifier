use std::f32::consts::PI;

#[derive(Clone, Debug, PartialEq)]
pub struct MapThumbnailSpawnPoint {
    pub world_x: f32,
    pub world_z: f32,
    pub team: i32,
    pub label: String,
    pub is_player: bool,
}

impl MapThumbnailSpawnPoint {
    pub fn new(world_x: f32, world_z: f32, team: i32, label: impl Into<String>, is_player: bool) -> Self {
        Self {
            world_x,
            world_z,
            team,
            label: label.into(),
            is_player,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThumbnailRenderOptions {
    pub width: usize,
    pub height: usize,
    pub sun_azimuth_deg: f32,
    pub sun_altitude_deg: f32,
    pub relief_strength: f32,
    pub show_water: bool,
    pub show_shorelines: bool,
    pub show_spawn_banners: bool,
    pub show_roads: bool,
    pub water_level: f32,
    pub height_step: f32,
}

impl Default for ThumbnailRenderOptions {
    fn default() -> Self {
        Self {
            width: 512,
            height: 512,
            sun_azimuth_deg: 315.0,
            sun_altitude_deg: 45.0,
            relief_strength: 1.35,
            show_water: true,
            show_shorelines: true,
            show_spawn_banners: true,
            show_roads: true,
            water_level: 0.0,
            height_step: 4.0,
        }
    }
}

pub struct MinimapThumbnailRenderer;

const TEAM_PALETTE: &[(u8, u8, u8)] = &[
    (30, 110, 235), // 0: Player / Blue
    (225, 45, 45),  // 1: Red
    (35, 175, 60),  // 2: Green
    (240, 195, 25), // 3: Gold
    (165, 55, 215), // 4: Violet
    (25, 205, 215), // 5: Cyan
    (240, 125, 25), // 6: Orange
    (215, 220, 230),// 7: Silver
];

impl MinimapThumbnailRenderer {
    pub fn render_to_bmp(
        height_grid: Option<&[u8]>,
        height_size: usize,
        options: &ThumbnailRenderOptions,
        _collision_grid: Option<&[u8]>,
        _collision_size: usize,
        spawn_points: Option<&[MapThumbnailSpawnPoint]>,
        road_waypoints: Option<&[(f32, f32)]>,
    ) -> Vec<u8> {
        let width = options.width;
        let height = options.height;
        let rgba = Self::render_to_rgba(
            height_grid,
            height_size,
            options,
            _collision_grid,
            _collision_size,
            spawn_points,
            road_waypoints,
        );

        let stride = (width * 3 + 3) & !3;
        let image_size = stride * height;
        let file_size = 54 + image_size;
        let mut bmp = vec![0u8; file_size];

        // BITMAPFILEHEADER
        bmp[0] = b'B';
        bmp[1] = b'M';
        bmp[2..6].copy_from_slice(&(file_size as u32).to_le_bytes());
        bmp[10..14].copy_from_slice(&54u32.to_le_bytes());

        // BITMAPINFOHEADER
        bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
        bmp[18..22].copy_from_slice(&(width as i32).to_le_bytes());
        bmp[22..26].copy_from_slice(&(height as i32).to_le_bytes());
        bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
        bmp[28..30].copy_from_slice(&24u16.to_le_bytes());
        bmp[34..38].copy_from_slice(&(image_size as u32).to_le_bytes());

        // Pixels: bottom-up, BGR
        for y in 0..height {
            let bmp_row_offset = 54 + (height - 1 - y) * stride;
            let rgba_row_offset = y * width * 4;

            for x in 0..width {
                let src = rgba_row_offset + x * 4;
                let dst = bmp_row_offset + x * 3;

                bmp[dst] = rgba[src + 2];     // B
                bmp[dst + 1] = rgba[src + 1]; // G
                bmp[dst + 2] = rgba[src];     // R
            }
        }

        bmp
    }

    pub fn render_to_rgba(
        height_grid: Option<&[u8]>,
        height_size: usize,
        options: &ThumbnailRenderOptions,
        _collision_grid: Option<&[u8]>,
        _collision_size: usize,
        spawn_points: Option<&[MapThumbnailSpawnPoint]>,
        road_waypoints: Option<&[(f32, f32)]>,
    ) -> Vec<u8> {
        let width = options.width;
        let height = options.height;
        let mut pixels = vec![0u8; width * height * 4];

        let sun_az_rad = options.sun_azimuth_deg * PI / 180.0;
        let sun_alt_rad = options.sun_altitude_deg * PI / 180.0;

        let lx = sun_az_rad.sin() * sun_alt_rad.cos();
        let ly = -sun_az_rad.cos() * sun_alt_rad.cos();
        let lz = sun_alt_rad.sin();

        let has_heights = height_grid.map_or(false, |g| height_size > 1 && g.len() >= height_size * height_size);
        let water_world = options.water_level;
        let height_step = options.height_step;

        for py in 0..height {
            let norm_y = (py as f32 + 0.5) / height as f32;
            for px in 0..width {
                let norm_x = (px as f32 + 0.5) / width as f32;
                let pixel_index = (py * width + px) * 4;

                let mut elevation = 0.0f32;
                let mut shade_factor = 1.0f32;
                let mut is_submerged = false;
                let mut water_depth = 0.0f32;

                if has_heights {
                    let grid = height_grid.unwrap();
                    let gx = norm_x * (height_size - 1) as f32;
                    let gy = norm_y * (height_size - 1) as f32;

                    let x0 = (gx as usize).min(height_size - 1);
                    let y0 = (gy as usize).min(height_size - 1);
                    let x1 = (x0 + 1).min(height_size - 1);
                    let y1 = (y0 + 1).min(height_size - 1);

                    let fx = gx - x0 as f32;
                    let fy = gy - y0 as f32;

                    let h00 = grid[y0 * height_size + x0] as f32;
                    let h10 = grid[y0 * height_size + x1] as f32;
                    let h01 = grid[y1 * height_size + x0] as f32;
                    let h11 = grid[y1 * height_size + x1] as f32;

                    let raw_height = (1.0 - fx) * (1.0 - fy) * h00
                        + fx * (1.0 - fy) * h10
                        + (1.0 - fx) * fy * h01
                        + fx * fy * h11;
                    elevation = raw_height * height_step;

                    let xm = x0.saturating_sub(1);
                    let xp = (x1 + 1).min(height_size - 1);
                    let ym = y0.saturating_sub(1);
                    let yp = (y1 + 1).min(height_size - 1);

                    let dx_span = (xp - xm).max(1) as f32;
                    let dy_span = (yp - ym).max(1) as f32;

                    let dzdx = ((grid[y0 * height_size + xp] as f32 - grid[y0 * height_size + xm] as f32) * height_step) / dx_span;
                    let dzdy = ((grid[yp * height_size + x0] as f32 - grid[ym * height_size + x0] as f32) * height_step) / dy_span;

                    let mut nx = -dzdx * options.relief_strength * 0.1;
                    let mut ny = -dzdy * options.relief_strength * 0.1;
                    let mut nz = 1.0f32;
                    let inv_len = 1.0 / (nx * nx + ny * ny + nz * nz).sqrt();
                    nx *= inv_len;
                    ny *= inv_len;
                    nz *= inv_len;

                    let dot = nx * lx + ny * ly + nz * lz;
                    shade_factor = (0.35 + 0.65 * dot.max(0.0)).clamp(0.25, 1.75);

                    if options.show_water && elevation < water_world {
                        is_submerged = true;
                        water_depth = water_world - elevation;
                    }
                }

                let (r, g, b) = if is_submerged {
                    if options.show_shorelines && water_depth <= 3.0 {
                        let t = water_depth / 3.0;
                        (
                            ((1.0 - t) * 220.0 + t * 65.0).clamp(0.0, 255.0) as u8,
                            ((1.0 - t) * 205.0 + t * 155.0).clamp(0.0, 255.0) as u8,
                            ((1.0 - t) * 150.0 + t * 195.0).clamp(0.0, 255.0) as u8,
                        )
                    } else {
                        let depth_ratio = (water_depth / 50.0).clamp(0.0, 1.0);
                        (
                            ((1.0 - depth_ratio) * 45.0 + depth_ratio * 15.0).clamp(0.0, 255.0) as u8,
                            ((1.0 - depth_ratio) * 140.0 + depth_ratio * 45.0).clamp(0.0, 255.0) as u8,
                            ((1.0 - depth_ratio) * 185.0 + depth_ratio * 95.0).clamp(0.0, 255.0) as u8,
                        )
                    }
                } else {
                    let (base_r, base_g, base_b) = if elevation > 550.0 {
                        (220.0, 225.0, 230.0)
                    } else if elevation > 350.0 {
                        (130.0, 125.0, 120.0)
                    } else if elevation > 180.0 {
                        (95.0, 135.0, 70.0)
                    } else {
                        (65.0, 118.0, 52.0)
                    };
                    (
                        (base_r * shade_factor).clamp(0.0, 255.0) as u8,
                        (base_g * shade_factor).clamp(0.0, 255.0) as u8,
                        (base_b * shade_factor).clamp(0.0, 255.0) as u8,
                    )
                };

                pixels[pixel_index] = r;
                pixels[pixel_index + 1] = g;
                pixels[pixel_index + 2] = b;
                pixels[pixel_index + 3] = 255;
            }
        }

        if options.show_roads {
            if let Some(waypoints) = road_waypoints {
                if waypoints.len() > 1 {
                    Self::render_road_network(&mut pixels, width, height, waypoints);
                }
            }
        }

        if options.show_spawn_banners {
            if let Some(spawns) = spawn_points {
                if !spawns.is_empty() {
                    Self::render_spawn_banners(&mut pixels, width, height, spawns);
                }
            }
        }

        pixels
    }

    fn render_road_network(pixels: &mut [u8], width: usize, height: usize, waypoints: &[(f32, f32)]) {
        for i in 0..waypoints.len() - 1 {
            let p1 = waypoints[i];
            let p2 = waypoints[i + 1];

            let x0 = (p1.0 / 16384.0 * width as f32) as i32;
            let y0 = (p1.1 / 16384.0 * height as f32) as i32;
            let x1 = (p2.0 / 16384.0 * width as f32) as i32;
            let y1 = (p2.1 / 16384.0 * height as f32) as i32;

            Self::draw_thick_line(pixels, width, height, x0, y0, x1, y1, 3, (180, 168, 142));
        }
    }

    fn draw_thick_line(
        pixels: &mut [u8],
        width: usize,
        height: usize,
        mut x0: i32,
        mut y0: i32,
        x1: i32,
        y1: i32,
        thickness: i32,
        color: (u8, u8, u8),
    ) {
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        let half = thickness / 2;

        loop {
            for oy in -half..=half {
                for ox in -half..=half {
                    let px = x0 + ox;
                    let py = y0 + oy;
                    if px >= 0 && (px as usize) < width && py >= 0 && (py as usize) < height {
                        let idx = (py as usize * width + px as usize) * 4;
                        pixels[idx] = color.0;
                        pixels[idx + 1] = color.1;
                        pixels[idx + 2] = color.2;
                    }
                }
            }

            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    fn render_spawn_banners(pixels: &mut [u8], width: usize, height: usize, spawns: &[MapThumbnailSpawnPoint]) {
        let banner_radius = (width / 36).clamp(6, 16) as i32;

        for spawn in spawns {
            let cx = (spawn.world_x / 16384.0 * width as f32) as i32;
            let cy = (spawn.world_z / 16384.0 * height as f32) as i32;

            if cx < banner_radius || cx >= (width as i32) - banner_radius || cy < banner_radius || cy >= (height as i32) - banner_radius {
                continue;
            }

            let team_idx = (spawn.team as usize).min(TEAM_PALETTE.len() - 1);
            let (tr, tg, tb) = TEAM_PALETTE[team_idx];

            let r_outer_sq = (banner_radius as f32 + 1.5) * (banner_radius as f32 + 1.5);
            let r_inner_sq = (banner_radius as f32 - 1.5) * (banner_radius as f32 - 1.5);
            let r_center_sq = (banner_radius as f32 - 4.0) * (banner_radius as f32 - 4.0);
            let r_shadow_sq = (banner_radius as f32 + 3.5) * (banner_radius as f32 + 3.5);

            for dy in -banner_radius - 2..=banner_radius + 2 {
                for dx in -banner_radius - 2..=banner_radius + 2 {
                    let px = cx + dx;
                    let py = cy + dy;
                    if px < 0 || (px as usize) >= width || py < 0 || (py as usize) >= height {
                        continue;
                    }

                    let dist_sq = (dx * dx + dy * dy) as f32;
                    let idx = (py as usize * width + px as usize) * 4;

                    if dist_sq <= r_center_sq {
                        if dx.abs() <= 1 || dy.abs() <= 1 {
                            pixels[idx] = 255;
                            pixels[idx + 1] = 255;
                            pixels[idx + 2] = 255;
                        } else {
                            pixels[idx] = tr;
                            pixels[idx + 1] = tg;
                            pixels[idx + 2] = tb;
                        }
                    } else if dist_sq <= r_inner_sq {
                        pixels[idx] = tr;
                        pixels[idx + 1] = tg;
                        pixels[idx + 2] = tb;
                    } else if dist_sq <= r_outer_sq {
                        pixels[idx] = 20;
                        pixels[idx + 1] = 20;
                        pixels[idx + 2] = 20;
                    } else if dist_sq <= r_shadow_sq && dx >= 0 && dy >= 0 {
                        pixels[idx] = (pixels[idx] as f32 * 0.55) as u8;
                        pixels[idx + 1] = (pixels[idx + 1] as f32 * 0.55) as u8;
                        pixels[idx + 2] = (pixels[idx + 2] as f32 * 0.55) as u8;
                    }
                }
            }
        }
    }
}
