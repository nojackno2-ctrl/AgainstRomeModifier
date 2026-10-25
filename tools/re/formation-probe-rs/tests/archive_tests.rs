use formation_probe::archive::{export_soldier_frames, FrameExportInfo};
use std::fs::{self, File};
use std::io::Write;
use zip::write::FileOptions;
use zip::ZipWriter;

fn make_synthetic_alr() -> Vec<u8> {
    let mut buf = Vec::new();
    let version = 4u32;
    let count = 30u32;
    let cols = 2u32;
    let rows = 15u32;
    let variants = 1u32;
    let anchor_w = 40u32;
    let anchor_h = 40u32;

    // Header (23 words = 92 bytes)
    let header_words: [u32; 23] = [
        0x41524C41, version, 0x100, count, 8, 0x200, cols, rows, variants, 0x300, 0x401, 0x402,
        0x403, 0x404, 0x405, anchor_w, anchor_h, 0x501, 0x502, 0x503, 0x504, 0x601, 0x602,
    ];
    for w in header_words {
        buf.extend_from_slice(&w.to_le_bytes());
    }

    // Palette: 2 colors (color 0: 0, color 1: 0x00FF0000 ARGB red)
    let palette = [0u32, 0x00FF0000u32];

    // Frame 0: provides palette
    {
        buf.extend_from_slice(&(-1i32).to_le_bytes()); // reference -1
        let pixels = [1u8, 1u8];
        let payload_len = palette.len() * 4 + ((pixels.len() + 3) & !3);
        buf.extend_from_slice(&(payload_len as u32).to_le_bytes()); // table_offset
        let pos = 5u32 | (5u32 << 16);
        buf.extend_from_slice(&pos.to_le_bytes());
        let packed_size =
            (2u32 & 0x7FF) | ((1u32 & 0x7FF) << 11) | ((palette.len() as u32 & 0x1FF) << 22);
        buf.extend_from_slice(&packed_size.to_le_bytes());
        buf.extend_from_slice(&(pixels.len() as u32).to_le_bytes());
        for &c in &palette {
            buf.extend_from_slice(&c.to_le_bytes());
        }
        buf.extend_from_slice(&pixels);
        buf.extend_from_slice(&[0u8, 0u8]); // 4-byte align padding
                                            // Row table: height 1 -> 2 descriptors
        buf.extend_from_slice(&(0u32).to_le_bytes());
        buf.extend_from_slice(&(2u32).to_le_bytes());
    }

    // Frames 1..30: share frame 0
    for _ in 1..count {
        buf.extend_from_slice(&0i32.to_le_bytes()); // reference to frame 0
    }

    buf
}

fn create_synthetic_zip(zip_path: &std::path::Path, entry_name: &str, alr_data: &[u8]) {
    let file = File::create(zip_path).unwrap();
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file(entry_name, options).unwrap();
    zip.write_all(alr_data).unwrap();
    zip.finish().unwrap();
}

#[test]
fn test_archive_export_frames() {
    let temp_dir = crate::temp_fixture::dir("alr_archive_test");
    fs::create_dir_all(&temp_dir).unwrap();

    let zip_path = temp_dir.join("alr.dat");
    let alr_bytes = make_synthetic_alr();
    create_synthetic_zip(&zip_path, "SYSTEM/DATA/ALR/gerinf01.alr", &alr_bytes);

    let output_dir = temp_dir.join("extracted_output");
    let frames = export_soldier_frames(&zip_path, &output_dir, None).unwrap();

    // Since layout_columns = 2, max frames min(24, 2) = 2 frames
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].frame, 0);
    assert_eq!(frames[1].frame, 1);
    assert_eq!(frames[0].width, 2);
    assert_eq!(frames[0].height, 1);

    // Verify files on disk
    let frame0_file = output_dir.join("soldier-0.rgba");
    let frame1_file = output_dir.join("soldier-1.rgba");
    let json_file = output_dir.join("frames.json");

    assert!(frame0_file.exists());
    assert!(frame1_file.exists());
    assert!(json_file.exists());

    let f0_bytes = fs::read(&frame0_file).unwrap();
    assert_eq!(f0_bytes.len(), 2 * 1 * 4); // width * height * 4

    let json_text = fs::read_to_string(&json_file).unwrap();
    let parsed: Vec<FrameExportInfo> = serde_json::from_str(&json_text).unwrap();
    assert_eq!(parsed, frames);

    // Verify rejecting existing output directory
    let conflict_err = export_soldier_frames(&zip_path, &output_dir, None);
    assert!(conflict_err.is_err());
    assert!(conflict_err.unwrap_err().contains("已存在"));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_archive_missing_asset() {
    let temp_dir = crate::temp_fixture::dir("alr_missing_test").join("alr_missing_test");
    fs::create_dir_all(&temp_dir).unwrap();

    let zip_path = temp_dir.join("alr.dat");
    create_synthetic_zip(&zip_path, "SYSTEM/DATA/ALR/other.alr", &[]);

    let output_dir = temp_dir.join("new_dir");
    let res = export_soldier_frames(&zip_path, &output_dir, Some("not_found.alr"));
    assert!(res.is_err());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_archive_entry_lookup_preserves_zip_case_sensitivity() {
    let temp_dir = crate::temp_fixture::dir("alr_case_test");
    fs::create_dir_all(&temp_dir).unwrap();

    let zip_path = temp_dir.join("alr.dat");
    create_synthetic_zip(&zip_path, "SYSTEM/DATA/ALR/GerInf01.alr", &[]);
    let output_dir = temp_dir.join("new_dir");

    let result = export_soldier_frames(&zip_path, &output_dir, None);
    assert!(result.unwrap_err().contains("找不到指定的 ALR 資產"));
    assert!(!output_dir.exists());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[cfg(test)]
mod temp_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/temp_unique.rs"));
}
