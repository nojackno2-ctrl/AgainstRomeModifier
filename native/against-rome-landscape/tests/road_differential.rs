use against_rome_landscape::road::{plan, RoadTile};

/// Compares against output frozen from the original C# planner (tests/fixtures).
#[test]
fn frozen_csharp_road_differential() {
    let stdout = include_str!("fixtures/road_oracle.txt");
    let actual: Vec<_> = stdout
        .lines()
        .filter(|s| s.contains("|True|") || s.contains("|False|"))
        .collect();
    let available: Vec<_> = (1..=15)
        .map(|connections| RoadTile {
            texture: format!("road{connections}"),
            connections,
        })
        .collect();
    let mut expected = vec![];
    for a in 0..36 {
        for b in 0..36 {
            let mut textures = vec!["grass".into(); 36];
            for x in 0..6 {
                textures[12 + x] = "road10".into();
            }
            let p = plan(
                6,
                &textures,
                &[(a % 6, a / 6), (b % 6, b / 6), (2, 2)],
                &available,
                "road10",
            )
            .unwrap();
            let tiles = p
                .tiles
                .iter()
                .map(|t| format!("{},{},{}", t.x, t.y, t.texture))
                .collect::<Vec<_>>()
                .join(";");
            expected.push(format!(
                "{a},{b}|{}|{tiles}",
                if p.succeeded { "True" } else { "False" }
            ));
        }
    }
    assert_eq!(actual.len(), 1296, "oracle must execute every case");
    assert_eq!(
        actual, expected,
        "Rust road placements differ from linked C# sources"
    );
    println!("C# / Rust road differential: 1296 exact placement sequences match");
}
