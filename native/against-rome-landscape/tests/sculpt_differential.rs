//! Concrete tests plus a C# differential for `sculpt::{terrace, sharpen_ridge, carve_valley}` and the
//! pipeline wrappers against the real `TerrainSculptFilter.cs` / `TerrainSculptPipeline.cs` linked into
//! tests/sculpt_oracle/. Rust writes every input (f32 bit patterns) for the oracle to replay.
//! Run: cargo test --release --locked --offline --test sculpt_differential -- --ignored
use against_rome_landscape::erosion::{TerrainHeightWorkBuffer, TerrainSampleChange};
use against_rome_landscape::sculpt::{
    carve_valley, sharpen_ridge, sharpen_ridge_changes, terrace, terrace_changes, TerraceParams,
};
use std::collections::HashMap;
use std::process::Command;

const TOL: f32 = 1e-3;

fn buf(size: usize, v: &[f32]) -> TerrainHeightWorkBuffer {
    TerrainHeightWorkBuffer::from_floats(size, v).unwrap()
}

#[test]
fn terrace_quantizes_slope_toward_steps() {
    // step 16, default flatness 0.85 / sharpness 1.5: 16 and 24 are fixed points, 20 is pulled up.
    let mut b = buf(2, &[16.0, 24.0, 20.0, 32.0]);
    terrace(&mut b, -1.0, -1.0, 0.0, &TerraceParams::new(16.0));
    let d = b.raw_data();
    assert_eq!(d[0], 16.0);
    assert!((d[1] - 24.0).abs() < 1e-4);
    assert!((d[2] - 20.99584).abs() < 1e-3, "{}", d[2]);
    assert_eq!(d[3], 32.0);
}

#[test]
fn terrace_noops_and_brush_bounds() {
    let src = [20.0f32; 25];
    for p in [
        TerraceParams::new(0.1),
        TerraceParams::new(-4.0),
        TerraceParams {
            strength: 0.0,
            ..TerraceParams::new(16.0)
        },
    ] {
        let mut b = buf(5, &src);
        terrace(&mut b, -1.0, -1.0, 0.0, &p);
        assert_eq!(b.raw_data(), &src);
    }
    // Brush mode touches only the disc: far corner is unchanged, centre moves.
    let mut b = buf(5, &src);
    terrace(&mut b, 1.0, 1.0, 1.5, &TerraceParams::new(16.0));
    assert_ne!(b.get(1, 1), 20.0);
    assert_eq!(b.get(4, 4), 20.0);
    // A centre with a negative axis is global (C# isGlobal).
    let mut g = buf(5, &src);
    terrace(&mut g, -1.0, 2.0, 1.0, &TerraceParams::new(16.0));
    assert_ne!(g.get(4, 4), 20.0);
}

#[test]
fn terrace_clamps_output_and_parameters() {
    let mut b = buf(2, &[250.0, 255.0, 1.0, 0.0]);
    let p = TerraceParams {
        flatness: 9.0,
        edge_sharpness: 0.0,
        strength: 5.0,
        ..TerraceParams::new(40.0)
    };
    terrace(&mut b, -1.0, -1.0, 0.0, &p);
    assert!(b.raw_data().iter().all(|h| (0.0..=255.0).contains(h)));
}

#[test]
fn sharpen_ridge_peak_and_ridge_only() {
    let mut src = [0.0f32; 9];
    src[4] = 100.0;
    let mut b = buf(3, &src);
    sharpen_ridge(&mut b, -1.0, -1.0, 0.0, 0.5, true, 1);
    assert_eq!(b.get(1, 1), 255.0); // 100 + 0.5 * 400 clamps to 255
    assert_eq!(b.get(1, 0), 0.0); // concave neighbours unchanged when ridge_only
    let mut both = buf(3, &src);
    sharpen_ridge(&mut both, -1.0, -1.0, 0.0, 0.5, false, 1);
    assert_eq!(both.get(1, 0), 0.0); // delta -50 clamps at 0
    assert_eq!(both.get(1, 1), 255.0);
    // Non-positive gain / iterations are no-ops.
    let mut n = buf(3, &src);
    sharpen_ridge(&mut n, -1.0, -1.0, 0.0, 0.0, true, 1);
    sharpen_ridge(&mut n, -1.0, -1.0, 0.0, 1.0, true, 0);
    assert_eq!(n.raw_data(), &src);
}

#[test]
fn carve_valley_deepens_pit_and_requires_radius() {
    let mut src = [200.0f32; 9];
    src[4] = 100.0;
    let mut b = buf(3, &src);
    carve_valley(&mut b, 1.0, 1.0, 1.5, 0.1, 1.0);
    assert!((b.get(1, 1) - 60.0).abs() < 1e-4); // laplacian 400 * 0.1 * weight 1
    assert_eq!(b.get(0, 0), 200.0); // outside the disc
    for (r, d, s) in [(0.0, 1.0, 1.0), (1.5, 0.0, 1.0), (1.5, 1.0, 0.0)] {
        let mut n = buf(3, &src);
        carve_valley(&mut n, 1.0, 1.0, r, d, s);
        assert_eq!(n.raw_data(), &src);
    }
    // Floor at zero.
    let mut pit = [255.0f32; 9];
    pit[4] = 5.0;
    let mut deep = buf(3, &pit);
    carve_valley(&mut deep, 1.0, 1.0, 1.5, 5.0, 1.0);
    assert_eq!(deep.get(1, 1), 0.0);
}

#[test]
fn pipeline_returns_changes_leaves_input_and_reports_errors() {
    let heights: Vec<u8> = (0..25).map(|i| (i * 9) as u8).collect();
    let before = heights.clone();
    let p = TerraceParams::new(16.0);
    let changes = terrace_changes(5, &heights, -1.0, -1.0, 0.0, &p).unwrap();
    assert_eq!(heights, before);
    assert!(!changes.is_empty());
    for c in &changes {
        assert_eq!(c.before, before[c.index]);
        assert_ne!(c.before, c.after);
    }
    assert!(terrace_changes(5, &heights[..24], -1.0, -1.0, 0.0, &p).is_err());
    assert!(terrace_changes(1, &[0], -1.0, -1.0, 0.0, &p).is_err());
    // Flat map under sharpen produces no changes.
    assert!(
        sharpen_ridge_changes(3, &[7; 9], -1.0, -1.0, 0.0, 1.0, false, 3)
            .unwrap()
            .is_empty()
    );
}

fn lcg(state: &mut u32) -> f32 {
    *state = state.wrapping_mul(1664525).wrapping_add(1013904223);
    (*state >> 8) as f32 / (1u32 << 24) as f32
}

fn terrain(size: usize, pattern: u8) -> Vec<f32> {
    let mut s = 0xBEEF_1234u32 ^ (size as u32) << 8 ^ pattern as u32;
    let c = (size as f32 - 1.0) / 2.0;
    (0..size * size)
        .map(|i| {
            let (x, y) = ((i % size) as f32, (i / size) as f32);
            match pattern {
                0 => x * 255.0 / (size as f32 - 1.0),
                1 => (230.0
                    - ((x - c).powi(2) + (y - c).powi(2)).sqrt() * 255.0 / size as f32 * 1.6)
                    .clamp(0.0, 255.0),
                2 => lcg(&mut s) * 255.0,
                3 => (30.0 + ((x - c).powi(2) + (y - c).powi(2)).sqrt() * 12.0).min(255.0),
                4 => 100.0,
                // integral heights hitting exact terrace ties, plus hard edges at 0 / 255
                5 => ((x as i32 * 17 + y as i32 * 31) % 256) as f32,
                _ => (x * 0.7 + y * 1.3).sin() * 80.0 + 128.0 + lcg(&mut s) * 3.0,
            }
        })
        .collect()
}

fn fb(f: f32) -> String {
    format!("{:x}", f.to_bits())
}
fn bits(v: &[f32]) -> String {
    v.iter().map(|f| fb(*f)).collect::<Vec<_>>().join(",")
}
fn parse(s: &str) -> Vec<f32> {
    s.split(',')
        .map(|h| f32::from_bits(u32::from_str_radix(h, 16).unwrap()))
        .collect()
}
fn bytes_of(v: &[f32]) -> Vec<u8> {
    v.iter()
        .map(|f| f.round_ties_even().clamp(0.0, 255.0) as u8)
        .collect()
}
fn dec(v: &[u8]) -> String {
    v.iter()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

type Brush = (f32, f32, f32);

const BRUSHES: [Brush; 10] = [
    (-1.0, -1.0, 0.0), // global
    (4.0, 4.0, 3.5),
    (0.0, 0.0, 3.0),
    (8.0, 8.0, 2.0),
    (2.6, 6.3, 0.4),
    (4.0, 4.0, 100.0),
    (4.0, 4.0, -2.0),
    (-3.0, 4.0, 5.0),
    (4.0, -3.0, 5.0),
    (40.0, 40.0, 3.0),
];

fn run_oracle(cases_file: &str) -> String {
    let root = env!("CARGO_MANIFEST_DIR");
    let project = format!("{root}/tests/sculpt_oracle/SculptOracle.csproj");
    let config = format!("{root}/tests/sculpt_oracle/NuGet.Config");
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
    run(&[
        "restore",
        &project,
        "--configfile",
        &config,
        "--verbosity",
        "quiet",
    ]);
    run(&[
        "run",
        "--no-restore",
        "--project",
        &project,
        "-c",
        "Release",
        "--verbosity",
        "quiet",
        "--",
        cases_file,
    ])
}

enum Case {
    A(usize, u8, Brush, TerraceParams),
    B(usize, u8, Brush, f32, bool, i32),
    C(usize, u8, Brush, f32, f32),
    P(usize, u8, Brush, TerraceParams),
    Q(usize, u8, Brush, f32, bool, i32),
}

impl Case {
    fn grid(&self) -> (usize, u8) {
        match self {
            Case::A(s, p, ..)
            | Case::B(s, p, ..)
            | Case::C(s, p, ..)
            | Case::P(s, p, ..)
            | Case::Q(s, p, ..) => (*s, *p),
        }
    }
}

fn tp(step: f32, flat: f32, edge: f32, st: f32, base: f32) -> TerraceParams {
    TerraceParams {
        step_interval: step,
        flatness: flat,
        edge_sharpness: edge,
        strength: st,
        base_offset: base,
    }
}

fn cases() -> Vec<Case> {
    let mut v = Vec::new();
    for size in [3usize, 5, 9, 17] {
        for pat in 0..7u8 {
            v.push(Case::A(size, pat, BRUSHES[0], TerraceParams::new(16.0)));
            v.push(Case::B(size, pat, BRUSHES[0], 0.5, true, 1));
            v.push(Case::B(size, pat, BRUSHES[1], 0.8, false, 2));
            v.push(Case::C(size, pat, BRUSHES[1], 1.0, 1.0));
            v.push(Case::P(size, pat, BRUSHES[0], TerraceParams::new(24.0)));
            v.push(Case::Q(size, pat, BRUSHES[0], 0.4, false, 2));
        }
    }
    for (i, b) in BRUSHES.iter().enumerate() {
        for pat in [1u8, 2, 3, 5] {
            v.push(Case::A(9, pat, *b, TerraceParams::new(20.0)));
            v.push(Case::B(9, pat, *b, 0.7, i % 2 == 0, 2));
            v.push(Case::C(9, pat, *b, 0.6, 0.9));
            v.push(Case::P(9, pat, *b, tp(12.0, 0.5, 2.5, 0.7, 3.0)));
            v.push(Case::Q(9, pat, *b, 0.6, true, 1));
        }
    }
    // Terrace parameter boundaries, including clamp edges and negative/odd base offsets.
    for step in [0.0f32, 0.1, 0.11, 1.0, 7.5, 64.0, 300.0] {
        v.push(Case::A(9, 2, BRUSHES[0], TerraceParams::new(step)));
    }
    for flat in [-1.0f32, 0.0, 0.5, 1.0, 3.0] {
        for edge in [-2.0f32, 0.5, 1.0, 1.0001, 1.5, 2.0, 4.0, 9.0] {
            v.push(Case::A(9, 2, BRUSHES[1], tp(16.0, flat, edge, 1.0, 0.0)));
            v.push(Case::P(9, 5, BRUSHES[0], tp(16.0, flat, edge, 1.0, 0.0)));
        }
    }
    for st in [-1.0f32, 0.0, 0.3, 1.0, 4.0] {
        v.push(Case::A(9, 2, BRUSHES[0], tp(16.0, 0.85, 1.5, st, 0.0)));
    }
    for base in [-50.0f32, -8.0, 0.0, 5.5, 16.0, 300.0] {
        v.push(Case::A(9, 2, BRUSHES[0], tp(16.0, 0.85, 1.5, 1.0, base)));
        v.push(Case::P(9, 5, BRUSHES[0], tp(16.0, 0.85, 1.5, 1.0, base)));
    }
    // Sharpen / carve parameter boundaries.
    for gain in [-1.0f32, 0.0, 0.0005, 0.3, 1.0, 5.0] {
        for it in [-1, 0, 1, 3] {
            v.push(Case::B(9, 1, BRUSHES[0], gain, true, it));
            v.push(Case::B(9, 6, BRUSHES[1], gain, false, it));
        }
    }
    for depth in [-1.0f32, 0.0, 0.01, 0.5, 2.0, 50.0] {
        for st in [-1.0f32, 0.0, 0.5, 1.0, 3.0] {
            v.push(Case::C(9, 3, BRUSHES[1], depth, st));
        }
    }
    // Pipeline error contract: undersized grids are rejected by the C# session constructor.
    v.push(Case::P(1, 4, BRUSHES[0], TerraceParams::new(16.0)));
    v
}

fn brush_s(b: Brush) -> String {
    format!("{}|{}|{}", fb(b.0), fb(b.1), fb(b.2))
}

fn terrace_s(p: &TerraceParams) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        fb(p.step_interval),
        fb(p.flatness),
        fb(p.edge_sharpness),
        fb(p.strength),
        fb(p.base_offset)
    )
}

fn encode(i: usize, c: &Case) -> String {
    let (s, pat) = c.grid();
    let src = terrain(s, pat);
    match c {
        Case::A(_, _, br, p) => format!(
            "A|c{i}|{s}|{}|{}|{}\n",
            brush_s(*br),
            terrace_s(p),
            bits(&src)
        ),
        Case::B(_, _, br, g, r, it) => {
            format!(
                "B|c{i}|{s}|{}|{}|{}|{it}|{}\n",
                brush_s(*br),
                fb(*g),
                *r as u8,
                bits(&src)
            )
        }
        Case::C(_, _, br, d, st) => {
            format!(
                "C|c{i}|{s}|{}|{}|{}|{}\n",
                brush_s(*br),
                fb(*d),
                fb(*st),
                bits(&src)
            )
        }
        Case::P(_, _, br, p) => {
            format!(
                "P|c{i}|{s}|{}|{}|{}\n",
                brush_s(*br),
                terrace_s(p),
                dec(&bytes_of(&src))
            )
        }
        Case::Q(_, _, br, g, r, it) => format!(
            "Q|c{i}|{s}|{}|{}|{}|{it}|{}\n",
            brush_s(*br),
            fb(*g),
            *r as u8,
            dec(&bytes_of(&src))
        ),
    }
}

#[test]
#[ignore = "requires .NET 10 SDK to execute the linked C# sculpt filters"]
fn csharp_sculpt_differential() {
    let cs = cases();
    let input: String = cs.iter().enumerate().map(|(i, c)| encode(i, c)).collect();
    let path = std::env::temp_dir().join(format!("sculpt_cases_{}.txt", std::process::id()));
    std::fs::write(&path, input).unwrap();
    let stdout = run_oracle(path.to_str().unwrap());
    let _ = std::fs::remove_file(&path);

    let out: HashMap<String, String> = stdout
        .lines()
        .filter(|l| {
            matches!(l.as_bytes().first(), Some(b'A' | b'B' | b'C' | b'P' | b'Q'))
                && l.as_bytes().get(1) == Some(&b'|')
        })
        .map(|l| {
            let f: Vec<&str> = l.splitn(3, '|').collect();
            (f[1].to_string(), f[2].to_string())
        })
        .collect();
    assert_eq!(
        out.len(),
        cs.len(),
        "every oracle case must execute: {stdout}"
    );

    let (mut samples, mut exact, mut max) = (0usize, 0usize, 0f32);
    let (mut changed, mut errs, mut pipeline_changes) = (0usize, 0usize, 0usize);
    for (i, c) in cs.iter().enumerate() {
        let id = format!("c{i}");
        let o = &out[&id];
        let (size, pat) = c.grid();
        let src = terrain(size, pat);
        let pipeline: Option<Result<Vec<TerrainSampleChange>, String>> = match c {
            Case::P(_, _, br, p) => {
                Some(terrace_changes(size, &bytes_of(&src), br.0, br.1, br.2, p))
            }
            Case::Q(_, _, br, g, r, it) => Some(sharpen_ridge_changes(
                size,
                &bytes_of(&src),
                br.0,
                br.1,
                br.2,
                *g,
                *r,
                *it,
            )),
            _ => None,
        };
        if let Some(res) = pipeline {
            if o.starts_with("ERR") {
                assert!(res.is_err(), "{id}: C# threw {o} but Rust succeeded");
                errs += 1;
                continue;
            }
            let rust =
                res.unwrap_or_else(|e| panic!("{id}: Rust errored ({e}) but C# returned {o}"));
            let text = rust
                .iter()
                .map(|x| format!("{}:{}:{}", x.index, x.before, x.after))
                .collect::<Vec<_>>()
                .join(";");
            assert_eq!(&text, o, "{id} pipeline changes");
            pipeline_changes += rust.len();
            continue;
        }
        assert!(!o.starts_with("ERR"), "{id}: C# threw {o}");
        let mut w = buf(size, &src);
        match c {
            Case::A(_, _, br, p) => terrace(&mut w, br.0, br.1, br.2, p),
            Case::B(_, _, br, g, r, it) => sharpen_ridge(&mut w, br.0, br.1, br.2, *g, *r, *it),
            Case::C(_, _, br, d, st) => carve_valley(&mut w, br.0, br.1, br.2, *d, *st),
            _ => unreachable!(),
        }
        changed += (w.raw_data() != &src[..]) as usize;
        let expected = parse(o);
        assert_eq!(expected.len(), w.raw_data().len(), "{id} length");
        for (k, (&r, &e)) in w.raw_data().iter().zip(&expected).enumerate() {
            samples += 1;
            exact += (r.to_bits() == e.to_bits()) as usize;
            let d = (r - e).abs();
            assert!(
                d <= TOL || (r.is_nan() && e.is_nan()),
                "{id}[{k}]: rust={r} csharp={e}"
            );
            if d.is_finite() {
                max = max.max(d);
            }
        }
    }
    assert!(
        changed > 100 && pipeline_changes > 100 && errs >= 1,
        "matrix must exercise the filters"
    );
    println!(
        "C# / Rust sculpt differential: {} cases, {samples} samples ({exact} bit-exact, max diff {max:e}), {changed} changed grids, {pipeline_changes} pipeline changes, {errs} error cases",
        cs.len()
    );
}
