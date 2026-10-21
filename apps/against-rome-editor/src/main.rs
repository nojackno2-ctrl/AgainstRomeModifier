mod cli;

use against_rome_ambience::Language;
use against_rome_editor_app::design_check_ui::{format_issue_row, headless_design_check, issue_summary};
use against_rome_editor_app::{load_map, EditorApp};
use against_rome_maps::diagnostics::MapIssueSeverity;
use cli::{parse_startup_options, resolve_startup_map};
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

fn run() -> Result<(), String> {
    let options = parse_startup_options(std::env::args_os().skip(1))?;
    if options.show_help {
        println!("against-rome-editor [--game <PATH> --map <ID> | --map DIRECTORY] [--validate-map | --smoke-test]\n--validate-map runs the read-only Design Check (exit 2 when errors would block Safe Save).\n--smoke-test runs three native UI updates and closes.");
        return Ok(());
    }
    if options.show_version {
        println!("against-rome-editor {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let map = resolve_startup_map(&options)?;
    let loaded = map.as_ref().map(|p| load_map(p)).transpose()?;
    if options.validate_map {
        let session = loaded.unwrap();
        println!(
            "Validated height {}x{}, collision {}x{}; no files written.",
            session.vertex_size,
            session.vertex_size,
            session.collision_size,
            session.collision_size
        );
        let lang = Language::English;
        let rows = headless_design_check(map.as_ref().unwrap(), session, lang)?;
        for row in &rows {
            println!("{}", format_issue_row(row, lang));
        }
        println!("{}", issue_summary(&rows, lang));
        if rows.iter().any(|r| r.severity == MapIssueSeverity::Error) {
            std::process::exit(2);
        }
        return Ok(());
    }
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
