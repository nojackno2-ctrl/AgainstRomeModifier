use against_rome_editor_app::{app::shortcut_action, viewport_3d::camera_key_action};
use eframe::egui::Key;

#[test]
fn guide_shortcut_table_matches_live_mappings() {
    let guide = include_str!("../../../docs/map-editor-user-guide.md");
    let table = guide
        .split("## 9. 快捷鍵總表")
        .nth(1)
        .expect("快捷鍵章節")
        .split("### 9.1")
        .next()
        .expect("快捷鍵表格");
    let mut count = 0;
    for row in table.lines().filter(|line| line.starts_with("| `")) {
        let cell = row.split('|').nth(1).expect("按鍵欄");
        for chord in cell.split('`').skip(1).step_by(2) {
            let parts: Vec<_> = chord.split(" + ").collect();
            let key_name = *parts.last().expect("按鍵名稱");
            let key = match key_name {
                "[" => Key::OpenBracket,
                "]" => Key::CloseBracket,
                "1" => Key::Num1,
                "2" => Key::Num2,
                "3" => Key::Num3,
                "4" => Key::Num4,
                "5" => Key::Num5,
                "6" => Key::Num6,
                "7" => Key::Num7,
                "8" => Key::Num8,
                "9" => Key::Num9,
                other => Key::from_name(other).unwrap_or_else(|| panic!("未知按鍵：{other}")),
            };
            assert!(
                parts[..parts.len() - 1]
                    .iter()
                    .all(|m| matches!(*m, "Ctrl" | "Shift" | "Alt")),
                "未知修飾鍵：{chord}"
            );
            let ctrl = parts.contains(&"Ctrl");
            let shift = parts.contains(&"Shift");
            let alt = parts.contains(&"Alt");
            let polled = |source: &str, name: &str| {
                let keys = source
                    .split(&format!("const {name}:"))
                    .nth(1)
                    .expect("按鍵常數")
                    .split("= [")
                    .nth(1)
                    .expect("按鍵陣列")
                    .split("];")
                    .next()
                    .expect("陣列結尾");
                keys.split(',')
                    .any(|entry| entry.trim() == format!("egui::Key::{key:?}"))
            };
            let app = include_str!("../src/app.rs");
            let camera = include_str!("../src/viewport_3d.rs");
            assert!(
                (polled(app, "SHORTCUT_KEYS")
                    && shortcut_action(key, ctrl, shift, alt, false).is_some())
                    || (!shift
                        && polled(camera, "CAMERA_KEYS")
                        && camera_key_action(key, ctrl, alt).is_some()),
                "指南無對應映射：{chord}"
            );
            count += 1;
        }
    }
    assert!(count >= 35, "快捷鍵表格不可消失或漏列");
}
