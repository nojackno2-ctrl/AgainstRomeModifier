//! Synthetic coverage for services, install features, detectors and the Endless AI orchestrator.
use against_rome_modifier::{
    detectors::FeatureDetector,
    endless_ai::{EndlessAiOrchestrator, PatchState},
    exe_patches::ALL_EXE_PATCHES,
    install::*,
    services::*,
    PatchProfile,
};
use std::{fs, io::Write, path::{Path, PathBuf}, sync::atomic::{AtomicUsize, Ordering}};

static N: AtomicUsize = AtomicUsize::new(0);
struct Tmp(PathBuf);
impl Tmp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("arm-svc-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        Tmp(p)
    }
}
impl Drop for Tmp { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
fn put(root: &Path, rel: &str, bytes: &[u8]) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, bytes).unwrap();
}

#[test]
fn locator_prefers_existing_preferred_then_base_exe_then_registry_then_default() {
    let t = Tmp::new();
    let (pref, base, def) = (t.0.join("pref"), t.0.join("base"), t.0.join("def"));
    for d in [&pref, &base, &def] { fs::create_dir_all(d).unwrap(); }
    let none = Path::new("");
    assert_eq!(GameDirectoryLocator::resolve_with(Some(&pref), &base, none, &def), pref);
    assert_eq!(GameDirectoryLocator::resolve_with(Some(&t.0.join("missing")), &base, none, &def), def);
    put(&base, "Against_Rome.exe", b"x");
    assert_eq!(GameDirectoryLocator::resolve_with(None, &base, Path::new("R:/reg"), &def), base);
    fs::remove_file(base.join("Against_Rome.exe")).unwrap();
    assert_eq!(GameDirectoryLocator::resolve_with(None, &base, Path::new("R:/reg"), &def), PathBuf::from("R:/reg"));
    assert_eq!(GameDirectoryLocator::resolve_with(None, &base, none, &t.0.join("nodef")), PathBuf::new());
}

#[test]
fn backup_manager_zip_roundtrip_case_insensitive_and_rejects_traversal() {
    let mut buf = Vec::new();
    {
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let o = zip::write::FileOptions::default();
        z.start_file("SYSTEM\\cl_script.ini", o).unwrap();
        z.write_all(b"abc").unwrap();
        z.finish().unwrap();
    }
    let files = BackupManager::load_zip(&buf).unwrap();
    let mut b = BackupManager::new();
    for (k, v) in files { b.set_backup_file(&k, v); }
    assert!(b.has_file("system/CL_SCRIPT.ini"));
    assert_eq!(b.get_backup_bytes("SYSTEM/cl_script.ini").unwrap(), b"abc");
    assert!(b.get_backup_bytes("nothing").is_err());
    assert!(!b.find_missing_backup_resources().is_empty());
    assert!(b.validate_backup_resources().is_err());

    let mut bad = Vec::new();
    {
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut bad));
        z.start_file("../evil.txt", zip::write::FileOptions::default()).unwrap();
        z.write_all(b"x").unwrap();
        z.finish().unwrap();
    }
    assert!(BackupManager::load_zip(&bad).is_err());
}

#[test]
fn custom_map_marker_excludes_files() {
    let t = Tmp::new();
    put(&t.0, "MAPS/ENDL_005/.arm_custom_map", b"");
    put(&t.0, "MAPS/ENDL_000/team.dat", b"");
    let maps = t.0.join("MAPS");
    assert!(is_custom_map_file(&maps, &maps.join("ENDL_005/team.dat")));
    assert!(!is_custom_map_file(&maps, &maps.join("ENDL_000/team.dat")));
}

#[test]
fn validators_reject_garbage() {
    assert!(!OriginalFileValidator::is_exe_original(b"MZ"));
    assert!(!OriginalFileValidator::is_objdef_original(b"not pfil"));
    assert!(!OriginalFileValidator::is_partgeo_original(b""));
}

#[test]
fn exe_timestamp_and_trace_ini() {
    let mut pe = vec![0u8; 0x100];
    pe[0x3c] = 0x80;
    pe[0x80..0x84].copy_from_slice(b"PE\0\0");
    pe[0x88..0x8c].copy_from_slice(&ArgmTraceFeature::KNOWN_TIMESTAMP.to_le_bytes());
    assert_eq!(exe_timestamp(&pe), Some(ArgmTraceFeature::KNOWN_TIMESTAMP));
    assert_eq!(exe_timestamp(&pe[..0x40]), None);
    assert!(build_trace_ini(0x404d1710).contains("expectedTimeDateStamp=404D1710"));
    assert!(build_trace_ini(0).contains("expectedTimeDateStamp=0"));
}

#[test]
fn language_pack_enable_disable_restores_exactly() {
    let t = Tmp::new();
    put(&t.0, "ToEng/TEXT/a.txt", b"english");
    put(&t.0, "ToEng/new.txt", b"new");
    put(&t.0, "TEXT/a.txt", b"original");
    let plan = LanguagePackFeature::plan(&t.0, true).unwrap();
    for (p, b) in &plan.writes { fs::create_dir_all(p.parent().unwrap()).unwrap(); fs::write(p, b).unwrap(); }
    assert_eq!(fs::read(t.0.join("TEXT/a.txt")).unwrap(), b"english");
    assert_eq!(language_overlay_state(&t.0).unwrap(), Some(true));
    // re-enabling while already enabled keeps the original backup
    assert!(LanguagePackFeature::plan(&t.0, true).is_ok());
    let plan = LanguagePackFeature::plan(&t.0, false).unwrap();
    for (p, b) in &plan.writes { fs::write(p, b).unwrap(); }
    for p in &plan.deletes { fs::remove_file(p).unwrap(); }
    assert_eq!(fs::read(t.0.join("TEXT/a.txt")).unwrap(), b"original");
    assert!(!t.0.join("new.txt").exists());
}

#[test]
fn language_paths_reject_traversal_and_streams() {
    let root = Path::new("C:/g");
    for bad in ["../x", "a/../../x", "/abs", "C:/x", "a:stream", ""] {
        assert!(safe_language_path(root, bad).is_err(), "{bad}");
    }
    assert!(safe_language_path(root, "TEXT/US/a.txt").is_ok());
}

#[test]
fn widescreen_settings_require_exactly_one_key_and_keep_bom() {
    let conf = "\u{feff}FullScreenMode = false\r\nScalingMode=original\r\nCenterAppWindow=false\r\nWindowedAttributes=x\r\nFullscreenAttributes=y\r\n";
    let out = apply_native_widescreen_window(conf.as_bytes()).unwrap();
    assert!(out.starts_with(&[239, 187, 191]) || out.starts_with("\u{feff}".as_bytes()));
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("FullScreenMode = true") || text.contains("FullScreenMode=true"));
    assert!(text.contains("ScalingMode=stretched_ar") && text.contains("FullscreenAttributes=fake"));
    assert!(apply_native_widescreen_window(b"FullScreenMode=true\r\n").is_err());
}

fn resources() -> InstallResources {
    let mut r = InstallResources::default();
    for n in ["D3D8.dll", "DDraw.dll", "dgVoodooCpl.exe"] { r.dgvoodoo.insert(n.into(), n.as_bytes().to_vec()); }
    r.dgvoodoo.insert("dgVoodoo.conf".into(), b"FullScreenMode=false\nScalingMode=a\nCenterAppWindow=false\nWindowedAttributes=\nFullscreenAttributes=\n".to_vec());
    r.argm_winmm = b"winmm-bytes".to_vec();
    r
}
fn commit(plan: &InstallPlan) {
    for (p, b) in &plan.writes { fs::create_dir_all(p.parent().unwrap()).unwrap(); fs::write(p, b).unwrap(); }
    for p in &plan.deletes { let _ = fs::remove_file(p); }
}

#[test]
fn dgvoodoo_install_detect_conflict_and_uninstall() {
    let t = Tmp::new();
    let plan = DgVoodooFeature::plan(&t.0, true, true, &resources()).unwrap();
    commit(&plan);
    assert!(DgVoodooFeature::is_installed(&t.0));
    assert!(DgVoodooFeature::is_centered_presentation_configured(&t.0));
    let mut profile = PatchProfile::new();
    InstallFeatureDetector::detect(&t.0, &mut profile);
    assert!(profile.get_bool("DgVoodoo") && profile.get_bool("NativeWidescreen1920x1080"));
    // user-modified dll is a conflict; modified conf is preserved
    fs::write(t.0.join("dgVoodoo.conf"), b"user edits\n").unwrap();
    assert!(DgVoodooFeature::plan(&t.0, true, false, &resources()).is_ok());
    fs::write(t.0.join("D3D8.dll"), b"changed").unwrap();
    assert!(DgVoodooFeature::plan(&t.0, true, false, &resources()).is_err());
    // uninstall leaves the changed dll but removes untouched files
    commit(&DgVoodooFeature::plan(&t.0, false, false, &resources()).unwrap());
    assert!(t.0.join("D3D8.dll").exists() && !t.0.join("DDraw.dll").exists());
}

#[test]
fn dgvoodoo_refuses_unmanaged_existing_files() {
    let t = Tmp::new();
    put(&t.0, "DDraw.dll", b"somebody else");
    assert!(DgVoodooFeature::plan(&t.0, true, false, &resources()).is_err());
}

#[test]
fn argm_trace_install_uninstall_and_conflict() {
    let t = Tmp::new();
    assert!(ArgmTraceFeature::plan(&t.0, true, &InstallResources::default()).is_err());
    commit(&ArgmTraceFeature::plan(&t.0, true, &resources()).unwrap());
    assert!(ArgmTraceFeature::is_installed(&t.0));
    assert!(fs::read_to_string(t.0.join("argm_trace.ini")).unwrap().contains("enableHooks=0"));
    commit(&ArgmTraceFeature::plan(&t.0, false, &resources()).unwrap());
    assert!(!t.0.join("winmm.dll").exists() && !ArgmTraceFeature::is_installed(&t.0));
    put(&t.0, "winmm.dll", b"foreign");
    assert!(ArgmTraceFeature::plan(&t.0, true, &resources()).is_err());
}

#[test]
fn orchestrator_modules_and_empty_game_dir_is_unknown() {
    let t = Tmp::new();
    let mut o = EndlessAiOrchestrator::new(true);
    for id in ["M1", "Core", "M2", "M3", "M4", "M5", "M6", "VillageGarrisonQuota3x", "R0"] {
        assert!(o.get_module(id).is_some(), "{id}");
        assert_eq!(o.detect_module_by_id(&t.0, id), Some(PatchState::Unknown), "{id}");
    }
    assert!(o.get_module("nope").is_none());
    assert_eq!(o.detect_module_by_id(&t.0, "nope"), None);
    assert!(!o.apply_module_by_id(&t.0, "M1", false).unwrap().unwrap());
    assert!(o.save_all().unwrap().is_empty());
    o.set_village_garrison_quota_multiplier(0);
    assert_eq!(o.village_quota_multiplier, 3);
    o.set_village_garrison_quota_multiplier(5);
    assert_eq!(o.village_quota_multiplier, 5);
    // mandatory repair on an empty directory is a no-op
    assert!(!o.apply_mandatory_repair(&t.0).unwrap());
}

#[test]
fn detector_on_empty_dir_reports_nothing_enabled() {
    let t = Tmp::new();
    let profile = FeatureDetector::detect(&t.0, &BackupManager::new());
    for f in against_rome_modifier::FeatureRegistry::toggle_features() {
        assert!(!profile.get_bool(f.id), "{}", f.id);
    }
}

#[test]
fn exe_detector_sees_patched_bytes() {
    let t = Tmp::new();
    let size = ALL_EXE_PATCHES.iter().map(|p| p.offset + p.original.len()).max().unwrap() + 16;
    let mut exe = vec![0x90u8; size];
    for p in ALL_EXE_PATCHES { exe[p.offset..p.offset + p.original.len()].copy_from_slice(p.original); }
    put(&t.0, "Against_Rome.exe", &exe);
    let mut original = PatchProfile::new();
    against_rome_modifier::detectors::ExeFeatureDetector::detect(&t.0, &mut original);
    assert!(!original.get_bool("FocusLoss") && !original.get_bool("UnitRecruit20"));
    for p in ALL_EXE_PATCHES { exe[p.offset..p.offset + p.replacement.len()].copy_from_slice(p.replacement); }
    put(&t.0, "Against_Rome.exe", &exe);
    let mut patched = PatchProfile::new();
    against_rome_modifier::detectors::ExeFeatureDetector::detect(&t.0, &mut patched);
    assert!(patched.get_bool("FocusLoss"), "focus patch should be detected");
}
