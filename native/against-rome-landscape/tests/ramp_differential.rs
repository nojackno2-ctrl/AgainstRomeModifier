//! C# differential for `ramp::calculate_ramp` against the real `TerrainRampTool.cs`
//! (linked into tests/ramp_oracle/RampOracle.csproj). The case matrix mirrors
//! tests/ramp_oracle/Program.cs one-to-one.
//! Run: cargo test --release --locked --offline --test ramp_differential -- --ignored
use against_rome_landscape::ramp::{calculate_ramp, TerrainRampParameters};
use against_rome_landscape::TerrainFalloffType as T;
use std::process::Command;

fn grid(size: usize, pattern: u8) -> Vec<u8> {
    (0..size * size)
        .map(|i| {
            if pattern == 0 {
                ((i * 17 + i / size * 13) % 256) as u8
            } else {
                (i % size * 255 / (size - 1)) as u8
            }
        })
        .collect()
}

fn run_oracle() -> String {
    let root = env!("CARGO_MANIFEST_DIR");
    let project = format!("{root}/tests/ramp_oracle/RampOracle.csproj");
    let config = format!("{root}/tests/ramp_oracle/NuGet.Config");
    let appdata = format!("{root}/tests/oracle/cache");
    let run = |args: &[&str]| {
        let o = Command::new("dotnet")
            .env("APPDATA", &appdata)
            .args(args)
            .output()
            .expect("dotnet SDK required");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        assert!(o.status.success(), "{text}");
        String::from_utf8_lossy(&o.stdout).into_owned()
    };
    run(&["restore", &project, "--configfile", &config, "--verbosity", "quiet"]);
    run(&["run", "--no-restore", "--project", &project, "-c", "Release", "--verbosity", "quiet"])
}

#[test]
#[ignore = "requires .NET 10 SDK to execute the linked C# TerrainRampTool oracle"]
fn csharp_ramp_differential() {
    let stdout = run_oracle();
    let actual: Vec<&str> = stdout.lines().filter(|l| l.starts_with("R|")).collect();
    let types = [T::Smoothstep, T::Cosine, T::Spherical, T::Gaussian, T::Linear, T::Plateau];
    let mut expected = Vec::new();
    for size in [9usize, 33] {
        for pattern in [0u8, 1] {
            for ty in types {
                for caps in [false, true] {
                    let m = (size - 1) as f32;
                    let cases: [([f32; 4], f32, f32, Option<f32>, Option<f32>); 5] = [
                        ([1.5, 2.25, m - 1.75, m - 3.5], 3.5, 2.75, None, None),
                        ([2.0, m / 2.0, m - 2.0, m / 2.0], 2.0, 3.0, Some(12.5), Some(241.5)),
                        ([-3.0, -3.0, m + 4.0, m + 4.0], 0.0, 1.5, Some(-50.0), Some(900.0)),
                        ([4.5, 4.5, 4.5, 4.5], 4.0, 2.0, None, None),
                        ([3.0, 6.0, 3.25, 2.5], 1.0, 0.0, Some(100.0), Some(200.0)),
                    ];
                    for (p, width, falloff, h0, h1) in cases {
                        let params = TerrainRampParameters {
                            width,
                            falloff,
                            start_height: h0,
                            end_height: h1,
                            falloff_type: ty,
                            round_caps: caps,
                        };
                        let heights = grid(size, pattern);
                        let line = match calculate_ramp(&heights, size, (p[0], p[1]), (p[2], p[3]), &params) {
                            Ok(ch) => ch
                                .iter()
                                .map(|c| format!("{},{},{}", c.index, c.before, c.after))
                                .collect::<Vec<_>>()
                                .join(";"),
                            Err(_) => "ERR".into(),
                        };
                        expected.push(format!("R|{}|{line}", expected.len()));
                    }
                }
            }
        }
    }
    assert_eq!(actual.len(), expected.len(), "every oracle case must execute");
    for (i, (a, e)) in actual.iter().zip(&expected).enumerate() {
        assert_eq!(a, e, "ramp case {i}");
    }
    // Error cases.
    assert!(stdout.contains("E|neg|ERR") && stdout.contains("E|len|ERR"));
    let p = TerrainRampParameters { width: -1.0, ..Default::default() };
    assert!(calculate_ramp(&grid(9, 0), 9, (0.0, 0.0), (1.0, 1.0), &p).is_err());
    assert!(calculate_ramp(&[0u8; 80], 9, (0.0, 0.0), (1.0, 1.0), &Default::default()).is_err());
    println!("C# / Rust ramp differential: {} cases match", expected.len());
}
