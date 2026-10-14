use against_rome_data::particle_definition_sources::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "arm-sources-{}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("SYSTEM")).unwrap();
        Self(root)
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert!(self.0.starts_with(std::env::temp_dir()));
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn configured_prefix_and_order_preserve_omitted_values_and_initialize_new_ids() {
    let f = Fixture::new();
    f.write("SYSTEM/clengine.ini", "[DefaultPath]\nAlternate/\n");
    f.write(
        "Alternate/partpar.dau",
        "[ParticleParameterDefault]\n22,1,Arrow,64,15000\n",
    );
    f.write(
        "Alternate/partpar.d00",
        "[ParticleParameterDefault]\n22,2,Renamed\n500,1,New\n",
    );
    f.write(
        "Alternate/partpar.d09",
        "[ParticleParameterDefault]\n22,3\n",
    );
    f.write("Alternate/objdef.d09", "[ObjectDefaults]\n2499,1\n");
    let sources = load(&f.0).unwrap();
    assert_eq!(sources.sources.len(), 5);
    assert_eq!(sources.parameters[&22].active_raw(), 3);
    assert_eq!(sources.parameters[&22].lifetime(), 15000);
    assert_eq!(sources.parameters[&22].name, "Renamed");
    assert_eq!(
        sources.parameters[&500].numeric_raw,
        [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(sources.objects[&2499].numeric_raw, [1, 1, -1, -1, -1, -1]);
    assert_eq!(sources.grafiks[&511].numeric_raw, [0, -1, -1, -1, -1, 0, 0]);
}
#[test]
fn delimiter_runs_follow_native_token_ordinals_and_fallback_is_only_for_missing_engine() {
    let f = Fixture::new();
    f.write(
        "SYSTEM/DATA_MP/DEFAULTS/partpar.dau",
        "[ParticleParameterDefault]\n1,,2,Name,3,4\n",
    );
    assert_eq!(
        load(&f.0).unwrap().parameters[&1].numeric_raw[0..3],
        [2, 3, 4]
    );
    f.write("SYSTEM/clengine.ini", "[Other]\nIgnored/\n");
    assert!(load(&f.0).is_err());
}
#[test]
fn present_corruption_and_escape_are_rejected_without_partial_snapshot() {
    let f = Fixture::new();
    f.write(
        "SYSTEM/DATA_MP/DEFAULTS/partpar.dau",
        "[ParticleParameterDefault]\n1,1,N,3,4\n",
    );
    f.write(
        "SYSTEM/DATA_MP/DEFAULTS/partpar.d09",
        "[ParticleParameterDefault]\n1,2,N,3,overflow\n",
    );
    assert!(load(&f.0).is_err());
    f.write("SYSTEM/clengine.ini", "[DefaultPath]\n../outside/\n");
    assert!(load(&f.0).is_err());
    f.write("SYSTEM/clengine.ini", "[DefaultPath]\nC:\\other\\\n");
    assert!(load(&f.0).is_err());
}
#[test]
fn prefix_is_literal_concatenation_not_automatic_directory_join() {
    let f = Fixture::new();
    f.write("SYSTEM/clengine.ini", "[DefaultPath]\nSelected\n");
    f.write(
        "Selectedpartpar.dau",
        "[ParticleParameterDefault]\n1,1,N,3,4\n",
    );
    assert_eq!(load(&f.0).unwrap().parameters[&1].lifetime(), 4);
}

#[test]
fn explicit_disp_overlays_preserve_omitted_mode_and_reject_corruption() {
    assert_eq!(
        EffectiveDefinitions::initialized().objects[&227].mode_native(),
        None
    );
    let f = Fixture::new();
    f.write("SYSTEM/clengine.ini", "[DefaultPath]\nSelected/\n");
    f.write(
        "Selected/objdef.dau",
        "[ObjectDefaults]\n227,1,1000,0,0,258,0,65539\n",
    );
    f.write("Selected/objdef.d09", "[ObjectDefaults]\n227,1,250\n");
    let first = load(&f.0).unwrap();
    assert_eq!(first.objects[&227].mode_raw, Some(65539));
    assert_eq!(first.objects[&227].mode_native(), Some(3));
    assert_eq!(first.objects[&227].animation_length(), 250);
    f.write(
        "Selected/objdef.d09",
        "[ObjectDefaults]\n227,1,250,0,0,258,0,bad\n",
    );
    assert!(load(&f.0).is_err());
    assert_eq!(first.objects[&227].mode_native(), Some(3));
}
