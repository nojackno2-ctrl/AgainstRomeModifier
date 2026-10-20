use against_rome_landscape::erosion::{
    calculate_falloff, simulate_hydraulic_erosion, simulate_thermal_erosion, BrushRegion,
    DotNetRandom, HydraulicErosionParams, TerrainFalloffType, TerrainHeightWorkBuffer,
    ThermalErosionParams,
};

#[test]
fn oracle_dotnet_random_matches_system_random_stream() {
    let mut rng = DotNetRandom::new(42);
    let expected = [
        0.6681064659115423,
        0.14090729837348093,
        0.12551828945312568,
        0.5227642760252413,
        0.16843422416990353,
    ];
    for (i, &exp) in expected.iter().enumerate() {
        let val = rng.next_double();
        assert!(
            (val - exp).abs() < 1e-12,
            "sample {i}: got {val}, expected {exp}"
        );
    }
}

#[test]
fn oracle_thermal_erosion_5x5_ramp_matches_csharp() {
    let size = 5;
    let mut buf = TerrainHeightWorkBuffer::new(size).unwrap();
    for y in 0..size as i32 {
        for x in 0..size as i32 {
            buf.set(x, y, x as f32 * 20.0);
        }
    }

    let params = ThermalErosionParams::default();
    simulate_thermal_erosion(&mut buf, Some(&params), None, None);

    let expected: [f32; 25] = [
        33.35585, 35.786346, 39.626926, 43.467537, 46.0835,
        34.058563, 36.287167, 40.32548, 43.880085, 46.069798,
        34.216827, 36.55748, 40.498615, 44.111557, 46.73305,
        34.058563, 36.287163, 40.32548, 43.880085, 46.069798,
        33.355846, 35.786343, 39.62693, 43.467533, 46.0835,
    ];

    for (i, (&actual, &exp)) in buf.raw_data().iter().zip(&expected).enumerate() {
        assert!(
            (actual - exp).abs() < 1e-4,
            "at index {i} (x={}, y={}): actual={actual}, expected={exp}",
            i % size,
            i / size
        );
    }
}

#[test]
fn oracle_thermal_erosion_8x8_cliff_and_scree_matches_csharp() {
    let size = 8;
    let mut buf = TerrainHeightWorkBuffer::new(size).unwrap();
    for y in 0..size as i32 {
        for x in 0..size as i32 {
            let val = if x <= 3 { 150.0 } else { 50.0 };
            buf.set(x, y, val);
        }
    }

    let params = ThermalErosionParams {
        talus_angle_degrees: 35.0,
        erosion_rate: 0.4,
        iterations: 10,
        grid_spacing: 1.0,
    };
    let mut scree = vec![0.0f32; size * size];
    simulate_thermal_erosion(&mut buf, Some(&params), None, Some(&mut scree));

    let expected_heights: [f32; 64] = [
        126.324554, 124.26978, 112.93131, 105.77004, 92.73183, 84.16678, 75.88855, 71.781204,
        126.483955, 125.96045, 111.133316, 110.64298, 90.59633, 87.95793, 74.96172, 73.14047,
        126.93706, 127.1666, 111.41219, 111.21342, 89.237274, 89.545166, 73.62063, 73.16597,
        127.29953, 127.448494, 111.5007, 111.63369, 89.161674, 89.28663, 73.28305, 73.34674,
        127.29953, 127.448494, 111.5007, 111.63369, 89.161674, 89.28662, 73.28305, 73.34674,
        126.93706, 127.1666, 111.41219, 111.21341, 89.23726, 89.545166, 73.62063, 73.16596,
        126.483955, 125.96045, 111.133316, 110.64298, 90.59632, 87.95793, 74.961716, 73.14047,
        126.324554, 124.26978, 112.93131, 105.77004, 92.73181, 84.16678, 75.88855, 71.781204,
    ];

    for (i, (&actual, &exp)) in buf.raw_data().iter().zip(&expected_heights).enumerate() {
        assert!(
            (actual - exp).abs() < 1e-4,
            "Height mismatch at index {i} (x={}, y={}): actual={actual}, expected={exp}",
            i % size,
            i / size
        );
    }

    let expected_scree: [f32; 64] = [
        0.0, 22.46239, 49.990456, 85.3719, 124.99941, 81.63781, 46.9366, 21.781208,
        0.115531206, 26.247202, 52.924347, 98.80091, 142.96257, 95.03384, 52.223057, 24.466413,
        0.0, 22.450657, 45.125645, 84.702866, 125.08701, 86.09126, 47.00047, 23.165964,
        0.0, 22.636143, 45.192863, 84.01112, 123.46216, 85.31414, 46.46015, 23.34674,
        0.0, 22.636143, 45.19286, 84.011116, 123.46214, 85.31414, 46.460144, 23.346735,
        0.0, 22.450655, 45.12565, 84.702866, 125.08702, 86.091255, 47.000477, 23.16596,
        0.115531206, 26.247202, 52.924343, 98.80092, 142.96259, 95.03383, 52.223053, 24.466406,
        0.0, 22.462389, 49.99046, 85.371895, 124.99941, 81.637794, 46.936596, 21.781204,
    ];

    for (i, (&actual, &exp)) in scree.iter().zip(&expected_scree).enumerate() {
        assert!(
            (actual - exp).abs() < 1e-4,
            "Scree mismatch at index {i} (x={}, y={}): actual={actual}, expected={exp}",
            i % size,
            i / size
        );
    }
}

#[test]
fn oracle_thermal_erosion_10x10_brush_matches_csharp() {
    let size = 10;
    let mut buf = TerrainHeightWorkBuffer::new(size).unwrap();
    for y in 0..size as i32 {
        for x in 0..size as i32 {
            let dist = (((x as f32 - 5.0).powi(2) + (y as f32 - 5.0).powi(2))).sqrt();
            let val = (100.0 - dist * 18.0).max(10.0);
            buf.set(x, y, val);
        }
    }

    let params = ThermalErosionParams {
        iterations: 6,
        ..Default::default()
    };
    let brush = BrushRegion::new(5.0, 5.0, 3.5);
    simulate_thermal_erosion(&mut buf, Some(&params), Some(brush), None);

    let expected: [f32; 100] = [
        10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0,
        10.0, 10.0, 10.0, 20.305113, 27.665852, 30.215876, 27.665852, 20.305113, 10.0, 10.0,
        10.0, 10.0, 27.181177, 44.87895, 51.51496, 52.99763, 51.51496, 44.87895, 27.181177, 10.0,
        10.0, 20.305113, 44.87895, 54.445095, 56.124275, 57.85459, 56.124275, 54.445095, 44.87895, 20.305113,
        10.0, 27.665852, 51.51496, 56.124275, 59.242577, 60.05993, 59.242577, 56.124275, 51.51496, 27.665852,
        10.0, 30.215876, 52.99763, 57.85459, 60.05993, 62.883083, 60.05993, 57.85459, 52.99763, 30.215876,
        10.0, 27.665852, 51.51496, 56.124275, 59.242577, 60.05993, 59.242577, 56.124275, 51.51496, 27.665852,
        10.0, 20.305113, 44.87895, 54.445095, 56.124275, 57.85459, 56.124275, 54.445095, 44.87895, 20.305113,
        10.0, 10.0, 27.181177, 44.87895, 51.51496, 52.99763, 51.51496, 44.87895, 27.181177, 10.0,
        10.0, 10.0, 10.0, 20.305113, 27.665852, 30.215876, 27.665852, 20.305113, 10.0, 10.0,
    ];

    for (i, (&actual, &exp)) in buf.raw_data().iter().zip(&expected).enumerate() {
        assert!(
            (actual - exp).abs() < 1e-4,
            "Brush mismatch at index {i} (x={}, y={}): actual={actual}, expected={exp}",
            i % size,
            i / size
        );
    }
}

#[test]
fn oracle_hydraulic_erosion_8x8_10_droplets_matches_csharp() {
    let size = 8;
    let mut buf = TerrainHeightWorkBuffer::new(size).unwrap();
    for y in 0..size as i32 {
        for x in 0..size as i32 {
            let dist = (((x as f32 - 3.5).powi(2) + (y as f32 - 3.5).powi(2))).sqrt();
            let val = (100.0 - dist * 15.0).max(10.0);
            buf.set(x, y, val);
        }
    }

    let params = HydraulicErosionParams {
        droplet_count: 10,
        random_seed: 42,
        ..Default::default()
    };
    simulate_hydraulic_erosion(&mut buf, Some(&params), None);

    let expected: [f32; 64] = [
        25.753788, 35.48256, 41.55139, 46.350357, 46.96699, 42.881702, 35.48256, 25.753788,
        35.48256, 44.857403, 52.629383, 58.414497, 60.12399, 56.26786, 46.96699, 35.48256,
        41.55139, 52.29953, 62.955986, 70.73289, 72.40831, 66.52405, 56.26786, 42.881702,
        45.871098, 58.26562, 70.89304, 84.7395, 86.449005, 74.97189, 61.10184, 46.96699,
        45.584846, 59.68414, 73.570465, 87.04618, 88.08237, 75.243416, 61.757355, 46.96699,
        41.78581, 54.885715, 67.0843, 76.28292, 75.62741, 68.18019, 56.26786, 42.881702,
        35.48256, 46.27592, 56.26786, 61.757355, 61.757355, 56.26786, 46.96699, 35.48256,
        25.753788, 35.48256, 42.881702, 46.96699, 46.96699, 42.881702, 35.48256, 25.753788,
    ];

    for (i, (&actual, &exp)) in buf.raw_data().iter().zip(&expected).enumerate() {
        assert!(
            (actual - exp).abs() < 1e-4,
            "Hydro 8x8 mismatch at index {i} (x={}, y={}): actual={actual}, expected={exp}",
            i % size,
            i / size
        );
    }
}

#[test]
fn oracle_hydraulic_erosion_10x10_10_droplets_brush_matches_csharp() {
    let size = 10;
    let mut buf = TerrainHeightWorkBuffer::new(size).unwrap();
    for y in 0..size as i32 {
        for x in 0..size as i32 {
            let dist = (((x as f32 - 5.0).powi(2) + (y as f32 - 5.0).powi(2))).sqrt();
            let val = (100.0 - dist * 18.0).max(10.0);
            buf.set(x, y, val);
        }
    }

    let params = HydraulicErosionParams {
        droplet_count: 10,
        random_seed: 123,
        ..Default::default()
    };
    let brush = BrushRegion::new(5.0, 5.0, 3.0);
    simulate_hydraulic_erosion(&mut buf, Some(&params), Some(brush));

    let expected: [f32; 100] = [
        10.0, 10.0, 10.0, 10.0, 8.385265, 10.0, 10.0, 10.0, 10.0, 10.0,
        10.0, 9.264892, 9.297531, 16.940928, 21.751694, 25.439375, 25.001118, 19.501553, 10.0, 10.0,
        8.834276, 7.415818, 19.447071, 28.766378, 35.738384, 40.255604, 39.898304, 33.858437, 23.632467, 10.0,
        7.8273153, 15.891291, 29.58254, 39.935596, 49.70899, 56.855328, 55.759705, 47.522346, 34.317097, 19.501553,
        8.834276, 22.482937, 38.183838, 53.431507, 67.217964, 74.86815, 70.40927, 57.17836, 43.079002, 25.7841,
        10.0, 27.264893, 44.160553, 61.15383, 76.190094, 95.333244, 77.44972, 62.32162, 45.16081, 28.0,
        10.0, 25.7841, 43.079002, 58.245716, 72.07745, 78.30498, 71.28961, 57.63192, 43.079002, 25.7841,
        10.0, 19.501553, 35.10008, 49.08831, 58.50105, 62.42384, 57.661858, 49.08831, 35.10008, 19.501553,
        10.0, 10.0, 23.632467, 35.10008, 43.079002, 45.211918, 43.079002, 35.10008, 23.632467, 10.0,
        10.0, 10.0, 10.0, 19.501553, 25.7841, 28.0, 25.7841, 19.501553, 10.0, 10.0,
    ];

    for (i, (&actual, &exp)) in buf.raw_data().iter().zip(&expected).enumerate() {
        assert!(
            (actual - exp).abs() < 1e-4,
            "Hydro 10x10 brush mismatch at index {i} (x={}, y={}): actual={actual}, expected={exp}",
            i % size,
            i / size
        );
    }
}

#[test]
fn oracle_falloff_curves_match_expected_math() {
    // Smoothstep: at t=0 => 1.0, at t=0.5 => 0.5, at t=1.0 => 0.0
    assert_eq!(calculate_falloff(0.0, TerrainFalloffType::Smoothstep), 1.0);
    assert_eq!(calculate_falloff(0.5, TerrainFalloffType::Smoothstep), 0.5);
    assert_eq!(calculate_falloff(1.0, TerrainFalloffType::Smoothstep), 0.0);

    // Linear
    assert_eq!(calculate_falloff(0.25, TerrainFalloffType::Linear), 0.75);

    // Plateau: flat 1.0 below 0.4
    assert_eq!(calculate_falloff(0.2, TerrainFalloffType::Plateau), 1.0);
    assert_eq!(calculate_falloff(0.4, TerrainFalloffType::Plateau), 1.0);
    assert!(calculate_falloff(0.7, TerrainFalloffType::Plateau) < 1.0);
}

#[test]
fn oracle_simulation_guards_and_input_validation() {
    let mut buf = TerrainHeightWorkBuffer::new(2).unwrap();
    buf.set(0, 0, 100.0);

    // Size < 3 is safe no-op
    simulate_thermal_erosion(&mut buf, None, None, None);
    assert_eq!(buf.get(0, 0), 100.0);

    simulate_hydraulic_erosion(&mut buf, None, None);
    assert_eq!(buf.get(0, 0), 100.0);

    // Iterations = 0 is safe no-op
    let mut buf3 = TerrainHeightWorkBuffer::new(3).unwrap();
    buf3.set(1, 1, 100.0);
    simulate_thermal_erosion(
        &mut buf3,
        Some(&ThermalErosionParams {
            iterations: 0,
            ..Default::default()
        }),
        None,
        None,
    );
    assert_eq!(buf3.get(1, 1), 100.0);

    // DropletCount = 0 is safe no-op
    simulate_hydraulic_erosion(
        &mut buf3,
        Some(&HydraulicErosionParams {
            droplet_count: 0,
            ..Default::default()
        }),
        None,
    );
    assert_eq!(buf3.get(1, 1), 100.0);

    // MaxLifetime = 0 is safe no-op
    simulate_hydraulic_erosion(
        &mut buf3,
        Some(&HydraulicErosionParams {
            max_lifetime: 0,
            ..Default::default()
        }),
        None,
    );
    assert_eq!(buf3.get(1, 1), 100.0);
}
