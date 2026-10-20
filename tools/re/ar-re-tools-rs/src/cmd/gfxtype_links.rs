//! `probe_gfxtype_links.py`: objects record +73 -> native gfxtype slot diagnostics.

use super::Parsed;
use crate::jobj;
use crate::json::{Counter, J};
use crate::pools::{self, scan_links};
use crate::util::*;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub fn inspect_links(objects: &[u8], gfxtype: &[u8]) -> R<J> {
    let objects = decode(objects)?;
    let gfxtype = decode(gfxtype)?;
    let objs = Objects::from_decoded(objects, false, "unsupported objects layout")?;
    let gfx_count = pools::inspect(&gfxtype, "gfxtype")?.slots;
    let active: BTreeSet<usize> = (0..gfx_count).filter(|&i| gfxtype[8 + i * 15] != 0).collect();
    let mut used: BTreeMap<i64, u64> = BTreeMap::new();
    let mut issues = J::arr();
    let (mut active_objects, mut unlinked) = (0u64, 0u64);
    for slot in 0..objs.count {
        if !objs.active(slot) {
            continue;
        }
        active_objects += 1;
        let target = i16_at(&objs.data, 16 + slot * 79 + 73)? as i64;
        if target == -1 {
            unlinked += 1;
        } else if !(0..gfx_count as i64).contains(&target) {
            issues.push(jobj! { "kind" => "out-of-range", "object" => slot, "target" => target });
        } else {
            *used.entry(target).or_default() += 1;
            if !active.contains(&(target as usize)) {
                issues.push(jobj! { "kind" => "inactive-target", "object" => slot, "target" => target });
            }
        }
    }
    let unlinked_gfx: Vec<usize> = active.iter().copied().filter(|i| !used.contains_key(&(*i as i64))).collect();
    Ok(jobj! {
        "objects_decoded_sha256" => sha256_hex(&objs.data),
        "gfxtype_decoded_sha256" => sha256_hex(&gfxtype),
        "active_objects" => active_objects,
        "active_gfxtype" => active.len(),
        "unlinked_objects" => unlinked,
        "linked_objects" => used.values().sum::<u64>(),
        "shared_gfxtype_slots" => pools::sorted_counts(&used, 1),
        "active_gfxtype_without_object_link" => unlinked_gfx,
        "issues" => issues,
    })
}

pub fn run(args: &[String]) -> R<i32> {
    let p = Parsed::parse(args, &[])?;
    p.exact(2, "probe-gfxtype-links ROOT OUTPUT.json")?;
    let root = Path::new(&p.pos[0]);
    let output = new_output(Path::new(&p.pos[1]), &[resolve(root)], "source directory")?;
    req(root.join("MAPS").is_dir(), "missing MAPS directory")?;
    let scan = scan_links(root, &["objects.dat", "gfxtype.dat"], |s| inspect_links(&s[0], &s[1]))?;
    let mut kinds = Counter::new();
    for report in &scan.reports {
        if let Some(J::Arr(issues)) = report.get("issues") {
            for issue in issues {
                kinds.inc(issue.get("kind").and_then(J::as_str).unwrap_or(""));
            }
        }
    }
    let result = jobj! {
        "maps" => J::Arr(scan.reports.clone()),
        "errors" => J::Arr(scan.errors.clone()),
        "missing_pools" => scan.missing.clone(),
        "limitations" => vec![
            "one slot link only; not UID or whole-map consistency",
            "no gfxtype field semantics or runtime loading proof",
            "strict writer layout; PFIL trailing integrity not checked",
            "shared or unreferenced slots are observations, not proven corruption",
        ],
    };
    write_new(&output, &(result.pretty(true) + "\n"))?;
    println!("{}", jobj! { "maps" => scan.reports.len(), "errors" => scan.errors.len(), "issues" => kinds.to_json() }.line(true));
    Ok(if scan.errors.is_empty() { 0 } else { 1 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_support::*;

    fn gfxtype(count: usize, active: &[usize]) -> Vec<u8> {
        let mut data = u32s(&[1, count as u32]);
        data.resize(8 + count * 17, 0);
        for &i in active {
            data[8 + i * 15] = 1;
        }
        data
    }

    #[test]
    fn classifies_links() {
        let mut objs = objects(5);
        for (slot, target) in [(0usize, 0i16), (1, 0), (2, 1), (3, 9), (4, -1)] {
            obj_activate(&mut objs, slot, 1);
            obj_record_i16(&mut objs, slot, 73, target);
        }
        let report = inspect_links(&objs, &gfxtype(3, &[0, 2])).unwrap();
        assert_eq!(report.get("active_objects"), Some(&J::Int(5)));
        assert_eq!(report.get("unlinked_objects"), Some(&J::Int(1)));
        assert_eq!(report.get("linked_objects"), Some(&J::Int(3)));
        assert_eq!(report.get("shared_gfxtype_slots").unwrap().get("0"), Some(&J::Int(2)));
        assert_eq!(report.get("issues").unwrap().len(), 2);
        assert_eq!(report.get("active_gfxtype_without_object_link"), Some(&J::Arr(vec![J::Int(2)])));
    }

    #[test]
    fn pfil_wrapped_inputs_are_accepted() {
        let objs = pfil_store(&objects(2));
        let report = inspect_links(&objs, &pfil_store(&gfxtype(2, &[]))).unwrap();
        assert_eq!(report.get("active_objects"), Some(&J::Int(0)));
    }
}
