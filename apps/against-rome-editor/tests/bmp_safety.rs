use against_rome_editor_app::bmp::BmpImage;

#[test]
fn collision_retains_rgb_average_and_only_changes_edited_pixels() {
    let mut image = BmpImage::new_solid(3, 1, 0, 0, 0);
    image
        .bgr_pixels
        .copy_from_slice(&[255, 0, 0, 1, 0, 0, 3, 4, 5]);
    assert_eq!(image.extract_collision_channel(), vec![85, 0, 4]);
    image.update_collision_channel(&[85, 0, 255]);
    assert_eq!(image.bgr_pixels, vec![255, 0, 0, 1, 0, 0, 255, 255, 255]);
    let gray = BmpImage::from_collision_channel(2, 1, &[85, 1]);
    assert_eq!(gray.extract_collision_channel(), vec![85, 1]);
}

#[test]
fn top_down_metadata_padding_and_trailing_bytes_survive_unchanged() {
    let standard = BmpImage::new_solid(3, 2, 17, 33, 91).encode_bmp();
    let mut bytes = standard[..54].to_vec();
    bytes.extend_from_slice(&[7, 8, 9, 10, 11, 12]);
    bytes.extend_from_slice(&standard[54..]);
    bytes.extend_from_slice(&[99, 98, 97, 96]);
    bytes[10..14].copy_from_slice(&60u32.to_le_bytes());
    bytes[22..26].copy_from_slice(&(-2i32).to_le_bytes());
    let length = bytes.len() as u32;
    bytes[2..6].copy_from_slice(&length.to_le_bytes());
    bytes[6] = 41;
    bytes[38] = 42;
    bytes[69..72].fill(43);
    bytes[81..84].fill(44);
    let mut image = BmpImage::parse(&bytes).unwrap();
    assert_eq!(image.encode_bmp(), bytes);
    let mut h = image.extract_green_channel();
    h[4] = 77;
    image.update_green_channel(&h);
    let actual = image.encode_bmp();
    let mut expected = bytes.clone();
    expected[60 + 12 + 3 + 1] = 77;
    assert_eq!(actual, expected);
}

#[test]
fn malformed_headers_extents_and_dimensions_are_rejected_without_panic() {
    let good = BmpImage::new_solid(3, 2, 1, 2, 3).encode_bmp();
    for end in 0..good.len() {
        assert!(BmpImage::parse(&good[..end]).is_err(), "truncated at {end}");
    }
    for (offset, data) in [
        (10, 20u32.to_le_bytes()),
        (14, u32::MAX.to_le_bytes()),
        (18, i32::MAX.to_le_bytes()),
        (22, i32::MIN.to_le_bytes()),
        (22, 0u32.to_le_bytes()),
        (2, 54u32.to_le_bytes()),
        (34, 1u32.to_le_bytes()),
    ] {
        let mut bytes = good.clone();
        bytes[offset..offset + 4].copy_from_slice(&data);
        assert!(BmpImage::parse(&bytes).is_err(), "header offset {offset}");
    }
    let mut compressed = good.clone();
    compressed[30] = 1;
    assert!(BmpImage::parse(&compressed).is_err());
    let mut bad_planes = good.clone();
    bad_planes[26] = 2;
    assert!(BmpImage::parse(&bad_planes).is_err());
}
