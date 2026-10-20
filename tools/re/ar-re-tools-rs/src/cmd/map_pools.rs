//! `probe_map_pools.py`: structural probe of action/anim/gfxtype pools of every map.

use super::Parsed;
use crate::jobj;
use crate::json::{Counter, J};
use crate::pools;
use crate::util::*;
use std::path::Path;

pub fn scan(root: &Path) -> R<(J, usize)> {
    let mut reports = J::arr();
    let mut errors = J::arr();
    let mut families = Counter::new();
    let resolved_root = resolve(root);
    for directory in map_dirs(root)? {
        for family in ["action", "anim", "gfxtype"] {
            let path = directory.join("DATA").join(format!("{family}.dat"));
            if !path.is_file() {
                continue;
            }
            req(is_inside(&resolve(&path), &resolved_root), "pool path escapes source")?;
            let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            match read(&path).and_then(|bytes| pools::inspect(&bytes, family)) {
                Ok(info) => {
                    let mut entry = jobj! { "path" => rel };
                    if let J::Obj(items) = info.json {
                        for (k, v) in items {
                            entry.set(&k, v);
                        }
                    }
                    families.inc(family);
                    reports.push(entry);
                }
                Err(error) => errors.push(jobj! { "path" => rel, "error" => error }),
            }
        }
    }
    let error_count = errors.len();
    let result = jobj! {
        "files" => reports,
        "errors" => errors,
        "families" => families.to_json(),
        "limitations" => vec![
            "native writer version 1 layout only; stricter capacity and length checks than native readers",
            "PFIL decoded to declared size; compressed trailing bytes and wrapper integrity not established",
            "no field semantics, pool cross-references, runtime load success or missing-file completeness inferred",
        ],
    };
    Ok((result, error_count))
}

pub fn run(args: &[String]) -> R<i32> {
    let p = Parsed::parse(args, &[])?;
    p.exact(2, "probe-map-pools ROOT OUTPUT.json")?;
    let (root, output) = (Path::new(&p.pos[0]), Path::new(&p.pos[1]));
    let output = new_output(output, &[resolve(root)], "source directory")?;
    let (result, errors) = scan(root)?;
    write_new(&output, &(result.pretty(true) + "\n"))?;
    println!("{}", jobj! { "families" => result.get("families").cloned().unwrap_or(J::Null), "errors" => errors }.line(true));
    Ok(if errors > 0 { 1 } else { 0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_support::*;

    #[test]
    fn scans_synthetic_maps() {
        let dir = TempDir::new();
        let data = dir.path().join("MAPS/M1/DATA");
        std::fs::create_dir_all(&data).unwrap();
        // action: version 1, count 2, words 6, two 25-byte records
        let mut action = Vec::new();
        action.extend([1u32, 2, 6].iter().flat_map(|v| v.to_le_bytes()));
        action.extend(vec![0u8; 50]);
        action[12] = 1;
        std::fs::write(data.join("action.dat"), &action).unwrap();
        // broken gfxtype (wrong length)
        let mut gfx = Vec::new();
        gfx.extend([1u32, 2].iter().flat_map(|v| v.to_le_bytes()));
        gfx.extend(vec![0u8; 5]);
        std::fs::write(data.join("gfxtype.dat"), &gfx).unwrap();
        let (result, errors) = scan(dir.path()).unwrap();
        assert_eq!(errors, 1);
        let files = result.get("files").unwrap();
        assert_eq!(files.len(), 1);
        let first = match files {
            J::Arr(a) => &a[0],
            _ => unreachable!(),
        };
        assert_eq!(first.get("slots"), Some(&J::Int(2)));
        assert_eq!(first.get("leading_byte_histogram").unwrap().get("1"), Some(&J::Int(1)));
        assert_eq!(first.get("path").unwrap().as_str(), Some("MAPS/M1/DATA/action.dat").map(|s| s).or(first.get("path").unwrap().as_str()));
    }

    #[test]
    fn refuses_existing_or_inner_output() {
        let dir = TempDir::new();
        let inside = dir.path().join("report.json");
        assert!(new_output(&inside, &[dir.path().to_path_buf()], "source directory").is_err());
        let outside = TempDir::new();
        let ok = outside.path().join("report.json");
        assert!(new_output(&ok, &[dir.path().to_path_buf()], "source directory").is_ok());
        std::fs::write(&ok, b"x").unwrap();
        assert!(new_output(&ok, &[dir.path().to_path_buf()], "source directory").is_err());
    }
}
