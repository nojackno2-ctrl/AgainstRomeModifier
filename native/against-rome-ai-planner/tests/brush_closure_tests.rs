//! /brush closure: every operator log line is canonical, parseable text, and replaying the
//! logged commands through the parser reproduces the exact same terrain as the direct run.

use against_rome_ai_planner::brush::{
    BrushCommand, EditorBrushStroke, EditorBrushTool, EditorHeightBrush,
};
use against_rome_ai_planner::operator::AiEditorOperator;
use against_rome_ai_planner::plan::{AiMapFeature, AiMapPlan};
use against_rome_editor::terrain::TerrainSession;

fn session(dimension: usize, h: u8) -> TerrainSession {
    let v = dimension * 4 + 1;
    let c = dimension * 4;
    TerrainSession::new(v, vec![h; v * v], c, Some(vec![0u8; c * c])).unwrap()
}

fn feature(t: &str, x: i32, y: i32, r: i32, to: Option<(i32, i32)>, amount: Option<i32>) -> AiMapFeature {
    AiMapFeature {
        feature_type: Some(t.to_string()),
        x,
        y,
        radius: r,
        x2: to.map(|p| p.0),
        y2: to.map(|p| p.1),
        amount,
        ..Default::default()
    }
}

fn plan() -> AiMapPlan {
    AiMapPlan {
        base_height: Some(90),
        features: vec![
            feature("hill", 20, 20, 6, None, Some(40)),
            feature("mountain", 45, 18, 5, None, Some(90)),
            feature("valley", 12, 40, 5, None, Some(30)),
            feature("plateau", 30, 30, 5, None, Some(120)),
            feature("lake", 40, 44, 5, None, Some(15)),
            feature("river", 8, 8, 2, Some((55, 12)), Some(12)),
            feature("ridge", 10, 55, 2, Some((50, 55)), Some(40)),
            feature("blocked", 5, 5, 1, Some((15, 5)), None),
            feature("passable", 6, 5, 1, None, None),
        ],
        ..Default::default()
    }
}

fn command_of(log_line: &str) -> &str {
    &log_line[log_line.find("/brush").expect("log line carries a /brush command")..]
}

#[test]
fn logged_commands_replay_to_identical_terrain() {
    let mut direct = session(64, 100);
    let result = AiEditorOperator::apply(&plan(), Some(&mut direct), None, 64, 50.0, true, Some(&|_, _, _, _| true), None, None);
    assert!(result.operation_log.len() > 5);

    let mut replay = session(64, 100);
    for line in &result.operation_log {
        let cmd = command_of(line);
        BrushCommand::run(cmd, Some(&mut replay), None, 64, 50.0)
            .unwrap_or_else(|e| panic!("operator emitted unparseable command `{cmd}`: {e}"));
    }
    assert_eq!(direct.heights(), replay.heights());
    assert_eq!(direct.collision(), replay.collision());
    assert!(replay.can_undo());
}

#[test]
fn every_logged_command_roundtrips_through_parser() {
    let mut s = session(64, 100);
    let mut p = plan();
    let mut mat = feature("material", 30, 10, 4, None, None);
    mat.material = Some("Rock \"x\"".to_string());
    p.features.push(mat);
    let result = AiEditorOperator::apply(&p, Some(&mut s), None, 64, 50.0, true, Some(&|_, _, _, _| true), None, None);
    let mut materials = 0;
    for line in &result.operation_log {
        let cmd = command_of(line);
        let stroke = BrushCommand::parse(cmd, 64).unwrap_or_else(|e| panic!("`{cmd}`: {e}"));
        assert_eq!(cmd, BrushCommand::format(&stroke).unwrap());
        if stroke.tool == EditorBrushTool::Material {
            materials += 1;
        }
    }
    assert!(materials > 0);
}

#[test]
fn canonical_format_matches_csharp_brush_command() {
    let mut s = EditorBrushStroke::new(EditorBrushTool::Height, vec![(2, 3), (4, 3)]);
    s.size = 3;
    s.strength_level = 2;
    s.height_operation = EditorHeightBrush::Flatten;
    assert_eq!("/brush height flatten 2,3 4,3 --size 3 --strength strong", BrushCommand::format(&s).unwrap());
    s.strength_level = 0;
    assert!(BrushCommand::format(&s).unwrap().ends_with("--strength gentle"));

    let mut c = EditorBrushStroke::new(EditorBrushTool::Collision, vec![(1, 1)]);
    c.block = false;
    assert_eq!("/brush collision passable 1,1 --size 1", BrushCommand::format(&c).unwrap());

    let mut m = EditorBrushStroke::new(EditorBrushTool::Material, vec![(1, 1)]);
    m.material = Some("Grass".into());
    m.auto_bridge = true;
    assert_eq!("/brush material \"Grass\" 1,1 --size 1 --auto-bridge", BrushCommand::format(&m).unwrap());
    m.material = None;
    assert!(BrushCommand::format(&m).is_err());
    assert!(BrushCommand::format(&EditorBrushStroke::new(EditorBrushTool::Height, vec![])).is_err());
}

#[test]
fn invalid_commands_are_rejected_before_editing() {
    for text in [
        "/brush height raise 1,1 --size 2",
        "/brush height raise 1,1 --strength invalid",
        "/brush height raise 1,1 --strength light",
        "/brush height raise 1,1 --size",
        "/brush height raise 1,1 --typo 3",
        "/brush height raise 1,1 invalid",
        "/brush height raise 8,1",
        "/brush height raise -1,1",
        "/brush height 1 1,1",
        "/brush collision invalid 1,1",
        "/brush collision clear 1,1",
        "/brush height raise",
        "/brush height raise 1,1 --auto-bridge",
        "/brush collision block 1,1 --strength medium",
        "/brush material \"Grass 1,1",
        "/brush material Grass 1,1 --auto-bridge 5",
        "/brush",
        "height raise 1,1",
    ] {
        let mut s = session(2, 100);
        assert!(BrushCommand::run(text, Some(&mut s), None, 8, 0.0).is_err(), "should reject: {text}");
        assert!(!s.is_dirty() && !s.can_undo(), "no edit may happen for: {text}");
    }
    // Material without a blend layer is a clean error, not a panic or partial edit.
    let mut s = session(2, 100);
    assert!(BrushCommand::run("/brush material Grass 1,1", Some(&mut s), None, 8, 0.0).is_err());
    assert!(!s.is_dirty());
}

#[test]
fn canonicalize_normalizes_case_flag_order_and_quotes() {
    // Flag before coordinates
    let raw1 = "/BRUSH HEIGHT RAISE --size 3 --strength gentle 2,3 4,3";
    let canon1 = BrushCommand::canonicalize(raw1, 10).unwrap();
    assert_eq!(canon1, "/brush height raise 2,3 4,3 --size 3 --strength gentle");

    // Unquoted material becomes quoted in canonical format
    let raw2 = "/brush material Sand 1,1 --auto-bridge";
    let canon2 = BrushCommand::canonicalize(raw2, 10).unwrap();
    assert_eq!(canon2, "/brush material \"Sand\" 1,1 --size 1 --auto-bridge");

    // Escaped material name retains fidelity through canonicalize and parse
    let raw3 = r#"/brush material "Stone \"Special\" \\ 1" 5,5 --size 5"#;
    let canon3 = BrushCommand::canonicalize(raw3, 10).unwrap();
    assert_eq!(canon3, raw3);
    let stroke = BrushCommand::parse(&canon3, 10).unwrap();
    assert_eq!(stroke.material.as_deref(), Some(r#"Stone "Special" \ 1"#));
    assert_eq!(BrushCommand::format(&stroke).unwrap(), raw3);
}

#[test]
fn repeated_flags_are_strictly_rejected() {
    let cases = [
        "/brush height raise 1,1 --size 3 --size 5",
        "/brush height raise 1,1 --strength gentle --strength strong",
        "/brush material Grass 1,1 --auto-bridge --auto-bridge",
    ];
    for cmd in cases {
        assert!(BrushCommand::parse(cmd, 10).is_err(), "should reject duplicate flag: {cmd}");
    }
}

#[test]
fn format_validates_stroke_properties() {
    let mut s = EditorBrushStroke::new(EditorBrushTool::Height, vec![(1, 1)]);
    s.size = 2; // Not in BRUSH_SIZES for Height
    assert!(BrushCommand::format(&s).is_err());

    s.size = 3;
    s.strength_level = 99; // Invalid strength level
    assert!(BrushCommand::format(&s).is_err());

    s.strength_level = 1;
    s.path = vec![(-1, 0)]; // Negative coordinate
    assert!(BrushCommand::format(&s).is_err());
}

#[test]
fn run_rejects_missing_collision_and_invalid_water_safely() {
    // Session without collision layer
    let v = 16 * 4 + 1;
    let mut no_col = TerrainSession::new(v, vec![100; v * v], 0, None).unwrap();
    assert!(BrushCommand::run("/brush collision block 1,1", Some(&mut no_col), None, 16, 0.0).is_err());
    assert!(!no_col.is_dirty() && !no_col.can_undo());

    // Water brush with invalid / negative surface height
    let mut s = session(16, 100);
    assert!(BrushCommand::run("/brush height water 1,1", Some(&mut s), None, 16, -1.0).is_err());
    assert!(!s.is_dirty() && !s.can_undo());
}

