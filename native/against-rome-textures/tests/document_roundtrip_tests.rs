use against_rome_data::sha256::sha256_hex;
use against_rome_textures::BodenTexturesDocument;

#[test]
fn changes_exactly_one_tile_and_round_trips() {
    let original = "; Test comment\r\n[Dimension]\r\n2\r\n[Texturen]\r\n4BB___50\r\n4BB___50\r\n4BB___50\r\n4BB___50\r\n";
    let mut doc = BodenTexturesDocument::from_str(original).unwrap();
    assert_eq!(doc.dimension(), 2);
    assert_eq!(doc.get_texture(1, 0).unwrap(), "4BB___50");

    doc.set_texture(1, 0, "4BC___51").unwrap();
    assert_eq!(doc.get_texture(1, 0).unwrap(), "4BC___51");

    let saved = doc.text();
    let reloaded = BodenTexturesDocument::from_str(saved).unwrap();
    assert_eq!(reloaded.get_texture(0, 0).unwrap(), "4BB___50");
    assert_eq!(reloaded.get_texture(1, 0).unwrap(), "4BC___51");
    assert_eq!(reloaded.get_texture(0, 1).unwrap(), "4BB___50");
    assert_eq!(reloaded.get_texture(1, 1).unwrap(), "4BB___50");
}

#[test]
fn set_textures_writes_whole_grid_in_one_pass() {
    let original = "; Header comment\r\n[Dimension]\r\n2\r\n[Texturen]\r\ntex0\r\ntex1\r\ntex2\r\ntex3\r\n";
    let mut doc = BodenTexturesDocument::from_str(original).unwrap();

    let new_textures = vec![
        "new0".to_string(),
        "new1".to_string(),
        "new2".to_string(),
        "new3".to_string(),
    ];
    doc.set_textures(&new_textures).unwrap();

    assert_eq!(doc.textures(), new_textures);
    assert!(doc.text().contains("; Header comment"));
}

#[test]
fn set_textures_rejects_wrong_count() {
    let original = "[Dimension]\r\n2\r\n[Texturen]\r\ntex0\r\ntex1\r\ntex2\r\ntex3\r\n";
    let mut doc = BodenTexturesDocument::from_str(original).unwrap();
    let invalid = vec!["a".to_string(), "b".to_string()];
    assert!(doc.set_textures(&invalid).is_err());
}

#[test]
fn byte_preservation_and_sha256_identical_when_unchanged() {
    let original_bytes = b"; Test header\r\n[Dimension]\r\n2\r\n[Texturen]\r\n4BB___50\r\n4BC___51\r\n4BD___52\r\n4BE___53\r\n; Trailing comment\r\n";
    let orig_hash = sha256_hex(original_bytes);

    let mut doc = BodenTexturesDocument::from_bytes(original_bytes).unwrap();
    let textures = doc.textures();
    doc.set_textures(&textures).unwrap();

    let saved_bytes = doc.to_bytes();
    let saved_hash = sha256_hex(&saved_bytes);

    assert_eq!(
        orig_hash, saved_hash,
        "Bit-for-bit SHA256 identical invariant failed"
    );
}

#[test]
fn short_headers_never_panic() {
    // 0 bytes
    assert!(BodenTexturesDocument::from_bytes(&[]).is_err());
    // Short prefix
    assert!(BodenTexturesDocument::from_bytes(b"PF").is_err());
    assert!(BodenTexturesDocument::from_bytes(b"PFI").is_err());
    assert!(BodenTexturesDocument::from_bytes(b"PFIL").is_err());
    // Truncated header (e.g. 16, 32, 63 bytes)
    for len in [5, 10, 16, 32, 48, 63] {
        let truncated = vec![b'X'; len];
        let mut pfil_truncated = b"PFIL".to_vec();
        pfil_truncated.resize(len, 0);
        assert!(BodenTexturesDocument::from_bytes(&truncated).is_err());
        assert!(BodenTexturesDocument::from_bytes(&pfil_truncated).is_err());
    }
}

#[test]
fn pfil_preserves_unknown_bytes_in_64_byte_header() {
    let text = "[Dimension]\r\n2\r\n[Texturen]\r\n4BB___50\r\n4BB___50\r\n4BB___50\r\n4BB___50\r\n";
    let uncompressed = text.as_bytes();

    // Construct a synthetic 64-byte PFIL header with distinctive unknown bytes
    let mut custom_header = [0x5Au8; 64];
    custom_header[0..4].copy_from_slice(b"PFIL");
    // offset 16..20 will be size, fill other bytes with test pattern
    for i in 4..16 {
        custom_header[i] = (i * 7) as u8;
    }
    for i in 20..64 {
        custom_header[i] = (i * 13) as u8;
    }

    let pfil_bytes = against_rome_textures::lzss::compress_pfil(uncompressed, &custom_header).unwrap();
    let mut doc = BodenTexturesDocument::from_bytes(&pfil_bytes).unwrap();
    assert!(doc.is_pfil());

    // Modify a texture tile
    doc.set_texture(0, 0, "4BC___51").unwrap();
    assert_eq!(doc.get_texture(0, 0).unwrap(), "4BC___51");

    let saved = doc.to_bytes();
    assert!(saved.len() >= 64);
    assert_eq!(&saved[0..4], b"PFIL");

    // Bytes 4..16 and 20..64 must be preserved exactly
    assert_eq!(&saved[4..16], &custom_header[4..16]);
    assert_eq!(&saved[20..64], &custom_header[20..64]);

    // Reload and verify edited content
    let reloaded = BodenTexturesDocument::from_bytes(&saved).unwrap();
    assert_eq!(reloaded.get_texture(0, 0).unwrap(), "4BC___51");
}

#[test]
fn safe_save_rejects_readonly_file_and_preserves_content() {
    let temp_dir = crate::temp_fixture::dir("ar_textures_ro");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let temp_file = temp_dir.join("readonly_test.txt");

    let original = "[Dimension]\r\n2\r\n[Texturen]\r\n4BB___50\r\n4BB___50\r\n4BB___50\r\n4BB___50\r\n";
    std::fs::write(&temp_file, original.as_bytes()).unwrap();

    let writable_perms = std::fs::metadata(&temp_file).unwrap().permissions();
    let mut ro_perms = writable_perms.clone();
    ro_perms.set_readonly(true);
    std::fs::set_permissions(&temp_file, ro_perms).unwrap();

    let mut doc = BodenTexturesDocument::from_str(original).unwrap();
    doc.set_texture(0, 0, "4BC___51").unwrap();

    let result = doc.save(&temp_file);
    assert!(result.is_err(), "Saving to a read-only file must return an error");

    // Verify file on disk was not modified
    let disk_bytes = std::fs::read(&temp_file).unwrap();
    assert_eq!(disk_bytes, original.as_bytes());

    // Restore permissions and clean up
    std::fs::set_permissions(&temp_file, writable_perms).unwrap();
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn safe_save_creates_backup_and_preserves_content() {
    let temp_dir = crate::temp_fixture::dir("ar_textures_bak");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let temp_file = temp_dir.join("backup_test.txt");

    let original = "[Dimension]\r\n2\r\n[Texturen]\r\n4BB___50\r\n4BB___50\r\n4BB___50\r\n4BB___50\r\n";
    std::fs::write(&temp_file, original.as_bytes()).unwrap();

    let mut doc = BodenTexturesDocument::load(&temp_file).unwrap();
    doc.set_texture(0, 0, "4BC___51").unwrap();
    doc.save(&temp_file).unwrap();

    // Verify target file has new content
    let reloaded = BodenTexturesDocument::load(&temp_file).unwrap();
    assert_eq!(reloaded.get_texture(0, 0).unwrap(), "4BC___51");

    // Verify .bak file exists and has original content
    let backup_file = temp_dir.join("backup_test.txt.bak");
    assert!(backup_file.is_file(), "Backup file must be created on save");
    let bak_bytes = std::fs::read(&backup_file).unwrap();
    assert_eq!(bak_bytes, original.as_bytes());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn safe_save_detects_external_file_conflict() {
    let temp_dir = crate::temp_fixture::dir("ar_textures_conflict");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let temp_file = temp_dir.join("conflict_test.txt");

    let original = "[Dimension]\r\n2\r\n[Texturen]\r\n4BB___50\r\n4BB___50\r\n4BB___50\r\n4BB___50\r\n";
    std::fs::write(&temp_file, original.as_bytes()).unwrap();

    let mut doc = BodenTexturesDocument::load(&temp_file).unwrap();
    doc.set_texture(0, 0, "4BC___51").unwrap();

    // Externally tamper with the file on disk
    std::fs::write(&temp_file, b"externally changed content").unwrap();

    let result = doc.save(&temp_file);
    assert!(result.is_err(), "Saving to an externally modified file must be rejected");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn utf8_and_multibyte_comments_do_not_panic() {
    let content = "; Русский комментарий / Deutsche Kommentare (äöüß)\r\n[Dimension]\r\n2\r\n[Texturen]\r\n4BB___50\r\n4BB___50\r\n4BB___50\r\n4BB___50\r\n; Завершающий комментарий\r\n";
    let doc = BodenTexturesDocument::from_bytes(content.as_bytes()).unwrap();
    assert_eq!(doc.dimension(), 2);
    assert_eq!(doc.textures().len(), 4);
}

#[cfg(test)]
mod temp_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/temp_unique.rs"));
}
