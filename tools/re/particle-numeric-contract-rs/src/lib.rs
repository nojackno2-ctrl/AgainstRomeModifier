use iced_x86::{Decoder, DecoderOptions, Instruction, OpKind};
pub use particle_render_contract::{hash, hex, Pe, Result, HASH};
use serde_json::{json, Value};
use std::{fs, io::Write, path::Path};

pub const WINDOWS: &[(u64, usize)] = &[
    (0x419c80, 55),
    (0x419cc0, 35),
    (0x49be90, 126),
    (0x40e4d0, 217),
    (0x40e5c0, 393),
    (0x4dcfc0, 1617),
];
pub const CONSTANTS: &[(u64, usize)] = &[
    (0x612c70, 4),
    (0x5f4ba0, 8),
    (0x5f4b98, 8),
    (0x5f6826, 4),
    (0x5f682a, 4),
    (0x5f682e, 4),
    (0x5f6832, 4),
];
pub fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(message.into())
    }
}
pub fn address(s: &str) -> Result<u64> {
    Ok(u64::from_str_radix(s.trim_start_matches("0x"), 16)?)
}
pub fn decode(pe: &Pe, va: u64, size: usize) -> Result<Vec<Instruction>> {
    let end = va.checked_add(size as u64).ok_or("VA overflow")?;
    let mut decoder = Decoder::with_ip(32, pe.read(va, size)?, va, DecoderOptions::NONE);
    let mut out = Vec::new();
    while decoder.can_decode() {
        let i = decoder.decode();
        require(
            !i.is_invalid() && i.next_ip() <= end,
            "invalid or partial bounded instruction",
        )?;
        out.push(i);
    }
    require(
        out.last().is_some_and(|i| i.next_ip() == end),
        "incomplete body",
    )?;
    Ok(out)
}
pub fn finite(raw: &[u8], width: usize) -> Result<f64> {
    require(raw.len() == width, "constant operand width mismatch")?;
    let v = match width {
        4 => f32::from_le_bytes(raw.try_into()?) as f64,
        8 => f64::from_le_bytes(raw.try_into()?),
        _ => return Err("unsupported constant width".into()),
    };
    require(v.is_finite(), "nonfinite constant")?;
    Ok(v)
}
pub fn region(pe: &Pe, va: u64, size: usize) -> Result<Value> {
    let raw = pe.read(va, size)?;
    Ok(json!({"va":format!("{va:#x}"),"size":size,"raw_hex":hex(raw),"sha256":hash(raw)}))
}
pub fn collect(exe: &Path, export: &Path) -> Result<Value> {
    let bytes = fs::read(exe)?;
    require(hash(&bytes) == HASH, "unsupported EXE SHA256")?;
    let eb = fs::read(export)?;
    let ledger: Value = serde_json::from_slice(&eb)?;
    let records = ledger["records"].as_array().ok_or("records array")?;
    require(records.len() == 22, "expected 22 export records")?;
    let pe = Pe::parse(&bytes)?;
    let mut functions = Vec::new();
    let mut operands = Vec::new();
    let mut count = 0;
    for &(va, size) in WINDOWS {
        let key = format!("{va:#x}");
        let selected: Vec<_> = records
            .iter()
            .filter(|r| r["operation"] == "analyze_function" && r["parameters"]["procedure"] == key)
            .collect();
        require(selected.len() == 1, "selected record missing or duplicate")?;
        let r = selected[0];
        require(r["subject"]["digest"]["sha256"] == HASH, "record EXE hash")?;
        // Serialize the complete selected record, not only its pseudocode, for provenance.
        let record_hash = hash(&serde_json::to_vec(r)?);
        let n = &r["normalized_result"];
        let body = &n["procedure"]["body"];
        require(
            body["total_bytes"] == size && body["non_contiguous"] == false,
            "body size/contiguity",
        )?;
        let ranges = body["ranges"].as_array().ok_or("ranges")?;
        require(
            ranges.len() == 1
                && address(ranges[0]["start"].as_str().ok_or("start")?)? == va
                && address(ranges[0]["end"].as_str().ok_or("end")?)? + 1 == va + size as u64,
            "body boundaries",
        )?;
        let ins = decode(&pe, va, size)?;
        let assembly = n["assembly"].as_array().ok_or("assembly")?;
        require(ins.len() == assembly.len(), "export instruction count")?;
        let mut rows = Vec::new();
        for (i, a) in ins.iter().zip(assembly) {
            let text = a.as_str().ok_or("assembly string")?;
            require(
                address(text.split_once(':').ok_or("assembly VA")?.0)? == i.ip(),
                "export instruction boundary mismatch",
            )?;
            let mut row = pe.item(i)?;
            row["export_assembly"] = a.clone();
            rows.push(row);
            if (0..i.op_count()).any(|k| i.op_kind(k) == OpKind::Memory) {
                let target = i.memory_displacement64();
                if CONSTANTS.iter().any(|&(a, _)| a == target) {
                    let expected = CONSTANTS.iter().find(|&&(a, _)| a == target).unwrap().1;
                    require(
                        i.memory_size().size() == expected,
                        "declared operand width changed",
                    )?;
                    operands.push(json!({"instruction_va":format!("{:#x}",i.ip()),"constant_va":format!("{target:#x}"),"declared_width":expected,"assembly":a}));
                }
            }
        }
        count += rows.len();
        let mut f = region(&pe, va, size)?;
        f["evidence_id"] = r["evidence_id"].clone();
        f["complete_record_canonical_sha256"] = json!(record_hash);
        f["instructions"] = json!(rows);
        f["pseudocode"] = n["pseudocode"].clone();
        functions.push(f);
    }
    let mut constants = Vec::new();
    for &(va, width) in CONSTANTS {
        require(
            operands
                .iter()
                .any(|r| r["constant_va"] == format!("{va:#x}")),
            "constant has no selected operand",
        )?;
        let mut v = region(&pe, va, width)?;
        v["type"] = json!(if width == 4 { "f32" } else { "f64" });
        v["finite_value"] = json!(finite(pe.read(va, width)?, width)?);
        constants.push(v);
    }
    let mut defaults = Vec::new();
    for va in [0x618b54, 0x618b44] {
        let raw = pe.read(va, 4)?;
        let target = u32::from_le_bytes(raw.try_into()?) as u64;
        let mut p = region(&pe, va, 4)?;
        p["target"] = region(&pe, target, 64)?;
        p["meaning"] = json!("on-disk default only; runtime dispatch unproven");
        defaults.push(p);
    }
    // General angle invokes a CRT boundary. Follow only this direct callee, at most 128 bytes.
    let mut callee = region(&pe, 0x5c5f4b, 128)?;
    let mut d = Decoder::with_ip(32, pe.read(0x5c5f4b, 128)?, 0x5c5f4b, DecoderOptions::NONE);
    let mut rows = Vec::new();
    while d.can_decode() {
        let i = d.decode();
        if i.is_invalid() {
            break;
        }
        rows.push(pe.item(&i)?);
        if i.mnemonic() == iced_x86::Mnemonic::Ret {
            break;
        }
    }
    callee["instructions"] = json!(rows);
    callee["reason"] =
        json!("angle direct CALL 0x5c5f4b leaves CRT/fpatan boundary; no other callees followed");
    require(
        fs::read(exe)? == bytes && fs::read(export)? == eb,
        "source changed",
    )?;
    Ok(
        json!({"exe_sha256":HASH,"export_sha256":hash(&eb),"export_record_count":records.len(),"selected_record_count":functions.len(),"frozen_instruction_count":count,"functions":functions,"constant_operands":operands,"constants":constants,"default_pointers":defaults,"bounded_angle_callee":callee,"source_unchanged":true,"unknowns":["x87 control word, precision, exceptions and runtime rounding policy","general CRT atan result and quadrant boundary rounding","runtime dispatch chosen at 618b54/618b44","caller palette parameter ranges, aliasing and lifecycle","no product numerical API, native execution, game launch or pixel equivalence"]}),
    )
}
pub fn write_new(path: &Path, value: &Value) -> Result<()> {
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    serde_json::to_writer_pretty(&mut f, value)?;
    f.write_all(b"\n")?;
    Ok(())
}

pub fn unhex(s: &str) -> Result<Vec<u8>> {
    require(
        !s.is_empty() && s.len() % 2 == 0 && s.is_ascii(),
        "invalid hex width/encoding",
    )?;
    Ok((0..s.len())
        .step_by(2)
        .map(|p| u8::from_str_radix(&s[p..p + 2], 16))
        .collect::<std::result::Result<Vec<_>, _>>()?)
}
pub fn verify_region(pe: &Pe, row: &Value) -> Result<()> {
    let va = address(row["va"].as_str().ok_or("region VA")?)?;
    let size = usize::try_from(row["size"].as_u64().ok_or("region size")?)?;
    let raw = unhex(row["raw_hex"].as_str().ok_or("region hex")?)?;
    require(raw.len() == size, "region hex length")?;
    require(
        hash(&raw) == row["sha256"].as_str().ok_or("region hash")?,
        "region hash mismatch",
    )?;
    require(pe.read(va, size)? == raw, "region bytes mismatch")
}
pub fn verify_frozen(pe: &Pe, frozen: &Value) -> Result<usize> {
    let mut total = 0;
    let functions = frozen["functions"].as_array().ok_or("functions")?;
    require(functions.len() == WINDOWS.len(), "function count")?;
    for (f, &(va, size)) in functions.iter().zip(WINDOWS) {
        require(
            f["va"] == format!("{va:#x}") && f["size"] == size,
            "function window",
        )?;
        verify_region(pe, f)?;
        total += particle_render_contract::verify(pe, &f["instructions"])?;
    }
    require(frozen["frozen_instruction_count"] == total, "frozen count")?;
    let constants = frozen["constants"].as_array().ok_or("constants")?;
    require(constants.len() == CONSTANTS.len(), "constant count")?;
    for (c, &(va, width)) in constants.iter().zip(CONSTANTS) {
        require(
            c["va"] == format!("{va:#x}") && c["size"] == width,
            "constant width/VA",
        )?;
        verify_region(pe, c)?;
        require(
            c["finite_value"].as_f64() == Some(finite(pe.read(va, width)?, width)?),
            "constant value",
        )?;
    }
    for p in frozen["default_pointers"].as_array().ok_or("pointers")? {
        verify_region(pe, p)?;
        verify_region(pe, &p["target"])?;
    }
    verify_region(pe, &frozen["bounded_angle_callee"])?;
    total += particle_render_contract::verify(pe, &frozen["bounded_angle_callee"]["instructions"])?;
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use particle_render_contract::Section;
    fn sample(raw: &[u8]) -> Pe<'_> {
        Pe {
            source: raw,
            sections: vec![Section {
                va: 0x400000,
                offset: 0,
                size: raw.len(),
                executable: true,
            }],
        }
    }
    #[test]
    fn mutation_bytes_va_hash_size() {
        let p = sample(&[0x90, 0xc3]);
        let r = region(&p, 0x400000, 2).unwrap();
        verify_region(&p, &r).unwrap();
        for (field, bad) in [
            ("raw_hex", json!("90c2")),
            ("va", json!("0x400001")),
            ("size", json!(3)),
            ("sha256", json!("wrong")),
        ] {
            let mut x = r.clone();
            x[field] = bad;
            assert!(verify_region(&p, &x).is_err(), "{field}");
        }
        let changed = sample(&[0x90, 0xc2]);
        assert!(verify_region(&changed, &r).is_err());
    }
    #[test]
    fn mapped_bounds_and_overflow() {
        let p = sample(&[0x90, 0xc3]);
        assert!(p.read(0x3fffff, 1).is_err());
        assert!(p.read(0x400001, 2).is_err());
        assert!(p.read(0x400000, usize::MAX).is_err());
        assert!(decode(&p, u64::MAX, 2).is_err());
    }
    #[test]
    fn instruction_tail_and_mutation() {
        let p = sample(&[0x83, 0xc0, 4]);
        assert_eq!(decode(&p, 0x400000, 3).unwrap().len(), 1);
        assert!(decode(&p, 0x400000, 2).is_err());
        assert!(
            particle_render_contract::verify(&p, &json!([{"va":"0x400000","bytes":"83c005"}]))
                .is_err()
        );
        assert!(
            particle_render_contract::verify(&p, &json!([{"va":"0x400000","bytes":"83c0"}]))
                .is_err()
        );
    }
    #[test]
    fn constants_require_exact_width_finite() {
        assert_eq!(
            finite(&[0xfd, 0xff, 0xff, 0x3e], 4).unwrap(),
            0.49999991059303284
        );
        assert_eq!(finite(&180f64.to_le_bytes(), 8).unwrap(), 180.);
        assert!(finite(&180f64.to_le_bytes(), 4).is_err());
        assert!(finite(&180f32.to_le_bytes(), 8).is_err());
        assert!(finite(&[0; 2], 2).is_err());
        assert!(finite(&f32::NAN.to_le_bytes(), 4).is_err());
        assert!(finite(&f64::INFINITY.to_le_bytes(), 8).is_err());
    }
    #[test]
    fn declared_instruction_widths() {
        for (raw, width) in [
            (&[0xd8, 0x25, 0x70, 0x2c, 0x61, 0][..], 4),
            (&[0xdc, 0x0d, 0x98, 0x4b, 0x5f, 0][..], 8),
        ] {
            let p = sample(raw);
            let i = decode(&p, 0x400000, raw.len()).unwrap();
            assert_eq!(i[0].memory_size().size(), width);
        }
    }
    #[test]
    fn malformed_hex_and_pe() {
        for s in ["", "a", "zz", "é"] {
            assert!(unhex(s).is_err());
        }
        assert!(Pe::parse(&[]).is_err());
        assert!(Pe::parse(&[0; 64]).is_err());
    }
    #[test]
    fn output_create_new_preserves_existing() {
        let p = crate::temp_fixture::dir("particle-numeric-create-new-test-.json").join("particle-numeric-create-new-test-.json");
        let _ = fs::remove_file(&p);
        write_new(&p, &json!({"test":1})).unwrap();
        let b = fs::read(&p).unwrap();
        assert!(write_new(&p, &json!({"test":2})).is_err());
        assert_eq!(fs::read(&p).unwrap(), b);
        let _ = fs::remove_file(&p);
    }
    #[test]
    #[ignore = "needs the installed retail Against_Rome.exe and local re_workspace evidence"]
    fn installed_frozen_contract_and_mutated_constants() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .unwrap();
        let exe = Path::new("C:/Program Files (x86)/Against Rome/Against_Rome.exe");
        let frozen: Value = serde_json::from_str(include_str!("frozen.json")).unwrap();
        let b = fs::read(exe).unwrap();
        assert_eq!(hash(&b), HASH);
        let pe = Pe::parse(&b).unwrap();
        assert_eq!(verify_frozen(&pe, &frozen).unwrap(), 700);
        let actual = collect(
            exe,
            &repo.join("re_workspace/particle-direction-palette-20261012-evidence.json"),
        )
        .unwrap();
        assert_eq!(actual, frozen);
        for bad in [json!(4), json!(16)] {
            let mut f = frozen.clone();
            f["constants"][1]["size"] = bad;
            assert!(verify_frozen(&pe, &f).is_err());
        }
        let mut f = frozen.clone();
        f["functions"][1]["instructions"][4]["bytes"] = json!("d825712c6100");
        assert!(verify_frozen(&pe, &f).is_err());
        assert_eq!(fs::read(exe).unwrap(), b);
    }
}

#[cfg(test)]
mod temp_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/temp_unique.rs"));
}
