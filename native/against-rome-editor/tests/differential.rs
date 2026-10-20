//! Differential against terrain output frozen from the original C# implementation
//! (tests/fixtures/terrain_oracle.bin); no dotnet needed.
use against_rome_editor::terrain::*;

#[test]
fn frozen_csharp_terrain_differential() {
    let expected = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/terrain_oracle.bin"),
    )
    .unwrap();
    let mut actual = Vec::new();
    fn snapshot(out: &mut Vec<u8>, s: &TerrainSession) {
        out.extend_from_slice(s.heights());
        out.extend_from_slice(s.collision().unwrap());
    }
    for size in [193, 257] {
        let initial: Vec<u8> = (0..size * size)
            .map(|i| ((i * 17 + i / size * 13) % 256) as u8)
            .collect();
        for falloff in [
            Falloff::Smoothstep,
            Falloff::Cosine,
            Falloff::Spherical,
            Falloff::Gaussian,
            Falloff::Linear,
            Falloff::Plateau,
        ] {
            for caps in [false, true] {
                let p = RampParameters {
                    width: 5.5,
                    falloff: 3.75,
                    start_height: Some(12.5),
                    end_height: Some(241.5),
                    falloff_type: falloff,
                    round_caps: caps,
                };
                let mut h = initial.clone();
                for c in calculate_ramp(
                    &h,
                    size,
                    (2.5, 4.25),
                    (size as f32 - 3.75, size as f32 - 5.5),
                    &p,
                )
                .unwrap()
                {
                    h[c.index] = c.after;
                }
                actual.extend_from_slice(&h);
            }
        }
        let cs = size - 1;
        let collision = (0..cs * cs)
            .map(|i| if i % 11 == 0 { 90 } else { 0 })
            .collect();
        let mut s = TerrainSession::new(size, initial, cs, Some(collision)).unwrap();
        for op in [
            HeightOperation::Raise,
            HeightOperation::Lower,
            HeightOperation::Smooth,
            HeightOperation::Flatten,
            HeightOperation::Roughen,
        ] {
            s.paint_height((19.5, 21.25), 8.75, op, 17, Some(73), -12345)
                .unwrap();
            s.paint_height((22.0, 20.0), 7.25, op, 9, Some(81), -12345)
                .unwrap();
            s.commit_stroke();
            snapshot(&mut actual, &s);
        }
        s.paint_water((20.5, 20.75), 9.25, 64, 12, 110.0).unwrap();
        s.commit_stroke();
        snapshot(&mut actual, &s);
        s.undo();
        snapshot(&mut actual, &s);
        s.undo();
        snapshot(&mut actual, &s);
        s.redo();
        snapshot(&mut actual, &s);
        s.paint_height((20.0, 20.0), 5.0, HeightOperation::Lower, 7, None, 0)
            .unwrap();
        s.cancel_stroke();
        snapshot(&mut actual, &s);
        s.commit_baseline();
        snapshot(&mut actual, &s);
    }
    assert_eq!(actual.len() + 17 * 17 * 4, expected.len());
    let mismatch = actual.iter().zip(&expected).position(|(a, b)| a != b);
    assert!(mismatch.is_none(), "first layer byte mismatch {mismatch:?}");
    let mut offset = actual.len();
    let mut max_delta = 0.0f32;
    for y in -8..=8 {
        for x in -8..=8 {
            let cs = f32::from_le_bytes(expected[offset..offset + 4].try_into().unwrap());
            offset += 4;
            let rs = roughness_noise(x, y, -12345);
            max_delta = max_delta.max((cs - rs).abs());
            assert!((cs - rs).abs() <= 1e-6, "noise ({x},{y}) C#={cs} Rust={rs}");
        }
    }
    println!("C# differential: {} exact layer bytes, 24 ramps, 22 session snapshots, 289 noise samples, max noise delta {}",actual.len(),max_delta);
}
