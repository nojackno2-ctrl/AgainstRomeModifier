use against_rome_ambience::atmosphere::*;

#[test]
fn ini_round_trip_preserves_values() {
    let mut d = BodenIniData::default();
    d.set_water_rgb(0.25, 0.5, 0.75);
    let text = BodenIniSerializer::generate_default_ini(&d);
    let back = BodenIniSerializer::parse_text(&text);
    let v = back.get_water_rgb();
    assert!((v[0] - 0.25).abs() < 0.01 && (v[1] - 0.5).abs() < 0.01 && (v[2] - 0.75).abs() < 0.01);
    assert_eq!(text, BodenIniSerializer::generate_default_ini(&back));
}

#[test]
fn ini_default_validates_and_unknown_keys_are_ignored() {
    let (ok, errs) = BodenIniSerializer::validate(&BodenIniData::default());
    assert!(ok, "{errs:?}");
    let _ = BodenIniSerializer::parse_text("garbage\r\n[nokey]\r\n");
}

#[test]
fn presets_are_unique_and_lookup_is_case_insensitive() {
    let all = WeatherPresets::all();
    assert_eq!(all.len(), 5);
    for p in &all {
        let found = WeatherPresets::try_get_preset(&p.preset_id.to_uppercase()).unwrap();
        assert!(found.value_equals(p));
    }
    assert!(WeatherPresets::try_get_preset("nope").is_none());
}

#[test]
fn edit_session_dirty_tracking() {
    let mut s = AtmosphereEditSession::new();
    assert!(!s.is_dirty());
    s.apply_preset("storm");
    assert!(s.is_dirty());
    s.accept_changes();
    assert!(!s.is_dirty());
    s.set_flash_probability(50);
    assert!(s.is_dirty());
    s.reset();
    assert!(!s.is_dirty());
}

#[test]
fn lighting_bridge_advances_time_and_flash() {
    let p = WeatherPresets::create_storm();
    let mut b = AtmosphereLightingBridge::new();
    b.update(1.0, &p);
    assert!(b.simulation_time() > 0.99);
    b.trigger_manual_flash(1.0);
    assert!(b.flash_intensity() > 0.0);
}
