//! Formation-width validation and route feasibility diagnostics, ported from
//! `FormationWidthValidator.cs`. Editor heuristics only; not the native game pathfinder.
//!
//! C# boundary semantics preserved here:
//! - troop counts are signed; `<= 0` needs no clearance and `<= 1` is never a corridor warning;
//! - clearance is Manhattan distance to a blocked cell and an obstacle-free grid is all `-1`,
//!   which still compares `< required` and is therefore reported (see [`RouteDiagnostic::no_obstacles`]);
//! - the choke on a route is the *first* cell with the minimum clearance, deduplicated per cell;
//! - out-of-bounds or impassable endpoints yield "no route" in C#; here the cause is also reported.
use crate::{choke, clearance_field, find_path, ChokePoint, PassabilityGrid};
use std::collections::HashSet;

/// Signed variant of [`crate::required_clearance`] matching `GetRequiredClearance(int)`.
pub fn required_clearance_signed(troops: i32) -> (i32, f32) {
    match troops {
        i32::MIN..=0 => (0, 0.0),
        1 => (1, 64.0),
        2..=5 => (2, 160.0),
        6..=10 => (4, 280.0),
        11..=15 => (5, 360.0),
        _ => (6, 440.0),
    }
}

/// One squad movement to validate; mirrors the `(Start, Target, TroopCount)` tuple.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormationTrip {
    pub start: (i32, i32),
    pub target: (i32, i32),
    pub troops: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndpointIssue {
    OutOfBounds,
    Collision,
    Submerged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteStatus {
    Reachable,
    /// Start and/or target is not passable (C# `FindPath` returns null).
    EndpointUnusable,
    /// Both endpoints are passable but no four-neighbor route joins them.
    Disconnected,
}

#[derive(Debug, PartialEq)]
pub struct RouteDiagnostic {
    pub trip: FormationTrip,
    pub status: RouteStatus,
    pub start_issue: Option<EndpointIssue>,
    pub target_issue: Option<EndpointIssue>,
    /// Number of cells on the shortest route (0 unless reachable).
    pub path_len: usize,
    /// Narrowest cell on the route and its clearance (first minimum, as in C#).
    pub narrowest: Option<((i32, i32), i32)>,
    /// True when the formation rules were applied (reachable and `troops > 1`).
    pub formation_evaluated: bool,
    pub required_clearance: i32,
    pub required_width: f32,
    /// True when the grid has no obstacle at all, so clearance is the `-1` sentinel.
    pub no_obstacles: bool,
    /// Reported choke (same entry `evaluate_corridors` would emit, before cross-trip deduplication).
    pub choke: Option<ChokePoint>,
}

impl RouteDiagnostic {
    /// `Some(false)` when the formation cannot pass, `None` when not evaluated.
    pub fn fits(&self) -> Option<bool> {
        self.formation_evaluated.then_some(self.choke.is_none())
    }
}

#[derive(Debug, PartialEq)]
pub struct FormationReport {
    pub routes: Vec<RouteDiagnostic>,
    /// Corridor chokes, deduplicated across trips by cell (== `EvaluateCorridors`).
    pub corridor_chokes: Vec<ChokePoint>,
    /// Topological fallback, filled only when no corridor choke exists and squads exist,
    /// as `MapDiagnostics` does.
    pub topological_chokes: Vec<ChokePoint>,
}

fn endpoint_issue(grid: &PassabilityGrid, p: (i32, i32)) -> Option<EndpointIssue> {
    if !grid.in_bounds(p.0, p.1) {
        Some(EndpointIssue::OutOfBounds)
    } else if grid.collision_blocked(p.0, p.1) {
        Some(EndpointIssue::Collision)
    } else if grid.submerged(p.0, p.1) {
        Some(EndpointIssue::Submerged)
    } else {
        None
    }
}

fn diagnose_with_field(
    grid: &PassabilityGrid,
    field: &[i32],
    no_obstacles: bool,
    trip: FormationTrip,
) -> RouteDiagnostic {
    let (required_clearance, required_width) = required_clearance_signed(trip.troops);
    let start_issue = endpoint_issue(grid, trip.start);
    let target_issue = endpoint_issue(grid, trip.target);
    let mut d = RouteDiagnostic {
        trip,
        status: RouteStatus::EndpointUnusable,
        start_issue,
        target_issue,
        path_len: 0,
        narrowest: None,
        formation_evaluated: false,
        required_clearance,
        required_width,
        no_obstacles,
        choke: None,
    };
    if start_issue.is_some() || target_issue.is_some() {
        return d;
    }
    let Some(path) = find_path(grid, trip.start, trip.target) else {
        d.status = RouteStatus::Disconnected;
        return d;
    };
    d.status = RouteStatus::Reachable;
    d.path_len = path.len();
    let at = |&(x, z): &(i32, i32)| field[z as usize * grid.size + x as usize];
    // `min_by_key` keeps the first minimum, matching the strict `<` scan in C#.
    d.narrowest = path.iter().min_by_key(|p| at(p)).map(|p| (*p, at(p)));
    d.formation_evaluated = trip.troops > 1;
    if let (true, Some(((x, z), c))) = (d.formation_evaluated, d.narrowest) {
        if c < required_clearance {
            d.choke = Some(choke(
                grid,
                x,
                z,
                c,
                trip.troops as usize,
                "TravelPathChoke",
            ));
        }
    }
    d
}

/// Diagnose one trip (computes its own clearance field).
pub fn diagnose_route(grid: &PassabilityGrid, trip: FormationTrip) -> RouteDiagnostic {
    let field = clearance_field(grid);
    let none = field.iter().all(|&c| c == -1);
    diagnose_with_field(grid, &field, none, trip)
}

/// Diagnose many trips sharing one clearance field. Output order equals input order.
pub fn diagnose_routes(grid: &PassabilityGrid, trips: &[FormationTrip]) -> Vec<RouteDiagnostic> {
    let field = clearance_field(grid);
    let none = field.iter().all(|&c| c == -1);
    trips
        .iter()
        .map(|&t| diagnose_with_field(grid, &field, none, t))
        .collect()
}

/// Signed-count equivalent of [`crate::evaluate_corridors`].
pub fn evaluate_corridors_signed(
    grid: &PassabilityGrid,
    trips: &[FormationTrip],
) -> Vec<ChokePoint> {
    dedup_chokes(diagnose_routes(grid, trips).iter())
}

fn dedup_chokes<'a>(routes: impl Iterator<Item = &'a RouteDiagnostic>) -> Vec<ChokePoint> {
    let mut seen = HashSet::new();
    routes
        .filter_map(|r| r.choke.as_ref().zip(r.narrowest))
        .filter(|(_, (cell, _))| seen.insert(*cell))
        .map(|(c, _)| c.clone())
        .collect()
}

/// Signed-count equivalent of [`crate::topological_chokes`]; counts `<= 0` yield none.
pub fn topological_chokes_signed(grid: &PassabilityGrid, min_troop_check: i32) -> Vec<ChokePoint> {
    let required = required_clearance_signed(min_troop_check).0;
    if required <= 0 {
        return vec![];
    }
    let field = clearance_field(grid);
    let mut result = vec![];
    for z in 1..grid.size as i32 - 1 {
        for x in 1..grid.size as i32 - 1 {
            let c = field[z as usize * grid.size + x as usize];
            if grid.passable(x, z) && c > 0 && c < required {
                let horizontal = !grid.passable(x - 1, z) && !grid.passable(x + 1, z);
                let vertical = !grid.passable(x, z - 1) && !grid.passable(x, z + 1);
                if horizontal || vertical {
                    let kind = if horizontal {
                        "HorizontalChasm"
                    } else {
                        "VerticalChasm"
                    };
                    result.push(choke(grid, x, z, c, min_troop_check as usize, kind));
                }
            }
        }
    }
    result
}

/// Full designer report: per-trip diagnostics, corridor chokes, and the topological fallback
/// (used only when there is no corridor choke and `squad_max_troops > 0`, like `MapDiagnostics`).
pub fn analyze_formation(
    grid: &PassabilityGrid,
    trips: &[FormationTrip],
    squad_max_troops: i32,
) -> FormationReport {
    let routes = diagnose_routes(grid, trips);
    let corridor_chokes = dedup_chokes(routes.iter());
    let topological_chokes = if corridor_chokes.is_empty() && squad_max_troops > 0 {
        topological_chokes_signed(grid, squad_max_troops)
    } else {
        vec![]
    };
    FormationReport {
        routes,
        corridor_chokes,
        topological_chokes,
    }
}
