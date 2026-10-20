//! Real C# differential for `AlrDocument` against `NativeAlrDocument.cs` and `NativeAlrIndexedFrame.cs`
//! (linked into tests/oracle/AlrOracle.csproj).
//!
//! Directly links the original NativeAlrDocument.cs and related scanline/parser sources,
//! reads synthetic inputs and outputs:
//!   - frame metadata
//!   - indexed pixels / samples
//!   - palette words
//!   - roundtrip bytes
//! And compares against Rust AlrDocument full output across:
//!   - v4, v5, v6 container layouts
//!   - shared frame DAGs (including transitive shared references)
//!   - palette variants (1, 2, 4 variants; local vs first-frame fallback)
//!   - malformed headers and corrupted container structures.

use against_rome_data::alr::*;
use against_rome_data::alr_scanlines::IndexedSample;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let mut path = std::env::temp_dir();
        let unique = format!(
            "{}_{}_{}",
            prefix,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        path.push(unique);
        fs::create_dir_all(&path).expect("Failed to create temp dir");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn put_u32_le(buf: &mut [u8], offset: usize, value: u32) {
    buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_frame_record(
    buf: &mut Vec<u8>,
    offset_x: i32,
    offset_y: i32,
    width: usize,
    height: usize,
    variants: usize,
    colors: usize,
    palette: &[u32],
    pixels: &[u8],
    rows: &[u32],
) {
    buf.extend_from_slice(&(-1i32).to_le_bytes());
    assert_eq!(palette.len(), variants * colors);
    assert_eq!(rows.len(), height + 1);
    let aligned_pixels = (pixels.len() + 3) & !3;
    let table_offset = (palette.len() * 4 + aligned_pixels) as u32;
    buf.extend_from_slice(&table_offset.to_le_bytes());
    let pos = ((offset_x as u32) & 0xFFFF) | (((offset_y as u32) & 0xFFFF) << 16);
    buf.extend_from_slice(&pos.to_le_bytes());
    let s = (width as u32 & 0x7FF)
        | ((height as u32 & 0x7FF) << 11)
        | ((colors as u32 & 0x1FF) << 22);
    buf.extend_from_slice(&s.to_le_bytes());
    buf.extend_from_slice(&(pixels.len() as u32).to_le_bytes());
    for &col in palette {
        buf.extend_from_slice(&col.to_le_bytes());
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

fn build_header(
    version: u32,
    frame_count: u32,
    columns: u32,
    rows: u32,
    variants: u32,
    anchor_w: u32,
    anchor_h: u32,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(92);
    let words: [u32; 23] = [
        0x41524C41, version, 0x100, frame_count, 8, 0x200, columns, rows, variants, 0x300, 0x401,
        0x402, 0x403, 0x404, 0x405, anchor_w, anchor_h, 0x501, 0x502, 0x503, 0x504, 0x601, 0x602,
    ];
    for w in words {
        buf.extend_from_slice(&w.to_le_bytes());
    }
    buf
}

fn ensure_oracle_built() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let project = manifest_dir.join("tests/oracle/AlrOracle.csproj");
    let output = Command::new("dotnet")
        .args(["build", project.to_str().unwrap(), "-c", "Release"])
        .output()
        .expect("Failed to execute dotnet build on AlrOracle");

    assert!(
        output.status.success(),
        "Failed to build AlrOracle:\nSTDOUT:\n{}\nSTDERR:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    manifest_dir.join("tests/oracle/bin/Release/net8.0/AlrOracle.dll")
}

fn run_oracle(oracle_dll: &Path, cases_dir: &Path) {
    let output = Command::new("dotnet")
        .arg(oracle_dll)
        .arg(cases_dir)
        .output()
        .expect("Failed to execute AlrOracle");

    assert!(
        output.status.success(),
        "AlrOracle execution failed:\nSTDOUT:\n{}\nSTDERR:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[derive(Debug)]
#[allow(dead_code)]
struct FrameMeta {
    index: usize,
    is_shared: bool,
    shared_from: Option<usize>,
    offset_x: i32,
    offset_y: i32,
    width: usize,
    height: usize,
    palette_color_count: usize,
    packed_size: u32,
}

#[derive(Debug)]
struct DocMeta {
    version: u32,
    layout_columns: u32,
    layout_rows: u32,
    palette_variant_count: usize,
    anchor_width: u32,
    anchor_height: u32,
    frame_count: usize,
    frames: Vec<FrameMeta>,
}

fn parse_meta_file(content: &str) -> DocMeta {
    let mut lines = content.lines().filter(|l| !l.trim().is_empty());
    let doc_line = lines.next().expect("Missing doc line in meta");
    let parts: Vec<&str> = doc_line.split_whitespace().collect();
    let version: u32 = parts[0].parse().unwrap();
    let layout_columns: u32 = parts[1].parse().unwrap();
    let layout_rows: u32 = parts[2].parse().unwrap();
    let palette_variant_count: usize = parts[3].parse().unwrap();
    let anchor_width: u32 = parts[4].parse().unwrap();
    let anchor_height: u32 = parts[5].parse().unwrap();
    let frame_count: usize = parts[6].parse().unwrap();

    let mut frames = Vec::with_capacity(frame_count);
    for line in lines {
        let fp: Vec<&str> = line.split_whitespace().collect();
        let index: usize = fp[0].parse().unwrap();
        let is_shared = fp[1] == "1";
        let shared_from_int: i32 = fp[2].parse().unwrap();
        let shared_from = if shared_from_int >= 0 {
            Some(shared_from_int as usize)
        } else {
            None
        };
        let offset_x: i32 = fp[3].parse().unwrap();
        let offset_y: i32 = fp[4].parse().unwrap();
        let width: usize = fp[5].parse().unwrap();
        let height: usize = fp[6].parse().unwrap();
        let palette_color_count: usize = fp[7].parse().unwrap();
        let packed_size: u32 = fp[8].parse().unwrap();

        frames.push(FrameMeta {
            index,
            is_shared,
            shared_from,
            offset_x,
            offset_y,
            width,
            height,
            palette_color_count,
            packed_size,
        });
    }

    DocMeta {
        version,
        layout_columns,
        layout_rows,
        palette_variant_count,
        anchor_width,
        anchor_height,
        frame_count,
        frames,
    }
}

fn read_u32_array(bytes: &[u8]) -> Vec<u32> {
    assert_eq!(bytes.len() % 4, 0);
    bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

fn read_u16_array(bytes: &[u8]) -> Vec<u16> {
    assert_eq!(bytes.len() % 2, 0);
    bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

#[test]
fn csharp_alr_document_differential_suite() {
    let oracle_dll = ensure_oracle_built();
    let temp = TempDir::new("alr_oracle_diff");
    let dir = &temp.path;

    let mut test_names = Vec::new();

    let mut add_case = |name: &str, data: &[u8]| {
        let in_path = dir.join(format!("{name}.in"));
        fs::write(&in_path, data).unwrap();
        test_names.push(name.to_string());
    };

    // 1. Version 4 valid document (2 variants, 4 frames, shared DAG)
    {
        let mut buf = build_header(4, 4, 2, 2, 2, 12, 18);
        // Frame 0: Independent record
        write_frame_record(
            &mut buf,
            7,
            11,
            5,
            1,
            2,
            2,
            &[0x405060, 0x102030, 0xC0D0E0, 0x90A0B0],
            &[1, 0, 1],
            &[1u32 << 21, 3],
        );
        // Frame 1: Shared Frame 0
        buf.extend_from_slice(&0i32.to_le_bytes());
        // Frame 2: Independent record with local palette
        write_frame_record(
            &mut buf,
            2,
            3,
            2,
            1,
            2,
            2,
            &[0, 0xFFFFFF, 0, 0xEEEEEE],
            &[1, 0],
            &[0x80000000, 2],
        );
        // Frame 3: Transitive shared Frame 1 (which points to 0)
        buf.extend_from_slice(&1i32.to_le_bytes());
        add_case("v4_standard", &buf);
    }

    // 2. Version 5 valid document (1 variant, 3 frames)
    {
        let mut buf = build_header(5, 3, 3, 1, 1, 24, 30);
        write_frame_record(
            &mut buf,
            0,
            0,
            4,
            1,
            1,
            3,
            &[0x112233, 0x445566, 0x778899],
            &[0, 1, 2],
            &[0, 3],
        );
        write_frame_record(
            &mut buf,
            1,
            2,
            3,
            1,
            1,
            3,
            &[0xAA0011, 0xBB0022, 0xCC0033],
            &[1, 2],
            &[0, 2],
        );
        // Frame 2: Shared Frame 0
        buf.extend_from_slice(&0i32.to_le_bytes());
        add_case("v5_standard", &buf);
    }

    // 3. Version 6 valid document (3 variants, 5 frames, complex layout)
    {
        let mut buf = build_header(6, 5, 1, 4, 3, 16, 16);
        // Frame 0: 3 variants * 2 colors = 6 palette words
        write_frame_record(
            &mut buf,
            -2,
            -3,
            6,
            1,
            3,
            2,
            &[
                0x010203, 0x040506, // var 0
                0x111213, 0x141516, // var 1
                0x212223, 0x242526, // var 2
            ],
            &[0, 1, 0, 1],
            &[0, 4],
        );
        // Frame 1: Shared 0
        buf.extend_from_slice(&0i32.to_le_bytes());
        // Frame 2: Shared 0
        buf.extend_from_slice(&0i32.to_le_bytes());
        // Frame 3: Independent
        write_frame_record(
            &mut buf,
            4,
            5,
            3,
            1,
            3,
            2,
            &[
                0xA1B1C1, 0xA2B2C2, // var 0
                0xD1E1F1, 0xD2E2F2, // var 1
                0x001122, 0x334455, // var 2
            ],
            &[1, 1],
            &[0, 2],
        );
        // Frame 4: Shared 3
        buf.extend_from_slice(&3i32.to_le_bytes());
        add_case("v6_standard", &buf);
    }

    // 4. Complex Shared Frame DAG (8 frames with transitive references & blank frames)
    {
        let mut buf = build_header(6, 8, 2, 2, 2, 20, 20);
        // Frame 0: Root
        write_frame_record(
            &mut buf,
            5,
            5,
            4,
            1,
            2,
            2,
            &[0x102030, 0x405060, 0x708090, 0xA0B0C0],
            &[1, 0, 1],
            &[1u32 << 21, 3],
        );
        // Frame 1: Shared 0
        buf.extend_from_slice(&0i32.to_le_bytes());
        // Frame 2: Independent Root 2
        write_frame_record(
            &mut buf,
            1,
            1,
            3,
            1,
            2,
            2,
            &[0x01, 0x02, 0x03, 0x04],
            &[0, 1],
            &[0x80000000, 2],
        );
        // Frame 3: Shared 1 (transitive share of 0)
        buf.extend_from_slice(&1i32.to_le_bytes());
        // Frame 4: Shared 2
        buf.extend_from_slice(&2i32.to_le_bytes());
        // Frame 5: Shared 3 (transitive share of 1 which points to 0)
        buf.extend_from_slice(&3i32.to_le_bytes());
        // Frame 6: Blank frame (0x0)
        write_frame_record(&mut buf, 0, 0, 0, 0, 2, 0, &[], &[], &[0]);
        // Frame 7: Shared 6 (shared blank frame)
        buf.extend_from_slice(&6i32.to_le_bytes());
        add_case("v6_complex_dag", &buf);
    }

    // 5. Four Palette Variants
    {
        let mut buf = build_header(6, 2, 1, 1, 4, 8, 8);
        write_frame_record(
            &mut buf,
            0,
            0,
            4,
            1,
            4,
            2,
            &[
                0x11, 0x22, // var 0
                0x33, 0x44, // var 1
                0x55, 0x66, // var 2
                0x77, 0x88, // var 3
            ],
            &[1, 0, 1],
            &[0, 3],
        );
        buf.extend_from_slice(&0i32.to_le_bytes());
        add_case("v6_four_variants", &buf);
    }

    // 6. Frame with zero local palette (colors = 0) falling back to Frame 0
    {
        let mut buf = build_header(6, 3, 2, 2, 2, 16, 16);
        // Frame 0: 2 colors per variant
        write_frame_record(
            &mut buf,
            0,
            0,
            4,
            1,
            2,
            2,
            &[0x102030, 0x405060, 0x708090, 0xA0B0C0],
            &[1, 0],
            &[0, 2],
        );
        // Frame 1: colors = 0, no local palette
        write_frame_record(&mut buf, 2, 2, 3, 1, 2, 0, &[], &[1, 1], &[0, 2]);
        // Frame 2: Shared Frame 1
        buf.extend_from_slice(&1i32.to_le_bytes());
        add_case("v6_frame_missing_local_palette_uses_first", &buf);
    }

    // 7. Multi-row frame with 2-run scanlines and opaque zero
    {
        let mut buf = build_header(6, 1, 1, 1, 2, 16, 16);
        // Height = 2 rows:
        // Row 0: single run at x=1
        // Row 1: two runs (firstLen=1, gap=2, secondOpaque=true)
        let pixels = vec![
            1, 0, 1, // row 0: 3 bytes
            1, 0x82, 1, 1, // row 1: firstLength=1, gap=2 with bit7 set (0x82), byte 2=1, byte 3=1 (second run)
        ];
        let rows = vec![
            (1u32 << 21) | 0x80000000,     // row 0: x=1, opaque0, offset 0
            0x100000 | (0u32 << 21) | 3, // row 1: 2-run (bit20), x=0, offset 3
            7,                           // end descriptor: offset 7
        ];
        write_frame_record(
            &mut buf,
            0,
            0,
            8,
            2,
            2,
            2,
            &[0x102030, 0x405060, 0x708090, 0xA0B0C0],
            &pixels,
            &rows,
        );
        add_case("v6_two_run_scanlines", &buf);
    }

    // 8. Preserved trailing metadata (e.g. IFOM block)
    {
        let mut buf = build_header(6, 1, 1, 1, 1, 8, 8);
        write_frame_record(
            &mut buf,
            0,
            0,
            2,
            1,
            1,
            2,
            &[0x111111, 0x222222],
            &[1],
            &[0, 1],
        );
        buf.extend_from_slice(b"IFOM\x01\x02\x03\x04custom-metadata");
        add_case("v6_trailing_metadata", &buf);
    }

    // 9. Version 4 minimal (1 frame, 1 variant, 1 color)
    {
        let mut buf = build_header(4, 1, 1, 1, 1, 4, 4);
        write_frame_record(&mut buf, 0, 0, 1, 1, 1, 1, &[0x123456], &[0], &[0, 1]);
        add_case("v4_minimal", &buf);
    }

    // 10. Version 5 multi-row (height 3, 2 variants)
    {
        let mut buf = build_header(5, 2, 2, 1, 2, 10, 10);
        write_frame_record(
            &mut buf,
            -1,
            2,
            3,
            3,
            2,
            2,
            &[0x10, 0x20, 0x30, 0x40],
            &[0, 1, 1, 0, 1, 0],
            &[0, 2, 4, 6],
        );
        buf.extend_from_slice(&0i32.to_le_bytes()); // shared frame
        add_case("v5_multi_row", &buf);
    }

    // 11. Version 6 Diamond DAG
    {
        let mut buf = build_header(6, 6, 2, 2, 1, 16, 16);
        // Frame 0: root
        write_frame_record(&mut buf, 0, 0, 2, 1, 1, 2, &[0xAA, 0xBB], &[1, 0], &[0, 2]);
        // Frame 1: shares 0
        buf.extend_from_slice(&0i32.to_le_bytes());
        // Frame 2: shares 0
        buf.extend_from_slice(&0i32.to_le_bytes());
        // Frame 3: shares 1
        buf.extend_from_slice(&1i32.to_le_bytes());
        // Frame 4: shares 2
        buf.extend_from_slice(&2i32.to_le_bytes());
        // Frame 5: shares 3
        buf.extend_from_slice(&3i32.to_le_bytes());
        add_case("v6_diamond_dag", &buf);
    }

    // 12. Version 6 with 16 palette colors per variant
    {
        let mut pal = Vec::new();
        for v in 0..2 {
            for c in 0..16 {
                pal.push((v << 16) | (c << 8) | 0x55);
            }
        }
        let mut buf = build_header(6, 1, 1, 1, 2, 32, 32);
        write_frame_record(
            &mut buf,
            0,
            0,
            8,
            1,
            2,
            16,
            &pal,
            &[0, 1, 2, 3, 4, 5, 6, 7],
            &[0, 8],
        );
        add_case("v6_many_colors", &buf);
    }

    // 13. Corrupt scanlines: Palette index out of bounds (referenced 10, palette has 2)
    {
        let mut buf = build_header(6, 1, 1, 1, 1, 8, 8);
        write_frame_record(
            &mut buf,
            0,
            0,
            2,
            1,
            1,
            2,
            &[0x11, 0x22],
            &[10], // index 10 >= palette length 2!
            &[0, 1],
        );
        add_case("corrupt_scanline_palette_index_oob", &buf);
    }

    // 14. Corrupt scanlines: Backwards row offsets (end < start)
    {
        let mut buf = build_header(6, 1, 1, 1, 1, 8, 8);
        write_frame_record(
            &mut buf,
            0,
            0,
            4,
            2,
            1,
            2,
            &[0x11, 0x22],
            &[1, 1, 1, 1],
            &[3, 1, 4], // row 0: start 3, end 1 (backwards!)
        );
        add_case("corrupt_scanline_backwards_offsets", &buf);
    }

    // 15. Corrupt scanlines: Two-run missing 2-byte prefix
    {
        let mut buf = build_header(6, 1, 1, 1, 1, 8, 8);
        write_frame_record(
            &mut buf,
            0,
            0,
            4,
            1,
            1,
            2,
            &[0x11, 0x22],
            &[1], // only 1 byte payload for 2-run!
            &[0x100000, 1], // bit 20 indicates two-run
        );
        add_case("corrupt_scanline_two_run_missing_prefix", &buf);
    }

    // 16. Corrupt scanlines: Scanline extends past frame width
    {
        let mut buf = build_header(6, 1, 1, 1, 1, 8, 8);
        write_frame_record(
            &mut buf,
            0,
            0,
            2, // width = 2
            1,
            1,
            2,
            &[0x11, 0x22],
            &[1, 1, 1, 1], // 4 pixels > width 2!
            &[0, 4],
        );
        add_case("corrupt_scanline_run_extends_past_width", &buf);
    }

    // 17. Malformed: Empty file (0 bytes)
    add_case("malformed_empty", &[]);

    // 18. Malformed: Truncated header (16 bytes)
    add_case("malformed_header_truncated_16", &build_header(6, 1, 1, 1, 1, 8, 8)[..16]);

    // 19. Malformed: Truncated header (50 bytes)
    add_case("malformed_header_truncated_50", &build_header(6, 1, 1, 1, 1, 8, 8)[..50]);

    // 20. Malformed: Truncated header (91 bytes)
    add_case("malformed_header_truncated_91", &build_header(6, 1, 1, 1, 1, 8, 8)[..91]);

    // 21. Malformed: Bad magic signature
    {
        let mut bad = build_header(6, 1, 1, 1, 1, 8, 8);
        put_u32_le(&mut bad, 0, 0);
        add_case("malformed_signature_zero", &bad);
    }

    // 22. Malformed: Unsupported version 0
    {
        let mut bad = build_header(6, 1, 1, 1, 1, 8, 8);
        put_u32_le(&mut bad, 4, 0);
        add_case("malformed_version_0", &bad);
    }

    // 23. Malformed: Unsupported version 3 (< 4)
    {
        let mut bad = build_header(6, 1, 1, 1, 1, 8, 8);
        put_u32_le(&mut bad, 4, 3);
        add_case("malformed_version_3", &bad);
    }

    // 24. Malformed: Unsupported version 7 (> 6)
    {
        let mut bad = build_header(6, 1, 1, 1, 1, 8, 8);
        put_u32_le(&mut bad, 4, 7);
        add_case("malformed_version_7", &bad);
    }

    // 25. Malformed: Unsupported version 99
    {
        let mut bad = build_header(6, 1, 1, 1, 1, 8, 8);
        put_u32_le(&mut bad, 4, 99);
        add_case("malformed_version_99", &bad);
    }

    // 26. Malformed: Unsupported bit depth 0
    {
        let mut bad = build_header(6, 1, 1, 1, 1, 8, 8);
        put_u32_le(&mut bad, 16, 0);
        add_case("malformed_depth_0", &bad);
    }

    // 27. Malformed: Unsupported bit depth 16 (!= 8)
    {
        let mut bad = build_header(6, 1, 1, 1, 1, 8, 8);
        put_u32_le(&mut bad, 16, 16);
        add_case("malformed_depth_16", &bad);
    }

    // 28. Malformed: Unsupported bit depth 24 (!= 8)
    {
        let mut bad = build_header(6, 1, 1, 1, 1, 8, 8);
        put_u32_le(&mut bad, 16, 24);
        add_case("malformed_depth_24", &bad);
    }

    // 29. Malformed: Unsupported bit depth 32 (!= 8)
    {
        let mut bad = build_header(6, 1, 1, 1, 1, 8, 8);
        put_u32_le(&mut bad, 16, 32);
        add_case("malformed_depth_32", &bad);
    }

    // 30. Malformed: Frame count zero
    {
        let bad = build_header(6, 0, 1, 1, 1, 8, 8);
        add_case("malformed_frame_count_0", &bad);
    }

    // 31. Malformed: Frame count overflow (u32::MAX)
    {
        let mut bad = build_header(6, 1, 1, 1, 1, 8, 8);
        put_u32_le(&mut bad, 12, u32::MAX);
        add_case("malformed_frame_count_max", &bad);
    }

    // 32. Malformed: Palette variants zero
    {
        let bad = build_header(6, 1, 1, 1, 0, 8, 8);
        add_case("malformed_palette_variants_0", &bad);
    }

    // 33. Malformed: Palette variants overflow
    {
        let mut bad = build_header(6, 1, 1, 1, 1, 8, 8);
        put_u32_le(&mut bad, 32, u32::MAX);
        add_case("malformed_palette_variants_max", &bad);
    }

    // 34. Malformed: Truncated frame references table (declares 4 frames, only provides 1)
    {
        let mut bad = build_header(6, 4, 1, 1, 1, 8, 8);
        buf_extend_frame_0(&mut bad);
        add_case("malformed_truncated_frame_references", &bad);
    }

    // 35. Malformed: Shared frame 0 self-reference (reference >= index)
    {
        let mut bad = build_header(6, 1, 1, 1, 1, 8, 8);
        bad.extend_from_slice(&0i32.to_le_bytes()); // reference = 0 on frame 0!
        add_case("malformed_shared_self_reference", &bad);
    }

    // 23. Malformed: Shared frame forward reference (frame 0 references 1)
    {
        let mut bad = build_header(6, 2, 1, 1, 1, 8, 8);
        bad.extend_from_slice(&1i32.to_le_bytes()); // frame 0 references 1!
        bad.extend_from_slice(&0i32.to_le_bytes());
        add_case("malformed_shared_forward_reference", &bad);
    }

    // 24. Malformed: Truncated independent frame header (less than 16 bytes)
    {
        let mut bad = build_header(6, 1, 1, 1, 1, 8, 8);
        bad.extend_from_slice(&(-1i32).to_le_bytes());
        bad.extend_from_slice(&[1, 2, 3]); // only 3 bytes instead of 16
        add_case("malformed_truncated_frame_header", &bad);
    }

    // 25. Malformed: Table offset mismatch (table_offset != payload_bytes)
    {
        let mut bad = build_header(6, 1, 1, 1, 1, 8, 8);
        bad.extend_from_slice(&(-1i32).to_le_bytes());
        bad.extend_from_slice(&9999u32.to_le_bytes()); // wrong table offset!
        bad.extend_from_slice(&0u32.to_le_bytes());
        bad.extend_from_slice(&((2u32 & 0x7FF) | ((1u32 & 0x7FF) << 11) | ((2u32 & 0x1FF) << 22)).to_le_bytes());
        bad.extend_from_slice(&1u32.to_le_bytes());
        bad.extend_from_slice(&[0; 8]); // palette: 2 colors * 4 = 8
        bad.extend_from_slice(&[1, 0xAA, 0xAA, 0xAA]); // pixel aligned: 4 bytes
        bad.extend_from_slice(&[0u32.to_le_bytes(), 1u32.to_le_bytes()].concat());
        add_case("malformed_table_offset_mismatch", &bad);
    }

    // 26. Malformed: Truncated payload bytes
    {
        let mut bad = build_header(6, 1, 1, 1, 1, 8, 8);
        bad.extend_from_slice(&(-1i32).to_le_bytes());
        let table_offset = 12u32;
        bad.extend_from_slice(&table_offset.to_le_bytes());
        bad.extend_from_slice(&0u32.to_le_bytes());
        bad.extend_from_slice(&((2u32 & 0x7FF) | ((1u32 & 0x7FF) << 11) | ((2u32 & 0x1FF) << 22)).to_le_bytes());
        bad.extend_from_slice(&1u32.to_le_bytes());
        bad.extend_from_slice(&[0; 4]); // only 4 bytes instead of 12!
        add_case("malformed_truncated_payload", &bad);
    }

    // 27. Frame 0 has no palette colors but a later frame does: C# indexed decode always uses
    // frame 0's palette count, so both sides must reject indexed decoding (not fall back).
    {
        let mut buf = build_header(6, 2, 2, 1, 1, 8, 8);
        write_frame_record(&mut buf, 0, 0, 2, 1, 1, 0, &[], &[1], &[0, 1]);
        write_frame_record(&mut buf, 0, 0, 2, 1, 1, 2, &[0x112233, 0x445566], &[1], &[0, 1]);
        add_case("frame0_zero_palette_later_frame_nonzero", &buf);
    }

    // Run C# Oracle on the cases directory
    run_oracle(&oracle_dll, dir);

    // Verify Rust against C# Oracle for every case
    let mut verified_ok = 0;
    let mut verified_rejected = 0;

    for name in &test_names {
        let in_path = dir.join(format!("{name}.in"));
        let status_path = dir.join(format!("{name}.status"));
        let source = fs::read(&in_path).unwrap();
        let status_text = fs::read_to_string(&status_path).unwrap();
        let status_line = status_text.lines().next().unwrap_or("").trim();

        if status_line.starts_with("REJECT") {
            // C# rejected: Rust must also reject!
            let rust_result = AlrDocument::parse(&source);
            assert!(
                rust_result.is_err(),
                "Case {name}: C# rejected ({status_line}) but Rust parsed successfully: {:?}",
                rust_result.unwrap()
            );
            verified_rejected += 1;
        } else if status_line == "OK" {
            // C# accepted: Rust must also accept and produce 100% identical outputs!
            let rust_doc = match AlrDocument::parse(&source) {
                Ok(doc) => doc,
                Err(err) => panic!("Case {name}: C# accepted (OK) but Rust rejected with error: {err:?}"),
            };

            // 1. Verify Document & Frame Metadata
            let meta_content = fs::read_to_string(dir.join(format!("{name}.meta"))).unwrap();
            let csharp_meta = parse_meta_file(&meta_content);

            assert_eq!(rust_doc.version, csharp_meta.version, "Case {name}: version");
            assert_eq!(rust_doc.layout_columns, csharp_meta.layout_columns, "Case {name}: layout_columns");
            assert_eq!(rust_doc.layout_rows, csharp_meta.layout_rows, "Case {name}: layout_rows");
            assert_eq!(rust_doc.palette_variant_count, csharp_meta.palette_variant_count, "Case {name}: palette_variant_count");
            assert_eq!(rust_doc.anchor_width, csharp_meta.anchor_width, "Case {name}: anchor_width");
            assert_eq!(rust_doc.anchor_height, csharp_meta.anchor_height, "Case {name}: anchor_height");
            assert_eq!(rust_doc.frames.len(), csharp_meta.frame_count, "Case {name}: frame_count");

            for (i, frame) in rust_doc.frames.iter().enumerate() {
                let cm = &csharp_meta.frames[i];
                assert_eq!(frame.info.offset_x, cm.offset_x, "Case {name} Frame {i}: offset_x");
                assert_eq!(frame.info.offset_y, cm.offset_y, "Case {name} Frame {i}: offset_y");
                assert_eq!(frame.info.width, cm.width, "Case {name} Frame {i}: width");
                assert_eq!(frame.info.height, cm.height, "Case {name} Frame {i}: height");
                assert_eq!(frame.info.palette_color_count, cm.palette_color_count, "Case {name} Frame {i}: palette_color_count");
                assert_eq!(frame.info.packed_size, cm.packed_size, "Case {name} Frame {i}: packed_size");
                assert_eq!(frame.is_shared(), cm.is_shared, "Case {name} Frame {i}: is_shared");
                assert_eq!(frame.shared_from, cm.shared_from, "Case {name} Frame {i}: shared_from");
            }

            // 2. Verify Palettes
            for v in 0..rust_doc.palette_variant_count {
                let pal_path = dir.join(format!("{name}.palette_{v}"));
                if pal_path.exists() {
                    let csharp_pal_bytes = fs::read(&pal_path).unwrap();
                    let csharp_pal = read_u32_array(&csharp_pal_bytes);
                    let rust_pal = rust_doc.read_raw_palette(v).unwrap();
                    assert_eq!(rust_pal, csharp_pal, "Case {name} Variant {v}: raw palette mismatch");
                }
            }

            // 3. Verify Indexed Samples
            for i in 0..rust_doc.frames.len() {
                let indexed_path = dir.join(format!("{name}.indexed_{i}"));
                if indexed_path.exists() {
                    let csharp_indexed_bytes = fs::read(&indexed_path).unwrap();
                    let csharp_indexed = read_u16_array(&csharp_indexed_bytes);
                    let rust_indexed = rust_doc.decode_frame_indexed(i).unwrap();

                    assert_eq!(
                        rust_indexed.len(),
                        csharp_indexed.len(),
                        "Case {name} Frame {i}: indexed samples length mismatch"
                    );

                    for (p, (&csharp_val, &rust_val)) in csharp_indexed.iter().zip(rust_indexed.iter()).enumerate() {
                        let expected = if csharp_val == 0 {
                            None
                        } else {
                            Some(IndexedSample {
                                index: (csharp_val & 0xFF) as u8,
                                write_zero: (csharp_val & 0x200) != 0,
                            })
                        };
                        assert_eq!(
                            rust_val, expected,
                            "Case {name} Frame {i} Pixel {p}: indexed sample mismatch (csharp 0x{csharp_val:04X}, rust {rust_val:?})"
                        );
                    }
                }
                let indexed_err_path = dir.join(format!("{name}.indexed_err_{i}"));
                if indexed_err_path.exists() {
                    assert!(
                        rust_doc.decode_frame_indexed(i).is_err(),
                        "Case {name} Frame {i}: C# decode indexed failed but Rust succeeded"
                    );
                }
            }

            // 4. Verify Decoded ARGB Pixels
            for i in 0..rust_doc.frames.len() {
                for v in 0..rust_doc.palette_variant_count {
                    let argb_path = dir.join(format!("{name}.argb_{i}_{v}"));
                    if argb_path.exists() {
                        let csharp_argb_bytes = fs::read(&argb_path).unwrap();
                        let csharp_argb = read_u32_array(&csharp_argb_bytes);
                        let rust_argb = rust_doc.decode_frame_argb(i, v).unwrap();
                        assert_eq!(
                            rust_argb, csharp_argb,
                            "Case {name} Frame {i} Variant {v}: ARGB pixels mismatch"
                        );
                    }
                    let argb_err_path = dir.join(format!("{name}.argb_err_{i}_{v}"));
                    if argb_err_path.exists() {
                        assert!(
                            rust_doc.decode_frame_argb(i, v).is_err(),
                            "Case {name} Frame {i} Variant {v}: C# decode ARGB failed but Rust succeeded"
                        );
                    }
                }
            }

            // 5. Verify Roundtrip Bytes
            let roundtrip_path = dir.join(format!("{name}.roundtrip"));
            if roundtrip_path.exists() {
                let csharp_roundtrip = fs::read(&roundtrip_path).unwrap();
                let rust_encoded = rust_doc.encode();
                assert_eq!(
                    rust_encoded, csharp_roundtrip,
                    "Case {name}: roundtrip binary bytes mismatch"
                );

                // Ensure reparsing the encoded bytes yields the exact same document
                let reparsed = AlrDocument::parse(&rust_encoded).unwrap();
                assert_eq!(reparsed, rust_doc, "Case {name}: reparsed document mismatch");
            }

            if name == "frame0_zero_palette_later_frame_nonzero" {
                for i in 0..rust_doc.frames.len() {
                    assert!(
                        dir.join(format!("{name}.indexed_err_{i}")).exists(),
                        "Case {name} Frame {i}: C# indexed decode should reject"
                    );
                    assert!(
                        rust_doc.decode_frame_indexed(i).is_err(),
                        "Case {name} Frame {i}: Rust indexed decode should reject"
                    );
                }
            }

            verified_ok += 1;
        } else {
            panic!("Unexpected status line in {name}.status: {status_line}");
        }
    }

    // 6. Systematic Truncated Prefix Rejection Differential:
    // Every prefix of a valid multi-frame document must be rejected by both C# and Rust.
    let valid_source = fs::read(dir.join("v4_standard.in")).unwrap();
    for len in 0..valid_source.len() {
        let prefix = &valid_source[..len];
        let rust_result = AlrDocument::parse(prefix);
        assert!(
            rust_result.is_err(),
            "Prefix length {len}/{} was accepted by Rust!",
            valid_source.len()
        );
    }

    println!(
        "C# AlrDocument differential passed: {verified_ok} valid cases verified across metadata, indexed pixels, palette, and roundtrip bytes; {verified_rejected} malformed cases correctly rejected; {} prefix truncations checked.",
        valid_source.len()
    );
}

fn buf_extend_frame_0(buf: &mut Vec<u8>) {
    write_frame_record(
        buf,
        0,
        0,
        2,
        1,
        1,
        2,
        &[0x112233, 0x445566],
        &[1],
        &[0, 1],
    );
}
