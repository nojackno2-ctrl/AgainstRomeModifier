use against_rome_packaging::profiler::*;

fn scene_obj(name: &str, x: f32, z: f32) -> MapSceneObject {
    MapSceneObject::new(name, x, 0.0, z, 0.0, "test.sdl", 0)
}

fn light(x: f32, z: f32, radius: f32) -> NativeLightInstance {
    NativeLightInstance::new(x, 0.0, z, radius)
}

#[test]
fn test_empty_map_returns_healthy_grade_and_perfect_score() {
    let snapshot = MapBudgetSnapshot::default();
    let report = MapBudgetProfiler::analyze(&snapshot);

    assert_eq!(MapHealthGrade::Healthy, report.overall_grade);
    assert_eq!(100, report.overall_score);
    assert_eq!(0, report.object_budget.total_active_objects);
    assert_eq!(0, report.light_budget.total_scene_lights);
    assert!(report.bottlenecks.is_empty());
}

#[test]
fn test_object_count_exceeding_14000_triggers_engine_exceeded() {
    let mut objects = Vec::with_capacity(14005);
    for i in 0..14005 {
        objects.push(scene_obj("LanGerTree01", (i % 1000) as f32, (i / 1000) as f32));
    }

    let snapshot = MapBudgetSnapshot {
        scene_objects: Some(objects),
        ..Default::default()
    };
    let report = MapBudgetProfiler::analyze(&snapshot);

    assert_eq!(MapHealthGrade::EngineExceeded, report.overall_grade);
    assert!(report.object_budget.objects_usage_percent > 100.0);
    assert!(report.bottlenecks.iter().any(|b| b.code == "objects-dat-overflow" && b.is_engine_limit_violation));
}

#[test]
fn test_object_count_between_redline_and_limit_triggers_warning() {
    let mut objects = Vec::with_capacity(10500);
    for i in 0..10500 {
        objects.push(scene_obj("LanGerTree01", (i % 1000) as f32, (i / 1000) as f32));
    }

    let snapshot = MapBudgetSnapshot {
        scene_objects: Some(objects),
        ..Default::default()
    };
    let report = MapBudgetProfiler::analyze(&snapshot);

    assert_eq!(MapHealthGrade::Warning, report.overall_grade);
    assert!(report.bottlenecks.iter().any(|b| b.code == "objects-dat-redline" && !b.is_engine_limit_violation));
}

#[test]
fn test_local_light_cluster_exceeding_64_triggers_engine_exceeded_with_hotspot() {
    let mut lights = Vec::with_capacity(70);
    for i in 0..70 {
        let offset_x = (i % 8) as f32 * 10.0;
        let offset_z = (i / 8) as f32 * 10.0;
        lights.push(light(5000.0 + offset_x, 5000.0 + offset_z, 350.0));
    }

    let snapshot = MapBudgetSnapshot {
        lights: Some(lights),
        ..Default::default()
    };
    let report = MapBudgetProfiler::analyze(&snapshot);

    assert_eq!(MapHealthGrade::EngineExceeded, report.overall_grade);
    assert!(report.light_budget.peak_cluster_lights >= 70);
    assert!(!report.light_budget.hotspots.is_empty());

    let hotspot = &report.light_budget.hotspots[0];
    assert!(hotspot.exceeds_hard_limit);
    assert!((hotspot.world_x - 5000.0).abs() < 600.0);
    assert!((hotspot.world_z - 5000.0).abs() < 600.0);

    assert!(report.bottlenecks.iter().any(|b| b.code == "cluster-lights-overflow" && b.is_engine_limit_violation));
    let cluster_overflow = report.bottlenecks.iter().find(|b| b.code == "cluster-lights-overflow").unwrap();
    assert!(cluster_overflow.world_x.is_some());
}

#[test]
fn test_global_light_exceeding_1024_triggers_engine_exceeded() {
    let mut lights = Vec::with_capacity(1050);
    for i in 0..1050 {
        let x = (i % 32) as f32 * 500.0 + 100.0;
        let z = (i / 32) as f32 * 500.0 + 100.0;
        lights.push(light(x, z, 100.0));
    }

    let snapshot = MapBudgetSnapshot {
        lights: Some(lights),
        ..Default::default()
    };
    let report = MapBudgetProfiler::analyze(&snapshot);

    assert_eq!(MapHealthGrade::EngineExceeded, report.overall_grade);
    assert!(report.bottlenecks.iter().any(|b| b.code == "global-lights-overflow" && b.is_engine_limit_violation));
}

#[test]
fn test_draw_call_and_atlas_estimation_with_large_sprite_quads_and_many_types() {
    let snapshot = MapBudgetSnapshot {
        decoded_sprite_count: 15000,
        decoded_shadow_count: 3000,
        decoded_sprite_types: 300,
        ..Default::default()
    };

    let report = MapBudgetProfiler::analyze(&snapshot);

    assert!(report.draw_budget.estimated_atlas_pages >= 2);
    assert!(report.draw_budget.estimated_atlas_switches > 0);
    assert!(report.draw_budget.sprite_batch_draw_calls > 1);
    assert!(report.draw_budget.estimated_total_draw_calls > 5);
    assert!(report.draw_budget.batch_fragmentation_score > 0.0);
}

#[test]
fn test_pathfinding_chokepoint_detection_detects_corridors_and_scores_complexity() {
    let size = 16;
    let mut collision = vec![0u8; size * size];

    // 建造一堵中央隔牆，僅在 (X=8, Y=8) 留出 1 格通道
    for y in 0..size {
        if y != 8 {
            collision[y * size + 8] = 255;
        }
    }

    let snapshot = MapBudgetSnapshot {
        collision_size: size,
        collision: Some(collision),
        ..Default::default()
    };
    let report = MapBudgetProfiler::analyze(&snapshot);

    assert!(report.path_budget.chokepoint_count > 0);
    let chokepoint = &report.path_budget.chokepoints[0];
    assert_eq!(8, chokepoint.cell_x);
    assert_eq!(8, chokepoint.cell_z);
    assert_eq!(1, chokepoint.passage_width_cells);
    assert!(report.path_budget.pathfinding_complexity_score > 0);
}

#[test]
fn test_dead_end_pockets_and_isolated_islands_contribute_to_path_complexity() {
    let size = 16;
    let mut collision = vec![0u8; size * size];

    // 建立封閉孤島：周圍一圈牆壁，內部有一格可通行
    for y in 1..=3 {
        for x in 1..=3 {
            if x == 2 && y == 2 {
                collision[y * size + x] = 0;
            } else {
                collision[y * size + x] = 255;
            }
        }
    }

    // 建立死胡同：3 側為牆，只有一側開口
    collision[8 * size + 8] = 255;
    collision[8 * size + 10] = 255;
    collision[9 * size + 9] = 255;

    let snapshot = MapBudgetSnapshot {
        collision_size: size,
        collision: Some(collision),
        ..Default::default()
    };
    let report = MapBudgetProfiler::analyze(&snapshot);

    assert!(report.path_budget.passable_regions_count >= 2);
}

#[test]
fn test_endl000_benchmark_profile_matches_known_metrics() {
    let mut objects = Vec::with_capacity(7231);
    for i in 0..7231 {
        let name = if i < 5000 {
            "LanGerTree01"
        } else if i < 6500 {
            "BauGerHau00"
        } else {
            "GerInf01"
        };
        objects.push(scene_obj(name, (i % 256 * 64) as f32, (i / 256 * 64) as f32));
    }

    let mut lights = Vec::with_capacity(50);
    for i in 0..50 {
        lights.push(light(
            (i % 10) as f32 * 1500.0 + 500.0,
            (i / 10) as f32 * 1500.0 + 500.0,
            350.0,
        ));
    }

    let snapshot = MapBudgetSnapshot {
        scene_objects: Some(objects),
        lights: Some(lights),
        decoded_sprite_count: 7143,
        decoded_shadow_count: 2050,
        decoded_sprite_types: 212,
        ..Default::default()
    };

    let report = MapBudgetProfiler::analyze(&snapshot);

    assert!(matches!(report.overall_grade, MapHealthGrade::Healthy | MapHealthGrade::Caution));
    assert!(report.overall_score >= 75);
    assert_eq!(7231, report.object_budget.total_active_objects);
    assert!(report.object_budget.objects_usage_percent < 60.0);
    assert_eq!(50, report.light_budget.total_scene_lights);
    assert!(report.light_budget.peak_cluster_lights < 48);
    assert_eq!(1, report.draw_budget.estimated_atlas_pages);
    assert_eq!(0, report.draw_budget.estimated_atlas_switches);
    assert!(!report.bottlenecks.iter().any(|b| b.is_engine_limit_violation));
}
