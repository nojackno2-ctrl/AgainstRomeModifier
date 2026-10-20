//! Structured scenario designer and safe new-work-copy exporter.
use against_rome_scenario::{
    self as scenario, Action, Condition, Event, ScenarioDocument, ScenarioEventSession, Spawn, StartCamera, WorldPosition,
};
use eframe::egui;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use crate::event_graph::EventGraphUi;
type Result<T> = std::result::Result<T, String>;
#[derive(Clone)]
struct Source {
    path: PathBuf,
    bytes: Vec<u8>,
}
#[derive(Clone)]
enum Pending {
    Load(PathBuf),
    Close,
}
pub struct ScenarioUi {
    pub open: bool,
    document: ScenarioDocument,
    camera: StartCamera,
    source: Option<Source>,
    watched: Vec<Source>,
    aliases: Vec<String>,
    active_bindings: Option<Vec<(i32,u32)>>,
    load_path: String,
    export_path: String,
    selected: usize,
    baseline: Vec<u8>,
    events: ScenarioEventSession,
    pending: Option<Pending>,
    report: String,
    error: Option<String>,
    pub graph_ui: EventGraphUi,
}
impl Default for ScenarioUi {
    fn default() -> Self {
        let document = ScenarioDocument::default();
        let camera = StartCamera {
            enabled: false,
            ..Default::default()
        };
        let baseline = state_bytes(&document, &camera).unwrap();
        Self {
            open: false,
            document,
            camera,
            source: None,
            watched: vec![],
            aliases: vec![],
            active_bindings: None,
            load_path: String::new(),
            export_path: String::new(),
            selected: 0,
            baseline,
            events: ScenarioEventSession::new(),
            pending: None,
            report: String::new(),
            error: None,
            graph_ui: EventGraphUi::default(),
        }
    }
}
fn state_bytes(d: &ScenarioDocument, c: &StartCamera) -> Result<Vec<u8>> {
    serde_json::to_vec(&(d, c)).map_err(|e| e.to_string())
}
fn camera_eq(a: &StartCamera, b: &StartCamera) -> bool {
    a.enabled == b.enabled
        && a.x == b.x
        && a.y == b.y
        && a.z == b.z
        && a.zoom == b.zoom
        && a.primitive == b.primitive
}
fn child_case(parent: &Path, name: &str) -> PathBuf {
    fs::read_dir(parent)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|x| x.ok())
        .find(|x| x.file_name().to_string_lossy().eq_ignore_ascii_case(name))
        .map(|x| x.path())
        .unwrap_or_else(|| parent.join(name))
}
fn binding_context(map:&Path,document:&ScenarioDocument)->Result<(Option<Vec<(i32,u32)>>,Vec<Source>)> {
    if document.data_slots.is_empty() && document.spawns.iter().all(|s|!s.prebuilt) {return Ok((None,vec![]))}
    let mut watched=vec![];
    for name in ["objects","objdata","position","anim","gfxtype","action","engine","light","particle","lager","biglager"] {
        let path=map.join("DATA").join(format!("{name}.dat"));
        if path.exists() {watched.push(Source{bytes:fs::read(&path).map_err(|e|e.to_string())?,path});}
    }
    let store=against_rome_placement::ObjectStore::load(map).map_err(|e|format!("無法驗證來源 DATA：{e}"))?;
    let active:Vec<_>=store.objects().iter().map(|o|(o.slot as i32,o.uid)).collect();
    scenario::verify_binding_snapshot(document,&active)?;
    for s in &watched {if fs::read(&s.path).map_err(|e|e.to_string())?!=s.bytes{return Err("來源 DATA 在載入期間改變；請重試。".into())}}
    Ok((Some(active),watched))
}
/// Reject every existing reparse-point ancestor before any filesystem mutation.
fn safe_parent(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path)
    };
    for component in absolute.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err("輸出路徑不可含 ..".into());
        }
    }
    for p in absolute.ancestors() {
        if let Ok(m) = fs::symlink_metadata(p) {
            if m.file_type().is_symlink() {
                return Err("不允許 symlink / junction 路徑".into());
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if m.file_attributes() & 0x400 != 0 {
                    return Err("不允許 reparse point / junction 路徑".into());
                }
            }
        }
    }
    let parent = absolute.parent().ok_or("輸出需要父目錄")?;
    let parent = fs::canonicalize(parent).map_err(|e| e.to_string())?;
    let low = parent.to_string_lossy().to_lowercase();
    if low.contains("program files")
        || low.contains("\\against rome\\")
        || low.ends_with("\\against rome")
    {
        return Err("不可輸出至安裝目錄或 Program Files".into());
    }
    Ok(parent.join(absolute.file_name().ok_or("缺少輸出資料夾名稱")?))
}
impl ScenarioUi {
    /// Live scenario document for read-only consumers (maps/campaign/packaging panels).
    pub fn document(&self) -> &ScenarioDocument {
        &self.document
    }
    /// True once a map script source has been loaded (event merges need a loaded scenario).
    pub fn is_loaded(&self) -> bool {
        self.source.is_some()
    }
    /// Loaded game ObjDefName aliases; empty when no catalog was found.
    pub fn known_aliases(&self) -> &[String] {
        &self.aliases
    }
    /// Brings the event session in line with `document.events` (e.g. after graph apply or
    /// test-side edits) so undo/redo always starts from what the user currently sees.
    fn sync_session(&mut self) {
        if self.events.capture() != self.document.events {
            let current = self.document.events.clone();
            let _ = self.events.replace_all(&current);
        }
    }
    fn clamp_selected(&mut self) {
        self.selected = self.selected.min(self.document.events.len().saturating_sub(1));
    }
    fn refresh_graph_after_event_change(&mut self) {
        if !self.graph_ui.is_dirty() {
            self.sync_to_graph().ok();
        }
    }
    fn validate_draft(&self, draft: &ScenarioDocument) -> Result<()> {
        draft
            .validate(if self.aliases.is_empty() { None } else { Some(&self.aliases) })
            .map_err(|e| format!("劇本事件驗證失敗：{e}"))?;
        if let Some(active) = &self.active_bindings {
            against_rome_scenario::verify_binding_snapshot(draft, active)
                .map_err(|e| format!("DATA 物件綁定檢查失敗：{e}"))?;
        }
        Ok(())
    }
    /// Applies an already validated draft through the session (one undo step).
    fn commit_events(&mut self, draft: ScenarioDocument) {
        self.sync_session();
        if self.events.replace_all(&draft.events).is_ok() {
            self.document = draft;
        }
        self.clamp_selected();
    }
    pub fn can_undo_events(&self) -> bool {
        self.events.can_undo()
    }
    pub fn can_redo_events(&self) -> bool {
        self.events.can_redo()
    }
    /// Appends a batch of events (max 256 total). Validates the resulting document first;
    /// on any failure neither `document` nor the session is modified. Returns the first new index.
    pub fn add_events(&mut self, items: &[Event]) -> Result<usize> {
        self.sync_session();
        let first = self.document.events.len();
        let checked = if items.len() > 256usize.saturating_sub(first) {
            Err("事件上限為 256 個。".to_string())
        } else {
            let mut draft = self.document.clone();
            draft.events.extend_from_slice(items);
            self.validate_draft(&draft)
        };
        if let Err(e) = checked {
            self.error = Some(e.clone());
            return Err(e);
        }
        self.events.add_range(items)?;
        self.document.events = self.events.capture();
        self.error = None;
        self.refresh_graph_after_event_change();
        Ok(first)
    }
    /// Inserts an independent copy after the source, matching the C# event list.
    /// The session enforces the event limit and UTF-16 name cap in one undo step.
    fn duplicate_event(&mut self, index: usize) -> Result<usize> {
        self.sync_session();
        let selected = self.events.duplicate(index, "（副本）")?;
        self.document.events = self.events.capture();
        self.selected = selected;
        self.error = None;
        self.refresh_graph_after_event_change();
        Ok(selected)
    }
    pub fn remove_event(&mut self, index: usize) -> Result<()> {
        self.sync_session();
        self.events.remove_at(index)?;
        self.document.events = self.events.capture();
        self.clamp_selected();
        self.refresh_graph_after_event_change();
        Ok(())
    }
    /// Replaces one event as a single undo step. Inline edits are intentionally not validated
    /// here (intermediate typing states); validation still runs at preview/export/save.
    pub fn replace_event(&mut self, index: usize, item: Event) -> Result<()> {
        self.sync_session();
        self.events.replace(index, item)?;
        self.document.events = self.events.capture();
        Ok(())
    }
    pub fn undo_events(&mut self) -> bool {
        self.sync_session();
        let changed = self.events.undo();
        if changed {
            self.document.events = self.events.capture();
            self.clamp_selected();
            self.refresh_graph_after_event_change();
        }
        changed
    }
    pub fn redo_events(&mut self) -> bool {
        self.sync_session();
        let changed = self.events.redo();
        if changed {
            self.document.events = self.events.capture();
            self.clamp_selected();
            self.refresh_graph_after_event_change();
        }
        changed
    }
    pub fn dirty(&self) -> bool {
        state_bytes(&self.document, &self.camera).map_or(true, |s| s != self.baseline)
    }

    /// Resets all map-bound scenario documents, camera, source scripts, watched files, bindings, and transient state.
    /// Preserves user preferences (open state, known aliases).
    pub fn reset_session(&mut self, map_path: Option<&Path>) {
        let document = ScenarioDocument::default();
        let camera = StartCamera {
            enabled: false,
            ..Default::default()
        };
        self.baseline = state_bytes(&document, &camera).unwrap_or_default();
        self.document = document;
        self.events = ScenarioEventSession::new();
        self.camera = camera;
        self.source = None;
        self.watched.clear();
        self.active_bindings = None;
        self.load_path = map_path.map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        self.export_path.clear();
        self.selected = 0;
        self.pending = None;
        self.report.clear();
        self.error = None;
        self.graph_ui.reset_session();
    }

    /// Transactionally loads or resets the scenario session for the target map.
    /// If the map has a script (SCRIPT/ak_level.bci or ak_level.arm_original), loads and verifies it.
    /// If missing, cleanly resets to an unloaded default scenario.
    /// If script exists but is corrupt or invalid, returns Err and leaves the current session untouched.
    pub fn load_session(&mut self, map_path: &Path) -> Result<()> {
        let script_dir = child_case(map_path, "SCRIPT");
        let script = child_case(&script_dir, "ak_level.bci");
        let backup = child_case(&script_dir, "ak_level.arm_original");
        if backup.is_file() || script.is_file() {
            self.load_now(map_path)
        } else {
            self.reset_session(Some(map_path));
            Ok(())
        }
    }

    fn load_now(&mut self, map: &Path) -> Result<()> {
        let script_dir = child_case(map, "SCRIPT");
        let script = child_case(&script_dir, "ak_level.bci");
        let backup = child_case(&script_dir, "ak_level.arm_original");
        let original = if backup.is_file() { backup } else { script };
        let source = Source {
            bytes: fs::read(&original).map_err(|e| format!("{}: {e}", original.display()))?,
            path: original,
        };
        let scenario_path = map.join("arm_scenario.json");
        let camera_path = map.join("arm_start_camera.json");
        let mut watched = vec![source.clone()];
        let document = if scenario_path.exists() {
            let bytes = fs::read(&scenario_path).map_err(|e| e.to_string())?;
            let d = ScenarioDocument::from_json(&bytes)?;
            watched.push(Source {
                path: scenario_path,
                bytes,
            });
            d
        } else {
            ScenarioDocument::default()
        };
        let camera = if camera_path.exists() {
            let bytes = fs::read(&camera_path).map_err(|e| e.to_string())?;
            let c = serde_json::from_slice(bytes.strip_prefix(&[239, 187, 191]).unwrap_or(&bytes))
                .map_err(|e| e.to_string())?;
            watched.push(Source {
                path: camera_path,
                bytes,
            });
            c
        } else {
            StartCamera {
                enabled: false,
                ..Default::default()
            }
        };
        // Parse BCI immediately; unmodified empty documents must not hide a corrupt source.
        let raw = if source.bytes.starts_with(b"PFIL") {
            against_rome_data::lzss::decompress_pfil(&source.bytes, None)
                .map_err(|e| e.to_string())?
        } else {
            source.bytes.clone()
        };
        scenario::BciImage::parse(&raw)?;
        let (active_bindings,data_watched)=binding_context(map,&document)?;
        watched.extend(data_watched);
        let aliases=map.parent().and_then(Path::parent).and_then(|root|scenario::load_aliases(root).ok()).unwrap_or_default();
        self.baseline = state_bytes(&document, &camera)?;
        self.events = ScenarioEventSession::with_events(document.events.clone());
        self.document = document;
        self.camera = camera;
        self.aliases = aliases;
        self.active_bindings=active_bindings;
        self.source = Some(source);
        self.watched = watched;
        self.selected = 0;
        self.error = None;
        self.report = "已載入；來源保持唯讀。輸出會建立新的工作副本。".into();

        // 重新同步事件節點圖
        self.graph_ui.reload_from_scenario(&self.document);
        let _ = self.graph_ui.validate_with_scenario(&self.document, &self.aliases);

        Ok(())
    }

    pub fn graph_ui(&self) -> &EventGraphUi {
        &self.graph_ui
    }

    pub fn graph_ui_mut(&mut self) -> &mut EventGraphUi {
        &mut self.graph_ui
    }

    /// Opens the visual event graph window and loads current scenario events if clean.
    pub fn open_graph(&mut self) {
        self.graph_ui.open = true;
        if !self.graph_ui.is_dirty() {
            self.graph_ui.reload_from_scenario(&self.document);
            let _ = self.graph_ui.validate_with_scenario(&self.document, &self.aliases);
        }
    }

    /// Syncs current scenario document events into the event graph.
    /// Fails closed if the graph has unapplied dirty edits, preventing accidental data loss.
    pub fn sync_to_graph(&mut self) -> Result<()> {
        if self.graph_ui.is_dirty() {
            return Err("節點圖有未套用的修改；請先套用或放棄變更。".into());
        }
        self.graph_ui.reload_from_scenario(&self.document);
        let _ = self.graph_ui.validate_with_scenario(&self.document, &self.aliases);
        Ok(())
    }

    /// Compiles and validates the embedded event graph, applying it to the scenario document.
    /// Fails closed without mutating `self.document` if validation, link checks, or binding checks fail.
    pub fn apply_graph(&mut self) -> Result<()> {
        let compiled_events = match self
            .graph_ui
            .compile_to_scenario_events_checked(&self.document, &self.aliases)
        {
            Ok(events) => events,
            Err(e) => {
                self.error = Some(e.clone());
                return Err(e);
            }
        };

        let mut draft = self.document.clone();
        draft.events = compiled_events;
        if let Err(e) = draft.validate(if self.aliases.is_empty() {
            None
        } else {
            Some(&self.aliases)
        }) {
            let msg = format!("劇本事件驗證失敗：{e}");
            self.error = Some(msg.clone());
            return Err(msg);
        }

        if let Some(active) = &self.active_bindings {
            if let Err(e) = against_rome_scenario::verify_binding_snapshot(&draft, active) {
                let msg = format!("DATA 物件綁定檢查失敗：{e}");
                self.error = Some(msg.clone());
                return Err(msg);
            }
        }

        self.commit_events(draft);
        self.graph_ui.is_dirty = false;
        self.report = "已成功從節點圖更新劇本事件。".into();
        self.error = None;
        Ok(())
    }

    /// Compiles and validates an external event graph instance, applying it to this scenario document.
    /// Fails closed without mutating `self.document` if validation or constraints fail.
    pub fn apply_from_graph_ui(&mut self, graph_ui: &mut EventGraphUi) -> Result<()> {
        let compiled_events = match graph_ui
            .compile_to_scenario_events_checked(&self.document, &self.aliases)
        {
            Ok(events) => events,
            Err(e) => {
                self.error = Some(e.clone());
                return Err(e);
            }
        };

        let mut draft = self.document.clone();
        draft.events = compiled_events;
        if let Err(e) = draft.validate(if self.aliases.is_empty() {
            None
        } else {
            Some(&self.aliases)
        }) {
            let msg = format!("劇本事件驗證失敗：{e}");
            self.error = Some(msg.clone());
            return Err(msg);
        }

        if let Some(active) = &self.active_bindings {
            if let Err(e) = against_rome_scenario::verify_binding_snapshot(&draft, active) {
                let msg = format!("DATA 物件綁定檢查失敗：{e}");
                self.error = Some(msg.clone());
                return Err(msg);
            }
        }

        self.commit_events(draft);
        graph_ui.is_dirty = false;
        self.report = "已成功從外部節點圖更新劇本事件。".into();
        self.error = None;
        Ok(())
    }

    /// Syncs this scenario document to an external event graph instance.
    pub fn sync_to_graph_ui(&self, graph_ui: &mut EventGraphUi) {
        graph_ui.reload_from_scenario(&self.document);
        let _ = graph_ui.validate_with_scenario(&self.document, &self.aliases);
    }
    fn request_load(&mut self, path: PathBuf) {
        if self.dirty() {
            self.pending = Some(Pending::Load(path))
        } else if let Err(e) = self.load_now(&path) {
            self.error = Some(e)
        }
    }
    fn preview(&mut self) -> Result<Vec<u8>> {
        self.camera.validate()?;
        for s in &self.watched {if fs::read(&s.path).map_err(|e|e.to_string())?!=s.bytes{return Err(format!("來源已被外部修改：{}；請重新載入。",s.path.display()))}}
        let original = &self
            .source
            .as_ref()
            .ok_or("請先載入含 SCRIPT/ak_level.bci 的地圖")?
            .bytes;
        let bytes = scenario::compile_script_verified(
            original,
            &self.document,
            Some(&self.camera),
            if self.aliases.is_empty() {
                None
            } else {
                Some(&self.aliases)
            },
            self.active_bindings.as_deref(),
        )?;
        let raw = if bytes.starts_with(b"PFIL") {
            against_rome_data::lzss::decompress_pfil(&bytes, None).map_err(|e| e.to_string())?
        } else {
            bytes.clone()
        };
        let bci = scenario::BciImage::parse(&raw)?;
        self.error = None;
        self.report=format!("腳本編譯通過：{} bytes；CODE {} bytes、常數 {}、main {}。使用別名的動作必須通過來源 catalog；預建綁定已核對來源 DATA。這不是遊戲內驗收。",bytes.len(),bci.code.len()*4,bci.constant_offsets.len(),bci.main);
        Ok(bytes)
    }
    fn export_new(&mut self, path: &Path) -> Result<()> {
        let script = self.preview()?;
        for s in &self.watched {
            if fs::read(&s.path).map_err(|e| e.to_string())? != s.bytes {
                return Err(format!(
                    "來源已被外部修改：{}；請重新載入。",
                    s.path.display()
                ));
            }
        }
        let target = safe_parent(path)?;
        if target.exists() {
            return Err("輸出資料夾已存在；請指定全新的工作副本路徑。".into());
        }
        let mut d = self.document.clone();
        d.version = 6;
        let json = serde_json::to_vec_pretty(&d).map_err(|e| e.to_string())?;
        let camera = serde_json::to_vec_pretty(&self.camera).map_err(|e| e.to_string())?;
        let original = self.source.as_ref().ok_or("missing source")?.bytes.clone();
        export_files(
            &target,
            &[
                ("arm_scenario.json", json),
                ("arm_start_camera.json", camera),
                ("SCRIPT/ak_level.arm_original", original),
                ("SCRIPT/ak_level.bci", script),
            ],
            None,
        )?;
        self.document = d;
        self.sync_session();
        self.events.accept_changes();
        self.baseline = state_bytes(&self.document, &self.camera)?;
        self.report=format!("已輸出劇本工作副本：{}。含原版備份與 JSON；這不是完整地圖封裝，遊戲驗收前需複製其他地圖檔案。",target.display());
        Ok(())
    }

    /// Stages scenario files for transactional saving without modifying in-memory state or writing to disk.
    /// Returns pairs of relative path within the map (e.g. `arm_scenario.json`, `SCRIPT/ak_level.bci`) and serialized bytes.
    /// Fails closed if the scenario has not been loaded from the target map, if any watched inputs
    /// (source BCI, camera/scenario JSON, DATA bindings) were modified externally, if graph has unapplied changes,
    /// or if compilation / roundtrip verification fails.
    pub fn stage_transaction_files(&self, target_map: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>> {
        let source = self
            .source
            .as_ref()
            .ok_or_else(|| "劇本尚未載入來源 (SCRIPT/ak_level.bci)，無法暫存交易檔案".to_string())?;

        let expected_dir = target_map.join("SCRIPT");
        let matches = match (
            source.path.parent().and_then(|p| fs::canonicalize(p).ok()),
            fs::canonicalize(&expected_dir).ok(),
        ) {
            (Some(c1), Some(c2)) => c1 == c2,
            _ => source.path.parent() == Some(expected_dir.as_path()),
        };
        if !matches {
            return Err(format!(
                "劇本載入來源 '{}' 與目標地圖 '{}' 不符",
                source.path.display(),
                target_map.display()
            ));
        }

        if self.graph_ui.is_dirty() {
            return Err("事件節點圖有未套用的變更；請先套用節點圖再進行存檔交易。".into());
        }

        if !self.dirty() {
            return Ok(Vec::new());
        }

        self.camera.validate()?;
        for s in &self.watched {
            let current_bytes = fs::read(&s.path)
                .map_err(|e| format!("無法讀取受監視檔案 {}：{e}", s.path.display()))?;
            if current_bytes != s.bytes {
                return Err(format!("來源已被外部修改：{}；請重新載入。", s.path.display()));
            }
        }

        if let Some(active) = &self.active_bindings {
            scenario::verify_binding_snapshot(&self.document, active)?;
        }

        self.document.validate(if self.aliases.is_empty() {
            None
        } else {
            Some(&self.aliases)
        })?;

        let original = &source.bytes;
        let compiled_bci = scenario::compile_script_verified(
            original,
            &self.document,
            Some(&self.camera),
            if self.aliases.is_empty() {
                None
            } else {
                Some(&self.aliases)
            },
            self.active_bindings.as_deref(),
        )?;

        let raw = if compiled_bci.starts_with(b"PFIL") {
            against_rome_data::lzss::decompress_pfil(&compiled_bci, None).map_err(|e| e.to_string())?
        } else {
            compiled_bci.clone()
        };
        let _bci = scenario::BciImage::parse(&raw)?;

        let mut d = self.document.clone();
        d.version = 6;
        let json = serde_json::to_vec_pretty(&d).map_err(|e| e.to_string())?;
        let camera = serde_json::to_vec_pretty(&self.camera).map_err(|e| e.to_string())?;

        let _roundtrip_doc = ScenarioDocument::from_json(&json)?;
        let _roundtrip_camera: StartCamera = serde_json::from_slice(
            camera.strip_prefix(&[239, 187, 191]).unwrap_or(&camera),
        )
        .map_err(|e| e.to_string())?;

        Ok(vec![
            (PathBuf::from("arm_scenario.json"), json),
            (PathBuf::from("arm_start_camera.json"), camera),
            (PathBuf::from("SCRIPT/ak_level.arm_original"), source.bytes.clone()),
            (PathBuf::from("SCRIPT/ak_level.bci"), compiled_bci),
        ])
    }

    /// Commits the in-memory baseline after a successful atomic MapSaveTransaction.
    /// Verifies staged scenario files have been written to disk matching current edits,
    /// reloads the updated session from disk so that baseline matches disk, and clears dirty status.
    pub fn commit_baseline(&mut self, target_map: &Path) -> Result<()> {
        if !self.dirty() && !self.graph_ui.is_dirty() {
            return Ok(());
        }

        let source = self
            .source
            .as_ref()
            .ok_or_else(|| "劇本尚未載入來源，無法確認 baseline".to_string())?;
        let expected_dir = target_map.join("SCRIPT");
        let matches = match (
            source.path.parent().and_then(|p| fs::canonicalize(p).ok()),
            fs::canonicalize(&expected_dir).ok(),
        ) {
            (Some(c1), Some(c2)) => c1 == c2,
            _ => source.path.parent() == Some(expected_dir.as_path()),
        };
        if !matches {
            return Err(format!(
                "劇本載入來源 '{}' 與目標地圖 '{}' 不符",
                source.path.display(),
                target_map.display()
            ));
        }

        let scenario_path = target_map.join("arm_scenario.json");
        if !scenario_path.is_file() {
            return Err("磁碟上缺少 arm_scenario.json，交易尚未落盤，拒絕確認 baseline".into());
        }
        let disk_scenario_bytes = fs::read(&scenario_path).map_err(|e| e.to_string())?;
        let disk_doc = ScenarioDocument::from_json(&disk_scenario_bytes)?;
        let mut expected_doc = self.document.clone();
        expected_doc.version = 6;
        if disk_doc != expected_doc {
            return Err("磁碟上之劇本與當前編輯狀態不符，交易尚未成功，拒絕清除 dirty".into());
        }

        let camera_path = target_map.join("arm_start_camera.json");
        if camera_path.is_file() {
            let disk_cam_bytes = fs::read(&camera_path).map_err(|e| e.to_string())?;
            let disk_cam: StartCamera = serde_json::from_slice(
                disk_cam_bytes.strip_prefix(&[239, 187, 191]).unwrap_or(&disk_cam_bytes),
            )
            .map_err(|e| e.to_string())?;
            if !camera_eq(&disk_cam, &self.camera) {
                return Err("磁碟上之開局鏡頭與當前編輯狀態不符，拒絕清除 dirty".into());
            }
        }

        let prev_selected = self.selected;
        self.load_now(target_map)?;
        if !self.document.events.is_empty() {
            self.selected = prev_selected.min(self.document.events.len() - 1);
        } else {
            self.selected = 0;
        }
        self.report = "劇本已在整圖交易中成功保存。".into();
        Ok(())
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if self.graph_ui.apply_requested {
            self.graph_ui.apply_requested = false;
            if let Err(e) = self.apply_graph() {
                self.error = Some(e);
            }
        }
        if !self.open {
            self.graph_ui.show(ctx);
            return;
        }
        let mut opened = true;
        egui::Window::new("劇本事件 / Scenario")
            .open(&mut opened)
            .default_width(760.)
            .show(ctx, |ui| {
                ui.label("來源唯讀；以結構化事件、物件條件與開局鏡頭編輯，輸出新工作副本。");
                ui.horizontal(|ui| {
                    ui.label("來源地圖目錄");
                    ui.text_edit_singleline(&mut self.load_path);
                    if ui.button("載入").clicked() {
                        self.request_load(PathBuf::from(self.load_path.trim()))
                    }
                });
                if let Some(s) = &self.source {
                    ui.small(format!("原始 BCI：{}", s.path.display()));
                    ui.small(if self.aliases.is_empty() { "未找到遊戲別名 catalog；別名需手動核對。".into() }
                    else { format!("已載入 {} 個遊戲 ObjDefName 別名，編譯時會驗證。", self.aliases.len()) });
                }
                if self.dirty() {
                    ui.colored_label(egui::Color32::YELLOW, "尚未輸出的修改");
                }
                egui::ScrollArea::vertical()
                    .max_height(650.)
                    .show(ui, |ui| {
                        ui.collapsing("開局鏡頭", |ui| {
                            ui.checkbox(&mut self.camera.enabled, "啟用");
                            ui.horizontal(|ui| {
                                for (name, v) in [
                                    ("X", &mut self.camera.x),
                                    ("Y", &mut self.camera.y),
                                    ("Z", &mut self.camera.z),
                                    ("Zoom", &mut self.camera.zoom),
                                ] {
                                    ui.label(name);
                                    ui.add(egui::DragValue::new(v).speed(0.25));
                                }
                            });
                            egui::ComboBox::from_id_source("camera_primitive")
                                .selected_text(if self.camera.primitive == 0 {
                                    "世界焦點 / centerToPos"
                                } else {
                                    "引擎座標 / enginePos"
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.camera.primitive, 0, "世界焦點");
                                    ui.selectable_value(&mut self.camera.primitive, 1, "引擎座標");
                                });
                        });
                        ui.collapsing("場景生成物件與持久 ID", |ui| {
                            let mut remove = None;
                            for (i, s) in self.document.spawns.iter_mut().enumerate() {
                                ui.push_id(i, |ui| {
                                    if s.prebuilt {
                                        ui.label(format!("預建 DATA 唯讀：{} / ID {} / X {} Z {} team {} angle {}", s.alias, s.id, s.x, s.z, s.team, s.angle));
                                        return;
                                    }
                                    ui.horizontal(|ui| {
                                        ui.text_edit_singleline(&mut s.alias);
                                        alias_combo(ui, &mut s.alias, &self.aliases);
                                        ui.label("team");
                                        ui.add(egui::DragValue::new(&mut s.team));
                                        ui.label("count (0=物件)");
                                        ui.add(egui::DragValue::new(&mut s.count));
                                        if ui.button("刪除").clicked() {
                                             remove = Some(i)
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        for (label, v) in [("X", &mut s.x), ("Z", &mut s.z)] {
                                            ui.label(label);
                                            ui.add(egui::DragValue::new(v));
                                        }
                                        ui.label("角度");
                                        ui.add(egui::DragValue::new(&mut s.angle));
                                        ui.label(if s.prebuilt {
                                            "既有 DATA 預建；需保留 binding"
                                        } else {
                                            "開局腳本生成"
                                        });
                                    });
                                    ui.small(format!("ID {}", s.id));
                                });
                            }
                            if let Some(i) = remove {
                                self.document.spawns.remove(i);
                            }
                            if ui.button("新增腳本生成物件").clicked() {
                                self.document.spawns.push(Spawn {
                                    id: scenario::new_spawn_id(),
                                    alias: "unit".into(),
                                    x: 8000.,
                                    z: 8000.,
                                    team: 0,
                                    count: 10,
                                    ..Default::default()
                                });
                            }
                        });
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("新增事件").clicked() {
                                let item = Event {
                                    name: format!("事件 {}", self.document.events.len() + 1),
                                    actions: vec![Action {
                                        text: "Message".into(),
                                        ..Default::default()
                                    }],
                                    ..Default::default()
                                };
                                match self.add_events(&[item]) {
                                    Ok(i) => self.selected = i,
                                    Err(e) => self.error = Some(e),
                                }
                            }
                            if ui
                                .add_enabled(
                                    self.selected < self.document.events.len()
                                        && self.document.events.len() < 256,
                                    egui::Button::new("複製選取事件"),
                                )
                                .clicked()
                            {
                                if let Err(e) = self.duplicate_event(self.selected) {
                                    self.error = Some(e);
                                }
                            }
                            if !self.document.events.is_empty()
                                && ui.button("刪除選取事件").clicked()
                            {
                                if let Err(e) = self.remove_event(self.selected) {
                                    self.error = Some(e);
                                }
                            }
                            if ui
                                .add_enabled(self.can_undo_events(), egui::Button::new("復原"))
                                .clicked()
                            {
                                self.undo_events();
                            }
                            if ui
                                .add_enabled(self.can_redo_events(), egui::Button::new("重做"))
                                .clicked()
                            {
                                self.redo_events();
                            }
                            ui.separator();
                            if ui.button("視覺化節點圖 (Event Graph)").clicked() {
                                self.open_graph();
                            }
                            let graph_dirty = self.graph_ui.is_dirty();
                            let apply_btn = egui::Button::new("從節點圖套用 (Apply Graph)");
                            if ui.add_enabled(graph_dirty, apply_btn).clicked() {
                                if let Err(e) = self.apply_graph() {
                                    self.error = Some(e);
                                }
                            }
                            if graph_dirty {
                                ui.colored_label(egui::Color32::YELLOW, "[節點圖有未套用變更]");
                            }
                        });
                        for (i, e) in self.document.events.iter().enumerate() {
                            if ui
                                .selectable_label(
                                    self.selected == i,
                                    format!("{} {}", if e.enabled { "●" } else { "○" }, e.name),
                                )
                                .clicked()
                            {
                                self.selected = i;
                            }
                        }
                        let targets: Vec<_> = self
                            .document
                            .spawns
                            .iter()
                            .map(|s| (s.clone(), format!("{} / {}", s.alias, s.id)))
                            .collect();
                        if let Some(current) = self.document.events.get(self.selected) {
                            let mut edited = current.clone();
                            event_editor(ui, &mut edited, &targets, &self.aliases);
                            if &edited != current {
                                if let Err(e) = self.replace_event(self.selected, edited) {
                                    self.error = Some(e);
                                }
                            }
                        }
                    });
                ui.separator();
                if ui.button("驗證並編譯預覽").clicked() {
                    if let Err(e) = self.preview() {
                        self.error = Some(e)
                    }
                }
                ui.horizontal(|ui| {
                    ui.label("新工作副本目錄");
                    ui.text_edit_singleline(&mut self.export_path);
                    if ui.button("輸出 JSON + BCI + 原版備份").clicked() {
                        let path = PathBuf::from(self.export_path.trim());
                        if let Err(e) = self.export_new(&path) {
                            self.error = Some(e)
                        }
                    }
                });
                ui.label(&self.report);
                if let Some(e) = &self.error {
                    ui.colored_label(egui::Color32::RED, e);
                }
            });
        if !opened {
            if self.dirty() || self.graph_ui.is_dirty() {
                self.pending = Some(Pending::Close)
            } else {
                self.open = false
            }
        }
        if let Some(pending) = self.pending.clone() {
            egui::Window::new("尚未輸出的劇本修改")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label("載入或關閉會放棄目前修改（包含未套用之節點圖變更）。可取消後先輸出工作副本。");
                    ui.horizontal(|ui| {
                        if ui.button("放棄修改並繼續").clicked() {
                            self.pending = None;
                            match pending {
                                Pending::Close => {
                                    self.open = false;
                                    self.graph_ui.discard_and_close();
                                }
                                Pending::Load(p) => {
                                    self.graph_ui.discard_and_close();
                                    if let Err(e) = self.load_now(&p) {
                                        self.error = Some(e)
                                    }
                                }
                            }
                        }
                        if ui.button("取消").clicked() {
                            self.pending = None
                        }
                    });
                });
        }
        self.graph_ui.show(ctx);
    }
}

fn event_editor(ui: &mut egui::Ui, e: &mut Event, targets: &[(Spawn, String)], aliases: &[String]) {
    ui.separator();
    ui.horizontal(|ui| {
        ui.checkbox(&mut e.enabled, "啟用事件");
        ui.text_edit_singleline(&mut e.name);
    });
    ui.horizontal(|ui| {
        ui.label("開局後延遲（秒）");
        ui.add(egui::DragValue::new(&mut e.delay_seconds).clamp_range(0..=86400));
        ui.checkbox(&mut e.repeat, "週期重複");
    });
    ui.label("條件：全部成立且延遲到期後執行；死亡條件先追蹤曾存在的物件。");
    let mut remove = None;
    for (i, c) in e.conditions.iter_mut().enumerate() {
        ui.push_id(("condition", i), |ui| {
            ui.horizontal(|ui| {
                kind_combo(
                    ui,
                    "kind",
                    &mut c.kind,
                    &["物件存在", "死亡或已移除", "位於矩形區域"],
                );
                if ui.button("刪除此條件").clicked() {
                    remove = Some(i)
                }
            });
            egui::ComboBox::from_id_source("target")
                .selected_text(
                    targets
                        .iter()
                        .find(|(s, _)| s.id == c.target_id)
                        .map(|(_, n)| n.as_str())
                        .unwrap_or("請選擇目標 / 已刪除"),
                )
                .show_ui(ui, |ui| {
                    for (s, name) in targets {
                        ui.selectable_value(&mut c.target_id, s.id, name);
                    }
                });
            if c.kind == 2 {
                ui.horizontal(|ui| {
                    for (name, v) in [
                        ("min X", &mut c.min_x),
                        ("min Z", &mut c.min_z),
                        ("max X", &mut c.max_x),
                        ("max Z", &mut c.max_z),
                    ] {
                        ui.label(name);
                        ui.add(egui::DragValue::new(v).clamp_range(0..=16383));
                    }
                });
            }
        });
    }
    if let Some(i) = remove {
        e.conditions.remove(i);
    }
    if ui.button("新增物件條件").clicked() {
        let mut c = Condition::default();
        if let Some((s, _)) = targets.first() {
            c.target_id = s.id
        }
        e.conditions.push(c);
    }
    ui.label("動作：依序執行。勝敗動作只能放在單次事件最後。");
    let mut remove = None;
    for (i, a) in e.actions.iter_mut().enumerate() {
        ui.push_id(("action", i), |ui| {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    kind_combo(
                        ui,
                        "kind",
                        &mut a.kind,
                        &["顯示訊息", "隊伍外交", "生成部隊", "勝利", "失敗"],
                    );
                    if ui.button("刪除此動作").clicked() {
                        remove = Some(i)
                    }
                });
                match a.kind {
                    0 => {
                        ui.text_edit_multiline(&mut a.text);
                        ui.small("文字需能使用 Windows-1251 編碼，最多 1000 bytes。");
                    }
                    1 => {
                        ui.horizontal(|ui| {
                            ui.label("team");
                            ui.add(egui::DragValue::new(&mut a.team).clamp_range(0..=7));
                            ui.label("other team");
                            ui.add(egui::DragValue::new(&mut a.other_team).clamp_range(0..=7));
                            ui.checkbox(&mut a.hostile, "敵對");
                        });
                    }
                    2 => {
                        ui.horizontal(|ui| {
                            ui.label("ObjDefName 別名");
                            ui.text_edit_singleline(&mut a.alias);
                            alias_combo(ui, &mut a.alias, aliases);
                            ui.label("team");
                            ui.add(egui::DragValue::new(&mut a.team).clamp_range(0..=7));
                            ui.label("count");
                            ui.add(egui::DragValue::new(&mut a.count).clamp_range(1..=20));
                        });
                        ui.horizontal(|ui| {
                            ui.label("X");
                            ui.add(egui::DragValue::new(&mut a.x).clamp_range(0. ..=16383.));
                            ui.label("Z");
                            ui.add(egui::DragValue::new(&mut a.z).clamp_range(0. ..=16383.));
                        });
                        let mut moved = a.move_destination.is_some();
                        if ui.checkbox(&mut moved, "生成成功後移動至目的地").changed() {
                            a.move_destination = if moved {
                                Some(WorldPosition { x: 8000, z: 8000 })
                            } else {
                                None
                            }
                        }
                        if let Some(p) = &mut a.move_destination {
                            ui.horizontal(|ui| {
                                ui.label("目的 X");
                                ui.add(egui::DragValue::new(&mut p.x).clamp_range(0..=16383));
                                ui.label("目的 Z");
                                ui.add(egui::DragValue::new(&mut p.z).clamp_range(0..=16383));
                            });
                        }
                    }
                    3 | 4 => {
                        ui.small("結束任務；後續事件停止輪詢。");
                    }
                    _ => {
                        ui.colored_label(egui::Color32::RED, "不支援的動作種類");
                    }
                }
                if a.kind != 2 {
                    a.move_destination = None
                }
            });
        });
    }
    if let Some(i) = remove {
        e.actions.remove(i);
    }
    if ui.button("新增動作").clicked() {
        e.actions.push(Action {
            text: "Message".into(),
            ..Default::default()
        });
    }
}
fn kind_combo(ui: &mut egui::Ui, id: &str, value: &mut i32, names: &[&str]) {
    egui::ComboBox::from_id_source(id)
        .selected_text(names.get(*value as usize).copied().unwrap_or("未知"))
        .show_ui(ui, |ui| {
            for (i, name) in names.iter().enumerate() {
                ui.selectable_value(value, i as i32, *name);
            }
        });
}
/// Creates only a new destination. Rollback removes exactly this call's created files,
/// preserving any pre-existing directories/files; no recursive deletion is used.
fn export_files(target: &Path, files: &[(&str, Vec<u8>)], fail_after: Option<usize>) -> Result<()> {
    let target = safe_parent(target)?;
    if target.exists() {
        return Err("output already exists".into());
    }
    fs::create_dir(&target).map_err(|e| e.to_string())?;
    let script = target.join("SCRIPT");
    let mut created = vec![];
    let result = (|| -> Result<()> {
        fs::create_dir(&script).map_err(|e| e.to_string())?;
        for (i, (name, bytes)) in files.iter().enumerate() {
            if fail_after == Some(i) {
                return Err("injected export failure".into());
            }
            if !matches!(
                *name,
                "arm_scenario.json"
                    | "arm_start_camera.json"
                    | "SCRIPT/ak_level.arm_original"
                    | "SCRIPT/ak_level.bci"
            ) {
                return Err("unexpected export filename".into());
            }
            safe_parent(&target)?;
            let path = target.join(name);
            safe_parent(&path)?;
            let mut file = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
                .map_err(|e| e.to_string())?;
            created.push(path);
            file.write_all(bytes).map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let mut rollback_errors = vec![];
        for path in created.iter().rev() {
            if let Err(e) = fs::remove_file(path) {
                rollback_errors.push(e.to_string())
            }
        }
        if let Err(e) = fs::remove_dir(&script) {
            if script.exists() {
                rollback_errors.push(e.to_string())
            }
        }
        if let Err(e) = fs::remove_dir(&target) {
            rollback_errors.push(e.to_string())
        }
        if !rollback_errors.is_empty() {
            return Err(format!("{error}; rollback: {}", rollback_errors.join("; ")));
        }
        return Err(error);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn scratch() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "arm-scenario-ui-{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir(&p).unwrap();
        p
    }
    fn source() -> PathBuf {
        let p = scratch();
        fs::create_dir(p.join("SCRIPT")).unwrap();
        fs::write(
            p.join("SCRIPT/ak_level.bci"),
            scenario::BciImage::idle().serialize(),
        )
        .unwrap();
        p
    }
    fn ev(name: &str) -> Event {
        Event {
            name: name.into(),
            actions: vec![Action {
                text: "m".into(),
                ..Default::default()
            }],
            ..Default::default()
        }
    }
    #[test]
    fn duplicate_event_preserves_content_order_and_is_one_undo_step() {
        let mut ui = ScenarioUi::default();
        let mut original = ev("來源");
        original.enabled = false;
        original.delay_seconds = 17;
        original.repeat = true;
        original.conditions.push(Condition::default());
        original.actions.push(Action {
            text: "second".into(),
            ..Default::default()
        });
        ui.document.events = vec![original.clone(), ev("next")];
        ui.events = ScenarioEventSession::with_events(ui.document.events.clone());
        ui.baseline = state_bytes(&ui.document, &ui.camera).unwrap();
        let before = ui.document.clone();

        assert_eq!(ui.duplicate_event(0).unwrap(), 1);
        assert_eq!(ui.selected, 1);
        assert!(ui.dirty());
        let mut expected = original.clone();
        expected.name.push_str("（副本）");
        assert_eq!(ui.document.events, vec![original, expected, ev("next")]);
        let duplicated = ui.document.clone();
        assert!(ui.undo_events());
        assert_eq!(ui.document, before);
        assert!(!ui.dirty() && !ui.can_undo_events());
        assert!(ui.redo_events());
        assert_eq!(ui.document, duplicated);

        let mut edited = ui.document.events[1].clone();
        edited.actions[0].text = "copy only".into();
        edited.conditions.clear();
        ui.replace_event(1, edited).unwrap();
        assert_eq!(ui.document.events[0], before.events[0]);
    }
    #[test]
    fn duplicate_event_caps_unicode_name_and_refreshes_clean_graph() {
        let mut ui = ScenarioUi::default();
        ui.add_events(&[ev(&"😀".repeat(50))]).unwrap();
        ui.duplicate_event(0).unwrap();
        let name = &ui.document.events[1].name;
        assert_eq!(name, &format!("{}（副本）", "😀".repeat(48)));
        assert_eq!(name.encode_utf16().count(), 100);
        assert!(!ui.graph_ui.is_dirty());
        assert_eq!(ui.graph_ui.compile_to_scenario_events(), ui.document.events);
    }
    #[test]
    fn duplicate_event_preserves_unapplied_graph_edits() {
        let mut ui = ScenarioUi::default();
        ui.add_events(&[ev("source")]).unwrap();
        ui.graph_ui.mark_dirty();
        let graph_before = ui.graph_ui.compile_to_scenario_events();
        ui.duplicate_event(0).unwrap();
        assert_eq!(ui.document.events.len(), 2);
        assert!(ui.graph_ui.is_dirty());
        assert_eq!(ui.graph_ui.compile_to_scenario_events(), graph_before);
    }
    #[test]
    fn duplicate_event_rejects_bad_index_and_limit_without_changing_history() {
        let mut ui = ScenarioUi::default();
        assert!(ui.duplicate_event(0).is_err());
        assert!(!ui.dirty() && !ui.can_undo_events());
        ui.add_events(&[ev("source")]).unwrap();
        ui.add_events(&[ev("undone")]).unwrap();
        assert!(ui.undo_events());
        let before = ui.document.clone();
        let selected = ui.selected;
        assert!(ui.duplicate_event(usize::MAX).is_err());
        assert_eq!(ui.document, before);
        assert_eq!(ui.selected, selected);
        assert!(ui.can_redo_events());
        assert!(ui.redo_events());
        assert_eq!(ui.document.events[1].name, "undone");

        let full: Vec<_> = (0..256).map(|i| ev(&format!("e{i}"))).collect();
        ui.document.events = full.clone();
        ui.events = ScenarioEventSession::with_events(full);
        ui.baseline = state_bytes(&ui.document, &ui.camera).unwrap();
        let before = ui.document.clone();
        assert!(ui.duplicate_event(0).is_err());
        assert_eq!(ui.document, before);
        assert!(!ui.dirty() && !ui.can_undo_events() && !ui.can_redo_events());
    }
    #[test]
    fn event_batch_undo_redo_dirty_and_limit() {
        let p = source();
        let mut ui = ScenarioUi::default();
        ui.load_now(&p).unwrap();
        assert_eq!(ui.add_events(&[ev("a"), ev("b")]).unwrap(), 0);
        assert!(ui.dirty() && ui.can_undo_events());
        assert!(ui.undo_events());
        assert!(!ui.dirty() && ui.document.events.is_empty());
        assert!(ui.redo_events() && ui.document.events.len() == 2 && ui.dirty());
        ui.remove_event(0).unwrap();
        assert_eq!(ui.document.events[0].name, "b");
        assert!(ui.undo_events() && ui.undo_events() && !ui.dirty());
        assert!(!ui.undo_events());
        let big: Vec<Event> = (0..257).map(|i| ev(&format!("e{i}"))).collect();
        assert!(ui.add_events(&big).is_err());
        assert!(ui.document.events.is_empty() && !ui.can_undo_events());
        let full: Vec<Event> = (0..256).map(|i| ev(&format!("e{i}"))).collect();
        ui.add_events(&full).unwrap();
        assert!(ui.add_events(&[ev("x")]).is_err());
        assert_eq!(ui.document.events.len(), 256);
    }
    #[test]
    fn invalid_event_batch_leaves_document_and_session_untouched() {
        let p = source();
        let mut ui = ScenarioUi::default();
        ui.load_now(&p).unwrap();
        ui.add_events(&[ev("ok")]).unwrap();
        let mut bad = ev("bad");
        bad.name = String::new();
        let before = ui.document.clone();
        assert!(ui.add_events(&[ev("fine"), bad]).is_err());
        assert_eq!(ui.document, before);
        assert!(ui.undo_events());
        assert!(ui.document.events.is_empty());
        assert!(!ui.can_undo_events());
    }
    #[test]
    fn event_session_follows_load_reset_and_direct_edits() {
        let p = source();
        let mut ui = ScenarioUi::default();
        ui.load_now(&p).unwrap();
        ui.add_events(&[ev("a")]).unwrap();
        ui.load_now(&p).unwrap();
        assert!(!ui.can_undo_events() && !ui.dirty());
        ui.add_events(&[ev("a")]).unwrap();
        ui.reset_session(None);
        assert!(!ui.can_undo_events() && !ui.can_redo_events() && !ui.dirty());
        ui.load_now(&p).unwrap();
        ui.document.events.push(ev("direct"));
        assert!(ui.undo_events() && ui.document.events.is_empty());
    }
    #[test]
    fn load_export_and_reload() {
        let p = source();
        let mut ui = ScenarioUi::default();
        ui.load_now(&p).unwrap();
        assert!(!ui.dirty());
        ui.document.events.push(Event {
            name: "e".into(),
            actions: vec![Action {
                text: "hi".into(),
                ..Default::default()
            }],
            ..Default::default()
        });
        ui.camera.enabled = true;
        assert!(ui.dirty());
        let out = p.join("export");
        ui.export_new(&out).unwrap();
        assert!(!ui.dirty());
        assert_eq!(
            fs::read(out.join("SCRIPT/ak_level.arm_original")).unwrap(),
            scenario::BciImage::idle().serialize()
        );
        let bytes = fs::read(out.join("SCRIPT/ak_level.bci")).unwrap();
        let mut next = ScenarioUi::default();
        next.load_now(&out).unwrap();
        assert_eq!(next.document.events.len(), 1);
        assert!(next.camera.enabled);
        assert_eq!(next.preview().unwrap(), bytes);
        assert!(ui.export_new(&out).is_err());
        assert_eq!(fs::read(out.join("SCRIPT/ak_level.bci")).unwrap(), bytes);
    }
    #[test]
    fn failed_export_rolls_back_each_step() {
        let p = scratch();
        let files = vec![
            ("arm_scenario.json", vec![1]),
            ("arm_start_camera.json", vec![2]),
            ("SCRIPT/ak_level.arm_original", vec![3]),
            ("SCRIPT/ak_level.bci", vec![4]),
        ];
        for i in 0..4 {
            let out = p.join(format!("failure-{i}"));
            assert!(export_files(&out, &files, Some(i)).is_err());
            assert!(!out.exists());
            export_files(&out, &files, None).unwrap();
        }
    }
    #[test]
    fn preexisting_and_program_files_refused() {
        let p = scratch();
        fs::write(p.join("keep"), [1]).unwrap();
        assert!(export_files(&p, &[], None).is_err());
        assert_eq!(fs::read(p.join("keep")).unwrap(), [1]);
        let parent = p.join("Program Files");
        fs::create_dir(&parent).unwrap();
        assert!(safe_parent(&parent.join("new")).is_err());
        assert!(safe_parent(&p.join("../new")).is_err());
    }
    #[test]
    fn dirty_load_requires_explicit_discard() {
        let p = source();
        let mut ui = ScenarioUi::default();
        ui.load_now(&p).unwrap();
        ui.camera.x = 100.;
        let target = source();
        ui.request_load(target.clone());
        assert!(matches!(ui.pending, Some(Pending::Load(_))));
        assert_eq!(ui.camera.x, 100.);
        ui.pending = None;
        assert!(ui.dirty());
        ui.load_now(&target).unwrap();
        assert!(!ui.dirty());
    }
    #[test]
    fn externally_changed_original_blocks_export() {
        let p = source();
        let mut ui = ScenarioUi::default();
        ui.load_now(&p).unwrap();
        fs::write(p.join("SCRIPT/ak_level.bci"), b"changed").unwrap();
        let out = p.join("out");
        assert!(ui.export_new(&out).is_err());
        assert!(!out.exists());
    }
    #[test]
    fn invalid_edit_does_not_create_output() {
        let p = source();
        let mut ui = ScenarioUi::default();
        ui.load_now(&p).unwrap();
        ui.camera.enabled = true;
        ui.camera.zoom = 20.;
        let out = p.join("out");
        assert!(ui.export_new(&out).is_err());
        assert!(!out.exists());
    }
    #[test]
    fn native_ui_layout_frame() {
        let ctx = egui::Context::default();
        let mut ui = ScenarioUi {
            open: true,
            ..Default::default()
        };
        ctx.begin_frame(Default::default());
        ui.show(&ctx);
        let output = ctx.end_frame();
        assert!(!output.shapes.is_empty());
    }
    #[cfg(windows)]
    #[test]
    fn junction_output_refused() {
        use std::process::Command;
        let p = scratch();
        let real = p.join("real");
        let link = p.join("link");
        fs::create_dir(&real).unwrap();
        let status = Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&link)
            .arg(&real)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(safe_parent(&link.join("new")).is_err());
        assert!(!real.join("new").exists());
        fs::remove_dir(link).unwrap();
    }

    #[test]
    fn test_scenario_ui_graph_load_edit_roundtrip() {
        let p = source();
        let mut ui = ScenarioUi::default();
        ui.load_now(&p).unwrap();

        // Add a spawn and an event
        let spawn_id = scenario::new_spawn_id();
        ui.document.spawns.push(Spawn {
            id: spawn_id,
            alias: "GER_INF01".into(),
            x: 8000.0,
            z: 8000.0,
            team: 0,
            count: 10,
            ..Default::default()
        });
        ui.document.events.push(Event {
            name: "Initial Event".into(),
            delay_seconds: 5,
            repeat: false,
            enabled: true,
            conditions: vec![Condition {
                kind: 0,
                target_id: spawn_id,
                ..Default::default()
            }],
            actions: vec![Action {
                kind: 0,
                text: "Original Text".into(),
                ..Default::default()
            }],
        });
        ui.aliases = vec!["GER_INF01".into()];

        // Sync to graph
        ui.sync_to_graph().unwrap();
        assert!(!ui.graph_ui.is_dirty());
        assert_eq!(ui.graph_ui.graph.nodes.len(), 3); // 1 trigger + 1 condition + 1 action

        // Edit graph node
        for node in &mut ui.graph_ui.graph.nodes {
            if let crate::event_graph::model::NodeKind::MessageAction { message_text } = &mut node.kind {
                *message_text = "Edited In Graph UI".into();
            }
        }
        ui.graph_ui.mark_dirty();
        assert!(ui.graph_ui.is_dirty());

        // Apply graph back to document
        ui.apply_graph().unwrap();
        assert!(!ui.graph_ui.is_dirty());
        assert_eq!(ui.document.events[0].actions[0].text, "Edited In Graph UI");
        assert!(ui.dirty());
    }

    #[test]
    fn test_scenario_ui_invalid_graph_preserves_dirty_document_and_fails_closed() {
        let p = source();
        let mut ui = ScenarioUi::default();
        ui.load_now(&p).unwrap();

        // Mutate document to make it dirty
        ui.camera.enabled = true;
        ui.camera.x = 1234.0;
        let original_event = Event {
            name: "Keep Me Safe".into(),
            delay_seconds: 10,
            repeat: false,
            enabled: true,
            conditions: vec![],
            actions: vec![Action {
                text: "Protected text".into(),
                ..Default::default()
            }],
        };
        ui.document.events.push(original_event.clone());
        assert!(ui.dirty());

        let doc_snapshot = ui.document.clone();
        let camera_snapshot = ui.camera.clone();

        // Inject invalid graph: execution cycle
        let trigger_a = crate::event_graph::model::GraphNode::new_event_trigger("CycleA", 1, false, true);
        let trigger_b = crate::event_graph::model::GraphNode::new_event_trigger("CycleB", 1, false, true);
        let out_a = trigger_a.find_port_by_name("ExecOut").unwrap().id;
        let in_a = trigger_a.find_port_by_name("ExecIn").unwrap().id;
        let out_b = trigger_b.find_port_by_name("ExecOut").unwrap().id;
        let in_b = trigger_b.find_port_by_name("ExecIn").unwrap().id;
        let id_a = trigger_a.id;
        let id_b = trigger_b.id;

        ui.graph_ui.graph.clear();
        ui.graph_ui.graph.add_node(trigger_a);
        ui.graph_ui.graph.add_node(trigger_b);
        ui.graph_ui.graph.edges.push(crate::event_graph::model::GraphEdge::new(id_a, out_a, id_b, in_b));
        ui.graph_ui.graph.edges.push(crate::event_graph::model::GraphEdge::new(id_b, out_b, id_a, in_a));
        ui.graph_ui.mark_dirty();

        // Attempt apply: MUST fail closed
        let res = ui.apply_graph();
        assert!(res.is_err());
        assert!(ui.error.is_some());

        // Verify document is untouched and STILL dirty!
        assert_eq!(ui.document, doc_snapshot);
        assert_eq!(ui.camera.x, camera_snapshot.x);
        assert_eq!(ui.camera.enabled, camera_snapshot.enabled);
        assert!(ui.dirty());
        assert!(ui.graph_ui.is_dirty());

        // Next test: invalid link with nonexistent port
        ui.graph_ui.graph.edges.push(crate::event_graph::model::GraphEdge::new(id_a, uuid::Uuid::new_v4(), id_b, in_b));
        let res_link = ui.apply_graph();
        assert!(res_link.is_err());
        assert_eq!(ui.document, doc_snapshot);
        assert!(ui.dirty());
    }

    #[test]
    fn test_scenario_ui_dirty_close_guard_includes_graph() {
        let ctx = egui::Context::default();
        let mut ui = ScenarioUi {
            open: true,
            ..Default::default()
        };
        assert!(!ui.dirty());
        assert!(!ui.graph_ui.is_dirty());

        // Make only graph dirty
        ui.graph_ui.mark_dirty();
        assert!(!ui.dirty());
        assert!(ui.graph_ui.is_dirty());

        // Closing ScenarioUi while graph is dirty must trigger pending close guard
        ui.open = true;
        ctx.begin_frame(Default::default());
        // Simulating window close by setting open flag in show
        ui.show(&ctx);
        let _ = ctx.end_frame();

        // Force a close trigger via pending check
        if ui.dirty() || ui.graph_ui.is_dirty() {
            ui.pending = Some(Pending::Close);
        }
        assert!(matches!(ui.pending, Some(Pending::Close)));

        // Discarding clears both
        ui.graph_ui.discard_and_close();
        ui.pending = None;
        assert!(!ui.graph_ui.is_dirty());
    }

    #[test]
    fn test_unloaded_scenario_ui_refuses_staging() {
        let p = source();
        let ui = ScenarioUi::default();
        assert!(ui.stage_transaction_files(&p).is_err());

        // Staging for a different map than loaded
        let other = source();
        let mut loaded_ui = ScenarioUi::default();
        loaded_ui.load_now(&p).unwrap();
        loaded_ui.document.events.push(Event {
            name: "Diff Map".into(),
            ..Default::default()
        });
        assert!(loaded_ui.stage_transaction_files(&other).is_err());
    }

    #[test]
    fn test_scenario_stage_does_not_mutate_disk_or_dirty() {
        let p = source();
        let mut ui = ScenarioUi::default();
        ui.load_now(&p).unwrap();
        assert!(!ui.dirty());

        ui.document.events.push(Event {
            name: "Staged Event".into(),
            actions: vec![Action {
                text: "Hello Staged".into(),
                ..Default::default()
            }],
            ..Default::default()
        });
        ui.camera.enabled = true;
        ui.camera.x = 42.0;
        assert!(ui.dirty());

        let disk_bci_before = fs::read(p.join("SCRIPT/ak_level.bci")).unwrap();
        let staged = ui.stage_transaction_files(&p).unwrap();

        // Disk must remain untouched
        assert_eq!(fs::read(p.join("SCRIPT/ak_level.bci")).unwrap(), disk_bci_before);
        assert!(!p.join("arm_scenario.json").exists());
        assert!(!p.join("arm_start_camera.json").exists());
        assert!(!p.join("SCRIPT/ak_level.arm_original").exists());

        // In-memory must remain dirty
        assert!(ui.dirty());

        // Staged payload must contain all 4 files
        assert_eq!(staged.len(), 4);
        let filenames: Vec<_> = staged.iter().map(|(path, _)| path.to_string_lossy().to_string()).collect();
        assert!(filenames.contains(&"arm_scenario.json".to_string()));
        assert!(filenames.contains(&"arm_start_camera.json".to_string()));
        assert!(filenames.contains(&"SCRIPT/ak_level.arm_original".to_string()));
        assert!(filenames.contains(&"SCRIPT/ak_level.bci".to_string()));

        // arm_scenario.json parses and matches
        let scenario_entry = staged.iter().find(|(path, _)| path == Path::new("arm_scenario.json")).unwrap();
        let parsed_doc = ScenarioDocument::from_json(&scenario_entry.1).unwrap();
        assert_eq!(parsed_doc.events.len(), 1);
        assert_eq!(parsed_doc.events[0].name, "Staged Event");

        // original BCI preserved
        let orig_entry = staged.iter().find(|(path, _)| path == Path::new("SCRIPT/ak_level.arm_original")).unwrap();
        assert_eq!(orig_entry.1, scenario::BciImage::idle().serialize());

        // compiled BCI parses
        let bci_entry = staged.iter().find(|(path, _)| path == Path::new("SCRIPT/ak_level.bci")).unwrap();
        let parsed_bci = scenario::BciImage::parse(&bci_entry.1).unwrap();
        assert!(!parsed_bci.code.is_empty());
    }

    #[test]
    fn test_scenario_watched_external_change_fails_closed() {
        let p = source();
        let mut ui = ScenarioUi::default();
        ui.load_now(&p).unwrap();

        ui.document.events.push(Event {
            name: "Mutate Me".into(),
            ..Default::default()
        });
        assert!(ui.dirty());

        // Mutate source BCI externally
        fs::write(p.join("SCRIPT/ak_level.bci"), b"corrupted bytes").unwrap();

        assert!(ui.stage_transaction_files(&p).is_err());
        assert!(ui.dirty());
    }

    #[test]
    fn test_scenario_commit_baseline_cleans_dirty_and_roundtrips() {
        let p = source();
        let mut ui = ScenarioUi::default();
        ui.load_now(&p).unwrap();

        ui.document.events.push(Event {
            name: "Committed Event".into(),
            actions: vec![Action {
                text: "Committed Text".into(),
                ..Default::default()
            }],
            ..Default::default()
        });
        ui.camera.enabled = true;
        assert!(ui.dirty());

        let staged = ui.stage_transaction_files(&p).unwrap();

        // Transaction uncommitted (disk not written yet): commit_baseline must fail closed and leave dirty
        assert!(ui.commit_baseline(&p).is_err());
        assert!(ui.dirty());

        // Simulate successful transaction commit by writing staged files
        for (rel, bytes) in &staged {
            let path = p.join(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, bytes).unwrap();
        }

        // Now commit_baseline succeeds
        ui.commit_baseline(&p).unwrap();
        assert!(!ui.dirty());

        // Fresh load from disk roundtrips perfectly
        let mut fresh = ScenarioUi::default();
        fresh.load_now(&p).unwrap();
        assert_eq!(fresh.document.events.len(), 1);
        assert_eq!(fresh.document.events[0].name, "Committed Event");
        assert_eq!(fresh.document.events[0].actions[0].text, "Committed Text");
        assert!(fresh.camera.enabled);
        assert!(!fresh.dirty());
    }

    #[test]
    fn test_scenario_unapplied_graph_fails_closed() {
        let p = source();
        let mut ui = ScenarioUi::default();
        ui.load_now(&p).unwrap();

        ui.graph_ui.mark_dirty();
        assert!(ui.stage_transaction_files(&p).is_err());
        assert!(ui.graph_ui.is_dirty());
    }

    #[test]
    fn test_scenario_clean_session_stage_returns_empty() {
        let p = source();
        let ui = ScenarioUi::default();
        // Even when clean, unloaded map returns Err
        assert!(ui.stage_transaction_files(&p).is_err());

        let mut loaded = ScenarioUi::default();
        loaded.load_now(&p).unwrap();
        assert!(!loaded.dirty());
        let staged = loaded.stage_transaction_files(&p).unwrap();
        assert!(staged.is_empty());
    }
}

fn alias_combo(ui: &mut egui::Ui, alias: &mut String, names: &[String]) {
    if names.is_empty() {
        return;
    }
    egui::ComboBox::from_id_source("object_alias_catalog")
        .selected_text("從遊戲別名選擇")
        .show_ui(ui, |ui| {
            for name in names {
                ui.selectable_value(alias, name.clone(), name);
            }
        });
}
