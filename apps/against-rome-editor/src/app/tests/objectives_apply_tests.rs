use super::*;
use against_rome_campaign::objectives::{ObjectiveDefinition, ObjectiveKind};
use against_rome_scenario::{Action, Event, Spawn};
use uuid::Uuid;

fn fixture(tag: &str) -> (EditorApp, PathBuf, PathBuf) {
    let (mut app, root, map) = spawn_save_tests::fixture(tag, &[]);
    let spawn = Spawn {
        id: Uuid::new_v4(),
        alias: "TROOP".into(),
        x: 64.,
        z: 64.,
        count: 1,
        ..Default::default()
    };
    app.scenario.apply_spawns(vec![spawn.clone()]).unwrap();
    let mut objective = ObjectiveDefinition::default();
    objective.kind = ObjectiveKind::DestroyBuilding;
    objective.parameters.target_guids = vec![spawn.id];
    app.campaign.objectives = vec![objective];
    (app, root, map)
}

fn preview(app: &mut EditorApp) {
    app.campaign
        .compile_objectives_to_events(Some(app.scenario.document()), app.scenario.known_aliases())
        .unwrap();
}

fn apply(app: &mut EditorApp) {
    app.campaign.request_objective_apply();
    app.apply_campaign_objective_request();
    assert!(
        !app.campaign.status_is_error,
        "{}",
        app.campaign.status_message
    );
}

fn filler(i: usize) -> Event {
    Event {
        name: format!("user_{i}"),
        actions: vec![Action {
            text: "keep".into(),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn reject_unchanged(app: &mut EditorApp) {
    let before = app.scenario.document().clone();
    let draft = serde_json::to_string(&app.campaign.objectives).unwrap();
    let preview = app.campaign.last_compiled_objective_events.clone();
    let undo = app.scenario.can_undo_events();
    let redo = app.scenario.can_redo_events();
    let dirty = app.scenario.dirty();
    app.campaign.request_objective_apply();
    app.apply_campaign_objective_request();
    assert!(
        app.campaign.status_is_error,
        "{}",
        app.campaign.status_message
    );
    assert_eq!(app.scenario.document(), &before);
    assert_eq!(
        serde_json::to_string(&app.campaign.objectives).unwrap(),
        draft
    );
    assert_eq!(app.campaign.last_compiled_objective_events, preview);
    assert_eq!(app.scenario.can_undo_events(), undo);
    assert_eq!(app.scenario.can_redo_events(), redo);
    assert_eq!(app.scenario.dirty(), dirty);
}

#[test]
fn objectives_apply_valid_explicit_request_and_duplicate_are_one_undo_step() {
    let (mut app, root, _) = fixture("objective_valid");
    preview(&mut app);
    app.apply_campaign_objective_request();
    assert!(
        app.scenario.document().events.is_empty(),
        "preview is read only"
    );
    assert!(app.campaign.has_unsupported_edits());
    apply(&mut app);
    assert_eq!(app.scenario.document().events.len(), 1);
    assert!(!app.campaign.has_unsupported_edits());
    assert!(app.scenario.dirty());
    let applied = app.scenario.document().clone();
    apply(&mut app);
    assert_eq!(app.scenario.document(), &applied);
    assert!(app.scenario.undo_events());
    assert!(app.scenario.document().events.is_empty());
    assert!(
        !app.scenario.can_undo_events(),
        "duplicate must not add an undo step"
    );
    assert!(app.scenario.redo_events());
    assert_eq!(app.scenario.document(), &applied);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn objectives_apply_replaces_by_uuid_after_title_change_and_undo_redo() {
    let (mut app, root, _) = fixture("objective_replace");
    app.scenario.add_events(&[filler(0)]).unwrap();
    apply(&mut app);
    let before = app.scenario.document().clone();
    app.campaign.objectives[0].title = "Renamed objective".into();
    assert!(app.campaign.has_unsupported_edits());
    apply(&mut app);
    assert_eq!(app.scenario.document().events.len(), 2);
    assert_eq!(app.scenario.document().events[0], before.events[0]);
    assert_eq!(
        app.scenario.document().events[1].name,
        before.events[1].name
    );
    assert_ne!(
        app.scenario.document().events[1].actions,
        before.events[1].actions
    );
    let after = app.scenario.document().clone();
    assert!(app.scenario.undo_events());
    assert_eq!(app.scenario.document(), &before);
    assert!(app.scenario.redo_events());
    assert_eq!(app.scenario.document(), &after);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn objectives_apply_cap_failure_preserves_draft_preview_and_history() {
    let (mut app, root, _) = fixture("objective_cap");
    preview(&mut app);
    app.scenario
        .add_events(&(0..256).map(filler).collect::<Vec<_>>())
        .unwrap();
    reject_unchanged(&mut app);
    assert!(app.campaign.status_message.contains("256"));
    assert!(app.scenario.undo_events());
    assert!(app.scenario.document().events.is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn objectives_apply_compiler_failure_preserves_draft_preview_and_redo() {
    let (mut app, root, _) = fixture("objective_compile_error");
    preview(&mut app);
    app.scenario.add_events(&[filler(0)]).unwrap();
    app.scenario.undo_events();
    app.campaign.objectives[0].kind = ObjectiveKind::EliminateAllEnemies;
    reject_unchanged(&mut app);
    assert!(app
        .campaign
        .status_message
        .contains("OBJ_EXPORT_UNSUPPORTED"));
    assert!(app.scenario.redo_events());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn objectives_apply_duplicate_objective_id_preserves_everything() {
    let (mut app, root, _) = fixture("objective_input_error");
    preview(&mut app);
    app.campaign
        .objectives
        .push(app.campaign.objectives[0].clone());
    reject_unchanged(&mut app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn objectives_apply_rejects_existing_generated_name_collision() {
    let (mut app, root, _) = fixture("objective_name_collision");
    preview(&mut app);
    let generated_name = app.campaign.last_compiled_objective_events[0].name.clone();
    app.scenario
        .add_events(&[Event {
            name: generated_name,
            actions: vec![Action {
                text: "user authored".into(),
                ..Default::default()
            }],
            ..Default::default()
        }])
        .unwrap();
    reject_unchanged(&mut app);
    assert!(app.campaign.status_message.contains("conflicts"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn objectives_apply_rejects_manually_edited_generated_event() {
    let (mut app, root, _) = fixture("objective_manual_edit");
    apply(&mut app);
    let mut edited = app.scenario.document().events[0].clone();
    edited.actions[0].text = "user edit that must survive".into();
    app.scenario.replace_event(0, edited).unwrap();
    let preserved = app.scenario.document().clone();
    let undo = app.scenario.can_undo_events();
    let redo = app.scenario.can_redo_events();
    app.campaign.request_objective_apply();
    app.apply_campaign_objective_request();
    assert!(app.campaign.status_is_error);
    assert!(app.campaign.status_message.contains("was edited"));
    assert_eq!(app.scenario.document(), &preserved);
    assert_eq!(app.scenario.can_undo_events(), undo);
    assert_eq!(app.scenario.can_redo_events(), redo);
    assert!(app.scenario.document().events[0].actions[0]
        .text
        .contains("user edit"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn objectives_apply_revalidates_removed_live_spawn_after_preview() {
    let (mut app, root, _) = fixture("objective_stale_spawn");
    preview(&mut app);
    app.scenario.apply_spawns(vec![]).unwrap();
    reject_unchanged(&mut app);
    assert!(app.campaign.status_message.contains("OBJ_SCENARIO_INVALID"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn objectives_apply_recompiles_changed_draft_after_preview_and_keeps_live_events() {
    let (mut app, root, _) = fixture("objective_stale_preview");
    preview(&mut app);
    app.campaign.objectives[0].title = "Live draft".into();
    app.scenario.add_events(&[filler(0)]).unwrap();
    apply(&mut app);
    assert_eq!(app.scenario.document().events[0], filler(0));
    assert_eq!(
        app.scenario.document().events[1].actions[0].text,
        "Objective completed: Live draft"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn objectives_apply_save_reload_raw_and_pfil() {
    for pfil in [false, true] {
        let (mut app, root, map) = fixture("objective_save");
        if pfil {
            let raw = against_rome_scenario::BciImage::idle().serialize();
            let mut bytes = vec![0; 64];
            bytes[..4].copy_from_slice(b"PFIL");
            bytes[16..20].copy_from_slice(&(raw.len() as u32).to_le_bytes());
            for chunk in raw.chunks(8) {
                bytes.push(((1u16 << chunk.len()) - 1) as u8);
                bytes.extend_from_slice(chunk);
            }
            std::fs::write(map.join("SCRIPT/ak_level.bci"), bytes).unwrap();
            let spawns = app.scenario.document().spawns.clone();
            app.scenario.load_session(&map).unwrap();
            app.scenario.apply_spawns(spawns).unwrap();
        }
        preview(&mut app);
        let disk = save_file_snapshot(&map);
        apply(&mut app);
        assert_eq!(save_file_snapshot(&map), disk, "Apply writes no files");
        let expected = app.scenario.document().clone();
        app.safe_save_current();
        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(!app.scenario.dirty() && !app.campaign.is_dirty());
        let mut reloaded = crate::scenario_ui::ScenarioUi::default();
        reloaded.load_session(&map).unwrap();
        assert_eq!(reloaded.document(), &expected);
        assert!(!reloaded.dirty());
        let bci = std::fs::read(map.join("SCRIPT/ak_level.bci")).unwrap();
        let raw = if pfil {
            against_rome_data::lzss::decompress_pfil(&bci, None).unwrap()
        } else {
            bci
        };
        assert!(against_rome_scenario::BciImage::parse(&raw).is_ok());
        apply(&mut app);
        assert!(
            !app.scenario.dirty(),
            "duplicate Apply after save is a no-op"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn objectives_apply_does_not_clear_other_campaign_edits() {
    let (mut app, root, _) = fixture("objective_other_dirty");
    app.campaign.mark_dirty();
    apply(&mut app);
    assert!(app.campaign.has_unsupported_edits());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn objectives_apply_unloaded_scenario_preserves_draft_and_preview() {
    let (mut app, root, _) = fixture("objective_unloaded");
    preview(&mut app);
    app.scenario.reset_session(None);
    reject_unchanged(&mut app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn objectives_apply_duplicate_keeps_later_live_event_order_and_redo() {
    let (mut app, root, _) = fixture("objective_order");
    apply(&mut app);
    app.scenario.add_events(&[filler(0)]).unwrap();
    app.scenario.add_events(&[filler(1)]).unwrap();
    app.scenario.undo_events();
    let before = app.scenario.document().clone();
    apply(&mut app);
    assert_eq!(app.scenario.document(), &before);
    assert!(app.scenario.can_redo_events());
    assert!(app.scenario.undo_events());
    assert_eq!(app.scenario.document().events.len(), 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn objectives_apply_replaces_two_survival_events_and_removes_obsolete_failure() {
    let (mut app, root, _) = fixture("objective_survival");
    app.campaign.objectives[0].kind = ObjectiveKind::Survival;
    app.campaign.objectives[0].parameters.hold_duration_seconds = 30;
    apply(&mut app);
    assert_eq!(app.scenario.document().events.len(), 2);
    let before = app.scenario.document().clone();
    app.campaign.objectives[0].parameters.hold_duration_seconds = 60;
    apply(&mut app);
    assert_eq!(app.scenario.document().events[1].delay_seconds, 60);
    assert!(app.scenario.undo_events());
    assert_eq!(app.scenario.document(), &before);
    assert!(app.scenario.redo_events());
    app.campaign.objectives[0].kind = ObjectiveKind::DestroyBuilding;
    app.campaign.objectives[0].parameters.hold_duration_seconds = 0;
    apply(&mut app);
    assert_eq!(app.scenario.document().events.len(), 1);
    assert!(app.scenario.document().events[0]
        .name
        .starts_with("OBJ_WIN_"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn objectives_apply_live_validation_failure_is_unchanged() {
    let (mut app, root, _) = fixture("objective_validation");
    preview(&mut app);
    app.scenario.add_events(&[filler(0)]).unwrap();
    let mut invalid = filler(0);
    invalid.delay_seconds = -1;
    app.scenario.replace_event(0, invalid).unwrap();
    reject_unchanged(&mut app);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn objectives_apply_nonlossless_transaction_is_rejected_without_changes() {
    let (mut app, root, _) = fixture("objective_lossless");
    apply(&mut app);
    let mut event = filler(0);
    event.actions[0].count = 42; // Inactive fields must also survive a graph conversion.
    app.scenario.add_events(&[event]).unwrap();
    app.campaign.objectives[0].title = "Change".into();
    reject_unchanged(&mut app);
    assert!(app
        .campaign
        .status_message
        .contains("would alter existing events"));
    std::fs::remove_dir_all(root).unwrap();
}
