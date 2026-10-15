use against_rome_ai_planner::plan::{AiMapFeature, AiMapPlan};
use against_rome_ai_planner::preview::AiMapPlanPreviewBuilder;
use against_rome_editor::terrain::{HeightOperation, TerrainSession};

#[test]
fn test_preview_keeps_source_intact_and_preserves_pending_stroke() {
    let mut source = TerrainSession::new(
        9,
        vec![80u8; 81],
        8,
        Some(vec![0u8; 64]),
    )
    .unwrap();

    source
        .paint_height((4.0, 4.0), 3.0, HeightOperation::Raise, 10, None, 0)
        .unwrap();
    source.commit_stroke();
    let edited = source.heights().to_vec();

    assert!(source.undo().is_some());
    let pending = source.heights().to_vec();
    let can_undo = source.can_undo();
    let can_redo = source.can_redo();

    let plan = AiMapPlan {
        base_height: Some(20),
        features: vec![AiMapFeature {
            feature_type: Some("blocked".to_string()),
            x: 2,
            y: 2,
            radius: 1,
            ..Default::default()
        }],
        ..Default::default()
    };

    let preview = AiMapPlanPreviewBuilder::build(&plan, &source, None, 4, 30.0, None);

    assert_eq!(&pending, source.heights());
    assert_eq!(can_undo, source.can_undo());
    assert_eq!(can_redo, source.can_redo());
    assert!(preview.has_collision);
    assert_eq!(9, preview.image.width);
    assert_eq!(9, preview.image.height);

    // Source redo remains user's prior edit
    assert!(source.redo().is_some());
    assert_eq!(edited[4 * 9 + 4], source.heights()[4 * 9 + 4]);
    assert_ne!(20, source.heights()[4 * 9 + 4]);
}
