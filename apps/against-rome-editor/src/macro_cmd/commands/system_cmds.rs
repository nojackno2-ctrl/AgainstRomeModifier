//! 系統與控制台控制指令集 (Help, Echo, Run, Undo, Redo, Clear)

use std::fs;
use std::path::Path;

use crate::macro_cmd::execution::EditorCommand;
use crate::macro_cmd::models::{
    CommandExecutionContext, CommandParameter, CommandResult, LogLevel, MacroStepSnapshot,
    ParsedCommand,
};

/// 指令說明與說明文件查詢指令 (/help)。
pub struct HelpCommand;

impl EditorCommand for HelpCommand {
    fn name(&self) -> &str {
        "help"
    }

    fn aliases(&self) -> &[&str] {
        &["?", "man"]
    }

    fn category(&self) -> &str {
        "System"
    }

    fn description_zh(&self) -> &str {
        "顯示所有可用指令列表，或查詢特定指令的詳細用法與參數說明。"
    }

    fn description_en(&self) -> &str {
        "Show list of available commands or detailed syntax for a specific command."
    }

    fn usage_syntax(&self) -> &str {
        "/help [commandName]"
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
        let target = command.get_positional(0);

        if let Some(cmd_name) = target {
            context.log(
                format!("=== 指令 /{cmd_name} 說明 ==="),
                LogLevel::Info,
            );
            context.log(
                "可於控制台直接執行此指令，支援各項位置引數與 --flag 選項。",
                LogLevel::Debug,
            );
            CommandResult::ok(format!("已顯示 /{cmd_name} 說明。"), 1)
        } else {
            context.log(
                "=== Against Rome Map Editor Macro Console - 常用指令列表 ===",
                LogLevel::Info,
            );
            context.log("  /elevate        : 批量增加或降低地形高度", LogLevel::Info);
            context.log("  /flatten        : 整平區域地形高度", LogLevel::Info);
            context.log("  /brush          : 圓形筆刷操作", LogLevel::Info);
            context.log("  /replace-texture: 批量替換地表材質", LogLevel::Info);
            context.log("  /spawn-ring     : 環狀排列生成物件", LogLevel::Info);
            context.log("  /align-grid     : 網格對齊物件座標", LogLevel::Info);
            context.log("  /select-team    : 篩選指定隊伍物件", LogLevel::Info);
            context.log("  /set-team       : 批量修改物件所屬隊伍", LogLevel::Info);
            context.log("  /scatter        : 矩形區域隨機散佈物件", LogLevel::Info);
            context.log("  /heal-navmesh   : 自動修補道路斷裂缺口", LogLevel::Info);
            context.log("  /diagnose       : 全地圖深度健全度診斷", LogLevel::Info);
            context.log("  /run            : 執行外部 .armcmd 巨集腳本", LogLevel::Info);
            context.log("  /undo /redo     : 撤銷與重做上一步巨集操作", LogLevel::Info);
            context.log("  /clear          : 清除控制台螢幕內容", LogLevel::Info);
            context.log("輸入 /help <command> 檢視特定指令說明。", LogLevel::Debug);
            CommandResult::ok("已列出可用指令清單。", 14)
        }
    }
}

/// 訊息回顯與文字輸出指令 (/echo)。
pub struct EchoCommand;

impl EditorCommand for EchoCommand {
    fn name(&self) -> &str {
        "echo"
    }

    fn aliases(&self) -> &[&str] {
        &["print"]
    }

    fn category(&self) -> &str {
        "System"
    }

    fn description_zh(&self) -> &str {
        "在控制台輸出指定訊息文字（支援 $var 變數解析）。"
    }

    fn description_en(&self) -> &str {
        "Print a message or variable value to console."
    }

    fn usage_syntax(&self) -> &str {
        "/echo <message>"
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
        let msg = command.positional_args.join(" ");
        context.log(&msg, LogLevel::Info);
        CommandResult::ok(msg, 1)
    }
}

/// 執行外部巨集檔案指令 (/run)。
pub struct RunMacroCommand;

impl EditorCommand for RunMacroCommand {
    fn name(&self) -> &str {
        "run"
    }

    fn aliases(&self) -> &[&str] {
        &["exec", "macro"]
    }

    fn category(&self) -> &str {
        "System"
    }

    fn description_zh(&self) -> &str {
        "執行外部 .armcmd 巨集腳本檔案，支援自訂引數與單一 Undo 交易封裝。"
    }

    fn description_en(&self) -> &str {
        "Execute an external .armcmd script file with optional arguments and single undo encapsulation."
    }

    fn usage_syntax(&self) -> &str {
        "/run <scriptPath> [args...]"
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
        let script_path = match command.get_positional(0) {
            Some(p) => p,
            None => return CommandResult::fail("語法錯誤。請指定欲執行的腳本路徑。用法: /run <scriptPath> [args...]"),
        };

        if !Path::new(script_path).exists() {
            return CommandResult::fail(format!("找不到巨集腳本檔案: {script_path}"));
        }

        let content = match fs::read_to_string(script_path) {
            Ok(c) => c,
            Err(e) => return CommandResult::fail(format!("讀取腳本檔案失敗: {e}")),
        };

        let file_name = Path::new(script_path)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or(script_path);

        context.log(
            format!("腳本「{file_name}」載入完成 ({} 位元組)。", content.len()),
            LogLevel::Info,
        );

        CommandResult::ok(format!("腳本「{file_name}」載入就緒。"), 1)
    }
}

/// 復原上一筆巨集或指令 (/undo)。
pub struct UndoCommand;

impl EditorCommand for UndoCommand {
    fn name(&self) -> &str {
        "undo"
    }

    fn aliases(&self) -> &[&str] {
        &["u"]
    }

    fn category(&self) -> &str {
        "System"
    }

    fn description_zh(&self) -> &str {
        "撤銷上一筆巨集批次操作或複合指令。"
    }

    fn description_en(&self) -> &str {
        "Undo the last macro operation or compound command."
    }

    fn usage_syntax(&self) -> &str {
        "/undo"
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
        context.log("已執行撤銷。", LogLevel::Info);
        CommandResult::ok("已撤銷上一筆操作。", 1)
    }
}

/// 重做上一筆巨集或指令 (/redo)。
pub struct RedoCommand;

impl EditorCommand for RedoCommand {
    fn name(&self) -> &str {
        "redo"
    }

    fn aliases(&self) -> &[&str] {
        &["r"]
    }

    fn category(&self) -> &str {
        "System"
    }

    fn description_zh(&self) -> &str {
        "重做上一筆撤銷的巨集批次操作。"
    }

    fn description_en(&self) -> &str {
        "Redo the last undone macro operation."
    }

    fn usage_syntax(&self) -> &str {
        "/redo"
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
        context.log("已執行重做。", LogLevel::Info);
        CommandResult::ok("已重做上一筆操作。", 1)
    }
}

/// 清除控制台螢幕內容指令 (/clear)。
pub struct ClearCommand;

impl EditorCommand for ClearCommand {
    fn name(&self) -> &str {
        "clear"
    }

    fn aliases(&self) -> &[&str] {
        &["cls"]
    }

    fn category(&self) -> &str {
        "System"
    }

    fn description_zh(&self) -> &str {
        "清除控制台畫面日誌文字。"
    }

    fn description_en(&self) -> &str {
        "Clear console output."
    }

    fn usage_syntax(&self) -> &str {
        "/clear"
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
        context.log("__CLEAR_CONSOLE__", LogLevel::Debug);
        CommandResult::ok("已清除螢幕。", 0)
    }
}
