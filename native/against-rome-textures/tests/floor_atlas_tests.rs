use against_rome_textures::{
    AtlasRect, FloorTextureAtlas, FloortexLibrary,
};
use against_rome_textures::terrain_layer::TerrainLayer;
use std::io::Write;
use zip::write::FileOptions;
use zip::ZipWriter;

#[test]
fn test_capacity_and_grid_dimension_parity() {
    // Capacity for 2048: (2048 / 144) = 14 -> 14 * 14 = 196
    assert_eq!(FloorTextureAtlas::capacity(2048), 196);
    // Capacity for 4096: (4096 / 144) = 28 -> 28 * 28 = 784
    assert_eq!(FloorTextureAtlas::capacity(4096), 784);
    // Capacity for 1024: (1024 / 144) = 7 -> 7 * 7 = 49
    assert_eq!(FloorTextureAtlas::capacity(1024), 49);
    // Smaller than one stride (144)
    assert_eq!(FloorTextureAtlas::capacity(143), 0);
    assert_eq!(FloorTextureAtlas::capacity(144), 1);

    // Grid layout calculations
    // 0 or 1 texture -> 1 column, 1 row
    let (cols, rows) = FloorTextureAtlas::get_grid(0, 2048).unwrap();
    assert_eq!((cols, rows), (1, 1));
    let (cols, rows) = FloorTextureAtlas::get_grid(1, 2048).unwrap();
    assert_eq!((cols, rows), (1, 1));

    // 4 textures -> 2x2
    let (cols, rows) = FloorTextureAtlas::get_grid(4, 2048).unwrap();
    assert_eq!((cols, rows), (2, 2));

    // 5 textures -> ceil(sqrt(5)) = 3 cols, ceil(5/3) = 2 rows
    let (cols, rows) = FloorTextureAtlas::get_grid(5, 2048).unwrap();
    assert_eq!((cols, rows), (3, 2));

    // 196 textures in 2048 -> 14x14
    let (cols, rows) = FloorTextureAtlas::get_grid(196, 2048).unwrap();
    assert_eq!((cols, rows), (14, 14));

    // 197 textures exceeds 2048 capacity (196)
    let err = FloorTextureAtlas::get_grid(197, 2048).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("197"), "Error message must report count");
    assert!(msg.contains("196"), "Error message must report max capacity");

    // 197 textures in 4096 -> ceil(sqrt(197)) = 15 cols, ceil(197/15) = 14 rows
    let (cols, rows) = FloorTextureAtlas::get_grid(197, 4096).unwrap();
    assert_eq!((cols, rows), (15, 14));

    // 785 textures exceeds 4096 capacity (784)
    assert!(FloorTextureAtlas::get_grid(785, 4096).is_err());
}

#[test]
fn test_layout_case_insensitivity_and_coordinate_mapping() {
    let names = ["4B6___51", "4b6___51", "4B6___51.bmp", "4B7___51", "PFAD1"];
    let layout = FloorTextureAtlas::layout(names, 2048).unwrap();

    // Deduplication should result in 3 unique entries
    assert_eq!(layout.len(), 3);
    assert!(layout.contains_key("4b6___51"));
    assert!(layout.contains_key("4b7___51"));
    assert!(layout.contains_key("pfad1"));

    // For 3 items: columns = ceil(sqrt(3)) = 2, rows = ceil(3/2) = 2
    // Item 0: cellX=0, cellY=0, inner=(8, 8), size=128x128
    let r0 = layout.get("4b6___51").unwrap();
    assert_eq!(*r0, AtlasRect::new(8, 8, 128, 128));

    // Item 1: cellX=144, cellY=0, inner=(152, 8), size=128x128
    let r1 = layout.get("4b7___51").unwrap();
    assert_eq!(*r1, AtlasRect::new(152, 8, 128, 128));

    // Item 2: cellX=0, cellY=144, inner=(8, 152), size=128x128
    let r2 = layout.get("pfad1").unwrap();
    assert_eq!(*r2, AtlasRect::new(8, 152, 128, 128));
}

#[test]
fn test_uv_inset_and_fallback_semantics() {
    let rect = AtlasRect::new(8, 8, 128, 128);
    let size = 2048;
    let uv = rect.to_uv(size);

    let inset = 0.5 / 2048.0;
    let expected_left = 8.0 / 2048.0 + inset;
    let expected_top = 8.0 / 2048.0 + inset;
    let expected_right = 136.0 / 2048.0 - inset;
    let expected_bottom = 136.0 / 2048.0 - inset;

    // Check TL, TR, BR, BL order
    assert!((uv[0].u - expected_left).abs() < 1e-6);
    assert!((uv[0].v - expected_top).abs() < 1e-6);
    assert!((uv[1].u - expected_right).abs() < 1e-6);
    assert!((uv[1].v - expected_top).abs() < 1e-6);
    assert!((uv[2].u - expected_right).abs() < 1e-6);
    assert!((uv[2].v - expected_bottom).abs() < 1e-6);
    assert!((uv[3].u - expected_left).abs() < 1e-6);
    assert!((uv[3].v - expected_bottom).abs() < 1e-6);

    // Build small atlas to test fallback UV lookup
    let atlas = FloorTextureAtlas::create_with_decoder(&["4B6___51"], |_| None).unwrap();
    assert!(atlas.contains("4B6___51"));
    assert!(!atlas.contains("non_existent_texture"));

    // Known texture gets correct UV
    let known_uv = atlas.get_uv("4B6___51");
    assert_eq!(known_uv, uv);

    // Missing texture gets full-atlas UV with half-texel inset
    let fallback_uv = atlas.get_uv("non_existent_texture");
    assert!((fallback_uv[0].u - (0.5 / 2048.0)).abs() < 1e-6);
    assert!((fallback_uv[0].v - (0.5 / 2048.0)).abs() < 1e-6);
    assert!((fallback_uv[2].u - (1.0 - 0.5 / 2048.0)).abs() < 1e-6);
    assert!((fallback_uv[2].v - (1.0 - 0.5 / 2048.0)).abs() < 1e-6);

    let (u0, v0, u1, v1) = atlas.get_uv_bounds("non_existent_texture");
    assert!((u0 - (0.5 / 2048.0)).abs() < 1e-6);
    assert!((v0 - (0.5 / 2048.0)).abs() < 1e-6);
    assert!((u1 - (1.0 - 0.5 / 2048.0)).abs() < 1e-6);
    assert!((v1 - (1.0 - 0.5 / 2048.0)).abs() < 1e-6);
}

#[test]
fn test_fallback_color_determinism_and_range() {
    let textures = ["4B6___51", "4UAB__11", "PFAD1", "L2B02T5A", "water_deep"];
    for name in textures {
        let col1 = FloorTextureAtlas::fallback_color(name);
        let col2 = FloorTextureAtlas::fallback_color(&name.to_lowercase());
        let col3 = FloorTextureAtlas::fallback_color(&name.to_uppercase());

        // Must be case-insensitive
        assert_eq!(col1, col2);
        assert_eq!(col1, col3);

        // Alpha channel must be 0xFF (opaque)
        assert_eq!((col1 >> 24) & 0xFF, 0xFF);

        // R, G, B channels must be in range [90..=169]
        let r = (col1 >> 16) & 0xFF;
        let g = (col1 >> 8) & 0xFF;
        let b = col1 & 0xFF;
        assert!(r >= 90 && r <= 169, "R channel out of range: {}", r);
        assert!(g >= 90 && g <= 169, "G channel out of range: {}", g);
        assert!(b >= 90 && b <= 169, "B channel out of range: {}", b);
    }

    // Verify hash formula directly
    let hash = 0x1234_5678i32;
    let expected_r = 90 + ((hash as u32) & 0x4f);
    let expected_g = 90 + (((hash as u32) >> 8) & 0x4f);
    let expected_b = 90 + (((hash as u32) >> 16) & 0x4f);
    let expected_col = 0xFF00_0000 | (expected_r << 16) | (expected_g << 8) | expected_b;
    assert_eq!(
        FloorTextureAtlas::fallback_color_from_hash(hash),
        expected_col
    );
}

#[test]
fn test_atlas_pixel_blitting_central_fidelity_and_gutter_dilation() {
    // Create a 128x128 synthetic layer with a distinct gradient
    let mut argb = Vec::with_capacity(128 * 128);
    for y in 0..128 {
        for x in 0..128 {
            let r = (x * 2) as u32;
            let g = (y * 2) as u32;
            let b = 100u32;
            argb.push(0xFF00_0000 | (r << 16) | (g << 8) | b);
        }
    }
    let layer = TerrainLayer {
        width: 128,
        height: 128,
        argb: argb.clone(),
        green: vec![0; 128 * 128],
    };

    let atlas = FloorTextureAtlas::create_with_decoder(&["tile_a"], |name| {
        if name == "tile_a" {
            Some(layer.clone())
        } else {
            None
        }
    })
    .unwrap();

    assert_eq!(atlas.size(), 2048);
    let pixels = atlas.argb_pixels();

    // 1. Verify central 128x128 pixels match layer.argb bit-for-bit
    let rect = atlas.get_rect("tile_a").unwrap();
    assert_eq!(rect.x, 8);
    assert_eq!(rect.y, 8);

    for dy in 0..128 {
        for dx in 0..128 {
            let src_idx = dy * 128 + dx;
            let dst_idx = (rect.y as usize + dy) * 2048 + (rect.x as usize + dx);
            assert_eq!(
                pixels[dst_idx], argb[src_idx],
                "Pixel mismatch at ({}, {}) in central 128x128",
                dx, dy
            );
        }
    }

    // 2. Verify gutter pixels (e.g. at x=0..7, y=0..7) are NOT clear background (DEFAULT_BG_COLOR)
    for gy in 0..8 {
        for gx in 0..8 {
            let dst_idx = gy * 2048 + gx;
            assert_ne!(
                pixels[dst_idx],
                FloorTextureAtlas::DEFAULT_BG_COLOR,
                "Gutter pixel at ({}, {}) must be filled with dilated texture color",
                gx,
                gy
            );
        }
    }

    // 3. Verify untouched area far away is default background
    let far_idx = 500 * 2048 + 500;
    assert_eq!(pixels[far_idx], FloorTextureAtlas::DEFAULT_BG_COLOR);
}

#[test]
fn test_missing_texture_blits_fallback_color_across_cell() {
    let atlas =
        FloorTextureAtlas::create_with_decoder(&["missing_one"], |_| None).unwrap();
    let rect = atlas.get_rect("missing_one").unwrap();
    let expected_color = FloorTextureAtlas::fallback_color("missing_one");

    let pixels = atlas.argb_pixels();
    let cell_x = rect.x as usize - FloorTextureAtlas::PAD;
    let cell_y = rect.y as usize - FloorTextureAtlas::PAD;

    // The whole 144x144 stride should be filled with fallback color
    for gy in 0..FloorTextureAtlas::STRIDE {
        for gx in 0..FloorTextureAtlas::STRIDE {
            let dst_idx = (cell_y + gy) * 2048 + (cell_x + gx);
            assert_eq!(
                pixels[dst_idx], expected_color,
                "Pixel at stride ({}, {}) should match fallback color",
                gx, gy
            );
        }
    }
}

#[test]
fn test_scale_to_4096_when_capacity_exceeds_2048() {
    // 200 unique textures exceeds 2048 capacity (196), so atlas must choose 4096
    let names: Vec<String> = (0..200).map(|i| format!("T_{:03}", i)).collect();
    let atlas = FloorTextureAtlas::create_with_decoder(&names, |_| None).unwrap();

    assert_eq!(atlas.size(), 4096);
    assert_eq!(atlas.count(), 200);

    // Check first and last placement
    let r_first = atlas.get_rect("T_000").unwrap();
    assert_eq!(r_first, AtlasRect::new(8, 8, 128, 128));

    let r_last = atlas.get_rect("T_199").unwrap();
    assert!(r_last.x >= 8 && r_last.y >= 8);
    assert!(r_last.x + r_last.width <= 4096);
    assert!(r_last.y + r_last.height <= 4096);
}

fn create_mock_bmp(width: usize, height: usize, fill_color: u32) -> Vec<u8> {
    // 24-bit uncompressed BMP header
    let row_stride = (width * 3 + 3) / 4 * 4;
    let pixel_bytes = row_stride * height;
    let file_size = 54 + pixel_bytes;

    let mut bmp = vec![0u8; file_size];
    // Magic 'BM'
    bmp[0] = b'B';
    bmp[1] = b'M';
    // File size
    bmp[2..6].copy_from_slice(&(file_size as u32).to_le_bytes());
    // Offset to data = 54
    bmp[10..14].copy_from_slice(&54u32.to_le_bytes());
    // DIB header size = 40
    bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
    // Width and Height
    bmp[18..22].copy_from_slice(&(width as i32).to_le_bytes());
    bmp[22..26].copy_from_slice(&(height as i32).to_le_bytes());
    // Planes = 1, bpp = 24
    bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
    bmp[28..30].copy_from_slice(&24u16.to_le_bytes());

    let r = ((fill_color >> 16) & 0xFF) as u8;
    let g = ((fill_color >> 8) & 0xFF) as u8;
    let b = (fill_color & 0xFF) as u8;

    for y in 0..height {
        let row_start = 54 + y * row_stride;
        for x in 0..width {
            bmp[row_start + x * 3] = b;
            bmp[row_start + x * 3 + 1] = g;
            bmp[row_start + x * 3 + 2] = r;
        }
    }
    bmp
}

#[test]
fn test_floortex_library_mock_zip_roundtrip() {
    let temp_dir = crate::temp_fixture::dir("floor_atlas_test");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let zip_path = temp_dir.join("mock_floortex.dat");

    // Create a mock zip with two 128x128 floor textures
    {
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip = ZipWriter::new(file);

        let options = FileOptions::default().compression_method(zip::CompressionMethod::Stored);

        zip.start_file("4B6___51.bmp", options).unwrap();
        zip.write_all(&create_mock_bmp(128, 128, 0xFF33_6699))
            .unwrap();

        zip.start_file("PFAD1.BMP", options).unwrap();
        zip.write_all(&create_mock_bmp(128, 128, 0xFF99_6633))
            .unwrap();

        zip.finish().unwrap();
    }

    // Open library and build atlas
    let mut lib = FloortexLibrary::open(&zip_path).unwrap();
    assert_eq!(lib.names().len(), 2);
    assert!(lib.contains("4b6___51"));
    assert!(lib.contains("pfad1"));

    let layer = lib.get_layer("4b6___51").unwrap();
    assert_eq!(layer.width, 128);
    assert_eq!(layer.height, 128);
    assert_eq!(layer.argb[0], 0xFF33_6699);

    let atlas = FloorTextureAtlas::create(&["4B6___51", "PFAD1", "MISSING_TILE"], &mut lib).unwrap();
    assert_eq!(atlas.size(), 2048);
    assert_eq!(atlas.count(), 3);
    assert!(atlas.contains("4B6___51"));
    assert!(atlas.contains("PFAD1"));
    assert!(atlas.contains("MISSING_TILE"));

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[cfg(test)]
mod temp_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/temp_unique.rs"));
}
