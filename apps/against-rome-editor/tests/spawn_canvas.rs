//! 2D canvas interaction of the spawn panel (press-select, drag-move) and the building vs
//! landscape clearance warning (C# `PlaceObjectAt` / `MoveSelectedPlacements` texts).

use against_rome_ambience::Language;
use against_rome_editor_app::scenario_ui::ScenarioUi;
use against_rome_editor_app::spawn_placement::{
    landscape_move_warning, landscape_nearby, landscape_place_warning, nearest_placement,
};
use against_rome_editor_app::spawn_placement_ui::{SpawnContext, SpawnPlacementUi};
use against_rome_maps::sdl_objects::{SdlObjectCategory, SdlObjectType, SdlPlacedObject};
use against_rome_scenario::model::Spawn;
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

fn object_type(name: &str, category: SdlObjectCategory, alias: &str) -> SdlObjectType {
    SdlObjectType::new(name, -1, category, "Rom", 0, HashMap::from([("alias".to_string(), alias.to_string())]))
}

fn catalog() -> Vec<SdlObjectType> {
    vec![
        object_type("BauRomHaus", SdlObjectCategory::Building, "house"),
        object_type("FigRomLeg", SdlObjectCategory::Figure, "legion"),
    ]
}

fn cx() -> SpawnContext<'static> {
    SpawnContext { lang: Language::English, heights: None, vertex_size: 0, height_step: 4.0, read_only: false }
}

struct Fixture {
    map: PathBuf,
    scenario: ScenarioUi,
    ui: SpawnPlacementUi,
}

impl Fixture {
    fn new(spawns: Vec<Spawn>) -> Self {
        let map = std::env::temp_dir().join(format!("arm_spawn_canvas_{}", Uuid::new_v4().simple()));
        std::fs::create_dir_all(map.join("SCRIPT")).unwrap();
        std::fs::write(map.join("SCRIPT/ak_level.bci"), against_rome_scenario::BciImage::idle().serialize()).unwrap();
        let mut scenario = ScenarioUi::default();
        scenario.load_session(&map).unwrap();
        scenario.apply_spawns(spawns).unwrap();
        let mut ui = SpawnPlacementUi::default();
        ui.set_catalog(catalog());
        ui.sync_with_document(&scenario, Some(&map));
        Self { map, scenario, ui }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.map);
    }
}

fn spawn(alias: &str, x: f32, z: f32) -> Spawn {
    Spawn { alias: alias.into(), x, z, team: 0, count: 0, angle: 0, y: 3.0, id: Uuid::new_v4(), ..Default::default() }
}

#[test]
fn nearest_placement_picks_within_radius_and_prefers_the_closest() {
    let mut a = SdlPlacedObject::new(catalog()[0].clone(), 1000.0, 0.0, 1000.0, 0, 0.0, 0);
    let mut b = SdlPlacedObject::new(catalog()[0].clone(), 1100.0, 0.0, 1000.0, 0, 0.0, 0);
    a.scenario_id = Uuid::new_v4();
    b.scenario_id = Uuid::new_v4();
    let objects = [a, b];
    assert_eq!(nearest_placement(&objects, 1090.0, 1010.0, 200.0), Some(1));
    assert_eq!(nearest_placement(&objects, 1040.0, 1000.0, 200.0), Some(0));
    assert_eq!(nearest_placement(&objects, 5000.0, 5000.0, 200.0), None);
}

#[test]
fn press_selects_and_a_plain_click_changes_nothing() {
    let mut f = Fixture::new(vec![spawn("house", 1000.0, 1000.0), spawn("house", 4000.0, 4000.0)]);
    assert!(!f.ui.canvas_press((9000.0, 9000.0), 100.0, false, false), "a miss belongs to the brush");
    assert!(!f.ui.is_capturing());

    assert!(f.ui.canvas_press((4010.0, 3990.0), 100.0, false, false));
    assert!(f.ui.is_capturing() && f.ui.is_dragging());
    assert_eq!(f.ui.selection(), vec![1]);
    f.ui.canvas_drag((4012.0, 3991.0), 40.0);
    assert!(f.ui.drag_offset().is_none(), "below the threshold it is still a click");
    assert!(!f.ui.canvas_release(&mut f.scenario, &cx()), "a click does not move anything");
    assert!(!f.ui.is_capturing());
    assert_eq!(f.scenario.document().spawns[1].x, 4000.0);
    assert_eq!(f.ui.session().undo_depth(), 0);
}

#[test]
fn modifier_presses_toggle_and_extend_the_selection_without_dragging() {
    let mut f = Fixture::new(vec![spawn("house", 1000.0, 1000.0), spawn("house", 4000.0, 4000.0)]);
    assert!(f.ui.canvas_press((1000.0, 1000.0), 100.0, false, false));
    f.ui.canvas_release(&mut f.scenario, &cx());
    assert!(f.ui.canvas_press((4000.0, 4000.0), 100.0, false, true));
    assert!(!f.ui.is_dragging(), "shift adds without arming a drag");
    f.ui.canvas_release(&mut f.scenario, &cx());
    assert_eq!(f.ui.selection(), vec![0, 1]);
    assert!(f.ui.canvas_press((1000.0, 1000.0), 100.0, true, false));
    f.ui.canvas_release(&mut f.scenario, &cx());
    assert_eq!(f.ui.selection(), vec![1], "ctrl toggles a member off");
}

#[test]
fn dragging_moves_the_whole_selection_as_one_undo_step() {
    let mut f = Fixture::new(vec![spawn("house", 1000.0, 1000.0), spawn("house", 2000.0, 1000.0), spawn("house", 9000.0, 9000.0)]);
    f.ui.select_all();
    assert!(f.ui.canvas_press((1000.0, 1000.0), 100.0, false, false));
    assert_eq!(f.ui.selection().len(), 3, "pressing a selected member keeps the group");
    f.ui.canvas_drag((1500.0, 1250.0), 40.0);
    assert_eq!(f.ui.drag_offset(), Some((500.0, 250.0)));
    assert!(f.ui.canvas_release(&mut f.scenario, &cx()));
    let spawns = &f.scenario.document().spawns;
    assert_eq!((spawns[0].x, spawns[0].z), (1500.0, 1250.0));
    assert_eq!((spawns[1].x, spawns[1].z), (2500.0, 1250.0));
    assert_eq!((spawns[2].x, spawns[2].z), (9500.0, 9250.0));
    assert_eq!(f.ui.session().undo_depth(), 1);
    assert!(f.ui.undo(&mut f.scenario, &cx()));
    assert_eq!(f.scenario.document().spawns[0].x, 1000.0);
}

#[test]
fn a_drag_that_would_leave_the_map_is_refused_without_changes() {
    let mut f = Fixture::new(vec![spawn("house", 16200.0, 1000.0)]);
    assert!(f.ui.canvas_press((16200.0, 1000.0), 100.0, false, false));
    f.ui.canvas_drag((16500.0, 1000.0), 40.0);
    assert!(f.ui.canvas_release(&mut f.scenario, &cx()));
    assert!(f.ui.status().1 && f.ui.status().0.contains("leave the map"), "{}", f.ui.status().0);
    assert_eq!(f.scenario.document().spawns[0].x, 16200.0);
    assert_eq!(f.ui.session().undo_depth(), 0);
}

#[test]
fn cancel_drops_the_gesture() {
    let mut f = Fixture::new(vec![spawn("house", 1000.0, 1000.0)]);
    assert!(f.ui.canvas_press((1000.0, 1000.0), 100.0, false, false));
    f.ui.canvas_drag((3000.0, 3000.0), 40.0);
    f.ui.cancel_drag();
    assert!(!f.ui.is_capturing() && f.ui.drag_offset().is_none());
    assert!(!f.ui.canvas_release(&mut f.scenario, &cx()));
    assert_eq!(f.scenario.document().spawns[0].x, 1000.0);
}

#[test]
fn prebuilt_spawns_are_draggable_like_any_other() {
    let mut prebuilt = spawn("house", 1000.0, 1000.0);
    prebuilt.prebuilt = true;
    let mut f = Fixture::new(vec![prebuilt]);
    assert!(f.ui.is_data_bound_index(0));
    assert!(f.ui.canvas_press((1000.0, 1000.0), 100.0, false, false));
    f.ui.canvas_drag((1400.0, 1000.0), 40.0);
    assert!(f.ui.canvas_release(&mut f.scenario, &cx()));
    assert_eq!(f.scenario.document().spawns[0].x, 1400.0);
    assert!(f.scenario.document().spawns[0].prebuilt, "the DATA transaction at Save decides the rest");
}

#[test]
fn landscape_clearance_rule_is_a_384_box_test() {
    let trees = [(1000.0, 1000.0)];
    assert!(landscape_nearby(&trees, 1383.0, 1383.0));
    assert!(!landscape_nearby(&trees, 1384.0, 1000.0), "exactly 384 away is clear");
    assert!(!landscape_nearby(&trees, 1000.0, 616.0));
    assert!(landscape_place_warning(true).contains("trees") && landscape_place_warning(false).contains("樹木"));
    assert!(landscape_move_warning(true).contains("landscape") && landscape_move_warning(false).contains("地景"));
}

#[test]
fn placing_or_moving_a_building_near_landscape_warns_but_units_do_not() {
    let mut f = Fixture::new(vec![]);
    f.ui.set_landscape(vec![(896.0, 1152.0)]);
    f.ui.select_type("BauRomHaus");
    f.ui.set_placement_values(0, 1, 0);
    f.ui.place_at_tile(&mut f.scenario, &cx(), 3, 4);
    assert!(f.ui.status().0.contains("Warning: trees"), "{}", f.ui.status().0);
    assert!(!f.ui.status().1, "a warning is not an error");

    f.ui.place_at_tile(&mut f.scenario, &cx(), 20, 20);
    assert!(!f.ui.status().0.contains("Warning"), "{}", f.ui.status().0);

    // moving the far building next to the tree warns with the move wording
    assert_eq!(f.ui.selection(), vec![1]);
    f.ui.canvas_press((20.0 * 256.0 + 128.0, 20.0 * 256.0 + 128.0), 100.0, false, false);
    f.ui.canvas_drag((896.0, 1152.0), 40.0);
    f.ui.canvas_release(&mut f.scenario, &cx());
    assert!(f.ui.status().0.contains("Warning: landscape objects are close"), "{}", f.ui.status().0);

    // squads never warn
    f.ui.select_type("FigRomLeg");
    f.ui.place_at_tile(&mut f.scenario, &cx(), 3, 4);
    assert!(!f.ui.status().0.contains("Warning"), "{}", f.ui.status().0);
}
