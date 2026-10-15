use against_rome_landscape::river::{
    self, RiverElevationMode, RiverPlannerOptions, RiverTileCatalog,
};
use std::process::Command;

#[test]
#[ignore = "requires .NET 10 SDK to execute the linked C# oracle"]
fn csharp_river_differential() {
    let project = format!(
        "{}/tests/oracle/RoadOracle.csproj",
        env!("CARGO_MANIFEST_DIR")
    );
    let config = format!("{}/tests/oracle/NuGet.Config", env!("CARGO_MANIFEST_DIR"));
    let appdata = format!("{}/tests/oracle/cache", env!("CARGO_MANIFEST_DIR"));
    let restore = Command::new("dotnet")
        .env("APPDATA", &appdata)
        .args([
            "restore",
            &project,
            "--configfile",
            &config,
            "--verbosity",
            "quiet",
        ])
        .output()
        .expect("restore framework-only oracle");
    assert!(
        restore.status.success(),
        "{}{}",
        String::from_utf8_lossy(&restore.stdout),
        String::from_utf8_lossy(&restore.stderr)
    );
    let output = Command::new("dotnet")
        .env("APPDATA", &appdata)
        .args([
            "run",
            "--no-restore",
            "--project",
            &project,
            "-c",
            "Release",
            "--verbosity",
            "quiet",
            "--",
            "river",
        ])
        .output()
        .expect("execute .NET river oracle");
    assert!(
        output.status.success(),
        "C# oracle failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let actual: Vec<_> = stdout.lines().filter(|s| s.starts_with("R|")).collect();
    let catalog = RiverTileCatalog::build_available(None, None, None);
    let mut expected = vec![];
    for mode in 0..3 {
        for a in 0..16 {
            for b in 0..16 {
                let mut textures = vec!["grass".into(); 16];
                textures[3] = "SEE1".into();
                let heights: Vec<_> = (0..81)
                    .map(|i| (50 + (i * 7 + a * 3 + b) % 150) as u8)
                    .collect();
                let options = RiverPlannerOptions {
                    elevation_mode: match mode {
                        0 => RiverElevationMode::ValidateOnly,
                        1 => RiverElevationMode::AutoCarveDescending,
                        _ => RiverElevationMode::AutoCarveTrench,
                    },
                    auto_detect_downhill_flow: a % 2 == 0,
                    water_level: if b % 2 == 0 { Some(160.0) } else { None },
                    ..Default::default()
                };
                let p = river::plan(
                    4,
                    &textures,
                    Some(&heights),
                    9,
                    &[(a % 4, a / 4), (b % 4, b / 4), (2, 2)],
                    &catalog,
                    &options,
                )
                .unwrap();
                let water = p
                    .water_tiles
                    .iter()
                    .map(|t| {
                        format!(
                            "{},{},{},{},{:?},{}",
                            t.x, t.y, t.texture, t.connections, t.kind, t.flow
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(";");
                let bank = p
                    .bank_tiles
                    .iter()
                    .map(|t| format!("{},{},{},{}", t.x, t.y, t.texture, t.side))
                    .collect::<Vec<_>>()
                    .join(";");
                let heights = p
                    .height_adjustments
                    .iter()
                    .map(|h| {
                        format!(
                            "{},{},{},{},{}",
                            h.index, h.vertex_x, h.vertex_y, h.before, h.after
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(";");
                let warnings = p
                    .warnings
                    .iter()
                    .map(|w| {
                        format!(
                            "{},{},{},{}",
                            w.x, w.y, w.current_elevation, w.target_elevation
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(";");
                expected.push(format!(
                    "R|{mode},{a},{b}|{}|{water}|{bank}|{heights}|{warnings}",
                    if p.succeeded { "True" } else { "False" }
                ));
            }
        }
    }
    assert_eq!(actual.len(), 768, "oracle must execute every case");
    for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
        assert_eq!(actual, expected, "river case {index}");
    }
    println!("C# / Rust river differential: 768 exact water/bank/height/warning sequences match");
}
