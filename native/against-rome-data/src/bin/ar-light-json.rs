//! Read-only evidence CLI.
//!
//!   ar-light-json dump  <light.dat> [--all-slots]
//!   ar-light-json audit <install-root | MAPS-dir | light.dat>... [--out <new-file>]
//!
//! Files are only ever opened for reading. `--out` uses create-new and refuses existing files.

use against_rome_data::json;
use against_rome_data::sha256::sha256_hex;
use against_rome_data::{LightPool, Source, CAPACITY};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("dump") => dump(&args[1..]),
        Some("audit") => audit(&args[1..]),
        _ => Err("usage: ar-light-json dump <light.dat> [--all-slots] | audit <path>... [--out <new-file>]".into()),
    };
    match result {
        Ok(text) => println!("{text}"),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    }
}

fn dump(args: &[String]) -> Result<String, String> {
    let all = args.iter().any(|a| a == "--all-slots");
    let path = args.iter().find(|a| !a.starts_with("--")).ok_or("missing light.dat path")?;
    let bytes = fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let pool = LightPool::from_file_bytes(&bytes).map_err(|e| format!("{path}: {e}"))?;
    let slots: Vec<u32> = if all { (0..CAPACITY as u32).collect() } else { pool.active_slots().collect() };
    let items: Vec<String> =
        slots.iter().filter_map(|&s| pool.snapshot_any(s).ok()).map(|s| json::snapshot(&s)).collect();
    Ok(format!(
        "{{\"file\":{},\"file_sha256\":\"{}\",\"wrapped\":{},\"decoded_len\":{},\"decoded_sha256\":\"{}\",\"slots\":[{}]}}",
        json::string(path),
        sha256_hex(&bytes),
        matches!(pool.source(), Source::Pfil { .. }),
        pool.payload().len(),
        sha256_hex(pool.payload()),
        items.join(",")
    ))
}

fn collect_files(inputs: &[String]) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for input in inputs {
        let p = Path::new(input);
        if p.is_file() {
            files.push(p.to_path_buf());
            continue;
        }
        let maps = if p.join("MAPS").is_dir() { p.join("MAPS") } else { p.to_path_buf() };
        let rd = fs::read_dir(&maps).map_err(|e| format!("{}: {e}", maps.display()))?;
        for entry in rd.flatten() {
            let f = entry.path().join("DATA").join("light.dat");
            if f.is_file() {
                files.push(f);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn stamp(path: &Path) -> Result<(u64, u128), String> {
    let m = fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let t = m.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
    Ok((m.len(), t))
}

#[derive(Default)]
struct Totals {
    files: u32,
    decoded_ok: u32,
    hash_equal: u32,
    active: u32,
    non1_active: u32,
    unequal_radii: u32,
    nonfinite: u32,
    color_gt1: u32,
    inactive_not_pristine: u32,
    active_raw: BTreeMap<u32, u32>,
    types: BTreeMap<u16, u32>,
}

fn hist<K: std::fmt::Display>(m: &BTreeMap<K, u32>) -> String {
    let items: Vec<String> = m.iter().map(|(k, v)| format!("\"{k}\":{v}")).collect();
    format!("{{{}}}", items.join(","))
}

fn audit(args: &[String]) -> Result<String, String> {
    let mut inputs = Vec::new();
    let mut out: Option<&String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--out" {
            out = Some(it.next().ok_or("--out needs a path")?);
        } else {
            inputs.push(a.clone());
        }
    }
    if inputs.is_empty() {
        return Err("audit needs at least one path".into());
    }
    let files = collect_files(&inputs)?;
    if files.is_empty() {
        return Err("no light.dat found".into());
    }
    let mut t = Totals::default();
    let mut entries = Vec::new();
    for path in &files {
        t.files += 1;
        let map = path.parent().and_then(Path::parent).and_then(Path::file_name).map_or(String::new(), |n| {
            n.to_string_lossy().into_owned()
        });
        let stamp_before = stamp(path)?;
        let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let hash_before = sha256_hex(&bytes);
        let parsed = LightPool::from_file_bytes(&bytes);
        let detail;
        match &parsed {
            Err(e) => detail = format!("\"decode_error\":{},\"decode_error_code\":{}", json::string(&e.to_string()), e.code()),
            Ok(pool) => {
                t.decoded_ok += 1;
                let (mut active, mut non1, mut uneq, mut nonfin, mut gt1, mut pristine_bad) = (0u32, 0u32, 0u32, 0u32, 0u32, 0u32);
                let mut uneq_slots = Vec::new();
                let mut raws = BTreeMap::new();
                let mut types = BTreeMap::new();
                for slot in 0..CAPACITY as u32 {
                    let s = pool.snapshot_any(slot).unwrap();
                    if s.active_raw() == 0 {
                        let f = s.raw_fields();
                        let pristine = f[..50].iter().all(|&b| b == 0) && f[50..].iter().all(|&b| b == 0xFF);
                        if !pristine {
                            pristine_bad += 1;
                        }
                        continue;
                    }
                    active += 1;
                    *raws.entry(s.active_raw()).or_insert(0u32) += 1;
                    *types.entry(s.type_raw()).or_insert(0u32) += 1;
                    if s.active_raw() != 1 {
                        non1 += 1;
                    }
                    if s.effective_radius_bits() != s.baseline_radius_bits() {
                        uneq += 1;
                        if uneq_slots.len() < 8 {
                            uneq_slots.push(slot.to_string());
                        }
                    }
                    let vals = [s.position(), s.color(), [s.effective_radius(), s.baseline_radius(), s.type_parameter()]];
                    if vals.iter().flatten().any(|v| !v.is_finite()) {
                        nonfin += 1;
                    }
                    if s.color().iter().any(|&c| c > 1.0) {
                        gt1 += 1;
                    }
                }
                t.active += active;
                t.non1_active += non1;
                t.unequal_radii += uneq;
                t.nonfinite += nonfin;
                t.color_gt1 += gt1;
                t.inactive_not_pristine += pristine_bad;
                for (k, v) in &raws {
                    *t.active_raw.entry(*k).or_insert(0) += v;
                }
                for (k, v) in &types {
                    *t.types.entry(*k).or_insert(0) += v;
                }
                detail = format!(
                    concat!(
                        "\"wrapped\":{},\"decoded_len\":{},\"decoded_sha256\":\"{}\",\"active_count\":{},",
                        "\"active_raw_histogram\":{},\"non1_active_count\":{},\"unequal_radii_count\":{},",
                        "\"unequal_radii_first_slots\":[{}],\"nonfinite_active_count\":{},\"color_gt1_active_count\":{},",
                        "\"type_raw_histogram\":{},\"inactive_slots_not_pristine\":{}"
                    ),
                    matches!(pool.source(), Source::Pfil { .. }),
                    pool.payload().len(),
                    sha256_hex(pool.payload()),
                    active,
                    hist(&raws),
                    non1,
                    uneq,
                    uneq_slots.join(","),
                    nonfin,
                    gt1,
                    hist(&types),
                    pristine_bad
                );
            }
        }
        // Re-read after all parsing to prove the source was left untouched.
        let hash_after = sha256_hex(&fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?);
        let stamp_after = stamp(path)?;
        let equal = hash_before == hash_after && stamp_before == stamp_after;
        if equal {
            t.hash_equal += 1;
        }
        entries.push(format!(
            "{{\"map\":{},\"file_len\":{},\"sha256_before\":\"{}\",\"sha256_after\":\"{}\",\"unchanged\":{},{}}}",
            json::string(&map),
            stamp_before.0,
            hash_before,
            hash_after,
            equal,
            detail
        ));
    }
    let text = format!(
        concat!(
            "{{\"tool\":\"ar-light-json audit\",\"scope\":\"saved-state observation of installed files; ",
            "not game runtime, rendering or provenance evidence\",\"totals\":{{\"files\":{},\"decoded_ok\":{},",
            "\"unchanged_files\":{},\"active\":{},\"non1_active\":{},\"unequal_radii\":{},\"nonfinite_active\":{},",
            "\"color_gt1_active\":{},\"inactive_slots_not_pristine\":{},\"active_raw_histogram\":{},",
            "\"type_raw_histogram\":{}}},\"maps\":[{}]}}"
        ),
        t.files,
        t.decoded_ok,
        t.hash_equal,
        t.active,
        t.non1_active,
        t.unequal_radii,
        t.nonfinite,
        t.color_gt1,
        t.inactive_not_pristine,
        hist(&t.active_raw),
        hist(&t.types),
        entries.join(",")
    );
    if let Some(path) = out {
        let mut f = fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(|e| format!("{path}: {e}"))?;
        f.write_all(text.as_bytes()).map_err(|e| format!("{path}: {e}"))?;
    }
    Ok(text)
}
