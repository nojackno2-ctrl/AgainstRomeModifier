//! 巨集指令解析與執行單元測試 (Macro Command Tests)
//!
//! 完整移植自 C# `EditorCommandParserTests.cs`, `MacroScriptRunnerTests.cs`, `MacroDomainCommandsTests.cs`。

use std::collections::HashMap;
use uuid::Uuid;

use crate::macro_cmd::commands;
use crate::macro_cmd::execution::{CommandRegistry, MacroScriptRunner};
use crate::macro_cmd::models::{
    CommandExecutionContext, MacroPlacedObject, ParsedCommand,
};
use crate::macro_cmd::parser;

fn create_test_runner() -> (CommandRegistry, MacroScriptRunner) {
    let mut registry = CommandRegistry::new();
    commands::register_default_commands(&mut registry);
    let mut reg_copy = CommandRegistry::new();
    commands::register_default_commands(&mut reg_copy);
    let runner = MacroScriptRunner::new(registry);
    (reg_copy, runner)
}

#[test]
fn test_parse_basic_command_with_positional_args() {
    let input = "/elevate rect 10 20 30 40 15";
    let cmd = parser::parse(input, None).expect("應成功解析指令");

    assert_eq!(cmd.command_name, "elevate");
    assert_eq!(cmd.positional_args.len(), 6);
    assert_eq!(cmd.positional_args[0], "rect");
    assert_eq!(cmd.positional_args[1], "10");
    assert_eq!(cmd.positional_args[2], "20");
    assert_eq!(cmd.positional_args[3], "30");
    assert_eq!(cmd.positional_args[4], "40");
    assert_eq!(cmd.positional_args[5], "15");
    assert!(cmd.named_flags.is_empty());
}

#[test]
fn test_parse_with_quotes_and_flags() {
    let input = "/scatter \"BauGer Wald 01\" 25 100,200,300,400 --spacing 64 --seed 42 --selected";
    let cmd = parser::parse(input, None).expect("應成功解析包含引號與旗標之指令");

    assert_eq!(cmd.command_name, "scatter");
    assert_eq!(cmd.positional_args.len(), 3);
    assert_eq!(cmd.positional_args[0], "BauGer Wald 01");
    assert_eq!(cmd.positional_args[1], "25");
    assert_eq!(cmd.positional_args[2], "100,200,300,400");

    assert!(cmd.has_flag("spacing"));
    assert_eq!(cmd.get_flag("spacing"), Some("64"));

    assert!(cmd.has_flag("seed"));
    assert_eq!(cmd.get_flag("seed"), Some("42"));

    assert!(cmd.has_flag("selected"));
    assert_eq!(cmd.get_flag("selected"), None); // 布林旗標值為 None
}

#[test]
fn test_parse_comments_and_whitespace_returns_none() {
    assert!(parser::parse("   ", None).is_none());
    assert!(parser::parse("# This is a comment", None).is_none());
    assert!(parser::parse("// Another comment style", None).is_none());
}

#[test]
fn test_parse_variable_substitution() {
    let mut vars = HashMap::new();
    vars.insert("tree".to_string(), "LanGerTanne01".to_string());
    vars.insert("radius".to_string(), "500".to_string());

    let input = "/spawn-ring $tree 1000 2000 $radius 8";
    let cmd = parser::parse(input, Some(&vars)).expect("應成功替換變數");

    assert_eq!(cmd.positional_args[0], "LanGerTanne01");
    assert_eq!(cmd.positional_args[3], "500");
}

#[test]
fn test_geometry_rect_and_circle_parsing() {
    let r = ParsedCommand::try_parse_rect("10,20,30,40").expect("應成功解析矩形");
    assert_eq!(r.min_x, 10);
    assert_eq!(r.min_z, 20);
    assert_eq!(r.max_x, 30);
    assert_eq!(r.max_z, 40);
    assert!(r.contains(15, 25));
    assert!(!r.contains(5, 25));

    let c = ParsedCommand::try_parse_circle("100,200,50").expect("應成功解析圓形");
    assert_eq!(c.center_x, 100.0);
    assert_eq!(c.center_z, 200.0);
    assert_eq!(c.radius, 50.0);
    assert!(c.contains(110.0, 210.0));
    assert!(!c.contains(200.0, 200.0));
}

#[test]
fn test_tab_completion_command_names() {
    let (registry, _) = create_test_runner();
    let completions = parser::get_completions("/el", 3, None, registry.get_all_commands());

    assert!(completions.contains(&"/elevate".to_string()));
}

#[test]
fn test_execute_line_variable_assignment_stores_variable() {
    let (_, mut runner) = create_test_runner();
    let mut context = CommandExecutionContext::default();

    let res = runner.execute_line("$delta = 20", &mut context, false);
    assert!(res.success);
    assert_eq!(context.variables.get("delta").map(|s| s.as_str()), Some("20"));
}

#[test]
fn test_execute_script_valid_macro_succeeds_and_bundles_into_single_undo() {
    let (_, mut runner) = create_test_runner();
    let heights = vec![50u8; 16 * 16];
    let mut context = CommandExecutionContext {
        heights: Some(heights),
        vertex_size: 16,
        world_dimension: 1024.0,
        ..Default::default()
    };

    let script = r#"
# @name TestElevationMacro
$step = 10
/elevate rect 2 2 4 4 $step
/elevate rect 5 5 7 7 15
"#;

    let report = runner.execute_script(script, &mut context, None, None, true);

    assert!(report.success);
    assert_eq!(report.total_commands, 3);
    assert_eq!(report.succeeded_commands, 3);
    assert_eq!(report.failed_commands, 0);
    assert!(!report.was_rolled_back);

    let h = context.heights.as_ref().unwrap();
    // 頂點 (3,3) 原本 50，現在應為 60
    assert_eq!(h[3 * 16 + 3], 60);
    // 頂點 (6,6) 原本 50，現在應為 65
    assert_eq!(h[6 * 16 + 6], 65);

    // 單次 Undo 撤銷全部巨集步驟
    assert!(runner.can_undo());
    let undone = runner.undo(&mut context);
    assert!(undone.is_some());
    assert_eq!(undone.unwrap().step_count(), 2);
    assert!(!runner.can_undo());

    let h_undo = context.heights.as_ref().unwrap();
    assert_eq!(h_undo[3 * 16 + 3], 50);
    assert_eq!(h_undo[6 * 16 + 6], 50);

    // 單次 Redo 重新套用全部巨集步驟
    assert!(runner.can_redo());
    let redone = runner.redo(&mut context);
    assert!(redone.is_some());

    let h_redo = context.heights.as_ref().unwrap();
    assert_eq!(h_redo[3 * 16 + 3], 60);
    assert_eq!(h_redo[6 * 16 + 6], 65);
}

#[test]
fn test_execute_script_fails_midway_performs_atomic_rollback() {
    let (_, mut runner) = create_test_runner();
    let heights = vec![50u8; 16 * 16];
    let mut context = CommandExecutionContext {
        heights: Some(heights),
        vertex_size: 16,
        world_dimension: 1024.0,
        ..Default::default()
    };

    let failing_script = r#"
# @name PartialFailMacro
/elevate rect 2 2 4 4 25
/non_existent_command_error
/elevate rect 5 5 7 7 25
"#;

    let report = runner.execute_script(failing_script, &mut context, None, None, true);

    assert!(!report.success);
    assert!(report.was_rolled_back);
    assert_eq!(report.failed_commands, 1);

    // 關鍵斷言：第 1 步的 elevate 必須被原子性回滾，狀態恢復 50
    let h = context.heights.as_ref().unwrap();
    assert_eq!(h[3 * 16 + 3], 50);
    assert_eq!(h[6 * 16 + 6], 50);
}

#[test]
fn test_elevate_and_flatten_rect_and_circle_modifies_heights_correctly() {
    let (_, mut runner) = create_test_runner();
    let heights = vec![40u8; 16 * 16];
    let mut context = CommandExecutionContext {
        heights: Some(heights),
        vertex_size: 16,
        world_dimension: 1024.0,
        ..Default::default()
    };

    // 1. Elevate rect
    let res1 = runner.execute_line("/elevate rect 2 2 6 6 20", &mut context, true);
    assert!(res1.success);
    assert_eq!(context.heights.as_ref().unwrap()[4 * 16 + 4], 60);

    // 2. Flatten circle
    let res2 = runner.execute_line("/flatten circle 4 4 2 95", &mut context, true);
    assert!(res2.success);
    assert_eq!(context.heights.as_ref().unwrap()[4 * 16 + 4], 95);

    // 3. Undo
    runner.undo(&mut context); // undo flatten
    assert_eq!(context.heights.as_ref().unwrap()[4 * 16 + 4], 60);

    runner.undo(&mut context); // undo elevate
    assert_eq!(context.heights.as_ref().unwrap()[4 * 16 + 4], 40);
}

#[test]
fn test_spawn_ring_places_objects_in_circle_with_correct_coordinates() {
    let (_, mut runner) = create_test_runner();
    let mut context = CommandExecutionContext {
        world_dimension: 16384.0,
        ..Default::default()
    };

    let res = runner.execute_line(
        "/spawn-ring BauRomTurm01 5000 5000 400 4 --team 1",
        &mut context,
        true,
    );

    assert!(res.success);
    assert_eq!(res.affected_count, 4);
    assert_eq!(context.placed_objects.len(), 4);

    for i in 0..4 {
        let item = &context.placed_objects[i];
        assert_eq!(item.type_name, "BauRomTurm01");
        assert_eq!(item.team, 1);

        let dx = item.world_x - 5000.0;
        let dz = item.world_z - 5000.0;
        let dist = (dx * dx + dz * dz).sqrt();
        assert!((dist - 400.0).abs() < 1.0);
    }

    // 驗證單步 Undo
    assert!(runner.can_undo());
    runner.undo(&mut context);
    assert_eq!(context.placed_objects.len(), 0);
}

#[test]
fn test_align_grid_snaps_coordinates_to_specified_step() {
    let (_, mut runner) = create_test_runner();
    let mut context = CommandExecutionContext::default();

    context.placed_objects.push(MacroPlacedObject {
        id: Uuid::new_v4(),
        type_name: "BauGerHau01".to_string(),
        world_x: 105.0,
        world_y: 0.0,
        world_z: 215.0,
        team: 0,
        unit_count: 1,
    });
    context.placed_objects.push(MacroPlacedObject {
        id: Uuid::new_v4(),
        type_name: "BauGerHau01".to_string(),
        world_x: 301.0,
        world_y: 0.0,
        world_z: 410.0,
        team: 0,
        unit_count: 1,
    });

    let res = runner.execute_line("/align-grid 32", &mut context, true);
    assert!(res.success);
    assert_eq!(res.affected_count, 2);

    // 105 -> 96 (32 * 3), 215 -> 224 (32 * 7)
    assert_eq!(context.placed_objects[0].world_x, 96.0);
    assert_eq!(context.placed_objects[0].world_z, 224.0);

    // 301 -> 288 (32 * 9), 410 -> 416 (32 * 13)
    assert_eq!(context.placed_objects[1].world_x, 288.0);
    assert_eq!(context.placed_objects[1].world_z, 416.0);

    // Undo 測試
    runner.undo(&mut context);
    assert_eq!(context.placed_objects[0].world_x, 105.0);
    assert_eq!(context.placed_objects[0].world_z, 215.0);
    assert_eq!(context.placed_objects[1].world_x, 301.0);
    assert_eq!(context.placed_objects[1].world_z, 410.0);

    assert!(runner.can_redo());
    runner.redo(&mut context);
    assert_eq!(context.placed_objects[0].world_x, 96.0);
    assert_eq!(context.placed_objects[0].world_z, 224.0);
    assert_eq!(context.placed_objects[1].world_x, 288.0);
    assert_eq!(context.placed_objects[1].world_z, 416.0);
}

#[test]
fn test_select_and_set_team_filters_and_batch_modifies_team_id() {
    let (_, mut runner) = create_test_runner();
    let mut context = CommandExecutionContext::default();

    context.placed_objects.push(MacroPlacedObject {
        id: Uuid::new_v4(),
        type_name: "GER_INF01".to_string(),
        world_x: 100.0,
        world_y: 0.0,
        world_z: 100.0,
        team: 0,
        unit_count: 10,
    });
    context.placed_objects.push(MacroPlacedObject {
        id: Uuid::new_v4(),
        type_name: "GER_INF01".to_string(),
        world_x: 200.0,
        world_y: 0.0,
        world_z: 200.0,
        team: 1,
        unit_count: 10,
    });
    context.placed_objects.push(MacroPlacedObject {
        id: Uuid::new_v4(),
        type_name: "BauRomTurm01".to_string(),
        world_x: 300.0,
        world_y: 0.0,
        world_z: 300.0,
        team: 1,
        unit_count: 1,
    });

    // 1. Select team 1
    let sel_res = runner.execute_line("/select-team 1", &mut context, false);
    assert!(sel_res.success);
    assert_eq!(context.selected_placement_indices.len(), 2);

    // 2. Set team 3 on selected
    let set_res = runner.execute_line("/set-team 3 --selected", &mut context, true);
    assert!(set_res.success);
    assert_eq!(set_res.affected_count, 2);

    assert_eq!(context.placed_objects[0].team, 0);
    assert_eq!(context.placed_objects[1].team, 3);
    assert_eq!(context.placed_objects[2].team, 3);

    // 3. Undo
    runner.undo(&mut context);
    assert_eq!(context.placed_objects[1].team, 1);
    assert_eq!(context.placed_objects[2].team, 1);
}

#[test]
fn test_replace_texture_replaces_in_bounding_rect_and_undo_restores() {
    let (_, mut runner) = create_test_runner();
    const DIM: usize = 64;
    let textures = vec!["Gras1".to_string(); DIM * DIM];

    let mut context = CommandExecutionContext {
        textures: Some(textures),
        map_tile_dimension: DIM,
        world_dimension: (DIM * 256) as f32,
        ..Default::default()
    };

    // Replace Gras1 with Sand1 in rect 10,10 to 12,12 (3x3 = 9 tiles)
    let res = runner.execute_line(
        "/replace-texture Gras1 Sand1 --rect 10,10,12,12",
        &mut context,
        true,
    );
    assert!(res.success);
    assert_eq!(res.affected_count, 9);

    let t = context.textures.as_ref().unwrap();
    assert_eq!(t[10 * DIM + 10], "Sand1");
    assert_eq!(t[12 * DIM + 12], "Sand1");
    assert_eq!(t[9 * DIM + 10], "Gras1");

    // Undo
    assert!(runner.can_undo());
    runner.undo(&mut context);
    let t_undo = context.textures.as_ref().unwrap();
    assert_eq!(t_undo[10 * DIM + 10], "Gras1");
    assert_eq!(t_undo[12 * DIM + 12], "Gras1");
}

#[test]
fn test_heal_navmesh_finds_and_heals_road_gaps_and_undo_restores() {
    let (_, mut runner) = create_test_runner();
    const DIM: usize = 64;
    let mut textures = vec!["Gras1".to_string(); DIM * DIM];

    // 在 (20, 15) 留出缺口：端點在 (19, 15) 與 (21, 15)
    textures[15 * DIM + 18] = "H_WEG1".to_string();
    textures[15 * DIM + 19] = "H_WEG1".to_string();
    // 缺口在 (20, 15)
    textures[15 * DIM + 21] = "H_WEG1".to_string();
    textures[15 * DIM + 22] = "H_WEG1".to_string();

    let mut context = CommandExecutionContext {
        textures: Some(textures),
        map_tile_dimension: DIM,
        ..Default::default()
    };

    let res = runner.execute_line("/heal-navmesh", &mut context, true);
    assert!(res.success);
    assert_eq!(res.affected_count, 1);

    // 缺口 (20, 15) 應被修補為 H_WEG1
    assert_eq!(context.textures.as_ref().unwrap()[15 * DIM + 20], "H_WEG1");

    // Undo 測試
    assert!(runner.can_undo());
    runner.undo(&mut context);
    assert_eq!(context.textures.as_ref().unwrap()[15 * DIM + 20], "Gras1");
}
