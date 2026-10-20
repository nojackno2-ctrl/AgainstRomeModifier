use against_rome_editor_app::app::{is_original_map_dir, shortcut_action, ShortcutAction};
use against_rome_editor_app::viewport_3d::{camera_key_action, CameraKeyAction};
use eframe::egui::Key;
use std::path::Path;

#[test]
fn ctrl_arrow_nudges_but_plain_arrow_pans_camera() {
    assert_eq!(
        shortcut_action(Key::ArrowDown, true, false, false, false),
        Some(ShortcutAction::Nudge { dx: 0.0, dz: 256.0 })
    );
    assert_eq!(shortcut_action(Key::ArrowDown, false, false, false, false), None);
    assert_eq!(camera_key_action(Key::ArrowDown, true, false), None);
    assert_eq!(
        camera_key_action(Key::ArrowDown, false, false),
        Some(CameraKeyAction::Pan(0.0, -0.1))
    );
}

#[test]
fn text_focus_blocks_everything_but_save() {
    for key in [Key::A, Key::D, Key::Delete, Key::ArrowLeft] {
        assert_eq!(shortcut_action(key, true, false, false, true), None);
    }
    assert_eq!(
        shortcut_action(Key::S, true, false, false, true),
        Some(ShortcutAction::Save)
    );
}

#[test]
fn retail_slots_are_read_only() {
    assert!(is_original_map_dir(Path::new("MAPS/ENDL_003")));
    assert!(is_original_map_dir(Path::new("MAPS/TUTOR_01")));
    assert!(!is_original_map_dir(Path::new("work/ENDL_005")));
}
