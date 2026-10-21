use against_rome_editor_app::bmp::BmpImage;
use against_rome_editor_app::storage::{is_forbidden_install_path, load_map, safe_save_map};
use against_rome_editor_app::terrain::{CollisionBrushOp, HeightBrushOp, TerrainSession};
use std::fs;
use std::path::Path;

#[test]
fn test_green_channel_exact_preservation_and_non_257_sizes() {
    let temp_dir = std::env::temp_dir().join("ar_test_pixel_preservation_non_257");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // 建立 193x193 (TUTOR_01 尺寸) 原生 BMP，賦予獨特的 Red 與 Blue 通道特徵
    let width = 193;
    let height = 193;
    let initial_bmp = BmpImage::new_solid(width, height, 123, 40, 210); // B=123, G=40, R=210

    let boden_path = temp_dir.join("boden.bmp");
    fs::write(&boden_path, initial_bmp.encode_bmp()).unwrap();

    // 建立 192x192 碰撞圖
    let col_bmp = BmpImage::new_solid(192, 192, 0, 0, 0); // 通行
    let col_path = temp_dir.join("collision.bmp");
    fs::write(&col_path, col_bmp.encode_bmp()).unwrap();

    // 1. 載入地圖
    let mut session = load_map(&temp_dir).expect("Load map should succeed");
    assert_eq!(
        session.vertex_size, 193,
        "Should correctly recognize non-257 193x193 heightmap"
    );
    assert_eq!(
        session.collision_size, 192,
        "Should correctly recognize 192x192 collision grid"
    );
    assert_eq!(
        session.heights()[0],
        40,
        "Initial green channel height must be 40"
    );

    // 2. 使用筆刷修改高度
    session
        .paint_height(10.0, 10.0, 5.0, HeightBrushOp::Raise, 20, None, 0)
        .unwrap();
    session.commit_stroke();
    assert!(session.is_dirty());
    let modified_height = session.heights()[10 * 193 + 10];
    assert!(modified_height > 40);

    // 3. 安全存檔
    let report = safe_save_map(&temp_dir, &mut session).expect("Safe save must succeed");
    assert!(
        report.boden_backup.is_some(),
        "Must create backup of existing boden.bmp"
    );
    let backup_file = report.boden_backup.unwrap();
    assert!(backup_file.exists(), "Backup file must exist on disk");

    // 4. 驗證儲存後的檔案：Green 通道已更新，但 Blue 與 Red 通道 100% 完整保留！
    let saved_bytes = fs::read(&boden_path).unwrap();
    let re_read_bmp = BmpImage::parse(&saved_bytes).expect("Parsed saved boden.bmp");
    assert_eq!(re_read_bmp.width, 193);
    assert_eq!(re_read_bmp.height, 193);

    for y in 0..193 {
        for x in 0..193 {
            let idx = y * 193 + x;
            let b = re_read_bmp.bgr_pixels[idx * 3];
            let g = re_read_bmp.bgr_pixels[idx * 3 + 1];
            let r = re_read_bmp.bgr_pixels[idx * 3 + 2];

            assert_eq!(b, 123, "Blue pixel at ({}, {}) must remain intact", x, y);
            assert_eq!(r, 210, "Red pixel at ({}, {}) must remain intact", x, y);
            assert_eq!(
                g,
                session.heights()[idx],
                "Green channel must match updated height"
            );
        }
    }

    // 清理臨時目錄
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn water_brush_height_and_collision_undo_redo_save_reload_roundtrip() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ar_water_roundtrip_{}_{}",
        std::process::id(),
        unique
    ));
    fs::create_dir(&temp_dir).unwrap();

    let height = BmpImage::new_solid(9, 9, 123, 100, 210);
    fs::write(temp_dir.join("boden.bmp"), height.encode_bmp()).unwrap();
    let mut collision = vec![0; 8 * 8];
    collision[4 * 8 + 4] = 90;
    let collision_bmp = BmpImage::from_collision_channel(8, 8, &collision);
    fs::write(temp_dir.join("collision.bmp"), collision_bmp.encode_bmp()).unwrap();

    let mut session = load_map(&temp_dir).expect("load water fixture");
    let original_heights = session.heights().to_vec();
    let original_collision = session.collision().to_vec();
    let stroke = session
        .paint_water(4.0, 4.0, 2.5, 64, 10, 99.0)
        .expect("paint water");
    assert!(!stroke.heights.is_empty());
    assert!(!stroke.collision.is_empty());
    assert_eq!(
        session.collision()[4 * 8 + 4],
        90,
        "authored collision is preserved"
    );
    assert!(session.commit_stroke());
    let painted_heights = session.heights().to_vec();
    let painted_collision = session.collision().to_vec();

    assert!(session.undo());
    assert_eq!(session.heights(), original_heights);
    assert_eq!(session.collision(), original_collision);
    assert!(session.redo());
    assert_eq!(session.heights(), painted_heights);
    assert_eq!(session.collision(), painted_collision);

    safe_save_map(&temp_dir, &mut session).expect("save water fixture");
    let reloaded = load_map(&temp_dir).expect("reload water fixture");
    assert_eq!(reloaded.heights(), painted_heights);
    assert_eq!(reloaded.collision(), painted_collision);
    assert!(!reloaded.is_dirty());

    fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_undo_redo_stroke_integrity() {
    let mut session = TerrainSession::create_blank(257, 50);

    // Stroke 1: Raise
    session
        .paint_height(50.0, 50.0, 10.0, HeightBrushOp::Raise, 15, None, 0)
        .unwrap();
    session.commit_stroke();
    let after_stroke1 = session.heights()[50 * 257 + 50];
    assert!(after_stroke1 > 50);

    // Stroke 2: Block Collision
    session
        .paint_collision(20.0, 20.0, 4.0, CollisionBrushOp::Block)
        .unwrap();
    session.commit_stroke();
    assert_eq!(session.collision()[20 * session.collision_size + 20], 255);

    // Undo Stroke 2
    assert!(session.undo());
    assert_eq!(
        session.collision()[20 * session.collision_size + 20],
        0,
        "Collision undo verified"
    );
    assert_eq!(
        session.heights()[50 * 257 + 50],
        after_stroke1,
        "Stroke 1 remains untouched"
    );

    // Undo Stroke 1
    assert!(session.undo());
    assert_eq!(session.heights()[50 * 257 + 50], 50, "Height undo verified");

    // Redo Stroke 1
    assert!(session.redo());
    assert_eq!(
        session.heights()[50 * 257 + 50],
        after_stroke1,
        "Height redo verified"
    );

    // Redo Stroke 2
    assert!(session.redo());
    assert_eq!(
        session.collision()[20 * session.collision_size + 20],
        255,
        "Collision redo verified"
    );
}

#[test]
fn test_security_guard_rejects_install_path() {
    let forbidden1 = Path::new(r"C:\Program Files (x86)\Against Rome\MAPS\TEST");
    let forbidden2 = Path::new(r"C:\Program Files\Against Rome\MAPS\ENDL_001");
    let safe = Path::new(r"D:\Github\AgainstRomeModifier\TEMP\test_map");

    assert!(is_forbidden_install_path(forbidden1));
    assert!(is_forbidden_install_path(forbidden2));
    assert!(!is_forbidden_install_path(safe));

    let mut session = TerrainSession::create_blank(257, 32);
    let result = safe_save_map(forbidden1, &mut session);
    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("Security Guard"));
}
