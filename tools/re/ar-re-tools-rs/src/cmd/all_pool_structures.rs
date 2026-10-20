//! `probe_all_pool_structures.py`: header probe of the 24 native pools for a few sample maps.
//! Read-only; prints the JSON to stdout.

use super::Parsed;
use crate::json::J;
use crate::pools::FILES_24;
use crate::util::*;
use std::path::{Path, PathBuf};

pub const SAMPLE_MAPS: [&str; 4] = ["ENDL_000", "ENDL_005", "MP_001", "C1_M01"];

pub fn game_root_arg(p: &Parsed) -> PathBuf {
    p.pos.first().map(PathBuf::from).unwrap_or_else(installed_game)
}

pub fn analyze_map(maps_dir: &Path, map: &str) -> R<Option<J>> {
    let data_dir = maps_dir.join(map).join("DATA");
    if !data_dir.is_dir() {
        return Ok(None);
    }
    let mut summary = J::obj();
    for name in FILES_24 {
        let path = data_dir.join(name);
        if !path.is_file() {
            summary.set(name, crate::jobj! { "exists" => false });
            continue;
        }
        let raw = read(&path)?;
        let (is_pfil, decoded) = decode_unbounded(&raw)?;
        let mut info = crate::jobj! {
            "exists" => true,
            "is_pfil" => is_pfil,
            "raw_len" => raw.len(),
            "decoded_len" => decoded.len(),
        };
        for (i, key) in ["header_u32_0", "header_u32_1", "header_u32_2", "header_u32_3"].iter().enumerate() {
            if decoded.len() >= (i + 1) * 4 && (i >= 2 || decoded.len() >= 8) {
                info.set(key, u32_at(&decoded, i * 4)?);
            }
        }
        summary.set(name, info);
    }
    Ok(Some(summary))
}

pub fn run(args: &[String]) -> R<i32> {
    let p = Parsed::parse(args, &[])?;
    if p.pos.len() > 1 {
        return Err(usage("probe-all-pool-structures [GAME_ROOT]"));
    }
    let maps_dir = game_root_arg(&p).join("MAPS");
    let mut result = J::obj();
    for map in SAMPLE_MAPS {
        if let Some(report) = analyze_map(&maps_dir, map)? {
            result.set(map, report);
        }
    }
    println!("{}", result.pretty(true));
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_support::*;

    #[test]
    fn reports_headers_and_missing_files() {
        let dir = TempDir::new();
        write(dir.path(), "MAPS/MP_001/DATA/light.dat", &u32s(&[1, 1024, 7, 8, 9]));
        write(dir.path(), "MAPS/MP_001/DATA/engine.dat", &pfil_store(&u32s(&[5, 6])));
        let report = analyze_map(&dir.path().join("MAPS"), "MP_001").unwrap().unwrap();
        let light = report.get("light.dat").unwrap();
        assert_eq!(light.get("header_u32_3"), Some(&J::Int(8)));
        assert_eq!(light.get("decoded_len"), Some(&J::Int(20)));
        let engine = report.get("engine.dat").unwrap();
        assert_eq!(engine.get("is_pfil"), Some(&J::Bool(true)));
        assert_eq!(engine.get("header_u32_1"), Some(&J::Int(6)));
        assert!(engine.get("header_u32_2").is_none());
        assert_eq!(report.get("rain.dat").unwrap().get("exists"), Some(&J::Bool(false)));
        assert!(analyze_map(&dir.path().join("MAPS"), "ENDL_000").unwrap().is_none());
    }
}
