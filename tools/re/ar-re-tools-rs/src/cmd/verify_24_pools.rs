//! `probe_verify_all_24_pools.py`: decoded-size and version check of the 24 pools of every map.
//!
//! The Python script silently overwrote `re_workspace/all-24-pools-validation-report.json`;
//! here the report is only written with `--output FILE` and never over an existing file.

use super::Parsed;
use crate::jobj;
use crate::json::J;
use crate::pools::EXPECTED_SIZES;
use crate::util::*;
use std::path::{Path, PathBuf};

pub fn validate(maps_dir: &Path) -> R<J> {
    let (mut total_maps, mut checked, mut size_matches, mut version_matches) = (0u64, 0u64, 0u64, 0u64);
    let mut anomalies: Vec<String> = Vec::new();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(maps_dir)
        .map_err(|e| io(e, maps_dir))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort_by_key(|p| file_name(p).to_lowercase());
    for map_dir in dirs {
        let data_dir = map_dir.join("DATA");
        if !data_dir.is_dir() {
            continue;
        }
        total_maps += 1;
        let map = file_name(&map_dir);
        for (name, expected) in EXPECTED_SIZES {
            let path = data_dir.join(name);
            if !path.is_file() {
                anomalies.push(format!("Missing {name} in {map}"));
                continue;
            }
            checked += 1;
            let decoded = match read(&path).and_then(|raw| decode_unbounded(&raw)) {
                Ok((_, decoded)) => decoded,
                Err(error) => {
                    anomalies.push(format!("{map}/{name} decode failed: {error}"));
                    continue;
                }
            };
            if decoded.len() == expected {
                size_matches += 1;
            } else {
                anomalies.push(format!("{map}/{name} size mismatch: got {}, expected {expected}", decoded.len()));
            }
            if decoded.len() >= 4 {
                let version = u32_at(&decoded, 0)?;
                if version == 1 {
                    version_matches += 1;
                } else {
                    anomalies.push(format!("{map}/{name} unsupported version: {version}"));
                }
            }
        }
    }
    Ok(jobj! {
        "total_maps" => total_maps,
        "total_files_checked" => checked,
        "expected_total_files" => total_maps * 24,
        "size_matches" => size_matches,
        "version_matches" => version_matches,
        "anomaly_count" => anomalies.len(),
        "anomalies_sample" => anomalies.iter().take(10).cloned().collect::<Vec<_>>(),
    })
}

pub fn run(args: &[String]) -> R<i32> {
    let p = Parsed::parse(args, &["output"])?;
    if p.pos.len() > 1 {
        return Err(usage("verify-all-24-pools [MAPS_DIR] [--output FILE.json]"));
    }
    let maps_dir = p.pos.first().map(PathBuf::from).unwrap_or_else(|| installed_game().join("MAPS"));
    let output = match p.opt("output") {
        Some(path) => Some(new_output(Path::new(path), &[resolve(&maps_dir), installed_game()], "the maps directory")?),
        None => None,
    };
    let report = validate(&maps_dir)?;
    println!("{}", report.pretty(true));
    if let Some(output) = output {
        write_new(&output, &report.pretty(true))?;
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_support::*;

    #[test]
    fn counts_size_version_and_missing() {
        let dir = TempDir::new();
        let mut engine = vec![0u8; 94];
        engine[0] = 1;
        write(dir.path(), "M1/DATA/engine.dat", &engine);
        write(dir.path(), "M1/DATA/gametime.dat", &pfil_store(&vec![2u8; 31]));
        write(dir.path(), "NODATA/readme.txt", b"x");
        let report = validate(dir.path()).unwrap();
        assert_eq!(report.get("total_maps"), Some(&J::Int(1)));
        assert_eq!(report.get("total_files_checked"), Some(&J::Int(2)));
        assert_eq!(report.get("size_matches"), Some(&J::Int(1)));
        assert_eq!(report.get("version_matches"), Some(&J::Int(1)));
        // 22 missing + gametime size mismatch + gametime version 2
        assert_eq!(report.get("anomaly_count"), Some(&J::Int(24)));
    }
}
