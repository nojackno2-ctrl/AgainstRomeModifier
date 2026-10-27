mod cli;

use against_rome_ambience::Language;
use against_rome_editor_app::design_check_ui::{format_issue_row, headless_design_check, issue_summary};
use against_rome_editor_app::{load_map, EditorApp};
use against_rome_maps::diagnostics::MapIssueSeverity;
use cli::{batch_validation_maps, parse_startup_options, resolve_startup_map};
use std::path::Path;
use eframe::egui;

struct Desktop {
    editor: EditorApp,
    frames_left: Option<u8>,
}

fn native_startup_error_message(error: &str) -> String {
    format!(
        "無法啟動 Against Rome 地圖編輯器視窗。\n\n原始錯誤：\n{error}\n\n建議：確認顯示卡驅動程式支援 OpenGL，更新驅動程式後在本機桌面重新啟動；若使用遠端桌面或虛擬顯示環境，請改用一般桌面工作階段。若問題持續，請回報上方完整錯誤。\n\nFailed to start the editor window. Check that your graphics driver supports OpenGL, update it, and retry in a local desktop session. If you use Remote Desktop or a virtual display, retry in a regular desktop session. If it still fails, report the full error above."
    )
}

fn show_native_startup_error(error: &str) {
    rfd::MessageDialog::new()
        .set_title("Against Rome Map Editor — 啟動失敗 / Startup Failed")
        .set_description(native_startup_error_message(error))
        .set_level(rfd::MessageLevel::Error)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}
impl eframe::App for Desktop {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        eframe::App::update(&mut self.editor, ctx, frame);
        if let Some(left) = &mut self.frames_left {
            *left = left.saturating_sub(1);
            if *left == 0 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.request_repaint();
            }
        }
    }
}

/// Prints one map's Design Check; returns whether an Error would block Safe Save.
fn validate_one(path: &Path, lang: Language) -> Result<bool, String> {
    let session = load_map(path)?;
    let (v, c) = (session.vertex_size, session.collision_size);
    match lang {
        Language::English => println!("Validated height {v}x{v}, collision {c}x{c}; no files written."),
        _ => println!("已檢查高度 {v}x{v}、碰撞 {c}x{c}；未寫入任何檔案。"),
    }
    let rows = headless_design_check(path, session, lang)?;
    for row in &rows {
        println!("{}", format_issue_row(row, lang));
    }
    println!("{}", issue_summary(&rows, lang));
    Ok(rows.iter().any(|r| r.severity == MapIssueSeverity::Error))
}

/// Checks every editable map under `--game`; a map that cannot be opened counts as blocked.
fn validate_all(game: &Path, lang: Language) -> Result<bool, String> {
    let maps = batch_validation_maps(game)?;
    let mut failed = Vec::new();
    for info in &maps {
        println!("== {}", info.id);
        match validate_one(&info.directory_path, lang) {
            Ok(false) => {}
            Ok(true) => failed.push(info.id.clone()),
            Err(e) => {
                println!("{e}");
                failed.push(info.id.clone());
            }
        }
    }
    let list = failed.join(", ");
    match lang {
        Language::English => println!("Checked {} maps; {} with errors{}", maps.len(), failed.len(),
            if failed.is_empty() { String::new() } else { format!(": {list}") }),
        _ => println!("已檢查 {} 張地圖；{} 張有錯誤{}", maps.len(), failed.len(),
            if failed.is_empty() { String::new() } else { format!("：{list}") }),
    }
    Ok(!failed.is_empty())
}

fn run() -> Result<(), String> {
    let options = parse_startup_options(std::env::args_os().skip(1))?;
    if options.show_help {
        println!("against-rome-editor [--game <PATH> --map <ID> | --map DIRECTORY] [--validate-map [--lang zh|en] | --smoke-test]\n--validate-map runs the read-only Design Check (exit 2 when errors would block Safe Save).\n--game <PATH> --validate-map without --map checks every editable (non-campaign) map.\n--lang zh prints Design Check results in Traditional Chinese.\n--smoke-test runs three native UI updates and closes.");
        return Ok(());
    }
    if options.show_version {
        println!("against-rome-editor {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let map = resolve_startup_map(&options)?;
    if options.validate_map {
        let lang = if options.chinese == Some(true) {
            Language::TraditionalChinese
        } else {
            Language::English
        };
        let blocked = match &map {
            Some(path) => validate_one(path, lang)?,
            None => validate_all(options.game_path.as_deref().unwrap(), lang)?,
        };
        if blocked {
            std::process::exit(2);
        }
        return Ok(());
    }
    let loaded = map.as_ref().map(|p| load_map(p)).transpose()?;
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    if let Err(error) = eframe::run_native(
        "Against Rome Map Editor — Rust",
        native_options,
        Box::new(move |cc| {
            let mut editor = EditorApp::new(cc);
            if let Some(session) = loaded {
                editor.set_loaded_map(map.unwrap(), session)
                    .expect("new editor has no unsaved map changes");
            }
            Box::new(Desktop {
                editor,
                frames_left: options.smoke_test.then_some(3),
            })
        }),
    ) {
        show_native_startup_error(&error.to_string());
        return Err(error.to_string());
    }
    if options.smoke_test {
        println!("Native desktop smoke test completed: three UI updates executed.");
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::native_startup_error_message;

    #[test]
    fn native_startup_error_message_keeps_cause_and_actionable_guidance() {
        let message =
            native_startup_error_message("WGL: The driver does not appear to support OpenGL");

        assert!(message.contains("WGL: The driver does not appear to support OpenGL"));
        assert!(message.contains("更新驅動程式"));
        assert!(message.contains("本機桌面"));
        assert!(message.contains("report the full error"));
    }
}
