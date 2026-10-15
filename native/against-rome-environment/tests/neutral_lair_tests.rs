use against_rome_environment::{
    LairCategory, LairDifficultyTier, LairLootReward, LairWaveSpawnRule, NeutralLairCatalog,
    NeutralLairDefinition, PlacedNeutralLair, ResourceRegenerationPlanner, ScenarioActionKind,
    ScenarioCondition, ScenarioConditionKind, ScenarioEvent,
    WildLairScenarioEventBinder,
};
use uuid::Uuid;

fn sample_aliases() -> Vec<(String, String)> {
    vec![
        ("ALL_ZIVMAN00".to_string(), "FigZivMan00_Zivilist".to_string()),
        ("ALL_ZIVWEI00".to_string(), "FigZivWei00_Zivilistin".to_string()),
        ("ALL_PACKPF00".to_string(), "FigTiePac00_Packpferd".to_string()),
        ("ALL_BAE00".to_string(), "FigTieBae00_Baer".to_string()),
        ("ALL_RAU00".to_string(), "FigTieRau00_Raubkatze".to_string()),
        ("ALL_WOL00".to_string(), "FigTieWol00_Wilder_Wolf".to_string()),
        ("ALL_EBE00".to_string(), "FigTieEbe00_Wildschwein".to_string()),
        ("GER_INF01".to_string(), "FigGerInf01_Schwert".to_string()),
        ("ROM_INF01".to_string(), "FigRomInf01_Schwert_Schild".to_string()),
        ("GER_HAU00".to_string(), "BauGerHau00_Haupthaus".to_string()),
        ("GER_MIN00".to_string(), "BauGerMin00_Mine".to_string()),
        ("GER_GOL00".to_string(), "BauGerGol00_Goldschmiede".to_string()),
    ]
}

fn blueprint() -> NeutralLairDefinition {
    NeutralLairDefinition {
        id: "AUTHORED_OUTPOST".to_string(),
        display_name_zh: "Outpost".to_string(),
        display_name_en: "Outpost".to_string(),
        category: LairCategory::BarbarianCamp,
        tier: LairDifficultyTier::Unrated,
        native_building_or_landscape_type: "BauGerHau00_Haupthaus".to_string(),
        footprint_radius_tiles: 1.0,
        default_guards: Vec::new(),
        wave_rules: vec![LairWaveSpawnRule {
            wave_id: "timer".to_string(),
            unit_alias: "GER_INF01".to_string(),
            spawn_count: 2,
            interval_seconds: 60,
            initial_delay_seconds: 60,
            max_active_waves: 0,
            spawn_radius_tiles: 1.0,
            aggro_behavior: "None".to_string(),
        }],
        loot: LairLootReward::default(),
        biome_affinity: String::new(),
        description: String::new(),
        script_alias: None,
    }
}

fn placed(d: &NeutralLairDefinition, id: Option<Uuid>, core: Option<Uuid>) -> PlacedNeutralLair {
    PlacedNeutralLair {
        instance_id: id.unwrap_or_else(Uuid::new_v4),
        definition_id: d.id.clone(),
        world_x: 4000.0,
        world_y: 0.0,
        world_z: 6000.0,
        rotation_deg: 0.0,
        team: 7,
        core_structure_spawn_id: core,
        guard_spawn_ids: None,
        bound_event_names: None,
        is_active: true,
    }
}

fn bind(d: &NeutralLairDefinition, l: Option<&PlacedNeutralLair>) -> Result<Vec<ScenarioEvent>, String> {
    let aliases = sample_aliases();
    let default_placed = placed(d, None, None);
    let target_placed = l.unwrap_or(&default_placed);
    WildLairScenarioEventBinder::generate_events_for_lair(target_placed, d, 0, 256.0, &aliases)
}

#[test]
fn catalog_contains_real_land_animals_without_invented_mechanics() {
    let aliases = sample_aliases();
    let catalog = NeutralLairCatalog::default().filtered_by(&aliases);
    assert_eq!(catalog.all_definitions().len(), 4);

    let wolf = catalog.get_by_id("ALL_WOL00").expect("wolf found");
    assert_eq!(wolf.native_building_or_landscape_type, "FigTieWol00_Wilder_Wolf");
    assert!(catalog.get_by_id("LAIR_WOLF_DEN_SMALL").is_none());

    assert!(NeutralLairCatalog::calculate_threat_rating(&blueprint()).is_err());
    assert!(catalog.get_by_category(LairCategory::BarbarianCamp).is_empty());
    assert_eq!(
        catalog.filter(Some(LairCategory::WildAnimal), Some(LairDifficultyTier::Unrated), None).len(),
        4
    );

    for animal in catalog.all_definitions() {
        assert!(NeutralLairCatalog::validate_definition(animal).is_ok());
        assert!(animal.default_guards.is_empty());
        assert!(animal.wave_rules.is_empty());
        assert_eq!(animal.loot, LairLootReward::default());
        assert_eq!(NeutralLairCatalog::calculate_threat_rating(animal), Ok(0.0));
    }

    assert!(NeutralLairCatalog::default()
        .filtered_by(&[("ALL_WOL00".to_string(), "FigGerInf01_Schwert".to_string())])
        .all_definitions()
        .is_empty());
}

#[test]
fn animal_alias_existence_does_not_imply_timer_spawn_support() {
    let catalog = NeutralLairCatalog::default();
    for id in ["ALL_WOL00", "ALL_BAE00", "ALL_EBE00", "ALL_RAU00"] {
        let d = catalog.get_by_id(id).unwrap();
        assert!(bind(d, None).is_err());
    }
}

#[test]
fn authored_timer_uses_real_alias_world_scale_core_condition_and_existing_compiler() {
    let mut d = blueprint();
    d.loot.completion_message = "Core removed".to_string();
    let core = Uuid::new_v4();
    let l = placed(&d, None, Some(core));

    let events = bind(&d, Some(&l)).expect("events generated");
    assert_eq!(events.len(), 2);

    let timer = events.iter().find(|e| e.repeat).expect("repeat event found");
    assert_eq!(timer.delay_seconds, 60);
    assert_eq!(timer.actions.len(), 1);
    let action = &timer.actions[0];
    assert_eq!(action.kind, ScenarioActionKind::SpawnUnit);
    assert_eq!(action.alias, "GER_INF01");
    assert_eq!(action.x, 4256.0);
    assert_eq!(action.z, 6000.0);
    assert_eq!(action.team, 7);
    assert_eq!(timer.conditions, vec![ScenarioCondition::new(ScenarioConditionKind::ObjectExists, core)]);

    let cleared = events.iter().find(|e| !e.repeat).expect("cleared event found");
    assert_eq!(cleared.conditions, vec![ScenarioCondition::new(ScenarioConditionKind::ObjectDeadOrRemoved, core)]);
}

#[test]
fn non_troop_or_unknown_alias_is_gated() {
    for alias in ["ALL_WOL00", "ALL_ZIVMAN00", "GER_MIN00", "LanGerNad00_Tanne_gross", "MISSING"] {
        let mut d = blueprint();
        d.wave_rules[0].unit_alias = alias.to_string();
        assert!(bind(&d, None).is_err(), "alias {alias} should be rejected");
    }
}

#[test]
fn unsupported_mechanics_and_invalid_coordinates_do_not_get_silently_clamped() {
    let d = blueprint();
    let w = d.wave_rules[0].clone();

    // Bad wave rule configs
    for bad in [
        LairWaveSpawnRule { initial_delay_seconds: 30, ..w.clone() },
        LairWaveSpawnRule { max_active_waves: 2, ..w.clone() },
        LairWaveSpawnRule { aggro_behavior: "PatrolHostile".to_string(), ..w.clone() },
    ] {
        let mut modified = d.clone();
        modified.wave_rules = vec![bad];
        assert!(bind(&modified, None).is_err());
    }

    // Default guards
    let mut with_guards = d.clone();
    with_guards.default_guards = vec![against_rome_environment::LairGuardUnit::new("GER_INF01", 1)];
    assert!(bind(&with_guards, None).is_err());

    // Loot with gold
    let mut with_loot = d.clone();
    with_loot.loot.gold = 1;
    assert!(bind(&with_loot, None).is_err());

    // Team 8 neutral
    let neutral_l = placed(&d, None, None);
    let neutral_l = PlacedNeutralLair { team: 8, ..neutral_l };
    assert!(bind(&d, Some(&neutral_l)).is_err());

    // Completion message without core structure
    let mut with_msg = d.clone();
    with_msg.loot.completion_message = "Cleared".to_string();
    assert!(bind(&with_msg, None).is_err());

    // Invalid coordinates
    let nan_l = PlacedNeutralLair { world_x: f32::NAN, ..placed(&d, None, None) };
    assert!(bind(&d, Some(&nan_l)).is_err());

    let out_l = PlacedNeutralLair { world_x: 16500.0, ..placed(&d, None, None) };
    assert!(bind(&d, Some(&out_l)).is_err());

    // Spawn count out of range
    let mut over_spawn = d.clone();
    over_spawn.wave_rules[0].spawn_count = 21;
    assert!(bind(&over_spawn, None).is_err());

    // Interval too short
    let mut short_interval = d.clone();
    short_interval.wave_rules[0].interval_seconds = 5;
    assert!(bind(&short_interval, None).is_err());
}

#[test]
fn session_sync_is_validated_before_mutation_and_uses_complete_instance_id() {
    let d = blueprint();
    let aliases = sample_aliases();
    let id_one = Uuid::parse_str("12345678-0000-0000-0000-000000000001").unwrap();
    let id_two = Uuid::parse_str("12345678-0000-0000-0000-000000000002").unwrap();

    let one = placed(&d, Some(id_one), None);
    let two = placed(&d, Some(id_two), None);

    let mut session = vec![ScenarioEvent::new("PLAYER", 10, false, true)];
    session[0].actions.push(against_rome_environment::ScenarioAction::message("Hello"));

    WildLairScenarioEventBinder::sync_lair_events(&mut session, &one, &d, 0, &aliases, 256.0).unwrap();
    WildLairScenarioEventBinder::sync_lair_events(&mut session, &two, &d, 0, &aliases, 256.0).unwrap();
    WildLairScenarioEventBinder::sync_lair_events(&mut session, &one, &d, 0, &aliases, 256.0).unwrap();

    assert_eq!(session.len(), 3);
    let before: Vec<String> = session.iter().map(|e| e.name.clone()).collect();

    let mut invalid_d = d.clone();
    invalid_d.loot.wood = 1;
    assert!(WildLairScenarioEventBinder::sync_lair_events(&mut session, &one, &invalid_d, 0, &aliases, 256.0).is_err());
    let after: Vec<String> = session.iter().map(|e| e.name.clone()).collect();
    assert_eq!(before, after);

    assert!(WildLairScenarioEventBinder::validate_lair_bindings(&session, &[one.clone(), two]).is_empty());
    assert_eq!(WildLairScenarioEventBinder::remove_lair_events(&mut session, one.instance_id), 1);
    assert_eq!(session.len(), 2);
    assert_eq!(WildLairScenarioEventBinder::validate_lair_bindings(&session, &[]).len(), 1);
}

#[test]
fn capacity_failure_preserves_existing_events() {
    let d = blueprint();
    let l = placed(&d, None, None);
    let aliases = sample_aliases();
    let mut session = Vec::new();
    for i in 0..256 {
        session.push(ScenarioEvent::new(format!("PLAYER_{i}"), 1, false, true));
    }

    assert!(WildLairScenarioEventBinder::sync_lair_events(&mut session, &l, &d, 0, &aliases, 256.0).is_err());
    assert_eq!(session.len(), 256);
    assert!(session.iter().all(|e| e.name.starts_with("PLAYER_")));
}

#[test]
fn real_resource_requests_are_explicit_and_timed_game_regeneration_is_gated() {
    let planner = ResourceRegenerationPlanner;
    for name in [
        "LanGerNad00_Tanne_gross",
        "LanBriLau00_Laubbaum_gross",
        "LanGerSte00_1Stein",
        "LanItaWei00_Weizenfeld",
        "BauGerMin00_Mine",
        "BauGerGol00_Goldschmiede",
    ] {
        let request = planner.plan_replacement(name, 4000.0, 6000.0, 120, 8).expect("valid replacement");
        assert_eq!(request.team, 8);
        assert_eq!(request.delay_seconds, 120);
        assert!(WildLairScenarioEventBinder::generate_events_for_resource(&request).is_err());
    }
}

#[test]
fn resource_planner_rejects_guessed_names_and_requires_real_editor_template() {
    let p = ResourceRegenerationPlanner;
    assert!(p.plan_replacement("LanGerTanne01", 1.0, 1.0, 60, 8).is_err());
    assert!(p.plan_replacement("LanGerNad00_Tanne_gross", f32::INFINITY, 1.0, 60, 8).is_err());

    let r = p.plan_replacement("LanGerNad00_Tanne_gross", 4000.0, 6000.0, 60, 8).unwrap();
    assert!(p.create_editor_addition(&r, |_| None, 0.0).is_err());
    // Mismatched definition id
    assert!(p.create_editor_addition(&r, |_| Some(999), 0.0).is_err());

    let stone = p.plan_replacement("LanGerSte00_1Stein", 4000.0, 6000.0, 60, 8).unwrap();
    let addition = p.create_editor_addition(&stone, |name| if name == stone.name_def { Some(0) } else { None }, 150.0).unwrap();
    assert_eq!(addition.name, stone.name_def);
    assert_eq!(addition.x, 4000.0);
    assert_eq!(addition.z, 6000.0);
    assert_eq!(addition.y, 150.0);

    let mine = p.plan_replacement("BauGerMin00_Mine", 1.0, 1.0, 60, 8).unwrap();
    assert!(p.create_editor_addition(&mine, |_| Some(43), 0.0).is_err());
}
