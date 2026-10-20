//! Shared driver for the "fingerprint-gated EXE proof + saved-map audit" probes
//! (`probe_native_{lager,light,object_parents,particle}_pool`, `probe_{type_position,typus_creation}_contract`).

use super::Parsed;
use crate::json::J;
use crate::util::*;
use std::path::{Path, PathBuf};

pub struct Audit {
    pub name: &'static str,
    pub inspect_code: fn(&[u8]) -> R<J>,
    pub inspect_maps: fn(&Path) -> R<J>,
    /// Keys echoed on the final stdout line.
    pub summary_keys: &'static [&'static str],
    /// Extra limitations appended after the maps section (scripts that add them in `main`).
    pub extra_limitations: Option<Vec<&'static str>>,
}

pub fn merge(into: &mut J, from: J) {
    if let J::Obj(items) = from {
        for (k, v) in items {
            into.set(&k, v);
        }
    }
}

pub fn run(audit: &Audit, args: &[String]) -> R<i32> {
    let p = Parsed::parse(args, &["exe"])?;
    p.exact(2, &format!("{} GAME_ROOT OUTPUT.json --exe EXE", audit.name))?;
    let exe_path = PathBuf::from(p.opt("exe").ok_or_else(|| usage("the following arguments are required: --exe"))?);
    let root = resolve(Path::new(&p.pos[0]));
    let exe_dir = resolve(&exe_path).parent().map(Path::to_path_buf).unwrap_or_default();
    let output = new_output(
        Path::new(&p.pos[1]),
        &[root.clone(), exe_dir, installed_game()],
        "source and installed-game directories",
    )?;
    let source = read(&exe_path)?;
    let mut result = (audit.inspect_code)(&source)?;
    merge(&mut result, (audit.inspect_maps)(&root)?);
    if let Some(limits) = &audit.extra_limitations {
        result.set("limitations", limits.clone());
    }
    req(read(&exe_path)? == source, "EXE changed during audit")?;
    write_new(&output, &(result.pretty(true) + "\n"))?;
    let mut summary = J::obj();
    for key in audit.summary_keys {
        if *key == "maps" {
            // The original reports the number of audited maps, not the maps themselves.
            summary.set("maps", result.get("maps").map_or(0, J::len));
        } else {
            summary.set(key, result.get(key).cloned().unwrap_or(J::Null));
        }
    }
    println!("{}", summary.line(true));
    Ok(0)
}
