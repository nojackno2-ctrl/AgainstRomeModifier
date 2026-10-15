//! Pure fortification planning and collision prescriptions, ported from the C# editor.
use crate::Point;
use against_rome_navigation::PassabilityGrid;
use std::collections::{HashMap, VecDeque};

/// GUID bytes in canonical UUID order (the order displayed by Guid.ToString("N")).
pub type FortificationId = [u8; 16];

#[cfg(windows)]
fn new_placement_id() -> Result<FortificationId, String> {
    #[repr(C)]
    struct Guid {
        data1: u32,
        data2: u16,
        data3: u16,
        data4: [u8; 8],
    }
    #[link(name = "ole32")]
    unsafe extern "system" {
        fn CoCreateGuid(guid: *mut Guid) -> i32;
    }
    let mut guid = Guid {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };
    // SAFETY: Guid has the Win32 GUID layout and the pointer is valid and writable
    // for the duration of this synchronous OS call. CoCreateGuid needs no COM setup.
    let status = unsafe { CoCreateGuid(&mut guid) };
    if status < 0 {
        return Err(format!(
            "CoCreateGuid failed: HRESULT 0x{:08X}",
            status as u32
        ));
    }
    let mut bytes = [0; 16];
    bytes[..4].copy_from_slice(&guid.data1.to_be_bytes());
    bytes[4..6].copy_from_slice(&guid.data2.to_be_bytes());
    bytes[6..8].copy_from_slice(&guid.data3.to_be_bytes());
    bytes[8..].copy_from_slice(&guid.data4);
    Ok(bytes)
}

#[cfg(not(windows))]
fn new_placement_id() -> Result<FortificationId, String> {
    use std::io::Read;
    let mut bytes = [0; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut source| source.read_exact(&mut bytes))
        .map_err(|error| format!("UUID entropy unavailable: {error}"))?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(bytes)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FortificationStyle {
    RomanStoneWall,
    GermanicPalisade,
    CelticPalisade,
    HunBarricade,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WallComponentKind {
    Straight,
    Corner,
    TJunction,
    CrossJunction,
    Gate,
    Tower,
    EndCap,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatePassabilityState {
    Open,
    Closed,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WallConnections(pub u8);
impl WallConnections {
    pub const NONE: Self = Self(0);
    pub const NORTH: Self = Self(1);
    pub const EAST: Self = Self(2);
    pub const SOUTH: Self = Self(4);
    pub const WEST: Self = Self(8);
    pub const ALL: Self = Self(15);
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl std::ops::BitOr for WallConnections {
    type Output = Self;
    fn bitor(self, r: Self) -> Self {
        Self(self.0 | r.0)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct WallComponentDefinition {
    pub name_def: String,
    pub kind: WallComponentKind,
    pub supported_connections: WallConnections,
    pub default_angle_deg: f32,
    pub footprint_tiles: usize,
    pub collision_pixel_span: usize,
    pub foundation_texture: Option<String>,
}
impl WallComponentDefinition {
    pub fn matches(&self, kind: WallComponentKind, connections: WallConnections) -> bool {
        self.kind == kind && self.supported_connections.contains(connections)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct FortificationPlacement {
    /// OS-generated GUID identity, suitable for persistence across process restarts.
    pub id: FortificationId,
    pub name_def: String,
    pub kind: WallComponentKind,
    pub tile: Point,
    pub world_x: f32,
    pub world_y: f32,
    pub world_z: f32,
    pub angle_deg: f32,
    pub team: i32,
    pub gate_state: Option<GatePassabilityState>,
    pub foundation_texture: Option<String>,
}
#[derive(Clone, Debug)]
pub struct WallStrokeOptions {
    pub style: FortificationStyle,
    pub team: i32,
    pub auto_corner_towers: bool,
    pub auto_gate_on_road_crossing: bool,
    pub smooth_end_caps: bool,
    pub stamp_foundations: bool,
    pub default_gate_state: GatePassabilityState,
    pub forced_gate_locations: Vec<Point>,
}
impl Default for WallStrokeOptions {
    fn default() -> Self {
        Self {
            style: FortificationStyle::RomanStoneWall,
            team: 0,
            auto_corner_towers: true,
            auto_gate_on_road_crossing: true,
            smooth_end_caps: true,
            stamp_foundations: true,
            default_gate_state: GatePassabilityState::Open,
            forced_gate_locations: vec![],
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WallStrokePlan {
    pub succeeded: bool,
    pub placements: Vec<FortificationPlacement>,
    pub footprint_tiles: Vec<Point>,
    pub gate_passage_tiles: Vec<Point>,
    pub blocked_tiles: Vec<Point>,
    pub unsupported_tiles: Vec<Point>,
    pub failure_reason: Option<String>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct FortressEnclosureReport {
    pub is_fully_enclosed: bool,
    pub enclosed_area_tiles: usize,
    pub gaps: Vec<Point>,
    pub gates: Vec<FortificationPlacement>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CollisionPixelChange {
    pub index: usize,
    pub before: u8,
    pub after: u8,
}
#[derive(Clone, Debug)]
pub struct WallTileCatalog {
    definitions: HashMap<FortificationStyle, Vec<WallComponentDefinition>>,
}
impl Default for WallTileCatalog {
    fn default() -> Self {
        use FortificationStyle::*;
        use WallComponentKind::*;
        let mut c = Self::empty();
        let styles = [
            (
                RomanStoneWall,
                [
                    "BauRomMau00_Mauer",
                    "BauRomMau02_Mauerecke",
                    "BauRomTur00_Turm",
                    "BauRomTor02_Mauertor_auf",
                ],
                "Pflaster_braun1",
                "Pflaster_braun2",
                "weg1",
                Some("BauRomPalEnd"),
            ),
            (
                GermanicPalisade,
                [
                    "BauGerPal00_Palisade",
                    "BauGerPal02_Palisadenecke",
                    "BauGerTur00",
                    "BauGerTor00_Palisadentor_auf",
                ],
                "Erde",
                "Erde",
                "PFAD1",
                Some("BauGerPalEnd"),
            ),
            (
                CelticPalisade,
                [
                    "BauKelMau00_Mauer",
                    "BauKelMau02_Mauerecke",
                    "BauKelTur00",
                    "BauKelTor00_Mauertor_auf",
                ],
                "Erde",
                "Erde",
                "PFAD1",
                None,
            ),
            (
                HunBarricade,
                [
                    "BauHunPal00",
                    "BauHunPal02_Palisadenecke",
                    "BauHunTur00",
                    "BauHunPalTor",
                ],
                "Erde",
                "Erde",
                "PFAD1",
                None,
            ),
        ];
        for (style, names, foundation, tower_foundation, gate_foundation, end) in styles {
            for (i, kind) in [Straight, Corner, Tower, Gate].into_iter().enumerate() {
                c.register(
                    style,
                    WallComponentDefinition {
                        name_def: names[i].into(),
                        kind,
                        supported_connections: match kind {
                            Corner => WallConnections(3),
                            Tower => WallConnections::ALL,
                            _ => WallConnections(10),
                        },
                        default_angle_deg: 0.,
                        footprint_tiles: if kind == Tower { 2 } else { 1 },
                        collision_pixel_span: if kind == Tower { 8 } else { 4 },
                        foundation_texture: Some(
                            match kind {
                                Tower => tower_foundation,
                                Gate => gate_foundation,
                                _ => foundation,
                            }
                            .into(),
                        ),
                    },
                );
            }
            if let Some(name) = end {
                c.register(
                    style,
                    WallComponentDefinition {
                        name_def: name.into(),
                        kind: EndCap,
                        supported_connections: WallConnections::NORTH,
                        default_angle_deg: 0.,
                        footprint_tiles: 1,
                        collision_pixel_span: 4,
                        foundation_texture: Some(foundation.into()),
                    },
                );
            }
        }
        c
    }
}
impl WallTileCatalog {
    pub fn empty() -> Self {
        Self {
            definitions: HashMap::new(),
        }
    }
    pub fn register(&mut self, style: FortificationStyle, definition: WallComponentDefinition) {
        self.definitions.entry(style).or_default().push(definition);
    }
    pub fn components(&self, style: FortificationStyle) -> &[WallComponentDefinition] {
        self.definitions.get(&style).map_or(&[], Vec::as_slice)
    }
    pub fn filtered(&self, mut exists: impl FnMut(&str) -> bool) -> Self {
        let mut result = Self::empty();
        for (&style, defs) in &self.definitions {
            for d in defs {
                if exists(&d.name_def) {
                    result.register(style, d.clone());
                }
            }
        }
        result
    }
    pub fn resolve_component(
        &self,
        style: FortificationStyle,
        c: WallConnections,
        prefer_tower: bool,
    ) -> Result<(&WallComponentDefinition, f32), &'static str> {
        use WallComponentKind::*;
        let defs = self.components(style);
        let first = defs.first().ok_or("no components registered for style")?;
        let find = |kind| defs.iter().find(|d| d.kind == kind);
        let degree = (c.0 & 15).count_ones();
        if degree == 0 {
            let d = find(EndCap).or_else(|| find(Tower)).unwrap_or(first);
            return Ok((d, d.default_angle_deg));
        }
        if degree == 1 {
            let angle = if c.contains(WallConnections::NORTH) {
                180.
            } else if c.contains(WallConnections::EAST) {
                270.
            } else if c.contains(WallConnections::SOUTH) {
                0.
            } else {
                90.
            };
            if let Some(d) = find(EndCap) {
                return Ok((d, angle));
            }
            if let Some(d) = find(Straight) {
                return Ok((d, if c.0 & 5 != 0 { 90. } else { 0. }));
            }
        }
        if degree == 2 {
            if c.0 == 5 || c.0 == 10 {
                if let Some(d) = find(Straight) {
                    return Ok((d, if c.0 == 5 { 90. } else { 0. }));
                }
            }
            if prefer_tower {
                if let Some(d) = find(Tower) {
                    return Ok((d, d.default_angle_deg));
                }
            }
            if let Some(d) = find(Corner) {
                let angle = if c.0 & 3 == 3 {
                    0.
                } else if c.0 & 6 == 6 {
                    90.
                } else if c.0 & 12 == 12 {
                    180.
                } else if c.0 & 9 == 9 {
                    270.
                } else {
                    0.
                };
                return Ok((d, angle));
            }
        }
        if degree == 3 {
            if let Some(d) = find(TJunction) {
                let angle = if c.0 & 8 == 0 {
                    0.
                } else if c.0 & 1 == 0 {
                    90.
                } else if c.0 & 2 == 0 {
                    180.
                } else if c.0 & 4 == 0 {
                    270.
                } else {
                    0.
                };
                return Ok((d, angle));
            }
            if let Some(d) = find(Tower) {
                return Ok((d, d.default_angle_deg));
            }
        }
        if degree == 4 {
            if let Some(d) = find(CrossJunction) {
                return Ok((d, 0.));
            }
            if let Some(d) = find(Tower) {
                return Ok((d, d.default_angle_deg));
            }
        }
        let d = find(Straight).unwrap_or(first);
        Ok((d, d.default_angle_deg))
    }
    pub fn resolve_gate(
        &self,
        style: FortificationStyle,
        c: WallConnections,
    ) -> Result<(&WallComponentDefinition, f32), &'static str> {
        let defs = self.components(style);
        let d = defs
            .iter()
            .find(|d| d.kind == WallComponentKind::Gate)
            .or_else(|| defs.iter().find(|d| d.kind == WallComponentKind::Straight))
            .or_else(|| defs.first())
            .ok_or("no components registered for style")?;
        Ok((d, if c.0 & 5 != 0 { 90. } else { 0. }))
    }
    pub fn resolve_tower(
        &self,
        style: FortificationStyle,
    ) -> Result<&WallComponentDefinition, &'static str> {
        let defs = self.components(style);
        defs.iter()
            .find(|d| d.kind == WallComponentKind::Tower)
            .or_else(|| defs.first())
            .ok_or("no components registered for style")
    }
}
/// Orthogonal interpolation walks X before Z, including repeated path visits.
pub fn discretize_orthogonal_path(path: &[Point]) -> Vec<Point> {
    let Some(&start) = path.first() else {
        return vec![];
    };
    let mut result = vec![start];
    for &(tx, tz) in &path[1..] {
        let (mut x, mut z) = *result.last().unwrap();
        while x != tx {
            x += if tx > x { 1 } else { -1 };
            result.push((x, z));
        }
        while z != tz {
            z += if tz > z { 1 } else { -1 };
            result.push((x, z));
        }
    }
    result
}
/// Callbacks are sampled in row-major placement order. As in C#, smooth_end_caps is retained but unused.
pub fn plan_wall_stroke(
    map_dimension: i32,
    raw_path: &[Point],
    options: &WallStrokeOptions,
    catalog: &WallTileCatalog,
    height: Option<&mut dyn FnMut(i32, i32) -> f32>,
    road: Option<&mut dyn FnMut(i32, i32) -> bool>,
    obstructed: Option<&mut dyn FnMut(i32, i32) -> bool>,
) -> WallStrokePlan {
    plan_wall_stroke_with_ids(
        map_dimension,
        raw_path,
        options,
        catalog,
        height,
        road,
        obstructed,
        new_placement_id,
    )
}

fn plan_wall_stroke_with_ids(
    map_dimension: i32,
    raw_path: &[Point],
    options: &WallStrokeOptions,
    catalog: &WallTileCatalog,
    mut height: Option<&mut dyn FnMut(i32, i32) -> f32>,
    mut road: Option<&mut dyn FnMut(i32, i32) -> bool>,
    mut obstructed: Option<&mut dyn FnMut(i32, i32) -> bool>,
    mut next_id: impl FnMut() -> Result<FortificationId, String>,
) -> WallStrokePlan {
    let fail = |tiles, reason: &str| WallStrokePlan {
        unsupported_tiles: tiles,
        failure_reason: Some(reason.into()),
        ..Default::default()
    };
    if map_dimension <= 0 {
        return fail(vec![], "地圖尺寸必須大於 0。");
    }
    let outside: Vec<_> = raw_path
        .iter()
        .copied()
        .filter(|&(x, z)| x < 0 || z < 0 || x >= map_dimension || z >= map_dimension)
        .collect();
    if !outside.is_empty() {
        return fail(outside, "路徑座標超出地圖邊界。");
    }
    let path = discretize_orthogonal_path(raw_path);
    let mut connections: HashMap<Point, WallConnections> = HashMap::new();
    if let Some(&p) = path.first() {
        connections.insert(p, WallConnections::NONE);
    }
    for pair in path.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (ca, cb) = if b.1 < a.1 {
            (1, 4)
        } else if b.0 > a.0 {
            (2, 8)
        } else if b.1 > a.1 {
            (4, 1)
        } else {
            (8, 2)
        };
        connections.entry(a).or_default().0 |= ca;
        connections.entry(b).or_default().0 |= cb;
    }
    let mut nodes: Vec<_> = connections.into_iter().collect();
    nodes.sort_by_key(|&(p, _)| (p.1, p.0));
    let mut result = WallStrokePlan {
        succeeded: true,
        ..Default::default()
    };
    for ((x, z), conn) in nodes {
        if obstructed.as_mut().is_some_and(|f| f(x, z)) {
            result.unsupported_tiles.push((x, z));
            continue;
        }
        let y = height.as_mut().map_or(0., |f| f(x, z));
        let forced = options.forced_gate_locations.contains(&(x, z));
        let crossing = options.auto_gate_on_road_crossing && road.as_mut().is_some_and(|f| f(x, z));
        let gate = forced || crossing;
        let resolved = if gate {
            catalog.resolve_gate(options.style, conn)
        } else {
            catalog.resolve_component(options.style, conn, options.auto_corner_towers)
        };
        let (d, angle) = match resolved {
            Ok(d) => d,
            Err(e) => return fail(vec![(x, z)], e),
        };
        if gate {
            result.gate_passage_tiles.push((x, z));
        }
        if !gate || options.default_gate_state == GatePassabilityState::Closed {
            result.blocked_tiles.push((x, z));
        }
        result.footprint_tiles.push((x, z));
        let id = match next_id() {
            Ok(id) => id,
            Err(reason) => return fail(vec![(x, z)], &reason),
        };
        result.placements.push(FortificationPlacement {
            id,
            name_def: d.name_def.clone(),
            kind: d.kind,
            tile: (x, z),
            world_x: (x as f32 + 0.5) * 256.,
            world_y: y,
            world_z: (z as f32 + 0.5) * 256.,
            angle_deg: angle,
            team: options.team,
            gate_state: gate.then_some(options.default_gate_state),
            foundation_texture: if options.stamp_foundations {
                d.foundation_texture.clone()
            } else {
                None
            },
        });
    }
    if !result.unsupported_tiles.is_empty() {
        return fail(
            result.unsupported_tiles,
            "路徑中有位置與不可移動之大型物件衝突。",
        );
    }
    result
}
#[derive(Clone, Debug, PartialEq)]
pub struct PalisadePiece {
    pub name: String,
    pub x: f32,
    pub z: f32,
}
pub const PALISADE_SPACING: f32 = 64.;
pub fn plan_palisade_run(
    vertices: &[(f32, f32)],
    along_z: &str,
    along_x: &str,
    corner: &str,
) -> Vec<PalisadePiece> {
    let mut path: Vec<Point> = vec![];
    let add = |path: &mut Vec<Point>, p| {
        if path.last() != Some(&p) {
            path.push(p);
        }
    };
    for &(x, z) in vertices {
        let p = (
            (x / 64.).round_ties_even() as i32,
            (z / 64.).round_ties_even() as i32,
        );
        if let Some(&(lx, lz)) = path.last() {
            if lx != p.0 && lz != p.1 {
                add(&mut path, (p.0, lz));
            }
        }
        add(&mut path, p);
    }
    if path.len() < 2 {
        return vec![];
    }
    let axis = |a: Point, b: Point| a.0 == b.0;
    let mut pieces = vec![];
    let push = |out: &mut Vec<PalisadePiece>, name: &str, p: Point| {
        out.push(PalisadePiece {
            name: name.into(),
            x: p.0 as f32 * 64.,
            z: p.1 as f32 * 64.,
        })
    };
    for (i, &p) in path.iter().enumerate() {
        let is_corner = i > 0 && i + 1 < path.len() && axis(path[i - 1], p) != axis(p, path[i + 1]);
        let vertical = if i + 1 < path.len() {
            axis(p, path[i + 1])
        } else {
            axis(path[i - 1], p)
        };
        push(
            &mut pieces,
            if is_corner {
                corner
            } else if vertical {
                along_z
            } else {
                along_x
            },
            p,
        );
        if let Some(&next) = path.get(i + 1) {
            let dx = (next.0 - p.0).signum();
            let dz = (next.1 - p.1).signum();
            let steps = (next.0 - p.0).abs() + (next.1 - p.1).abs();
            for step in 1..steps {
                push(
                    &mut pieces,
                    if dz != 0 { along_z } else { along_x },
                    (p.0 + dx * step, p.1 + dz * step),
                );
            }
        }
    }
    pieces
}
fn collision_scale(grid: &[u8], size: usize, dimension: usize) -> Result<usize, &'static str> {
    if size == 0 || size.checked_mul(size) != Some(grid.len()) || size > i32::MAX as usize {
        return Err("collision grid dimensions do not match");
    }
    if dimension == 0 {
        return Err("map dimension must be positive");
    }
    Ok((size / dimension).max(1))
}
fn paint_collision(
    grid: &mut [u8],
    size: usize,
    scale: usize,
    p: &FortificationPlacement,
    state: Option<GatePassabilityState>,
    passage_only: bool,
    changes: &mut Vec<CollisionPixelChange>,
) {
    let gate = p.kind == WallComponentKind::Gate;
    let span = if !gate && p.kind == WallComponentKind::Tower {
        scale * 2
    } else {
        scale
    };
    for dz in 0..span {
        for dx in 0..span {
            let passage = dx >= scale / 4
                && dx < scale - scale / 4
                && dz >= scale / 4
                && dz < scale - scale / 4;
            if passage_only && !passage {
                continue;
            }
            let x = p.tile.0 as i64 * scale as i64 + dx as i64;
            let z = p.tile.1 as i64 * scale as i64 + dz as i64;
            if x < 0 || z < 0 || x >= size as i64 || z >= size as i64 {
                continue;
            }
            let index = z as usize * size + x as usize;
            let after = if gate && passage && state == Some(GatePassabilityState::Open) {
                0
            } else {
                255
            };
            let before = grid[index];
            if before != after {
                grid[index] = after;
                changes.push(CollisionPixelChange {
                    index,
                    before,
                    after,
                });
            }
        }
    }
}
pub fn synchronize_passability(
    grid: &mut [u8],
    size: usize,
    dimension: usize,
    plan: &WallStrokePlan,
) -> Result<Vec<CollisionPixelChange>, &'static str> {
    let scale = collision_scale(grid, size, dimension)?;
    let mut changes = vec![];
    for p in &plan.placements {
        paint_collision(grid, size, scale, p, p.gate_state, false, &mut changes);
    }
    Ok(changes)
}
pub fn set_gate_state(
    grid: &mut [u8],
    size: usize,
    dimension: usize,
    gate: &FortificationPlacement,
    state: GatePassabilityState,
) -> Result<Vec<CollisionPixelChange>, &'static str> {
    if gate.kind != WallComponentKind::Gate {
        return Err("component is not a gate");
    }
    let scale = collision_scale(grid, size, dimension)?;
    let mut changes = vec![];
    paint_collision(grid, size, scale, gate, Some(state), true, &mut changes);
    Ok(changes)
}
/// Replay forward in write order; undo in reverse order to preserve overlapping footprints.
pub fn replay_collision_changes(
    grid: &mut [u8],
    changes: &[CollisionPixelChange],
    undo: bool,
) -> Result<(), &'static str> {
    if changes.iter().any(|c| c.index >= grid.len()) {
        return Err("collision change index out of bounds");
    }
    if undo {
        for c in changes.iter().rev() {
            grid[c.index] = c.before;
        }
    } else {
        for c in changes {
            grid[c.index] = c.after;
        }
    }
    Ok(())
}
/// Build navigation's immutable snapshot after synchronization, preserving optional height/water inputs.
pub fn passability_grid(
    size: usize,
    collision: &[u8],
    height: Option<(usize, &[u8], f32, f32)>,
    world_dimension: f32,
) -> Result<PassabilityGrid, &'static str> {
    collision_scale(collision, size, 1)?;
    PassabilityGrid::new(size, Some(collision), height, world_dimension)
}
pub fn analyze_fortress_enclosure(
    dimension: i32,
    fortifications: &[FortificationPlacement],
    mut natural: Option<&mut dyn FnMut(i32, i32) -> bool>,
) -> FortressEnclosureReport {
    let mut report = FortressEnclosureReport {
        is_fully_enclosed: false,
        enclosed_area_tiles: 0,
        gaps: vec![],
        gates: vec![],
    };
    if dimension <= 0 {
        return report;
    }
    let size = dimension as usize;
    let mut blocked = vec![false; size * size];
    for p in fortifications {
        if p.kind != WallComponentKind::Gate || p.gate_state == Some(GatePassabilityState::Closed) {
            let (x, z) = p.tile;
            if x >= 0 && z >= 0 && x < dimension && z < dimension {
                blocked[z as usize * size + x as usize] = true;
            }
        }
    }
    if let Some(f) = natural.as_mut() {
        for z in 0..dimension {
            for x in 0..dimension {
                if f(x, z) {
                    blocked[z as usize * size + x as usize] = true;
                }
            }
        }
    }
    let mut seen = vec![false; blocked.len()];
    let mut queue = VecDeque::new();
    let enqueue = |x: usize, z: usize, seen: &mut Vec<bool>, q: &mut VecDeque<(usize, usize)>| {
        let i = z * size + x;
        if !blocked[i] && !seen[i] {
            seen[i] = true;
            q.push_back((x, z));
        }
    };
    for x in 0..size {
        enqueue(x, 0, &mut seen, &mut queue);
        enqueue(x, size - 1, &mut seen, &mut queue);
    }
    for z in 0..size {
        enqueue(0, z, &mut seen, &mut queue);
        enqueue(size - 1, z, &mut seen, &mut queue);
    }
    while let Some((x, z)) = queue.pop_front() {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let nx = x as i32 + dx;
            let nz = z as i32 + dz;
            if nx >= 0 && nz >= 0 && nx < dimension && nz < dimension {
                enqueue(nx as usize, nz as usize, &mut seen, &mut queue);
            }
        }
    }
    report.enclosed_area_tiles = blocked
        .iter()
        .zip(seen)
        .filter(|(b, s)| !**b && !*s)
        .count();
    report.is_fully_enclosed = report.enclosed_area_tiles > 0;
    report.gates = fortifications
        .iter()
        .filter(|p| p.kind == WallComponentKind::Gate)
        .cloned()
        .collect();
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persistent_ids_are_nonzero_and_distinct_across_plans() {
        let first = plan(&[(1, 1)], &WallStrokeOptions::default());
        let second = plan(&[(1, 1)], &WallStrokeOptions::default());
        assert!(first.succeeded && second.succeeded);
        assert_ne!(first.placements[0].id, [0; 16]);
        assert_ne!(first.placements[0].id, second.placements[0].id);
        #[cfg(not(windows))]
        {
            assert_eq!(first.placements[0].id[6] >> 4, 4);
            assert_eq!(first.placements[0].id[8] >> 6, 2);
        }
    }
    #[test]
    fn guid_failure_discards_partially_planned_objects() {
        let mut calls = 0;
        let failed = plan_wall_stroke_with_ids(
            8,
            &[(1, 1), (2, 1)],
            &WallStrokeOptions::default(),
            &WallTileCatalog::default(),
            None,
            None,
            None,
            || {
                calls += 1;
                if calls == 1 {
                    Ok([1; 16])
                } else {
                    Err("CoCreateGuid failed: HRESULT 0x80004005".into())
                }
            },
        );
        assert!(!failed.succeeded);
        assert!(failed.placements.is_empty());
        assert!(failed.footprint_tiles.is_empty());
        assert!(failed.blocked_tiles.is_empty());
        assert_eq!(failed.unsupported_tiles, vec![(2, 1)]);
        assert_eq!(
            failed.failure_reason.as_deref(),
            Some("CoCreateGuid failed: HRESULT 0x80004005")
        );
    }
    fn plan(path: &[Point], options: &WallStrokeOptions) -> WallStrokePlan {
        plan_wall_stroke(
            8,
            path,
            options,
            &WallTileCatalog::default(),
            None,
            None,
            None,
        )
    }
    #[test]
    fn topology_rotations_and_filtered_fallbacks() {
        use WallComponentKind::*;
        let c = WallTileCatalog::default();
        let style = FortificationStyle::RomanStoneWall;
        for (mask, angle) in [(3, 0.), (6, 90.), (12, 180.), (9, 270.)] {
            let (d, a) = c
                .resolve_component(style, WallConnections(mask), false)
                .unwrap();
            assert_eq!(d.kind, Corner);
            assert_eq!(a, angle);
        }
        assert_eq!(
            c.resolve_component(style, WallConnections(3), true)
                .unwrap()
                .0
                .kind,
            Tower
        );
        for (mask, angle) in [(1, 180.), (2, 270.), (4, 0.), (8, 90.)] {
            assert_eq!(
                c.resolve_component(style, WallConnections(mask), true)
                    .unwrap()
                    .1,
                angle
            );
        }
        assert_eq!(c.resolve_gate(style, WallConnections(5)).unwrap().1, 90.);
        let filtered = c.filtered(|n| n == "BauRomMau00_Mauer");
        assert_eq!(
            filtered
                .resolve_component(style, WallConnections(3), true)
                .unwrap()
                .0
                .kind,
            Straight
        );
        assert!(filtered
            .resolve_component(FortificationStyle::HunBarricade, WallConnections(0), true)
            .is_err());
        assert_eq!(c.components(FortificationStyle::CelticPalisade).len(), 4);
    }
    #[test]
    fn custom_junctions_and_matching() {
        let mut c = WallTileCatalog::empty();
        let style = FortificationStyle::RomanStoneWall;
        let mut d = WallTileCatalog::default().components(style)[0].clone();
        d.kind = WallComponentKind::TJunction;
        d.supported_connections = WallConnections::ALL;
        assert!(d.matches(WallComponentKind::TJunction, WallConnections(11)));
        c.register(style, d);
        for (mask, angle) in [(7, 0.), (14, 90.), (13, 180.), (11, 270.)] {
            assert_eq!(
                c.resolve_component(style, WallConnections(mask), true)
                    .unwrap()
                    .1,
                angle
            );
        }
    }
    #[test]
    fn stroke_discretizes_and_callbacks_preserve_order() {
        assert_eq!(
            discretize_orthogonal_path(&[(1, 1), (3, 3)]),
            vec![(1, 1), (2, 1), (3, 1), (3, 2), (3, 3)]
        );
        let mut h = |x, z| (x + z) as f32;
        let mut road = |x, z| x == 2 && z == 1;
        let mut visited = vec![];
        let mut obstacle = |x, z| {
            visited.push((x, z));
            false
        };
        let options = WallStrokeOptions {
            team: 7,
            ..Default::default()
        };
        let p = plan_wall_stroke(
            8,
            &[(1, 1), (3, 3)],
            &options,
            &WallTileCatalog::default(),
            Some(&mut h),
            Some(&mut road),
            Some(&mut obstacle),
        );
        assert!(p.succeeded);
        assert_eq!(p.footprint_tiles, visited);
        assert_eq!(p.gate_passage_tiles, vec![(2, 1)]);
        assert!(!p.blocked_tiles.contains(&(2, 1)));
        assert_eq!(p.placements[1].world_x, 640.);
        assert_eq!(p.placements[1].world_y, 3.);
        assert_eq!(p.placements[1].team, 7);
        assert_eq!(p.placements[2].kind, WallComponentKind::Tower);
        let mut obstacle = |x, _| x == 2;
        let failed = plan_wall_stroke(
            8,
            &[(1, 1), (3, 1)],
            &options,
            &WallTileCatalog::default(),
            None,
            None,
            Some(&mut obstacle),
        );
        assert!(!failed.succeeded);
        assert!(failed.placements.is_empty());
        assert_eq!(failed.unsupported_tiles, vec![(2, 1)]);
        assert!(!plan(&[(-1, 0)], &options).succeeded);
        assert!(plan(&[], &options).succeeded);
    }
    #[test]
    fn gate_raster_and_navigation_snapshot_undo() {
        let options = WallStrokeOptions {
            forced_gate_locations: vec![(2, 2)],
            ..Default::default()
        };
        let p = plan(&[(2, 2)], &options);
        let mut grid = vec![37; 32 * 32];
        let original = grid.clone();
        let changes = synchronize_passability(&mut grid, 32, 8, &p).unwrap();
        assert_eq!(grid[8 * 32 + 8], 255);
        assert_eq!(grid[9 * 32 + 9], 0);
        assert_eq!(changes.len(), 16);
        let nav = passability_grid(32, &grid, None, 8192.).unwrap();
        assert!(!nav.passable(8, 8));
        assert!(nav.passable(9, 9));
        let gate = &p.placements[0];
        let closed = set_gate_state(&mut grid, 32, 8, gate, GatePassabilityState::Closed).unwrap();
        assert_eq!(closed.len(), 4);
        assert!(closed.iter().all(|c| c.before == 0 && c.after == 255));
        let reopened = set_gate_state(&mut grid, 32, 8, gate, GatePassabilityState::Open).unwrap();
        assert_eq!(reopened.len(), 4);
        assert!(reopened.iter().all(|c| c.before == 255 && c.after == 0));
        assert_eq!(grid[8 * 32 + 8], 255);
        replay_collision_changes(&mut grid, &reopened, true).unwrap();
        replay_collision_changes(&mut grid, &closed, true).unwrap();
        replay_collision_changes(&mut grid, &changes, true).unwrap();
        assert_eq!(grid, original);
        replay_collision_changes(&mut grid, &changes, false).unwrap();
        assert_eq!(grid[9 * 32 + 9], 0);
        assert!(set_gate_state(&mut grid, 32, 0, gate, GatePassabilityState::Open).is_err());
    }
    #[test]
    fn overlapping_writes_undo_reverse_and_tower_clips() {
        let mut p = plan(&[(1, 1)], &WallStrokeOptions::default());
        p.placements[0].kind = WallComponentKind::Tower;
        let mut gate = p.placements[0].clone();
        gate.kind = WallComponentKind::Gate;
        gate.gate_state = Some(GatePassabilityState::Open);
        p.placements.push(gate);
        let mut grid = vec![0; 8 * 8];
        let changes = synchronize_passability(&mut grid, 8, 4, &p).unwrap();
        assert_eq!(grid.iter().filter(|&&v| v != 0).count(), 12);
        replay_collision_changes(&mut grid, &changes, true).unwrap();
        assert!(grid.iter().all(|&v| v == 0));
        p.placements.truncate(1);
        p.placements[0].tile = (3, 3);
        let changes = synchronize_passability(&mut grid, 8, 4, &p).unwrap();
        assert_eq!(changes.len(), 4);
        let before = grid.clone();
        assert!(replay_collision_changes(
            &mut grid,
            &[CollisionPixelChange {
                index: 64,
                before: 0,
                after: 1
            }],
            false
        )
        .is_err());
        assert_eq!(before, grid);
    }
    #[test]
    fn enclosure_open_gate_closed_gate_and_natural_barrier() {
        let options = WallStrokeOptions {
            forced_gate_locations: vec![(2, 1)],
            default_gate_state: GatePassabilityState::Closed,
            ..Default::default()
        };
        let mut p = plan(&[(1, 1), (3, 1), (3, 3), (1, 3), (1, 1)], &options);
        let r = analyze_fortress_enclosure(5, &p.placements, None);
        assert!(r.is_fully_enclosed);
        assert_eq!(r.enclosed_area_tiles, 1);
        assert_eq!(r.gates.len(), 1);
        assert!(r.gaps.is_empty());
        p.placements
            .iter_mut()
            .find(|p| p.kind == WallComponentKind::Gate)
            .unwrap()
            .gate_state = Some(GatePassabilityState::Open);
        assert!(!analyze_fortress_enclosure(5, &p.placements, None).is_fully_enclosed);
        let mut natural = |x, z| (x, z) == (2, 1);
        assert_eq!(
            analyze_fortress_enclosure(5, &p.placements, Some(&mut natural)).enclosed_area_tiles,
            1
        );
        assert!(!analyze_fortress_enclosure(0, &p.placements, None).is_fully_enclosed);
    }
    #[test]
    fn palisades_snap_ties_even_and_insert_exact_bends() {
        let p = plan_palisade_run(&[(32., 32.), (160., 160.)], "Z", "X", "C");
        assert_eq!(
            p.iter()
                .map(|p| (p.name.as_str(), p.x, p.z))
                .collect::<Vec<_>>(),
            vec![
                ("X", 0., 0.),
                ("X", 64., 0.),
                ("C", 128., 0.),
                ("Z", 128., 64.),
                ("Z", 128., 128.)
            ]
        );
        assert!(plan_palisade_run(&[(0., 0.), (0., 0.)], "Z", "X", "C").is_empty());
        let p = plan_palisade_run(&[(0., 0.), (0., 128.), (128., 128.)], "Z", "X", "C");
        assert_eq!(p[2].name, "C");
        assert_eq!(p.len(), 5);
    }
}
