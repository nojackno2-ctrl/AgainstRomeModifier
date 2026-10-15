use against_rome_editor::terrain::*;

#[test]
fn ramp_linear_center_and_cap_geometry() {
    let mut p = RampParameters {
        width: 2.0,
        falloff: 0.0,
        start_height: Some(20.0),
        end_height: Some(100.0),
        ..Default::default()
    };
    let source = vec![0; 81];
    let round = calculate_ramp(&source, 9, (2.0, 4.0), (6.0, 4.0), &p).unwrap();
    assert_eq!(
        round.iter().find(|c| c.index == 4 * 9 + 4).unwrap().after,
        60
    );
    assert_eq!(
        round.iter().find(|c| c.index == 4 * 9 + 1).unwrap().after,
        20
    );
    p.round_caps = false;
    let flat = calculate_ramp(&source, 9, (2.0, 4.0), (6.0, 4.0), &p).unwrap();
    assert!(!flat.iter().any(|c| c.index == 4 * 9 + 1));
    assert!(source.iter().all(|v| *v == 0));
}
#[test]
fn retail_193_vertex_grid_and_bilinear_endpoints() {
    let mut source = vec![0; 193 * 193];
    source[192 * 193 + 192] = 200;
    assert_eq!(sample_bilinear(&source, 193, 191.5, 191.5).unwrap(), 50.0);
    assert_eq!(sample_bilinear(&source, 193, 999.0, 999.0).unwrap(), 200.0);
    assert_eq!(tile_to_vertex(47, 47, 48, 193), (190.0, 190.0));
    let c = calculate_ramp(
        &source,
        193,
        (191.5, 191.5),
        (192.0, 192.0),
        &Default::default(),
    )
    .unwrap();
    assert!(!c.is_empty());
    assert!(c.iter().all(|v| v.index < 193 * 193));
}
#[test]
fn midpoint_rounding_matches_dotnet_ties_to_even() {
    let p = RampParameters {
        width: 1.0,
        falloff: 0.0,
        start_height: Some(10.0),
        end_height: Some(11.0),
        ..Default::default()
    };
    let c = calculate_ramp(&[0; 9], 3, (0.0, 1.0), (2.0, 1.0), &p).unwrap();
    assert_eq!(c.iter().find(|c| c.index == 4).unwrap().after, 10);
}
#[test]
fn shared_stroke_cancel_undo_redo_and_saved_baseline() {
    let mut s = TerrainSession::new(5, vec![100; 25], 4, Some(vec![0; 16])).unwrap();
    s.paint_height((2.0, 2.0), 1.0, HeightOperation::Raise, 10, None, 0)
        .unwrap();
    s.paint_collision((1.5, 1.5), 0.6, CollisionOperation::Block)
        .unwrap();
    assert!(s.commit_stroke());
    assert!(s.is_dirty());
    let inverse = s.undo().unwrap();
    assert_eq!(inverse.heights[0].after, 100);
    assert!(!s.is_dirty());
    assert_eq!(s.collision().unwrap()[5], 0);
    s.redo().unwrap();
    assert_eq!(s.collision().unwrap()[5], 255);
    s.commit_baseline();
    assert!(!s.is_dirty());
    assert!(!s.can_undo());
    s.paint_height((2.0, 2.0), 1.0, HeightOperation::Lower, 5, None, 0)
        .unwrap();
    s.cancel_stroke();
    assert!(!s.is_dirty());
}
#[test]
fn reverting_pending_change_creates_no_history() {
    let mut s = TerrainSession::new(2, vec![10; 4], 0, None).unwrap();
    s.apply_height_changes(&[SampleChange {
        index: 0,
        before: 99,
        after: 20,
    }]);
    s.apply_height_changes(&[SampleChange {
        index: 0,
        before: 99,
        after: 10,
    }]);
    assert!(!s.commit_stroke());
    assert!(!s.can_undo());
    assert!(!s.is_dirty());
}
#[test]
fn water_preserves_authored_collision_and_undoes_together() {
    let mut c = vec![0; 16];
    c[5] = 90;
    let mut s = TerrainSession::new(5, vec![100; 25], 4, Some(c)).unwrap();
    let stroke = s.paint_water((2.0, 2.0), 2.0, 64, 10, 99.0).unwrap();
    assert!(!stroke.heights.is_empty());
    assert!(!stroke.collision.is_empty());
    assert_eq!(s.collision().unwrap()[5], 90);
    s.undo().unwrap();
    assert!(!s.is_dirty());
    assert!(s.heights().iter().all(|v| *v == 100));
}
#[test]
fn smoothing_uses_snapshot_and_roughening_is_seeded() {
    let mut h = vec![0; 25];
    h[12] = 90;
    let mut s = TerrainSession::new(5, h, 0, None).unwrap();
    s.paint_height((2.0, 2.0), 1.0, HeightOperation::Smooth, 8, None, 0)
        .unwrap();
    assert_eq!(s.heights()[12], 10);
    assert_eq!(roughness_noise(11, 29, -7), roughness_noise(11, 29, -7));
    assert_ne!(roughness_noise(11, 29, -7), roughness_noise(11, 29, 7));
    for y in 0..20 {
        for x in 0..20 {
            assert!((-1.0..=1.0).contains(&roughness_noise(x, y, i32::MAX)));
        }
    }
}
#[test]
fn invalid_input_is_rejected_without_mutation() {
    assert!(TerrainSession::new(usize::MAX, vec![], 0, None).is_err());
    let mut s = TerrainSession::new(2, vec![10; 4], 0, None).unwrap();
    assert!(s
        .paint_height((f32::NAN, 0.0), 1.0, HeightOperation::Raise, 1, None, 0)
        .is_err());
    assert!(s
        .apply_ramp(
            (0.0, 0.0),
            (1.0, 1.0),
            &RampParameters {
                width: f32::INFINITY,
                ..Default::default()
            }
        )
        .is_err());
    assert!(!s.is_dirty());
    assert!(
        calculate_ramp(&[0; 4], 2, (0.0, 0.0), (0.0, 0.0), &Default::default())
            .unwrap()
            .is_empty()
    );
}
