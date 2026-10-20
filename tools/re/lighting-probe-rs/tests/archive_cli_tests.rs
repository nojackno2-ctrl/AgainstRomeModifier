use lighting_probe::{cli, AptSample};
use std::ffi::OsString;
use std::fs::File;
use std::io::{Cursor, Write};
use zip::write::FileOptions;
use zip::ZipWriter;

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

#[test]
fn test_archive_cli_mode_synthetic_zip() {
    let temp_dir = std::env::temp_dir();
    let zip_path = temp_dir.join(format!("test_lighting_probe_{}.zip", std::process::id()));

    {
        let file = File::create(&zip_path).expect("create test zip");
        let mut zip = ZipWriter::new(file);

        let options = FileOptions::default().compression_method(zip::CompressionMethod::Stored);

        let apt_bytes = make_apt_fixture(2);

        // Put gerhau00 with mixed casing and subdirectories
        zip.start_file("SYSTEM/DATA/APT/gerhau00.apt", options)
            .unwrap();
        zip.write_all(&apt_bytes).unwrap();

        // Put gerwoh00 with backslashes and uppercase
        zip.start_file("system\\assets\\GERWOH00.APT", options)
            .unwrap();
        zip.write_all(&apt_bytes).unwrap();

        zip.finish().unwrap();
    }

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let args = [OsString::from(&zip_path)];

    let code = cli(&args, Cursor::new(Vec::new()), &mut stdout, &mut stderr);
    let _ = std::fs::remove_file(&zip_path);

    let err_str = String::from_utf8_lossy(&stderr);
    assert_eq!(
        code, 0,
        "CLI should succeed with exit code 0. stderr: {err_str}"
    );
    assert!(stderr.is_empty(), "stderr should be empty");

    let out_str = String::from_utf8(stdout).expect("valid utf-8 output");
    assert!(
        out_str.ends_with(lighting_probe::NEWLINE),
        "archive CLI output must end with platform NEWLINE"
    );
    let samples: Vec<AptSample> = serde_json::from_str(&out_str).expect("valid JSON samples");
    assert_eq!(samples.len(), 2);
    assert_eq!(samples[0].name, "gerhau00");
    assert_eq!(samples[0].frame, 0);
    assert_eq!(samples[0].width, 64);
    assert_eq!(samples[0].height, 31);
    assert_eq!(samples[0].anchor_x, 30);
    assert_eq!(samples[0].anchor_y, 15);
    assert_eq!(samples[0].pixels.len(), 64 * 31);

    assert_eq!(samples[1].name, "gerwoh00");
    assert_eq!(samples[1].frame, 0);
    assert_eq!(samples[1].width, 64);
    assert_eq!(samples[1].height, 31);
}
