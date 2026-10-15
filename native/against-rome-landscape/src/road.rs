//! Ports RoadStrokePlanner.cs and RoadTileCatalog.cs, including catalog fallback rules.
use crate::{in_bounds, orthogonal_between, valid_dimensions, Point, NEIGHBORS};
use std::collections::{BTreeMap, HashMap};

pub const NORTH: u8 = 1;
pub const EAST: u8 = 2;
pub const SOUTH: u8 = 4;
pub const WEST: u8 = 8;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoadTile {
    pub texture: String,
    pub connections: u8,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoadTilePlacement {
    pub x: i32,
    pub y: i32,
    pub texture: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoadStrokePlan {
    pub succeeded: bool,
    pub tiles: Vec<RoadTilePlacement>,
    pub unsupported: Vec<Point>,
}
fn endpoint(c: u8) -> bool {
    matches!(c, 1 | 2 | 4 | 8)
}
fn straight(c: u8) -> bool {
    matches!(c, 5 | 10)
}
fn add(links: &mut BTreeMap<(i32, i32), u8>, p: Point, mask: u8) {
    // Y/X key preserves the C# final row-major order.
    *links.entry((p.1, p.0)).or_default() |= mask;
}

pub fn plan(
    dimension: i32,
    textures: &[String],
    path: &[Point],
    available: &[RoadTile],
    preferred: &str,
) -> Result<RoadStrokePlan, String> {
    if !valid_dimensions(dimension, textures.len()) {
        return Err("Invalid road map dimensions.".into());
    }
    let unsupported: Vec<_> = path
        .iter()
        .copied()
        .filter(|&p| !in_bounds(p, dimension))
        .collect();
    if !unsupported.is_empty() {
        return Ok(RoadStrokePlan {
            succeeded: false,
            tiles: vec![],
            unsupported,
        });
    }
    let mut by_name: HashMap<String, u8> = HashMap::new();
    for t in available {
        *by_name.entry(t.texture.to_ascii_uppercase()).or_default() |= t.connections;
    }
    let mut links = BTreeMap::new();
    if let Some(&first) = path.first() {
        add(&mut links, first, 0);
    }
    for pair in path.windows(2) {
        let mut previous = pair[0];
        for next in orthogonal_between(previous, pair[1]) {
            let &(_, _, out, back) = NEIGHBORS
                .iter()
                .find(|&&(dx, dy, _, _)| dx == next.0 - previous.0 && dy == next.1 - previous.1)
                .expect("orthogonal raster");
            add(&mut links, previous, out);
            add(&mut links, next, back);
            previous = next;
        }
    }
    let keys: Vec<_> = links.keys().copied().collect();
    for (y, x) in keys {
        let Some(current) =
            by_name.get(&textures[(y * dimension + x) as usize].to_ascii_uppercase())
        else {
            continue;
        };
        for (dx, dy, out, back) in NEIGHBORS {
            let p = (x + dx, y + dy);
            if in_bounds(p, dimension)
                && current & out != 0
                && by_name
                    .get(&textures[(p.1 * dimension + p.0) as usize].to_ascii_uppercase())
                    .is_some_and(|c| c & back != 0)
            {
                add(&mut links, (x, y), out);
            }
        }
    }
    let mut tiles = vec![];
    let mut unsupported = vec![];
    for ((y, x), required) in links {
        let chosen = available
            .iter()
            .filter(|t| {
                t.connections == required
                    || (endpoint(required)
                        && t.connections & required == required
                        && straight(t.connections))
                    || (required == 0 && t.texture.eq_ignore_ascii_case(preferred))
            })
            .min_by_key(|t| {
                (
                    u8::from(t.connections != required),
                    u8::from(!t.texture.eq_ignore_ascii_case(preferred)),
                    t.texture.to_ascii_uppercase(),
                )
            });
        match chosen {
            Some(t) => tiles.push(RoadTilePlacement {
                x,
                y,
                texture: t.texture.clone(),
            }),
            None => unsupported.push((x, y)),
        }
    }
    if !unsupported.is_empty() {
        tiles.clear();
    }
    Ok(RoadStrokePlan {
        succeeded: unsupported.is_empty(),
        tiles,
        unsupported,
    })
}

/// Native texture identifiers are ASCII; filtering and preference match C# ordinal-ignore-case.
pub fn build_available(existing: Option<&[String]>, preferred: Option<&str>) -> Vec<RoadTile> {
    const SH: &[&str] = &["H_WEG1", "H_WEG2", "H_WEG3", "H_WEG4", "H_WEG5"];
    const SV: &[&str] = &["V_WEG1", "V_WEG2", "V_WEG3"];
    const SJ: &[&str] = &["weg1", "weg2", "weg3"];
    const RH: &[&str] = &["WEG_H1ROM", "WEG_H2ROM"];
    const RV: &[&str] = &["WEG_V1ROM", "WEG_V2ROM", "WEG_V3ROM", "WEG_V4ROM"];
    const RJ: &[&str] = &[
        "Pflaster_braun1",
        "Pflaster_braun2",
        "Pflaster_braun3",
        "weg1",
        "weg2",
        "weg3",
    ];
    let available =
        |name: &str| existing.is_none_or(|list| list.iter().any(|s| s.eq_ignore_ascii_case(name)));
    let roman = preferred.is_some_and(|p| {
        let u = p.to_ascii_uppercase();
        u.contains("ROM") || u.starts_with("PFLASTER")
    });
    let choose = |standard: &[&str], roman_names: &[&str]| -> Vec<String> {
        let selected: Vec<&str> = if roman && roman_names.iter().any(|s| available(s)) {
            roman_names.to_vec()
        } else if !roman && standard.iter().any(|s| available(s)) {
            standard.to_vec()
        } else {
            standard.iter().chain(roman_names).copied().collect()
        };
        selected.into_iter().map(str::to_string).collect()
    };
    let horizontal = choose(SH, RH);
    let vertical = choose(SV, RV);
    let junctions: Vec<_> = if roman {
        RJ.to_vec()
    } else {
        SJ.iter().chain(RJ).copied().collect()
    };
    let mut tiles = vec![];
    for (names, connections) in [(&horizontal, 10), (&vertical, 5)] {
        for name in names.iter().filter(|s| available(s)) {
            tiles.push(RoadTile {
                texture: name.clone(),
                connections,
            });
        }
    }
    let junctions: Vec<_> = junctions.into_iter().filter(|s| available(s)).collect();
    if !junctions.is_empty() {
        for connections in 0..=15 {
            if straight(connections) || endpoint(connections) {
                let axis = if matches!(connections, 10 | 2 | 8) {
                    &horizontal
                } else {
                    &vertical
                };
                if axis.iter().any(|s| available(s)) {
                    continue;
                }
            }
            for name in &junctions {
                tiles.push(RoadTile {
                    texture: (*name).into(),
                    connections,
                });
            }
        }
    }
    if let Some(name) = preferred.filter(|s| !s.is_empty() && available(s)) {
        if !tiles
            .iter()
            .any(|t| t.connections == 0 && t.texture.eq_ignore_ascii_case(name))
        {
            tiles.push(RoadTile {
                texture: name.into(),
                connections: 0,
            });
        }
    }
    tiles
}

#[cfg(test)]
mod tests {
    use super::*;
    fn complete() -> Vec<RoadTile> {
        (1..=15)
            .map(|connections| RoadTile {
                texture: format!("road{connections}"),
                connections,
            })
            .collect()
    }
    fn blank() -> Vec<String> {
        vec!["grass".into(); 36]
    }
    fn tex(plan: &RoadStrokePlan, p: Point) -> &str {
        &plan.tiles.iter().find(|t| (t.x, t.y) == p).unwrap().texture
    }
    #[test]
    fn sparse_path_matches_csharp_reference() {
        let p = plan(
            6,
            &blank(),
            &[(1, 1), (4, 1), (4, 4)],
            &complete(),
            "road10",
        )
        .unwrap();
        assert!(p.succeeded);
        assert_eq!(p.tiles.len(), 7);
        for (point, name) in [
            ((1, 1), "road2"),
            ((2, 1), "road10"),
            ((4, 1), "road12"),
            ((4, 2), "road5"),
            ((4, 4), "road1"),
        ] {
            assert_eq!(tex(&p, point), name);
        }
    }
    #[test]
    fn crossing_preserves_existing_reciprocal_connections() {
        let mut textures = blank();
        for x in 0..6 {
            textures[12 + x] = "ROAD10".into();
        }
        let baseline = textures.clone();
        let p = plan(6, &textures, &[(3, 0), (3, 5)], &complete(), "road5").unwrap();
        assert_eq!(tex(&p, (3, 2)), "road15");
        assert_eq!(textures, baseline);
        textures[14] = "grass".into();
        let p = plan(6, &textures, &[(3, 0), (3, 5)], &complete(), "road5").unwrap();
        assert_eq!(tex(&p, (3, 2)), "road7");
    }
    #[test]
    fn missing_turn_rejects_complete_plan() {
        let tiles = vec![
            RoadTile {
                texture: "h".into(),
                connections: 10,
            },
            RoadTile {
                texture: "v".into(),
                connections: 5,
            },
        ];
        let p = plan(6, &blank(), &[(1, 1), (4, 1), (4, 4)], &tiles, "h").unwrap();
        assert!(!p.succeeded);
        assert!(p.tiles.is_empty());
        assert_eq!(p.unsupported, [(4, 1)]);
        assert!(
            plan(6, &blank(), &[(1, 1), (4, 1)], &tiles, "h")
                .unwrap()
                .succeeded
        );
    }
    #[test]
    fn backtracking_combines_links_and_sorts_unique_placements() {
        let p = plan(
            6,
            &blank(),
            &[(1, 2), (4, 2), (2, 2), (2, 4)],
            &complete(),
            "road10",
        )
        .unwrap();
        assert_eq!(tex(&p, (2, 2)), "road14");
        assert!(p
            .tiles
            .windows(2)
            .all(|w| (w[0].y, w[0].x) < (w[1].y, w[1].x)));
    }
    #[test]
    fn dimensions_bounds_empty_and_single_point() {
        assert!(plan(0, &[], &[], &[], "").is_err());
        assert!(plan(6, &[], &[], &[], "").is_err());
        let p = plan(6, &blank(), &[(1, 1), (6, 1)], &complete(), "").unwrap();
        assert_eq!(p.unsupported, [(6, 1)]);
        assert!(p.tiles.is_empty());
        assert!(plan(6, &blank(), &[], &[], "").unwrap().succeeded);
        let tiles = build_available(None, Some("H_WEG1"));
        assert_eq!(
            tex(
                &plan(6, &blank(), &[(2, 2)], &tiles, "H_WEG1").unwrap(),
                (2, 2)
            ),
            "H_WEG1"
        );
    }
    #[test]
    fn standard_and_roman_catalog_rules() {
        let existing = [
            "H_WEG1",
            "V_WEG1",
            "weg1",
            "WEG_H1ROM",
            "WEG_V1ROM",
            "Pflaster_braun1",
        ]
        .map(String::from);
        let standard = build_available(Some(&existing), None);
        let p = plan(6, &blank(), &[(1, 1), (3, 1), (3, 3)], &standard, "H_WEG1").unwrap();
        assert_eq!(tex(&p, (2, 1)), "H_WEG1");
        assert_eq!(tex(&p, (3, 1)), "Pflaster_braun1");
        assert_eq!(tex(&p, (3, 2)), "V_WEG1");
        let roman = build_available(Some(&existing), Some("WEG_H1ROM"));
        let p = plan(6, &blank(), &[(1, 1), (3, 1), (3, 3)], &roman, "WEG_H1ROM").unwrap();
        assert_eq!(tex(&p, (2, 1)), "WEG_H1ROM");
        assert_eq!(tex(&p, (3, 1)), "Pflaster_braun1");
        assert_eq!(tex(&p, (3, 2)), "WEG_V1ROM");
    }
    #[test]
    fn junction_fallback_handles_every_mask() {
        let tiles = build_available(Some(&["weg1".into()]), None);
        for mask in 0..=15 {
            assert!(tiles.iter().any(|t| t.connections == mask));
        }
        let p = plan(6, &blank(), &[(1, 1), (3, 1), (3, 3)], &tiles, "weg1").unwrap();
        assert!(p.succeeded);
        assert!(p.tiles.iter().all(|t| t.texture == "weg1"));
    }
    #[test]
    fn diagonal_strokes_have_no_disconnected_cells() {
        for path in [
            [(1, 1), (4, 4)],
            [(4, 4), (1, 1)],
            [(1, 4), (4, 1)],
            [(4, 1), (1, 4)],
        ] {
            let p = plan(6, &blank(), &path, &complete(), "road10").unwrap();
            assert!(p.succeeded);
            assert_eq!(p.tiles.len(), 7);
            for t in &p.tiles {
                assert!(p
                    .tiles
                    .iter()
                    .any(|n| (n.x - t.x).abs() + (n.y - t.y).abs() == 1));
            }
        }
    }
}
