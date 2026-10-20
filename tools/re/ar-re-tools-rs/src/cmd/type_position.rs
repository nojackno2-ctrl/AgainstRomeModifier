//! `probe_type_position_contract.py`: afstr parser/create proof and definition-to-position audit.

use super::common::{self, Audit};
use super::defaults::*;
use crate::dis::{check_all, Code};
use crate::jobj;
use crate::json::{Counter, J};
use crate::util::*;
use std::path::Path;

const CHECKS: &[(u64, &str, &str)] = &[
    (0x4b149a, "cmp", "edx, 0xd0"),
    (0x4b14a2, "jmp", "dword ptr [edx*4 + 0x4b1130]"),
    (0x4b21cb, "call", "0x567320"),
    (0x4b21d0, "imul", "edx, dword ptr [0x1fc933c], 0x2a4"),
    (0x4b21e4, "mov", "word ptr [edx + 0xc64922], ax"),
    (0x4aa917, "call", "0x4afd80"),
    (0x4aa921, "je", "0x4aa96c"),
    (0x4aa923, "imul", "ebx, ebx, 0x2a4"),
    (0x4aa929, "cmp", "word ptr [ebx + 0xc64922], 0"),
    (0x4aa931, "je", "0x4aa96c"),
    (0x4aa933, "call", "0x4ab4a0"),
    (0x4aa93e, "mov", "eax, dword ptr [esp + 0x34]"),
    (0x4aa942, "push", "eax"),
    (0x4aa944, "call", "0x4aae40"),
    (0x47eaa6, "push", "0x5f29ad"),
    (0x47eac2, "push", "0x770de8"),
    (0x47eac9, "call", "0x414cf0"),
    (0x4b4206, "push", "0x770de8"),
    (0x4b420b, "push", "0x5f5eda"),
    (0x4b4219, "call", "0x572f50"),
    (0x4b4232, "push", "0x4b1480"),
    (0x4b4246, "push", "0x5f5ee1"),
    (0x4b424c, "call", "0x42a230"),
    (0x480c91, "call", "0x4afdc0"),
    (0x480d1b, "push", "0x5f3187"),
    (0x480d20, "call", "0x4b4200"),
    (0x480f42, "xor", "ebx, ebx"),
    (0x480f6b, "push", "0x5f3359"),
    (0x480f75, "call", "0x5c5de0"),
    (0x480f80, "call", "0x4b4200"),
    (0x4810e8, "inc", "ebx"),
    (0x4810f1, "cmp", "ebx, 0xa"),
    (0x4810f4, "jl", "0x480f4c"),
    (0x4afec2, "mov", "dword ptr [ebx + 0x64], 0"),
    (0x4aff39, "mov", "byte ptr [ebx], 0"),
    (0x567df7, "je", "0x567e05"),
    (0x567dfb, "je", "0x567e10"),
    (0x567dff, "jmp", "0x567de6"),
    (0x567e1c, "mov", "edi, eax"),
    (0x42a2ae, "call", "0x567d50"),
    (0x42a2bc, "jle", "0x42a2fd"),
    (0x42a2df, "call", "ebp"),
    (0x42a2f5, "inc", "ebx"),
    (0x4b18c9, "mov", "edx, 0x5f5c0f"),
    (0x4b18e9, "mov", "dword ptr [edx + 0xc648e8], eax"),
    (0x4b1938, "mov", "edx, 0x5f5c17"),
    (0x4b1958, "mov", "dword ptr [edx + 0xc648f0], eax"),
    (0x4a99d5, "fild", "dword ptr [eax + 0xc648e8]"),
    (0x4a99db, "fstp", "dword ptr [esp + 4]"),
    (0x4a99df, "mov", "esi, 0x3f800000"),
    (0x4a99e5, "mov", "dword ptr [esp + 0xc], esi"),
    (0x4a99ff, "fild", "dword ptr [eax + 0xc648f0]"),
    (0x4a9a05, "fstp", "dword ptr [esp + 0xc]"),
    (0x4a9a09, "mov", "ebp, 0x3f800000"),
    (0x4a9a0f, "mov", "dword ptr [esp + 0x14], ebp"),
    (0x4afe63, "mov", "dword ptr [ebx + 0x2c], 0"),
    (0x4afe70, "mov", "dword ptr [ebx + 0x34], 0"),
    (0x4b1582, "mov", "edx, 0x5f5bd4"),
    (0x4b15a2, "mov", "dword ptr [edx + 0xc648c8], eax"),
    (0x4b35c3, "mov", "edx, 0x5f5df9"),
    (0x4b35e3, "mov", "dword ptr [edx + 0xc64ae8], eax"),
    (0x4a9a25, "mov", "eax, dword ptr [eax + 0xc648c8]"),
    (0x4a9a2b, "mov", "dword ptr [esp + 0x14], eax"),
    (0x4a9af8, "mov", "eax, dword ptr [eax + 0xc64ae8]"),
    (0x4a9afe, "mov", "dword ptr [esp + 0x4c], eax"),
    (0x4afe2b, "mov", "dword ptr [ebx + 0xc], 0"),
    (0x4b0077, "mov", "dword ptr [ebx + 0x22c], 0"),
    (0x4bb661, "fld", "dword ptr [edx + 0xf5ffa8]"),
    (0x4bb66d, "jae", "0x4bb623"),
    (0x4bb68b, "xor", "ecx, ecx"),
    (0x4bb693, "fmul", "dword ptr [0x771c68]"),
];

const STRINGS: &[(u64, &[u8])] = &[
    (0x5f3359, b"objdef.d%02ld"),
    (0x5f3187, b"objdef.dau"),
    (0x5f29ad, b"[DefaultPath]"),
    (0x5f5c0f, b"%ld"),
    (0x5f5c17, b"%ld"),
    (0x5f5bd4, b"%f"),
    (0x5f5df9, b"%ld"),
];

pub fn inspect_code(exe: &[u8]) -> R<J> {
    require_exe(exe)?;
    let table = read_va(exe, 0x4b1130, 209 * 4)?;
    let case = |i: usize| u32_at(table, i * 4);
    req(case(60)? == 0x4b21bc, "unexpected afstr parser case")?;
    req(case(19)? == 0x4b18c1 && case(21)? == 0x4b1930, "unexpected vitals parser cases")?;
    req(case(4)? == 0x4b157a && case(152)? == 0x4b35bb, "unexpected scalar parser cases")?;
    let regions = [
        (0x4aa780, 1703),
        (0x4b1480, 0x29),
        (0x4b21bc, 0x38),
        (0x480850, 0x1000),
        (0x47e850, 890),
        (0x4b4200, 95),
        (0x4afe10, 864),
        (0x567d50, 0xd2),
        (0x42a230, 0xfa),
        (0x4b18c1, 0xa6),
        (0x4a9740, 0x609),
        (0x4b157a, 0x37),
        (0x4b35bb, 0x37),
        (0x4bb5d0, 0xd7),
    ];
    let code = Code::regions(exe, &regions)?;
    check_all(&code, CHECKS).map_err(|e| e.replace("instruction mismatch", "unexpected instruction at"))?;
    for (address, expected) in STRINGS {
        let text = read_va(exe, *address, 32)?;
        req(text.split(|&b| b == 0).next() == Some(*expected), "unexpected source string")?;
    }
    Ok(jobj! {
        "exe_sha256" => EXE_SHA256,
        "instruction_assertions" => CHECKS.len(),
        "parser_case" => 60,
        "runtime_field" => "0xc64922 + type * 0x2a4",
        "instructions" => code.lines(),
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct Definition {
    pub active: bool,
    pub afstr: i64,
    pub initial_hp: i64,
    pub initial_morale: i64,
    pub initial_moves: f64,
    pub initial_maxruhm: i64,
}

impl Definition {
    fn new() -> Self {
        Self { active: false, afstr: 0, initial_hp: 0, initial_morale: 0, initial_moves: 0.0, initial_maxruhm: 0 }
    }
    fn to_json(&self) -> J {
        jobj! {
            "active" => self.active,
            "afstr" => self.afstr,
            "initial_hp" => self.initial_hp,
            "initial_morale" => self.initial_morale,
            "initial_moves" => self.initial_moves,
            "initial_maxruhm" => self.initial_maxruhm,
        }
    }
}

/// Insertion-ordered type id -> effective definition after layering the `objdef` files.
pub type Definitions = Vec<(i64, Definition)>;

/// Applies the `[ObjectDefaults]` rows of one file (later files override earlier fields).
pub fn apply_objdef(definitions: &mut Definitions, text: &str) -> R<()> {
    for columns in section_rows(text, "[objectdefaults]") {
        let type_id = py_int(&columns[0])?;
        req((0..2500).contains(&type_id), "invalid type")?;
        let position = definitions.iter().position(|(id, _)| *id == type_id);
        let mut entry = position.map_or_else(Definition::new, |i| definitions[i].1.clone());
        if columns.len() > 1 {
            let flag = py_int(&columns[1])?;
            req((0..=255).contains(&flag), "invalid active flag")?;
            entry.active = flag != 0;
        }
        if columns.len() > 60 {
            let afstr = py_int(&columns[60])?;
            req((-32768..=32767).contains(&afstr), "invalid afstr")?;
            entry.afstr = afstr;
        }
        for (column, which) in [(19usize, 0), (21, 1), (152, 2)] {
            if columns.len() > column {
                let value = py_int(&columns[column])?;
                req((-2147483648..=2147483647).contains(&value), "invalid signed32 vital")?;
                match which {
                    0 => entry.initial_hp = value,
                    1 => entry.initial_morale = value,
                    _ => entry.initial_maxruhm = value,
                }
            }
        }
        if columns.len() > 4 {
            let value: f64 = columns[4].trim().parse().map_err(|_| format!("could not convert string to float: '{}'", columns[4]))?;
            req(value.is_finite(), "nonfinite moves")?;
            let value = value as f32 as f64;
            req(value.is_finite(), "float32 moves overflow")?;
            entry.initial_moves = value;
        }
        match position {
            Some(i) => definitions[i].1 = entry,
            None => definitions.push((type_id, entry)),
        }
    }
    Ok(())
}

/// For one map: (counts, mismatches) of afstr requirement vs the third position link.
pub fn audit_objects(objects: &[u8], definitions: &Definitions) -> R<(Counter, J)> {
    let objs = Objects::from_decoded(decode(objects)?, true, "invalid objects layout")?;
    let column10 = objs.column(10);
    let mut counts = Counter::new();
    let mut mismatches = J::arr();
    for slot in 0..objs.count {
        let record = 16 + slot * 79;
        if objs.data[record] == 0 {
            continue;
        }
        let type_id = u16_at(&objs.data, record + 75)?;
        let third = u16_at(&objs.data, column10 + slot * 2)?;
        let definition = definitions.iter().find(|(id, _)| *id == type_id as i64).map(|(_, d)| d);
        match definition {
            Some(d) if d.active => {
                let required = d.afstr != 0;
                counts.inc(if required { "requires-third" } else { "two-positions" });
                if required != (third != 0xffff) {
                    mismatches.push(jobj! { "slot" => slot, "type" => type_id, "third" => third, "afstr" => d.afstr });
                }
            }
            _ => mismatches.push(jobj! { "slot" => slot, "type" => type_id, "reason" => "invalid-type" }),
        }
    }
    Ok((counts, mismatches))
}

pub fn inspect_maps(root: &Path) -> R<J> {
    let prefix = default_prefix(root)?;
    let mut definitions: Definitions = Vec::new();
    let mut sources = J::arr();
    for source in definition_files(root, &prefix, "objdef")? {
        let raw = read(&source)?;
        let text = decode_cp1252(&decode(&raw)?)?;
        apply_objdef(&mut definitions, &text)?;
        req(read(&source)? == raw, "source changed during audit")?;
        let rel = source.strip_prefix(root).unwrap_or(&source).to_string_lossy().into_owned();
        sources.push(jobj! { "file" => rel, "sha256" => sha256_hex(&raw) });
    }
    req(!definitions.is_empty(), "no effective definitions")?;
    let mut maps = J::arr();
    let (mut mismatch_count, mut active_objects) = (0usize, 0i128);
    for dir in map_dirs(root).unwrap_or_default() {
        let path = dir.join("DATA").join("objects.dat");
        if !path.is_file() {
            continue;
        }
        let packed = read(&path)?;
        let (counts, mismatches) = audit_objects(&packed, &definitions)?;
        req(read(&path)? == packed, "source changed during audit")?;
        mismatch_count += mismatches.len();
        active_objects += counts.items().iter().map(|(_, v)| *v).sum::<i128>();
        maps.push(jobj! {
            "map" => file_name(&dir),
            "source_sha256" => sha256_hex(&packed),
            "counts" => counts.to_json(),
            "mismatches" => mismatches,
        });
    }
    let third_defs = J::Obj(
        definitions.iter().filter(|(_, d)| d.active && d.afstr != 0).map(|(id, d)| (id.to_string(), d.to_json())).collect(),
    );
    Ok(jobj! {
        "definition_sources" => sources,
        "definitions" => definitions.len(),
        "third_position_definitions" => third_defs,
        "maps" => maps,
        "mismatch_count" => mismatch_count,
        "active_objects" => active_objects,
    })
}

pub const AUDIT: Audit = Audit {
    name: "probe-type-position-contract",
    inspect_code,
    inspect_maps,
    summary_keys: &["exe_sha256", "instruction_assertions", "definitions", "active_objects", "mismatch_count"],
    extra_limitations: Some(Vec::new()),
};

pub fn limitations() -> Vec<&'static str> {
    vec![
        "static code and disk snapshots; no runtime execution",
        "configured loose DefaultPath and objdef.d00..d09 only; virtual directory policies/archives and runtime selection unobserved",
        "afstr nonzero controls third-position allocation; its full gameplay semantics are unknown",
        "other type-dependent object and auxiliary initialization remains incomplete",
    ]
}

pub fn run(args: &[String]) -> R<i32> {
    let audit = Audit { extra_limitations: Some(limitations()), ..AUDIT };
    common::run(&audit, args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_support::*;

    fn row(columns: &[(usize, &str)]) -> String {
        let width = columns.iter().map(|(i, _)| *i).max().unwrap() + 1;
        let mut cells = vec!["0".to_string(); width];
        for (i, v) in columns {
            cells[*i] = v.to_string();
        }
        cells.join(",")
    }

    #[test]
    fn layers_definitions_and_parses_vitals() {
        let first = format!("[ObjectDefaults]\n{}\n{}\n", row(&[(0, "7"), (1, "1"), (4, "1.5"), (19, "40"), (60, "3")]), row(&[(0, "8"), (1, "0")]));
        let second = format!("[ObjectDefaults]\n{}\n", row(&[(0, "8"), (1, "1"), (60, "0"), (21, "-5")]));
        let mut defs = Definitions::new();
        apply_objdef(&mut defs, &first).unwrap();
        apply_objdef(&mut defs, &second).unwrap();
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0].1.initial_hp, 40);
        assert_eq!(defs[0].1.initial_moves, 1.5);
        assert!(defs[1].1.active && defs[1].1.initial_morale == -5);
        assert!(apply_objdef(&mut defs, "[ObjectDefaults]\n2500,1\n").is_err());
        assert!(apply_objdef(&mut defs, &format!("[ObjectDefaults]\n{}\n", row(&[(0, "1"), (4, "1e60")]))).is_err());
    }

    #[test]
    fn flags_third_position_mismatches() {
        let text = format!("[ObjectDefaults]\n{}\n{}\n", row(&[(0, "7"), (1, "1"), (60, "3")]), row(&[(0, "9"), (1, "1")]));
        let mut defs = Definitions::new();
        apply_objdef(&mut defs, &text).unwrap();
        let mut objs = objects(4);
        for (slot, type_id) in [(0usize, 7u16), (1, 7), (2, 9), (3, 100)] {
            obj_activate(&mut objs, slot, type_id);
        }
        obj_unlink_all(&mut objs, 4, 10);
        obj_column(&mut objs, 4, 10, 0, 5); // type 7 with third: ok
        // slot 1 (type 7) has no third: mismatch; slot 2 (type 9) none: ok; slot 3 unknown type
        let (counts, mismatches) = audit_objects(&objs, &defs).unwrap();
        assert_eq!(counts.get("requires-third"), 2);
        assert_eq!(counts.get("two-positions"), 1);
        assert_eq!(mismatches.len(), 2);
    }

    #[test]
    fn default_path_comes_from_clengine_ini() {
        let dir = TempDir::new();
        let root = resolve(dir.path());
        assert_eq!(default_prefix(&root).unwrap(), DEFAULT_PREFIX);
        write(&root, "SYSTEM/clengine.ini", b"[Other]\nx=1\n[DefaultPath]\n;c\nSYSTEM\\MYDEF/\n");
        assert_eq!(default_prefix(&root).unwrap(), "SYSTEM/MYDEF/");
        write(&root, "SYSTEM/clengine.ini", b"[DefaultPath]\n..\\escape\n");
        assert!(default_prefix(&root).is_err());
        write(&root, "SYSTEM/clengine.ini", b"[Other]\n");
        assert!(default_prefix(&root).is_err());
    }

    #[test]
    fn code_gate_rejects_foreign_exe() {
        assert!(inspect_code(b"MZ").is_err());
    }
}
