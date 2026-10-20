mod bitmap_tests;
use bitmap_tests::build_bitmap;
use lighting_probe::{MapLightingDayNight, ProbeError};

fn div256(v: [f32; 3]) -> [f32; 3] {
    [v[0] / 256.0, v[1] / 256.0, v[2] / 256.0]
}

#[test]
fn daynight_preserves_unused_rows_and_uses_native_quantized_interpolation() {
    let bytes = build_bitmap(24, 6, false, |x, y| {
        if y == 0 {
            [(x * 10) as f32, (100 + x) as f32, (200 - x) as f32]
        } else {
            [5.0, y as f32, 9.0]
        }
    });

    let table = MapLightingDayNight::parse(&bytes).expect("parse daynight failed");
    assert_eq!(table.raw_color(8, 4).unwrap(), [5.0, 4.0, 9.0]);
    assert_eq!(
        table.ambient_at(2, 0).unwrap(),
        div256([20.0, 102.0, 198.0])
    );
    assert_eq!(
        table.ambient_at(2, 30).unwrap(),
        div256([25.0, 102.0, 197.0])
    );
    // Weights floor separately; at minute=1 the sum of weights is 65535, giving integer quantization.
    assert_eq!(
        table.ambient_at(2, 1).unwrap(),
        div256([20.0, 102.0, 197.0])
    );
    assert_eq!(
        table.ambient_at(23, 30).unwrap(),
        div256([115.0, 111.0, 188.0])
    );

    assert!(matches!(
        table.ambient_at(24, 0),
        Err(ProbeError::OutOfRange(_))
    ));
    assert!(matches!(
        table.ambient_at(0, 60),
        Err(ProbeError::OutOfRange(_))
    ));
    assert!(matches!(
        table.ambient_at(-1, 0),
        Err(ProbeError::OutOfRange(_))
    ));
    assert!(matches!(
        table.ambient_at(0, -1),
        Err(ProbeError::OutOfRange(_))
    ));

    let invalid_dimensions = build_bitmap(3, 2, false, |_, _| [1.0, 1.0, 1.0]);
    assert!(matches!(
        MapLightingDayNight::parse(&invalid_dimensions),
        Err(ProbeError::NotSupported(_))
    ));
}
