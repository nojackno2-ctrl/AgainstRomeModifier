use saved_light_contract::{run, Result};
use std::path::Path;

fn main() {
    if let Err(error) = execute() {
        eprintln!("saved-light-contract: {error}");
        std::process::exit(1);
    }
}

fn execute() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: saved-light-contract EXE OUTPUT.json STRENGTHENED_REFERENCE.json".into());
    }
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3)
        .ok_or("manifest is not under repository/tools/re")?;
    let reference = Path::new(&args[2]);
    let summary = run(repo, Path::new(&args[0]), Path::new(&args[1]), reference)?;
    println!("{}", serde_json::to_string(&summary)?);
    Ok(())
}
