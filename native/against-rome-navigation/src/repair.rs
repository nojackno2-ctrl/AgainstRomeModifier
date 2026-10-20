//! Repair actions with stable ids, focus and bilingual text, ported from
//! `RoadPathHealer.CreateRepairActions` / `CreateBridgeAction`. Pure data: the caller owns
//! transactional application and undo.
use crate::{bridge_repair, Coordinate, IsolatedRegion, RoadGap, TileChange};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssueKind {
    IsolatedLand,
    RoadGap,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RepairAction {
    pub id: String,
    pub kind: IssueKind,
    pub title_zh: String,
    pub title_en: String,
    pub description_zh: String,
    pub description_en: String,
    pub changes: Vec<TileChange>,
    pub focus: Coordinate,
}

/// One action per road gap. Ids are `road-heal-{1-based index}-{x}-{z}` of the first gap tile.
pub fn road_heal_actions(gaps: &[RoadGap]) -> Vec<RepairAction> {
    gaps.iter()
        .enumerate()
        .map(|(i, g)| {
            // Gaps always have at least one tile; C# falls back to the start point regardless.
            let mid = g.tiles.first().copied().unwrap_or(g.start);
            let collision = g.clear_collision.then_some(0);
            RepairAction {
                id: format!("road-heal-{}-{}-{}", i + 1, mid.x, mid.z),
                kind: IssueKind::RoadGap,
                title_zh: format!("自動補齊道路間隙（{} 格，{} 風格）", g.tiles.len(), g.style),
                title_en: format!("Heal Road Gap ({} tiles, {} style)", g.tiles.len(), g.style),
                description_zh: format!(
                    "在座標 ({}, {}) 填補圖塊「{}」{}",
                    mid.x,
                    mid.z,
                    g.texture,
                    if g.clear_collision {
                        " 並清除碰撞硬閘阻擋。"
                    } else {
                        "。"
                    }
                ),
                description_en: format!(
                    "Place tile '{}' at ({}, {}){}",
                    g.texture,
                    mid.x,
                    mid.z,
                    if g.clear_collision {
                        " and clear collision blockage."
                    } else {
                        "."
                    }
                ),
                changes: g
                    .tiles
                    .iter()
                    .map(|p| TileChange {
                        x: p.x,
                        z: p.z,
                        texture: g.texture.into(),
                        collision,
                    })
                    .collect(),
                focus: mid,
            }
        })
        .collect()
}

/// Combines road heal actions and isolated region bridge actions into a single recommended repair list,
/// matching C# `RoadPathHealer` / `MapDiagnostics.AnalyzeNavMesh`.
pub fn recommended_repairs(
    gaps: &[RoadGap],
    isolated: &[IsolatedRegion],
    bridge_texture: &str,
    dimension: usize,
) -> Vec<RepairAction> {
    let mut repairs = road_heal_actions(gaps);
    for region in isolated {
        if region.bridge.is_some() {
            if let Some(bridge) = bridge_action(region, bridge_texture, dimension) {
                repairs.push(bridge);
            }
        }
    }
    repairs
}


/// Editor session surface needed to apply a [`RepairAction`]; mirrors the C#
/// `TerrainBlendEditSession` / `TerrainHeightEditSession` calls used by `ApplyRepair`.
/// Implemented by the caller's session (not in this crate); strokes must be undoable.
pub trait RepairSession {
    /// Tiles per side of the blend layer; `0` falls back to 64.
    fn tile_dimension(&self) -> usize;
    /// Stamp a texture; returns whether anything changed.
    fn stamp_texture(&mut self, x: i32, z: i32, texture: &str) -> bool;
    fn commit_texture_stroke(&mut self);
    fn undo_texture_stroke(&mut self);
    /// Collision pixels per side; `0` means the session has no collision layer.
    fn collision_size(&self) -> usize;
    /// Clear (`clear`) or block one collision pixel; returns whether it changed.
    fn paint_collision(&mut self, px: usize, pz: usize, clear: bool) -> bool;
    fn commit_collision_stroke(&mut self);
    fn undo_collision_stroke(&mut self);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepairError {
    /// A change addresses a tile outside the blend grid; nothing was modified.
    TileOutOfRange { x: i32, z: i32 },
}

/// Which strokes an applied repair committed; hand it back to [`undo_repair`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RepairOutcome {
    pub texture_committed: bool,
    pub collision_committed: bool,
}

impl RepairOutcome {
    pub fn changed(&self) -> bool {
        self.texture_committed || self.collision_committed
    }
}

/// Apply an action like C# `ApplyRepair`. All tiles are validated first, so an invalid action
/// leaves the session untouched (stricter than C#, which would write unchecked indices).
pub fn apply_repair<S: RepairSession>(
    action: &RepairAction,
    session: &mut S,
) -> Result<RepairOutcome, RepairError> {
    let dim = match session.tile_dimension() {
        0 => 64,
        d => d,
    };
    if let Some(c) = action
        .changes
        .iter()
        .find(|c| c.x < 0 || c.z < 0 || c.x as usize >= dim || c.z as usize >= dim)
    {
        return Err(RepairError::TileOutOfRange { x: c.x, z: c.z });
    }
    let (mut texture, mut collision) = (false, false);
    for c in &action.changes {
        if !c.texture.trim().is_empty() && session.stamp_texture(c.x, c.z, &c.texture) {
            texture = true;
        }
        let size = session.collision_size();
        if let (Some(v), true) = (c.collision, size > 0) {
            let step = (size / dim).max(1);
            let (x0, z0) = (c.x as usize * step, c.z as usize * step);
            for pz in z0..((c.z as usize + 1) * step).min(size) {
                for px in x0..((c.x as usize + 1) * step).min(size) {
                    collision |= session.paint_collision(px, pz, v == 0);
                }
            }
        }
    }
    if texture {
        session.commit_texture_stroke();
    }
    if collision {
        session.commit_collision_stroke();
    }
    Ok(RepairOutcome {
        texture_committed: texture,
        collision_committed: collision,
    })
}

/// Undo exactly the strokes `apply_repair` committed (collision first, reverse of apply order).
pub fn undo_repair<S: RepairSession>(outcome: RepairOutcome, session: &mut S) {
    if outcome.collision_committed {
        session.undo_collision_stroke();
    }
    if outcome.texture_committed {
        session.undo_texture_stroke();
    }
}

/// Passage action for an isolated region; `None` when there is no bridge point or distance.
/// A zero `dimension` falls back to 64 like C#.
pub fn bridge_action(
    region: &IsolatedRegion,
    texture: &str,
    dimension: usize,
) -> Option<RepairAction> {
    let change = bridge_repair(region, texture, dimension)?;
    let dim = if dimension == 0 { 64 } else { dimension };
    let (x, z) = (change.x, change.z);
    Some(RepairAction {
        id: format!("bridge-{}-{}-{}", region.component_id, x, z),
        kind: IssueKind::IsolatedLand,
        title_zh: format!("打通孤立區域過渡通道（跨度 {} 格）", region.bridge_distance),
        title_en: format!("Open Passage to Isolated Region ({} tiles)", region.bridge_distance),
        description_zh: format!(
            "在座標 ({x}, {z}) 清除阻擋硬閘並鋪設過渡道路，連接包含 {} 格之孤立陸地。",
            region.tile_count
        ),
        description_en: format!(
            "Clear collision and place transition road at ({x}, {z}) to reconnect {} isolated tiles.",
            region.tile_count
        ),
        changes: vec![change],
        focus: Coordinate::from_tile(x, z, dim, 16384.0),
    })
}
