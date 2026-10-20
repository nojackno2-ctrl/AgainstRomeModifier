//! `probe_native_particle_pool.py`: native particle layout, object backlinks and child-light audit.

use super::common::{self, Audit};
use crate::dis::{check_all, Code};
use crate::jobj;
use crate::json::{Counter, J};
use crate::util::*;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const CAPACITY: usize = 1024;
pub const CHILDREN: usize = 64;
pub const RECORD: usize = 2252;
pub const LENGTH: usize = 12 + CAPACITY * (RECORD + 16);

pub fn inspect_code(exe: &[u8]) -> R<J> {
    require_exe(exe)?;
    let code = Code::regions(exe, &[(0x48dd90, 0x475), (0x4db800, 0x130)])?;
    let checks: [(u64, &str, &str); 21] = [
        (0x48de5e, "add", "ebp, 0x8e0"),
        (0x48deb4, "mov", "word ptr [ebp], ax"),
        (0x48dee0, "mov", "dword ptr [ebp + 4], eax"),
        (0x48df07, "mov", "word ptr [ebp + 8], ax"),
        (0x48df1f, "mov", "dword ptr [ebp + 0xc], eax"),
        (0x48df93, "mov", "byte ptr [edi + ebp + 0x1c], al"),
        (0x48df69, "mov", "word ptr [edi + 0x85c], ax"),
        (0x48de7f, "add", "eax, 0x10"),
        (0x48de84, "call", "0x415320"),
        (0x48e156, "add", "eax, 0x14"),
        (0x48e15b, "call", "0x4152f0"),
        (0x48e185, "add", "eax, 0x18"),
        (0x48e18a, "call", "0x4152f0"),
        (0x48e1b4, "add", "eax, 0x8dc"),
        (0x48e1bb, "call", "0x4152f0"),
        (0x4db80c, "cmp", "esi, 0x400"),
        (0x4db871, "mov", "ebp, 0x1e"),
        (0x4db876, "mov", "word ptr [eax + 0x12355dc], bx"),
        (0x4db87d, "mov", "dword ptr [eax + 0x1235eb0], ebp"),
        (0x4db8fc, "call", "0x49f700"),
        (0x4db923, "call", "0x4acb00"),
    ];
    let reads = code.calls_in(0x48de2e, 0x48e124, &["0x414df0", "0x414e40", "0x414e90"]);
    let mut expected = vec!["0x414e40", "0x414e90", "0x414e40", "0x414e90", "0x414df0"];
    expected.extend(["0x414e90"; 7]);
    expected.extend(["0x414e40"; 3]);
    req(reads == expected, "serialized read width order mismatch")?;
    check_all(&code, &checks)?;
    Ok(jobj! {
        "exe_sha256" => EXE_SHA256,
        "instruction_assertions" => checks.len(),
        "record_bytes" => RECORD,
        "child_bytes" => 35,
        "payload_bytes" => LENGTH,
        "parallel_runtime_offsets" => vec![0x10, 0x14, 0x18, 0x8dc],
        "instructions" => code.lines(),
    })
}

pub fn audit_map(particle: &[u8], objects: &[u8], light: &[u8]) -> R<(Counter, J)> {
    req(particle.len() == LENGTH && u32_at(particle, 0)? == 1 && u32_at(particle, 4)? == CAPACITY as u32 && u32_at(particle, 8)? == CHILDREN as u32, "unsupported particle layout")?;
    let objs = Objects::from_decoded(objects.to_vec(), true, "unsupported objects layout")?;
    req(light.len() == 57352 && u32_at(light, 0)? == 1 && u32_at(light, 4)? == 1024, "unsupported light layout")?;
    let backlink = objs.column(11);
    let mut counts = Counter::new();
    let mut links = J::arr();
    let mut owners: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
    let mut light_users: BTreeMap<i64, BTreeSet<usize>> = BTreeMap::new();
    for slot in 0..CAPACITY {
        let record = 12 + slot * RECORD;
        let active = u16_at(particle, record)?;
        if active == 0 {
            continue;
        }
        counts.inc("active_emitters");
        let owner = i16_at(particle, record + 6)? as i64;
        let status = if owner == -1 {
            "none"
        } else if !(0..objs.count as i64).contains(&owner) {
            "out_of_range"
        } else if !objs.active(owner as usize) {
            "inactive"
        } else {
            "valid"
        };
        counts.inc(format!("owner_{status}"));
        if status == "valid" {
            let back = u16_at(&objs.data, backlink + owner as usize * 2)?;
            counts.inc(if back as usize == slot { "backlink_match" } else { "backlink_mismatch" });
        }
        if owner >= 0 {
            owners.entry(owner).or_default().push(slot);
        }
        let mut children = J::arr();
        for child in 0..CHILDREN {
            let target = i16_at(particle, record + 12 + child * 35 + 33)? as i64;
            if target == -1 {
                continue;
            }
            let status = if !(0..1024).contains(&target) {
                "out_of_range"
            } else if u32_at(light, 8 + target as usize * 4)? != 0 {
                "active"
            } else {
                "inactive"
            };
            counts.inc(format!("child_light_{status}"));
            light_users.entry(target).or_default().insert(slot);
            children.push(jobj! { "child" => child, "light" => target, "status" => status });
        }
        links.push(jobj! { "slot" => slot, "active" => active, "owner" => owner, "children" => children });
    }
    for slot in 0..objs.count {
        if !objs.active(slot) {
            continue;
        }
        let target = u16_at(&objs.data, backlink + slot * 2)?;
        if target == 65535 {
            continue;
        }
        counts.inc("object_particle_references");
        let status = if target >= 1024 {
            "out_of_range"
        } else if u16_at(particle, 12 + target as usize * RECORD)? == 0 {
            "inactive"
        } else {
            "active"
        };
        counts.inc(format!("object_target_{status}"));
    }
    counts.set("shared_owner_objects", owners.values().filter(|v| v.len() > 1).count() as i128);
    counts.set("lights_shared_between_emitters", light_users.values().filter(|v| v.len() > 1).count() as i128);
    Ok((counts, links))
}

pub fn inspect_maps(root: &Path) -> R<J> {
    let mut maps = J::arr();
    let mut totals = Counter::new();
    let mut found = Vec::new();
    for dir in map_dirs(root).unwrap_or_default() {
        let path = dir.join("DATA").join("particle.dat");
        if path.is_file() {
            found.push(path);
        }
    }
    for path in found {
        let paths = [path.clone(), path.with_file_name("objects.dat"), path.with_file_name("light.dat")];
        req(paths.iter().all(|p| is_inside(&resolve(p), root)), "source outside root")?;
        let sources = paths.iter().map(|p| read(p)).collect::<R<Vec<_>>>()?;
        let decoded = sources.iter().map(|s| decode(s)).collect::<R<Vec<_>>>()?;
        let (counts, links) = audit_map(&decoded[0], &decoded[1], &decoded[2])?;
        for (p, s) in paths.iter().zip(&sources) {
            req(read(p)? == *s, "source changed during audit")?;
        }
        totals.update(&counts);
        let mut hashes = J::obj();
        for (p, s) in paths.iter().zip(&sources) {
            hashes.set(&file_name(p), sha256_hex(s));
        }
        maps.push(jobj! {
            "map" => file_name(path.parent().and_then(Path::parent).unwrap()),
            "counts" => counts.to_json(),
            "links" => links,
            "source_sha256" => hashes,
        });
    }
    req(!maps.is_empty(), "no source maps found")?;
    Ok(jobj! {
        "maps" => maps,
        "totals" => totals.to_json(),
        "limitations" => vec![
            "Static reader/reset and saved-map evidence; not particle simulation or game execution.",
            "Unknown child fields are retained, not given speculative semantic names.",
            "Layout verification does not prove complete create/copy or lager ownership closure.",
        ],
    })
}

pub const AUDIT: Audit = Audit {
    name: "probe-native-particle-pool",
    inspect_code,
    inspect_maps,
    summary_keys: &["instruction_assertions", "maps", "totals"],
    extra_limitations: None,
};

pub fn run(args: &[String]) -> R<i32> {
    common::run(&AUDIT, args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_support::*;

    fn particle_blob() -> Vec<u8> {
        let mut data = u32s(&[1, CAPACITY as u32, CHILDREN as u32]);
        data.resize(LENGTH, 0);
        data
    }
    fn light_blob(active: &[usize]) -> Vec<u8> {
        let mut data = u32s(&[1, 1024]);
        data.resize(57352, 0);
        for &i in active {
            data[8 + i * 4] = 1;
        }
        data
    }
    fn set_i16(data: &mut [u8], at: usize, v: i16) {
        data[at..at + 2].copy_from_slice(&v.to_le_bytes());
    }

    #[test]
    fn audits_owner_backlink_and_child_lights() {
        let mut objs = objects(2);
        obj_activate(&mut objs, 0, 1);
        obj_unlink_all(&mut objs, 2, 11);
        obj_column(&mut objs, 2, 11, 0, 3); // object 0 -> emitter 3
        let mut particle = particle_blob();
        for slot in [3usize, 5] {
            let record = 12 + slot * RECORD;
            set_i16(&mut particle, record, 1);
            set_i16(&mut particle, record + 6, 0); // both emitters claim object 0
            for child in 0..CHILDREN {
                set_i16(&mut particle, record + 12 + child * 35 + 33, -1);
            }
            set_i16(&mut particle, record + 12 + 33, 7); // child 0 -> light 7 (shared)
        }
        let (counts, links) = audit_map(&particle, &objs, &light_blob(&[7])).unwrap();
        assert_eq!(counts.get("active_emitters"), 2);
        assert_eq!(counts.get("owner_valid"), 2);
        assert_eq!(counts.get("backlink_match"), 1);
        assert_eq!(counts.get("backlink_mismatch"), 1);
        assert_eq!(counts.get("child_light_active"), 2);
        assert_eq!(counts.get("shared_owner_objects"), 1);
        assert_eq!(counts.get("lights_shared_between_emitters"), 1);
        assert_eq!(counts.get("object_particle_references"), 1);
        assert_eq!(counts.get("object_target_active"), 1);
        assert_eq!(links.len(), 2);
    }

    #[test]
    fn rejects_wrong_layout() {
        assert!(audit_map(&particle_blob()[..50], &objects(1), &light_blob(&[])).is_err());
    }

    #[test]
    fn layout_constant_matches_pool_size() {
        assert_eq!(LENGTH, 2322444);
    }
}
