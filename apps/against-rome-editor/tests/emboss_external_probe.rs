//! 確認載入時缺少浮雕圖、外部稍後新增時的存檔行為。
use against_rome_editor_app::{app::EditorApp, bmp::BmpImage, load_map, terrain::SampleChange};

#[test]
fn externally_created_emboss_blocks_height_save_and_retry_preserves_edits() {
    let dir = std::env::temp_dir().join(format!("arm-emboss-external-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("boden.bmp"), BmpImage::new_solid(9, 9, 32, 32, 32).encode_bmp()).unwrap();
    let mut app = EditorApp::initial_state();
    app.set_loaded_map(dir.clone(), load_map(&dir).unwrap()).unwrap();
    let external = BmpImage::new_solid(9, 9, 50, 50, 50).encode_bmp();
    std::fs::write(dir.join("emboss.bmp"), &external).unwrap();
    let terrain = app.session.as_mut().unwrap();
    terrain.apply_height_changes(&[SampleChange { index: 40, before: 32, after: 120 }]);
    terrain.commit_stroke();
    app.safe_save_current();
    assert!(app.status_is_error, "{}", app.status_message);
    assert!(app.status_message.contains("Map source changed externally"), "{}", app.status_message);
    assert_eq!(std::fs::read(dir.join("emboss.bmp")).unwrap(), external);
    assert_eq!(load_map(&dir).unwrap().heights()[40], 32);
    assert!(app.session.as_ref().unwrap().is_dirty());
    std::fs::remove_file(dir.join("emboss.bmp")).unwrap();
    app.safe_save_current();
    assert!(!app.status_is_error, "{}", app.status_message);
    assert_eq!(load_map(&dir).unwrap().heights()[40], 120);
    assert!(!dir.join("emboss.bmp").exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn blank_map_first_save_refuses_preexisting_emboss() {
    let dir = std::env::temp_dir().join(format!("arm-emboss-blank-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let external = BmpImage::new_solid(9, 9, 50, 50, 50).encode_bmp();
    std::fs::write(dir.join("emboss.bmp"), &external).unwrap();
    let mut app = EditorApp::initial_state();
    app.session = Some(against_rome_editor_app::terrain::TerrainSession::create_blank(9, 32));
    app.map_path_input = dir.display().to_string();
    app.safe_save_current();
    assert!(app.status_is_error, "{}", app.status_message);
    assert!(app.status_message.contains("Map source changed externally"), "{}", app.status_message);
    assert_eq!(std::fs::read(dir.join("emboss.bmp")).unwrap(), external);
    assert!(!dir.join("boden.bmp").exists());
    std::fs::remove_file(dir.join("emboss.bmp")).unwrap();
    app.safe_save_current();
    assert!(!app.status_is_error, "{}", app.status_message);
    assert_eq!(load_map(&dir).unwrap().heights()[40], 32);
    std::fs::remove_dir_all(dir).unwrap();
}
