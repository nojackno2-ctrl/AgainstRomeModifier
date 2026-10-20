use against_rome_data::apt::AptDocument;
use against_rome_data::apt_geometry::AptVector3;
use apt_probe::{argb_to_rgba, cli, run_probe, scan_attachments, AttachmentStats, ProbeError};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "apt-probe-rs-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn row_width(y: usize) -> usize {
    if y <= 15 {
        (y + 1) * 4
    } else {
        (31 - y) * 4
    }
}

/// Valid APAT with layout [1,2,1,1] (1 group, 2 light variants), 2 tiles, 2 palette
/// variants. Every selection has 1 light + 1 smoke point at (0,0). Origin (32,20),
/// anchor (30,15). `frames` are (first_tile, tile_count) ranges.
fn fixture(version: u32, frames: &[(u32, u32)], marker: (i32, i32)) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut header = [0u32; 28];
    header[0] = 0x54415041;
    header[1] = version;
    header[2] = frames.len() as u32;
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
    buf.extend_from_slice(&marker.0.to_le_bytes());
    buf.extend_from_slice(&marker.1.to_le_bytes());
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
    for (first, count) in frames {
        buf.extend_from_slice(&first.to_le_bytes());
        buf.extend_from_slice(&count.to_le_bytes());
    }
    for w in [0xAA556677u32, 0xBB112233, 0xCC778899, 0xDDABCDEF] {
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

const NO_MARKER: (i32, i32) = (-1_000_000, -1_000_000);

fn simple(version: u32) -> Vec<u8> {
    fixture(version, &[(0, 1), (0, 2)], NO_MARKER)
}

/// 1001 frames so that frame index 1000 exists for sample export.
fn big_frames() -> Vec<u8> {
    let mut frames = vec![(0u32, 1u32); 1001];
    frames[1] = (0, 2);
    fixture(3, &frames, NO_MARKER)
}

fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let mut z = zip::ZipWriter::new(File::create(path).unwrap());
    let opts = zip::write::FileOptions::default();
    for (name, data) in entries {
        z.start_file(*name, opts).unwrap();
        z.write_all(data).unwrap();
    }
    z.finish().unwrap();
}

fn report_text(out: &Path) -> String {
    fs::read_to_string(out.join("report.json")).unwrap()
}

fn report_json(out: &Path) -> serde_json::Value {
    serde_json::from_str(&report_text(out)).unwrap()
}

fn sha_hex(path: &Path) -> String {
    Sha256::digest(fs::read(path).unwrap())
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect()
}

fn args(items: &[&Path]) -> Vec<OsString> {
    items.iter().map(|p| p.as_os_str().to_owned()).collect()
}

fn run_cli(a: &[OsString]) -> (i32, String, String) {
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let code = cli(a, &mut o, &mut e);
    (
        code,
        String::from_utf8(o).unwrap(),
        String::from_utf8(e).unwrap(),
    )
}

#[test]
fn fixtures_are_valid_apt() {
    for v in [2, 3] {
        let d = AptDocument::parse(&simple(v)).unwrap();
        assert_eq!(d.layout, [1, 2, 1, 1]);
        assert_eq!(d.tiles.len(), 2);
    }
    assert_eq!(
        AptDocument::parse(&big_frames()).unwrap().frames.len(),
        1001
    );
}

#[test]
fn rgba_channel_layout_is_r_g_b_a() {
    assert_eq!(
        argb_to_rgba(&[0xFF332211, 0x80010203]),
        [0x33, 0x22, 0x11, 0xFF, 0x01, 0x02, 0x03, 0x80]
    );
}

#[test]
fn attachment_stats_and_geometry_match_expected_native_placement() {
    let doc = AptDocument::parse(&fixture(3, &[(0, 1)], (32, 20))).unwrap();
    let mut stats = AttachmentStats::default();
    let mut placed: Vec<AptVector3> = Vec::new();
    scan_attachments(&doc, &mut stats, |p| placed.push(p)).unwrap();
    assert_eq!(
        stats,
        AttachmentStats {
            selections: 2,
            light_points: 2,
            smoke_points: 2,
            markers: 2
        }
    );
    // Per selection: light (0,0), smoke (0,0), marker (32,20).
    // project(0,0) = (-2,-1), project(origin) = (0,0), anchor project = (-1,-1).
    let point = AptVector3::new(-32.0, 0.0, 32.0);
    let marker = AptVector3::new(96.0, 0.0, 96.0);
    assert_eq!(placed, vec![point, point, marker, point, point, marker]);

    let none = AptDocument::parse(&simple(3)).unwrap();
    let mut stats = AttachmentStats::default();
    let mut count = 0;
    scan_attachments(&none, &mut stats, |_| count += 1).unwrap();
    assert_eq!((stats.markers, count), (0, 4));
}

#[test]
fn full_mode_counts_and_report_structure() {
    let t = TempDir::new();
    let zip_path = t.0.join("in").join("apt.dat");
    fs::create_dir_all(zip_path.parent().unwrap()).unwrap();
    write_zip(
        &zip_path,
        &[
            ("a/one.apt", &simple(3)),
            ("a/two.apt", &fixture(2, &[(0, 1), (0, 2)], (32, 20))),
            ("a/junk.apt", b"not apt"),
        ],
    );
    let before = fs::read(&zip_path).unwrap();
    let out = t.0.join("out");
    let r = run_probe(&zip_path, &out, false).unwrap();
    assert_eq!(fs::read(&zip_path).unwrap(), before);

    assert_eq!((r.entries, r.documents), (3, 2));
    // 2 documents x 2 palette variants x (2 tiles / 2 frames)
    assert_eq!((r.tiles, r.frames), (8, 8));
    assert_eq!(r.failures.len(), 1);
    assert_eq!(r.failures[0].entry, "a/junk.apt");
    assert_eq!(r.hash, sha_hex(&zip_path));
    assert_eq!(r.hash, r.hash_after);
    assert!(r.samples.is_empty());

    let text = report_text(&out);
    let order: Vec<usize> = [
        "\"hash\"",
        "\"hashAfter\"",
        "\"attachmentsOnly\"",
        "\"entries\"",
        "\"documents\"",
        "\"tiles\"",
        "\"frames\"",
        "\"failures\"",
        "\"samples\"",
        "\"attachments\"",
    ]
    .iter()
    .map(|k| text.find(k).unwrap_or_else(|| panic!("missing {k}")))
    .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
    assert!(text.contains("\"lightPoints\"") && text.contains("\"ProjectionOrigin\""));

    let v = report_json(&out);
    assert_eq!(v["attachmentsOnly"], false);
    let a = &v["attachments"];
    assert_eq!(a.as_array().unwrap().len(), 2);
    assert_eq!(a[0]["entry"], "a/one.apt");
    assert_eq!(a[0]["Version"], 3);
    assert_eq!(a[0]["Layout"], serde_json::json!([1, 2, 1, 1]));
    assert_eq!(
        a[0]["ProjectionOrigin"],
        serde_json::json!({"X": 32, "Y": 20})
    );
    assert_eq!(a[0]["anchor"], serde_json::json!({"X": 30, "Y": 15}));
    assert_eq!(
        (
            a[0]["groups"].as_u64(),
            a[0]["selections"].as_u64(),
            a[0]["lightPoints"].as_u64()
        ),
        (Some(1), Some(2), Some(2))
    );
    assert_eq!(
        (a[0]["smokePoints"].as_u64(), a[0]["markers"].as_u64()),
        (Some(2), Some(0))
    );
    assert_eq!(a[1]["Version"], 2);
    assert_eq!(a[1]["markers"], 2);
}

#[test]
fn attachments_only_skips_decoding_and_samples() {
    let t = TempDir::new();
    let zip_path = t.0.join("in").join("apt.dat");
    fs::create_dir_all(zip_path.parent().unwrap()).unwrap();
    write_zip(&zip_path, &[("x/gerhau02.apt", &big_frames())]);
    let out = t.0.join("out");
    let r = run_probe(&zip_path, &out, true).unwrap();
    assert_eq!((r.documents, r.tiles, r.frames), (1, 0, 0));
    assert!(r.samples.is_empty() && r.failures.is_empty());
    assert_eq!(r.attachments.len(), 1);
    assert_eq!(report_json(&out)["attachmentsOnly"], true);
    let files: Vec<_> = fs::read_dir(&out)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(files, vec![OsString::from("report.json")]);
}

#[test]
fn safe_samples_use_generated_names_and_rgba_bytes() {
    let t = TempDir::new();
    let zip_path = t.0.join("in").join("apt.dat");
    fs::create_dir_all(zip_path.parent().unwrap()).unwrap();
    let apt = big_frames();
    write_zip(
        &zip_path,
        &[
            ("Buildings/GERHAU02.APT", &apt),
            ("../evil/gerhau02.apt", &apt),
            ("other.apt", &apt),
        ],
    );
    let out = t.0.join("out");
    let r = run_probe(&zip_path, &out, false).unwrap();
    // "Buildings/GERHAU02.APT" and "../evil/gerhau02.apt" match; frames 0 and 1000 each.
    assert_eq!(r.samples.len(), 4);
    assert_eq!(r.documents, 3);
    for (i, s) in r.samples.iter().enumerate() {
        assert_eq!(s.file, format!("sample-{i}.rgba"));
        assert_eq!(
            (s.width, s.height, s.anchor_x, s.anchor_y),
            (64, 31, 30, 15)
        );
        assert_eq!(s.frame_index, if i % 2 == 0 { 0 } else { 1000 });
        let bytes = fs::read(out.join(&s.file)).unwrap();
        assert_eq!(bytes.len(), 64 * 31 * 4);
        assert_eq!(
            &bytes[30 * 4..34 * 4],
            &[0x33, 0x22, 0x11, 0xFF, 0, 0, 0, 0, 0x33, 0x22, 0x11, 0xFF, 0, 0, 0, 0]
        );
    }
    assert_eq!(r.samples[1].entry, "Buildings/GERHAU02.APT");
    assert_eq!(r.samples[2].entry, "../evil/gerhau02.apt");
    let v = report_json(&out);
    assert_eq!(v["samples"][0]["frameIndex"], 0);
    assert_eq!(v["samples"][0]["Width"], 64);
    assert!(v["samples"][0].get("AnchorX").is_some());
    // Nothing escaped the output directory.
    assert!(!t.0.join("evil").exists());
    assert!(!t.0.join("in").join("evil").exists());
    let mut names: Vec<String> = fs::read_dir(&out)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "report.json",
            "sample-0.rgba",
            "sample-1.rgba",
            "sample-2.rgba",
            "sample-3.rgba"
        ]
    );
}

#[test]
fn output_safety_rules() {
    let t = TempDir::new();
    let dir = t.0.join("in");
    fs::create_dir_all(&dir).unwrap();
    let zip_path = dir.join("apt.dat");
    write_zip(&zip_path, &[("a.apt", &simple(3))]);

    let inside = dir.join("out");
    assert!(matches!(
        run_probe(&zip_path, &inside, false),
        Err(ProbeError::OutputInsideSource(_))
    ));
    assert!(!inside.exists());
    // Case-insensitive like the C# StartsWith(OrdinalIgnoreCase).
    let upper = PathBuf::from(dir.to_string_lossy().to_uppercase()).join("out2");
    assert!(matches!(
        run_probe(&zip_path, &upper, false),
        Err(ProbeError::OutputInsideSource(_))
    ));
    assert!(!inside.with_file_name("out2").exists());

    let existing = t.0.join("existing");
    fs::create_dir_all(&existing).unwrap();
    fs::write(existing.join("keep.txt"), b"keep").unwrap();
    assert!(matches!(
        run_probe(&zip_path, &existing, false),
        Err(ProbeError::OutputExists(_))
    ));
    assert_eq!(fs::read(existing.join("keep.txt")).unwrap(), b"keep");
    let file_out = t.0.join("file.out");
    fs::write(&file_out, b"x").unwrap();
    assert!(matches!(
        run_probe(&zip_path, &file_out, false),
        Err(ProbeError::OutputExists(_))
    ));
    assert_eq!(fs::read(&file_out).unwrap(), b"x");
}

#[test]
fn cli_exit_codes_and_summary() {
    let t = TempDir::new();
    let dir = t.0.join("in");
    fs::create_dir_all(&dir).unwrap();
    let zip_path = dir.join("apt.dat");
    write_zip(&zip_path, &[("a.apt", &simple(3))]);

    // Usage errors.
    let (code, _, err) = run_cli(&[]);
    assert_eq!(code, 2);
    assert!(err.contains(
        "Usage: apt-probe <readonly apt.dat> <new output directory> [--attachments-only]"
    ));
    let out = t.0.join("o");
    let mut bad = args(&[&zip_path, &out]);
    bad.push("--nope".into());
    assert_eq!(run_cli(&bad).0, 2);
    let mut four = args(&[&zip_path, &out]);
    four.push("--attachments-only".into());
    four.push("extra".into());
    assert_eq!(run_cli(&four).0, 2);
    assert_eq!(run_cli(&args(&[&zip_path])).0, 2);
    assert!(!out.exists());

    // Conflicts.
    let (code, _, err) = run_cli(&args(&[&zip_path, &dir.join("o")]));
    assert_eq!(code, 2);
    assert!(err.contains("outside the source archive directory"));
    let existing = t.0.join("existing");
    fs::create_dir_all(&existing).unwrap();
    let (code, _, err) = run_cli(&args(&[&zip_path, &existing]));
    assert_eq!(code, 2);
    assert!(err.contains("existing files are never overwritten"));

    // Success.
    let (code, out_text, _) = run_cli(&args(&[&zip_path, &out]));
    assert_eq!(code, 0);
    assert_eq!(
        out_text.trim(),
        "Documents=1 PaletteTiles=4 PaletteFrames=4 Failures=0"
    );

    // Attachments-only success.
    let ao = t.0.join("ao");
    let mut a = args(&[&zip_path, &ao]);
    a.push("--attachments-only".into());
    let (code, out_text, _) = run_cli(&a);
    assert_eq!(code, 0);
    assert_eq!(
        out_text.trim(),
        "Documents=1 PaletteTiles=0 PaletteFrames=0 Failures=0"
    );

    // Scan failure => 1 and report still written.
    let bad_zip = dir.join("bad.dat");
    write_zip(&bad_zip, &[("junk.apt", b"junk")]);
    let fail_out = t.0.join("fail");
    let (code, out_text, _) = run_cli(&args(&[&bad_zip, &fail_out]));
    assert_eq!(code, 1);
    assert!(out_text.contains("Failures=1"));
    assert!(fail_out.join("report.json").exists());

    // Unreadable input => 1.
    let (code, _, _) = run_cli(&args(&[&dir.join("missing.dat"), &t.0.join("m")]));
    assert_eq!(code, 1);
}
