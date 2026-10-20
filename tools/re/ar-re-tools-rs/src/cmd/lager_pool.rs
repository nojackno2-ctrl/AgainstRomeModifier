//! `probe_native_lager_pool.py`: native lager/biglager layouts and object ownership frontier.

use super::common::{self, Audit};
use crate::dis::{check_all, Code};
use crate::jobj;
use crate::json::{Counter, J};
use crate::util::*;
use std::collections::BTreeMap;
use std::path::Path;

pub fn inspect_code(exe: &[u8]) -> R<J> {
    require_exe(exe)?;
    let regions = [(0x48d2d0, 0x2b5), (0x48d590, 0x140), (0x4ac3e0, 0xb2), (0x4ac4a0, 0x39), (0x4d4870, 0x17d), (0x415030, 0x130)];
    let code = Code::regions(exe, &regions)?;
    let checks: [(u64, &str, &str); 19] = [
        (0x48d3d4, "add", "edi, 0x30"),
        (0x48d432, "mov", "byte ptr [ebx], al"),
        (0x48d45c, "mov", "dword ptr [ebx + 4], eax"),
        (0x48d497, "mov", "dword ptr [ebx + 8], eax"),
        (0x48d4bf, "mov", "word ptr [ebx + 0xc], ax"),
        (0x48d4c8, "lea", "eax, [ebx + 0xe]"),
        (0x48d4d1, "call", "0x415160"),
        (0x48d514, "mov", "word ptr [edi + 0x1a], ax"),
        (0x48d41a, "mov", "word ptr [edi - 2], ax"),
        (0x4ac3ef, "cmp", "esi, 0xc80"),
        (0x4ac40b, "mov", "byte ptr [edx + 0x120fdd4], ah"),
        (0x4ac432, "mov", "word ptr [eax + 0x120fde0], cx"),
        (0x4ac456, "mov", "word ptr [eax + 0x120fdec], di"),
        (0x4ac46b, "call", "0x4ac4a0"),
        (0x4ac470, "mov", "word ptr [ebx + 0x120fe02], di"),
        (0x4ac4a9, "cmp", "edx, 0x20"),
        (0x4ac4ae, "imul", "edx, edx, 0x642"),
        (0x4ac4bb, "mov", "byte ptr [edx + 0x153a718], ah"),
        (0x4ac4cc, "mov", "word ptr [eax + 0x153a718], bx"),
    ];
    check_all(&code, &checks)?;
    Ok(jobj! {
        "exe_sha256" => EXE_SHA256,
        "instruction_assertions" => checks.len(),
        "lager" => jobj! { "header" => vec![1, 3200, 6, 10], "record_bytes" => 43, "tail_array_width" => 2 },
        "biglager" => jobj! { "header" => vec![1, 32, 800], "record_bytes" => 1601 },
        "instructions" => code.lines(),
    })
}

fn member_status(objs: &Objects, member: i64) -> &'static str {
    if !(0..objs.count as i64).contains(&member) {
        "out_of_range"
    } else if objs.data[16 + member as usize * 79] != 0 {
        "active"
    } else {
        "inactive"
    }
}

/// Audits one map's lager / biglager / objects triple (already decoded).
pub fn audit_map(lager: &[u8], big: &[u8], objects: &[u8]) -> R<(Counter, J, J)> {
    req(lager.len() == 144016 && u32_at(lager, 0)? == 1 && u32_at(lager, 4)? == 3200 && u32_at(lager, 8)? == 6 && u32_at(lager, 12)? == 10, "unsupported lager layout")?;
    req(big.len() == 51244 && u32_at(big, 0)? == 1 && u32_at(big, 4)? == 32 && u32_at(big, 8)? == 800, "unsupported biglager layout")?;
    let objs = Objects::from_decoded(objects.to_vec(), true, "unsupported objects layout")?;
    let offset = objs.column(5);
    let mut counts = Counter::new();
    let mut owners: BTreeMap<u16, Vec<usize>> = BTreeMap::new();
    let mut big_owners: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
    let mut links = J::arr();
    for slot in 0..objs.count {
        if !objs.active(slot) {
            continue;
        }
        let target = u16_at(&objs.data, offset + slot * 2)?;
        if target == 65535 {
            continue;
        }
        counts.inc("object_lager_references");
        owners.entry(target).or_default().push(slot);
        let status = if target >= 3200 {
            "out_of_range"
        } else if lager[16 + target as usize * 43] != 0 {
            "active"
        } else {
            "inactive"
        };
        counts.inc(format!("object_target_{status}"));
    }
    for slot in 0..3200usize {
        let record = 16 + slot * 43;
        if lager[record] == 0 {
            continue;
        }
        counts.inc("active_lager");
        let occupancy = i16_at(lager, record + 9)?;
        let mut members = Vec::new();
        for i in 0..10 {
            let m = i16_at(lager, record + 23 + i * 2)?;
            if m != -1 {
                members.push(m as i64);
            }
        }
        let target = i16_at(lager, 16 + 3200 * 43 + slot * 2)? as i64;
        counts.add("nonzero_occupancy", occupancy != 0);
        counts.add("unreferenced_lager", !owners.contains_key(&(slot as u16)));
        for &member in &members {
            counts.inc(format!("small_member_{}", member_status(&objs, member)));
        }
        if target != -1 {
            counts.inc("lager_big_references");
            big_owners.entry(target).or_default().push(slot);
            let status = if !(0..32).contains(&target) {
                "out_of_range"
            } else if big[12 + target as usize * 1601] != 0 {
                "active"
            } else {
                "inactive"
            };
            counts.inc(format!("big_target_{status}"));
        }
        links.push(jobj! {
            "lager" => slot,
            "owner_objects" => owners.get(&(slot as u16)).cloned().unwrap_or_default(),
            "occupancy" => occupancy,
            "small_members" => members,
            "biglager" => target,
        });
    }
    let mut big_links = J::arr();
    for slot in 0..32usize {
        let record = 12 + slot * 1601;
        if big[record] == 0 {
            continue;
        }
        counts.inc("active_biglager");
        counts.add("unreferenced_biglager", !big_owners.contains_key(&(slot as i64)));
        let mut members = Vec::new();
        for i in 0..800 {
            let m = i16_at(big, record + 1 + i * 2)?;
            if m != -1 {
                members.push(m as i64);
            }
        }
        for &member in &members {
            counts.inc(format!("big_member_{}", member_status(&objs, member)));
        }
        big_links.push(jobj! {
            "biglager" => slot,
            "lager_users" => big_owners.get(&(slot as i64)).cloned().unwrap_or_default(),
            "members" => members,
        });
    }
    counts.add("lager_shared_between_objects", owners.values().filter(|v| v.len() > 1).count() as i128);
    counts.add("biglager_shared_between_lagers", big_owners.values().filter(|v| v.len() > 1).count() as i128);
    Ok((counts, links, big_links))
}

pub fn inspect_maps(root: &Path) -> R<J> {
    let mut maps = J::arr();
    let mut totals = Counter::new();
    let mut found = Vec::new();
    for dir in crate::util::map_dirs(root).unwrap_or_default() {
        let path = dir.join("DATA").join("lager.dat");
        if path.is_file() {
            found.push(path);
        }
    }
    for path in found {
        let paths = [path.clone(), path.with_file_name("biglager.dat"), path.with_file_name("objects.dat")];
        req(paths.iter().all(|p| is_inside(&resolve(p), root)), "source outside root")?;
        let sources = paths.iter().map(|p| read(p)).collect::<R<Vec<_>>>()?;
        let decoded = sources.iter().map(|s| decode(s)).collect::<R<Vec<_>>>()?;
        let (counts, links, big_links) = audit_map(&decoded[0], &decoded[1], &decoded[2])?;
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
            "lager_links" => links,
            "biglager_links" => big_links,
            "source_sha256" => hashes,
        });
    }
    req(!maps.is_empty(), "no source maps found")?;
    Ok(jobj! {
        "maps" => maps,
        "totals" => totals.to_json(),
        "limitations" => vec![
            "Static layout/reset and saved-map evidence; not runtime acceptance.",
            "Member consumers use object slots; lifecycle, flags and all six auxiliary fields remain incomplete.",
            "Unreferenced active slots are reported; no automatic deletion or repair is inferred.",
        ],
    })
}

pub const AUDIT: Audit = Audit {
    name: "probe-native-lager-pool",
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

    fn lager_blob() -> Vec<u8> {
        let mut data = u32s(&[1, 3200, 6, 10]);
        data.resize(144016, 0);
        for slot in 0..3200 {
            let tail = 16 + 3200 * 43 + slot * 2;
            data[tail..tail + 2].copy_from_slice(&(-1i16).to_le_bytes());
        }
        data
    }
    fn big_blob() -> Vec<u8> {
        let mut data = u32s(&[1, 32, 800]);
        data.resize(51244, 0);
        data
    }
    fn set_i16(data: &mut [u8], at: usize, v: i16) {
        data[at..at + 2].copy_from_slice(&v.to_le_bytes());
    }

    #[test]
    fn audits_references_and_members() {
        let mut objs = objects(3);
        obj_activate(&mut objs, 0, 1);
        obj_activate(&mut objs, 1, 1);
        obj_unlink_all(&mut objs, 3, 5);
        obj_column(&mut objs, 3, 5, 0, 0); // -> active lager 0
        obj_column(&mut objs, 3, 5, 1, 4000); // out of range
        let mut lager = lager_blob();
        lager[16] = 1; // lager 0 active
        set_i16(&mut lager, 16 + 9, 3); // occupancy
        for i in 0..10 {
            set_i16(&mut lager, 16 + 23 + i * 2, -1);
        }
        set_i16(&mut lager, 16 + 23, 1); // member = active object 1
        set_i16(&mut lager, 16 + 25, 2); // member = inactive object 2
        set_i16(&mut lager, 16 + 3200 * 43, 0); // biglager 0
        let mut big = big_blob();
        big[12] = 1;
        for i in 0..800 {
            set_i16(&mut big, 12 + 1 + i * 2, -1);
        }
        set_i16(&mut big, 13, 99); // member out of range
        let (counts, links, big_links) = audit_map(&lager, &big, &objs).unwrap();
        assert_eq!(counts.get("object_lager_references"), 2);
        assert_eq!(counts.get("object_target_active"), 1);
        assert_eq!(counts.get("object_target_out_of_range"), 1);
        assert_eq!(counts.get("nonzero_occupancy"), 1);
        assert_eq!(counts.get("small_member_active"), 1);
        assert_eq!(counts.get("small_member_inactive"), 1);
        assert_eq!(counts.get("big_target_active"), 1);
        assert_eq!(counts.get("big_member_out_of_range"), 1);
        assert_eq!(counts.get("unreferenced_lager"), 0);
        assert_eq!(links.len(), 1);
        assert_eq!(big_links.len(), 1);
    }

    #[test]
    fn rejects_wrong_layouts() {
        let objs = objects(1);
        assert!(audit_map(&lager_blob()[..100], &big_blob(), &objs).is_err());
        assert!(audit_map(&lager_blob(), &big_blob()[..100], &objs).is_err());
    }

    #[test]
    fn code_gate_rejects_foreign_exe() {
        assert!(inspect_code(&[0u8; 64]).is_err());
    }
}
