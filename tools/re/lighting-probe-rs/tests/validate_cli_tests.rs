mod bitmap_tests;
use bitmap_tests::build_bitmap;
use lighting_probe::cli;
use std::ffi::OsString;
use std::io::Cursor;

fn to_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);

        if chunk.len() == 1 {
            out.push('=');
            out.push('=');
        } else if chunk.len() == 2 {
            out.push(TABLE[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize] as char);
            out.push('=');
        } else {
            out.push(TABLE[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize] as char);
            out.push(TABLE[(b2 & 0x3F) as usize] as char);
        }
    }
    out
}

#[test]
fn test_validate_cli_mode() {
    // 1. Shadows.dat
    let mut shadows_bytes = vec![0u8; 32 + 128 * 256 * 256 / 8];
    let header: [u32; 8] = [256, 256, 128, 6, 0, 16000, 5000, 12345];
    for (i, val) in header.iter().enumerate() {
        shadows_bytes[i * 4..(i + 1) * 4].copy_from_slice(&val.to_le_bytes());
    }

    // 2. Daynight.bmp
    let daynight_bytes = build_bitmap(24, 6, false, |x, y| {
        if y == 0 && x == 17 {
            [100.0, 150.0, 200.0]
        } else if y == 0 && x == 18 {
            [120.0, 170.0, 220.0]
        } else if y == 4 && x == 12 {
            [50.0, 60.0, 70.0]
        } else {
            [0.0, 0.0, 0.0]
        }
    });

    // 3. 1x1 BMP
    let bmp_bytes = build_bitmap(1, 1, false, |_, _| [30.0, 20.0, 10.0]);

    let payload = serde_json::json!([
        {
            "name": "test/shadows.dat",
            "bytes": to_base64(&shadows_bytes)
        },
        {
            "name": "test/daynight.bmp",
            "bytes": to_base64(&daynight_bytes)
        },
        {
            "name": "test/terrain.bmp",
            "bytes": to_base64(&bmp_bytes)
        }
    ]);

    let input = payload.to_string();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let args = [OsString::from("validate")];

    let code = cli(&args, Cursor::new(input), &mut stdout, &mut stderr);
    assert_eq!(code, 0, "CLI exit code should be 0");
    assert!(stderr.is_empty(), "stderr should be empty on success");

    let out_str = String::from_utf8(stdout).expect("valid utf-8 output");
    assert!(
        out_str.ends_with(lighting_probe::NEWLINE),
        "CLI output must end with platform NEWLINE"
    );
    let lines: Vec<&str> = out_str.lines().collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(
        lines[0],
        "VALID test/shadows.dat decoded=1048608 slices=128 header=256,256,128,6,0,16000,5000,12345"
    );
    assert_eq!(
        lines[1],
        "VALID test/daynight.bmp ambient17:55=<0.4609375, 0.65625, 0.8515625> row4hour12=<50, 60, 70>"
    );
    assert_eq!(lines[2], "VALID test/terrain.bmp 1x1 rgb0=<30, 20, 10>");
    assert_eq!(
        out_str,
        lines.join(lighting_probe::NEWLINE) + lighting_probe::NEWLINE
    );
}

#[test]
fn test_validate_cli_errors_and_usage() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    // Invalid args
    let code = cli(&[], Cursor::new("[]"), &mut stdout, &mut stderr);
    assert_eq!(code, 2);
    let err_str = String::from_utf8_lossy(&stderr);
    assert!(err_str.contains("Usage: lighting-probe <readonly apt.dat copy>"));
    assert!(err_str.ends_with(lighting_probe::NEWLINE));

    // Invalid JSON
    stdout.clear();
    stderr.clear();
    let args = [OsString::from("validate")];
    let code = cli(&args, Cursor::new("not json"), &mut stdout, &mut stderr);
    assert_eq!(code, 1);
    assert!(!stderr.is_empty());
    assert!(String::from_utf8_lossy(&stderr).ends_with(lighting_probe::NEWLINE));

    // Corrupted payload
    stdout.clear();
    stderr.clear();
    let bad_payload = serde_json::json!([
        {
            "name": "test/shadows.dat",
            "bytes": to_base64(&[1, 2, 3]) // too short for shadows.dat
        }
    ]);
    let code = cli(
        &args,
        Cursor::new(bad_payload.to_string()),
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(code, 1);
    assert!(!stderr.is_empty());
    assert!(String::from_utf8_lossy(&stderr).ends_with(lighting_probe::NEWLINE));
}
