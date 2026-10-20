use against_rome_scene::{
    lighting::{
        MapLightingBitmap, MapLightingDayNight, MapLightingModel, NativeLightCatalog,
        NativeLightDefinition, NativeLightInstance, NativeObjectLightingCatalog,
        PreparedSceneLighting, SavedSceneLightSet, SceneLightDiagnostics,
        SceneLightSuppressionKey, SceneLightingContext, MAXIMUM_GAME_LIGHTS,
    },
    Vec3,
};

fn create_daynight_bmp() -> Vec<u8> {
    let mut bytes = vec![0u8; 54 + 72 * 6];
    bytes[0] = b'B';
    bytes[1] = b'M';

    let write_i32 = |b: &mut [u8], offset: usize, val: i32| {
        b[offset..offset + 4].copy_from_slice(&val.to_le_bytes());
    };
    let write_u16 = |b: &mut [u8], offset: usize, val: u16| {
        b[offset..offset + 2].copy_from_slice(&val.to_le_bytes());
    };

    let len = bytes.len() as i32;
    write_i32(&mut bytes, 2, len);
    write_i32(&mut bytes, 10, 54);
    write_i32(&mut bytes, 14, 40);
    write_i32(&mut bytes, 18, 24);
    write_i32(&mut bytes, 22, -6);
    write_i32(&mut bytes, 34, 72 * 6);
    write_u16(&mut bytes, 26, 1);
    write_u16(&mut bytes, 28, 24);

    for y in 0..6 {
        for x in 0..24 {
            let i = 54 + y * 72 + x * 3;
            bytes[i] = if y == 0 { (200 - x as i32) as u8 } else { 255 };
            bytes[i + 1] = if y == 0 { (100 + x as i32) as u8 } else { 255 };
            bytes[i + 2] = if y == 0 { (10 * x as i32) as u8 } else { 255 };
        }
    }
    bytes
}

#[test]
fn map_lighting_bitmap_validates_dimensions_and_channels() {
    let bmp = create_daynight_bmp();
    let parsed = MapLightingBitmap::parse(&bmp).expect("valid daynight BMP");
    assert_eq!(parsed.width, 24);
    assert_eq!(parsed.height, 6);

    // Row 0, x = 2: B=198, G=102, R=20
    let c = parsed.color_at(2, 0);
    assert_eq!(c, Vec3::new(20.0, 102.0, 198.0));
}

#[test]
fn ambient_uses_row_zero_native_floor_weights_and_wraps_at_24() {
    let bmp = create_daynight_bmp();
    let daynight = MapLightingDayNight::parse(&bmp).unwrap();
    let context = SceneLightingContext::new(Some(daynight), None, None, Default::default());

    let scale = 1.0 / 256.0;
    assert_eq!(context.get_ambient_color(2.0), Vec3::new(20.0, 102.0, 198.0) * scale);
    assert_eq!(context.get_ambient_color(2.5), Vec3::new(25.0, 102.0, 197.0) * scale);
    assert_eq!(context.get_ambient_color(2.02), Vec3::new(20.0, 102.0, 197.0) * scale);
    assert_eq!(context.get_ambient_color(23.5), Vec3::new(115.0, 111.0, 188.0) * scale);

    // Wrapping and clamping behaviors
    assert_eq!(context.get_ambient_color(0.0), context.get_ambient_color(24.0));
    assert_eq!(context.get_ambient_color(-1.0), context.get_ambient_color(0.0));
    assert_eq!(context.get_ambient_color(f32::NAN), context.get_ambient_color(12.0));
}

#[test]
fn map_lighting_model_applies_quadratic_falloff_and_clamps() {
    let ambient = Vec3::new(0.2, 0.2, 0.2);
    let light = Vec3::new(1.0, 0.5, 0.0);

    // Center of light (dist = 0)
    let lit_center = MapLightingModel::apply_local_light(ambient, light, 0.0, 100.0);
    assert_eq!(lit_center, Vec3::new(1.0, 0.5, 0.2));

    // Beyond radius (dist^2 >= rad^2)
    let lit_outside = MapLightingModel::apply_local_light(ambient, light, 150.0, 100.0);
    assert_eq!(lit_outside, ambient);

    // At half radius squared (falloff = 1 - 50/100 = 0.5)
    let lit_half = MapLightingModel::apply_local_light(ambient, light, 50.0, 100.0);
    assert_eq!(lit_half, Vec3::new(0.5, 0.25, 0.2));
}

#[test]
fn native_light_catalog_parses_csv_definitions() {
    let ini = "[LightDefault]\n1,1,1.48,1.22,0,350,1,.05,0,Coal\n7,1,1.33,.61,.33,300,1,.15,0,Fire";
    let catalog = NativeLightCatalog::parse(ini);
    assert_eq!(catalog.len(), 2);

    let l1 = catalog.get_definition(1);
    assert!(l1.is_active);
    assert_eq!(l1.radius, 350.0);
    assert_eq!(l1.name, "Coal");

    let l7 = catalog.get_definition(7);
    assert!(l7.is_active);
    assert_eq!(l7.radius, 300.0);
    assert_eq!(l7.name, "Fire");

    // Missing falls back to default
    let missing = catalog.get_definition(999);
    assert_eq!(missing.name, "DefaultLight");
}

#[test]
fn native_object_lighting_catalog_parses_columns() {
    // Col 0: id, Col 14: aptix, Col 52: name, Col 66: lidef, Col 67: lihei, Col 162: aptli, Col 163: aptlh
    let mut cols = vec!["0"; 164];
    cols[0] = "42";
    cols[14] = "3";
    cols[52] = "House";
    cols[66] = "7";
    cols[67] = "15";
    cols[162] = "1";
    cols[163] = "30";
    let line = cols.join(",");

    let catalog = NativeObjectLightingCatalog::parse(&line);
    assert_eq!(catalog.len(), 1);

    let def = catalog.try_get_definition("house").expect("case-insensitive lookup");
    assert_eq!(def.type_id, 42);
    assert_eq!(def.apt_index, 3);
    assert_eq!(def.light_def_index, 7);
    assert_eq!(def.light_height_offset, 15);
    assert_eq!(def.apt_light_def_index, 1);
    assert_eq!(def.apt_light_height_offset, 30);
}

#[test]
fn native_light_instance_saved_vs_synthetic_and_renderability() {
    let def = NativeLightDefinition {
        index: 3,
        is_active: true,
        color: Vec3::new(1.0, 0.5, 0.2),
        radius: 400.0,
        ..Default::default()
    };
    let synth = NativeLightInstance::create_synthetic(Vec3::new(10.0, 20.0, 30.0), &def, true);
    assert!(!synth.is_saved);
    assert!(synth.is_renderable());
    assert_eq!(synth.radius, 400.0);

    let saved = NativeLightInstance::create_saved(
        5,
        Vec3::new(100.0, 50.0, 200.0),
        Vec3::new(2.5, 0.25, 0.0), // RGB > 1 allowed
        320.0,
        90.0,
        true,
        None,
    );
    assert!(saved.is_saved);
    assert_eq!(saved.saved_slot, 5);
    assert_eq!(saved.radius, 320.0);
    assert_eq!(saved.baseline_radius, 90.0);
    assert!(saved.is_renderable());

    // Non-renderable cases
    let zero_rad = NativeLightInstance::create_saved(0, Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), 0.0, 0.0, true, None);
    assert!(!zero_rad.is_renderable());

    let nan_pos = NativeLightInstance::create_saved(0, Vec3::new(f32::NAN, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0), 10.0, 10.0, true, None);
    assert!(!nan_pos.is_renderable());

    let inactive = NativeLightInstance::create_saved(0, Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), 10.0, 10.0, false, None);
    assert!(!inactive.is_renderable());
}

#[test]
fn suppression_key_normalizes_and_skips_negative_one() {
    assert_eq!(SceneLightSuppressionKey::try_for("Maps\\A.sdl", -1), None);
    let key = SceneLightSuppressionKey::try_for("Maps\\A.sdl", 4).unwrap();
    assert_eq!(key.source_file, "MAPS/A.SDL");
    assert_eq!(key.object_index, 4);

    // Negative indices other than -1 are preserved
    let neg = SceneLightSuppressionKey::try_for("maps/b.sdl", -100000).unwrap();
    assert_eq!(neg.object_index, -100000);
}

#[test]
fn saved_scene_light_set_enforces_saved_instances() {
    let saved = NativeLightInstance::create_saved(0, Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), 10.0, 10.0, true, None);
    let set = SavedSceneLightSet::new(vec![saved], Default::default());
    assert!(set.is_ok());
    assert!(set.unwrap().has_data());

    let unsaved = NativeLightInstance::create_synthetic(Vec3::ZERO, &NativeLightDefinition::default(), true);
    let invalid = SavedSceneLightSet::new(vec![unsaved], Default::default());
    assert!(invalid.is_err());
}

#[test]
fn prepared_scene_lighting_selects_nearest_and_packs_uniforms() {
    let mut lights = Vec::new();
    for i in 0..100 {
        let x = (i as f32) * 50.0;
        lights.push(NativeLightInstance::create_saved(
            i,
            Vec3::new(x, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 1.0),
            100.0,
            100.0,
            true,
            None,
        ));
    }

    let target = Vec3::new(0.0, 0.0, 0.0);
    let diag = SceneLightDiagnostics::default();
    let prepared = PreparedSceneLighting::prepare(&lights, target, diag);

    // Should cap at MAXIMUM_GAME_LIGHTS (64)
    assert_eq!(prepared.selected_count, MAXIMUM_GAME_LIGHTS);
    // Nearest light at x=0 should be first
    assert_eq!(prepared.light_positions[0], 0.0);
    // Packed is_saved flag is 1.0
    assert_eq!(prepared.light_colors[3], 1.0);
}
// Expected (hour, minute) values were checked by executing GetAmbientColor from
// src.MapEditor.Modules/NativeAssets/SceneLightingContext.cs with a capturing
// daynight table. Keep normalization separate from the existing floor-weight test.
#[test]
fn ambient_hour_matches_csharp_clamp_and_nonfinite_contract() {
    let daynight = MapLightingDayNight::parse(&create_daynight_bmp()).unwrap();
    let context = SceneLightingContext::new(Some(daynight.clone()), None, None, Default::default());
    for (input, hour, minute) in [
        (-25.0, 0, 0),
        (-1.0, 0, 0),
        (0.0, 0, 0),
        (2.02, 2, 1),
        (23.5, 23, 30),
        (24.0, 0, 0),
        (25.0, 0, 0),
        (49.0, 0, 0),
        (f32::MIN, 0, 0),
        (f32::MAX, 0, 0),
        (f32::NAN, 12, 0),
        (f32::INFINITY, 12, 0),
        (f32::NEG_INFINITY, 12, 0),
    ] {
        assert_eq!(
            context.get_ambient_color(input),
            daynight.ambient_at(hour, minute),
            "C# ambient hour/minute contract for {input:?}"
        );
    }
}

#[test]
fn ambient_hour_preserves_csharp_minute_boundaries() {
    let daynight = MapLightingDayNight::parse(&create_daynight_bmp()).unwrap();
    let context = SceneLightingContext::new(Some(daynight.clone()), None, None, Default::default());
    // Adding 24 before the modulo loses precision even for valid clock hours.
    // C# selects minutes 1, 2 and 59 for these exact f32 inputs.
    for minute in [1, 2, 59] {
        let input = minute as f32 / 60.0;
        assert_eq!(
            context.get_ambient_color(input),
            daynight.ambient_at(0, minute),
            "C# minute boundary for {input:?}"
        );
    }
}

#[test]
fn ambient_without_daynight_remains_neutral_for_any_hour() {
    let context = SceneLightingContext::new(None, None, None, Default::default());
    for input in [-1.0, 0.0, 12.0, 24.0, 25.0, f32::NAN, f32::INFINITY] {
        assert_eq!(context.get_ambient_color(input), Vec3::new(1.0, 1.0, 1.0));
    }
}
