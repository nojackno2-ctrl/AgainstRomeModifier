use lighting_probe::{MapLightingModel, ProbeError};

fn mul_vec(v: [f32; 3], s: f32) -> [f32; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

#[test]
fn local_light_uses_per_channel_max_and_squared_distance_not_addition() {
    let ambient = [0.2f32, 0.7f32, 0.6f32];
    assert_eq!(
        MapLightingModel::apply_local_light(ambient, [1.0, 0.5, 0.0], 25.0, 100.0).unwrap(),
        [0.75, 0.7, 0.6]
    );
    assert_eq!(
        MapLightingModel::apply_local_light(ambient, [1.0, 1.0, 1.0], 100.0, 100.0).unwrap(),
        ambient
    );
    assert_eq!(
        MapLightingModel::apply_local_light([0.0, 0.0, 0.0], [2.0, 2.0, 2.0], 0.0, 100.0).unwrap(),
        [1.0, 1.0, 1.0]
    );
    assert!(matches!(
        MapLightingModel::apply_local_light(ambient, [1.0, 1.0, 1.0], -1.0, 100.0),
        Err(ProbeError::OutOfRange(_))
    ));
    assert!(matches!(
        MapLightingModel::apply_local_light(ambient, [1.0, 1.0, 1.0], 0.0, 0.0),
        Err(ProbeError::OutOfRange(_))
    ));
}

#[test]
fn sprite_gain_has_shadow_floor_and_squared_extra_byte_factor() {
    let scale = f32::from_bits(0x3B7FFF34);
    let ambient = [0.2f32, 0.7f32, 0.6f32];

    assert_eq!(
        MapLightingModel::sprite_gain(ambient, 1.0, 255).unwrap(),
        mul_vec(ambient, 255.0 * scale * 255.0 * scale)
    );
    assert_eq!(
        MapLightingModel::sprite_gain(ambient, 0.0, 128).unwrap(),
        mul_vec(ambient, 0.293 * 128.0 * scale * 128.0 * scale)
    );
    assert_eq!(
        MapLightingModel::sprite_gain(ambient, 1.0, 0).unwrap(),
        [0.0, 0.0, 0.0]
    );

    assert!(matches!(
        MapLightingModel::sprite_gain(ambient, -0.1, 100),
        Err(ProbeError::OutOfRange(_))
    ));
    assert!(matches!(
        MapLightingModel::sprite_gain(ambient, 1.1, 100),
        Err(ProbeError::OutOfRange(_))
    ));
    assert!(matches!(
        MapLightingModel::sprite_gain(ambient, f32::NAN, 100),
        Err(ProbeError::OutOfRange(_))
    ));
}

#[test]
fn test_shadow_error_conversion_preserves_classification() {
    use against_rome_environment::shadow::ShadowError;

    let err_invalid: ProbeError = ShadowError::InvalidData("corrupt shadow payload".into()).into();
    assert_eq!(
        err_invalid,
        ProbeError::InvalidData("corrupt shadow payload".into())
    );

    let err_not_supported: ProbeError =
        ShadowError::NotSupported("unsupported format".into()).into();
    assert_eq!(
        err_not_supported,
        ProbeError::NotSupported("unsupported format".into())
    );

    let err_range: ProbeError = ShadowError::OutOfRange("slice_index").into();
    assert_eq!(err_range, ProbeError::OutOfRange("slice_index".into()));

    let err_io: ProbeError = ShadowError::Io("read failed".into()).into();
    assert_eq!(err_io, ProbeError::Io("read failed".into()));
}

#[test]
fn test_apply_local_light_nan_propagation_matching_dotnet() {
    let ambient = [0.2f32, 0.7f32, 0.6f32];
    let light_color = [1.0f32, 0.5f32, 0.0f32];

    // NaN in ambient propagates (within radius)
    let res = MapLightingModel::apply_local_light([f32::NAN, 0.7, 0.6], light_color, 25.0, 100.0)
        .unwrap();
    assert!(res[0].is_nan());
    assert_eq!(res[1], 0.7);
    assert_eq!(res[2], 0.6);

    // NaN in light_color propagates (within radius)
    let res =
        MapLightingModel::apply_local_light(ambient, [1.0, f32::NAN, 0.0], 25.0, 100.0).unwrap();
    assert_eq!(res[0], 0.75);
    assert!(res[1].is_nan());
    assert_eq!(res[2], 0.6);

    // NaN in ambient when distance_squared >= radius_squared propagates via Vector3.Min(ambient, Vector3.One)
    let res = MapLightingModel::apply_local_light([f32::NAN, 0.7, 0.6], light_color, 100.0, 100.0)
        .unwrap();
    assert!(res[0].is_nan());
    assert_eq!(res[1], 0.7);
    assert_eq!(res[2], 0.6);

    // All channels NaN
    let res = MapLightingModel::apply_local_light(
        [f32::NAN, f32::NAN, f32::NAN],
        light_color,
        25.0,
        100.0,
    )
    .unwrap();
    assert!(res[0].is_nan());
    assert!(res[1].is_nan());
    assert!(res[2].is_nan());
}
