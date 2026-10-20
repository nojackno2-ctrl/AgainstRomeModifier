use super::*;
use against_rome_data::{AlrFrame, AlrFrameInfo};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "anim-probe-test-{}-{}-{}",
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
}
impl Drop for Temp {
    fn drop(&mut self) {
        // Only our uniquely-created directory, contained under the real TEMP.
        let path = fs::canonicalize(&self.0).unwrap();
        let temp = fs::canonicalize(std::env::temp_dir()).unwrap();
        assert!(
            path.starts_with(temp)
                && path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("anim-probe-test-")
        );
        fs::remove_dir_all(path).unwrap();
    }
}

fn fixture_alr() -> AlrDocument {
    let mut payload = Vec::new();
    for v in 0..9u32 {
        payload.extend_from_slice(&0x00998877u32.to_le_bytes());
        payload.extend_from_slice(&(0x00332211 + v).to_le_bytes());
    }
    payload.extend_from_slice(&[1, 0, 1, 0]);
    let frame = AlrFrame {
        info: AlrFrameInfo::new(1, 1, 3, 1, 2, 3),
        payload,
        rows: vec![0, 3],
        shared_from: None,
    };
    AlrDocument::new(6, 2, 15, 9, 2, 3, vec![frame; 30])
}

fn fixture_apt() -> Vec<u8> {
    // 1,126 empty frames through the real parser, with tiny canvas.
    let mut words = vec![0u32; 28];
    for (index, value) in [
        (0, 0x54415041),
        (1, 3),
        (2, 1126),
        (3, 112),
        (6, 64),
        (7, 31),
        (8, 5),
        (9, 2),
        (10, 5),
        (11, 25),
        (13, 8),
        (20, 1),
        (21, 1),
        (23, 2),
        (24, 1),
    ] {
        words[index] = value;
    }
    words.extend([0; 62 + 7]); // row tables, projection/anchor/extras and zero groups
    words.extend(vec![0; 1126 * 2]); // empty frame ranges
    words.extend([0x00332211, 0x54414450]); // palette, PDAT, zero pixel data
    words.into_iter().flat_map(u32::to_le_bytes).collect()
}

fn zip(path: &Path, name: &str, bytes: &[u8]) {
    let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
    zip.start_file(name, zip::write::FileOptions::default())
        .unwrap();
    zip.write_all(bytes).unwrap();
    zip.finish().unwrap();
}

#[test]
fn refuses_existing_equal_root_outside_and_traversal() {
    let temp = Temp::new();
    let inside = temp.0.join("new");
    let output = Output::under(&inside, &temp.0).unwrap();
    assert!(Output::under(&inside, &temp.0).is_err());
    assert!(Output::under(&temp.0, &temp.0).is_err());
    assert!(Output::under(temp.0.ancestors().last().unwrap(), &temp.0).is_err());
    assert!(Output::under(&temp.0.join("../escape"), &temp.0).is_err());
    assert!(Output::under(&temp.0.join("new/../escape"), &temp.0).is_err());
    assert!(Output::under(&temp.0.join("missing/child"), &temp.0).is_err());
    let file = temp.0.join("file");
    fs::write(&file, b"unchanged").unwrap();
    assert!(Output::under(&file, &temp.0).is_err());
    assert_eq!(fs::read(file).unwrap(), b"unchanged");
    let outside = Temp::new();
    assert!(Output::under(&outside.0.join("child"), &temp.0).is_err());
    assert!(!outside.0.join("child").exists());
    drop(output);
}

#[test]
fn refuses_windows_aliases_and_generated_name_escape_or_overwrite() {
    let temp = Temp::new();
    for name in ["child.", "child ", "file:stream"] {
        assert!(Output::under(&temp.0.join(name), &temp.0).is_err());
    }
    let out = Output::under(&temp.0.join("output"), &temp.0).unwrap();
    assert!(out.create("../escape").is_err());
    out.create(PNG_NAMES[0])
        .unwrap()
        .write_all(b"keep")
        .unwrap();
    assert!(out.create(PNG_NAMES[0]).is_err());
    assert_eq!(fs::read(out.path.join(PNG_NAMES[0])).unwrap(), b"keep");
}

#[cfg(windows)]
#[test]
fn refuses_junction_ancestor_and_pins_output_against_replacement() {
    let temp = Temp::new();
    let target = temp.0.join("target");
    fs::create_dir(&target).unwrap();
    let link = temp.0.join("junction");
    let status = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(&link)
        .arg(&target)
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    assert!(Output::under(&link.join("child"), &temp.0).is_err());
    assert!(Output::under(&link, &temp.0).is_err());
    assert!(!target.join("child").exists());
    fs::remove_dir(link).unwrap(); // remove junction only
    let out = Output::under(&temp.0.join("output"), &temp.0).unwrap();
    assert!(fs::rename(&out.path, temp.0.join("moved")).is_err());
    assert!(fs::rename(&temp.0, temp.0.with_extension("moved")).is_err());
    drop(out);
}

#[cfg(unix)]
#[test]
fn refuses_symlink_ancestors_and_dangling_destinations() {
    use std::os::unix::fs::symlink;
    let temp = Temp::new();
    let target = temp.0.join("target");
    fs::create_dir(&target).unwrap();
    let link = temp.0.join("link");
    symlink(&target, &link).unwrap();
    assert!(Output::under(&link.join("child"), &temp.0).is_err());
    let dangling = temp.0.join("dangling");
    symlink(temp.0.join("missing"), &dangling).unwrap();
    assert!(Output::under(&dangling, &temp.0).is_err());
}

#[test]
fn alr_direction14_offsets_transparency_global_clipping_and_nine_palettes() {
    let mut doc = fixture_alr();
    // Every other direction must stay invisible even though its data is valid.
    for frame in &mut doc.frames[..28] {
        frame.info.offset_y = 999;
    }
    let doc = AlrDocument::parse(&doc.encode()).unwrap();
    let (idle, teams) = alr_sheets(&doc).unwrap();
    assert_eq!((idle.width, idle.height), (12, 12));
    assert_eq!(&idle.pixels[12..17], &[0, 0xff112233, 0, 0xff112233, 0]);
    assert!(idle.pixels[..12].iter().all(|&c| c == 0));
    assert_eq!((teams.width, teams.height), (18, 3));
    for v in 0..9 {
        assert_eq!(teams.pixels[18 + v * 2], 0); // transparent zero skipped
        assert_eq!(
            teams.pixels[18 + v * 2 + 1],
            0xff112233 + ((v as u32) << 16)
        );
    }
    // Positive offset crosses the cell boundary; later transparent sample cannot erase it.
    doc_cross_cell();
}

fn doc_cross_cell() {
    let mut doc = fixture_alr();
    for f in &mut doc.frames {
        f.info.offset_x = 1;
        f.info.offset_y = 0;
    }
    let (idle, _) = alr_sheets(&doc).unwrap();
    assert_eq!(
        &idle.pixels[..7],
        &[0, 0xff112233, 0, 0xff112233, 0, 0xff112233, 0]
    );
    // Negative Y clips at whole sheet origin.
    doc.frames[28].info.offset_y = -1;
    let (idle, _) = alr_sheets(&doc).unwrap();
    assert_eq!(idle.pixels[1], 0);
}

#[test]
fn alr_rejects_missing_direction_palette_and_frame_without_panicking() {
    let mut doc = fixture_alr();
    doc.layout_rows = 14;
    assert!(alr_sheets(&doc).is_err());
    doc.layout_rows = 15;
    doc.palette_variant_count = 8;
    assert!(alr_sheets(&doc).is_err());
    doc.palette_variant_count = 9;
    doc.frames.truncate(28);
    assert!(alr_sheets(&doc).is_err());
}

#[test]
fn apt_selects_all_25_frames_crop_origin_edges_and_background() {
    let mut seen = Vec::new();
    let sheet = apt_sheet(|index| {
        seen.push(index);
        let mut image = Image::new(481, 401, 0)?;
        image.pixels[200 * 481 + 280] = 0xff000000 | index as u32;
        image.pixels[399 * 481 + 479] = 0x80123456;
        image.pixels[200 * 481 + 279] = 0xffffffff; // outside crop
        image.pixels[400 * 481 + 480] = 0xffffffff; // outside opposite edge
        Ok(image)
    })
    .unwrap();
    assert_eq!(seen, (1000..1025).collect::<Vec<_>>());
    for d in 0..25 {
        let start = (d / 5) * 200 * 1000 + (d % 5) * 200;
        assert_eq!(sheet.pixels[start], 0xff000000 | (1000 + d) as u32);
        assert_eq!(sheet.pixels[start + 1], 0xff202020);
        assert_eq!(sheet.pixels[start + 199 * 1000 + 199], 0x80123456);
    }
    let empty = apt_sheet(|_| Image::new(1, 1, 0xffffffff)).unwrap();
    assert!(empty.pixels.iter().all(|&c| c == 0xff202020));
}

#[test]
fn png_decoding_preserves_straight_argb_channels_and_dimensions() {
    let temp = Temp::new();
    let out = Output::under(&temp.0.join("output"), &temp.0).unwrap();
    let image = Image {
        width: 2,
        height: 2,
        pixels: vec![0x80123456, 0x00112233, 0xffabcdef, 0],
    };
    image.write(&out, PNG_NAMES[0]).unwrap();
    let mut reader = png::Decoder::new(File::open(out.path.join(PNG_NAMES[0])).unwrap())
        .read_info()
        .unwrap();
    let mut bytes = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut bytes).unwrap();
    assert_eq!(
        (info.width, info.height, info.color_type),
        (2, 2, png::ColorType::Rgba)
    );
    assert_eq!(
        &bytes[..info.buffer_size()],
        &[0x12, 0x34, 0x56, 0x80, 0x11, 0x22, 0x33, 0, 0xab, 0xcd, 0xef, 255, 0, 0, 0, 0]
    );
}

#[test]
fn exact_case_sensitive_zip_lookup_never_extracts_paths() {
    let temp = Temp::new();
    let path = temp.0.join("input.zip");
    zip(&path, "SYSTEM/DATA/ALR/gersch01.alr", b"payload");
    assert_eq!(
        entry(&path, "SYSTEM/DATA/ALR/gersch01.alr").unwrap(),
        b"payload"
    );
    assert!(entry(&path, "system/data/alr/gersch01.alr").is_err());
    assert!(entry(&path, "../gersch01.alr").is_err());
}

#[test]
fn synthetic_archives_export_five_decodable_pngs_and_oracle_summary() {
    let temp = Temp::new();
    let alr = temp.0.join("alr.dat");
    let apt = temp.0.join("apt.dat");
    zip(
        &alr,
        "SYSTEM/DATA/ALR/gersch01.alr",
        &fixture_alr().encode(),
    );
    zip(&apt, "SYSTEM/DATA/APT/gerhau02.apt", &fixture_apt());
    let out = Output::under(&temp.0.join("output"), &temp.0).unwrap();
    run(&alr, &apt, &out).unwrap();
    let dimensions = [(12, 12), (18, 3), (2, 1), (2, 1), (1000, 1000)];
    for (name, dims) in PNG_NAMES.iter().zip(dimensions) {
        let mut reader = png::Decoder::new(File::open(out.path.join(name)).unwrap())
            .read_info()
            .unwrap();
        let mut bytes = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut bytes).unwrap();
        assert_eq!((info.width, info.height), dims);
    }
    let actual: serde_json::Value =
        serde_json::from_slice(&fs::read(out.path.join("probe_summary.json")).unwrap()).unwrap();
    assert_eq!(actual, serde_json::to_value(summary()).unwrap());
    let summary_text = fs::read_to_string(out.path.join("probe_summary.json")).unwrap();
    #[cfg(windows)]
    assert!(summary_text.contains("\r\n"));
    #[cfg(not(windows))]
    assert!(!summary_text.contains("\r\n"));
    assert!(summary_text.find("\"status\"").unwrap() < summary_text.find("\"unit_anim\"").unwrap());
    assert!(
        summary_text.find("\"unit_anim\"").unwrap()
            < summary_text.find("\"team_variants\"").unwrap()
    );
    assert!(
        summary_text.find("\"team_variants\"").unwrap()
            < summary_text.find("\"apt_axis1_val0\"").unwrap()
    );
    assert!(
        summary_text.find("\"apt_axis1_val0\"").unwrap()
            < summary_text.find("\"apt_axis1_val1\"").unwrap()
    );
    assert!(
        summary_text.find("\"apt_axis1_val1\"").unwrap()
            < summary_text.find("\"apt_anim\"").unwrap()
    );
    assert_eq!(fs::read_dir(&out.path).unwrap().count(), 6);
    assert!(run(&alr, &apt, &out).is_err());
}
