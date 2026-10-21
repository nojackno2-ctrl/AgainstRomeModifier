#[path = "../src/cli.rs"]
mod cli;

use against_rome_editor_app::bmp::BmpImage;
use cli::{parse_startup_options, resolve_direct_map, resolve_startup_map};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(prefix: &str) -> Self {
        let dir = env::temp_dir().join(format!("{}_{}", prefix, Uuid::new_v4().simple()));
        fs::create_dir_all(&dir).unwrap();
        Self { path: dir }
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn create_mock_map(map_dir: &Path, size: usize, include_meta: bool, include_bmps: bool) {
    fs::create_dir_all(map_dir).unwrap();
    if include_meta {
        // Retail layout: one texture tile per 4x4 height cells.
        let dimension = (size.saturating_sub(1) / 4).max(1);
        let tiles = "4BB___50\n".repeat(dimension * dimension);
        fs::write(
            map_dir.join("boden.txt"),
            format!("[Dimension]\n{dimension}\n[Texturen]\n{tiles}"),
        )
        .unwrap();
        let minimap = BmpImage::new_solid(16, 16, 0, 0, 0);
        fs::write(map_dir.join("minimap.bmp"), minimap.encode_bmp()).unwrap();
    }
    if include_bmps {
        let boden = BmpImage::new_solid(size, size, 0, 50, 0);
        fs::write(map_dir.join("boden.bmp"), boden.encode_bmp()).unwrap();
        if size > 1 {
            let col_size = size - 1;
            let col = BmpImage::new_solid(col_size, col_size, 0, 0, 0);
            fs::write(map_dir.join("collision.bmp"), col.encode_bmp()).unwrap();
        }
    }
}

fn create_synthetic_game(root: &Path) {
    let maps_dir = root.join("MAPS");
    fs::create_dir_all(&maps_dir).unwrap();

    // 1. Supported endless map: ENDL_001
    create_mock_map(&maps_dir.join("ENDL_001"), 65, true, true);

    // 2. Supported historical map: HIST_01
    create_mock_map(&maps_dir.join("HIST_01"), 65, true, true);

    // 3. Campaign map: KAMP_001 (catalog valid, but editor not supported)
    create_mock_map(&maps_dir.join("KAMP_001"), 65, true, true);

    // 4. Incomplete map (missing boden.txt and minimap.bmp)
    create_mock_map(&maps_dir.join("INCOMPLETE_01"), 65, false, true);
}

#[test]
fn test_case_insensitive_flag_parsing() {
    let temp = TempDirGuard::new("ar_test_case_flags");
    create_synthetic_game(&temp.path);

    // Mixed case flags: --GAME, --MAP, --VALIDATE-MAP
    let args = [
        "--GAME",
        temp.path.to_str().unwrap(),
        "--MAP",
        "endl_001",
        "--VALIDATE-MAP",
    ];
    let options = parse_startup_options(args).expect("Mixed case flags should parse");
    assert_eq!(options.game_path, Some(temp.path.clone()));
    assert_eq!(options.map_arg.as_deref(), Some("endl_001"));
    assert!(options.validate_map);
    assert!(!options.smoke_test);

    // Map ID case insensitivity during resolution: "endl_001" resolves "ENDL_001"
    let resolved = resolve_startup_map(&options).expect("Case-insensitive map resolution should succeed");
    assert!(resolved.is_some());
    let resolved_path = resolved.unwrap();
    assert!(resolved_path.join("boden.txt").is_file());

    // Alternative flag cases: --Game, --Map, --Smoke-Test
    let args2 = [
        "--Game",
        temp.path.to_str().unwrap(),
        "--Map",
        "HIST_01",
        "--Smoke-Test",
    ];
    let options2 = parse_startup_options(args2).expect("Camel-case flags should parse");
    assert_eq!(options2.map_arg.as_deref(), Some("HIST_01"));
    assert!(options2.smoke_test);
    assert!(!options2.validate_map);

    // Help and version flags case-insensitivity
    let opt_help = parse_startup_options(["--HELP"]).unwrap();
    assert!(opt_help.show_help);

    let opt_h = parse_startup_options(["-H"]).unwrap();
    assert!(opt_h.show_help);

    let opt_ver = parse_startup_options(["--VERSION"]).unwrap();
    assert!(opt_ver.show_version);
}

#[test]
fn test_missing_and_mismatched_flags() {
    let temp = TempDirGuard::new("ar_test_mismatched_flags");
    create_synthetic_game(&temp.path);

    // 1. Missing path value for --game
    let err_game = parse_startup_options(["--game"]).unwrap_err();
    assert_eq!(err_game, "--game 缺少路徑參數。");

    // 2. Missing map id value for --map
    let err_map = parse_startup_options(["--game", temp.path.to_str().unwrap(), "--map"]).unwrap_err();
    assert_eq!(err_map, "--map 缺少地圖代號。");

    let err_map_alone = parse_startup_options(["--map"]).unwrap_err();
    assert_eq!(err_map_alone, "--map 缺少地圖代號。");

    // 3. --game provided without --map
    let opt_game_only = parse_startup_options(["--game", temp.path.to_str().unwrap()]).unwrap();
    let err_game_only = resolve_startup_map(&opt_game_only).unwrap_err();
    assert_eq!(err_game_only, "未指定 --map 地圖代號。");

    // 4. --map <bare_id> provided without --game
    let opt_map_bare = parse_startup_options(["--map", "ENDL_001"]).unwrap();
    let err_map_bare = resolve_startup_map(&opt_map_bare).unwrap_err();
    assert_eq!(err_map_bare, "使用 --map 直接開啟地圖時必須同時指定 --game <path>。");

    // 5. --validate-map without map
    let opt_val_alone = parse_startup_options(["--validate-map"]).unwrap();
    let err_val_alone = resolve_startup_map(&opt_val_alone).unwrap_err();
    assert_eq!(
        err_val_alone,
        "--validate-map requires --map and cannot combine with --smoke-test"
    );

    // 6. --validate-map combined with --smoke-test
    let opt_val_smoke = parse_startup_options([
        "--game",
        temp.path.to_str().unwrap(),
        "--map",
        "ENDL_001",
        "--validate-map",
        "--smoke-test",
    ])
    .unwrap();
    let err_val_smoke = resolve_startup_map(&opt_val_smoke).unwrap_err();
    assert_eq!(
        err_val_smoke,
        "--validate-map requires --map and cannot combine with --smoke-test"
    );

    // 7. Duplicate flags
    let err_dup_game = parse_startup_options(["--game", "a", "--game", "b"]).unwrap_err();
    assert!(err_dup_game.contains("unknown or duplicate argument: --game"));

    let err_dup_map = parse_startup_options(["--map", "a", "--map", "b"]).unwrap_err();
    assert!(err_dup_map.contains("unknown or duplicate argument: --map"));

    let err_dup_val = parse_startup_options(["--validate-map", "--validate-map"]).unwrap_err();
    assert!(err_dup_val.contains("unknown or duplicate argument: --validate-map"));

    let err_dup_smoke = parse_startup_options(["--smoke-test", "--smoke-test"]).unwrap_err();
    assert!(err_dup_smoke.contains("unknown or duplicate argument: --smoke-test"));

    // 8. Unknown flag
    let err_unknown = parse_startup_options(["--foobar"]).unwrap_err();
    assert_eq!(err_unknown, "unknown or duplicate argument: --foobar");
}

#[test]
fn test_traversal_and_invalid_ids() {
    let temp = TempDirGuard::new("ar_test_traversal");
    create_synthetic_game(&temp.path);

    // Traversal attempts
    let traversals = [
        "../ENDL_001",
        "..",
        "../../MAPS",
        "ENDL/001",
        "ENDL\\001",
        "ENDL*01",
        "MAP 01",
    ];
    for bad_id in traversals {
        let opt = parse_startup_options(["--game", temp.path.to_str().unwrap(), "--map", bad_id]).unwrap();
        let err = resolve_startup_map(&opt).unwrap_err();
        assert_eq!(err, "無效的地圖代號。", "Testing bad map ID: {bad_id}");
    }

    // Empty ID
    let opt_empty = parse_startup_options(["--game", temp.path.to_str().unwrap(), "--map", ""]).unwrap();
    let err_empty = resolve_startup_map(&opt_empty).unwrap_err();
    assert_eq!(err_empty, "未指定 --map 地圖代號。");
}

#[test]
fn test_missing_maps_and_campaign_rejection() {
    let temp = TempDirGuard::new("ar_test_missing_maps");
    create_synthetic_game(&temp.path);

    // 1. Nonexistent game path
    let missing_game = temp.path.join("nonexistent_game_root");
    let opt_bad_game = parse_startup_options(["--game", missing_game.to_str().unwrap(), "--map", "ENDL_001"]).unwrap();
    let err_bad_game = resolve_startup_map(&opt_bad_game).unwrap_err();
    assert!(
        err_bad_game.contains("找不到遊戲路徑"),
        "Expected missing game path error, got: {err_bad_game}"
    );

    // 2. Nonexistent map ID
    let opt_bad_map = parse_startup_options(["--game", temp.path.to_str().unwrap(), "--map", "ENDL_999"]).unwrap();
    let err_bad_map = resolve_startup_map(&opt_bad_map).unwrap_err();
    assert_eq!(err_bad_map, "找不到地圖: ENDL_999");

    // 3. Incomplete map (missing boden.txt / minimap.bmp)
    let opt_incomplete = parse_startup_options(["--game", temp.path.to_str().unwrap(), "--map", "INCOMPLETE_01"]).unwrap();
    let err_incomplete = resolve_startup_map(&opt_incomplete).unwrap_err();
    assert_eq!(err_incomplete, "找不到地圖: INCOMPLETE_01");

    // 4. Campaign map (KAMP_001) rejection
    let opt_kamp = parse_startup_options(["--game", temp.path.to_str().unwrap(), "--map", "KAMP_001"]).unwrap();
    let err_kamp = resolve_startup_map(&opt_kamp).unwrap_err();
    assert_eq!(err_kamp, "劇情戰役地圖不在地圖編輯器支援範圍：KAMP_001");
}

#[test]
fn test_synthetic_supported_map_fixture() {
    let temp = TempDirGuard::new("ar_test_supported_fixture");
    create_synthetic_game(&temp.path);

    // 1. Resolve ENDL_001 through --game and --map
    let direct_info = resolve_direct_map(&temp.path, "ENDL_001").expect("Direct map resolution should succeed");
    assert_eq!(direct_info.id, "ENDL_001");

    let opt1 = parse_startup_options(["--game", temp.path.to_str().unwrap(), "--map", "ENDL_001"]).unwrap();
    let res1 = resolve_startup_map(&opt1).expect("Should resolve ENDL_001").unwrap();
    assert_eq!(
        res1.canonicalize().unwrap(),
        temp.path.join("MAPS").join("ENDL_001").canonicalize().unwrap()
    );

    // 2. Resolve HIST_01 through --game and --map
    let opt2 = parse_startup_options(["--game", temp.path.to_str().unwrap(), "--map", "HIST_01"]).unwrap();
    let res2 = resolve_startup_map(&opt2).expect("Should resolve HIST_01").unwrap();
    assert_eq!(
        res2.canonicalize().unwrap(),
        temp.path.join("MAPS").join("HIST_01").canonicalize().unwrap()
    );

    // 3. Backwards-compatible direct directory form without --game
    let direct_dir = temp.path.join("MAPS").join("ENDL_001");
    let opt_direct = parse_startup_options(["--map", direct_dir.to_str().unwrap()]).unwrap();
    let res_direct = resolve_startup_map(&opt_direct).expect("Direct dir should resolve").unwrap();
    assert_eq!(res_direct, direct_dir);
}

#[test]
fn test_binary_cli_execution() {
    let temp = TempDirGuard::new("ar_test_bin_cli");
    create_synthetic_game(&temp.path);

    let bin = PathBuf::from(env!("CARGO_BIN_EXE_against-rome-editor"));

    // 1. --help exits 0 and prints usage including --game
    let out_help = Command::new(&bin).arg("--help").output().unwrap();
    assert!(out_help.status.success());
    let stdout_help = String::from_utf8_lossy(&out_help.stdout);
    assert!(stdout_help.contains("--game <PATH> --map <ID>"));
    assert!(stdout_help.contains("--validate-map"));

    // 2. --version exits 0 and prints version
    let out_ver = Command::new(&bin).arg("--version").output().unwrap();
    assert!(out_ver.status.success());
    let stdout_ver = String::from_utf8_lossy(&out_ver.stdout);
    assert!(stdout_ver.contains("against-rome-editor"));

    // 3. Supported map with --validate-map
    let out_val = Command::new(&bin)
        .args([
            "--game",
            temp.path.to_str().unwrap(),
            "--map",
            "ENDL_001",
            "--validate-map",
        ])
        .output()
        .unwrap();
    assert!(out_val.status.success(), "Stderr: {}", String::from_utf8_lossy(&out_val.stderr));
    let stdout_val = String::from_utf8_lossy(&out_val.stdout);
    assert!(stdout_val.contains("Validated height 65x65, collision 64x64; no files written."));
    assert!(stdout_val.contains("0 errors,"), "{stdout_val}");

    // 4. Backwards-compatible direct directory form with --validate-map
    let direct_map = temp.path.join("MAPS").join("ENDL_001");
    let out_direct = Command::new(&bin)
        .args(["--map", direct_map.to_str().unwrap(), "--validate-map"])
        .output()
        .unwrap();
    assert!(out_direct.status.success(), "Stderr: {}", String::from_utf8_lossy(&out_direct.stderr));
    let stdout_direct = String::from_utf8_lossy(&out_direct.stdout);
    assert!(stdout_direct.contains("Validated height 65x65, collision 64x64; no files written."));

    // 5. Campaign map rejected with error
    let out_kamp = Command::new(&bin)
        .args([
            "--game",
            temp.path.to_str().unwrap(),
            "--map",
            "KAMP_001",
            "--validate-map",
        ])
        .output()
        .unwrap();
    assert!(!out_kamp.status.success());
    let stderr_kamp = String::from_utf8_lossy(&out_kamp.stderr);
    assert!(stderr_kamp.contains("劇情戰役地圖不在地圖編輯器支援範圍：KAMP_001"));

    // 6. Traversal rejected with error
    let out_trav = Command::new(&bin)
        .args([
            "--game",
            temp.path.to_str().unwrap(),
            "--map",
            "../../ENDL_001",
            "--validate-map",
        ])
        .output()
        .unwrap();
    assert!(!out_trav.status.success());
    let stderr_trav = String::from_utf8_lossy(&out_trav.stderr);
    assert!(stderr_trav.contains("無效的地圖代號。"));

    // 7. Missing value error on --game
    let out_game_err = Command::new(&bin).arg("--game").output().unwrap();
    assert!(!out_game_err.status.success());
    let stderr_game = String::from_utf8_lossy(&out_game_err.stderr);
    assert!(stderr_game.contains("--game 缺少路徑參數。"));

    // 8. Missing value error on --map
    let out_map_err = Command::new(&bin).arg("--map").output().unwrap();
    assert!(!out_map_err.status.success());
    let stderr_map = String::from_utf8_lossy(&out_map_err.stderr);
    assert!(stderr_map.contains("--map 缺少地圖代號。"));

    // 9. Bare map ID without --game
    let out_bare = Command::new(&bin).args(["--map", "ENDL_001"]).output().unwrap();
    assert!(!out_bare.status.success());
    let stderr_bare = String::from_utf8_lossy(&out_bare.stderr);
    assert!(stderr_bare.contains("使用 --map 直接開啟地圖時必須同時指定 --game <path>。"));
}
