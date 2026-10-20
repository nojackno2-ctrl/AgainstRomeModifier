//! `probe_typus_creation_contract.py`: typus creation proof; saved field values are observations.

use super::common::{self, Audit};
use super::defaults::*;
use crate::dis::{check_all, Code};
use crate::jobj;
use crate::json::{Counter, J};
use crate::util::*;
use std::collections::BTreeMap;
use std::path::Path;

/// objdata.dat per-object column widths (sum 123).
pub const WIDTHS: [usize; 36] = [
    9, 8, 4, 4, 2, 16, 4, 4, 2, 2, 2, 2, 4, 2, 2, 2, 4, 4, 4, 4, 2, 4, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 4, 4, 2, 2,
];

const CHECKS: &[(u64, &str, &str)] = &[
    (0x480ca5, "call", "0x4d7b70"),
    (0x480dc7, "push", "0x5f3212"),
    (0x480dcc, "call", "0x4d7a20"),
    (0x480fe3, "push", "0x5f3394"),
    (0x480ff8, "call", "0x4d7a20"),
    (0x4810e8, "inc", "ebx"),
    (0x4810f1, "cmp", "ebx, 0xa"),
    (0x4810f4, "jl", "0x480f4c"),
    (0x4aff9b, "mov", "word ptr [ebx + 0xbe], 0xffff"),
    (0x4b2074, "mov", "edx, 0x5f5c99"),
    (0x4b2094, "mov", "word ptr [edx + 0xc6497a], ax"),
    (0x4a9a32, "mov", "word ptr [esp + 0x1c], si"),
    (0x4a9a41, "call", "0x4afd80"),
    (0x4a9a53, "mov", "esi, dword ptr [esi + 0xc64978]"),
    (0x4a9a59, "sar", "esi, 0x10"),
    (0x4a9a5d, "call", "0x4d7b20"),
    (0x4a9a67, "je", "0x4a9a88"),
    (0x4a9a77, "test", "byte ptr [eax*4 + 0x9f5031], 4"),
    (0x4a9a7f, "je", "0x4a9a88"),
    (0x4a9a81, "mov", "word ptr [esp + 0x1c], 0x64"),
    (0x4d784e, "jmp", "dword ptr [edx*4 + 0x4d7810]"),
    (0x4d7887, "push", "0x5f676c"),
    (0x4d78b3, "mov", "dword ptr [edx + 0x9f502c], eax"),
    (0x4d7913, "push", "0x5f6773"),
    (0x4d793f, "mov", "dword ptr [edx + 0x9f5030], eax"),
    (0x4d7a26, "push", "0x770de8"),
    (0x4d7a2b, "push", "0x5f677f"),
    (0x4d7a39, "call", "0x572f50"),
    (0x4d7a52, "push", "0x4d7830"),
    (0x4d7a5e, "push", "0x9c4"),
    (0x4d7a63, "push", "0x5f6786"),
    (0x4d7a69, "call", "0x42a230"),
    (0x4d7b26, "jl", "0x4d7b4e"),
    (0x4d7b28, "cmp", "edx, 0x9c4"),
    (0x4d7b3e, "cmp", "dword ptr [eax*4 + 0x9f502c], 0"),
    (0x4d7b74, "call", "0x4d7b90"),
    (0x4d7b7d, "cmp", "ebx, 0x9c4"),
    (0x4d7baa, "call", "0x4d7bd0"),
    (0x4d7be6, "mov", "dword ptr [ebx], 0"),
    (0x4d7bf6, "lea", "edx, [ebx + 0x10]"),
    (0x4d7bfc, "mov", "dword ptr [eax], 0"),
    (0x4d7c04, "jne", "0x4d7bf9"),
];

const STRINGS: &[(u64, &[u8])] = &[
    (0x5f3212, b"typus.dau"),
    (0x5f3394, b"typus.d%02ld"),
    (0x5f677f, b"%s%s"),
    (0x5f6786, b"[TypusDefaults]"),
    (0x5f676c, b"%ld"),
    (0x5f6773, b"%x"),
    (0x5f5c99, b"%ld"),
];

pub fn inspect_code(exe: &[u8]) -> R<J> {
    require_exe(exe)?;
    req(u32_at(read_va(exe, 0x4b1130, 836)?, 54 * 4)? == 0x4b206c, "unexpected typus case")?;
    let callbacks = read_va(exe, 0x4d7810, 28)?;
    let values: Vec<u32> = (0..7).map(|i| u32_at(callbacks, i * 4)).collect::<R<_>>()?;
    req(values == [0x4d7855, 0x4d787f, 0x4d78c1, 0x4d790b, 0x4d794d, 0x4d798f, 0x4d79d1], "unexpected typus callbacks")?;
    let regions = [
        (0x480850, 0x1000),
        (0x4afe10, 864),
        (0x4b206c, 0x37),
        (0x4a9740, 0x609),
        (0x4d7830, 0x1e3),
        (0x4d7a20, 0x5c),
        (0x4d7b20, 0x31),
        (0x4d7b70, 0x17),
        (0x4d7b90, 0x23),
        (0x4d7bd0, 0x3d),
    ];
    let code = Code::regions(exe, &regions)?;
    check_all(&code, CHECKS).map_err(|e| e.replace("instruction mismatch", "unexpected instruction"))?;
    for (address, expected) in STRINGS {
        req(read_va(exe, *address, 32)?.split(|&b| b == 0).next() == Some(*expected), "unexpected literal")?;
    }
    Ok(jobj! {
        "exe_sha256" => EXE_SHA256,
        "instruction_assertions" => CHECKS.len(),
        "literal_assertions" => STRINGS.len(),
        "instructions" => code.lines(),
    })
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub active: bool,
    pub index: i64,
    pub flags: i64,
}

impl Entry {
    fn new() -> Self {
        Self { active: false, index: -1, flags: 0 }
    }
}

pub type Table = BTreeMap<i64, Entry>;
pub struct Tables {
    pub objdef: Table,
    pub typus: Table,
}

/// Applies one definition file: objdef column 54 (type index) or typus column 3 (hex flags).
pub fn apply_table(table: &mut Table, text: &str, family: &str) -> R<()> {
    let heading = if family == "objdef" { "[objectdefaults]" } else { "[typusdefaults]" };
    for c in section_rows(text, heading) {
        let id = py_int(&c[0])?;
        req((0..2500).contains(&id), "invalid table index")?;
        let entry = table.entry(id).or_insert_with(Entry::new);
        if c.len() > 1 {
            entry.active = py_int(&c[1])? != 0;
        }
        if family == "objdef" && c.len() > 54 {
            entry.index = py_int(&c[54])?;
        }
        if family == "typus" && c.len() > 3 {
            entry.flags = py_hex(&c[3])?;
        }
    }
    Ok(())
}

/// For one map: observations of expected creation value vs the saved objdata field.
pub fn audit_map(objects: &[u8], objdata: &[u8], tables: &Tables) -> R<Counter> {
    let objs = Objects::from_decoded(decode(objects)?, true, "invalid objects")?;
    let data = decode(objdata)?;
    let count = objs.count;
    req(data.len() >= 8 && u32_at(&data, 0)? == 1 && u32_at(&data, 4)? as usize == count && data.len() == 8 + count * WIDTHS.iter().sum::<usize>(), "invalid objdata")?;
    let link_column = 16 + count * (79 + COLUMN_WIDTHS[0]);
    let mut counts = Counter::new();
    for slot in 0..count {
        let record = 16 + slot * 79;
        if objs.data[record] == 0 {
            continue;
        }
        let type_id = u16_at(&objs.data, record + 75)? as i64;
        let entry = tables.objdef.get(&type_id);
        let typus = entry.and_then(|e| tables.typus.get(&e.index));
        let expected = match (entry, typus) {
            (Some(e), Some(t)) if e.active && t.active && t.flags & 0x400 != 0 => 100,
            _ => 0,
        };
        let target = u16_at(&objs.data, link_column + slot * 2)? as usize;
        req(target < count, "invalid objdata link")?;
        let actual = u16_at(&data, 8 + count * WIDTHS[..4].iter().sum::<usize>() + target * 2)?;
        counts.inc(format!("create={expected},saved={actual}"));
    }
    Ok(counts)
}

pub fn inspect_maps(root: &Path) -> R<J> {
    let prefix = default_prefix(root)?;
    let mut tables = Tables { objdef: Table::new(), typus: Table::new() };
    let mut sources = J::arr();
    for family in ["objdef", "typus"] {
        for path in definition_files(root, &prefix, family)? {
            let packed = read(&path)?;
            let text = decode_cp1252(&decode(&packed)?)?;
            let table = if family == "objdef" { &mut tables.objdef } else { &mut tables.typus };
            apply_table(table, &text, family)?;
            req(read(&path)? == packed, "definition changed during audit")?;
            let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().into_owned();
            sources.push(jobj! { "file" => rel, "sha256" => sha256_hex(&packed) });
        }
    }
    let mut observations = Counter::new();
    let mut maps = J::arr();
    for dir in map_dirs(root).unwrap_or_default() {
        let path = dir.join("DATA").join("objects.dat");
        if !path.is_file() {
            continue;
        }
        let data_path = path.with_file_name("objdata.dat");
        req(is_inside(&resolve(&path), root) && is_inside(&resolve(&data_path), root), "map outside root")?;
        let (packed, data_packed) = (read(&path)?, read(&data_path)?);
        let counts = audit_map(&packed, &data_packed, &tables)?;
        req(read(&path)? == packed && read(&data_path)? == data_packed, "map changed during audit")?;
        observations.update(&counts);
        maps.push(jobj! {
            "map" => file_name(&dir),
            "observations" => counts.to_json(),
            "objects_sha256" => sha256_hex(&packed),
            "objdata_sha256" => sha256_hex(&data_packed),
        });
    }
    Ok(jobj! {
        "sources" => sources,
        "definitions" => tables.objdef.len(),
        "typus_records" => tables.typus.len(),
        "maps" => maps,
        "observations" => observations.to_json(),
        "active_objects" => observations.items().iter().map(|(_, v)| *v).sum::<i128>(),
    })
}

pub const AUDIT: Audit = Audit {
    name: "probe-typus-creation-contract",
    inspect_code,
    inspect_maps,
    summary_keys: &["instruction_assertions", "literal_assertions", "definitions", "typus_records", "active_objects", "observations"],
    extra_limitations: None,
};

pub fn run(args: &[String]) -> R<i32> {
    let audit = Audit {
        extra_limitations: Some(vec![
            "static code and disk observations; no runtime proof",
            "saved field may change after creation; differences are not validation errors",
            "loose DefaultPath layers only; VFS runtime selection unobserved",
            "field1c gameplay name and post-create consumers unverified",
        ]),
        ..AUDIT
    };
    common::run(&audit, args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_support::*;

    fn cells(width: usize, set: &[(usize, &str)]) -> String {
        let mut c = vec!["0".to_string(); width];
        for (i, v) in set {
            c[*i] = v.to_string();
        }
        c.join(",")
    }

    #[test]
    fn widths_sum_to_objdata_record() {
        assert_eq!(WIDTHS.iter().sum::<usize>(), 123);
    }

    #[test]
    fn builds_tables_and_expected_creation_value() {
        let mut tables = Tables { objdef: Table::new(), typus: Table::new() };
        // objdef type 7 -> typus index 3; typus 3 active with flag 0x400
        apply_table(&mut tables.objdef, &format!("[ObjectDefaults]\n{}\n", cells(55, &[(0, "7"), (1, "1"), (54, "3")])), "objdef").unwrap();
        apply_table(&mut tables.typus, "[TypusDefaults]\n3,1,0,400\n4,1,0,0\n", "typus").unwrap();
        assert_eq!(tables.typus[&3].flags, 0x400);

        let count = 2;
        let mut objs = objects(count);
        obj_activate(&mut objs, 0, 7);
        obj_activate(&mut objs, 1, 8); // unknown objdef
        // link column (index 1) -> objdata target slot
        let link = 16 + count * (79 + COLUMN_WIDTHS[0]);
        objs[link..link + 2].copy_from_slice(&0u16.to_le_bytes());
        objs[link + 2..link + 4].copy_from_slice(&1u16.to_le_bytes());
        let mut data = u32s(&[1, count as u32]);
        data.resize(8 + count * 123, 0);
        let field = 8 + count * (9 + 8 + 4 + 4);
        data[field..field + 2].copy_from_slice(&100u16.to_le_bytes()); // target 0 saved 100
        let counts = audit_map(&objs, &data, &tables).unwrap();
        assert_eq!(counts.get("create=100,saved=100"), 1);
        assert_eq!(counts.get("create=0,saved=0"), 1);
        // invalid target is an error
        objs[link..link + 2].copy_from_slice(&9u16.to_le_bytes());
        assert!(audit_map(&objs, &data, &tables).is_err());
    }

    #[test]
    fn rejects_wrong_objdata_shape() {
        let objs = objects(1);
        let tables = Tables { objdef: Table::new(), typus: Table::new() };
        assert!(audit_map(&objs, &u32s(&[1, 1]), &tables).is_err());
    }
}
