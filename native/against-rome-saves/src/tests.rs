use super::*;
pub(crate) struct Temp(pub(crate) PathBuf);
impl Temp {
    pub(crate) fn new() -> Self {
        let p = temp_fixture::dir("ar_saves_test");

        Self(p)
    }
    pub(crate) fn service(&self) -> SaveService {
        SaveService::new(self.0.join("game"), self.0.join("backups")).unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn pfil(text: &[u8]) -> Vec<u8> {
    let mut b = vec![0; 64];
    b[..4].copy_from_slice(b"PFIL");
    b[16..20].copy_from_slice(&(text.len() as u32).to_le_bytes());
    for chunk in text.chunks(8) {
        b.push(((1u16 << chunk.len()) - 1) as u8);
        b.extend(chunk);
    }
    b
}
fn source(s: &SaveService) {
    let p = s.root().join("SAVE_001");
    fs::create_dir_all(p.join("DATA")).unwrap();
    fs::write(
        p.join("save.ini"),
        pfil(b"[titel]\r\nTitle\r\n[orglevelname]\r\nENDL_000\r\n"),
    )
    .unwrap();
    fs::write(p.join("DATA/a.dat"), [0, 1, 2, 255]).unwrap();
    fs::write(p.join("savepic.tga"), b"preview").unwrap();
}
fn archive(s: &SaveService, name: &str, entries: &[(&str, &[u8])]) {
    fs::create_dir_all(&s.backups).unwrap();
    let mut z = ZipWriter::new(File::create(s.backups.join(name)).unwrap());
    for (n, b) in entries {
        z.start_file(*n, FileOptions::default()).unwrap();
        z.write_all(b).unwrap();
    }
    z.finish().unwrap();
}
#[test]
fn roundtrip_real_zip_and_pascal_manifest() {
    let t = Temp::new();
    let s = t.service();
    source(&s);
    s.backup_as(
        "SAVE_001",
        "round.zip",
        "Title",
        "ENDL_000",
        "2026-10-15T10:00:00+08:00",
    )
    .unwrap();
    let mut z = ZipArchive::new(File::open(s.backups.join("round.zip")).unwrap()).unwrap();
    let bytes = read_limited(&mut z.by_name("manifest.json").unwrap(), 1024).unwrap();
    assert!(std::str::from_utf8(&bytes).unwrap().contains("OrigFolder"));
    assert!(!std::str::from_utf8(&bytes).unwrap().contains("orig_folder"));
    assert_eq!(s.scan().unwrap().saves[0].title, "Title");
    assert_eq!(
        s.scan().unwrap().backups[0].manifest.orig_folder,
        "SAVE_001"
    );
    fs::write(s.root().join("SAVE_001/DATA/a.dat"), b"changed").unwrap();
    s.restore("round.zip", "SAVE_001").unwrap();
    assert_eq!(
        fs::read(s.root().join("SAVE_001/DATA/a.dat")).unwrap(),
        [0, 1, 2, 255]
    );
    assert_eq!(
        s.save_preview("SAVE_001").unwrap(),
        s.backup_preview("round.zip").unwrap()
    );
    assert!(s.backup_as("SAVE_001", "round.zip", "", "", "").is_err());
}
#[test]
fn rollback_restores_old_directory_after_move() {
    let t = Temp::new();
    let s = t.service();
    source(&s);
    s.backup_as("SAVE_001", "good.zip", "", "", "").unwrap();
    fs::write(s.root().join("SAVE_001/old"), b"must survive").unwrap();
    assert!(s
        .restore_impl("good.zip", "SAVE_001", || fail("injected publish failure"))
        .is_err());
    assert_eq!(
        fs::read(s.root().join("SAVE_001/old")).unwrap(),
        b"must survive"
    );
    assert_eq!(fs::read_dir(s.root()).unwrap().count(), 1);
}
#[test]
fn rollback_preserves_reappeared_destination_and_original_save() {
    let t = Temp::new();
    let s = t.service();
    source(&s);
    s.backup_as("SAVE_001", "good.zip", "", "", "").unwrap();
    let dest = s.root().join("SAVE_001");
    fs::write(dest.join("old"), b"original save").unwrap();

    let error = s
        .restore_impl("good.zip", "SAVE_001", || {
            // Another writer creates a save after the original was moved aside.
            fs::create_dir(&dest)?;
            fs::write(dest.join("new"), b"other writer's save")?;
            fail("injected publish failure")
        })
        .unwrap_err()
        .to_string();

    assert_eq!(fs::read(dest.join("new")).unwrap(), b"other writer's save");
    let old = fs::read_dir(s.root())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".restore_old_")
        })
        .expect("original save must be retained for recovery");
    assert_eq!(fs::read(old.join("old")).unwrap(), b"original save");
    assert!(error.contains("injected publish failure"), "{error}");
    assert!(error.contains("rollback failed"), "{error}");
    assert!(error.contains(&old.display().to_string()), "{error}");
    assert_eq!(fs::read_dir(s.root()).unwrap().count(), 2);
}
#[test]
fn invalid_archives_never_replace_save() {
    let t = Temp::new();
    let s = t.service();
    source(&s);
    for (n, path) in [
        ("parent.zip", "../outside"),
        ("absolute.zip", "/outside"),
        ("drive.zip", "C:/outside"),
        ("backslash.zip", "a\\b"),
        ("dot.zip", "a/./b"),
        ("device.zip", "NUL.txt"),
    ] {
        archive(&s, n, &[("save.ini", b"data"), (path, b"bad")]);
        assert!(s.restore(n, "SAVE_001").is_err(), "{path}");
        assert!(s.root().join("SAVE_001/DATA/a.dat").is_file());
    }
    archive(&s, "missing.zip", &[("readme", b"x")]);
    assert!(s.restore("missing.zip", "SAVE_001").is_err());
    archive(
        &s,
        "duplicate.zip",
        &[("save.ini", b"x"), ("SAVE.INI", b"y")],
    );
    assert!(s.restore("duplicate.zip", "SAVE_001").is_err());
    assert_eq!(fs::read_dir(s.root()).unwrap().count(), 1);
}
#[test]
fn archive_symlink_is_rejected() {
    let t = Temp::new();
    let s = t.service();
    fs::create_dir_all(&s.backups).unwrap();
    let mut z = ZipWriter::new(File::create(s.backups.join("link.zip")).unwrap());
    z.add_symlink("save.ini", "outside", FileOptions::default())
        .unwrap();
    z.finish().unwrap();
    assert!(s.restore("link.zip", "SAVE_001").is_err());
    assert!(!s.root().join("SAVE_001").exists());
}
#[test]
fn pfil_cp1251_and_truncation() {
    let b = pfil(b"[titel]\r\n\xCF\xF0\xE8\xE2\xE5\xF2\r\n[orglevelname]\r\nMAP\r\n");
    let (t, l) = read_save_info(&b).unwrap();
    assert_eq!(t, "Привет");
    assert_eq!(l, "MAP");
    assert!(read_save_info(&b[..b.len() - 1]).is_err());
    assert_eq!(
        read_save_info(b"[titel]\nPlain\n[orglevelname]\nMAP\n").unwrap(),
        ("Plain".into(), "MAP".into())
    );
}
#[test]
fn bom_manifest_and_legacy_name_fallback() {
    let t = Temp::new();
    let s = t.service();
    let ini = pfil(b"[titel]\nFallback\n[orglevelname]\nMAP\n");
    archive(&s,"Backup_SAVE_001_20261015_123456.zip",&[("save.ini",&ini),("manifest.json",b"\xef\xbb\xbf{\"OrigFolder\":\"\",\"Title\":\"\",\"Level\":\"\",\"BackupTime\":\"\"}")]);
    let c = s.scan().unwrap();
    assert!(c.backups[0].parsed);
    assert_eq!(c.backups[0].manifest.title, "Fallback");
    assert_eq!(c.backups[0].manifest.orig_folder, "SAVE_001");
    assert_eq!(c.backups[0].manifest.backup_time, "2026-10-15 12:34:56");
}
#[test]
fn invalid_names_and_delete() {
    let t = Temp::new();
    let s = t.service();
    source(&s);
    for n in [
        "../x", ".", "..", "C:", "a/b", "CON", "NUL.txt", "a.", "a ", "LPT1",
    ] {
        assert!(!simple_name(n), "{n}");
        assert!(s.restore("x.zip", n).is_err());
    }
    s.backup_as("SAVE_001", "ok.zip", "", "", "").unwrap();
    s.delete_backup("ok.zip").unwrap();
    s.delete_save("SAVE_001").unwrap();
    assert!(s.scan().unwrap().saves.is_empty());
}

#[test]
fn restore_to_new_folder_and_nested_backup_root_guard() {
    let t = Temp::new();
    let s = t.service();
    source(&s);
    s.backup_as("SAVE_001", "fresh.zip", "", "", "").unwrap();
    s.restore("fresh.zip", "SAVE_002").unwrap();
    assert_eq!(
        fs::read(s.root().join("SAVE_002/DATA/a.dat")).unwrap(),
        [0, 1, 2, 255]
    );
    let nested = SaveService::new(&s.game, s.root().join("SAVE_001/backups")).unwrap();
    assert!(nested.backup_as("SAVE_001", "bad.zip", "", "", "").is_err());
}
#[cfg(unix)]
#[test]
fn source_and_destination_symlinks_rejected() {
    use std::os::unix::fs::symlink;
    let t = Temp::new();
    let s = t.service();
    source(&s);
    symlink(t.0.join("elsewhere"), s.root().join("SAVE_001/link")).unwrap();
    assert!(s.backup_as("SAVE_001", "bad.zip", "", "", "").is_err());
    assert!(s.delete_save("SAVE_001").is_err());
}
#[test]
fn verify_backup_accepts_fresh_backup_and_reads_metadata() {
    let t = Temp::new();
    let s = t.service();
    source(&s);
    s.backup_as(
        "SAVE_001",
        "ok.zip",
        "Title",
        "ENDL_000",
        "2026-10-18T00:00:00+00:00",
    )
    .unwrap();
    let r = s.verify_backup("ok.zip").unwrap();
    assert!(r.is_ok(), "{:?}", r.errors);
    assert_eq!(r.title, "Title");
    assert_eq!(r.level, "ENDL_000");
    assert_eq!(r.entries, 4); // save.ini, DATA/a.dat, savepic.tga, manifest.json
}
#[test]
fn verify_backup_detects_bit_rot_via_crc_without_extracting() {
    let t = Temp::new();
    let s = t.service();
    fs::create_dir_all(&s.backups).unwrap();
    let ini = pfil(b"[titel]\nT\n[orglevelname]\nM\n");
    let payload = vec![0x41u8; 256];
    let mut z = ZipWriter::new(File::create(s.backups.join("rot.zip")).unwrap());
    let stored = FileOptions::default().compression_method(zip::CompressionMethod::Stored);
    z.start_file("save.ini", stored).unwrap();
    z.write_all(&ini).unwrap();
    z.start_file("DATA/blob.bin", stored).unwrap();
    z.write_all(&payload).unwrap();
    z.finish().unwrap();
    assert!(s.verify_backup("rot.zip").unwrap().is_ok());
    let path = s.backups.join("rot.zip");
    let mut raw = fs::read(&path).unwrap();
    let at = raw
        .windows(256)
        .position(|w| w == payload.as_slice())
        .unwrap();
    raw[at + 100] ^= 1;
    fs::write(&path, raw).unwrap();
    let r = s.verify_backup("rot.zip").unwrap();
    assert!(!r.is_ok());
    assert!(
        r.errors.iter().any(|e| e.contains("blob.bin")),
        "{:?}",
        r.errors
    );
    assert!(!s.root().join("SAVE_001").exists());
}
#[test]
fn verify_backup_flags_missing_save_ini_unsafe_paths_and_garbage() {
    let t = Temp::new();
    let s = t.service();
    archive(&s, "noini.zip", &[("DATA/a.dat", b"x")]);
    assert!(s
        .verify_backup("noini.zip")
        .unwrap()
        .errors
        .iter()
        .any(|e| e.contains("save.ini")));
    archive(
        &s,
        "evil.zip",
        &[("save.ini", b"x"), ("../escape.txt", b"x"), ("CON", b"x")],
    );
    assert!(s.verify_backup("evil.zip").unwrap().errors.len() >= 2);
    fs::write(s.backups.join("junk.zip"), b"not a zip at all").unwrap();
    let r = s.verify_backup("junk.zip").unwrap();
    assert!(!r.is_ok());
    assert!(s.verify_backup("../x.zip").is_err());
}
#[test]
fn verify_backup_warns_on_bad_manifest_but_stays_ok() {
    let t = Temp::new();
    let s = t.service();
    archive(
        &s,
        "m.zip",
        &[
            ("save.ini", b"[titel]\nT\n[orglevelname]\nL\n"),
            ("manifest.json", b"{broken"),
        ],
    );
    let r = s.verify_backup("m.zip").unwrap();
    assert!(r.is_ok());
    assert_eq!(r.warnings.len(), 1);
}
#[test]
fn verify_save_reports_state_of_live_folder() {
    let t = Temp::new();
    let s = t.service();
    assert!(!s.verify_save("SAVE_001").unwrap().is_ok());
    source(&s);
    let r = s.verify_save("SAVE_001").unwrap();
    assert!(r.is_ok());
    assert_eq!(r.title, "Title");
    assert!(r.warnings.iter().any(|w| w.contains("scr.dat")));
    fs::write(s.root().join("SAVE_001/save.ini"), b"PFIL\0\0").unwrap();
    assert!(!s.verify_save("SAVE_001").unwrap().is_ok());
    assert!(s.verify_save("a/b").is_err());
}
#[test]
fn simple_name_rejects_clock_dollar_com0_lpt0_and_leading_spaces() {
    for bad in [
        "CLOCK$",
        "clock$.txt",
        "COM0",
        "com0.ini",
        "LPT0",
        "lpt0.dat",
        " foo",
        "foo. ",
        "foo.",
        "CON",
        "PRN",
        "AUX",
        "NUL",
    ] {
        assert!(!simple_name(bad), "expected false for {bad}");
    }
    assert!(simple_name("valid_name.zip"));
    assert!(simple_name("SAVE_001"));
}
#[test]
fn restore_rollback_preserves_original_even_with_readonly_files() {
    let t = Temp::new();
    let s = t.service();
    source(&s);
    let original_ini = fs::read(s.root().join("SAVE_001/save.ini")).unwrap();
    let mut perm = fs::metadata(s.root().join("SAVE_001/save.ini"))
        .unwrap()
        .permissions();
    perm.set_readonly(true);
    fs::set_permissions(s.root().join("SAVE_001/save.ini"), perm).unwrap();

    archive(
        &s,
        "new.zip",
        &[("save.ini", b"[titel]\nNewTitle\n[orglevelname]\nNewLevel\n")],
    );

    let res = s.restore_impl("new.zip", "SAVE_001", || {
        fail("simulated crash before publish")
    });
    assert!(res.is_err());

    assert!(s.root().join("SAVE_001/save.ini").exists());
    let mut perm = fs::metadata(s.root().join("SAVE_001/save.ini"))
        .unwrap()
        .permissions();
    perm.set_readonly(false);
    let _ = fs::set_permissions(s.root().join("SAVE_001/save.ini"), perm);
    assert_eq!(
        fs::read(s.root().join("SAVE_001/save.ini")).unwrap(),
        original_ini
    );
}
#[test]
fn extract_decompression_bomb_truncated_safely() {
    let t = Temp::new();
    let s = t.service();
    fs::create_dir_all(&s.backups).unwrap();
    let path = s.backups.join("bomb.zip");

    let file = File::create(&path).unwrap();
    let mut z = ZipWriter::new(file);
    z.start_file(
        "save.ini",
        FileOptions::default().compression_method(zip::CompressionMethod::Deflated),
    )
    .unwrap();
    z.write_all(&vec![b'A'; 1024]).unwrap();
    z.finish().unwrap();

    let mut raw = fs::read(&path).unwrap();
    raw[22..26].copy_from_slice(&8u32.to_le_bytes());
    if let Some(pos) = raw.windows(4).position(|w| w == [0x50, 0x4b, 0x01, 0x02]) {
        raw[pos + 24..pos + 28].copy_from_slice(&8u32.to_le_bytes());
    }
    fs::write(&path, raw).unwrap();

    let dest = s.root().join("BOMB");
    let err = extract(&path, &dest).unwrap_err().to_string();
    assert!(err.contains("size mismatch"), "{err}");
    if let Ok(meta) = fs::metadata(dest.join("save.ini")) {
        assert!(meta.len() <= 9, "wrote too many bytes: {}", meta.len());
    }
}

#[test]
fn backup_preview_missing_backup_returns_none() {
    let t = Temp::new();
    let s = t.service();
    source(&s);
    assert_eq!(s.backup_preview("missing.zip").unwrap(), None);
}

fn make_dir_link(link: &Path, target: &Path) -> bool {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .is_ok_and(|o| o.status.success())
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
}
fn remove_dir_link(link: &Path) {
    let _ = fs::remove_dir(link).or_else(|_| fs::remove_file(link));
}

#[test]
fn scan_skips_linked_entries_and_keeps_valid_ones() {
    let t = Temp::new();
    let s = t.service();
    source(&s);
    s.backup_as("SAVE_001", "good.zip", "", "", "").unwrap();
    let outside = t.0.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(
        outside.join("save.ini"),
        pfil(b"[titel]\nX\n[orglevelname]\nM\n"),
    )
    .unwrap();
    let save_link = s.root().join("SAVE_LINK");
    let zip_link = s.backups.join("evil.zip");
    if !make_dir_link(&save_link, &outside) || !make_dir_link(&zip_link, &outside) {
        eprintln!("skipping: cannot create directory links");
        return;
    }
    let c = s.scan().unwrap();
    remove_dir_link(&save_link);
    remove_dir_link(&zip_link);
    assert_eq!(
        c.saves
            .iter()
            .map(|x| x.folder.as_str())
            .collect::<Vec<_>>(),
        ["SAVE_001"]
    );
    assert_eq!(
        c.backups
            .iter()
            .map(|x| x.file_name.as_str())
            .collect::<Vec<_>>(),
        ["good.zip"]
    );
    assert!(outside.join("save.ini").exists());
}

#[test]
fn scan_missing_game_path_never_creates_game_directories() {
    let t = Temp::new();
    let s = t.service();
    let c = s.scan().unwrap();
    assert!(c.saves.is_empty() && c.backups.is_empty());
    // Game install path (and its SAVE folder) must stay untouched by a scan.
    assert!(!s.game.exists());
    assert!(!s.root().exists());
    // C# SaveBackupService.Scan explicitly calls Directory.CreateDirectory(backupDirectory); kept for parity.
    assert!(s.backups.is_dir());
}

#[test]
fn malformed_save_ini_keeps_manifest_metadata_and_reports_unparsed() {
    let t = Temp::new();
    let s = t.service();
    // Valid PFIL header but truncated payload: read_save_info fails, like C# TryReadSaveInfo.
    let mut bad_ini = pfil(b"[titel]\nX\n[orglevelname]\nMAP\n");
    bad_ini.truncate(bad_ini.len() - 1);
    let manifest = br#"{"OrigFolder":"SAVE_007","Title":"Kept Title","Level":"","BackupTime":"2026-10-15T10:00:00"}"#;
    archive(
        &s,
        "bad_ini.zip",
        &[("save.ini", &bad_ini), ("manifest.json", manifest)],
    );
    // Unreadable manifest JSON: nothing from it survives, but the legacy file name still does.
    archive(
        &s,
        "Backup_SAVE_002_20261015_123456.zip",
        &[("save.ini", &bad_ini), ("manifest.json", b"{not json")],
    );
    let c = s.scan().unwrap();
    let b = c
        .backups
        .iter()
        .find(|b| b.file_name == "bad_ini.zip")
        .unwrap();
    assert!(!b.parsed);
    assert_eq!(b.manifest.orig_folder, "SAVE_007");
    assert_eq!(b.manifest.title, "Kept Title");
    assert_eq!(b.manifest.backup_time, "2026-10-15T10:00:00");
    let l = c
        .backups
        .iter()
        .find(|b| b.file_name.starts_with("Backup_"))
        .unwrap();
    assert!(!l.parsed);
    assert_eq!(l.manifest.orig_folder, "SAVE_002");
    assert_eq!(l.manifest.backup_time, "2026-10-15 12:34:56");
    assert!(l.manifest.title.is_empty());
    // Restore safety is independent of `parsed`: unsafe entries still fail closed.
    archive(
        &s,
        "unsafe.zip",
        &[
            ("save.ini", &bad_ini),
            ("../evil.txt", b"x"),
            ("manifest.json", manifest),
        ],
    );
    assert!(!s.verify_backup("unsafe.zip").unwrap().is_ok());
    assert!(s.restore("unsafe.zip", "SAVE_007").is_err());
    assert!(!s.root().join("SAVE_007").exists());
}

mod temp_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/temp_unique.rs"));
}
