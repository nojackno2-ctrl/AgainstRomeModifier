//! Designer-facing diagnostics ported from MapEditor.Modules/Pathfinding.
//! These are editor heuristics, not an implementation of the native game pathfinder.
use std::collections::{HashSet, VecDeque};

pub mod formation;
pub mod repair;
pub use formation::{
    analyze_formation, diagnose_route, diagnose_routes, evaluate_corridors_signed,
    required_clearance_signed, topological_chokes_signed, EndpointIssue, FormationReport,
    FormationTrip, RouteDiagnostic, RouteStatus,
};
pub use repair::{
    apply_repair, bridge_action, recommended_repairs, road_heal_actions, undo_repair, IssueKind,
    RepairAction, RepairError, RepairOutcome, RepairSession,
};

const DIRS: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coordinate {
    pub x: i32,
    pub z: i32,
    pub world_x: f32,
    pub world_z: f32,
}
impl Coordinate {
    pub fn from_tile(x: i32, z: i32, size: usize, world: f32) -> Self {
        let scale = world / size as f32;
        Self {
            x,
            z,
            world_x: (x as f32 + 0.5) * scale,
            world_z: (z as f32 + 0.5) * scale,
        }
    }

    /// Construct coordinate from world space floats, clamped to the grid bounds,
    /// matching C# `NavMeshCoordinate.FromWorld`.
    pub fn from_world(world_x: f32, world_z: f32, size: usize, world: f32) -> Self {
        let size = if size == 0 { 256 } else { size };
        let world = if world.is_finite() && world > 0.0 {
            world
        } else {
            16384.0
        };
        let scale = world / size as f32;
        let max = size.saturating_sub(1) as i32;
        let x = if scale > 0.0 {
            ((world_x / scale) as i32).clamp(0, max)
        } else {
            0
        };
        let z = if scale > 0.0 {
            ((world_z / scale) as i32).clamp(0, max)
        } else {
            0
        };
        Self {
            x,
            z,
            world_x,
            world_z,
        }
    }
}
#[derive(Clone, Debug)]
pub struct PassabilityGrid {
    pub size: usize,
    pub world_dimension: f32,
    collision: Option<Vec<u8>>,
    heights: Option<(usize, Vec<u8>, f32, f32)>,
}
impl PassabilityGrid {
    /// Invalid optional layers are ignored, matching the existing editor loader.
    pub fn new(
        size: usize,
        collision: Option<&[u8]>,
        height: Option<(usize, &[u8], f32, f32)>,
        world_dimension: f32,
    ) -> Result<Self, &'static str> {
        let cells = size
            .checked_mul(size)
            .filter(|_| size > 0 && size <= i32::MAX as usize)
            .ok_or("invalid grid size")?;
        if !world_dimension.is_finite() || world_dimension <= 0.0 {
            return Err("invalid world dimension");
        }
        let collision = collision.filter(|v| v.len() == cells).map(<[u8]>::to_vec);
        let heights = height
            .filter(|(s, v, step, _)| {
                *s > 1 && s.checked_mul(*s) == Some(v.len()) && step.is_finite() && *step > 0.0
            })
            .map(|(s, v, step, water)| {
                (
                    s,
                    v.to_vec(),
                    step,
                    if water.is_finite() { water } else { 0.0 },
                )
            });
        Ok(Self {
            size,
            world_dimension,
            collision,
            heights,
        })
    }
    pub fn in_bounds(&self, x: i32, z: i32) -> bool {
        x >= 0 && z >= 0 && (x as usize) < self.size && (z as usize) < self.size
    }
    pub fn collision_byte(&self, x: i32, z: i32) -> u8 {
        if !self.in_bounds(x, z) {
            return 0;
        }
        self.collision
            .as_ref()
            .map_or(0, |v| v[z as usize * self.size + x as usize])
    }
    pub fn collision_blocked(&self, x: i32, z: i32) -> bool {
        !self.in_bounds(x, z) || self.collision_byte(x, z) != 0
    }
    pub fn submerged(&self, x: i32, z: i32) -> bool {
        if !self.in_bounds(x, z) {
            return false;
        }
        self.heights.as_ref().is_some_and(|(s, v, step, water)| {
            let hx = ((x as f32 + 0.5) * (*s - 1) as f32 / self.size as f32) as usize;
            let hz = ((z as f32 + 0.5) * (*s - 1) as f32 / self.size as f32) as usize;
            v[hz.min(*s - 1) * *s + hx.min(*s - 1)] as f32 * *step < *water
        })
    }
    pub fn passable(&self, x: i32, z: i32) -> bool {
        self.in_bounds(x, z) && !self.collision_blocked(x, z) && !self.submerged(x, z)
    }
    pub fn coordinate(&self, x: i32, z: i32) -> Coordinate {
        Coordinate::from_tile(x, z, self.size, self.world_dimension)
    }
    pub fn world_to_coordinate(&self, world_x: f32, world_z: f32) -> Coordinate {
        Coordinate::from_world(world_x, world_z, self.size, self.world_dimension)
    }
    pub fn tile_scale(&self) -> f32 {
        self.world_dimension / self.size as f32
    }
    pub fn world_to_tile(&self, x: f32, z: f32) -> (i32, i32) {
        let max = self.size as i32 - 1;
        (
            (x / self.tile_scale()) as i32,
            (z / self.tile_scale()) as i32,
        )
            .map_clamped(max)
    }
    pub fn tile_collision_blocked(&self, x: i32, z: i32, dimension: usize) -> bool {
        if dimension == 0 {
            return true;
        }
        if dimension == self.size {
            return self.collision_blocked(x, z);
        }
        let scale = self.size as f32 / dimension as f32;
        let max = self.size as i32 - 1;
        let x0 = (x as f32 * scale).floor() as i32;
        let x1 = ((x + 1) as f32 * scale).ceil() as i32 - 1;
        let z0 = (z as f32 * scale).floor() as i32;
        let z1 = ((z + 1) as f32 * scale).ceil() as i32 - 1;
        (z0.clamp(0, max)..=z1.clamp(0, max)).any(|zz| {
            (x0.clamp(0, max)..=x1.clamp(0, max)).any(|xx| self.collision_blocked(xx, zz))
        })
    }
    pub fn tile_submerged(&self, x: i32, z: i32, dimension: usize) -> bool {
        if dimension == 0 {
            return false;
        }
        if dimension == self.size {
            return self.submerged(x, z);
        }
        let scale = self.size as f32 / dimension as f32;
        let (x, z) = (
            ((x as f32 + 0.5) * scale) as i32,
            ((z as f32 + 0.5) * scale) as i32,
        )
            .map_clamped(self.size as i32 - 1);
        self.submerged(x, z)
    }
    fn neighbors(&self, index: usize) -> impl Iterator<Item = usize> + '_ {
        let x = (index % self.size) as i32;
        let z = (index / self.size) as i32;
        DIRS.into_iter().filter_map(move |(dx, dz)| {
            self.in_bounds(x + dx, z + dz).then_some(
                ((z + dz) as usize)
                    .wrapping_mul(self.size)
                    .wrapping_add((x + dx) as usize),
            )
        })
    }
}
trait ClampPair {
    fn map_clamped(self, max: i32) -> Self;
}
impl ClampPair for (i32, i32) {
    fn map_clamped(self, max: i32) -> Self {
        (self.0.clamp(0, max), self.1.clamp(0, max))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct IsolatedRegion {
    pub component_id: usize,
    pub tile_count: usize,
    pub centroid: Coordinate,
    pub bounds: (i32, i32, i32, i32),
    pub contains_interest: bool,
    pub bridge: Option<Coordinate>,
    pub bridge_distance: usize,
}
pub fn analyze_connectivity(
    grid: &PassabilityGrid,
    seeds: &[(i32, i32)],
    interest: &[(i32, i32)],
    minimum: usize,
) -> Vec<IsolatedRegion> {
    let mut labels = vec![-1i32; grid.size * grid.size];
    for (i, label) in labels.iter_mut().enumerate() {
        if !grid.passable((i % grid.size) as i32, (i / grid.size) as i32) {
            *label = -2;
        }
    }
    let mut components: Vec<Vec<usize>> = Vec::new();
    for start in 0..labels.len() {
        if labels[start] != -1 {
            continue;
        }
        let id = components.len() as i32;
        let mut cells = vec![start];
        labels[start] = id;
        let mut head = 0;
        while head < cells.len() {
            let current = cells[head];
            head += 1;
            for n in grid.neighbors(current) {
                if labels[n] == -1 {
                    labels[n] = id;
                    cells.push(n);
                }
            }
        }
        components.push(cells);
    }
    let primary: HashSet<i32> = seeds
        .iter()
        .filter(|&&(x, z)| grid.in_bounds(x, z))
        .map(|&(x, z)| labels[z as usize * grid.size + x as usize])
        .filter(|&id| id >= 0)
        .collect();
    let mut result = Vec::new();
    for (id, cells) in components.iter().enumerate() {
        if primary.contains(&(id as i32)) || cells.len() < minimum {
            continue;
        }
        let coords: Vec<_> = cells
            .iter()
            .map(|&i| ((i % grid.size) as i32, (i / grid.size) as i32))
            .collect();
        let centroid = grid.coordinate(
            (coords.iter().map(|p| p.0 as i64).sum::<i64>() / cells.len() as i64) as i32,
            (coords.iter().map(|p| p.1 as i64).sum::<i64>() / cells.len() as i64) as i32,
        );
        let bounds = (
            coords.iter().map(|p| p.0).min().unwrap(),
            coords.iter().map(|p| p.1).min().unwrap(),
            coords.iter().map(|p| p.0).max().unwrap(),
            coords.iter().map(|p| p.1).max().unwrap(),
        );
        let (bridge, bridge_distance) = shortest_bridge(grid, &labels, cells, &primary);
        result.push(IsolatedRegion {
            component_id: id,
            tile_count: cells.len(),
            centroid,
            bounds,
            contains_interest: coords.iter().any(|p| interest.contains(p)),
            bridge,
            bridge_distance,
        });
    }
    result.sort_by_key(|r| {
        (
            std::cmp::Reverse(r.contains_interest),
            std::cmp::Reverse(r.tile_count),
        )
    });
    result
}
fn shortest_bridge(
    grid: &PassabilityGrid,
    labels: &[i32],
    cells: &[usize],
    primary: &HashSet<i32>,
) -> (Option<Coordinate>, usize) {
    if primary.is_empty() {
        return (None, 0);
    }
    let mut distances = vec![usize::MAX; labels.len()];
    let mut previous = vec![0; labels.len()];
    let mut queue = VecDeque::new();
    for &c in cells {
        if grid.neighbors(c).any(|n| labels[n] != labels[c]) {
            distances[c] = 0;
            queue.push_back(c);
        }
    }
    while let Some(c) = queue.pop_front() {
        let d = distances[c];
        for n in grid.neighbors(c) {
            if primary.contains(&labels[n]) {
                let mut mid = c;
                for _ in 0..d.saturating_sub(1) / 2 {
                    mid = previous[mid];
                }
                return (
                    Some(grid.coordinate((mid % grid.size) as i32, (mid / grid.size) as i32)),
                    d,
                );
            }
            if d < 12 && labels[n] == -2 && distances[n] == usize::MAX {
                distances[n] = d + 1;
                previous[n] = c;
                queue.push_back(n);
            }
        }
    }
    (None, 0)
}

/// Manhattan distance to an actual blocked cell; -1 means no obstacle exists.
/// Map boundaries are intentionally not obstacles, preserving the C# implementation.
pub fn clearance_field(grid: &PassabilityGrid) -> Vec<i32> {
    let mut field = vec![-1; grid.size * grid.size];
    let mut queue = VecDeque::new();
    for (i, v) in field.iter_mut().enumerate() {
        if !grid.passable((i % grid.size) as i32, (i / grid.size) as i32) {
            *v = 0;
            queue.push_back(i);
        }
    }
    while let Some(c) = queue.pop_front() {
        for n in grid.neighbors(c) {
            if field[n] == -1 {
                field[n] = field[c] + 1;
                queue.push_back(n);
            }
        }
    }
    field
}
pub fn required_clearance(count: usize) -> (i32, f32) {
    match count {
        0 => (0, 0.0),
        1 => (1, 64.0),
        2..=5 => (2, 160.0),
        6..=10 => (4, 280.0),
        11..=15 => (5, 360.0),
        _ => (6, 440.0),
    }
}
pub fn max_safe_unit_count(clearance: i32) -> usize {
    match clearance {
        i32::MIN..=0 => 0,
        1 => 1,
        2 => 5,
        3 => 8,
        4 => 12,
        _ => 20,
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct ChokePoint {
    pub location: Coordinate,
    pub clearance: i32,
    pub width: f32,
    pub required: i32,
    pub required_width: f32,
    pub safe_units: usize,
    pub troop_count: usize,
    pub kind: &'static str,
}
pub(crate) fn choke(
    grid: &PassabilityGrid,
    x: i32,
    z: i32,
    c: i32,
    count: usize,
    kind: &'static str,
) -> ChokePoint {
    let (required, required_width) = required_clearance(count);
    ChokePoint {
        location: grid.coordinate(x, z),
        clearance: c,
        width: c as f32 * grid.tile_scale(),
        required,
        required_width,
        safe_units: max_safe_unit_count(c),
        troop_count: count,
        kind,
    }
}
pub fn find_path(
    grid: &PassabilityGrid,
    start: (i32, i32),
    end: (i32, i32),
) -> Option<Vec<(i32, i32)>> {
    if !grid.passable(start.0, start.1) || !grid.passable(end.0, end.1) {
        return None;
    }
    let index = |p: (i32, i32)| p.1 as usize * grid.size + p.0 as usize;
    let s = index(start);
    let e = index(end);
    let mut parent = vec![usize::MAX; grid.size * grid.size];
    parent[s] = s;
    let mut queue = VecDeque::from([s]);
    while let Some(c) = queue.pop_front() {
        if c == e {
            break;
        }
        for n in grid.neighbors(c) {
            if parent[n] == usize::MAX
                && grid.passable((n % grid.size) as i32, (n / grid.size) as i32)
            {
                parent[n] = c;
                queue.push_back(n);
            }
        }
    }
    if parent[e] == usize::MAX {
        return None;
    }
    let mut path = vec![];
    let mut c = e;
    loop {
        path.push(((c % grid.size) as i32, (c / grid.size) as i32));
        if c == s {
            break;
        }
        c = parent[c];
    }
    path.reverse();
    Some(path)
}
pub fn evaluate_corridors(
    grid: &PassabilityGrid,
    trips: &[((i32, i32), (i32, i32), usize)],
) -> Vec<ChokePoint> {
    let field = clearance_field(grid);
    let mut seen = HashSet::new();
    let mut result = vec![];
    for &(start, end, count) in trips {
        if count <= 1 {
            continue;
        }
        if let Some(path) = find_path(grid, start, end) {
            if let Some(&(x, z)) = path
                .iter()
                .min_by_key(|&&(x, z)| field[z as usize * grid.size + x as usize])
            {
                let c = field[z as usize * grid.size + x as usize];
                if c < required_clearance(count).0 && seen.insert((x, z)) {
                    result.push(choke(grid, x, z, c, count, "TravelPathChoke"));
                }
            }
        }
    }
    result
}
pub fn topological_chokes(grid: &PassabilityGrid, count: usize) -> Vec<ChokePoint> {
    let field = clearance_field(grid);
    let mut result = vec![];
    for z in 1..grid.size as i32 - 1 {
        for x in 1..grid.size as i32 - 1 {
            let c = field[z as usize * grid.size + x as usize];
            if grid.passable(x, z) && c > 0 && c < required_clearance(count).0 {
                let horizontal = !grid.passable(x - 1, z) && !grid.passable(x + 1, z);
                let vertical = !grid.passable(x, z - 1) && !grid.passable(x, z + 1);
                if horizontal || vertical {
                    result.push(choke(
                        grid,
                        x,
                        z,
                        c,
                        count,
                        if horizontal {
                            "HorizontalChasm"
                        } else {
                            "VerticalChasm"
                        },
                    ));
                }
            }
        }
    }
    result
}

pub fn is_road(texture: &str) -> bool {
    let s = texture.to_ascii_uppercase();
    !s.trim().is_empty()
        && (s.starts_with("H_WEG")
            || s.starts_with("V_WEG")
            || s.starts_with("WEG")
            || s.contains("WEG_")
            || s.contains("ROM")
            || s.starts_with("PFLASTER")
            || s.starts_with("PFAD"))
}
pub fn road_style(texture: &str) -> &'static str {
    let s = texture.to_ascii_uppercase();
    if s.contains("ROM") || s.starts_with("PFLASTER") {
        "Roman"
    } else if s.starts_with("PFAD") {
        "Dirt"
    } else {
        "Standard"
    }
}
fn supports(texture: &str, dx: i32, dz: i32) -> bool {
    let s = texture.to_ascii_uppercase();
    if s.starts_with("H_WEG") || s.starts_with("WEG_H") {
        dz == 0
    } else if s.starts_with("V_WEG") || s.starts_with("WEG_V") {
        dx == 0
    } else {
        true
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct RoadGap {
    pub start: Coordinate,
    pub end: Coordinate,
    pub tiles: Vec<Coordinate>,
    pub texture: &'static str,
    pub clear_collision: bool,
    pub style: &'static str,
}
impl RoadGap {
    /// Number of missing road tiles in the gap, matching C# `RoadGapCandidate.GapDistance`.
    #[inline]
    pub fn gap_distance(&self) -> usize {
        self.tiles.len()
    }
}
pub fn detect_road_gaps(
    dimension: usize,
    textures: &[String],
    grid: Option<&PassabilityGrid>,
    maximum: usize,
) -> Result<Vec<RoadGap>, &'static str> {
    if dimension == 0
        || dimension > i32::MAX as usize
        || dimension.checked_mul(dimension) != Some(textures.len())
    {
        return Err("invalid texture dimensions");
    }
    let road: HashSet<(i32, i32)> = textures
        .iter()
        .enumerate()
        .filter(|(_, t)| is_road(t))
        .map(|(i, _)| ((i % dimension) as i32, (i / dimension) as i32))
        .collect();
    // Row-major enumeration makes repair IDs stable across processes.
    let endpoints: Vec<_> = (0..textures.len())
        .map(|i| ((i % dimension) as i32, (i / dimension) as i32))
        .filter(|&(x, z)| {
            road.contains(&(x, z))
                && DIRS
                    .iter()
                    .filter(|&&(dx, dz)| road.contains(&(x + dx, z + dz)))
                    .count()
                    <= 1
        })
        .collect();
    let mut seen = HashSet::new();
    let mut result = vec![];
    for (i, &a) in endpoints.iter().enumerate() {
        for &b in &endpoints[i + 1..] {
            let dx = (a.0 - b.0).abs();
            let dz = (a.1 - b.1).abs();
            let ta = &textures[a.1 as usize * dimension + a.0 as usize];
            let tb = &textures[b.1 as usize * dimension + b.0 as usize];
            if (dx + dz) as usize > maximum.saturating_add(1)
                || !supports(ta, dx, dz)
                || !supports(tb, dx, dz)
            {
                continue;
            }
            let mut gaps = vec![];
            if a.0 == b.0 {
                for z in a.1.min(b.1) + 1..a.1.max(b.1) {
                    if !road.contains(&(a.0, z)) {
                        gaps.push((a.0, z));
                    }
                }
            } else if a.1 == b.1 {
                for x in a.0.min(b.0) + 1..a.0.max(b.0) {
                    if !road.contains(&(x, a.1)) {
                        gaps.push((x, a.1));
                    }
                }
            } else if !road.contains(&(a.0, b.1)) {
                gaps.push((a.0, b.1));
            } else if !road.contains(&(b.0, a.1)) {
                gaps.push((b.0, a.1));
            }
            if gaps.is_empty()
                || gaps.len() > maximum
                || grid
                    .is_some_and(|g| gaps.iter().any(|&(x, z)| g.tile_submerged(x, z, dimension)))
            {
                continue;
            }
            let mut key = gaps.clone();
            key.sort_by_key(|&(x, z)| (z, x));
            if !seen.insert(key) {
                continue;
            }
            let styles = (road_style(ta), road_style(tb));
            let style = if styles.0 == "Roman" || styles.1 == "Roman" {
                "Roman"
            } else if styles.0 == "Dirt" || styles.1 == "Dirt" {
                "Dirt"
            } else {
                "Standard"
            };
            let texture = match style {
                "Roman" => {
                    if dz == 0 {
                        "WEG_H1ROM"
                    } else if dx == 0 {
                        "WEG_V1ROM"
                    } else {
                        "Pflaster_braun1"
                    }
                }
                "Dirt" => "PFAD1",
                _ => {
                    if dz == 0 {
                        "H_WEG1"
                    } else if dx == 0 {
                        "V_WEG1"
                    } else {
                        "weg1"
                    }
                }
            };
            let coord = |(x, z)| Coordinate::from_tile(x, z, dimension, 16384.0);
            result.push(RoadGap {
                start: coord(a),
                end: coord(b),
                clear_collision: grid.is_some_and(|g| {
                    gaps.iter()
                        .any(|&(x, z)| g.tile_collision_blocked(x, z, dimension))
                }),
                tiles: gaps.into_iter().map(coord).collect(),
                texture,
                style,
            });
        }
    }
    Ok(result)
}

#[derive(Clone, Debug, PartialEq)]
pub struct TileChange {
    pub x: i32,
    pub z: i32,
    pub texture: String,
    pub collision: Option<u8>,
}
/// Pure prescriptions; the caller owns transactional application and undo history.
pub fn road_repairs(gaps: &[RoadGap]) -> Vec<Vec<TileChange>> {
    gaps.iter()
        .map(|g| {
            g.tiles
                .iter()
                .map(|p| TileChange {
                    x: p.x,
                    z: p.z,
                    texture: g.texture.into(),
                    collision: g.clear_collision.then_some(0),
                })
                .collect()
        })
        .collect()
}
pub fn bridge_repair(
    region: &IsolatedRegion,
    texture: &str,
    dimension: usize,
) -> Option<TileChange> {
    if region.bridge_distance == 0 {
        return None;
    }
    let p = region.bridge?;
    let dim = if dimension == 0 { 64 } else { dimension };
    let scale = 16384.0 / dim as f32;
    let (x, z) =
        ((p.world_x / scale) as i32, (p.world_z / scale) as i32).map_clamped(dim as i32 - 1);
    Some(TileChange {
        x,
        z,
        texture: texture.into(),
        collision: Some(0),
    })
}

/// Complete NavMesh analysis report, mirroring C# `NavMeshAnalysisResult` in
/// `AgainstRomeMapEditor.Modules.Pathfinding.NavMeshRepairContracts`.
#[derive(Clone, Debug, PartialEq)]
pub struct NavMeshAnalysisResult {
    pub isolated_regions: Vec<IsolatedRegion>,
    pub road_gaps: Vec<RoadGap>,
    pub formation_choke_points: Vec<ChokePoint>,
    pub recommended_repairs: Vec<RepairAction>,
}

impl NavMeshAnalysisResult {
    pub fn empty() -> Self {
        Self {
            isolated_regions: Vec::new(),
            road_gaps: Vec::new(),
            formation_choke_points: Vec::new(),
            recommended_repairs: Vec::new(),
        }
    }
}

/// Request configuration for end-to-end NavMesh analysis.
#[derive(Clone, Debug)]
pub struct NavMeshAnalysisRequest<'a> {
    pub primary_seeds: &'a [(i32, i32)],
    pub interest_points: &'a [(i32, i32)],
    pub min_isolated_size: usize,
    pub textures: Option<(&'a [String], usize)>,
    pub max_gap_distance: usize,
    pub trips: &'a [FormationTrip],
    pub squad_max_troops: i32,
    pub bridge_texture: &'a str,
}

impl<'a> Default for NavMeshAnalysisRequest<'a> {
    fn default() -> Self {
        Self {
            primary_seeds: &[],
            interest_points: &[],
            min_isolated_size: 1,
            textures: None,
            max_gap_distance: 2,
            trips: &[],
            squad_max_troops: 0,
            bridge_texture: "H_WEG1",
        }
    }
}

/// Runs full NavMesh connectivity, road gap, formation corridor, and repair action diagnostics,
/// returning a unified `NavMeshAnalysisResult` matching C# `MapDiagnostics.AnalyzeNavMesh`.
pub fn analyze_navmesh(
    grid: &PassabilityGrid,
    request: &NavMeshAnalysisRequest,
) -> NavMeshAnalysisResult {
    let isolated = analyze_connectivity(
        grid,
        request.primary_seeds,
        request.interest_points,
        request.min_isolated_size.max(1),
    );

    let (gaps, tex_dim) = match request.textures {
        Some((tex, dim)) if dim > 0 && tex.len() == dim * dim => {
            let detected = detect_road_gaps(dim, tex, Some(grid), request.max_gap_distance)
                .unwrap_or_default();
            (detected, dim)
        }
        _ => (Vec::new(), 64),
    };

    let repairs = recommended_repairs(
        &gaps,
        &isolated,
        request.bridge_texture,
        tex_dim,
    );

    let formation_report = analyze_formation(grid, request.trips, request.squad_max_troops);
    let chokes = if !formation_report.corridor_chokes.is_empty() {
        formation_report.corridor_chokes
    } else {
        formation_report.topological_chokes
    };

    NavMeshAnalysisResult {
        isolated_regions: isolated,
        road_gaps: gaps,
        formation_choke_points: chokes,
        recommended_repairs: repairs,
    }
}

