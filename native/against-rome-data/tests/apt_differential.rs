//! Differential oracle for `AptDocument`, `NativeAptAttachmentGeometry`, and `NativeAptLightPoints`
//! against the true C# implementation linked into `tests/oracle_apt/AptOracle.csproj`.
//!
//! Direct parity verification across:
//!   - Synthetic APT v2 and v3 valid containers
//!   - Raw and compressed rows
//!   - Transparent color 0 and zero-opaque bit flags
//!   - Gap runs (4-pixel units) and diamond center offsets
//!   - Diamond tile and frame compositions (including layered overlaps)
//!   - Offsets, canvas dimensions, and projection origins/anchors
//!   - Multi-axis attachment groups, stage/variant fallback (v2 replicated vs v3 per-variant)
//!   - Smoke points, markers (present vs -1000000 none sentinel), and decoy global extras
//!   - Geometry: Project, WorldGrid, WorldGridOrigin, Place, ExtractLightPoints, GetBuildingWorldLightPositions
//!   - Malformed counts, invalid table refs, corrupted PDAT headers, and prefix truncations

use against_rome_data::apt::*;
use against_rome_data::apt_geometry::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let mut path = std::env::temp_dir();
        let unique = format!(
            "{}_{}_{}",
            prefix,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        path.push(unique);
        fs::create_dir_all(&path).expect("Failed to create temp dir");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn row_width(y: usize) -> usize {
    if y <= 15 {
        (y + 1) * 4
    } else {
        (31 - y) * 4
    }
}

static ORACLE_DLL: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

fn ensure_oracle_built() -> &'static Path {
    ORACLE_DLL.get_or_init(|| {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let project = manifest_dir.join("tests/oracle_apt/AptOracle.csproj");
        let output = Command::new("dotnet")
            .args(["build", project.to_str().unwrap(), "-c", "Release"])
            .output()
            .expect("Failed to execute dotnet build on AptOracle");

        assert!(
            output.status.success(),
            "Failed to build AptOracle:\nSTDOUT:\n{}\nSTDERR:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        manifest_dir.join("tests/oracle_apt/bin/Release/net8.0/AptOracle.dll")
    })
}

fn run_oracle(oracle_dll: &Path, cases_dir: &Path) {
    let output = Command::new("dotnet")
        .arg(oracle_dll)
        .arg(cases_dir)
        .output()
        .expect("Failed to execute AptOracle");

    assert!(
        output.status.success(),
        "AptOracle execution failed:\nSTDOUT:\n{}\nSTDERR:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_oracle_geom(oracle_dll: &Path, inputs: &[f32]) -> Vec<Result<f32, String>> {
    let temp = TempDir::new("apt_oracle_geom");
    let in_file = temp.path.join("in.txt");
    let out_file = temp.path.join("out.txt");

    let mut in_str = String::new();
    for f in inputs {
        if f.is_nan() {
            in_str.push_str("NaN\n");
        } else if f.is_infinite() {
            if *f > 0.0 {
                in_str.push_str("Infinity\n");
            } else {
                in_str.push_str("-Infinity\n");
            }
        } else {
            in_str.push_str(&format!("{f}\n"));
        }
    }
    fs::write(&in_file, in_str).unwrap();

    let output = Command::new("dotnet")
        .arg(oracle_dll)
        .arg("--geom")
        .arg(&in_file)
        .arg(&out_file)
        .output()
        .expect("Failed to run oracle --geom");

    assert!(output.status.success());
    let out_content = fs::read_to_string(&out_file).unwrap();
    out_content
        .lines()
        .map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts[0] == "OK" {
                Ok(parts[1].parse::<f32>().unwrap())
            } else {
                Err(parts[1].to_string())
            }
        })
        .collect()
}

// Helper struct to define synthetic attachment groups
#[derive(Clone)]
struct SynthGroup {
    marker: AptPoint,
    light_variants: Vec<Vec<AptPoint>>,
    smoke: Vec<AptPoint>,
}

// Helper struct to define synthetic tile
#[derive(Clone)]
struct SynthTile {
    x: i32,
    y: i32,
    aux: u32,
    flags: u32,
    opaque: u32,
    // If compressed == false: raw pixels indexed by row_offsets
    // If compressed == true: rows table + payload
    tile_bytes: Vec<u8>,
}

#[derive(Clone)]
struct SynthAptBuilder {
    version: u32,
    canvas_w: u32,
    canvas_h: u32,
    anchor_x: i32,
    anchor_y: i32,
    proj_origin: AptPoint,
    layout: [u32; 4],
    palette_colors: u32,
    palette_variants: u32,
    palettes: Vec<u32>,
    groups: Vec<SynthGroup>,
    extra_a: u32,
    extra_b: u32,
    frames: Vec<(u32, u32)>, // (first, count)
    tiles: Vec<SynthTile>,
    row_offsets: [u32; 31],
    row_widths: [u32; 31],
    header_size: u32,
}

impl SynthAptBuilder {
    fn new(version: u32) -> Self {
        let mut row_offsets = [0u32; 31];
        let mut row_widths = [0u32; 31];
        let mut pos = 0u32;
        for y in 0..31 {
            let rw = row_width(y) as u32;
            row_offsets[y] = pos;
            row_widths[y] = rw;
            pos += rw;
        }

        Self {
            version,
            canvas_w: 64,
            canvas_h: 31,
            anchor_x: 30,
            anchor_y: 15,
            proj_origin: AptPoint::new(0, 0),
            layout: [1, 1, 1, 1],
            palette_colors: 4,
            palette_variants: 1,
            palettes: vec![0x00112233, 0x00445566, 0x00778899, 0x00AABBCC],
            groups: vec![SynthGroup {
                marker: AptPoint::new(-1000000, 0),
                light_variants: vec![vec![]],
                smoke: vec![],
            }],
            extra_a: 0,
            extra_b: 0,
            frames: vec![(0, 1)],
            tiles: vec![],
            row_offsets,
            row_widths,
            header_size: 112,
        }
    }

    fn build(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        let frame_count = self.frames.len() as u32;
        let tile_count = self.tiles.len() as u32;
        let group_variants = self.layout[1];

        // 1. Header (28 words = 112 bytes)
        let mut header = [0u32; 28];
        header[0] = 0x54415041;
        header[1] = self.version;
        header[2] = frame_count;
        header[3] = self.header_size;
        header[4] = tile_count;
        header[6] = 64;
        header[7] = 31;
        header[8] = self.layout[0];
        header[9] = self.layout[1];
        header[10] = self.layout[2];
        header[11] = self.layout[3];
        header[13] = 8;
        header[20] = self.palette_colors;
        header[21] = self.palette_variants;
        header[23] = self.canvas_w;
        header[24] = self.canvas_h;

        // Tile data offsets and PDAT calculation
        let mut tile_data_offsets = Vec::new();
        let mut pdat_body = Vec::new();
        for t in &self.tiles {
            tile_data_offsets.push(pdat_body.len() as u32);
            pdat_body.extend_from_slice(&t.flags.to_le_bytes());
            pdat_body.extend_from_slice(&t.opaque.to_le_bytes());
            pdat_body.extend_from_slice(&t.tile_bytes);
        }
        header[27] = pdat_body.len() as u32;

        for w in header {
            buf.extend_from_slice(&w.to_le_bytes());
        }

        // Header padding
        if self.header_size > 112 {
            buf.extend(vec![0u8; (self.header_size - 112) as usize]);
        }

        // Row tables
        for w in self.row_offsets {
            buf.extend_from_slice(&w.to_le_bytes());
        }
        for w in self.row_widths {
            buf.extend_from_slice(&w.to_le_bytes());
        }

        // Proj Origin & Anchor & Extras & Groups count
        buf.extend_from_slice(&self.proj_origin.x.to_le_bytes());
        buf.extend_from_slice(&self.proj_origin.y.to_le_bytes());
        buf.extend_from_slice(&self.anchor_x.to_le_bytes());
        buf.extend_from_slice(&self.anchor_y.to_le_bytes());
        buf.extend_from_slice(&self.extra_a.to_le_bytes());
        buf.extend_from_slice(&self.extra_b.to_le_bytes());
        buf.extend_from_slice(&(self.groups.len() as u32).to_le_bytes());

        // Groups header
        for g in &self.groups {
            buf.extend_from_slice(&g.marker.x.to_le_bytes());
            buf.extend_from_slice(&g.marker.y.to_le_bytes());
            buf.extend_from_slice(&1u32.to_le_bytes()); // first
            buf.extend_from_slice(&1u32.to_le_bytes()); // second
            let v_count = if self.version == 2 {
                1
            } else {
                group_variants as usize
            };
            for v in 0..v_count {
                let cnt = if v < g.light_variants.len() {
                    g.light_variants[v].len() as u32
                } else {
                    0
                };
                buf.extend_from_slice(&cnt.to_le_bytes());
            }
            buf.extend_from_slice(&(g.smoke.len() as u32).to_le_bytes());
            buf.extend_from_slice(&1u32.to_le_bytes()); // tail
        }

        // Decoy extras: (extraA + extraB) * 8 bytes
        let decoy_bytes = ((self.extra_a + self.extra_b) as usize) * 8;
        buf.extend(vec![0x77u8; decoy_bytes]);

        // Groups payload
        for g in &self.groups {
            buf.extend(vec![0u8; 1 * 12 + 1 * 8]); // skip1
            let v_count = if self.version == 2 {
                1
            } else {
                group_variants as usize
            };
            for v in 0..v_count {
                if v < g.light_variants.len() {
                    for pt in &g.light_variants[v] {
                        buf.extend_from_slice(&pt.x.to_le_bytes());
                        buf.extend_from_slice(&pt.y.to_le_bytes());
                    }
                }
            }
            for pt in &g.smoke {
                buf.extend_from_slice(&pt.x.to_le_bytes());
                buf.extend_from_slice(&pt.y.to_le_bytes());
            }
            buf.extend(vec![0u8; 1 * 16]); // skip2
        }

        // Frames table
        for (first, count) in &self.frames {
            buf.extend_from_slice(&first.to_le_bytes());
            buf.extend_from_slice(&count.to_le_bytes());
        }

        // Palettes
        for c in &self.palettes {
            buf.extend_from_slice(&c.to_le_bytes());
        }

        // Tiles table
        for (i, t) in self.tiles.iter().enumerate() {
            let offset = tile_data_offsets[i];
            let packed = ((t.x as u32) & 0x7FF) | (((t.y as u32) & 0x7FF) << 11);
            buf.extend_from_slice(&offset.to_le_bytes());
            buf.extend_from_slice(&packed.to_le_bytes());
            buf.extend_from_slice(&t.aux.to_le_bytes());
        }

        // PDAT Magic & Body
        buf.extend_from_slice(&0x54414450u32.to_le_bytes());
        buf.extend_from_slice(&pdat_body);

        buf
    }
}

fn make_raw_tile_pixels(color_pattern: &[u8]) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(1024);
    let mut i = 0;
    for y in 0..31 {
        let rw = row_width(y);
        for _ in 0..rw {
            let c = color_pattern[i % color_pattern.len()];
            pixels.push(c);
            i += 1;
        }
    }
    pixels
}

fn make_compressed_tile(
    gap_pattern: &[u16],
    skip_rows: u32,
    opaque_rows: u32,
    colors: &[u8],
) -> (u32, u32, Vec<u8>) {
    let mut table = Vec::with_capacity(64);
    let mut payload = Vec::new();
    let mut start: u16 = 64; // row table occupies 64 bytes (8..72)

    for y in 0..31 {
        let gap = gap_pattern[y % gap_pattern.len()] & 0x3C;
        let word = start | (gap << 9);
        table.extend_from_slice(&word.to_le_bytes());

        let max_w = row_width(y) as u16;
        let eff_w = if max_w > gap { max_w - gap } else { 0 };
        for _ in 0..eff_w {
            payload.push(colors[payload.len() % colors.len()]);
        }
        start += eff_w;
    }
    // 32nd entry
    table.extend_from_slice(&start.to_le_bytes());

    let mut body = table;
    body.extend_from_slice(&payload);

    let flags = 0x8000_0000 | (skip_rows & 0x7FFF_FFFF);
    (flags, opaque_rows, body)
}

#[test]
fn test_pure_geometry_parity() {
    let oracle_dll = ensure_oracle_built();

    let inputs: Vec<f32> = vec![
        0.0,
        63.9,
        64.0,
        -0.5,
        -1.0,
        -64.5,
        100.25,
        -200.75,
        1000.0,
        -1000.0,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        1e20,
        -1e20,
    ];

    let oracle_results = run_oracle_geom(&oracle_dll, &inputs);

    for (i, val) in inputs.iter().enumerate() {
        let rust_res = world_grid_origin(*val);
        let csharp_res = &oracle_results[i];

        match (rust_res, csharp_res) {
            (Ok(rf), Ok(cf)) => {
                assert_eq!(
                    rf, *cf,
                    "WorldGridOrigin({val}): Rust {rf} != C# {cf}"
                );
            }
            (Err(_), Err(_)) => {
                // Both rejected correctly
            }
            (Ok(rf), Err(ce)) => {
                panic!("WorldGridOrigin({val}): Rust passed ({rf}) but C# rejected ({ce})");
            }
            (Err(re), Ok(cf)) => {
                panic!("WorldGridOrigin({val}): Rust rejected ({re}) but C# passed ({cf})");
            }
        }
    }
}

#[test]
fn csharp_apt_differential() {
    let oracle_dll = ensure_oracle_built();
    let temp_cases = TempDir::new("apt_diff_cases");

    let mut valid_cases = HashMap::new();
    let mut malformed_cases = HashMap::new();

    // 1. Synthetic Valid Case: v2 Raw
    {
        let mut b = SynthAptBuilder::new(2);
        b.canvas_w = 64;
        b.canvas_h = 31;
        b.anchor_x = 30;
        b.anchor_y = 15;
        b.proj_origin = AptPoint::new(3, 5);
        b.layout = [2, 2, 1, 1];
        b.palette_colors = 4;
        b.palette_variants = 2;
        b.palettes = vec![
            0x00112233, 0x00445566, 0x00778899, 0x00AABBCC, // var 0
            0x00FF0011, 0x0000FF22, 0x000033FF, 0x00FFFF44, // var 1
        ];
        b.groups = vec![
            SynthGroup {
                marker: AptPoint::new(12, 34),
                light_variants: vec![vec![AptPoint::new(5, 7)], vec![AptPoint::new(9, 11)]],
                smoke: vec![AptPoint::new(21, 23)],
            },
            SynthGroup {
                marker: AptPoint::new(-1000000, 0), // null marker
                light_variants: vec![vec![AptPoint::new(15, 17)]],
                smoke: vec![],
            },
        ];
        b.extra_a = 1;
        b.extra_b = 2;
        b.frames = vec![(0, 1), (0, 2)];
        let p1 = make_raw_tile_pixels(&[0, 1, 2, 3]);
        let p2 = make_raw_tile_pixels(&[1, 0, 3, 2]);
        b.tiles = vec![
            SynthTile {
                x: 30,
                y: 0,
                aux: 10,
                flags: 0,
                opaque: 0,
                tile_bytes: p1,
            },
            SynthTile {
                x: 30,
                y: 0,
                aux: 20,
                flags: 0,
                opaque: 0xFFFFFFFF, // opaque color 0
                tile_bytes: p2,
            },
        ];
        let bytes = b.build();
        valid_cases.insert("v2_raw".to_string(), bytes);
    }

    // 2. Synthetic Valid Case: v3 Compressed with gap runs and skipped rows
    {
        let mut b = SynthAptBuilder::new(3);
        b.canvas_w = 128;
        b.canvas_h = 64;
        b.anchor_x = 40;
        b.anchor_y = 20;
        b.proj_origin = AptPoint::new(10, 10);
        b.layout = [2, 3, 2, 1];
        b.palette_colors = 4;
        b.palette_variants = 2;
        b.palettes = vec![
            0x00AABBCC, 0x00112233, 0x00445566, 0x00778899,
            0x00CCBBAA, 0x00332211, 0x00665544, 0x00998877,
        ];
        b.groups = vec![
            SynthGroup {
                marker: AptPoint::new(50, 60),
                light_variants: vec![
                    vec![AptPoint::new(1, 2)],
                    vec![AptPoint::new(3, 4), AptPoint::new(5, 6)],
                    vec![AptPoint::new(7, 8)],
                ],
                smoke: vec![AptPoint::new(9, 10)],
            },
            SynthGroup {
                marker: AptPoint::new(-1000000, 0),
                light_variants: vec![vec![], vec![], vec![]],
                smoke: vec![],
            },
            SynthGroup {
                marker: AptPoint::new(100, 150),
                light_variants: vec![vec![AptPoint::new(11, 12)], vec![], vec![]],
                smoke: vec![AptPoint::new(13, 14)],
            },
            SynthGroup {
                marker: AptPoint::new(-1000000, -1000000),
                light_variants: vec![vec![], vec![], vec![]],
                smoke: vec![],
            },
        ];
        b.frames = vec![(0, 1), (1, 1), (0, 2)];
        let p_raw = make_raw_tile_pixels(&[1, 2, 3, 0]);
        let gaps = [0, 4, 8, 4, 12, 0, 4, 8];
        let (flags, opaque, comp_body) = make_compressed_tile(&gaps, 0b1010, 0b0101, &[1, 2, 3, 0]);
        b.tiles = vec![
            SynthTile {
                x: 30,
                y: 0,
                aux: 0,
                flags: 0,
                opaque: 0,
                tile_bytes: p_raw,
            },
            SynthTile {
                x: 40,
                y: 10,
                aux: 5,
                flags,
                opaque,
                tile_bytes: comp_body,
            },
        ];
        let bytes = b.build();
        valid_cases.insert("v3_compressed".to_string(), bytes);
    }

    // 3. Synthetic Valid Case: Diamond Composition & Overlap
    {
        let mut b = SynthAptBuilder::new(3);
        b.canvas_w = 100;
        b.canvas_h = 80;
        b.layout = [1, 1, 1, 1];
        b.palette_colors = 4;
        b.palette_variants = 1;
        b.palettes = vec![0x00000000, 0x00FF0000, 0x0000FF00, 0x000000FF];
        b.groups = vec![SynthGroup {
            marker: AptPoint::new(-1000000, 0),
            light_variants: vec![vec![]],
            smoke: vec![],
        }];
        b.frames = vec![(0, 3)]; // All 3 tiles in 1 frame
        let t1 = make_raw_tile_pixels(&[1]); // all color 1
        let t2 = make_raw_tile_pixels(&[0, 2]); // alternating 0 and 2
        let t3 = make_raw_tile_pixels(&[3]); // all color 3
        b.tiles = vec![
            SynthTile {
                x: 30,
                y: 0,
                aux: 0,
                flags: 0,
                opaque: 0,
                tile_bytes: t1,
            },
            SynthTile {
                x: 35,
                y: 5,
                aux: 0,
                flags: 0,
                opaque: 0,
                tile_bytes: t2,
            },
            SynthTile {
                x: 40,
                y: 10,
                aux: 0,
                flags: 0,
                opaque: 0,
                tile_bytes: t3,
            },
        ];
        valid_cases.insert("diamond_composition".to_string(), b.build());
    }

    // 4. Synthetic Valid Case: 21 Lights Capacity 20 Truncation
    {
        let mut b = SynthAptBuilder::new(3);
        let pts: Vec<AptPoint> = (0..21).map(|i| AptPoint::new(65 + i, i * 2)).collect();
        b.groups = vec![SynthGroup {
            marker: AptPoint::new(-1000000, 0),
            light_variants: vec![pts],
            smoke: vec![],
        }];
        b.tiles = vec![SynthTile {
            x: 30,
            y: 0,
            aux: 0,
            flags: 0,
            opaque: 0,
            tile_bytes: make_raw_tile_pixels(&[1]),
        }];
        valid_cases.insert("capacity_20_lights".to_string(), b.build());
    }

    // 5. Synthetic Valid Container with Runtime Error Tiles (Color out of range & Pixel out of bounds)
    {
        let mut b = SynthAptBuilder::new(3);
        b.canvas_w = 64;
        b.canvas_h = 31;
        b.palette_colors = 4;
        b.palette_variants = 1;
        b.palettes = vec![0x00112233, 0x00445566, 0x00778899, 0x00AABBCC];
        b.frames = vec![(0, 1), (1, 1), (2, 1)];
        let p_normal = make_raw_tile_pixels(&[1, 2, 3]);
        let p_bad_color = make_raw_tile_pixels(&[99]); // Color 99 >= PaletteColorCount (4)
        let p_out_bounds = make_raw_tile_pixels(&[1]);
        b.tiles = vec![
            SynthTile {
                x: 30,
                y: 0,
                aux: 0,
                flags: 0,
                opaque: 0,
                tile_bytes: p_normal,
            },
            SynthTile {
                x: 30,
                y: 0,
                aux: 0,
                flags: 0,
                opaque: 0,
                tile_bytes: p_bad_color,
            },
            SynthTile {
                x: 1000, // Large X causing pixel to be outside canvas width (64)
                y: 10,
                aux: 0,
                flags: 0,
                opaque: 0,
                tile_bytes: p_out_bounds,
            },
        ];
        valid_cases.insert("runtime_error_tiles".to_string(), b.build());
    }

    // 6. Malformed cases: Signature, Version, Canvas, Tables, Overlaps, etc.
    let base_valid = valid_cases["v2_raw"].clone();

    // Bad signature
    {
        let mut broken = base_valid.clone();
        broken[0..4].copy_from_slice(&0x12345678u32.to_le_bytes());
        malformed_cases.insert("bad_sig".to_string(), broken);
    }
    // Unsupported versions: 1, 4, 0
    {
        let mut b1 = base_valid.clone();
        b1[4..8].copy_from_slice(&1u32.to_le_bytes());
        malformed_cases.insert("unsupported_v1".to_string(), b1);

        let mut b4 = base_valid.clone();
        b4[4..8].copy_from_slice(&4u32.to_le_bytes());
        malformed_cases.insert("unsupported_v4".to_string(), b4);

        let mut b0 = base_valid.clone();
        b0[4..8].copy_from_slice(&0u32.to_le_bytes());
        malformed_cases.insert("unsupported_v0".to_string(), b0);
    }
    // Unsupported format: width != 64, height != 31, depth != 8
    {
        let mut bw = base_valid.clone();
        bw[24..28].copy_from_slice(&32u32.to_le_bytes());
        malformed_cases.insert("format_w32".to_string(), bw);

        let mut bh = base_valid.clone();
        bh[28..32].copy_from_slice(&15u32.to_le_bytes());
        malformed_cases.insert("format_h15".to_string(), bh);

        let mut bd = base_valid.clone();
        bd[52..56].copy_from_slice(&16u32.to_le_bytes());
        malformed_cases.insert("format_d16".to_string(), bd);
    }
    // Unsupported group variants: 0 or 4
    {
        let mut g0 = base_valid.clone();
        g0[36..40].copy_from_slice(&0u32.to_le_bytes());
        malformed_cases.insert("group_variants_0".to_string(), g0);

        let mut g4 = base_valid.clone();
        g4[36..40].copy_from_slice(&4u32.to_le_bytes());
        malformed_cases.insert("group_variants_4".to_string(), g4);
    }
    // Invalid canvas dimensions: 0 or > 2048
    {
        let mut cw0 = base_valid.clone();
        cw0[92..96].copy_from_slice(&0u32.to_le_bytes());
        malformed_cases.insert("canvas_w0".to_string(), cw0);

        let mut cw_big = base_valid.clone();
        cw_big[92..96].copy_from_slice(&2049u32.to_le_bytes());
        malformed_cases.insert("canvas_w2049".to_string(), cw_big);
    }
    // Invalid palette dimensions: colors 0, 257; variants 0
    {
        let mut c0 = base_valid.clone();
        c0[80..84].copy_from_slice(&0u32.to_le_bytes());
        malformed_cases.insert("palette_c0".to_string(), c0);

        let mut c257 = base_valid.clone();
        c257[80..84].copy_from_slice(&257u32.to_le_bytes());
        malformed_cases.insert("palette_c257".to_string(), c257);

        let mut v0 = base_valid.clone();
        v0[84..88].copy_from_slice(&0u32.to_le_bytes());
        malformed_cases.insert("palette_v0".to_string(), v0);
    }
    // Header overlaps tables (< 112)
    {
        let mut hov = base_valid.clone();
        hov[12..16].copy_from_slice(&108u32.to_le_bytes());
        malformed_cases.insert("header_overlap".to_string(), hov);
    }
    // Row width exceeds diamond (> 64)
    {
        let mut rw = base_valid.clone();
        rw[236..240].copy_from_slice(&65u32.to_le_bytes());
        malformed_cases.insert("row_width_65".to_string(), rw);
    }
    // Missing PDAT block
    {
        let mut no_pdat = base_valid.clone();
        // Find 0x54414450 and corrupt it
        for i in 0..no_pdat.len() - 4 {
            if &no_pdat[i..i + 4] == 0x54414450u32.to_le_bytes() {
                no_pdat[i..i + 4].copy_from_slice(&0x11111111u32.to_le_bytes());
                break;
            }
        }
        malformed_cases.insert("missing_pdat".to_string(), no_pdat);
    }
    // Frame references out of range (first + count > tile_count)
    {
        let mut b = SynthAptBuilder::new(2);
        b.tiles = vec![SynthTile {
            x: 30,
            y: 0,
            aux: 0,
            flags: 0,
            opaque: 0,
            tile_bytes: make_raw_tile_pixels(&[1]),
        }];
        b.frames = vec![(0, 5)]; // first=0, count=5 > tile_count=1
        malformed_cases.insert("frame_ref_out_of_range".to_string(), b.build());
    }
    // Tile offset outside pixel block
    {
        let mut b = SynthAptBuilder::new(2);
        b.tiles = vec![SynthTile {
            x: 30,
            y: 0,
            aux: 0,
            flags: 0,
            opaque: 0,
            tile_bytes: vec![0u8; 10],
        }];
        let mut broken = b.build();
        // Find tiles table and set offset to huge
        // Tiles table is right before PDAT block.
        // In this case, tile table has 1 tile (12 bytes: offset, packed, aux).
        // PDAT block is 4 bytes magic (0x54414450) + data.
        // The 12 bytes immediately before PDAT magic are the tile info!
        for i in 0..broken.len() - 16 {
            if &broken[i + 12..i + 16] == 0x54414450u32.to_le_bytes() {
                broken[i..i + 4].copy_from_slice(&99999u32.to_le_bytes());
                break;
            }
        }
        malformed_cases.insert("tile_offset_outside".to_string(), broken);
    }

    // Write all input cases to temp_cases
    for (name, bytes) in &valid_cases {
        let p = temp_cases.path.join(format!("{name}.in"));
        fs::write(p, bytes).unwrap();
    }
    for (name, bytes) in &malformed_cases {
        let p = temp_cases.path.join(format!("{name}.in"));
        fs::write(p, bytes).unwrap();
    }

    // Prefix truncation cases
    let mut trunc_cases = Vec::new();
    let trunc_offsets = [0, 1, 4, 10, 28, 50, 111, 112, 200, 300, base_valid.len() - 1];
    for &off in &trunc_offsets {
        if off < base_valid.len() {
            let name = format!("trunc_{off}");
            let p = temp_cases.path.join(format!("{name}.in"));
            fs::write(p, &base_valid[..off]).unwrap();
            trunc_cases.push(name);
        }
    }

    // Execute oracle on temp_cases
    run_oracle(&oracle_dll, &temp_cases.path);

    // Verify Valid Cases
    for (name, bytes) in &valid_cases {
        let status = fs::read_to_string(temp_cases.path.join(format!("{name}.status"))).unwrap();
        assert_eq!(status.trim(), "OK", "Case {name} failed in C# oracle: {status}");

        let rust_doc = AptDocument::parse(bytes).expect(&format!("Rust parse failed on {name}"));

        // Compare Meta
        let meta_content = fs::read_to_string(temp_cases.path.join(format!("{name}.meta"))).unwrap();
        let mut lines = meta_content.lines();
        let doc_line = lines.next().unwrap();
        let dp: Vec<&str> = doc_line.split_whitespace().collect();
        assert_eq!(dp[0], "DOC");
        assert_eq!(rust_doc.version, dp[1].parse::<u32>().unwrap());
        assert_eq!(rust_doc.width, dp[2].parse::<usize>().unwrap());
        assert_eq!(rust_doc.height, dp[3].parse::<usize>().unwrap());
        assert_eq!(rust_doc.anchor_x, dp[4].parse::<i32>().unwrap());
        assert_eq!(rust_doc.anchor_y, dp[5].parse::<i32>().unwrap());
        assert_eq!(rust_doc.projection_origin.x, dp[6].parse::<i32>().unwrap());
        assert_eq!(rust_doc.projection_origin.y, dp[7].parse::<i32>().unwrap());
        assert_eq!(rust_doc.layout[0], dp[8].parse::<u32>().unwrap());
        assert_eq!(rust_doc.layout[1], dp[9].parse::<u32>().unwrap());
        assert_eq!(rust_doc.layout[2], dp[10].parse::<u32>().unwrap());
        assert_eq!(rust_doc.layout[3], dp[11].parse::<u32>().unwrap());
        assert_eq!(rust_doc.palette_color_count, dp[12].parse::<usize>().unwrap());
        assert_eq!(rust_doc.palette_variant_count, dp[13].parse::<usize>().unwrap());
        assert_eq!(rust_doc.frames.len(), dp[14].parse::<usize>().unwrap());
        assert_eq!(rust_doc.tiles.len(), dp[15].parse::<usize>().unwrap());
        assert_eq!(rust_doc.attachment_groups.len(), dp[16].parse::<usize>().unwrap());

        // Compare Attachment Queries
        let queries_content = fs::read_to_string(temp_cases.path.join(format!("{name}.queries"))).unwrap();
        for ql in queries_content.lines() {
            let qp: Vec<&str> = ql.split_whitespace().collect();
            let a0: u32 = qp[1].parse().unwrap();
            let a1: u32 = qp[2].parse().unwrap();
            let a2: u32 = qp[3].parse().unwrap();

            let rust_res = rust_doc.get_attachments(a0, a1, a2);
            if qp[4] == "OK" {
                let rust_att = rust_res.expect("Rust attachment query failed");
                let marker_str = if let Some(m) = rust_att.marker {
                    format!("Marker={},{}", m.x, m.y)
                } else {
                    "Marker=NONE".to_string()
                };
                assert_eq!(marker_str, qp[5], "Query ({a0},{a1},{a2}) marker mismatch");

                let lights_str = format!(
                    "Lights={}:{}",
                    rust_att.lights.len(),
                    rust_att
                        .lights
                        .iter()
                        .map(|p| format!("{},{}", p.x, p.y))
                        .collect::<Vec<_>>()
                        .join(";")
                );
                assert_eq!(lights_str, qp[6], "Query ({a0},{a1},{a2}) lights mismatch");

                let smoke_str = format!(
                    "Smoke={}:{}",
                    rust_att.smoke.len(),
                    rust_att
                        .smoke
                        .iter()
                        .map(|p| format!("{},{}", p.x, p.y))
                        .collect::<Vec<_>>()
                        .join(";")
                );
                assert_eq!(smoke_str, qp[7], "Query ({a0},{a1},{a2}) smoke mismatch");
            } else {
                assert!(rust_res.is_err(), "Rust should reject query ({a0},{a1},{a2})");
            }
        }

        // Compare Tiles Decoding
        for t in 0..rust_doc.tiles.len() {
            for v in 0..rust_doc.palette_variant_count {
                let bin_path = temp_cases.path.join(format!("{name}.tile_{t}_{v}.bin"));
                let err_path = temp_cases.path.join(format!("{name}.tile_{t}_{v}.err"));

                let rust_tile = rust_doc.decode_tile(t, v);
                if bin_path.exists() {
                    let rust_img = rust_tile.expect("Rust decode_tile failed");
                    let data = fs::read(&bin_path).unwrap();
                    let cs_w = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
                    let cs_h = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
                    assert_eq!(rust_img.width, cs_w);
                    assert_eq!(rust_img.height, cs_h);
                    let mut cs_pixels = Vec::with_capacity(cs_w * cs_h);
                    for chunk in data[8..].chunks_exact(4) {
                        cs_pixels.push(u32::from_le_bytes(chunk.try_into().unwrap()));
                    }
                    assert_eq!(rust_img.argb_pixels, cs_pixels, "Tile {t} var {v} pixel mismatch");
                } else if err_path.exists() {
                    assert!(rust_tile.is_err(), "Rust decode_tile should fail on {t} var {v}");
                }
            }
        }

        // Compare Frames Decoding
        for f in 0..rust_doc.frames.len() {
            for v in 0..rust_doc.palette_variant_count {
                let bin_path = temp_cases.path.join(format!("{name}.frame_{f}_{v}.bin"));
                let err_path = temp_cases.path.join(format!("{name}.frame_{f}_{v}.err"));

                let rust_frame = rust_doc.decode_frame(f, v);
                if bin_path.exists() {
                    let rust_img = rust_frame.expect("Rust decode_frame failed");
                    let data = fs::read(&bin_path).unwrap();
                    let cs_w = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
                    let cs_h = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
                    assert_eq!(rust_img.width, cs_w);
                    assert_eq!(rust_img.height, cs_h);
                    let mut cs_pixels = Vec::with_capacity(cs_w * cs_h);
                    for chunk in data[8..].chunks_exact(4) {
                        cs_pixels.push(u32::from_le_bytes(chunk.try_into().unwrap()));
                    }
                    assert_eq!(rust_img.argb_pixels, cs_pixels, "Frame {f} var {v} pixel mismatch");
                } else if err_path.exists() {
                    assert!(rust_frame.is_err(), "Rust decode_frame should fail on {f} var {v}");
                }
            }
        }

        // Compare Geometry
        let geom_content = fs::read_to_string(temp_cases.path.join(format!("{name}.geom"))).unwrap();
        for gl in geom_content.lines() {
            let gp: Vec<&str> = gl.split_whitespace().collect();
            if gp[0] == "PROJECT" {
                let x: i32 = gp[1].parse().unwrap();
                let y: i32 = gp[2].parse().unwrap();
                let proj = project(&rust_doc, AptPoint::new(x, y));
                assert_eq!(proj.x, gp[4].parse::<i32>().unwrap());
                assert_eq!(proj.y, gp[5].parse::<i32>().unwrap());
            } else if gp[0] == "PLACE" {
                let pt = AptPoint::new(gp[1].parse().unwrap(), gp[2].parse().unwrap());
                let px: f32 = gp[3].parse().unwrap();
                let pz: f32 = gp[4].parse().unwrap();
                let h: f32 = gp[5].parse().unwrap();
                let rust_place = place(&rust_doc, pt, px, pz, h);
                if gp[6] == "OK" {
                    let v = rust_place.expect("Rust place failed");
                    assert_eq!(v.x, gp[7].parse::<f32>().unwrap());
                    assert_eq!(v.y, gp[8].parse::<f32>().unwrap());
                    assert_eq!(v.z, gp[9].parse::<f32>().unwrap());
                } else {
                    assert!(rust_place.is_err());
                }
            } else if gp[0] == "EXTRACT_LIGHTS" {
                let rust_lights = extract_light_points(bytes);
                if gp[1] == "OK" {
                    let pts = rust_lights.expect("Rust extract_light_points failed");
                    let expected_cnt: usize = gp[2].parse().unwrap();
                    assert_eq!(pts.len(), expected_cnt);
                    if expected_cnt > 0 {
                        let actual_str = pts
                            .iter()
                            .map(|p| format!("{},{},{},{}", p.screen_x, p.screen_y, p.delta_anchor_x, p.delta_anchor_y))
                            .collect::<Vec<_>>()
                            .join(";");
                        assert_eq!(actual_str, gp[3]);
                    }
                } else {
                    assert!(rust_lights.is_err());
                }
            } else if gp[0] == "BUILDING_LIGHTS" {
                let rust_bl = get_building_world_light_positions(bytes, 100.0, 200.0, 50.0, 10.0);
                if gp[1] == "OK" {
                    let vlist = rust_bl.expect("Rust get_building_world_light_positions failed");
                    let expected_cnt: usize = gp[2].parse().unwrap();
                    assert_eq!(vlist.len(), expected_cnt);
                    if expected_cnt > 0 {
                        let actual_str = vlist
                            .iter()
                            .map(|v| format!("{},{},{}", v.x, v.y, v.z))
                            .collect::<Vec<_>>()
                            .join(";");
                        assert_eq!(actual_str, gp[3]);
                    }
                } else {
                    assert!(rust_bl.is_err());
                }
            }
        }
    }

    // Verify Malformed & Truncated Cases (Both must reject, no panic)
    for (name, bytes) in &malformed_cases {
        let status = fs::read_to_string(temp_cases.path.join(format!("{name}.status"))).unwrap();
        assert!(status.starts_with("REJECT"), "C# oracle should reject {name}, got: {status}");
        let rust_res = AptDocument::parse(bytes);
        assert!(rust_res.is_err(), "Rust parser should reject {name}, but returned Ok");
    }

    for name in &trunc_cases {
        let status = fs::read_to_string(temp_cases.path.join(format!("{name}.status"))).unwrap();
        assert!(status.starts_with("REJECT"), "C# oracle should reject {name}, got: {status}");
        let bytes = fs::read(temp_cases.path.join(format!("{name}.in"))).unwrap();
        let rust_res = AptDocument::parse(&bytes);
        assert!(rust_res.is_err(), "Rust parser should reject {name}, but returned Ok");
    }

    println!(
        "Differential parity confirmed across {} valid cases, {} malformed cases, and {} truncations.",
        valid_cases.len(),
        malformed_cases.len(),
        trunc_cases.len()
    );
}
