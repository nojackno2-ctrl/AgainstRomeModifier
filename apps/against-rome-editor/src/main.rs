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
    eframe::run_native(
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
    )
    .map_err(|e| e.to_string())?;
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
