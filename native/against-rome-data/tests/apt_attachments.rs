use against_rome_data::apt::*;
use against_rome_data::apt_geometry::*;

fn put_u32_le(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn make_attachment_fixture(
    version: u32,
    lights: &[&[&[AptPoint]]],
    smoke: &[&[AptPoint]],
    markers: &[AptPoint],
    origin: Option<AptPoint>,
    anchor: Option<AptPoint>,
) -> Vec<u8> {
    let mut buf = Vec::new();
    let num_groups = lights.len();
    let variants = lights[0].len();

    let mut header = [0u32; 28];
    header[0] = 0x54415041;
    header[1] = version;
    header[2] = (num_groups * variants) as u32;
    header[3] = 112;
    header[6] = 64;
    header[7] = 31;
    header[8] = num_groups as u32;
    header[9] = variants as u32;
    header[10] = 1;
    header[11] = 1;
    header[13] = 8;
    header[20] = 1;
    header[21] = 1;
    header[23] = 64;
    header[24] = 31;

    for w in header {
        buf.extend_from_slice(&w.to_le_bytes());
    }

    // 62 words of row offsets and row widths (all zeros)
    buf.extend_from_slice(&[0u8; 62 * 4]);

    let org = origin.unwrap_or(AptPoint::new(3, 5));
    buf.extend_from_slice(&org.x.to_le_bytes());
    buf.extend_from_slice(&org.y.to_le_bytes());

    let anc = anchor.unwrap_or(AptPoint::new(7, 11));
    buf.extend_from_slice(&anc.x.to_le_bytes());
    buf.extend_from_slice(&anc.y.to_le_bytes());

    buf.extend_from_slice(&1u32.to_le_bytes()); // extraA
    buf.extend_from_slice(&0u32.to_le_bytes()); // extraB
    buf.extend_from_slice(&(num_groups as u32).to_le_bytes());

    for i in 0..num_groups {
        buf.extend_from_slice(&markers[i].x.to_le_bytes());
        buf.extend_from_slice(&markers[i].y.to_le_bytes());
        buf.extend_from_slice(&1u32.to_le_bytes()); // first
        buf.extend_from_slice(&1u32.to_le_bytes()); // second
        let v_count = if version == 2 { 1 } else { variants };
        for v in 0..v_count {
            buf.extend_from_slice(&(lights[i][v].len() as u32).to_le_bytes());
        }
        buf.extend_from_slice(&(smoke[i].len() as u32).to_le_bytes());
        buf.extend_from_slice(&1u32.to_le_bytes()); // tail
    }

    // Decoy global extraA: 8 bytes
    buf.extend_from_slice(&999i32.to_le_bytes());
    buf.extend_from_slice(&997i32.to_le_bytes());

    for i in 0..num_groups {
        buf.extend_from_slice(&[0u8; 20]);
        let v_count = if version == 2 { 1 } else { variants };
        for v in 0..v_count {
            for p in lights[i][v] {
                buf.extend_from_slice(&p.x.to_le_bytes());
                buf.extend_from_slice(&p.y.to_le_bytes());
            }
        }
        for p in smoke[i] {
            buf.extend_from_slice(&p.x.to_le_bytes());
            buf.extend_from_slice(&p.y.to_le_bytes());
        }
        buf.extend_from_slice(&[0u8; 16]);
    }

    // Frames: (first=0, count=0)
    for _ in 0..(num_groups * variants) {
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
    }

    // 1 palette word (color 0)
    buf.extend_from_slice(&0u32.to_le_bytes());

    // 0 tiles

    // PDAT magic
    buf.extend_from_slice(&0x54414450u32.to_le_bytes());

    buf
}

fn single_lights(deltas: &[(i32, i32)]) -> Vec<u8> {
    let pts: Vec<AptPoint> = deltas
        .iter()
        .map(|&(x, y)| AptPoint::new(x + 569, y + 405))
        .collect();
    let pts_slice = pts.as_slice();
    let lights: &[&[AptPoint]] = &[pts_slice];
    let all_lights: &[&[&[AptPoint]]] = &[lights];
    let empty_smoke: &[AptPoint] = &[];
    let all_smoke: &[&[AptPoint]] = &[empty_smoke];
    let markers = [AptPoint::new(-1_000_000, -1_000_000)];

    make_attachment_fixture(
        3,
        all_lights,
        all_smoke,
        &markers,
        Some(AptPoint::new(0, 0)),
        Some(AptPoint::new(569, 405)),
    )
}

#[test]
fn attachment_groups_select_stage_and_variant_without_using_global_extra_points() {
    for version in [2u32, 3] {
        let g0_v0 = [AptPoint::new(5, 7)];
        let g0_v1 = [AptPoint::new(11, 13), AptPoint::new(17, 19)];
        let g0: &[&[AptPoint]] = &[&g0_v0, &g0_v1];

        let g1_v0 = [AptPoint::new(23, 29)];
        let g1_v1 = [AptPoint::new(31, 37)];
        let g1: &[&[AptPoint]] = &[&g1_v0, &g1_v1];

        let all_lights: &[&[&[AptPoint]]] = &[g0, g1];

        let s0 = [AptPoint::new(41, 43)];
        let s1 = [AptPoint::new(47, 53)];
        let all_smoke: &[&[AptPoint]] = &[&s0, &s1];

        let markers = [AptPoint::new(-1_000_000, 0), AptPoint::new(59, 61)];

        let mut source = make_attachment_fixture(version, all_lights, all_smoke, &markers, None, None);
        let doc = AptDocument::parse(&source).expect("parse fixture");
        source.fill(0);

        assert_eq!(doc.projection_origin, AptPoint::new(3, 5));

        let att1 = doc.get_attachments(1, 0, 0).expect("get_attachments(1, 0, 0)");
        assert_eq!(att1.lights, &[AptPoint::new(23, 29)]);

        let att0 = doc.get_attachments(0, 1, 0).expect("get_attachments(0, 1, 0)");
        if version == 2 {
            assert_eq!(att0.lights, &g0_v0);
        } else {
            assert_eq!(att0.lights, &g0_v1);
        }

        let att_max = doc
            .get_attachments(u32::MAX, 0, u32::MAX)
            .expect("get_attachments max");
        assert_eq!(att_max.smoke, &[AptPoint::new(47, 53)]);

        assert_eq!(att1.marker, Some(AptPoint::new(59, 61)));
        assert_eq!(doc.get_attachments(0, 0, 0).unwrap().marker, None);

        let fixture_bytes = make_attachment_fixture(
            version,
            all_lights,
            &[&[], &[]],
            &[AptPoint::default(), AptPoint::default()],
            None,
            None,
        );
        let extracted = extract_light_points(&fixture_bytes).expect("extract_light_points");
        assert_eq!(extracted.len(), 1);
        assert_eq!(extracted[0].screen_x, 23);
    }
}

#[test]
fn default_projection_floors_signed_points_relative_to_first_anchor() {
    let cases = [
        (0, 0, 0, 0),
        (63, 0, 0, -1),
        (64, 0, 1, -1),
        (-1, 0, -1, 0),
        (0, -1, -1, -1),
    ];

    let bytes = single_lights(&[(0, 0)]);
    let doc = AptDocument::parse(&bytes).expect("parse single lights");

    for (x, y, grid_x, grid_z) in cases {
        let projected = project(&doc, AptPoint::new(x, y));
        assert_eq!(
            projected,
            AptPoint::new(grid_x, grid_z),
            "failed projection for ({x}, {y})"
        );
    }
}

#[test]
fn parent_world_position_uses_native_biased_conversion_then_signed_grid_shift() {
    let cases = [
        (0.0f32, 0.0f32),
        (63.9, 0.0),
        (64.0, 64.0),
        (-0.5, 0.0),
        (-1.0, -64.0),
        (-64.5, -64.0),
    ];

    for (world, expected) in cases {
        let actual = world_grid_origin(world).expect("world_grid_origin");
        assert_eq!(
            actual, expected,
            "failed world_grid_origin for {world}: expected {expected}, got {actual}"
        );
    }
}

#[test]
fn per_point_quantization_preserves_anchor_phase_and_native_capacity20() {
    let points: Vec<AptPoint> = (0..21).map(|_| AptPoint::new(65, 0)).collect();
    let pts_slice = points.as_slice();
    let lights: &[&[AptPoint]] = &[pts_slice];
    let all_lights: &[&[&[AptPoint]]] = &[lights];
    let empty_smoke: &[AptPoint] = &[];
    let all_smoke: &[&[AptPoint]] = &[empty_smoke];
    let markers = [AptPoint::default()];

    let fixture = make_attachment_fixture(
        3,
        all_lights,
        all_smoke,
        &markers,
        Some(AptPoint::new(0, 0)),
        Some(AptPoint::new(63, 0)),
    );

    let doc = AptDocument::parse(&fixture).expect("parse fixture");
    let placed = place(&doc, AptPoint::new(65, 0), 0.0, 0.0, 80.0).expect("place");
    assert_eq!(placed, AptVector3::new(96.0, 80.0, -32.0));

    let world_lights = get_building_world_light_positions(&fixture, 0.0, 0.0, 0.0, 80.0)
        .expect("get_building_world_light_positions");
    assert_eq!(world_lights.len(), 20);
}

#[test]
fn every_truncated_prefix_and_missing_group_is_rejected() {
    for version in [2u32, 3] {
        let g0_v0 = [AptPoint::new(5, 7)];
        let g0_v1 = [AptPoint::new(11, 13)];
        let g0: &[&[AptPoint]] = &[&g0_v0, &g0_v1];
        let all_lights: &[&[&[AptPoint]]] = &[g0];

        let s0 = [AptPoint::new(17, 19)];
        let all_smoke: &[&[AptPoint]] = &[&s0];
        let markers = [AptPoint::default()];

        let mut bytes = make_attachment_fixture(version, all_lights, all_smoke, &markers, None, None);

        for i in 0..bytes.len() {
            assert!(
                AptDocument::parse(&bytes[..i]).is_err(),
                "expected truncation error for length {i}"
            );
        }

        put_u32_le(&mut bytes, 32, 2); // layout[0] = 2 now references an absent group
        let doc = AptDocument::parse(&bytes).expect("parse with modified layout");
        assert!(matches!(
            doc.get_attachments(1, 0, 0),
            Err(AptError::AttachmentGroupMissing { .. })
        ));
    }
}
