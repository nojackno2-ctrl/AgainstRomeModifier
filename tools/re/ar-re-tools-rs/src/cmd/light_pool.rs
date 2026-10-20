//! `probe_native_light_pool.py`: fingerprint-gated native light layout and object backlink audit.

use super::common::{self, Audit};
use crate::dis::{check_all, Code};
use crate::jobj;
use crate::json::{Counter, J};
use crate::util::*;
use std::path::Path;

pub const WIDTHS: [usize; 16] = [4, 4, 4, 4, 4, 4, 4, 4, 4, 2, 4, 4, 4, 2, 2, 2];

pub fn inspect_code(exe: &[u8]) -> R<J> {
    require_exe(exe)?;
    let regions = [(0x48ab70, 1350), (0x49f480, 0x50), (0x49f500, 0xd2), (0x49f700, 0xe0), (0x48bae0, 1501), (0x4aa780, 0x680), (0x4b230c, 56)];
    let code = Code::regions(exe, &regions)?;
    let reads: [u64; 16] = [
        0x48abee, 0x48ac30, 0x48ac80, 0x48acd2, 0x48ad24, 0x48ad74, 0x48adc6, 0x48ae18, 0x48ae68, 0x48aeba, 0x48aefd,
        0x48af4d, 0x48af8f, 0x48afd1, 0x48b014, 0x48b04f,
    ];
    let reader_types: [&str; 16] = [
        "0x414e90", "0x414f10", "0x414f10", "0x414f10", "0x414f10", "0x414f10", "0x414f10", "0x414f10", "0x414f10",
        "0x414e40", "0x414f10", "0x414e90", "0x414e90", "0x414e40", "0x414e40", "0x414e40",
    ];
    let mut checks: Vec<(u64, &str, &str)> = reads.iter().zip(reader_types).map(|(a, t)| (*a, "call", t)).collect();
    checks.extend([
        (0x49f502, "call", "0x49f480"),
        (0x49f515, "call", "0x49f700"),
        (0x48bf92, "add", "eax, 0x24"),
        (0x48bf97, "call", "0x415160"),
        (0x4aad9b, "mov", "word ptr [ebx + 0xa14c20], ax"),
        (0x4aadb9, "mov", "word ptr [eax*4 + 0x7f9f24], di"),
        (0x49f741, "mov", "word ptr [eax*4 + 0xa14c20], 0xffff"),
        (0x4b2334, "mov", "word ptr [edx + 0xc6492e], ax"),
        (0x4aad81, "push", "dword ptr [eax*4 + 0xb18848]"),
        (0x4aad88, "push", "dword ptr [eax*4 + 0xb18844]"),
        (0x4aad8f, "push", "dword ptr [eax*4 + 0xb18840]"),
        (0x4aad96, "call", "0x49f5f0"),
    ]);
    check_all(&code, &checks)?;
    let callbacks: Vec<u64> = code
        .list
        .iter()
        .filter(|i| (0x48bae0..0x48bae0 + 1501).contains(&i.address()) && i.mnemonic == "call" && ["0x415160", "0x4152f0"].contains(&i.operands.as_str()))
        .map(|i| i.address())
        .collect();
    req(callbacks.len() == 18 && callbacks[12] == 0x48bf97, "unexpected object column12 order")?;
    let table = read_va(exe, 0x4b1130, 836)?;
    req(u32_at(table, 66 * 4)? == 0x4b230c, "unexpected lidef callback")?;
    Ok(jobj! {
        "exe_sha256" => EXE_SHA256,
        "instruction_assertions" => checks.len(),
        "light_column_widths" => WIDTHS.to_vec(),
        "object_backlink_column" => 12,
        "instructions" => code.lines(),
    })
}

pub fn audit_map(light: &[u8], objects: &[u8]) -> R<(Counter, J)> {
    req(light.len() == 57352 && u32_at(light, 0)? == 1 && u32_at(light, 4)? == 1024, "unsupported light layout")?;
    let objs = Objects::from_decoded(objects.to_vec(), true, "unsupported objects layout")?;
    let backlink_offset = objs.column(12);
    let offsets: Vec<usize> = (0..16).map(|c| 8 + 1024 * WIDTHS[..c].iter().sum::<usize>()).collect();
    let mut counts = Counter::new();
    let mut links = J::arr();
    for slot in 0..1024usize {
        if u32_at(light, offsets[0] + slot * 4)? == 0 {
            continue;
        }
        counts.inc("active");
        let owner = i16_at(light, offsets[13] + slot * 2)? as i64;
        let parent = i16_at(light, offsets[14] + slot * 2)? as i64;
        let definition = i16_at(light, offsets[15] + slot * 2)? as i64;
        for (name, target) in [("owner", owner), ("parent", parent)] {
            let status = if target == -1 {
                "none"
            } else if !(0..objs.count as i64).contains(&target) {
                "out_of_range"
            } else if !objs.active(target as usize) {
                "inactive"
            } else {
                "valid"
            };
            counts.inc(format!("{name}_{status}"));
            if name == "owner" && status == "valid" {
                let back = i16_at(&objs.data, backlink_offset + target as usize * 2)? as i64;
                counts.inc(if back == slot as i64 { "backlink_match" } else { "backlink_mismatch" });
            }
        }
        links.push(jobj! { "light" => slot, "owner" => owner, "parent" => parent, "definition" => definition });
    }
    Ok((counts, links))
}

pub fn inspect_maps(root: &Path) -> R<J> {
    let mut maps = J::arr();
    let mut totals = Counter::new();
    let mut found = Vec::new();
    for dir in map_dirs(root).unwrap_or_default() {
        let path = dir.join("DATA").join("light.dat");
        if path.is_file() {
            found.push(path);
        }
    }
    for path in found {
        let objects_path = path.with_file_name("objects.dat");
        req(is_inside(&resolve(&path), root) && is_inside(&resolve(&objects_path), root), "source outside root")?;
        let sources = [read(&path)?, read(&objects_path)?];
        let (light, objects) = (decode(&sources[0])?, decode(&sources[1])?);
        let (counts, links) = audit_map(&light, &objects)?;
        req(read(&path)? == sources[0] && read(&objects_path)? == sources[1], "source changed during audit")?;
        totals.update(&counts);
        maps.push(jobj! {
            "map" => file_name(path.parent().and_then(Path::parent).unwrap()),
            "source_sha256" => jobj! { "light" => sha256_hex(&sources[0]), "objects" => sha256_hex(&sources[1]) },
            "counts" => counts.to_json(),
            "links" => links,
        });
    }
    req(!maps.is_empty(), "no source maps found")?;
    Ok(jobj! {
        "maps" => maps,
        "totals" => totals.to_json(),
        "limitations" => vec![
            "static code and saved maps only; no runtime acceptance",
            "batch light model is not a complete cross-pool object-family transaction",
        ],
    })
}

pub const AUDIT: Audit = Audit {
    name: "probe-native-light-pool",
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

    fn light_blob() -> Vec<u8> {
        let mut data = u32s(&[1, 1024]);
        data.resize(57352, 0);
        data
    }
    fn set_light(data: &mut [u8], column: usize, slot: usize, value: i16) {
        let at = 8 + 1024 * WIDTHS[..column].iter().sum::<usize>() + slot * WIDTHS[column];
        if WIDTHS[column] == 4 {
            data[at..at + 4].copy_from_slice(&(value as u32).to_le_bytes());
        } else {
            data[at..at + 2].copy_from_slice(&value.to_le_bytes());
        }
    }

    #[test]
    fn classifies_owner_parent_and_backlinks() {
        let mut objs = objects(3);
        obj_activate(&mut objs, 0, 1);
        obj_activate(&mut objs, 1, 1);
        obj_unlink_all(&mut objs, 3, 12);
        obj_column(&mut objs, 3, 12, 0, 0); // object 0 -> light 0
        let mut light = light_blob();
        set_light(&mut light, 0, 0, 1);
        set_light(&mut light, 13, 0, 0); // owner active object 0, backlink matches
        set_light(&mut light, 14, 0, -1);
        set_light(&mut light, 15, 0, 7);
        set_light(&mut light, 0, 1, 1);
        set_light(&mut light, 13, 1, 1); // owner object 1: backlink 0xffff != light 1
        set_light(&mut light, 14, 1, 2); // inactive object
        set_light(&mut light, 0, 2, 1);
        set_light(&mut light, 13, 2, 50); // out of range
        set_light(&mut light, 14, 2, -1);
        let (counts, links) = audit_map(&light, &objs).unwrap();
        assert_eq!(counts.get("active"), 3);
        assert_eq!(counts.get("owner_valid"), 2);
        assert_eq!(counts.get("backlink_match"), 1);
        assert_eq!(counts.get("backlink_mismatch"), 1);
        assert_eq!(counts.get("owner_out_of_range"), 1);
        assert_eq!(counts.get("parent_inactive"), 1);
        assert_eq!(counts.get("parent_none"), 2);
        assert_eq!(links.len(), 3);
    }

    #[test]
    fn rejects_wrong_light_layout() {
        assert!(audit_map(&light_blob()[..100], &objects(1)).is_err());
    }
}
