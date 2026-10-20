use against_rome_textures::{
    apt_finished_frame, direction_for_angle, NativeAnimationVisualState, NativeSpriteCatalog,
    NativeSpriteDefinition,
};
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn size(width: usize, height: usize, colors: usize) -> u32 {
    (width as u32 & 0x7FF) | ((height as u32 & 0x7FF) << 11) | ((colors as u32 & 0x1FF) << 22)
}

fn record(
    buf: &mut Vec<u8>,
    offset_x: i32,
    offset_y: i32,
    width: usize,
    palette: &[u32],
    pixels: &[u8],
    rows: &[u32],
) {
    buf.extend_from_slice(&(-1i32).to_le_bytes());
    let table_offset = (palette.len() * 4 + ((pixels.len() + 3) & !3)) as u32;
    buf.extend_from_slice(&table_offset.to_le_bytes());
    let pos = ((offset_x as u32) & 0xFFFF) | (((offset_y as u32) & 0xFFFF) << 16);
    buf.extend_from_slice(&pos.to_le_bytes());
    let s = size(width, 1, palette.len() / 2);
    buf.extend_from_slice(&s.to_le_bytes());
    buf.extend_from_slice(&(pixels.len() as u32).to_le_bytes());
    for &color in palette {
        buf.extend_from_slice(&color.to_le_bytes());
    }
    buf.extend_from_slice(pixels);
    let rem = pixels.len() & 3;
    if rem != 0 {
        for _ in 0..(4 - rem) {
            buf.push(0xAA);
        }
    }
    for &row in rows {
        buf.extend_from_slice(&row.to_le_bytes());
    }
}

fn make_alr_fixture(version: u32, second_frame_palette: bool) -> Vec<u8> {
    let mut buf = Vec::new();
    let header_words: [u32; 23] = [
        0x41524C41, version, 0x100, 4, 8, 0x200, 2, 2, 2, 0x300, 0x401, 0x402, 0x403, 0x404,
        0x405, 12, 18, 0x501, 0x502, 0x503, 0x504, 0x601, 0x602,
    ];
    for w in header_words {
        buf.extend_from_slice(&w.to_le_bytes());
    }

    // Frame 0: Record
    record(
        &mut buf,
        7,
        11,
        5,
        &[0x405060, 0x102030, 0xC0D0E0, 0x90A0B0],
        &[1, 0, 1],
        &[1u32 << 21, 3],
    );

    // Frame 1: Shared frame 0
    buf.extend_from_slice(&0i32.to_le_bytes());

    // Frame 2: Record
    let second_palette = if second_frame_palette {
        vec![0, 0xFFFFFF, 0, 0xEEEEEE]
    } else {
        vec![]
    };
    record(
        &mut buf,
        2,
        3,
        2,
        &second_palette,
        &[1, 0],
        &[0x80000000, 2],
    );

    // Frame 3: Transitive shared frame 1
    buf.extend_from_slice(&1i32.to_le_bytes());

    buf
}

fn row_width(y: usize) -> usize {
    if y <= 15 {
        (y + 1) * 4
    } else {
        (31 - y) * 4
    }
}

fn make_apt_fixture(version: u32) -> Vec<u8> {
    let mut buf = Vec::new();

    let mut header = [0u32; 28];
    header[0] = 0x54415041;
    header[1] = version;
    header[2] = 2;
    header[3] = 112;
    header[4] = 2;
    header[6] = 64;
    header[7] = 31;
    header[8] = 1;
    header[9] = 2;
    header[10] = 1;
    header[11] = 1;
    header[13] = 8;
    header[20] = 2;
    header[21] = 2;
    header[23] = 64;
    header[24] = 31;
    header[27] = 1108;

    for w in header {
        buf.extend_from_slice(&w.to_le_bytes());
    }

    let mut position = 0u32;
    for y in 0..31 {
        buf.extend_from_slice(&position.to_le_bytes());
        position += row_width(y) as u32;
    }
    for y in 0..31 {
        buf.extend_from_slice(&(row_width(y) as u32).to_le_bytes());
    }

    for w in [32u32, 20, 30, 15, 1, 1, 1] {
        buf.extend_from_slice(&w.to_le_bytes());
    }

    buf.extend_from_slice(&(-1_000_000i32).to_le_bytes());
    buf.extend_from_slice(&(-1_000_000i32).to_le_bytes());
    buf.extend_from_slice(&1u32.to_le_bytes());
    buf.extend_from_slice(&1u32.to_le_bytes());
    buf.extend_from_slice(&1u32.to_le_bytes());
    if version == 3 {
        buf.extend_from_slice(&1u32.to_le_bytes());
    }
    buf.extend_from_slice(&1u32.to_le_bytes());
    buf.extend_from_slice(&1u32.to_le_bytes());

    buf.extend_from_slice(&[0u8; 16]);
    buf.extend_from_slice(&vec![0u8; if version == 3 { 60 } else { 52 }]);

    for w in [
        0u32, 1, 0, 2, 0xAA556677, 0xBB112233, 0xCC778899, 0xDDABCDEF,
    ] {
        buf.extend_from_slice(&w.to_le_bytes());
    }

    for w in [0u32, 30, 0, 1032, 30, 0] {
        buf.extend_from_slice(&w.to_le_bytes());
    }

    buf.extend_from_slice(&0x54414450u32.to_le_bytes());
    buf.extend_from_slice(&0x7FFFFFFEu32.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());

    buf.extend_from_slice(&[1, 0, 1, 0]);
    buf.extend_from_slice(&[0u8; 1020]);

    buf.extend_from_slice(&0xFFFFFFFEu32.to_le_bytes());
    buf.extend_from_slice(&1u32.to_le_bytes());

    for y in 0..32 {
        let val: u16 = if y == 0 { 64 } else { 66 };
        buf.extend_from_slice(&val.to_le_bytes());
    }
    buf.extend_from_slice(&[0, 1, 0, 0]);

    buf
}

fn solid_alr(width: i32, height: i32, offset_x: i32, offset_y: i32, color: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    let header_words: [u32; 23] = [
        0x41524C41, 6, 0, 1, 8, 0, 1, 1, 1, 0, 0, 0, 0, 0, 0, 8, 8, 0, 0, 0, 0, 0, 0,
    ];
    for w in header_words {
        buf.extend_from_slice(&w.to_le_bytes());
    }
    let pixels = (width * height) as usize;
    let padded = (pixels + 3) & !3;
    buf.extend_from_slice(&(-1i32).to_le_bytes());
    buf.extend_from_slice(&((8 + padded) as u32).to_le_bytes());
    let pos = ((offset_x as u32) & 0xFFFF) | (((offset_y as u32) & 0xFFFF) << 16);
    buf.extend_from_slice(&pos.to_le_bytes());
    let s = (width as u32 & 0x7FF) | ((height as u32 & 0x7FF) << 11) | (2u32 << 22);
    buf.extend_from_slice(&s.to_le_bytes());
    buf.extend_from_slice(&(pixels as u32).to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&color.to_le_bytes());
    buf.extend(std::iter::repeat(1u8).take(pixels));
    buf.extend(std::iter::repeat(0u8).take(padded - pixels));
    for y in 0..=height {
        buf.extend_from_slice(&((y * width) as u32).to_le_bytes());
    }
    buf
}

const ALR_LIST: &str = "[AlrNames]\r\n0000,unit.alr,0000\r\n0001,broken.alr,0000\r\n0002,missing.alr,0000\r\n0003,trunk.alr\r\n0004,crown.alr\r\n";
const APT_LIST: &str = "[AptNames]\r\n0000,house.apt,0000\r\n";

fn row(
    id: i32,
    name: &str,
    alr: i32,
    apt: i32,
    palette: i32,
    layer: i32,
    layer_type: i32,
    cycle: i32,
    anim_add: i32,
    anim_mask: &str,
) -> String {
    let mut columns: Vec<String> = vec!["   0".to_string(); 60];
    columns[0] = id.to_string();
    columns[2] = cycle.to_string();
    columns[5] = alr.to_string();
    columns[6] = anim_add.to_string();
    columns[8] = layer.to_string();
    columns[10] = layer_type.to_string();
    columns[14] = apt.to_string();
    columns[17] = palette.to_string();
    columns[45] = anim_mask.to_string();
    columns[52] = name.to_string();
    columns.join(",")
}

fn objdef() -> String {
    vec![
        "[ObjectDefaults]".to_string(),
        ";idx ,activ,alen ,rotsp,moves,alrid".to_string(),
        row(10, "FigTest", 0, -1, 1, -1, -1, 1000, 0, "1"),
        row(11, "LanTest", 0, -1, 0, -1, -1, 0, -1, "0"),
        row(12, "BauTest", -1, 0, 0, -1, -1, 0, -1, "0"),
        row(13, "ParNone", -1, -1, 0, -1, -1, 0, -1, "0"),
        row(14, "FigBroken", 1, -1, 0, -1, -1, 0, -1, "0"),
        row(15, "FigMissing", 2, -1, 0, -1, -1, 0, -1, "0"),
        row(16, "LanTree", 3, -1, 0, 4, 2, 0, -1, "0"),
        row(17, "LanCover", -1, -1, 0, 4, 10, 0, -1, "0"),
        row(18, "LanBrokenCrown", 3, -1, 0, 1, 2, 0, -1, "0"),
    ]
    .join("\r\n")
}

fn make_test_catalog() -> NativeSpriteCatalog {
    NativeSpriteCatalog::from_text(
        &objdef(),
        ALR_LIST,
        APT_LIST,
        |name| match name {
            "unit.alr" => Some(make_alr_fixture(6, true)),
            "trunk.alr" => Some(solid_alr(1, 3, 4, 1, 0x0000FF)),
            "crown.alr" => Some(solid_alr(3, 2, 3, 0, 0x00FF00)),
            "broken.alr" => Some(vec![1, 2, 3]),
            _ => None,
        },
        |name| {
            if name == "house.apt" {
                Some(make_apt_fixture(3))
            } else {
                None
            }
        },
    )
}

#[test]
fn alr_still_uses_direction_rows_team_palette_and_canvas_centre_ground_anchor() {
    let catalog = make_test_catalog();

    let front = catalog
        .get_sprite("FigTest", 0, 14, None, None)
        .expect("Front sprite must decode");
    // Frame 0 sits at offset (7,11) in a 12x18 canvas; opaque pixels x=1..3 are cropped.
    assert_eq!(3, front.width);
    assert_eq!(1, front.height);
    assert_eq!(&[0xFF302010, 0, 0xFF302010], front.argb_pixels.as_slice());
    assert_eq!(6 - 7 - 1, front.anchor_x);
    assert_eq!(9 - 11, front.anchor_y);
    assert_eq!("unit.alr", front.asset_name);

    // Team 1 palette
    let team1 = catalog
        .get_sprite("FigTest", 1, 14, None, None)
        .expect("Team 1 sprite");
    assert_eq!(&[0xFFB0A090, 0, 0xFFB0A090], team1.argb_pixels.as_slice());

    // Team clamped: team 9 clamps to variant 1
    let team9 = catalog
        .get_sprite("FigTest", 9, 14, None, None)
        .expect("Team 9 sprite");
    assert_eq!(team1.argb_pixels, team9.argb_pixels);

    // Direction 1 starts at frame 1 * LayoutColumns(2) = frame 2, offset (2,3).
    let side = catalog
        .get_sprite("figtest", 0, 1, None, None)
        .expect("Side sprite");
    assert_eq!(&[0xFF302010, 0xFF605040], side.argb_pixels.as_slice());
    assert_eq!(6 - 2, side.anchor_x);
    assert_eq!(9 - 3, side.anchor_y);

    // Direction modulo wrap
    let side_wrapped = catalog
        .get_sprite("FigTest", 0, -1, None, None)
        .expect("Wrapped sprite");
    assert_eq!(side.argb_pixels, side_wrapped.argb_pixels);

    // Non-team palette type (palette_type == 0) ignores team
    let lan = catalog
        .get_sprite("LanTest", 1, 14, None, None)
        .expect("LanTest");
    assert_eq!(front.argb_pixels, lan.argb_pixels);
}

#[test]
fn scenario_angle_maps_to_direction_row() {
    let cases = [
        (0.0f32, 16, 14),
        (45.0, 16, 12),
        (90.0, 16, 10),
        (135.0, 16, 8),
        (180.0, 16, 6),
        (315.0, 16, 0),
        (-45.0, 16, 0),
        (360.0, 16, 14),
        (22.5, 16, 13),
        (0.0, 32, 28),
        (90.0, 32, 20),
        (123.0, 1, 0),
        (f32::NAN, 16, 0),
    ];

    for (angle, rows, expected) in cases {
        assert_eq!(
            expected,
            direction_for_angle(angle, rows),
            "Angle {angle} rows {rows}"
        );
    }
}

#[test]
fn angle_selects_the_direction_row_of_the_asset() {
    let catalog = make_test_catalog();
    // The fixture has 2 rows: angle 0 maps to row 14*2/16 = 1, angle 180 to row 0.
    let sprite_dir1 = catalog.get_sprite("FigTest", 0, 1, None, None).unwrap();
    let sprite_angle0 = catalog
        .get_sprite("FigTest", 0, 0, Some(0.0), None)
        .unwrap();
    assert_eq!(sprite_dir1.argb_pixels, sprite_angle0.argb_pixels);

    let sprite_dir0 = catalog.get_sprite("FigTest", 0, 0, None, None).unwrap();
    let sprite_angle180 = catalog
        .get_sprite("FigTest", 0, 0, Some(180.0), None)
        .unwrap();
    assert_eq!(sprite_dir0.argb_pixels, sprite_angle180.argb_pixels);
}

#[test]
fn apt_still_uses_finished_stage_and_cropped_anchor() {
    let catalog = make_test_catalog();
    let house = catalog
        .get_sprite("BauTest", 0, 0, None, None)
        .expect("BauTest sprite");
    assert_eq!(3, house.width);
    assert_eq!(1, house.height);
    assert_eq!(&[0xFF332211, 0, 0xFF332211], house.argb_pixels.as_slice());
    assert_eq!(0, house.anchor_x);
    assert_eq!(15, house.anchor_y);
}

#[test]
fn layered_alr_composites_crown_over_trunk_on_a_shared_ground_anchor() {
    let catalog = make_test_catalog();
    let tree = catalog
        .get_sprite("LanTree", 0, 0, None, None)
        .expect("LanTree sprite");
    assert_eq!("trunk.alr+crown.alr", tree.asset_name);
    // Ground-relative: trunk x 0, y -3..-1; crown x -1..1, y -4..-3 => 3x4 union anchored at (1,4).
    assert_eq!(3, tree.width);
    assert_eq!(4, tree.height);
    assert_eq!(1, tree.anchor_x);
    assert_eq!(4, tree.anchor_y);

    const G: u32 = 0xFF00FF00;
    const R: u32 = 0xFFFF0000;
    assert_eq!(
        &[G, G, G, G, G, G, 0, R, 0, 0, R, 0],
        tree.argb_pixels.as_slice()
    );

    // alrml-only ground cover uses that layer alone
    let cover = catalog
        .get_sprite("LanCover", 0, 0, None, None)
        .expect("LanCover sprite");
    assert_eq!("crown.alr", cover.asset_name);
    assert_eq!(3, cover.width);
    assert_eq!(2, cover.height);
    assert_eq!(1, cover.anchor_x);
    assert_eq!(4, cover.anchor_y);

    // Undecodable crown keeps trunk
    let trunk_only = catalog
        .get_sprite("LanBrokenCrown", 0, 0, None, None)
        .expect("Trunk only");
    assert_eq!("trunk.alr", trunk_only.asset_name);
    assert_eq!(1, trunk_only.width);
    assert_eq!(3, trunk_only.height);
}

#[test]
fn finished_apt_frame_is_last_construction_stage() {
    let cases = [
        (&[5u32, 2, 5, 25][..], 1250, 1000),
        (&[5u32, 2, 5, 1][..], 50, 40),
        (&[1u32, 2, 1, 1][..], 2, 0),
        (&[5u32, 2, 5, 25][..], 10, 9),
    ];

    for (layout, frames, expected) in cases {
        assert_eq!(expected, apt_finished_frame(layout, frames));
    }
}

#[test]
fn unknown_unmapped_missing_and_corrupt_assets_return_none() {
    let reads = Arc::new(AtomicUsize::new(0));
    let reads_clone = Arc::clone(&reads);

    let catalog = NativeSpriteCatalog::from_text(
        &objdef(),
        ALR_LIST,
        APT_LIST,
        move |name| {
            reads_clone.fetch_add(1, Ordering::SeqCst);
            match name {
                "unit.alr" => Some(make_alr_fixture(6, true)),
                "broken.alr" => Some(vec![1, 2, 3]),
                _ => None,
            }
        },
        |_| None,
    );

    assert!(catalog.get_sprite("Nope", 0, 0, None, None).is_none());
    assert!(catalog.get_sprite("ParNone", 0, 0, None, None).is_none());
    assert!(catalog.get_sprite("FigMissing", 0, 0, None, None).is_none());
    assert!(catalog.get_sprite("FigBroken", 0, 0, None, None).is_none());
    assert!(catalog.get_sprite("FigBroken", 0, 0, None, None).is_none());
    assert!(catalog.get_sprite("BauTest", 0, 0, None, None).is_none());

    // broken and missing archives are read once and cached
    assert_eq!(2, reads.load(Ordering::SeqCst));

    let definition = catalog
        .try_get_definition(" FigTest ")
        .expect("Definition exists");
    assert_eq!(
        NativeSpriteDefinition {
            type_id: 10,
            name: "FigTest".to_string(),
            alr_id: 0,
            apt_index: -1,
            palette_type: 1,
            layer_alr_id: -1,
            layer_type: -1,
            animation_length_ms: 1000,
            animation_add: 0,
            animation_mask: 1,
        },
        *definition
    );
}

#[test]
fn visual_state_parse_record_21_bytes_and_signed_alr_axes() {
    // Record size must be exactly 21
    assert!(NativeAnimationVisualState::parse_record(&[0u8; 20]).is_none());
    assert!(NativeAnimationVisualState::parse_record(&[0u8; 22]).is_none());

    // Inactive record (record[0] == 0)
    let mut inactive = [0u8; 21];
    inactive[5] = 10;
    assert!(NativeAnimationVisualState::parse_record(&inactive).is_none());

    // Valid active record
    let mut record = [0u8; 21];
    record[0] = 1; // active
    record[5..7].copy_from_slice(&(-5i16).to_le_bytes()); // alr_frame
    record[13..15].copy_from_slice(&10i16.to_le_bytes()); // axis 0
    record[17..19].copy_from_slice(&20i16.to_le_bytes()); // axis 1
    record[15..17].copy_from_slice(&30i16.to_le_bytes()); // axis 2
    record[19..21].copy_from_slice(&40i16.to_le_bytes()); // axis 3

    let state = NativeAnimationVisualState::parse_record(&record).expect("Valid record");
    assert_eq!(-5, state.alr_frame);
    assert_eq!(10, state.apt_axis0);
    assert_eq!(20, state.apt_axis1);
    assert_eq!(30, state.apt_axis2);
    assert_eq!(40, state.apt_axis3);
}

#[test]
fn try_apt_state_frame_bounds_and_overflow_protection() {
    let layout = [2u32, 2, 3, 4];
    // Layout [2,2,3,4], total 48 frames.
    // Index: (((1*2)+1)*3+2)*4+2 = 46.
    let state = NativeAnimationVisualState::new(0, 1, 1, 2, 2);
    assert_eq!(
        Some(46),
        NativeSpriteCatalog::try_apt_state_frame(&layout, 48, &state)
    );

    // Negative axis
    let neg = NativeAnimationVisualState::new(0, -1, 1, 2, 2);
    assert_eq!(
        None,
        NativeSpriteCatalog::try_apt_state_frame(&layout, 48, &neg)
    );

    // Axis >= layout
    let oob = NativeAnimationVisualState::new(0, 2, 1, 2, 2);
    assert_eq!(
        None,
        NativeSpriteCatalog::try_apt_state_frame(&layout, 48, &oob)
    );

    // Frame count boundary
    assert_eq!(
        None,
        NativeSpriteCatalog::try_apt_state_frame(&layout, 46, &state)
    );

    // Invalid layout length
    assert_eq!(
        None,
        NativeSpriteCatalog::try_apt_state_frame(&[2, 2, 3], 48, &state)
    );
}

#[test]
fn animation_retrieval_and_safe_fallback() {
    let catalog = make_test_catalog();

    // FigTest has animation_length_ms = 1000, layout_columns = 2, total frames = 4.
    let anim = catalog
        .get_animation("FigTest", 0, None, None)
        .expect("Animation for FigTest");
    assert_eq!(2, anim.frames().len());
    assert_eq!(1000, anim.cycle_duration_ms());

    // Non-animated object (LanTest has cycle = 0) returns None
    assert!(catalog.get_animation("LanTest", 0, None, None).is_none());

    // Broken object returns None
    assert!(catalog.get_animation("FigBroken", 0, None, None).is_none());
}

#[test]
fn open_reads_mock_game_directory_and_returns_none_when_incomplete() {
    let temp_dir = std::env::temp_dir().join(format!("arm-test-catalog-{}", uuid_simple()));
    let _ = std::fs::remove_dir_all(&temp_dir);

    assert!(NativeSpriteCatalog::open(&temp_dir).is_none());

    let sys_defaults = temp_dir.join("SYSTEM").join("DATA_MP").join("DEFAULTS");
    std::fs::create_dir_all(&sys_defaults).unwrap();
    std::fs::write(sys_defaults.join("objdef.dau"), objdef()).unwrap();
    std::fs::write(temp_dir.join("SYSTEM").join("cl_alr.ini"), ALR_LIST).unwrap();
    assert!(NativeSpriteCatalog::open(&temp_dir).is_none());

    std::fs::write(temp_dir.join("SYSTEM").join("cl_apt.ini"), APT_LIST).unwrap();
    assert!(NativeSpriteCatalog::open(&temp_dir).is_none());

    // Create alr.dat zip
    create_zip_file(
        &temp_dir.join("alr.dat"),
        "SYSTEM/DATA/ALR/unit.alr",
        &make_alr_fixture(6, true),
    );
    // Create apt.dat zip
    create_zip_file(
        &temp_dir.join("apt.dat"),
        "SYSTEM/DATA/APT/house.apt",
        &make_apt_fixture(3),
    );

    let catalog = NativeSpriteCatalog::open(&temp_dir).expect("Open complete catalog");
    assert_eq!(9, catalog.definition_count());

    let sprite = catalog
        .get_sprite("FigTest", 0, 14, None, None)
        .expect("FigTest sprite");
    assert_eq!(3, sprite.width);
    assert_eq!(1, sprite.height);

    let house = catalog
        .get_sprite("BauTest", 0, 0, None, None)
        .expect("BauTest sprite");
    assert_eq!(3, house.width);

    // Atlas packing
    let atlas = NativeSpriteCatalog::build_atlas(&[sprite, house], 64).expect("Pack atlas");
    assert_eq!(2, atlas.count());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn create_zip_file(path: &std::path::Path, entry_name: &str, content: &[u8]) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);
    zip.start_file(entry_name, options).unwrap();
    zip.write_all(content).unwrap();
    zip.finish().unwrap();
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{:032x}", nanos)
}

#[test]
fn alr_saved_linear_frame_includes_animation_and_direction_and_ignores_angle() {
    let catalog = make_test_catalog();
    // In our fixture, frame 2 sits at offset (2, 3), width 2, height 1.
    let state = NativeAnimationVisualState::new(2, 0, 0, 0, 0);
    let sprite = catalog
        .get_sprite("FigTest", 0, -7, Some(180.0), Some(&state))
        .expect("Explicit linear frame 2 sprite");
    assert_eq!(2, sprite.width);
    assert_eq!(1, sprite.height);
    assert_eq!(6 - 2, sprite.anchor_x);
    assert_eq!(9 - 3, sprite.anchor_y);

    // Frame 0 with state vs without state
    let state0 = NativeAnimationVisualState::new(0, 0, 0, 0, 0);
    let sprite0 = catalog
        .get_sprite("FigTest", 0, 0, None, Some(&state0))
        .expect("State 0 sprite");
    let legacy = catalog
        .get_sprite("FigTest", 0, 14, None, None)
        .expect("Legacy sprite");
    assert_eq!(sprite0.argb_pixels, legacy.argb_pixels);
}

#[test]
fn invalid_signed_alr_frames_return_none_without_legacy_fallback() {
    let catalog = make_test_catalog();
    for invalid_frame in [-1i16, i16::MIN, 4, 10, i16::MAX] {
        let state = NativeAnimationVisualState::new(invalid_frame, 0, 0, 0, 0);
        assert!(
            catalog
                .get_sprite("FigTest", 0, 0, None, Some(&state))
                .is_none(),
            "Frame {invalid_frame} must return None"
        );
        assert!(
            catalog
                .get_animation("FigTest", 0, None, Some(&state))
                .is_none(),
            "Animation for Frame {invalid_frame} must return None"
        );
    }
}

#[test]
fn definition_lookup_is_case_and_whitespace_insensitive() {
    let catalog = make_test_catalog();
    assert!(catalog.try_get_definition("figtest").is_some());
    assert!(catalog.try_get_definition("FIGTEST").is_some());
    assert!(catalog.try_get_definition("  FigTest  ").is_some());
    assert!(catalog.try_get_definition("bautest").is_some());
    assert!(catalog.try_get_definition("NonExistent").is_none());
}

