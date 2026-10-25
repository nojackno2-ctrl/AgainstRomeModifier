//! Read-only Map Balance Summary pure computation.
//!
//! Evaluates the current scenario and placement state:
//! 1. Per team 0..7: scripted spawns (document.spawns), total unit count,
//!    and placed DATA buildings (if placement is loaded).
//! 2. Attack waves: timeline of enabled events with SpawnUnit actions (kind 2)
//!    for teams 1..7, cumulative units, and destination status.
//! 3. Objectives: presence of enabled Victory (kind 3) and Defeat (kind 4) events.

use against_rome_maps::{SdlObjectCatalog, SdlObjectCategory};
use against_rome_placement::WorldObject;
use against_rome_scenario::ScenarioDocument;
use std::collections::HashMap;

/// Format delay seconds into `m:ss` display (e.g. 90 -> "1:30").
pub(crate) fn format_time_mmss(seconds: i32) -> String {
    if seconds < 0 {
        let abs = seconds.abs();
        format!("-{}:{:02}", abs / 60, abs % 60)
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

/// Balance statistics for a single team (0..=7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TeamBalance {
    pub team: i32,
    pub spawn_count: usize,
    pub total_units: i32,
    pub placed_buildings: Option<usize>,
}

/// A single attack wave entry corresponding to an enabled `SpawnUnit` action for teams 1..=7.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AttackWaveEntry {
    pub delay_seconds: i32,
    pub time_formatted: String,
    pub event_name: String,
    pub team: i32,
    pub units: i32,
    pub cumulative_units: i32,
    pub has_move_destination: bool,
}

/// Objectives presence status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ObjectivesSummary {
    pub has_victory: bool,
    pub has_defeat: bool,
}

/// Aggregated map balance summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BalanceSummary {
    pub teams: [TeamBalance; 8],
    pub waves: Vec<AttackWaveEntry>,
    pub total_enemy_wave_units: i32,
    pub waves_with_destination: usize,
    pub total_wave_actions: usize,
    pub objectives: ObjectivesSummary,
}

/// Pure computation of scenario balance summary from in-memory document and optional placement.
pub(crate) fn summarize(
    document: &ScenarioDocument,
    placement: Option<(&[WorldObject], &HashMap<i32, String>)>,
) -> BalanceSummary {
    // 1. Per-team statistics (0..=7)
    let mut team_spawn_counts = [0usize; 8];
    let mut team_total_units = [0i32; 8];

    for spawn in &document.spawns {
        if (0..=7).contains(&spawn.team) {
            let idx = spawn.team as usize;
            team_spawn_counts[idx] += 1;
            team_total_units[idx] += spawn.count.max(0);
        }
    }

    let team_buildings: Option<[usize; 8]> = placement.map(|(objects, type_names)| {
        let mut buildings = [0usize; 8];
        for root in objects.iter().filter(|o| o.parent.is_none()) {
            let team_idx = root.team as usize;
            if team_idx < 8 {
                if let Some(name) = type_names.get(&i32::from(root.type_id)) {
                    if SdlObjectCatalog::categorize(name) == SdlObjectCategory::Building {
                        buildings[team_idx] += 1;
                    }
                }
            }
        }
        buildings
    });

    let teams = std::array::from_fn(|t| TeamBalance {
        team: t as i32,
        spawn_count: team_spawn_counts[t],
        total_units: team_total_units[t],
        placed_buildings: team_buildings.map(|b| b[t]),
    });

    // 2. Attack waves & objectives
    struct CandidateWave {
        delay_seconds: i32,
        event_name: String,
        team: i32,
        units: i32,
        has_move_destination: bool,
    }

    let mut candidates = Vec::new();
    let mut has_victory = false;
    let mut has_defeat = false;

    for event in &document.events {
        if !event.enabled {
            continue;
        }

        for action in &event.actions {
            match action.kind {
                // Action kind 3 = Victory
                3 => has_victory = true,
                // Action kind 4 = Defeat
                4 => has_defeat = true,
                // Action kind 2 = SpawnUnit; waves are enemy teams 1..=7 only
                2 if (1..=7).contains(&action.team) => {
                    candidates.push(CandidateWave {
                        delay_seconds: event.delay_seconds,
                        event_name: event.name.clone(),
                        team: action.team,
                        units: action.count.max(0),
                        has_move_destination: action.move_destination.is_some(),
                    });
                }
                _ => {}
            }
        }
    }

    // Sort timeline order: delay_seconds ascending, then team ascending
    candidates.sort_by_key(|c| (c.delay_seconds, c.team));

    let mut waves = Vec::with_capacity(candidates.len());
    let mut cumulative = 0;
    let mut waves_with_destination = 0;

    for c in candidates {
        cumulative += c.units;
        if c.has_move_destination {
            waves_with_destination += 1;
        }
        waves.push(AttackWaveEntry {
            delay_seconds: c.delay_seconds,
            time_formatted: format_time_mmss(c.delay_seconds),
            event_name: c.event_name,
            team: c.team,
            units: c.units,
            cumulative_units: cumulative,
            has_move_destination: c.has_move_destination,
        });
    }

    let total_wave_actions = waves.len();
    let total_enemy_wave_units = cumulative;

    // 3. Objectives summary
    let objectives = ObjectivesSummary {
        has_victory,
        has_defeat,
    };

    BalanceSummary {
        teams,
        waves,
        total_enemy_wave_units,
        waves_with_destination,
        total_wave_actions,
        objectives,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use against_rome_scenario::{Action, Event, Spawn, WorldPosition};

    #[test]
    fn test_format_time_mmss() {
        assert_eq!(format_time_mmss(0), "0:00");
        assert_eq!(format_time_mmss(5), "0:05");
        assert_eq!(format_time_mmss(59), "0:59");
        assert_eq!(format_time_mmss(60), "1:00");
        assert_eq!(format_time_mmss(90), "1:30");
        assert_eq!(format_time_mmss(125), "2:05");
        assert_eq!(format_time_mmss(600), "10:00");
        assert_eq!(format_time_mmss(-75), "-1:15");
    }

    #[test]
    fn test_per_team_counts() {
        let mut doc = ScenarioDocument::default();
        // Team 0: 2 spawns, 5 + 12 = 17 units
        doc.spawns.push(Spawn { team: 0, count: 5, ..Default::default() });
        doc.spawns.push(Spawn { team: 0, count: 12, ..Default::default() });
        // Team 1: 1 spawn, 8 units
        doc.spawns.push(Spawn { team: 1, count: 8, ..Default::default() });
        // Team 7: 1 spawn, 3 units
        doc.spawns.push(Spawn { team: 7, count: 3, ..Default::default() });
        // Spawn with team outside 0..7 (ignored for 0..7)
        doc.spawns.push(Spawn { team: 8, count: 100, ..Default::default() });

        // First test without placement loaded
        let summary_no_placement = summarize(&doc, None);
        assert_eq!(summary_no_placement.teams[0].spawn_count, 2);
        assert_eq!(summary_no_placement.teams[0].total_units, 17);
        assert_eq!(summary_no_placement.teams[0].placed_buildings, None);
        assert_eq!(summary_no_placement.teams[1].spawn_count, 1);
        assert_eq!(summary_no_placement.teams[1].total_units, 8);
        assert_eq!(summary_no_placement.teams[1].placed_buildings, None);
        assert_eq!(summary_no_placement.teams[2].spawn_count, 0);
        assert_eq!(summary_no_placement.teams[2].total_units, 0);
        assert_eq!(summary_no_placement.teams[2].placed_buildings, None);
        assert_eq!(summary_no_placement.teams[7].spawn_count, 1);
        assert_eq!(summary_no_placement.teams[7].total_units, 3);
        assert_eq!(summary_no_placement.teams[7].placed_buildings, None);

        // Second test with placement loaded
        let mut type_names = HashMap::new();
        type_names.insert(1, "BauGerHaus".to_string()); // Building
        type_names.insert(2, "BauHunZelt".to_string()); // Building
        type_names.insert(3, "FigGerKrieger".to_string()); // Figure (not a building)

        let objects = vec![
            // Team 0 root building
            WorldObject { slot: 0, type_id: 1, team: 0, uid: 1, position: [0.0; 3], rotation_degrees: 0.0, parent: None, vitals: None },
            // Team 0 child object of slot 0 (must NOT be counted as separate building)
            WorldObject { slot: 1, type_id: 1, team: 0, uid: 2, position: [0.0; 3], rotation_degrees: 0.0, parent: Some(0), vitals: None },
            // Team 1 root building
            WorldObject { slot: 2, type_id: 2, team: 1, uid: 3, position: [0.0; 3], rotation_degrees: 0.0, parent: None, vitals: None },
            // Team 1 figure (not a building)
            WorldObject { slot: 3, type_id: 3, team: 1, uid: 4, position: [0.0; 3], rotation_degrees: 0.0, parent: None, vitals: None },
            // Team 2 root building
            WorldObject { slot: 4, type_id: 1, team: 2, uid: 5, position: [0.0; 3], rotation_degrees: 0.0, parent: None, vitals: None },
            WorldObject { slot: 5, type_id: 1, team: 2, uid: 6, position: [0.0; 3], rotation_degrees: 0.0, parent: None, vitals: None },
        ];

        let summary = summarize(&doc, Some((&objects, &type_names)));
        assert_eq!(summary.teams[0].placed_buildings, Some(1));
        assert_eq!(summary.teams[1].placed_buildings, Some(1));
        assert_eq!(summary.teams[2].placed_buildings, Some(2));
        assert_eq!(summary.teams[3].placed_buildings, Some(0));
    }

    #[test]
    fn test_wave_timeline_ordering_and_cumulative_totals() {
        let mut doc = ScenarioDocument::default();
        // Insert events out of chronological order
        doc.events.push(Event {
            name: "Late Wave".into(),
            delay_seconds: 300,
            enabled: true,
            actions: vec![Action {
                kind: 2,
                team: 1,
                count: 15,
                move_destination: Some(WorldPosition { x: 100, z: 200 }),
                ..Default::default()
            }],
            ..Default::default()
        });
        doc.events.push(Event {
            name: "Early Wave".into(),
            delay_seconds: 45,
            enabled: true,
            actions: vec![Action {
                kind: 2,
                team: 2,
                count: 8,
                move_destination: None,
                ..Default::default()
            }],
            ..Default::default()
        });
        doc.events.push(Event {
            name: "Mid Wave".into(),
            delay_seconds: 120,
            enabled: true,
            actions: vec![Action {
                kind: 2,
                team: 1,
                count: 10,
                move_destination: Some(WorldPosition { x: 50, z: 50 }),
                ..Default::default()
            }],
            ..Default::default()
        });

        let summary = summarize(&doc, None);
        assert_eq!(summary.waves.len(), 3);
        assert_eq!(summary.total_wave_actions, 3);
        assert_eq!(summary.total_enemy_wave_units, 33);
        assert_eq!(summary.waves_with_destination, 2);

        // Wave 0: 45s (0:45), team 2, 8 units, cumulative 8, destination None
        assert_eq!(summary.waves[0].delay_seconds, 45);
        assert_eq!(summary.waves[0].time_formatted, "0:45");
        assert_eq!(summary.waves[0].event_name, "Early Wave");
        assert_eq!(summary.waves[0].team, 2);
        assert_eq!(summary.waves[0].units, 8);
        assert_eq!(summary.waves[0].cumulative_units, 8);
        assert!(!summary.waves[0].has_move_destination);

        // Wave 1: 120s (2:00), team 1, 10 units, cumulative 18, destination Some
        assert_eq!(summary.waves[1].delay_seconds, 120);
        assert_eq!(summary.waves[1].time_formatted, "2:00");
        assert_eq!(summary.waves[1].event_name, "Mid Wave");
        assert_eq!(summary.waves[1].team, 1);
        assert_eq!(summary.waves[1].units, 10);
        assert_eq!(summary.waves[1].cumulative_units, 18);
        assert!(summary.waves[1].has_move_destination);

        // Wave 2: 300s (5:00), team 1, 15 units, cumulative 33, destination Some
        assert_eq!(summary.waves[2].delay_seconds, 300);
        assert_eq!(summary.waves[2].time_formatted, "5:00");
        assert_eq!(summary.waves[2].event_name, "Late Wave");
        assert_eq!(summary.waves[2].team, 1);
        assert_eq!(summary.waves[2].units, 15);
        assert_eq!(summary.waves[2].cumulative_units, 33);
        assert!(summary.waves[2].has_move_destination);
    }

    #[test]
    fn test_disabled_events_ignored() {
        let mut doc = ScenarioDocument::default();
        doc.events.push(Event {
            name: "Disabled Wave".into(),
            delay_seconds: 30,
            enabled: false,
            actions: vec![Action {
                kind: 2,
                team: 1,
                count: 50,
                move_destination: Some(WorldPosition { x: 0, z: 0 }),
                ..Default::default()
            }],
            ..Default::default()
        });
        doc.events.push(Event {
            name: "Disabled Victory".into(),
            delay_seconds: 0,
            enabled: false,
            actions: vec![Action { kind: 3, ..Default::default() }],
            ..Default::default()
        });
        doc.events.push(Event {
            name: "Disabled Defeat".into(),
            delay_seconds: 0,
            enabled: false,
            actions: vec![Action { kind: 4, ..Default::default() }],
            ..Default::default()
        });

        let summary = summarize(&doc, None);
        assert!(summary.waves.is_empty());
        assert_eq!(summary.total_enemy_wave_units, 0);
        assert_eq!(summary.total_wave_actions, 0);
        assert_eq!(summary.waves_with_destination, 0);
        assert!(!summary.objectives.has_victory);
        assert!(!summary.objectives.has_defeat);
    }

    #[test]
    fn test_missing_victory_defeat_flagged() {
        // Neither
        let doc0 = ScenarioDocument::default();
        let s0 = summarize(&doc0, None);
        assert!(!s0.objectives.has_victory);
        assert!(!s0.objectives.has_defeat);

        // Only victory
        let mut doc1 = ScenarioDocument::default();
        doc1.events.push(Event {
            enabled: true,
            actions: vec![Action { kind: 3, ..Default::default() }],
            ..Default::default()
        });
        let s1 = summarize(&doc1, None);
        assert!(s1.objectives.has_victory);
        assert!(!s1.objectives.has_defeat);

        // Only defeat
        let mut doc2 = ScenarioDocument::default();
        doc2.events.push(Event {
            enabled: true,
            actions: vec![Action { kind: 4, ..Default::default() }],
            ..Default::default()
        });
        let s2 = summarize(&doc2, None);
        assert!(!s2.objectives.has_victory);
        assert!(s2.objectives.has_defeat);

        // Both enabled
        let mut doc3 = ScenarioDocument::default();
        doc3.events.push(Event {
            enabled: true,
            actions: vec![Action { kind: 3, ..Default::default() }],
            ..Default::default()
        });
        doc3.events.push(Event {
            enabled: true,
            actions: vec![Action { kind: 4, ..Default::default() }],
            ..Default::default()
        });
        let s3 = summarize(&doc3, None);
        assert!(s3.objectives.has_victory);
        assert!(s3.objectives.has_defeat);
    }

    #[test]
    fn test_team_0_spawns_not_counted_as_waves() {
        let mut doc = ScenarioDocument::default();
        // Enabled event spawning reinforcements for player (team 0)
        doc.events.push(Event {
            name: "Player reinforcements".into(),
            delay_seconds: 60,
            enabled: true,
            actions: vec![Action {
                kind: 2,
                team: 0,
                count: 20,
                ..Default::default()
            }],
            ..Default::default()
        });
        // Enabled event spawning enemy wave (team 2)
        doc.events.push(Event {
            name: "Enemy attack".into(),
            delay_seconds: 90,
            enabled: true,
            actions: vec![Action {
                kind: 2,
                team: 2,
                count: 12,
                move_destination: Some(WorldPosition { x: 10, z: 20 }),
                ..Default::default()
            }],
            ..Default::default()
        });

        let summary = summarize(&doc, None);
        // Only the enemy wave (team 2) should be present
        assert_eq!(summary.waves.len(), 1);
        assert_eq!(summary.waves[0].team, 2);
        assert_eq!(summary.waves[0].units, 12);
        assert_eq!(summary.total_enemy_wave_units, 12);
    }
}
