//! Original-file backup dictionary contracts from BackupValidator / BackupAutoHealer.
//! These are distinct from the save ZIP manifest. Existing entries are never rewritten.
use crate::{absolute, fail, reject_links, Result};
use std::{collections::BTreeMap, fs, path::Path};

pub type BackupFiles = BTreeMap<String, Vec<u8>>;
pub const REQUIRED_FILES: &[&str] = &[
    "Against_Rome.exe",
    "SYSTEM/cl_script.ini",
    "SYSTEM/cl_epara.ini",
    "SYSTEM/ress.ini",
    "SYSTEM/DATA_MP/DEFAULTS/objdef.dau",
    "SYSTEM/CLMK/icon.ini",
    "SYSTEM/CLAK/cl_scint.ini",
    "SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci",
];
pub const PARTGEO: &str = "SYSTEM/DATA_MP/DEFAULTS/partgeo.dau";
fn contains(files: &BackupFiles, key: &str) -> bool {
    files.keys().any(|k| k.eq_ignore_ascii_case(key))
}
fn has_team(files: &BackupFiles) -> bool {
    files.keys().any(|k| {
        let k = k.to_ascii_lowercase();
        k.starts_with("maps/") && k.ends_with("team.dat")
    })
}
/// Loads an original-file Backup.zip into a dictionary (C# BackupSourceLoader.LoadZip).
/// Unsafe paths, duplicate (case-insensitive) keys, and entries whose real length differs
/// from the header or exceeds the limits are errors instead of being silently accepted.
pub fn load_zip(reader: impl std::io::Read + std::io::Seek) -> Result<BackupFiles> {
    use std::io::Read;
    const ENTRY_LIMIT: u64 = 256 * 1024 * 1024;
    const TOTAL_LIMIT: u64 = 1024 * 1024 * 1024;
    let mut zip = zip::ZipArchive::new(reader)?;
    if zip.len() > 100000 {
        return fail("Backup.zip has too many entries");
    }
    let mut files = BackupFiles::new();
    let mut seen = std::collections::HashSet::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut e = zip.by_index(i)?;
        if let Some(mode) = e.unix_mode() {
            let ty = mode & 0o170000;
            if ty != 0 && ty != 0o100000 && ty != 0o040000 {
                return fail(format!("Backup.zip symbolic/special link rejected: {}", e.name()));
            }
        }
        let key = e.name().replace('\\', "/");
        let trim_key = key.trim_end_matches('/');
        let bad = trim_key.is_empty()
            || key.starts_with('/')
            || trim_key.split('/').any(|p| !crate::simple_name(p));
        if bad {
            return fail(format!("Backup.zip contains an unsafe entry path: {}", e.name()));
        }
        if e.is_dir() {
            continue;
        }
        if !seen.insert(key.to_ascii_lowercase()) {
            return fail(format!("Backup.zip contains a duplicate entry: {key}"));
        }
        let declared = e.size();
        total = total.saturating_add(declared);
        if declared > ENTRY_LIMIT || total > TOTAL_LIMIT {
            return fail("Backup.zip exceeds size limits");
        }
        let mut buf = Vec::new();
        e.by_ref().take(declared.saturating_add(1)).read_to_end(&mut buf)?;
        if buf.len() as u64 != declared {
            return fail(format!("Backup.zip entry size mismatch: {key}"));
        }
        files.insert(key, buf);
    }
    Ok(files)
}
pub fn find_missing(files: &BackupFiles) -> Vec<String> {
    let mut missing: Vec<_> = REQUIRED_FILES
        .iter()
        .filter(|&&k| !contains(files, k))
        .map(|&k| k.to_owned())
        .collect();
    if !has_team(files) {
        missing.push("MAPS/.../team.dat".into());
    }
    missing
}
pub fn validate(files: &BackupFiles) -> Result<()> {
    let m = find_missing(files);
    if m.is_empty() {
        Ok(())
    } else {
        fail(format!("Backup incomplete\r\n{}", m.join("\r\n")))
    }
}
#[derive(Debug, Default)]
pub struct HealReport {
    pub changed: bool,
    pub recovered: Vec<String>,
    pub errors: Vec<String>,
}

/// Heal missing entries from .bak first, then the current file; epara comes only
/// from the caller's clean factory. Source tree is read-only, including partgeo.
/// The C# incidental creation of partgeo.dau.bak is intentionally omitted to
/// honor the read-only game-directory contract; recovered bytes remain in memory.
pub fn heal(
    game: impl AsRef<Path>,
    files: &mut BackupFiles,
    clean_epara: impl FnOnce() -> Result<Vec<u8>>,
) -> Result<HealReport> {
    let game = absolute(game.as_ref())?;
    let mut report = HealReport::default();
    if !game.is_dir() {
        return Ok(report);
    }
    let mut factory = Some(clean_epara);
    for relative in REQUIRED_FILES.iter().copied().chain([PARTGEO]) {
        if contains(files, relative) {
            continue;
        }
        let result = (|| -> Result<Option<Vec<u8>>> {
            let bytes = if relative == "SYSTEM/cl_epara.ini" {
                factory.take().unwrap()()?
            } else {
                let p = game.join(relative);
                let bak = game.join(format!("{relative}.bak"));
                reject_links(&p)?;
                reject_links(&bak)?;
                let load = if bak.is_file() { bak } else { p };
                if !load.is_file() {
                    return Ok(None);
                }
                fs::read(load)?
            };
            if relative == PARTGEO && !is_partgeo_original(&bytes) {
                return fail("partgeo.dau is already modified or invalid");
            }
            Ok(Some(bytes))
        })();
        match result {
            Ok(Some(b)) => {
                files.insert(relative.into(), b);
                report.recovered.push(relative.into());
            }
            Ok(None) => (),
            Err(e) => report.errors.push(format!("{relative}: {e}")),
        }
    }
    if !has_team(files) && game.join("MAPS").is_dir() {
        // C# excludes a whole owning map, including nested files, when marked custom.
        match team_files(&game.join("MAPS"), &game, files, &mut report) {
            Ok(()) => (),
            Err(e) => report.errors.push(format!("team.dat: {e}")),
        }
    }
    report.changed = !report.recovered.is_empty();
    Ok(report)
}
fn team_files(
    dir: &Path,
    game: &Path,
    files: &mut BackupFiles,
    report: &mut HealReport,
) -> Result<()> {
    reject_links(dir)?;
    for e in fs::read_dir(dir)? {
        let e = e?;
        let p = e.path();
        reject_links(&p)?;
        if e.file_type()?.is_dir() {
            if dir == game.join("MAPS") {
                let marker = p.join(".arm_custom_map");
                reject_links(&marker)?;
                if marker.is_file() {
                    continue;
                }
            }
            team_files(&p, game, files, report)?;
        } else if e.file_name().eq_ignore_ascii_case("team.dat") {
            let bak = p.with_file_name(format!("{}.bak", e.file_name().to_string_lossy()));
            reject_links(&bak)?;
            let load = if bak.is_file() { bak } else { p.clone() };
            let relative = p
                .strip_prefix(game)?
                .to_str()
                .ok_or("non UTF-8 path")?
                .replace('\\', "/");
            files.insert(relative.clone(), fs::read(load)?);
            report.recovered.push(relative);
        }
    }
    Ok(())
}
pub fn is_partgeo_original(raw: &[u8]) -> bool {
    let decoded = if raw.len() >= 64 && raw.starts_with(b"PFIL") {
        match against_rome_data::lzss::decompress_pfil(raw, None) {
            Ok(b) => b,
            Err(_) => return false,
        }
    } else {
        raw.to_vec()
    };
    let (text, _, _) = encoding_rs::WINDOWS_1251.decode(&decoded);
    for line in text.lines() {
        let cols: Vec<_> = line.split(',').collect();
        if cols.len() > 12 && cols[2].trim() == "Pfeil00" {
            return cols[12].trim().parse::<i64>() == Ok(5832704);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::Temp;
    fn put(root: &Path, name: &str, data: &[u8]) {
        let p = root.join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, data).unwrap();
    }
    fn complete() -> BackupFiles {
        let mut f = BackupFiles::new();
        for k in REQUIRED_FILES {
            f.insert(k.to_string(), vec![]);
        }
        f.insert("mApS/ENDL_000/TEAM.DAT".into(), vec![]);
        f
    }
    #[test]
    fn validator_requires_eight_files_and_any_team_but_not_partgeo() {
        let mut f = complete();
        assert!(find_missing(&f).is_empty());
        validate(&f).unwrap();
        f.remove("SYSTEM/ress.ini");
        assert_eq!(find_missing(&f), ["SYSTEM/ress.ini"]);
        f.clear();
        assert_eq!(find_missing(&f).len(), 9);
        assert!(validate(&f).is_err());
    }
    #[test]
    fn case_insensitive_existing_entries_are_preserved_byte_exactly() {
        let t = Temp::new();
        let mut f = BackupFiles::new();
        for (k, v) in complete() {
            f.insert(k.to_ascii_lowercase(), v);
        }
        let before = f.clone();
        assert!(validate(&f).is_ok());
        let report = heal(&t.0, &mut f, || panic!("factory must not run")).unwrap();
        assert!(!report.changed);
        assert_eq!(f, before);
    }
    #[test]
    fn healer_prefers_bak_uses_clean_factory_and_excludes_custom_maps() {
        let t = Temp::new();
        put(&t.0, "Against_Rome.exe", b"modified");
        put(&t.0, "Against_Rome.exe.bak", b"original");
        put(&t.0, "SYSTEM/cl_epara.ini.bak", b"wrong");
        put(&t.0, "MAPS/ENDL_000/TEAM.dat", b"current");
        put(&t.0, "MAPS/ENDL_000/TEAM.dat.bak", b"baseline");
        put(&t.0, "MAPS/ENDL_005/.arm_custom_map", b"");
        put(&t.0, "MAPS/ENDL_005/sub/team.dat", b"custom");
        let mut f = BackupFiles::new();
        let report = heal(&t.0, &mut f, || Ok(b"clean".to_vec())).unwrap();
        assert!(report.changed);
        assert_eq!(report.recovered.len(), 3);
        assert_eq!(f["Against_Rome.exe"], b"original");
        assert_eq!(f["SYSTEM/cl_epara.ini"], b"clean");
        assert_eq!(f["MAPS/ENDL_000/TEAM.dat"], b"baseline");
        assert!(!f.contains_key("MAPS/ENDL_005/sub/team.dat"));
        assert_eq!(fs::read(t.0.join("Against_Rome.exe")).unwrap(), b"modified");
    }
    #[test]
    fn healer_does_not_load_more_teams_when_any_team_is_present() {
        let t = Temp::new();
        put(&t.0, "MAPS/ENDL_000/team.dat", b"new");
        let mut f = complete();
        let original = f.clone();
        assert!(!heal(&t.0, &mut f, || panic!()).unwrap().changed);
        assert_eq!(f, original);
    }
    #[test]
    fn clean_factory_failure_is_reported_and_other_files_still_recover() {
        let t = Temp::new();
        put(&t.0, "SYSTEM/ress.ini", b"resource");
        let mut f = BackupFiles::new();
        let report = heal(&t.0, &mut f, || fail("factory unavailable")).unwrap();
        assert!(report.changed);
        assert_eq!(report.errors.len(), 1);
        assert_eq!(f["SYSTEM/ress.ini"], b"resource");
        assert!(!f.contains_key("SYSTEM/cl_epara.ini"));
    }
    fn partgeo(value: &str) -> Vec<u8> {
        let mut cols = vec!["0"; 13];
        cols[2] = "Pfeil00";
        cols[12] = value;
        cols.join(",").into_bytes()
    }
    #[test]
    fn partgeo_modified_baseline_rejected_original_recovers_without_source_write() {
        let t = Temp::new();
        let mut f = complete();
        put(&t.0, PARTGEO, &partgeo("1"));
        let r = heal(&t.0, &mut f, || panic!()).unwrap();
        assert!(!r.changed);
        assert_eq!(r.errors.len(), 1);
        assert!(!is_partgeo_original(b"invalid"));
        assert!(!is_partgeo_original(b"PFIL"));
        let original = partgeo("5832704");
        put(&t.0, PARTGEO, &original);
        let r = heal(&t.0, &mut f, || panic!()).unwrap();
        assert!(r.changed);
        assert_eq!(f[PARTGEO], original);
        assert!(!t.0.join(format!("{PARTGEO}.bak")).exists());
        assert_eq!(fs::read(t.0.join(PARTGEO)).unwrap(), original);
    }
    #[test]
    fn missing_game_does_not_invoke_factory_or_change_dictionary() {
        let t = Temp::new();
        let mut f = BackupFiles::new();
        assert!(
            !heal(t.0.join("missing"), &mut f, || panic!())
                .unwrap()
                .changed
        );
        assert!(f.is_empty());
    }
}

#[cfg(test)]
mod load_zip_tests {
    use super::*;
    use std::io::{Cursor, Write};
    fn zip_of(entries: &[(&str, &[u8])]) -> Cursor<Vec<u8>> {
        let mut z = zip::ZipWriter::new(Cursor::new(vec![]));
        for (n, b) in entries {
            z.start_file(*n, zip::write::FileOptions::default()).unwrap();
            z.write_all(b).unwrap();
        }
        Cursor::new(z.finish().unwrap().into_inner())
    }
    #[test]
    fn loads_and_normalizes_keys_then_feeds_validator() {
        let mut entries: Vec<(String, &[u8])> = REQUIRED_FILES.iter().map(|k| (k.replace('/', "\\"), &b"x"[..])).collect();
        entries.push(("MAPS/ENDL_000/team.dat".into(), b"t"));
        let refs: Vec<(&str, &[u8])> = entries.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        let files = load_zip(zip_of(&refs)).unwrap();
        assert!(files.contains_key("SYSTEM/cl_script.ini"));
        validate(&files).unwrap();
    }
    #[test]
    fn rejects_traversal_rooted_drive_and_case_duplicates() {
        for bad in ["../x", "/abs", "C:/x", "a/../b", "a//b"] {
            assert!(load_zip(zip_of(&[(bad, b"x")])).is_err(), "{bad}");
        }
        assert!(load_zip(zip_of(&[("A/b.ini", b"1"), ("a/B.ini", b"2")])).is_err());
    }
    #[test]
    fn incomplete_zip_is_reported_by_find_missing() {
        let files = load_zip(zip_of(&[("Against_Rome.exe", b"x")])).unwrap();
        let m = find_missing(&files);
        assert!(m.contains(&"SYSTEM/ress.ini".to_string()));
        assert!(m.contains(&"MAPS/.../team.dat".to_string()));
        assert!(validate(&files).is_err());
    }
    #[test]
    fn rejects_devices_symlinks_and_non_simple_names() {
        for bad in ["CON", "CON/", "CLOCK$", "CLOCK$/", "COM0", "LPT0", "SYSTEM/./cl_script.ini", " foo.ini", "../", "SYSTEM/../evil"] {
            assert!(load_zip(zip_of(&[(bad, b"x")])).is_err(), "expected error for {bad}");
        }
        let mut z = zip::ZipWriter::new(Cursor::new(vec![]));
        z.start_file("SYSTEM/cl_script.ini", zip::write::FileOptions::default().unix_permissions(0o120777)).unwrap();
        z.write_all(b"symlink").unwrap();
        let mut raw = z.finish().unwrap().into_inner();
        let at = raw.windows(4).position(|w| w == [0x50, 0x4b, 0x01, 0x02]).unwrap();
        raw[at + 5] = 3;
        raw[at + 38..at + 42].copy_from_slice(&(0o120777u32 << 16).to_le_bytes());
        assert!(load_zip(Cursor::new(raw)).is_err());
    }
}
