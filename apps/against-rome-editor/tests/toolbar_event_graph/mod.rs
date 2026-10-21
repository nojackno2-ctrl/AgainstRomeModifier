//! 在 app 單元測試中驗證跨元件工具列接線，避免正式 API 暴露面板內部可變狀態。
use super::*;
use crate::event_graph::{GraphEdge, NodeKind};
use against_rome_scenario::{Action, BciImage, Event, ScenarioDocument};
use std::fs;
use uuid::Uuid;

struct Fixture {
    root: PathBuf,
    map: PathBuf,
}
impl Fixture {
    fn new(pfil: bool) -> (Self, EditorApp) {
        let root = std::env::temp_dir().join(format!("arm-toolbar-graph-{}", Uuid::new_v4()));
        let map = root.join("MAPS/ENDL_005");
        fs::create_dir_all(map.join("SCRIPT")).unwrap();
        fs::create_dir_all(root.join("SYSTEM/CLAK")).unwrap();
        fs::write(
            root.join("SYSTEM/CLAK/cl_scint.ini"),
            "[ObjDefName]\nUNIT=FigUnit\n",
        )
        .unwrap();
        let raw = BciImage::idle().serialize();
        let bytes = if pfil {
            let mut bytes = vec![0; 64];
            bytes[..4].copy_from_slice(b"PFIL");
            bytes[16..20].copy_from_slice(&(raw.len() as u32).to_le_bytes());
            for chunk in raw.chunks(8) {
                bytes.push(((1u16 << chunk.len()) - 1) as u8);
                bytes.extend_from_slice(chunk);
            }
            bytes
        } else {
            raw
        };
        fs::write(map.join("SCRIPT/ak_level.bci"), bytes).unwrap();
        write_document(&map, "saved");
        let mut app = EditorApp::initial_state();
        app.set_loaded_map(map.clone(), TerrainSession::create_blank(5, 32))
            .unwrap();
        (Self { root, map }, app)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn event(text: &str) -> Event {
    Event {
        name: "toolbar event".into(),
        actions: vec![Action {
            text: text.into(),
            ..Default::default()
        }],
        ..Default::default()
    }
}
fn write_document(map: &Path, text: &str) {
    let doc = ScenarioDocument {
        events: vec![event(text)],
        ..Default::default()
    };
    fs::write(map.join("arm_scenario.json"), doc.to_json().unwrap()).unwrap();
}
fn edit(graph: &mut EventGraphUi, text: &str) {
    let node = graph
        .graph
        .nodes
        .iter_mut()
        .find(|n| matches!(n.kind, NodeKind::MessageAction { .. }))
        .unwrap();
    if let NodeKind::MessageAction { message_text } = &mut node.kind {
        *message_text = text.into();
    }
    graph.mark_dirty();
}
fn graph_snapshot(graph: &EventGraphUi) -> String {
    format!("{:?}", graph.graph)
}
fn frame(app: &mut EditorApp) {
    let ctx = egui::Context::default();
    let _ = ctx.run(Default::default(), |ctx| app.show_studio_panels(ctx));
}
fn request_apply(app: &mut EditorApp) {
    app.event_graph.apply_requested = true;
    frame(app);
    assert!(
        !app.event_graph.apply_requested,
        "request must be consumed after rendering"
    );
}

#[test]
fn toolbar_apply_then_safe_save_and_reload_raw_and_pfil() {
    for pfil in [false, true] {
        let (fixture, mut app) = Fixture::new(pfil);
        let original = fs::read(fixture.map.join("SCRIPT/ak_level.bci")).unwrap();
        app.open_toolbar_event_graph();
        assert!(app.scenario.is_loaded());
        assert_eq!(
            app.event_graph.compile_to_scenario_events()[0].actions[0].text,
            "saved"
        );
        // Include an existing scenario history entry and a live unsaved event in the graph.
        app.scenario
            .add_events(&[Event {
                name: "live".into(),
                ..event("live")
            }])
            .unwrap();
        app.open_toolbar_event_graph();
        let before = app.scenario.document().clone();
        assert_eq!(app.event_graph.compile_to_scenario_events().len(), 2);
        edit(&mut app.event_graph, "applied");
        request_apply(&mut app);
        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(!app.event_graph.is_dirty());
        assert!(app.scenario.dirty());
        let applied = app.scenario.document().clone();
        assert_eq!(applied.events[0].actions[0].text, "applied");
        assert_eq!(applied.events[1], before.events[1]);
        assert!(app.scenario.undo_events());
        assert_eq!(app.scenario.document(), &before);
        assert!(
            app.scenario.undo_events(),
            "pre-existing history is retained"
        );
        assert_eq!(app.scenario.document().events.len(), 1);
        assert!(app.scenario.redo_events() && app.scenario.redo_events());
        assert_eq!(app.scenario.document(), &applied);
        assert_eq!(
            fs::read(fixture.map.join("SCRIPT/ak_level.bci")).unwrap(),
            original,
            "Apply is memory only"
        );
        app.safe_save_current();
        assert!(!app.status_is_error, "{}", app.status_message);
        assert!(!app.scenario.dirty());
        assert!(!app.event_graph.is_dirty());
        let mut reloaded = ScenarioUi::default();
        reloaded.load_session(&fixture.map).unwrap();
        assert_eq!(reloaded.document(), &applied);
        let compiled = fs::read(fixture.map.join("SCRIPT/ak_level.bci")).unwrap();
        assert_eq!(compiled.starts_with(b"PFIL"), pfil);
        let raw = if pfil {
            against_rome_data::lzss::decompress_pfil(&compiled, None).unwrap()
        } else {
            compiled
        };
        BciImage::parse(&raw).unwrap();
    }
}

#[test]
fn invalid_graph_apply_fails_closed_and_keeps_graph_document_history_and_disk() {
    let (fixture, mut app) = Fixture::new(false);
    app.open_toolbar_event_graph();
    app.scenario
        .add_events(&[Event {
            name: "live".into(),
            ..event("live")
        }])
        .unwrap();
    app.open_toolbar_event_graph();
    let doc = app.scenario.document().clone();
    let disk = fs::read(fixture.map.join("arm_scenario.json")).unwrap();
    edit(&mut app.event_graph, "keep this draft");
    app.event_graph.graph.edges.push(GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    ));
    let graph = graph_snapshot(&app.event_graph);
    request_apply(&mut app);
    assert!(app.status_is_error);
    assert!(app.event_graph.is_dirty());
    assert_eq!(graph_snapshot(&app.event_graph), graph);
    assert_eq!(app.scenario.document(), &doc);
    assert!(app.scenario.dirty() && app.scenario.can_undo_events());
    assert!(!app.scenario.can_redo_events());
    assert!(app
        .event_graph
        .diagnostics
        .iter()
        .any(|d| d.code == "INVALID_LINK"));
    app.safe_save_current();
    assert!(app.status_is_error && app.status_message.contains("EventGraph"));
    assert_eq!(
        fs::read(fixture.map.join("arm_scenario.json")).unwrap(),
        disk
    );
    assert!(!fixture.map.join("boden.bmp").exists());
    assert!(app.scenario.undo_events());
    assert_eq!(app.scenario.document().events.len(), 1);
}

#[test]
fn unapplied_graph_blocks_safe_save_and_reopening_preserves_edits() {
    let (fixture, mut app) = Fixture::new(false);
    app.open_toolbar_event_graph();
    let disk = fs::read(fixture.map.join("arm_scenario.json")).unwrap();
    edit(&mut app.event_graph, "pending");
    let graph = graph_snapshot(&app.event_graph);
    app.open_toolbar_event_graph();
    frame(&mut app);
    assert_eq!(graph_snapshot(&app.event_graph), graph);
    app.safe_save_current();
    assert!(app.status_is_error && app.status_message.contains("unapplied"));
    assert!(app.event_graph.is_dirty());
    assert!(!app.scenario.dirty());
    assert_eq!(
        fs::read(fixture.map.join("arm_scenario.json")).unwrap(),
        disk
    );
    assert!(!fixture.map.join("boden.bmp").exists());
}

#[test]
fn changed_live_scenario_or_reload_rejects_apply_without_replacing_either_draft() {
    for reload in [false, true] {
        let (fixture, mut app) = Fixture::new(false);
        app.open_toolbar_event_graph();
        edit(&mut app.event_graph, "graph draft");
        if reload {
            write_document(&fixture.map, "reloaded");
            app.scenario.load_session(&fixture.map).unwrap();
        } else {
            app.scenario
                .replace_event(0, event("scenario draft"))
                .unwrap();
        }
        let graph = graph_snapshot(&app.event_graph);
        let doc = app.scenario.document().clone();
        app.open_toolbar_event_graph();
        request_apply(&mut app);
        assert!(app.status_is_error && app.status_message.contains("scenario changed"));
        assert_eq!(graph_snapshot(&app.event_graph), graph);
        assert_eq!(app.scenario.document(), &doc);
        assert!(app.event_graph.is_dirty());
    }
}

#[test]
fn absent_scenario_apply_preserves_graph_edits() {
    let (_fixture, mut app) = Fixture::new(false);
    app.open_toolbar_event_graph();
    edit(&mut app.event_graph, "keep me");
    let graph = graph_snapshot(&app.event_graph);
    app.scenario.reset_session(None);
    app.open_toolbar_event_graph();
    request_apply(&mut app);
    assert!(app.status_is_error && app.status_message.contains("no loaded scenario"));
    assert!(!app.scenario.is_loaded());
    assert!(app.event_graph.is_dirty());
    assert_eq!(graph_snapshot(&app.event_graph), graph);
}

#[test]
fn successful_toolbar_apply_preserves_separate_scenario_graph_pending_edits() {
    let (_fixture, mut app) = Fixture::new(false);
    app.open_toolbar_event_graph();
    edit(app.scenario.graph_ui_mut(), "separate draft");
    app.scenario.graph_ui_mut().apply_requested = true;
    let separate = graph_snapshot(app.scenario.graph_ui());
    edit(&mut app.event_graph, "toolbar draft");
    request_apply(&mut app);
    assert!(!app.status_is_error, "{}", app.status_message);
    assert_eq!(
        app.scenario.document().events[0].actions[0].text,
        "toolbar draft"
    );
    assert_eq!(graph_snapshot(app.scenario.graph_ui()), separate);
    assert!(app.scenario.graph_ui().is_dirty() && app.scenario.graph_ui().apply_requested);
    app.safe_save_current();
    assert!(
        app.status_is_error,
        "separate pending graph must still block saving"
    );
}

#[test]
fn clean_graph_syncs_on_reload_and_does_not_rebuild_each_frame() {
    let (fixture, mut app) = Fixture::new(false);
    app.open_toolbar_event_graph();
    let initial = graph_snapshot(&app.event_graph);
    frame(&mut app);
    assert_eq!(graph_snapshot(&app.event_graph), initial);
    write_document(&fixture.map, "reloaded");
    app.scenario.load_session(&fixture.map).unwrap();
    frame(&mut app);
    assert_eq!(
        app.event_graph.compile_to_scenario_events()[0].actions[0].text,
        "reloaded"
    );
    assert!(!app.event_graph.is_dirty());
}

#[test]
fn discard_then_reopen_reloads_the_unchanged_live_document() {
    let (_fixture, mut app) = Fixture::new(false);
    app.open_toolbar_event_graph();
    edit(&mut app.event_graph, "discard me");
    app.event_graph.discard_and_close();
    app.open_toolbar_event_graph();
    assert_eq!(
        app.event_graph.compile_to_scenario_events()[0].actions[0].text,
        "saved"
    );
    assert!(!app.event_graph.is_dirty());
    assert!(!app.scenario.dirty());
}
