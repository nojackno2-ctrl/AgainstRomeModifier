//! Adapts explicitly authored lair timers to scenario events.

use super::models::{LairCategory, NeutralLairDefinition, PlacedNeutralLair, TimedResourceReplacement};
use super::planner::ResourceRegenerationPlanner;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const LAIR_EVENT_PREFIX: &str = "LAIR_";

pub fn lair_event_prefix_for(id: &Uuid) -> String {
    format!("{}{:x}_", LAIR_EVENT_PREFIX, id.as_simple())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScenarioActionKind {
    SpawnUnit,
    Message,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioAction {
    pub kind: ScenarioActionKind,
    pub team: i32,
    pub alias: String,
    pub x: f32,
    pub z: f32,
    pub count: i32,
    pub text: String,
}

impl ScenarioAction {
    pub fn spawn_unit(team: i32, alias: impl Into<String>, x: f32, z: f32, count: i32) -> Self {
        Self {
            kind: ScenarioActionKind::SpawnUnit,
            team,
            alias: alias.into(),
            x,
            z,
            count,
            text: String::new(),
        }
    }

    pub fn message(text: impl Into<String>) -> Self {
        Self {
            kind: ScenarioActionKind::Message,
            team: 0,
            alias: String::new(),
            x: 0.0,
            z: 0.0,
            count: 0,
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScenarioConditionKind {
    ObjectExists,
    ObjectDeadOrRemoved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioCondition {
    pub kind: ScenarioConditionKind,
    pub target_id: Uuid,
}

impl ScenarioCondition {
    pub fn new(kind: ScenarioConditionKind, target_id: Uuid) -> Self {
        Self { kind, target_id }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioEvent {
    pub name: String,
    pub delay_seconds: i32,
    pub repeat: bool,
    pub active: bool,
    pub conditions: Vec<ScenarioCondition>,
    pub actions: Vec<ScenarioAction>,
}

impl ScenarioEvent {
    pub fn new(name: impl Into<String>, delay_seconds: i32, repeat: bool, active: bool) -> Self {
        Self {
            name: name.into(),
            delay_seconds,
            repeat,
            active,
            conditions: Vec::new(),
            actions: Vec::new(),
        }
    }
}

pub struct WildLairScenarioEventBinder;

impl WildLairScenarioEventBinder {
    pub fn generate_events_for_lair(
        lair: &PlacedNeutralLair,
        definition: &NeutralLairDefinition,
        _player_team: i32,
        tile_world_size: f32,
        aliases: &[(String, String)], // (alias, name_def)
    ) -> Result<Vec<ScenarioEvent>, String> {
        super::catalog::NeutralLairCatalog::validate_definition(definition)?;

        if lair.instance_id.is_nil() || lair.definition_id != definition.id {
            return Err("A persistent instance ID and matching definition are required.".to_string());
        }
        if definition.category == LairCategory::WildAnimal {
            return Err("FigTie animals require single-object creation and neutral team 8. ScenarioEvent SpawnUnit only provides troop creation for teams 0-7.".to_string());
        }
        if !aliases.iter().any(|(_, name_def)| {
            name_def.eq_ignore_ascii_case(&definition.native_building_or_landscape_type)
        }) {
            return Err("The blueprint's core object is not in the loaded alias catalog.".to_string());
        }
        if !(0..=7).contains(&lair.team) {
            return Err("ScenarioEvent SpawnUnit cannot express neutral team 8.".to_string());
        }
        if !tile_world_size.is_finite()
            || tile_world_size <= 0.0
            || !lair.world_x.is_finite()
            || !lair.world_z.is_finite()
            || !lair.rotation_deg.is_finite()
            || !(0.0..=16383.0).contains(&lair.world_x)
            || !(0.0..=16383.0).contains(&lair.world_z)
        {
            return Err("Use finite world coordinates and tile scale in 0..=16383.".to_string());
        }
        if !definition.default_guards.is_empty()
            || lair.guard_spawn_ids.as_ref().map(|g| !g.is_empty()).unwrap_or(false)
        {
            return Err("Guard placement, patrol and death-triggered respawn are not supported by this timer adapter.".to_string());
        }
        if definition.loot.wood != 0
            || definition.loot.food != 0
            || definition.loot.gold != 0
            || definition.loot.honor_points != 0
        {
            return Err("The verified event compiler has no resource or honor reward action.".to_string());
        }
        if !definition.loot.completion_message.is_empty()
            && lair.core_structure_spawn_id.is_none()
        {
            return Err("A completion message requires a tracked ScenarioSpawn core ID.".to_string());
        }

        let prefix = lair_event_prefix_for(&lair.instance_id);
        let mut events = Vec::new();

        for (i, wave) in definition.wave_rules.iter().enumerate() {
            if wave.initial_delay_seconds != wave.interval_seconds
                || wave.max_active_waves != 0
                || !wave.aggro_behavior.eq_ignore_ascii_case("None")
            {
                return Err("A ScenarioEvent has one timer period; separate initial delays, active-wave caps and aggression orders cannot be expressed. Use MaxActiveWaves=0 (uncapped) and AggroBehavior=None.".to_string());
            }

            let matching_alias = aliases
                .iter()
                .find(|(a, _)| a.eq_ignore_ascii_case(&wave.unit_alias))
                .ok_or_else(|| {
                    format!(
                        "Alias {} is not an observed faction infantry member for SpawnUnit.",
                        wave.unit_alias
                    )
                })?;

            if !is_troop_member(&matching_alias.1) {
                return Err(format!(
                    "Alias {} is not an observed faction infantry member for SpawnUnit.",
                    wave.unit_alias
                ));
            }

            if !wave.spawn_radius_tiles.is_finite() || wave.spawn_radius_tiles < 0.0 {
                return Err("Spawn radius must be finite and nonnegative.".to_string());
            }

            let angle = (i as f32) * std::f32::consts::PI / 2.0
                + lair.rotation_deg * std::f32::consts::PI / 180.0;
            let x = lair.world_x + angle.cos() * wave.spawn_radius_tiles * tile_world_size;
            let z = lair.world_z + angle.sin() * wave.spawn_radius_tiles * tile_world_size;

            let mut event = ScenarioEvent::new(
                format!("{prefix}WAVE_{i}"),
                wave.interval_seconds,
                true,
                lair.is_active,
            );
            event.actions.push(ScenarioAction::spawn_unit(
                lair.team,
                &matching_alias.0,
                x,
                z,
                wave.spawn_count,
            ));

            if let Some(core_id) = lair.core_structure_spawn_id {
                event
                    .conditions
                    .push(ScenarioCondition::new(ScenarioConditionKind::ObjectExists, core_id));
            }
            events.push(event);
        }

        if !definition.loot.completion_message.is_empty() {
            if let Some(core_id) = lair.core_structure_spawn_id {
                let mut cleared = ScenarioEvent::new(
                    format!("{prefix}CLEARED"),
                    1,
                    false,
                    lair.is_active,
                );
                cleared.conditions.push(ScenarioCondition::new(
                    ScenarioConditionKind::ObjectDeadOrRemoved,
                    core_id,
                ));
                cleared.actions.push(ScenarioAction::message(
                    &definition.loot.completion_message,
                ));
                events.push(cleared);
            }
        }

        Ok(events)
    }

    pub fn generate_events_for_resource(request: &TimedResourceReplacement) -> Result<Vec<ScenarioEvent>, String> {
        let planner = ResourceRegenerationPlanner;
        planner.plan_replacement(
            &request.name_def,
            request.world_x,
            request.world_z,
            request.delay_seconds,
            request.team,
        )?;
        Err("ScenarioEvent has no landscape/single-object spawn or depletion/replenishment action. Tree, stone, field, mine and goldsmith timers are gated; no bytecode is generated.".to_string())
    }

    pub fn sync_lair_events(
        events_list: &mut Vec<ScenarioEvent>,
        lair: &PlacedNeutralLair,
        definition: &NeutralLairDefinition,
        player_team: i32,
        aliases: &[(String, String)],
        tile_world_size: f32,
    ) -> Result<(), String> {
        let replacement = Self::generate_events_for_lair(
            lair,
            definition,
            player_team,
            tile_world_size,
            aliases,
        )?;

        let prefix = lair_event_prefix_for(&lair.instance_id);
        let retained = events_list
            .iter()
            .filter(|e| !e.name.starts_with(&prefix))
            .count();

        if retained + replacement.len() > 256 {
            return Err("Event limit exceeded; existing session was preserved.".to_string());
        }

        Self::remove_lair_events(events_list, lair.instance_id);
        events_list.extend(replacement);
        Ok(())
    }

    pub fn remove_lair_events(events_list: &mut Vec<ScenarioEvent>, instance_id: Uuid) -> usize {
        let prefix = lair_event_prefix_for(&instance_id);
        let original_len = events_list.len();
        events_list.retain(|e| !e.name.starts_with(&prefix));
        original_len - events_list.len()
    }

    pub fn validate_lair_bindings(
        events: &[ScenarioEvent],
        active_lairs: &[PlacedNeutralLair],
    ) -> Vec<String> {
        let valid_ids: std::collections::HashSet<Uuid> =
            active_lairs.iter().map(|l| l.instance_id).collect();

        let mut errors = Vec::new();
        for e in events {
            if e.name.starts_with(LAIR_EVENT_PREFIX) {
                let rest = &e.name[LAIR_EVENT_PREFIX.len()..];
                if rest.len() < 33 || rest.as_bytes()[32] != b'_' {
                    errors.push(format!("Event {} has an unknown or legacy lair instance reference.", e.name));
                    continue;
                }
                let hex_part = &rest[..32];
                match Uuid::parse_str(hex_part) {
                    Ok(id) if valid_ids.contains(&id) => {}
                    _ => {
                        errors.push(format!("Event {} has an unknown or legacy lair instance reference.", e.name));
                    }
                }
            }
        }
        errors
    }
}

fn is_troop_member(name: &str) -> bool {
    ["FigGerInf", "FigHunInf", "FigKelInf", "FigRomInf"]
        .iter()
        .any(|prefix| name.starts_with(prefix))
}
