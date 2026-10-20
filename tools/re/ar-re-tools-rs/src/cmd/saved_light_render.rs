//! `probe_saved_light_render_contract.py`: hash-bound static light reader/update/render proof.
//!
//! An equivalent Rust port already exists as `tools/re/saved-light-contract-rs`
//! (`saved_light_contract::run`, 746 instruction assertions via iced-x86, the same
//! exclusive-create / `re_workspace`-only output rules), so this subcommand delegates to it
//! instead of duplicating the 980-line contract. Unlike the Python script it needs the
//! strengthened reference JSON as explicit third argument (cross-checked against the frozen contract).

use super::Parsed;
use crate::util::*;
use std::path::Path;

pub fn run(args: &[String]) -> R<i32> {
    let p = Parsed::parse(args, &[])?;
    p.exact(3, "probe-saved-light-render-contract EXE OUTPUT.json STRENGTHENED_REFERENCE.json")?;
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3).ok_or("manifest is not under repository/tools/re")?;
    let summary = saved_light_contract::run(repo, Path::new(&p.pos[0]), Path::new(&p.pos[1]), Path::new(&p.pos[2])).map_err(|e| e.to_string())?;
    println!("{summary}");
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_three_arguments() {
        assert!(run(&["exe".into()]).unwrap_err().starts_with("usage:"));
    }

    #[test]
    fn refuses_outputs_outside_repository_re_workspace() {
        let dir = crate::tests_support::TempDir::new();
        let args = [
            dir.path().join("nope.exe").display().to_string(),
            dir.path().join("report.json").display().to_string(),
            dir.path().join("ref.json").display().to_string(),
        ];
        let error = run(&args).unwrap_err();
        assert!(error.contains("re_workspace") || error.contains("cannot find"), "{error}");
    }
}
