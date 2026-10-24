//! Synthetic current-session Settlement -> Placement + Nature -> transaction regressions.
use super::*;
use against_rome_maps::TransactionFailurePoint as Fail;
use against_rome_placement::ObjectStore;

pub(super) fn fixture(tag: &str, include_nature: bool, capacity: usize) -> (EditorApp, PathBuf, PathBuf) {
    let (mut app, root, map) = spawn_save_tests::fixture(tag, &[]);
    std::fs::write(
        map.join(against_rome_maps::CustomMapManifest::MARKER_FILE_NAME),
        r#"{"format":"AgainstRomeModifier.CustomMap","version":1,"mapName":"ENDL_005"}"#,
    )
    .unwrap();
    app.session = Some(TerrainSession::create_blank(129, 20));
    app.water_surface = 0.0;
    app.height_step = 4.0;
    safe_save_map(&map, app.session.as_mut().unwrap()).unwrap();
    app.environment.settlement_players = 2;
    app.environment.settlement_include_nature = include_nature;
    generate(&mut app);
    let dist = app.environment.last_distribution.as_ref().unwrap();
    let mut names = BTreeSet::new();
    for p in &dist.players {
        names.extend(p.buildings.iter().map(|x| x.type_name.clone()));
        names.extend(p.wildlife_and_food.iter().map(|x| x.type_name.clone()));
        names.extend(
            p.forest_trees
                .iter()
                .chain(&p.stone_quarries)
                .map(|x| x.type_name.clone()),
        );
    }
    let mut definitions = String::from("[ObjectDefaults]\n");
    let mut live = Vec::new();
    for (i, name) in names.iter().enumerate() {
        let id = 1000 + i as u16;
        live.push((id, None));
        let mut columns = vec!["0".to_owned(); 53];
        columns[0] = id.to_string();
        columns[1] = "1".into();
        columns[52] = name.clone();
        definitions.push_str(&columns.join(","));
        definitions.push('\n');
    }
    std::fs::write(root.join("SYSTEM/DATA_MP/DEFAULTS/objdef.dau"), definitions).unwrap();
    spawn_save_tests::write_data(&map, capacity, &live, 100);
    app.placement.load_session(&map).unwrap();
    (app, root, map)
}

fn generate(app: &mut EditorApp) {
    let s = app.session.as_ref().unwrap();
    app.environment
        .generate_settlement(
            s.vertex_size,
            Some(s.heights()),
            Some((s.collision_size, s.collision())),
            app.water_surface,
            app.height_step,
        )
        .unwrap();
}

fn counts(app: &EditorApp) -> (usize, usize) {
    let d = app.environment.last_distribution.as_ref().unwrap();
    (
        d.players
            .iter()
            .map(|p| p.buildings.len() + p.wildlife_and_food.len())
            .sum(),
        if app.environment.settlement_include_nature {
            d.players
                .iter()
                .map(|p| p.forest_trees.len() + p.stone_quarries.len())
                .sum()
        } else {
            0
        },
    )
}

fn apply(app: &mut EditorApp) {
    app.handle_environment_apply(EnvApply::Settlement);
    assert!(
        !app.environment.status_is_error,
        "{}",
        app.environment.status_message
    );
}

#[test]
fn current_session_generate_apply_single_undo_redo_safe_save_reload() {
    for include in [true, false] {
        let (mut app, root, map) = fixture("settlement_roundtrip", include, 384);
        // Generation must read the unsaved live terrain and its world height step.
        app.session
            .as_mut()
            .unwrap()
            .apply_height_changes(&[crate::terrain::SampleChange {
                index: 64 * 129 + 64,
                before: 20,
                after: 27,
            }]);
        generate(&mut app);
        let (placed, nature) = counts(&app);
        assert!(placed > 0);
        assert_eq!(nature > 0, include);
        let baseline = app.placement.object_count();
        let disk_before = save_file_snapshot(&map);
        apply(&mut app);
        assert_eq!(app.placement.object_count(), baseline + placed);
        assert_eq!(app.nature_pending_counts(), (nature, 0));
        assert_eq!(save_file_snapshot(&map), disk_before);
        let objects = app.placement.objects();
        let additions = app.nature.session.capture();
        assert!(app.can_undo_any());
        app.perform_undo();
        assert_eq!(app.placement.object_count(), baseline);
        assert_eq!(app.nature_pending_counts(), (0, 0));
        assert!(!app.placement.is_dirty());
        assert!(app.can_redo_any());
        app.perform_redo();
        assert_eq!(app.placement.objects(), objects);
        assert_eq!(app.nature.session.capture(), additions);
        app.safe_save_current();
        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(!app.placement.is_dirty() && !app.nature_is_dirty());
        assert!(app.settlement_undo.is_empty() && app.settlement_redo.is_empty());
        let loaded = ObjectStore::load(&map).unwrap();
        assert_eq!(loaded.objects().len(), baseline + placed + nature);
        for addition in &additions.additions {
            assert!(loaded
                .objects()
                .iter()
                .any(|o| i32::from(o.type_id) == addition.type_id
                    && o.position
                        .iter()
                        .zip([addition.x, addition.y, addition.z])
                        .all(|(a, b)| (a - b).abs() < 0.001)));
        }
        let terrain = load_map(&map).unwrap();
        assert_eq!(terrain.heights(), app.session.as_ref().unwrap().heights());
        let mut reloaded = EditorApp::initial_state();
        reloaded.set_loaded_map(map.clone(), terrain).unwrap();
        reloaded.placement.load_session(&map).unwrap();
        assert_eq!(
            reloaded.placement.object_count(),
            baseline + placed + nature
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn settlement_more_than_96_placements_share_one_history_step() {
    let (mut app, root, map) = fixture("settlement_large", true, 512);
    let p = &mut app.environment.last_distribution.as_mut().unwrap().players[0];
    let b = p.buildings[0].clone();
    p.buildings.extend(std::iter::repeat_n(b, 105));
    let (placed, nature) = counts(&app);
    assert!(placed > 96);
    let baseline = app.placement.object_count();
    apply(&mut app);
    assert_eq!(app.placement.undo_depth(), 1);
    assert_eq!(app.placement.object_count(), baseline + placed);
    assert_eq!(app.nature_pending_counts().0, nature);
    app.perform_undo();
    assert_eq!(app.placement.object_count(), baseline);
    assert_eq!(app.nature_pending_counts(), (0, 0));
    app.perform_redo();
    assert_eq!(app.placement.object_count(), baseline + placed);
    app.safe_save_current();
    assert!(!app.status_is_error, "{}", app.status_message);
    assert_eq!(
        ObjectStore::load(&map).unwrap().objects().len(),
        baseline + placed + nature
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn independent_placement_history_survives_joint_undo_redo() {
    let (mut app, root, _) = fixture("settlement_independent_history", true, 384);
    let baseline = app.placement.object_count();
    apply(&mut app);
    let applied = app.placement.objects();
    let object = applied[0].clone();
    app.placement.select_object(Some((object.slot, object.uid)));
    app.placement.nudge_selection(64., 0.).unwrap();
    let nudged = app.placement.objects();
    app.placement.undo().unwrap();
    app.perform_undo();
    assert_eq!(app.placement.object_count(), baseline);
    assert_eq!(app.nature_pending_counts(), (0, 0));
    app.perform_redo();
    assert_eq!(app.placement.objects(), applied);
    app.placement.redo().unwrap();
    assert_eq!(app.placement.objects(), nudged);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn known_definition_without_cloneable_template_is_unresolved() {
    let (mut app, root, map) = fixture("settlement_missing_template", true, 384);
    let missing = app.environment.last_distribution.as_ref().unwrap().players[0].forest_trees[0]
        .type_name
        .clone();
    let defs = against_rome_maps::NativeObjectDefinitions::load(&root).unwrap();
    let missing_id = defs
        .iter()
        .find(|(_, d)| d.name.as_deref() == Some(&missing))
        .unwrap()
        .0;
    let live: Vec<_> = app
        .placement
        .objects()
        .iter()
        .filter(|o| i32::from(o.type_id) != *missing_id)
        .map(|o| (o.type_id, None))
        .collect();
    spawn_save_tests::write_data(&map, 384, &live, 100);
    app.placement.load_session(&map).unwrap();
    let state = app.placement.capture_state();
    app.handle_environment_apply(EnvApply::Settlement);
    assert!(
        app.environment.status_is_error && app.environment.status_message.contains(&missing),
        "{}",
        app.environment.status_message
    );
    assert!(app.placement.matches_state(&state));
    assert_eq!(app.nature_pending_counts(), (0, 0));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unresolved_names_across_both_destinations_fail_closed_and_keep_plan() {
    let (mut app, root, map) = fixture("settlement_unresolved", true, 384);
    let d = app.environment.last_distribution.as_mut().unwrap();
    d.players[0].buildings[0].type_name = "missing building".into();
    d.players[0].forest_trees[0].type_name = "missing tree".into();
    d.players[0].stone_quarries[0].type_name = "missing rock".into();
    let plan = app.environment.last_distribution.clone();
    let state = app.placement.capture_state();
    let disk = save_file_snapshot(&map);
    app.handle_environment_apply(EnvApply::Settlement);
    for name in ["missing building", "missing tree", "missing rock"] {
        assert!(
            app.environment.status_message.contains(name),
            "{}",
            app.environment.status_message
        );
    }
    assert!(app.environment.status_is_error);
    assert_eq!(app.environment.last_distribution, plan);
    assert!(app.placement.matches_state(&state));
    assert_eq!(app.nature_pending_counts(), (0, 0));
    assert!(app.settlement_undo.is_empty());
    assert_eq!(save_file_snapshot(&map), disk);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unavailable_catalog_reports_every_required_name_without_applying() {
    let (mut app, root, _) = fixture("settlement_no_catalog", true, 384);
    std::fs::remove_file(root.join("SYSTEM/DATA_MP/DEFAULTS/objdef.dau")).unwrap();
    let plan = app.environment.last_distribution.clone().unwrap();
    let state = app.placement.capture_state();
    app.handle_environment_apply(EnvApply::Settlement);
    assert!(app.environment.status_is_error);
    for p in &plan.players {
        for name in p
            .buildings
            .iter()
            .map(|b| &b.type_name)
            .chain(p.wildlife_and_food.iter().map(|w| &w.type_name))
            .chain(
                p.forest_trees
                    .iter()
                    .chain(&p.stone_quarries)
                    .map(|n| &n.type_name),
            )
        {
            assert!(
                app.environment.status_message.contains(name),
                "{}",
                app.environment.status_message
            );
        }
    }
    assert!(app.placement.matches_state(&state));
    assert_eq!(app.nature_pending_counts(), (0, 0));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn late_nature_pool_failure_publishes_neither_candidate() {
    let (mut app, root, map) = fixture("settlement_late", true, 384);
    let templates = app.placement.object_count();
    let (placed, _) = counts(&app);
    // Enough space for all buildings/animals, then run out in the Nature replay.
    let live: Vec<_> = app
        .placement
        .objects()
        .iter()
        .map(|o| (o.type_id, None))
        .collect();
    spawn_save_tests::write_data(&map, templates + placed + 1, &live, 100);
    app.placement.load_session(&map).unwrap();
    let state = app.placement.capture_state();
    let plan = app.environment.last_distribution.clone();
    let disk = save_file_snapshot(&map);
    app.handle_environment_apply(EnvApply::Settlement);
    assert!(
        app.environment.status_is_error
            && app
                .environment
                .status_message
                .contains("Nature: cannot add objects"),
        "{}",
        app.environment.status_message
    );
    assert!(app.placement.matches_state(&state));
    assert_eq!(app.environment.last_distribution, plan);
    assert_eq!(app.nature_pending_counts(), (0, 0));
    assert!(app.settlement_undo.is_empty());
    assert_eq!(save_file_snapshot(&map), disk);
    std::fs::remove_dir_all(root).unwrap();
}

fn pfil(bytes: &[u8]) -> Vec<u8> {
    let mut out = vec![0x5a; 64];
    out[..4].copy_from_slice(b"PFIL");
    out[16..20].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
    for c in bytes.chunks(8) {
        out.push(((1u16 << c.len()) - 1) as u8);
        out.extend_from_slice(c);
    }
    out
}

#[test]
fn save_failure_restores_bytes_and_joint_history_then_retry_preserves_pfil() {
    for failure in [
        Fail::AfterPath("DATA/objects.dat".into()),
        Fail::BeforeFinalCommit,
    ] {
        let (mut app, root, map) = fixture("settlement_save_failure", true, 384);
        for name in ["objects", "position", "objdata", "engine"] {
            let path = map.join(format!("DATA/{name}.dat"));
            std::fs::write(&path, pfil(&std::fs::read(&path).unwrap())).unwrap();
        }
        app.placement.load_session(&map).unwrap();
        let baseline = app.placement.object_count();
        let (placed, nature) = counts(&app);
        apply(&mut app);
        let state = app.placement.capture_state();
        let additions = app.nature.session.capture();
        let before = save_file_snapshot(&map);
        app.safe_save_current_using(|tx| {
            let mut injected =
                std::mem::replace(tx, against_rome_maps::MapSaveTransaction::new("unused"))
                    .with_injected_failure(failure, "settlement save failure");
            injected.commit().map_err(|e| e.to_string())
        });
        assert!(
            app.status_is_error && app.status_message.contains("CleanlyRestored"),
            "{}",
            app.status_message
        );
        assert_eq!(save_file_snapshot(&map), before);
        assert!(app.placement.matches_state(&state));
        assert_eq!(app.nature.session.capture(), additions);
        app.perform_undo();
        assert_eq!(app.placement.object_count(), baseline);
        assert_eq!(app.nature_pending_counts(), (0, 0));
        app.perform_redo();
        app.safe_save_current();
        assert!(!app.status_is_error, "{}", app.status_message);
        assert_eq!(
            ObjectStore::load(&map).unwrap().objects().len(),
            baseline + placed + nature
        );
        for name in ["objects", "position", "objdata", "engine"] {
            let path = format!("DATA/{name}.dat");
            let bytes = std::fs::read(map.join(&path)).unwrap();
            let original = before.get(&PathBuf::from(&path)).unwrap();
            assert_eq!(&bytes[..64], &original[..64]);
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
