//! 地形高程調整指令集 (Elevate, Flatten, Brush)

use crate::macro_cmd::execution::EditorCommand;
use crate::macro_cmd::models::{
    CommandExecutionContext, CommandParameter, CommandParameterType, CommandResult,
    MacroStepSnapshot, MapRegionCircle, MapRegionRect, ParsedCommand,
};

/// 高程批量調整指令 (/elevate)。
pub struct ElevateCommand;

impl EditorCommand for ElevateCommand {
    fn name(&self) -> &str {
        "elevate"
    }

    fn aliases(&self) -> &[&str] {
        &["height-add", "raise"]
    }

    fn category(&self) -> &str {
        "Terrain"
    }

    fn description_zh(&self) -> &str {
        "在指定矩形或圓形範圍內批量增加或降低地形高度。"
    }

    fn description_en(&self) -> &str {
        "Bulk elevate or lower terrain height in a rectangular or circular region."
    }

    fn usage_syntax(&self) -> &str {
        "/elevate rect <x1> <z1> <x2> <z2> <delta> 或 /elevate circle <cx> <cz> <radius> <delta>"
    }

    fn parameters(&self) -> &[CommandParameter] {
        static PARAMS: &[CommandParameter] = &[
            CommandParameter {
                name: String::new(),
                param_type: CommandParameterType::String,
                desc_zh: String::new(),
                desc_en: String::new(),
                is_required: true,
                default_value: None,
                possible_values: None,
            },
        ];
        PARAMS
    }

    fn execute(
        &self,
        command: &ParsedCommand,
        context: &mut CommandExecutionContext,
        snapshots: &mut Option<&mut Vec<MacroStepSnapshot>>,
    ) -> CommandResult {
        let size = context.vertex_size;
        let heights = match &mut context.heights {
            Some(h) if h.len() >= size * size => h,
            _ => return CommandResult::fail("地形高度未就緒。"),
        };

        #[allow(unused_assignments)]
        let mut delta = 0;
        let mut changes = Vec::new();

        // 判斷旗標模式或位置引數模式
        if command.has_flag("rect") || command.has_flag("circle") {
            if let Some(d) = command.try_get_flag_i32("delta") {
                delta = d;
            } else if let Some(d) = command.try_get_positional_i32(0) {
                delta = d;
            } else {
                return CommandResult::fail("缺少必需的 --delta 數值。");
            }

            if let Some(rect_raw) = command.get_flag("rect") {
                let r = match ParsedCommand::try_parse_rect(rect_raw) {
                    Some(rect) => rect,
                    None => return CommandResult::fail("無法解析 --rect 區域。格式: x1,z1,x2,z2"),
                };
                collect_rect(heights, size, r, delta, &mut changes);
            } else if let Some(circle_raw) = command.get_flag("circle") {
                let c = match ParsedCommand::try_parse_circle(circle_raw) {
                    Some(circle) => circle,
                    None => return CommandResult::fail("無法解析 --circle 區域。格式: cx,cz,radius"),
                };
                collect_circle(heights, size, c, delta, &mut changes);
            }
        } else {
            let mode = command.get_positional(0).map(|s| s.to_ascii_lowercase());
            match mode.as_deref() {
                Some("rect") => {
                    let x1 = match command.try_get_positional_i32(1) {
                        Some(v) => v,
                        None => return CommandResult::fail("語法錯誤。用法: /elevate rect <x1> <z1> <x2> <z2> <delta>"),
                    };
                    let z1 = match command.try_get_positional_i32(2) {
                        Some(v) => v,
                        None => return CommandResult::fail("語法錯誤。用法: /elevate rect <x1> <z1> <x2> <z2> <delta>"),
                    };
                    let x2 = match command.try_get_positional_i32(3) {
                        Some(v) => v,
                        None => return CommandResult::fail("語法錯誤。用法: /elevate rect <x1> <z1> <x2> <z2> <delta>"),
                    };
                    let z2 = match command.try_get_positional_i32(4) {
                        Some(v) => v,
                        None => return CommandResult::fail("語法錯誤。用法: /elevate rect <x1> <z1> <x2> <z2> <delta>"),
                    };
                    delta = match command.try_get_positional_i32(5) {
                        Some(v) => v,
                        None => return CommandResult::fail("語法錯誤。用法: /elevate rect <x1> <z1> <x2> <z2> <delta>"),
                    };
                    let r = MapRegionRect::new(x1, z1, x2, z2);
                    collect_rect(heights, size, r, delta, &mut changes);
                }
                Some("circle") => {
                    let cx = match command.try_get_positional_f32(1) {
                        Some(v) => v,
                        None => return CommandResult::fail("語法錯誤。用法: /elevate circle <cx> <cz> <radius> <delta>"),
                    };
                    let cz = match command.try_get_positional_f32(2) {
                        Some(v) => v,
                        None => return CommandResult::fail("語法錯誤。用法: /elevate circle <cx> <cz> <radius> <delta>"),
                    };
                    let radius = match command.try_get_positional_f32(3) {
                        Some(v) => v,
                        None => return CommandResult::fail("語法錯誤。用法: /elevate circle <cx> <cz> <radius> <delta>"),
                    };
                    delta = match command.try_get_positional_i32(4) {
                        Some(v) => v,
                        None => return CommandResult::fail("語法錯誤。用法: /elevate circle <cx> <cz> <radius> <delta>"),
                    };
                    let c = MapRegionCircle::new(cx, cz, radius);
                    collect_circle(heights, size, c, delta, &mut changes);
                }
                _ => return CommandResult::fail("未知的幾何形狀。請指定 rect 或 circle。"),
            }
        }

        if changes.is_empty() {
            return CommandResult::ok("目標區域無符合頂點或高度無變化。", 0);
        }

        // 套用變更
        for &(idx, _, after) in &changes {
            heights[idx] = after;
        }

        let count = changes.len();
        if let Some(recorder) = snapshots {
            recorder.push(MacroStepSnapshot::HeightDeltas { changes });
        }

        CommandResult::ok(
            format!("成功調整 {count} 個高程頂點 (delta = {delta})。"),
            count,
        )
    }
}

/// 高程整平指令 (/flatten)。
pub struct FlattenCommand;

impl EditorCommand for FlattenCommand {
    fn name(&self) -> &str {
        "flatten"
    }

    fn aliases(&self) -> &[&str] {
        &["level", "flat"]
    }

    fn category(&self) -> &str {
        "Terrain"
    }

    fn description_zh(&self) -> &str {
        "將指定矩形或圓形範圍內之地形高度整平至指定目標高度值。"
    }

    fn description_en(&self) -> &str {
        "Flatten terrain height in a rectangular or circular region to target height."
    }

    fn usage_syntax(&self) -> &str {
        "/flatten rect <x1> <z1> <x2> <z2> <target> 或 /flatten circle <cx> <cz> <radius> <target>"
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
        let size = context.vertex_size;
        let heights = match &mut context.heights {
            Some(h) if h.len() >= size * size => h,
            _ => return CommandResult::fail("地形高度未就緒。"),
        };

        #[allow(unused_assignments)]
        let mut target_val = 0;
        let mut changes = Vec::new();

        if command.has_flag("rect") || command.has_flag("circle") {
            if let Some(t) = command.try_get_flag_i32("target") {
                target_val = t;
            } else if let Some(t) = command.try_get_positional_i32(0) {
                target_val = t;
            } else {
                return CommandResult::fail("缺少必需的 --target 數值。");
            }

            let target_byte = target_val.clamp(0, 255) as u8;

            if let Some(rect_raw) = command.get_flag("rect") {
                let r = match ParsedCommand::try_parse_rect(rect_raw) {
                    Some(rect) => rect,
                    None => return CommandResult::fail("無法解析 --rect 區域。"),
                };
                collect_rect_flatten(heights, size, r, target_byte, &mut changes);
            } else if let Some(circle_raw) = command.get_flag("circle") {
                let c = match ParsedCommand::try_parse_circle(circle_raw) {
                    Some(circle) => circle,
                    None => return CommandResult::fail("無法解析 --circle 區域。"),
                };
                collect_circle_flatten(heights, size, c, target_byte, &mut changes);
            }
        } else {
            let mode = command.get_positional(0).map(|s| s.to_ascii_lowercase());
            match mode.as_deref() {
                Some("rect") => {
                    let x1 = match command.try_get_positional_i32(1) {
                        Some(v) => v,
                        None => return CommandResult::fail("語法錯誤。用法: /flatten rect <x1> <z1> <x2> <z2> <target>"),
                    };
                    let z1 = match command.try_get_positional_i32(2) {
                        Some(v) => v,
                        None => return CommandResult::fail("語法錯誤。用法: /flatten rect <x1> <z1> <x2> <z2> <target>"),
                    };
                    let x2 = match command.try_get_positional_i32(3) {
                        Some(v) => v,
                        None => return CommandResult::fail("語法錯誤。用法: /flatten rect <x1> <z1> <x2> <z2> <target>"),
                    };
                    let z2 = match command.try_get_positional_i32(4) {
                        Some(v) => v,
                        None => return CommandResult::fail("語法錯誤。用法: /flatten rect <x1> <z1> <x2> <z2> <target>"),
                    };
                    target_val = match command.try_get_positional_i32(5) {
                        Some(v) => v,
                        None => return CommandResult::fail("語法錯誤。用法: /flatten rect <x1> <z1> <x2> <z2> <target>"),
                    };
                    let target_byte = target_val.clamp(0, 255) as u8;
                    let r = MapRegionRect::new(x1, z1, x2, z2);
                    collect_rect_flatten(heights, size, r, target_byte, &mut changes);
                }
                Some("circle") => {
                    let cx = match command.try_get_positional_f32(1) {
                        Some(v) => v,
                        None => return CommandResult::fail("語法錯誤。用法: /flatten circle <cx> <cz> <radius> <target>"),
                    };
                    let cz = match command.try_get_positional_f32(2) {
                        Some(v) => v,
                        None => return CommandResult::fail("語法錯誤。用法: /flatten circle <cx> <cz> <radius> <target>"),
                    };
                    let radius = match command.try_get_positional_f32(3) {
                        Some(v) => v,
                        None => return CommandResult::fail("語法錯誤。用法: /flatten circle <cx> <cz> <radius> <target>"),
                    };
                    target_val = match command.try_get_positional_i32(4) {
                        Some(v) => v,
                        None => return CommandResult::fail("語法錯誤。用法: /flatten circle <cx> <cz> <radius> <target>"),
                    };
                    let target_byte = target_val.clamp(0, 255) as u8;
                    let c = MapRegionCircle::new(cx, cz, radius);
                    collect_circle_flatten(heights, size, c, target_byte, &mut changes);
                }
                _ => return CommandResult::fail("未知的幾何形狀。請指定 rect 或 circle。"),
            }
        }

        if changes.is_empty() {
            return CommandResult::ok("目標區域無符合頂點或高度無變化。", 0);
        }

        for &(idx, _, after) in &changes {
            heights[idx] = after;
        }

        let count = changes.len();
        if let Some(recorder) = snapshots {
            recorder.push(MacroStepSnapshot::HeightDeltas { changes });
        }

        CommandResult::ok(
            format!("成功整平 {count} 個高程頂點至目標高度 {target_val}。"),
            count,
        )
    }
}

/// 筆刷單點模擬指令 (/brush)。
pub struct BrushCommand;

impl EditorCommand for BrushCommand {
    fn name(&self) -> &str {
        "brush"
    }

    fn aliases(&self) -> &[&str] {
        &["paint", "stroke"]
    }

    fn category(&self) -> &str {
        "Terrain"
    }

    fn description_zh(&self) -> &str {
        "在指定世界座標套用單次圓形地形筆刷操作。"
    }

    fn description_en(&self) -> &str {
        "Apply a circular terrain brush stroke at specified coordinates."
    }

    fn usage_syntax(&self) -> &str {
        "/brush <raise|lower|flatten|smooth> <cx> <cz> <radius> [strength]"
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
        let op = match command.get_positional(0) {
            Some(o) => o.to_ascii_lowercase(),
            None => return CommandResult::fail("缺少筆刷操作類型 (raise|lower|flatten|smooth)。"),
        };

        let cx = match command.try_get_positional_f32(1) {
            Some(v) => v,
            None => return CommandResult::fail("缺少圓心 X 座標。"),
        };
        let cz = match command.try_get_positional_f32(2) {
            Some(v) => v,
            None => return CommandResult::fail("缺少圓心 Z 座標。"),
        };
        let radius = match command.try_get_positional_f32(3) {
            Some(v) => v,
            None => return CommandResult::fail("缺少半徑。"),
        };
        let strength = command.try_get_positional_i32(4).unwrap_or(8);

        let delta = match op.as_str() {
            "raise" => strength,
            "lower" => -strength,
            "flatten" => {
                let target = strength.clamp(0, 255) as u8;
                let size = context.vertex_size;
                let heights = match &mut context.heights {
                    Some(h) if h.len() >= size * size => h,
                    _ => return CommandResult::fail("地形高度未就緒。"),
                };
                let mut changes = Vec::new();
                let c = MapRegionCircle::new(cx, cz, radius);
                collect_circle_flatten(heights, size, c, target, &mut changes);
                for &(idx, _, after) in &changes {
                    heights[idx] = after;
                }
                let count = changes.len();
                if let Some(recorder) = snapshots {
                    recorder.push(MacroStepSnapshot::HeightDeltas { changes });
                }
                return CommandResult::ok(format!("成功整平 {count} 頂點至 {target}。"), count);
            }
            _ => strength,
        };

        let size = context.vertex_size;
        let heights = match &mut context.heights {
            Some(h) if h.len() >= size * size => h,
            _ => return CommandResult::fail("地形高度未就緒。"),
        };

        let mut changes = Vec::new();
        let c = MapRegionCircle::new(cx, cz, radius);
        collect_circle(heights, size, c, delta, &mut changes);
        for &(idx, _, after) in &changes {
            heights[idx] = after;
        }
        let count = changes.len();
        if let Some(recorder) = snapshots {
            recorder.push(MacroStepSnapshot::HeightDeltas { changes });
        }

        CommandResult::ok(format!("成功套用筆刷 {op} 於 {count} 個頂點。"), count)
    }
}

fn collect_rect(
    heights: &[u8],
    size: usize,
    r: MapRegionRect,
    delta: i32,
    changes: &mut Vec<(usize, u8, u8)>,
) {
    let min_x = r.min_x.clamp(0, size as i32 - 1) as usize;
    let max_x = r.max_x.clamp(0, size as i32 - 1) as usize;
    let min_z = r.min_z.clamp(0, size as i32 - 1) as usize;
    let max_z = r.max_z.clamp(0, size as i32 - 1) as usize;

    for z in min_z..=max_z {
        for x in min_x..=max_x {
            let idx = z * size + x;
            let cur = heights[idx];
            let next = (cur as i32 + delta).clamp(0, 255) as u8;
            if cur != next {
                changes.push((idx, cur, next));
            }
        }
    }
}

fn collect_circle(
    heights: &[u8],
    size: usize,
    c: MapRegionCircle,
    delta: i32,
    changes: &mut Vec<(usize, u8, u8)>,
) {
    let min_x = (c.center_x - c.radius).floor().clamp(0.0, (size - 1) as f32) as usize;
    let max_x = (c.center_x + c.radius).ceil().clamp(0.0, (size - 1) as f32) as usize;
    let min_z = (c.center_z - c.radius).floor().clamp(0.0, (size - 1) as f32) as usize;
    let max_z = (c.center_z + c.radius).ceil().clamp(0.0, (size - 1) as f32) as usize;

    for z in min_z..=max_z {
        for x in min_x..=max_x {
            if c.contains(x as f32, z as f32) {
                let idx = z * size + x;
                let cur = heights[idx];
                let next = (cur as i32 + delta).clamp(0, 255) as u8;
                if cur != next {
                    changes.push((idx, cur, next));
                }
            }
        }
    }
}

fn collect_rect_flatten(
    heights: &[u8],
    size: usize,
    r: MapRegionRect,
    target: u8,
    changes: &mut Vec<(usize, u8, u8)>,
) {
    let min_x = r.min_x.clamp(0, size as i32 - 1) as usize;
    let max_x = r.max_x.clamp(0, size as i32 - 1) as usize;
    let min_z = r.min_z.clamp(0, size as i32 - 1) as usize;
    let max_z = r.max_z.clamp(0, size as i32 - 1) as usize;

    for z in min_z..=max_z {
        for x in min_x..=max_x {
            let idx = z * size + x;
            let cur = heights[idx];
            if cur != target {
                changes.push((idx, cur, target));
            }
        }
    }
}

fn collect_circle_flatten(
    heights: &[u8],
    size: usize,
    c: MapRegionCircle,
    target: u8,
    changes: &mut Vec<(usize, u8, u8)>,
) {
    let min_x = (c.center_x - c.radius).floor().clamp(0.0, (size - 1) as f32) as usize;
    let max_x = (c.center_x + c.radius).ceil().clamp(0.0, (size - 1) as f32) as usize;
    let min_z = (c.center_z - c.radius).floor().clamp(0.0, (size - 1) as f32) as usize;
    let max_z = (c.center_z + c.radius).ceil().clamp(0.0, (size - 1) as f32) as usize;

    for z in min_z..=max_z {
        for x in min_x..=max_x {
            if c.contains(x as f32, z as f32) {
                let idx = z * size + x;
                let cur = heights[idx];
                if cur != target {
                    changes.push((idx, cur, target));
                }
            }
        }
    }
}
