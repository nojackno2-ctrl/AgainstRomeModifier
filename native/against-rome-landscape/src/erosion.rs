//! Faithful planning ports of the C# hydraulic and thermal erosion simulators.
//! Corresponds to `HydraulicErosionSimulator.cs`, `ThermalErosionSimulator.cs`,
//! `TerrainHeightWorkBuffer.cs`, and `TerrainSculptFilter.cs`.
//! Pure planning/simulation functions that do not write map files directly.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainSampleChange {
    pub index: usize,
    pub before: u8,
    pub after: u8,
}

impl TerrainSampleChange {
    pub fn new(index: usize, before: u8, after: u8) -> Self {
        Self {
            index,
            before,
            after,
        }
    }

    pub fn x(&self, size: usize) -> usize {
        assert!(size > 0);
        self.index % size
    }

    pub fn y(&self, size: usize) -> usize {
        assert!(size > 0);
        self.index / size
    }
}

/// Falloff curve mode for circular brush strokes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TerrainFalloffType {
    /// Smooth cubic polynomial `1 - t^2 * (3 - 2t)`.
    #[default]
    Smoothstep,
    /// Half-period cosine `0.5 * (1 + cos(pi * t))`.
    Cosine,
    /// Spherical hemisphere `sqrt(1 - t^2)`.
    Spherical,
    /// Gaussian peak `exp(-3.5 * t^2)`.
    Gaussian,
    /// Linear cone `1 - t`.
    Linear,
    /// Plateau falloff: flat 1.0 inside central 40%, smoothstep descent outside.
    Plateau,
}

/// Evaluates normalized falloff weight at distance `t` in `[0, 1]`.
pub fn calculate_falloff(t: f32, falloff_type: TerrainFalloffType) -> f32 {
    if t <= 0.0 {
        return 1.0;
    }
    if t >= 1.0 {
        return 0.0;
    }
    match falloff_type {
        TerrainFalloffType::Smoothstep => 1.0 - t * t * (3.0 - 2.0 * t),
        TerrainFalloffType::Cosine => 0.5 * (1.0 + (std::f32::consts::PI * t).cos()),
        TerrainFalloffType::Spherical => (1.0 - t * t).max(0.0).sqrt(),
        TerrainFalloffType::Gaussian => (-3.5 * t * t).exp(),
        TerrainFalloffType::Linear => 1.0 - t,
        TerrainFalloffType::Plateau => {
            if t < 0.4 {
                1.0
            } else {
                let v = (t - 0.4) / 0.6;
                1.0 - (v * v * (3.0 - 2.0 * v))
            }
        }
    }
}

/// Circular brush area on the height grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrushRegion {
    pub center_x: f32,
    pub center_y: f32,
    pub radius: f32,
}

impl BrushRegion {
    pub fn new(center_x: f32, center_y: f32, radius: f32) -> Self {
        Self {
            center_x,
            center_y,
            radius,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.center_x.is_finite()
            && self.center_y.is_finite()
            && self.radius.is_finite()
            && self.radius > 0.0
    }
}

/// High-precision 32-bit floating point terrain height work buffer.
/// Equivalent to C# `TerrainHeightWorkBuffer`.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainHeightWorkBuffer {
    size: usize,
    heights: Vec<f32>,
}

impl TerrainHeightWorkBuffer {
    pub fn new(size: usize) -> Result<Self, String> {
        if size < 2 {
            return Err("高度圖尺寸至少須為 2x2。".into());
        }
        let len = size
            .checked_mul(size)
            .ok_or_else(|| "高度圖尺寸超出記憶體上限。".to_string())?;
        Ok(Self {
            size,
            heights: vec![0.0; len],
        })
    }

    pub fn from_bytes(size: usize, initial_heights: &[u8]) -> Result<Self, String> {
        if size < 2 {
            return Err("高度圖尺寸至少須為 2x2。".into());
        }
        let expected_len = size
            .checked_mul(size)
            .ok_or_else(|| "高度圖尺寸超出記憶體上限。".to_string())?;
        if initial_heights.len() != expected_len {
            return Err(format!(
                "輸入的高度陣列長度 ({}) 與尺寸規格 ({size}x{size}={expected_len}) 不符。",
                initial_heights.len()
            ));
        }
        let heights = initial_heights.iter().map(|&b| b as f32).collect();
        Ok(Self { size, heights })
    }

    pub fn from_floats(size: usize, initial_heights: &[f32]) -> Result<Self, String> {
        if size < 2 {
            return Err("高度圖尺寸至少須為 2x2。".into());
        }
        let expected_len = size
            .checked_mul(size)
            .ok_or_else(|| "高度圖尺寸超出記憶體上限。".to_string())?;
        if initial_heights.len() != expected_len {
            return Err(format!(
                "輸入的高度陣列長度 ({}) 與尺寸規格 ({size}x{size}={expected_len}) 不符。",
                initial_heights.len()
            ));
        }
        Ok(Self {
            size,
            heights: initial_heights.to_vec(),
        })
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn raw_data(&self) -> &[f32] {
        &self.heights
    }

    pub fn raw_data_mut(&mut self) -> &mut [f32] {
        &mut self.heights
    }

    pub fn is_valid_coord(&self, x: i32, y: i32) -> bool {
        x >= 0 && (x as usize) < self.size && y >= 0 && (y as usize) < self.size
    }

    pub fn get(&self, x: i32, y: i32) -> f32 {
        assert!(self.is_valid_coord(x, y), "座標 ({x}, {y}) 超出緩衝區邊界");
        self.heights[y as usize * self.size + x as usize]
    }

    pub fn set(&mut self, x: i32, y: i32, value: f32) {
        assert!(self.is_valid_coord(x, y), "座標 ({x}, {y}) 超出緩衝區邊界");
        self.heights[y as usize * self.size + x as usize] = value;
    }

    pub fn get_clamped(&self, x: i32, y: i32) -> f32 {
        let max_idx = self.size as i32 - 1;
        let cx = x.clamp(0, max_idx) as usize;
        let cy = y.clamp(0, max_idx) as usize;
        self.heights[cy * self.size + cx]
    }

    /// Bilinear interpolation sampling at sub-grid coordinate `(x, y)`.
    pub fn sample_bilinear(&self, x: f32, y: f32) -> f32 {
        let max_coord = self.size as f32 - 1.0;
        let cx = x.clamp(0.0, max_coord);
        let cy = y.clamp(0.0, max_coord);

        let x0 = cx.floor() as usize;
        let y0 = cy.floor() as usize;
        let x1 = (x0 + 1).min(self.size - 1);
        let y1 = (y0 + 1).min(self.size - 1);

        let tx = cx - x0 as f32;
        let ty = cy - y0 as f32;

        let h00 = self.heights[y0 * self.size + x0];
        let h10 = self.heights[y0 * self.size + x1];
        let h01 = self.heights[y1 * self.size + x0];
        let h11 = self.heights[y1 * self.size + x1];

        let top = h00 + (h10 - h00) * tx;
        let bottom = h01 + (h11 - h01) * tx;
        top + (bottom - top) * ty
    }

    /// Calculates slope gradient vector `(gx, gy)` at coordinate `(x, y)`.
    pub fn calculate_gradient(&self, x: f32, y: f32) -> (f32, f32) {
        let max_coord = self.size as f32 - 1.0;
        let cx = x.clamp(0.0, max_coord);
        let cy = y.clamp(0.0, max_coord);

        let x0 = cx.floor() as usize;
        let y0 = cy.floor() as usize;
        let x1 = (x0 + 1).min(self.size - 1);
        let y1 = (y0 + 1).min(self.size - 1);

        let tx = cx - x0 as f32;
        let ty = cy - y0 as f32;

        let h00 = self.heights[y0 * self.size + x0];
        let h10 = self.heights[y0 * self.size + x1];
        let h01 = self.heights[y1 * self.size + x0];
        let h11 = self.heights[y1 * self.size + x1];

        let gx = (h10 - h00) * (1.0 - ty) + (h11 - h01) * ty;
        let gy = (h01 - h00) * (1.0 - tx) + (h11 - h10) * tx;
        (gx, gy)
    }

    /// Discrete Laplacian (curvature) operator at discrete coordinate `(x, y)`.
    pub fn calculate_laplacian(&self, x: i32, y: i32) -> f32 {
        let center = self.get_clamped(x, y);
        let left = self.get_clamped(x - 1, y);
        let right = self.get_clamped(x + 1, y);
        let top = self.get_clamped(x, y - 1);
        let bottom = self.get_clamped(x, y + 1);

        left + right + top + bottom - 4.0 * center
    }

    /// Quantizes float heights to `[0, 255]` with ties-to-even rounding and extracts modified vertices.
    pub fn extract_changes(&self, baseline_heights: &[u8]) -> Vec<TerrainSampleChange> {
        let length = self.heights.len().min(baseline_heights.len());
        let mut changes = Vec::new();
        for i in 0..length {
            let before = baseline_heights[i];
            let rounded = self.heights[i].round_ties_even();
            let after = (rounded as i32).clamp(0, 255) as u8;
            if before != after {
                changes.push(TerrainSampleChange {
                    index: i,
                    before,
                    after,
                });
            }
        }
        changes
    }
}

/// Knuth subtractive pseudo-random number generator, bit-exact compatible with .NET `System.Random(seed)`.
#[derive(Clone, Debug)]
pub struct DotNetRandom {
    seed_array: [i32; 56],
    inext: usize,
    inextp: usize,
}

impl DotNetRandom {
    pub fn new(seed: i32) -> Self {
        let mut seed_array = [0i32; 56];
        let subtraction = if seed == i32::MIN {
            i32::MAX
        } else {
            seed.abs()
        };
        let mut mj = 161803398i32.wrapping_sub(subtraction);
        seed_array[55] = mj;
        let mut mk = 1i32;
        for i in 1..55 {
            let ii = (21 * i) % 55;
            seed_array[ii] = mk;
            mk = mj.wrapping_sub(mk);
            if mk < 0 {
                mk = mk.wrapping_add(i32::MAX);
            }
            mj = seed_array[ii];
        }
        for _ in 1..5 {
            for i in 1..56 {
                let idx = 1 + (i + 30) % 55;
                seed_array[i] = seed_array[i].wrapping_sub(seed_array[idx]);
                if seed_array[i] < 0 {
                    seed_array[i] = seed_array[i].wrapping_add(i32::MAX);
                }
            }
        }
        Self {
            seed_array,
            inext: 0,
            inextp: 21,
        }
    }

    pub fn next_double(&mut self) -> f64 {
        self.internal_sample() as f64 * (1.0 / i32::MAX as f64)
    }

    fn internal_sample(&mut self) -> i32 {
        let mut loc_in_ext = self.inext + 1;
        let mut loc_in_extp = self.inextp + 1;
        if loc_in_ext >= 56 {
            loc_in_ext = 1;
        }
        if loc_in_extp >= 56 {
            loc_in_extp = 1;
        }
        let mut ret_val = self.seed_array[loc_in_ext].wrapping_sub(self.seed_array[loc_in_extp]);
        if ret_val == i32::MAX {
            ret_val -= 1;
        }
        if ret_val < 0 {
            ret_val = ret_val.wrapping_add(i32::MAX);
        }
        self.seed_array[loc_in_ext] = ret_val;
        self.inext = loc_in_ext;
        self.inextp = loc_in_extp;
        ret_val
    }
}

/// Parameters for thermal / talus slope erosion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThermalErosionParams {
    /// Angle of repose in degrees (typically 32°~42° for sand/talus, 50°~60° for rock). Clamped to `[5°, 85°]`.
    pub talus_angle_degrees: f32,
    /// Transfer relaxation factor `[0, 1]`.
    pub erosion_rate: f32,
    /// Number of iterative diffusion passes.
    pub iterations: usize,
    /// Grid spacing in world units (default 1.0).
    pub grid_spacing: f32,
}

impl Default for ThermalErosionParams {
    fn default() -> Self {
        Self {
            talus_angle_degrees: 38.0,
            erosion_rate: 0.40,
            iterations: 8,
            grid_spacing: 1.0,
        }
    }
}

const THERMAL_NEIGHBORS: [(i32, i32, f32); 8] = [
    (-1, 0, 1.0),
    (1, 0, 1.0),
    (0, -1, 1.0),
    (0, 1, 1.0),
    (-1, -1, 1.41421356),
    (1, -1, 1.41421356),
    (-1, 1, 1.41421356),
    (1, 1, 1.41421356),
];

/// Simulates thermal talus slope erosion on `buffer`.
///
/// Disperses overly steep cliffs into natural scree slopes according to the angle of repose.
/// Mass is strictly conserved across transfers within the height grid boundaries.
pub fn simulate_thermal_erosion(
    buffer: &mut TerrainHeightWorkBuffer,
    params: Option<&ThermalErosionParams>,
    brush: Option<BrushRegion>,
    mut scree_accumulation_map: Option<&mut [f32]>,
) {
    let default_params = ThermalErosionParams::default();
    let parameters = params.unwrap_or(&default_params);

    let size = buffer.size();
    if size < 3 || parameters.iterations == 0 {
        return;
    }

    let is_brush = brush.map(|b| b.is_valid()).unwrap_or(false);
    let (bcx, bcy, brad) = if let Some(b) = brush {
        (b.center_x, b.center_y, b.radius)
    } else {
        (0.0, 0.0, 0.0)
    };

    let mut min_x = 0;
    let mut max_x = size - 1;
    let mut min_y = 0;
    let mut max_y = size - 1;

    if is_brush {
        min_x = ((bcx - brad).floor() as i32).clamp(0, size as i32 - 1) as usize;
        max_x = ((bcx + brad).ceil() as i32).clamp(0, size as i32 - 1) as usize;
        min_y = ((bcy - brad).floor() as i32).clamp(0, size as i32 - 1) as usize;
        max_y = ((bcy + brad).ceil() as i32).clamp(0, size as i32 - 1) as usize;
    }

    let angle_rad =
        parameters.talus_angle_degrees.clamp(5.0, 85.0) * (std::f32::consts::PI / 180.0);
    let tan_angle = angle_rad.tan();
    let t_ortho = tan_angle * parameters.grid_spacing;
    let t_diag = tan_angle * (parameters.grid_spacing * 1.41421356);

    let mut delta_heights = vec![0.0f32; size * size];
    let mut excesses = [0.0f32; 8];

    for _ in 0..parameters.iterations {
        delta_heights.fill(0.0);

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let brush_weight = if is_brush {
                    let dist = ((x as f32 - bcx).powi(2) + (y as f32 - bcy).powi(2)).sqrt();
                    if dist > brad {
                        continue;
                    }
                    calculate_falloff(dist / brad, TerrainFalloffType::Smoothstep)
                } else {
                    1.0
                };

                let current_h = buffer.heights[y * size + x];
                let mut total_excess = 0.0f32;
                let mut min_neighbor_h = current_h;

                for (i, &(dx, dy, dist_ratio)) in THERMAL_NEIGHBORS.iter().enumerate() {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;

                    if nx < 0 || nx >= size as i32 || ny < 0 || ny >= size as i32 {
                        excesses[i] = 0.0;
                        continue;
                    }

                    let nbr_h = buffer.heights[ny as usize * size + nx as usize];
                    if nbr_h < min_neighbor_h {
                        min_neighbor_h = nbr_h;
                    }

                    let threshold = if dist_ratio > 1.2 { t_diag } else { t_ortho };
                    let drop = current_h - nbr_h;

                    if drop > threshold {
                        let excess = drop - threshold;
                        excesses[i] = excess;
                        total_excess += excess;
                    } else {
                        excesses[i] = 0.0;
                    }
                }

                if total_excess <= 1e-4 {
                    continue;
                }

                let max_allowed = (current_h - min_neighbor_h) * 0.5;
                let transfer_amount =
                    (total_excess * parameters.erosion_rate).min(max_allowed) * brush_weight;

                if transfer_amount <= 0.0 {
                    continue;
                }

                delta_heights[y * size + x] -= transfer_amount;

                let inv_total = 1.0 / total_excess;
                for (i, &(dx, dy, _)) in THERMAL_NEIGHBORS.iter().enumerate() {
                    if excesses[i] <= 0.0 {
                        continue;
                    }

                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    let n_index = ny as usize * size + nx as usize;

                    let received = transfer_amount * (excesses[i] * inv_total);
                    delta_heights[n_index] += received;

                    if let Some(ref mut scree) = scree_accumulation_map {
                        if n_index < scree.len() {
                            scree[n_index] += received;
                        }
                    }
                }
            }
        }

        for (i, &delta) in delta_heights.iter().enumerate() {
            if delta.abs() > 1e-5 {
                buffer.heights[i] = (buffer.heights[i] + delta).clamp(0.0, 255.0);
            }
        }
    }
}

/// Parameters for Lagrangian droplet hydraulic erosion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HydraulicErosionParams {
    /// Total droplet particles simulated (20,000~100,000 recommended for full map; 500~5,000 for local brush).
    pub droplet_count: usize,
    /// Maximum flow steps per droplet before termination.
    pub max_lifetime: usize,
    /// Droplet momentum inertia `[0, 1)`.
    pub inertia: f32,
    /// Gravitational acceleration constant.
    pub gravity: f32,
    /// Sediment carrying capacity factor Kc.
    pub capacity_factor: f32,
    /// Minimum slope clamp preventing zero capacity in flat areas.
    pub min_slope: f32,
    /// Soil dissolution / erosion rate Ke in `(0, 1]`.
    pub erosion_rate: f32,
    /// Sediment deposition rate Kd in `(0, 1]`.
    pub deposition_rate: f32,
    /// Droplet evaporation rate Kv in `(0, 1)`.
    pub evaporation_rate: f32,
    /// Erosion kernel radius in grid vertices.
    pub erosion_radius: usize,
    /// PRNG seed for deterministic simulation.
    pub random_seed: i32,
}

impl Default for HydraulicErosionParams {
    fn default() -> Self {
        Self {
            droplet_count: 30000,
            max_lifetime: 40,
            inertia: 0.12,
            gravity: 4.0,
            capacity_factor: 4.0,
            min_slope: 0.01,
            erosion_rate: 0.35,
            deposition_rate: 0.30,
            evaporation_rate: 0.02,
            erosion_radius: 2,
            random_seed: 42,
        }
    }
}

fn precompute_erosion_kernel(radius: usize) -> Vec<(i32, i32, f32)> {
    let radius_i = radius.max(1) as i32;
    let mut list = Vec::new();
    let mut sum = 0.0f32;

    for dy in -radius_i..=radius_i {
        for dx in -radius_i..=radius_i {
            let dist = ((dx * dx + dy * dy) as f32).sqrt();
            if dist <= radius_i as f32 {
                let w = (radius_i as f32 + 1.0 - dist).max(0.0);
                list.push((dx, dy, w));
                sum += w;
            }
        }
    }

    let inv_sum = if sum > 0.0 { 1.0 / sum } else { 1.0 };
    list.into_iter()
        .map(|(dx, dy, raw_weight)| (dx, dy, raw_weight * inv_sum))
        .collect()
}

fn deposit_bilinear(buffer: &mut TerrainHeightWorkBuffer, x: f32, y: f32, amount: f32) {
    if amount <= 0.0 {
        return;
    }
    let size = buffer.size();
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(size - 1);
    let y1 = (y0 + 1).min(size - 1);

    let tx = x - x0 as f32;
    let ty = y - y0 as f32;

    let w00 = (1.0 - tx) * (1.0 - ty);
    let w10 = tx * (1.0 - ty);
    let w01 = (1.0 - tx) * ty;
    let w11 = tx * ty;

    let raw = buffer.raw_data_mut();
    let i00 = y0 * size + x0;
    let i10 = y0 * size + x1;
    let i01 = y1 * size + x0;
    let i11 = y1 * size + x1;

    raw[i00] = (raw[i00] + amount * w00).clamp(0.0, 255.0);
    raw[i10] = (raw[i10] + amount * w10).clamp(0.0, 255.0);
    raw[i01] = (raw[i01] + amount * w01).clamp(0.0, 255.0);
    raw[i11] = (raw[i11] + amount * w11).clamp(0.0, 255.0);
}

fn erode_kernel(
    buffer: &mut TerrainHeightWorkBuffer,
    cx: i32,
    cy: i32,
    total_erosion: f32,
    kernel: &[(i32, i32, f32)],
) {
    if total_erosion <= 0.0 {
        return;
    }
    let size = buffer.size() as i32;
    let raw = buffer.raw_data_mut();
    let size_u = size as usize;

    for &(dx, dy, weight) in kernel {
        let nx = cx + dx;
        let ny = cy + dy;

        if nx >= 0 && nx < size && ny >= 0 && ny < size {
            let delta = total_erosion * weight;
            let idx = ny as usize * size_u + nx as usize;
            raw[idx] = (raw[idx] - delta).max(0.0);
        }
    }
}

/// Simulates droplet-based Lagrangian hydraulic erosion on `buffer`.
///
/// Models rainfall, channel rill erosion, downhill sediment transport,
/// and alluvial deposition in depressions and plains.
pub fn simulate_hydraulic_erosion(
    buffer: &mut TerrainHeightWorkBuffer,
    params: Option<&HydraulicErosionParams>,
    brush: Option<BrushRegion>,
) {
    let default_params = HydraulicErosionParams::default();
    let parameters = params.unwrap_or(&default_params);

    let size = buffer.size();
    if size < 3 || parameters.droplet_count == 0 || parameters.max_lifetime == 0 {
        return;
    }

    let is_brush = brush.map(|b| b.is_valid()).unwrap_or(false);
    let (bcx, bcy, brad) = if let Some(b) = brush {
        (b.center_x, b.center_y, b.radius)
    } else {
        (0.0, 0.0, 0.0)
    };

    let mut rng = DotNetRandom::new(parameters.random_seed);
    let erosion_offsets = precompute_erosion_kernel(parameters.erosion_radius);
    let size_f = size as f32;

    for _ in 0..parameters.droplet_count {
        let (mut px, mut py) = if is_brush {
            let angle = (rng.next_double() * std::f64::consts::PI * 2.0) as f32;
            let r = (rng.next_double() as f32).sqrt() * brad;
            let px = bcx + angle.cos() * r;
            let py = bcy + angle.sin() * r;
            if px < 1.0 || px >= size_f - 2.0 || py < 1.0 || py >= size_f - 2.0 {
                continue;
            }
            (px, py)
        } else {
            let px = 1.0 + (rng.next_double() as f32) * (size_f - 3.0);
            let py = 1.0 + (rng.next_double() as f32) * (size_f - 3.0);
            (px, py)
        };

        let mut dir_x = 0.0f32;
        let mut dir_y = 0.0f32;
        let mut speed = 1.0f32;
        let mut water = 1.0f32;
        let mut sediment = 0.0f32;

        for _ in 0..parameters.max_lifetime {
            let node_x = px.floor() as i32;
            let node_y = py.floor() as i32;

            let (gx, gy) = buffer.calculate_gradient(px, py);

            dir_x = dir_x * parameters.inertia - gx * (1.0 - parameters.inertia);
            dir_y = dir_y * parameters.inertia - gy * (1.0 - parameters.inertia);

            let mut dir_len = (dir_x * dir_x + dir_y * dir_y).sqrt();
            if dir_len < 1e-5 {
                dir_x = (rng.next_double() * 2.0 - 1.0) as f32;
                dir_y = (rng.next_double() * 2.0 - 1.0) as f32;
                dir_len = (dir_x * dir_x + dir_y * dir_y).sqrt();
                if dir_len < 1e-5 {
                    break;
                }
            }

            dir_x /= dir_len;
            dir_y /= dir_len;

            let next_px = px + dir_x;
            let next_py = py + dir_y;

            if next_px < 1.0 || next_px >= size_f - 2.0 || next_py < 1.0 || next_py >= size_f - 2.0
            {
                break;
            }

            let h_old = buffer.sample_bilinear(px, py);
            let h_new = buffer.sample_bilinear(next_px, next_py);
            let delta_h = h_new - h_old;

            let capacity =
                (-delta_h).max(parameters.min_slope) * speed * water * parameters.capacity_factor;

            if delta_h > 0.0 {
                let fill_amount = sediment.min(delta_h);
                deposit_bilinear(buffer, px, py, fill_amount);
                sediment -= fill_amount;

                if sediment <= 0.0 {
                    break;
                }
            } else if sediment > capacity {
                let deposit_amount = (sediment - capacity) * parameters.deposition_rate;
                deposit_bilinear(buffer, px, py, deposit_amount);
                sediment -= deposit_amount;
            } else {
                let erode_amount = ((capacity - sediment) * parameters.erosion_rate).min(-delta_h);
                erode_kernel(buffer, node_x, node_y, erode_amount, &erosion_offsets);
                sediment += erode_amount;
            }

            speed = (0.01f32.max(speed * speed + (-delta_h) * parameters.gravity)).sqrt();
            water *= 1.0 - parameters.evaporation_rate;

            if water < 0.005 {
                deposit_bilinear(buffer, px, py, sediment);
                break;
            }

            px = next_px;
            py = next_py;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_buffer_creation_and_bounds_validation() {
        assert!(TerrainHeightWorkBuffer::new(1).is_err());
        assert!(TerrainHeightWorkBuffer::new(2).is_ok());

        let buf = TerrainHeightWorkBuffer::new(4).unwrap();
        assert_eq!(buf.size(), 4);
        assert_eq!(buf.raw_data().len(), 16);

        let bytes = vec![50u8; 16];
        let from_b = TerrainHeightWorkBuffer::from_bytes(4, &bytes).unwrap();
        assert_eq!(from_b.get(0, 0), 50.0);

        assert!(TerrainHeightWorkBuffer::from_bytes(4, &bytes[..15]).is_err());
    }

    #[test]
    fn work_buffer_bilinear_and_gradient_parity() {
        let mut buf = TerrainHeightWorkBuffer::new(4).unwrap();
        for y in 0..4 {
            for x in 0..4 {
                buf.set(x, y, x as f32 * 10.0);
            }
        }

        let sample = buf.sample_bilinear(1.5, 1.5);
        assert!((sample - 15.0).abs() < 1e-4);

        let (gx, gy) = buf.calculate_gradient(1.5, 1.5);
        assert!((gx - 10.0).abs() < 1e-4);
        assert!(gy.abs() < 1e-4);
    }

    #[test]
    fn work_buffer_extract_changes_matches_csharp_ties_to_even() {
        let baseline = vec![0u8; 16];
        let mut buf = TerrainHeightWorkBuffer::from_bytes(4, &baseline).unwrap();

        buf.set(1, 1, 50.4); // Rounds to 50
        buf.set(2, 2, 0.2); // Rounds to 0, matching baseline

        let changes = buf.extract_changes(&baseline);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].index, 1 * 4 + 1);
        assert_eq!(changes[0].before, 0);
        assert_eq!(changes[0].after, 50);
    }

    #[test]
    fn dotnet_random_matches_system_random_stream() {
        let mut rng = DotNetRandom::new(42);
        let expected = [
            0.6681064659115423,
            0.14090729837348093,
            0.12551828945312568,
            0.5227642760252413,
            0.16843422416990353,
        ];
        for exp in expected {
            let actual = rng.next_double();
            assert!(
                (actual - exp).abs() < 1e-12,
                "RNG stream mismatch: actual={actual}, expected={exp}"
            );
        }
    }

    #[test]
    fn thermal_erosion_mass_conservation_in_closed_grid() {
        let size = 16;
        let mut buf = TerrainHeightWorkBuffer::new(size).unwrap();
        for y in 0..size as i32 {
            for x in 0..size as i32 {
                let h = if x <= 7 { 150.0 } else { 50.0 };
                buf.set(x, y, h);
            }
        }

        let mass_before: f32 = buf.raw_data().iter().sum();
        let mut scree = vec![0.0f32; size * size];

        let params = ThermalErosionParams {
            talus_angle_degrees: 35.0,
            erosion_rate: 0.40,
            iterations: 10,
            grid_spacing: 1.0,
        };

        simulate_thermal_erosion(&mut buf, Some(&params), None, Some(&mut scree));

        let mass_after: f32 = buf.raw_data().iter().sum();
        assert!(
            (mass_before - mass_after).abs() < 1e-2,
            "Mass before={mass_before}, after={mass_after}"
        );

        // Summit of cliff collapsed
        assert!(buf.get(7, 8) < 150.0);
        // Base of cliff accumulated scree
        assert!(buf.get(8, 8) > 50.0);
        assert!(scree[8 * size + 8] > 0.0);
    }

    #[test]
    fn hydraulic_erosion_deterministic_replay() {
        let size = 12;
        let mut buf1 = TerrainHeightWorkBuffer::new(size).unwrap();
        let mut buf2 = TerrainHeightWorkBuffer::new(size).unwrap();

        for y in 0..size as i32 {
            for x in 0..size as i32 {
                let dist = ((x as f32 - 5.5).powi(2) + (y as f32 - 5.5).powi(2)).sqrt();
                let h = (100.0 - dist * 12.0).max(10.0);
                buf1.set(x, y, h);
                buf2.set(x, y, h);
            }
        }

        let params = HydraulicErosionParams {
            droplet_count: 500,
            random_seed: 999,
            ..Default::default()
        };

        simulate_hydraulic_erosion(&mut buf1, Some(&params), None);
        simulate_hydraulic_erosion(&mut buf2, Some(&params), None);

        assert_eq!(
            buf1.raw_data(),
            buf2.raw_data(),
            "Simulation must be deterministic with same seed"
        );

        let mut buf3 = TerrainHeightWorkBuffer::new(size).unwrap();
        for y in 0..size as i32 {
            for x in 0..size as i32 {
                let dist = ((x as f32 - 5.5).powi(2) + (y as f32 - 5.5).powi(2)).sqrt();
                let h = (100.0 - dist * 12.0).max(10.0);
                buf3.set(x, y, h);
            }
        }
        let diff_params = HydraulicErosionParams {
            droplet_count: 500,
            random_seed: 111,
            ..Default::default()
        };
        simulate_hydraulic_erosion(&mut buf3, Some(&diff_params), None);
        assert_ne!(
            buf1.raw_data(),
            buf3.raw_data(),
            "Different seed must yield different results"
        );
    }

    #[test]
    fn hydraulic_erosion_preserves_outer_protection_ring() {
        let size = 16;
        let mut buf = TerrainHeightWorkBuffer::new(size).unwrap();
        for y in 0..size as i32 {
            for x in 0..size as i32 {
                buf.set(x, y, 100.0);
            }
        }
        let original_border: Vec<f32> = (0..size)
            .flat_map(|i| {
                [
                    buf.get(i as i32, 0),
                    buf.get(0, i as i32),
                    buf.get(i as i32, size as i32 - 1),
                    buf.get(size as i32 - 1, i as i32),
                ]
            })
            .collect();

        let params = HydraulicErosionParams {
            droplet_count: 1000,
            random_seed: 42,
            ..Default::default()
        };
        simulate_hydraulic_erosion(&mut buf, Some(&params), None);

        let after_border: Vec<f32> = (0..size)
            .flat_map(|i| {
                [
                    buf.get(i as i32, 0),
                    buf.get(0, i as i32),
                    buf.get(i as i32, size as i32 - 1),
                    buf.get(size as i32 - 1, i as i32),
                ]
            })
            .collect();
        assert_eq!(
            original_border, after_border,
            "Outer protection border must be unmodified"
        );
    }

    #[test]
    fn test_10_droplets() {
        let size = 8;
        let mut buf = TerrainHeightWorkBuffer::new(size).unwrap();
        for y in 0..size as i32 {
            for x in 0..size as i32 {
                let dist = ((x as f32 - 3.5).powi(2) + (y as f32 - 3.5).powi(2)).sqrt();
                let val = (100.0 - dist * 15.0).max(10.0);
                buf.set(x, y, val);
            }
        }
        let params = HydraulicErosionParams {
            droplet_count: 10,
            random_seed: 42,
            ..Default::default()
        };
        simulate_hydraulic_erosion(&mut buf, Some(&params), None);
        for v in buf.raw_data() {
            print!("{v},");
        }
        println!();
    }
}
