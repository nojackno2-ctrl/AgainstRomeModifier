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

        ui.append_log("=== Against Rome Map Editor 即時控制台已就緒 ===", LogLevel::Info);
        ui.append_log("輸入 /help 查詢所有指令，或使用 Tab 進行自動補全與候選切換。", LogLevel::Debug);
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

        if trimmed == "/clear" || trimmed == "/cls" {
            self.clear_output();
            self.status_text = "已清除螢幕".to_string();
            return CommandResult::ok("已清除螢幕。", 0);
        }

        self.append_log(&format!("> {trimmed}"), LogLevel::Command);

        let result = self.runner.execute_line(trimmed, &mut self.context, true);

        // 將執行過程收集到的日誌注入至輸出視窗
        let logs: Vec<_> = self.context.logs.drain(..).collect();
        for (msg, lvl) in logs {
            self.append_log(&msg, lvl);
        }

        self.status_text = if result.success {
            format!("執行成功 (影響 {} 項)", result.affected_count)
        } else {
            "執行失敗".to_string()
        };

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
                self.input_text = format!("{} {}", &self.completion_prefix[..last_space], candidate);
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
                                ui.colored_label(Color32::from_rgb(100, 100, 100), format!("[{}]", entry.timestamp));
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
        console.input_text = "/el".to_string();

        console.handle_tab_completion();
        assert_eq!(console.input_text, "/elevate");
    }

    #[test]
    fn test_console_clear_output() {
        let mut console = ConsoleUi::new();
        assert!(!console.output_logs.is_empty());

        console.execute_input("/clear");
        assert!(console.output_logs.is_empty());
    }

    #[test]
    fn test_console_echo_and_variable_execution() {
        let mut console = ConsoleUi::new();
        let res1 = console.execute_input("$msg = HelloAgainstRome");
        assert!(res1.success);

        let res2 = console.execute_input("/echo $msg");
        assert!(res2.success);

        assert!(console.output_logs.iter().any(|l| l.message.contains("HelloAgainstRome")));
    }
}
