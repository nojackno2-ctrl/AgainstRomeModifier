use against_rome_data::alr::*;
use against_rome_data::alr_scanlines::ScanlineError;

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

fn make_fixture(version: u32, second_frame_palette: bool) -> Vec<u8> {
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

fn put_u32_le(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn raw_palette_retains_high_byte_and_owns_words_without_argb_conversion() {
    let mut source = make_fixture(6, true);
    // Overwrite the first palette color with high byte set: 0xab405060
    put_u32_le(&mut source, 23 * 4 + 20, 0xAB405060);

    let document = AlrDocument::parse(&source).unwrap();
    let raw = document.read_raw_palette(0).unwrap();
    assert_eq!(raw[0], 0xAB405060);
    assert_eq!(raw[1], 0x102030);

    // Clear source bytes to verify parsed records own their data
    source.fill(0);
    assert_eq!(raw[0], 0xAB405060);

    let raw_v1 = document.read_raw_palette(1).unwrap();
    assert_eq!(raw_v1[0], 0xC0D0E0);

    assert_eq!(
        document.read_raw_palette(2),
        Err(AlrError::PaletteVariantOutOfRange {
            variant: 2,
            count: 2
        })
    );
}

#[test]
fn raw_container_preserves_shared_frames_palette_variants_offsets_and_pixels_and_roundtrips() {
    for version in 4..=6 {
        let source = make_fixture(version, true);
        let document = AlrDocument::parse(&source).unwrap();

        assert_eq!(document.version, version);
        assert_eq!(document.layout_columns, 2);
        assert_eq!(document.layout_rows, 2);
        assert_eq!(document.anchor_width, 12);
        assert_eq!(document.anchor_height, 18);
        assert_eq!(document.palette_variant_count, 2);
        assert_eq!(document.frame_count(), 4);

        // Frame 0 info
        let expected_info0 = AlrFrameInfo {
            offset_x: 7,
            offset_y: 11,
            width: 5,
            height: 1,
            palette_color_count: 2,
            packed_size: size(5, 1, 2),
            stored_pixel_bytes: 3,
        };
        assert_eq!(document.frames[0].info, expected_info0);
        assert!(!document.frames[0].is_shared());
        assert_eq!(document.frames[0].shared_from, None);

        // Frame 1 and 3 are shared
        assert!(document.frames[1].is_shared());
        assert_eq!(document.frames[1].shared_from, Some(0));
        assert_eq!(document.frames[1].info, document.frames[0].info);

        assert!(document.frames[3].is_shared());
        assert_eq!(document.frames[3].shared_from, Some(1));
        assert_eq!(document.frames[3].info, document.frames[1].info);

        // Decoded ARGB pixels match C# exact values:
        // [0, 0xFF302010, 0, 0xFF302010, 0]
        let pixels0 = document.decode_frame_argb(0, 0).unwrap();
        assert_eq!(pixels0, vec![0, 0xFF302010, 0, 0xFF302010, 0]);
        assert_eq!(document.decode_frame_argb(1, 0).unwrap(), pixels0);
        assert_eq!(document.decode_frame_argb(3, 0).unwrap(), pixels0);

        // Variant 1: [0, 0xFFB0A090, 0, 0xFFB0A090, 0]
        let pixels0_v1 = document.decode_frame_argb(0, 1).unwrap();
        assert_eq!(pixels0_v1, vec![0, 0xFFB0A090, 0, 0xFFB0A090, 0]);

        // Frame 2 contains a different local palette, but native helper 0x4E48F0 selects frame 0's palette
        let pixels2 = document.decode_frame_argb(2, 0).unwrap();
        assert_eq!(pixels2, vec![0xFF302010, 0xFF605040]);
        let pixels2_v1 = document.decode_frame_argb(2, 1).unwrap();
        assert_eq!(pixels2_v1, vec![0xFFB0A090, 0xFFE0D0C0]);

        // Out of bounds checks
        assert_eq!(
            document.decode_frame_argb(4, 0),
            Err(AlrError::FrameIndexOutOfRange { index: 4, count: 4 })
        );
        assert_eq!(
            document.decode_frame_argb(0, 2),
            Err(AlrError::PaletteVariantOutOfRange { variant: 2, count: 2 })
        );

        // Indexed sample decoding
        let indexed0 = document.decode_frame_indexed(0).unwrap();
        let indexed1 = document.decode_frame_indexed(1).unwrap();
        assert_eq!(indexed0, indexed1);
        assert_eq!(indexed0.len(), 5);

        // Exact binary roundtrip!
        let encoded = document.encode();
        assert_eq!(encoded, source, "Roundtrip bytes must match source exactly for version {version}");

        let reparsed = AlrDocument::parse(&encoded).unwrap();
        assert_eq!(reparsed, document, "Reparsed document must equal original");
    }
}

#[test]
fn frame_without_local_palette_uses_first_record_and_trailing_metadata_is_preserved() {
    let mut source = make_fixture(6, false);
    // Append an opaque trailing IFOM block
    source.extend_from_slice(&[0x49, 0x46, 0x4F, 0x4D, 0, 0, 0, 0]);

    let document = AlrDocument::parse(&source).unwrap();
    assert_eq!(document.frames[2].info.palette_color_count, 0);
    assert_eq!(
        document.decode_frame_argb(2, 0).unwrap(),
        vec![0xFF302010, 0xFF605040]
    );
    assert_eq!(
        document.decode_frame_argb(2, 1).unwrap(),
        vec![0xFFB0A090, 0xFFE0D0C0]
    );

    // Trailing bytes preserved
    assert_eq!(document.trailing_bytes, vec![0x49, 0x46, 0x4F, 0x4D, 0, 0, 0, 0]);

    // Exact binary roundtrip including trailing metadata
    let encoded = document.encode();
    assert_eq!(encoded, source);
    let reparsed = AlrDocument::parse(&encoded).unwrap();
    assert_eq!(reparsed, document);
}

#[test]
fn serialized_two_run_offsets_and_blank_frames_are_valid_and_roundtrip() {
    let mut header_words: [u32; 23] = [
        0x41524C41, 6, 0x100, 2, 8, 0x200, 2, 2, 2, 0x300, 0x401, 0x402, 0x403, 0x404, 0x405,
        12, 18, 0x501, 0x502, 0x503, 0x504, 0x601, 0x602,
    ];
    header_words[3] = 2; // 2 frames

    let mut buf = Vec::new();
    for w in header_words {
        buf.extend_from_slice(&w.to_le_bytes());
    }

    // Frame 0: Two-run scanline
    record(
        &mut buf,
        0,
        0,
        6,
        &[0, 0x102030, 0, 0x90A0B0],
        &[1, 2, 1, 0, 1],
        &[0x100000, 5],
    );

    // Frame 1: Blank frame
    record(&mut buf, 0, 0, 0, &[], &[], &[0, 0]);

    let document = AlrDocument::parse(&buf).unwrap();
    assert_eq!(document.frame_count(), 2);
    assert_eq!(
        document.decode_frame_argb(0, 0).unwrap(),
        vec![0xFF302010, 0, 0, 0, 0xFF302010, 0]
    );
    assert_eq!(
        document.decode_frame_argb(0, 1).unwrap(),
        vec![0xFFB0A090, 0, 0, 0, 0xFFB0A090, 0]
    );

    // Blank frame has 0x0 pixels
    assert_eq!(document.decode_frame_argb(1, 0).unwrap(), Vec::<u32>::new());

    // Roundtrip
    let encoded = document.encode();
    assert_eq!(encoded, buf);
    let reparsed = AlrDocument::parse(&encoded).unwrap();
    assert_eq!(reparsed, document);
}

#[test]
fn every_truncated_prefix_of_a_complete_frame_table_is_rejected() {
    let source = make_fixture(6, true);
    for len in 0..source.len() {
        let prefix = &source[..len];
        let result = AlrDocument::parse(prefix);
        assert!(
            result.is_err(),
            "Prefix of length {len}/{} should be rejected",
            source.len()
        );
    }
    assert_eq!(AlrDocument::parse(&source).unwrap().frame_count(), 4);
}

#[test]
fn invalid_header_or_frame_reference_is_rejected() {
    let source = make_fixture(6, true);

    // Bad signature
    let mut bad_sig = source.clone();
    put_u32_le(&mut bad_sig, 0, 0);
    assert_eq!(
        AlrDocument::parse(&bad_sig),
        Err(AlrError::InvalidSignature(0))
    );

    // Unsupported version 3
    let mut bad_ver = source.clone();
    put_u32_le(&mut bad_ver, 4, 3);
    assert_eq!(
        AlrDocument::parse(&bad_ver),
        Err(AlrError::UnsupportedVersion(3))
    );

    // Unsupported version 7
    let mut bad_ver7 = source.clone();
    put_u32_le(&mut bad_ver7, 4, 7);
    assert_eq!(
        AlrDocument::parse(&bad_ver7),
        Err(AlrError::UnsupportedVersion(7))
    );

    // Unsupported depth 24
    let mut bad_depth = source.clone();
    put_u32_le(&mut bad_depth, 16, 24);
    assert_eq!(
        AlrDocument::parse(&bad_depth),
        Err(AlrError::UnsupportedDepth(24))
    );

    // Frame count zero
    let mut zero_count = source.clone();
    put_u32_le(&mut zero_count, 12, 0);
    assert_eq!(
        AlrDocument::parse(&zero_count),
        Err(AlrError::InvalidFrameCount(0))
    );

    // Frame count overflow
    let mut max_count = source.clone();
    put_u32_le(&mut max_count, 12, u32::MAX);
    assert_eq!(
        AlrDocument::parse(&max_count),
        Err(AlrError::InvalidFrameCount(u32::MAX))
    );

    // Palette variants zero
    let mut zero_vars = source.clone();
    put_u32_le(&mut zero_vars, 32, 0);
    assert_eq!(
        AlrDocument::parse(&zero_vars),
        Err(AlrError::InvalidPaletteVariants(0))
    );

    // Palette variants overflow
    let mut max_vars = source.clone();
    put_u32_le(&mut max_vars, 32, u32::MAX);
    assert!(matches!(
        AlrDocument::parse(&max_vars),
        Err(AlrError::InvalidPaletteVariants(u32::MAX))
    ));

    // First frame references itself (reference = 0 at byte 92)
    let mut self_ref = source.clone();
    put_u32_le(&mut self_ref, 92, 0);
    assert_eq!(
        AlrDocument::parse(&self_ref),
        Err(AlrError::SharedFrameInvalidReference {
            index: 0,
            reference: 0
        })
    );

    // Invalid row table position / table offset mismatch (byte 96)
    let mut bad_table_offset = source.clone();
    put_u32_le(&mut bad_table_offset, 96, 0x1234);
    assert!(matches!(
        AlrDocument::parse(&bad_table_offset),
        Err(AlrError::InvalidTableOffset { .. })
    ));

    // Forward shared reference (frame 1 at byte 140 references frame 3)
    let mut fwd_ref = source.clone();
    put_u32_le(&mut fwd_ref, 140, 3);
    assert_eq!(
        AlrDocument::parse(&fwd_ref),
        Err(AlrError::SharedFrameInvalidReference {
            index: 1,
            reference: 3
        })
    );
}

#[test]
fn corrupt_row_or_palette_data_fails_safely_on_decode() {
    let mut source = make_fixture(6, true);

    // Palette index outside the first frame palette
    source[128] = 2; // Palette has only 2 colors (indices 0 and 1)
    let doc = AlrDocument::parse(&source).unwrap();
    assert_eq!(
        doc.decode_frame_argb(0, 0),
        Err(AlrError::Scanline(ScanlineError::Index))
    );

    // Backwards pixel offsets in row table
    let mut source_offsets = make_fixture(6, true);
    put_u32_le(&mut source_offsets, 132, 2);
    put_u32_le(&mut source_offsets, 136, 1);
    let doc_offsets = AlrDocument::parse(&source_offsets).unwrap();
    assert_eq!(
        doc_offsets.decode_frame_argb(0, 0),
        Err(AlrError::Scanline(ScanlineError::RowOffsets))
    );
}

#[test]
fn source_header_bridge_connects_to_metadata_normalization() {
    let source = make_fixture(6, true);
    let doc = AlrDocument::parse(&source).unwrap();
    let meta = doc.source_header().unwrap();
    assert_eq!(meta.version(), 6);
    assert_eq!(meta.frame_count(), 4);
    assert_eq!(meta.depth(), 8);
    assert_eq!(meta.palette_variants(), 2);
    assert_eq!(meta.raw_columns(), 2);
    assert_eq!(meta.raw_rows(), 2);
}
