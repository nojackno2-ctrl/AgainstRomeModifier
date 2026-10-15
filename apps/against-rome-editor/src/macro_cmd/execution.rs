//! 巨集腳本與指令執行引擎 (Execution Engine & Registry)

use std::collections::HashMap;
use std::time::Instant;

use crate::macro_cmd::models::{
    CommandExecutionContext, CommandParameter, CommandResult, LogLevel,
    MacroExecutionReport, MacroStepSnapshot, ParsedCommand,
};
use crate::macro_cmd::parser;

/// 地圖編輯器命令抽象介面。
pub trait EditorCommand: Send + Sync {
    fn name(&self) -> &str;
    fn aliases(&self) -> &[&str];
    fn category(&self) -> &str;
    fn description_zh(&self) -> &str;
    fn description_en(&self) -> &str;
    fn usage_syntax(&self) -> &str;
    fn parameters(&self) -> &[CommandParameter];
    fn execute(
        &self,
        command: &ParsedCommand,
        context: &mut CommandExecutionContext,
        snapshots: &mut Option<&mut Vec<MacroStepSnapshot>>,
    ) -> CommandResult;
}

/// 交易型複合巨集指令：將腳本批次執行的所有個別子操作打包為單一 Undo/Redo 單元。
#[derive(Clone, Debug)]
pub struct CompoundMacroCommand {
    pub macro_name: String,
    pub description: String,
    pub snapshots: Vec<MacroStepSnapshot>,
}

impl CompoundMacroCommand {
    pub fn new(macro_name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            macro_name: macro_name.into(),
            description: description.into(),
            snapshots: Vec::new(),
        }
    }

    pub fn step_count(&self) -> usize {
        self.snapshots.len()
    }

    pub fn register_snapshot(&mut self, snapshot: MacroStepSnapshot) {
        self.snapshots.push(snapshot);
    }

    /// 單次無損撤銷（倒序套用所有子步驟之 Undo）。
    pub fn undo(&self, context: &mut CommandExecutionContext) {
        for snapshot in self.snapshots.iter().rev() {
            apply_undo_snapshot(snapshot, context);
        }
    }

    /// 單次無損重做（正序套用所有子步驟之 Redo）。
    pub fn redo(&self, context: &mut CommandExecutionContext) {
        for snapshot in &self.snapshots {
            apply_redo_snapshot(snapshot, context);
        }
    }

    /// 交易異常時的強制回滾（撤銷已執行的所有步驟）。
    pub fn rollback(&self, context: &mut CommandExecutionContext) {
        self.undo(context);
    }
}

fn apply_undo_snapshot(snapshot: &MacroStepSnapshot, context: &mut CommandExecutionContext) {
    match snapshot {
        MacroStepSnapshot::HeightDeltas { changes } => {
            if let Some(heights) = &mut context.heights {
                for &(idx, before, _) in changes {
                    if idx < heights.len() {
                        heights[idx] = before;
                    }
                }
            }
        }
        MacroStepSnapshot::TextureDeltas { changes } => {
            if let Some(textures) = &mut context.textures {
                for (idx, before, _) in changes {
                    if *idx < textures.len() {
                        textures[*idx] = before.clone();
                    }
                }
            }
        }
        MacroStepSnapshot::PlacementAdd { added_indices } => {
            // 倒序刪除新增的物件
            let mut indices = added_indices.clone();
            indices.sort_unstable();
            for &idx in indices.iter().rev() {
                if idx < context.placed_objects.len() {
                    context.placed_objects.remove(idx);
                }
            }
        }
        MacroStepSnapshot::PlacementModify { changes } => {
            for (idx, before, _) in changes {
                if *idx < context.placed_objects.len() {
                    context.placed_objects[*idx] = before.clone();
                }
            }
        }
        MacroStepSnapshot::PlacementRemove { removed } => {
            for (idx, obj) in removed {
                if *idx <= context.placed_objects.len() {
                    context.placed_objects.insert(*idx, obj.clone());
                } else {
                    context.placed_objects.push(obj.clone());
                }
            }
        }
        MacroStepSnapshot::Custom { .. } => {}
    }
}

fn apply_redo_snapshot(snapshot: &MacroStepSnapshot, context: &mut CommandExecutionContext) {
    match snapshot {
        MacroStepSnapshot::HeightDeltas { changes } => {
            if let Some(heights) = &mut context.heights {
                for &(idx, _, after) in changes {
                    if idx < heights.len() {
                        heights[idx] = after;
                    }
                }
            }
        }
        MacroStepSnapshot::TextureDeltas { changes } => {
            if let Some(textures) = &mut context.textures {
                for (idx, _, after) in changes {
                    if *idx < textures.len() {
                        textures[*idx] = after.clone();
                    }
                }
            }
        }
        MacroStepSnapshot::PlacementAdd { .. } => {
            // Placement add redo handled by caller or state
        }
        MacroStepSnapshot::PlacementModify { changes } => {
            for (idx, _, after) in changes {
                if *idx < context.placed_objects.len() {
                    context.placed_objects[*idx] = after.clone();
                }
            }
        }
        MacroStepSnapshot::PlacementRemove { removed } => {
            let mut indices: Vec<_> = removed.iter().map(|(idx, _)| *idx).collect();
            indices.sort_unstable();
            for &idx in indices.iter().rev() {
                if idx < context.placed_objects.len() {
                    context.placed_objects.remove(idx);
                }
            }
        }
        MacroStepSnapshot::Custom { .. } => {}
    }
}

/// 指令登錄表：管理所有註冊的 DSL 編輯指令。
pub struct CommandRegistry {
    index_map: HashMap<String, usize>,
    unique_commands: Vec<Box<dyn EditorCommand>>,
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            index_map: HashMap::new(),
            unique_commands: Vec::new(),
        }
    }

    pub fn count(&self) -> usize {
        self.unique_commands.len()
    }

    pub fn register(&mut self, command: Box<dyn EditorCommand>) {
        let idx = self.unique_commands.len();
        self.index_map
            .insert(command.name().to_ascii_lowercase(), idx);
        for alias in command.aliases() {
            self.index_map.insert(alias.to_ascii_lowercase(), idx);
        }
        self.unique_commands.push(command);
    }

    pub fn try_get_command(&self, name_or_alias: &str) -> Option<&Box<dyn EditorCommand>> {
        let clean = if name_or_alias.starts_with('/') {
            &name_or_alias[1..]
        } else {
            name_or_alias
        };
        let idx = self.index_map.get(&clean.to_ascii_lowercase())?;
        self.unique_commands.get(*idx)
    }

    pub fn get_all_commands(&self) -> &[Box<dyn EditorCommand>] {
        &self.unique_commands
    }
}

/// 批次巨集腳本執行器：負責執行 .armcmd 腳本、處理變數替換、多命令交易打包與自動 Rollback。
pub struct MacroScriptRunner {
    pub registry: CommandRegistry,
    undo_stack: Vec<CompoundMacroCommand>,
    redo_stack: Vec<CompoundMacroCommand>,
}

impl MacroScriptRunner {
    pub fn new(registry: CommandRegistry) -> Self {
        Self {
            registry,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self, context: &mut CommandExecutionContext) -> Option<CompoundMacroCommand> {
        let compound = self.undo_stack.pop()?;
        compound.undo(context);
        self.redo_stack.push(compound.clone());
        Some(compound)
    }

    pub fn redo(&mut self, context: &mut CommandExecutionContext) -> Option<CompoundMacroCommand> {
        let compound = self.redo_stack.pop()?;
        compound.redo(context);
        self.undo_stack.push(compound.clone());
        Some(compound)
    }

    /// 執行單行指令文字。若 record_single_action 為 true，該單行操作會獨立封裝入 Undo 歷史。
    pub fn execute_line(
        &mut self,
        line: &str,
        context: &mut CommandExecutionContext,
        record_single_action: bool,
    ) -> CommandResult {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            return CommandResult::ok("忽略空白或註解行。", 0);
        }

        // 1. 處理變數賦值語法: $varName = value
        if trimmed.starts_with('$') && trimmed.contains('=') {
            if let Some(eq_idx) = trimmed.find('=') {
                let var_name = trimmed[1..eq_idx].trim();
                let mut var_val = trimmed[(eq_idx + 1)..].trim();
                if var_val.starts_with('"') && var_val.ends_with('"') && var_val.len() >= 2 {
                    var_val = &var_val[1..var_val.len() - 1];
                }
                context
                    .variables
                    .insert(var_name.to_string(), var_val.to_string());
                context.log(
                    format!("[VAR] 定義變數 ${var_name} = {var_val}"),
                    LogLevel::Debug,
                );
                return CommandResult::ok(format!("已設定變數 ${var_name} = {var_val}"), 1);
            }
        }

        // 2. 解析指令
        let parsed = match parser::parse(trimmed, Some(&context.variables)) {
            Some(p) => p,
            None => return CommandResult::ok("無可執行指令。", 0),
        };

        let cmd = match self.registry.try_get_command(&parsed.command_name) {
            Some(c) => c,
            None => {
                let err = format!(
                    "未知指令: /{}。輸入 /help 查詢可用指令列表。",
                    parsed.command_name
                );
                context.log(&err, LogLevel::Error);
                return CommandResult::fail(err);
            }
        };

        // 3. 封裝與執行
        let mut single_compound = if record_single_action {
            Some(CompoundMacroCommand::new(
                &parsed.command_name,
                &parsed.raw_text,
            ))
        } else {
            None
        };

        let mut step_recorder = single_compound
            .as_mut()
            .map(|c| &mut c.snapshots);

        let result = cmd.execute(&parsed, context, &mut step_recorder);

        if result.success {
            if let Some(compound) = single_compound {
                if compound.step_count() > 0 {
                    self.undo_stack.push(compound);
                    self.redo_stack.clear();
                }
            }
            if parsed.command_name != "clear" && parsed.command_name != "cls" {
                context.log(
                    format!("[OK] /{}: {}", parsed.command_name, result.message),
                    LogLevel::Success,
                );
            }
        } else {
            if let Some(compound) = single_compound {
                if compound.step_count() > 0 {
                    compound.rollback(context);
                }
            }
            context.log(
                format!("[FAIL] /{}: {}", parsed.command_name, result.message),
                LogLevel::Error,
            );
        }

        result
    }

    /// 執行 .armcmd 巨集腳本字串。支援單次交易打包與原子性回滾。
    pub fn execute_script(
        &mut self,
        script_content: &str,
        context: &mut CommandExecutionContext,
        script_path: Option<&str>,
        script_args: Option<&[String]>,
        transactional: bool,
    ) -> MacroExecutionReport {
        let start_time = Instant::now();
        let mut macro_name = script_path
            .and_then(|p| std::path::Path::new(p).file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("Macro")
            .to_string();

        let mut compound = CompoundMacroCommand::new(&macro_name, format!("執行腳本 {macro_name}"));

        // 參數注入: $1, $2, ...
        if let Some(args) = script_args {
            for (i, arg) in args.iter().enumerate() {
                context
                    .variables
                    .insert((i + 1).to_string(), arg.clone());
            }
            context.variables.insert("0".to_string(), macro_name.clone());
        }

        let lines: Vec<&str> = script_content.lines().collect();
        let mut total_commands = 0;
        let mut succeeded_commands = 0;
        let mut failed_commands = 0;
        let mut affected_elements = 0;
        let mut was_rolled_back = false;
        let mut failure_error = None;

        context.log(
            format!("開始執行巨集「{macro_name}」，共 {} 行腳本...", lines.len()),
            LogLevel::Info,
        );

        for (line_index, raw_line) in lines.iter().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }

            // 註解與中繼指令讀取
            if line.starts_with('#') || line.starts_with("//") {
                if let Some(name_part) = line.strip_prefix("# @name") {
                    macro_name = name_part.trim().to_string();
                }
                continue;
            }

            total_commands += 1;

            // 執行行命令
            let mut step_recorder = if transactional {
                Some(&mut compound.snapshots)
            } else {
                None
            };

            // 變數賦值或一般指令
            let res = if line.starts_with('$') && line.contains('=') {
                if let Some(eq_idx) = line.find('=') {
                    let var_name = line[1..eq_idx].trim();
                    let mut var_val = line[(eq_idx + 1)..].trim();
                    if var_val.starts_with('"') && var_val.ends_with('"') && var_val.len() >= 2 {
                        var_val = &var_val[1..var_val.len() - 1];
                    }
                    context
                        .variables
                        .insert(var_name.to_string(), var_val.to_string());
                    context.log(
                        format!("[VAR] 定義變數 ${var_name} = {var_val}"),
                        LogLevel::Debug,
                    );
                    CommandResult::ok(format!("已設定變數 ${var_name} = {var_val}"), 1)
                } else {
                    CommandResult::fail("無效的變數賦值語法。")
                }
            } else {
                match parser::parse(line, Some(&context.variables)) {
                    Some(parsed) => {
                        match self.registry.try_get_command(&parsed.command_name) {
                            Some(cmd) => {
                                let r = cmd.execute(&parsed, context, &mut step_recorder);
                                if r.success {
                                    context.log(
                                        format!("[OK] /{}: {}", parsed.command_name, r.message),
                                        LogLevel::Success,
                                    );
                                } else {
                                    context.log(
                                        format!("[FAIL] /{}: {}", parsed.command_name, r.message),
                                        LogLevel::Error,
                                    );
                                }
                                r
                            }
                            None => {
                                let err = format!("未知指令: /{}", parsed.command_name);
                                context.log(&err, LogLevel::Error);
                                CommandResult::fail(err)
                            }
                        }
                    }
                    None => CommandResult::ok("無可執行指令。", 0),
                }
            };

            if res.success {
                succeeded_commands += 1;
                affected_elements += res.affected_count;
            } else {
                failed_commands += 1;
                failure_error = Some(format!("行 {}: {}", line_index + 1, res.message));

                if transactional {
                    context.log(
                        format!(
                            "巨集於第 {} 行中斷失敗: {}。正在執行原子性回滾...",
                            line_index + 1,
                            res.message
                        ),
                        LogLevel::Warning,
                    );
                    compound.rollback(context);
                    was_rolled_back = true;
                    break;
                }
            }
        }

        if !was_rolled_back && compound.step_count() > 0 {
            self.undo_stack.push(compound);
            self.redo_stack.clear();
        }

        let elapsed_ms = start_time.elapsed().as_millis() as u64;
        let overall_success = failed_commands == 0 && !was_rolled_back;

        let summary = if overall_success {
            format!(
                "巨集「{macro_name}」執行成功！總步驟: {succeeded_commands}/{total_commands}，影響項目: {affected_elements}，耗時: {elapsed_ms} ms。"
            )
        } else {
            format!(
                "巨集「{macro_name}」執行失敗！失敗步驟: {failed_commands}，已回滾: {}，原因: {}",
                if was_rolled_back { "是" } else { "否" },
                failure_error.as_deref().unwrap_or("未知錯誤")
            )
        };

        context.log(
            summary,
            if overall_success {
                LogLevel::Success
            } else {
                LogLevel::Error
            },
        );

        MacroExecutionReport {
            script_path: script_path.map(|s| s.to_string()),
            success: overall_success,
            total_commands,
            succeeded_commands,
            failed_commands,
            elapsed_milliseconds: elapsed_ms,
            affected_elements,
            was_rolled_back,
            error_message: failure_error,
            logs: context.logs.clone(),
        }
    }
}
