use against_rome_packaging::{ModBundleExporter, ModExportOptions};
use std::env;
use std::path::Path;

fn print_usage() {
    eprintln!("Usage: ArmMapPackage install <bundle.zip> <game-root> | export <map-directory> <bundle.zip> [--force]");
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    let force_export = args.len() == 4
        && args.first().map(String::as_str) == Some("export")
        && args.get(3).map(String::as_str) == Some("--force");

    let is_valid_cmd = matches!(args.first().map(String::as_str), Some("install") | Some("export"));

    if (!force_export && args.len() != 3) || !is_valid_cmd {
        print_usage();
        std::process::exit(2);
    }

    match run(&args, force_export) {
        Ok(()) => std::process::exit(0),
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

fn run(args: &[String], force_export: bool) -> Result<(), Box<dyn std::error::Error>> {
    if args[0] == "install" {
        let zip_path = Path::new(&args[1]);
        let game_root = Path::new(&args[2]);
        let result = ModBundleExporter::install_from_zip(zip_path, game_root, None)?;

        println!(
            "Installed {}: ENDL_{:03} ({} payload files)",
            result.manifest.title, result.installed_slot, result.extracted_file_count
        );
        println!("{}", result.directory_path);
        let (error_count_str, warning_count_str) = match &result.preflight_report {
            Some(report) => (
                report.error_count().to_string(),
                report.warning_count().to_string(),
            ),
            None => (String::new(), String::new()),
        };
        println!(
            "Playability remains unverified: preflight {} errors, {} warnings.",
            error_count_str, warning_count_str
        );
    } else {
        let map_directory = Path::new(&args[1]);
        let output_zip = Path::new(&args[2]);
        let options = ModExportOptions {
            force_export_on_errors: force_export,
            ..Default::default()
        };
        let result = ModBundleExporter::export_to_zip(map_directory, output_zip, Some(&options))?;

        println!(
            "Exported {}: {} payload files",
            result.manifest.title, result.file_count
        );
        println!("{}", result.output_zip_path);
        println!(
            "Preflight: {} errors, {} warnings.",
            result.preflight_report.error_count(),
            result.preflight_report.warning_count()
        );
    }
    Ok(())
}
