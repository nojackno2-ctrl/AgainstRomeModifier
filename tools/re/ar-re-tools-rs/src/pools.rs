//! Native map pool structure checks shared by several probes (`probe_map_pools.inspect`).

use crate::json::J;
use crate::util::*;
use std::collections::BTreeMap;

pub struct PoolInfo {
    pub json: J,
    pub slots: usize,
    pub decoded: Vec<u8>,
}

/// Structural probe of an action / anim / gfxtype pool (raw or `PFIL@` wrapped).
pub fn inspect(data: &[u8], family: &str) -> R<PoolInfo> {
    let source_hash = sha256_hex(data);
    let wrapped = data.starts_with(b"PFIL@");
    let decoded = if wrapped {
        req(data.len() >= 64, "short PFIL header")?;
        let size = i32_at(data, 16)?;
        req((0..=4 * 1024 * 1024).contains(&size), "decoded pool exceeds analysis limit")?;
        pfil_prefix(data, size as usize)?.1
    } else {
        data.to_vec()
    };
    req(decoded.len() >= 8, "short pool header")?;
    let (version, count) = (u32_at(&decoded, 0)?, u32_at(&decoded, 4)?);
    req(version == 1, "unsupported pool version")?;
    // The native readers do not visibly enforce this capacity; the probe does.
    req(count <= 14000, "count exceeds native writer capacity")?;
    let count = count as usize;
    let (offset, stride, trailing) = match family {
        "action" => {
            req(decoded.len() >= 12, "short action header")?;
            req(u32_at(&decoded, 8)? == 6, "unsupported action word count")?;
            req(decoded.len() == 12 + count * 25, "action length does not match native layout")?;
            (12, 25, 0)
        }
        "anim" => {
            req(decoded.len() == 8 + count * 25, "anim length does not match native layout")?;
            // 21-byte records, then two independent count * uint16 arrays.
            (8, 21, count * 4)
        }
        "gfxtype" => {
            req(decoded.len() == 8 + count * 17, "gfxtype length does not match native layout")?;
            // 15-byte records (byte + seven uint16), then one count * uint16 array.
            (8, 15, count * 2)
        }
        _ => return Err("unsupported family".into()),
    };
    let mut leading: BTreeMap<u8, u64> = BTreeMap::new();
    for i in 0..count {
        *leading.entry(decoded[offset + i * stride]).or_default() += 1;
    }
    let histogram = J::Obj(leading.iter().map(|(k, v)| (k.to_string(), J::from(*v))).collect());
    let json = crate::jobj! {
        "family" => family,
        "source_sha256" => source_hash,
        "pfil_wrapped" => wrapped,
        "decoded_sha256" => sha256_hex(&decoded),
        "decoded_bytes" => decoded.len(),
        "version" => version,
        "slots" => count,
        "record_bytes" => stride,
        "leading_byte_histogram" => histogram,
        "trailing_array_bytes" => trailing,
    };
    Ok(PoolInfo { json, slots: count, decoded })
}

/// Counts occurrences (like `Counter(iterable)`), preserving the sorted-key view used by reports.
pub fn sorted_counts(values: &BTreeMap<i64, u64>, min: u64) -> J {
    J::Obj(values.iter().filter(|(_, n)| **n > min).map(|(k, n)| (k.to_string(), J::from(*n))).collect())
}

/// The 24 native `DATA/*.dat` pools in `probe_all_pool_structures.FILES_24` order.
pub const FILES_24: [&str; 24] = [
    "light.dat", "gametime.dat", "rain.dat", "hagel.dat", "snow.dat", "flash.dat", "objects.dat", "position.dat",
    "anim.dat", "gfxtype.dat", "action.dat", "objdata.dat", "hirarchy.dat", "formatio.dat", "lager.dat", "engine.dat",
    "fow.dat", "fowreq.dat", "way.dat", "particle.dat", "explos.dat", "hitex.dat", "stat.dat", "biglager.dat",
];

/// Exact decoded sizes of the 24 pools in standard maps (`probe_verify_all_24_pools.EXPECTED_SIZES`).
pub const EXPECTED_SIZES: [(&str, usize); 24] = [
    ("light.dat", 57352),
    ("gametime.dat", 32),
    ("rain.dat", 262164),
    ("hagel.dat", 229396),
    ("snow.dat", 65556),
    ("flash.dat", 23312),
    ("objects.dat", 1694016),
    ("position.dat", 561008),
    ("anim.dat", 350008),
    ("gfxtype.dat", 238008),
    ("action.dat", 350012),
    ("objdata.dat", 1722008),
    ("hirarchy.dat", 336012),
    ("formatio.dat", 350008),
    ("lager.dat", 144016),
    ("engine.dat", 94),
    ("fow.dat", 66053),
    ("fowreq.dat", 4616),
    ("way.dat", 791052),
    ("particle.dat", 2322444),
    ("explos.dat", 54008),
    ("hitex.dat", 40008),
    ("stat.dat", 6540),
    ("biglager.dat", 51244),
];

/// Result of scanning every map for a fixed set of pool files.
pub struct LinkScan {
    pub reports: Vec<J>,
    pub errors: Vec<J>,
    pub missing: Vec<String>,
}

/// Shared driver of the objects<->pool link audits: for each map directory that has all `files`,
/// reads them (bounded to the source root) and runs `inspect`; per-map failures become `errors`.
pub fn scan_links(root: &std::path::Path, files: &[&str], inspect: impl Fn(&[Vec<u8>]) -> R<J>) -> R<LinkScan> {
    scan_links_in(root, files, inspect, false)
}

/// Like [`scan_links`]; with `fail_fast` a per-map failure aborts instead of being recorded
/// (the behavior of the scripts that have no `errors` section).
pub fn scan_links_in(
    root: &std::path::Path,
    files: &[&str],
    inspect: impl Fn(&[Vec<u8>]) -> R<J>,
    fail_fast: bool,
) -> R<LinkScan> {
    let resolved_root = resolve(root);
    let mut scan = LinkScan { reports: Vec::new(), errors: Vec::new(), missing: Vec::new() };
    for directory in map_dirs(root)? {
        let name = file_name(&directory);
        let paths: Vec<_> = files.iter().map(|f| directory.join("DATA").join(f)).collect();
        if !paths.iter().all(|p| p.is_file()) {
            scan.missing.push(name);
            continue;
        }
        let outcome = (|| -> R<J> {
            req(paths.iter().all(|p| is_inside(&resolve(p), &resolved_root)), "pool path escapes source")?;
            let sources = paths.iter().map(|p| read(p)).collect::<R<Vec<_>>>()?;
            let mut entry = crate::jobj! { "map" => name.clone() };
            if let J::Obj(items) = inspect(&sources)? {
                for (k, v) in items {
                    entry.set(&k, v);
                }
            }
            Ok(entry)
        })();
        match outcome {
            Ok(entry) => scan.reports.push(entry),
            Err(error) if fail_fast => return Err(error),
            Err(error) => scan.errors.push(crate::jobj! { "map" => name, "error" => error }),
        }
    }
    Ok(scan)
}
