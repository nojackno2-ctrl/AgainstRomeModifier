//! Scenario route / formation-width findings for the map editor.
//!
//! Bridges `against-rome-navigation`'s `diagnose_routes` / `analyze_formation` into
//! `MapIssue`s so designers get actionable messages ("widen to N tiles", "reduce squad to M",
//! "this endpoint is blocked"). Editor heuristics only; not the native game pathfinder.
use crate::diagnostics::{MapCheckSnapshot, MapDiagnostics, MapIssue, MapIssueSeverity};
use against_rome_navigation::{
    analyze_formation, ChokePoint, EndpointIssue, FormationReport, FormationTrip,
    PassabilityGrid, RouteDiagnostic, RouteStatus,
};
use std::collections::HashSet;
use uuid::Uuid;

/// Kind reported by navigation for a choke found on a concrete travel route.
pub const TRAVEL_PATH_CHOKE: &str = "TravelPathChoke";

/// Largest collision layer the route checks accept (same bound as the C# diagnostics).
pub const MAX_ROUTE_GRID_SIZE: usize = 1024;

/// One squad movement to check, in world coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct MapRouteRequest {
    /// Human readable name used in messages, e.g. `"LEGION → ENEMY"`.
    pub label: String,
    pub start_id: Uuid,
    pub target_id: Uuid,
    pub start: (f32, f32),
    pub target: (f32, f32),
    /// Signed like the C# validator: `<= 1` troops never produce a width finding.
    pub troops: i32,
}

#[derive(Debug)]
pub struct MapRouteReport {
    /// Requests that had valid world points, in input order (parallel to `formation.routes`).
    pub checked: Vec<MapRouteRequest>,
    pub formation: FormationReport,
    /// Per-route findings plus invalid-request errors.
    pub route_issues: Vec<MapIssue>,
    /// Passage-width findings from the topological fallback (no specific route involved).
    pub topological_issues: Vec<MapIssue>,
}

impl MapRouteReport {
    pub fn empty() -> Self {
        Self {
            checked: vec![],
            formation: FormationReport {
                routes: vec![],
                corridor_chokes: vec![],
                topological_chokes: vec![],
            },
            route_issues: vec![],
            topological_issues: vec![],
        }
    }

    /// All findings, route-specific first.
    pub fn issues(&self) -> Vec<MapIssue> {
        let mut all = self.route_issues.clone();
        all.extend(self.topological_issues.iter().cloned());
        all
    }

    /// Corridor + topological chokes; empty when the grid has no obstacle at all.
    pub fn choke_points(&self) -> Vec<ChokePoint> {
        let mut all = self.formation.corridor_chokes.clone();
        all.extend(self.formation.topological_chokes.iter().cloned());
        all
    }
}

fn cause_text(issue: EndpointIssue) -> (&'static str, &'static str) {
    match issue {
        EndpointIssue::OutOfBounds => ("超出地圖", "outside the map"),
        EndpointIssue::Collision => ("位於阻擋區", "on blocked (collision) ground"),
        EndpointIssue::Submerged => ("位於水面下", "under water"),
    }
}

fn issue_at(
    severity: MapIssueSeverity,
    code: &str,
    chinese: String,
    english: String,
    object_id: Uuid,
    at: (f32, f32),
) -> MapIssue {
    let mut i = MapIssue::new(severity, code, chinese, english);
    i.object_id = object_id;
    if at.0.is_finite() && at.1.is_finite() {
        i.world_x = Some(at.0);
        i.world_z = Some(at.1);
    }
    i
}

/// Actionable message for a route whose narrowest cell cannot take the formation.
pub fn formation_choke_issue(label: &str, choke: &ChokePoint) -> MapIssue {
    let (zh_where, en_where) = if label.is_empty() {
        (String::new(), String::new())
    } else {
        (format!("路線「{label}」："), format!("Route '{label}': "))
    };
    let mut i = MapIssue::new(
        MapIssueSeverity::Warning,
        "route-formation-choke",
        format!(
            "{zh_where}最窄處淨寬 {} 格（約 {:.0} 單位），{} 人方陣需 {} 格（約 {:.0} 單位）。請將通道加寬至至少 {} 格，或把該隊人數降到 {} 人以內。",
            choke.clearance, choke.width, choke.troop_count, choke.required, choke.required_width, choke.required, choke.safe_units
        ),
        format!(
            "{en_where}narrowest point is {} tiles wide (~{:.0} units) but a {}-man formation needs {} tiles (~{:.0} units). Widen the passage to at least {} tiles or reduce the squad to {} or fewer.",
            choke.clearance, choke.width, choke.troop_count, choke.required, choke.required_width, choke.required, choke.safe_units
        ),
    );
    i.world_x = Some(choke.location.world_x);
    i.world_z = Some(choke.location.world_z);
    i
}

/// Passage-width finding without a route (topological fallback).
pub fn bottleneck_issue(choke: &ChokePoint) -> MapIssue {
    let mut i = MapIssue::new(
        MapIssueSeverity::Warning,
        "formation-bottleneck",
        format!(
            "關鍵通道淨寬僅 {} 格（約 {:.0} 單位），無法容納 {} 人方陣通行（需 {} 格，安全上限 {} 人）。",
            choke.clearance, choke.width, choke.troop_count, choke.required, choke.safe_units
        ),
        format!(
            "Choke point clearance is only {} tiles (~{:.0} units); cannot fit {}-man formation (requires {} tiles, safe max {}).",
            choke.clearance, choke.width, choke.troop_count, choke.required, choke.safe_units
        ),
    );
    i.world_x = Some(choke.location.world_x);
    i.world_z = Some(choke.location.world_z);
    i
}

fn route_issues_for(req: &MapRouteRequest, d: &RouteDiagnostic, out: &mut Vec<MapIssue>) {
    let label = &req.label;
    match d.status {
        RouteStatus::EndpointUnusable => {
            for (is_start, issue) in [(true, d.start_issue), (false, d.target_issue)] {
                let Some(cause) = issue else { continue };
                let (zh_side, en_side) = if is_start { ("起點", "start") } else { ("終點", "target") };
                let (zh_cause, en_cause) = cause_text(cause);
                let (id, at) = if is_start {
                    (req.start_id, req.start)
                } else {
                    (req.target_id, req.target)
                };
                out.push(issue_at(
                    MapIssueSeverity::Warning,
                    "route-endpoint-blocked",
                    format!("路線「{label}」的{zh_side}{zh_cause}，部隊無法出發或抵達。請移動該物件，或清除該處的阻擋／抬高地形。"),
                    format!("Route '{label}': the {en_side} is {en_cause}, so units cannot start or arrive. Move the object, or clear the blocking / raise the terrain there."),
                    id,
                    at,
                ));
            }
        }
        RouteStatus::Disconnected => out.push(issue_at(
            MapIssueSeverity::Warning,
            "route-disconnected",
            format!("路線「{label}」的起點與終點分屬不連通的通行區，部隊無法抵達。請開出通道、補道路或架橋。"),
            format!("Route '{label}': start and target are in disconnected passable regions, so units cannot arrive. Open a passage, add a road, or build a bridge."),
            req.start_id,
            req.start,
        )),
        RouteStatus::Reachable => {}
    }
}

impl MapDiagnostics {
    /// Passability grid used by route checks; `None` when the snapshot has no usable collision.
    pub fn passability_grid(map: &MapCheckSnapshot) -> Option<PassabilityGrid> {
        let collision = map.collision.as_ref()?;
        if map.collision_size == 0
            || map.collision_size > MAX_ROUTE_GRID_SIZE
            || collision.len() != map.collision_size * map.collision_size {
            return None;
        }
        let height = map.heights.as_deref().filter(|h| {
            map.height_size > 1 && h.len() == map.height_size * map.height_size
        });
        PassabilityGrid::new(
            map.collision_size,
            map.collision.as_deref(),
            height.map(|h| (map.height_size, h, map.height_step, map.water_level)),
            16384.0,
        )
        .ok()
    }

    /// Default scenario routes: every squad (> 1 unit) toward the first three non-player
    /// targets. Placed objects come first; enabled event spawns of non-player teams (attack
    /// waves, which have no placed object) fill the remaining slots, one per spawn point.
    pub fn scenario_route_requests(map: &MapCheckSnapshot) -> Vec<MapRouteRequest> {
        let squads = map
            .objects
            .iter()
            .filter(|o| o.spawn.count > 1 && Self::valid_point(o.spawn.x, o.spawn.z));
        let mut targets: Vec<(Uuid, String, (f32, f32))> = map
            .objects
            .iter()
            .filter(|o| o.spawn.team != 0 && Self::valid_point(o.spawn.x, o.spawn.z))
            .take(3)
            .map(|o| (o.spawn.id, o.spawn.alias.clone(), (o.spawn.x, o.spawn.z)))
            .collect();
        let waves = map
            .events
            .iter()
            .filter(|e| e.enabled)
            .flat_map(|e| e.actions.iter().map(move |a| (e, a)))
            .filter(|(_, a)| a.kind == 2 && a.team != 0 && Self::valid_point(a.x, a.z));
        for (event, action) in waves {
            if targets.len() >= 3 {
                break;
            }
            if targets.iter().all(|t| t.2 != (action.x, action.z)) {
                let label = format!("{}: {}", event.name, action.alias);
                targets.push((Uuid::nil(), label, (action.x, action.z)));
            }
        }
        squads
            .flat_map(|s| {
                targets.iter().filter(move |t| t.0 != s.spawn.id).map(move |t| MapRouteRequest {
                    label: format!("{} → {}", s.spawn.alias, t.1),
                    start_id: s.spawn.id,
                    target_id: t.0,
                    start: (s.spawn.x, s.spawn.z),
                    target: t.2,
                    troops: s.spawn.count,
                })
            })
            .collect()
    }

    /// Designer hint when squads exist but nothing can be a route target, so no route is
    /// checked at all (otherwise the route check silently reports nothing).
    pub fn route_coverage_issues(map: &MapCheckSnapshot) -> Vec<MapIssue> {
        let squads = map
            .objects
            .iter()
            .filter(|o| o.spawn.count > 1 && Self::valid_point(o.spawn.x, o.spawn.z))
            .count();
        if squads == 0 || !Self::scenario_route_requests(map).is_empty() {
            return vec![];
        }
        vec![MapIssue::new(
            MapIssueSeverity::Warning,
            "route-no-target",
            format!("有 {squads} 支部隊，但沒有可作為目標的非玩家物件或敵方事件生成點，無法檢查部隊路線。請放置敵方或目標物件。"),
            format!("{squads} squad(s) exist but there is no non-player object or enemy event spawn to route toward, so no route was checked. Place an enemy or target object."),
        )]
    }

    /// Check caller-supplied routes against the snapshot's collision/height layers.
    /// Returns an empty report when no usable collision layer exists
    /// (`MapDiagnostics::check` already warns about that).
    pub fn check_routes(map: &MapCheckSnapshot, requests: &[MapRouteRequest]) -> MapRouteReport {
        let Some(grid) = Self::passability_grid(map) else {
            return MapRouteReport::empty();
        };
        let squad_max = requests.iter().map(|r| r.troops).max().unwrap_or(0);
        Self::check_routes_on_grid(&grid, requests, squad_max)
    }

    /// Same as [`Self::check_routes`] on a prepared grid. `squad_max_troops` drives the
    /// topological fallback, which runs only when no corridor choke exists.
    pub fn check_routes_on_grid(
        grid: &PassabilityGrid,
        requests: &[MapRouteRequest],
        squad_max_troops: i32,
    ) -> MapRouteReport {
        let mut route_issues = Vec::new();
        let mut checked = Vec::new();
        let mut trips = Vec::new();
        for req in requests {
            let finite = |p: (f32, f32)| MapDiagnostics::valid_point(p.0, p.1);
            if !finite(req.start) || !finite(req.target) {
                route_issues.push(issue_at(
                    MapIssueSeverity::Error,
                    "route-invalid-point",
                    format!("路線「{}」的起點或終點座標超出地圖範圍或不是有限數值。", req.label),
                    format!("Route '{}' has a start/target outside the map or non-finite.", req.label),
                    req.start_id,
                    req.start,
                ));
                continue;
            }
            trips.push(FormationTrip {
                start: grid.world_to_tile(req.start.0, req.start.1),
                target: grid.world_to_tile(req.target.0, req.target.1),
                troops: req.troops,
            });
            checked.push(req.clone());
        }

        let mut formation = analyze_formation(grid, &trips, squad_max_troops);
        // An obstacle-free grid has the C# `-1` clearance sentinel, which would flag every
        // formation; there is nothing to widen on an open map, so do not report it.
        let open_map = formation.routes.iter().any(|r| r.no_obstacles);
        if open_map {
            formation.corridor_chokes.clear();
            for r in &mut formation.routes {
                r.choke = None;
            }
        }

        let mut seen = HashSet::new();
        for (req, d) in checked.iter().zip(&formation.routes) {
            route_issues_for(req, d, &mut route_issues);
            if let (Some(choke), Some((cell, _))) = (&d.choke, d.narrowest) {
                if seen.insert(cell) {
                    route_issues.push(formation_choke_issue(&req.label, choke));
                }
            }
        }
        let topological_issues = formation.topological_chokes.iter().map(bottleneck_issue).collect();
        MapRouteReport {
            checked,
            formation,
            route_issues,
            topological_issues,
        }
    }
}
