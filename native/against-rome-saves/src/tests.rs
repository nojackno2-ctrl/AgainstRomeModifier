use super::*;
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = unique(&std::env::temp_dir(), "ar_saves_test");
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn service(&self) -> SaveService {
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
