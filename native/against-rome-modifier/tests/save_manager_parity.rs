//! Synthetic disk regressions for the C# SaveBackupService/CLI contract; no installed files.
use against_rome_modifier::{cli::run_cli, save_manager::SaveBackupService, tga};
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::PathBuf,

};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let dir = crate::temp_fixture::dir("arm-save-parity");
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn service(&self) -> SaveBackupService {
        SaveBackupService::new(&self.0, &self.0.join("SAVE_BACKUP")).unwrap()
    }
    fn put(&self, path: &str, bytes: &[u8]) {
        let p = self.0.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, bytes).unwrap();
    }
    fn save(&self) {
        self.put(
            "SAVE/ESAVE_000/save.ini",
            b"[titel]\r\nTest save\r\n[orglevelname]\r\nENDL_000\r\n",
        );
        self.put("SAVE/ESAVE_000/DATA/objdata", b"original objects");
        self.put("SAVE/ESAVE_000/savepic.tga", &thumbnail());
    }
    fn cli(&self, tail: &[&str]) -> (i32, Value) {
        let mut args: Vec<String> = tail.iter().map(|s| s.to_string()).collect();
        args.extend([
            "--game".into(),
            self.0.to_string_lossy().into_owned(),
            "--json".into(),
        ]);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_cli(&args, &mut out, &mut err);
        assert!(err.is_empty(), "{}", String::from_utf8_lossy(&err));
        (code, serde_json::from_slice(&out).unwrap())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn thumbnail() -> Vec<u8> {
    let mut t = vec![0; 18];
    t[2] = 2;
    t[12] = 1;
    t[14] = 1;
    t[16] = 32;
    t[17] = 0x20;
    t.extend([9, 8, 7, 6]);
    t
}

#[test]
fn backup_restore_preview_delete_preserve_nested_bytes_and_catalog() {
    let f = Fixture::new();
    f.save();
    let svc = f.service();
    let cat = svc.scan().unwrap();
    let s = &cat.saves[0];
    assert_eq!(
        (&*s.title, &*s.level, s.parsed),
        ("Test save", "ENDL_000", true)
    );
    assert!(s.last_write_time.contains('T'));
    let name = svc.create_backup(&s.folder, &s.title, &s.level).unwrap();
    let b = svc.scan().unwrap().backups.remove(0);
    assert_eq!(
        (&*b.orig_folder, &*b.title, &*b.level, b.parsed),
        ("ESAVE_000", "Test save", "ENDL_000", true)
    );
    assert_eq!(b.backup_time.len(), 19);
    assert_eq!(svc.read_backup_preview(&name).unwrap(), Some(thumbnail()));
    assert_eq!(
        svc.decode_save_preview("ESAVE_000").unwrap(),
        svc.decode_backup_preview(&name).unwrap()
    );
    f.put("SAVE/ESAVE_000/DATA/objdata", b"changed");
    f.put("SAVE/ESAVE_000/stale", b"must disappear");
    assert!(svc.restore_backup(&name, "ESAVE_000").unwrap().is_empty());
    assert_eq!(
        fs::read(f.0.join("SAVE/ESAVE_000/DATA/objdata")).unwrap(),
        b"original objects"
    );
    assert!(!f.0.join("SAVE/ESAVE_000/stale").exists());
    svc.delete_save("ESAVE_000").unwrap();
    svc.delete_backup(&name).unwrap();
    let cat = svc.scan().unwrap();
    assert!(cat.saves.is_empty() && cat.backups.is_empty());
}

#[test]
fn cli_exposes_save_manager_lifecycle_and_operation_results() {
    let f = Fixture::new();
    f.save();
    let (code, v) = f.cli(&["saves", "backup", "ESAVE_000"]);
    assert_eq!(code, 0, "{v}");
    let archive = v["data"]["operation"]["fileName"].as_str().unwrap();
    let (code, p) = f.cli(&["saves", "preview-backup", archive]);
    assert_eq!(code, 0);
    assert_eq!(p["data"]["width"], 1);
    assert_eq!(p["data"]["height"], 1);
    let (code, v) = f.cli(&["saves", "restore", archive, "RESTORED"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        v["data"]["operation"]["cleanupWarnings"],
        serde_json::json!([])
    );
    assert_eq!(
        fs::read(f.0.join("SAVE/RESTORED/DATA/objdata")).unwrap(),
        b"original objects"
    );
    assert_eq!(f.cli(&["saves", "delete", "RESTORED"]).0, 0);
    assert_eq!(f.cli(&["saves", "delete-backup", archive]).0, 0);
    assert!(!f.0.join("SAVE/RESTORED").exists());
    assert!(!f.0.join("SAVE_BACKUP").join(archive).exists());
}

#[test]
fn invalid_cli_names_missing_targets_and_dry_run_do_not_create_directories() {
    let f = Fixture::new();
    for args in [
        vec!["saves", "restore", "a.zip"],
        vec!["saves", "delete", "../escape"],
        vec!["saves", "delete-backup", "NUL"],
        vec!["saves", "backup"],
        vec!["saves", "typo"],
        vec!["saves", "backup", "ESAVE_000", "--dry-run"],
        vec!["saves", "restore", "a.zip", ".."],
        vec!["saves", "delete", "ESAVE_000", "typo"],
        vec!["saves", "restore", "a.zip", "ESAVE_000", "typo"],
    ] {
        let (code, v) = f.cli(&args);
        assert_eq!(code, 2, "{args:?}: {v}");
        assert_eq!(v["success"], false);
        assert_eq!(fs::read_dir(&f.0).unwrap().count(), 0);
    }
}

#[test]
fn corrupt_zip_and_zip_traversal_never_replace_existing_save() {
    let f = Fixture::new();
    f.save();
    let svc = f.service();
    f.put("SAVE_BACKUP/bad.zip", b"not zip");
    assert!(!svc.scan().unwrap().backups[0].parsed);
    assert!(svc.restore_backup("bad.zip", "ESAVE_000").is_err());
    let file = fs::File::create(f.0.join("SAVE_BACKUP/escape.zip")).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    for (name, bytes) in [("save.ini", "[titel]\nInjected"), ("../outside", "escape")] {
        zip.start_file(name, zip::write::FileOptions::default())
            .unwrap();
        zip.write_all(bytes.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
    assert!(svc.restore_backup("escape.zip", "ESAVE_000").is_err());
    assert_eq!(
        fs::read(f.0.join("SAVE/ESAVE_000/DATA/objdata")).unwrap(),
        b"original objects"
    );
    assert!(!f.0.join("SAVE/outside").exists());
    assert_eq!(fs::read_dir(f.0.join("SAVE")).unwrap().count(), 1);
}

#[test]
fn missing_unsupported_and_io_failed_previews_remain_distinct() {
    let f = Fixture::new();
    f.save();
    let svc = f.service();
    assert!(svc.decode_save_preview("MISSING").unwrap().is_none());
    f.put("SAVE/ESAVE_000/savepic.tga", b"bad");
    assert!(svc.decode_save_preview("ESAVE_000").unwrap().is_none());
    f.put("SAVE_BACKUP/corrupt.zip", b"bad");
    assert!(svc.decode_backup_preview("corrupt.zip").is_err());
    assert!(svc.decode_save_preview("../escape").is_err());
    let mut huge = vec![0; 18];
    huge[2] = 2;
    huge[16] = 24;
    huge[12..14].copy_from_slice(&20000u16.to_le_bytes());
    huge[14..16].copy_from_slice(&20000u16.to_le_bytes());
    assert!(tga::decode(&huge).is_none()); // no 1.6 GB allocation for an 18-byte file
}

#[cfg(test)]
mod temp_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/temp_unique.rs"));
}
