//! 導航與地圖診斷指令集 (HealNavMesh, Diagnose)

use crate::macro_cmd::execution::EditorCommand;
use crate::macro_cmd::models::{
    CommandExecutionContext, CommandParameter, CommandResult,
    MacroStepSnapshot, ParsedCommand,
};

/// 道路斷裂自動修復指令 (/heal-navmesh)。
pub struct HealNavMeshCommand;

impl EditorCommand for HealNavMeshCommand {
    fn name(&self) -> &str {
        "heal-navmesh"
    }

    fn aliases(&self) -> &[&str] {
        &["heal-roads", "fix-nav"]
    }

    fn category(&self) -> &str {
        "Pathfinding"
    }

    fn description_zh(&self) -> &str {
        "自動偵測地表道路斷裂缺口（1 格間隙）並進行修補接合。"
    }

    fn description_en(&self) -> &str {
        "Detect and heal 1-tile gaps in road networks."
    }

    fn usage_syntax(&self) -> &str {
        "/heal-navmesh"
    }

    fn parameters(&self) -> &[CommandParameter] {
        static PARAMS: &[CommandParameter] = &[];
        PARAMS
    }

    fn execute(
        &self,
        _command: &ParsedCommand,
        context: &mut CommandExecutionContext,
        snapshots: &mut Option<&mut Vec<MacroStepSnapshot>>,
    ) -> CommandResult {
        let dim = context.map_tile_dimension;
        let textures = match &mut context.textures {
            Some(t) if t.len() >= dim * dim => t,
            _ => return CommandResult::fail("地表材質未就緒。"),
        };

        let is_road = |t: &str| -> bool {
            let upper = t.to_ascii_uppercase();
            upper.contains("WEG") || upper.contains("ROAD") || upper.contains("STRASSE")
        };

        let mut changes = Vec::new();

        // 掃描水平方向 1-格缺口
        for z in 0..dim {
            for x in 1..(dim - 1) {
                let idx = z * dim + x;
                let cur = &textures[idx];
                if !is_road(cur) {
                    let left = &textures[z * dim + (x - 1)];
                    let right = &textures[z * dim + (x + 1)];
                    if is_road(left) && is_road(right) {
                        let road_mat = left.clone();
                        changes.push((idx, cur.clone(), road_mat.clone()));
                        textures[idx] = road_mat;
                    }
                }
            }
        }

        // 掃描垂直方向 1-格缺口
        for z in 1..(dim - 1) {
            for x in 0..dim {
                let idx = z * dim + x;
                let cur = &textures[idx];
                if !is_road(cur) {
                    let up = &textures[(z - 1) * dim + x];
                    let down = &textures[(z + 1) * dim + x];
                    if is_road(up) && is_road(down) {
                        let road_mat = up.clone();
                        changes.push((idx, cur.clone(), road_mat.clone()));
                        textures[idx] = road_mat;
                    }
                }
            }
        }

        let count = changes.len();
        if let Some(recorder) = snapshots {
            recorder.push(MacroStepSnapshot::TextureDeltas { changes });
        }

        CommandResult::ok(format!("成功自動修補 {count} 處道路斷裂缺口。"), count)
    }
}

/// 地圖健全度診斷指令 (/diagnose)。
pub struct DiagnoseCommand;

impl EditorCommand for DiagnoseCommand {
    fn name(&self) -> &str {
        "diagnose"
    }

    fn aliases(&self) -> &[&str] {
        &["diag", "check-map"]
    }

    fn category(&self) -> &str {
        "Diagnostics"
    }

    fn description_zh(&self) -> &str {
        "執行全地圖深度健全度診斷，回報高程範圍、阻擋率與物件配置統計。"
    }

    fn description_en(&self) -> &str {
        "Run deep map diagnostics on height bounds, collision ratio, and placed objects."
    }

    fn usage_syntax(&self) -> &str {
        "/diagnose"
    }

    fn parameters(&self) -> &[CommandParameter] {
        static PARAMS: &[CommandParameter] = &[];
        PARAMS
    }

    fn execute(
        &self,
        _command: &ParsedCommand,
        context: &mut CommandExecutionContext,
        _snapshots: &mut Option<&mut Vec<MacroStepSnapshot>>,
    ) -> CommandResult {
        let mut report = Vec::new();
        report.push("=== Against Rome 地圖健全度診斷報告 ===".to_string());

        // 高程場資訊
        if let Some(heights) = &context.heights {
            let min_h = heights.iter().copied().min().unwrap_or(0);
            let max_h = heights.iter().copied().max().unwrap_or(0);
            report.push(format!(
                "高程網格: {}x{}，高度範圍: {} ~ {} (World Y: {:.1} ~ {:.1})",
                context.vertex_size,
                context.vertex_size,
                min_h,
                max_h,
                min_h as f32 * context.height_step,
                max_h as f32 * context.height_step
            ));
        }

        // 阻擋網格資訊
        if let Some(collision) = &context.collision {
            let blocked_count = collision.iter().filter(|&&b| b > 0).count();
            let ratio = (blocked_count as f32 / collision.len().max(1) as f32) * 100.0;
            report.push(format!(
                "阻擋網格: {}x{}，阻擋像素: {} / {} ({:.1}%)",
                context.collision_size,
                context.collision_size,
                blocked_count,
                collision.len(),
                ratio
            ));
        }

        // 物件資訊
        report.push(format!("場上物件總數: {} 個", context.placed_objects.len()));

        CommandResult::ok_with_details(
            format!("診斷完成，產出 {} 項統計指標。", report.len()),
            report.len(),
            report,
        )
    }
}
