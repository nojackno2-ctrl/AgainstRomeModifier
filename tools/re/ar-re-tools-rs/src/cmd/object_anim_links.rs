//! `probe_object_anim_links.py`: objects -> animation slot consistency diagnostics.

use super::Parsed;
use crate::jobj;
use crate::json::J;
use crate::pools::{self, scan_links};
use crate::util::*;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub fn inspect_links(objects: &[u8], anim: &[u8]) -> R<J> {
    let objs = Objects::from_decoded(decode(objects)?, false, "unsupported objects layout")?;
    let anim = decode(anim)?;
    let anim_count = pools::inspect(&anim, "anim")?.slots;
    let active_anim: BTreeSet<usize> = (0..anim_count).filter(|&i| anim[8 + i * 21] != 0).collect();
    let mut used: BTreeMap<i64, u64> = BTreeMap::new();
    let (mut invalid, mut inactive) = (J::arr(), J::arr());
    let (mut active_objects, mut unlinked) = (0u64, 0u64);
    for slot in 0..objs.count {
        if !objs.active(slot) {
            continue;
        }
        active_objects += 1;
        // Native CHECK consumes this u16 as the signed high half of a packed word.
        let target = i16_at(&objs.data, 16 + slot * 79 + 71)? as i64;
        if target == -1 {
            unlinked += 1;
        } else if !(0..anim_count as i64).contains(&target) {
            invalid.push(jobj! { "object_slot" => slot, "anim_slot" => target });
        } else {
            *used.entry(target).or_default() += 1;
            if !active_anim.contains(&(target as usize)) {
                inactive.push(jobj! { "object_slot" => slot, "anim_slot" => target });
            }
        }
    }
    let without: Vec<usize> = active_anim.iter().copied().filter(|i| !used.contains_key(&(*i as i64))).collect();
    Ok(jobj! {
        "objects_decoded_sha256" => sha256_hex(&objs.data),
        "anim_decoded_sha256" => sha256_hex(&anim),
        "object_slots" => objs.count,
        "anim_slots" => anim_count,
        "active_objects" => active_objects,
        "active_anim_slots" => active_anim.len(),
        "unlinked_objects" => unlinked,
        "linked_objects" => used.values().sum::<u64>(),
        "out_of_range_links" => invalid,
        "inactive_target_links" => inactive,
        "shared_anim_slots" => pools::sorted_counts(&used, 1),
        "active_anim_without_active_object_link" => without,
    })
}

pub fn run(args: &[String]) -> R<i32> {
    let p = Parsed::parse(args, &[])?;
    p.exact(2, "probe-object-anim-links ROOT OUTPUT.json")?;
    let root = Path::new(&p.pos[0]);
    let output = new_output(Path::new(&p.pos[1]), &[resolve(root)], "source directory")?;
    req(root.join("MAPS").is_dir(), "missing MAPS directory")?;
    let scan = scan_links(root, &["objects.dat", "anim.dat"], |s| inspect_links(&s[0], &s[1]))?;
    let count = |key: &str| -> usize {
        scan.reports.iter().map(|m| m.get(key).map_or(0, J::len)).sum()
    };
    let (oor, inact) = (count("out_of_range_links"), count("inactive_target_links"));
    let result = jobj! {
        "maps" => J::Arr(scan.reports.clone()),
        "errors" => J::Arr(scan.errors.clone()),
        "missing_pool_pairs" => scan.missing.clone(),
        "limitations" => vec![
            "one objects record field and anim validity byte only; other pools and UID relations not checked",
            "shared slots and unreferenced active slots are observations, not proven corruption",
            "stricter version/capacity/length checks than native; PFIL trailing integrity not checked",
            "no game execution, original file writes or runtime success inference",
        ],
    };
    write_new(&output, &(result.pretty(true) + "\n"))?;
    println!(
        "{}",
        jobj! { "maps" => scan.reports.len(), "errors" => scan.errors.len(), "out_of_range_links" => oor, "inactive_target_links" => inact }.line(true)
    );
    Ok(if scan.errors.is_empty() { 0 } else { 1 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_support::*;

    fn anim(count: usize, active: &[usize]) -> Vec<u8> {
        let mut data = u32s(&[1, count as u32]);
        data.resize(8 + count * 25, 0);
        for &i in active {
            data[8 + i * 21] = 1;
        }
        data
    }

    #[test]
    fn reports_range_inactive_and_shared() {
        let mut objs = objects(4);
        for (slot, target) in [(0usize, 0i16), (1, 0), (2, 1), (3, 7)] {
            obj_activate(&mut objs, slot, 3);
            obj_record_i16(&mut objs, slot, 71, target);
        }
        let report = inspect_links(&objs, &anim(3, &[0, 2])).unwrap();
        assert_eq!(report.get("out_of_range_links").unwrap().len(), 1);
        assert_eq!(report.get("inactive_target_links").unwrap().len(), 1);
        assert_eq!(report.get("shared_anim_slots").unwrap().get("0"), Some(&J::Int(2)));
        assert_eq!(report.get("active_anim_without_active_object_link"), Some(&J::Arr(vec![J::Int(2)])));
        assert_eq!(report.get("anim_slots"), Some(&J::Int(3)));
    }

    #[test]
    fn rejects_bad_layouts() {
        let mut objs = objects(2);
        objs.pop();
        assert!(inspect_links(&objs, &anim(1, &[])).is_err());
        assert!(inspect_links(&objects(2), &anim(1, &[])[..20]).is_err());
    }
}
