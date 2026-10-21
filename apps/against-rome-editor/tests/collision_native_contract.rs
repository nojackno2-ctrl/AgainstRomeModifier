use against_rome_editor_app::{
    environment_ui::EnvironmentUi, navigation::diagnose, TerrainSession,
};

#[test]
fn settlement_generation_rejects_medium_collision_like_255() {
    let heights = vec![20; 129 * 129];
    let mut ui = EnvironmentUi::default();
    let clear = vec![0; 128 * 128];
    ui.generate_settlement(129, Some(&heights), Some((128, &clear)), 0.0, 4.0)
        .unwrap();
    let original = ui.last_distribution.clone().unwrap();
    let site = &original.players[0].site_evaluation;
    let index = site.anchor_tile_z as usize * 128 + site.anchor_tile_x as usize;
    let mut blocked = clear.clone();
    blocked[index] = 255;
    ui.generate_settlement(129, Some(&heights), Some((128, &blocked)), 0.0, 4.0)
        .unwrap();
    let expected_blocked = ui.last_distribution.clone().unwrap();
    assert_ne!(original, expected_blocked, "原選點必須避開阻擋格");

    for (value, rejects_site) in [
        (0, false),
        (1, false),
        (5, false),
        (6, true),
        (127, true),
        (128, true),
        (129, true),
        (255, true),
    ] {
        blocked[index] = value;
        ui.generate_settlement(129, Some(&heights), Some((128, &blocked)), 0.0, 4.0)
            .unwrap();
        assert_eq!(
            ui.last_distribution.as_ref(),
            Some(if rejects_site {
                &expected_blocked
            } else {
                &original
            }),
            "collision={value}"
        );
    }
}

#[test]
fn session_diagnostics_preserve_raw_values_and_use_conservative_routes() {
    for value in [0, 1, 127, 128, 129, 255] {
        let mut collision = vec![0; 9];
        collision[3..6].fill(value);
        let session =
            TerrainSession::try_new(4, vec![20; 16], 3, collision.clone(), None, None).unwrap();
        let report = diagnose(&session, 0.0, [1, 0], [1, 2]).unwrap();
        assert_eq!(report.path_cells.is_some(), value == 0, "collision={value}");
        assert_eq!(report.passable, if value == 0 { 9 } else { 6 });
        assert_eq!(session.collision(), collision, "診斷不得改寫來源");
    }
}
