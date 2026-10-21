use against_rome_navigation::{find_path, placement_collision_blocked, PassabilityGrid};

#[test]
fn free_cell_selection_and_conservative_routes_have_distinct_contracts() {
    // 原生空地選點接受 <=5；未指定單位能力的編輯器通行診斷保守拒絕非零。
    for (value, placement_blocked, route_blocked) in [
        (0, false, false),
        (1, false, true),
        (5, false, true),
        (6, true, true),
        (127, true, true),
        (128, true, true),
        (129, true, true),
        (255, true, true),
    ] {
        let mut collision = [0; 9];
        collision[3..6].fill(value);
        let grid = PassabilityGrid::new(3, Some(&collision), None, 192.0).unwrap();
        assert_eq!(
            placement_collision_blocked(value),
            placement_blocked,
            "選點 {value}"
        );
        assert_eq!(grid.collision_blocked(1, 1), route_blocked, "診斷 {value}");
        assert_eq!(
            find_path(&grid, (1, 0), (1, 2)).is_none(),
            route_blocked,
            "路徑 {value}"
        );
        assert_eq!(grid.collision_byte(1, 1), value, "保留原始碰撞值");
    }
}

#[test]
fn invisible_native_cell_value_is_rejected_by_selection_and_diagnostics() {
    // 0x4d8830 在 visible bit0=0 時寫入 255；此 fixture 是載入後的格值。
    let collision = [255];
    let grid = PassabilityGrid::new(1, Some(&collision), None, 64.0).unwrap();
    assert!(placement_collision_blocked(collision[0]));
    assert!(!grid.passable(0, 0));
    assert!(find_path(&grid, (0, 0), (0, 0)).is_none());
}
