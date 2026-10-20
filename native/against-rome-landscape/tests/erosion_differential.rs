//! C# differential for `erosion::{simulate_thermal_erosion, simulate_hydraulic_erosion}` against the
//! real `ThermalErosionSimulator.cs` / `HydraulicErosionSimulator.cs` (+ `TerrainHeightWorkBuffer.cs`)
//! linked into tests/erosion_oracle/ErosionOracle.csproj. Rust is the single source of the case matrix:
//! it writes every input (f32 bit patterns) to a file the C# oracle replays, then compares outputs.
//! Run: cargo test --release --locked --offline --test erosion_differential -- --ignored
use against_rome_landscape::erosion::{
    simulate_hydraulic_erosion, simulate_thermal_erosion, BrushRegion, HydraulicErosionParams,
    TerrainHeightWorkBuffer, ThermalErosionParams,
};
use std::collections::HashMap;
use std::process::Command;

/// Max |rust - csharp| per sample. Thermal is pure f32 arithmetic (+ `tan`); hydraulic adds sin/cos/sqrt
/// and chaotic droplet paths, so it is bounded separately and reported with the exact-match ratio.
const THERMAL_TOL: f32 = 1e-3;
const HYDRAULIC_TOL: f32 = 1e-2;

fn lcg(state: &mut u32) -> f32 {
    *state = state.wrapping_mul(1664525).wrapping_add(1013904223);
    (*state >> 8) as f32 / (1u32 << 24) as f32
}

fn terrain(size: usize, pattern: u8) -> Vec<f32> {
    let mut s = 0x1234_5678u32 ^ (size as u32) << 8 ^ pattern as u32;
    let c = (size as f32 - 1.0) / 2.0;
    (0..size * size)
        .map(|i| {
            let (x, y) = ((i % size) as f32, (i / size) as f32);
            match pattern {
                0 => x * 255.0 / (size as f32 - 1.0),
                1 => (230.0 - ((x - c).powi(2) + (y - c).powi(2)).sqrt() * 255.0 / size as f32 * 1.6)
                    .clamp(0.0, 255.0),
                2 => 20.0 + lcg(&mut s) * 215.0,
                3 => {
                    if x < c {
                        190.5
                    } else {
                        40.25
                    }
                }
                4 => 100.0,
                _ => (x * 0.7 + y * 1.3).sin() * 80.0 + 128.0 + lcg(&mut s) * 3.0,
            }
        })
        .collect()
}

fn bits(v: &[f32]) -> String {
    v.iter().map(|f| format!("{:x}", f.to_bits())).collect::<Vec<_>>().join(",")
}
fn fb(f: f32) -> String {
    format!("{:x}", f.to_bits())
}
fn brush_s(b: Option<BrushRegion>) -> String {
    b.map_or("-".into(), |b| format!("{}:{}:{}", fb(b.center_x), fb(b.center_y), fb(b.radius)))
}
fn parse(s: &str) -> Vec<f32> {
    s.split(',').map(|h| f32::from_bits(u32::from_str_radix(h, 16).unwrap())).collect()
}

struct Thermal {
    size: usize,
    pat: u8,
    p: ThermalErosionParams,
    brush: Option<BrushRegion>,
    scree: bool,
}
struct Hydro {
    size: usize,
    pat: u8,
    p: HydraulicErosionParams,
    brush: Option<BrushRegion>,
}

fn thermal_cases() -> Vec<Thermal> {
    let mut v = Vec::new();
    let t = |size, pat, p: ThermalErosionParams, brush, scree| Thermal { size, pat, p, brush, scree };
    let d = ThermalErosionParams::default;
    // Pattern x size sweep with defaults and scree output.
    for size in [3usize, 4, 9, 17, 33] {
        for pat in 0..6u8 {
            v.push(t(size, pat, d(), None, true));
        }
    }
    // Parameter boundaries: talus clamp [5,85], rate 0..1(+overshoot), iterations, spacing.
    for talus in [-10.0f32, 0.0, 4.9, 5.0, 20.0, 45.0, 84.9, 85.0, 90.0, 400.0] {
        v.push(t(17, 3, ThermalErosionParams { talus_angle_degrees: talus, ..d() }, None, true));
        v.push(t(17, 2, ThermalErosionParams { talus_angle_degrees: talus, iterations: 3, ..d() }, None, false));
    }
    for rate in [0.0f32, 0.05, 0.4, 1.0, 2.5, -0.5] {
        v.push(t(17, 3, ThermalErosionParams { erosion_rate: rate, ..d() }, None, true));
    }
    for iters in [0usize, 1, 2, 8, 40] {
        v.push(t(17, 5, ThermalErosionParams { iterations: iters, ..d() }, None, true));
    }
    for sp in [0.0f32, 0.25, 0.5, 1.0, 2.0, 16.0] {
        v.push(t(17, 3, ThermalErosionParams { grid_spacing: sp, ..d() }, None, false));
    }
    // Brush: interior, edge-hugging, corner, oversize, tiny, and non-brush radii (<= 0 => full map).
    let brushes = [
        (8.0, 8.0, 5.5),
        (0.0, 0.0, 4.0),
        (16.0, 16.0, 6.0),
        (3.3, 12.7, 0.4),
        (8.0, 8.0, 100.0),
        (8.0, 8.0, 0.0),
        (8.0, 8.0, -3.0),
        (-5.0, 8.0, 7.0),
        (40.0, 40.0, 3.0),
    ];
    for (cx, cy, r) in brushes {
        for pat in [1u8, 2, 3] {
            let p = ThermalErosionParams { iterations: 6, ..d() };
            v.push(t(17, pat, p, Some(BrushRegion::new(cx, cy, r)), pat == 3));
        }
    }
    v
}

fn hydro_cases() -> Vec<Hydro> {
    let mut v = Vec::new();
    let h = |size, pat, p: HydraulicErosionParams, brush| Hydro { size, pat, p, brush };
    let d = |n: usize, seed: i32| HydraulicErosionParams { droplet_count: n, random_seed: seed, ..Default::default() };
    // Pattern x size sweep; flat pattern exercises the random-perturbation branch.
    for size in [3usize, 4, 9, 17, 33] {
        for pat in 0..6u8 {
            v.push(h(size, pat, d(60, 42), None));
        }
    }
    // Fixed seeds, including 0 and i32::MAX.
    for seed in [0, 1, 42, 12345, i32::MAX] {
        v.push(h(33, 1, d(200, seed), None));
        v.push(h(33, 5, d(200, seed), None));
    }
    // Parameter boundaries.
    for count in [0usize, 1, 5, 500] {
        v.push(h(17, 1, d(count, 7), None));
    }
    for life in [0usize, 1, 2, 10, 100] {
        v.push(h(17, 1, HydraulicErosionParams { max_lifetime: life, ..d(80, 9) }, None));
    }
    let tweak: [fn(&mut HydraulicErosionParams, f32); 7] = [
        |p, x| p.inertia = x,
        |p, x| p.gravity = x,
        |p, x| p.capacity_factor = x,
        |p, x| p.min_slope = x,
        |p, x| p.erosion_rate = x,
        |p, x| p.deposition_rate = x,
        |p, x| p.evaporation_rate = x,
    ];
    for f in tweak {
        for x in [0.0f32, 0.01, 0.5, 0.99, 1.0, 4.0] {
            let mut p = d(80, 11);
            f(&mut p, x);
            v.push(h(17, 5, p, None));
        }
    }
    for radius in [0usize, 1, 2, 3, 6] {
        v.push(h(17, 1, HydraulicErosionParams { erosion_radius: radius, ..d(80, 13) }, None));
    }
    // Brush starts (disc sampling, rejection near border, non-brush radii).
    let brushes = [
        (16.0, 16.0, 10.0),
        (8.0, 8.0, 6.0),
        (2.0, 2.0, 5.0),
        (30.0, 30.0, 8.0),
        (16.0, 16.0, 0.0),
        (16.0, 16.0, -4.0),
        (16.0, 16.0, 200.0),
        (-3.0, 16.0, 9.0),
        (16.0, 16.0, 0.3),
    ];
    for (cx, cy, r) in brushes {
        for pat in [1u8, 2] {
            v.push(h(33, pat, d(120, 21), Some(BrushRegion::new(cx, cy, r))));
        }
    }
    v
}

fn run_oracle(cases_file: &str) -> String {
    let root = env!("CARGO_MANIFEST_DIR");
    let project = format!("{root}/tests/erosion_oracle/ErosionOracle.csproj");
    let config = format!("{root}/tests/erosion_oracle/NuGet.Config");
    let appdata = format!("{root}/tests/oracle/cache");
    let run = |args: &[&str]| {
        let o = Command::new("dotnet")
            .env("APPDATA", &appdata)
            .args(args)
            .output()
            .expect("dotnet SDK required");
        let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        assert!(o.status.success(), "{text}");
        String::from_utf8_lossy(&o.stdout).into_owned()
    };
    run(&["restore", &project, "--configfile", &config, "--verbosity", "quiet"]);
    run(&["run", "--no-restore", "--project", &project, "-c", "Release", "--verbosity", "quiet", "--", cases_file])
}

#[derive(Default)]
struct Stats {
    samples: usize,
    exact: usize,
    max: f32,
}
impl Stats {
    fn add(&mut self, id: &str, what: &str, rust: &[f32], cs: &[f32], tol: f32) {
        assert_eq!(rust.len(), cs.len(), "{id} {what} length");
        for (i, (&r, &c)) in rust.iter().zip(cs).enumerate() {
            self.samples += 1;
            self.exact += (r.to_bits() == c.to_bits()) as usize;
            let d = (r - c).abs();
            assert!(d <= tol, "{id} {what}[{i}]: rust={r} csharp={c} diff={d}");
            self.max = self.max.max(d);
        }
    }
}

#[test]
#[ignore = "requires .NET 10 SDK to execute the linked C# erosion simulators"]
fn csharp_erosion_differential() {
    let thermal = thermal_cases();
    let hydro = hydro_cases();
    let mut input = String::new();
    for (i, c) in thermal.iter().enumerate() {
        input += &format!(
            "T|t{i}|{}|{}|{}|{}|{}|{}|{}|{}\n",
            c.size,
            fb(c.p.talus_angle_degrees),
            fb(c.p.erosion_rate),
            c.p.iterations,
            fb(c.p.grid_spacing),
            brush_s(c.brush),
            c.scree as u8,
            bits(&terrain(c.size, c.pat))
        );
    }
    for (i, c) in hydro.iter().enumerate() {
        let p = &c.p;
        input += &format!(
            "H|h{i}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}\n",
            c.size,
            p.droplet_count,
            p.max_lifetime,
            fb(p.inertia),
            fb(p.gravity),
            fb(p.capacity_factor),
            fb(p.min_slope),
            fb(p.erosion_rate),
            fb(p.deposition_rate),
            fb(p.evaporation_rate),
            p.erosion_radius,
            p.random_seed,
            brush_s(c.brush),
            bits(&terrain(c.size, c.pat))
        );
    }
    let path = std::env::temp_dir().join(format!("erosion_cases_{}.txt", std::process::id()));
    std::fs::write(&path, input).unwrap();
    let stdout = run_oracle(path.to_str().unwrap());
    let _ = std::fs::remove_file(&path);

    let out: HashMap<String, Vec<&str>> = stdout
        .lines()
        .filter(|l| l.starts_with("T|") || l.starts_with("H|"))
        .map(|l| {
            let f: Vec<&str> = l.split('|').collect();
            (f[1].to_string(), f[2..].to_vec())
        })
        .collect();
    assert_eq!(out.len(), thermal.len() + hydro.len(), "every oracle case must execute: {stdout}");

    let (mut ts, mut hs) = (Stats::default(), Stats::default());
    let (mut changed_t, mut changed_h) = (false, false);
    for (i, c) in thermal.iter().enumerate() {
        let id = format!("t{i}");
        let o = &out[&id];
        assert!(!o[0].starts_with("ERR"), "{id} C# threw {}", o[0]);
        let mut buf = TerrainHeightWorkBuffer::from_floats(c.size, &terrain(c.size, c.pat)).unwrap();
        let mut scree = vec![0.0f32; c.size * c.size];
        simulate_thermal_erosion(&mut buf, Some(&c.p), c.brush, c.scree.then_some(&mut scree[..]));
        changed_t |= buf.raw_data() != &terrain(c.size, c.pat)[..];
        ts.add(&id, "heights", buf.raw_data(), &parse(o[0]), THERMAL_TOL);
        if c.scree {
            ts.add(&id, "scree", &scree, &parse(o[1]), THERMAL_TOL);
        }
    }
    for (i, c) in hydro.iter().enumerate() {
        let id = format!("h{i}");
        let o = &out[&id];
        assert!(!o[0].starts_with("ERR"), "{id} C# threw {}", o[0]);
        let mut buf = TerrainHeightWorkBuffer::from_floats(c.size, &terrain(c.size, c.pat)).unwrap();
        simulate_hydraulic_erosion(&mut buf, Some(&c.p), c.brush);
        changed_h |= buf.raw_data() != &terrain(c.size, c.pat)[..];
        hs.add(&id, "heights", buf.raw_data(), &parse(o[0]), HYDRAULIC_TOL);
    }
    assert!(changed_t && changed_h, "matrix must actually erode terrain");
    println!(
        "C# / Rust erosion differential: thermal {} cases {} samples ({} bit-exact, max diff {:e}); hydraulic {} cases {} samples ({} bit-exact, max diff {:e})",
        thermal.len(),
        ts.samples,
        ts.exact,
        ts.max,
        hydro.len(),
        hs.samples,
        hs.exact,
        hs.max
    );
}
