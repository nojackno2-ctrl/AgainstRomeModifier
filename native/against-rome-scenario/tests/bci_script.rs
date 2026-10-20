//! BCI word-pattern and script-file semantics. No .NET needed:
//! `csharp_bci_pattern_and_pfil_differential` compares against outputs frozen from the (since removed)
//! shipped C# `BciPattern`/`GameLZSS` (tests/fixtures/bci_pattern_pfil_oracle.json).
use against_rome_scenario::bci_pattern::*;
use against_rome_scenario::{BciImage, BciScriptFile};
use serde_json::Value;
use std::{fs, path::PathBuf};

fn words(w: &[i32]) -> Vec<u8> {
    w.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn pfil_template() -> Vec<u8> {
    let mut h = vec![0u8; 64];
    h[..4].copy_from_slice(b"PFIL");
    h[4] = 7; // opaque metadata that must survive recompression
    h
}

fn script() -> BciScriptFile {
    let plain = BciImage::idle().serialize();
    let mut raw =
        against_rome_scenario::bci_script::compress_pfil_literal(&plain, &pfil_template()).unwrap();
    raw[5] = 9;
    BciScriptFile::from_bytes("ak_level.bci", raw).unwrap()
}

#[test]
fn finds_with_wildcards_and_overlap() {
    let d = words(&[1, 1, 1, 2, 1, 1]);
    let p = [Some(1), None, Some(1)];
    assert_eq!(find_all_word_pattern_sites(&d, &p).unwrap(), vec![0, 8]);
    assert_eq!(find_word_pattern(&d, &p, 4).unwrap(), Some(8));
    assert_eq!(find_word_pattern(&d, &p, 12).unwrap(), None);
    assert_eq!(find_word_pattern(&d, &p, 400).unwrap(), None);
}

#[test]
fn rejects_empty_pattern_and_unaligned_start() {
    let d = words(&[1, 2]);
    assert!(find_word_pattern(&d, &[], 0).is_err());
    assert!(find_word_pattern(&d, &[Some(2)], 2).is_err());
    assert!(find_all_word_pattern_sites(&d, &[]).is_err());
}

#[test]
fn pattern_longer_than_data_and_trailing_partial_word() {
    let mut d = words(&[5, 6]);
    d.extend([9, 9]); // unaligned tail is ignored, never read
    assert_eq!(find_word_pattern(&d, &[Some(6)], 0).unwrap(), Some(4));
    assert_eq!(
        find_word_pattern(&d, &[Some(5), Some(6), Some(0)], 0).unwrap(),
        None
    );
    assert_eq!(find_word_pattern(&[], &[None], 0).unwrap(), None);
}

#[test]
fn unique_site_fails_on_absent_and_ambiguous() {
    let d = words(&[1, 2, 1, 2]);
    assert!(find_unique_word_pattern_site(&d, &[Some(1), Some(2)], "p")
        .unwrap_err()
        .contains("ambiguous"));
    assert!(find_unique_word_pattern_site(&d, &[Some(3)], "p")
        .unwrap_err()
        .contains("not found"));
    assert_eq!(
        find_unique_word_pattern_site(&d, &[Some(2), Some(1)], "p").unwrap(),
        4
    );
}

#[test]
fn verified_write_checks_expected_and_bounds() {
    let mut d = words(&[10, 20]);
    assert!(write_i32_verified(&mut d, 0, 11, 1, "w").is_err());
    assert!(write_i32_verified(&mut d, 8, 0, 1, "w").is_err());
    assert!(write_i32_verified(&mut d, usize::MAX - 1, 0, 1, "w").is_err());
    assert_eq!(d, words(&[10, 20]));
    write_i32_verified(&mut d, 4, 20, -1, "w").unwrap();
    assert_eq!(d, words(&[10, -1]));
}

#[test]
fn replace_unique_keeps_wildcard_words_and_is_atomic() {
    let mut d = words(&[1, 2, 3, 9]);
    let before = d.clone();
    assert!(replace_unique_word_pattern(&mut d, &[Some(1), None], &[Some(5)], "r").is_err());
    assert!(replace_unique_word_pattern(&mut d, &[Some(7)], &[Some(5)], "r").is_err());
    assert_eq!(d, before);
    let site = replace_unique_word_pattern(
        &mut d,
        &[Some(1), None, Some(3)],
        &[Some(10), None, Some(30)],
        "r",
    )
    .unwrap();
    assert_eq!((site, d), (0, words(&[10, 2, 30, 9])));
}

#[test]
fn replace_all_requires_exact_count_and_rejects_overlap() {
    let mut d = words(&[1, 0, 1, 0, 1]);
    let before = d.clone();
    assert!(replace_all_word_pattern_sites(&mut d, &[Some(1)], &[Some(2)], 2, "r").is_err());
    assert_eq!(d, before);
    let mut o = words(&[1, 1, 1]);
    assert!(replace_all_word_pattern_sites(
        &mut o,
        &[Some(1), Some(1)],
        &[Some(2), Some(2)],
        2,
        "r"
    )
    .unwrap_err()
    .contains("overlapping"));
    assert_eq!(o, words(&[1, 1, 1]));
    let sites = replace_all_word_pattern_sites(&mut d, &[Some(1)], &[Some(2)], 3, "r").unwrap();
    assert_eq!((sites, d), (vec![0, 8, 16], words(&[2, 0, 2, 0, 2])));
}

#[test]
fn script_file_round_trips_and_preserves_header() {
    let mut f = script();
    assert!(!f.is_modified());
    let original = f.raw_bytes().to_vec();
    assert_eq!(f.compressed_bytes().unwrap(), &original[..]);
    f.image().unwrap();
    let main = f.image().unwrap().main;
    let site = f
        .replace_unique(&[Some(-20)], &[Some(-24)], "idle jump")
        .unwrap();
    assert!(f.is_modified());
    let code = f.image().unwrap().code;
    assert_eq!(code[(site - 36) / 4], -24); // 36 = BCI0 header + CODE tag
    assert!(!code.contains(&-20));
    let packed = f.compressed_bytes().unwrap().to_vec();
    assert!(!f.is_modified());
    assert_eq!(&packed[..4], b"PFIL");
    assert_eq!(packed[5], 9);
    let reloaded = BciScriptFile::from_bytes("x", packed).unwrap();
    assert_eq!(reloaded.decompressed_bytes(), f.decompressed_bytes());
    assert_eq!(reloaded.image().unwrap().main, main);
}

#[test]
fn script_file_failed_edit_leaves_state_untouched() {
    let mut f = script();
    let plain = f.decompressed_bytes().to_vec();
    assert!(f.replace_unique(&[Some(0x7777)], &[Some(1)], "x").is_err());
    assert!(f.write_i32(0, 12345, 1, "x").is_err());
    assert!(f.write_i32(plain.len(), 0, 1, "x").is_err());
    assert!(f.replace_all(&[Some(95)], &[Some(1)], 2, "x").is_err());
    assert!(!f.is_modified());
    assert_eq!(f.decompressed_bytes(), &plain[..]);
    // identical replacement is a no-op, not a modification
    f.replace_unique(&[Some(95)], &[Some(95)], "same").unwrap();
    assert!(!f.is_modified());
}

#[test]
fn script_file_rejects_malformed_input() {
    let good = script().raw_bytes().to_vec();
    assert!(BciScriptFile::from_bytes("x", vec![]).is_err());
    assert!(BciScriptFile::from_bytes("x", plain_bci()).is_err()); // not PFIL
    assert!(BciScriptFile::from_bytes("x", good[..good.len() - 3].to_vec()).is_err());
    let mut f = script();
    let state = (f.raw_bytes().to_vec(), f.decompressed_bytes().to_vec());
    assert!(f.replace_raw(good[..70].to_vec()).is_err());
    assert!(f.update_decompressed(vec![1, 2, 3]).is_err());
    assert_eq!(
        (f.raw_bytes().to_vec(), f.decompressed_bytes().to_vec()),
        state
    );
    assert!(!f.is_modified());
    // update/replace_raw mark modified like C#
    let mut plain = f.decompressed_bytes().to_vec();
    *plain.last_mut().unwrap() ^= 0; // keep valid
    f.update_decompressed(plain).unwrap();
    assert!(f.is_modified());
    f.replace_raw(good).unwrap();
    assert!(f.is_modified());
}

fn plain_bci() -> Vec<u8> {
    BciImage::idle().serialize()
}

#[test]
fn script_file_save_is_atomic_and_reloadable() {
    let dir = std::env::temp_dir().join(format!("arm-bci-script-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ak_level.bci");
    fs::write(&path, script().raw_bytes()).unwrap();
    let mut f = BciScriptFile::load(&path).unwrap();
    f.replace_unique(&[Some(-20)], &[Some(-28)], "p").unwrap();
    f.save().unwrap();
    assert!(!f.is_modified());
    assert!(!dir.join("ak_level.bci.arm-tmp").exists());
    let again = BciScriptFile::load(&path).unwrap();
    assert!(again.image().unwrap().code.contains(&-28));
    assert!(BciScriptFile::load(dir.join("missing.bci")).is_err());
    fs::remove_dir_all(dir).ok();
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn csharp_bci_pattern_and_pfil_differential() {
    // Deterministic xorshift corpus over a tiny value alphabet so matches and overlaps are common.
    let mut s = 0x2545F491u32;
    let mut next = move || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        s
    };
    let mut cases: Vec<(Vec<u8>, Vec<Option<i32>>)> = vec![];
    for _ in 0..300 {
        let n = (next() % 24) as usize;
        let data: Vec<i32> = (0..n).map(|_| (next() % 3) as i32 - 1).collect();
        let mut bytes = words(&data);
        if next() % 4 == 0 {
            bytes.extend(&[0xAA, 0xBB][..(next() % 3) as usize]);
        }
        let plen = 1 + (next() % 5) as usize;
        let pattern = (0..plen)
            .map(|_| {
                if next() % 3 == 0 {
                    None
                } else {
                    Some((next() % 3) as i32 - 1)
                }
            })
            .collect();
        cases.push((bytes, pattern));
    }
    let plains: Vec<Vec<u8>> = (0..40usize)
        .map(|i| (0..i * 7).map(|j| (j * 31 + i) as u8).collect())
        .collect();
    let res: Value = serde_json::from_slice(
        &fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bci_pattern_pfil_oracle.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(res["Sites"].as_array().unwrap().len(), cases.len());
    assert_eq!(res["Compressed"].as_array().unwrap().len(), plains.len());
    // The frozen "Decoded" column is the C# decode of the Rust compressor output for each plain input.
    let rust_packed: Vec<Vec<u8>> = plains
        .iter()
        .map(|p| {
            against_rome_scenario::bci_script::compress_pfil_literal(p, &pfil_template()).unwrap()
        })
        .collect();
    for (i, (d, p)) in cases.iter().enumerate() {
        let expected: Vec<usize> = res["Sites"][i]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as usize)
            .collect();
        assert_eq!(
            find_all_word_pattern_sites(d, p).unwrap(),
            expected,
            "case {i}"
        );
    }
    for (i, plain) in plains.iter().enumerate() {
        // C# compressor output must decode to the same bytes in Rust, with the size header updated.
        let cs = unhex(res["Compressed"][i].as_str().unwrap());
        let decoded = against_rome_data::lzss::decompress_pfil(&cs, None).unwrap();
        assert_eq!(&decoded, plain, "C# -> Rust pfil {i}");
        // Rust output must decode identically in C#.
        assert_eq!(
            unhex(res["Decoded"][i].as_str().unwrap()),
            *plain,
            "Rust -> C# pfil {i}"
        );
        assert_eq!(
            against_rome_data::lzss::decompress_pfil(&rust_packed[i], None).unwrap(),
            *plain,
            "Rust round-trip pfil {i}"
        );
    }
}
