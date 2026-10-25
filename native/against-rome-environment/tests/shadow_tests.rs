//! Shadow projection data / occlusion query tests.
//!
//! "Oracle" constants were produced by running the unmodified C# sources
//! (`NativeShadowCatalog.cs`, `NativeShadowDocument.cs`, `MapLightingShadowMap.cs`) in a
//! throwaway net8.0 console program on 2026-10-18 with the same deterministic inputs as below.

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use against_rome_environment::shadow::{
    darken_argb, parse_objdef_shadows, parse_shadow_names, LightingShadowMap, ShadowCatalog,
    ShadowDocument, ShadowError, ShadowType, ShadowWorldQuad, DECODED_LENGTH,
};

// ---------------------------------------------------------------- helpers

const BASIS: u64 = 0xcbf2_9ce4_8422_2325;

fn fnv(mut h: u64, data: &[u8]) -> u64 {
    for &b in data {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn shadows(seed: u32) -> Vec<u8> {
    let mut bytes = vec![0u8; DECODED_LENGTH];
    for (i, v) in [256i32, 256, 128, 6, 0, 16000, 5000, 12345].iter().enumerate() {
        bytes[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    let mut s = seed;
    for b in &mut bytes[32..] {
        s = s.wrapping_mul(1664525).wrapping_add(1013904223);
        *b = (s >> 24) as u8;
    }
    bytes
}

fn blank_map() -> Vec<u8> {
    let mut bytes = shadows(0);
    bytes[32..].fill(0);
    bytes
}

fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn fixture(top_down: bool) -> Vec<u8> {
    let mut bytes = vec![0u8; 1086];
    bytes[0..2].copy_from_slice(&0x4D42u16.to_le_bytes());
    put32(&mut bytes, 2, 1086);
    put32(&mut bytes, 10, 1078);
    put32(&mut bytes, 14, 40);
    put32(&mut bytes, 18, 3);
    put32(&mut bytes, 22, if top_down { (-2i32) as u32 } else { 2 });
    bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&8u16.to_le_bytes());
    put32(&mut bytes, 34, 8);
    put32(&mut bytes, 46, 256);
    for i in 0..256 {
        put32(&mut bytes, 54 + i * 4, 0xEE20_4080);
    }
    let pixels: [u8; 8] = if top_down {
        [0, 1, 128, 0xAA, 255, 64, 192, 0xBB]
    } else {
        [255, 64, 192, 0xBB, 0, 1, 128, 0xAA]
    };
    bytes[1078..].copy_from_slice(&pixels);
    bytes
}

fn class(result: &Result<ShadowDocument, ShadowError>) -> char {
    match result {
        Ok(_) => 'O',
        Err(ShadowError::InvalidData(_)) => 'I',
        Err(ShadowError::NotSupported(_)) => 'N',
        Err(other) => panic!("unexpected error class {other:?}"),
    }
}

fn is_out_of_range<T: std::fmt::Debug>(r: Result<T, ShadowError>) -> bool {
    matches!(r, Err(ShadowError::OutOfRange(_)))
}

// ------------------------------------------------------- lighting shadow map

#[test]
fn map_layout_is_slice_then_z_then_lsb_x_and_preserves_cache_metadata() {
    let mut bytes = blank_map();
    bytes[32 + 17 * 8192 + 19 * 32 + 1] = 0x81;
    let map = LightingShadowMap::parse_decoded(&bytes).unwrap();
    bytes.fill(0);
    assert!(map.is_occluded(17, 8, 19).unwrap() && map.is_occluded(17, 15, 19).unwrap());
    assert!(!map.is_occluded(17, 9, 19).unwrap() && !map.is_occluded(18, 8, 19).unwrap());
    assert_eq!(map.header()[5], 16000);
    assert_eq!(map.header()[6], 5000);
    assert_eq!(map.header()[7], 12345);
    assert!(is_out_of_range(map.is_occluded(128, 0, 0)));
    assert!(is_out_of_range(map.is_occluded(-1, 0, 0)));
    assert!(is_out_of_range(map.is_occluded(0, 256, 0)));
    assert!(is_out_of_range(map.is_occluded(0, 0, -1)));
    assert!(is_out_of_range(map.is_occluded(0, 0, 256)));
}

#[test]
fn map_temporal_interpolation_wraps_midnight_and_has_native_floor() {
    let mut bytes = blank_map();
    bytes[32] = 1;
    let map = LightingShadowMap::parse_decoded(&bytes).unwrap();
    assert_eq!(map.occlusion_at(0, 0, 0).unwrap(), 255);
    assert_eq!(map.occlusion_at(337, 0, 0).unwrap(), 128);
    assert_eq!(map.occlusion_at(675, 0, 0).unwrap(), 0);
    assert_eq!(map.occlusion_at(86399, 0, 0).unwrap(), 254);
    assert!(is_out_of_range(map.occlusion_at(86400, 0, 0)));
    assert!(is_out_of_range(map.occlusion_at(-1, 0, 0)));
}

#[test]
fn map_spatial_sampling_preserves_65535_weights_and_native_darkening() {
    let mut bytes = blank_map();
    bytes[32..].fill(255);
    let map = LightingShadowMap::parse_decoded(&bytes).unwrap();
    assert_eq!(map.sample_occlusion(0, 128, 192).unwrap(), 254);
    assert_eq!(map.sample_occlusion(0, 32767, -1).unwrap(), 254);
    assert_eq!(map.shadow_factor(0, 128, 192, 0.0).unwrap(), 1.0);
    assert_eq!(map.shadow_factor(0, 128, 192, 1.0).unwrap(), 2.0 / 256.0);
    assert!(is_out_of_range(map.shadow_factor(0, 0, 0, f32::NAN)));
    assert!(is_out_of_range(map.shadow_factor(0, 0, 0, f32::INFINITY)));
    assert!(is_out_of_range(map.shadow_factor(0, 0, 0, -0.01)));
    assert!(is_out_of_range(map.shadow_factor(0, 0, 0, 1.01)));
    assert!(is_out_of_range(map.sample_occlusion(0, i32::MAX, 0)));
    assert!(is_out_of_range(map.sample_occlusion(0, 0, i32::MIN)));
    assert!(is_out_of_range(map.sample_occlusion(0, 32768, 0)));
    assert!(is_out_of_range(map.sample_occlusion(0, -32769, 0)));
    assert!(is_out_of_range(map.sample_occlusion(86400, 0, 0)));

    let mut bytes = blank_map();
    bytes[32] = 1;
    bytes[32 + 32] = 1;
    let map = LightingShadowMap::parse_decoded(&bytes).unwrap();
    assert_eq!(map.sample_occlusion(0, 32, 0).unwrap(), 127);
}

#[test]
fn map_empty_or_clear_data_never_occludes_and_out_of_map_clamps() {
    let map = LightingShadowMap::parse_decoded(&blank_map()).unwrap();
    for s in [0, 337, 86399] {
        for w in [-32768, -1, 0, 16384, 32767] {
            assert_eq!(map.sample_occlusion(s, w, w).unwrap(), 0);
            assert_eq!(map.shadow_factor(s, w, w, 1.0).unwrap(), 1.0);
        }
    }
    // Fully opaque map: coordinates left/above the map clamp to the edge cell instead of failing.
    let mut full = blank_map();
    full[32..].fill(255);
    let full = LightingShadowMap::parse_decoded(&full).unwrap();
    assert_eq!(full.sample_occlusion(0, -32768, -32768).unwrap(), 254);
    assert_eq!(full.sample_occlusion(0, 32767, 32767).unwrap(), 254);
}

#[test]
fn map_rejects_extent_and_unsupported_dimensions() {
    let bytes = shadows(1);
    assert!(matches!(
        LightingShadowMap::parse_decoded(&bytes[..bytes.len() - 1]),
        Err(ShadowError::InvalidData(_))
    ));
    let mut longer = bytes.clone();
    longer.push(0);
    assert!(matches!(LightingShadowMap::parse_decoded(&longer), Err(ShadowError::InvalidData(_))));
    assert!(matches!(LightingShadowMap::parse_decoded(&[]), Err(ShadowError::InvalidData(_))));
    for (slot, value) in [(0usize, 257i32), (1, 255), (2, 64), (2, 129)] {
        let mut bad = bytes.clone();
        bad[slot * 4..slot * 4 + 4].copy_from_slice(&value.to_le_bytes());
        assert!(
            matches!(LightingShadowMap::parse_decoded(&bad), Err(ShadowError::NotSupported(_))),
            "header slot {slot} = {value}"
        );
    }
}

#[test]
fn map_matches_csharp_oracle_exhaustively() {
    let map = LightingShadowMap::parse_decoded(&shadows(12345)).unwrap();
    let seconds = [0, 1, 337, 674, 675, 676, 12345, 43200, 85724, 85725, 86399];
    let world = [
        -32768, -1000, -64, -1, 0, 1, 31, 32, 63, 64, 100, 4095, 8191, 16320, 16383, 16384, 32000, 32767,
    ];
    let cells = [0, 1, 7, 8, 100, 254, 255];

    let (mut h_occ, mut n) = (BASIS, 0);
    for &s in &seconds {
        for &x in &cells {
            for &z in &cells {
                h_occ = fnv(h_occ, &map.occlusion_at(s, x, z).unwrap().to_le_bytes());
                n += 1;
            }
        }
    }
    assert_eq!((n, h_occ), (539, 0x7dad_1448_d4dd_a80c));

    let (mut h_sample, mut n) = (BASIS, 0);
    for &s in &seconds {
        for &wx in &world {
            for &wz in &world {
                h_sample = fnv(h_sample, &map.sample_occlusion(s, wx, wz).unwrap().to_le_bytes());
                n += 1;
            }
        }
    }
    assert_eq!((n, h_sample), (3564, 0xb45e_676c_4daf_9564));

    let (mut h_factor, mut n) = (BASIS, 0);
    for &s in &seconds {
        for &wx in &world {
            for &wz in &world {
                for st in [0.0f32, 0.25, 0.5, 1.0] {
                    let v = map.shadow_factor(s, wx, wz, st).unwrap();
                    h_factor = fnv(h_factor, &v.to_bits().to_le_bytes());
                    n += 1;
                }
            }
        }
    }
    assert_eq!((n, h_factor), (14256, 0x1518_5edf_fd90_9a49));

    let (mut h_bit, mut n) = (BASIS, 0);
    for slice in (0..128).step_by(5) {
        for &x in &cells {
            for &z in &cells {
                h_bit = fnv(h_bit, &[u8::from(map.is_occluded(slice, x, z).unwrap())]);
                n += 1;
            }
        }
    }
    assert_eq!((n, h_bit), (1274, 0x5f96_c84b_c920_196a));

    // (seconds, world_x, world_z, sample, cell occlusion, factor at strength 0.5)
    let explicit: [(i32, i32, i32, i32, i32, &str); 6] = [
        (0, 128, 192, 254, 255, "0.50390625"),
        (337, 100, 100, 168, 128, "0.671875"),
        (12345, -1000, 16384, 113, 182, "0.7792969"),
        (43200, 16383, 16383, 254, 255, "0.50390625"),
        (86399, 32767, 32767, 0, 0, "1"),
        (675, 31, 63, 131, 0, "0.7441406"),
    ];
    for (s, wx, wz, sample, cell, factor) in explicit {
        assert_eq!(map.sample_occlusion(s, wx, wz).unwrap(), sample, "sample {s} {wx} {wz}");
        let (cx, cz) = ((wx >> 6).clamp(0, 255), (wz >> 6).clamp(0, 255));
        assert_eq!(map.occlusion_at(s, cx, cz).unwrap(), cell);
        assert_eq!(map.shadow_factor(s, wx, wz, 0.5).unwrap(), factor.parse::<f32>().unwrap());
    }
}

// ------------------------------------------------------------------ BMP document

#[test]
fn document_padded_rows_are_top_down_index_strengths_independent_of_palette() {
    for top_down in [false, true] {
        let mut source = fixture(top_down);
        let document = ShadowDocument::parse(&source).unwrap();
        assert_eq!((document.width(), document.height()), (3, 2));
        assert_eq!(document.stored_stride(), 4);
        assert_eq!(document.is_top_down(), top_down);
        assert_eq!(document.palette_color_count(), 256);
        assert_eq!(document.frame_count(), 1);
        assert_eq!(document.trailing_byte_count(), 0);
        source.fill(0);
        let frame = document.decode_frame(0).unwrap();
        assert_eq!(frame.alpha_mask(), [0, 1, 128, 255, 64, 192]);
        assert_eq!(
            frame.argb_pixels(),
            [0, 0x0100_0000, 0x8000_0000, 0xFE00_0000, 0x4000_0000, 0xBF00_0000]
        );
        assert_eq!((frame.width(), frame.height()), (3, 2));
        assert_eq!(document.strength_at(2, 1), Some(192));
        assert_eq!(document.strength_at(3, 0), None);
        assert_eq!(document.strength_at(0, 2), None);
        assert!(is_out_of_range(document.decode_frame(-1)));
        assert!(is_out_of_range(document.decode_frame(1)));
    }
}

#[test]
fn document_matches_csharp_oracle_for_header_mutations_and_prefixes() {
    let values: [u32; 16] = [
        0, 1, 2, 8, 12, 40, 54, 255, 256, 257, 1077, 1078, 1086, 0x8000_0000, i32::MAX as u32, u32::MAX,
    ];
    let offsets = [2usize, 6, 10, 14, 18, 22, 26, 28, 30, 34, 46, 50];
    let mut classes = String::new();
    for &off in &offsets {
        for &v in &values {
            let mut d = fixture(false);
            put32(&mut d, off, v);
            classes.push(class(&ShadowDocument::parse(&d)));
        }
    }
    assert_eq!(
        classes,
        "IIIIIIIIIIIIOIIIOIIIIIIIIIIIIIIIIIIIIIIIIIIOIIIINNNNNONNNNNNNNNNIOOIIIIIIIIIIIIIIIOIIIIIIIIIIIIIINIIIIIIIIIIIIIINNNONNNNNNNNNNNNONNNNNNNNNNNNNNNOIIOIIIIIIIIIIIIOIIIIIIIOIIIIIIIOOOOOOOOOIIIIIII"
    );

    let fx = fixture(false);
    let mut h = BASIS;
    for len in 0..fx.len() {
        let mut p = fx[..len].to_vec();
        h = fnv(h, &[class(&ShadowDocument::parse(&p)) as u8]);
        if len >= 6 {
            put32(&mut p, 2, len as u32);
            h = fnv(h, &[class(&ShadowDocument::parse(&p)) as u8]);
        }
    }
    assert_eq!(h, 0x1f34_d52b_c4f5_469b);
}

#[test]
fn document_every_prefix_is_rejected_as_invalid_data() {
    let fx = fixture(false);
    for len in 0..fx.len() {
        let mut p = fx[..len].to_vec();
        assert!(matches!(ShadowDocument::parse(&p), Err(ShadowError::InvalidData(_))), "len {len}");
        if len >= 6 {
            put32(&mut p, 2, len as u32);
            assert!(matches!(ShadowDocument::parse(&p), Err(ShadowError::InvalidData(_))), "len {len} adjusted");
        }
    }
}

#[test]
fn document_zero_colors_zero_image_size_and_declared_trailer_match_real_bmps() {
    let mut source = fixture(false);
    source.extend_from_slice(&[0xAA, 0xBB]);
    let total = source.len() as u32;
    put32(&mut source, 2, total);
    put32(&mut source, 34, 0);
    put32(&mut source, 46, 0);
    let document = ShadowDocument::parse(&source).unwrap();
    assert_eq!(document.palette_color_count(), 256);
    assert_eq!(document.trailing_byte_count(), 2);
    assert_eq!(document.mask(), [0, 1, 128, 255, 64, 192]);

    // An extra byte with an unchanged declared size is rejected.
    let mut extra = fixture(false);
    extra.push(0);
    assert!(matches!(ShadowDocument::parse(&extra), Err(ShadowError::InvalidData(_))));
    // Index 128 cannot reference a two-entry palette.
    let mut small = fixture(false);
    put32(&mut small, 46, 2);
    assert!(matches!(ShadowDocument::parse(&small), Err(ShadowError::InvalidData(_))));
}

#[test]
fn document_unsupported_variants_are_explicit() {
    for (offset, value) in [(14usize, 12u32), (14, 108), (30, 1), (30, 2)] {
        let mut d = fixture(false);
        put32(&mut d, offset, value);
        assert!(matches!(ShadowDocument::parse(&d), Err(ShadowError::NotSupported(_))));
    }
    for depth in [1u16, 4, 24, 32] {
        let mut d = fixture(false);
        d[28..30].copy_from_slice(&depth.to_le_bytes());
        assert!(matches!(ShadowDocument::parse(&d), Err(ShadowError::NotSupported(_))));
    }
    let mut d = fixture(false);
    d[0] = 0;
    assert!(matches!(ShadowDocument::parse(&d), Err(ShadowError::InvalidData(_))));
    let mut d = fixture(false);
    d[26] = 2;
    assert!(matches!(ShadowDocument::parse(&d), Err(ShadowError::InvalidData(_))));
}

#[test]
fn document_parse_never_panics_on_corrupted_input() {
    let base = fixture(true);
    let mut state = 0x1234_5678u32;
    let mut next = move || {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        state
    };
    for _ in 0..20_000 {
        let mut d = base.clone();
        for _ in 0..(1 + next() % 4) {
            let at = (next() as usize) % 60;
            d[at] = (next() >> 24) as u8;
        }
        if next() % 5 == 0 {
            d.truncate((next() as usize) % (d.len() + 1));
        }
        let _ = ShadowDocument::parse(&d);
    }
}

#[test]
fn darken_matches_csharp_oracle_and_native_formula() {
    assert_eq!(darken_argb(0x7F80_4020, 0), 0x7F80_4020);
    assert_eq!(darken_argb(0x7F80_4020, 128), 0x7F40_2010);
    assert_eq!(darken_argb(0x7FFF_FFFF, 255), 0x7F00_0000);
    assert_eq!(darken_argb(0x7FFF_FFFF, 1), 0x7FFE_FEFE);
    for strength in 0..=255u32 {
        assert_eq!(darken_argb(0xFFFF_FFFF, strength as u8) & 255, 255 * (256 - strength) / 256);
    }
    let mut h = BASIS;
    for dst in [0x7F80_4020u32, 0xFFFF_FFFF, 0x0001_0203, 0xFE10_2030] {
        for st in 0..=255u8 {
            h = fnv(h, &darken_argb(dst, st).to_le_bytes());
        }
    }
    assert_eq!(h, 0x33cc_2183_8958_daca);
}

// ----------------------------------------------------------------------- catalog

const INI: &str = "garbage,line.bmp\r\n[Other]\r\n0007,ignored.bmp\r\n[shadownames]\r\n; comment\r\n0000,small_round_shadow.bmp\r\n 0001 , schuetze_shadow.bmp \r\n+2,plus.bmp\r\n0003,dup1.bmp\r\n3,dup2.bmp\r\n1999,last.bmp\r\n2000,toobig.bmp\r\n-1,neg.bmp\r\n0004,sub/dir.bmp\r\n0005,back\\slash.bmp\r\n0006,\r\n0008\r\nabc,x.bmp\r\n0009,comma,inside.bmp\r\n";

fn row(name: &str, shidx: &str, shsiz: &str, shtyp: &str, shacx: &str, shacz: &str) -> String {
    let mut r = vec!["0"; 53];
    r[3] = shacz;
    r[4] = shacx;
    r[5] = shtyp;
    r[6] = shsiz;
    r[7] = shidx;
    r[52] = name;
    r.join(",") + "\r\n"
}

fn objdef() -> String {
    format!(
        "[ObjectDefaults]\r\n;idx,a,b,shacz,shacx,shtyp,shsiz,shidx\r\n{}{}{}{}{}{}6,1,Short,1,1\r\n{}{}{}{};comment,line\r\n{}",
        row("Alpha", "0", "25", "1", "5", "-3"),
        row("beta", "1", "30", "0", "-7", "+9"),
        row("ALPHA", "1", "99", "1", "0", "0"),
        row("NoShadow", "-1", "10", "1", "0", "0"),
        row("Unmapped", "42", "10", "1", "0", "0"),
        row("BadNum", "1", "x", "1", "0", "0"),
        row("", "1", "5", "1", "0", "0"),
        row(" Spaced ", "2", " 12 ", "7", " 1 ", " 2 "),
        row("Plus", "3", "4", "1", "0", "0"),
        row("Last", "1999", "8", "0", "100", "-100"),
        row("Beta2", "1", "5", "1", "0", "0"),
    )
}

fn def_lines(defs: &[(String, against_rome_environment::shadow::ShadowDefinition)]) -> Vec<String> {
    defs.iter()
        .map(|(name, d)| {
            format!(
                "{name}|{}|{}|{}|{}|{}|{}",
                d.shadow_id, d.shadow_size, d.shadow_type as i32, d.correction_x, d.correction_z, d.texture_file_name
            )
        })
        .collect()
}

#[test]
fn parse_shadow_names_matches_csharp_oracle() {
    let names = parse_shadow_names(INI);
    let mut listed: Vec<_> = names.iter().map(|(k, v)| (*k, v.as_str())).collect();
    listed.sort();
    assert_eq!(
        listed,
        [
            (0, "small_round_shadow.bmp"),
            (1, "schuetze_shadow.bmp"),
            (2, "plus.bmp"),
            (3, "dup1.bmp"),
            (9, "comma,inside.bmp"),
            (1999, "last.bmp"),
        ]
    );
    assert!(parse_shadow_names("").is_empty());
    assert!(parse_shadow_names("0,a.bmp").is_empty(), "entries outside [ShadowNames] are ignored");
}

#[test]
fn parse_objdef_matches_csharp_oracle_including_header_relocation() {
    let names = parse_shadow_names(INI);
    let defs = parse_objdef_shadows(&objdef(), &names);
    assert_eq!(
        def_lines(&defs),
        [
            "Alpha|0|25|1|5|-3|small_round_shadow.bmp",
            "beta|1|30|0|-7|9|schuetze_shadow.bmp",
            "Spaced|2|12|0|1|2|plus.bmp",
            "Plus|3|4|1|0|0|dup1.bmp",
            "Last|1999|8|0|100|-100|last.bmp",
            "Beta2|1|5|1|0|0|schuetze_shadow.bmp",
        ]
    );

    // No header line: default columns 11/12/13/15/16 and name at 52.
    let mut cols = vec!["0".to_owned(); 53];
    cols[11] = "1".into();
    cols[12] = "30".into();
    cols[13] = "1".into();
    cols[15] = "5".into();
    cols[16] = "-3".into();
    cols[52] = "DefaultCols".into();
    let defs = parse_objdef_shadows(&cols.join(","), &names);
    assert_eq!(def_lines(&defs), ["DefaultCols|1|30|1|5|-3|schuetze_shadow.bmp"]);
    assert_eq!(defs[0].1.shadow_type, ShadowType::Circle);
    assert!(parse_objdef_shadows("", &names).is_empty());
    assert!(parse_objdef_shadows("anything", &Default::default()).is_empty());
    // Multi-byte text that is not a header must not panic on the ";idx" probe.
    assert!(parse_objdef_shadows("\u{4e2d}\u{6587}\u{6e2c}\u{8a66},1", &names).is_empty());
}

fn valid_bmp_catalog() -> ShadowCatalog {
    ShadowCatalog::from_text(&objdef(), INI, Box::new(|_| Some(fixture(false))))
}

#[test]
fn projection_matches_csharp_oracle() {
    let catalog = valid_bmp_catalog();
    assert_eq!((catalog.definition_count(), catalog.shadow_name_mapping_count()), (6, 6));
    let cases: [(&str, f64, f64, &str); 4] = [
        ("Alpha", 11392.0, 10112.0, "quad Alpha 11397 10109 25 NW=11372,10084 NE=11422,10084 SE=11422,10134 SW=11372,10134 screen=4,0.5"),
        ("BETA", -5.5, 0.25, "quad BETA -12.5 9.25 30 NW=-42.5,-20.75 NE=17.5,-20.75 SE=17.5,39.25 SW=-42.5,39.25 screen=-8,0.5"),
        ("Last", 0.0, 0.0, "quad Last 100 -100 8 NW=92,-108 NE=108,-108 SE=108,-92 SW=92,-92 screen=100,0"),
        ("Spaced", 1e9, -1e9, "quad Spaced 1000000001 -999999998 12 NW=999999989,-1000000010 NE=1000000013,-1000000010 SE=1000000013,-999999986 SW=999999989,-999999986 screen=-0.5,0.75"),
    ];
    for (object, ax, az, expected) in cases {
        let shadow = catalog.get_shadow(object).expect(object);
        assert_eq!(shadow.object_name, object);
        let q = shadow.compute_world_quad(ax, az);
        let (dx, dy) = shadow.compute_screen_offset();
        let line = format!(
            "quad {object} {} {} {} NW={},{} NE={},{} SE={},{} SW={},{} screen={dx},{dy}",
            q.center_x, q.center_z, q.half_extent, q.corner_nw.0, q.corner_nw.1, q.corner_ne.0,
            q.corner_ne.1, q.corner_se.0, q.corner_se.1, q.corner_sw.0, q.corner_sw.1
        );
        assert_eq!(line, expected);
        assert_eq!(q.corners(), [q.corner_nw, q.corner_ne, q.corner_se, q.corner_sw]);
    }
}

#[test]
fn quad_contains_is_inclusive_and_rejects_nonfinite() {
    let q = ShadowWorldQuad::from_center_and_size(100.0, -50.0, 10.0);
    assert!(q.contains(100.0, -50.0) && q.contains(110.0, -40.0) && q.contains(90.0, -60.0));
    assert!(!q.contains(110.5, -50.0) && !q.contains(100.0, -39.9));
    assert!(!q.contains(f64::NAN, -50.0) && !q.contains(100.0, f64::INFINITY));
    // Documented oracle behavior: a non-finite anchor propagates, it does not panic.
    let catalog = valid_bmp_catalog();
    let q = catalog.get_shadow("Alpha").unwrap().compute_world_quad(f64::NAN, 0.0);
    assert!(q.center_x.is_nan() && !q.contains(0.0, 0.0));
}

#[test]
fn unknown_unmapped_and_invalid_assets_resolve_to_none_without_losing_definitions() {
    let catalog = valid_bmp_catalog();
    for missing in ["NonExistentObject", "NoShadow", "Unmapped", "BadNum", "Short", ""] {
        assert!(catalog.get_shadow(missing).is_none(), "{missing}");
        assert!(catalog.try_get_definition(missing).is_none(), "{missing}");
    }
    assert_eq!(catalog.try_get_definition("PLUS").unwrap().texture_file_name, "dup1.bmp");

    let bad = ShadowCatalog::from_text(
        &objdef(),
        INI,
        Box::new(|name| match name {
            "dup1.bmp" => Some(Vec::new()),
            "schuetze_shadow.bmp" => Some(vec![1, 2, 3]),
            _ => None,
        }),
    );
    assert!(bad.get_shadow("Alpha").is_none(), "reader returned None");
    assert!(bad.get_shadow("beta").is_none(), "garbage bytes");
    assert!(bad.get_shadow("Plus").is_none(), "empty bytes");
    assert!(bad.try_get_definition("beta").is_some(), "definition survives a broken mask");
    assert!(bad.get_document("anything.bmp").is_none());
}

#[test]
fn documents_are_cached_by_case_insensitive_name_including_failures() {
    static READS: AtomicUsize = AtomicUsize::new(0);
    let catalog = ShadowCatalog::from_text(
        &objdef(),
        INI,
        Box::new(|name| {
            READS.fetch_add(1, Ordering::SeqCst);
            (name == "small_round_shadow.bmp").then(|| fixture(true))
        }),
    );
    let a = catalog.get_shadow("Alpha").unwrap();
    let b = catalog.get_shadow("alpha").unwrap();
    assert!(Arc::ptr_eq(&a.document, &b.document));
    let c = catalog.get_document("SMALL_ROUND_SHADOW.BMP").unwrap();
    assert!(Arc::ptr_eq(&a.document, &c));
    assert_eq!(READS.load(Ordering::SeqCst), 1);
    assert!(catalog.get_shadow("beta").is_none());
    assert!(catalog.get_shadow("beta").is_none());
    assert_eq!(READS.load(Ordering::SeqCst), 2, "a failed load is cached too");
}

#[test]
fn catalog_is_shareable_across_threads() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ShadowCatalog>();
    let catalog = Arc::new(valid_bmp_catalog());
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let c = Arc::clone(&catalog);
            std::thread::spawn(move || c.get_shadow("Last").unwrap().document.width())
        })
        .collect();
    for h in handles {
        assert_eq!(h.join().unwrap(), 3);
    }
}

// ------------------------------------------------------------------- open() on disk

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        static N: AtomicUsize = AtomicUsize::new(0);
        let path = crate::temp_fixture::dir("arm-env-shadow");
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, relative: &str, bytes: &[u8]) {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut cursor);
        let options = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();
    }
    cursor.into_inner()
}

/// PFIL with an all-literal LZSS body (valid for the strict decoder).
fn pfil_literal(plain: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; 64];
    out[..4].copy_from_slice(b"PFIL");
    out[16..20].copy_from_slice(&(plain.len() as u32).to_le_bytes());
    for chunk in plain.chunks(8) {
        out.push(0xFF);
        out.extend_from_slice(chunk);
    }
    out
}

const OPEN_OBJDEF: &str = "[ObjectDefaults]\r\n;idx,a,b,shacz,shacx,shtyp,shsiz,shidx\r\n";

fn open_objdef() -> String {
    format!(
        "{OPEN_OBJDEF}{}{}{}",
        row("BauGerWoh00_Wohnhaus", "283", "138", "0", "-48", "21"),
        row("ArcherUnit", "1", "30", "1", "5", "-3"),
        row("BauGerHau00_Haupthaus", "255", "277", "0", "-50", "35"),
    )
}

const OPEN_INI: &str =
    "[ShadowNames]\r\n0001,schuetze_shadow.bmp\r\n0255,shadowmap_haupthaus_1.bmp\r\n0283,shadowmap_wohnhaus_1.bmp\r\n";

#[test]
fn open_returns_none_for_missing_directory_or_required_files() {
    let missing = crate::temp_fixture::dir("arm-env-shadow-definitely-missing-dir").join("arm-env-shadow-definitely-missing-dir");
    assert!(ShadowCatalog::open(&missing).unwrap().is_none());

    let dir = TempDir::new("partial");
    assert!(ShadowCatalog::open(&dir.0).unwrap().is_none(), "empty directory");
    dir.write("SYSTEM/DATA_MP/DEFAULTS/objdef.dau", open_objdef().as_bytes());
    assert!(ShadowCatalog::open(&dir.0).unwrap().is_none(), "no shad.dat");
    dir.write("shad.dat", &zip_bytes(&[]));
    assert!(ShadowCatalog::open(&dir.0).unwrap().is_none(), "no cl_shado.ini");
    dir.write("SYSTEM/cl_shado.ini", OPEN_INI.as_bytes());
    assert!(ShadowCatalog::open(&dir.0).unwrap().is_some());

    let no_objdef = TempDir::new("noobjdef");
    no_objdef.write("shad.dat", &zip_bytes(&[]));
    no_objdef.write("cl_shado.ini", OPEN_INI.as_bytes());
    assert!(ShadowCatalog::open(&no_objdef.0).unwrap().is_none(), "no objdef");
}

#[test]
fn open_reads_the_game_install_layout_with_zip_masks() {
    let dir = TempDir::new("layout");
    dir.write("SYSTEM/DATA_MP/DEFAULTS/objdef.dau", open_objdef().as_bytes());
    dir.write("SYSTEM/cl_shado.ini", OPEN_INI.as_bytes());
    let mask = fixture(false);
    dir.write(
        "shad.dat",
        &zip_bytes(&[
            ("SYSTEM/DATA/SHADOWTEXTURE/shadowmap_wohnhaus_1.bmp", &mask),
            // Only the leaf name matches (other folder) and the name differs by case.
            ("elsewhere/Schuetze_Shadow.BMP", &fixture(true)),
        ]),
    );
    let catalog = ShadowCatalog::open(&dir.0).unwrap().unwrap();
    assert_eq!((catalog.definition_count(), catalog.shadow_name_mapping_count()), (3, 3));

    let house = catalog.get_shadow("BauGerWoh00_Wohnhaus").unwrap();
    assert_eq!(house.definition.shadow_type, ShadowType::Box);
    assert_eq!(house.document.mask(), [0, 1, 128, 255, 64, 192]);
    let archer = catalog.get_shadow("ArcherUnit").unwrap();
    assert_eq!(archer.definition.shadow_type, ShadowType::Circle);
    assert!(archer.document.is_top_down(), "leaf-name fallback picked the top-down entry");
    assert!(catalog.get_shadow("BauGerHau00_Haupthaus").is_none(), "mask absent from the archive");
    assert!(catalog.try_get_definition("BauGerHau00_Haupthaus").is_some());
}

#[test]
fn open_accepts_flat_copies_and_pfil_compressed_text_assets() {
    let dir = TempDir::new("flat");
    dir.write("objdef.dau", &pfil_literal(open_objdef().as_bytes()));
    dir.write("cl_shado.ini", &pfil_literal(OPEN_INI.as_bytes()));
    dir.write("shad.dat", &zip_bytes(&[("shadowmap_wohnhaus_1.bmp", &fixture(false))]));
    let catalog = ShadowCatalog::open(&dir.0).unwrap().unwrap();
    assert!(catalog.get_shadow("BauGerWoh00_Wohnhaus").is_some());
}

#[test]
fn open_reports_corrupt_assets_and_skips_oversized_or_invalid_masks() {
    let corrupt_zip = TempDir::new("badzip");
    corrupt_zip.write("objdef.txt", open_objdef().as_bytes());
    corrupt_zip.write("cl_shado.txt", OPEN_INI.as_bytes());
    corrupt_zip.write("shad.dat", b"this is not a zip archive");
    assert!(matches!(ShadowCatalog::open(&corrupt_zip.0), Err(ShadowError::InvalidData(_))));

    let truncated_pfil = TempDir::new("badpfil");
    let mut pfil = pfil_literal(open_objdef().as_bytes());
    pfil.truncate(pfil.len() - 3);
    truncated_pfil.write("objdef.dau", &pfil);
    truncated_pfil.write("cl_shado.ini", OPEN_INI.as_bytes());
    truncated_pfil.write("shad.dat", &zip_bytes(&[]));
    assert!(matches!(ShadowCatalog::open(&truncated_pfil.0), Err(ShadowError::InvalidData(_))));

    let junk = TempDir::new("junkmask");
    junk.write("objdef.txt", open_objdef().as_bytes());
    junk.write("cl_shado.ini", OPEN_INI.as_bytes());
    junk.write(
        "shad.dat",
        &zip_bytes(&[
            ("shadowmap_wohnhaus_1.bmp", b"BM not really"),
            ("shadowmap_haupthaus_1.bmp", &[]),
        ]),
    );
    let catalog = ShadowCatalog::open(&junk.0).unwrap().unwrap();
    assert!(catalog.get_shadow("BauGerWoh00_Wohnhaus").is_none());
    assert!(catalog.get_shadow("BauGerHau00_Haupthaus").is_none());
}

#[cfg(test)]
mod temp_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/temp_unique.rs"));
}
