//! `probe_object_pool_links.py`: action and hirarchy slot diagnostics for native map objects.

use super::Parsed;
use crate::jobj;
use crate::json::{Counter, J};
use crate::pools::{self, scan_links};
use crate::util::*;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub fn inspect_links(objects: &[u8], action: &[u8], hirarchy: &[u8]) -> R<J> {
    let objs = Objects::from_decoded(decode(objects)?, false, "unsupported objects layout")?;
    let action = decode(action)?;
    let hirarchy = decode(hirarchy)?;
    let count = objs.count;
    let action_count = pools::inspect(&action, "action")?.slots;
    req(hirarchy.len() >= 12, "short hirarchy header")?;
    let (version, groups, width) = (u32_at(&hirarchy, 0)?, u32_at(&hirarchy, 4)? as usize, u32_at(&hirarchy, 8)?);
    req(version == 1 && groups <= 3200 && width == 50, "unsupported hirarchy layout")?;
    req(hirarchy.len() == 12 + groups * 105, "hirarchy length mismatch")?;
    let active_objects: BTreeSet<usize> = (0..count).filter(|&i| objs.active(i)).collect();
    let active_action: BTreeSet<usize> = (0..action_count).filter(|&i| action[12 + i * 25] != 0).collect();
    let active_groups: BTreeSet<usize> = (0..groups).filter(|&i| hirarchy[12 + i * 103] != 0).collect();
    // Reader maps objects column 4 to runtime +0x28, the hirarchy backlink.
    let group_column = objs.column(4);
    let group_link = |slot: usize| -> R<i64> { Ok(i16_at(&objs.data, group_column + slot * 2)? as i64) };
    let mut action_used: BTreeMap<i64, u64> = BTreeMap::new();
    let mut issues = J::arr();
    let mut members: BTreeMap<usize, Vec<i64>> = BTreeMap::new();
    let mut group_links: BTreeMap<usize, i64> = BTreeMap::new();
    for &slot in &active_objects {
        let target = i16_at(&objs.data, 16 + slot * 79 + 77)? as i64;
        if target != -1 {
            if !(0..action_count as i64).contains(&target) {
                issues.push(jobj! { "kind" => "action-out-of-range", "object" => slot, "target" => target });
            } else {
                *action_used.entry(target).or_default() += 1;
                if !active_action.contains(&(target as usize)) {
                    issues.push(jobj! { "kind" => "action-inactive", "object" => slot, "target" => target });
                }
            }
        }
        let target = group_link(slot)?;
        if target != -1 {
            group_links.insert(slot, target);
            if !(0..groups as i64).contains(&target) {
                issues.push(jobj! { "kind" => "hirarchy-out-of-range", "object" => slot, "target" => target });
            } else if !active_groups.contains(&(target as usize)) {
                issues.push(jobj! { "kind" => "hirarchy-inactive", "object" => slot, "target" => target });
            }
        }
    }
    for &group in &active_groups {
        let offset = 12 + group * 103;
        let size = i16_at(&hirarchy, offset + 1)? as i64;
        if !(0..=50).contains(&size) {
            issues.push(jobj! { "kind" => "hirarchy-member-count", "group" => group, "count" => size });
            continue;
        }
        let values = (0..size as usize).map(|i| i16_at(&hirarchy, offset + 3 + i * 2).map(i64::from)).collect::<R<Vec<_>>>()?;
        for &slot in &values {
            if !(0..count as i64).contains(&slot) {
                issues.push(jobj! { "kind" => "member-out-of-range", "group" => group, "object" => slot });
            } else if !active_objects.contains(&(slot as usize)) {
                issues.push(jobj! { "kind" => "member-inactive", "group" => group, "object" => slot });
            } else if group_link(slot as usize)? != group as i64 {
                issues.push(jobj! {
                    "kind" => "member-backlink-mismatch", "group" => group, "object" => slot,
                    "backlink" => group_link(slot as usize)?
                });
            }
        }
        let unique: BTreeSet<_> = values.iter().collect();
        if unique.len() != values.len() {
            issues.push(jobj! { "kind" => "duplicate-members", "group" => group });
        }
        members.insert(group, values);
    }
    for (&slot, &group) in &group_links {
        if group >= 0 {
            if let Some(list) = members.get(&(group as usize)) {
                if !list.contains(&(slot as i64)) {
                    issues.push(jobj! { "kind" => "backlink-member-mismatch", "group" => group, "object" => slot });
                }
            }
        }
    }
    let mut histogram: BTreeMap<usize, u64> = BTreeMap::new();
    for list in members.values() {
        *histogram.entry(list.len()).or_default() += 1;
    }
    let without: Vec<usize> = active_action.iter().copied().filter(|i| !action_used.contains_key(&(*i as i64))).collect();
    Ok(jobj! {
        "decoded_sha256" => jobj! {
            "objects" => sha256_hex(&objs.data),
            "action" => sha256_hex(&action),
            "hirarchy" => sha256_hex(&hirarchy),
        },
        "active_objects" => active_objects.len(),
        "active_action" => active_action.len(),
        "active_hirarchy" => active_groups.len(),
        "action_linked_objects" => action_used.values().sum::<u64>(),
        "hirarchy_linked_objects" => group_links.len(),
        "shared_action_slots" => pools::sorted_counts(&action_used, 1),
        "active_action_without_object_link" => without,
        "hirarchy_member_count_histogram" => J::Obj(histogram.iter().map(|(k, v)| (k.to_string(), J::from(*v))).collect()),
        "issues" => issues,
    })
}

pub fn run(args: &[String]) -> R<i32> {
    let p = Parsed::parse(args, &[])?;
    p.exact(2, "probe-object-pool-links ROOT OUTPUT.json")?;
    let root = Path::new(&p.pos[0]);
    let output = new_output(Path::new(&p.pos[1]), &[resolve(root)], "source directory")?;
    req(root.join("MAPS").is_dir(), "missing MAPS directory")?;
    let scan = scan_links(root, &["objects.dat", "action.dat", "hirarchy.dat"], |s| inspect_links(&s[0], &s[1], &s[2]))?;
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
            "partial link diagnostics, not proof of corruption or runtime loading success",
            "no UID checks, other pools, tail-word semantics or action-word semantics",
            "strict writer version/capacity/length; PFIL compressed trailing integrity not checked",
            "no game execution, extraction or original file writes",
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

    fn action(count: usize, active: &[usize]) -> Vec<u8> {
        let mut data = u32s(&[1, count as u32, 6]);
        data.resize(12 + count * 25, 0);
        for &i in active {
            data[12 + i * 25] = 1;
        }
        data
    }

    fn hirarchy(groups: usize, active: &[(usize, Vec<i16>)]) -> Vec<u8> {
        let mut data = u32s(&[1, groups as u32, 50]);
        data.resize(12 + groups * 105, 0);
        for (group, list) in active {
            let base = 12 + group * 103;
            data[base] = 1;
            data[base + 1..base + 3].copy_from_slice(&(list.len() as i16).to_le_bytes());
            for (i, member) in list.iter().enumerate() {
                data[base + 3 + i * 2..base + 5 + i * 2].copy_from_slice(&member.to_le_bytes());
            }
        }
        data
    }

    #[test]
    fn finds_action_and_hirarchy_issues() {
        let mut objs = objects(3);
        for slot in 0..3 {
            obj_activate(&mut objs, slot, 1);
            obj_record_i16(&mut objs, slot, 77, -1);
        }
        obj_unlink_all(&mut objs, 3, 4);
        obj_record_i16(&mut objs, 0, 77, 0);
        obj_record_i16(&mut objs, 1, 77, 5);
        obj_column(&mut objs, 3, 4, 0, 0);
        obj_column(&mut objs, 3, 4, 1, 0);
        // group 0 lists objects 0 and 2 (2 has no backlink); object 1 links to group 0 but is not a member
        let report = inspect_links(&objs, &action(2, &[1]), &hirarchy(2, &[(0, vec![0, 2])])).unwrap();
        let kinds: Vec<String> = match report.get("issues").unwrap() {
            J::Arr(a) => a.iter().map(|i| i.get("kind").unwrap().as_str().unwrap().to_string()).collect(),
            _ => panic!(),
        };
        for expected in ["action-inactive", "action-out-of-range", "member-backlink-mismatch", "backlink-member-mismatch"] {
            assert!(kinds.contains(&expected.to_string()), "{expected} missing in {kinds:?}");
        }
        assert_eq!(report.get("hirarchy_linked_objects"), Some(&J::Int(2)));
    }

    #[test]
    fn rejects_wrong_hirarchy_header() {
        let objs = objects(1);
        let mut bad = hirarchy(1, &[]);
        bad[8] = 49;
        assert!(inspect_links(&objs, &action(1, &[]), &bad).is_err());
    }
}
