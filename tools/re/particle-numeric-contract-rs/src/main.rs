use particle_numeric_contract::{collect, hash, require, verify_frozen, write_new, Pe, Result};
use serde_json::{json, Value};
use std::{fs, path::Path};
fn main() {
    if let Err(e) = run() {
        eprintln!("particle-numeric-contract: {e}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let a: Vec<_> = std::env::args().skip(1).collect();
    require(
        a.len() == 3,
        "usage: particle-numeric-contract EXE EXPORT NEW_OUTPUT.json",
    )?;
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .ok_or("repo")?
        .canonicalize()?;
    let out = Path::new(&a[2]);
    let parent = out.parent().ok_or("output parent")?.canonicalize()?;
    require(
        parent.starts_with(repo.join("re_workspace")) && !out.exists(),
        "output must be new inside repository re_workspace",
    )?;
    let frozen: Value = serde_json::from_str(include_str!("frozen.json"))?;
    let mut actual = collect(Path::new(&a[0]), Path::new(&a[1]))?;
    let source = fs::read(&a[0])?;
    require(
        hash(&source) == particle_numeric_contract::HASH,
        "source changed after collect",
    )?;
    let assertions = verify_frozen(&Pe::parse(&source)?, &frozen)?;
    require(actual == frozen, "frozen evidence changed")?;
    require(fs::read(&a[0])? == source, "source changed before output")?;
    actual["verified_instruction_assertions"] = json!(assertions);
    actual["frozen_json_sha256"] = json!(hash(include_bytes!("frozen.json")));
    write_new(out, &actual)?;
    println!("selected=6 body_instructions=693 callee_instructions=7 constants=7 operand_widths=9 pointers=2 assertions={assertions} source_unchanged=true");
    Ok(())
}
