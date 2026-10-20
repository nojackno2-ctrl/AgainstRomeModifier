//! Deterministic formation-width and passability golden generator.
//!
//! Ports `native/against-rome-navigation/tools/csharp-golden/Program.cs` and
//! `FormationGolden.csproj` to pure Rust, consuming `against-rome-navigation` APIs read-only.

use std::ffi::OsString;
use std::fmt::Write as FmtWrite;
use std::fs::OpenOptions;
use std::io::Write as IoWrite;
use std::path::Path;

use against_rome_navigation::{
    clearance_field, evaluate_corridors_signed, find_path, max_safe_unit_count,
    required_clearance_signed, topological_chokes_signed, ChokePoint, Coordinate, FormationTrip,
    PassabilityGrid,
};

/// 32-bit XorShift PRNG matching C# `uint state = 0x9E3779B9; Next() => state ^= state << 13; ...`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XorShift32 {
    state: u32,
}

impl Default for XorShift32 {
    fn default() -> Self {
        Self::new(0x9E3779B9)
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

/// Bit-exact float formatting matching C# `BitConverter.SingleToUInt32Bits(v).ToString(CultureInfo.InvariantCulture)`.
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

/// Choke point formatting matching C# `Choke(FormationChokePoint c)`.
#[inline]
pub fn format_choke(c: &ChokePoint) -> String {
    format!(
        "{} {} {} {} {} {} {} {}",
        format_coord(&c.location),
        c.clearance,
        c.width.to_bits(),
        c.required,
        c.required_width.to_bits(),
        c.safe_units,
        c.troop_count,
        c.kind
    )
}

pub const CSHARP_FLOAT_NAN: f32 = f32::from_bits(0xFFC0_0000);

pub const TROOP_CHOICES: [i32; 12] = [-3, 0, 1, 2, 5, 6, 10, 11, 15, 16, 20, 30];
pub const WORLDS: [f32; 5] = [16384.0, 16384.0, 16384.0, 1000.0, 7777.5];
pub const DENSITIES: [usize; 5] = [0, 5, 15, 30, 50];
pub const TOTAL_CASES: usize = 248;

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
    trips: &[(i32, i32, i32, i32, i32)],
    min_troop: i32,
) {
    let height_arg = heights.map(|h| (hs, h, step, water));
    let grid = PassabilityGrid::new(size, collision, height_arg, world).expect("valid grid");

    let current_id = *id;
    *id += 1;

    let _ = write!(sb, "CASE {current_id}\r\n");
    let _ = write!(
        sb,
        "GRID {size} {} {} {hs} {} {} {}\r\n",
        world.to_bits(),
        format_hex(collision),
        step.to_bits(),
        water.to_bits(),
        format_hex(heights)
    );

    let cell_count = (size + 2) * (size + 2);
    let mut pass = String::with_capacity(cell_count);
    let mut sub = String::with_capacity(cell_count);
    let mut blk = String::with_capacity(cell_count);
    let mut cb = String::with_capacity(cell_count * 2);

    for z in -1..=(size as i32) {
        for x in -1..=(size as i32) {
            pass.push(if grid.passable(x, z) { '1' } else { '0' });
            sub.push(if grid.submerged(x, z) { '1' } else { '0' });
            blk.push(if grid.collision_blocked(x, z) {
                '1'
            } else {
                '0'
            });
            let _ = write!(cb, "{:02X}", grid.collision_byte(x, z));
        }
    }

    let _ = write!(sb, "PASS {pass}\r\n");
    let _ = write!(sb, "SUB {sub}\r\n");
    let _ = write!(sb, "BLK {blk}\r\n");
    let _ = write!(sb, "CBYTE {cb}\r\n");

    let dims: &[usize] = if size <= 12 {
        &[
            0,
            1,
            3,
            std::cmp::max(1, size / 2),
            size,
            size + 3,
            size * 2 + 1,
        ]
    } else {
        &[0, 1, 3, std::cmp::max(1, size / 2), size, size + 3]
    };

    for &dim in dims {
        if dim == 0 {
            sb.push_str("TILE 0 - -\r\n");
        } else {
            let mut tb = String::with_capacity(dim * dim);
            let mut ts = String::with_capacity(dim * dim);
            for z in 0..dim {
                for x in 0..dim {
                    tb.push(if grid.tile_collision_blocked(x as i32, z as i32, dim) {
                        '1'
                    } else {
                        '0'
                    });
                    ts.push(if grid.tile_submerged(x as i32, z as i32, dim) {
                        '1'
                    } else {
                        '0'
                    });
                }
            }
            let _ = write!(sb, "TILE {dim} {tb} {ts}\r\n");
        }
    }

    let probes = [
        -50.0f32,
        0.0,
        1.0,
        (world / size as f32) * 0.5,
        world / 3.0,
        world - 0.01,
        world,
        world * 2.0,
    ];
    for i in 0..probes.len() {
        let px = probes[i];
        let pz = probes[(i + 3) % probes.len()];
        let (tx, tz) = grid.world_to_tile(px, pz);
        let _ = write!(
            sb,
            "W2T {} {} {tx} {tz} {}\r\n",
            px.to_bits(),
            pz.to_bits(),
            grid.tile_scale().to_bits()
        );
    }

    let field = clearance_field(&grid);
    sb.push_str("CLEAR");
    for val in &field {
        let _ = write!(sb, " {val}");
    }
    sb.push_str("\r\n");

    let formation_trips: Vec<FormationTrip> = trips
        .iter()
        .map(|&(sx, sz, tx, tz, n)| FormationTrip {
            start: (sx, sz),
            target: (tx, tz),
            troops: n,
        })
        .collect();

    for (i, &(sx, sz, tx, tz, n)) in trips.iter().enumerate() {
        let _ = write!(sb, "TRIP {i} {sx} {sz} {tx} {tz} {n}\r\n");
        let path = find_path(&grid, (sx, sz), (tx, tz));
        match path {
            None => {
                let _ = write!(sb, "PATH {i} none\r\n");
            }
            Some(p) => {
                let _ = write!(sb, "PATH {i} {} ", p.len());
                for (idx, pt) in p.iter().enumerate() {
                    if idx > 0 {
                        sb.push(';');
                    }
                    let _ = write!(sb, "{},{}", pt.0, pt.1);
                }
                sb.push_str("\r\n");
            }
        }
    }

    let corr = evaluate_corridors_signed(&grid, &formation_trips);
    for c in &corr {
        let _ = write!(sb, "CORR {}\r\n", format_choke(c));
    }

    let _ = write!(sb, "TOPOARG {min_troop}\r\n");
    let topo = topological_chokes_signed(&grid, min_troop);
    for c in &topo {
        let _ = write!(sb, "TOPO {}\r\n", format_choke(c));
    }

    sb.push_str("END\r\n");
}

fn rand_trips(rng: &mut XorShift32, size: usize) -> Vec<(i32, i32, i32, i32, i32)> {
    let count = rng.rnd(7);
    let mut trips = Vec::with_capacity(count);
    for _ in 0..count {
        let sx = rng.rnd(size + 2) as i32 - 1;
        let sz = rng.rnd(size + 2) as i32 - 1;
        let tx = rng.rnd(size + 2) as i32 - 1;
        let tz = rng.rnd(size + 2) as i32 - 1;
        let n = TROOP_CHOICES[rng.rnd(TROOP_CHOICES.len())];
        trips.push((sx, sz, tx, tz, n));
    }
    trips
}

/// Generates the complete 248-case golden fixture text (CRLF line endings, no BOM).
pub fn generate_golden_fixture() -> String {
    let mut rng = XorShift32::default();
    let mut sb = String::with_capacity(3_600_000);
    sb.push_str("# generated by tools/csharp-golden from src.MapEditor.Modules/Pathfinding; do not edit\r\n");

    for n in -5..=25 {
        let (t, w) = required_clearance_signed(n);
        let _ = write!(sb, "REQ {n} {t} {}\r\n", w.to_bits());
    }
    for c in -2..=9 {
        let _ = write!(sb, "SAFE {c} {}\r\n", max_safe_unit_count(c));
    }

    let mut id = 0;

    // 220 randomized cases
    for i in 0..220 {
        let size = if i < 40 {
            1 + i
        } else if i % 25 == 0 {
            64
        } else {
            1 + rng.rnd(40)
        };
        let density = DENSITIES[rng.rnd(DENSITIES.len())];
        let mut coll = vec![0u8; size * size];
        for slot in coll.iter_mut() {
            *slot = if rng.rnd(100) < density {
                (rng.rnd(255) + 1) as u8
            } else {
                0
            };
        }
        let collision = if rng.rnd(10) == 0 {
            Some(vec![0u8; size * size + 1])
        } else if rng.rnd(12) == 0 {
            None
        } else {
            Some(coll)
        };

        let mut hs = 0;
        let mut heights: Option<Vec<u8>> = None;
        let mut step = 4.0f32;
        let mut water = 0.0f32;

        let hmode = rng.rnd(4);
        if hmode != 0 {
            hs = match hmode {
                1 => size + 1,
                2 => std::cmp::max(1, size / 2 + 1),
                _ => size * 2 + 1,
            };
            if rng.rnd(15) == 0 {
                hs = 1;
            }
            let extra = if rng.rnd(15) == 0 { 1 } else { 0 };
            let len = hs * hs + extra;
            let mut h = vec![0u8; len];
            for slot in h.iter_mut() {
                *slot = rng.rnd(256) as u8;
            }
            heights = Some(h);
            step = if rng.rnd(12) == 0 {
                if rng.rnd(2) == 0 {
                    0.0
                } else {
                    CSHARP_FLOAT_NAN
                }
            } else if rng.rnd(2) == 0 {
                4.0
            } else {
                1.5
            };
            water = if rng.rnd(20) == 0 {
                CSHARP_FLOAT_NAN
            } else {
                rng.rnd(400) as f32
            };
        }

        let world = WORLDS[rng.rnd(WORLDS.len())];
        let trips = rand_trips(&mut rng, size);
        let min_troop = TROOP_CHOICES[rng.rnd(TROOP_CHOICES.len())];

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
            &trips,
            min_troop,
        );
    }

    // Structured corridor / canyon cases: a vertical lane of width w through a wall band.
    for &size in &[24, 32] {
        for w in 1..=7 {
            let mut coll = vec![0u8; size * size];
            let x0 = size as i32 / 2 - w as i32 / 2;
            for z in (size / 4)..(size * 3 / 4) {
                for x in 0..size {
                    if (x as i32) < x0 || (x as i32) >= x0 + w as i32 {
                        coll[z * size + x] = 255;
                    }
                }
            }
            let mut trips = Vec::with_capacity(5);
            for &n in &[2, 6, 11, 16, 20] {
                trips.push((x0, 1, x0, size as i32 - 2, n));
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
                &trips,
                10,
            );
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
                &[],
                20,
            );
        }
    }

    sb
}

/// Writes the golden fixture to a new file using `create_new(true)` to guarantee no overwrite.
/// Returns the number of cases generated (248).
pub fn write_golden_to_new_file(path: impl AsRef<Path>) -> std::io::Result<usize> {
    let content = generate_golden_fixture();
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path.as_ref())?;
    file.write_all(content.as_bytes())?;
    file.flush()?;
    Ok(TOTAL_CASES)
}

/// CLI execution handler for standalone invocation.
pub fn run_cli(args: &[OsString], stdout: &mut dyn IoWrite, stderr: &mut dyn IoWrite) -> i32 {
    if args.len() != 1 {
        let _ = writeln!(stderr, "Usage: formation-golden <output_file>");
        return 1;
    }
    let target = Path::new(&args[0]);
    match write_golden_to_new_file(target) {
        Ok(count) => {
            let _ = writeln!(stdout, "wrote {count} cases to {}", target.display());
            0
        }
        Err(err) => {
            let _ = writeln!(stderr, "Error writing to {}: {err}", target.display());
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xorshift32_seed_parity() {
        let mut rng = XorShift32::default();
        let first = rng.next();
        let second = rng.next();
        let third = rng.next();
        assert_ne!(first, 0);
        assert_ne!(second, 0);
        assert_ne!(third, 0);
        assert_eq!(rng.rnd(10), (rng.next() % 10) as usize);
    }

    #[test]
    fn test_format_float_bits() {
        assert_eq!(format_float(0.0), "0");
        assert_eq!(format_float(1.0), "1065353216");
        assert_eq!(format_float(16384.0), "1182793728");
    }

    #[test]
    fn test_format_hex_none_and_empty() {
        assert_eq!(format_hex(None), "-");
        assert_eq!(format_hex(Some(&[])), "-");
        assert_eq!(format_hex(Some(&[0x00, 0xFF, 0x1A])), "00FF1A");
    }

    #[test]
    fn test_format_coord_and_choke() {
        let coord = Coordinate {
            x: 5,
            z: 7,
            world_x: 100.0,
            world_z: 200.0,
        };
        assert_eq!(
            format_coord(&coord),
            format!("5 7 {} {}", 100.0f32.to_bits(), 200.0f32.to_bits())
        );

        let choke = ChokePoint {
            location: coord,
            clearance: 2,
            width: 128.0,
            required: 4,
            required_width: 256.0,
            safe_units: 5,
            troop_count: 10,
            kind: "TravelPathChoke",
        };
        assert_eq!(
            format_choke(&choke),
            format!(
                "5 7 {} {} 2 {} 4 {} 5 10 TravelPathChoke",
                100.0f32.to_bits(),
                200.0f32.to_bits(),
                128.0f32.to_bits(),
                256.0f32.to_bits()
            )
        );
    }

    #[test]
    fn test_constants_and_counts() {
        assert_eq!(TROOP_CHOICES.len(), 12);
        assert_eq!(WORLDS.len(), 5);
        assert_eq!(DENSITIES.len(), 5);
        assert_eq!(TOTAL_CASES, 248);
    }
}
