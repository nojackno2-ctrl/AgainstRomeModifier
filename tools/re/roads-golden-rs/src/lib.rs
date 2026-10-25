//! Deterministic road connectivity and healing golden generator.
//!
//! Ports `native/against-rome-navigation/tools/csharp-golden-roads/Program.cs` and
//! `Stubs.cs` to pure Rust, consuming `against-rome-navigation` APIs read-only.

use std::ffi::OsString;
use std::fmt::Write as FmtWrite;
use std::fs::OpenOptions;
use std::io::Write as IoWrite;
use std::path::Path;

use against_rome_navigation::{
    analyze_connectivity, bridge_action, detect_road_gaps, road_heal_actions, Coordinate,
    IsolatedRegion, IssueKind, PassabilityGrid, RepairAction, RoadGap, TileChange,
};

/// 32-bit XorShift PRNG matching C# `uint state = 0xC0FFEE11; Next() => state ^= state << 13; ...`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XorShift32 {
    state: u32,
}

impl Default for XorShift32 {
    fn default() -> Self {
        Self::new(0xC0FFEE11)
    }
}

impl XorShift32 {
    #[inline]
    pub const fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    #[inline]
    pub fn next(&mut self) -> u32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;
        self.state
    }

    #[inline]
    pub fn rnd(&mut self, n: usize) -> usize {
        debug_assert!(n > 0);
        (self.next() % (n as u32)) as usize
    }
}

/// Bit-exact float formatting matching C# `BitConverter.SingleToUInt32Bits(v).ToString(...)`.
#[inline]
pub fn format_float(v: f32) -> String {
    v.to_bits().to_string()
}

/// Uppercase hexadecimal byte slice formatting matching C# `Convert.ToHexString(b)`.
pub fn format_hex(b: Option<&[u8]>) -> String {
    match b {
        None => "-".to_string(),
        Some(slice) if slice.is_empty() => "-".to_string(),
        Some(slice) => {
            let mut s = String::with_capacity(slice.len() * 2);
            for &byte in slice {
                let _ = write!(s, "{byte:02X}");
            }
            s
        }
    }
}

/// Coordinate formatting matching C# `Coord(NavMeshCoordinate c)`.
#[inline]
pub fn format_coord(c: &Coordinate) -> String {
    format!(
        "{} {} {} {}",
        c.x,
        c.z,
        c.world_x.to_bits(),
        c.world_z.to_bits()
    )
}

/// Point sequence formatting matching C# `Pts(IEnumerable<(int, int)> p)`.
pub fn format_pts(p: impl IntoIterator<Item = (i32, i32)>) -> String {
    let mut iter = p.into_iter();
    let first = match iter.next() {
        Some(first) => first,
        None => return "-".to_string(),
    };
    let mut s = format!("{},{}", first.0, first.1);
    for pt in iter {
        let _ = write!(s, ";{},{}", pt.0, pt.1);
    }
    s
}

/// Texture name encoding matching C# `Tex(string t)`.
#[inline]
pub fn format_tex(t: &str) -> &str {
    if t.is_empty() {
        "~E"
    } else if t.trim().is_empty() {
        "~S"
    } else {
        t
    }
}

/// Tile changes serialization matching C# `Changes(IReadOnlyList<NavMeshTileChange> c)`.
pub fn format_changes(c: &[TileChange]) -> String {
    if c.is_empty() {
        return "-".to_string();
    }
    c.iter()
        .map(|t| {
            let coll = t.collision.map_or("~N".to_string(), |v| v.to_string());
            format!("{},{},{},{}", t.x, t.z, t.texture, coll)
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// Repair action serialization matching C# `Action(NavMeshRepairAction a)`.
pub fn format_action(a: &RepairAction) -> String {
    let kind = match a.kind {
        IssueKind::IsolatedLand => "IsolatedLand",
        IssueKind::RoadGap => "RoadGap",
    };
    format!(
        "{} {} {} {} | {} | {} | {} | {}",
        a.id,
        kind,
        format_coord(&a.focus),
        format_changes(&a.changes),
        a.title_zh,
        a.title_en,
        a.description_zh,
        a.description_en
    )
}

/// Isolated region serialization matching differential fixture line `ISO ...`.
pub fn format_iso(r: &IsolatedRegion) -> String {
    let b = r.bridge.as_ref().map_or("-".to_string(), format_coord);
    format!(
        "{} {} {} {} {} {} {} {} {} {}",
        r.component_id,
        r.tile_count,
        format_coord(&r.centroid),
        r.bounds.0,
        r.bounds.1,
        r.bounds.2,
        r.bounds.3,
        u8::from(r.contains_interest),
        r.bridge_distance,
        b
    )
}

/// Road gap serialization matching differential fixture line `GAP ...`.
pub fn format_gap(g: &RoadGap) -> String {
    format!(
        "{} {} {} {} {} {} {}",
        format_coord(&g.start),
        format_coord(&g.end),
        format_pts(g.tiles.iter().map(|t| (t.x, t.z))),
        g.tiles.len(),
        g.texture,
        u8::from(g.clear_collision),
        g.style
    )
}

const WORLDS: [f32; 4] = [16384.0, 16384.0, 1000.0, 7777.5];
const POOL: [&str; 16] = [
    "gras",
    "sand",
    "H_WEG1",
    "V_WEG1",
    "WEG_H1ROM",
    "WEG_V2",
    "weg2",
    "PFAD1",
    "Pflaster_braun1",
    "ROMAN",
    "xWEG_1",
    "h_weg3",
    "Weg",
    "stein",
    "",
    "  ",
];
const DENSITIES: [usize; 5] = [0, 10, 25, 40, 55];

fn rand_collision(rng: &mut XorShift32, size: usize, density: usize) -> Option<Vec<u8>> {
    let m = rng.rnd(12);
    if m == 0 {
        return None;
    }
    if m == 1 {
        return Some(vec![0u8; size * size + 1]);
    }
    let mut c = vec![0u8; size * size];
    for slot in c.iter_mut() {
        *slot = if rng.rnd(100) < density {
            (rng.rnd(255) + 1) as u8
        } else {
            0
        };
    }
    Some(c)
}

fn rand_pts(rng: &mut XorShift32, size: usize, max: usize) -> Vec<(i32, i32)> {
    let n = rng.rnd(max + 1);
    let mut pts = Vec::with_capacity(n);
    for _ in 0..n {
        let x = rng.rnd(size + 2) as i32 - 1;
        let z = rng.rnd(size + 2) as i32 - 1;
        pts.push((x, z));
    }
    pts
}

fn emit_case(
    sb: &mut String,
    id: &mut usize,
    size: usize,
    collision: Option<&[u8]>,
    hs: usize,
    heights: Option<&[u8]>,
    step: f32,
    water: f32,
    world: f32,
    dim: usize,
    textures: &[String],
    seeds: &[(i32, i32)],
    interest: &[(i32, i32)],
    min_iso: usize,
    max_gap: usize,
) {
    let height_arg = heights.map(|h| (hs, h, step, water));
    let grid = PassabilityGrid::new(size, collision, height_arg, world).expect("valid grid");

    let current_id = *id;
    *id += 1;

    let _ = write!(sb, "CASE {current_id}\r\n");
    let _ = write!(
        sb,
        "GRID {size} {} {} {hs} {} {} {}\r\n",
        format_float(world),
        format_hex(collision),
        format_float(step),
        format_float(water),
        format_hex(heights)
    );
    let _ = write!(sb, "SEEDS {}\r\n", format_pts(seeds.iter().copied()));
    let _ = write!(sb, "INTEREST {}\r\n", format_pts(interest.iter().copied()));
    let _ = write!(sb, "MIN {min_iso}\r\n");

    let regions = analyze_connectivity(&grid, seeds, interest, min_iso);
    for r in &regions {
        let b = r.bridge.as_ref().map_or("-".to_string(), format_coord);
        let _ = write!(
            sb,
            "ISO {} {} {} {} {} {} {} {} {} {b}\r\n",
            r.component_id,
            r.tile_count,
            format_coord(&r.centroid),
            r.bounds.0,
            r.bounds.1,
            r.bounds.2,
            r.bounds.3,
            u8::from(r.contains_interest),
            r.bridge_distance
        );
        for &bdim in &[0, 64, dim] {
            let a = bridge_action(r, "H_WEG1", bdim);
            let a_str = match a {
                Some(ref act) => format_action(act),
                None => "none".to_string(),
            };
            let _ = write!(sb, "BRIDGE {bdim} {a_str}\r\n");
        }
    }

    let tex_line = textures
        .iter()
        .map(|t| format_tex(t))
        .collect::<Vec<_>>()
        .join(" ");
    let _ = write!(sb, "ROADS {dim} {max_gap} {tex_line}\r\n");

    for &with_grid in &[false, true] {
        let g = detect_road_gaps(
            dim,
            textures,
            if with_grid { Some(&grid) } else { None },
            max_gap,
        )
        .expect("detect_road_gaps");
        let _ = write!(sb, "GAPS {} {}\r\n", if with_grid { 1 } else { 0 }, g.len());
        for c in &g {
            let _ = write!(
                sb,
                "GAP {} {} {} {} {} {} {}\r\n",
                format_coord(&c.start),
                format_coord(&c.end),
                format_pts(c.tiles.iter().map(|t| (t.x, t.z))),
                c.tiles.len(),
                c.texture,
                u8::from(c.clear_collision),
                c.style
            );
        }
        for a in road_heal_actions(&g) {
            let _ = write!(sb, "HEAL {}\r\n", format_action(&a));
        }
    }
    sb.push_str("END\r\n");
}

/// Generates the complete 194-case golden fixture text (CRLF line endings, no BOM).
pub fn generate_golden_fixture() -> String {
    let mut rng = XorShift32::default();
    let mut sb = String::with_capacity(4_500_000);
    sb.push_str("# generated by tools/csharp-golden-roads from src.MapEditor.Modules/Pathfinding; do not edit\r\n");
    let mut id = 0;

    // 160 random cases
    for i in 0..160 {
        let size = if i < 30 { 1 + i } else { 1 + rng.rnd(28) };
        let density = DENSITIES[rng.rnd(DENSITIES.len())];
        let collision = rand_collision(&mut rng, size, density);

        let mut hs = 0;
        let mut heights: Option<Vec<u8>> = None;
        let mut step = 4.0f32;
        let mut water = 0.0f32;

        let hmode = rng.rnd(4);
        if hmode != 0 {
            hs = if hmode == 1 {
                size + 1
            } else if hmode == 2 {
                std::cmp::max(1, size / 2 + 1)
            } else {
                size * 2 + 1
            };
            let mut h = vec![0u8; hs * hs];
            for slot in h.iter_mut() {
                *slot = rng.rnd(256) as u8;
            }
            step = if rng.rnd(2) == 0 { 4.0 } else { 1.5 };
            water = rng.rnd(300) as f32;
            heights = Some(h);
        }

        let dims = [size, size, std::cmp::max(1, size / 2), size * 2, 3, 1];
        let dim = dims[rng.rnd(dims.len())];
        let mut tex = Vec::with_capacity(dim * dim);
        let road_chances = [10, 25, 50];
        let road_chance = road_chances[rng.rnd(3)];
        for _ in 0..(dim * dim) {
            let t = if rng.rnd(100) < road_chance {
                POOL[2 + rng.rnd(POOL.len() - 2)]
            } else {
                POOL[rng.rnd(2)]
            };
            tex.push(t.to_string());
        }

        let world = WORLDS[rng.rnd(WORLDS.len())];
        let seeds = rand_pts(&mut rng, size, 3);
        let interest = rand_pts(&mut rng, size, 6);
        let min_iso_pool = [0, 1, 1, 2, 4];
        let min_iso = min_iso_pool[rng.rnd(5)];
        let max_gap_pool = [0, 1, 2, 2, 3, 4];
        let max_gap = max_gap_pool[rng.rnd(6)];

        emit_case(
            &mut sb,
            &mut id,
            size,
            collision.as_deref(),
            hs,
            heights.as_deref(),
            step,
            water,
            world,
            dim,
            &tex,
            &seeds,
            &interest,
            min_iso,
            max_gap,
        );
    }

    // Structured road lines with 1-4 tile breaks, mixed styles and a collision plug in even gaps.
    let styles = ["H_WEG1", "WEG_H1ROM", "PFAD1", "weg2"];
    let gap_lens = [1, 2, 3, 4];
    for style in styles {
        for gap_len in gap_lens {
            let size = 16;
            let mut tex = vec!["gras".to_string(); size * size];
            for x in 1..15 {
                if x < 5 || x >= 5 + gap_len {
                    tex[4 * size + x] = style.to_string();
                }
            }
            for z in 7..15 {
                if z < 9 || z >= 9 + gap_len {
                    tex[z * size + 3] =
                        (if style == "H_WEG1" { "V_WEG1" } else { style }).to_string();
                }
            }
            tex[10 * size + 10] = style.to_string();
            tex[12 * size + 12] = style.to_string();
            let mut coll = vec![0u8; size * size];
            if gap_len % 2 == 0 {
                coll[4 * size + 5] = 9;
            }
            emit_case(
                &mut sb,
                &mut id,
                size,
                Some(&coll),
                0,
                None,
                4.0,
                0.0,
                16384.0,
                size,
                &tex,
                &[(0, 0)],
                &[],
                1,
                2,
            );
        }
    }

    // Isolated land behind walls of thickness 1..14 (bridge depth limit is 12).
    let thicknesses = [1, 2, 3, 4, 5, 8, 12, 13, 14];
    for thick in thicknesses {
        let size = 40;
        let mut coll = vec![0u8; size * size];
        for z in 0..size {
            for x in 10..(10 + thick) {
                coll[z * size + x] = 255;
            }
        }
        let tex_gras_40 = vec!["gras".to_string(); size * size];
        emit_case(
            &mut sb,
            &mut id,
            size,
            Some(&coll),
            0,
            None,
            4.0,
            0.0,
            16384.0,
            size,
            &tex_gras_40,
            &[(2, 2)],
            &[(35, 20)],
            1,
            2,
        );
        let tex_gras_20 = vec!["gras".to_string(); 400];
        emit_case(
            &mut sb,
            &mut id,
            size,
            Some(&coll),
            0,
            None,
            4.0,
            0.0,
            16384.0,
            20,
            &tex_gras_20,
            &[],
            &[(35, 20)],
            1,
            2,
        );
    }

    sb
}

/// Writes the golden fixture to a new file using `create_new(true)` to guarantee no overwrite.
/// Returns the number of cases generated (194).
pub fn write_golden_to_new_file(path: impl AsRef<Path>) -> std::io::Result<usize> {
    let content = generate_golden_fixture();
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path.as_ref())?;
    file.write_all(content.as_bytes())?;
    file.flush()?;
    Ok(194)
}

/// CLI execution handler for standalone invocation.
pub fn run_cli(args: &[OsString], stdout: &mut dyn IoWrite, stderr: &mut dyn IoWrite) -> i32 {
    if args.len() != 1 {
        let _ = writeln!(stderr, "Usage: roads-golden <output_file>");
        return 1;
    }
    let target = Path::new(&args[0]);
    match write_golden_to_new_file(target) {
        Ok(count) => {
            let _ = writeln!(stdout, "wrote {count} cases to {}", target.display());
            0
        }
        Err(e) => {
            let _ = writeln!(stderr, "Error writing to {}: {e}", target.display());
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED_GOLDEN: &str = include_str!(
        "../../../../native/against-rome-navigation/tests/fixtures/connectivity_roads_golden.txt"
    );

    #[test]
    fn test_xorshift_deterministic_sequence() {
        let mut rng = XorShift32::new(0xC0FFEE11);
        let first = rng.next();
        let second = rng.next();
        let third = rng.next();
        assert_ne!(first, 0);
        assert_ne!(first, second);
        assert_ne!(second, third);

        // Verify repeatability
        let mut rng2 = XorShift32::new(0xC0FFEE11);
        assert_eq!(rng2.next(), first);
        assert_eq!(rng2.next(), second);
        assert_eq!(rng2.next(), third);
    }

    #[test]
    fn test_formatting_primitives() {
        assert_eq!(format_float(16384.0), "1182793728");
        assert_eq!(format_float(0.0), "0");

        assert_eq!(format_hex(None), "-");
        assert_eq!(format_hex(Some(&[])), "-");
        assert_eq!(format_hex(Some(&[0x00, 0x5C, 0xFF])), "005CFF");

        assert_eq!(format_pts([]), "-");
        assert_eq!(format_pts([(1, 2)]), "1,2");
        assert_eq!(format_pts([(1, 2), (-3, 4)]), "1,2;-3,4");

        assert_eq!(format_tex(""), "~E");
        assert_eq!(format_tex("   "), "~S");
        assert_eq!(format_tex("H_WEG1"), "H_WEG1");
    }

    #[test]
    fn test_create_new_file_refuses_overwrite() {
        let temp_dir = crate::temp_fixture::dir("roads-create-new");
        let unique = format!(
            "roads_golden_overwrite_test_{}_{}.txt",
            std::process::id(),
            XorShift32::new(0x12345678).next()
        );
        let test_path = temp_dir.join(unique);

        // Pre-create the file
        std::fs::write(&test_path, b"existing content").expect("pre-create temp file");

        // write_golden_to_new_file must fail because file exists
        let result = write_golden_to_new_file(&test_path);
        assert!(result.is_err(), "Must fail when file already exists");
        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );

        // Verify existing content was not overwritten
        let existing = std::fs::read(&test_path).expect("read temp file");
        assert_eq!(existing, b"existing content");

        // Clean up
        let _ = std::fs::remove_file(&test_path);
    }

    #[test]
    fn test_golden_fixture_byte_exact_parity() {
        let generated = generate_golden_fixture();
        assert_eq!(
            generated.len(),
            EXPECTED_GOLDEN.len(),
            "Length mismatch: generated {} vs expected {}",
            generated.len(),
            EXPECTED_GOLDEN.len()
        );
        assert_eq!(
            generated, EXPECTED_GOLDEN,
            "Byte-exact content mismatch against connectivity_roads_golden.txt"
        );
    }
}

#[cfg(test)]
mod temp_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/temp_unique.rs"));
}
