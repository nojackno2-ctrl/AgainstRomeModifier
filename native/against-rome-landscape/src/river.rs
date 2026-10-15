//! Pure river catalogue, stroke topology and height-field planning.
use crate::{orthogonal_between, Point, NEIGHBORS};
use std::collections::{HashMap, HashSet};

pub type RiverConnections = u8;
pub type RiverFlowDirections = u8;
pub type RiverBankSide = u8;
pub const NORTH: u8 = 1;
pub const EAST: u8 = 2;
pub const SOUTH: u8 = 4;
pub const WEST: u8 = 8;
pub const ALL: u8 = 15;
pub const INFLOW_NORTH: u8 = 1;
pub const INFLOW_EAST: u8 = 2;
pub const INFLOW_SOUTH: u8 = 4;
pub const INFLOW_WEST: u8 = 8;
pub const OUTFLOW_NORTH: u8 = 16;
pub const OUTFLOW_EAST: u8 = 32;
pub const OUTFLOW_SOUTH: u8 = 64;
pub const OUTFLOW_WEST: u8 = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RiverTileKind {
    Source,
    Straight,
    Turn,
    TConfluence,
    Cross,
    Estuary,
    LakeWater,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RiverElevationMode {
    ValidateOnly,
    AutoCarveDescending,
    AutoCarveTrench,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RiverTile {
    pub texture: String,
    pub connections: RiverConnections,
    pub kind: RiverTileKind,
    pub flow: RiverFlowDirections,
    pub variant: i32,
}
impl RiverTile {
    pub fn new(texture: impl Into<String>, connections: u8, kind: RiverTileKind) -> Self {
        Self {
            texture: texture.into(),
            connections,
            kind,
            flow: 0,
            variant: 1,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RiverTilePlacement {
    pub x: i32,
    pub y: i32,
    pub texture: String,
    pub connections: u8,
    pub kind: RiverTileKind,
    pub flow: u8,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RiverBankPlacement {
    pub x: i32,
    pub y: i32,
    pub texture: String,
    pub side: u8,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RiverHeightAdjustment {
    pub index: usize,
    pub vertex_x: i32,
    pub vertex_y: i32,
    pub before: u8,
    pub after: u8,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RiverElevationWarning {
    pub x: i32,
    pub y: i32,
    pub current_elevation: i32,
    pub target_elevation: i32,
    pub message: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct RiverPlannerOptions {
    pub elevation_mode: RiverElevationMode,
    pub auto_detect_downhill_flow: bool,
    pub generate_bank_transitions: bool,
    pub trench_depth: i32,
    pub min_slope_drop: i32,
    pub preferred_river_texture: Option<String>,
    pub preferred_bank_texture: Option<String>,
    pub water_level: Option<f32>,
    pub heightmap_step: f32,
    pub water_bed_depth: i32,
    pub bank_slope_vertices: i32,
}
impl Default for RiverPlannerOptions {
    fn default() -> Self {
        Self {
            elevation_mode: RiverElevationMode::AutoCarveDescending,
            auto_detect_downhill_flow: false,
            generate_bank_transitions: true,
            trench_depth: 2,
            min_slope_drop: 1,
            preferred_river_texture: None,
            preferred_bank_texture: None,
            water_level: None,
            heightmap_step: 4.0,
            water_bed_depth: 16,
            bank_slope_vertices: 3,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RiverStrokePlan {
    pub succeeded: bool,
    pub water_tiles: Vec<RiverTilePlacement>,
    pub bank_tiles: Vec<RiverBankPlacement>,
    pub height_adjustments: Vec<RiverHeightAdjustment>,
    pub warnings: Vec<RiverElevationWarning>,
    pub unsupported: Vec<Point>,
}
impl RiverStrokePlan {
    fn empty(succeeded: bool, unsupported: Vec<Point>) -> Self {
        Self {
            succeeded,
            unsupported,
            water_tiles: vec![],
            bank_tiles: vec![],
            height_adjustments: vec![],
            warnings: vec![],
        }
    }
}

#[derive(Clone, Debug)]
pub struct RiverTileCatalog {
    pub available_river_tiles: Vec<RiverTile>,
    /// Insertion order is significant for the fallback to the first available bank.
    pub available_bank_tiles: Vec<(RiverBankSide, Vec<String>)>,
    pub preferred_river_texture: Option<String>,
    pub preferred_bank_texture: Option<String>,
}
impl RiverTileCatalog {
    pub fn new(
        river_tiles: Vec<RiverTile>,
        bank_tiles: Vec<(u8, Vec<String>)>,
        preferred_river: Option<String>,
        preferred_bank: Option<String>,
    ) -> Self {
        Self {
            available_river_tiles: river_tiles,
            available_bank_tiles: bank_tiles,
            preferred_river_texture: preferred_river,
            preferred_bank_texture: preferred_bank,
        }
    }
    pub fn build_available(
        names: Option<&[String]>,
        preferred_river: Option<&str>,
        preferred_bank: Option<&str>,
    ) -> Self {
        let available = |name: &str| {
            names.is_none_or(|names| names.iter().any(|n| n.eq_ignore_ascii_case(name)))
        };
        let fluss = ["FLUSS1", "FLUSS2", "FLUSS3", "FLUSS4", "FLUSS5"];
        let base = preferred_river
            .filter(|s| available(s))
            .or_else(|| fluss.iter().copied().find(|s| available(s)))
            .unwrap_or("FLUSS1");
        let candidates = |native: &[&str]| -> Vec<String> {
            let mut result: Vec<_> = native
                .iter()
                .filter(|s| available(s))
                .map(|s| s.to_string())
                .collect();
            if result.is_empty() {
                result.push(base.to_owned());
            }
            result
        };
        let h = candidates(&["FLUSS1", "FLUSS2"]);
        let v = candidates(&["FLUSS1", "FLUSS3"]);
        let ne = candidates(&["FLUSS1", "FLUSS5"]);
        let se = candidates(&["FLUSS1", "FLUSS4"]);
        let j = candidates(&["FLUSS1", "FLUSS4", "FLUSS5"]);
        let mut tiles = vec![];
        for t in &h {
            tiles.push(RiverTile::new(t, EAST | WEST, RiverTileKind::Straight));
        }
        for t in &v {
            tiles.push(RiverTile::new(t, NORTH | SOUTH, RiverTileKind::Straight));
        }
        for t in &ne {
            for c in [NORTH | EAST, NORTH | WEST] {
                tiles.push(RiverTile::new(t, c, RiverTileKind::Turn));
            }
        }
        for t in &se {
            for c in [SOUTH | EAST, SOUTH | WEST] {
                tiles.push(RiverTile::new(t, c, RiverTileKind::Turn));
            }
        }
        for c in [
            NORTH | EAST | SOUTH,
            NORTH | EAST | WEST,
            NORTH | SOUTH | WEST,
            EAST | SOUTH | WEST,
        ] {
            for t in &j {
                tiles.push(RiverTile::new(t, c, RiverTileKind::TConfluence));
            }
        }
        for t in &j {
            tiles.push(RiverTile::new(t, ALL, RiverTileKind::Cross));
        }
        for c in [NORTH, EAST, SOUTH, WEST] {
            let texture = if c == EAST || c == WEST { &h[0] } else { &v[0] };
            tiles.push(RiverTile::new(texture, c, RiverTileKind::Source));
            tiles.push(RiverTile::new(texture, c, RiverTileKind::Estuary));
        }
        tiles.push(RiverTile::new(base, 0, RiverTileKind::LakeWater));
        let mut banks = vec![];
        for (side, name) in [
            (NORTH, "ErdeFlussR1"),
            (EAST, "ErdeFlussR2"),
            (SOUTH, "ErdeFlussR3"),
            (WEST, "ErdeFlussR4"),
        ] {
            if available(name) {
                banks.push((side, vec![name.to_owned()]));
            } else if let Some(p) = preferred_bank.filter(|s| !s.is_empty() && available(s)) {
                banks.push((side, vec![p.to_owned()]));
            }
        }
        Self::new(
            tiles,
            banks,
            Some(base.to_owned()),
            preferred_bank.map(str::to_owned),
        )
    }
    pub fn find_water_tile(
        &self,
        connections: u8,
        _kind: RiverTileKind,
        preferred: Option<&str>,
        seed: i32,
    ) -> Option<&RiverTile> {
        let pref = preferred
            .or(self.preferred_river_texture.as_deref())
            .unwrap_or("");
        let list: Vec<_> = self
            .available_river_tiles
            .iter()
            .filter(|t| t.connections == connections)
            .collect();
        let index = |len| seed.unsigned_abs() as usize % len;
        if !list.is_empty() {
            if !pref.is_empty() {
                if let Some(t) = list.iter().find(|t| t.texture.eq_ignore_ascii_case(pref)) {
                    return Some(t);
                }
            }
            return Some(list[index(list.len())]);
        }
        if matches!(connections, NORTH | EAST | SOUTH | WEST) {
            let c = if connections == EAST || connections == WEST {
                EAST | WEST
            } else {
                NORTH | SOUTH
            };
            let fallback: Vec<_> = self
                .available_river_tiles
                .iter()
                .filter(|t| t.connections == c)
                .collect();
            if !fallback.is_empty() {
                return Some(fallback[index(fallback.len())]);
            }
        }
        if self.available_river_tiles.is_empty() {
            None
        } else {
            Some(&self.available_river_tiles[index(self.available_river_tiles.len())])
        }
    }
    pub fn find_bank_tile(&self, side: u8, preferred: Option<&str>, seed: i32) -> Option<&str> {
        let pref = preferred
            .or(self.preferred_bank_texture.as_deref())
            .unwrap_or("");
        if let Some((_, list)) = self.available_bank_tiles.iter().find(|(s, _)| *s == side) {
            if !list.is_empty() {
                if !pref.is_empty() {
                    if let Some(t) = list.iter().find(|t| t.eq_ignore_ascii_case(pref)) {
                        return Some(t);
                    }
                }
                return Some(&list[seed.unsigned_abs() as usize % list.len()]);
            }
        }
        if let Some(p) = self
            .preferred_bank_texture
            .as_deref()
            .filter(|s| !s.is_empty())
        {
            return Some(p);
        }
        self.available_bank_tiles
            .iter()
            .find_map(|(_, list)| list.first().map(String::as_str))
    }
    pub fn is_water_texture(&self, texture: &str) -> bool {
        if texture.trim().is_empty() {
            return false;
        }
        if self
            .available_river_tiles
            .iter()
            .any(|t| t.texture.eq_ignore_ascii_case(texture))
        {
            return true;
        }
        let upper = texture.to_ascii_uppercase();
        upper.starts_with("FLUSS")
            || upper.starts_with("SEE")
            || upper.starts_with("WASSER")
            || (upper.len() > 2 && upper.starts_with("4S"))
    }
    pub fn is_bank_texture(&self, texture: &str) -> bool {
        !texture.trim().is_empty()
            && (self
                .available_bank_tiles
                .iter()
                .any(|(_, list)| list.iter().any(|t| t.eq_ignore_ascii_case(texture)))
                || texture.to_ascii_uppercase().starts_with("ERDEFLUSSR"))
    }
}

pub fn rasterize_orthogonal_path(path: &[Point]) -> Vec<Point> {
    let Some(&first) = path.first() else {
        return vec![];
    };
    let mut result = vec![first];
    for &next in &path[1..] {
        let prev = *result.last().unwrap();
        if prev == next {
            continue;
        }
        for step in orthogonal_between(prev, next) {
            if result.last() == Some(&step) {
                continue;
            }
            if result.len() >= 2 && result[result.len() - 2] == step {
                result.pop();
            } else {
                result.push(step);
            }
        }
    }
    result
}
fn classify(conn: u8, index: usize) -> RiverTileKind {
    match conn.count_ones() {
        0 => RiverTileKind::LakeWater,
        1 => {
            if index == 0 {
                RiverTileKind::Source
            } else {
                RiverTileKind::Estuary
            }
        }
        2 => {
            if conn == NORTH | SOUTH || conn == EAST | WEST {
                RiverTileKind::Straight
            } else {
                RiverTileKind::Turn
            }
        }
        3 => RiverTileKind::TConfluence,
        _ => RiverTileKind::Cross,
    }
}
pub fn plan(
    dimension: i32,
    textures: &[String],
    heights: Option<&[u8]>,
    vertex_size: i32,
    path: &[Point],
    catalog: &RiverTileCatalog,
    options: &RiverPlannerOptions,
) -> Result<RiverStrokePlan, String> {
    if dimension <= 0 || textures.len() as i64 != dimension as i64 * dimension as i64 {
        return Err("地圖圖塊尺寸與紋理陣列長度不符。".into());
    }
    if let Some(h) = heights {
        if vertex_size < 2 || h.len() as i64 != vertex_size as i64 * vertex_size as i64 {
            return Err("Height field dimensions do not match.".into());
        }
    }
    if let Some(water) = options.water_level {
        if options.elevation_mode != RiverElevationMode::ValidateOnly
            && (!water.is_finite()
                || !options.heightmap_step.is_finite()
                || options.heightmap_step <= 0.0
                || water / options.heightmap_step < 6.0)
        {
            return Ok(RiverStrokePlan::empty(false, path.to_vec()));
        }
    }
    if path.is_empty() {
        return Ok(RiverStrokePlan::empty(true, vec![]));
    }
    let unsupported: Vec<_> = path
        .iter()
        .copied()
        .filter(|&(x, y)| x < 0 || y < 0 || x >= dimension || y >= dimension)
        .collect();
    if !unsupported.is_empty() {
        return Ok(RiverStrokePlan::empty(false, unsupported));
    }
    let mut continuous = rasterize_orthogonal_path(path);
    if options.auto_detect_downhill_flow && continuous.len() > 1 {
        if let Some(h) = heights {
            if sample(dimension, h, vertex_size, continuous[0])
                < sample(dimension, h, vertex_size, *continuous.last().unwrap())
            {
                continuous.reverse();
            }
        }
    }
    let mut result = RiverStrokePlan::empty(true, vec![]);
    if let Some(h) = heights {
        height_profile(dimension, h, vertex_size, &continuous, options, &mut result);
    }
    let mut links: HashMap<Point, u8> = HashMap::new();
    let mut flows: HashMap<Point, u8> = HashMap::new();
    links.insert(continuous[0], 0);
    for pair in continuous.windows(2) {
        let (prev, curr) = (pair[0], pair[1]);
        if let Some(&(_, _, out, back)) = NEIGHBORS
            .iter()
            .find(|&&(dx, dy, _, _)| (dx, dy) == (curr.0 - prev.0, curr.1 - prev.1))
        {
            *links.entry(prev).or_default() |= out;
            *links.entry(curr).or_default() |= back;
            *flows.entry(prev).or_default() |= out << 4;
            *flows.entry(curr).or_default() |= back;
        }
    }
    let coords: HashSet<_> = continuous.iter().copied().collect();
    for &p in &coords {
        for &(dx, dy, out, _) in &NEIGHBORS {
            let n = (p.0 + dx, p.1 + dy);
            if n.0 >= 0
                && n.1 >= 0
                && n.0 < dimension
                && n.1 < dimension
                && !coords.contains(&n)
                && catalog.is_water_texture(&textures[(n.1 * dimension + n.0) as usize])
            {
                *links.entry(p).or_default() |= out;
            }
        }
    }
    let mut seed = 17;
    for (i, &(x, y)) in continuous.iter().enumerate() {
        let conn = *links.get(&(x, y)).unwrap_or(&0);
        let flow = *flows.get(&(x, y)).unwrap_or(&0);
        let kind = classify(conn, i);
        let tile =
            catalog.find_water_tile(conn, kind, options.preferred_river_texture.as_deref(), seed);
        seed += 1;
        if let Some(tile) = tile {
            result.water_tiles.push(RiverTilePlacement {
                x,
                y,
                texture: tile.texture.clone(),
                connections: conn,
                kind,
                flow,
            });
        } else {
            result.unsupported.push((x, y));
        }
    }
    if options.generate_bank_transitions {
        let mut visited = HashSet::new();
        for &(x, y) in &continuous {
            for &(dx, dy, side, _) in &NEIGHBORS {
                let (bx, by) = (x + dx, y + dy);
                if bx < 0
                    || by < 0
                    || bx >= dimension
                    || by >= dimension
                    || coords.contains(&(bx, by))
                    || visited.contains(&(bx, by))
                    || catalog.is_water_texture(&textures[(by * dimension + bx) as usize])
                {
                    continue;
                }
                let bank =
                    catalog.find_bank_tile(side, options.preferred_bank_texture.as_deref(), seed);
                seed += 1;
                if let Some(bank) = bank.filter(|s| !s.is_empty()) {
                    result.bank_tiles.push(RiverBankPlacement {
                        x: bx,
                        y: by,
                        texture: bank.into(),
                        side,
                    });
                    visited.insert((bx, by));
                }
            }
        }
    }
    if !result.unsupported.is_empty() {
        result.succeeded = false;
        result.water_tiles.clear();
        result.bank_tiles.clear();
        result.height_adjustments.clear();
    }
    Ok(result)
}

fn sample(dimension: i32, heights: &[u8], size: i32, (x, y): Point) -> f32 {
    let scale = (size as f64 - 1.0) / dimension as f64;
    let left = (x as f64 * scale).round_ties_even() as i32;
    let right = ((x + 1) as f64 * scale).round_ties_even() as i32;
    let top = (y as f64 * scale).round_ties_even() as i32;
    let bottom = ((y + 1) as f64 * scale).round_ties_even() as i32;
    (heights[(top * size + left) as usize] as i32
        + heights[(top * size + right) as usize] as i32
        + heights[(bottom * size + left) as usize] as i32
        + heights[(bottom * size + right) as usize] as i32) as f32
        / 4.0
}
/// Update a minimum while retaining the C# Dictionary insertion order.
fn minimum(
    targets: &mut Vec<(usize, u8)>,
    positions: &mut HashMap<usize, usize>,
    index: usize,
    value: u8,
) {
    if let Some(&p) = positions.get(&index) {
        targets[p].1 = targets[p].1.min(value);
    } else {
        positions.insert(index, targets.len());
        targets.push((index, value));
    }
}
fn height_profile(
    dimension: i32,
    heights: &[u8],
    size: i32,
    path: &[Point],
    options: &RiverPlannerOptions,
    result: &mut RiverStrokePlan,
) {
    let elevations: Vec<_> = path
        .iter()
        .map(|&p| sample(dimension, heights, size, p))
        .collect();
    for i in 1..path.len() {
        if elevations[i] > elevations[i - 1] {
            result.warnings.push(RiverElevationWarning {
                x: path[i].0,
                y: path[i].1,
                current_elevation: elevations[i].round_ties_even() as i32,
                target_elevation: elevations[i - 1].round_ties_even() as i32,
                message: "河水逆流警示：下游海拔高於上游。".into(),
            });
        }
    }
    if options.elevation_mode == RiverElevationMode::ValidateOnly {
        return;
    }
    let water_bed = options
        .water_level
        .map(|w| {
            ((w / options.heightmap_step).floor() - options.water_bed_depth.max(6) as f32)
                .clamp(0.0, 255.0) as i32
        })
        .unwrap_or(255);
    let mut beds = vec![water_bed.min(elevations[0].round_ties_even() as i32)];
    for &e in &elevations[1..] {
        beds.push(
            (beds.last().unwrap() - options.min_slope_drop.max(0))
                .min(e.round_ties_even() as i32)
                .max(0),
        );
    }
    if options.elevation_mode == RiverElevationMode::AutoCarveTrench && options.trench_depth > 0 {
        for b in &mut beds {
            *b = (*b - options.trench_depth).max(0);
        }
    }
    let scale = (size as f64 - 1.0) / dimension as f64;
    let mut targets = vec![];
    let mut positions = HashMap::new();
    if options.water_level.is_some() {
        let core = 0.75f64.max(scale / 4.0);
        let bank = options.bank_slope_vertices.max(1) as f64;
        let radius = core + bank;
        for i in 0..path.len() {
            let next = (i + 1).min(path.len() - 1);
            let ax = (path[i].0 as f64 + 0.5) * scale;
            let ay = (path[i].1 as f64 + 0.5) * scale;
            let bx = (path[next].0 as f64 + 0.5) * scale;
            let by = (path[next].1 as f64 + 0.5) * scale;
            let dx = bx - ax;
            let dy = by - ay;
            let length2 = dx * dx + dy * dy;
            let left = ((ax.min(bx) - radius).ceil() as i32).max(0);
            let right = ((ax.max(bx) + radius).floor() as i32).min(size - 1);
            let top = ((ay.min(by) - radius).ceil() as i32).max(0);
            let bottom = ((ay.max(by) + radius).floor() as i32).min(size - 1);
            for y in top..=bottom {
                for x in left..=right {
                    let t = if length2 == 0.0 {
                        0.0
                    } else {
                        (((x as f64 - ax) * dx + (y as f64 - ay) * dy) / length2).clamp(0.0, 1.0)
                    };
                    let distance = ((x as f64 - ax - t * dx).powi(2)
                        + (y as f64 - ay - t * dy).powi(2))
                    .sqrt();
                    if distance >= radius {
                        continue;
                    }
                    let index = (y * size + x) as usize;
                    let bed = beds[i] as f64 + t * (beds[next] - beds[i]) as f64;
                    let slope = ((distance - core) / bank).clamp(0.0, 1.0);
                    let slope = slope * slope * (3.0 - 2.0 * slope);
                    let target = (bed + (heights[index] as f64 - bed) * slope)
                        .floor()
                        .clamp(0.0, heights[index] as f64) as u8;
                    if target < heights[index] {
                        minimum(&mut targets, &mut positions, index, target);
                    }
                }
            }
        }
        targets.sort_by_key(|&(i, _)| i);
    } else {
        for (i, &(x, y)) in path.iter().enumerate() {
            let target = beds[i].clamp(0, 255) as u8;
            let mut vy = (y as f64 * scale).ceil() as i32;
            while vy as f64 <= (y + 1) as f64 * scale && vy < size {
                let mut vx = (x as f64 * scale).ceil() as i32;
                while vx as f64 <= (x + 1) as f64 * scale && vx < size {
                    let index = (vy * size + vx) as usize;
                    if target < heights[index] {
                        minimum(&mut targets, &mut positions, index, target);
                    }
                    vx += 1;
                }
                vy += 1;
            }
        }
    }
    for (index, after) in targets {
        result.height_adjustments.push(RiverHeightAdjustment {
            index,
            vertex_x: index as i32 % size,
            vertex_y: index as i32 / size,
            before: heights[index],
            after,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ground(d: i32) -> Vec<String> {
        vec!["grass".into(); (d * d) as usize]
    }
    #[test]
    fn catalog_native_masks_and_case_insensitive_preferences() {
        let c = RiverTileCatalog::build_available(None, Some("fluss2"), None);
        assert_eq!(c.available_river_tiles.len(), 36);
        assert_eq!(
            c.find_water_tile(EAST | WEST, RiverTileKind::Straight, None, 0)
                .unwrap()
                .texture,
            "FLUSS2"
        );
        assert!(c.is_water_texture("seeFoam"));
        assert!(c.is_water_texture("4sA"));
        assert!(!c.is_water_texture("4S"));
        assert!(c.is_bank_texture("erdeflussr99"));
    }
    #[test]
    fn empty_inventory_keeps_csharp_base_fallback() {
        let c = RiverTileCatalog::build_available(Some(&[]), None, None);
        assert!(c
            .available_river_tiles
            .iter()
            .all(|t| t.texture == "FLUSS1"));
        assert!(c.find_bank_tile(NORTH, None, 0).is_none());
    }
    #[test]
    fn backtrack_is_removed_and_diagonals_are_orthogonal() {
        assert_eq!(
            rasterize_orthogonal_path(&[(0, 0), (1, 0), (0, 0)]),
            vec![(0, 0)]
        );
        let p = rasterize_orthogonal_path(&[(0, 0), (3, 3)]);
        assert_eq!(p.len(), 7);
        assert!(p
            .windows(2)
            .all(|w| (w[0].0 - w[1].0).abs() + (w[0].1 - w[1].1).abs() == 1));
    }
    #[test]
    fn straight_flow_and_ordered_banks() {
        let c = RiverTileCatalog::build_available(None, None, None);
        let p = plan(
            5,
            &ground(5),
            None,
            0,
            &[(1, 2), (3, 2)],
            &c,
            &Default::default(),
        )
        .unwrap();
        assert_eq!(
            p.water_tiles
                .iter()
                .map(|t| (t.connections, t.flow, t.kind))
                .collect::<Vec<_>>(),
            vec![
                (EAST, OUTFLOW_EAST, RiverTileKind::Source),
                (
                    EAST | WEST,
                    INFLOW_WEST | OUTFLOW_EAST,
                    RiverTileKind::Straight
                ),
                (WEST, INFLOW_WEST, RiverTileKind::Estuary)
            ]
        );
        assert_eq!(
            (p.bank_tiles[0].x, p.bank_tiles[0].y, p.bank_tiles[0].side),
            (1, 1, NORTH)
        );
        assert_eq!(p.bank_tiles.len(), 8);
    }
    #[test]
    fn existing_water_adds_connection_without_flow_or_bank() {
        let c = RiverTileCatalog::build_available(None, None, None);
        let mut tex = ground(3);
        tex[1] = "SEE1".into();
        let p = plan(3, &tex, None, 0, &[(1, 1)], &c, &Default::default()).unwrap();
        assert_eq!(p.water_tiles[0].connections, NORTH);
        assert_eq!(p.water_tiles[0].flow, 0);
        assert_eq!(p.bank_tiles.len(), 3);
    }
    #[test]
    fn unsupported_is_atomic_and_bounds_are_rejected() {
        let c = RiverTileCatalog::new(vec![], vec![], None, None);
        let p = plan(
            3,
            &ground(3),
            Some(&[20; 16]),
            4,
            &[(0, 0), (1, 0)],
            &c,
            &Default::default(),
        )
        .unwrap();
        assert!(!p.succeeded);
        assert!(p.height_adjustments.is_empty());
        assert_eq!(p.unsupported, vec![(0, 0), (1, 0)]);
        assert!(plan(2, &ground(3), None, 0, &[], &c, &Default::default()).is_err());
        assert_eq!(
            plan(3, &ground(3), None, 0, &[(-1, 0)], &c, &Default::default())
                .unwrap()
                .unsupported,
            vec![(-1, 0)]
        );
    }
    #[test]
    fn validate_and_downhill_reversal() {
        let c = RiverTileCatalog::build_available(None, None, None);
        let heights: Vec<_> = (0..4).flat_map(|_| [10, 20, 30, 40]).collect();
        let mut o = RiverPlannerOptions {
            elevation_mode: RiverElevationMode::ValidateOnly,
            ..Default::default()
        };
        let p = plan(3, &ground(3), Some(&heights), 4, &[(0, 1), (2, 1)], &c, &o).unwrap();
        assert_eq!(p.warnings.len(), 2);
        assert!(p.height_adjustments.is_empty());
        o.auto_detect_downhill_flow = true;
        let p = plan(3, &ground(3), Some(&heights), 4, &[(0, 1), (2, 1)], &c, &o).unwrap();
        assert_eq!(p.water_tiles[0].x, 2);
        assert!(p.warnings.is_empty());
    }
    #[test]
    fn elevation_carving_only_lowers_and_uses_even_rounding() {
        let c = RiverTileCatalog::build_available(None, None, None);
        let h = [10, 11, 20, 10, 11, 20, 10, 11, 20];
        let p = plan(
            2,
            &ground(2),
            Some(&h),
            3,
            &[(0, 0), (1, 0)],
            &c,
            &Default::default(),
        )
        .unwrap();
        assert!(p.height_adjustments.iter().all(|a| a.after < a.before));
        assert!(p
            .height_adjustments
            .iter()
            .any(|a| a.vertex_x == 2 && a.after == 9));
        assert_eq!(p.warnings[0].target_elevation, 10);
    }
    #[test]
    fn water_channel_is_local_sorted_and_below_water_bed() {
        let c = RiverTileCatalog::build_available(None, None, None);
        let o = RiverPlannerOptions {
            water_level: Some(80.0),
            water_bed_depth: 6,
            ..Default::default()
        };
        let p = plan(
            4,
            &ground(4),
            Some(&[60; 289]),
            17,
            &[(1, 1), (2, 1)],
            &c,
            &o,
        )
        .unwrap();
        assert!(p.succeeded);
        assert!(!p.height_adjustments.is_empty());
        assert!(p
            .height_adjustments
            .windows(2)
            .all(|w| w[0].index < w[1].index));
        assert!(p
            .height_adjustments
            .iter()
            .any(|a| a.vertex_x == 6 && a.vertex_y == 6 && a.after == 14));
        assert!(p
            .height_adjustments
            .iter()
            .all(|a| a.after < 60 && a.vertex_y < 11));
    }
    #[test]
    fn invalid_water_metadata_returns_original_path() {
        let c = RiverTileCatalog::build_available(None, None, None);
        for water in [f32::NAN, f32::INFINITY, 20.0] {
            let o = RiverPlannerOptions {
                water_level: Some(water),
                ..Default::default()
            };
            let p = plan(2, &ground(2), None, 0, &[(0, 0), (1, 1)], &c, &o).unwrap();
            assert!(!p.succeeded);
            assert_eq!(p.unsupported, vec![(0, 0), (1, 1)]);
        }
    }
}
