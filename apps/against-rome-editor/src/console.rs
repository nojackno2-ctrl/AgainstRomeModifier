//! 即時互動控制台控制項 (Editor Console UI)
//!
//! 提供語法著色輸出、Tab 自動補全循環、歷史紀錄回溯、非同步/同步巨集指令執行與單鍵 Undo/Redo。

use eframe::egui::{self, Color32, Key, ScrollArea, TextEdit, Vec2};

use crate::macro_cmd::commands;
use crate::macro_cmd::execution::{CommandRegistry, MacroScriptRunner};
use crate::macro_cmd::models::{CommandExecutionContext, CommandResult, LogLevel};
use crate::macro_cmd::parser;

/// 紀錄單筆輸出日誌條目
#[derive(Clone, Debug, PartialEq)]
pub struct ConsoleLogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
}

pub struct ConsoleUi {
    pub open: bool,
    pub input_text: String,
    pub history: Vec<String>,
    pub history_index: usize,
    pub saved_current_input: String,
    pub output_logs: Vec<ConsoleLogEntry>,
    pub runner: MacroScriptRunner,
    pub context: CommandExecutionContext,
    pub status_text: String,

    // Tab 自動補全狀態
    current_completions: Option<Vec<String>>,
    completion_cycle_index: usize,
    completion_prefix: String,

    // 自動捲動至底標記
    scroll_to_bottom: bool,
}

impl Default for ConsoleUi {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsoleUi {
    pub fn new() -> Self {
        let mut registry = CommandRegistry::new();
        commands::register_default_commands(&mut registry);
        let runner = MacroScriptRunner::new(registry);
        let context = CommandExecutionContext::default();

        let mut ui = Self {
            open: false,
            input_text: String::new(),
            history: Vec::new(),
            history_index: 0,
            saved_current_input: String::new(),
            output_logs: Vec::new(),
            runner,
            context,
            status_text: "就緒 (Ready)".to_string(),
            current_completions: None,
            completion_cycle_index: 0,
            completion_prefix: String::new(),
            scroll_to_bottom: true,
        };

        ui.append_log(
            "=== Against Rome Map Editor 即時控制台已就緒 ===",
            LogLevel::Info,
        );
        ui.append_log(
            "輸入 /help 查詢所有指令，或使用 Tab 進行自動補全與候選切換。",
            LogLevel::Debug,
        );
        ui
    }

    pub fn append_log(&mut self, message: &str, level: LogLevel) {
        if message == "__CLEAR_CONSOLE__" {
            self.clear_output();
            return;
        }

        let timestamp = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => {
                let s = d.as_secs();
                format!("{:02}:{:02}:{:02}", (s / 3600) % 24, (s / 60) % 60, s % 60)
            }
            Err(_) => "00:00:00".to_string(),
        };
        self.output_logs.push(ConsoleLogEntry {
            timestamp,
            level,
            message: message.to_string(),
        });
        self.scroll_to_bottom = true;
    }

    pub fn clear_output(&mut self) {
        self.output_logs.clear();
    }

    pub fn execute_input(&mut self, text: &str) -> CommandResult {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return CommandResult::ok("無輸入指令。", 0);
        }

        self.history.push(trimmed.to_string());
        self.history_index = self.history.len();
        self.saved_current_input.clear();
        self.input_text.clear();
        self.current_completions = None;

        self.append_log(&format!("> {trimmed}"), LogLevel::Command);

        let result = self.execute_console_line(trimmed);

        // 將執行過程收集到的日誌注入至輸出視窗
        let logs: Vec<_> = self.context.logs.drain(..).collect();
        for (msg, lvl) in logs {
            self.append_log(&msg, lvl);
        }

        self.status_text = "就緒 (Ready)".to_string();

        result
    }

    /// System commands need the console's live runner, not the registry's stubs.
    fn execute_console_line(&mut self, text: &str) -> CommandResult {
        let mut names: Vec<_> = self.context.variables.keys().cloned().collect();
        let line = Self::variable_spelling(text, &mut names);
        if line.starts_with('$') && line.contains('=') {
            return self.runner.execute_line(&line, &mut self.context, true);
        }
        let Some(parsed) = parser::parse(&line, Some(&self.context.variables)) else {
            return self.runner.execute_line(&line, &mut self.context, true);
        };
        let name = self
            .runner
            .registry
            .try_get_command(&parsed.command_name)
            .map(|cmd| cmd.name().to_string());
        let result = match name.as_deref() {
            Some("help") => self.show_command_help(&parsed),
            Some("clear") => {
                self.context.log("__CLEAR_CONSOLE__", LogLevel::Debug);
                CommandResult::ok("已清除螢幕。", 0)
            }
            Some("echo") => {
                let message = parsed.positional_args.join(" ");
                self.context.log(&message, LogLevel::Info);
                CommandResult::ok(message, 0)
            }
            Some("undo") => match self.runner.undo(&mut self.context) {
                Some(command) => CommandResult::ok(
                    format!(
                        "已撤銷巨集「{}」({} 個子步驟)。",
                        command.macro_name,
                        command.step_count()
                    ),
                    0,
                ),
                None => CommandResult::fail("無可撤銷之巨集歷史。"),
            },
            Some("redo") => match self.runner.redo(&mut self.context) {
                Some(command) => CommandResult::ok(
                    format!(
                        "已重做巨集「{}」({} 個子步驟)。",
                        command.macro_name,
                        command.step_count()
                    ),
                    0,
                ),
                None => CommandResult::fail("無可重做之巨集歷史。"),
            },
            Some("run") => self.run_macro(&parsed),
            _ => return self.runner.execute_line(&line, &mut self.context, true),
        };
        self.context.log(
            format!(
                "[{}] /{}: {}",
                if result.success { "OK" } else { "FAIL" },
                parsed.command_name,
                result.message
            ),
            if result.success {
                LogLevel::Success
            } else {
                LogLevel::Error
            },
        );
        result
    }

    fn show_command_help(
        &mut self,
        command: &crate::macro_cmd::models::ParsedCommand,
    ) -> CommandResult {
        let target = command
            .get_positional(0)
            .filter(|name| !name.trim().is_empty());
        let Some(target) = target else {
            let mut commands: Vec<_> = self.runner.registry.get_all_commands().iter().collect();
            commands.sort_by(|a, b| a.category().cmp(b.category()).then(a.name().cmp(b.name())));
            self.context.log(
                "=== Against Rome Map Editor Macro Console - 指令列表 ===",
                LogLevel::Info,
            );
            let mut category = "";
            for command in &commands {
                if command.category() != category {
                    category = command.category();
                    self.context.log(format!("\n[{category}]"), LogLevel::Debug);
                }
                self.context.log(
                    format!("  /{:<16} : {}", command.name(), command.description_zh()),
                    LogLevel::Info,
                );
            }
            self.context.log(
                "\n輸入 /help <command> 檢視特定指令之詳細語法與參數說明。",
                LogLevel::Info,
            );
            return CommandResult::ok(
                format!("已列出 {} 個可用指令。", commands.len()),
                commands.len(),
            );
        };

        let Some(found) = self.runner.registry.try_get_command(target) else {
            return CommandResult::fail(format!("未找到指令「{target}」。"));
        };
        self.context.log(
            format!("\n指令: /{} (分類: {})", found.name(), found.category()),
            LogLevel::Info,
        );
        if !found.aliases().is_empty() {
            let aliases = found
                .aliases()
                .iter()
                .map(|alias| format!("/{alias}"))
                .collect::<Vec<_>>()
                .join(", ");
            self.context
                .log(format!("別名: {aliases}"), LogLevel::Debug);
        }
        self.context.log(
            format!(
                "說明: {} ({})",
                found.description_zh(),
                found.description_en()
            ),
            LogLevel::Info,
        );
        self.context
            .log(format!("語法: {}", found.usage_syntax()), LogLevel::Success);
        if !found.parameters().is_empty() {
            self.context.log("參數規格:", LogLevel::Info);
            for parameter in found.parameters() {
                let required = if parameter.is_required {
                    "[必填]"
                } else {
                    "[可選]"
                };
                let default = parameter
                    .default_value
                    .as_ref()
                    .map(|value| format!(" (預設: {value})"))
                    .unwrap_or_default();
                self.context.log(
                    format!(
                        "  --{:<12} ({:?}) {required}: {}{default}",
                        parameter.name, parameter.param_type, parameter.desc_zh
                    ),
                    LogLevel::Info,
                );
            }
        }
        CommandResult::ok(format!("已顯示 /{} 之詳細說明。", found.name()), 0)
    }

    fn run_macro(&mut self, command: &crate::macro_cmd::models::ParsedCommand) -> CommandResult {
        let Some(path) = command.get_positional(0) else {
            return CommandResult::fail(
                "語法錯誤。請指定欲執行的腳本路徑。用法: /run <scriptPath> [args...]",
            );
        };
        if !std::path::Path::new(path).is_file() {
            return CommandResult::fail(format!("找不到巨集腳本檔案: {path}"));
        }
        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) => return CommandResult::fail(format!("讀取腳本檔案失敗: {error}")),
        };
        let mut names: Vec<_> = self.context.variables.keys().cloned().collect();
        let script = content
            .trim_start_matches('\u{feff}')
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .split('\n')
            .map(|line| Self::variable_spelling(line.trim(), &mut names))
            .collect::<Vec<_>>()
            .join("\n");
        let report = self.runner.execute_script(
            &script,
            &mut self.context,
            Some(path),
            Some(&command.positional_args[1..]),
            true,
        );
        let file_name = std::path::Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(path);
        if report.success {
            CommandResult::ok(
                format!(
                    "腳本「{file_name}」執行成功 (影響 {} 項目)。",
                    report.affected_elements
                ),
                report.affected_elements,
            )
        } else {
            CommandResult::fail(format!(
                "腳本「{file_name}」執行失敗: {}",
                report.error_message.as_deref().unwrap_or("未知錯誤")
            ))
        }
    }

    /// C# Variables uses OrdinalIgnoreCase. Reuse the stored spelling while
    /// leaving values untouched (assignments deliberately do not expand values).
    fn variable_spelling(text: &str, names: &mut Vec<String>) -> String {
        if text.starts_with('#') || text.starts_with("//") {
            return text.to_string();
        }
        if text.starts_with('$') {
            if let Some(eq) = text.find('=') {
                let name = text[1..eq].trim();
                if let Some(existing) = names.iter().find(|key| key.eq_ignore_ascii_case(name)) {
                    return format!("${existing} {}", &text[eq..]);
                }
                names.push(name.to_string());
                return text.to_string();
            }
        }
        let mut result = String::new();
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            result.push(ch);
            if ch == '$' {
                let mut name = String::new();
                while chars
                    .peek()
                    .is_some_and(|ch| ch.is_alphanumeric() || *ch == '_')
                {
                    name.push(chars.next().unwrap());
                }
                result.push_str(
                    names
                        .iter()
                        .find(|key| key.eq_ignore_ascii_case(&name))
                        .map(String::as_str)
                        .unwrap_or(&name),
                );
            }
        }
        result
    }

    pub fn handle_tab_completion(&mut self) {
        let current = self.input_text.clone();
        if self.current_completions.is_none() {
            let completions = parser::get_completions(
                &current,
                current.len(),
                Some(&self.context),
                self.runner.registry.get_all_commands(),
            );

            if completions.is_empty() {
                return;
            }

            self.current_completions = Some(completions);
            self.completion_cycle_index = 0;
            self.completion_prefix = current.clone();
        }

        if let Some(completions) = &self.current_completions {
            if completions.is_empty() {
                return;
            }

            let candidate = &completions[self.completion_cycle_index % completions.len()];
            self.completion_cycle_index += 1;

            if let Some(last_space) = self.completion_prefix.rfind(' ') {
                self.input_text =
                    format!("{} {}", &self.completion_prefix[..last_space], candidate);
            } else {
                self.input_text = candidate.clone();
            }
        }
    }

    pub fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }

        if self.history_index == self.history.len() {
            self.saved_current_input = self.input_text.clone();
        }

        if self.history_index > 0 {
            self.history_index -= 1;
            self.input_text = self.history[self.history_index].clone();
        }
    }

    pub fn history_down(&mut self) {
        if self.history.is_empty() || self.history_index >= self.history.len() {
            return;
        }

        self.history_index += 1;
        if self.history_index == self.history.len() {
            self.input_text = self.saved_current_input.clone();
        } else {
            self.input_text = self.history[self.history_index].clone();
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }

        let mut open = self.open;
        egui::Window::new("Interactive Console (即時指令控制台)")
            .open(&mut open)
            .default_size(Vec2::new(700.0, 420.0))
            .show(ctx, |ui| {
                // 1. 頂部工具列
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(140, 220, 140), &self.status_text);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("說明 (/help)").clicked() {
                            self.execute_input("/help");
                        }
                        if ui.button("清空 (Clear)").clicked() {
                            self.clear_output();
                        }
                        if ui
                            .add_enabled(self.runner.can_undo(), egui::Button::new("Undo"))
                            .clicked()
                        {
                            self.execute_input("/undo");
                        }
                        if ui
                            .add_enabled(self.runner.can_redo(), egui::Button::new("Redo"))
                            .clicked()
                        {
                            self.execute_input("/redo");
                        }
                    });
                });

                ui.separator();

                // 2. 日誌輸出檢視區
                let available_height = ui.available_height() - 36.0;
                ScrollArea::vertical()
                    .max_height(available_height.max(100.0))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for entry in &self.output_logs {
                            let text_color = match entry.level {
                                LogLevel::Command => Color32::from_rgb(80, 220, 255),
                                LogLevel::Success => Color32::from_rgb(120, 240, 120),
                                LogLevel::Warning => Color32::from_rgb(255, 215, 60),
                                LogLevel::Error => Color32::from_rgb(255, 110, 110),
                                LogLevel::Debug => Color32::from_rgb(140, 140, 140),
                                LogLevel::Info => Color32::from_rgb(225, 225, 225),
                            };

                            ui.horizontal(|ui| {
                                ui.colored_label(
                                    Color32::from_rgb(100, 100, 100),
                                    format!("[{}]", entry.timestamp),
                                );
                                ui.colored_label(text_color, &entry.message);
                            });
                        }

                        if self.scroll_to_bottom {
                            ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                            self.scroll_to_bottom = false;
                        }
                    });

                ui.separator();

                // 3. 底部指令輸入列
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(80, 200, 255), ">");

                    let input_edit = TextEdit::singleline(&mut self.input_text)
                        .desired_width(ui.available_width() - 60.0)
                        .font(egui::TextStyle::Monospace);

                    let response = ui.add(input_edit);

                    // 按鍵互動
                    if response.has_focus() {
                        if ctx.input(|i| i.key_pressed(Key::Enter)) {
                            let text = self.input_text.clone();
                            self.execute_input(&text);
                        } else if ctx.input(|i| i.key_pressed(Key::ArrowUp)) {
                            self.history_up();
                        } else if ctx.input(|i| i.key_pressed(Key::ArrowDown)) {
                            self.history_down();
                        } else if ctx.input(|i| i.key_pressed(Key::Tab)) {
                            self.handle_tab_completion();
                        } else if ctx.input(|i| i.key_pressed(Key::Escape)) {
                            self.input_text.clear();
                            self.current_completions = None;
                        }
                    }

                    if ui.button("執行").clicked() {
                        let text = self.input_text.clone();
                        self.execute_input(&text);
                    }
                });
            });

        self.open = open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_console_history_navigation_and_draft_save() {
        let mut console = ConsoleUi::new();

        console.input_text = "/elevate rect 0 0 10 10 5".to_string();
        console.execute_input(&console.input_text.clone());

        console.input_text = "/flatten circle 5 5 2 40".to_string();
        console.execute_input(&console.input_text.clone());

        assert_eq!(console.history.len(), 2);
        assert_eq!(console.history_index, 2);

        // 當前輸入草稿
        console.input_text = "my partial draft".to_string();

        // Up -> 第 2 個指令
        console.history_up();
        assert_eq!(console.input_text, "/flatten circle 5 5 2 40");
        assert_eq!(console.history_index, 1);

        // Up -> 第 1 個指令
        console.history_up();
        assert_eq!(console.input_text, "/elevate rect 0 0 10 10 5");
        assert_eq!(console.history_index, 0);

        // Down -> 第 2 個指令
        console.history_down();
        assert_eq!(console.input_text, "/flatten circle 5 5 2 40");
        assert_eq!(console.history_index, 1);

        // Down -> 回到草稿
        console.history_down();
        assert_eq!(console.input_text, "my partial draft");
        assert_eq!(console.history_index, 2);
    }

    #[test]
    fn test_console_tab_completion_cycling() {
        let mut console = ConsoleUi::new();
        console.input_text = "/he".to_string();
        for expected in [
            "/heal-navmesh",
            "/heal-roads",
            "/height-add",
            "/help",
            "/heal-navmesh",
        ] {
            console.handle_tab_completion();
            assert_eq!(console.input_text, expected);
        }
        // Submitting a command clears the old completion cycle.
        console.execute_input("/echo done");
        console.input_text = "/und".to_string();
        console.handle_tab_completion();
        assert_eq!(console.input_text, "/undo");
        console.execute_input("/echo done");
        console.input_text = "/no-such-command".to_string();
        console.handle_tab_completion();
        assert_eq!(console.input_text, "/no-such-command");
    }

    #[test]
    fn test_console_clear_output() {
        let mut console = ConsoleUi::new();
        assert!(!console.output_logs.is_empty());

        console.execute_input("/clear");
        assert_eq!(console.output_logs.len(), 1);
        assert_eq!(console.output_logs[0].message, "[OK] /clear: 已清除螢幕。");
        assert_eq!(console.output_logs[0].level, LogLevel::Success);
    }

    #[test]
    fn test_console_echo_and_variable_execution() {
        let mut console = ConsoleUi::new();
        let res1 = console.execute_input("$msg = HelloAgainstRome");
        assert!(res1.success);

        let res2 = console.execute_input("/echo $msg");
        assert!(res2.success);

        assert!(console
            .output_logs
            .iter()
            .any(|l| l.message.contains("HelloAgainstRome")));
    }

    #[test]
    fn test_console_brush_log_replays_canonical_command() {
        use crate::macro_cmd::commands::terrain_cmds::canonical_from_log;

        let fresh = || {
            let mut console = ConsoleUi::new();
            console.context.heights = Some(vec![100u8; 17 * 17]);
            console.context.vertex_size = 17;
            console.context.map_tile_dimension = 16;
            console
        };

        let mut console = fresh();
        let res = console.execute_input("/paint height raise 4,4 7,4 --size 3 --strength strong");
        assert!(res.success, "{}", res.message);
        let log = console
            .output_logs
            .iter()
            .find(|l| l.message.starts_with("[OK] /paint"))
            .expect("console 應記錄 /paint (別名) 成功日誌");
        let canonical = canonical_from_log(&log.message)
            .expect("日誌缺少 canonical 文字")
            .to_string();
        assert_eq!(
            canonical,
            "/brush height raise 4,4 7,4 --size 3 --strength strong"
        );

        let mut replay = fresh();
        assert!(replay.execute_input(&canonical).success);
        assert_eq!(replay.context.heights, console.context.heights);
        assert_ne!(replay.context.heights, Some(vec![100u8; 17 * 17]));

        // 舊版語法在 console 被拒絕，高度不變。
        let before = replay.context.heights.clone();
        assert!(!replay.execute_input("/brush raise 5 5 3").success);
        assert_eq!(replay.context.heights, before);
    }

    #[test]
    fn help_lists_the_live_registry_and_groups_commands() {
        let mut console = ConsoleUi::new();
        let count = console.runner.registry.count();
        let result = console.execute_input("/help");
        assert!(result.success);
        assert_eq!(result.affected_count, count);
        assert_eq!(result.message, format!("已列出 {count} 個可用指令。"));
        let entries: Vec<_> = console
            .output_logs
            .iter()
            .filter(|entry| entry.message.starts_with("  /"))
            .map(|entry| entry.message.clone())
            .collect();
        assert_eq!(entries.len(), count);
        for command in console.runner.registry.get_all_commands() {
            assert!(entries.contains(&format!(
                "  /{:<16} : {}",
                command.name(),
                command.description_zh()
            )));
        }
        assert!(console
            .output_logs
            .iter()
            .any(|entry| entry.message == "\n[System]" && entry.level == LogLevel::Debug));
        assert!(!console.runner.can_undo());
    }

    #[test]
    fn help_resolves_aliases_and_reports_real_syntax_or_unknown_command() {
        let mut console = ConsoleUi::new();
        let result = console.execute_input("/MAN /HEIGHT-ADD");
        assert!(result.success);
        assert_eq!(result.affected_count, 0);
        assert_eq!(result.message, "已顯示 /elevate 之詳細說明。");
        assert!(console
            .output_logs
            .iter()
            .any(|entry| entry.message == "別名: /height-add, /raise"));
        assert!(console
            .output_logs
            .iter()
            .any(|entry| entry.message.starts_with("語法: /elevate rect")
                && entry.level == LogLevel::Success));
        assert!(console
            .output_logs
            .iter()
            .any(|entry| entry.message == "參數規格:"));
        assert!(console
            .output_logs
            .iter()
            .any(|entry| entry.message.contains("(String) [必填]")));
        let result = console.execute_input("/? no-such-command");
        assert!(!result.success);
        assert_eq!(result.message, "未找到指令「no-such-command」。");
        assert_eq!(console.output_logs.last().unwrap().level, LogLevel::Error);
        assert_eq!(console.status_text, "就緒 (Ready)");
    }

    #[test]
    fn console_aliases_preserve_clear_echo_and_case_insensitive_variables() {
        let mut console = ConsoleUi::new();
        assert!(console.execute_input("$Message = original").success);
        assert!(console.execute_input("$MESSAGE = \"hello world\"").success);
        assert_eq!(console.context.variables.len(), 1);
        let result = console.execute_input("/PRINT $message");
        assert_eq!(result.message, "hello world");
        assert_eq!(result.affected_count, 0);
        assert!(console
            .output_logs
            .iter()
            .any(|entry| entry.message == "hello world" && entry.level == LogLevel::Info));
        assert!(console.execute_input("/CLS").success);
        assert_eq!(console.output_logs.len(), 1);
        assert_eq!(console.output_logs[0].message, "[OK] /CLS: 已清除螢幕。");
        assert_eq!(console.context.variables["Message"], "hello world");
        assert!(console.context.logs.is_empty());
        assert_eq!(console.status_text, "就緒 (Ready)");
    }

    fn terrain_console() -> ConsoleUi {
        let mut console = ConsoleUi::new();
        console.context.heights = Some(vec![100; 17 * 17]);
        console.context.vertex_size = 17;
        console.context.map_tile_dimension = 16;
        console
    }

    struct ScriptFile(std::path::PathBuf);

    impl ScriptFile {
        fn new(content: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("console parity {}.armcmd", uuid::Uuid::new_v4()));
            std::fs::write(&path, content).unwrap();
            Self(path)
        }

        fn command(&self, alias: &str) -> String {
            // The command tokenizer treats backslashes as escape characters.
            format!(
                "/{alias} \"{}\" 3",
                self.0.to_str().unwrap().replace('\\', "/")
            )
        }
    }

    impl Drop for ScriptFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn console_runs_bom_script_with_arguments_as_one_undo_redo_transaction() {
        let script = ScriptFile::new("\u{feff}# comment\r\n$Delta = 3\r\n/elevate rect 4 4 4 4 $DELTA\r\n/elevate rect 4 4 4 4 $1\r\n");
        let mut console = terrain_console();
        let before = console.context.heights.clone();
        let result = console.execute_input(&script.command("EXEC"));
        assert!(result.success, "{}", result.message);
        assert_eq!(console.context.heights.as_ref().unwrap()[4 * 17 + 4], 106);
        assert_eq!(console.context.variables["1"], "3");
        assert_eq!(
            console.context.variables["0"],
            script.0.file_stem().unwrap().to_str().unwrap()
        );
        let after = console.context.heights.clone();
        assert!(console.execute_input("/u").success);
        assert_eq!(console.context.heights, before);
        assert!(!console.runner.can_undo());
        assert!(console.runner.can_redo());
        assert!(console.execute_input("/r").success);
        assert_eq!(console.context.heights, after);
        assert!(!console.runner.can_redo());
        assert!(console.context.logs.is_empty());
    }

    #[test]
    fn console_failed_script_rolls_back_without_losing_previous_history() {
        let script = ScriptFile::new(
            "/elevate rect 4 4 4 4 3\r\n/no-such-command\r\n/elevate rect 4 4 4 4 7",
        );
        let mut console = terrain_console();
        let initial = console.context.heights.clone();
        assert!(!console.execute_input("/undo").success);
        assert!(!console.execute_input("/redo").success);
        assert!(console.execute_input("/elevate rect 4 4 4 4 1").success);
        let before_script = console.context.heights.clone();
        let result = console.execute_input(&script.command("macro"));
        assert!(!result.success);
        assert!(result.message.contains("行 2: 未知指令"));
        assert_eq!(console.context.heights, before_script);
        assert!(console.execute_input("/undo").success);
        assert_eq!(console.context.heights, initial);
        assert!(!console.runner.can_undo());
        assert!(!console.execute_input("/run").success);
        assert!(
            !console
                .execute_input("/run no-such-console-script.armcmd")
                .success
        );
        assert_eq!(console.status_text, "就緒 (Ready)");
    }

    #[test]
    fn console_history_boundaries_and_blank_input_preserve_the_draft() {
        let mut console = ConsoleUi::new();
        console.input_text = "unsent draft".to_string();
        console.history_up();
        console.history_down();
        console.execute_input("   ");
        assert_eq!(console.input_text, "unsent draft");
        assert!(console.history.is_empty());
        console.execute_input("  /echo first  ");
        console.input_text = "draft".to_string();
        console.history_up();
        console.history_up();
        assert_eq!(console.input_text, "/echo first");
        console.history_down();
        console.history_down();
        assert_eq!(console.input_text, "draft");
    }
}
