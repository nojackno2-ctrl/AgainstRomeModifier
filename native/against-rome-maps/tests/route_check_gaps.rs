use against_rome_maps::{MapCheckObject, MapCheckSnapshot, MapDiagnostics, MAX_ROUTE_GRID_SIZE};
use against_rome_scenario::model::Spawn;
use uuid::Uuid;

fn obj(x: f32, z: f32, team: i32, count: i32) -> MapCheckObject {
    MapCheckObject::new(
        Spawn { alias: "U".into(), x, z, team, count, id: Uuid::new_v4(), ..Default::default() },
        false,
        true,
    )
}

fn snap(objects: Vec<MapCheckObject>, size: usize) -> MapCheckSnapshot {
    MapCheckSnapshot::new(objects, vec![], vec!["U".into()], size, Some(vec![0u8; size * size]))
}

#[test]
fn squad_without_target_gets_designer_hint() {
    let s = snap(vec![obj(100.0, 100.0, 0, 20)], 8);
    assert!(MapDiagnostics::scenario_route_requests(&s).is_empty());
    let issues = MapDiagnostics::route_coverage_issues(&s);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, "route-no-target");
}

#[test]
fn squad_with_enemy_has_no_hint_and_no_self_route() {
    let enemy = obj(9000.0, 9000.0, 1, 20); // also a squad: must not route to itself
    let eid = enemy.spawn.id;
    let s = snap(vec![obj(100.0, 100.0, 0, 20), enemy], 8);
    assert!(MapDiagnostics::route_coverage_issues(&s).is_empty());
    let reqs = MapDiagnostics::scenario_route_requests(&s);
    assert_eq!(reqs.len(), 1);
    assert!(reqs.iter().all(|r| r.start_id != r.target_id));
    assert!(reqs.iter().any(|r| r.target_id == eid));
}

#[test]
fn enemy_event_wave_spawn_is_a_route_target() {
    use against_rome_scenario::model::{Action, Event};
    let wave = |enabled, team| Event {
        name: "Wave_01".into(),
        enabled,
        actions: vec![
            Action { kind: 2, team, alias: "HUN_KAVINF00".into(), x: 9000.0, z: 9000.0, count: 3, ..Default::default() },
            // Same spawn point again: one target, not two.
            Action { kind: 2, team, alias: "HUN_KAVSCH00".into(), x: 9000.0, z: 9000.0, count: 3, ..Default::default() },
        ],
        ..Default::default()
    };
    let with_events = |events| {
        let mut s = snap(vec![obj(100.0, 100.0, 0, 20)], 8);
        s.events = events;
        s
    };
    let s = with_events(vec![wave(true, 1)]);
    let reqs = MapDiagnostics::scenario_route_requests(&s);
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].target, (9000.0, 9000.0));
    assert!(reqs[0].target_id.is_nil());
    assert!(reqs[0].label.contains("Wave_01"));
    assert!(MapDiagnostics::route_coverage_issues(&s).is_empty());
    // Disabled events and player-team spawns are not enemy targets.
    for events in [vec![wave(false, 1)], vec![wave(true, 0)]] {
        assert_eq!(MapDiagnostics::route_coverage_issues(&with_events(events)).len(), 1);
    }
}

#[test]
fn no_squads_no_hint() {
    assert!(MapDiagnostics::route_coverage_issues(&snap(vec![obj(1.0, 1.0, 1, 1)], 8)).is_empty());
}

#[test]
fn oversized_collision_grid_is_rejected() {
    let size = MAX_ROUTE_GRID_SIZE + 1;
    let s = snap(vec![], size);
    assert!(MapDiagnostics::passability_grid(&s).is_none());
    assert!(MapDiagnostics::passability_grid(&snap(vec![], 8)).is_some());
}

#[test]
fn check_surfaces_route_no_target_hint() {
    let s = snap(vec![obj(100.0, 100.0, 0, 20)], 8);
    let issues = MapDiagnostics::check(&s);
    assert_eq!(issues.iter().filter(|i| i.code == "route-no-target").count(), 1);
}

#[test]
fn check_omits_route_no_target_when_enemy_present() {
    let s = snap(vec![obj(100.0, 100.0, 0, 20), obj(9000.0, 9000.0, 1, 20)], 8);
    assert!(MapDiagnostics::check(&s).iter().all(|i| i.code != "route-no-target"));
}
