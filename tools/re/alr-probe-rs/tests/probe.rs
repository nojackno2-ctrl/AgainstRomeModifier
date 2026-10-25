use against_rome_data::alr::AlrDocument;
use alr_probe::{argb_to_rgba, cli, run_probe, ProbeError};
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};



struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        let p = crate::temp_fixture::dir("alr-probe-rs");
        fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn size(width: usize, height: usize, colors: usize) -> u32 {
    (width as u32 & 0x7FF) | ((height as u32 & 0x7FF) << 11) | ((colors as u32 & 0x1FF) << 22)
}

fn record(
    buf: &mut Vec<u8>,
    ox: i32,
    oy: i32,
    width: usize,
    palette: &[u32],
    pixels: &[u8],
    rows: &[u32],
) {
    buf.extend_from_slice(&(-1i32).to_le_bytes());
    let table_offset = (palette.len() * 4 + ((pixels.len() + 3) & !3)) as u32;
    buf.extend_from_slice(&table_offset.to_le_bytes());
    let pos = ((ox as u32) & 0xFFFF) | (((oy as u32) & 0xFFFF) << 16);
    buf.extend_from_slice(&pos.to_le_bytes());
    buf.extend_from_slice(&size(width, 1, palette.len() / 2).to_le_bytes());
    buf.extend_from_slice(&(pixels.len() as u32).to_le_bytes());
    for c in palette {
        buf.extend_from_slice(&c.to_le_bytes());
    }
    buf.extend_from_slice(pixels);
    let rem = pixels.len() & 3;
    if rem != 0 {
        buf.extend(std::iter::repeat(0xAA).take(4 - rem));
    }
    for r in rows {
        buf.extend_from_slice(&r.to_le_bytes());
    }
}

/// 4 frames (record, shared 0, record without palette, shared 1), 2 palette variants.
fn alr_fixture() -> Vec<u8> {
    let mut buf = Vec::new();
    let header: [u32; 23] = [
        0x41524C41, 6, 0x100, 4, 8, 0x200, 2, 2, 2, 0x300, 0x401, 0x402, 0x403, 0x404, 0x405, 12,
        18, 0x501, 0x502, 0x503, 0x504, 0x601, 0x602,
    ];
    for w in header {
        buf.extend_from_slice(&w.to_le_bytes());
    }
    record(
        &mut buf,
        7,
        11,
        5,
        &[0x405060, 0x102030, 0xC0D0E0, 0x90A0B0],
        &[1, 0, 1],
        &[1u32 << 21, 3],
    );
    buf.extend_from_slice(&0i32.to_le_bytes());
    record(&mut buf, 2, 3, 2, &[], &[1, 0], &[0x80000000, 2]);
    buf.extend_from_slice(&1i32.to_le_bytes());
    buf
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

fn report_json(out: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(out.join("report.json")).unwrap()).unwrap()
}

#[test]
fn fixture_is_valid_alr() {
    let doc = AlrDocument::parse(&alr_fixture()).unwrap();
    assert_eq!((doc.frames.len(), doc.palette_variant_count), (4, 2));
}

#[test]
fn rgba_channel_layout_is_r_g_b_a() {
    assert_eq!(
        argb_to_rgba(&[0x80112233, 0xFF000001]),
        vec![0x11, 0x22, 0x33, 0x80, 0, 0, 1, 0xFF]
    );
}

#[test]
fn counts_samples_and_report_for_synthetic_zip() {
    let t = TempDir::new();
    let zip_path = t.0.join("alr.dat");
    let alr = alr_fixture();
    write_zip(
        &zip_path,
        &[
            ("anim/fialgesc00.alr", &alr),
            ("other.alr", &alr),
            ("lagestgr00.alr", &alr),
            ("broken.alr", b"not an alr file at all"),
        ],
    );
    let before = fs::read(&zip_path).unwrap();
    let out = t.0.join("out");
    let r = run_probe(&zip_path, &out).unwrap();
    assert_eq!(
        fs::read(&zip_path).unwrap(),
        before,
        "input must stay untouched"
    );
    assert_eq!(
        (r.entries, r.documents, r.frames, r.variants, r.blanks),
        (4, 3, 12, 24, 0)
    );
    assert_eq!(r.failures.len(), 1);
    assert_eq!(r.failures[0].entry, "broken.alr");
    assert!(r.failures[0].error.contains("Truncated ALR header"));
    assert_eq!(r.samples.len(), 2);
    assert_eq!(r.samples[0].file, "sample-0.rgba");
    assert_eq!(r.samples[0].entry, "anim/fialgesc00.alr");

    // RGBA layout against an independent decode of the same document.
    let doc = AlrDocument::parse(&alr).unwrap();
    let expect = argb_to_rgba(&doc.decode_frame_argb(0, 0).unwrap());
    assert_eq!(fs::read(out.join("sample-0.rgba")).unwrap(), expect);
    assert_eq!(expect.len(), 5 * 1 * 4);

    let j = report_json(&out);
    assert_eq!(j["hash"].as_str().unwrap().len(), 64);
    assert_eq!(j["hash"], j["hash"].as_str().unwrap().to_uppercase());
    assert_eq!(j["entries"], 4);
    assert_eq!(j["samples"][0]["Width"], 5);
    assert_eq!(j["samples"][0]["info"]["OffsetX"], 7);
    assert_eq!(j["samples"][0]["info"]["PaletteColorCount"], 2);
    assert_eq!(j["samples"][0]["AnchorWidth"], 12);
    assert_eq!(j["samples"][0]["AnchorHeight"], 18);
    assert_eq!(j["failures"][0]["entry"], "broken.alr");
    let text = fs::read_to_string(out.join("report.json")).unwrap();
    assert!(
        text.contains("\n  \"hash\""),
        "pretty-printed with 2-space indent"
    );
}

#[test]
fn path_traversal_entries_never_write_outside_output() {
    let t = TempDir::new();
    let zip_path = t.0.join("alr.dat");
    let alr = alr_fixture();
    write_zip(
        &zip_path,
        &[
            ("../../evil/gersch01.alr", &alr),
            ("..\\escape\\lagenabust05.alr", &alr),
        ],
    );
    let out = t.0.join("nested").join("out");
    let r = run_probe(&zip_path, &out).unwrap();
    assert_eq!(r.samples.len(), 2);
    assert_eq!(r.samples[0].entry, "../../evil/gersch01.alr");
    let mut names: Vec<String> = fs::read_dir(&out)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["report.json", "sample-0.rgba", "sample-1.rgba"]);
    assert!(!t.0.join("evil").exists() && !t.0.parent().unwrap().join("evil").exists());
    assert!(!t.0.join("escape").exists());
    let mut top: Vec<_> = fs::read_dir(&t.0)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    top.sort();
    assert_eq!(top, ["alr.dat", "nested"]);
}

#[test]
fn existing_output_is_refused_and_preserved() {
    let t = TempDir::new();
    let zip_path = t.0.join("alr.dat");
    write_zip(&zip_path, &[("a.alr", &alr_fixture())]);
    let out = t.0.join("out");
    fs::create_dir(&out).unwrap();
    fs::write(out.join("keep.txt"), b"precious").unwrap();
    assert!(matches!(
        run_probe(&zip_path, &out),
        Err(ProbeError::OutputExists(_))
    ));
    assert_eq!(fs::read(out.join("keep.txt")).unwrap(), b"precious");
    assert_eq!(fs::read_dir(&out).unwrap().count(), 1);

    // Existing file at output path is also a conflict.
    let file_out = t.0.join("out-file");
    fs::write(&file_out, b"x").unwrap();
    let args: Vec<OsString> = vec![zip_path.clone().into(), file_out.clone().into()];
    let (mut o, mut e) = (Vec::new(), Vec::new());
    assert_eq!(cli(&args, &mut o, &mut e), 2);
    assert_eq!(fs::read(&file_out).unwrap(), b"x");
}

#[test]
fn cli_exit_codes_and_summary() {
    let t = TempDir::new();
    let zip_path = t.0.join("alr.dat");
    let alr = alr_fixture();
    write_zip(&zip_path, &[("a.alr", &alr)]);
    let (mut o, mut e) = (Vec::new(), Vec::new());
    assert_eq!(cli(&[], &mut o, &mut e), 2);
    assert!(String::from_utf8(e)
        .unwrap()
        .starts_with("Usage: alr-probe"));

    let args: Vec<OsString> = vec![zip_path.clone().into(), t.0.join("ok").into()];
    let (mut o, mut e) = (Vec::new(), Vec::new());
    assert_eq!(cli(&args, &mut o, &mut e), 0);
    assert_eq!(
        String::from_utf8(o).unwrap(),
        "Documents=1 Frames=4 PaletteFrames=8 Blanks=0 Failures=0\n"
    );

    let bad = t.0.join("bad.dat");
    write_zip(&bad, &[("junk.alr", b"junk")]);
    let args: Vec<OsString> = vec![bad.into(), t.0.join("fail").into()];
    let (mut o, mut e) = (Vec::new(), Vec::new());
    assert_eq!(cli(&args, &mut o, &mut e), 1);
    assert!(String::from_utf8(o).unwrap().contains("Failures=1"));
    assert!(t.0.join("fail").join("report.json").exists());
}

#[cfg(test)]
mod temp_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/temp_unique.rs"));
}
