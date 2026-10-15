use against_rome_landscape::road::{plan, RoadTile};
use std::process::Command;

/// Explicit .NET gate; ordinary unit tests require only Rust and local path dependencies.
#[test]
#[ignore = "requires .NET 10 SDK to execute the linked C# oracle"]
fn csharp_road_differential() {
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
        ])
        .output()
        .expect("execute .NET road oracle");
    assert!(
        output.status.success(),
        "C# oracle failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
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
