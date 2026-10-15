use against_rome_ai_planner::scope::{AiMapEditScope, Rectangle};
use against_rome_editor::terrain::TerrainSession;
use against_rome_ai_planner::operator::AiEditorOperator;
use against_rome_ai_planner::plan::{AiMapFeature, AiMapPlan};

#[test]
fn test_scope_copies_locks_and_rejects_overflow_or_malformed_rectangles() {
    let locks = vec![Rectangle::new(16, 16, 4, 4)];
    let scope = AiMapEditScope::new(Rectangle::new(0, 0, 64, 64), locks, true).unwrap();
    assert!(!scope.allows_tile(16, 16));
    assert!(scope.allows_tile(0, 0));

    for bad in &["1,2,3", "0,0,0,1", "63,63,2,1", "2147483647,0,2,1", "1,1,2147483647,2"] {
        assert!(AiMapEditScope::parse(bad, "", true).is_err());
    }
}

#[test]
fn test_partial_height_and_collision_preserve_locked_tiles_and_shared_edges() {
    let scope = AiMapEditScope::parse("8,8,24,24", "16,16,4,4", true).unwrap();
    let vertex_size = 129;
    let collision_size = 128;
    let mut session = TerrainSession::new(
        vertex_size,
        vec![80u8; vertex_size * vertex_size],
        collision_size,
        Some(vec![0u8; collision_size * collision_size]),
    )
    .unwrap();

    let plan = AiMapPlan {
        base_height: Some(150),
        edit_scope: Some(scope.clone()),
        features: vec![AiMapFeature {
            feature_type: Some("blocked".to_string()),
            x: 18,
            y: 18,
            radius: 32,
            ..Default::default()
        }],
        ..Default::default()
    };

    let result = AiEditorOperator::apply(
        &plan,
        Some(&mut session),
        None,
        64,
        30.0,
        false,
        None,
        None,
        None,
    );

    assert!(result.height_samples_changed > 0);
    assert!(result.collision_pixels_changed > 0);
    assert_eq!(150, session.heights()[24 * 129 + 24]);

    for y in 0..129 {
        for x in 0..129 {
            if !scope.allows_vertex(x as f32 / 2.0, y as f32 / 2.0, 64) {
                assert_eq!(80, session.heights()[y * 129 + x]);
            }
        }
    }

    let col = session.collision().unwrap();
    for y in 0..128 {
        for x in 0..128 {
            if !scope.allows_tile((x / 2) as i32, (y / 2) as i32) {
                assert_eq!(0, col[y * 128 + x]);
            }
        }
    }
}

#[test]
fn test_protected_passability_ignores_model_blocked_features_when_disabled() {
    let mut session = TerrainSession::new(
        65,
        vec![0u8; 65 * 65],
        64,
        Some(vec![0u8; 64 * 64]),
    )
    .unwrap();

    let scope = AiMapEditScope::parse("0,0,64,64", "", false).unwrap();
    let plan = AiMapPlan {
        edit_scope: Some(scope),
        features: vec![AiMapFeature {
            feature_type: Some("blocked".to_string()),
            x: 32,
            y: 32,
            radius: 20,
            ..Default::default()
        }],
        ..Default::default()
    };

    let result = AiEditorOperator::apply(
        &plan,
        Some(&mut session),
        None,
        64,
        30.0,
        false,
        None,
        None,
        None,
    );

    assert_eq!(0, result.collision_pixels_changed);
    assert!(session.collision().unwrap().iter().all(|&v| v == 0));
}
