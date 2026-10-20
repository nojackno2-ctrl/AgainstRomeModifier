use against_rome_editor::terrain::*;

// The driver references the original Modules assembly (including the real session,
// pipeline, filter and work buffer), rather than translating their formulas to C#.
const SCULPT_ORACLE: &str = r#"
using AgainstRomeMapEditor;
using var input = new BinaryReader(File.OpenRead(args[0]));
using var output = new BinaryWriter(File.Create(args[1]));
int count = input.ReadInt32();
for (int test = 0; test < count; test++)
{
    int size = input.ReadInt32();
    byte[] heights = input.ReadBytes(size * size);
    bool up = input.ReadBoolean();
    var falloff = (TerrainFalloffType)input.ReadInt32();
    float cx = input.ReadSingle(), cy = input.ReadSingle(), radius = input.ReadSingle();
    float strength = input.ReadSingle(), limit = input.ReadSingle();
    int history = input.ReadInt32();
    var session = new TerrainHeightEditSession(size, heights, null, 0, null);
    if (history == 1)
    {
        session.ApplyHeightAdjustments(new[] { (heights.Length - 1, (byte)(heights[^1] ^ 1)) });
        session.Undo(); // A no-op must preserve this redo entry.
    }
    if (history == 2)
        session.ApplyHeightAdjustments(new[] { (0, (byte)(heights[0] ^ 1)) });
    // Two separate pipeline calls must quantize separately and share one stroke.
    for (int repeat = 0; repeat < 2; repeat++)
    {
        var changes = up
            ? TerrainSculptPipeline.Elevate(session, cx, cy, radius, strength, falloff, limit)
            : TerrainSculptPipeline.Depress(session, cx, cy, radius, strength, falloff, limit);
        output.Write(changes.Count);
        foreach (var c in changes)
        {
            output.Write(c.Index); output.Write(c.Before); output.Write(c.After);
        }
        Snapshot(session);
    }
    output.Write(session.CommitStroke());
    output.Write(session.Undo() is not null); Snapshot(session);
    output.Write(session.Redo() is not null); Snapshot(session);
    session.ResetToBaseline(); Snapshot(session);
}
void Snapshot(TerrainHeightEditSession s)
{
    output.Write(s.Heights.ToArray());
    output.Write(s.IsDirty); output.Write(s.CanUndo); output.Write(s.CanRedo);
}
"#;

#[test]
#[ignore = "executes original C# sculpt pipeline; requires .NET 8 SDK/runtime"]
fn live_csharp_sculpt_pipeline_differential() {
    use std::{fs, process::Command};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = std::env::temp_dir().join(format!(
        "terrain-sculpt-oracle-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&directory).unwrap();
    let module = root
        .join("src.MapEditor.Modules/AgainstRome.MapEditor.Modules.csproj")
        .canonicalize()
        .unwrap();
    // AssemblyName is an existing InternalsVisibleTo friend of Modules.
    fs::write(directory.join("Oracle.csproj"), format!(r#"<Project Sdk="Microsoft.NET.Sdk">
<PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net8.0</TargetFramework>
<ImplicitUsings>enable</ImplicitUsings><AssemblyName>AlrScanlineDifferential</AssemblyName></PropertyGroup>
<ItemGroup><ProjectReference Include="{}" /></ItemGroup></Project>"#,
        module.display().to_string().trim_start_matches(r"\\?\").replace('&', "&amp;").replace('"', "&quot;")
    )).unwrap();
    fs::write(directory.join("Program.cs"), SCULPT_ORACLE).unwrap();

    let mut cases = vec![];
    for up in [false, true] {
        for falloff in 0..6 {
            let normal = [
                [2.0, 2.0, 2.0, 8.0, if up { 255.0 } else { 0.0 }],
                [2.0, 2.0, 1.0, 1.0, if up { 90.5 } else { 110.5 }],
                [0.0, 0.0, 0.25, 0.5, 100.5],
                [0.0, 0.0, 0.25, 1.5, 101.5],
                [0.0, 0.0, 0.25, 80.5, if up { 999.0 } else { -999.0 }],
                [-0.5, 0.0, 1.0, 8.0, 100.0],
                [8.0, 8.0, 1.0, 8.0, 100.0],
                [2.25, 1.75, 2.8, 23.75, 127.5],
                [2.0, 2.0, 0.0, 8.0, 100.0],
                [2.0, 2.0, -1.0, 8.0, 100.0],
                [2.0, 2.0, 1.0, 0.0, 100.0],
                [2.0, 2.0, 1.0, -1.0, 100.0],
                [2.0, 2.0, 1.0, f32::MAX, f32::MAX],
                [2.0, 2.0, 1.0, f32::MAX, -f32::MAX],
                [2.0, 2.0, f32::MAX, 8.0, 100.0],
                [f32::MAX, 2.0, f32::MAX, 8.0, 100.0],
                [2.0, 2.0, f32::MIN_POSITIVE, 8.0, 100.0],
            ];
            for p in normal {
                cases.push((5, up, falloff, p));
            }
            for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                for field in 0..5 {
                    for (radius, strength) in [(1.0, 8.0), (0.0, 8.0), (1.0, 0.0)] {
                        let mut p = [2.0, 2.0, radius, strength, 100.0];
                        p[field] = bad;
                        cases.push((5, up, falloff, p));
                    }
                }
            }
            // Reproducible fractional positions/radii exercise every falloff across
            // byte thresholds, not just a single half-radius sample.
            let mut seed = 0x1234_5678u32;
            for _ in 0..100 {
                let mut next = || {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    (seed >> 8) as f32 / 16777216.0
                };
                cases.push((
                    5,
                    up,
                    falloff,
                    [
                        next() * 8.0 - 2.0,
                        next() * 8.0 - 2.0,
                        next() * 6.0 + 0.01,
                        next() * 300.0,
                        next() * 400.0 - 70.0,
                    ],
                ));
            }
            for size in [193, 257] {
                cases.push((
                    size,
                    up,
                    falloff,
                    [
                        size as f32 - 2.5,
                        size as f32 - 1.75,
                        8.75,
                        17.5,
                        if up { 241.5 } else { 12.5 },
                    ],
                ));
            }
        }
    }
    fn i32_bytes(bytes: &mut Vec<u8>, value: i32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    fn snapshot(bytes: &mut Vec<u8>, session: &TerrainSession) {
        bytes.extend_from_slice(session.heights());
        bytes.extend([
            session.is_dirty() as u8,
            session.can_undo() as u8,
            session.can_redo() as u8,
        ]);
    }
    let mut input = vec![];
    i32_bytes(&mut input, cases.len() as i32);
    for (id, &(size, up, falloff, p)) in cases.iter().enumerate() {
        i32_bytes(&mut input, size);
        input.extend((0..size * size).map(|i| {
            if id % 2 == 0 {
                100
            } else {
                (i * 37 % 256) as u8
            }
        }));
        input.push(up as u8);
        i32_bytes(&mut input, falloff);
        for f in p {
            input.extend_from_slice(&f.to_le_bytes());
        }
        i32_bytes(&mut input, (id % 3) as i32);
    }
    fs::write(directory.join("input.bin"), input).unwrap();
    let result = Command::new("dotnet")
        .args(["run", "--project"])
        .arg(directory.join("Oracle.csproj"))
        .args(["-c", "Release", "--"])
        .arg(directory.join("input.bin"))
        .arg(directory.join("output.bin"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "oracle failed: {}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let expected = fs::read(directory.join("output.bin")).unwrap();
    let mut offset = 0;
    for (id, &(size, up, falloff, p)) in cases.iter().enumerate() {
        let initial = (0..size * size)
            .map(|i| {
                if id % 2 == 0 {
                    100
                } else {
                    (i * 37 % 256) as u8
                }
            })
            .collect::<Vec<_>>();
        let mut session = TerrainSession::new(size as usize, initial.clone(), 0, None).unwrap();
        if id % 3 == 1 {
            let last = initial.len() - 1;
            session.apply_height_adjustments(&[(last, initial[last] ^ 1)]);
            session.undo();
        }
        if id % 3 == 2 {
            session.apply_height_adjustments(&[(0, initial[0] ^ 1)]);
        }
        let falloff = [
            Falloff::Smoothstep,
            Falloff::Cosine,
            Falloff::Spherical,
            Falloff::Gaussian,
            Falloff::Linear,
            Falloff::Plateau,
        ][falloff as usize];
        let mut actual = vec![];
        for _ in 0..2 {
            let result = if up {
                session.elevate((p[0], p[1]), p[2], p[3], falloff, p[4])
            } else {
                session.depress((p[0], p[1]), p[2], p[3], falloff, p[4])
            };
            let changes = result.unwrap_or_else(|e| panic!("case {id}, up={up}, {falloff:?}, {p:?}: C# returned changes, Rust returned {e}"));
            i32_bytes(&mut actual, changes.len() as i32);
            for c in changes {
                i32_bytes(&mut actual, c.index as i32);
                actual.extend([c.before, c.after]);
            }
            snapshot(&mut actual, &session);
        }
        actual.push(session.commit_stroke() as u8);
        actual.push(session.undo().is_some() as u8);
        snapshot(&mut actual, &session);
        actual.push(session.redo().is_some() as u8);
        snapshot(&mut actual, &session);
        session.reset_to_baseline();
        snapshot(&mut actual, &session);
        assert_eq!(
            Some(actual.as_slice()),
            expected.get(offset..offset + actual.len()),
            "case {id}, size={size}, up={up}, {falloff:?}, {p:?}"
        );
        offset += actual.len();
    }
    assert_eq!(offset, expected.len());
    println!(
        "C# sculpt differential: {} cases, {} exact change/history/snapshot bytes",
        cases.len(),
        offset
    );
    let resolved = directory.canonicalize().unwrap();
    assert!(resolved.starts_with(std::env::temp_dir().canonicalize().unwrap()));
    assert_eq!(resolved.file_name(), directory.file_name());
    fs::remove_dir_all(resolved).unwrap();
}

#[test]
fn ramp_linear_center_and_cap_geometry() {
    let mut p = RampParameters {
        width: 2.0,
        falloff: 0.0,
        start_height: Some(20.0),
        end_height: Some(100.0),
        ..Default::default()
    };
    let source = vec![0; 81];
    let round = calculate_ramp(&source, 9, (2.0, 4.0), (6.0, 4.0), &p).unwrap();
    assert_eq!(
        round.iter().find(|c| c.index == 4 * 9 + 4).unwrap().after,
        60
    );
    assert_eq!(
        round.iter().find(|c| c.index == 4 * 9 + 1).unwrap().after,
        20
    );
    p.round_caps = false;
    let flat = calculate_ramp(&source, 9, (2.0, 4.0), (6.0, 4.0), &p).unwrap();
    assert!(!flat.iter().any(|c| c.index == 4 * 9 + 1));
    assert!(source.iter().all(|v| *v == 0));
}
#[test]
fn retail_193_vertex_grid_and_bilinear_endpoints() {
    let mut source = vec![0; 193 * 193];
    source[192 * 193 + 192] = 200;
    assert_eq!(sample_bilinear(&source, 193, 191.5, 191.5).unwrap(), 50.0);
    assert_eq!(sample_bilinear(&source, 193, 999.0, 999.0).unwrap(), 200.0);
    assert_eq!(tile_to_vertex(47, 47, 48, 193), (190.0, 190.0));
    let c = calculate_ramp(
        &source,
        193,
        (191.5, 191.5),
        (192.0, 192.0),
        &Default::default(),
    )
    .unwrap();
    assert!(!c.is_empty());
    assert!(c.iter().all(|v| v.index < 193 * 193));
}
#[test]
fn midpoint_rounding_matches_dotnet_ties_to_even() {
    let p = RampParameters {
        width: 1.0,
        falloff: 0.0,
        start_height: Some(10.0),
        end_height: Some(11.0),
        ..Default::default()
    };
    let c = calculate_ramp(&[0; 9], 3, (0.0, 1.0), (2.0, 1.0), &p).unwrap();
    assert_eq!(c.iter().find(|c| c.index == 4).unwrap().after, 10);
}
#[test]
fn shared_stroke_cancel_undo_redo_and_saved_baseline() {
    let mut s = TerrainSession::new(5, vec![100; 25], 4, Some(vec![0; 16])).unwrap();
    s.paint_height((2.0, 2.0), 1.0, HeightOperation::Raise, 10, None, 0)
        .unwrap();
    s.paint_collision((1.5, 1.5), 0.6, CollisionOperation::Block)
        .unwrap();
    assert!(s.commit_stroke());
    assert!(s.is_dirty());
    let inverse = s.undo().unwrap();
    assert_eq!(inverse.heights[0].after, 100);
    assert!(!s.is_dirty());
    assert_eq!(s.collision().unwrap()[5], 0);
    s.redo().unwrap();
    assert_eq!(s.collision().unwrap()[5], 255);
    s.commit_baseline();
    assert!(!s.is_dirty());
    assert!(!s.can_undo());
    s.paint_height((2.0, 2.0), 1.0, HeightOperation::Lower, 5, None, 0)
        .unwrap();
    s.cancel_stroke();
    assert!(!s.is_dirty());
}
#[test]
fn reverting_pending_change_creates_no_history() {
    let mut s = TerrainSession::new(2, vec![10; 4], 0, None).unwrap();
    s.apply_height_changes(&[SampleChange {
        index: 0,
        before: 99,
        after: 20,
    }]);
    s.apply_height_changes(&[SampleChange {
        index: 0,
        before: 99,
        after: 10,
    }]);
    assert!(!s.commit_stroke());
    assert!(!s.can_undo());
    assert!(!s.is_dirty());
}
#[test]
fn water_preserves_authored_collision_and_undoes_together() {
    let mut c = vec![0; 16];
    c[5] = 90;
    let mut s = TerrainSession::new(5, vec![100; 25], 4, Some(c)).unwrap();
    let stroke = s.paint_water((2.0, 2.0), 2.0, 64, 10, 99.0).unwrap();
    assert!(!stroke.heights.is_empty());
    assert!(!stroke.collision.is_empty());
    assert_eq!(s.collision().unwrap()[5], 90);
    s.undo().unwrap();
    assert!(!s.is_dirty());
    assert!(s.heights().iter().all(|v| *v == 100));
}
#[test]
fn smoothing_uses_snapshot_and_roughening_is_seeded() {
    let mut h = vec![0; 25];
    h[12] = 90;
    let mut s = TerrainSession::new(5, h, 0, None).unwrap();
    s.paint_height((2.0, 2.0), 1.0, HeightOperation::Smooth, 8, None, 0)
        .unwrap();
    assert_eq!(s.heights()[12], 10);
    assert_eq!(roughness_noise(11, 29, -7), roughness_noise(11, 29, -7));
    assert_ne!(roughness_noise(11, 29, -7), roughness_noise(11, 29, 7));
    for y in 0..20 {
        for x in 0..20 {
            assert!((-1.0..=1.0).contains(&roughness_noise(x, y, i32::MAX)));
        }
    }
}
#[test]
fn invalid_input_is_rejected_without_mutation() {
    assert!(TerrainSession::new(usize::MAX, vec![], 0, None).is_err());
    let mut s = TerrainSession::new(2, vec![10; 4], 0, None).unwrap();
    assert!(s
        .paint_height((f32::NAN, 0.0), 1.0, HeightOperation::Raise, 1, None, 0)
        .is_err());
    assert!(s
        .apply_ramp(
            (0.0, 0.0),
            (1.0, 1.0),
            &RampParameters {
                width: f32::INFINITY,
                ..Default::default()
            }
        )
        .is_err());
    assert!(!s.is_dirty());
    assert!(
        calculate_ramp(&[0; 4], 2, (0.0, 0.0), (0.0, 0.0), &Default::default())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn transform_and_adjustments_track_one_undoable_stroke() {
    let mut s = TerrainSession::new(3, vec![10; 9], 0, None).unwrap();
    let c = s.transform_heights(|x, y, h| match (x, y) {
        (0, 0) => 2.5,
        (1, 0) => 3.5,
        (2, 0) => 999.0,
        (0, 1) => f32::NAN,
        (1, 1) => -4.0,
        _ => h as f32,
    });
    let after: Vec<u8> = c.iter().map(|c| c.after).collect();
    assert_eq!(after, vec![2, 4, 255, 0]);
    assert!(c.iter().all(|c| c.before == 10));
    let adj = s.apply_height_adjustments(&[(8, 50), (99, 1), (7, 10)]);
    assert_eq!(adj.len(), 1);
    assert_eq!(adj[0].before, 10);
    assert!(s.commit_stroke());
    s.undo().unwrap();
    assert_eq!(s.heights(), &[10; 9]);
}

// Contract from TerrainSculptFilter + TerrainHeightWorkBuffer.ExtractChanges:
// at distance/radius = 1/2 and strength 8, round the final height ties-to-even.
#[test]
fn sculpt_all_falloffs_match_reference_half_radius_samples() {
    for (falloff, delta) in [
        (Falloff::Smoothstep, 4),
        (Falloff::Cosine, 4),
        (Falloff::Spherical, 7),
        (Falloff::Gaussian, 3),
        (Falloff::Linear, 4),
        (Falloff::Plateau, 7),
    ] {
        for up in [true, false] {
            let mut s = TerrainSession::new(5, vec![100; 25], 0, None).unwrap();
            let changes = if up {
                s.elevate((2.0, 2.0), 2.0, 8.0, falloff, 255.0)
            } else {
                s.depress((2.0, 2.0), 2.0, 8.0, falloff, 0.0)
            }
            .unwrap();
            let expected = if up { 100 + delta } else { 100 - delta };
            for index in [7, 11, 13, 17] {
                assert_eq!(s.heights()[index], expected, "{falloff:?}, up={up}");
            }
            assert_eq!(s.heights()[12], if up { 108 } else { 92 });
            for index in [0, 2, 10, 14, 22, 24] {
                assert_eq!(s.heights()[index], 100);
            }
            assert!(changes.windows(2).all(|p| p[0].index < p[1].index));
            assert!(changes.iter().all(|c| c.before == 100 && c.after != 100));
            // No auto-commit: cancellation must restore the entire call.
            s.cancel_stroke();
            assert_eq!(s.heights(), &[100; 25]);
            assert!(!s.can_undo());
        }
    }
}

#[test]
fn sculpt_float_strength_rounding_and_bounds_precede_quantization() {
    for (initial, strength, expected_up, expected_down) in
        [(10, 0.5, 10, 10), (11, 0.5, 12, 10), (100, 80.5, 180, 20)]
    {
        let mut s = TerrainSession::new(2, vec![initial; 4], 0, None).unwrap();
        s.elevate((0.0, 0.0), 0.25, strength, Falloff::Linear, 255.0)
            .unwrap();
        assert_eq!(s.heights(), &[expected_up, initial, initial, initial]);
        s.reset_to_baseline();
        s.depress((0.0, 0.0), 0.25, strength, Falloff::Linear, 0.0)
            .unwrap();
        assert_eq!(s.heights(), &[expected_down, initial, initial, initial]);
    }
    let mut s = TerrainSession::new(3, vec![100; 9], 0, None).unwrap();
    // A cap below the existing height changes even zero-weight circle endpoints.
    let c = s
        .elevate((1.0, 1.0), 1.0, 1.0, Falloff::Linear, 90.5)
        .unwrap();
    assert_eq!(
        c.iter().map(|c| c.index).collect::<Vec<_>>(),
        vec![1, 3, 4, 5, 7]
    );
    assert!(c.iter().all(|c| c.after == 90));
    s.reset_to_baseline();
    let c = s
        .depress((1.0, 1.0), 1.0, 1.0, Falloff::Linear, 110.5)
        .unwrap();
    assert!(c.iter().all(|c| c.after == 110));
    assert_eq!(c.len(), 5);
    s.reset_to_baseline();
    s.elevate((1.0, 1.0), 0.25, 500.0, Falloff::Linear, 999.0)
        .unwrap();
    assert_eq!(s.heights()[4], 255);
    s.depress((1.0, 1.0), 0.25, 500.0, Falloff::Linear, -999.0)
        .unwrap();
    assert_eq!(s.heights()[4], 0);
}

#[test]
fn sculpt_clips_off_map_circle_without_enabling_global_mode() {
    let mut s = TerrainSession::new(3, vec![100; 9], 0, None).unwrap();
    let c = s
        .elevate((-0.5, 0.0), 1.0, 8.0, Falloff::Linear, 255.0)
        .unwrap();
    assert_eq!(
        c,
        vec![SampleChange {
            index: 0,
            before: 100,
            after: 104
        }]
    );
    assert!(s
        .elevate((-10.0, -10.0), 1.0, 8.0, Falloff::Linear, 255.0)
        .unwrap()
        .is_empty());
}

#[test]
fn sculpt_shares_pending_history_and_reports_actual_before_values() {
    let mut s = TerrainSession::new(3, vec![100; 9], 2, Some(vec![0; 4])).unwrap();
    s.apply_height_adjustments(&[(4, 110)]);
    let c = s
        .elevate((1.0, 1.0), 0.25, 2.5, Falloff::Smoothstep, 255.0)
        .unwrap();
    assert_eq!(
        c,
        vec![SampleChange {
            index: 4,
            before: 110,
            after: 112
        }]
    );
    s.paint_collision((0.5, 0.5), 0.25, CollisionOperation::Block)
        .unwrap();
    let inverse = s.undo().unwrap(); // Commits the combined pending stroke.
    assert_eq!(
        inverse.heights,
        vec![SampleChange {
            index: 4,
            before: 112,
            after: 100
        }]
    );
    assert_eq!(
        inverse.collision,
        vec![SampleChange {
            index: 0,
            before: 255,
            after: 0
        }]
    );
    assert!(!s.is_dirty());
    assert!(!s.can_undo());
    assert!(s.can_redo());
    // Quantized no-op and finite non-positive inputs preserve redo.
    assert!(s
        .elevate((1.0, 1.0), 1.0, 0.1, Falloff::Linear, 255.0)
        .unwrap()
        .is_empty());
    for (radius, strength) in [(0.0, 8.0), (-1.0, 8.0), (1.0, 0.0), (1.0, -1.0)] {
        assert!(s
            .depress((1.0, 1.0), radius, strength, Falloff::Linear, 0.0)
            .unwrap()
            .is_empty());
    }
    assert!(s.can_redo());
    let redone = s.redo().unwrap();
    assert_eq!(redone.heights[0].after, 112);
    assert_eq!(s.collision().unwrap()[0], 255);
    s.undo().unwrap();
    s.depress((1.0, 1.0), 0.25, 2.0, Falloff::Linear, 0.0)
        .unwrap();
    assert!(!s.can_redo());
    s.elevate((1.0, 1.0), 0.25, 2.0, Falloff::Linear, 255.0)
        .unwrap();
    assert!(!s.commit_stroke()); // Net-zero pending sculpt creates no history.
    assert!(!s.is_dirty());
}

#[test]
fn sculpt_nonfinite_geometry_is_csharp_noop_preserving_redo() {
    let mut s = TerrainSession::new(2, vec![100; 4], 0, None).unwrap();
    s.apply_height_adjustments(&[(0, 110)]);
    s.undo().unwrap();
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for field in 0..3 {
            let mut p = [0.0, 0.0, 1.0, 8.0, 100.0];
            p[field] = bad;
            assert!(s
                .elevate((p[0], p[1]), p[2], p[3], Falloff::Linear, p[4])
                .unwrap()
                .is_empty());
            assert!(s
                .depress((p[0], p[1]), p[2], p[3], Falloff::Linear, p[4])
                .unwrap()
                .is_empty());
            assert_eq!(s.heights(), &[100; 4]);
            assert!(s.can_redo());
            assert!(!s.can_undo());
        }
    }
    s.redo().unwrap();
    assert_eq!(s.heights()[0], 110);
}

// Frozen expectations confirmed by live_csharp_sculpt_pipeline_differential.
#[test]
fn sculpt_nan_infinity_and_int_overflow_follow_dotnet8_x64() {
    for up in [false, true] {
        for (strength, limit, center, edge) in [
            (f32::NAN, 100.0, 0, 0),
            (8.0, f32::NAN, 0, 0),
            (
                f32::INFINITY,
                if up { 255.0 } else { 0.0 },
                if up { 255 } else { 0 },
                0,
            ),
            (
                8.0,
                if up { f32::NEG_INFINITY } else { f32::INFINITY },
                0,
                0,
            ),
            (f32::MAX, f32::MAX, 0, if up { 100 } else { 0 }),
        ] {
            let mut s = TerrainSession::new(3, vec![100; 9], 0, None).unwrap();
            let changes = if up {
                s.elevate((1.0, 1.0), 1.0, strength, Falloff::Linear, limit)
            } else {
                s.depress((1.0, 1.0), 1.0, strength, Falloff::Linear, limit)
            }
            .unwrap();
            assert_eq!(
                s.heights(),
                &[100, edge, 100, edge, center, edge, 100, edge, 100],
                "up={up}, strength={strength}, limit={limit}"
            );
            assert!(!changes.is_empty());
            s.undo().unwrap();
            assert_eq!(s.heights(), &[100; 9]);
            assert!(s.can_redo());
        }
    }
    // Finite radius whose upper bound overflows int is a no-op in GetBrushBounds,
    // even though a distance-only scan would include every vertex.
    let mut s = TerrainSession::new(3, vec![100; 9], 0, None).unwrap();
    assert!(s
        .elevate((1.0, 1.0), f32::MAX, 8.0, Falloff::Linear, 255.0)
        .unwrap()
        .is_empty());
    // Non-positive strength short-circuits before a NaN limit is used.
    assert!(s
        .depress(
            (1.0, 1.0),
            1.0,
            f32::NEG_INFINITY,
            Falloff::Linear,
            f32::NAN
        )
        .unwrap()
        .is_empty());
    assert!(!s.can_undo());
}

#[test]
fn sculpt_rounds_final_height_each_call_instead_of_delta_or_stroke() {
    for (up, initial, expected) in [
        (true, 10, 10),
        (true, 11, 12),
        (false, 10, 10),
        (false, 11, 10),
    ] {
        let mut s = TerrainSession::new(2, vec![initial; 4], 0, None).unwrap();
        for _ in 0..2 {
            if up {
                s.elevate((0.0, 0.0), 0.25, 0.5, Falloff::Linear, 255.0)
            } else {
                s.depress((0.0, 0.0), 0.25, 0.5, Falloff::Linear, 0.0)
            }
            .unwrap();
            assert_eq!(s.heights()[0], expected);
        }
    }
}
