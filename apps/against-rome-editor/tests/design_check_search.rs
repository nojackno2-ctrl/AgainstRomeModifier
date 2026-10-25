use against_rome_editor_app::design_check_ui::{DesignCheckUi, IssueRow};
use against_rome_maps::diagnostics::MapIssueSeverity;
use uuid::Uuid;

#[test]
fn search_matches_both_languages_and_code_without_changing_indices() {
    let row = |severity, code: &str, chinese: &str, english: &str| IssueRow {
        severity,
        code: code.into(),
        chinese: chinese.into(),
        english: english.into(),
        world_x: Some(64.0),
        world_z: Some(128.0),
        object_id: Uuid::nil(),
        event_index: -1,
    };
    let rows = vec![
        row(
            MapIssueSeverity::Error,
            "coordinates",
            "座標越界",
            "Outside map",
        ),
        row(
            MapIssueSeverity::Warning,
            "wave-not-hostile",
            "波次未敵對",
            "Wave not hostile",
        ),
    ];
    let mut ui = DesignCheckUi::default();
    for query in [" WAVE ", "未敵對", "NOT-HOSTILE"] {
        ui.search = query.into();
        let found = ui.filtered_rows(&rows);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, 1);
        assert_eq!(found[0].1.focus_world(), Some((64.0, 128.0)));
    }
    ui.show_warnings = false;
    assert!(ui.filtered_rows(&rows).is_empty());
    ui.search.clear();
    assert_eq!(ui.filtered_rows(&rows).len(), 1);
    ui.show_warnings = true;
    assert_eq!(ui.filtered_rows(&rows).len(), 2);
    ui.search = "不存在".into();
    assert!(ui.filtered_rows(&rows).is_empty());
}
