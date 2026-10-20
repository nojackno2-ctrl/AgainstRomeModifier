//! Native road stroke planner (port of C# `RoadStrokePlanner`, `RoadTileCatalog` and
//! `TerrainBlendEditSession.PaintRoadPath`).
//!
//! The planner is pure: it computes which native road tiles a dragged path needs without touching
//! the map. Missing turns/junctions reject the *whole* plan so a stroke is never half-written.

use crate::error::TextureError;
use crate::session::{TerrainBlendEditSession, TerrainTextureChange};
use std::collections::{HashMap, HashSet};

/// Bit set of open road edges of a tile (C# `RoadConnections`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct RoadConnections(pub u8);

impl RoadConnections {
    pub const NONE: Self = Self(0);
    pub const NORTH: Self = Self(1);
    pub const EAST: Self = Self(2);
    pub const SOUTH: Self = Self(4);
    pub const WEST: Self = Self(8);

    pub fn contains_all(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
    fn is_endpoint(self) -> bool {
        matches!(self.0, 1 | 2 | 4 | 8)
    }
    fn is_straight(self) -> bool {
        self == Self::NORTH | Self::SOUTH || self == Self::EAST | Self::WEST
    }
}

impl std::ops::BitOr for RoadConnections {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoadTile {
    pub texture: String,
    pub connections: RoadConnections,
}

impl RoadTile {
    pub fn new(texture: impl Into<String>, connections: RoadConnections) -> Self {
        Self {
            texture: texture.into(),
            connections,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoadTilePlacement {
    pub x: i32,
    pub y: i32,
    pub texture: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoadStrokePlan {
    pub succeeded: bool,
    pub tiles: Vec<RoadTilePlacement>,
    pub unsupported: Vec<(i32, i32)>,
}

/// Outcome of [`TerrainBlendEditSession::paint_road_path`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrainRoadPaintResult {
    pub succeeded: bool,
    pub texture_changes: Vec<TerrainTextureChange>,
    pub unsupported: Vec<(i32, i32)>,
}

const NEIGHBORS: [(i32, i32, RoadConnections, RoadConnections); 4] = [
    (0, -1, RoadConnections::NORTH, RoadConnections::SOUTH),
    (1, 0, RoadConnections::EAST, RoadConnections::WEST),
    (0, 1, RoadConnections::SOUTH, RoadConnections::NORTH),
    (-1, 0, RoadConnections::WEST, RoadConnections::EAST),
];

/// Bresenham line between two tiles: excludes the start, includes the end
/// (C# `TerrainStrokePath.Between`).
pub fn stroke_path_between(from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
    let dx = (to.0 - from.0).abs();
    let dy = -(to.1 - from.1).abs();
    let sx = if from.0 < to.0 { 1 } else { -1 };
    let sy = if from.1 < to.1 { 1 } else { -1 };
    let mut error = dx + dy;
    let (mut x, mut y) = from;
    let mut points = Vec::new();
    while (x, y) != to {
        let twice = 2 * error;
        if twice >= dy {
            error += dy;
            x += sx;
        }
        if twice <= dx {
            error += dx;
            y += sy;
        }
        points.push((x, y));
    }
    points
}

/// Like [`stroke_path_between`] but every step is edge-connected (diagonals get an extra corner).
pub fn orthogonal_between(start: (i32, i32), end: (i32, i32)) -> Vec<(i32, i32)> {
    let mut previous = start;
    let mut out = Vec::new();
    for next in stroke_path_between(start, end) {
        if next.0 != previous.0 && next.1 != previous.1 {
            out.push((next.0, previous.1));
        }
        out.push(next);
        previous = next;
    }
    out
}

/// Plans connected native road tiles without changing the map.
pub fn plan_road_stroke(
    dimension: usize,
    textures: &[String],
    path: &[(i32, i32)],
    available: &[RoadTile],
    preferred_texture: &str,
) -> Result<RoadStrokePlan, TextureError> {
    if dimension == 0 || textures.len() != dimension * dimension {
        return Err(TextureError::DimensionMismatch {
            expected: dimension * dimension,
            actual: textures.len(),
        });
    }
    let dim = dimension as i32;
    let out_of_bounds: Vec<(i32, i32)> = path
        .iter()
        .copied()
        .filter(|p| p.0 < 0 || p.1 < 0 || p.0 >= dim || p.1 >= dim)
        .collect();
    if !out_of_bounds.is_empty() {
        return Ok(RoadStrokePlan {
            succeeded: false,
            tiles: Vec::new(),
            unsupported: out_of_bounds,
        });
    }

    // Union of connections per texture name (case-insensitive).
    let mut by_name: HashMap<String, RoadConnections> = HashMap::new();
    for tile in available {
        let entry = by_name.entry(tile.texture.to_ascii_uppercase()).or_default();
        *entry = *entry | tile.connections;
    }

    let mut links: HashMap<(i32, i32), RoadConnections> = HashMap::new();
    fn add(links: &mut HashMap<(i32, i32), RoadConnections>, p: (i32, i32), c: RoadConnections) {
        let entry = links.entry(p).or_default();
        *entry = *entry | c;
    }
    if let Some(&first) = path.first() {
        add(&mut links, first, RoadConnections::NONE);
    }
    // Intended links are added separately so crossings and backtracking preserve every segment.
    for i in 1..path.len() {
        let mut previous = path[i - 1];
        for next in orthogonal_between(previous, path[i]) {
            let direction = NEIGHBORS
                .iter()
                .find(|n| n.0 == next.0 - previous.0 && n.1 == next.1 - previous.1)
                .expect("orthogonal step");
            add(&mut links, previous, direction.2);
            add(&mut links, next, direction.3);
            previous = next;
        }
    }
    // Connect to already-existing neighbouring road tiles that point back at us.
    let keys: Vec<(i32, i32)> = links.keys().copied().collect();
    for p in keys {
        let Some(&current) = by_name.get(&textures[p.1 as usize * dimension + p.0 as usize].to_ascii_uppercase()) else {
            continue;
        };
        for neighbor in NEIGHBORS {
            let (x, y) = (p.0 + neighbor.0, p.1 + neighbor.1);
            if x < 0 || y < 0 || x >= dim || y >= dim {
                continue;
            }
            if current.contains_all(neighbor.2) && (current.0 & neighbor.2 .0) != 0 {
                if let Some(&adjacent) =
                    by_name.get(&textures[y as usize * dimension + x as usize].to_ascii_uppercase())
                {
                    if adjacent.0 & neighbor.3 .0 != 0 {
                        add(&mut links, p, neighbor.2);
                    }
                }
            }
        }
    }

    let preferred_upper = preferred_texture.to_ascii_uppercase();
    let mut ordered: Vec<((i32, i32), RoadConnections)> = links.into_iter().collect();
    ordered.sort_by_key(|(p, _)| (p.1, p.0));
    let mut placements = Vec::new();
    let mut unsupported = Vec::new();
    for (p, required) in ordered {
        // Endpoints may use a straight tile with one open end; turns/junctions need an exact piece.
        let mut candidates: Vec<&RoadTile> = available
            .iter()
            .filter(|t| {
                t.connections == required
                    || (required.is_endpoint()
                        && t.connections.contains_all(required)
                        && t.connections.is_straight())
                    || (required == RoadConnections::NONE
                        && t.texture.eq_ignore_ascii_case(preferred_texture))
            })
            .collect();
        candidates.sort_by_key(|t| {
            (
                if t.connections == required { 0 } else { 1 },
                if t.texture.to_ascii_uppercase() == preferred_upper { 0 } else { 1 },
                t.texture.to_ascii_uppercase(),
            )
        });
        match candidates.first() {
            None => unsupported.push(p),
            Some(tile) => placements.push(RoadTilePlacement {
                x: p.0,
                y: p.1,
                texture: tile.texture.clone(),
            }),
        }
    }
    if unsupported.is_empty() {
        Ok(RoadStrokePlan {
            succeeded: true,
            tiles: placements,
            unsupported,
        })
    } else {
        Ok(RoadStrokePlan {
            succeeded: false,
            tiles: Vec::new(),
            unsupported,
        })
    }
}

const STANDARD_HORIZONTAL: [&str; 5] = ["H_WEG1", "H_WEG2", "H_WEG3", "H_WEG4", "H_WEG5"];
const STANDARD_VERTICAL: [&str; 3] = ["V_WEG1", "V_WEG2", "V_WEG3"];
const STANDARD_JUNCTIONS: [&str; 3] = ["weg1", "weg2", "weg3"];
const ROMAN_HORIZONTAL: [&str; 2] = ["WEG_H1ROM", "WEG_H2ROM"];
const ROMAN_VERTICAL: [&str; 4] = ["WEG_V1ROM", "WEG_V2ROM", "WEG_V3ROM", "WEG_V4ROM"];
const ROMAN_JUNCTIONS: [&str; 6] = [
    "Pflaster_braun1",
    "Pflaster_braun2",
    "Pflaster_braun3",
    "weg1",
    "weg2",
    "weg3",
];

/// Native road pieces (`Steinweg` and `Roemerstrasse`) mapped to connection masks
/// (C# `RoadTileCatalog.BuildAvailable`). `existing` is the set of texture names that exist
/// (floortex library); `None` means every piece is assumed present.
pub fn build_available_road_tiles(
    existing: Option<&[String]>,
    preferred_texture: Option<&str>,
) -> Vec<RoadTile> {
    let existing: Option<HashSet<String>> =
        existing.map(|names| names.iter().map(|n| n.to_ascii_uppercase()).collect());
    let is_available = |name: &str| {
        existing
            .as_ref()
            .map_or(true, |set| set.contains(&name.to_ascii_uppercase()))
    };
    let prefer_roman = preferred_texture.is_some_and(|t| {
        t.to_ascii_uppercase().contains("ROM") || t.to_ascii_uppercase().starts_with("PFLASTER")
    });

    let has_roman_h = ROMAN_HORIZONTAL.iter().any(|n| is_available(n));
    let has_roman_v = ROMAN_VERTICAL.iter().any(|n| is_available(n));
    let has_std_h = STANDARD_HORIZONTAL.iter().any(|n| is_available(n));
    let has_std_v = STANDARD_VERTICAL.iter().any(|n| is_available(n));
    let both = |a: &[&'static str], b: &[&'static str]| -> Vec<&'static str> {
        a.iter().chain(b.iter()).copied().collect()
    };

    let horizontal: Vec<&str> = if prefer_roman && has_roman_h {
        ROMAN_HORIZONTAL.to_vec()
    } else if !prefer_roman && has_std_h {
        STANDARD_HORIZONTAL.to_vec()
    } else {
        both(&STANDARD_HORIZONTAL, &ROMAN_HORIZONTAL)
    };
    let vertical: Vec<&str> = if prefer_roman && has_roman_v {
        ROMAN_VERTICAL.to_vec()
    } else if !prefer_roman && has_std_v {
        STANDARD_VERTICAL.to_vec()
    } else {
        both(&STANDARD_VERTICAL, &ROMAN_VERTICAL)
    };
    let junctions: Vec<&str> = if prefer_roman {
        ROMAN_JUNCTIONS.to_vec()
    } else {
        both(&STANDARD_JUNCTIONS, &ROMAN_JUNCTIONS)
    };

    let mut tiles = Vec::new();
    for h in horizontal.iter().filter(|n| is_available(n)) {
        tiles.push(RoadTile::new(*h, RoadConnections::EAST | RoadConnections::WEST));
    }
    for v in vertical.iter().filter(|n| is_available(n)) {
        tiles.push(RoadTile::new(*v, RoadConnections::NORTH | RoadConnections::SOUTH));
    }
    let available_junctions: Vec<&str> = junctions.iter().copied().filter(|n| is_available(n)).collect();
    if !available_junctions.is_empty() {
        for mask in 0..=15u8 {
            let conn = RoadConnections(mask);
            let pure_straight = conn.is_straight();
            let single_endpoint = conn.is_endpoint();
            if pure_straight || single_endpoint {
                let horizontal_family =
                    conn == RoadConnections::EAST | RoadConnections::WEST
                        || conn == RoadConnections::EAST
                        || conn == RoadConnections::WEST;
                let has_specific = if horizontal_family {
                    horizontal.iter().any(|n| is_available(n))
                } else {
                    vertical.iter().any(|n| is_available(n))
                };
                if has_specific {
                    continue;
                }
            }
            for j in &available_junctions {
                tiles.push(RoadTile::new(*j, conn));
            }
        }
    }
    if let Some(preferred) = preferred_texture.filter(|p| !p.is_empty()) {
        if is_available(preferred)
            && !tiles.iter().any(|t| {
                t.connections == RoadConnections::NONE && t.texture.eq_ignore_ascii_case(preferred)
            })
        {
            tiles.push(RoadTile::new(preferred, RoadConnections::NONE));
        }
    }
    tiles
}

impl TerrainBlendEditSession {
    /// Paints one connected road stroke using native pieces. A rejected plan restores the complete
    /// pending stroke (C# `PaintRoadPath`); committed history is untouched.
    pub fn paint_road_path(
        &mut self,
        path: &[(i32, i32)],
        available: &[RoadTile],
        preferred_texture: &str,
    ) -> Result<TerrainRoadPaintResult, TextureError> {
        let plan = plan_road_stroke(
            self.tile_dimension(),
            self.current_textures(),
            path,
            available,
            preferred_texture,
        )?;
        if !plan.succeeded {
            return Ok(TerrainRoadPaintResult {
                succeeded: false,
                texture_changes: self.cancel_stroke(),
                unsupported: plan.unsupported,
            });
        }
        let mut changes = Vec::new();
        for tile in &plan.tiles {
            if let Some(change) = self.stamp_texture(tile.x as usize, tile.y as usize, &tile.texture) {
                changes.push(change);
            }
        }
        Ok(TerrainRoadPaintResult {
            succeeded: true,
            texture_changes: changes,
            unsupported: Vec::new(),
        })
    }
}
