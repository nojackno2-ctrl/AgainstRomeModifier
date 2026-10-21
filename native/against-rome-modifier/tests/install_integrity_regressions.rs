use against_rome_modifier::{install::*, PatchProfile};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "arm-install-integrity-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn commit(&self, plan: InstallPlan) {
        for (path, bytes) in plan.writes {
            assert!(path.starts_with(&self.0));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        for path in plan.deletes {
            assert!(path.starts_with(&self.0));
            fs::remove_file(path).unwrap();
        }
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn resources() -> InstallResources {
    let mut r = InstallResources::default();
    for name in ["D3D8.dll", "DDraw.dll", "dgVoodooCpl.exe"] {
        r.dgvoodoo.insert(name.into(), name.as_bytes().to_vec());
    }
    r.dgvoodoo.insert("dgVoodoo.conf".into(), b"FullScreenMode=false\nScalingMode=original\nCenterAppWindow=false\nWindowedAttributes=\nFullscreenAttributes=\n".to_vec());
    r.argm_winmm = b"managed-trace".to_vec();
    r
}

#[test]
fn dgvoodoo_detection_checks_every_managed_binary_and_missing_config() {
    for name in ["D3D8.dll", "DDraw.dll", "dgVoodooCpl.exe", "dgVoodoo.conf"] {
        let temp = Temp::new();
        temp.commit(DgVoodooFeature::plan(&temp.0, true, true, &resources()).unwrap());
        assert!(DgVoodooFeature::is_installed(&temp.0));
        fs::remove_file(temp.0.join(name)).unwrap();
        assert!(!DgVoodooFeature::is_installed(&temp.0), "缺少 {name}");
        fs::write(temp.0.join(name), b"foreign").unwrap();
        if name != "dgVoodoo.conf" {
            assert!(!DgVoodooFeature::is_installed(&temp.0), "遭更動 {name}");
        } else {
            // 合法的使用者設定可保留，但不能因此宣稱置中呈現已啟用。
            assert!(DgVoodooFeature::is_installed(&temp.0));
            assert!(!DgVoodooFeature::is_centered_presentation_configured(
                &temp.0
            ));
        }
    }
}

#[test]
fn widescreen_installs_wrapper_without_separate_dgvoodoo_flag_and_clears_stale_state() {
    let temp = Temp::new();
    let mut profile = PatchProfile::new();
    profile.set_bool("NativeWidescreen1920x1080", true);
    temp.commit(plan_install(&temp.0, &profile, &resources()).unwrap());
    InstallFeatureDetector::detect(&temp.0, &mut profile);
    assert!(profile.get_bool("DgVoodoo"));
    assert!(profile.get_bool("NativeWidescreen1920x1080"));
    let config = temp.0.join("dgVoodoo.conf");
    let mut duplicate = fs::read_to_string(&config).unwrap();
    duplicate.push_str("CenterAppWindow=false\n");
    fs::write(config, duplicate).unwrap();
    InstallFeatureDetector::detect(&temp.0, &mut profile);
    assert!(!profile.get_bool("NativeWidescreen1920x1080"));
}

#[test]
fn trace_detection_rejects_modified_dll_or_missing_ini_but_preserves_user_configuration() {
    let temp = Temp::new();
    temp.commit(ArgmTraceFeature::plan(&temp.0, true, &resources()).unwrap());
    assert!(ArgmTraceFeature::is_installed(&temp.0));
    fs::write(temp.0.join("argm_trace.ini"), b"[general]\nenableHooks=1\n").unwrap();
    assert!(ArgmTraceFeature::is_installed(&temp.0));
    fs::write(temp.0.join("winmm.dll"), b"foreign").unwrap();
    assert!(!ArgmTraceFeature::is_installed(&temp.0));
    fs::write(temp.0.join("winmm.dll"), b"managed-trace").unwrap();
    fs::remove_file(temp.0.join("argm_trace.ini")).unwrap();
    assert!(!ArgmTraceFeature::is_installed(&temp.0));
}
