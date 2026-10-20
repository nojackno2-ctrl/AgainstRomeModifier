//! Package verification and install hardening: tamper, symlink entries, registry atomicity.
use against_rome_packaging::*;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use zip::write::FileOptions;
use zip::{ZipArchive, ZipWriter};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("arm_pkg_hard_{}", Uuid::new_v4().simple()));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn bmp(path: &Path, w: i32, h: i32, fill: u8) {
    let stride = ((w * 3 + 3) / 4) * 4;
    let size = stride * h;
    let mut b = vec![0u8; (54 + size) as usize];
    b[0] = b'B';
    b[1] = b'M';
    b[2..6].copy_from_slice(&(54 + size).to_le_bytes());
    b[10..14].copy_from_slice(&54u32.to_le_bytes());
    b[14..18].copy_from_slice(&40u32.to_le_bytes());
    b[18..22].copy_from_slice(&w.to_le_bytes());
    b[22..26].copy_from_slice(&h.to_le_bytes());
    b[26..28].copy_from_slice(&1u16.to_le_bytes());
    b[28..30].copy_from_slice(&24u16.to_le_bytes());
    b[34..38].copy_from_slice(&size.to_le_bytes());
    for v in b[54..].iter_mut() {
        *v = fill;
    }
    fs::write(path, b).unwrap();
}

fn map(dir: &Path) {
    fs::create_dir_all(dir.join("TEXT/US")).unwrap();
    fs::create_dir_all(dir.join("DATA")).unwrap();
    fs::create_dir_all(dir.join("SCRIPT")).unwrap();
    bmp(&dir.join("boden.bmp"), 257, 257, 128);
    bmp(&dir.join("collision.bmp"), 256, 256, 0);
    fs::write(dir.join("boden.ini"), "Waterlevel = 50.0\nWaterColor = 30 100 180\n").unwrap();
    fs::write(
        dir.join("TEXT/US/briefing.put"),
        "var:briefing_titel_1 = \"Hard\";\nvar:briefing_titel_2 = \"S\";\nvar:briefing_text = \"T\";\n",
    )
    .unwrap();
    for n in ["objects", "objdata", "pos"] {
        fs::write(dir.join(format!("DATA/{n}.dat")), vec![0u8; 64]).unwrap();
    }
    fs::write(dir.join("SCRIPT/ak_level.bci"), b"PFIL\0\0\0\0").unwrap();
    fs::write(
        dir.join("Endlos_005_Siedlung1.sdl"),
        "[settlement]\nname = MAPS/ENDL_005/Endlos_005_Siedlung1.sdl\n",
    )
    .unwrap();
}

fn fixture(t: &Temp) -> (PathBuf, PathBuf) {
    let src = t.0.join("src/MAPS/ENDL_005");
    map(&src);
    let zip = t.0.join("pkg.zip");
    ModBundleExporter::export_to_zip(
        &src,
        &zip,
        Some(&ModExportOptions { generate_thumbnail: false, ..Default::default() }),
    )
    .unwrap();
    let game = t.0.join("game");
    fs::create_dir_all(game.join("MAPS")).unwrap();
    (zip, game)
}

/// Copies every entry, letting `edit` replace the bytes of a named entry.
fn rewrite(zip: &Path, out: &Path, edit: impl Fn(&str, Vec<u8>) -> Vec<u8>) {
    let mut a = ZipArchive::new(File::open(zip).unwrap()).unwrap();
    let mut w = ZipWriter::new(File::create(out).unwrap());
    for i in 0..a.len() {
        let mut e = a.by_index(i).unwrap();
        let name = e.name().to_string();
        let mut b = vec![];
        e.read_to_end(&mut b).unwrap();
        w.start_file(&name, FileOptions::default()).unwrap();
        w.write_all(&edit(&name, b)).unwrap();
    }
    w.finish().unwrap();
}

fn maps_listing(game: &Path) -> Vec<String> {
    fs::read_dir(game.join("MAPS"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn verify_zip_accepts_good_package_and_touches_nothing() {
    let t = Temp::new();
    let (zip, game) = fixture(&t);
    let m = ModBundleExporter::verify_zip(&zip).unwrap();
    assert!(m.files.iter().any(|f| f.relative_path == "boden.bmp"));
    assert!(maps_listing(&game).is_empty());
}

#[test]
fn verify_zip_detects_same_size_payload_tampering() {
    let t = Temp::new();
    let (zip, _) = fixture(&t);
    let bad = t.0.join("bad.zip");
    rewrite(&zip, &bad, |n, mut b| {
        if n == "map/DATA/pos.dat" {
            b[0] ^= 0xFF; // same length, different hash
        }
        b
    });
    let e = ModBundleExporter::verify_zip(&bad).unwrap_err().to_string();
    assert!(e.contains("checksum mismatch"), "{e}");
}

#[test]
fn tampered_manifest_fails_checksum_gate() {
    let t = Temp::new();
    let (zip, _) = fixture(&t);
    let bad = t.0.join("bad.zip");
    let mut changed = false;
    let changed_ref = std::cell::Cell::new(false);
    rewrite(&zip, &bad, |n, b| {
        if n == "manifest.json" {
            let text = String::from_utf8(b).unwrap();
            assert!(text.contains("\"1.0.0\""), "{text}");
            changed_ref.set(true);
            text.replace("\"1.0.0\"", "\"9.9.9\"").into_bytes()
        } else {
            b
        }
    });
    changed |= changed_ref.get();
    assert!(changed);
    assert!(ModBundleExporter::verify_zip(&bad).is_err());
}

#[test]
fn symlink_entry_is_rejected_before_anything_is_staged() {
    let t = Temp::new();
    let (_, game) = fixture(&t);
    let evil = t.0.join("evil.zip");
    let mut w = ZipWriter::new(File::create(&evil).unwrap());
    w.start_file("map/boden.bmp", FileOptions::default().unix_permissions(0o120777)).unwrap();
    w.write_all(b"C:\\Windows\\win.ini").unwrap();
    w.finish().unwrap();
    // The Windows writer records a DOS host; mark the central header as Unix so the mode is honoured.
    let mut raw = fs::read(&evil).unwrap();
    let at = raw.windows(4).position(|w| w == [0x50, 0x4b, 0x01, 0x02]).unwrap();
    raw[at + 5] = 3;
    raw[at + 38..at + 42].copy_from_slice(&(0o120777u32 << 16).to_le_bytes()); // S_IFLNK
    fs::write(&evil, raw).unwrap();
    let e = ModBundleExporter::install_from_zip(&evil, &game, None).unwrap_err().to_string();
    assert!(e.contains("symbolic"), "{e}");
    assert!(maps_listing(&game).is_empty());
}

#[test]
fn install_publishes_valid_registry_and_leaves_no_temp_files() {
    let t = Temp::new();
    let (zip, game) = fixture(&t);
    let r = ModBundleExporter::install_from_zip(&zip, &game, None).unwrap();
    assert_eq!(r.installed_slot, 5);
    let reg_path = game.join("MAPS").join(CUSTOM_MAPS_REGISTRY_FILE);
    let entries: Vec<CustomMapEntry> = serde_json::from_str(&fs::read_to_string(&reg_path).unwrap()).unwrap();
    assert_eq!(entries.len(), 1);
    assert!(maps_listing(&game).iter().all(|n| !n.starts_with(".arm_")), "{:?}", maps_listing(&game));
    // A second install takes the next gap and keeps the first registry entry.
    let r2 = ModBundleExporter::install_from_zip(&zip, &game, None).unwrap();
    assert_eq!(r2.installed_slot, 6);
    let entries: Vec<CustomMapEntry> = serde_json::from_str(&fs::read_to_string(&reg_path).unwrap()).unwrap();
    assert_eq!(entries.iter().map(|e| e.slot).collect::<Vec<_>>(), vec![5, 6]);
}

#[test]
fn malformed_registry_blocks_install_and_is_preserved() {
    let t = Temp::new();
    let (zip, game) = fixture(&t);
    let reg = game.join("MAPS").join(CUSTOM_MAPS_REGISTRY_FILE);
    fs::write(&reg, "{not json").unwrap();
    assert!(ModBundleExporter::install_from_zip(&zip, &game, None).is_err());
    assert_eq!(fs::read_to_string(&reg).unwrap(), "{not json");
    assert!(!game.join("MAPS/ENDL_005").exists());
    assert!(maps_listing(&game).iter().all(|n| !n.starts_with(".arm_install")));
}

#[test]
fn failed_export_keeps_existing_package_and_leaves_no_temp() {
    let t = Temp::new();
    let (zip, _) = fixture(&t);
    let before = fs::read(&zip).unwrap();
    let src = t.0.join("src/MAPS/ENDL_005");
    fs::remove_file(src.join("boden.bmp")).unwrap(); // preflight now fails
    assert!(ModBundleExporter::export_to_zip(&src, &zip, None).is_err());
    assert_eq!(fs::read(&zip).unwrap(), before, "failed export must not clobber existing package");
    let leftovers = fs::read_dir(&t.0)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains("tmp_"))
        .count();
    assert_eq!(leftovers, 0);
}

#[test]
fn is_payload_path_blocks_devices_and_spaces() {
    for bad in ["CLOCK$", "COM0", "LPT0", " DATA/pos.dat", "boden.bmp ", "boden.bmp."] {
        assert!(!is_payload_path(bad), "expected false for {bad}");
    }
    assert!(is_payload_path("boden.bmp"));
    assert!(is_payload_path("DATA/pos.dat"));
    assert!(is_payload_path("SCRIPT/ak_level.bci"));
    assert!(is_payload_path("TEXT/US/briefing.put"));
}

#[test]
fn verify_zip_catches_corrupt_readme_or_thumbnail() {
    let t = Temp::new();
    let (zip, _) = fixture(&t);
    let mut raw = fs::read(&zip).unwrap();
    let pos = raw.windows(10).position(|w| w == b"README.txt").unwrap();
    // Corrupt compressed payload byte following the filename
    raw[pos + 12] ^= 0xFF;
    let bad = t.0.join("corrupt_readme.zip");
    fs::write(&bad, raw).unwrap();
    assert!(ModBundleExporter::verify_zip(&bad).is_err());
}

#[test]
fn install_from_zip_rejects_missing_or_invalid_zip_path() {
    let t = Temp::new();
    let (_, game) = fixture(&t);
    let missing = t.0.join("nonexistent.zip");
    assert!(ModBundleExporter::install_from_zip(&missing, &game, None).is_err());
}

#[test]
fn install_into_missing_game_dir_fails_without_creating_it() {
    let t = Temp::new();
    let (zip, _) = fixture(&t);
    let ghost = t.0.join("no_such_game");
    let e = ModBundleExporter::install_from_zip(&zip, &ghost, None).unwrap_err();
    assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
    assert!(!ghost.exists(), "installer must not create a game directory");
}

#[test]
fn sdl_glob_matches_csharp_endlos_star_siedlung_pattern() {
    let t = Temp::new();
    let dir = t.0.join("m");
    fs::create_dir_all(&dir).unwrap();
    let body = "name = MAPS/ENDL_005/x.sdl\n";
    // C# glob "Endlos_*_Siedlung*.sdl" needs a second underscore; "Endlos_Siedlung" must be untouched.
    fs::write(dir.join("Endlos_Siedlung1.sdl"), body).unwrap();
    fs::write(dir.join("Endlos_005_Siedlung1.sdl"), body).unwrap();
    ModBundleExporter::rewrite_known_files(&dir, "ENDL_005", "ENDL_006", "T").unwrap();
    assert_eq!(fs::read_to_string(dir.join("Endlos_Siedlung1.sdl")).unwrap(), body);
    assert!(fs::read_to_string(dir.join("Endlos_005_Siedlung1.sdl")).unwrap().contains("MAPS/ENDL_006/"));
}

#[test]
fn install_with_all_custom_slots_occupied_reports_no_free_slot() {
    let t = Temp::new();
    let (zip, game) = fixture(&t);
    for slot in 5..=999 {
        fs::create_dir_all(game.join("MAPS").join(format!("ENDL_{slot:03}"))).unwrap();
    }
    let before = maps_listing(&game);
    let e = ModBundleExporter::install_from_zip(&zip, &game, None).unwrap_err();
    assert_eq!(e.kind(), std::io::ErrorKind::StorageFull, "{e}");
    assert!(e.to_string().contains("沒有可用"), "{e}");
    assert_eq!(maps_listing(&game), before);
}

#[test]
fn catalog_lists_slots_titles_and_custom_flag() {
    let t = Temp::new();
    let maps = t.0.join("MAPS");
    for (name, marker, title) in [
        ("ENDL_007", true, Some("Seven")),
        ("ENDL_000", false, None),
        ("endl_005", true, None),
        ("ENDL_003", true, Some("Orig")),
        ("ENDL_12", true, None),
        ("ENDL_ABC", true, None),
    ] {
        let d = maps.join(name);
        fs::create_dir_all(d.join("TEXT").join("US")).unwrap();
        if marker {
            fs::write(d.join(".arm_custom_map"), "{}").unwrap();
        }
        if let Some(title) = title {
            fs::write(d.join("TEXT/US/briefing.put"), format!("var:briefing_titel_1 = \"{title}\";\r\n")).unwrap();
        }
    }
    fs::write(maps.join("ENDL_009"), "file, not a dir").unwrap();

    let list = list_endless_maps(&t.0).unwrap();
    let slots: Vec<i32> = list.iter().map(|m| m.slot).collect();
    assert_eq!(slots, vec![0, 3, 5, 7]);
    assert_eq!(list[3].display_name.as_deref(), Some("Seven"));
    assert_eq!(list[3].id(), "ENDL_007");
    assert!(list[3].is_custom && list[2].is_custom);
    assert!(!list[0].is_custom);
    assert!(!list[1].is_custom, "marker below slot 5 never counts as custom");
    assert_eq!(list[2].display_name, None);
}

#[test]
fn catalog_errors_are_explicit() {
    let t = Temp::new();
    assert!(list_endless_maps(&t.0).unwrap().is_empty(), "missing MAPS is empty");
    assert_eq!(list_endless_maps(&t.0.join("nope")).unwrap_err().kind(), std::io::ErrorKind::NotFound);
    assert_eq!(list_endless_maps(Path::new("")).unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
    fs::create_dir_all(t.0.join("MAPS/ENDL_006")).unwrap();
    assert_eq!(require_endless_map(&t.0, 6).unwrap().slot, 6);
    assert_eq!(require_endless_map(&t.0, 8).unwrap_err().kind(), std::io::ErrorKind::NotFound);
    assert_eq!(require_endless_map(&t.0, -1).unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(require_endless_map(&t.0, 1000).unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
}
