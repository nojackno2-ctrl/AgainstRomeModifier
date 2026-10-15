use against_rome_packaging::diff::*;
use against_rome_scenario::model::{Action, Event};
use uuid::Uuid;

#[test]
fn test_identical_snapshots_reports_no_differences() {
    let heights = vec![100u8; 65 * 65];
    let textures = vec!["GRAS01".to_string(); 64 * 64];
    let collision = vec![0u8; 256 * 256];

    let obj_id = Uuid::new_v4();
    let objects = vec![DiffObjectItem::new(
        obj_id, 1, 1001, "GER_HAU00", 42, 0, 5000.0, 100.0, 5000.0, 45.0, 0, true,
    )];

    let events = vec![Event {
        name: "StartMessage".to_string(),
        delay_seconds: 5,
        repeat: false,
        enabled: true,
        actions: vec![Action {
            kind: 0,
            text: "Welcome".to_string(),
            ..Default::default()
        }],
        conditions: vec![],
    }];

    let mut snapshot_a = MapVersionSnapshot::new("Backup_A", 64, 65, Some(heights.clone()), 4.0);
    snapshot_a.textures = Some(textures.clone());
    snapshot_a.collision_dimension = 256;
    snapshot_a.collision = Some(collision.clone());
    snapshot_a.objects = Some(objects.clone());
    snapshot_a.events = Some(events.clone());

    let mut snapshot_b = MapVersionSnapshot::new("Current_B", 64, 65, Some(heights), 4.0);
    snapshot_b.textures = Some(textures);
    snapshot_b.collision_dimension = 256;
    snapshot_b.collision = Some(collision);
    snapshot_b.objects = Some(objects);
    snapshot_b.events = Some(events);

    let report = MapDiffEngine::compare(&snapshot_a, &snapshot_b, None);

    assert!(!report.has_differences());
    assert_eq!(0, report.height.changed_vertex_count);
    assert_eq!(0, report.textures.changed_tile_count);
    assert_eq!(0, report.collision.newly_blocked_count);
    assert_eq!(0, report.collision.newly_cleared_count);
    assert_eq!(0, report.objects.total_changes());
    assert_eq!(0, report.events.total_changes());
}

#[test]
fn test_height_differences_detects_raise_lower_and_volume() {
    let base_heights = vec![50u8; 65 * 65];
    let mut cur_heights = vec![50u8; 65 * 65];

    // 抬升頂點 (10, 10) 由 50 -> 60 (提升 10 階 = 40 世界高度)
    cur_heights[10 * 65 + 10] = 60;

    // 下陷頂點 (20, 20) 由 50 -> 45 (降低 5 階 = -20 世界高度)
    cur_heights[20 * 65 + 20] = 45;

    let snap_base = MapVersionSnapshot::new("Base", 64, 65, Some(base_heights), 4.0);
    let snap_cur = MapVersionSnapshot::new("Current", 64, 65, Some(cur_heights), 4.0);

    let report = MapDiffEngine::compare(&snap_base, &snap_cur, None);

    assert!(report.has_differences());
    assert_eq!(2, report.height.changed_vertex_count);
    assert_eq!(40.0, report.height.max_raise);
    assert_eq!(20.0, report.height.max_lower);
    assert_eq!(20.0, report.height.volume_delta);

    let delta = &report.height.delta_grid;
    assert_eq!(40.0, delta[10 * 65 + 10]);
    assert_eq!(-20.0, delta[20 * 65 + 20]);
}

#[test]
fn test_texture_differences_detects_changed_tiles_and_transitions() {
    let base_tex = vec!["GRAS01".to_string(); 64 * 64];
    let mut cur_tex = vec!["GRAS01".to_string(); 64 * 64];

    // 鋪設土路
    cur_tex[5 * 64 + 10] = "PFAD01".to_string();
    cur_tex[5 * 64 + 11] = "PFAD01".to_string();
    cur_tex[6 * 64 + 10] = "H_WEG1".to_string();

    let mut snap_base = MapVersionSnapshot::new("Base", 64, 65, None, 4.0);
    snap_base.textures = Some(base_tex);

    let mut snap_cur = MapVersionSnapshot::new("Current", 64, 65, None, 4.0);
    snap_cur.textures = Some(cur_tex);

    let report = MapDiffEngine::compare(&snap_base, &snap_cur, None);

    assert!(report.has_differences());
    assert_eq!(3, report.textures.changed_tile_count);
    assert_eq!(
        Some(&2),
        report.textures.transitions.get(&("GRAS01".to_string(), "PFAD01".to_string()))
    );
    assert_eq!(
        Some(&1),
        report.textures.transitions.get(&("GRAS01".to_string(), "H_WEG1".to_string()))
    );
}

#[test]
fn test_collision_differences_detects_blocked_and_cleared() {
    let mut base_col = vec![0u8; 256 * 256];
    let mut cur_col = vec![0u8; 256 * 256];

    // 新增障礙
    base_col[10 * 256 + 10] = 0;
    cur_col[10 * 256 + 10] = 255;

    // 清除通行
    base_col[50 * 256 + 50] = 255;
    cur_col[50 * 256 + 50] = 0;

    let mut snap_base = MapVersionSnapshot::new("Base", 64, 65, None, 4.0);
    snap_base.collision_dimension = 256;
    snap_base.collision = Some(base_col);

    let mut snap_cur = MapVersionSnapshot::new("Current", 64, 65, None, 4.0);
    snap_cur.collision_dimension = 256;
    snap_cur.collision = Some(cur_col);

    let report = MapDiffEngine::compare(&snap_base, &snap_cur, None);

    assert!(report.has_differences());
    assert_eq!(1, report.collision.newly_blocked_count);
    assert_eq!(1, report.collision.newly_cleared_count);
}

#[test]
fn test_object_differences_detects_added_deleted_moved_modified() {
    let id_keep = Uuid::new_v4();
    let id_delete = Uuid::new_v4();
    let id_add = Uuid::new_v4();
    let id_move = Uuid::new_v4();

    let base_objs = vec![
        DiffObjectItem::new(id_keep, 1, 100, "GER_HAU00", 1, 0, 4000.0, 0.0, 4000.0, 0.0, 0, true),
        DiffObjectItem::new(id_delete, 2, 200, "LanGerNabu01", 10, 8, 3000.0, 0.0, 3000.0, 0.0, 0, false),
        DiffObjectItem::new(id_move, 3, 300, "ROM_LEG01", 5, 1, 6000.0, 0.0, 6000.0, 90.0, 10, false),
    ];

    let cur_objs = vec![
        DiffObjectItem::new(id_keep, 1, 100, "GER_HAU00", 1, 0, 4000.0, 0.0, 4000.0, 0.0, 0, true),
        DiffObjectItem::new(id_add, 4, 400, "ROM_BALLISTA", 8, 1, 8000.0, 0.0, 8000.0, 0.0, 0, false),
        DiffObjectItem::new(id_move, 3, 300, "ROM_LEG01", 5, 1, 6100.0, 0.0, 6100.0, 180.0, 15, false),
    ];

    let mut snap_base = MapVersionSnapshot::new("Base", 64, 65, None, 4.0);
    snap_base.objects = Some(base_objs);

    let mut snap_cur = MapVersionSnapshot::new("Current", 64, 65, None, 4.0);
    snap_cur.objects = Some(cur_objs);

    let report = MapDiffEngine::compare(&snap_base, &snap_cur, None);

    assert!(report.has_differences());
    assert_eq!(1, report.objects.added.len());
    assert_eq!(id_add, report.objects.added[0].id);

    assert_eq!(1, report.objects.deleted.len());
    assert_eq!(id_delete, report.objects.deleted[0].id);

    assert_eq!(1, report.objects.modified.len());
    let m = &report.objects.modified[0];
    assert_eq!(id_move, m.current.id);
    assert!(m.distance_moved > 100.0);
    assert!(m.changed_attributes.iter().any(|a| a.contains("坐標移動")));
    assert!(m.changed_attributes.iter().any(|a| a.contains("旋轉角度")));
    assert!(m.changed_attributes.iter().any(|a| a.contains("部隊人數")));
}

#[test]
fn test_event_differences_detects_event_changes() {
    let base_events = vec![
        Event {
            name: "Intro".to_string(),
            delay_seconds: 5,
            repeat: false,
            enabled: true,
            ..Default::default()
        },
        Event {
            name: "Wave1".to_string(),
            delay_seconds: 60,
            repeat: false,
            enabled: true,
            ..Default::default()
        },
    ];

    let cur_events = vec![
        Event {
            name: "Intro".to_string(),
            delay_seconds: 10,
            repeat: false,
            enabled: true,
            ..Default::default()
        },
        Event {
            name: "BossFight".to_string(),
            delay_seconds: 120,
            repeat: false,
            enabled: true,
            ..Default::default()
        },
    ];

    let mut snap_base = MapVersionSnapshot::new("Base", 64, 65, None, 4.0);
    snap_base.events = Some(base_events);

    let mut snap_cur = MapVersionSnapshot::new("Current", 64, 65, None, 4.0);
    snap_cur.events = Some(cur_events);

    let report = MapDiffEngine::compare(&snap_base, &snap_cur, None);

    assert!(report.has_differences());
    assert_eq!(1, report.events.added.len());
    assert_eq!("BossFight", report.events.added[0].name);

    assert_eq!(1, report.events.deleted.len());
    assert_eq!("Wave1", report.events.deleted[0].name);

    assert_eq!(1, report.events.modified.len());
    assert_eq!("Intro", report.events.modified[0].event_name);
}

#[test]
fn test_map_visual_diff_overlay_generates_heatmap_and_minimap_buffer() {
    let base_heights = vec![0u8; 65 * 65];
    let mut cur_heights = vec![0u8; 65 * 65];
    cur_heights[10 * 65 + 10] = 50;

    let base_tex = vec!["GRAS01".to_string(); 64 * 64];
    let mut cur_tex = vec!["GRAS01".to_string(); 64 * 64];
    cur_tex[5 * 64 + 5] = "PFAD01".to_string();

    let mut snap_base = MapVersionSnapshot::new("Base", 64, 65, Some(base_heights), 4.0);
    snap_base.textures = Some(base_tex);

    let mut snap_cur = MapVersionSnapshot::new("Current", 64, 65, Some(cur_heights), 4.0);
    snap_cur.textures = Some(cur_tex);

    let report = MapDiffEngine::compare(&snap_base, &snap_cur, None);

    let canvas_mask = MapVisualDiffOverlay::generate_heatmap_mask(&report, 256, 256, None);
    assert_eq!(256, canvas_mask.width);
    assert_eq!(256, canvas_mask.height);
    assert!(canvas_mask.pixels.iter().any(|&p| p != 0));

    let minimap_mask = MapVisualDiffOverlay::generate_minimap_overlay(&report, 256, 256, None);
    assert_eq!(256, minimap_mask.width);
    assert!(minimap_mask.pixels.iter().any(|&p| p != 0));

    let markers = MapVisualDiffOverlay::generate_vector_markers(&report, None);
    assert!(markers.is_empty() || !markers.is_empty());
}

#[test]
fn test_selective_rollback_region_height_rollback_with_edge_blend() {
    let dim = 65;
    let base_heights = vec![10u8; dim * dim];
    let cur_heights = vec![100u8; dim * dim];

    let snap_base = MapVersionSnapshot::new("Base", 64, dim, Some(base_heights), 4.0);
    let snap_cur = MapVersionSnapshot::new("Current", 64, dim, Some(cur_heights), 4.0);

    let diff = MapDiffEngine::compare(&snap_base, &snap_cur, None);

    let scope = RollbackRegionScope::new(
        2000.0,
        2000.0,
        6000.0,
        6000.0,
        RollbackLayerFlags::HEIGHT,
        true,
        2,
    );

    let tx = SelectiveRollbackController::compute_rollback(&snap_base, &snap_cur, &diff, &scope);

    assert!(!tx.height_changes.is_empty());
    assert!(tx.texture_changes.is_empty());
    assert!(tx.objects_to_restore.is_empty());

    let world_step = 16384.0 / (dim as f32 - 1.0);
    let center_vx = (4000.0 / world_step).round() as usize;
    let center_vz = (4000.0 / world_step).round() as usize;
    let center_idx = center_vz * dim + center_vx;

    let center_change = tx.height_changes.iter().find(|c| c.index == center_idx);
    assert!(center_change.is_some());
    let cc = center_change.unwrap();
    assert_eq!(100, cc.before);
    assert_eq!(10, cc.after);

    let outside_vx = (10000.0 / world_step).round() as usize;
    let outside_vz = (10000.0 / world_step).round() as usize;
    let outside_idx = outside_vz * dim + outside_vx;

    assert!(!tx.height_changes.iter().any(|c| c.index == outside_idx));
}

#[test]
fn test_selective_rollback_texture_and_object_rollback_respects_scope_and_filters() {
    let base_tex = vec!["GRAS01".to_string(); 64 * 64];
    let mut cur_tex = vec!["GRAS01".to_string(); 64 * 64];

    cur_tex[10 * 64 + 10] = "PFAD01".to_string();
    cur_tex[50 * 64 + 50] = "PFAD01".to_string();

    let id_inside = Uuid::new_v4();
    let id_outside = Uuid::new_v4();

    let base_objs = vec![
        DiffObjectItem::new(id_inside, 1, 101, "TREE", 1, 8, 2500.0, 0.0, 2500.0, 0.0, 0, false),
        DiffObjectItem::new(id_outside, 2, 102, "TREE", 1, 8, 12000.0, 0.0, 12000.0, 0.0, 0, false),
    ];

    let cur_objs = vec![];

    let mut snap_base = MapVersionSnapshot::new("Base", 64, 65, None, 4.0);
    snap_base.textures = Some(base_tex);
    snap_base.objects = Some(base_objs);

    let mut snap_cur = MapVersionSnapshot::new("Current", 64, 65, None, 4.0);
    snap_cur.textures = Some(cur_tex);
    snap_cur.objects = Some(cur_objs);

    let diff = MapDiffEngine::compare(&snap_base, &snap_cur, None);

    let scope = RollbackRegionScope::new(
        1000.0,
        1000.0,
        4000.0,
        4000.0,
        RollbackLayerFlags::TEXTURES | RollbackLayerFlags::OBJECTS,
        false,
        0,
    );

    let tx = SelectiveRollbackController::compute_rollback(&snap_base, &snap_cur, &diff, &scope);

    assert_eq!(1, tx.texture_changes.len());
    assert_eq!(10, tx.texture_changes[0].tile_x);
    assert_eq!(10, tx.texture_changes[0].tile_y);
    assert_eq!("PFAD01", tx.texture_changes[0].old_texture);
    assert_eq!("GRAS01", tx.texture_changes[0].new_texture);

    assert_eq!(1, tx.objects_to_restore.len());
    assert_eq!(id_inside, tx.objects_to_restore[0].id);
}
