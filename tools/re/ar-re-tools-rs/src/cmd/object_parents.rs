//! `probe_native_object_parents.py`: proof and audit of objects column13 parent links.

use super::common::{self, Audit};
use crate::dis::{check_all, Code};
use crate::jobj;
use crate::json::{Counter, J};
use crate::util::*;
use std::collections::BTreeSet;
use std::path::Path;

pub fn inspect_code(exe: &[u8]) -> R<J> {
    require_exe(exe)?;
    let regions = [
        (0x48bae0, 1501),
        (0x4a9740, 0x609),
        (0x4c9790, 0x89),
        (0x4c9820, 0x1bb),
        (0x4c9db0, 0xb7),
        (0x4c9e70, 0x140),
        (0x4c9610, 0x17a),
        (0x4b37e7, 0x70),
        (0x4b3bc7, 0x70),
    ];
    let code = Code::regions(exe, &regions)?;
    let checks: [(u64, &str, &str); 30] = [
        (0x48bfbe, "add", "eax, 0x30"),
        (0x48bfc3, "call", "0x415160"),
        (0x4a9cb6, "call", "0x4c9590"),
        (0x4a9cbf, "call", "0x4c9790"),
        (0x4a9cc8, "call", "0x4c9db0"),
        (0x4a9cd1, "call", "0x4c9610"),
        (0x4a9cda, "call", "0x4c9820"),
        (0x4a9ce3, "call", "0x4c9e70"),
        (0x4a9cee, "call", "0x4e1470"),
        (0x4c97c8, "mov", "eax, dword ptr [eax + 0xc64b06]"),
        (0x4c97fa, "mov", "eax, dword ptr [esi + 0xa14c2a]"),
        (0x4c9808, "call", "0x4acb00"),
        (0x4c9884, "mov", "ebp, dword ptr [ebx + 0xc64b06]"),
        (0x4c989a, "mov", "eax, dword ptr [ebx + 0xc64b08]"),
        (0x4c98c6, "call", "0x4c84e0"),
        (0x4c9981, "call", "0x4a9740"),
        (0x4c99ba, "mov", "word ptr [esi + 0xa14c2c], ax"),
        (0x4c9e0e, "call", "0x4c9d80"),
        (0x4c9e3f, "mov", "eax, dword ptr [esi + 0xa14c2a]"),
        (0x4c9e4d, "call", "0x4acb00"),
        (0x4c9eef, "call", "0x4c7cf0"),
        (0x4c9f74, "call", "0x4a9740"),
        (0x4c9f9f, "mov", "word ptr [eax*4 + 0xa14c2c], di"),
        (0x4c966d, "mov", "ebp, dword ptr [ebx + 0xc64b02]"),
        (0x4c9677, "call", "0x4d2f50"),
        (0x4c9751, "call", "0x49f5f0"),
        (0x4b380f, "mov", "word ptr [edx + 0xc64b04], ax"),
        (0x4b3847, "mov", "word ptr [edx + 0xc64b06], ax"),
        (0x4b3bef, "mov", "word ptr [edx + 0xc64b08], ax"),
        (0x4b3c27, "mov", "word ptr [edx + 0xc64b0a], ax"),
    ];
    check_all(&code, &checks)?;
    let table = read_va(exe, 0x4b1130, 836)?;
    let cases: Vec<u32> = [162usize, 163, 180, 181].iter().map(|&c| u32_at(table, c * 4)).collect::<R<_>>()?;
    req(cases == [0x4b37e7, 0x4b381f, 0x4b3bc7, 0x4b3bff], "unexpected parser cases")?;
    // Reader array order includes the three dword columns; column13 is the fourteenth callback.
    let callbacks: Vec<u64> = code
        .list
        .iter()
        .filter(|i| (0x48bae0..0x48bae0 + 1501).contains(&i.address()) && i.mnemonic == "call" && ["0x415160", "0x4152f0"].contains(&i.operands.as_str()))
        .map(|i| i.address())
        .collect();
    req(callbacks.len() == 18 && callbacks[13] == 0x48bfc3, "unexpected serialized array order")?;
    Ok(jobj! {
        "exe_sha256" => EXE_SHA256,
        "instruction_assertions" => checks.len(),
        "column" => 13,
        "runtime_field" => "object+0x30",
        "reader_array_callbacks" => callbacks.iter().map(|a| format!("{a:#x}")).collect::<Vec<_>>(),
        "instructions" => code.lines(),
    })
}

pub fn audit_map(objects: &[u8]) -> R<(Counter, J)> {
    let objs = Objects::from_decoded(objects.to_vec(), true, "invalid objects layout")?;
    let offset = objs.column(13);
    let mut counts = Counter::new();
    let mut parents = BTreeSet::new();
    let mut links = J::arr();
    for slot in 0..objs.count {
        if !objs.active(slot) {
            continue;
        }
        let parent = u16_at(&objs.data, offset + slot * 2)?;
        if parent == 0xffff {
            continue;
        }
        parents.insert(parent);
        counts.inc("children");
        let status = if parent as usize >= objs.count {
            "out-of-range"
        } else if parent as usize == slot {
            "self"
        } else if !objs.active(parent as usize) {
            "inactive"
        } else {
            "valid"
        };
        counts.inc(status);
        links.push(jobj! {
            "child" => slot, "parent" => parent, "status" => status,
            "type" => u16_at(&objs.data, 16 + slot * 79 + 75)?,
        });
    }
    counts.set("parents", parents.len() as i128);
    Ok((counts, links))
}

pub fn inspect_maps(root: &Path) -> R<J> {
    let mut maps = J::arr();
    let mut total = Counter::new();
    let mut found = Vec::new();
    for dir in map_dirs(root).unwrap_or_default() {
        let path = dir.join("DATA").join("objects.dat");
        if path.is_file() {
            found.push(path);
        }
    }
    for path in found {
        req(is_inside(&resolve(&path), root), "source outside root")?;
        let packed = read(&path)?;
        let (counts, links) = audit_map(&decode(&packed)?)?;
        total.update(&counts);
        req(read(&path)? == packed, "source changed during audit")?;
        maps.push(jobj! {
            "map" => file_name(path.parent().and_then(Path::parent).unwrap()),
            "sha256" => sha256_hex(&packed),
            "counts" => counts.to_json(),
            "links" => links,
        });
    }
    Ok(jobj! {
        "maps" => maps,
        "totals" => total.to_json(),
        "limitations" => vec![
            "static code and disk snapshots; no game execution",
            "APT projection and effect pool transactions incomplete",
            "parent and child linkage proven; full attached-object creation remains unimplemented",
        ],
    })
}

pub const AUDIT: Audit = Audit {
    name: "probe-native-object-parents",
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

    #[test]
    fn classifies_parent_links() {
        let mut objs = objects(5);
        for slot in 0..4 {
            obj_activate(&mut objs, slot, 10 + slot as u16);
        }
        obj_unlink_all(&mut objs, 5, 13);
        obj_column(&mut objs, 5, 13, 0, 1); // valid
        obj_column(&mut objs, 5, 13, 1, 1); // self
        obj_column(&mut objs, 5, 13, 2, 4); // inactive parent (slot 4 not activated)
        obj_column(&mut objs, 5, 13, 3, 99); // out of range
        let (counts, links) = audit_map(&objs).unwrap();
        assert_eq!(counts.get("children"), 4);
        assert_eq!(counts.get("valid"), 1);
        assert_eq!(counts.get("self"), 1);
        assert_eq!(counts.get("inactive"), 1);
        assert_eq!(counts.get("out-of-range"), 1);
        assert_eq!(counts.get("parents"), 3);
        assert_eq!(links.len(), 4);
        match &links {
            J::Arr(a) => assert_eq!(a[2].get("type"), Some(&J::Int(12))),
            _ => unreachable!(),
        }
    }

    #[test]
    fn rejects_zero_count_and_bad_names() {
        let mut empty = objects(0);
        assert!(audit_map(&empty).is_err());
        let mut bad = objects(1);
        bad[8] = 29;
        assert!(audit_map(&bad).is_err());
        empty.clear();
        assert!(audit_map(&empty).is_err());
    }
}
