//! Opt-in live differential: C# `NativeSpriteCatalog` (real .NET 10 oracle linking the Modules project)
//! versus the Rust catalog over identical synthetic ALR/APT/objdef/anim fixtures.
//! Run: cargo test --release --locked --offline --test catalog_csharp_differential -- --ignored --nocapture
//! Covers sprite ids, anchor/crop, finished APT frame, direction/team palette, animation sequences and
//! missing/corrupt asset fallback. Synthetic fixtures only; never touches the installed game.
use against_rome_textures::{NativeAnimationVisualState, NativeSprite, NativeSpriteCatalog};
use std::path::{Path, PathBuf};

fn size(width: usize, height: usize, colors: usize) -> u32 {
    (width as u32 & 0x7FF) | ((height as u32 & 0x7FF) << 11) | ((colors as u32 & 0x1FF) << 22)
}

fn record(
    buf: &mut Vec<u8>,
    offset_x: i32,
    offset_y: i32,
    width: usize,
    palette: &[u32],
    pixels: &[u8],
    rows: &[u32],
) {
    buf.extend_from_slice(&(-1i32).to_le_bytes());
    let table_offset = (palette.len() * 4 + ((pixels.len() + 3) & !3)) as u32;
    buf.extend_from_slice(&table_offset.to_le_bytes());
    let pos = ((offset_x as u32) & 0xFFFF) | (((offset_y as u32) & 0xFFFF) << 16);
    buf.extend_from_slice(&pos.to_le_bytes());
    let s = size(width, 1, palette.len() / 2);
    buf.extend_from_slice(&s.to_le_bytes());
    buf.extend_from_slice(&(pixels.len() as u32).to_le_bytes());
    for &color in palette {
        buf.extend_from_slice(&color.to_le_bytes());
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

fn make_alr_fixture(version: u32, second_frame_palette: bool) -> Vec<u8> {
    let mut buf = Vec::new();
    let header_words: [u32; 23] = [
        0x41524C41, version, 0x100, 4, 8, 0x200, 2, 2, 2, 0x300, 0x401, 0x402, 0x403, 0x404,
        0x405, 12, 18, 0x501, 0x502, 0x503, 0x504, 0x601, 0x602,
    ];
    for w in header_words {
        buf.extend_from_slice(&w.to_le_bytes());
    }

    // Frame 0: Record
    record(
        &mut buf,
        7,
        11,
        5,
        &[0x405060, 0x102030, 0xC0D0E0, 0x90A0B0],
        &[1, 0, 1],
        &[1u32 << 21, 3],
    );

    // Frame 1: Shared frame 0
    buf.extend_from_slice(&0i32.to_le_bytes());

    // Frame 2: Record
    let second_palette = if second_frame_palette {
        vec![0, 0xFFFFFF, 0, 0xEEEEEE]
    } else {
        vec![]
    };
    record(
        &mut buf,
        2,
        3,
        2,
        &second_palette,
        &[1, 0],
        &[0x80000000, 2],
    );

    // Frame 3: Transitive shared frame 1
    buf.extend_from_slice(&1i32.to_le_bytes());

    buf
}

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

fn solid_alr(width: i32, height: i32, offset_x: i32, offset_y: i32, color: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    let header_words: [u32; 23] = [
        0x41524C41, 6, 0, 1, 8, 0, 1, 1, 1, 0, 0, 0, 0, 0, 0, 8, 8, 0, 0, 0, 0, 0, 0,
    ];
    for w in header_words {
        buf.extend_from_slice(&w.to_le_bytes());
    }
    let pixels = (width * height) as usize;
    let padded = (pixels + 3) & !3;
    buf.extend_from_slice(&(-1i32).to_le_bytes());
    buf.extend_from_slice(&((8 + padded) as u32).to_le_bytes());
    let pos = ((offset_x as u32) & 0xFFFF) | (((offset_y as u32) & 0xFFFF) << 16);
    buf.extend_from_slice(&pos.to_le_bytes());
    let s = (width as u32 & 0x7FF) | ((height as u32 & 0x7FF) << 11) | (2u32 << 22);
    buf.extend_from_slice(&s.to_le_bytes());
    buf.extend_from_slice(&(pixels as u32).to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&color.to_le_bytes());
    buf.extend(std::iter::repeat(1u8).take(pixels));
    buf.extend(std::iter::repeat(0u8).take(padded - pixels));
    for y in 0..=height {
        buf.extend_from_slice(&((y * width) as u32).to_le_bytes());
    }
    buf
}

const ALR_LIST: &str = "[AlrNames]\r\n0000,unit.alr,0000\r\n0001,broken.alr,0000\r\n0002,missing.alr,0000\r\n0003,trunk.alr\r\n0004,crown.alr\r\n";
const APT_LIST: &str = "[AptNames]\r\n0000,house.apt,0000\r\n";

fn row(
    id: i32,
    name: &str,
    alr: i32,
    apt: i32,
    palette: i32,
    layer: i32,
    layer_type: i32,
    cycle: i32,
    anim_add: i32,
    anim_mask: &str,
) -> String {
    let mut columns: Vec<String> = vec!["   0".to_string(); 60];
    columns[0] = id.to_string();
    columns[2] = cycle.to_string();
    columns[5] = alr.to_string();
    columns[6] = anim_add.to_string();
    columns[8] = layer.to_string();
    columns[10] = layer_type.to_string();
    columns[14] = apt.to_string();
    columns[17] = palette.to_string();
    columns[45] = anim_mask.to_string();
    columns[52] = name.to_string();
    columns.join(",")
}

fn objdef() -> String {
    vec![
        "[ObjectDefaults]".to_string(),
        ";idx ,activ,alen ,rotsp,moves,alrid".to_string(),
        row(10, "FigTest", 0, -1, 1, -1, -1, 1000, 0, "1"),
        row(11, "LanTest", 0, -1, 0, -1, -1, 0, -1, "0"),
        row(12, "BauTest", -1, 0, 0, -1, -1, 0, -1, "0"),
        row(13, "ParNone", -1, -1, 0, -1, -1, 0, -1, "0"),
        row(14, "FigBroken", 1, -1, 0, -1, -1, 0, -1, "0"),
        row(15, "FigMissing", 2, -1, 0, -1, -1, 0, -1, "0"),
        row(16, "LanTree", 3, -1, 0, 4, 2, 0, -1, "0"),
        row(17, "LanCover", -1, -1, 0, 4, 10, 0, -1, "0"),
        row(18, "LanBrokenCrown", 3, -1, 0, 1, 2, 0, -1, "0"),
    ]
    .join("\r\n")
}


type ExtraRow = (i32, &'static str, i32, i32, i32, i32, i32, i32, i32, &'static str);

const OBJDEF_EXTRA: &[ExtraRow] = &[
    (19, "FigNoMask", 0, -1, 0, -1, -1, 800, 0, "0"),
    (20, "FigTree", 0, -1, 1, -1, 2, 800, 0, "1"),
    (21, "FigShort", 0, -1, 1, -1, -1, 1, 0, "1"),
    (22, "BauAnim", -1, 0, 0, -1, -1, 2000, -1, "0"),
];

fn objdef_text() -> String {
    let mut text = objdef();
    for e in OBJDEF_EXTRA {
        text.push_str("\r\n");
        text.push_str(&row(e.0, e.1, e.2, e.3, e.4, e.5, e.6, e.7, e.8, e.9));
    }
    text
}

fn write_fixture(dir: &Path, degraded: bool) {
    let assets = dir.join("assets");
    std::fs::create_dir_all(&assets).unwrap();
    std::fs::write(dir.join("objdef.txt"), objdef_text()).unwrap();
    std::fs::write(dir.join("alr_list.txt"), ALR_LIST).unwrap();
    std::fs::write(dir.join("apt_list.txt"), APT_LIST).unwrap();
    std::fs::write(assets.join("broken.alr"), [1u8, 2, 3]).unwrap();
    if degraded {
        // unit.alr missing, trunk corrupt, crown truncated, house.apt corrupt.
        let mut truncated = solid_alr(3, 2, 3, 0, 0x00FF00);
        truncated.truncate(truncated.len() - 9);
        std::fs::write(assets.join("crown.alr"), truncated).unwrap();
        std::fs::write(assets.join("trunk.alr"), [0u8; 40]).unwrap();
        std::fs::write(assets.join("house.apt"), [9u8; 64]).unwrap();
    } else {
        std::fs::write(assets.join("unit.alr"), make_alr_fixture(6, true)).unwrap();
        std::fs::write(assets.join("trunk.alr"), solid_alr(1, 3, 4, 1, 0x0000FF)).unwrap();
        std::fs::write(assets.join("crown.alr"), solid_alr(3, 2, 3, 0, 0x00FF00)).unwrap();
        std::fs::write(assets.join("house.apt"), make_apt_fixture(3)).unwrap();
    }
}

fn cases() -> Vec<String> {
    let names = [
        "FigTest", "figtest", " FigTest ", "LanTest", "BauTest", "ParNone", "FigBroken", "FigMissing",
        "LanTree", "LanCover", "LanBrokenCrown", "FigNoMask", "FigTree", "FigShort", "BauAnim", "Nope",
    ];
    let angles = ["-", "0", "45", "90", "180", "-45", "360", "22.5", "NaN", "Infinity"];
    let states = [
        "-", "0,0,0,0,0", "1,0,0,0,0", "2,0,0,0,0", "3,0,0,0,0", "4,0,0,0,0", "-1,0,0,0,0",
        "0,1,0,0,0", "0,0,1,0,0", "0,0,0,1,0", "0,0,0,0,1", "0,2,1,1,0", "1,1,1,1,1", "0,-1,0,0,0",
    ];
    let mut out = Vec::new();
    for n in names {
        out.push(format!("D|{n}"));
        for team in [0, 1, 9, -3] {
            for dir in [0, 1, -1, 14, 33] {
                out.push(format!("S|{n}|{team}|{dir}|-|-"));
            }
            for a in angles {
                out.push(format!("S|{n}|{team}|14|{a}|-"));
                out.push(format!("A|{n}|{team}|{a}|-"));
            }
            for s in states {
                out.push(format!("S|{n}|{team}|14|-|{s}"));
                out.push(format!("S|{n}|{team}|0|90|{s}"));
                out.push(format!("A|{n}|{team}|-|{s}"));
                out.push(format!("A|{n}|{team}|180|{s}"));
            }
        }
    }
    out
}

fn fmt_sprite(s: Option<&NativeSprite>) -> String {
    match s {
        None => "null".into(),
        Some(s) => format!(
            "{}x{}@{},{} {} {}",
            s.width,
            s.height,
            s.anchor_x,
            s.anchor_y,
            s.asset_name,
            s.argb_pixels
                .iter()
                .map(|p| format!("{p:08X}"))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

fn parse_angle(s: &str) -> Option<f32> {
    (s != "-").then(|| s.replace("Infinity", "inf").parse::<f32>().unwrap())
}

fn parse_state(s: &str) -> Option<NativeAnimationVisualState> {
    (s != "-").then(|| {
        let v: Vec<i16> = s.split(',').map(|x| x.parse().unwrap()).collect();
        NativeAnimationVisualState::new(v[0], v[1], v[2], v[3], v[4])
    })
}

fn run_case(catalog: &NativeSpriteCatalog, line: &str) -> String {
    let f: Vec<&str> = line.split('|').collect();
    let result = match f[0] {
        "S" => fmt_sprite(
            catalog
                .get_sprite(
                    f[1],
                    f[2].parse().unwrap(),
                    f[3].parse().unwrap(),
                    parse_angle(f[4]),
                    parse_state(f[5]).as_ref(),
                )
                .as_deref(),
        ),
        "A" => match catalog.get_animation(
            f[1],
            f[2].parse().unwrap(),
            parse_angle(f[3]),
            parse_state(f[4]).as_ref(),
        ) {
            None => "null".into(),
            Some(a) => format!(
                "cycle={} frames={} {}",
                a.cycle_duration_ms(),
                a.frames().len(),
                a.frames()
                    .iter()
                    .map(|s| fmt_sprite(Some(s)))
                    .collect::<Vec<_>>()
                    .join(" ; ")
            ),
        },
        "D" => match catalog.try_get_definition(f[1]) {
            None => "null".into(),
            Some(d) => format!(
                "{},{},{},{},{},{},{},{},{},{:X}",
                d.type_id,
                d.name,
                d.alr_id,
                d.apt_index,
                d.palette_type,
                d.layer_alr_id,
                d.layer_type,
                d.animation_length_ms,
                d.animation_add,
                d.animation_mask
            ),
        },
        _ => panic!("{line}"),
    };
    format!("{line} => {result}")
}

fn rust_output(dir: &Path, cases: &[String]) -> String {
    let assets = dir.join("assets");
    let (alr_assets, apt_assets) = (assets.clone(), assets);
    let catalog = NativeSpriteCatalog::from_text(
        &std::fs::read_to_string(dir.join("objdef.txt")).unwrap(),
        &std::fs::read_to_string(dir.join("alr_list.txt")).unwrap(),
        &std::fs::read_to_string(dir.join("apt_list.txt")).unwrap(),
        move |name| std::fs::read(alr_assets.join(name)).ok(),
        move |name| std::fs::read(apt_assets.join(name)).ok(),
    );
    let mut out = String::new();
    for c in cases {
        out.push_str(&run_case(&catalog, c));
        out.push('\n');
    }
    out.push_str(&format!("count={}\n", catalog.definition_count()));
    out
}

fn oracle_output(dir: &Path) -> String {
    let output = dir.join("csharp.txt");
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/csharp_sprite_oracle/SpriteCatalogOracle.csproj");
    let result = std::process::Command::new("dotnet")
        .args(["run", "--project"])
        .arg(&project)
        .args(["-c", "Release", "--"])
        .arg(dir)
        .arg(&output)
        .output()
        .expect("dotnet must be installed");
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    std::fs::read_to_string(output).unwrap()
}

fn compare(label: &str, degraded: bool) {
    let root: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../re_workspace/sprite-catalog-differential")
        .join(label);
    let _ = std::fs::remove_dir_all(&root);
    write_fixture(&root, degraded);
    let cases = cases();
    std::fs::write(root.join("cases.txt"), cases.join("\n")).unwrap();
    let expected = oracle_output(&root);
    let actual = rust_output(&root, &cases);
    let (e, a): (Vec<&str>, Vec<&str>) = (expected.lines().collect(), actual.lines().collect());
    assert_eq!(e.len(), a.len(), "{label}: line count");
    let all: Vec<String> = e
        .iter()
        .zip(&a)
        .filter(|(x, y)| x != y)
        .map(|(x, y)| format!("C#:   {x}\nRust: {y}"))
        .collect();
    std::fs::write(root.join("mismatches.txt"), all.join("\n")).unwrap();
    assert!(
        all.is_empty(),
        "{label}: {} mismatching cases (all in {}); first:\n{}",
        all.len(),
        root.join("mismatches.txt").display(),
        all.iter().take(4).cloned().collect::<Vec<_>>().join("\n")
    );
    let non_null = e.iter().filter(|l| l.starts_with("S|") && !l.ends_with("=> null")).count();
    let animations = e.iter().filter(|l| l.starts_with("A|") && !l.ends_with("=> null")).count();
    eprintln!("{label}: {} lines identical, {non_null} non-null sprites, {animations} animations", e.len());
    if degraded {
        assert!(
            e.iter().any(|l| l.starts_with("S|LanTree") && l.ends_with("=> null")),
            "degraded set must hit fallback"
        );
    } else {
        assert!(non_null > 100 && animations > 10, "fixture should exercise real decoding");
    }
}

#[test]
#[ignore = "builds and executes the real C# NativeSpriteCatalog; requires .NET 10 SDK"]
fn csharp_native_sprite_catalog_differential_normal_fixtures() {
    compare("normal", false);
}

#[test]
#[ignore = "builds and executes the real C# NativeSpriteCatalog; requires .NET 10 SDK"]
fn csharp_native_sprite_catalog_differential_missing_and_corrupt_fixtures() {
    compare("degraded", true);
}
