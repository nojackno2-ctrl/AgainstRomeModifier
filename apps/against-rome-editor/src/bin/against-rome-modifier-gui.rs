use against_rome_editor_app::modifier_ui::{default_state_dir, ModifierUi};
use eframe::egui;
use std::path::PathBuf;

struct ModifierStandaloneApp {
    modifier: ModifierUi,
}

impl ModifierStandaloneApp {
    fn new(
        cc: &eframe::CreationContext<'_>,
        state_dir: PathBuf,
        english: Option<bool>,
    ) -> Self {
        #[cfg(windows)]
        if let Some(windows) = std::env::var_os("WINDIR") {
            for name in ["msjh.ttc", "msyh.ttc"] {
                if let Ok(bytes) = std::fs::read(PathBuf::from(&windows).join("Fonts").join(name)) {
                    let mut fonts = egui::FontDefinitions::default();
                    fonts.font_data.insert("system-cjk".into(), egui::FontData::from_owned(bytes));
                    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                        fonts.families.entry(family).or_default().push("system-cjk".into());
                    }
                    cc.egui_ctx.set_fonts(fonts);
                    break;
                }
            }
        }
        #[cfg(not(windows))]
        let _ = cc;

        let mut modifier = ModifierUi::persisted(state_dir);
        modifier.run_startup_migrations();
        if let Some(english) = english {
            modifier.model.english = english;
        }
        modifier.open = true;
        Self { modifier }
    }
}

impl eframe::App for ModifierStandaloneApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |_ui| {
            // Background canvas for the standalone modifier window
        });
        self.modifier.show(ctx);
        if !self.modifier.open {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct CliArgs {
    smoke: bool,
    state_dir: Option<PathBuf>,
    /// `Some(true)` = English, `Some(false)` = Traditional Chinese, `None` = keep persisted choice.
    english: Option<bool>,
}

enum Parsed {
    Run(CliArgs),
    Exit(String),
}

/// Mirrors C# `--lang`: missing value is an error, blank keeps the default,
/// `en` (case-insensitive) is English, any other value is Traditional Chinese.
fn parse_args<I: IntoIterator<Item = std::ffi::OsString>>(args: I) -> Result<Parsed, String> {
    let mut cli = CliArgs::default();
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            return Ok(Parsed::Exit(
                "against-rome-modifier-gui [--state-dir DIRECTORY] [--lang zh|en] [--smoke-test]
                 --smoke-test runs headless egui frames and exits without opening a window."
                    .to_string(),
            ));
        } else if arg == "--version" || arg == "-v" {
            return Ok(Parsed::Exit(format!(
                "against-rome-modifier-gui {}",
                env!("CARGO_PKG_VERSION")
            )));
        } else if arg == "--smoke-test" && !cli.smoke {
            cli.smoke = true;
        } else if arg == "--state-dir" && cli.state_dir.is_none() {
            let next_arg = args.next().ok_or("--state-dir requires a directory")?;
            cli.state_dir = Some(PathBuf::from(next_arg));
        } else if arg.to_string_lossy().eq_ignore_ascii_case("--lang") {
            let value = args.next().ok_or("--lang requires a language code (zh or en)")?;
            let value = value.to_string_lossy();
            if !value.trim().is_empty() {
                cli.english = Some(value.eq_ignore_ascii_case("en"));
            }
        } else {
            return Err(format!(
                "unknown or duplicate argument: {}",
                arg.to_string_lossy()
            ));
        }
    }
    Ok(Parsed::Run(cli))
}

fn run() -> Result<(), String> {
    let CliArgs { smoke, state_dir, english } = match parse_args(std::env::args_os().skip(1))? {
        Parsed::Exit(text) => {
            println!("{text}");
            return Ok(());
        }
        Parsed::Run(cli) => cli,
    };

    let target_state_dir = state_dir.unwrap_or_else(default_state_dir);

    if smoke {
        let mut modifier = ModifierUi::persisted(target_state_dir);
        if let Some(english) = english {
            modifier.model.english = english;
        }
        modifier.open = true;
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let _ = ctx.run(Default::default(), |ctx| {
                modifier.show(ctx);
            });
        }
        println!("Native desktop smoke test completed: three UI updates executed.");
        return Ok(());
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Against Rome Modifier — Rust")
            .with_inner_size([820.0, 720.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Against Rome Modifier — Rust",
        native_options,
        Box::new(move |cc| Box::new(ModifierStandaloneApp::new(cc, target_state_dir, english))),
    )
    .map_err(|e| e.to_string())?;

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
    use super::*;

    fn parse(args: &[&str]) -> Result<CliArgs, String> {
        match parse_args(args.iter().map(std::ffi::OsString::from))? {
            Parsed::Run(cli) => Ok(cli),
            Parsed::Exit(_) => Err("exit".into()),
        }
    }

    #[test]
    fn lang_defaults_to_persisted_choice() {
        assert_eq!(parse(&[]).unwrap().english, None);
        assert_eq!(parse(&["--smoke-test"]).unwrap().english, None);
    }

    #[test]
    fn lang_en_is_case_insensitive() {
        assert_eq!(parse(&["--lang", "en"]).unwrap().english, Some(true));
        assert_eq!(parse(&["--LANG", "EN"]).unwrap().english, Some(true));
    }

    #[test]
    fn lang_zh_and_unknown_use_traditional_chinese() {
        assert_eq!(parse(&["--lang", "zh"]).unwrap().english, Some(false));
        assert_eq!(parse(&["--lang", "fr"]).unwrap().english, Some(false));
        assert_eq!(parse(&["--lang", "english"]).unwrap().english, Some(false));
    }

    #[test]
    fn lang_blank_keeps_default() {
        assert_eq!(parse(&["--lang", ""]).unwrap().english, None);
        assert_eq!(parse(&["--lang", "  "]).unwrap().english, None);
    }

    #[test]
    fn lang_missing_value_is_error() {
        assert!(parse(&["--lang"]).unwrap_err().contains("--lang"));
        assert!(parse(&["--smoke-test", "--lang"]).is_err());
    }

    #[test]
    fn lang_last_value_wins_and_combines_with_other_flags() {
        let cli = parse(&["--lang", "en", "--state-dir", "x", "--lang", "zh", "--smoke-test"]).unwrap();
        assert_eq!(cli.english, Some(false));
        assert_eq!(cli.state_dir, Some(PathBuf::from("x")));
        assert!(cli.smoke);
    }

    #[test]
    fn unknown_argument_still_rejected() {
        assert!(parse(&["--bogus"]).is_err());
    }
}
