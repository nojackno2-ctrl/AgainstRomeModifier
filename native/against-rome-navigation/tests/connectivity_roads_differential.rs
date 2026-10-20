//! Differential tests against the real C# `NavMeshConnectivityAnalyzer`, `RoadGapDetector` and
//! `RoadPathHealer`. `tests/fixtures/connectivity_roads_golden.txt` is produced by
//! `tools/csharp-golden-roads` (see README); floats are compared bit-for-bit as text.
use against_rome_navigation::*;

const GOLDEN: &str = include_str!("fixtures/connectivity_roads_golden.txt");

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
fn pts(token: &str) -> Vec<(i32, i32)> {
    if token == "-" {
        return vec![];
    }
    token
        .split(';')
        .map(|p| {
            let (x, z) = p.split_once(',').unwrap();
            (x.parse().unwrap(), z.parse().unwrap())
        })
        .collect()
}
fn pts_text(p: impl Iterator<Item = (i32, i32)>) -> String {
    let s: Vec<_> = p.map(|(x, z)| format!("{x},{z}")).collect();
    if s.is_empty() {
        "-".into()
    } else {
        s.join(";")
    }
}
fn coord(c: &Coordinate) -> String {
    format!(
        "{} {} {} {}",
        c.x,
        c.z,
        c.world_x.to_bits(),
        c.world_z.to_bits()
    )
}
fn changes(c: &[TileChange]) -> String {
    if c.is_empty() {
        return "-".into();
    }
    c.iter()
        .map(|t| {
            let coll = t.collision.map_or("~N".into(), |v| v.to_string());
            format!("{},{},{},{}", t.x, t.z, t.texture, coll)
        })
        .collect::<Vec<_>>()
        .join(";")
}
fn action(a: &RepairAction) -> String {
    let kind = match a.kind {
        IssueKind::IsolatedLand => "IsolatedLand",
        IssueKind::RoadGap => "RoadGap",
    };
    format!(
        "{} {} {} {} | {} | {} | {} | {}",
        a.id,
        kind,
        coord(&a.focus),
        changes(&a.changes),
        a.title_zh,
        a.title_en,
        a.description_zh,
        a.description_en
    )
}
fn iso(r: &IsolatedRegion) -> String {
    let b = r.bridge.as_ref().map_or("-".into(), coord);
    format!(
        "{} {} {} {} {} {} {} {} {} {}",
        r.component_id,
        r.tile_count,
        coord(&r.centroid),
        r.bounds.0,
        r.bounds.1,
        r.bounds.2,
        r.bounds.3,
        u8::from(r.contains_interest),
        r.bridge_distance,
        b
    )
}
fn gap(g: &RoadGap) -> String {
    format!(
        "{} {} {} {} {} {} {}",
        coord(&g.start),
        coord(&g.end),
        pts_text(g.tiles.iter().map(|t| (t.x, t.z))),
        g.tiles.len(),
        g.texture,
        u8::from(g.clear_collision),
        g.style
    )
}

#[derive(Default)]
struct Case {
    id: usize,
    grid: Option<PassabilityGrid>,
    seeds: Vec<(i32, i32)>,
    interest: Vec<(i32, i32)>,
    minimum: usize,
    iso: Vec<String>,
    bridges: Vec<(usize, String)>,
    dim: usize,
    max_gap: usize,
    textures: Vec<String>,
    // [without grid, with grid] -> (gap lines, heal lines)
    gaps: [(Vec<String>, Vec<String>); 2],
    current_gaps: usize,
    has_roads: bool,
}

fn check(c: &Case, stats: &mut [usize; 4]) {
    let grid = c.grid.as_ref().unwrap();
    let regions = analyze_connectivity(grid, &c.seeds, &c.interest, c.minimum);
    let got: Vec<_> = regions.iter().map(iso).collect();
    assert_eq!(got, c.iso, "case {} analyze_connectivity", c.id);
    stats[0] += regions.len();
    // BRIDGE lines are emitted per region and per dimension, in order.
    let mut expected = c.bridges.iter();
    for r in &regions {
        let mut dims = vec![0usize, 64, c.dim];
        dims.shrink_to_fit();
        for d in dims {
            let (bd, want) = expected.next().unwrap();
            assert_eq!(*bd, d, "case {} bridge dim", c.id);
            let got = bridge_action(r, "H_WEG1", d).map_or("none".to_string(), |a| action(&a));
            assert_eq!(&got, want, "case {} CreateBridgeAction dim {}", c.id, d);
            stats[1] += usize::from(got != "none");
        }
    }
    assert!(expected.next().is_none(), "case {} extra BRIDGE", c.id);
    for (with_grid, (want_gaps, want_heal)) in c.gaps.iter().enumerate() {
        let gaps = detect_road_gaps(
            c.dim,
            &c.textures,
            (with_grid == 1).then_some(grid),
            c.max_gap,
        )
        .unwrap();
        let got: Vec<_> = gaps.iter().map(gap).collect();
        assert_eq!(
            &got, want_gaps,
            "case {} DetectGaps grid={}",
            c.id, with_grid
        );
        let heal: Vec<_> = road_heal_actions(&gaps).iter().map(action).collect();
        assert_eq!(&heal, want_heal, "case {} CreateRepairActions", c.id);
        stats[2] += gaps.len();
        stats[3] += gaps.iter().filter(|g| g.clear_collision).count();
    }
}

#[test]
fn matches_csharp_connectivity_roads_and_repair_actions() {
    let (mut cases, mut stats) = (0, [0usize; 4]);
    let mut cur = Case::default();
    for line in GOLDEN
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let (tag, rest) = line.split_once(' ').unwrap_or((line, ""));
        let t: Vec<&str> = rest.split(' ').collect();
        match tag {
            "CASE" => {
                cur = Case {
                    id: t[0].parse().unwrap(),
                    ..Case::default()
                }
            }
            "GRID" => {
                let size: usize = t[0].parse().unwrap();
                let hs: usize = t[3].parse().unwrap();
                let (coll, heights) = (bytes(t[2]), bytes(t[6]));
                let height = heights.as_deref().map(|h| (hs, h, f(t[4]), f(t[5])));
                cur.grid =
                    Some(PassabilityGrid::new(size, coll.as_deref(), height, f(t[1])).unwrap());
            }
            "SEEDS" => cur.seeds = pts(t[0]),
            "INTEREST" => cur.interest = pts(t[0]),
            "MIN" => cur.minimum = t[0].parse().unwrap(),
            "ISO" => cur.iso.push(rest.to_string()),
            "BRIDGE" => {
                let (d, want) = rest.split_once(' ').unwrap();
                cur.bridges.push((d.parse().unwrap(), want.to_string()));
            }
            "ROADS" => {
                cur.dim = t[0].parse().unwrap();
                cur.max_gap = t[1].parse().unwrap();
                cur.textures = t[2..]
                    .iter()
                    .map(|s| match *s {
                        "~E" => String::new(),
                        "~S" => "  ".to_string(),
                        s => s.to_string(),
                    })
                    .collect();
                cur.has_roads = true;
            }
            "GAPS" => cur.current_gaps = t[0].parse().unwrap(),
            "GAP" => cur.gaps[cur.current_gaps].0.push(rest.to_string()),
            "HEAL" => cur.gaps[cur.current_gaps].1.push(rest.to_string()),
            "END" => {
                assert!(cur.has_roads);
                check(&cur, &mut stats);
                cases += 1;
            }
            other => panic!("unknown tag {other}"),
        }
    }
    assert!(cases >= 190, "only {cases} cases replayed");
    // The fixture must actually exercise the interesting branches.
    assert!(stats[0] > 500, "isolated regions: {}", stats[0]);
    assert!(stats[1] > 100, "bridge actions: {}", stats[1]);
    assert!(stats[2] > 1000, "road gaps: {}", stats[2]);
    assert!(stats[3] > 10, "collision-clearing gaps: {}", stats[3]);
}
