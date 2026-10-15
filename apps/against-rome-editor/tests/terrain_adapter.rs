use against_rome_editor::terrain::{HeightOperation, SampleChange};
use against_rome_editor_app::terrain::{BrushPath, HeightBrushOp, TerrainSession};

#[test]
fn adapter_uses_core_for_strokes_and_history() {
    let h: Vec<u8> = (0..193 * 193).map(|i| (i % 256) as u8).collect();
    let c = vec![90; 192 * 192];
    let mut direct =
        against_rome_editor::terrain::TerrainSession::new(193, h.clone(), 192, Some(c.clone()))
            .unwrap();
    let mut app = TerrainSession::new(193, h, 192, c, None, None);
    for op in [
        HeightOperation::Raise,
        HeightOperation::Lower,
        HeightOperation::Flatten,
        HeightOperation::Smooth,
        HeightOperation::Roughen,
    ] {
        direct
            .paint_height((20.5, 22.25), 7.75, op, 17, Some(73), -12345)
            .unwrap();
        direct.commit_stroke();
        app.paint_height(20.5, 22.25, 7.75, op, 17, Some(73), -12345)
            .unwrap();
        app.commit_stroke();
        assert_eq!(direct.heights(), app.heights());
    }
    app.paint_water(21.0, 23.0, 10.0, 64, 12, 110.0).unwrap();
    direct
        .paint_water((21.0, 23.0), 10.0, 64, 12, 110.0)
        .unwrap();
    assert_eq!(direct.heights(), app.heights());
    assert_eq!(direct.collision().unwrap(), app.collision());
    app.undo();
    direct.undo();
    assert_eq!(direct.heights(), app.heights());
    app.redo();
    direct.redo();
    assert_eq!(direct.heights(), app.heights());
    app.commit_baseline();
    assert_eq!(app.original_heights(), app.heights());
    assert_eq!(app.original_collision(), app.collision());
    assert!(!app.is_dirty());
}

#[test]
fn reverted_dab_disappears_and_pending_edits_clear_redo() {
    let mut app = TerrainSession::create_blank(5, 10);
    app.apply_height_changes(&[SampleChange {
        index: 0,
        before: 10,
        after: 20,
    }]);
    app.apply_height_changes(&[SampleChange {
        index: 0,
        before: 20,
        after: 10,
    }]);
    assert!(!app.commit_stroke());
    assert!(!app.can_undo());
    app.paint_height(2.0, 2.0, 1.0, HeightBrushOp::Raise, 10, None, 0)
        .unwrap();
    app.commit_stroke();
    app.undo();
    assert!(app.can_redo());
    app.paint_height(2.0, 2.0, 1.0, HeightBrushOp::Lower, 1, None, 0)
        .unwrap();
    assert!(!app.can_redo());
    app.cancel_stroke();
    assert!(!app.is_dirty());
}

#[test]
fn sampled_flatten_uses_dotnet_midpoint_rounding() {
    let mut h = vec![0; 9];
    h[0] = 10;
    h[1] = 100;
    let mut app = TerrainSession::new(3, h, 2, vec![0; 4], None, None);
    app.paint_height(0.5, 0.0, 10.0, HeightBrushOp::Flatten, 64, None, 0)
        .unwrap();
    assert_eq!(app.heights()[1], 11); // target is vertex 0 (10), not vertex 1 (100)
}

#[test]
fn brush_path_fills_gaps_ignores_repeated_positions_and_resets() {
    let mut path = BrushPath::default();
    assert_eq!(path.advance((2, 2)), vec![(2, 2)]);
    assert_eq!(path.advance((2, 2)), vec![]);
    assert_eq!(path.advance((5, 5)), vec![(3, 3), (4, 4), (5, 5)]);
    assert!(path.advance((2, 2)).is_empty());
    path.flatten_target = Some(17);
    path.clear();
    assert_eq!(path.flatten_target, None);
    assert_eq!(path.advance((2, 2)), vec![(2, 2)]);
}
