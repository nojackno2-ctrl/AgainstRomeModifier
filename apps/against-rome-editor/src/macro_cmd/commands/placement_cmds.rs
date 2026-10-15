//! 物件放置相關指令集 (SpawnRing, AlignGrid, SelectTeam, SetTeam, Scatter)

use std::f32::consts::PI;
use uuid::Uuid;

use crate::macro_cmd::execution::EditorCommand;
use crate::macro_cmd::models::{
    CommandExecutionContext, CommandParameter, CommandResult, MacroPlacedObject,
    MacroStepSnapshot, ParsedCommand,
};

/// 圓環生成物件指令 (/spawn-ring)。
pub struct SpawnRingCommand;

impl EditorCommand for SpawnRingCommand {
    fn name(&self) -> &str {
        "spawn-ring"
    }

    fn aliases(&self) -> &[&str] {
        &["ring", "circle-spawn"]
    }

    fn category(&self) -> &str {
        "Placement"
    }

    fn description_zh(&self) -> &str {
        "以指定圓心與半徑等距環狀生成物件 (部隊或建築)。"
    }

    fn description_en(&self) -> &str {
        "Spawn a ring of objects uniformly along a circle."
    }

    fn usage_syntax(&self) -> &str {
        "/spawn-ring <type> <cx> <cz> <radius> <count> [--team <team>] [--angle <angle>]"
    }

    fn parameters(&self) -> &[CommandParameter] {
        static PARAMS: &[CommandParameter] = &[];
        PARAMS
    }

    fn execute(
        &self,
        command: &ParsedCommand,
        context: &mut CommandExecutionContext,
        snapshots: &mut Option<&mut Vec<MacroStepSnapshot>>,
    ) -> CommandResult {
        let type_name = match command.get_positional(0) {
            Some(t) => t.to_string(),
            None => return CommandResult::fail("語法錯誤。請指定物件型別或別名。"),
        };

        let cx = match command.try_get_positional_f32(1) {
            Some(v) => v,
            None => return CommandResult::fail("語法錯誤。請指定圓心 X 座標。"),
        };
        let cz = match command.try_get_positional_f32(2) {
            Some(v) => v,
            None => return CommandResult::fail("語法錯誤。請指定圓心 Z 座標。"),
        };
        let radius = match command.try_get_positional_f32(3) {
            Some(v) => v,
            None => return CommandResult::fail("語法錯誤。請指定半徑。"),
        };
        let count = match command.try_get_positional_i32(4) {
            Some(v) if v > 0 => v as usize,
            _ => return CommandResult::fail("語法錯誤。請指定正整數生成數量。"),
        };

        let team = command.try_get_flag_i32("team").unwrap_or(0);

        let mut added_indices = Vec::new();
        let angle_step = 2.0 * PI / count as f32;

        for i in 0..count {
            let angle = i as f32 * angle_step;
            let x = cx + radius * angle.cos();
            let z = cz + radius * angle.sin();
            let y = context.get_height_at_world(x, z);

            let obj = MacroPlacedObject {
                id: Uuid::new_v4(),
                type_name: type_name.clone(),
                world_x: x,
                world_y: y,
                world_z: z,
                team,
                unit_count: 10,
            };

            let idx = context.placed_objects.len();
            context.placed_objects.push(obj);
            added_indices.push(idx);
        }

        if let Some(recorder) = snapshots {
            recorder.push(MacroStepSnapshot::PlacementAdd { added_indices });
        }

        CommandResult::ok(
            format!("成功沿圓環生成 {count} 個物件「{type_name}」。"),
            count,
        )
    }
}

/// 網格對齊指令 (/align-grid)。
pub struct AlignGridCommand;

impl EditorCommand for AlignGridCommand {
    fn name(&self) -> &str {
        "align-grid"
    }

    fn aliases(&self) -> &[&str] {
        &["snap-grid", "grid"]
    }

    fn category(&self) -> &str {
        "Placement"
    }

    fn description_zh(&self) -> &str {
        "將場上物件座標對齊至指定格線大小倍數。"
    }

    fn description_en(&self) -> &str {
        "Snap objects to the nearest grid step multiple."
    }

    fn usage_syntax(&self) -> &str {
        "/align-grid <step>"
    }

    fn parameters(&self) -> &[CommandParameter] {
        static PARAMS: &[CommandParameter] = &[];
        PARAMS
    }

    fn execute(
        &self,
        command: &ParsedCommand,
        context: &mut CommandExecutionContext,
        snapshots: &mut Option<&mut Vec<MacroStepSnapshot>>,
    ) -> CommandResult {
        let step = match command.try_get_positional_f32(0) {
            Some(s) if s > 0.0 => s,
            _ => return CommandResult::fail("請指定大於 0 的網格間距。用法: /align-grid <step>"),
        };

        let mut changes = Vec::new();

        for (idx, obj) in context.placed_objects.iter_mut().enumerate() {
            let snapped_x = (obj.world_x / step).round() * step;
            let snapped_z = (obj.world_z / step).round() * step;

            if (obj.world_x - snapped_x).abs() > 0.001 || (obj.world_z - snapped_z).abs() > 0.001 {
                let before = obj.clone();
                obj.world_x = snapped_x;
                obj.world_z = snapped_z;
                let after = obj.clone();
                changes.push((idx, before, after));
            }
        }

        let count = changes.len();
        if let Some(recorder) = snapshots {
            recorder.push(MacroStepSnapshot::PlacementModify { changes });
        }

        CommandResult::ok(format!("已對齊 {count} 個物件至 {step} 網格。"), count)
    }
}

/// 選取指定隊伍物件指令 (/select-team)。
pub struct SelectTeamCommand;

impl EditorCommand for SelectTeamCommand {
    fn name(&self) -> &str {
        "select-team"
    }

    fn aliases(&self) -> &[&str] {
        &["sel-team"]
    }

    fn category(&self) -> &str {
        "Placement"
    }

    fn description_zh(&self) -> &str {
        "選取所有所屬隊伍為指定 Team ID 之物件。"
    }

    fn description_en(&self) -> &str {
        "Select all objects belonging to specified Team ID."
    }

    fn usage_syntax(&self) -> &str {
        "/select-team <teamId>"
    }

    fn parameters(&self) -> &[CommandParameter] {
        static PARAMS: &[CommandParameter] = &[];
        PARAMS
    }

    fn execute(
        &self,
        command: &ParsedCommand,
        context: &mut CommandExecutionContext,
        _snapshots: &mut Option<&mut Vec<MacroStepSnapshot>>,
    ) -> CommandResult {
        let team = match command.try_get_positional_i32(0) {
            Some(t) => t,
            None => return CommandResult::fail("請指定欲選取的隊伍 ID。用法: /select-team <teamId>"),
        };

        context.selected_placement_indices.clear();
        for (idx, obj) in context.placed_objects.iter().enumerate() {
            if obj.team == team {
                context.selected_placement_indices.push(idx);
            }
        }

        let count = context.selected_placement_indices.len();
        CommandResult::ok(format!("已選取 {count} 個隊伍 {team} 之物件。"), count)
    }
}

/// 批量修改隊伍指令 (/set-team)。
pub struct SetTeamCommand;

impl EditorCommand for SetTeamCommand {
    fn name(&self) -> &str {
        "set-team"
    }

    fn aliases(&self) -> &[&str] {
        &["team"]
    }

    fn category(&self) -> &str {
        "Placement"
    }

    fn description_zh(&self) -> &str {
        "將選取或全部物件之所屬隊伍批量修改為目標 Team ID。"
    }

    fn description_en(&self) -> &str {
        "Change team of selected or all objects to target Team ID."
    }

    fn usage_syntax(&self) -> &str {
        "/set-team <teamId> [--selected]"
    }

    fn parameters(&self) -> &[CommandParameter] {
        static PARAMS: &[CommandParameter] = &[];
        PARAMS
    }

    fn execute(
        &self,
        command: &ParsedCommand,
        context: &mut CommandExecutionContext,
        snapshots: &mut Option<&mut Vec<MacroStepSnapshot>>,
    ) -> CommandResult {
        let target_team = match command.try_get_positional_i32(0) {
            Some(t) => t,
            None => return CommandResult::fail("請指定目標隊伍 ID。用法: /set-team <teamId>"),
        };

        let only_selected = command.has_flag("selected");
        let mut changes = Vec::new();

        let indices: Vec<usize> = if only_selected {
            context.selected_placement_indices.clone()
        } else {
            (0..context.placed_objects.len()).collect()
        };

        for idx in indices {
            if idx < context.placed_objects.len() {
                let obj = &mut context.placed_objects[idx];
                if obj.team != target_team {
                    let before = obj.clone();
                    obj.team = target_team;
                    let after = obj.clone();
                    changes.push((idx, before, after));
                }
            }
        }

        let count = changes.len();
        if let Some(recorder) = snapshots {
            recorder.push(MacroStepSnapshot::PlacementModify { changes });
        }

        CommandResult::ok(format!("已將 {count} 個物件隊伍變更為 {target_team}。"), count)
    }
}

/// 散佈物件指令 (/scatter)。
pub struct ScatterCommand;

impl EditorCommand for ScatterCommand {
    fn name(&self) -> &str {
        "scatter"
    }

    fn aliases(&self) -> &[&str] {
        &["distribute"]
    }

    fn category(&self) -> &str {
        "Nature"
    }

    fn description_zh(&self) -> &str {
        "在指定矩形區域內隨機散佈指定數量的物件。"
    }

    fn description_en(&self) -> &str {
        "Scatter objects randomly within a rectangular region."
    }

    fn usage_syntax(&self) -> &str {
        "/scatter <type> <count> <rect> [--spacing <s>] [--seed <seed>]"
    }

    fn parameters(&self) -> &[CommandParameter] {
        static PARAMS: &[CommandParameter] = &[];
        PARAMS
    }

    fn execute(
        &self,
        command: &ParsedCommand,
        context: &mut CommandExecutionContext,
        snapshots: &mut Option<&mut Vec<MacroStepSnapshot>>,
    ) -> CommandResult {
        let type_name = match command.get_positional(0) {
            Some(t) => t.to_string(),
            None => return CommandResult::fail("請指定物件型別名稱。"),
        };

        let count = match command.try_get_positional_i32(1) {
            Some(v) if v > 0 => v as usize,
            _ => return CommandResult::fail("請指定正整數散佈數量。"),
        };

        let rect_raw = match command.get_positional(2) {
            Some(r) => r,
            None => return CommandResult::fail("請指定散佈矩形範圍 x1,z1,x2,z2。"),
        };

        let rect = match ParsedCommand::try_parse_rect(rect_raw) {
            Some(r) => r,
            None => return CommandResult::fail("無法解析矩形範圍。格式: x1,z1,x2,z2"),
        };

        let seed = command.try_get_flag_i32("seed").unwrap_or(42) as u64;

        // 簡單 LCG 隨機生成點位
        let mut rng_state = seed.wrapping_add(1);
        let mut next_rand = || -> f32 {
            rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((rng_state >> 33) as f32) / (u32::MAX as f32)
        };

        let mut added_indices = Vec::new();
        let width = (rect.max_x - rect.min_x).max(1) as f32;
        let height = (rect.max_z - rect.min_z).max(1) as f32;

        for _ in 0..count {
            let x = rect.min_x as f32 + next_rand() * width;
            let z = rect.min_z as f32 + next_rand() * height;
            let y = context.get_height_at_world(x, z);

            let obj = MacroPlacedObject {
                id: Uuid::new_v4(),
                type_name: type_name.clone(),
                world_x: x,
                world_y: y,
                world_z: z,
                team: 0,
                unit_count: 1,
            };

            let idx = context.placed_objects.len();
            context.placed_objects.push(obj);
            added_indices.push(idx);
        }

        if let Some(recorder) = snapshots {
            recorder.push(MacroStepSnapshot::PlacementAdd { added_indices });
        }

        CommandResult::ok(format!("已成功散佈 {count} 個「{type_name}」。"), count)
    }
}
