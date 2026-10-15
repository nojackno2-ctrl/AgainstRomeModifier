//! 地表材質替換指令 (ReplaceTexture)

use crate::macro_cmd::execution::EditorCommand;
use crate::macro_cmd::models::{
    CommandExecutionContext, CommandParameter, CommandResult, MacroStepSnapshot, ParsedCommand,
};

/// 批量替換地表材質指令 (/replace-texture)。
pub struct ReplaceTextureCommand;

impl EditorCommand for ReplaceTextureCommand {
    fn name(&self) -> &str {
        "replace-texture"
    }

    fn aliases(&self) -> &[&str] {
        &["rep-tex", "swap-texture"]
    }

    fn category(&self) -> &str {
        "Terrain"
    }

    fn description_zh(&self) -> &str {
        "在指定範圍或全地圖將指定材質批量替換為目標材質。"
    }

    fn description_en(&self) -> &str {
        "Replace occurrences of a texture with another texture within a region or whole map."
    }

    fn usage_syntax(&self) -> &str {
        "/replace-texture <fromTexture> <toTexture> [--rect <rect>]"
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
        let from_tex = match command.get_positional(0) {
            Some(t) => t,
            None => return CommandResult::fail("語法錯誤。請指定來源材質名稱。用法: /replace-texture <from> <to> [--rect <rect>]"),
        };

        let to_tex = match command.get_positional(1) {
            Some(t) => t,
            None => return CommandResult::fail("語法錯誤。請指定目標材質名稱。"),
        };

        let dim = context.map_tile_dimension;
        let textures = match &mut context.textures {
            Some(t) if t.len() >= dim * dim => t,
            _ => return CommandResult::fail("地表材質未就緒。"),
        };

        let rect = command.get_flag("rect").and_then(ParsedCommand::try_parse_rect);

        let mut changes = Vec::new();

        if let Some(r) = rect {
            let min_x = r.min_x.clamp(0, dim as i32 - 1) as usize;
            let max_x = r.max_x.clamp(0, dim as i32 - 1) as usize;
            let min_z = r.min_z.clamp(0, dim as i32 - 1) as usize;
            let max_z = r.max_z.clamp(0, dim as i32 - 1) as usize;

            for z in min_z..=max_z {
                for x in min_x..=max_x {
                    let idx = z * dim + x;
                    if textures[idx].eq_ignore_ascii_case(from_tex) {
                        changes.push((idx, textures[idx].clone(), to_tex.to_string()));
                        textures[idx] = to_tex.to_string();
                    }
                }
            }
        } else {
            for (idx, tex) in textures.iter_mut().enumerate() {
                if tex.eq_ignore_ascii_case(from_tex) {
                    changes.push((idx, tex.clone(), to_tex.to_string()));
                    *tex = to_tex.to_string();
                }
            }
        }

        let count = changes.len();
        if let Some(recorder) = snapshots {
            recorder.push(MacroStepSnapshot::TextureDeltas { changes });
        }

        CommandResult::ok(
            format!("成功將 {count} 個地圖格材質「{from_tex}」替換為「{to_tex}」。"),
            count,
        )
    }
}
