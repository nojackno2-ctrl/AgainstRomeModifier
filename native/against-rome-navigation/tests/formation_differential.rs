//! Differential tests against the real C# `FormationWidthValidator` / `NavMeshPassabilityGrid`.
//! `tests/fixtures/formation_golden.txt` is produced by `tools/csharp-golden` (see README);
//! floats are compared bit-for-bit.
use against_rome_navigation::*;

const GOLDEN: &str = include_str!("fixtures/formation_golden.txt");

fn f(token: &str) -> f32 {
    f32::from_bits(token.parse().unwrap())
}
fn bytes(hex: &str) -> Option<Vec<u8>> {
    (hex != "-").then(|| {
        (0..hex.len() / 2)
            .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    })
}
/// "tileX tileZ worldXbits worldZbits clr widthbits req reqwbits safe troops kind"
fn parse_choke(tokens: &[&str]) -> (i32, i32, u32, u32, i32, u32, i32, u32, usize, usize, String) {
    (
        tokens[0].parse().unwrap(),
        tokens[1].parse().unwrap(),
        tokens[2].parse().unwrap(),
        tokens[3].parse().unwrap(),
        tokens[4].parse().unwrap(),
        tokens[5].parse().unwrap(),
        tokens[6].parse().unwrap(),
        tokens[7].parse().unwrap(),
        tokens[8].parse().unwrap(),
        tokens[9].parse().unwrap(),
        tokens[10].to_string(),
    )
}
type ChokeKey = (i32, i32, u32, u32, i32, u32, i32, u32, usize, usize, String);
fn key(c: &ChokePoint) -> ChokeKey {
    (
        c.location.x,
        c.location.z,
        c.location.world_x.to_bits(),
        c.location.world_z.to_bits(),
        c.clearance,
        c.width.to_bits(),
        c.required,
        c.required_width.to_bits(),
        c.safe_units,
        c.troop_count,
        c.kind.to_string(),
    )
}

#[derive(Default)]
struct Case {
    id: usize,
    grid: Option<PassabilityGrid>,
    size: usize,
    trips: Vec<FormationTrip>,
    paths: Vec<Option<Vec<(i32, i32)>>>,
    corr: Vec<ChokeKey>,
    topo_arg: i32,
    topo: Vec<ChokeKey>,
}

fn cells(size: usize, mut each: impl FnMut(i32, i32, usize)) {
    let mut i = 0;
    for z in -1..=size as i32 {
        for x in -1..=size as i32 {
            each(x, z, i);
            i += 1;
        }
    }
}

fn check_case(c: &Case, counts: &mut [usize; 3]) {
    let grid = c.grid.as_ref().unwrap();
    // Corridors: signed API must match C# exactly for every troop count, including <= 0.
    let got: Vec<_> = evaluate_corridors_signed(grid, &c.trips)
        .iter()
        .map(key)
        .collect();
    assert_eq!(got, c.corr, "case {} EvaluateCorridors", c.id);
    // The unsigned legacy API must agree wherever it can express the input.
    if c.trips.iter().all(|t| t.troops >= 0) {
        let legacy: Vec<_> = c
            .trips
            .iter()
            .map(|t| (t.start, t.target, t.troops as usize))
            .collect::<Vec<_>>();
        let got: Vec<_> = evaluate_corridors(grid, &legacy).iter().map(key).collect();
        assert_eq!(got, c.corr, "case {} legacy EvaluateCorridors", c.id);
    }
    let got: Vec<_> = topological_chokes_signed(grid, c.topo_arg)
        .iter()
        .map(key)
        .collect();
    assert_eq!(got, c.topo, "case {} DetectTopologicalChokePoints", c.id);
    if c.topo_arg >= 0 {
        let got: Vec<_> = topological_chokes(grid, c.topo_arg as usize)
            .iter()
            .map(key)
            .collect();
        assert_eq!(got, c.topo, "case {} legacy topological", c.id);
    }
    // Route diagnostics agree with the C# path search and corridor result.
    let report = analyze_formation(
        grid,
        &c.trips,
        c.trips.iter().map(|t| t.troops).max().unwrap_or(0),
    );
    for (i, route) in report.routes.iter().enumerate() {
        let want = &c.paths[i];
        let reachable = route.status == RouteStatus::Reachable;
        assert_eq!(reachable, want.is_some(), "case {} trip {} status", c.id, i);
        assert_eq!(
            route.path_len,
            want.as_ref().map_or(0, Vec::len),
            "case {} trip {}",
            c.id,
            i
        );
        assert_eq!(
            route.status == RouteStatus::EndpointUnusable,
            route.start_issue.is_some() || route.target_issue.is_some()
        );
        counts[usize::from(reachable)] += 1;
    }
    let corridor: Vec<_> = report.corridor_chokes.iter().map(key).collect();
    assert_eq!(corridor, c.corr, "case {} report corridor", c.id);
    counts[2] += corridor.len();
    if corridor.is_empty() && report.routes.iter().any(|r| r.trip.troops > 1) {
        // Fallback uses the squad maximum, which may differ from the case's own minTroop argument.
        let max = c.trips.iter().map(|t| t.troops).max().unwrap();
        assert_eq!(
            report
                .topological_chokes
                .iter()
                .map(key)
                .collect::<Vec<_>>(),
            topological_chokes_signed(grid, max)
                .iter()
                .map(key)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn matches_csharp_golden_vectors() {
    let mut cases = 0;
    let mut counts = [0usize; 3];
    let mut req = 0;
    let mut cur = Case::default();
    for line in GOLDEN
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let t: Vec<&str> = line.split(' ').collect();
        match t[0] {
            "REQ" => {
                let n: i32 = t[1].parse().unwrap();
                let want = (t[2].parse::<i32>().unwrap(), f(t[3]));
                assert_eq!(required_clearance_signed(n), want, "REQ {n}");
                if n >= 0 {
                    assert_eq!(required_clearance(n as usize), want);
                }
                req += 1;
            }
            "SAFE" => {
                let c: i32 = t[1].parse().unwrap();
                assert_eq!(
                    max_safe_unit_count(c),
                    t[2].parse::<usize>().unwrap(),
                    "SAFE {c}"
                );
            }
            "CASE" => {
                cur = Case {
                    id: t[1].parse().unwrap(),
                    ..Case::default()
                }
            }
            "GRID" => {
                cur.size = t[1].parse().unwrap();
                let (world, hs) = (f(t[2]), t[4].parse::<usize>().unwrap());
                let (coll, heights) = (bytes(t[3]), bytes(t[7]));
                let height = heights.as_deref().map(|h| (hs, h, f(t[5]), f(t[6])));
                cur.grid =
                    Some(PassabilityGrid::new(cur.size, coll.as_deref(), height, world).unwrap());
            }
            "PASS" | "SUB" | "BLK" | "CBYTE" => {
                let g = cur.grid.as_ref().unwrap();
                let want = t[1].as_bytes();
                cells(cur.size, |x, z, i| match t[0] {
                    "PASS" => assert_eq!(
                        g.passable(x, z),
                        want[i] == b'1',
                        "case {} PASS {x},{z}",
                        cur.id
                    ),
                    "SUB" => assert_eq!(
                        g.submerged(x, z),
                        want[i] == b'1',
                        "case {} SUB {x},{z}",
                        cur.id
                    ),
                    "BLK" => assert_eq!(
                        g.collision_blocked(x, z),
                        want[i] == b'1',
                        "case {} BLK {x},{z}",
                        cur.id
                    ),
                    _ => assert_eq!(
                        format!("{:02X}", g.collision_byte(x, z)).as_bytes(),
                        &want[i * 2..i * 2 + 2],
                        "case {} CBYTE {x},{z}",
                        cur.id
                    ),
                });
            }
            "TILE" => {
                let g = cur.grid.as_ref().unwrap();
                let dim: usize = t[1].parse().unwrap();
                let (blk, sub) = (t[2].as_bytes(), t[3].as_bytes());
                for z in 0..dim {
                    for x in 0..dim {
                        let i = z * dim + x;
                        assert_eq!(
                            g.tile_collision_blocked(x as i32, z as i32, dim),
                            blk[i] == b'1',
                            "case {} tile blocked dim {dim} {x},{z}",
                            cur.id
                        );
                        assert_eq!(
                            g.tile_submerged(x as i32, z as i32, dim),
                            sub[i] == b'1',
                            "case {} tile submerged dim {dim} {x},{z}",
                            cur.id
                        );
                    }
                }
                if dim == 0 {
                    assert!(g.tile_collision_blocked(0, 0, 0));
                    assert!(!g.tile_submerged(0, 0, 0));
                }
            }
            "W2T" => {
                let g = cur.grid.as_ref().unwrap();
                assert_eq!(
                    g.world_to_tile(f(t[1]), f(t[2])),
                    (t[3].parse().unwrap(), t[4].parse().unwrap()),
                    "case {} W2T",
                    cur.id
                );
                assert_eq!(g.tile_scale().to_bits(), f(t[5]).to_bits());
            }
            "CLEAR" => {
                let want: Vec<i32> = t[1..].iter().map(|v| v.parse().unwrap()).collect();
                assert_eq!(
                    clearance_field(cur.grid.as_ref().unwrap()),
                    want,
                    "case {} CLEAR",
                    cur.id
                );
            }
            "TRIP" => cur.trips.push(FormationTrip {
                start: (t[2].parse().unwrap(), t[3].parse().unwrap()),
                target: (t[4].parse().unwrap(), t[5].parse().unwrap()),
                troops: t[6].parse().unwrap(),
            }),
            "PATH" => {
                let trip = cur.trips[t[1].parse::<usize>().unwrap()];
                let got = find_path(cur.grid.as_ref().unwrap(), trip.start, trip.target);
                let want = (t[2] != "none").then(|| {
                    t[3].split(';')
                        .map(|p| {
                            let (x, z) = p.split_once(',').unwrap();
                            (x.parse().unwrap(), z.parse().unwrap())
                        })
                        .collect::<Vec<(i32, i32)>>()
                });
                assert_eq!(got, want, "case {} PATH {}", cur.id, t[1]);
                cur.paths.push(want);
            }
            "CORR" => cur.corr.push(parse_choke(&t[1..])),
            "TOPOARG" => cur.topo_arg = t[1].parse().unwrap(),
            "TOPO" => cur.topo.push(parse_choke(&t[1..])),
            "END" => {
                check_case(&cur, &mut counts);
                cases += 1;
            }
            other => panic!("unknown golden record {other}"),
        }
    }
    assert_eq!(req, 31);
    assert!(cases >= 200, "golden fixture truncated: {cases}");
    // The corpus must exercise reachable routes, unreachable routes and real chokes.
    assert!(counts.iter().all(|&n| n > 50), "weak coverage: {counts:?}");
}
