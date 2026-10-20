use against_rome_data::apt::*;

fn row_width(y: usize) -> usize {
    if y <= 15 {
        (y + 1) * 4
    } else {
        (31 - y) * 4
    }
}

fn put_u32_le(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u16_le(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn make_fixture(version: u32) -> Vec<u8> {
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

fn blob_offset(bytes: &[u8]) -> usize {
    bytes.len() - 1108
}

#[test]
fn raw_and_compressed_diamonds_compose_without_erasing_transparent_pixels() {
    for version in [2u32, 3] {
        let mut bytes = make_fixture(version);
        let doc = AptDocument::parse(&bytes).expect("parse failed");
        bytes.fill(0);

        assert_eq!(doc.version, version);
        assert_eq!(doc.anchor_x, 30);
        assert_eq!(doc.anchor_y, 15);
        assert_eq!(doc.layout, [1, 2, 1, 1]);
        assert_eq!(doc.frames[0], AptFrameRange::new(0, 1));
        assert_eq!(doc.frames[1], AptFrameRange::new(0, 2));
        assert_eq!(doc.tiles[0].x(), 30);
        assert_eq!(doc.tiles[0].y(), 0);

        let raw = doc.decode_tile(0, 0).expect("decode tile 0");
        assert_eq!(raw.width, 64);
        assert_eq!(raw.height, 31);
        assert_eq!(
            &raw.argb_pixels[30..34],
            &[0xFF332211, 0, 0xFF332211, 0]
        );
        assert!(raw.argb_pixels[64..].iter().all(|&c| c == 0));

        let composed = doc.decode_frame(1, 0).expect("decode frame 1");
        assert_eq!(
            &composed.argb_pixels[30..34],
            &[0xFF776655, 0xFF332211, 0xFF332211, 0]
        );

        let variant1 = doc.decode_frame(1, 1).expect("decode frame 1 variant 1");
        assert_eq!(
            &variant1.argb_pixels[30..34],
            &[0xFF998877, 0xFFEFCDAB, 0xFFEFCDAB, 0]
        );

        assert!(matches!(
            doc.decode_tile(999, 0),
            Err(AptError::TileIndexOutOfRange { .. })
        ));
        assert!(matches!(
            doc.decode_frame(2, 0),
            Err(AptError::FrameIndexOutOfRange { .. })
        ));
        assert!(matches!(
            doc.decode_frame(0, 2),
            Err(AptError::PaletteVariantOutOfRange { .. })
        ));
    }
}

#[test]
fn compressed_row_gap_is_measured_in_four_pixels_and_uses_full_diamond_center() {
    let mut bytes = make_fixture(3);
    let blob = blob_offset(&bytes);

    put_u32_le(&mut bytes, blob + 1032, 0xFFFFFFFD);
    put_u32_le(&mut bytes, blob + 1036, 2);
    put_u16_le(&mut bytes, blob + 1032 + 10, 0x800 | 64);

    let doc = AptDocument::parse(&bytes).expect("parse modified fixture");
    let tile = doc.decode_tile(1, 0).expect("decode compressed tile");

    assert_eq!(tile.argb_pixels[64 + 31], 0);
    assert_eq!(tile.argb_pixels[64 + 32], 0xFF776655);
    assert_eq!(tile.argb_pixels[64 + 33], 0xFF332211);
}

#[test]
fn malformed_counts_and_tables_are_rejected() {
    let cases = [
        (0usize, 0u32),
        (8, u32::MAX),
        (12, 108),
        (80, 0),
        (84, u32::MAX),
        (92, 2049),
        (236, 65),
    ];

    for (offset, value) in cases {
        let mut bytes = make_fixture(3);
        put_u32_le(&mut bytes, offset, value);
        assert!(
            AptDocument::parse(&bytes).is_err(),
            "expected error for offset {offset} value {value}"
        );
    }
}

#[test]
fn truncation_bad_references_and_bad_pixel_rows_are_rejected() {
    let bytes = make_fixture(3);

    for length in 0..bytes.len() {
        assert!(
            AptDocument::parse(&bytes[..length]).is_err(),
            "expected truncation error for length {length}"
        );
    }

    let blob = blob_offset(&bytes);
    let tiles = blob - 28;
    let frames = tiles - 32;

    let mut broken = bytes.clone();
    put_u32_le(&mut broken, frames + 4, 3);
    assert!(AptDocument::parse(&broken).is_err());

    let mut broken = bytes.clone();
    put_u32_le(&mut broken, tiles, u32::MAX);
    assert!(AptDocument::parse(&broken).is_err());

    let mut broken = bytes.clone();
    broken[blob + 8] = 2;
    assert!(matches!(
        AptDocument::parse(&broken).unwrap().decode_tile(0, 0),
        Err(AptError::MissingPaletteColor { .. })
    ));

    let mut broken = bytes.clone();
    put_u16_le(&mut broken, blob + 1032 + 8, 63);
    assert!(matches!(
        AptDocument::parse(&broken).unwrap().decode_tile(1, 0),
        Err(AptError::CompressedPixelsOverlapRowTable)
    ));

    let mut broken = bytes.clone();
    put_u32_le(&mut broken, tiles + 4, 2047);
    assert!(matches!(
        AptDocument::parse(&broken).unwrap().decode_frame(0, 0),
        Err(AptError::VisiblePixelOutsideCanvas { .. })
    ));
}

#[test]
fn unsupported_layouts_are_explicit() {
    let cases = [
        (4usize, 1u32),
        (4, 4),
        (24, 32),
        (28, 15),
        (52, 16),
        (36, 0),
        (36, 4),
    ];

    for (offset, value) in cases {
        let mut bytes = make_fixture(3);
        put_u32_le(&mut bytes, offset, value);
        assert!(
            AptDocument::parse(&bytes).is_err(),
            "expected unsupported error for offset {offset} value {value}"
        );
    }
}
