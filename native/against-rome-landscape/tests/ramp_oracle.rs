//! Hand-derived oracle for C# `TerrainRampTool.CalculateRamp` semantics.
use against_rome_landscape::ramp::{apply_changes, calculate_ramp, TerrainRampParameters};
use against_rome_landscape::TerrainFalloffType;

#[test]
fn oracle_linear_falloff_flat_caps_matches_csharp_rounding() {
    let size = 9;
    let heights = vec![0u8; size * size];
    let p = TerrainRampParameters {
        width: 2.0,
        falloff: 2.0,
        start_height: Some(100.0),
        end_height: Some(200.0),
        falloff_type: TerrainFalloffType::Linear,
        round_caps: false,
    };
    let changes = calculate_ramp(&heights, size, (2.0, 4.0), (6.0, 4.0), &p).unwrap();
    let mut out = heights.clone();
    apply_changes(&mut out, &changes, false).unwrap();
    let row = |y: usize| out[y * size..(y + 1) * size].to_vec();
    // Centre and flat-body rows (dist <= 1).
    for y in 3..=5 {
        assert_eq!(row(y), [0, 0, 100, 125, 150, 175, 200, 0, 0]);
    }
    // dist 2 -> weight 0.5; Math.Round ties-to-even: 62.5 -> 62, 87.5 -> 88.
    for y in [2, 6] {
        assert_eq!(row(y), [0, 0, 50, 62, 75, 88, 100, 0, 0]);
    }
    // dist 3 -> weight 0: untouched.
    for y in [0, 1, 7, 8] {
        assert_eq!(row(y), [0; 9]);
    }
}

#[test]
fn oracle_sampled_endpoints_and_diagonal_ramp_stay_in_bounds() {
    let size = 5;
    let mut heights = vec![0u8; size * size];
    heights[0] = 40;
    heights[size * size - 1] = 200;
    let p = TerrainRampParameters {
        width: 0.0,
        falloff: 1.0,
        ..Default::default()
    };
    // Endpoints beyond the grid are clamped for sampling; changes stay inside.
    let changes = calculate_ramp(&heights, size, (-3.0, -3.0), (9.0, 9.0), &p).unwrap();
    assert!(!changes.is_empty());
    assert!(changes.iter().all(|c| c.index < size * size && c.before != c.after));
}
