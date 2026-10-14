use particle_render_contract::{hash, run, Result, WINDOWS};
use serde_json::{json, Value};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

fn main() {
    if let Err(error) = execute() {
        eprintln!("particle-render-contract: {error}");
        std::process::exit(1);
    }
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    Ok(())
}

fn execute() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        return Err("usage: particle-render-contract EXE NEW_OUTPUT.json NATIVE_AUDIT.json [VA:HEX_LENGTH ...]".into());
    }
    let output = Path::new(&args[1]);
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .ok_or("repo")?
        .canonicalize()?;
    let parent = output.parent().ok_or("output parent")?.canonicalize()?;
    if !parent.starts_with(repo.join("re_workspace")) || output.exists() {
        return Err("output must be new in existing repository re_workspace".into());
    }
    let mut windows = WINDOWS.to_vec();
    for text in &args[3..] {
        let (va, length) = text.split_once(':').ok_or("VA:HEX_LENGTH")?;
        windows.push((
            u64::from_str_radix(va.trim_start_matches("0x"), 16)?,
            usize::from_str_radix(length.trim_start_matches("0x"), 16)?,
        ));
    }
    let mut value = run(Path::new(&args[0]), Path::new(&args[2]), &windows)?;
    let sources: Value = serde_json::from_str(include_str!("sources.json"))?;
    for row in sources.as_array().ok_or("sources")? {
        let raw = fs::read(repo.join(row["path"].as_str().ok_or("source path")?))?;
        if hash(&raw) != row["sha256"].as_str().ok_or("source hash")? {
            return Err("existing evidence hash mismatch".into());
        }
    }
    value["evidence_sources"] = sources;
    value["assertions_sha256"] = json!(hash(include_bytes!("assertions.json")));
    value["parent_export_status"] = json!("not supplied; no shared REA calls");
    write_json(output, &value)?;
    println!(
        "assertions={} reference={} xrefs={} source_unchanged=true",
        value["frozen_assertions"],
        value["reference_assertions"],
        value["xrefs"].as_array().ok_or("xrefs")?.len()
    );
    Ok(())
}
