use particle_numeric_contract::{collect, write_new, Result};
use std::path::Path;
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("freeze EXE EXPORT".into());
    }
    let v = collect(Path::new(&args[0]), Path::new(&args[1]))?;
    write_new(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src/frozen.json"),
        &v,
    )?;
    println!(
        "selected={} instructions={}",
        v["selected_record_count"], v["frozen_instruction_count"]
    );
    Ok(())
}
