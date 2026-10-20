use saved_light_contract::{canonical,contract,cross_check,inspect,output_path,sha256,validate_evidence,validate_fields,verify_instruction,Pe,EXE_SHA256,WIDTHS};
use serde_json::{json,Value};
use std::{fs,path::PathBuf,sync::atomic::{AtomicU64,Ordering}};

fn bytes(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i+2],16).unwrap()).collect()
}
fn static_evidence() -> Value {
    json!({"exe_sha256":EXE_SHA256,"instructions":["0x400000 90 nop "],"instruction_assertions":1,"light_column_widths":WIDTHS})
}
fn transform_evidence() -> Value {
    let mut value = static_evidence();
    value["callback_fields"] = json!([67,70,71]);
    value["owner_position_runtime_offset"] = json!(12);
    value["symbolic_xyz"] = json!({"dword ptr [esp + 0x20]":["Y"],"dword ptr [esp + 4]":["X"],"dword ptr [esp]":["Z"]});
    value
}
fn bundle() -> Value {
    json!({"records":[{"subject":{"digest":{"sha256":EXE_SHA256}},"evidence_id":"e1","operation":"read_bytes","parameters":{}}]})
}

#[test]
fn every_frozen_assertion_decodes_and_checks_operands() {
    let frozen = contract();
    let assertions = frozen["assertions"].as_array().unwrap();
    assert_eq!(assertions.len(),746);
    for a in assertions {
        let address = u64::from_str_radix(a["address"].as_str().unwrap().trim_start_matches("0x"),16).unwrap();
        verify_instruction(&bytes(a["bytes"].as_str().unwrap()),address,a["mnemonic"].as_str().unwrap(),a["operands"].as_str().unwrap()).unwrap_or_else(|e| panic!("{address:#x}: {e}"));
    }
    validate_fields(&frozen).unwrap();
    cross_check(&frozen).unwrap();
}
#[test]
fn every_region_line_decodes_and_hashes() {
    for region in contract()["regions"].as_array().unwrap() {
        let mut raw = Vec::new();
        let mut address = u64::from_str_radix(region["start"].as_str().unwrap().trim_start_matches("0x"),16).unwrap();
        for line in region["instructions"].as_array().unwrap() {
            let parts: Vec<_> = line.as_str().unwrap().splitn(4,' ').collect();
            assert_eq!(u64::from_str_radix(parts[0].trim_start_matches("0x"),16).unwrap(),address);
            let encoded = bytes(parts[1]);
            verify_instruction(&encoded,address,parts[2],parts.get(3).copied().unwrap_or("")).unwrap();
            address += encoded.len() as u64; raw.extend(encoded);
        }
        assert_eq!(raw.len() as u64,region["size"].as_u64().unwrap());
        assert_eq!(sha256(&raw),region["sha256"].as_str().unwrap());
    }
}
#[test]
fn decoder_rejects_wrong_semantics_truncation_and_trailing_bytes() {
    let raw = [0x83,0xc7,0x44];
    assert!(verify_instruction(&raw,0x48ac02,"add","edi, 68").is_ok());
    for (m,o) in [("sub","edi, 0x44"),("add","esi, 0x44"),("add","edi, 0x45")] {
        assert!(verify_instruction(&raw,0x48ac02,m,o).is_err());
    }
    assert!(verify_instruction(&raw[..2],0x48ac02,"add","edi, 0x44").is_err());
    assert!(verify_instruction(&[0x90,0x90],0x400000,"nop","").is_err());
    assert_ne!(canonical("mov","word ptr [edi - 4], ax").unwrap(),canonical("mov","dword ptr [edi - 4], eax").unwrap());
    assert_ne!(canonical("mov","eax, [edi - 4]").unwrap(),canonical("mov","eax, [edi + 4]").unwrap());
    assert_eq!(canonical("fsubrp","st(1), st(0)").unwrap(),canonical("fsubrp","st(1)").unwrap());
    assert_ne!(canonical("fsubrp","st(2)").unwrap(),canonical("fsubrp","st(1)").unwrap());
}
#[test]
fn reference_rejects_mutations_at_all_746_addresses_and_bytes() {
    let frozen = contract();
    for i in 0..746 {
        let mut changed = frozen.clone(); changed["assertions"][i]["address"] = json!("0x400000");
        assert!(cross_check(&changed).is_err());
        changed = frozen.clone(); changed["assertions"][i]["bytes"] = json!("90");
        assert!(cross_check(&changed).is_err());
    }
    for i in 0..16 {
        for key in ["column","width","snapshot_offset","runtime_offset","name","read_va","store_va","pool_offset_formula"] {
            let mut changed = frozen.clone(); changed["fields"][i][key] = Value::Null;
            assert!(cross_check(&changed).is_err()); assert!(validate_fields(&changed).is_err());
        }
    }
    let mut missing = frozen.clone(); missing["assertions"].as_array_mut().unwrap().pop();
    assert!(cross_check(&missing).is_err());
    for key in ["mnemonic","operands"] {
        let mut changed = frozen.clone(); changed["assertions"][0][key] = json!("bad");
        assert!(cross_check(&changed).is_err());
    }
    let mut changed = frozen; changed["regions"][0]["sha256"] = json!("bad");
    assert!(cross_check(&changed).is_err());
}
#[test]
fn schemas_accept_valid_binding_and_reject_missing_or_wrong_fields() {
    for (valid,kind) in [(static_evidence(),"static"),(transform_evidence(),"transform"),(bundle(),"bundle")] {
        validate_evidence(&valid,kind).unwrap();
        assert!(validate_evidence(&Value::Null,kind).is_err());
        assert!(validate_evidence(&valid,"unknown").is_err());
        if kind != "bundle" {
            for key in ["exe_sha256","instructions","instruction_assertions"] {
                let mut broken = valid.clone(); broken.as_object_mut().unwrap().remove(key);
                assert!(validate_evidence(&broken,kind).is_err(),"{kind} {key}");
            }
            for count in [json!(true),json!(0),json!(-1),json!(1.5),json!(2)] {
                let mut broken = valid.clone(); broken["instruction_assertions"] = count;
                assert!(validate_evidence(&broken,kind).is_err());
            }
            for lines in [json!([]),json!([17]),json!(["garbage"]),json!(["0x400000 91 nop "]),json!(["0x400000 90 nop ","0x400000 90 nop "])] {
                let mut broken = valid.clone(); broken["instructions"] = lines;
                assert!(validate_evidence(&broken,kind).is_err());
            }
            let mut broken = valid.clone(); broken["exe_sha256"] = json!("bad");
            assert!(validate_evidence(&broken,kind).is_err());
        } else {
            for key in ["subject","evidence_id","operation","parameters"] {
                let mut broken = valid.clone(); broken["records"][0].as_object_mut().unwrap().remove(key);
                assert!(validate_evidence(&broken,kind).is_err());
            }
            let mut broken = valid.clone(); broken["records"][0]["subject"]["digest"]["sha256"] = json!("bad");
            assert!(validate_evidence(&broken,kind).is_err());
            broken = valid.clone(); broken["records"] = json!([]);
            assert!(validate_evidence(&broken,kind).is_err());
            broken = valid.clone(); broken["records"].as_array_mut().unwrap().push(valid["records"][0].clone());
            assert!(validate_evidence(&broken,kind).is_err());
        }
    }
    let mut value = static_evidence(); value["light_column_widths"][9] = json!(4);
    assert!(validate_evidence(&value,"static").is_err());
    for key in ["symbolic_xyz","callback_fields","owner_position_runtime_offset"] {
        let mut value = transform_evidence(); value[key] = Value::Null;
        assert!(validate_evidence(&value,"transform").is_err());
    }
    let mut value = transform_evidence(); value["symbolic_xyz"]["extra"] = json!(0);
    assert!(validate_evidence(&value,"transform").is_err());
}

#[test]
#[ignore = "needs local re_workspace evidence files that are not committed"]
fn repository_evidence_schemas_and_all_instruction_lines_decode() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3).unwrap().to_path_buf();
    for ev in contract()["reused_evidence"].as_array().unwrap() {
        let path = repo.join(ev["path"].as_str().unwrap().replace('\\',"/"));
        let raw = fs::read(path).expect("repository evidence is required for this integration test");
        assert_eq!(sha256(&raw),ev["sha256"].as_str().unwrap());
        let data: Value = serde_json::from_slice(&raw).unwrap();
        validate_evidence(&data,ev["schema"].as_str().unwrap()).unwrap();
    }
}

fn minimal_pe() -> Vec<u8> {
    let mut raw = vec![0u8;0x204]; raw[..2].copy_from_slice(b"MZ");
    raw[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes()); raw[0x80..0x84].copy_from_slice(b"PE\0\0");
    raw[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes()); raw[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
    raw[0x94..0x96].copy_from_slice(&96u16.to_le_bytes()); raw[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
    raw[0xb4..0xb8].copy_from_slice(&0x400000u32.to_le_bytes());
    let table = 0x98+96;
    raw[table+12..table+16].copy_from_slice(&0x1000u32.to_le_bytes());
    raw[table+16..table+20].copy_from_slice(&4u32.to_le_bytes());
    raw[table+20..table+24].copy_from_slice(&0x200u32.to_le_bytes()); raw[0x200..].copy_from_slice(&[0x90;4]); raw
}
#[test]
fn pe_bounds_architecture_and_hash_fail_closed() {
    let raw = minimal_pe(); let pe = Pe::parse(&raw).unwrap();
    assert_eq!(pe.read(0x401000,4).unwrap(),[0x90;4]);
    assert!(pe.read(0x401001,4).is_err()); assert!(pe.read(0x3fffff,1).is_err()); assert!(pe.read(u64::MAX,2).is_err());
    assert!(Pe::parse(&raw[..0x203]).is_err());
    let mut wrong_magic = raw.clone(); wrong_magic[0x98] = 0x0c;
    assert!(Pe::parse(&wrong_magic).is_err());
    let mut wrong = raw.clone(); wrong[0x84] = 0x64; assert!(Pe::parse(&wrong).is_err());
    wrong = raw.clone(); wrong[0xb4] = 1; assert!(Pe::parse(&wrong).is_err());
    assert!(inspect(&raw).is_err());
}
#[test]
fn output_is_new_json_directly_in_workspace_and_create_new_prevents_race() {
    static ID: AtomicU64 = AtomicU64::new(0);
    // All fixtures stay in this crate's target artifacts, even when no Rust
    // toolchain has been installed globally.
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-scratch").join(format!("{}-{}",std::process::id(),ID.fetch_add(1,Ordering::Relaxed)));
    fs::create_dir_all(repo.join("re_workspace/nested")).unwrap();
    let output = repo.join("re_workspace/new.json");
    let validated = output_path(&repo,&output).unwrap();
    fs::write(&output,b"existing").unwrap();
    assert!(output_path(&repo,&output).is_err());
    assert!(fs::OpenOptions::new().write(true).create_new(true).open(validated).is_err());
    for path in [repo.join("outside.json"),repo.join("re_workspace/nested/new.json"),repo.join("re_workspace/new.txt"),repo.join("re_workspace/../re_workspace/traversal.json"),repo.join("re_workspace/NUL.json"),repo.join("re_workspace/file:stream.json")] {
        assert!(output_path(&repo,&path).is_err(),"{path:?}");
    }
    assert_eq!(fs::read(&output).unwrap(),b"existing");
}
