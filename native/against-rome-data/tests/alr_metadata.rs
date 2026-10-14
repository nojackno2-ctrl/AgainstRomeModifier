use against_rome_data::alr_metadata::*;

fn words(version: u32, count: u32, a: u32, b: u32) -> [u32; HEADER_WORDS] {
    let mut h = [0; HEADER_WORDS];
    h[0] = 0x4152_4c41;
    h[1] = version;
    h[3] = count;
    h[4] = 8;
    h[6] = a;
    h[7] = b;
    h[8] = 1;
    h
}
fn bytes(h: [u32; HEADER_WORDS]) -> Vec<u8> {
    let mut data: Vec<u8> = h.iter().flat_map(|v| v.to_le_bytes()).collect();
    data.resize(HEADER_BYTES + h[3] as usize * 4, 0);
    data
}
fn metadata(count: u32, a: u32, b: u32) -> LoadedHeaderMetadata {
    SourceHeaderMetadata::parse(&bytes(words(6, count, a, b)))
        .unwrap()
        .loaded_metadata(Normalization::Enabled)
        .unwrap()
}

#[test]
fn versions_4_5_6_preserve_all_source_words_and_read_order() {
    for version in 4..=6 {
        let mut h = words(version, 12, 3, 4);
        for index in [2, 5, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22] {
            h[index] = 0xa000_0000 + index as u32;
        }
        let source = SourceHeaderMetadata::parse(&bytes(h)).unwrap();
        assert_eq!(source.raw_words(), &h);
        assert_eq!(
            (source.version(), source.depth(), source.palette_variants()),
            (version, 8, 1)
        );
        let loaded = source.loaded_metadata(Normalization::Enabled).unwrap();
        assert_eq!((loaded.a(), loaded.b()), (3, 4));
        assert_eq!(loaded.source(), &source);
        assert_eq!(source.raw_words(), &h);
        assert_eq!(loaded.changes(), NormalizationChanges::default());
    }
}

#[test]
fn zero_axes_are_independently_replaced() {
    for (a, b, expected) in [(0, 0, (1, 1)), (0, 3, (1, 3)), (3, 0, (3, 1))] {
        let m = metadata(12, a, b);
        assert_eq!((m.a(), m.b()), expected);
        assert_eq!((m.source().raw_columns(), m.source().raw_rows()), (a, b));
        assert_eq!(
            m.changes(),
            NormalizationChanges {
                zero_a_to_one: a == 0,
                zero_b_to_one: b == 0,
                collapsed_to_frame_count: false,
            }
        );
    }
    let m = metadata(2, 0, 3);
    assert_eq!((m.a(), m.b()), (2, 1));
    assert!(m.changes().zero_a_to_one && m.changes().collapsed_to_frame_count);
}

#[test]
fn product_less_equal_greater_than_count() {
    for (count, expected, groups, collapsed) in [
        (24, (3, 4), (2, 0), false),
        (12, (3, 4), (1, 0), false),
        (11, (11, 1), (1, 0), true),
    ] {
        let m = metadata(count, 3, 4);
        assert_eq!((m.a(), m.b()), expected);
        let g = m.groups().unwrap();
        assert_eq!((g.whole_groups, g.remainder_frames), groups);
        assert_eq!(m.changes().collapsed_to_frame_count, collapsed);
    }
}

#[test]
fn nondivisible_groups_are_observations_not_rejected_or_collapsed() {
    let m = metadata(25, 3, 4);
    assert_eq!((m.a(), m.b()), (3, 4));
    assert_eq!(
        m.groups(),
        Some(GroupObservation {
            layout_product: 12,
            whole_groups: 2,
            remainder_frames: 1,
        })
    );
}

#[test]
fn disabled_flag_preserves_zero_and_excess_axes() {
    for (a, b) in [(0, 0), (0, 3), (3, 0), (3, 4)] {
        let source = SourceHeaderMetadata::parse(&bytes(words(6, 2, a, b))).unwrap();
        let m = source.loaded_metadata(Normalization::Disabled).unwrap();
        assert_eq!((m.a(), m.b()), (a, b));
        assert_eq!(m.normalization(), Normalization::Disabled);
        assert_eq!(m.changes(), NormalizationChanges::default());
        if a == 0 || b == 0 {
            assert_eq!(m.groups(), None);
        } else {
            assert_eq!(m.groups().unwrap().whole_groups, 0);
        }
    }
}

#[test]
fn unsigned_comparison_does_not_treat_high_bit_as_negative() {
    let m = metadata(12, 0x8000_0000, 1);
    assert_eq!((m.a(), m.b()), (12, 1));
    assert!(m.changes().collapsed_to_frame_count);
    let m = metadata(12, u32::MAX, 1);
    assert_eq!((m.a(), m.b()), (12, 1));
}

#[test]
fn overflow_is_explicit_even_if_native_wrapped_product_would_fit() {
    for (a, b) in [(65536, 65536), (u32::MAX, 2), (0x8000_0001, 2)] {
        let source = SourceHeaderMetadata::parse(&bytes(words(6, 12, a, b))).unwrap();
        for mode in [Normalization::Enabled, Normalization::Disabled] {
            assert_eq!(
                source.loaded_metadata(mode),
                Err(MetadataError::LayoutProductOverflow { a, b })
            );
        }
    }
}

#[test]
fn every_partial_header_and_missing_reference_extent_is_rejected() {
    let data = bytes(words(6, 12, 3, 4));
    for end in 0..HEADER_BYTES {
        assert_eq!(
            SourceHeaderMetadata::parse(&data[..end]),
            Err(MetadataError::TruncatedHeader { available: end })
        );
    }
    for end in HEADER_BYTES..data.len() {
        assert_eq!(
            SourceHeaderMetadata::parse(&data[..end]),
            Err(MetadataError::TruncatedFrameReferences {
                required: data.len() as u64,
                available: end as u64,
            })
        );
    }
}

#[test]
fn invalid_signature_depth_and_versions_are_explicit() {
    let mut h = words(6, 12, 3, 4);
    h[0] = 0;
    assert_eq!(
        SourceHeaderMetadata::parse(&bytes(h)),
        Err(MetadataError::InvalidSignature(0))
    );
    for v in [0, 1, 2, 3, 7, u32::MAX] {
        assert_eq!(
            SourceHeaderMetadata::parse(&bytes(words(v, 12, 3, 4))),
            Err(MetadataError::UnsupportedVersion(v))
        );
    }
    for depth in [0, 1, 16, 24, 32, u32::MAX] {
        let mut h = words(6, 12, 3, 4);
        h[4] = depth;
        assert_eq!(
            SourceHeaderMetadata::parse(&bytes(h)),
            Err(MetadataError::UnsupportedDepth(depth))
        );
    }
}

#[test]
fn zero_and_excess_count_palette_bounds_and_huge_extents() {
    for count in [0, MAX_FRAME_COUNT + 1, u32::MAX] {
        assert_eq!(
            SourceHeaderMetadata::from_header_words(words(6, count, 1, 1), u64::MAX),
            Err(MetadataError::InvalidFrameCount(count))
        );
    }
    for variants in [0, MAX_PALETTE_VARIANTS + 1, u32::MAX] {
        let mut h = words(6, 1, 1, 1);
        h[8] = variants;
        assert_eq!(
            SourceHeaderMetadata::from_header_words(h, u64::MAX),
            Err(MetadataError::InvalidPaletteVariants(variants))
        );
    }
    let mut h = words(6, MAX_FRAME_COUNT, 1, 1);
    h[8] = MAX_PALETTE_VARIANTS;
    assert!(SourceHeaderMetadata::from_header_words(h, u64::MAX).is_ok());
}

#[test]
fn unloaded_defaults_never_become_parsed_assets() {
    let m = metadata(12, 3, 4);
    for id in [i32::MIN, -1, 3000, i32::MAX] {
        assert_eq!(
            observe_native_query(id, Some(&m)),
            NativeQueryObservation::UnloadedDefault {
                reason: UnloadedReason::InvalidId,
                a: 1,
                b: 1,
            }
        );
    }
    for id in [0, 2999] {
        assert_eq!(
            observe_native_query(id, None),
            NativeQueryObservation::UnloadedDefault {
                reason: UnloadedReason::NoLoadedMetadata,
                a: 1,
                b: 1,
            }
        );
        assert_eq!(
            observe_native_query(id, Some(&m)),
            NativeQueryObservation::HeaderMetadata(&m)
        );
    }
}

/// PowerShell flattens the existing JSON (no game reads) to an ignored unique TSV:
/// member<TAB>length<TAB>23 comma-separated header_words_raw<TAB>expected A,B,groups,remainder.
#[test]
#[ignore = "requires ALR_METADATA_AUDIT_TSV from the existing 68-member inventory"]
fn installed_inventory_header_audit() {
    let path = std::env::var("ALR_METADATA_AUDIT_TSV").expect("audit TSV path");
    let input = std::fs::read_to_string(path).unwrap();
    let mut members = std::collections::BTreeSet::new();
    let mut totals = (0u32, 0u32, 0u32, 0u32, 0u32);
    for line in input.lines() {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 4);
        assert!(members.insert(fields[0].to_owned()));
        let length: u64 = fields[1].parse().unwrap();
        let h: [u32; HEADER_WORDS] = fields[2]
            .split(',')
            .map(|v| v.parse().unwrap())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let expected: Vec<u32> = fields[3].split(',').map(|v| v.parse().unwrap()).collect();
        let source = SourceHeaderMetadata::from_header_words(h, length).unwrap();
        assert_eq!(source.raw_words(), &h);
        let m = source.loaded_metadata(Normalization::Enabled).unwrap();
        let g = m.groups().unwrap();
        assert_eq!(
            vec![m.a(), m.b(), g.whole_groups, g.remainder_frames],
            expected,
            "{}",
            fields[0]
        );
        totals.0 += source.frame_count();
        totals.1 += u32::from(m.changes() != NormalizationChanges::default());
        totals.2 += u32::from(g.whole_groups > 1);
        totals.3 += u32::from(g.remainder_frames != 0);
        totals.4 += u32::from(source.version() == 6);
    }
    assert_eq!(members.len(), 68);
    assert_eq!(totals.0, 11345);
    println!("inventory audit: members={} frames={} normalized_changes={} multi_groups={} nondivisible={} v6={}", members.len(), totals.0, totals.1, totals.2, totals.3, totals.4);
}
