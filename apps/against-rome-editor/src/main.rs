use against_rome_editor_app::{load_map, EditorApp};
use eframe::egui;
use std::path::PathBuf;

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
    let mut map = None;
    let mut validate = false;
    let mut smoke = false;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            println!("against-rome-editor [--map DIRECTORY] [--validate-map | --smoke-test]\n--validate-map reads BMP layers without writing.\n--smoke-test runs three native UI updates and closes.");
            return Ok(());
        } else if arg == "--version" {
            println!("against-rome-editor {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        } else if arg == "--map" && map.is_none() {
            map = Some(PathBuf::from(
                args.next().ok_or("--map requires a directory")?,
            ));
        } else if arg == "--validate-map" && !validate {
            validate = true;
        } else if arg == "--smoke-test" && !smoke {
            smoke = true;
        } else {
            return Err(format!(
                "unknown or duplicate argument: {}",
                arg.to_string_lossy()
            ));
        }
    }
    if validate && (map.is_none() || smoke) {
        return Err("--validate-map requires --map and cannot combine with --smoke-test".into());
    }
    let loaded = map.as_ref().map(|p| load_map(p)).transpose()?;
    if validate {
        let session = loaded.as_ref().unwrap();
        println!(
            "Validated height {}x{}, collision {}x{}; no files written.",
            session.vertex_size,
            session.vertex_size,
            session.collision_size,
            session.collision_size
        );
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
                editor.set_loaded_map(map.unwrap(),session);
            }
            Box::new(Desktop {
                editor,
                frames_left: smoke.then_some(3),
            })
        }),
    )
    .map_err(|e| e.to_string())?;
    if smoke {
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
