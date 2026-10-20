use lighting_probe::{MapLightingBitmap, ProbeError};

pub fn build_bitmap(
    width: i32,
    height: i32,
    top_down: bool,
    pixel: impl Fn(i32, i32) -> [f32; 3],
) -> Vec<u8> {
    let stride = ((width * 3 + 3) & !3) as usize;
    let mut bytes = vec![0u8; 54 + stride * (height.abs() as usize)];
    bytes[0] = b'B';
    bytes[1] = b'M';

    let total_len = bytes.len() as u32;
    bytes[2..6].copy_from_slice(&total_len.to_le_bytes());
    bytes[10..14].copy_from_slice(&54u32.to_le_bytes());
    bytes[14..18].copy_from_slice(&40u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&width.to_le_bytes());
    let signed_height = if top_down { -height } else { height };
    bytes[22..26].copy_from_slice(&signed_height.to_le_bytes());
    bytes[26..28].copy_from_slice(&1u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&24u16.to_le_bytes());
    let extent = (stride * height.abs() as usize) as u32;
    bytes[34..38].copy_from_slice(&extent.to_le_bytes());

    let h = height.abs();
    for y in 0..h {
        for x in 0..width {
            let c = pixel(x, y);
            let stored_y = if top_down { y } else { h - 1 - y };
            let idx = 54 + (stored_y as usize) * stride + (x as usize) * 3;
            // BGR in BMP file
            bytes[idx] = c[2] as u8;
            bytes[idx + 1] = c[1] as u8;
            bytes[idx + 2] = c[0] as u8;
        }
    }
    bytes
}

#[test]
fn bitmap_decodes_bgr_padding_and_row_orientation_to_owned_rgb() {
    for top_down in [false, true] {
        let mut bytes = build_bitmap(3, 2, top_down, |x, y| {
            [(x + 10) as f32, (y + 30) as f32, (x + y + 50) as f32]
        });
        let image = MapLightingBitmap::parse(&bytes).expect("parse failed");
        bytes.fill(0); // prove image owns its buffer
        assert_eq!(image.width(), 3);
        assert_eq!(image.height(), 2);
        assert_eq!(image.color_at(2, 1).unwrap(), [12.0, 31.0, 53.0]);
        assert_eq!(image.color_at(0, 0).unwrap(), [10.0, 30.0, 50.0]);
        assert!(matches!(
            image.color_at(-1, 0),
            Err(ProbeError::OutOfRange(_))
        ));
        assert!(matches!(
            image.color_at(0, 2),
            Err(ProbeError::OutOfRange(_))
        ));
    }
}

#[test]
fn bitmap_rejects_bad_extents() {
    let cases: [(usize, i32); 5] = [(2, 99), (10, 53), (18, 0), (22, i32::MIN), (34, 99)];

    for (offset, value) in cases {
        let mut bytes = build_bitmap(3, 2, false, |_, _| [1.0, 1.0, 1.0]);
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(
            matches!(
                MapLightingBitmap::parse(&bytes),
                Err(ProbeError::InvalidData(_))
            ),
            "offset {offset} with value {value} should fail"
        );
    }
}

#[test]
fn bitmap_rejects_truncated_payload_even_with_consistent_file_size() {
    let valid = build_bitmap(3, 2, false, |_, _| [1.0, 1.0, 1.0]);
    for length in 0..valid.len() {
        let mut prefix = valid[..length].to_vec();
        if length >= 6 {
            let len_u32 = length as u32;
            prefix[2..6].copy_from_slice(&len_u32.to_le_bytes());
        }
        assert!(
            MapLightingBitmap::parse(&prefix).is_err(),
            "prefix length {length} should fail"
        );
    }
}

#[test]
fn bitmap_accepts_zero_image_size_and_trailer_but_rejects_other_depths() {
    let mut bytes = build_bitmap(3, 2, false, |_, _| [1.0, 1.0, 1.0]);
    bytes[34..38].copy_from_slice(&0u32.to_le_bytes());
    let img = MapLightingBitmap::parse(&bytes).expect("zero image size should be accepted");
    assert_eq!(img.color_at(0, 0).unwrap(), [1.0, 1.0, 1.0]);

    // Extra trailer bytes
    let mut trailer = bytes.clone();
    trailer.extend_from_slice(&[0, 0]);
    let trailer_len = trailer.len() as u32;
    trailer[2..6].copy_from_slice(&trailer_len.to_le_bytes());
    let img_trailer = MapLightingBitmap::parse(&trailer).expect("trailer should be accepted");
    assert_eq!(img_trailer.color_at(0, 0).unwrap(), [1.0, 1.0, 1.0]);

    // Reject 8-bit depth
    bytes[28..30].copy_from_slice(&8u16.to_le_bytes());
    assert!(matches!(
        MapLightingBitmap::parse(&bytes),
        Err(ProbeError::NotSupported(_))
    ));
}
