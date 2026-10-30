//! Map Diff against a saved baseline map folder: layer coverage, unavailable states, no false PASS.

use against_rome_editor_app::bmp::BmpImage;
use against_rome_editor_app::packaging_ui::{
    load_saved_snapshot, BaselineSource, DiffVerdict, LayerState, LiveDiffSessions, PackagingUi,
};
use against_rome_placement::{COLUMNS, SEGMENTS};
use against_rome_scenario::model::{DataSlot, ScenarioDocument, Spawn};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("arm-map-diff-{}", Uuid::new_v4()));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn sub(&self, name: &str) -> PathBuf {
        let p = self.0.join(name);
        fs::create_dir_all(&p).unwrap();
        p
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone)]
struct Spec {
    texture: &'static str,
    events: &'static str,
    obj1_x: f32,
}
const BASE: Spec = Spec {
    texture: "4BA___51",
    events: "[]",
    obj1_x: 20.0,
};

fn write_objects(map: &Path, obj1_x: f32) {
    let data = map.join("DATA");
    fs::create_dir_all(&data).unwrap();
    let n = 4usize; // Three active objects plus spare capacity for unsaved clone tests.
    let mut objects = vec![0u8; 16 + n * (79 + COLUMNS.iter().sum::<usize>())];
    objects[..4].copy_from_slice(&1u32.to_le_bytes());
    objects[4..8].copy_from_slice(&(n as u32).to_le_bytes());
    objects[8..12].copy_from_slice(&30u32.to_le_bytes());
    objects[12..16].copy_from_slice(&30u32.to_le_bytes());
    for slot in 0..n {
        let r = 16 + slot * 79;
        objects[r] = 1;
        objects[r + 3..r + 7].copy_from_slice(&(slot as u32 + 1).to_le_bytes());
        for offset in [67, 69] {
            objects[r + offset..r + offset + 2].copy_from_slice(&(slot as u16).to_le_bytes());
        }
        for offset in [71, 73, 77] {
            objects[r + offset..r + offset + 2].fill(255);
        }
        for col in 0..18 {
            let offset =
                16 + n * 79 + n * COLUMNS[..col].iter().sum::<usize>() + slot * COLUMNS[col];
            if COLUMNS[col] == 2 {
                objects[offset..offset + 2].fill(255);
            }
            if col == 1 {
                objects[offset..offset + 2].copy_from_slice(&(slot as u16).to_le_bytes());
            }
        }
    }
    let mut position = vec![0u8; 8 + n * 17];
    position[..4].copy_from_slice(&1u32.to_le_bytes());
    position[4..8].copy_from_slice(&(n as u32).to_le_bytes());
    for slot in 0..n {
        position[8 + slot * 17] = 1;
        let x = if slot == 1 {
            obj1_x
        } else {
            slot as f32 * 20.0
        };
        position[9 + slot * 17..13 + slot * 17].copy_from_slice(&x.to_le_bytes());
    }
    let mut objdata = vec![0u8; 8 + n * SEGMENTS.iter().sum::<usize>()];
    objects[16 + 3 * 79] = 0;
    position[8 + 3 * 17] = 0;
    objdata[..4].copy_from_slice(&1u32.to_le_bytes());
    objdata[4..8].copy_from_slice(&(n as u32).to_le_bytes());
    for (name, bytes) in [
        ("objects", objects),
        ("position", position),
        ("objdata", objdata),
    ] {
        fs::write(data.join(format!("{name}.dat")), bytes).unwrap();
    }
}

fn write_map(map: &Path, spec: &Spec) {
    fs::write(
        map.join("boden.bmp"),
        BmpImage::new_solid(5, 5, 0, 50, 0).encode_bmp(),
    )
    .unwrap();
    fs::write(
        map.join("collision.bmp"),
        BmpImage::new_solid(4, 4, 0, 0, 0).encode_bmp(),
    )
    .unwrap();
    let t = spec.texture;
    fs::write(
        map.join("boden.txt"),
        format!(
            "[Dimension]\r\n4\r\n[Texturen]\r\n{}{t}\r\n",
            "4BA___51\r\n".repeat(15)
        ),
    )
    .unwrap();
    write_objects(map, spec.obj1_x);
    fs::create_dir_all(map.join("SCRIPT")).unwrap();
    fs::write(
        map.join("SCRIPT/ak_level.bci"),
        against_rome_scenario::BciImage::idle().serialize(),
    )
    .unwrap();
    let mut doc = bound_scenario();
    doc.events = serde_json::from_str(spec.events).unwrap();
    write_scenario(map, &doc);
}

fn bound_scenario() -> ScenarioDocument {
    let spawns: Vec<_> = (0..3)
        .map(|slot| Spawn {
            id: Uuid::from_u128(100 + slot),
            alias: format!("Unit{slot}"),
            count: 5,
            prebuilt: true,
            ..Default::default()
        })
        .collect();
    let data_slots = spawns
        .iter()
        .enumerate()
        .map(|(slot, spawn)| DataSlot {
            slot: slot as i32,
            uid: slot as u32 + 1,
            spawn_id: spawn.id,
        })
        .collect();
    ScenarioDocument {
        spawns,
        data_slots,
        ..Default::default()
    }
}

fn write_scenario(map: &Path, doc: &ScenarioDocument) {
    fs::write(map.join("arm_scenario.json"), doc.to_json().unwrap()).unwrap();
}

fn compare_live(
    ui: &mut PackagingUi,
    label: &str,
    dimension: usize,
    heights: Option<&[u8]>,
    collision: Option<&[u8]>,
    map: Option<&Path>,
) -> Result<(), String> {
    let mut textures = against_rome_editor_app::textures_ui::TexturesUi::default();
    let mut placement = against_rome_editor_app::placement_ui::PlacementUi::default();
    let mut scenario = None;
    if let Some(map) = map {
        let _ = textures.load_session(map);
        let _ = placement.load_session(map);
        scenario = fs::read(map.join("arm_scenario.json"))
            .ok()
            .and_then(|b| ScenarioDocument::from_json(&b).ok());
    }
    ui.compare_live_sessions(
        label,
        dimension,
        heights,
        collision,
        LiveDiffSessions {
            textures: &textures,
            placement: &placement,
            scenario: scenario.as_ref(),
        },
    )
}
fn compare(base: &Path, cur: &Path) -> PackagingUi {
    let mut ui = PackagingUi::default();
    ui.load_saved_baseline(base).unwrap();
    compare_live(
        &mut ui,
        "cur",
        5,
        Some(&memory_heights()),
        Some(&[0; 16]),
        Some(cur),
    )
    .unwrap();
    ui
}

fn assert_unavailable(ui: &PackagingUi, layer: &str) {
    assert!(
        ui.layer_coverage
            .iter()
            .any(|(n, s)| *n == layer && matches!(s, LayerState::Unavailable(_))),
        "{layer}: {:?}",
        ui.layer_coverage
    );
    assert_ne!(ui.diff_verdict, Some(DiffVerdict::Identical));
    assert!(!ui.diff_summary_text.contains("兩版本完全一致"));
    assert!(!ui.diff_summary_text.contains("Verdict: IDENTICAL"));
}

fn memory_heights() -> Vec<u8> {
    vec![50u8; 25]
}

#[test]
fn saved_baseline_loads_all_layers() {
    let tmp = Temp::new();
    let base = tmp.sub("base");
    write_map(&base, &BASE);
    let (snap, av) = load_saved_snapshot(&base, "b").unwrap();
    assert_eq!(av, Default::default(), "all layers should load");
    assert_eq!(snap.height_dimension, 5);
    assert_eq!(snap.textures.as_ref().unwrap().len(), 16);
    assert_eq!(snap.objects.as_ref().unwrap().len(), 3);
    assert_eq!(snap.events.as_ref().unwrap().len(), 0);
}

#[test]
fn identical_saved_maps_with_all_layers_are_identical() {
    let tmp = Temp::new();
    let (base, cur) = (tmp.sub("base"), tmp.sub("cur"));
    write_map(&base, &BASE);
    write_map(&cur, &BASE);
    let mut ui = PackagingUi::default();
    ui.load_saved_baseline(&base).unwrap();
    assert_eq!(ui.baseline_source, Some(BaselineSource::SavedMap(base)));
    let h = memory_heights();
    compare_live(&mut ui, "cur", 5, Some(&h), Some(&[0; 16]), Some(&cur)).unwrap();
    assert_eq!(
        ui.diff_verdict,
        Some(DiffVerdict::Identical),
        "{}",
        ui.diff_summary_text
    );
    assert!(ui
        .layer_coverage
        .iter()
        .all(|(_, s)| *s == LayerState::Compared));
}

#[test]
fn detects_texture_object_event_height_and_collision_changes() {
    let tmp = Temp::new();
    let (base, cur) = (tmp.sub("base"), tmp.sub("cur"));
    write_map(&base, &BASE);
    write_map(
        &cur,
        &Spec {
            texture: "4BB___51",
            events: "[{\"Name\":\"E1\",\"Actions\":[{\"Kind\":0,\"Text\":\"hi\"}]}]",
            obj1_x: 99.0,
        },
    );
    let mut ui = PackagingUi::default();
    ui.load_saved_baseline(&base).unwrap();
    let mut h = memory_heights();
    h[3] = 80;
    let mut c = vec![0u8; 16];
    c[2] = 255;
    compare_live(&mut ui, "cur", 5, Some(&h), Some(&c), Some(&cur)).unwrap();
    let r = ui.diff_report.as_ref().unwrap();
    assert_eq!(ui.diff_verdict, Some(DiffVerdict::Differences));
    assert_eq!(r.height.changed_vertex_count, 1);
    assert_eq!(r.collision.newly_blocked_count, 1);
    assert_eq!(r.textures.changed_tile_count, 1);
    assert_eq!(r.objects.modified.len(), 1);
    assert_eq!(r.events.added.len(), 1);
    assert!(ui
        .layer_coverage
        .iter()
        .all(|(_, s)| *s == LayerState::Compared));
}

#[test]
fn missing_layers_are_unavailable_not_pass() {
    let tmp = Temp::new();
    let (base, cur) = (tmp.sub("base"), tmp.sub("cur"));
    write_map(&base, &BASE);
    write_map(&cur, &BASE);
    fs::remove_file(base.join("boden.txt")).unwrap();
    fs::remove_file(base.join("arm_scenario.json")).unwrap();
    fs::remove_file(base.join("collision.bmp")).unwrap();
    let mut ui = PackagingUi::default();
    ui.load_saved_baseline(&base).unwrap();
    let h = memory_heights();
    compare_live(&mut ui, "cur", 5, Some(&h), Some(&[0; 16]), Some(&cur)).unwrap();
    assert_eq!(ui.diff_verdict, Some(DiffVerdict::Incomplete));
    let unavailable: Vec<_> = ui
        .layer_coverage
        .iter()
        .filter(|(_, s)| matches!(s, LayerState::Unavailable(_)))
        .map(|(n, _)| *n)
        .collect();
    assert_eq!(
        unavailable,
        [
            "Collision",
            "Textures",
            "Scenario events",
            "Object alias/unit count",
            "Scenario spawns",
            "Scenario data slots"
        ]
    );
    assert!(ui.diff_summary_text.contains("NOT a pass"));
    assert!(!ui.diff_summary_text.contains("IDENTICAL"));
}

#[test]
fn unavailable_layer_hides_changes_instead_of_half_comparing() {
    let tmp = Temp::new();
    let (base, cur) = (tmp.sub("base"), tmp.sub("cur"));
    write_map(&base, &BASE);
    write_map(
        &cur,
        &Spec {
            texture: "4BB___51",
            ..BASE
        },
    );
    fs::write(base.join("arm_scenario.json"), b"{not json").unwrap();
    let mut ui = PackagingUi::default();
    ui.load_saved_baseline(&base).unwrap();
    let h = memory_heights();
    compare_live(&mut ui, "cur", 5, Some(&h), Some(&[0; 16]), Some(&cur)).unwrap();
    assert_eq!(ui.diff_verdict, Some(DiffVerdict::Differences));
    assert!(ui
        .layer_coverage
        .iter()
        .any(|(n, s)| *n == "Scenario events"
            && matches!(s, LayerState::Unavailable(r) if r.contains("arm_scenario.json"))));
}

#[test]
fn dimension_mismatch_is_unavailable() {
    let tmp = Temp::new();
    let base = tmp.sub("base");
    write_map(&base, &BASE);
    let mut ui = PackagingUi::default();
    ui.load_saved_baseline(&base).unwrap();
    let h = vec![50u8; 9 * 9];
    compare_live(&mut ui, "cur", 9, Some(&h), Some(&[0; 64]), None).unwrap();
    assert_eq!(ui.diff_verdict, Some(DiffVerdict::Incomplete));
    for (name, s) in &ui.layer_coverage {
        assert!(
            matches!(s, LayerState::Unavailable(_)),
            "{name} must be unavailable"
        );
    }
    assert!(
        ui.layer_coverage[0].1
            == LayerState::Unavailable("height dimension mismatch (baseline 5, current 9)".into())
    );
}

#[test]
fn memory_baseline_never_claims_identical() {
    let mut ui = PackagingUi::default();
    let h = memory_heights();
    ui.capture_baseline_from_memory("b", 5, Some(&h), None, None);
    ui.compare_snapshots("c", 5, Some(&h), None, None).unwrap();
    assert_eq!(ui.diff_verdict, Some(DiffVerdict::Incomplete));
    assert_eq!(ui.layer_coverage[0].1, LayerState::Compared);
    assert!(matches!(ui.layer_coverage[1].1, LayerState::Unavailable(_)));
}

#[test]
fn invalid_baseline_folder_reports_error_and_keeps_state() {
    let tmp = Temp::new();
    let empty = tmp.sub("empty");
    let mut ui = PackagingUi::default();
    assert!(ui.load_saved_baseline(&empty).is_err());
    assert!(ui.status_is_error);
    assert!(ui.baseline_snapshot.is_none());
    assert!(ui.load_saved_baseline(&tmp.0.join("nope")).is_err());
    // Compare without a baseline is an error, not a pass.
    assert!(compare_live(&mut ui, "c", 5, None, None, None).is_err());
    assert!(ui.diff_verdict.is_none());
}

#[test]
fn loading_baseline_does_not_modify_the_folder() {
    let tmp = Temp::new();
    let base = tmp.sub("base");
    write_map(&base, &BASE);
    let list = |d: &Path| {
        let mut v: Vec<_> = walk(d)
            .into_iter()
            .map(|p| (p.clone(), fs::read(&p).unwrap()))
            .collect();
        v.sort();
        v
    };
    fn walk(d: &Path) -> Vec<PathBuf> {
        let mut out = vec![];
        for e in fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                out.extend(walk(&p));
            } else {
                out.push(p);
            }
        }
        out
    }
    let before = list(&base);
    PackagingUi::default().load_saved_baseline(&base).unwrap();
    assert_eq!(before, list(&base));
}

#[test]
fn bound_alias_and_count_changes_are_reported_as_object_modifications() {
    let tmp = Temp::new();
    let (base, cur) = (tmp.sub("base"), tmp.sub("cur"));
    write_map(&base, &BASE);
    write_map(&cur, &BASE);
    let mut doc = bound_scenario();
    doc.spawns[1].alias = "ChangedAlias".into();
    doc.spawns[1].count = 12;
    write_scenario(&cur, &doc);
    let ui = compare(&base, &cur);
    let report = ui.diff_report.as_ref().unwrap();
    assert_eq!(ui.diff_verdict, Some(DiffVerdict::Differences));
    assert_eq!(report.objects.modified.len(), 1);
    let change = &report.objects.modified[0];
    assert_eq!(change.baseline.alias, "Unit1");
    assert_eq!(change.current.alias, "ChangedAlias");
    assert_eq!(
        (change.baseline.unit_count, change.current.unit_count),
        (5, 12)
    );
    assert_eq!(change.changed_attributes.len(), 2);
    assert!(ui.diff_summary_text.contains("ChangedAlias"));
    assert!(!ui.diff_summary_text.contains("兩版本完全一致"));
}

#[test]
fn every_spawn_field_and_list_add_remove_reorder_is_compared() {
    let tmp = Temp::new();
    let (base, cur) = (tmp.sub("base"), tmp.sub("cur"));
    write_map(&base, &BASE);
    write_map(&cur, &BASE);
    // Non-prebuilt records have no native object counterpart. Their entire typed state still matters.
    let mut original = bound_scenario();
    original.spawns.push(Spawn {
        id: Uuid::from_u128(999),
        alias: "ScriptUnit".into(),
        ..Default::default()
    });
    write_scenario(&base, &original);
    let mut variants = Vec::new();
    for field in 0..11 {
        let mut doc = original.clone();
        let spawn = &mut doc.spawns[3];
        match field {
            0 => spawn.alias = "Different".into(),
            1 => spawn.x = 123.0,
            2 => spawn.y = 17.0,
            3 => spawn.z = 456.0,
            4 => spawn.team = 2,
            5 => spawn.count = 7,
            6 => spawn.angle = 90,
            7 => spawn.id = Uuid::from_u128(888),
            8 => spawn.prebuilt = true,
            9 => {
                spawn.prebuilt = true;
                spawn.raw_rotation = Some(0.0);
            }
            _ => {
                doc.spawns.swap(0, 1);
            }
        }
        variants.push(doc);
    }
    let mut removed = original.clone();
    removed.spawns.pop();
    variants.push(removed);
    for doc in variants {
        write_scenario(&cur, &doc);
        let ui = compare(&base, &cur);
        assert_eq!(
            ui.diff_verdict,
            Some(DiffVerdict::Differences),
            "{}",
            ui.diff_summary_text
        );
        // Existing core report only covers events/native objects, so the supplemental verdict must win.
        assert!(!ui.diff_report.as_ref().unwrap().has_differences());
        assert!(ui.diff_summary_text.contains("--- Scenario spawns"));
        assert!(!ui.diff_summary_text.contains("兩版本完全一致"));
    }
    // Also exercise addition (reverse removal).
    write_scenario(
        &base,
        &ScenarioDocument {
            spawns: original.spawns[..3].to_vec(),
            ..original.clone()
        },
    );
    write_scenario(&cur, &original);
    assert_eq!(
        compare(&base, &cur).diff_verdict,
        Some(DiffVerdict::Differences)
    );
}

#[test]
fn data_slot_uid_target_add_remove_and_order_changes_are_reported() {
    let tmp = Temp::new();
    let (base, cur) = (tmp.sub("base"), tmp.sub("cur"));
    write_map(&base, &BASE);
    write_map(&cur, &BASE);
    for field in 0..5 {
        let mut doc = bound_scenario();
        match field {
            0 => doc.data_slots[0].uid = 900,
            1 => doc.data_slots[0].slot = 10,
            2 => {
                let id = doc.data_slots[0].spawn_id;
                doc.data_slots[0].spawn_id = doc.data_slots[1].spawn_id;
                doc.data_slots[1].spawn_id = id;
            }
            3 => {
                doc.data_slots.pop();
            }
            _ => doc.data_slots.swap(0, 1),
        }
        write_scenario(&cur, &doc);
        let ui = compare(&base, &cur);
        assert_eq!(ui.diff_verdict, Some(DiffVerdict::Differences));
        assert!(ui.diff_summary_text.contains("--- Scenario data slots"));
        assert!(!ui.diff_summary_text.contains("兩版本完全一致"));
    }
    write_scenario(
        &base,
        &ScenarioDocument {
            data_slots: vec![],
            ..bound_scenario()
        },
    );
    write_scenario(&cur, &bound_scenario());
    assert_eq!(
        compare(&base, &cur).diff_verdict,
        Some(DiffVerdict::Differences)
    );
}

#[test]
fn missing_or_stale_bindings_leave_metadata_unavailable_but_compare_geometry() {
    let tmp = Temp::new();
    let (base, cur) = (tmp.sub("base"), tmp.sub("cur"));
    write_map(&base, &BASE);
    write_map(&cur, &BASE);
    for stale in [false, true] {
        let mut doc = bound_scenario();
        if stale {
            doc.data_slots[1].uid = 999;
        } else {
            doc.data_slots.remove(1);
        }
        write_scenario(&base, &doc);
        write_scenario(&cur, &doc);
        let ui = compare(&base, &cur);
        assert_eq!(ui.diff_verdict, Some(DiffVerdict::Incomplete));
        assert_unavailable(&ui, "Object alias/unit count");
        assert!(!ui.diff_report.as_ref().unwrap().has_differences());
    }
    write_objects(&cur, 99.0);
    let ui = compare(&base, &cur);
    assert_eq!(ui.diff_verdict, Some(DiffVerdict::Differences));
    assert_eq!(ui.diff_report.as_ref().unwrap().objects.modified.len(), 1);
    assert_unavailable(&ui, "Object alias/unit count");
}

#[test]
fn one_sided_metadata_is_never_compared_against_fabricated_zero_counts() {
    let tmp = Temp::new();
    let (base, cur) = (tmp.sub("base"), tmp.sub("cur"));
    write_map(&base, &BASE);
    write_map(&cur, &BASE);
    let mut doc = bound_scenario();
    doc.data_slots.clear();
    write_scenario(&cur, &doc);
    let ui = compare(&base, &cur);
    assert_unavailable(&ui, "Object alias/unit count");
    assert!(ui.diff_report.as_ref().unwrap().objects.modified.is_empty());
    assert_eq!(ui.diff_verdict, Some(DiffVerdict::Differences)); // actual binding deletion
}

#[test]
fn malformed_or_missing_scenario_on_either_side_is_unavailable() {
    let tmp = Temp::new();
    let (base, cur) = (tmp.sub("base"), tmp.sub("cur"));
    for path in [&base, &cur] {
        for malformed in [false, true] {
            write_map(&base, &BASE);
            write_map(&cur, &BASE);
            if malformed {
                fs::write(path.join("arm_scenario.json"), b"{oops").unwrap();
            } else {
                fs::remove_file(path.join("arm_scenario.json")).unwrap();
            }
            let ui = compare(&base, &cur);
            assert_eq!(ui.diff_verdict, Some(DiffVerdict::Incomplete));
            for layer in [
                "Scenario events",
                "Scenario spawns",
                "Scenario data slots",
                "Object alias/unit count",
            ] {
                assert_unavailable(&ui, layer);
            }
        }
    }
}

#[test]
fn baseline_scenario_is_frozen_and_reset_by_memory_capture_or_session_reset() {
    let tmp = Temp::new();
    let (base, cur) = (tmp.sub("base"), tmp.sub("cur"));
    write_map(&base, &BASE);
    write_map(&cur, &BASE);
    let mut ui = PackagingUi::default();
    ui.load_saved_baseline(&base).unwrap();
    let mut doc = bound_scenario();
    doc.spawns[0].count = 11;
    write_scenario(&base, &doc);
    compare_live(
        &mut ui,
        "cur",
        5,
        Some(&memory_heights()),
        Some(&[0; 16]),
        Some(&cur),
    )
    .unwrap();
    assert_eq!(ui.diff_verdict, Some(DiffVerdict::Identical));
    ui.capture_baseline_from_memory("memory", 5, Some(&memory_heights()), Some(&[0; 16]), None);
    compare_live(
        &mut ui,
        "cur",
        5,
        Some(&memory_heights()),
        Some(&[0; 16]),
        Some(&cur),
    )
    .unwrap();
    assert_unavailable(&ui, "Scenario spawns");
    ui.reset_session(None);
    assert!(ui.diff_verdict.is_none());
    assert!(ui.layer_coverage.is_empty());
    assert!(ui.compare_snapshots("cur", 5, None, None, None).is_err());
}

#[test]
fn supplied_memory_objects_compare_geometry_but_do_not_trust_unbound_metadata() {
    let tmp = Temp::new();
    let base = tmp.sub("base");
    write_map(&base, &BASE);
    let (snapshot, _) = load_saved_snapshot(&base, "base").unwrap();
    let objects = snapshot.objects.unwrap();
    let mut current = objects.clone();
    current[0].unit_count += 1;
    current[0].x += 10.0;
    let mut ui = PackagingUi::default();
    ui.capture_baseline_from_memory(
        "base",
        5,
        Some(&memory_heights()),
        Some(&[0; 16]),
        Some(&objects),
    );
    ui.compare_snapshots(
        "cur",
        5,
        Some(&memory_heights()),
        Some(&[0; 16]),
        Some(&current),
    )
    .unwrap();
    assert_eq!(ui.diff_report.as_ref().unwrap().objects.modified.len(), 1);
    assert_eq!(ui.diff_verdict, Some(DiffVerdict::Differences));
    assert_unavailable(&ui, "Scenario spawns");
    assert_unavailable(&ui, "Object alias/unit count");
}

#[test]
fn partial_object_metadata_does_not_hide_known_scenario_alias_changes() {
    let tmp = Temp::new();
    let (base, cur) = (tmp.sub("base"), tmp.sub("cur"));
    write_map(&base, &BASE);
    write_map(&cur, &BASE);
    let mut doc = bound_scenario();
    doc.data_slots.pop();
    write_scenario(&base, &doc);
    doc.spawns[0].alias = "ChangedWithPartialBindings".into();
    write_scenario(&cur, &doc);
    let ui = compare(&base, &cur);
    assert_unavailable(&ui, "Object alias/unit count");
    assert_eq!(ui.diff_verdict, Some(DiffVerdict::Differences));
    assert!(ui.diff_summary_text.contains("ChangedWithPartialBindings"));
    assert!(ui.diff_report.as_ref().unwrap().objects.modified.is_empty());
}

struct Sessions {
    textures: against_rome_editor_app::textures_ui::TexturesUi,
    placement: against_rome_editor_app::placement_ui::PlacementUi,
    scenario: against_rome_editor_app::scenario_ui::ScenarioUi,
}
impl Sessions {
    fn load(map: &Path) -> Self {
        let mut s = Self {
            textures: Default::default(),
            placement: Default::default(),
            scenario: Default::default(),
        };
        s.textures.load_session(map).unwrap();
        s.placement.load_session(map).unwrap();
        s.scenario.load_session(map).unwrap();
        assert!(s.placement.is_loaded() && s.scenario.is_loaded());
        s
    }
    fn compare(&self, ui: &mut PackagingUi, h: &[u8], c: &[u8]) {
        ui.compare_live_sessions(
            "live",
            5,
            Some(h),
            Some(c),
            LiveDiffSessions::from_sessions(&self.textures, &self.placement, &self.scenario),
        )
        .unwrap();
    }
}

fn folder_bytes(map: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = vec![];
    for entry in fs::read_dir(map).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(folder_bytes(&path));
        } else {
            files.push((path.clone(), fs::read(path).unwrap()));
        }
    }
    files.sort();
    files
}

#[test]
fn actual_sessions_capture_all_unsaved_layers_without_writing_or_reloading() {
    use against_rome_scenario::model::Event;
    let tmp = Temp::new();
    let map = tmp.sub("live");
    write_map(&map, &BASE);
    let before = folder_bytes(&map);
    let mut ui = PackagingUi::default();
    ui.load_saved_baseline(&map).unwrap();
    let mut s = Sessions::load(&map);
    s.compare(&mut ui, &memory_heights(), &[0; 16]);
    assert_eq!(ui.diff_verdict, Some(DiffVerdict::Identical));

    let mut heights = memory_heights();
    heights[2] = 71;
    let mut collision = [0; 16];
    collision[1] = 255;
    s.textures
        .session_mut()
        .unwrap()
        .stamp_texture(0, 0, "4BB___51")
        .unwrap();
    s.textures.session_mut().unwrap().commit_stroke();
    s.placement
        .store_mut()
        .unwrap()
        .set_family_team(1, 2, 3)
        .unwrap();
    let mut spawns = s.scenario.document().spawns.clone();
    spawns[1].alias = "UnsavedAlias".into();
    spawns[1].count = 9;
    // Deleting a prebuilt spawn also removes its DATA binding in the live document.
    spawns.pop();
    s.scenario.apply_spawns(spawns).unwrap();
    s.scenario
        .add_events(&[serde_json::from_str::<Event>(
            r#"{"Name":"UnsavedEvent","Actions":[{"Kind":0,"Text":"live"}]}"#,
        )
        .unwrap()])
        .unwrap();
    s.compare(&mut ui, &heights, &collision);
    let report = ui.diff_report.as_ref().unwrap();
    assert_eq!(report.height.changed_vertex_count, 1);
    assert_eq!(report.collision.newly_blocked_count, 1);
    assert_eq!(report.textures.changed_tile_count, 1);
    assert_eq!(report.objects.modified.len(), 1);
    assert_eq!(report.events.added.len(), 1);
    assert!(ui.diff_summary_text.contains("UnsavedAlias"));
    assert!(ui.diff_summary_text.contains("--- Scenario data slots"));
    assert_unavailable(&ui, "Object alias/unit count");
    assert_eq!(
        ui.current_snapshot
            .as_ref()
            .unwrap()
            .objects
            .as_ref()
            .unwrap()[1]
            .team,
        3
    );
    assert_eq!(before, folder_bytes(&map));
    assert!(s.textures.is_dirty() && s.scenario.dirty());
    // A second comparison sees session state again and leaves the source directory unchanged.
    s.compare(&mut ui, &heights, &collision);
    assert_eq!(before, folder_bytes(&map));
}

#[test]
fn unsaved_event_action_payload_changes_with_same_count_are_detected() {
    use against_rome_scenario::model::Event;
    let tmp = Temp::new();
    let map = tmp.sub("live");
    write_map(
        &map,
        &Spec {
            events: "[{\"Name\":\"E1\",\"Actions\":[{\"Kind\":0,\"Text\":\"before\"}]}]",
            ..BASE
        },
    );
    let mut ui = PackagingUi::default();
    ui.load_saved_baseline(&map).unwrap();
    let mut s = Sessions::load(&map);
    let mut event: Event = s.scenario.document().events[0].clone();
    event.actions[0].text = "after".into();
    s.scenario.replace_event(0, event).unwrap();
    let before = folder_bytes(&map);
    s.compare(&mut ui, &memory_heights(), &[0; 16]);
    assert_eq!(ui.diff_verdict, Some(DiffVerdict::Differences));
    assert!(ui.diff_summary_text.contains("after"));
    assert_eq!(before, folder_bytes(&map));
    s.scenario.remove_event(0).unwrap();
    s.compare(&mut ui, &memory_heights(), &[0; 16]);
    assert_eq!(ui.diff_report.as_ref().unwrap().events.deleted.len(), 1);
}

#[test]
fn unsaved_alias_count_and_spawn_addition_use_the_loaded_scenario_document() {
    let tmp = Temp::new();
    let map = tmp.sub("live");
    write_map(&map, &BASE);
    let before = folder_bytes(&map);
    let mut ui = PackagingUi::default();
    ui.load_saved_baseline(&map).unwrap();
    let mut s = Sessions::load(&map);
    let mut spawns = s.scenario.document().spawns.clone();
    spawns[0].alias = "LiveAlias".into();
    spawns[0].count = 12;
    spawns.push(Spawn {
        id: Uuid::new_v4(),
        alias: "NewScriptSpawn".into(),
        ..Default::default()
    });
    s.scenario.apply_spawns(spawns).unwrap();
    s.compare(&mut ui, &memory_heights(), &[0; 16]);
    assert_eq!(ui.diff_verdict, Some(DiffVerdict::Differences));
    assert_eq!(ui.layer_coverage[5].1, LayerState::Compared);
    assert_eq!(
        ui.diff_report.as_ref().unwrap().objects.modified[0]
            .current
            .alias,
        "LiveAlias"
    );
    assert!(ui.diff_summary_text.contains("NewScriptSpawn"));
    assert_eq!(before, folder_bytes(&map));
}

#[test]
fn unsaved_object_removal_is_not_replaced_by_disk_objects() {
    let tmp = Temp::new();
    let map = tmp.sub("live");
    write_map(&map, &BASE);
    let before = folder_bytes(&map);
    let mut ui = PackagingUi::default();
    ui.load_saved_baseline(&map).unwrap();
    let mut s = Sessions::load(&map);
    s.placement
        .store_mut()
        .unwrap()
        .remove_family(1, 2)
        .unwrap();
    s.compare(&mut ui, &memory_heights(), &[0; 16]);
    assert_eq!(ui.diff_report.as_ref().unwrap().objects.deleted.len(), 1);
    assert_eq!(
        ui.current_snapshot
            .as_ref()
            .unwrap()
            .objects
            .as_ref()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(before, folder_bytes(&map));
}

#[test]
fn absent_actual_sessions_and_bad_buffer_dimensions_never_use_valid_disk_values() {
    let tmp = Temp::new();
    let map = tmp.sub("live");
    write_map(&map, &BASE);
    let before = folder_bytes(&map);
    let mut ui = PackagingUi::default();
    ui.load_saved_baseline(&map).unwrap();
    ui.map_dir_input = map.display().to_string();
    let textures = Default::default();
    let placement = Default::default();
    let scenario = Default::default();
    ui.compare_live_sessions(
        "missing",
        5,
        Some(&[50; 24]),
        Some(&[0; 15]),
        LiveDiffSessions::from_sessions(&textures, &placement, &scenario),
    )
    .unwrap();
    assert_eq!(ui.diff_verdict, Some(DiffVerdict::Incomplete));
    assert!(ui
        .layer_coverage
        .iter()
        .all(|(_, state)| matches!(state, LayerState::Unavailable(_))));
    ui.compare_with_current_map(
        "legacy",
        5,
        Some(&memory_heights()),
        Some(&[0; 16]),
        Some(&map),
    )
    .unwrap();
    assert_eq!(ui.diff_verdict, Some(DiffVerdict::Incomplete));
    for name in [
        "Textures",
        "Objects",
        "Scenario events",
        "Scenario spawns",
        "Scenario data slots",
    ] {
        assert_unavailable(&ui, name);
    }
    assert_eq!(before, folder_bytes(&map));
}

#[test]
fn texture_session_dimension_mismatch_and_ambiguous_metadata_are_unavailable() {
    let tmp = Temp::new();
    let map = tmp.sub("live");
    write_map(&map, &BASE);
    let mut ui = PackagingUi::default();
    ui.load_saved_baseline(&map).unwrap();
    fs::write(
        map.join("boden.txt"),
        "[Dimension]\n2\n[Texturen]\n4BA___51\n4BA___51\n4BA___51\n4BA___51\n",
    )
    .unwrap();
    let s = Sessions::load(&map);
    s.compare(&mut ui, &memory_heights(), &[0; 16]);
    assert_unavailable(&ui, "Textures");
    assert_eq!(ui.diff_verdict, Some(DiffVerdict::Incomplete));
    // Texture grids have their own dimensions, independent of terrain vertices.
    ui.load_saved_baseline(&map).unwrap();
    assert!(ui.baseline_availability.textures.is_none());
    s.compare(&mut ui, &memory_heights(), &[0; 16]);
    assert_eq!(ui.diff_verdict, Some(DiffVerdict::Identical));

    write_map(&map, &BASE);
    ui.load_saved_baseline(&map).unwrap();
    let s = Sessions::load(&map);
    for variant in 0..4 {
        let mut doc = s.scenario.document().clone();
        match variant {
            0 => doc.spawns.push(doc.spawns[0].clone()),
            1 => doc.data_slots.push(doc.data_slots[0].clone()),
            2 => doc.data_slots[1].spawn_id = doc.data_slots[0].spawn_id,
            _ => doc.spawns[0].prebuilt = false,
        }
        ui.compare_live_sessions(
            "ambiguous",
            5,
            Some(&memory_heights()),
            Some(&[0; 16]),
            LiveDiffSessions {
                textures: &s.textures,
                placement: &s.placement,
                scenario: Some(&doc),
            },
        )
        .unwrap();
        assert_unavailable(&ui, "Object alias/unit count");
        assert!(ui.diff_report.as_ref().unwrap().objects.modified.is_empty());
    }
}

#[test]
fn independent_live_texture_and_collision_grids_compare_at_their_own_dimensions() {
    let tmp = Temp::new();
    let map = tmp.sub("independent_grids");
    write_map(&map, &BASE);
    fs::write(
        map.join("boden.txt"),
        format!("[Dimension]\n2\n[Texturen]\n{}", "4BA___51\n".repeat(4)),
    )
    .unwrap();
    fs::write(
        map.join("collision.bmp"),
        BmpImage::new_solid(3, 3, 0, 0, 0).encode_bmp(),
    )
    .unwrap();
    let before = folder_bytes(&map);
    let mut ui = PackagingUi::default();
    ui.load_saved_baseline(&map).unwrap();
    let mut sessions = Sessions::load(&map);
    sessions.compare(&mut ui, &memory_heights(), &[0; 9]);
    assert_eq!(ui.diff_verdict, Some(DiffVerdict::Identical));
    assert_eq!(ui.current_snapshot.as_ref().unwrap().tile_dimension, 2);
    assert_eq!(ui.current_snapshot.as_ref().unwrap().collision_dimension, 3);
    sessions
        .textures
        .session_mut()
        .unwrap()
        .stamp_texture(0, 0, "4BB___51")
        .unwrap();
    sessions.textures.session_mut().unwrap().commit_stroke();
    let mut collision = [0; 9];
    collision[0] = 1;
    sessions.compare(&mut ui, &memory_heights(), &collision);
    assert_eq!(ui.diff_verdict, Some(DiffVerdict::Differences));
    let report = ui.diff_report.as_ref().unwrap();
    assert_eq!(report.textures.changed_tile_count, 1);
    assert_eq!(report.collision.newly_blocked_count, 1);
    assert_eq!(before, folder_bytes(&map));
}

#[test]
fn unsaved_object_addition_uses_live_store_and_leaves_metadata_unavailable() {
    let tmp = Temp::new();
    let map = tmp.sub("live");
    write_map(&map, &BASE);
    let before = folder_bytes(&map);
    let mut ui = PackagingUi::default();
    ui.load_saved_baseline(&map).unwrap();
    let mut s = Sessions::load(&map);
    let (slot, uid) = s
        .placement
        .store_mut()
        .unwrap()
        .clone_family_fixed(0, 1, [65536, 0, 0])
        .unwrap();
    s.compare(&mut ui, &memory_heights(), &[0; 16]);
    let objects = &ui.diff_report.as_ref().unwrap().objects;
    assert_eq!(objects.added.len(), 1);
    assert_eq!(
        (objects.added[0].slot as usize, objects.added[0].uid),
        (slot, uid)
    );
    assert_unavailable(&ui, "Object alias/unit count");
    assert_eq!(before, folder_bytes(&map));
}

#[test]
fn each_missing_live_session_is_unavailable_despite_complete_disk_layers() {
    let tmp = Temp::new();
    let map = tmp.sub("live");
    write_map(&map, &BASE);
    let before = folder_bytes(&map);
    for (index, layer) in [(0, "Textures"), (1, "Objects"), (2, "Scenario events")] {
        let mut ui = PackagingUi::default();
        ui.load_saved_baseline(&map).unwrap();
        let mut s = Sessions::load(&map);
        match index {
            0 => s.textures.reset_session(Some(&map)),
            1 => s.placement.reset_session(Some(&map)),
            _ => s.scenario.reset_session(Some(&map)),
        }
        s.compare(&mut ui, &memory_heights(), &[0; 16]);
        assert_unavailable(&ui, layer);
        assert_eq!(ui.diff_verdict, Some(DiffVerdict::Incomplete));
        assert_eq!(ui.layer_coverage[0].1, LayerState::Compared);
        assert_eq!(ui.layer_coverage[1].1, LayerState::Compared);
        if index == 2 {
            assert_unavailable(&ui, "Scenario spawns");
            assert_unavailable(&ui, "Scenario data slots");
            assert_unavailable(&ui, "Object alias/unit count");
        }
    }
    assert_eq!(before, folder_bytes(&map));
}


#[test]
fn test_browse_map_dir_and_baseline_dir_with_injected_picker() {
    let mut ui = PackagingUi::default();
    ui.folder_picker = Some(Box::new(|title| {
        if title.contains("來源地圖") || title.contains("Source Map") {
            Some(PathBuf::from(r"C:\Custom\SourceMap"))
        } else if title.contains("基準地圖") || title.contains("Baseline") {
            Some(PathBuf::from(r"C:\Custom\BaselineMap"))
        } else {
            None
        }
    }));

    ui.browse_map_dir();
    assert_eq!(ui.map_dir_input, r"C:\Custom\SourceMap");

    ui.browse_baseline_dir();
    assert_eq!(ui.baseline_dir_input, r"C:\Custom\BaselineMap");

    // Cancelling preserves existing input
    ui.folder_picker = Some(Box::new(|_| None));
    ui.browse_map_dir();
    assert_eq!(ui.map_dir_input, r"C:\Custom\SourceMap");
    ui.browse_baseline_dir();
    assert_eq!(ui.baseline_dir_input, r"C:\Custom\BaselineMap");
}

#[test]
fn test_capture_baseline_from_live_sessions_captures_all_layers_and_detects_edits() {
    let tmp = Temp::new();
    let map = tmp.sub("live_baseline");
    write_map(&map, &BASE);
    let s = Sessions::load(&map);
    let h = memory_heights();
    let c = [0u8; 16];

    let mut ui = PackagingUi::default();
    ui.capture_baseline_from_live_sessions(
        "Working Baseline",
        5,
        Some(&h),
        Some(&c),
        LiveDiffSessions {
            textures: &s.textures,
            placement: &s.placement,
            scenario: s.scenario.is_loaded().then(|| s.scenario.document()),
        },
    );

    assert_eq!(ui.baseline_source, Some(BaselineSource::Memory));
    assert!(ui.baseline_snapshot.is_some());
    assert!(ui.baseline_scenario.is_some());
    assert!(ui.baseline_availability.textures.is_none());
    assert!(ui.baseline_availability.objects.is_none());
    assert!(ui.baseline_availability.events.is_none());
    assert!(ui.baseline_availability.spawns.is_none());
    assert!(ui.baseline_availability.data_slots.is_none());

    // Comparing against identical live sessions reports Identical across all layers
    s.compare(&mut ui, &h, &c);
    assert_eq!(ui.diff_verdict, Some(DiffVerdict::Identical));
    assert!(ui.layer_coverage.iter().all(|(_, state)| *state == LayerState::Compared));

    // Modifying an object in memory triggers Differences without touching disk
    let mut modified_s = Sessions::load(&map);
    modified_s
        .placement
        .store_mut()
        .unwrap()
        .translate_family(0, 1, [100.0, 0.0, 0.0])
        .unwrap();

    modified_s.compare(&mut ui, &h, &c);
    assert_eq!(ui.diff_verdict, Some(DiffVerdict::Differences));
    let report = ui.diff_report.as_ref().unwrap();
    assert_eq!(report.objects.modified.len(), 1);
    assert_eq!(report.objects.modified[0].baseline.slot, 0);
}
