use super::*;
use crate::{tests::Temp, AiRepairResult};

pub(crate) fn fixture() -> Vec<u8> {
    let mut w = vec![];
    for (upper, lower) in [
        (960000, 480000),
        (960000, 480000),
        (360000, 240000),
        (120000, 60000),
    ] {
        w.extend([66, upper, 66, lower, 128, 16, 999]);
    }
    w.extend(ORIGINAL);
    w.push(999);
    w.extend([90, 0, 66, 4, 96, 98, 91, 11]);
    w.extend(GATE);
    w.push(999);
    for i in 0..10 {
        w.extend([81, 56, 90, -3, 90, if i == 8 { 6 } else { 15 }, 164, 999]);
    }
    w.extend([128, 214, 73, -2, 86, 66, 1, 96, 102, 117, 92, 999]);
    w.extend([
        90, 42, 117, 56, 90, 41, 90, 4, 90, 3, 128, 141, 73, -3, 86, 71, 112, 116,
    ]);
    let mut bci = vec![0; 36];
    bci[..4].copy_from_slice(b"BCI0");
    write(&mut bci, 8, (w.len() * 4) as i32);
    bci.extend(bytes(&w));
    bci.extend(b"SYMBCONS\0unknown-save-tail\xff");
    let mut b = b"opaque prefix\0MAPS/ENDL_003/SCRIPT/ak_level.bci\0".to_vec();
    b.extend(bci);
    let mut header = vec![0x5a; 64];
    header[..4].copy_from_slice(b"PFIL");
    compress_pfil(&b, &header)
}
fn decode(raw: &[u8]) -> Vec<u8> {
    against_rome_data::lzss::decompress_pfil(raw, None).unwrap()
}
fn bci_offset(b: &[u8]) -> usize {
    b.windows(4).position(|s| s == b"BCI0").unwrap()
}
#[test]
fn original_migration_repairs_exact_sites_and_preserves_prefix_and_tail() {
    let raw = fixture();
    let before = decode(&raw);
    let out = repair_payload(&raw).unwrap().unwrap();
    let b = decode(&out);
    let off = bci_offset(&b);
    let old = detect(&before[off..]).unwrap();
    let s = detect(&b[off..]).unwrap();
    let t = &b[off..];
    assert_eq!(&before[..off], &b[..off]);
    assert_eq!(&raw[..16], &out[..16]);
    assert_eq!(&raw[20..64], &out[20..64]);
    assert_eq!(
        &b[off + code_end(t).unwrap()..],
        &before[off + code_end(&before[off..]).unwrap()..]
    );
    assert!(exact(t, s.scheduler, &ultimate()));
    for p in s.loops {
        assert_eq!((word(t, p + 4), word(t, p + 12)), (30000, 30000));
    }
    assert_eq!(word(t, s.threshold + 12), 70);
    assert!(exact(t, s.threshold + 32, GATE));
    assert_eq!(
        (word(t, s.quota[8] + 16), word(t, s.quota[8] + 20)),
        (90, 6)
    );
    assert_eq!(
        (word(t, s.quota[9] + 16), word(t, s.quota[9] + 20)),
        (66, 0)
    );
    for i in 0..8 {
        assert_eq!(
            &t[s.quota[i]..s.quota[i] + 28],
            &before[off + old.quota[i]..off + old.quota[i] + 28]
        );
    }
    assert_eq!((word(t, s.filter + 36), word(t, s.filter + 40)), (117, 0));
    assert_eq!(
        (
            word(t, s.action + 40),
            word(t, s.action + 48),
            word(t, s.action + 52)
        ),
        (120, 112, 4)
    );
    assert!(exact(t, s.helper.unwrap(), HELPER));
}
#[test]
fn repaired_payload_has_identical_sha256_and_no_second_output() {
    let raw = repair_payload(&fixture()).unwrap().unwrap();
    let hash = against_rome_data::sha256::sha256(&raw);
    assert!(repair_payload(&raw).unwrap().is_none());
    assert_eq!(against_rome_data::sha256::sha256(&raw), hash);
}
#[test]
fn shipped_legacy_gates_limits_and_zeroed_spawn_budget_migrate() {
    for gate in LEGACY_GATES {
        for limit in [4, 8, 30, 40, 70] {
            let raw = fixture();
            let mut b = decode(&raw);
            let off = bci_offset(&b);
            let s = detect(&b[off..]).unwrap();
            let t = &mut b[off..];
            write(t, s.threshold + 12, limit);
            t[s.threshold + 32..s.threshold + 92].copy_from_slice(&bytes(gate));
            write(t, s.quota[8] + 16, 66);
            write(t, s.quota[8] + 20, 0);
            write(t, s.filter + 36, 118);
            for p in s.loops {
                write(t, p + 4, 2000);
                write(t, p + 12, 1000);
            }
            let repaired = repair_payload(&compress_pfil(&b, &raw)).unwrap().unwrap();
            assert!(repair_payload(&repaired).unwrap().is_none());
        }
    }
}
#[test]
fn broken_array_create_helper_opcode_is_migrated() {
    let raw = repair_payload(&fixture()).unwrap().unwrap();
    let mut b = decode(&raw);
    let off = bci_offset(&b);
    let s = detect(&b[off..]).unwrap();
    write(&mut b, off + s.action + 40, 160);
    let out = repair_payload(&compress_pfil(&b, &raw)).unwrap().unwrap();
    let b = decode(&out);
    assert_eq!(word(&b, off + s.action + 40), 120);
}
#[test]
fn unknown_control_flow_and_missing_sites_fail_before_mutation() {
    let raw = fixture();
    let b = decode(&raw);
    let off = bci_offset(&b);
    let s = detect(&b[off..]).unwrap();
    for (p, v) in [
        (s.scheduler, 123),
        (s.loops[0] + 4, 123),
        (s.threshold + 12, 71),
        (s.threshold + 32, 123),
        (s.quota[0], 123),
        (s.quota[8] + 20, 7),
        (s.quota[9] + 20, 7),
        (s.filter + 40, 123),
        (s.action + 44, 142),
    ] {
        let mut b = b.clone();
        write(&mut b, off + p, v);
        let raw = compress_pfil(&b, &raw);
        let unchanged = raw.clone();
        assert!(repair_payload(&raw).is_err(), "{p}");
        assert_eq!(raw, unchanged);
    }
}
#[test]
fn malformed_and_ambiguous_embedded_paths_fail_closed() {
    let raw = fixture();
    let b = decode(&raw);
    for data in [
        b"not pfil".to_vec(),
        raw[..63].to_vec(),
        raw[..raw.len() - 1].to_vec(),
    ] {
        assert!(repair_payload(&data).is_err());
    }
    for data in [
        b"MAPS/ENDL_abc/SCRIPT/ak_level\0BCI0".to_vec(),
        b"MAPS/ENDL_001/SCRIPT/ak_level12345678901234567BCI0".to_vec(),
        [b.clone(), b.clone()].concat(),
    ] {
        assert!(repair_payload(&compress_pfil(&data, &raw)).is_err());
    }
}
#[test]
fn file_service_backs_up_entire_save_and_noop_keeps_hash() {
    let temp = Temp::new();
    let svc = temp.service();
    let folder = svc.root().join("SAVE_001");
    std::fs::create_dir_all(folder.join("CLAK")).unwrap();
    let raw = fixture();
    std::fs::write(folder.join("CLAK/scr.dat"), &raw).unwrap();
    std::fs::write(folder.join("save.ini"), b"info").unwrap();
    std::fs::write(folder.join("other.dat"), b"preserve").unwrap();
    let backup = match svc.repair_endless_ai("SAVE_001").unwrap() {
        AiRepairResult::Changed { backup } => backup,
        _ => panic!(),
    };
    let after = std::fs::read(folder.join("CLAK/scr.dat")).unwrap();
    assert_eq!(
        svc.repair_endless_ai("SAVE_001").unwrap(),
        AiRepairResult::AlreadyRepaired
    );
    assert_eq!(
        against_rome_data::sha256::sha256(&after),
        against_rome_data::sha256::sha256(&std::fs::read(folder.join("CLAK/scr.dat")).unwrap())
    );
    assert_eq!(
        std::fs::read_dir(temp.0.join("backups")).unwrap().count(),
        1
    );
    svc.restore(&backup, "SAVE_002").unwrap();
    assert_eq!(
        std::fs::read(svc.root().join("SAVE_002/CLAK/scr.dat")).unwrap(),
        raw
    );
    assert_eq!(
        std::fs::read(svc.root().join("SAVE_002/other.dat")).unwrap(),
        b"preserve"
    );
}
#[test]
fn invalid_or_readonly_save_creates_no_backup_and_preserves_source() {
    let temp = Temp::new();
    let svc = temp.service();
    let folder = svc.root().join("SAVE_001/CLAK");
    std::fs::create_dir_all(&folder).unwrap();
    let p = folder.join("scr.dat");
    std::fs::write(&p, b"invalid").unwrap();
    assert!(svc.repair_endless_ai("SAVE_001").is_err());
    assert_eq!(std::fs::read(&p).unwrap(), b"invalid");
    std::fs::write(&p, fixture()).unwrap();
    let permissions = std::fs::metadata(&p).unwrap().permissions();
    let mut ro = permissions.clone();
    ro.set_readonly(true);
    std::fs::set_permissions(&p, ro).unwrap();
    assert!(svc.repair_endless_ai("SAVE_001").is_err());
    assert!(!temp.0.join("backups").exists());
    std::fs::set_permissions(&p, permissions).unwrap();
    assert_eq!(std::fs::read(&p).unwrap(), fixture());
    assert!(svc.repair_endless_ai("../bad").is_err());
}
#[test]
fn csharp_synthetic_differential() {
    // Expected outputs were frozen from the (now removed) C# oracle in tests/fixtures/ai_repair
    // ({i}.output = decoded BCI tail, {i}.pfil = byte-exact PFIL compression of that tail).
    use std::fs;
    let inputs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ai_repair");
    let mut cases = vec![fixture()];
    // Exercise compressor dictionary wrap, initial spaces/NULs and self-overlap.
    let raw = fixture();
    let mut large = decode(&raw);
    large.extend(vec![0x20; 4200]);
    large.extend(vec![0; 4300]);
    large.extend((0..17000).map(|i| ((i * 37 + i / 23) % 256) as u8));
    cases.push(compress_pfil(&large, &raw));
    for gate in LEGACY_GATES {
        for limit in [4, 8, 30, 40, 70] {
            let raw = fixture();
            let mut b = decode(&raw);
            let off = bci_offset(&b);
            let s = detect(&b[off..]).unwrap();
            let t = &mut b[off..];
            write(t, s.threshold + 12, limit);
            t[s.threshold + 32..s.threshold + 92].copy_from_slice(&bytes(gate));
            write(t, s.quota[8] + 16, 66);
            write(t, s.quota[8] + 20, 0);
            write(t, s.filter + 36, 118);
            for p in s.loops {
                write(t, p + 4, 2000);
                write(t, p + 12, 1000);
            }
            cases.push(compress_pfil(&b, &raw));
        }
    }
    let current = repair_payload(&fixture()).unwrap().unwrap();
    let mut broken = decode(&current);
    let off = bci_offset(&broken);
    let s = detect(&broken[off..]).unwrap();
    write(&mut broken, off + s.action + 40, 160);
    cases.push(current.clone());
    cases.push(compress_pfil(&broken, &current));
    let mut compared = 0;
    for (i, raw) in cases.iter().enumerate() {
        let out = repair_payload(raw).unwrap().unwrap_or_else(|| raw.clone());
        let b = decode(&out);
        let tail = &b[bci_offset(&b)..];
        let oracle = fs::read(inputs.join(format!("{i}.output"))).unwrap();
        assert_eq!(tail, oracle, "decoded case {i}");
        let mut header = vec![0; 64];
        header[..4].copy_from_slice(b"PFIL");
        assert_eq!(
            compress_pfil(tail, &header),
            fs::read(inputs.join(format!("{i}.pfil"))).unwrap(),
            "compressed case {i}"
        );
        compared += tail.len();
    }
    assert_eq!(cases.len(), 19);
    println!("C# P6/P8/P9 differential: {} cases, {compared} decoded bytes + byte-exact PFIL compression passed",cases.len());
}
#[test]
fn safety_backup_records_title_level_and_unrestorable_backup_blocks_repair() {
    let temp = Temp::new();
    let svc = temp.service();
    let folder = svc.root().join("SAVE_001");
    std::fs::create_dir_all(folder.join("CLAK")).unwrap();
    let scr = folder.join("CLAK/scr.dat");
    std::fs::write(&scr, fixture()).unwrap();
    std::fs::write(folder.join("save.ini"), b"[titel]\nMy Game\n[orglevelname]\nENDL_003\n").unwrap();
    let backup = match svc.repair_endless_ai("SAVE_001").unwrap() {
        AiRepairResult::Changed { backup } => backup,
        other => panic!("{other:?}"),
    };
    let info = svc.scan().unwrap().backups.into_iter().find(|b| b.file_name == backup).unwrap();
    assert_eq!(info.manifest.orig_folder, "SAVE_001");
    assert_eq!(info.manifest.title, "My Game");
    assert_eq!(info.manifest.level, "ENDL_003");
    assert!(svc.verify_backup(&backup).unwrap().is_ok());

    // A save without save.ini yields a backup that restore would refuse: repair must stop
    // before touching scr.dat and must not leave the useless backup behind.
    let bad = svc.root().join("SAVE_002/CLAK");
    std::fs::create_dir_all(&bad).unwrap();
    std::fs::write(bad.join("scr.dat"), fixture()).unwrap();
    let err = svc.repair_endless_ai("SAVE_002").unwrap_err().to_string();
    assert!(err.contains("safety backup failed verification"), "{err}");
    assert_eq!(std::fs::read(bad.join("scr.dat")).unwrap(), fixture());
    let leftovers: Vec<_> = svc.scan().unwrap().backups.into_iter().filter(|b| b.manifest.orig_folder == "SAVE_002").collect();
    assert!(leftovers.is_empty());
}
