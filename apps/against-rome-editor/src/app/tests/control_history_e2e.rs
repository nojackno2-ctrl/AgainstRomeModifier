use super::*;
use crate::undo_journal::{SubsystemId, UndoTarget};

fn setup() -> (EditorApp, PathBuf, egui::Context) {
    let (mut app, dir, _) = start_camera_app(false);
    app.recovery.root = Some(dir.join("recovery"));
    app.set_app_language(AppLanguage::English);
    (app, dir, egui::Context::default())
}
fn frame(app: &mut EditorApp, ctx: &egui::Context, panel: usize, events: Vec<egui::Event>) -> egui::FullOutput {
    ctx.run(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1800., 1400.))), events, ..Default::default() }, |ctx| {
        match panel { 0 => app.ui_frame(ctx), 1 => app.show_studio_panels(ctx), _ => app.show_map_properties(ctx) }
    })
}
fn click(app: &mut EditorApp, ctx: &egui::Context, panel: usize, label: &str) {
    frame(app, ctx, panel, vec![]);
    let mut out = frame(app, ctx, panel, vec![]);
    let mut found = gui_find(&out, label);
    for _ in 0..10 {
        if found.is_some() { break; }
        out = frame(app, ctx, panel, vec![egui::Event::PointerMoved(egui::pos2(300., 300.)), egui::Event::Scroll(egui::vec2(0., -240.))]);
        found = gui_find(&out, label);
    }
    let pos = found.unwrap_or_else(|| panic!("找不到 UI 按鈕 {label}: {}", gui_texts(&out, 0.))).center();
    for pressed in [true, false] { frame(app, ctx, panel, gui_pointer(pos, pressed)); }
}
fn camera_panel(app: &mut EditorApp) {
    app.campaign.open = true;
    app.campaign.current_tab = crate::campaign_ui::CampaignTab::CameraCinematics;
    app.view_mode = ViewMode::View3D;
}
fn terrain(app: &mut EditorApp) {
    let session = app.session.as_mut().unwrap();
    session.apply_height_changes(&[SampleChange { index: 0, before: 32, after: 45 }]);
    assert!(session.commit_stroke());
}

#[test]
fn batch44_ui_camera_capture_apply_cross_tool_branch_save_reload_and_switch() {
    let (mut app, dir, ctx) = setup();
    let original = app.campaign.camera_history_state();
    let disk = save_file_snapshot(&dir);
    camera_panel(&mut app);
    click(&mut app, &ctx, 1, "Capture Current 3D View");
    assert!(app.campaign.camera_dirty);
    assert!(!app.campaign.camera_pending());
    let captured = app.campaign.camera_history_state();
    terrain(&mut app);
    click(&mut app, &ctx, 1, "Apply Start Camera to Loaded Map");
    assert!(app.campaign.camera_candidate_ready(), "{}", app.status_message);
    assert_eq!(save_file_snapshot(&dir), disk, "候選與復原操作不得先寫入地圖");
    let applied = app.campaign.camera_history_state();
    assert_eq!(app.undo_journal.total_undo_entries(), 3);
    app.navigate_journal(false);
    assert!(app.campaign.camera_history_state() == captured);
    assert_eq!(app.session.as_ref().unwrap().heights()[0], 45);
    app.navigate_journal(false);
    assert_eq!(app.session.as_ref().unwrap().heights()[0], 32);
    app.navigate_journal(false);
    assert!(app.campaign.camera_history_state() == original);
    assert!(!app.has_unsaved_changes());
    for _ in 0..3 { app.navigate_journal(true); }
    assert!(app.campaign.camera_history_state() == applied);
    // 復原套用後，新的擷取必須清除全域 redo，且不能復活舊候選。
    app.navigate_journal(false);
    app.viewport_3d.camera.target.x += 1.;
    click(&mut app, &ctx, 1, "Capture Current 3D View");
    assert!(!app.undo_journal.can_redo());
    click(&mut app, &ctx, 1, "Apply Start Camera to Loaded Map");
    app.safe_save_current();
    assert!(!app.status_is_error, "{}", app.status_message);
    assert!(!app.has_unsaved_changes());
    assert!(!app.undo_journal.can_undo() && !app.undo_journal.can_redo());
    let saved_camera = app.campaign.start_camera.clone();
    app.navigate_journal(false);
    assert_eq!(app.campaign.start_camera, saved_camera);
    app.set_loaded_map(dir.clone(), crate::storage::load_map(&dir).unwrap()).unwrap();
    assert_eq!(app.campaign.start_camera, saved_camera);
    click(&mut app, &ctx, 1, "Capture Current 3D View");
    app.install_blank_map(5);
    assert!(!app.undo_journal.can_undo() && !app.undo_journal.can_redo());
    assert_eq!(app.token(SubsystemId::StartCamera), 0);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn batch44_ui_water_drag_preview_only_then_persist_sync_undo_save_reload() {
    let (mut app, dir, ctx) = setup();
    app.active_tool = ActiveTool::Water;
    frame(&mut app, &ctx, 0, vec![]);
    let out = frame(&mut app, &ctx, 0, vec![]);
    let label = gui_find(&out, "Water surface (G units)").expect("水位滑桿");
    let start = egui::pos2(label.min.x - 65., label.center().y);
    let baseline = app.water_surface;
    let boden = app.ambience.profile.boden.waterlevel;
    frame(&mut app, &ctx, 0, gui_pointer(start, true));
    for dx in [10., 20., 30.] { frame(&mut app, &ctx, 0, vec![egui::Event::PointerMoved(start + egui::vec2(dx, 0.))]); }
    frame(&mut app, &ctx, 0, gui_pointer(start + egui::vec2(30., 0.), false));
    assert_ne!(app.water_surface, baseline);
    let preview = app.water_surface;
    assert_eq!(app.ambience.profile.boden.waterlevel, boden);
    assert!(!app.has_unsaved_changes(), "預覽控制不可誤設地圖 dirty");
    assert_eq!(app.undo_journal.total_undo_entries(), 1, "整次拖曳合併為一步");
    app.navigate_journal(false);
    assert_eq!(app.water_surface, baseline);
    app.navigate_journal(true);
    assert_eq!(app.water_surface, preview);
    app.ambience.open = true;
    click(&mut app, &ctx, 1, "Sync with Editor Water");
    assert_eq!(app.ambience.profile.boden.waterlevel, preview);
    assert!(app.has_unsaved_changes());
    app.navigate_journal(false);
    assert_eq!(app.ambience.profile.boden.waterlevel, boden);
    assert_eq!(app.water_surface, preview);
    assert!(!app.has_unsaved_changes());
    app.navigate_journal(true);
    app.safe_save_current();
    assert!(!app.status_is_error, "{}", app.status_message);
    assert!(!app.has_unsaved_changes());
    app.navigate_journal(false);
    assert_eq!(app.ambience.profile.boden.waterlevel, preview);
    app.set_loaded_map(dir.clone(), crate::storage::load_map(&dir).unwrap()).unwrap();
    assert_eq!(app.ambience.profile.boden.waterlevel, preview);
    assert!(!app.undo_journal.can_undo());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn batch44_ui_map_properties_checkbox_and_original_readonly() {
    let (mut app, dir, ctx) = setup();
    app.map_properties_open = true;
    frame(&mut app, &ctx, 2, vec![]);
    let out = frame(&mut app, &ctx, 2, vec![]);
    let label = gui_find(&out, "Start Camera Enabled:").unwrap();
    let pos = egui::pos2(label.max.x + 12., label.center().y);
    for pressed in [true, false] { frame(&mut app, &ctx, 2, gui_pointer(pos, pressed)); }
    assert!(app.campaign.start_camera.enabled && app.campaign.camera_dirty);
    assert_eq!(app.undo_journal.total_undo_entries(), 1);
    app.navigate_journal(false);
    assert!(!app.campaign.start_camera.enabled && !app.has_unsaved_changes());
    app.navigate_journal(true);
    click(&mut app, &ctx, 2, "Apply");
    assert!(app.campaign.camera_candidate_ready());
    app.navigate_journal(false);
    assert!(!app.campaign.camera_pending() && app.campaign.camera_dirty);
    // 同一 UI 在 TEMP 原版目錄必須保持乾淨。
    let official = dir.join("MAPS/OFFICIAL");
    std::fs::create_dir_all(official.join("SCRIPT")).unwrap();
    std::fs::write(official.join("SCRIPT/ak_level.bci"), against_rome_scenario::BciImage::idle().serialize()).unwrap();
    app.install_loaded_map(official.clone(), TerrainSession::create_blank(5, 32)).unwrap();
    let before = save_file_snapshot(&official);
    camera_panel(&mut app);
    click(&mut app, &ctx, 1, "Capture Current 3D View");
    click(&mut app, &ctx, 1, "Apply Start Camera to Loaded Map");
    app.ambience.open = true;
    app.campaign.open = false;
    app.water_surface = 95.;
    click(&mut app, &ctx, 1, "Sync with Editor Water");
    assert!(!app.has_unsaved_changes());
    assert!(!app.undo_journal.can_undo());
    assert_eq!(save_file_snapshot(&official), before);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn batch44_ui_camera_drag_merges_but_never_crosses_terrain() {
    let (mut app, dir, ctx) = setup();
    app.map_properties_open = true;
    frame(&mut app, &ctx, 2, vec![]);
    let out = frame(&mut app, &ctx, 2, vec![]);
    let pos = out.shapes.iter().find_map(|s| match &s.shape {
        egui::epaint::Shape::Text(t) if t.galley.text().starts_with("X: ") => Some(t.pos + t.galley.size() * 0.5),
        _ => None,
    }).expect("地圖屬性的 X 座標拖曳欄位");
    let original = app.campaign.start_camera.x;
    frame(&mut app, &ctx, 2, gui_pointer(pos, true));
    for dx in [10., 20., 30.] { frame(&mut app, &ctx, 2, vec![egui::Event::PointerMoved(pos + egui::vec2(dx, 0.))]); }
    let first = app.campaign.start_camera.x;
    assert_ne!(first, original);
    assert_eq!(app.undo_journal.total_undo_entries(), 1);
    terrain(&mut app);
    for dx in [40., 50.] { frame(&mut app, &ctx, 2, vec![egui::Event::PointerMoved(pos + egui::vec2(dx, 0.))]); }
    frame(&mut app, &ctx, 2, gui_pointer(pos + egui::vec2(50., 0.), false));
    let last = app.campaign.start_camera.x;
    assert_ne!(last, first);
    assert_eq!(app.undo_journal.total_undo_entries(), 3, "同一手勢不得吞掉中間的地形操作");
    app.navigate_journal(false);
    assert_eq!(app.campaign.start_camera.x, first);
    assert_eq!(app.session.as_ref().unwrap().heights()[0], 45);
    app.navigate_journal(false);
    assert_eq!(app.session.as_ref().unwrap().heights()[0], 32);
    app.navigate_journal(false);
    assert_eq!(app.campaign.start_camera.x, original);
    assert!(!app.has_unsaved_changes());
    for _ in 0..3 { app.navigate_journal(true); }
    assert_eq!(app.campaign.start_camera.x, last);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn batch44_failed_save_preserves_control_history_and_water_preserves_other_dirty_fields() {
    let (mut app, dir, ctx) = setup();
    let before = app.capture_water_control();
    let level = app.ambience.profile.boden.waterlevel;
    app.ambience.sync_water_level(level + 10.);
    app.finish_control_edit(&ctx, before);
    app.ambience.profile.boden.water_bump_amplitude += 1;
    app.ambience.mark_dirty();
    app.navigate_journal(false);
    assert_eq!(app.ambience.profile.boden.waterlevel, level);
    assert!(app.ambience.is_dirty(), "復原水位不能清除其他氛圍變更的 dirty");
    app.navigate_journal(true);
    let journal = app.undo_journal.clone();
    app.safe_save_current_using(|_| Err("批次44注入交易失敗".into()));
    assert!(app.status_is_error);
    assert_eq!(app.undo_journal, journal);
    app.navigate_journal(false);
    assert_eq!(app.ambience.profile.boden.waterlevel, level);
    assert!(app.ambience.is_dirty());
    std::fs::remove_dir_all(dir).unwrap();
}
