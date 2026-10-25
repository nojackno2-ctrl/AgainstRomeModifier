//! Integration tests for PatchEngine::compose_with_install and compose_all,
//! verifying unified synthesis of core module patches and install features (LanguagePack,
//! DgVoodoo, ArgmTrace), preservation of the core-only compose contract, and
//! end-to-end rollback transaction guarantees across both core and install files.

use against_rome_modifier::{
    cli::{commit_plan, commit_plan_internal, run_cli},
    compress_pfil,
    endless_ai::words_bytes,
    install::InstallResources,
    services::{BackupManager, PatchEngine, OBJDEF_KEY},
    PatchProfile, FOCUS, ROMAN_ENDLESS, UNIT_RECRUIT_20,
};
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::PathBuf,

};


struct TestSandbox {
    root: PathBuf,
}

impl TestSandbox {
    fn new(prefix: &str) -> Self {
        let root = crate::temp_fixture::dir(&format!("arm-patch-compose-{prefix}"));
        fs::create_dir_all(&root).expect("create test sandbox dir");
        Self { root }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn write(&self, rel: &str, bytes: &[u8]) -> PathBuf {
        let p = self.path(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(&p, bytes).expect("write file in sandbox");
        p
    }

    fn read(&self, rel: &str) -> Vec<u8> {
        fs::read(self.path(rel)).expect("read file from sandbox")
    }

    fn exists(&self, rel: &str) -> bool {
        self.path(rel).exists()
    }
}

impl Drop for TestSandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn create_synthetic_exe() -> Vec<u8> {
    use against_rome_modifier::exe_features::*;
    let size = (WS_TEXT_OFFSET + WS_TEXT_ORIGINAL.len() + 1024).max(0x200000);
    let mut exe = vec![0x55u8; size];
    // Put valid PE header structure so ArgmTrace feature can inspect timestamp
    exe[0..2].copy_from_slice(b"MZ");
    exe[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
    exe[0x80..0x84].copy_from_slice(b"PE\0\0");
    exe[0x88..0x8c].copy_from_slice(&0x404d1710u32.to_le_bytes());

    for patch in [FOCUS, ROMAN_ENDLESS, UNIT_RECRUIT_20] {
        exe[patch.offset..patch.offset + patch.original.len()].copy_from_slice(patch.original);
    }
    exe[WS_ID_OFFSET..WS_ID_OFFSET + WS_ID_ORIGINAL.len()].copy_from_slice(WS_ID_ORIGINAL);
    exe[WS_CREATE_OFFSET..WS_CREATE_OFFSET + WS_CREATE_ORIGINAL.len()]
        .copy_from_slice(WS_CREATE_ORIGINAL);
    exe[WS_TEXT_OFFSET..WS_TEXT_OFFSET + WS_TEXT_ORIGINAL.len()].copy_from_slice(WS_TEXT_ORIGINAL);
    exe[WS_FORCE_OFFSET..WS_FORCE_OFFSET + WS_FORCE_ORIGINAL.len()]
        .copy_from_slice(WS_FORCE_ORIGINAL);
    exe[WS_ACTMODE_OFFSET..WS_ACTMODE_OFFSET + WS_ACTMODE_ORIGINAL.len()]
        .copy_from_slice(WS_ACTMODE_ORIGINAL);
    exe[WS_IGM_OFFSET..WS_IGM_OFFSET + WS_IGM_ORIGINAL.len()].copy_from_slice(WS_IGM_ORIGINAL);

    exe
}

fn create_synthetic_bci(symbol: i32) -> Vec<u8> {
    let mut raw = words_bytes(&[66, 1, 81, 10, 81, 98, 128, symbol, 73, -3, 86]);
    raw.resize(128, 0);
    compress_pfil(&raw, None).expect("compress synthetic bci")
}

fn setup_synthetic_game(sb: &TestSandbox) -> BackupManager {
    let exe = create_synthetic_exe();
    let epara_bytes = against_rome_modifier::CleanEparaBaseline::create_bytes().unwrap();
    sb.write("Against_Rome.exe", &exe);
    sb.write(
        "SYSTEM/cl_script.ini",
        b"[CiviDelay]\r\nCiviDelay=0, 5000\r\nFigGerPri00_Priester=1\r\nFigKelPri00_Priester=1\r\nFigHunPri00_Priester=1\r\n",
    );
    sb.write("SYSTEM/cl_epara.ini", &epara_bytes);
    sb.write("SYSTEM/ress.ini", b"[objres]\r\nCost=100\r\n");
    sb.write("SYSTEM/CLAK/cl_scint.ini", b"SCINT_ORIGINAL");

    let objdef_text = "header1\r\nheader2\r\n";
    let objdef_pfil = compress_pfil(objdef_text.as_bytes(), None).expect("compress objdef");
    sb.write(OBJDEF_KEY, &objdef_pfil);
    let team_pfil = compress_pfil(b"1\r\n2\r\n", None).expect("compress team");
    sb.write("MAPS/ENDL_000/team.dat", &team_pfil);

    // BCI scripts for orchestrator
    for (name, symbol) in [
        ("ak_anfuehrer", 87),
        ("ak_artillerie", 69),
        ("ak_geisterreiter", 68),
        ("ak_kampfverband", 89),
        ("ak_krieger", 73),
        ("ak_kundschafterwolf", 77),
        ("ak_landtier", 72),
        ("ak_packpferd", 57),
        ("ak_priester", 92),
        ("ak_verbandswolf", 68),
        ("ak_zivilist", 83),
        ("ak_zivilverband", 87),
    ] {
        let bci = create_synthetic_bci(symbol);
        sb.write(&format!("SYSTEM/CLAK/SCRIPT/{name}.bci"), &bci);
    }

    // Language pack fixtures: ToEng package and original game text
    sb.write("ToEng/TEXT/ui_strings.txt", b"English UI text");
    sb.write("TEXT/ui_strings.txt", b"German UI text original");

    // Populate BackupManager
    let mut bm = BackupManager::new();
    bm.set_backup_file("Against_Rome.exe", exe);
    bm.set_backup_file(
        "SYSTEM/cl_script.ini",
        b"[CiviDelay]\r\nCiviDelay=0, 5000\r\nFigGerPri00_Priester=1\r\nFigKelPri00_Priester=1\r\nFigHunPri00_Priester=1\r\n".to_vec(),
    );
    bm.set_backup_file("SYSTEM/cl_epara.ini", epara_bytes);
    bm.set_backup_file("SYSTEM/ress.ini", b"[objres]\r\nCost=100\r\n".to_vec());
    bm.set_backup_file("SYSTEM/CLAK/cl_scint.ini", b"SCINT_ORIGINAL".to_vec());
    bm.set_backup_file(OBJDEF_KEY, objdef_pfil);
    bm.set_backup_file("MAPS/ENDL_000/team.dat", team_pfil);
    bm.set_backup_file(
        "SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci",
        create_synthetic_bci(87),
    );
    bm
}

fn create_mock_install_resources() -> InstallResources {
    let mut r = InstallResources::default();
    for n in ["D3D8.dll", "DDraw.dll", "dgVoodooCpl.exe"] {
        r.dgvoodoo
            .insert(n.into(), format!("mock-{n}").into_bytes());
    }
    r.dgvoodoo.insert(
        "dgVoodoo.conf".into(),
        b"\xef\xbb\xbfFullScreenMode = false\r\nScalingMode=original\r\nCenterAppWindow=false\r\nWindowedAttributes=\r\nFullscreenAttributes=\r\n".to_vec(),
    );
    r.argm_winmm = b"mock-winmm-trace-payload".to_vec();
    r
}

#[test]
fn compose_core_only_preserves_pure_core_contract() {
    let sb = TestSandbox::new("core-only");
    let bm = setup_synthetic_game(&sb);
    let resources = create_mock_install_resources();

    let mut profile = PatchProfile::new();
    profile.set_bool("FastCiviProduction", true);
    profile.set_bool("RangedRange3x", true);
    // Even if install features are enabled in the profile, core-only compose MUST ignore them
    profile.set_bool("DgVoodoo", true);
    profile.set_bool("NativeWidescreen1920x1080", true);
    profile.set_bool("ArgmTrace", true);
    profile.set_bool("ToEnglish", true);

    let core_plan = PatchEngine::compose(&sb.root, &profile, &bm).expect("compose core plan");

    // Core plan must have files to write, but deletes must be empty
    assert!(!core_plan.files.is_empty(), "core plan must have files");
    assert!(
        core_plan.deletes.is_empty(),
        "core-only compose must never populate deletes"
    );

    // Verify core files are present in the plan
    assert!(core_plan
        .files
        .contains_key(&sb.path("SYSTEM/cl_script.ini")));
    assert!(core_plan
        .files
        .contains_key(&sb.path("SYSTEM/cl_epara.ini")));

    // Verify install feature targets are strictly absent from core-only plan
    assert!(!core_plan.files.contains_key(&sb.path("D3D8.dll")));
    assert!(!core_plan.files.contains_key(&sb.path("DDraw.dll")));
    assert!(!core_plan.files.contains_key(&sb.path("dgVoodooCpl.exe")));
    assert!(!core_plan.files.contains_key(&sb.path("dgVoodoo.conf")));
    assert!(!core_plan
        .files
        .contains_key(&sb.path(".against-rome-modifier-dgvoodoo.json")));
    assert!(!core_plan.files.contains_key(&sb.path("winmm.dll")));
    assert!(!core_plan.files.contains_key(&sb.path("argm_trace.ini")));
    assert!(!core_plan
        .files
        .contains_key(&sb.path(".against-rome-modifier-argmtrace.json")));
    assert!(!core_plan
        .files
        .contains_key(&sb.path("TEXT/ui_strings.txt")));

    // Calling compose_with_install, on the other hand, MUST include both
    let full_plan = PatchEngine::compose_with_install(&sb.root, &profile, &bm, &resources)
        .expect("compose full plan with install");
    let alias_plan = PatchEngine::compose_all(&sb.root, &profile, &bm, &resources)
        .expect("compose_all alias plan");

    // compose_all and compose_with_install must be 100% identical
    assert_eq!(full_plan.files, alias_plan.files);
    assert_eq!(full_plan.deletes, alias_plan.deletes);

    // full_plan must contain all core files AND install files
    for key in core_plan.files.keys() {
        assert!(
            full_plan.files.contains_key(key),
            "full plan must include core file: {}",
            key.display()
        );
    }
    assert!(full_plan.files.contains_key(&sb.path("D3D8.dll")));
    assert!(full_plan.files.contains_key(&sb.path("winmm.dll")));
    assert!(full_plan
        .files
        .contains_key(&sb.path("TEXT/ui_strings.txt")));
    assert!(full_plan.files.len() > core_plan.files.len());
}

#[test]
fn compose_with_install_synthesizes_core_and_install_features() {
    let sb = TestSandbox::new("full-synth");
    let bm = setup_synthetic_game(&sb);
    let resources = create_mock_install_resources();

    let mut profile = PatchProfile::new();
    profile.set_bool("FastCiviProduction", true);
    profile.set_bool("DgVoodoo", true);
    profile.set_bool("NativeWidescreen1920x1080", true);
    profile.set_bool("ArgmTrace", true);
    profile.set_bool("ToEnglish", true);

    let plan = PatchEngine::compose_with_install(&sb.root, &profile, &bm, &resources)
        .expect("compose with install");

    // 1. Verify core writes
    let cl_script_path = sb.path("SYSTEM/cl_script.ini");
    assert!(plan.files.contains_key(&cl_script_path));
    let script_content =
        String::from_utf8(plan.files[&cl_script_path].clone()).expect("cl_script utf8");
    assert!(script_content.contains("CiviDelay  =0, 500"));

    // 2. Verify DgVoodoo files + widescreen applied config
    let dgvoodoo_conf_path = sb.path("dgVoodoo.conf");
    assert!(plan.files.contains_key(&dgvoodoo_conf_path));
    let conf_content = String::from_utf8_lossy(&plan.files[&dgvoodoo_conf_path]).into_owned();
    assert!(
        conf_content.contains("FullScreenMode = true")
            || conf_content.contains("FullScreenMode=true")
    );
    assert!(conf_content.contains("ScalingMode=stretched_ar"));
    assert!(conf_content.contains("CenterAppWindow=true"));

    assert!(plan.files.contains_key(&sb.path("D3D8.dll")));
    assert!(plan.files.contains_key(&sb.path("DDraw.dll")));
    assert!(plan.files.contains_key(&sb.path("dgVoodooCpl.exe")));
    assert!(plan
        .files
        .contains_key(&sb.path(".against-rome-modifier-dgvoodoo.json")));

    // 3. Verify ArgmTrace files
    assert!(plan.files.contains_key(&sb.path("winmm.dll")));
    let trace_ini_path = sb.path("argm_trace.ini");
    assert!(plan.files.contains_key(&trace_ini_path));
    let trace_ini = String::from_utf8(plan.files[&trace_ini_path].clone()).expect("trace ini utf8");
    assert!(trace_ini.contains("expectedTimeDateStamp=404D1710"));
    assert!(plan
        .files
        .contains_key(&sb.path(".against-rome-modifier-argmtrace.json")));

    // 4. Verify LanguagePack files
    let lang_target = sb.path("TEXT/ui_strings.txt");
    assert!(plan.files.contains_key(&lang_target));
    assert_eq!(plan.files[&lang_target], b"English UI text");

    // Commit the composed plan and verify all disk mutations take effect cleanly
    commit_plan(&plan).expect("commit full plan");
    assert_eq!(sb.read("TEXT/ui_strings.txt"), b"English UI text");
    assert_eq!(sb.read("winmm.dll"), b"mock-winmm-trace-payload");
    assert_eq!(sb.read("D3D8.dll"), b"mock-D3D8.dll");
}

#[test]
fn compose_with_install_uninstallation_deletes_and_restores() {
    let sb = TestSandbox::new("uninstall-synth");
    let bm = setup_synthetic_game(&sb);
    let resources = create_mock_install_resources();

    // 1. Initial install
    let mut install_profile = PatchProfile::new();
    install_profile.set_bool("DgVoodoo", true);
    install_profile.set_bool("ArgmTrace", true);
    install_profile.set_bool("ToEnglish", true);

    let install_plan =
        PatchEngine::compose_with_install(&sb.root, &install_profile, &bm, &resources)
            .expect("install compose");
    commit_plan(&install_plan).expect("commit initial install");

    assert!(sb.exists("D3D8.dll"));
    assert!(sb.exists("winmm.dll"));
    assert_eq!(sb.read("TEXT/ui_strings.txt"), b"English UI text");

    // 2. Compose uninstallation
    let mut uninstall_profile = PatchProfile::new();
    uninstall_profile.set_bool("DgVoodoo", false);
    uninstall_profile.set_bool("ArgmTrace", false);
    uninstall_profile.set_bool("ToEnglish", false);
    uninstall_profile.set_bool("FastCiviProduction", true);

    let uninstall_plan =
        PatchEngine::compose_with_install(&sb.root, &uninstall_profile, &bm, &resources)
            .expect("uninstall compose");

    // plan.deletes must contain DgVoodoo and ArgmTrace files
    assert!(uninstall_plan.deletes.contains(&sb.path("D3D8.dll")));
    assert!(uninstall_plan.deletes.contains(&sb.path("DDraw.dll")));
    assert!(uninstall_plan.deletes.contains(&sb.path("winmm.dll")));
    assert!(uninstall_plan.deletes.contains(&sb.path("argm_trace.ini")));
    assert!(uninstall_plan
        .deletes
        .contains(&sb.path(".against-rome-modifier-dgvoodoo.json")));
    assert!(uninstall_plan
        .deletes
        .contains(&sb.path(".against-rome-modifier-argmtrace.json")));

    // Language pack restoration must be in plan.files (reverting to original German text)
    let lang_file = sb.path("TEXT/ui_strings.txt");
    assert!(uninstall_plan.files.contains_key(&lang_file));
    assert_eq!(uninstall_plan.files[&lang_file], b"German UI text original");

    // Commit uninstallation
    commit_plan(&uninstall_plan).expect("commit uninstallation");

    // Deleted files must no longer exist
    assert!(!sb.exists("D3D8.dll"));
    assert!(!sb.exists("DDraw.dll"));
    assert!(!sb.exists("winmm.dll"));
    assert!(!sb.exists("argm_trace.ini"));
    assert!(!sb.exists(".against-rome-modifier-dgvoodoo.json"));
    assert!(!sb.exists(".against-rome-modifier-argmtrace.json"));

    // Language file must be restored
    assert_eq!(sb.read("TEXT/ui_strings.txt"), b"German UI text original");
}

#[test]
fn rollback_transaction_contract_preserves_originals_and_cleans_creations_on_failure() {
    let sb = TestSandbox::new("rollback-contract");
    let bm = setup_synthetic_game(&sb);
    let resources = create_mock_install_resources();

    let mut profile = PatchProfile::new();
    profile.set_bool("FastCiviProduction", true);
    profile.set_bool("DgVoodoo", true);
    profile.set_bool("ArgmTrace", true);
    profile.set_bool("ToEnglish", true);

    let plan = PatchEngine::compose_with_install(&sb.root, &profile, &bm, &resources)
        .expect("compose full plan");

    // Pre-condition verification: install files do not exist yet
    assert!(!sb.exists("D3D8.dll"));
    assert!(!sb.exists("winmm.dll"));
    let orig_script = sb.read("SYSTEM/cl_script.ini");
    let orig_lang = sb.read("TEXT/ui_strings.txt");

    // Inject failure halfway through the file replacement loop (e.g. step 3)
    let err = commit_plan_internal(&plan, Some(3)).expect_err("must fail at injected step");
    assert!(
        err.to_string().contains("測試注入之交易失敗"),
        "error should indicate injected failure: {err}"
    );

    // Rollback transaction contract check:
    // 1. Files modified before failure must be reverted to pre-transaction contents
    assert_eq!(
        sb.read("SYSTEM/cl_script.ini"),
        orig_script,
        "cl_script.ini must be reverted to original bytes"
    );
    assert_eq!(
        sb.read("TEXT/ui_strings.txt"),
        orig_lang,
        "TEXT/ui_strings.txt must be reverted to original bytes"
    );

    // 2. Newly created files must NOT remain on disk
    assert!(
        !sb.exists("D3D8.dll"),
        "D3D8.dll should not exist after rollback"
    );
    assert!(
        !sb.exists("winmm.dll"),
        "winmm.dll should not exist after rollback"
    );
    assert!(
        !sb.exists(".against-rome-modifier-dgvoodoo.json"),
        "dgvoodoo marker should not exist after rollback"
    );
    assert!(
        !sb.exists(".against-rome-modifier-argmtrace.json"),
        "argmtrace marker should not exist after rollback"
    );

    // 3. No temporary files or locks left behind
    for entry in fs::read_dir(&sb.root).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        assert!(
            !name.ends_with(".arm-rust-lock") && !name.contains(".arm-tmp-"),
            "residual temporary file or lock found: {name}"
        );
    }
}

#[test]
fn cli_apply_uses_service_compose_with_install() {
    let sb = TestSandbox::new("cli-apply-compose");
    let _bm = setup_synthetic_game(&sb);

    // Create Backup.zip in sandbox so CLI can discover backup
    let zip_path = sb.path("Backup.zip");
    {
        let file = fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::FileOptions::default();
        let epara_bytes = against_rome_modifier::CleanEparaBaseline::create_bytes().unwrap();
        zip.start_file("Against_Rome.exe", options).unwrap();
        zip.write_all(&create_synthetic_exe()).unwrap();
        zip.start_file("SYSTEM/cl_script.ini", options).unwrap();
        zip.write_all(b"[CiviDelay]\r\nCiviDelay=0, 5000\r\n")
            .unwrap();
        zip.start_file("SYSTEM/cl_epara.ini", options).unwrap();
        zip.write_all(&epara_bytes).unwrap();
        zip.start_file("SYSTEM/ress.ini", options).unwrap();
        zip.write_all(b"[objres]\r\n").unwrap();
        zip.start_file(OBJDEF_KEY, options).unwrap();
        zip.write_all(&compress_pfil(b"h1\r\nh2\r\n", None).unwrap())
            .unwrap();
        zip.start_file("SYSTEM/CLMK/icon.ini", options).unwrap();
        zip.write_all(b"").unwrap();
        zip.start_file("SYSTEM/CLAK/cl_scint.ini", options).unwrap();
        zip.write_all(b"").unwrap();
        zip.start_file("SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci", options)
            .unwrap();
        zip.write_all(&create_synthetic_bci(87)).unwrap();
        zip.start_file("MAPS/ENDL_000/team.dat", options).unwrap();
        zip.write_all(&compress_pfil(b"1\r\n2\r\n", None).unwrap())
            .unwrap();
        zip.finish().unwrap();
    }

    // CLI dry-run apply
    let mut out = Vec::new();
    let mut err = Vec::new();
    let args = vec![
        "apply".to_string(),
        "--game".to_string(),
        sb.root.to_string_lossy().into_owned(),
        "--enable".to_string(),
        "FastCiviProduction".to_string(),
        "--dry-run".to_string(),
        "--json".to_string(),
    ];
    let code = run_cli(&args, &mut out, &mut err);
    assert_eq!(
        code,
        0,
        "CLI dry-run failed (code={code}): err={}, out={}",
        String::from_utf8_lossy(&err),
        String::from_utf8_lossy(&out)
    );

    let val: Value = serde_json::from_slice(&out).expect("parse json");
    assert_eq!(val["success"], true);
    assert_eq!(val["data"]["isDryRun"], true);
    assert!(val["data"]["plannedFiles"].as_u64().unwrap() > 0);
}

#[cfg(test)]
mod temp_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/temp_unique.rs"));
}
