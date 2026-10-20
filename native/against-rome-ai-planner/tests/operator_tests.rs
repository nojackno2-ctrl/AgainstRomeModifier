use against_rome_ai_planner::operator::AiEditorOperator;
use against_rome_ai_planner::plan::{AiMapFeature, AiMapPlan};
use against_rome_ai_planner::scope::AiMapEditScope;
use against_rome_editor::terrain::TerrainSession;

fn create_session(dimension: usize, initial_height: u8) -> TerrainSession {
    let vertex_size = dimension * 4 + 1;
    let collision_size = dimension * 4;
    TerrainSession::new(
        vertex_size,
        vec![initial_height; vertex_size * vertex_size],
        collision_size,
        Some(vec![0u8; collision_size * collision_size]),
    )
    .unwrap()
}

#[test]
fn test_operator_produces_only_brush_strokes_with_valid_parameters() {
    let mut session = create_session(64, 100);
    let plan = AiMapPlan {
        base_height: Some(80),
        base_material: Some("Grass".to_string()),
        features: vec![
            AiMapFeature {
                feature_type: Some("hill".to_string()),
                x: 20,
                y: 20,
                radius: 6,
                amount: Some(30),
                ..Default::default()
            },
            AiMapFeature {
                feature_type: Some("lake".to_string()),
                x: 40,
                y: 40,
                radius: 5,
                ..Default::default()
            },
            AiMapFeature {
                feature_type: Some("river".to_string()),
                x: 10,
                y: 10,
                x2: Some(50),
                y2: Some(10),
                radius: 2,
                ..Default::default()
            },
            AiMapFeature {
                feature_type: Some("blocked".to_string()),
                x: 5,
                y: 5,
                x2: Some(15),
                y2: Some(5),
                radius: 1,
                ..Default::default()
            },
            AiMapFeature {
                feature_type: Some("material".to_string()),
                material: Some("Rock".to_string()),
                x: 30,
                y: 30,
                radius: 4,
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let result = AiEditorOperator::apply(
        &plan,
        Some(&mut session),
        None,
        64,
        50.0,
        false,
        Some(&|_mat, _x, _y, _r| true),
        None,
        None,
    );

    assert!(result.operation_count() > 0);
    assert!(!result.operation_log.is_empty());

    let allowed_sizes = [1, 3, 5, 9, 15];
    for log_line in &result.operation_log {
        assert!(log_line.contains("/brush"));
        let brush_idx = log_line.find("/brush").unwrap();
        let cmd = &log_line[brush_idx..];
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        assert!(parts.len() >= 2);
        let tool = parts[1];
        assert!(
            ["height", "collision", "material"].contains(&tool),
            "Unknown tool: {}",
            tool
        );

        if let Some(pos) = parts.iter().position(|&p| p == "--size") {
            if pos + 1 < parts.len() {
                let size: i32 = parts[pos + 1].parse().unwrap();
                assert!(allowed_sizes.contains(&size));
            }
        }
    }
}

#[test]
fn test_hill_raises_center_by_approximately_amount() {
    let mut session = create_session(64, 100);
    let cx = 32i32;
    let cy = 32i32;
    let center_vertex = (cy as usize * 4 + 2) * 257 + cx as usize * 4 + 2;
    let initial_h = session.heights()[center_vertex];

    let plan = AiMapPlan {
        features: vec![AiMapFeature {
            feature_type: Some("hill".to_string()),
            x: cx,
            y: cy,
            radius: 8,
            amount: Some(40),
            ..Default::default()
        }],
        ..Default::default()
    };

    let result = AiEditorOperator::apply(
        &plan,
        Some(&mut session),
        None,
        64,
        50.0,
        false,
        None,
        None,
        None,
    );

    let final_h = session.heights()[center_vertex];
    let gained = final_h as i32 - initial_h as i32;
    assert!(
        (36..=44).contains(&gained),
        "Expected gained in 36..=44, got {}",
        gained
    );
    assert!(result.height_samples_changed > 0);
}

#[test]
fn test_lake_creates_samples_below_water_level() {
    let mut session = create_session(64, 100);
    let water_level = 60.0f32;
    let cx = 25i32;
    let cy = 25i32;
    let center_vertex = (cy as usize * 4 + 2) * 257 + cx as usize * 4 + 2;

    let plan = AiMapPlan {
        features: vec![AiMapFeature {
            feature_type: Some("lake".to_string()),
            x: cx,
            y: cy,
            radius: 6,
            ..Default::default()
        }],
        ..Default::default()
    };

    let result = AiEditorOperator::apply(
        &plan,
        Some(&mut session),
        None,
        64,
        water_level,
        false,
        None,
        None,
        None,
    );

    let center_height = session.heights()[center_vertex];
    assert!(
        (center_height as f32) < water_level,
        "Center height {} should be below water level {}",
        center_height,
        water_level
    );
    assert!(result.height_samples_changed > 0);
}

#[test]
fn test_scope_is_strictly_respected_and_outside_tiles_are_untouched() {
    let mut session = create_session(64, 100);
    let scope = AiMapEditScope::parse("20,20,20,20", "28,28,4,4", true).unwrap();

    let plan = AiMapPlan {
        edit_scope: Some(scope),
        base_height: Some(150),
        features: vec![
            AiMapFeature {
                feature_type: Some("hill".to_string()),
                x: 30,
                y: 30,
                radius: 15,
                amount: Some(50),
                ..Default::default()
            },
            AiMapFeature {
                feature_type: Some("lake".to_string()),
                x: 10,
                y: 10,
                radius: 8,
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let _result = AiEditorOperator::apply(
        &plan,
        Some(&mut session),
        None,
        64,
        50.0,
        false,
        None,
        None,
        None,
    );

    // Outside edit scope (e.g. tile 10, 10), vertices should remain 100
    let outside_vertex = (10 * 4 + 2) * 257 + 10 * 4 + 2;
    assert_eq!(100, session.heights()[outside_vertex]);

    // Locked hole at tile 30, 30: vertex (30*4+2, 30*4+2) is tile 30, 30 which is locked in 28..32!
    let locked_vertex = (30 * 4 + 2) * 257 + 30 * 4 + 2;
    assert_eq!(100, session.heights()[locked_vertex]);

    // Inside allowed area (e.g. tile 22, 22), height should be modified
    let inside_vertex = (22 * 4 + 2) * 257 + 22 * 4 + 2;
    assert_ne!(100, session.heights()[inside_vertex]);
}

#[test]
fn test_rejected_material_dabs_report_each_rejected_tile_not_every_path_point() {
    let plan = AiMapPlan {
        features: vec![AiMapFeature {
            feature_type: Some("material".to_string()),
            x: 20,
            y: 20,
            radius: 6,
            material: Some("Rock".to_string()),
            ..Default::default()
        }],
        ..Default::default()
    };
    let painted = std::cell::RefCell::new(Vec::new());
    let fallback = |_: &str, x: f32, y: f32, r: f32| {
        painted.borrow_mut().push((x, y, r));
        x < 20.0
    };
    let mut reported: Vec<(f32, f32, f32)> = Vec::new();
    let mut on_rejected = |x: f32, y: f32, r: f32| reported.push((x, y, r));
    let result = AiEditorOperator::apply(
        &plan,
        None,
        None,
        64,
        0.0,
        false,
        Some(&fallback),
        None,
        Some(&mut on_rejected),
    );
    let painted = painted.into_inner();
    let expected: Vec<_> = painted.iter().filter(|p| p.0 >= 20.0).copied().collect();
    assert!(!expected.is_empty());
    assert!(painted.iter().any(|p| p.0 < 20.0));
    assert_eq!(reported, expected, "callback must mirror C# onRejectedDab (one per rejected tile)");
    assert_eq!(result.rejected_material_strokes, 1);
}
