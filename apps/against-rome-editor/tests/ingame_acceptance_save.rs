//! 遊戲內驗收用的存檔產生器（預設忽略）。
//!
//! 以編輯器的 Safe Save 流程，把 `AR_ACCEPT_MAP_DIR` 指向的自製地圖「工作副本」
//! 載入、標記環境設定已變更後存回同一資料夾，並重新載入確認。產生的資料夾可再手動
//! 複製到遊戲的自製測試地圖位置做實機驗收；本測試本身不會碰已安裝的遊戲目錄。
//!
//! 執行：`$env:AR_ACCEPT_MAP_DIR='<工作副本>'; cargo test -p against-rome-editor-app
//! --locked --test ingame_acceptance_save -- --ignored --nocapture`

use against_rome_editor_app::{app::EditorApp, load_map};
use std::path::PathBuf;

#[test]
#[ignore = "needs AR_ACCEPT_MAP_DIR pointing at a scratch working copy of a custom map"]
fn editor_safe_save_roundtrip_on_working_copy() {
    let dir = PathBuf::from(
        std::env::var("AR_ACCEPT_MAP_DIR").expect("set AR_ACCEPT_MAP_DIR to a working copy"),
    );
    assert!(
        !dir.to_string_lossy().to_ascii_lowercase().contains("program files"),
        "refusing to save into an installed game directory"
    );

    let session = load_map(&dir).expect("load working copy");
    let mut app = EditorApp::initial_state();
    app.set_loaded_map(dir.clone(), session).expect("bind map");
    app.map_path_input = dir.display().to_string();
    app.ambience.mark_dirty();
    assert!(app.has_unsaved_changes());

    app.safe_save_current();
    println!("status: {}", app.status_message);
    assert!(!app.status_is_error, "{}", app.status_message);
    assert!(!app.has_unsaved_changes(), "{}", app.status_message);

    // 重新載入存檔結果，確認編輯器可再次開啟。
    let reloaded = load_map(&dir).expect("reload saved map");
    let mut again = EditorApp::initial_state();
    again.set_loaded_map(dir, reloaded).expect("rebind saved map");
    assert!(!again.has_unsaved_changes());
}
