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

// ---------------------------------------------------------------------------
// /brush：console 與 native AI planner 共用 canonical BrushCommand
// ---------------------------------------------------------------------------

mod brush_canonical {
    use against_rome_ai_planner::BrushCommand as CanonicalBrush;

    use super::create_test_runner;
    use crate::macro_cmd::commands::terrain_cmds::{canonical_from_log, canonicalize_brush_line};
    use crate::macro_cmd::models::CommandExecutionContext;

    const VERTEX: usize = 17;
    const TILES: usize = 16;

    fn brush_context() -> CommandExecutionContext {
        CommandExecutionContext {
            heights: Some(vec![100u8; VERTEX * VERTEX]),
            vertex_size: VERTEX,
            map_tile_dimension: TILES,
            water_level: 8.0,
            ..Default::default()
        }
    }

    fn heights(context: &CommandExecutionContext) -> Vec<u8> {
        context.heights.clone().unwrap()
    }

    #[test]
    fn brush_applies_canonical_height_stroke_and_undo_restores() {
        let (_, mut runner) = create_test_runner();
        let mut context = brush_context();
        let before = heights(&context);

        let res = runner.execute_line("/brush height raise 5,5 6,5 --size 3 --strength strong", &mut context, true);
        assert!(res.success, "{}", res.message);
        assert!(res.affected_count > 0);
        let after = heights(&context);
        assert_ne!(after, before);
        assert!(after.iter().zip(&before).all(|(a, b)| a >= b), "raise 不得降低高度");

        runner.undo(&mut context);
        assert_eq!(heights(&context), before);
        runner.redo(&mut context);
        assert_eq!(heights(&context), after);
    }

    #[test]
    fn brush_matches_native_canonical_run_exactly() {
        use against_rome_editor::terrain::TerrainSession;

        let (_, mut runner) = create_test_runner();
        let mut context = brush_context();
        let line = "/brush height lower 4,4 8,8 --size 5 --strength gentle";
        assert!(runner.execute_line(line, &mut context, true).success);

        let mut session = TerrainSession::new(VERTEX, vec![100u8; VERTEX * VERTEX], 0, None).unwrap();
        CanonicalBrush::run(line, Some(&mut session), None, TILES, 8.0).unwrap();
        assert_eq!(heights(&context), session.heights());
    }

    #[test]
    fn brush_aliases_and_spellings_canonicalize_and_roundtrip() {
        let spellings = [
            "/brush height raise 3,3",
            "/paint HEIGHT Raise 3,3 --size 1 --strength medium",
            "/stroke height raise 3,3 --strength medium --size 1",
            "brush height raise 3,3",
        ];
        let canonical = canonicalize_brush_line(spellings[0], TILES).unwrap();
        assert_eq!(canonical, "/brush height raise 3,3 --size 1 --strength medium");
        for s in spellings {
            assert_eq!(canonicalize_brush_line(s, TILES).unwrap(), canonical, "{s}");
        }
        // 規範化後再規範化是恆等式，且與 native format(parse()) 一致。
        assert_eq!(canonicalize_brush_line(&canonical, TILES).unwrap(), canonical);
        let stroke = CanonicalBrush::parse(&canonical, TILES).unwrap();
        assert_eq!(CanonicalBrush::format(&stroke).unwrap(), canonical);
    }

    #[test]
    fn brush_command_log_roundtrips_and_replays_to_identical_heights() {
        let (_, mut runner) = create_test_runner();
        let mut original = brush_context();
        let typed = [
            "/paint height raise 2,2 5,2 --size 3",
            "/brush HEIGHT smooth 4,4 --size 5 --strength strong",
            "/stroke height flatten 8,8 10,8 --size 3 --strength gentle",
        ];
        let mut replay_log = Vec::new();
        for line in typed {
            let res = runner.execute_line(line, &mut original, true);
            assert!(res.success, "{line}: {}", res.message);
            // 結果訊息與 context 日誌都帶有 canonical 重播文字。
            let from_msg = canonical_from_log(&res.message).expect("訊息缺少 canonical").to_string();
            assert_eq!(res.details.as_deref(), Some(&[from_msg.clone()][..]));
            let ok_log = original
                .logs
                .iter()
                .rev()
                .find(|(m, _)| m.starts_with("[OK] /"))
                .map(|(m, _)| m.clone())
                .unwrap();
            assert_eq!(canonical_from_log(&ok_log), Some(from_msg.as_str()));
            assert_eq!(canonicalize_brush_line(&from_msg, TILES).unwrap(), from_msg);
            replay_log.push(from_msg);
        }

        let (_, mut replay_runner) = create_test_runner();
        let mut replayed = brush_context();
        for line in &replay_log {
            assert!(replay_runner.execute_line(line, &mut replayed, true).success, "{line}");
        }
        assert_eq!(heights(&replayed), heights(&original));

        // 同一份 canonical 日誌以 transactional 腳本重播，結果相同且為單一 undo 單元。
        let (_, mut script_runner) = create_test_runner();
        let mut scripted = brush_context();
        let before = heights(&scripted);
        let report = script_runner.execute_script(&replay_log.join("\n"), &mut scripted, Some("replay.armcmd"), None, true);
        assert!(report.success, "{:?}", report.error_message);
        assert_eq!(heights(&scripted), heights(&original));
        script_runner.undo(&mut scripted);
        assert_eq!(heights(&scripted), before);
    }

    #[test]
    fn brush_rejects_legacy_syntax_without_touching_heights() {
        let (_, mut runner) = create_test_runner();
        let mut context = brush_context();
        let before = heights(&context);
        for line in ["/brush raise 5 5 3", "/brush flatten 5 5 3 90", "/paint LOWER 4 4 2 8"] {
            let res = runner.execute_line(line, &mut context, true);
            assert!(!res.success, "{line}");
            assert!(res.message.contains("/brush height"), "應提示 canonical 語法: {}", res.message);
        }
        assert_eq!(heights(&context), before);
        assert!(!runner.can_undo());
    }

    #[test]
    fn brush_rejects_invalid_and_unsupported_strokes_atomically() {
        let (_, mut runner) = create_test_runner();
        let mut context = brush_context();
        let before = heights(&context);
        for line in [
            "/brush",
            "/brush height raise",
            "/brush height raise 5,5 --size 4",
            "/brush height raise 5,5 --strength extreme",
            "/brush height raise 99,99",
            "/brush height raise -1,3",
            "/brush height raise 5.5,5",
            "/brush height raise 5,5 --size",
            "/brush height raise 5,5 --size 3 --size 3",
            "/brush height bogus 5,5",
            "/brush height raise 5,5 --auto-bridge",
            // console context 沒有 collision / blend 圖層：由 canonical run 拒絕。
            "/brush collision block 5,5",
            "/brush material \"Grass 1\" 5,5",
        ] {
            let res = runner.execute_line(line, &mut context, true);
            assert!(!res.success, "{line} 應失敗");
            assert_eq!(heights(&context), before, "{line} 失敗後高度必須不變");
        }
        assert!(!runner.can_undo());

        // 高度尚未就緒。
        let mut empty = CommandExecutionContext::default();
        assert!(!runner.execute_line("/brush height raise 5,5", &mut empty, true).success);
    }

    #[test]
    fn brush_failure_in_transactional_script_rolls_back_earlier_brush() {
        let (_, mut runner) = create_test_runner();
        let mut context = brush_context();
        let before = heights(&context);
        let script = "/brush height raise 6,6 --size 5\n/brush height raise 999,999\n";
        let report = runner.execute_script(script, &mut context, Some("bad.armcmd"), None, true);
        assert!(!report.success);
        assert!(report.was_rolled_back);
        assert_eq!(heights(&context), before);
    }

    #[test]
    fn brush_usage_is_the_canonical_usage() {
        let (registry, _) = create_test_runner();
        let cmd = registry.try_get_command("brush").unwrap();
        assert_eq!(cmd.usage_syntax(), CanonicalBrush::USAGE);
        assert!(registry.try_get_command("paint").is_some());
        assert!(registry.try_get_command("stroke").is_some());
    }
}
