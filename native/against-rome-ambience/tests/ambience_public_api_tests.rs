use against_rome_ambience::{
    AtmosphereEditSession, AtmosphereLightingBridge, AtmosphereShaderUniforms,
    BriefingTextTable, MultiLanguageTextStudio, PrecipitationType,
    PutScriptSyntaxValidator, SoundscapeBinaryStorage, SoundscapeEditorModule,
    SoundscapeSnapshot, SoundscapeZone, SupportedLanguage,
};
use std::collections::BTreeMap;

#[test]
fn atmosphere_edit_session_workflow_and_uniforms_parity() {
    let mut session = AtmosphereEditSession::new();
    assert!(!session.is_dirty());

    // Apply stormy preset
    session.apply_preset("storm");
    assert!(session.is_dirty());
    assert_eq!(session.current().precipitation, PrecipitationType::Rain);

    // Modify water parameters
    session.set_water_bump(300, 8, 12);
    assert_eq!(session.current().boden.water_bump_amplitude, 300);
    assert_eq!(session.current().boden.water_bump_frequency, 8);
    assert_eq!(session.current().boden.water_warp_shift, 12);

    // Bridge computes shader uniforms accurately
    let bridge = AtmosphereLightingBridge::new();
    let uniforms: AtmosphereShaderUniforms = bridge.compute_uniforms(
        session.current(),
        [1.0, 1.0, 1.0],
    );
    assert_eq!(uniforms.water_dynamics[0], 300.0);
    assert_eq!(uniforms.water_dynamics[1], 8.0);
    assert_eq!(uniforms.water_dynamics[2], 12.0);

    // Accept changes resets dirty flag
    session.accept_changes();
    assert!(!session.is_dirty());
}

#[test]
fn soundscape_editor_module_and_binary_storage_parity() {
    let mut editor = SoundscapeEditorModule::default();
    assert!(!editor.is_dirty());

    let zone = SoundscapeZone {
        id: uuid::Uuid::new_v4(),
        name: "RiverAmbience".into(),
        center_world_x: 100.0,
        center_world_z: 200.0,
        sound_def_id: "Amb_River_01".into(),
        ..Default::default()
    };

    editor.add_zone(&zone);
    assert!(editor.is_dirty());
    assert_eq!(editor.zones().len(), 1);

    // Roundtrip snapshot to binary SNDZ
    let snap = SoundscapeSnapshot {
        zones: vec![zone.clone()],
        ..Default::default()
    };
    let bytes = SoundscapeBinaryStorage::serialize_to_binary(&snap).expect("valid serialization");
    let back = SoundscapeBinaryStorage::deserialize_from_binary(&bytes).expect("valid deserialization");
    assert_eq!(back.zones.len(), 1);
    assert_eq!(back.zones[0].name, "RiverAmbience");
    assert_eq!(back.zones[0].center_world_x, 100.0);
}

#[test]
fn briefing_and_localization_parity() {
    let mut tables = BTreeMap::new();
    let mut en = BriefingTextTable::create_empty();
    en.set_title1("Attack at Dawn");
    en.set_team_name(0, "Romans");
    tables.insert(SupportedLanguage::English, en);

    let mut studio = MultiLanguageTextStudio::new();
    studio.load_from_tables(&tables);
    studio.refresh_parity_status();

    assert!(studio.calculate_coverage(SupportedLanguage::English) > 0.0);
    let exported = studio.export_to_tables();
    assert!(exported.contains_key(&SupportedLanguage::English));
    assert_eq!(
        exported[&SupportedLanguage::English].title1(),
        Some("Attack at Dawn")
    );

    // Validator check
    let valid = PutScriptSyntaxValidator::validate_table(&exported[&SupportedLanguage::English], "windows-1252");
    assert!(valid.is_valid());
}
