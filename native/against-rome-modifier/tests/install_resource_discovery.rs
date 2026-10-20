//! Synthetic resolver coverage: no process environment mutation or installed game access.
use against_rome_modifier::install::{ArgmTraceFeature, DgVoodooFeature, InstallResources};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "arm-resource-discovery-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn put(&self, relative: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }
    fn dg(&self, relative: &str) -> PathBuf {
        for name in ["D3D8.dll", "DDraw.dll", "dgVoodooCpl.exe", "dgVoodoo.conf"] {
            self.put(&format!("{relative}/dgVoodoo2/{name}"), name.as_bytes());
        }
        self.0.join(relative)
    }
    fn discover(&self, exe_dir: &Path) -> InstallResources {
        InstallResources::discover_with(Some(exe_dir), None, None)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn packaged_trace_is_independent_of_dgvoodoo() {
    let t = Temp::new();
    t.put("package/ThirdParty/argm-trace/winmm.dll", b"packaged rust");
    t.put("ThirdParty/argm-trace/winmm.dll", b"legacy cpp");
    t.dg("ThirdParty");
    let resources = t.discover(&t.0.join("package"));
    assert_eq!(resources.argm_winmm, b"packaged rust");
    assert_eq!(resources.dgvoodoo.len(), 4);
}

#[test]
fn app_local_trace_works_without_third_party() {
    let t = Temp::new();
    t.put("app/winmm.dll", b"local rust");
    let resources = t.discover(&t.0.join("app"));
    assert_eq!(resources.argm_winmm, b"local rust");
    assert!(resources.dgvoodoo.is_empty());
}

#[test]
fn explicit_trace_override_wins_over_package_and_build() {
    let t = Temp::new();
    let explicit = t.put("explicit/trace.dll", b"explicit rust");
    t.put("app/ThirdParty/argm-trace/winmm.dll", b"packaged rust");
    t.put(
        "re_workspace/target-against-rome-trace/i686-pc-windows-msvc/release/winmm.dll",
        b"built rust",
    );
    let dg = t.dg("custom");
    t.put("custom/argm-trace/winmm.dll", b"legacy cpp");
    let resources =
        InstallResources::discover_with(Some(&t.0.join("app")), Some(&dg), Some(&explicit));
    assert_eq!(resources.argm_winmm, b"explicit rust");
    assert_eq!(resources.dgvoodoo.len(), 4);
}

#[test]
fn missing_explicit_trace_does_not_fall_back() {
    let t = Temp::new();
    t.put("app/ThirdParty/argm-trace/winmm.dll", b"packaged rust");
    for path in [t.0.join("missing.dll"), t.0.join("app")] {
        let resources = InstallResources::discover_with(Some(&t.0.join("app")), None, Some(&path));
        assert!(resources.argm_winmm.is_empty());
        assert!(ArgmTraceFeature::plan(&t.0, true, &resources).is_err());
    }
}

#[test]
fn both_rust_release_layouts_are_discovered_without_dgvoodoo() {
    for target in [
        "target-against-rome-trace",
        "rust-rewrite-targets/native-against-rome-trace",
    ] {
        let t = Temp::new();
        t.put(
            &format!("re_workspace/{target}/i686-pc-windows-msvc/release/winmm.dll"),
            b"built rust",
        );
        t.put("ThirdParty/argm-trace/winmm.dll", b"legacy cpp");
        let resources = t.discover(&t.0.join("re_workspace/target-modifier/debug/deps"));
        assert_eq!(resources.argm_winmm, b"built rust", "{target}");
        assert!(resources.dgvoodoo.is_empty());
    }
}

#[test]
fn ancestor_and_third_party_override_never_implicitly_supply_trace() {
    let t = Temp::new();
    let dg = t.dg("ThirdParty");
    t.put("ThirdParty/argm-trace/winmm.dll", b"legacy cpp");
    for explicit_dg in [None, Some(dg.as_path())] {
        let resources = InstallResources::discover_with(Some(&t.0.join("app")), explicit_dg, None);
        assert!(resources.argm_winmm.is_empty());
        assert_eq!(resources.dgvoodoo.len(), 4);
        assert!(ArgmTraceFeature::plan(&t.0, true, &resources).is_err());
        assert!(ArgmTraceFeature::plan(&t.0, false, &resources)
            .unwrap()
            .writes
            .is_empty());
    }
}

#[test]
fn executable_at_source_root_does_not_use_tracked_cpp_payload() {
    for marker in [".git", "native/against-rome-trace/Cargo.toml"] {
        let t = Temp::new();
        t.put(marker, b"source checkout");
        t.put("ThirdParty/argm-trace/winmm.dll", b"legacy cpp");
        t.dg("ThirdParty");
        assert!(t.discover(&t.0).argm_winmm.is_empty());
        t.put(
            "re_workspace/target-against-rome-trace/i686-pc-windows-msvc/release/winmm.dll",
            b"built rust",
        );
        assert_eq!(t.discover(&t.0).argm_winmm, b"built rust");
    }
}

#[test]
fn package_precedes_app_local_and_build_then_publish_layout_precedes_old_target() {
    let t = Temp::new();
    let packaged = t.put("app/ThirdParty/argm-trace/winmm.dll", b"package");
    let local = t.put("app/winmm.dll", b"local");
    let preferred = t.put("re_workspace/rust-rewrite-targets/native-against-rome-trace/i686-pc-windows-msvc/release/winmm.dll", b"publish build");
    t.put(
        "re_workspace/target-against-rome-trace/i686-pc-windows-msvc/release/winmm.dll",
        b"old target build",
    );
    let exe = t.0.join("app");
    assert_eq!(t.discover(&exe).argm_winmm, b"package");
    fs::remove_file(packaged).unwrap();
    assert_eq!(t.discover(&exe).argm_winmm, b"local");
    fs::remove_file(local).unwrap();
    assert_eq!(t.discover(&exe).argm_winmm, b"publish build");
    fs::write(preferred, b"").unwrap();
    assert_eq!(t.discover(&exe).argm_winmm, b"old target build");
}

#[test]
fn dgvoodoo_remains_available_when_trace_is_missing() {
    let t = Temp::new();
    t.dg("ThirdParty");
    let resources = t.discover(&t.0.join("app"));
    assert!(resources.argm_winmm.is_empty());
    assert_eq!(resources.dgvoodoo.len(), 4);
    assert!(DgVoodooFeature::plan(&t.0, true, false, &resources).is_ok());
}

#[test]
fn dgvoodoo_override_precedes_app_and_incomplete_bundle_falls_back() {
    let t = Temp::new();
    let custom = t.dg("custom");
    t.dg("app/ThirdParty");
    t.put("custom/dgVoodoo2/D3D8.dll", b"custom dg");
    let resolve = || InstallResources::discover_with(Some(&t.0.join("app")), Some(&custom), None);
    assert_eq!(resolve().dgvoodoo["D3D8.dll"], b"custom dg");
    fs::remove_file(custom.join("dgVoodoo2/DDraw.dll")).unwrap();
    assert_eq!(resolve().dgvoodoo["D3D8.dll"], b"D3D8.dll");
}

#[test]
fn explicit_bundle_loader_retains_injection_contract() {
    let t = Temp::new();
    let root = t.dg("injected");
    assert!(InstallResources::from_third_party(&root).is_err());
    t.put("injected/argm-trace/winmm.dll", b"injected trace");
    let resources = InstallResources::from_third_party(&root).unwrap();
    assert_eq!(resources.argm_winmm, b"injected trace");
    assert_eq!(resources.dgvoodoo.len(), 4);
}

#[test]
fn no_discovery_inputs_yields_no_resources() {
    let resources = InstallResources::discover_with(None, None, None);
    assert!(resources.argm_winmm.is_empty());
    assert!(resources.dgvoodoo.is_empty());
}
