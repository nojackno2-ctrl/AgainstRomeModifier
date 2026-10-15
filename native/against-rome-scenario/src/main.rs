use against_rome_scenario::*;
use std::{fs, io::Write, path::Path};

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("inspect") => {
            let path = args.get(2).ok_or("inspect <script.bci>")?;
            let source = fs::read(path).map_err(|e| e.to_string())?;
            let raw = if source.starts_with(b"PFIL") {
                against_rome_data::lzss::decompress_pfil(&source, None).map_err(|e| e.to_string())?
            } else { source };
            let image = BciImage::parse(&raw)?;
            println!("CODE={} constants={} main={} dtor={} wait_hook={:?}",
                image.code.len()*4, image.constant_offsets.len(), image.main, image.dtor,
                find_wait_hook(&image.code, image.main));
        }
        Some("compile") => {
            if !(5..=8).contains(&args.len()) {
                return Err("compile <original.bci> <arm_scenario.json> <new-output.bci> [camera.json|-] [game-root|-] [source-map]".into());
            }
            let input = fs::read(&args[2]).map_err(|e| e.to_string())?;
            let document = ScenarioDocument::from_json(&fs::read(&args[3]).map_err(|e| e.to_string())?)?;
            let camera: Option<StartCamera> = args.get(5).filter(|p| p.as_str() != "-")
                .map(|p| fs::read(p).map_err(|e| e.to_string())
                    .and_then(|s| serde_json::from_slice(&s).map_err(|e| e.to_string())))
                .transpose()?;
            let aliases = args.get(6).filter(|p| p.as_str() != "-")
                .map(|p| load_aliases(Path::new(p))).transpose()?;
            let bindings = args.get(7).map(|p| DataBindingSource::load(Path::new(p))).transpose()?;
            let bytes = if let Some(source) = &bindings {
                source.compile_script(&input, &document, camera.as_ref(), aliases.as_deref())?
            } else {
                compile_script_verified(&input, &document, camera.as_ref(), aliases.as_deref(), None)?
            };
            let output = Path::new(&args[4]);
            let parent = output.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
            let canonical = fs::canonicalize(parent).map_err(|e| e.to_string())?;
            if canonical.to_string_lossy().to_ascii_lowercase().contains("\\against rome") {
                return Err("CLI output inside installed game directory is forbidden".into());
            }
            if let Some(source) = &bindings { source.verify_unchanged()?; }
            let mut file = fs::OpenOptions::new().write(true).create_new(true).open(output).map_err(|e| e.to_string())?;
            file.write_all(&bytes).map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
            println!("compiled {} bytes into {}", bytes.len(), output.display());
        }
        _ => return Err("commands: inspect, compile (input PFIL format preserved, existing files never overwritten)".into()),
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
