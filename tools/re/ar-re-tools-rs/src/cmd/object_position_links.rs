//! `probe_object_position_links.py`: read-only three-position link audit of native DATA pools.

use super::Parsed;
use crate::dis::Code;
use crate::jobj;
use crate::json::{Counter, J};
use crate::pools::scan_links_in;
use crate::util::*;
use std::collections::BTreeMap;
use std::path::Path;

/// Fingerprint-gated instruction proof of the four native bodies behind the third position link.
pub fn inspect_native(exe: &[u8]) -> R<J> {
    require_exe(exe)?;
    let windows: [(&str, u64, usize); 4] = [
        ("create", 0x4aa780, 1703),
        ("reader", 0x48bae0, 1501),
        ("release", 0x4abb80, 822),
        ("objdata_scalar_consumer", 0x4bb5d0, 215),
    ];
    let mut decoded = Vec::new();
    for (name, address, size) in windows {
        decoded.push((name, Code::regions(exe, &[(address, size)])?));
    }
    let checks: [(&str, u64, &str, &str); 11] = [
        ("create", 0x4aa89d, "call", "0x4ab4a0"),
        ("create", 0x4aa8da, "call", "0x4ab4a0"),
        ("create", 0x4aa929, "cmp", "word ptr [ebx + 0xc64922], 0"),
        ("create", 0x4aa933, "call", "0x4ab4a0"),
        ("create", 0x4aa99a, "mov", "word ptr [eax*4 + 0xa14c0c], dx"),
        ("reader", 0x48bf3a, "add", "eax, 0x10"),
        ("reader", 0x48bf3f, "call", "0x415160"),
        ("release", 0x4abcd7, "call", "0x4abec0"),
        ("release", 0x4abce9, "call", "0x4abec0"),
        ("release", 0x4abcfb, "call", "0x4abec0"),
        ("objdata_scalar_consumer", 0x4bb661, "fld", "dword ptr [edx + 0xf5ffa8]"),
    ];
    for (name, address, mnemonic, operands) in checks {
        let code = &decoded.iter().find(|(n, _)| *n == name).unwrap().1;
        code.check(address, mnemonic, operands).map_err(|_| format!("native instruction mismatch at {address:#x}"))?;
    }
    let mut instructions = J::obj();
    for (name, code) in &decoded {
        instructions.set(name, code.lines());
    }
    Ok(jobj! {
        "exe_sha256" => EXE_SHA256,
        "instructions" => instructions,
        "limitations" => vec![
            "four static function bodies; no game execution",
            "column 10 mapping uses reader array order and established column widths",
            "archetype flag names and third-position runtime consumers remain unresolved",
        ],
    })
}

pub fn inspect_links(objects: &[u8], positions: &[u8]) -> R<J> {
    let objs = Objects::from_decoded(decode(objects)?, true, "unsupported objects/position layout")?;
    let positions = decode(positions)?;
    req(positions.len() >= 8, "short pool header")?;
    let (pv, pc) = (u32_at(&positions, 0)?, u32_at(&positions, 4)? as usize);
    req(pv == 1, "unsupported objects/position layout")?;
    req(0 < pc && pc <= 33000 && positions.len() == 8 + pc * 17, "position length mismatch")?;
    let third = objs.column(10);
    let mut uses: BTreeMap<u16, u64> = BTreeMap::new();
    let mut thirds = J::arr();
    let mut types = Counter::new();
    let (mut invalid, mut inactive) = (J::arr(), J::arr());
    for slot in 0..objs.count {
        let record = 16 + slot * 79;
        if objs.data[record] == 0 {
            continue;
        }
        let links = [u16_at(&objs.data, record + 67)?, u16_at(&objs.data, record + 69)?, u16_at(&objs.data, third + slot * 2)?];
        if links[2] != 0xffff {
            let type_id = u16_at(&objs.data, record + 75)?;
            thirds.push(jobj! { "object_slot" => slot, "position_slot" => links[2], "type" => type_id });
            types.inc(type_id.to_string());
        }
        for (link_index, target) in links.into_iter().enumerate() {
            if target == 0xffff {
                continue;
            }
            let item = jobj! { "object_slot" => slot, "link_index" => link_index, "position_slot" => target };
            if target as usize >= pc {
                invalid.push(item);
            } else {
                *uses.entry(target).or_default() += 1;
                if positions[8 + target as usize * 17] == 0 {
                    inactive.push(item);
                }
            }
        }
    }
    let shared = J::Obj(uses.iter().filter(|(_, v)| **v > 1).map(|(k, v)| (k.to_string(), J::from(*v))).collect());
    Ok(jobj! {
        "objects_decoded_sha256" => sha256_hex(&objs.data),
        "positions_decoded_sha256" => sha256_hex(&positions),
        "third_position_links" => thirds,
        "third_position_types" => types.to_json(),
        "out_of_range_links" => invalid,
        "inactive_target_links" => inactive,
        "shared_position_slots" => shared,
    })
}

pub fn run(args: &[String]) -> R<i32> {
    let p = Parsed::parse(args, &["exe"])?;
    p.exact(2, "probe-object-position-links ROOT OUTPUT.json [--exe EXE]")?;
    let root = resolve(Path::new(&p.pos[0]));
    let output = new_output(Path::new(&p.pos[1]), &[root.clone(), installed_game()], "source directory")?;
    req(root.join("MAPS").is_dir(), "missing MAPS directory")?;
    let native = match p.opt("exe") {
        Some(exe) => inspect_native(&read(Path::new(exe))?)?,
        None => J::Null,
    };
    let scan = scan_links_in(&root, &["objects.dat", "position.dat"], |s| inspect_links(&s[0], &s[1]), true)?;
    let sum = |key: &str| -> usize { scan.reports.iter().map(|m| m.get(key).map_or(0, J::len)).sum() };
    let summary = jobj! {
        "maps" => scan.reports.len(),
        "third_links" => sum("third_position_links"),
        "out_of_range" => sum("out_of_range_links"),
        "inactive" => sum("inactive_target_links"),
        "shared_slots" => sum("shared_position_slots"),
    };
    let result = jobj! {
        "maps" => J::Arr(scan.reports.clone()),
        "missing_pool_pairs" => scan.missing.clone(),
        "native" => native,
        "limitations" => vec![
            "snapshot of three DATA position links only",
            "third link is zero-based objects column 10, runtime +0x10",
            "shared slots are observations, not established corruption",
            "does not determine archetype requirements, runtime use or creation semantics",
        ],
    };
    write_new(&output, &(result.pretty(true) + "\n"))?;
    println!("{}", summary.line(true));
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_support::*;

    fn positions(count: usize, active: &[usize]) -> Vec<u8> {
        let mut data = u32s(&[1, count as u32]);
        data.resize(8 + count * 17, 0);
        for &i in active {
            data[8 + i * 17] = 1;
        }
        data
    }

    #[test]
    fn audits_three_links() {
        let mut objs = objects(2);
        obj_activate(&mut objs, 0, 77);
        obj_activate(&mut objs, 1, 5);
        obj_unlink_all(&mut objs, 2, 10);
        for slot in 0..2 {
            obj_record_i16(&mut objs, slot, 67, -1);
            obj_record_i16(&mut objs, slot, 69, -1);
        }
        obj_record_i16(&mut objs, 0, 67, 0);
        obj_record_i16(&mut objs, 1, 69, 0);
        obj_column(&mut objs, 2, 10, 0, 1);
        obj_column(&mut objs, 2, 10, 1, 9);
        let report = inspect_links(&objs, &positions(3, &[0])).unwrap();
        assert_eq!(report.get("third_position_links").unwrap().len(), 2);
        assert_eq!(report.get("third_position_types").unwrap().get("77"), Some(&J::Int(1)));
        assert_eq!(report.get("out_of_range_links").unwrap().len(), 1);
        assert_eq!(report.get("inactive_target_links").unwrap().len(), 1);
        assert_eq!(report.get("shared_position_slots").unwrap().get("0"), Some(&J::Int(2)));
    }

    #[test]
    fn native_gate_rejects_other_executables() {
        assert!(inspect_native(b"MZ not the game").unwrap_err().contains("fingerprint"));
    }
}
