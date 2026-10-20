use against_rome_environment::shadow::{darken_argb, ShadowDocument};
use serde_json::Value;
use sha2::{Digest, Sha256};
use shad_probe::{bmp::audit_bmp, image, read_shadow_names, run, safety, PREVIEW_LIMIT};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = shad_probe::safety::temp_root().join(format!(
            "shad-probe-rs-tests-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn put32(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

// Equivalent to the existing environment shadow fixture: flat arbitrary palette,
// distinct raw indices, row padding, and both signed-height orientations.
fn bmp(top_down: bool) -> Vec<u8> {
    let mut b = vec![0; 1086];
    b[..2].copy_from_slice(b"BM");
    put32(&mut b, 2, 1086);
    put32(&mut b, 10, 1078);
    put32(&mut b, 14, 40);
    put32(&mut b, 18, 3);
    put32(&mut b, 22, if top_down { (-2i32) as u32 } else { 2 });
    b[26..28].copy_from_slice(&1u16.to_le_bytes());
    b[28..30].copy_from_slice(&8u16.to_le_bytes());
    put32(&mut b, 34, 8);
    put32(&mut b, 46, 256);
    for i in 0..256 {
        put32(&mut b, 54 + i * 4, 0xee204080);
    }
    b[1078..].copy_from_slice(if top_down {
        &[0, 1, 128, 0xaa, 255, 64, 192, 0xbb]
    } else {
        &[255, 64, 192, 0xbb, 0, 1, 128, 0xaa]
    });
    b
}
fn bmp24() -> Vec<u8> {
    let mut b = vec![0; 62];
    b[..2].copy_from_slice(b"BM");
    put32(&mut b, 2, 62);
    put32(&mut b, 10, 54);
    put32(&mut b, 14, 40);
    put32(&mut b, 18, 2);
    put32(&mut b, 22, 1);
    b[26..28].copy_from_slice(&1u16.to_le_bytes());
    b[28..30].copy_from_slice(&24u16.to_le_bytes());
    b
}
fn zip(path: &Path, entries: &[(&str, Vec<u8>)]) {
    let mut z = zip::ZipWriter::new(File::create(path).unwrap());
    for (name, bytes) in entries {
        z.start_file(*name, zip::write::FileOptions::default())
            .unwrap();
        z.write_all(bytes).unwrap();
    }
    z.finish().unwrap();
}
fn png_pixels(path: &Path) -> (u32, u32, Vec<u8>) {
    let reader = png::Decoder::new(File::open(path).unwrap());
    let mut reader = reader.read_info().unwrap();
    let mut b = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut b).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    b.truncate(info.buffer_size());
    (info.width, info.height, b)
}
fn objdef() -> String {
    let mut header = vec!["unused"; 53];
    header[0] = ";idx";
    for (i, c) in ["shidx", "shsiz", "shtyp", "shacx", "shacz"]
        .iter()
        .enumerate()
    {
        header[i + 1] = c;
    }
    let mut s = header.join(",");
    for (id, idx, size, kind, x, z) in [
        (12, 1, 40, 1, 1, 0),
        (42, 2, 48, 0, -3, 2),
        (46, 3, 0, 1, 0, 0),
        (99, 4, 30, 0, 0, 0),
    ] {
        let mut row = vec!["0".to_owned(); 53];
        for (i, v) in [id, idx, size, kind, x, z].iter().enumerate() {
            row[i] = v.to_string();
        }
        row[52] = format!("Object {id}");
        s.push('\n');
        s.push_str(&row.join(","));
    }
    s
}

#[test]
fn bmp_orientations_palette_ignored_alpha_and_native_darkening() {
    for top in [false, true] {
        let b = bmp(top);
        let a = audit_bmp(&b).unwrap();
        assert_eq!(a.stride, 4);
        assert!(a.image_size_matches);
        let d = ShadowDocument::parse(&b).unwrap();
        assert_eq!(d.is_top_down(), top);
        let f = d.decode_frame(0).unwrap();
        assert_eq!(f.alpha_mask(), &[0, 1, 128, 255, 64, 192]);
        assert_eq!(
            f.argb_pixels(),
            &[0, 0x01000000, 0x80000000, 0xfe000000, 0x40000000, 0xbf000000]
        );
        assert!(d.decode_frame(1).is_err());
    }
    assert_eq!(darken_argb(image::GROUND, 0), image::GROUND);
    assert_eq!(darken_argb(image::GROUND, 255), 0xff000000);
    assert_eq!(darken_argb(0x7f80c8ff, 128), 0x7f40647f);
}

#[test]
fn bmp_structural_mutations_truncations_and_palette_bounds() {
    let good = bmp(false);
    for n in 0..good.len() {
        assert!(audit_bmp(&good[..n]).is_err(), "length {n}");
    }
    for (off, v) in [
        (2, 1),
        (6, 1),
        (14, 108),
        (18, 0),
        (18, u32::MAX),
        (22, 0),
        (22, i32::MIN as u32),
        (30, 1),
        (46, 257),
        (10, 54),
        (10, u32::MAX),
    ] {
        let mut b = good.clone();
        put32(&mut b, off, v);
        assert!(audit_bmp(&b).is_err(), "offset {off}");
    }
    for off in [0, 26, 28] {
        let mut b = good.clone();
        b[off] = 0;
        assert!(audit_bmp(&b).is_err());
    }
    let mut b = good.clone();
    put32(&mut b, 46, 128);
    assert_eq!(
        audit_bmp(&b).unwrap_err(),
        "BMP palette index out of range."
    );
    let mut b = good.clone();
    put32(&mut b, 46, 0);
    assert!(audit_bmp(&b).is_ok());
    let mut b = good;
    put32(&mut b, 34, 999);
    assert!(!audit_bmp(&b).unwrap().image_size_matches);
    assert!(ShadowDocument::parse(&b).is_err());
}

#[test]
fn audit_24bit_trailing_gap_and_shadow_only_constraints() {
    let mut b = bmp24();
    let a = audit_bmp(&b).unwrap();
    assert_eq!((a.depth, a.stride, a.actual_image_bytes), (24, 8, 8));
    assert!(ShadowDocument::parse(&b).is_err());
    b.extend([1, 2, 3]);
    let len = b.len() as u32;
    put32(&mut b, 2, len);
    assert_eq!(audit_bmp(&b).unwrap().trailing_bytes, 3);
    let mut b = bmp(false);
    put32(&mut b, 50, 257);
    assert!(audit_bmp(&b).is_ok());
    assert!(ShadowDocument::parse(&b).is_err());
    let mut b = bmp(false);
    b.splice(1078..1078, [9, 8, 7, 6]);
    put32(&mut b, 10, 1082);
    put32(&mut b, 2, 1090);
    assert!(audit_bmp(&b).is_ok());
    assert_eq!(
        ShadowDocument::parse(&b).unwrap().mask(),
        &[0, 1, 128, 255, 64, 192]
    );
    b.extend([11, 12]);
    put32(&mut b, 2, 1092);
    assert_eq!(ShadowDocument::parse(&b).unwrap().trailing_byte_count(), 2);
}

#[test]
fn shadow_names_strict_decoded_grammar() {
    assert_eq!(
        read_shadow_names("[ShadowNames]\r1,a.bmp\r2,b.bmp")
            .unwrap()
            .len(),
        2
    );
    let n=read_shadow_names("[Ignored]\ninvalid\n[shadownames]\n; comment\n 1 , texture,with,commas.bmp\n1999,last.bmp\n[other]\nmalformed").unwrap();
    assert_eq!(n[&1], "texture,with,commas.bmp");
    assert_eq!(n.len(), 2);
    for s in [
        "PFIL",
        "[ShadowNames]\n",
        "[ShadowNames]\n0",
        "[ShadowNames]\n-1,a.bmp",
        "[ShadowNames]\n2000,a.bmp",
        "[ShadowNames]\n1,a.bmp\n1,b.bmp",
        "[ShadowNames]\n1,../a.bmp",
        "[ShadowNames]\n1,a\\b.bmp",
        "[ShadowNames]\n1,",
        "[ShadowNames]\n1,NUL.bmp",
        "[ShadowNames]\n1,a.bmp.",
    ] {
        assert!(read_shadow_names(s).is_err(), "{s}");
    }
}

#[test]
fn path_validation_rejects_traversal_devices_and_aliases() {
    for name in [
        "../a.bmp",
        "SYSTEM/../a.bmp",
        "/a.bmp",
        "a//b.bmp",
        "a/./b.bmp",
        "C:/a.bmp",
        "a\\b.bmp",
        "a.bmp:stream",
        "NUL.bmp",
        "COM1.bmp",
        "a.bmp.",
        "a.bmp ",
        "a/",
        "a\0.bmp",
    ] {
        // a/ is a valid directory; test the malformed empty-root case separately.
        if name == "a/" {
            assert!(safety::archive_path(name).is_ok());
            continue;
        }
        assert!(safety::archive_path(name).is_err(), "{name}");
    }
    assert!(safety::archive_path("/").is_err());
    let mut seen = std::collections::HashSet::new();
    safety::register_path("A/B.bmp", &mut seen).unwrap();
    assert!(safety::register_path("a/b.BMP", &mut seen).is_err());
    assert!(safety::component("PROGRA~1").is_err());
    assert!(safety::component("NUL .bmp").is_err());
    safety::register_path("Ä.bmp", &mut seen).unwrap();
    assert!(safety::register_path("ä.BMP", &mut seen).is_err());
}

#[test]
fn output_must_be_new_temp_child_and_never_overwrites() {
    let t = Temp::new();
    let temp = shad_probe::safety::temp_root();
    assert!(safety::validate_output(&temp, &temp).is_err());
    assert!(safety::validate_output(&t.0, &temp).is_err());
    assert!(safety::validate_output(&temp.with_extension("evil").join("new"), &temp).is_err());
    assert!(safety::validate_output(&t.join("../escape"), &temp).is_err());
    assert!(safety::validate_output(&t.join("a."), &temp).is_err());
    fs::write(t.join("existing"), b"keep").unwrap();
    assert!(safety::create_output(&t.join("existing")).is_err());
    assert_eq!(fs::read(t.join("existing")).unwrap(), b"keep");
    let nested = t.join("missing/new");
    assert_eq!(safety::create_output(&nested).unwrap(), nested);
    assert!(safety::create_output(&nested).is_err());
}

#[test]
fn png_rgba_roundtrip_json_schema_and_no_overwrite() {
    let t = Temp::new();
    let p = t.join("p.png");
    image::write_png(&p, 2, 1, &[0x12345678, 0xffabcdef]).unwrap();
    assert_eq!(
        png_pixels(&p),
        (2, 1, vec![0x34, 0x56, 0x78, 0x12, 0xab, 0xcd, 0xef, 0xff])
    );
    let before = fs::read(&p).unwrap();
    assert!(image::write_png(&p, 2, 1, &[0, 0]).is_err());
    assert_eq!(fs::read(p).unwrap(), before);
    assert!(image::write_png(&t.join("bad.png"), 2, 2, &[0]).is_err());
    assert!(!t.join("bad.png").exists());
    let sample = shad_probe::Sample {
        entry: "e".into(),
        file: "s.png".into(),
        ground_file: "g.png".into(),
        width: 3,
        height: 2,
    };
    let v = serde_json::to_value(sample).unwrap();
    assert_eq!(v["Width"], 3);
    assert_eq!(v["Height"], 2);
    assert!(v.get("width").is_none());
    assert_eq!(v["groundFile"], "g.png");
}

#[test]
fn full_zip_inventory_hash_samples_and_failures() {
    let t = Temp::new();
    let input = t.join("input.zip");
    let output = t.join("out");
    zip(
        &input,
        &[
            ("SYSTEM/", vec![]),
            ("other.bmp", bmp24()),
            ("not-a-bmp.txt", b"bad".to_vec()),
            ("SYSTEM/DATA/SHADOWTEXTURE/GerHau02_shadow.bmp", bmp(false)),
            ("SYSTEM/DATA/SHADOWTEXTURE/LakaDorn_shadow.bmp", bmp(true)),
            ("invalid-dir/", vec![1]),
        ],
    );
    let input_bytes = fs::read(&input).unwrap();
    let r = run(&input, &output, None, None).unwrap();
    assert_eq!(
        (
            r.entries,
            r.documents,
            r.frames,
            r.directories,
            r.other_bmps,
            r.failures.len(),
            r.samples.len()
        ),
        (6, 2, 2, 1, 1, 2, 2)
    );
    assert_eq!(r.hash, format!("{:X}", Sha256::digest(&input_bytes)));
    assert_eq!(fs::read(input).unwrap(), input_bytes);
    let report: Value =
        serde_json::from_slice(&fs::read(output.join("report.json")).unwrap()).unwrap();
    assert_eq!(report["inputBytes"], input_bytes.len());
    assert_eq!(report["inventory"][2]["TrailingByteCount"], 0);
    assert_eq!(report["inventory"][2]["minStrength"], 0);
    assert_eq!(report["inventory"][2]["maxStrength"], 255);
    assert_eq!(report["failures"][0]["error"], "Missing/truncated BMP.");
    let (_, _, pixels) = png_pixels(&output.join("sample-0.png"));
    assert_eq!(
        pixels
            .iter()
            .skip(3)
            .step_by(4)
            .copied()
            .collect::<Vec<_>>(),
        [0, 1, 128, 254, 64, 191]
    );
    let (_, _, ground) = png_pixels(&output.join("sample-0-ground.png"));
    assert_eq!(&ground[0..4], &[0x94, 0xb8, 0x70, 0xff]);
    assert_eq!(ground, png_pixels(&output.join("sample-1-ground.png")).2);
}

#[test]
fn zip_unsafe_paths_and_duplicate_leaf_fail_without_extraction() {
    let t = Temp::new();
    let input = t.join("input.zip");
    zip(
        &input,
        &[
            ("../escape.bmp", bmp(false)),
            ("NUL.bmp", bmp(false)),
            ("a.bmp", bmp(false)),
            ("A.BMP", bmp(false)),
            ("SYSTEM/DATA/SHADOWTEXTURE/sub/a.bmp", bmp(false)),
            ("SYSTEM/DATA/SHADOWTEXTURE/other/A.BMP", bmp(false)),
        ],
    );
    let r = run(&input, &t.join("out"), None, None).unwrap();
    assert_eq!(r.failures.len(), 4);
    assert_eq!(r.other_bmps, 1);
    assert_eq!((r.documents, r.frames), (2, 2));
    assert!(!t.join("escape.bmp").exists());
}

#[test]
fn object_candidates_mapping_missing_no_fallback_and_disclosures() {
    let t = Temp::new();
    let input = t.join("input.zip");
    let obj = t.join("obj.txt");
    let names = t.join("names.txt");
    zip(
        &input,
        &[
            (
                "SYSTEM/DATA/SHADOWTEXTURE/round_small_shadow.bmp",
                bmp(false),
            ),
            ("SYSTEM/DATA/SHADOWTEXTURE/GerHau02_shadow.bmp", bmp(true)),
            (
                "SYSTEM/DATA/SHADOWTEXTURE/GerNadelbaum_shadow.bmp",
                bmp(false),
            ),
        ],
    );
    fs::write(&obj, objdef()).unwrap();
    let r = run(&input, &t.join("candidate"), Some(&obj), None).unwrap();
    assert_eq!(r.objects.len(), 3);
    for o in &r.objects {
        assert_eq!(o.status, "UNVERIFIED candidate; ShadowNames missing");
        assert!(o.found);
        assert_eq!(o.frame, 0);
        assert_eq!(o.preview_limit, PREVIEW_LIMIT);
    }
    assert_eq!(r.objects[0].correction_screen_x, 0.5);
    assert_eq!(r.objects[0].correction_screen_y, 0.25);
    assert_eq!(
        png_pixels(&t.join("candidate/object-12-ground-assumption.png")).0,
        720
    );
    fs::write(
        &names,
        "[ShadowNames]\n1,GerHau02_shadow.bmp\n2,missing.bmp",
    )
    .unwrap();
    let r = run(&input, &t.join("mapped"), Some(&obj), Some(&names)).unwrap();
    assert_eq!(
        r.objects[0].mapped_entry.as_deref(),
        Some("GerHau02_shadow.bmp")
    );
    assert!(r.objects[0].candidate_entry.is_none());
    assert_eq!(r.objects[0].status, "ID from supplied ShadowNames");
    assert!(!r.objects[1].found);
    assert!(r.objects[1].preview.is_none());
    assert!(r.objects[1].candidate_entry.is_none());
    assert!(r.objects[2].candidate_entry.is_some());
}

#[test]
fn ground_projection_circle_square_cross_ties_even_and_no_radius() {
    let f = ShadowDocument::parse(&bmp(false))
        .unwrap()
        .decode_frame(0)
        .unwrap();
    let p = image::ground_preview(&f, 0, 1, 1, 2);
    assert_eq!(p[200 * 720 + 360], 0xff20ffff); // 359.5 rounds to 360; 200.75 to 201, vertical cross covers origin.
    assert_eq!(p[201 * 720 + 360], 0xff20ffff);
    assert_eq!(p[0], image::GROUND);
    let circle = image::ground_preview(&f, 60, 1, 0, 0);
    let square = image::ground_preview(&f, 60, 0, 0, 0);
    assert_ne!(circle, square);
    assert_eq!(circle.len(), 720 * 400);
    let p = image::ground_preview(&f, 0, 0, i32::MAX, i32::MIN);
    assert_eq!(p[200 * 720 + 360], 0xff20ffff); // C# unchecked int correction arithmetic wraps.
}

#[test]
fn cli_exit_codes_and_optional_malformed_text() {
    let t = Temp::new();
    let bin = env!("CARGO_BIN_EXE_shad-probe-rs");
    assert_eq!(Command::new(bin).output().unwrap().status.code(), Some(2));
    let input = t.join("input.zip");
    zip(&input, &[("ordinary.bmp", bmp24())]);
    let status = Command::new(bin)
        .args([&input, &t.join("ok")])
        .output()
        .unwrap();
    assert!(status.status.success(), "{:?}", status);
    assert_eq!(
        Command::new(bin)
            .args([&input, &t.join("ok")])
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
    zip(&input, &[("bad.bmp", vec![1])]);
    assert_eq!(
        Command::new(bin)
            .args([&input, &t.join("fail")])
            .output()
            .unwrap()
            .status
            .code(),
        Some(1)
    );
    fs::write(t.join("badobj"), "12,broken").unwrap();
    assert!(run(&input, &t.join("badobj-out"), Some(&t.join("badobj")), None).is_err());
    fs::write(t.join("names"), b"PFIL\x00\x01").unwrap();
    assert!(run(&input, &t.join("pfil-out"), None, Some(&t.join("names"))).is_err());
    assert!(!t.join("pfil-out").exists());
}

#[test]
fn utf16_decoded_text_input_is_supported() {
    let t = Temp::new();
    let input = t.join("input.zip");
    zip(&input, &[]);
    let text = objdef();
    let mut b = vec![0xff, 0xfe];
    for v in text.encode_utf16() {
        b.extend(v.to_le_bytes());
    }
    fs::write(t.join("obj"), b).unwrap();
    let r = run(&input, &t.join("out"), Some(&t.join("obj")), None).unwrap();
    assert_eq!(r.objects.len(), 3);
    assert!(r.objects.iter().all(|o| !o.found));
}

#[test]
fn crc_corruption_is_reported_and_processing_continues() {
    let t = Temp::new();
    let input = t.join("input.zip");
    zip(&input, &[("bad.bmp", bmp(false)), ("good.bmp", bmp24())]);
    let mut b = fs::read(&input).unwrap();
    // Corrupt the central-directory CRC, preserving structure and compressed data.
    let central = b.windows(4).position(|v| v == b"PK\x01\x02").unwrap();
    b[central + 16] ^= 1;
    fs::write(&input, b).unwrap();
    let r = run(&input, &t.join("out"), None, None).unwrap();
    assert_eq!(r.failures.len(), 1);
    assert_eq!(r.other_bmps, 1);
}

/// Compares Rust output to the frozen C# oracle output (report.json + every PNG) committed under
/// `tests/fixtures/csharp_oracle/{plain,mapped,edge}`; no .NET required.
#[test]
fn frozen_csharp_json_and_all_png_pixels_match() {
    let t = Temp::new();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/csharp_oracle");
    let input = t.join("input.zip");
    let mut mismatch = bmp24();
    put32(&mut mismatch, 34, 999);
    let mut bad_shadow = bmp(false);
    put32(&mut bad_shadow, 34, 1);
    zip(
        &input,
        &[
            ("SYSTEM/", vec![]),
            ("ordinary.bmp", mismatch),
            ("not-bmp", vec![7]),
            (
                "SYSTEM/DATA/SHADOWTEXTURE/round_small_shadow.bmp",
                bmp(false),
            ),
            ("SYSTEM/DATA/SHADOWTEXTURE/GerHau02_shadow.bmp", bmp(true)),
            (
                "SYSTEM/DATA/SHADOWTEXTURE/GerNadelbaum_shadow.bmp",
                bmp(false),
            ),
            ("SYSTEM/DATA/SHADOWTEXTURE/LakaDorn_shadow.bmp", bmp(true)),
            ("SYSTEM/DATA/SHADOWTEXTURE/error.bmp", bmp(false)),
            ("SYSTEM/DATA/SHADOWTEXTURE/badsize.bmp", bad_shadow),
            ("SYSTEM/DATA/SHADOWTEXTURE/unsupported.bmp", bmp24()),
        ],
    );
    let obj = t.join("obj.txt");
    fs::write(&obj, objdef()).unwrap();
    let names = t.join("names.txt");
    fs::write(
        &names,
        "[ShadowNames]\n1,GerNadelbaum_shadow.bmp\n2,missing.bmp",
    )
    .unwrap();
    // Both modes: all 3 candidate object PNGs, then supplied mappings including
    // a missing texture (which must suppress rather than fall back).
    for (mapped, extreme) in [(false, false), (true, false), (false, true)] {
        if extreme {
            fs::write(
                &obj,
                objdef().replace("12,1,40,1,1,0,", "12,1,40,1,2147483647,-2147483648,"),
            )
            .unwrap();
        }
        let cs = fixtures.join(if extreme { "edge" } else if mapped { "mapped" } else { "plain" });
        let rs = t.join(if extreme {
            "rs-edge"
        } else if mapped {
            "rs-mapped"
        } else {
            "rs"
        });
        run(&input, &rs, Some(&obj), mapped.then_some(names.as_path())).unwrap();
        let cs_report: Value =
            serde_json::from_slice(&fs::read(cs.join("report.json")).unwrap()).unwrap();
        let rs_report: Value =
            serde_json::from_slice(&fs::read(rs.join("report.json")).unwrap()).unwrap();
        fn compare(a: &Value, b: &Value, path: &str) {
            match (a, b) {
                (Value::Number(a), Value::Number(b)) => {
                    assert_eq!(a.as_f64(), b.as_f64(), "{path}")
                }
                (Value::Object(a), Value::Object(b)) => {
                    assert_eq!(a.len(), b.len(), "{path}");
                    for (k, v) in a {
                        compare(v, &b[k], &format!("{path}/{k}"));
                    }
                }
                (Value::Array(a), Value::Array(b)) => {
                    assert_eq!(a.len(), b.len(), "{path}");
                    for (i, (a, b)) in a.iter().zip(b).enumerate() {
                        compare(a, b, &format!("{path}/{i}"));
                    }
                }
                _ => assert_eq!(a, b, "{path}"),
            }
        }
        compare(&cs_report, &rs_report, "");
        let mut cs_files: Vec<_> = fs::read_dir(&cs)
            .unwrap()
            .map(|v| v.unwrap().file_name())
            .collect();
        cs_files.sort();
        let mut rs_files: Vec<_> = fs::read_dir(&rs)
            .unwrap()
            .map(|v| v.unwrap().file_name())
            .collect();
        rs_files.sort();
        assert_eq!(cs_files, rs_files);
        for file in cs_files {
            if Path::new(&file).extension().is_some_and(|v| v == "png") {
                assert_eq!(
                    png_pixels(&cs.join(&file)),
                    png_pixels(&rs.join(&file)),
                    "{:?}",
                    file
                );
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn symlink_output_ancestor_and_input_are_rejected() {
    use std::os::unix::fs::symlink;
    let t = Temp::new();
    let outside = Temp::new();
    symlink(&outside.0, t.join("link")).unwrap();
    assert!(safety::create_output(&t.join("link/out")).is_err());
    fs::write(outside.join("input"), b"data").unwrap();
    symlink(outside.join("input"), t.join("input")).unwrap();
    assert!(safety::input(&t.join("input")).is_err());
}

// Windows symlink creation requires Developer Mode/admin. Junctions do not, so
// this test exercises the real reparse-point check on a normal Windows account.
#[cfg(windows)]
#[test]
fn windows_junction_ancestor_is_rejected() {
    let t = Temp::new();
    let outside = Temp::new();
    let link = t.join("junction");
    let result = Command::new("cmd")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(&link)
        .arg(&outside.0)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(safety::create_output(&link.join("new")).is_err());
    fs::write(outside.join("input"), b"keep").unwrap();
    assert!(safety::input(&link.join("input")).is_err());
    fs::remove_dir(link).unwrap();
    assert_eq!(fs::read(outside.join("input")).unwrap(), b"keep");
}
