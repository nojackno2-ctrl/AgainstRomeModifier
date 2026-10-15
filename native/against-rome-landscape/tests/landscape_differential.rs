use against_rome_landscape::{cliff, fortification as fort};
use std::process::Command;

#[test]
#[ignore = "requires .NET 10 SDK to execute the linked C# oracle"]
fn csharp_cliff_and_fortification_differential() {
    let root = env!("CARGO_MANIFEST_DIR");
    let project = format!("{root}/tests/oracle/RoadOracle.csproj");
    let config = format!("{root}/tests/oracle/NuGet.Config");
    let appdata = format!("{root}/tests/oracle/cache");
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
        .unwrap();
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
            "landscape",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let actual: Vec<_> = stdout
        .lines()
        .filter(|s| s.starts_with("C|") || s.starts_with("F|"))
        .collect();
    let mut expected = vec![];
    let catalog = cliff::CliffTileCatalog::create_default();
    for seed in 0..64 {
        let heights: Vec<_> = (0..36)
            .map(|i| ((i * 17 + seed * 13) % 97) as f32)
            .collect();
        let d = cliff::detect_from_tile_heights(6, &heights, 45.0, 15.0).unwrap();
        let cells = d
            .cliff_cells
            .iter()
            .map(|c| {
                format!(
                    "{},{},{:?},{},{}",
                    c.x,
                    c.y,
                    c.facing,
                    c.elevation_delta,
                    catalog
                        .pick_tile(c.facing, c.x, c.y, seed, Some("FELS"))
                        .unwrap()
                )
            })
            .collect::<Vec<_>>()
            .join(";");
        let contours = d
            .contours
            .iter()
            .map(|c| {
                format!(
                    "{},{}:{}",
                    c.id,
                    if c.is_closed_loop { "True" } else { "False" },
                    c.cells
                        .iter()
                        .map(|p| format!("{},{}", p.x, p.y))
                        .collect::<Vec<_>>()
                        .join("/")
                )
            })
            .collect::<Vec<_>>()
            .join(";");
        expected.push(format!("C|{seed}|{cells}|{contours}"));
    }
    let catalog = fort::WallTileCatalog::default();
    for (style_index, style) in [
        fort::FortificationStyle::RomanStoneWall,
        fort::FortificationStyle::GermanicPalisade,
        fort::FortificationStyle::CelticPalisade,
        fort::FortificationStyle::HunBarricade,
    ]
    .into_iter()
    .enumerate()
    {
        for a in 0..16 {
            let b = 15 - a;
            let options = fort::WallStrokeOptions {
                style,
                team: 3,
                auto_corner_towers: a % 2 == 0,
                forced_gate_locations: vec![(1, 1)],
                default_gate_state: if a % 3 == 0 {
                    fort::GatePassabilityState::Closed
                } else {
                    fort::GatePassabilityState::Open
                },
                ..Default::default()
            };
            let p = fort::plan_wall_stroke(
                4,
                &[(a % 4, a / 4), (b % 4, b / 4), (1, 1)],
                &options,
                &catalog,
                Some(&mut |x, z| (x + z) as f32),
                Some(&mut |x, z| x == 2 && z % 2 == 0),
                None,
            );
            let placements = p
                .placements
                .iter()
                .map(|t| {
                    format!(
                        "{},{:?},{},{},{},{},{},{},{},{},{}",
                        t.name_def,
                        t.kind,
                        t.tile.0,
                        t.tile.1,
                        t.world_x,
                        t.world_y,
                        t.world_z,
                        t.angle_deg,
                        t.team,
                        t.gate_state.map(|s| format!("{s:?}")).unwrap_or_default(),
                        t.foundation_texture.as_deref().unwrap_or("")
                    )
                })
                .collect::<Vec<_>>()
                .join(";");
            let mut collision = vec![0; 256];
            let changes = fort::synchronize_passability(&mut collision, 16, 4, &p).unwrap();
            let hex = collision
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect::<String>();
            let changes = changes
                .iter()
                .map(|c| format!("{},{},{}", c.index, c.before, c.after))
                .collect::<Vec<_>>()
                .join(";");
            expected.push(format!(
                "F|{style_index},{a}|{}|{placements}|{hex}|{changes}",
                if p.succeeded { "True" } else { "False" }
            ));
        }
    }
    assert_eq!(actual.len(), 128, "every oracle case must execute");
    for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
        assert_eq!(actual, expected, "landscape case {index}");
    }
    println!("C# / Rust landscape differential: 64 cliff topology/catalog and 64 fortification placement/collision sequences match");
}
