//! Faithful planning port of the C# Cliff* modules. No game files are modified.
use std::collections::{BTreeSet, HashMap, HashSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum CliffFacing {
    #[default]
    None = 0,
    North,
    South,
    East,
    West,
    NorthEastOuter,
    NorthWestOuter,
    SouthEastOuter,
    SouthWestOuter,
    NorthEastInner,
    NorthWestInner,
    SouthEastInner,
    SouthWestInner,
}
use CliffFacing::*;

#[derive(Clone, Debug, PartialEq)]
pub struct CliffCell {
    pub x: i32,
    pub y: i32,
    pub facing: CliffFacing,
    pub slope_degrees: f32,
    pub elevation_delta: f32,
    pub gradient_x: f32,
    pub gradient_y: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct CliffContourPath {
    pub id: usize,
    pub cells: Vec<CliffCell>,
    pub is_closed_loop: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct CliffDetectionResult {
    pub dimension: usize,
    pub cliff_cells: Vec<CliffCell>,
    pub contours: Vec<CliffContourPath>,
}
impl CliffDetectionResult {
    pub fn is_cliff(&self, x: i32, y: i32) -> bool {
        self.get_cell(x, y).is_some()
    }
    pub fn get_cell(&self, x: i32, y: i32) -> Option<&CliffCell> {
        self.cliff_cells.iter().find(|c| c.x == x && c.y == y)
    }
}
pub const DEFAULT_MIN_SLOPE_DEGREES: f32 = 45.0;
pub const DEFAULT_MIN_ELEVATION_DELTA: f32 = 15.0;
fn slope(delta: f32) -> f32 {
    (delta / 16.0).atan() * (180.0 / std::f32::consts::PI)
}
// .NET Math.Max propagates NaN; Rust f32::max deliberately ignores one NaN.
fn maximum(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else {
        a.max(b)
    }
}

pub fn classify_facing(n: f32, s: f32, e: f32, w: f32, threshold: f32) -> CliffFacing {
    let t = threshold * 0.5;
    let d = [
        maximum(0.0, n),
        maximum(0.0, s),
        maximum(0.0, e),
        maximum(0.0, w),
    ];
    let u = [
        maximum(0.0, -n),
        maximum(0.0, -s),
        maximum(0.0, -e),
        maximum(0.0, -w),
    ];
    for (v, faces) in [
        (
            &d,
            [
                NorthEastOuter,
                NorthWestOuter,
                SouthEastOuter,
                SouthWestOuter,
            ],
        ),
        (
            &u,
            [
                NorthEastInner,
                NorthWestInner,
                SouthEastInner,
                SouthWestInner,
            ],
        ),
    ] {
        for ((a, b), f) in [(0, 2), (0, 3), (1, 2), (1, 3)].into_iter().zip(faces) {
            if v[a] >= t && v[b] >= t {
                return f;
            }
        }
    }
    for (v, faces) in [
        (&d, [North, South, East, West]),
        (&u, [South, North, West, East]),
    ] {
        let m = v.iter().copied().fold(0.0_f32, maximum);
        if m > 0.0 {
            return faces[v.iter().position(|v| *v == m).unwrap()];
        }
    }
    None
}

fn chain_contours(cells: &[CliffCell]) -> Vec<CliffContourPath> {
    let mut remaining: HashMap<_, _> = cells.iter().map(|c| ((c.x, c.y), c)).collect();
    let mut paths = Vec::new();
    // Dictionary removal in C# preserves the surviving entries' insertion order.
    for start in cells {
        if remaining.remove(&(start.x, start.y)).is_none() {
            continue;
        }
        let mut path = vec![start.clone()];
        let mut current = start;
        loop {
            let next = [
                (0, -1),
                (1, 0),
                (0, 1),
                (-1, 0),
                (1, -1),
                (1, 1),
                (-1, 1),
                (-1, -1),
            ]
            .into_iter()
            .find_map(|(dx, dy)| remaining.remove(&(current.x + dx, current.y + dy)));
            match next {
                Some(c) => {
                    path.push(c.clone());
                    current = c;
                }
                Option::None => break,
            }
        }
        let last = path.last().unwrap();
        let closed =
            path.len() > 2 && (start.x - last.x).abs() <= 1 && (start.y - last.y).abs() <= 1;
        paths.push(CliffContourPath {
            id: paths.len() + 1,
            cells: path,
            is_closed_loop: closed,
        });
    }
    paths
}

pub fn detect_from_tile_heights(
    dimension: usize,
    heights: &[f32],
    min_slope: f32,
    min_delta: f32,
) -> Result<CliffDetectionResult, String> {
    if dimension == 0
        || dimension.checked_mul(dimension) != Some(heights.len())
        || dimension > i32::MAX as usize
    {
        return Err("圖塊高度維度與數量不符。".into());
    }
    let mut cells = Vec::new();
    for y in 0..dimension {
        for x in 0..dimension {
            let h = heights[y * dimension + x];
            let neighbor = |dx: i32, dy: i32| {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx < 0 || ny < 0 || nx >= dimension as i32 || ny >= dimension as i32 {
                    h
                } else {
                    heights[ny as usize * dimension + nx as usize]
                }
            };
            let (n, s, e, w) = (
                neighbor(0, -1),
                neighbor(0, 1),
                neighbor(1, 0),
                neighbor(-1, 0),
            );
            let delta = [
                n,
                s,
                e,
                w,
                neighbor(1, -1),
                neighbor(-1, -1),
                neighbor(1, 1),
                neighbor(-1, 1),
            ]
            .into_iter()
            .map(|v| (h - v).abs())
            .fold(0.0_f32, maximum);
            let angle = slope(delta);
            if angle >= min_slope || delta >= min_delta {
                cells.push(CliffCell {
                    x: x as i32,
                    y: y as i32,
                    facing: classify_facing(h - n, h - s, h - e, h - w, min_delta),
                    slope_degrees: angle,
                    elevation_delta: delta,
                    gradient_x: (e - w) / 2.0,
                    gradient_y: (s - n) / 2.0,
                });
            }
        }
    }
    let contours = chain_contours(&cells);
    Ok(CliffDetectionResult {
        dimension,
        cliff_cells: cells,
        contours,
    })
}

pub fn detect_from_vertex_heights(
    vertex_size: usize,
    heights: &[u8],
    tile_dimension: usize,
    heightmap_step: usize,
    min_slope: f32,
    min_delta: f32,
) -> Result<CliffDetectionResult, String> {
    if vertex_size <= 1
        || vertex_size.checked_mul(vertex_size) != Some(heights.len())
        || tile_dimension == 0
        || heightmap_step == 0
        || tile_dimension.checked_mul(tile_dimension).is_none()
        || (tile_dimension - 1)
            .checked_mul(heightmap_step)
            .is_none_or(|v| v >= vertex_size)
        || tile_dimension.checked_mul(heightmap_step).is_none()
    {
        return Err("頂點高度資料長度或圖塊取樣維度無效。".into());
    }
    let mut averages = vec![0.0; tile_dimension * tile_dimension];
    let mut deltas = averages.clone();
    for ty in 0..tile_dimension {
        for tx in 0..tile_dimension {
            let (mut sum, mut count, mut min, mut max) = (0.0_f32, 0, 255u8, 0u8);
            for vy in ty * heightmap_step..=((ty + 1) * heightmap_step).min(vertex_size - 1) {
                for vx in tx * heightmap_step..=((tx + 1) * heightmap_step).min(vertex_size - 1) {
                    let v = heights[vy * vertex_size + vx];
                    sum += v as f32;
                    count += 1;
                    min = min.min(v);
                    max = max.max(v);
                }
            }
            let i = ty * tile_dimension + tx;
            averages[i] = sum / count as f32;
            deltas[i] = (max - min) as f32;
        }
    }
    let mut result = detect_from_tile_heights(tile_dimension, &averages, min_slope, min_delta)?;
    for y in 0..tile_dimension {
        for x in 0..tile_dimension {
            if result.is_cliff(x as i32, y as i32) {
                continue;
            }
            let delta = deltas[y * tile_dimension + x];
            let angle = slope(delta);
            if delta >= min_delta || angle >= min_slope {
                let cur = averages[y * tile_dimension + x];
                let n = if y > 0 {
                    cur - averages[(y - 1) * tile_dimension + x]
                } else {
                    0.0
                };
                let s = if y + 1 < tile_dimension {
                    cur - averages[(y + 1) * tile_dimension + x]
                } else {
                    0.0
                };
                let e = if x + 1 < tile_dimension {
                    cur - averages[y * tile_dimension + x + 1]
                } else {
                    0.0
                };
                let w = if x > 0 {
                    cur - averages[y * tile_dimension + x - 1]
                } else {
                    0.0
                };
                result.cliff_cells.push(CliffCell {
                    x: x as i32,
                    y: y as i32,
                    facing: classify_facing(n, s, e, w, min_delta),
                    slope_degrees: angle,
                    elevation_delta: delta,
                    gradient_x: 0.0,
                    gradient_y: 0.0,
                });
            }
        }
    }
    result.cliff_cells.sort_by_key(|c| (c.y, c.x));
    result.contours = chain_contours(&result.cliff_cells);
    Ok(result)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliffTileEntry {
    pub texture: String,
    pub facing: CliffFacing,
    pub family: String,
    pub weight: i32,
}
impl CliffTileEntry {
    pub fn new(texture: impl Into<String>, facing: CliffFacing, family: impl Into<String>) -> Self {
        Self {
            texture: texture.into(),
            facing,
            family: family.into(),
            weight: 1,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliffFamilyInfo {
    pub family_id: String,
    pub display_name: String,
    pub talus_material_id: String,
    pub crest_material_id: String,
}
#[derive(Clone, Debug, Default)]
pub struct CliffTileCatalog {
    pub entries: Vec<CliffTileEntry>,
    pub families: Vec<CliffFamilyInfo>,
    pub requires_exact_facing: bool,
}
impl CliffTileCatalog {
    pub fn register(&mut self, entry: CliffTileEntry) {
        self.entries.push(entry);
    }
    pub fn register_family(&mut self, family: CliffFamilyInfo) {
        if let Some(f) = self
            .families
            .iter_mut()
            .find(|f| f.family_id.eq_ignore_ascii_case(&family.family_id))
        {
            *f = family;
        } else {
            self.families.push(family);
        }
    }
    pub fn filtered_by(&self, exists: impl Fn(&str) -> bool) -> Self {
        Self {
            entries: self
                .entries
                .iter()
                .filter(|e| exists(&e.texture))
                .cloned()
                .collect(),
            ..self.clone()
        }
    }
    pub fn is_cliff_texture(&self, texture: &str) -> bool {
        !texture.trim().is_empty()
            && self
                .entries
                .iter()
                .any(|e| e.texture.eq_ignore_ascii_case(texture))
    }
    pub fn get_entries(&self, facing: CliffFacing, family: Option<&str>) -> Vec<&CliffTileEntry> {
        let family = family.filter(|f| !f.trim().is_empty());
        self.entries
            .iter()
            .filter(|e| {
                (facing == None || e.facing == facing)
                    && family.is_none_or(|f| e.family.eq_ignore_ascii_case(f))
            })
            .collect()
    }
    pub fn pick_tile(
        &self,
        facing: CliffFacing,
        x: i32,
        y: i32,
        seed: i32,
        preferred_family: Option<&str>,
    ) -> Option<&str> {
        if self.entries.is_empty() || (self.requires_exact_facing && facing == None) {
            return Option::None;
        }
        let mut candidates = self.get_entries(facing, preferred_family);
        if candidates.is_empty() && facing != None {
            candidates = self.get_entries(facing, Option::None);
        }
        if !self.requires_exact_facing {
            if candidates.is_empty() && facing != None {
                let fallback = match facing {
                    NorthEastOuter | NorthWestOuter => North,
                    SouthEastOuter | SouthWestOuter => South,
                    NorthEastInner | SouthEastInner => East,
                    NorthWestInner | SouthWestInner => West,
                    _ => None,
                };
                if fallback != None {
                    candidates = self.get_entries(fallback, preferred_family);
                    if candidates.is_empty() {
                        candidates = self.get_entries(fallback, Option::None);
                    }
                }
            }
            if candidates.is_empty() && preferred_family.is_some_and(|f| !f.trim().is_empty()) {
                candidates = self.get_entries(None, preferred_family);
            }
            if candidates.is_empty() {
                candidates = self.entries.iter().collect();
            }
        }
        if candidates.is_empty() {
            return Option::None;
        }
        let mut hash = (x as u32)
            .wrapping_mul(374761393)
            .wrapping_add((y as u32).wrapping_mul(668265263))
            .wrapping_add((seed as u32).wrapping_mul(1442695041));
        hash = (hash ^ (hash >> 13)).wrapping_mul(1274126177);
        hash ^= hash >> 16;
        let total: u64 = candidates.iter().map(|e| e.weight.max(1) as u64).sum();
        let roll = hash as u64 % total;
        let mut acc = 0u64;
        for e in &candidates {
            acc += e.weight.max(1) as u64;
            if roll < acc {
                return Some(&e.texture);
            }
        }
        Some(&candidates[0].texture)
    }
    pub fn build_real_names(existing: Option<&[String]>) -> Self {
        let mut c = Self {
            requires_exact_facing: true,
            ..Self::default()
        };
        c.register_family(CliffFamilyInfo {
            family_id: "FELS".into(),
            display_name: "Grass / rock cliffs".into(),
            talus_material_id: "BK".into(),
            crest_material_id: "BB".into(),
        });
        let mut seen = HashSet::new();
        for name in existing.unwrap_or(&[]) {
            if !seen.insert(name) {
                continue;
            }
            let f = match name.to_ascii_uppercase().as_str() {
                "FELS_AA_008" => North,
                "FELS_AA_004" => East,
                "FELS_AA_002" => South,
                "FELS_AA_006" => West,
                _ => continue,
            };
            c.register(CliffTileEntry::new(name.clone(), f, "FELS"));
        }
        c
    }
    fn base_families() -> Self {
        let mut c = Self::default();
        for (id, label, talus, crest) in [
            ("FELS", "標準岩壁 (Temperate Cliffs)", "BK", "B8"),
            ("ITA_FELS", "義大利岩壁 (Italian Cliffs)", "B8", "B4"),
            ("Berg", "高山岩壁 (Alpine Cliffs)", "BK", "B3"),
            ("STEIN", "石質陡坡 (Stony Slopes)", "BG", "B2"),
        ] {
            c.register_family(CliffFamilyInfo {
                family_id: id.into(),
                display_name: label.into(),
                talus_material_id: talus.into(),
                crest_material_id: crest.into(),
            });
        }
        c
    }
    /// Guessed legacy names, for compatibility; use build_real_names for game assets.
    pub fn create_default() -> Self {
        let mut c = Self::base_families();
        for family in ["FELS", "ITA_FELS", "Berg", "STEIN"] {
            for (suffix, facing) in [
                ("_N1", North),
                ("_N2", North),
                ("_S1", South),
                ("_S2", South),
                ("_E1", East),
                ("_E2", East),
                ("_W1", West),
                ("_W2", West),
                ("_NE_OUT", NorthEastOuter),
                ("_NW_OUT", NorthWestOuter),
                ("_SE_OUT", SouthEastOuter),
                ("_SW_OUT", SouthWestOuter),
                ("_NE_IN", NorthEastInner),
                ("_NW_IN", NorthWestInner),
                ("_SE_IN", SouthEastInner),
                ("_SW_IN", SouthWestInner),
                ("1", None),
                ("2", None),
            ] {
                c.register(CliffTileEntry::new(
                    format!("{family}{suffix}"),
                    facing,
                    family,
                ));
            }
        }
        c
    }
    pub fn build_available(existing: Option<&[String]>, _preferred_family: Option<&str>) -> Self {
        let Some(existing) = existing else {
            return Self::create_default();
        };
        let mut c = Self::base_families();
        let mut seen = HashSet::new();
        for name in existing {
            let upper = name.to_ascii_uppercase();
            if !seen.insert(upper.clone()) {
                continue;
            }
            let family = if upper.starts_with("ITA_FELS") {
                "ITA_FELS"
            } else if upper.starts_with("FELS") {
                "FELS"
            } else if upper.starts_with("BERG") {
                "Berg"
            } else if upper.starts_with("STEIN") {
                "STEIN"
            } else {
                continue;
            };
            c.register(CliffTileEntry::new(
                name.clone(),
                infer_facing_from_name(&upper),
                family,
            ));
        }
        if c.entries.is_empty() {
            Self::create_default()
        } else {
            c
        }
    }
}
pub fn infer_facing_from_name(upper: &str) -> CliffFacing {
    for (patterns, face) in [
        (&["NE_IN", "NO_IN"][..], NorthEastInner),
        (&["NW_IN"][..], NorthWestInner),
        (&["SE_IN", "SO_IN"][..], SouthEastInner),
        (&["SW_IN"][..], SouthWestInner),
        (&["NE_OUT", "NO_OUT", "_NE", "_NO"][..], NorthEastOuter),
        (&["NW_OUT", "_NW"][..], NorthWestOuter),
        (&["SE_OUT", "SO_OUT", "_SE", "_SO"][..], SouthEastOuter),
        (&["SW_OUT", "_SW"][..], SouthWestOuter),
        (&["_N"][..], North),
        (&["_S"][..], South),
        (&["_E", "_O"][..], East),
        (&["_W"][..], West),
    ] {
        if patterns.iter().any(|p| upper.contains(p)) {
            return face;
        }
    }
    None
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliffTilePlacement {
    pub x: i32,
    pub y: i32,
    pub texture: String,
    pub facing: CliffFacing,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreePlacement {
    pub x: i32,
    pub y: i32,
    pub material_id: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CliffPlanResult {
    pub succeeded: bool,
    pub cliff_tiles: Vec<CliffTilePlacement>,
    pub scree_placements: Vec<ScreePlacement>,
    pub blocked_collision_pixels: Vec<(i32, i32)>,
    pub issues: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct CliffPlannerOptions {
    pub preferred_family: Option<String>,
    pub generate_scree: bool,
    pub scree_material_id: String,
    pub scree_radius: i32,
    pub generate_crest_transition: bool,
    pub crest_material_id: String,
    pub auto_mark_collision: bool,
    pub random_seed: i32,
}
impl Default for CliffPlannerOptions {
    fn default() -> Self {
        Self {
            preferred_family: Some("FELS".into()),
            generate_scree: true,
            scree_material_id: "BK".into(),
            scree_radius: 1,
            generate_crest_transition: false,
            crest_material_id: "B8".into(),
            auto_mark_collision: true,
            random_seed: 42,
        }
    }
}
fn offsets(face: CliffFacing, uphill: bool) -> &'static [(i32, i32)] {
    match (face, uphill) {
        (North, false) | (South, true) => &[(0, -1)],
        (South, false) | (North, true) => &[(0, 1)],
        (East, false) | (West, true) => &[(1, 0)],
        (West, false) | (East, true) => &[(-1, 0)],
        (NorthEastOuter, false) => &[(0, -1), (1, 0), (1, -1)],
        (NorthWestOuter, false) => &[(0, -1), (-1, 0), (-1, -1)],
        (SouthEastOuter, false) => &[(0, 1), (1, 0), (1, 1)],
        (SouthWestOuter, false) => &[(0, 1), (-1, 0), (-1, 1)],
        (NorthEastOuter, true) => &[(-1, 1)],
        (NorthWestOuter, true) => &[(1, 1)],
        (SouthEastOuter, true) => &[(-1, -1)],
        (SouthWestOuter, true) => &[(1, -1)],
        (NorthEastInner, false) | (SouthWestInner, true) => &[(0, 1), (-1, 0)],
        (NorthWestInner, false) | (SouthEastInner, true) => &[(0, 1), (1, 0)],
        (SouthEastInner, false) | (NorthWestInner, true) => &[(0, -1), (-1, 0)],
        (SouthWestInner, false) | (NorthEastInner, true) => &[(0, -1), (1, 0)],
        _ => &[],
    }
}
pub fn plan_cliffs(
    dimension: usize,
    detection: &CliffDetectionResult,
    catalog: &CliffTileCatalog,
    options: &CliffPlannerOptions,
) -> Result<CliffPlanResult, String> {
    if dimension == 0 || dimension > (i32::MAX / 4) as usize {
        return Err("圖塊維度無效。".into());
    }
    let mut plan = CliffPlanResult::default();
    for c in &detection.cliff_cells {
        match catalog
            .pick_tile(
                c.facing,
                c.x,
                c.y,
                options.random_seed,
                options.preferred_family.as_deref(),
            )
            .filter(|s| !s.trim().is_empty())
        {
            Some(texture) => plan.cliff_tiles.push(CliffTilePlacement {
                x: c.x,
                y: c.y,
                texture: texture.into(),
                facing: c.facing,
            }),
            Option::None => plan.issues.push(format!(
                "無法為座標 ({}, {}) 朝向 {:?} 找到合適的岩壁圖塊。",
                c.x, c.y, c.facing
            )),
        }
    }
    let placed: HashSet<_> = plan.cliff_tiles.iter().map(|p| (p.x, p.y)).collect();
    let effect: Vec<_> = detection
        .cliff_cells
        .iter()
        .filter(|c| !catalog.requires_exact_facing || placed.contains(&(c.x, c.y)))
        .collect();
    for (enabled, material, radius, uphill) in [
        (
            options.generate_scree,
            &options.scree_material_id,
            options.scree_radius,
            false,
        ),
        (
            options.generate_crest_transition,
            &options.crest_material_id,
            1,
            true,
        ),
    ] {
        if !enabled || material.trim().is_empty() {
            continue;
        }
        let mut tiles = BTreeSet::new();
        for c in &effect {
            for &(dx, dy) in offsets(c.facing, uphill) {
                for r in 1..=radius {
                    let (x, y) = (
                        i64::from(c.x) + i64::from(dx) * i64::from(r),
                        i64::from(c.y) + i64::from(dy) * i64::from(r),
                    );
                    if x >= 0
                        && y >= 0
                        && x < dimension as i64
                        && y < dimension as i64
                        && !detection.is_cliff(x as i32, y as i32)
                    {
                        tiles.insert((y as i32, x as i32));
                    }
                }
            }
        }
        plan.scree_placements
            .extend(tiles.into_iter().map(|(y, x)| ScreePlacement {
                x,
                y,
                material_id: material.clone(),
            }));
    }
    if options.auto_mark_collision {
        for c in effect {
            for py in 0..4 {
                for px in 0..4 {
                    let x =
                        c.x.checked_mul(4)
                            .and_then(|v| v.checked_add(px))
                            .ok_or("碰撞座標溢位。")?;
                    let y =
                        c.y.checked_mul(4)
                            .and_then(|v| v.checked_add(py))
                            .ok_or("碰撞座標溢位。")?;
                    plan.blocked_collision_pixels.push((x, y));
                }
            }
        }
    }
    plan.succeeded =
        !plan.cliff_tiles.is_empty() && (plan.issues.is_empty() || catalog.requires_exact_facing);
    Ok(plan)
}

/// Adapters retain the original sessions' stamp/paint and stroke semantics.
pub trait CliffBlendSession {
    fn stamp_texture(&mut self, x: i32, y: i32, texture: &str) -> bool;
    fn paint_tiles(&mut self, tiles: &[(i32, i32)], material: &str, auto_bridge: bool) -> bool;
    fn commit_stroke(&mut self);
}
pub trait CliffHeightSession {
    fn block_collision(&mut self, x: i32, y: i32, radius: f32);
    fn commit_stroke(&mut self);
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CliffApplyResult {
    pub succeeded: bool,
    pub stamped_cliff_tiles: usize,
    pub painted_scree_tiles: usize,
    pub blocked_collision_pixels: usize,
    pub errors: Vec<String>,
}
pub fn apply_cliff_plan(
    blend: &mut dyn CliffBlendSession,
    mut height: Option<&mut dyn CliffHeightSession>,
    plan: &CliffPlanResult,
) -> CliffApplyResult {
    if !plan.succeeded {
        return CliffApplyResult {
            errors: plan.issues.clone(),
            ..CliffApplyResult::default()
        };
    }
    let mut result = CliffApplyResult::default();
    for p in &plan.cliff_tiles {
        if blend.stamp_texture(p.x, p.y, &p.texture) {
            result.stamped_cliff_tiles += 1;
        }
    }
    let mut groups: Vec<(&str, Vec<(i32, i32)>)> = Vec::new();
    for p in &plan.scree_placements {
        if let Some((_, tiles)) = groups.iter_mut().find(|(m, _)| *m == p.material_id) {
            tiles.push((p.x, p.y));
        } else {
            groups.push((&p.material_id, vec![(p.x, p.y)]));
        }
    }
    for (material, tiles) in groups {
        if blend.paint_tiles(&tiles, material, true) {
            result.painted_scree_tiles += tiles.len();
        } else {
            result.errors.push(format!(
                "坡腳材質 {material} 自動過渡烘焙失敗，缺少原版邊界圖塊。"
            ));
        }
    }
    if let Some(h) = height.as_mut() {
        if !plan.blocked_collision_pixels.is_empty() {
            for &(x, y) in &plan.blocked_collision_pixels {
                h.block_collision(x, y, 1.0);
                result.blocked_collision_pixels += 1;
            }
            h.commit_stroke();
        }
    }
    blend.commit_stroke();
    result.succeeded =
        result.stamped_cliff_tiles + result.painted_scree_tiles + result.blocked_collision_pixels
            > 0;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn detect(d: usize, heights: &[f32]) -> CliffDetectionResult {
        detect_from_tile_heights(d, heights, 45.0, 15.0).unwrap()
    }
    fn cell(x: i32, y: i32, facing: CliffFacing) -> CliffCell {
        CliffCell {
            x,
            y,
            facing,
            slope_degrees: 80.0,
            elevation_delta: 100.0,
            gradient_x: 0.0,
            gradient_y: 0.0,
        }
    }
    #[test]
    fn flat_and_invalid_dimensions() {
        assert!(detect(8, &[50.0; 64]).cliff_cells.is_empty());
        assert!(detect_from_tile_heights(0, &[], 45.0, 15.0).is_err());
        assert!(detect_from_tile_heights(8, &[0.0; 63], 45.0, 15.0).is_err());
        assert!(detect_from_vertex_heights(2, &[0; 4], usize::MAX, 4, 45.0, 15.0).is_err());
    }
    #[test]
    fn nan_neighbors_follow_dotnet_max_propagation() {
        let mut heights = [0.0; 9];
        heights[0] = f32::NAN;
        heights[4] = 100.0;
        let result = detect(3, &heights);
        assert!(!result.is_cliff(1, 1));
        assert_eq!(classify_facing(f32::NAN, 20.0, 0.0, 0.0, 15.0), None);
    }
    #[test]
    fn ridge_corners_and_contours() {
        let heights: Vec<_> = (0..64)
            .map(|i| if i / 8 <= 3 { 100.0 } else { 20.0 })
            .collect();
        let d = detect(8, &heights);
        assert_eq!(d.cliff_cells.len(), 16);
        assert!(d.cliff_cells.iter().all(|c| c.facing == South));
        assert_eq!(d.contours.len(), 1);
        assert_eq!(d.contours[0].cells.len(), 16);
        for (low, expected) in [(false, NorthEastOuter), (true, NorthEastInner)] {
            let h: Vec<_> = (0..64)
                .map(|i| {
                    if (i % 8 <= 3 && i / 8 >= 4) ^ low {
                        100.0
                    } else {
                        20.0
                    }
                })
                .collect();
            assert_eq!(detect(8, &h).get_cell(3, 4).unwrap().facing, expected);
        }
    }
    #[test]
    fn vertex_internal_slope_and_clipped_sampling() {
        let mut h = vec![0; 81];
        for y in 0..=4 {
            for x in 0..=4 {
                h[y * 9 + x] = (x * 10) as u8;
            }
        }
        assert!(detect_from_vertex_heights(9, &h, 2, 4, 45.0, 15.0)
            .unwrap()
            .is_cliff(0, 0));
        assert!(detect_from_vertex_heights(8, &[1; 64], 2, 4, 45.0, 15.0)
            .unwrap()
            .cliff_cells
            .is_empty());
    }
    #[test]
    fn classification_tie_precedence_and_uphill() {
        assert_eq!(classify_facing(20.0, 20.0, 0.0, 0.0, 15.0), North);
        assert_eq!(classify_facing(-20.0, 0.0, 0.0, 0.0, 15.0), South);
        assert_eq!(
            classify_facing(10.0, 10.0, 10.0, 10.0, 15.0),
            NorthEastOuter
        );
    }
    #[test]
    fn catalog_defaults_fallback_and_inference() {
        let c = CliffTileCatalog::create_default();
        assert_eq!(c.entries.len(), 72);
        assert_eq!(c.families.len(), 4);
        assert!(c
            .pick_tile(North, 10, 20, 1, Some("fels"))
            .unwrap()
            .starts_with("FELS_N"));
        assert!(c
            .pick_tile(South, 10, 20, 1, Some("ITA_FELS"))
            .unwrap()
            .starts_with("ITA_FELS_S"));
        let mut custom = CliffTileCatalog::default();
        custom.register(CliffTileEntry::new("Custom_N", North, "Custom"));
        assert_eq!(
            custom.pick_tile(NorthEastOuter, 5, 5, 0, Some("Custom")),
            Some("Custom_N")
        );
        let names =
            ["FELS_N1", "FELS_SO_OUT", "ITA_FELS_NW_IN", "Berg1", "Other"].map(String::from);
        let available = CliffTileCatalog::build_available(Some(&names), Option::None);
        assert_eq!(available.entries.len(), 4);
        assert_eq!(available.entries[1].facing, SouthEastOuter);
        assert_eq!(available.entries[2].facing, NorthWestInner);
        assert!(c.filtered_by(|_| false).entries.is_empty());
    }
    #[test]
    fn weighted_wrapping_hash_is_stable_for_negative_coordinates() {
        let c = CliffTileCatalog::create_default();
        for (x, y, seed, expected) in [
            (12, 34, 42, "FELS_N2"),
            (-1, -2, -3, "FELS_N2"),
            (i32::MAX, i32::MIN, 42, "FELS_N2"),
        ] {
            assert_eq!(c.pick_tile(North, x, y, seed, Some("FELS")), Some(expected));
        }
        let variants: HashSet<_> = (0..20)
            .filter_map(|x| c.pick_tile(North, x, 0, 42, Some("FELS")))
            .collect();
        assert_eq!(variants.len(), 2);
    }
    #[test]
    fn real_catalog_rejects_guesses_and_keeps_archive_case() {
        let names = [
            "Fels_AA_008",
            "fels_aa_004",
            "Fels_AA_002",
            "Fels_AA_006",
            "FELS_N1",
            "fels1",
        ]
        .map(String::from);
        let c = CliffTileCatalog::build_real_names(Some(&names));
        assert_eq!(c.entries.len(), 4);
        assert_eq!(
            c.pick_tile(East, 1, 1, 0, Option::None),
            Some("fels_aa_004")
        );
        for f in [None, NorthEastOuter, NorthWestInner] {
            assert!(c.pick_tile(f, 1, 1, 0, Option::None).is_none());
        }
        let c = c.filtered_by(|n| n == "Fels_AA_008");
        assert!(c.pick_tile(South, 0, 0, 0, Option::None).is_none());
    }
    #[test]
    fn real_plan_limits_effects_to_supported_faces() {
        let d = CliffDetectionResult {
            dimension: 16,
            cliff_cells: vec![cell(3, 3, North), cell(12, 12, NorthEastOuter)],
            contours: vec![],
        };
        let c = CliffTileCatalog::build_real_names(Some(&["Fels_AA_008".into()]));
        let p = plan_cliffs(16, &d, &c, &CliffPlannerOptions::default()).unwrap();
        assert!(p.succeeded);
        assert_eq!(p.issues.len(), 1);
        assert_eq!(p.cliff_tiles.len(), 1);
        assert_eq!(p.blocked_collision_pixels.len(), 16);
        assert_eq!(
            p.scree_placements,
            vec![ScreePlacement {
                x: 3,
                y: 2,
                material_id: "BK".into()
            }]
        );
        assert!(p
            .blocked_collision_pixels
            .iter()
            .all(|&(x, y)| (12..16).contains(&x) && (12..16).contains(&y)));
    }
    #[test]
    fn scree_crest_radius_and_collision_order() {
        let d = CliffDetectionResult {
            dimension: 8,
            cliff_cells: vec![cell(3, 3, South)],
            contours: vec![],
        };
        let c = CliffTileCatalog::create_default();
        let o = CliffPlannerOptions {
            generate_crest_transition: true,
            scree_radius: 2,
            ..CliffPlannerOptions::default()
        };
        let p = plan_cliffs(8, &d, &c, &o).unwrap();
        assert_eq!(
            p.scree_placements
                .iter()
                .map(|s| (s.x, s.y, s.material_id.as_str()))
                .collect::<Vec<_>>(),
            vec![(3, 4, "BK"), (3, 5, "BK"), (3, 2, "B8")]
        );
        assert_eq!(p.blocked_collision_pixels[0], (12, 12));
        assert_eq!(p.blocked_collision_pixels[15], (15, 15));
        let p = plan_cliffs(
            8,
            &d,
            &c,
            &CliffPlannerOptions {
                generate_scree: false,
                auto_mark_collision: false,
                ..CliffPlannerOptions::default()
            },
        )
        .unwrap();
        assert!(p.scree_placements.is_empty());
        assert!(p.blocked_collision_pixels.is_empty());
    }
    #[test]
    fn apply_failure_and_partial_paint_preserve_session_semantics() {
        #[derive(Default)]
        struct Blend {
            commits: usize,
            stamps: usize,
            paints: usize,
        }
        impl CliffBlendSession for Blend {
            fn stamp_texture(&mut self, _: i32, _: i32, _: &str) -> bool {
                self.stamps += 1;
                true
            }
            fn paint_tiles(&mut self, _: &[(i32, i32)], _: &str, auto: bool) -> bool {
                assert!(auto);
                self.paints += 1;
                false
            }
            fn commit_stroke(&mut self) {
                self.commits += 1;
            }
        }
        #[derive(Default)]
        struct Height {
            blocks: usize,
            commits: usize,
        }
        impl CliffHeightSession for Height {
            fn block_collision(&mut self, _: i32, _: i32, r: f32) {
                assert_eq!(r, 1.0);
                self.blocks += 1;
            }
            fn commit_stroke(&mut self) {
                self.commits += 1;
            }
        }
        let mut b = Blend::default();
        let mut h = Height::default();
        assert!(!apply_cliff_plan(&mut b, Some(&mut h), &CliffPlanResult::default()).succeeded);
        assert_eq!(b.commits, 0);
        let d = CliffDetectionResult {
            dimension: 8,
            cliff_cells: vec![cell(3, 3, South)],
            contours: vec![],
        };
        let p = plan_cliffs(
            8,
            &d,
            &CliffTileCatalog::create_default(),
            &CliffPlannerOptions::default(),
        )
        .unwrap();
        let r = apply_cliff_plan(&mut b, Some(&mut h), &p);
        assert!(r.succeeded);
        assert_eq!(r.errors.len(), 1);
        assert_eq!(r.painted_scree_tiles, 0);
        assert_eq!(
            (b.stamps, b.paints, b.commits, h.blocks, h.commits),
            (1, 1, 1, 16, 1)
        );
    }
}
