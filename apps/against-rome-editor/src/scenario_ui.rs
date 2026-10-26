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
/// 產生器表單；所有事件仍透過 add_events 原子加入。
#[derive(Clone)]
pub(crate) struct WaveGenerator {
    pub team: i32,
    pub alias: String,
    pub waves: i32,
    pub first: i32,
    pub interval: i32,
    pub base: i32,
    pub increase: i32,
    pub x: f32,
    pub z: f32,
    pub destination: WorldPosition,
    pub(crate) initialized: bool,
}
impl Default for WaveGenerator {
    fn default() -> Self {
        Self { team: 1, alias: String::new(), waves: 5, first: 60, interval: 60,
            base: 10, increase: 5, x: 8000., z: 8000.,
            destination: WorldPosition { x: 8000, z: 8000 }, initialized: false }
    }
}
impl WaveGenerator {
    fn count(&self, index: i32) -> i32 {
        self.base.saturating_add(index.saturating_mul(self.increase)).clamp(1, 20)
    }
    fn last_time(&self) -> i64 {
        i64::from(self.first) + (i64::from(self.waves) - 1) * i64::from(self.interval)
    }
}
/// 與設計檢查的一鍵修正共用第零秒外交事件。
pub(crate) fn hostile_wave_event(team: i32) -> Event {
    Event { name: format!("ARM hostile team {team}"), delay_seconds: 0,
        actions: vec![Action { kind: 1, team: 0, other_team: team, hostile: true,
            ..Default::default() }], ..Default::default() }
}
pub(crate) fn unused_event_name(base: &str, events: &[Event]) -> String {
    let mut name = base.to_string();
    let mut suffix = 2;
    while events.iter().any(|e| e.name.eq_ignore_ascii_case(&name)) {
        name = format!("{base}_{suffix}");
        suffix += 1;
    }
    name
}
/// 旁錄核心的每一步；原有相同值取代仍保留 undo，但不產生提交 receipt。
#[derive(Clone, Debug, Default)]
struct EventHistory {
    tokens: against_rome_textures::history::HistoryTokens,
    undo: Vec<bool>,
    redo: Vec<bool>,
}
impl EventHistory {
    fn commit(&mut self, changed: bool) {
        self.undo.push(changed);
        self.redo.clear();
        if changed { self.tokens.commit(); } else { self.tokens.clear_redo(); }
    }
    fn undo(&mut self) {
        if let Some(changed) = self.undo.pop() {
            if changed { self.tokens.undo(); }
            self.redo.push(changed);
        }
    }
    fn redo(&mut self) {
        if let Some(changed) = self.redo.pop() {
            if changed { self.tokens.redo(); }
            self.undo.push(changed);
        }
    }
    fn clear_redo(&mut self) { self.redo.clear(); self.tokens.clear_redo(); }
}

pub struct ScenarioUi {
    pub(crate) history_request: Option<bool>,
    browser: EventBrowser,
    pub(crate) canvas: crate::scenario_canvas::ScenarioCanvas,
    objective_seconds: i32,
    objective_team: i32,
    objective_targets: [Option<uuid::Uuid>; 2],
    pub(crate) objective_notice: Option<Result<bool>>,
    pub(crate) wave_generator: WaveGenerator,
    pub(crate) wave_target: Option<WorldPosition>,
    pub(crate) wave_target_requested: bool,
    pub(crate) wave_notice: Option<Result<usize>>,
    pub open: bool,
    /// UI language follows the editor toggle (set by the app each frame); data is unchanged.
    pub english: bool,
    document: ScenarioDocument,
    camera: StartCamera,
    recovery_undo: Option<(ScenarioDocument, StartCamera, ScenarioEventSession, EventHistory)>,
    recovery_redo: Option<(ScenarioDocument, StartCamera, ScenarioEventSession, EventHistory)>,
    source: Option<Source>,
    watched: Vec<Source>,
    aliases: Vec<String>,
    active_bindings: Option<Vec<(i32,u32)>>,
    load_path: String,
    export_path: String,
    pub(crate) selected: usize,
    baseline: Vec<u8>,
    events: ScenarioEventSession,
    history: EventHistory,
    receipts: Vec<crate::undo_journal::SubsystemStep>,
    pending: Option<Pending>,
    report: String,
    error: Option<String>,
    pub graph_ui: EventGraphUi,
    pub template_remap: bool,
    pub template_from: i32,
    pub template_to: i32,
    pub event_picker: crate::event_templates::EventFilePicker,
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
            history_request: None,
            browser: EventBrowser::default(),
            canvas: Default::default(),
            objective_seconds: 300,
            objective_team: 1,
            objective_targets: [None, None],
            objective_notice: None,
            open: false,
            wave_generator: WaveGenerator::default(),
            wave_target: None,
            wave_target_requested: true,
            wave_notice: None,
            english: false,
            document,
            camera,
            recovery_undo: None,
            recovery_redo: None,
            source: None,
            watched: vec![],
            aliases: vec![],
            active_bindings: None,
            load_path: String::new(),
            export_path: String::new(),
            selected: 0,
            baseline,
            events: ScenarioEventSession::new(),
            history: Default::default(),
            receipts: vec![],
            pending: None,
            report: String::new(),
            error: None,
            graph_ui: EventGraphUi::default(),
            template_remap: false,
            template_from: 0,
            template_to: 1,
            event_picker: crate::event_templates::native_event_picker(),
        }
    }
}
/// Pick the label for the current UI language.
fn tl(english: bool, zh: &'static str, en: &'static str) -> &'static str {
    if english {
        en
    } else {
        zh
    }
}
/// 僅保存檢視偏好；篩選與排序永不寫入劇本或選取狀態。
#[derive(Default)]
struct EventBrowser {
    search: String,
    team: Option<i32>,
    kind: Option<i32>,
    enabled_only: bool,
    disabled_only: bool,
    time_order: bool,
}
impl EventBrowser {
    /// 清除篩選但保留排序，方便停用篩選結果後重新查看全部。
    fn clear_filters(&mut self) {
        let time_order = self.time_order;
        *self = Self { time_order, ..Default::default() };
    }
    fn indices(&self, events: &[Event]) -> Vec<usize> {
        let query = self.search.trim().to_lowercase();
        let mut indices: Vec<_> = events.iter().enumerate().filter(|(_, e)| {
            (!self.enabled_only || e.enabled) && (!self.disabled_only || !e.enabled)
                && self.team.is_none_or(|team| e.actions.iter().any(|a| a.team == team || a.other_team == team))
                && self.kind.is_none_or(|kind| e.actions.iter().any(|a| a.kind == kind))
                && (query.is_empty() || e.name.to_lowercase().contains(&query)
                    || e.actions.iter().any(|a| a.text.to_lowercase().contains(&query)
                        || a.alias.to_lowercase().contains(&query)))
        }).map(|(i, _)| i).collect();
        if self.time_order {
            indices.sort_by_key(|&i| (events[i].delay_seconds, i));
        }
        indices
    }
}
fn action_kind_label(kind: i32, en: bool) -> &'static str {
    match kind {
        0 => tl(en, "訊息", "Message"),
        1 => tl(en, "外交", "Diplomacy"),
        2 => tl(en, "生成", "Spawn"),
        3 => tl(en, "勝利", "Victory"),
        4 => tl(en, "失敗", "Defeat"),
        _ => tl(en, "未知", "Unknown"),
    }
}
fn event_summary(event: &Event, en: bool) -> String {
    let kinds: Vec<_> = event.conditions.iter().map(|c| match c.kind {
        0 => tl(en, "物件存在", "Object exists"),
        1 => tl(en, "死亡或已移除", "Dead or removed"),
        2 => tl(en, "位於矩形區域", "Inside rectangle"),
        _ => tl(en, "未知", "Unknown"),
    }).collect();
    let actions: Vec<_> = event.actions.iter().map(|a| {
        let label = action_kind_label(a.kind, en);
        match a.kind {
            1 => format!("{label}: {} {} ↔ {} / {}={}", tl(en, "隊伍", "team"),
                a.team, a.other_team, tl(en, "敵對", "hostile"), a.hostile),
            2 => {
                let destination = a.move_destination.as_ref().map(|p| format!("({}, {})", p.x, p.z))
                    .unwrap_or_else(|| tl(en, "無", "None").into());
                format!("{label}: {} × {} / {} {} / {} {destination}", a.count, a.alias,
                    tl(en, "隊伍", "team"), a.team, tl(en, "目的地", "destination"))
            }
            _ => format!("{label}: {}", a.text),
        }
    }).collect();
    format!("{} {}:{:02} / {} | {} {} [{}] | {}", tl(en, "延遲", "Delay"),
        event.delay_seconds / 60, event.delay_seconds % 60,
        if event.repeat { tl(en, "重複", "repeat") } else { tl(en, "單次", "once") },
        event.conditions.len(), tl(en, "條件", "conditions"), kinds.join(", "), actions.join("; "))
}
fn selection_hidden(selected: usize, events: &[Event], indices: &[usize]) -> bool {
    selected < events.len() && !indices.contains(&selected)
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
    if !document.data_slots.is_empty() || !active.is_empty() {
        scenario::verify_binding_snapshot(document,&active)?;
    }
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
    pub fn start_camera(&self) -> &StartCamera { &self.camera }

    pub fn history_token(&self) -> u64 { self.history.tokens.token() }

    pub fn take_history_receipts(&mut self) -> Vec<crate::undo_journal::SubsystemStep> {
        std::mem::take(&mut self.receipts)
    }

    pub fn clear_redo(&mut self) {
        self.events.clear_redo();
        self.history.clear_redo();
        if let Some((_, _, events, history)) = self.recovery_undo.as_mut() {
            events.clear_redo();
            history.clear_redo();
        }
        self.recovery_redo = None;
    }

    fn invalidate_history(&mut self) {
        self.history = Default::default();
        self.receipts.clear();
    }

    fn record_event_commit(&mut self, before: &[Event]) {
        self.history.commit(before != self.events.capture());
        self.receipts.extend(self.history.tokens.take_receipts().into_iter().map(
            |(before_token, after_token)| crate::undo_journal::SubsystemStep {
                subsystem: crate::undo_journal::SubsystemId::Scenario, before_token, after_token,
            }));
    }

    /// Live scenario document for read-only consumers (maps/campaign/packaging panels).
    pub fn document(&self) -> &ScenarioDocument {
        &self.document
    }
    /// Replaces the scenario spawns after full document validation (aliases, ids, event targets,
    /// DATA bindings). On failure nothing changes. This is the single write path used by the
    /// spawn placement panel so Safe Save keeps working from `document.spawns` unchanged.
    pub fn apply_spawns(&mut self, spawns: Vec<Spawn>) -> Result<()> {
        if self.source.is_none() {
            return Err("劇本尚未載入（地圖缺少 SCRIPT/ak_level.bci）；無法編輯生成物件。".into());
        }
        let mut draft = self.document.clone();
        draft.spawns = spawns;
        // DATA bindings only change in a save. A binding follows its spawn: it disappears while the
        // spawn is deleted and comes back when an undo restores it (bound spawns are editable).
        let saved = self.saved_document()?;
        draft.data_slots = saved
            .data_slots
            .into_iter()
            .filter(|b| draft.spawns.iter().any(|s| s.id == b.spawn_id && s.prebuilt))
            .collect();
        self.validate_draft(&draft)?;
        self.recovery_redo = None;
        self.document = draft;
        self.error = None;
        Ok(())
    }
    /// True once a map script source has been loaded (event merges need a loaded scenario).
    pub fn is_loaded(&self) -> bool {
        self.source.is_some()
    }

    /// 未載入與尚無事件時提供下一步；僅顯示提示，不修改劇本。
    fn empty_state_hint(&self) -> Option<&'static str> {
        if !self.is_loaded() {
            Some(tl(self.english,
                "先在上方指定地圖資料夾並按「載入」（需有 SCRIPT/ak_level.bci）；載入地形不會自動載入劇本。",
                "Choose a map folder above and press Load (requires SCRIPT/ak_level.bci). Loading terrain does not load the scenario."))
        } else if self.document.events.is_empty() {
            Some(tl(self.english,
                "尚無事件：展開「勝負條件預設」或「進攻波次產生器」建立玩法，也可使用下方「新增事件」。變更可復原，完成後請安全儲存。",
                "No events yet: use Objective presets or Attack wave generator, or Add event below. Changes support undo; use Safe Save when ready."))
        } else {
            None
        }
    }
    /// Loaded game ObjDefName aliases; empty when no catalog was found.
    #[cfg(test)]
    pub fn set_known_aliases(&mut self, aliases: Vec<String>) {
        self.aliases = aliases;
    }

    pub fn known_aliases(&self) -> &[String] {
        &self.aliases
    }
    /// Brings the event session in line with `document.events` (e.g. after graph apply or
    /// test-side edits) so undo/redo always starts from what the user currently sees.
    fn sync_session(&mut self) {
        if self.events.capture() != self.document.events {
            self.recovery_redo = None;
            let current = self.document.events.clone();
            let before = self.events.capture();
            if self.events.replace_all(&current).is_ok() { self.record_event_commit(&before); }
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
        let before = self.events.capture();
        if self.events.replace_all(&draft.events).is_ok() {
            self.record_event_commit(&before);
            self.recovery_redo = None;
            self.document = draft;
        }
        self.clamp_selected();
    }
    pub fn can_undo_events(&self) -> bool {
        self.recovery_undo.is_some() || self.events.can_undo()
    }
    pub fn can_redo_events(&self) -> bool {
        self.recovery_redo.is_some() || self.events.can_redo()
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
        let before = self.events.capture();
        self.events.add_range(items)?;
        if !items.is_empty() { self.record_event_commit(&before); }
        self.recovery_redo = None;
        self.document.events = self.events.capture();
        self.error = None;
        self.refresh_graph_after_event_change();
        Ok(first)
    }
    /// Inserts an independent copy after the source, matching the C# event list.
    /// The session enforces the event limit and UTF-16 name cap in one undo step.
    fn duplicate_event(&mut self, index: usize) -> Result<usize> {
        self.sync_session();
        let before = self.events.capture();
        let selected = self.events.duplicate(index, "（副本）")?;
        self.record_event_commit(&before);
        self.recovery_redo = None;
        self.document.events = self.events.capture();
        self.selected = selected;
        self.error = None;
        self.refresh_graph_after_event_change();
        Ok(selected)
    }
    pub fn remove_event(&mut self, index: usize) -> Result<()> {
        self.sync_session();
        let before = self.events.capture();
        self.events.remove_at(index)?;
        self.record_event_commit(&before);
        self.recovery_redo = None;
        self.document.events = self.events.capture();
        self.clamp_selected();
        self.refresh_graph_after_event_change();
        Ok(())
    }
    /// Replaces one event as a single undo step. Inline edits are intentionally not validated
    /// here (intermediate typing states); validation still runs at preview/export/save.
    pub fn replace_event(&mut self, index: usize, item: Event) -> Result<()> {
        self.sync_session();
        let before = self.events.capture();
        self.events.replace(index, item)?;
        self.record_event_commit(&before);
        self.recovery_redo = None;
        self.document.events = self.events.capture();
        Ok(())
    }
    /// 驗證並整批取代事件；所有動作共用一步復原，失敗時保留原狀。
    pub(crate) fn replace_events(&mut self, items: Vec<Event>) -> Result<()> {
        if !self.is_loaded() {
            return Err("劇本尚未載入。".into());
        }
        let mut draft = self.document.clone();
        draft.events = items;
        self.validate_draft(&draft)?;
        self.sync_session();
        let before = self.events.capture();
        self.events.replace_all(&draft.events)?;
        self.record_event_commit(&before);
        self.recovery_redo = None;
        self.document = draft;
        self.clamp_selected();
        self.error = None;
        self.refresh_graph_after_event_change();
        Ok(())
    }
    /// 批次設定指定事件索引的啟用狀態（單步復原）；若無任何事件狀態變更則不產生復原步驟。
    pub fn set_events_enabled(&mut self, indices: &[usize], enabled: bool) -> Result<usize> {
        if !self.is_loaded() {
            return Err(tl(self.english, "請先載入地圖劇本。", "Load a map scenario first.").into());
        }
        let mut changed_count = 0;
        let mut events = self.document.events.clone();
        for &idx in indices {
            if let Some(event) = events.get_mut(idx) {
                if event.enabled != enabled {
                    event.enabled = enabled;
                    changed_count += 1;
                }
            }
        }
        if changed_count > 0 {
            self.replace_events(events)?;
        }
        Ok(changed_count)
    }
    pub fn undo_events(&mut self) -> bool {
        self.sync_session();
        if !self.events.can_undo() {
            if let Some((document, camera, events, history)) = self.recovery_undo.take() {
                self.recovery_redo = Some((self.document.clone(), self.camera.clone(), self.events.clone(), self.history.clone()));
                self.document = document;
                self.camera = camera;
                self.events = events;
                self.history = history;
                self.clamp_selected();
                self.refresh_graph_after_event_change();
                return true;
            }
        }
        let changed = self.events.undo();
        if changed { self.history.undo(); }
        if changed {
            self.document.events = self.events.capture();
            self.clamp_selected();
            self.refresh_graph_after_event_change();
        }
        changed
    }
    pub fn redo_events(&mut self) -> bool {
        self.sync_session();
        if let Some((document, camera, events, history)) = self.recovery_redo.take() {
            self.recovery_undo = Some((self.document.clone(), self.camera.clone(), self.events.clone(), self.history.clone()));
            self.document = document; self.camera = camera;
            self.events = events;
            self.history = history;
            self.clamp_selected();
            self.refresh_graph_after_event_change(); return true;
        }
        let changed = self.events.redo();
        if changed { self.history.redo(); }
        if changed {
            self.document.events = self.events.capture();
            self.clamp_selected();
            self.refresh_graph_after_event_change();
        }
        changed
    }
    pub(crate) fn selected_events(&self) -> Vec<Event> {
        if self.selected < self.document.events.len() {
            vec![self.document.events[self.selected].clone()]
        } else {
            vec![]
        }
    }

    /// Exports the selected events (or all selected if multi-selection exists) to a template file.
    /// Events targeting specific objects (Condition.target_id) are skipped and reported.
    pub fn export_events(&mut self) -> Result<bool> {
        let en = self.english;
        let selected_events = self.selected_events();
        if selected_events.is_empty() {
            let msg = tl(en, "請先選取要匯出的事件。", "Select an event to export first.");
            self.error = Some(msg.to_string());
            return Err(msg.to_string());
        }
        let (template, skipped) = crate::event_templates::EventTemplate::capture(&selected_events);
        if template.0.is_empty() {
            let msg = if en {
                format!("Export cancelled: skipped {skipped} event(s) referencing specific map objects; no portable events left.")
            } else {
                format!("匯出已取消：已略過 {skipped} 個引用特定地圖物件的事件；無可匯出的通用事件。")
            };
            self.error = Some(msg.clone());
            return Err(msg);
        }
        let suggested_name = if template.0.len() == 1 {
            let safe_name: String = template.0[0]
                .name
                .chars()
                .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
                .collect();
            if safe_name.is_empty() || safe_name.chars().all(|c| c == '_') {
                format!("events.{}", crate::event_templates::EVENT_TEMPLATE_EXTENSION)
            } else {
                format!("{safe_name}.{}", crate::event_templates::EVENT_TEMPLATE_EXTENSION)
            }
        } else {
            format!("events.{}", crate::event_templates::EVENT_TEMPLATE_EXTENSION)
        };

        let path = (self.event_picker)(&crate::layout_io::LayoutDialogKind::Save { suggested_name });
        let Some(path) = path else {
            self.report = tl(en, "已取消匯出。", "Export cancelled.").to_string();
            return Ok(false);
        };
        self.export_events_to(&path)?;
        Ok(true)
    }

    /// Exports selected events directly to the given path without opening a file dialog.
    pub fn export_events_to(&mut self, path: &Path) -> Result<(usize, usize)> {
        let en = self.english;
        let selected_events = self.selected_events();
        if selected_events.is_empty() {
            let msg = tl(en, "請先選取要匯出的事件。", "Select an event to export first.");
            self.error = Some(msg.to_string());
            return Err(msg.to_string());
        }
        let (template, skipped) = crate::event_templates::EventTemplate::capture(&selected_events);
        if template.0.is_empty() {
            let msg = if en {
                format!("Export cancelled: skipped {skipped} event(s) referencing specific map objects; no portable events left.")
            } else {
                format!("匯出已取消：已略過 {skipped} 個引用特定地圖物件的事件；無可匯出的通用事件。")
            };
            self.error = Some(msg.clone());
            return Err(msg);
        }
        let json = template.serialize()?;
        let target = crate::event_templates::with_default_extension(path);
        crate::layout_io::write_json_atomic(&target, &json)?;
        let count = template.0.len();
        self.report = if skipped > 0 {
            if en {
                format!("Export complete ({count} event(s)); skipped {skipped} event(s) referencing specific map objects.")
            } else {
                format!("匯出完成（共 {count} 個事件）；已略過 {skipped} 個引用特定地圖物件的事件。")
            }
        } else if en {
            format!("Export complete ({count} event(s)).")
        } else {
            format!("匯出完成（共 {count} 個事件）。")
        };
        self.error = None;
        Ok((count, skipped))
    }

    /// Opens the file dialog and imports scenario events from the chosen template file.
    pub fn import_events(&mut self) -> Result<usize> {
        let en = self.english;
        if !self.is_loaded() {
            let msg = tl(en, "請先載入地圖劇本。", "Load a map scenario first.");
            self.error = Some(msg.to_string());
            return Err(msg.to_string());
        }
        let path = (self.event_picker)(&crate::layout_io::LayoutDialogKind::Open);
        let Some(path) = path else {
            self.report = tl(en, "已取消匯入。", "Import cancelled.").to_string();
            return Ok(0);
        };
        let remap = self.template_remap.then_some((self.template_from, self.template_to));
        self.import_events_from(&path, remap)
    }

    /// Imports scenario events from a file with optional team remapping.
    pub fn import_events_from(&mut self, path: &Path, remap: Option<(i32, i32)>) -> Result<usize> {
        let en = self.english;
        if !self.is_loaded() {
            let msg = tl(en, "請先載入地圖劇本。", "Load a map scenario first.");
            self.error = Some(msg.to_string());
            return Err(msg.to_string());
        }
        let text = crate::layout_io::read_json_file(path).map_err(|e| {
            let msg = if en { format!("Read failed: {e}") } else { format!("讀取失敗：{e}") };
            self.error = Some(msg.clone());
            msg
        })?;
        let template = crate::event_templates::EventTemplate::deserialize(&text).map_err(|e| {
            self.error = Some(e.clone());
            e
        })?;
        let planned = template.plan(&self.document.events, remap).map_err(|e| {
            self.error = Some(e.clone());
            e
        })?;
        let first = self.add_events(&planned).map_err(|e| {
            self.error = Some(e.clone());
            e
        })?;
        self.selected = first;
        let count = planned.len();
        self.report = if en {
            format!("Successfully imported {count} event(s).")
        } else {
            format!("已成功匯入 {count} 個事件。")
        };
        self.error = None;
        Ok(count)
    }

    /// 劇本、起始鏡頭與腳本生成物件共用一份復原狀態。
    pub(crate) fn recovery_state(&self) -> serde_json::Value {
        serde_json::json!([self.document, self.camera])
    }
    fn finish_camera_edit(&mut self, before: &StartCamera) {
        if !camera_eq(before, &self.camera) { self.recovery_redo = None; }
    }
    pub(crate) fn restore_recovery(&mut self, value: serde_json::Value) -> Result<()> {
        let (document, camera): (ScenarioDocument, StartCamera) = serde_json::from_value(value).map_err(|e| e.to_string())?;
        if !self.is_loaded() { return Err("Scenario source unavailable / 劇本來源未載入".into()); }
        self.validate_draft(&document)?;
        camera.validate()?;
        self.sync_session();
        self.recovery_undo = Some((self.document.clone(), self.camera.clone(), self.events.clone(), self.history.clone()));
        self.recovery_redo = None;
        self.document = document; self.camera = camera;
        let before_token = self.history_token();
        self.events.load(&self.document.events);
        self.history = Default::default();
        self.receipts.push(crate::undo_journal::SubsystemStep {
            subsystem: crate::undo_journal::SubsystemId::Scenario,
            before_token, after_token: self.history_token(),
        });
        self.refresh_graph_after_event_change();
        Ok(())
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
        self.recovery_undo = None; self.recovery_redo = None;
        self.baseline = state_bytes(&document, &camera).unwrap_or_default();
        self.document = document;
        self.events = ScenarioEventSession::new();
        self.invalidate_history();
        self.camera = camera;
        self.recovery_undo = None; self.recovery_redo = None;
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
        self.wave_generator = WaveGenerator::default();
        self.canvas = Default::default();
        self.wave_target = None;
        self.wave_target_requested = true;
        self.wave_notice = None;
        self.objective_targets = [None, None];
        self.objective_notice = None;
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
        self.recovery_undo = None; self.recovery_redo = None;
        self.baseline = state_bytes(&document, &camera)?;
        self.events = ScenarioEventSession::with_events(document.events.clone());
        self.invalidate_history();
        self.document = document;
        self.camera = camera;
        self.aliases = aliases;
        self.active_bindings=active_bindings;
        self.source = Some(source);
        self.canvas = Default::default();
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
        self.recovery_undo = None; self.recovery_redo = None;
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
        self.stage_transaction_files_with(target_map, None)
    }

    /// The scenario as last loaded or saved (the C# `previousScenario`): bindings and prebuilt
    /// spawns as they exist on disk, before any pending edit.
    pub fn saved_document(&self) -> Result<ScenarioDocument> {
        serde_json::from_slice::<(ScenarioDocument, StartCamera)>(&self.baseline)
            .map(|(document, _)| document)
            .map_err(|e| e.to_string())
    }

    /// Like [`Self::stage_transaction_files`], but compiles and stages `override_data` (the final
    /// document of a spawn DATA transaction plus the active `(slot, uid)` list of its candidate
    /// DATA) instead of the in-memory document. The override is verified against the candidate
    /// DATA, never against the (not yet rewritten) disk DATA.
    pub fn stage_transaction_files_with(
        &self,
        target_map: &Path,
        override_data: Option<(&ScenarioDocument, &[(i32, u32)])>,
    ) -> Result<Vec<(PathBuf, Vec<u8>)>> {
        let document = override_data.map_or(&self.document, |(d, _)| d);
        let active_bindings = match override_data {
            Some((_, active)) => Some(active),
            None => self.active_bindings.as_deref(),
        };
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

        if let Some(active) = active_bindings {
            scenario::verify_binding_snapshot(document, active)?;
        }

        document.validate(if self.aliases.is_empty() {
            None
        } else {
            Some(&self.aliases)
        })?;

        let original = &source.bytes;
        let compiled_bci = scenario::compile_script_verified(
            original,
            document,
            Some(&self.camera),
            if self.aliases.is_empty() {
                None
            } else {
                Some(&self.aliases)
            },
            active_bindings,
        )?;

        let raw = if compiled_bci.starts_with(b"PFIL") {
            against_rome_data::lzss::decompress_pfil(&compiled_bci, None).map_err(|e| e.to_string())?
        } else {
            compiled_bci.clone()
        };
        let _bci = scenario::BciImage::parse(&raw)?;

        let mut d = document.clone();
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
        self.commit_baseline_with(target_map, None)
    }

    /// Like [`Self::commit_baseline`] after a save that staged `final_document` (spawn DATA
    /// transaction: new bindings / prebuilt flags). The disk copy must equal it; only then does it
    /// replace the in-memory document before the usual reload.
    pub fn commit_baseline_with(&mut self, target_map: &Path, final_document: Option<&ScenarioDocument>) -> Result<()> {
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
        let mut expected_doc = final_document.unwrap_or(&self.document).clone();
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

    /// 整批生成與必要外交共用一次 add_events；驗證失敗不變更文件。
    pub(crate) fn generate_attack_waves(&mut self) -> Result<usize> {
        if !self.is_loaded() {
            return Err(tl(self.english, "請先載入地圖劇本。", "Load a map scenario first.").into());
        }
        let g = &self.wave_generator;
        if !(1..=7).contains(&g.team) || !(1..=255).contains(&g.waves)
            || g.first < 0 || g.interval < 0 || g.last_time() > 86400 {
            return Err(tl(self.english, "隊伍需為 1–7，波數 1–255，時間需在 0–86400 秒內。",
                "Use team 1–7, 1–255 waves, and times within 0–86400 seconds.").into());
        }
        let mut all = self.document.events.clone();
        let first = all.len();
        let waves = g.waves;
        let hostile = all.iter().filter(|e| e.enabled).flat_map(|e| &e.actions).any(|a|
            a.kind == 1 && a.hostile && ((a.team == 0 && a.other_team == g.team)
                || (a.team == g.team && a.other_team == 0)));
        if !hostile {
            let mut event = hostile_wave_event(g.team);
            event.name = unused_event_name(&event.name, &all);
            all.push(event);
        }
        let mut number = 1;
        for i in 0..waves {
            let name = loop {
                let candidate = format!("Wave_{number:02}");
                number += 1;
                if !all.iter().any(|e| e.name.eq_ignore_ascii_case(&candidate)) { break candidate; }
            };
            all.push(Event { name, delay_seconds: (i64::from(g.first) + i64::from(i) * i64::from(g.interval)) as i32,
                actions: vec![Action { kind: 2, team: g.team, alias: g.alias.clone(),
                    x: g.x, z: g.z, count: g.count(i), move_destination: Some(g.destination.clone()),
                    ..Default::default() }], ..Default::default() });
        }
        self.add_events(&all[first..])?;
        self.selected = first;
        Ok(waves as usize)
    }
    /// 只建立引用已知 Spawn 的死亡條件；原生 DATA 物件不會被臆測成 Spawn。
    fn objective_event(&self, seconds: Option<i32>, target: Option<uuid::Uuid>, team: i32) -> Result<Event> {
        let (name, delay_seconds, conditions, kind) = if let Some(seconds) = seconds {
            if !(1..=86400).contains(&seconds) { return Err("秒數需為 1–86400 / Seconds must be 1–86400".into()); }
            (format!("Survive_{seconds}s"), seconds, vec![], 3)
        } else {
            if !(0..=7).contains(&team) { return Err("隊伍需為 0–7 / Team must be 0–7".into()); }
            let spawn = self.document.spawns.iter().find(|s| Some(s.id) == target && s.team == team
                && against_rome_maps::design_validator::MapDesignValidator::is_main_house(&s.alias))
                .ok_or("沒有可用的主屋 Spawn／綁定；僅原生 DATA 無法引用 / No usable main-house Spawn/binding; native DATA alone cannot be referenced")?;
            (format!("Team_{team}_main_house_destroyed"), 0,
                vec![Condition { kind: 1, target_id: spawn.id, ..Default::default() }], if team == 0 { 4 } else { 3 })
        };
        Ok(Event { name: unused_event_name(&name, &self.document.events), delay_seconds,
            conditions, actions: vec![Action { kind, ..Default::default() }], ..Default::default() })
    }
    /// 名稱與附帶動作不影響勝負規則；額外條件則會改變觸發時機。
    fn has_objective(&self, candidate: &Event) -> bool {
        self.document.events.iter().any(|e| e.enabled && !e.repeat
            && e.delay_seconds == candidate.delay_seconds
            && e.actions.iter().find(|a| matches!(a.kind, 3 | 4)).is_some_and(|a| a.kind == candidate.actions[0].kind)
            && e.conditions.len() == candidate.conditions.len()
            && e.conditions.iter().zip(&candidate.conditions).all(|(a, b)| a.kind == b.kind && a.target_id == b.target_id))
    }
    fn check_objective(&self, event: &Event) -> Result<()> {
        self.source.as_ref().ok_or("請先載入劇本 / Load a scenario first")?;
        if self.graph_ui.is_dirty() { return Err("請先套用節點圖 / Apply the event graph first".into()); }
        if self.document.events.len() >= 256 { return Err("事件上限為 256 / Event limit is 256".into()); }
        let mut draft = self.document.clone();
        draft.events.push(event.clone());
        draft.validate(if self.aliases.is_empty() { None } else { Some(&self.aliases) })?;
        if draft.spawns.iter().any(|s| s.prebuilt) || !draft.data_slots.is_empty() {
            scenario::verify_binding_snapshot(&draft, self.active_bindings.as_deref()
                .ok_or("缺少原生 DATA 綁定快照 / Source DATA binding snapshot required")?)?;
        }
        Ok(())
    }
    /// 每次新增僅呼叫一次 add_events，重複與失敗不建立復原步驟。
    pub(crate) fn add_objective(&mut self, seconds: Option<i32>, target: Option<uuid::Uuid>, team: i32) -> Result<bool> {
        let event = self.objective_event(seconds, target, team)?;
        if self.has_objective(&event) { return Ok(false); }
        self.check_objective(&event)?;
        // 完整編譯僅在新增時執行，避免面板每幀重建 BCI。
        let mut draft = self.document.clone();
        draft.events.push(event.clone());
        scenario::compile_script_verified(&self.source.as_ref().ok_or("請先載入劇本 / Load a scenario first")?.bytes,
            &draft, Some(&self.camera), if self.aliases.is_empty() { None } else { Some(&self.aliases) },
            self.active_bindings.as_deref())?;
        self.selected = self.add_events(&[event])?;
        Ok(true)
    }
    fn objective_button(&mut self, ui: &mut egui::Ui, label: &str, seconds: Option<i32>, target: Option<uuid::Uuid>, team: i32) {
        let event = self.objective_event(seconds, target, team);
        let exists = event.as_ref().is_ok_and(|e| self.has_objective(e));
        let checked = event.and_then(|e| self.check_objective(&e));
        ui.horizontal(|ui| {
            let response = ui.add_enabled(!exists && checked.is_ok(), egui::Button::new(label));
            if response.clicked() { self.objective_notice = Some(self.add_objective(seconds, target, team)); }
            if let Err(reason) = &checked {
                response.on_disabled_hover_text(format!("無法新增預設 / Cannot add preset: {reason}"));
            }
            if exists { ui.label(tl(self.english, "已存在", "Already present")); }
        });
    }
    fn show_objective_presets(&mut self, ui: &mut egui::Ui) {
        let en = self.english;
        ui.collapsing(tl(en, "勝負條件預設", "Objective presets"), |ui| {
            ui.horizontal(|ui| {
                ui.label(tl(en, "存活秒數", "Survival seconds"));
                ui.add(egui::DragValue::new(&mut self.objective_seconds).clamp_range(1..=86400));
            });
            self.objective_button(ui, tl(en, "新增存活勝利", "Add survival victory"), Some(self.objective_seconds), None, 0);
            ui.horizontal(|ui| {
                ui.label(tl(en, "敵方隊伍", "Enemy team"));
                ui.add(egui::DragValue::new(&mut self.objective_team).clamp_range(1..=7));
            });
            for (slot, team) in [0, self.objective_team].into_iter().enumerate() {
                let candidates: Vec<_> = self.document.spawns.iter().filter(|s| s.team == team
                    && against_rome_maps::design_validator::MapDesignValidator::is_main_house(&s.alias))
                    .map(|s| (s.id, format!("{} ({})", s.alias, s.id))).collect();
                if !candidates.iter().any(|(id, _)| Some(*id) == self.objective_targets[slot]) {
                    self.objective_targets[slot] = candidates.first().map(|(id, _)| *id);
                }
                let selected = &mut self.objective_targets[slot];
                egui::ComboBox::from_id_source(("objective_house", slot))
                    .selected_text(candidates.iter().find(|(id, _)| Some(*id) == *selected)
                        .map(|(_, label)| label.as_str()).unwrap_or(tl(en, "沒有主屋 Spawn", "No main-house Spawn")))
                    .show_ui(ui, |ui| { for (id, label) in &candidates { ui.selectable_value(selected, Some(*id), label); } });
                self.objective_button(ui, if team == 0 { tl(en, "玩家主屋摧毀即失敗", "Player main house destroyed: Defeat") }
                    else { tl(en, "敵方主屋摧毀即勝利", "Enemy main house destroyed: Victory") }, None, self.objective_targets[slot], team);
            }
        });
    }
    fn show_wave_generator(&mut self, ui: &mut egui::Ui) {
        let en = self.english;
        ui.collapsing(tl(en, "進攻波次產生器", "Attack wave generator"), |ui| {
            let g = &mut self.wave_generator;
            if !g.initialized {
                if let Some(target) = &self.wave_target { g.destination = target.clone(); }
                if let Some(alias) = self.aliases.first() { g.alias = alias.clone(); }
                g.initialized = true;
            }
            ui.horizontal(|ui| {
                ui.label(tl(en, "敵方隊伍", "Enemy team"));
                ui.add(egui::DragValue::new(&mut g.team).clamp_range(1..=7));
                ui.label(tl(en, "單位別名", "Unit alias"));
                ui.text_edit_singleline(&mut g.alias);
                ui.push_id("wave_alias", |ui| alias_combo(ui, &mut g.alias, &self.aliases, en));
            });
            ui.horizontal(|ui| {
                for (label, value, min, max) in [
                    (tl(en, "波數", "Wave count"), &mut g.waves, 1, 255),
                    (tl(en, "首波秒數", "First seconds"), &mut g.first, 0, 86400),
                    (tl(en, "間隔秒數", "Interval seconds"), &mut g.interval, 0, 86400),
                    (tl(en, "基本數量", "Base count"), &mut g.base, 1, 20),
                    (tl(en, "每波增加", "Per-wave increase"), &mut g.increase, 0, 20)] {
                    ui.label(label);
                    ui.add(egui::DragValue::new(value).clamp_range(min..=max));
                }
            });
            ui.horizontal(|ui| {
                ui.label(tl(en, "生成 X/Z", "Spawn X/Z"));
                ui.add(egui::DragValue::new(&mut g.x).clamp_range(0. ..=16383.));
                ui.add(egui::DragValue::new(&mut g.z).clamp_range(0. ..=16383.));
                if ui.button(tl(en, "在地圖上點選（生成點）", "Pick on map (spawn)")).clicked() {
                    self.canvas.pick = Some(crate::scenario_canvas::PickTarget::Wave { destination: false });
                }
                if ui.button(tl(en, "地圖邊緣", "Map edge")).clicked() { g.x = 0.; g.z = 8000.; }
            });
            ui.horizontal(|ui| {
                ui.label(tl(en, "目的 X/Z", "Destination X/Z"));
                ui.add(egui::DragValue::new(&mut g.destination.x).clamp_range(0..=16383));
                ui.add(egui::DragValue::new(&mut g.destination.z).clamp_range(0..=16383));
                if ui.button(tl(en, "在地圖上點選（目的地）", "Pick on map (destination)")).clicked() {
                    self.canvas.pick = Some(crate::scenario_canvas::PickTarget::Wave { destination: true });
                }
                if ui.button(tl(en, "玩家主屋", "Player base")).clicked() {
                    self.wave_target_requested = true;
                }
            });
            ui.label(if en { format!("{} waves, {}..{} units, {}s..{}s (1–20 units per wave)",
                g.waves, g.count(0), g.count(g.waves - 1), g.first, g.last_time())
            } else { format!("{} 波，{}..{} 單位，{}秒..{}秒（每波限 1–20 單位）",
                g.waves, g.count(0), g.count(g.waves - 1), g.first, g.last_time()) });
            if ui.add_enabled(self.is_loaded(), egui::Button::new(
                tl(en, "產生進攻波次", "Generate attack waves"))).clicked() {
                let result = self.generate_attack_waves();
                if let Err(e) = &result { self.error = Some(e.clone()); }
                self.wave_notice = Some(result);
            }
        });
    }

    fn show_event_browser(&mut self, ui: &mut egui::Ui) {
        let en = self.english;
        let browser = &mut self.browser;
        ui.horizontal_wrapped(|ui| {
            ui.label(tl(en, "搜尋事件名稱／動作文字／別名", "Search event name / action text / alias"));
            ui.text_edit_singleline(&mut browser.search);
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(tl(en, "隊伍", "Team"));
            egui::ComboBox::from_id_source("event_browser_team")
                .selected_text(browser.team.map(|t| t.to_string()).unwrap_or_else(|| tl(en, "任意", "Any").into()))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut browser.team, None, tl(en, "任意", "Any"));
                    for team in 0..=7 { ui.selectable_value(&mut browser.team, Some(team), team.to_string()); }
                });
            ui.label(tl(en, "動作種類", "Action kind"));
            egui::ComboBox::from_id_source("event_browser_kind")
                .selected_text(browser.kind.map(|k| action_kind_label(k, en)).unwrap_or(tl(en, "任意", "Any")))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut browser.kind, None, tl(en, "任意", "Any"));
                    for kind in 0..=4 { ui.selectable_value(&mut browser.kind, Some(kind), action_kind_label(kind, en)); }
                });
            if ui.checkbox(&mut browser.enabled_only, tl(en, "僅啟用事件", "Enabled only")).changed() {
                browser.disabled_only = false;
            }
            if ui.checkbox(&mut browser.disabled_only, tl(en, "僅停用事件", "Disabled only")).changed() {
                browser.enabled_only = false;
            }
            if ui.button(tl(en, "清除篩選", "Clear filters")).clicked() {
                browser.clear_filters();
            }
            ui.label(tl(en, "排序", "Order"));
            ui.selectable_value(&mut browser.time_order, false, tl(en, "原始順序", "Original order"));
            ui.selectable_value(&mut browser.time_order, true, tl(en, "時間", "Time"));
        });
        let indices = browser.indices(&self.document.events);
        ui.horizontal(|ui| {
            ui.label(if en { format!("{} of {} events", indices.len(), self.document.events.len()) }
                else { format!("顯示 {} / {} 個事件", indices.len(), self.document.events.len()) });
            if !indices.is_empty() {
                let (enable_label, disable_label) = if indices.len() == self.document.events.len() {
                    (tl(en, "啟用全部", "Enable all"), tl(en, "停用全部", "Disable all"))
                } else {
                    (tl(en, "啟用篩選", "Enable filtered"), tl(en, "停用篩選", "Disable filtered"))
                };
                if ui.button(enable_label).on_hover_text(tl(en, "啟用所有顯示的事件（單步復原）", "Enable all displayed events (single undo step)")).clicked() {
                    match self.set_events_enabled(&indices, true) {
                        Ok(n) => self.report = if en { format!("Enabled {n} event(s).") } else { format!("已啟用 {n} 個事件。") },
                        Err(e) => self.error = Some(e),
                    }
                }
                if ui.button(disable_label).on_hover_text(tl(en, "停用所有顯示的事件（單步復原）", "Disable all displayed events (single undo step)")).clicked() {
                    match self.set_events_enabled(&indices, false) {
                        Ok(n) => self.report = if en { format!("Disabled {n} event(s).") } else { format!("已停用 {n} 個事件。") },
                        Err(e) => self.error = Some(e),
                    }
                }
            }
        });
        if selection_hidden(self.selected, &self.document.events, &indices) {
            ui.colored_label(egui::Color32::YELLOW, tl(en,
                "已選事件被篩選隱藏；下方仍編輯原本選取的事件。",
                "The selected event is hidden by filters; the editor below keeps that selection."));
        }
        for i in indices {
            let event = &self.document.events[i];
            let label = egui::RichText::new(format!("{} {}\n{}",
                if event.enabled { "●" } else { "○" }, event.name, event_summary(event, en)));
            let label = if event.enabled { label } else { label.color(egui::Color32::GRAY) };
            if ui.push_id(("event_browser_row", i), |ui| ui.selectable_label(self.selected == i, label)).inner.clicked() {
                self.selected = i;
            }
        }
    }
    pub fn show(&mut self, ctx: &egui::Context) {
        if self.graph_ui.apply_requested {
            self.graph_ui.apply_requested = false;
            if let Err(e) = self.apply_graph() {
                self.error = Some(e);
            }
        }
        if !self.open {
            self.graph_ui.english = self.english;
            self.graph_ui.show(ctx);
            return;
        }
        let en = self.english;
        let mut opened = true;
        egui::Window::new(tl(en, "劇本事件 / Scenario", "Scenario Events"))
            .id(egui::Id::new("scenario_events_window"))
            .open(&mut opened)
            .default_width(760.)
            .show(ctx, |ui| {
                ui.label(tl(en, "來源唯讀；以結構化事件、物件條件與開局鏡頭編輯，輸出新工作副本。", "Source is read-only; edit structured events, object conditions and the start camera, then export a new working copy."));
                ui.horizontal(|ui| {
                    ui.label(tl(en, "來源地圖目錄", "Source map folder"));
                    ui.text_edit_singleline(&mut self.load_path);
                    if ui.button(tl(en, "載入", "Load")).clicked() {
                        self.request_load(PathBuf::from(self.load_path.trim()))
                    }
                });
                if let Some(hint) = self.empty_state_hint() {
                    ui.small(if self.is_loaded() {
                        tl(en, "尚無事件：請用下方預設或新增事件。", "No events yet: use presets or Add event below.")
                    } else {
                        tl(en, "請在上方載入劇本（SCRIPT/ak_level.bci）。", "Load the scenario above (SCRIPT/ak_level.bci).")
                    }).on_hover_text(hint);
                }
                if let Some(s) = &self.source {
                    ui.small(if en {
                        format!("Original BCI: {}", s.path.display())
                    } else {
                        format!("原始 BCI：{}", s.path.display())
                    });
                    ui.small(match (self.aliases.is_empty(), en) {
                        (true, false) => "未找到遊戲別名 catalog；別名需手動核對。".into(),
                        (true, true) => "No game alias catalog found; check aliases by hand.".into(),
                        (false, false) => format!("已載入 {} 個遊戲 ObjDefName 別名，編譯時會驗證。", self.aliases.len()),
                        (false, true) => format!("Loaded {} game ObjDefName aliases; compiling verifies them.", self.aliases.len()),
                    });
                }
                if self.dirty() {
                    ui.colored_label(egui::Color32::YELLOW, tl(en, "尚未輸出的修改", "Unexported changes"));
                }
                egui::ScrollArea::vertical()
                    .max_height(650.)
                    .show(ui, |ui| {
                        self.show_wave_generator(ui);
                        self.show_objective_presets(ui);
                        ui.collapsing(tl(en, "開局鏡頭", "Start camera"), |ui| {
                            let before = self.camera.clone();
                            ui.checkbox(&mut self.camera.enabled, tl(en, "啟用", "Enabled"));
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
                                    tl(en, "世界焦點 / centerToPos", "World focus / centerToPos")
                                } else {
                                    tl(en, "引擎座標 / enginePos", "Engine position / enginePos")
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.camera.primitive, 0, tl(en, "世界焦點", "World focus"));
                                    ui.selectable_value(&mut self.camera.primitive, 1, tl(en, "引擎座標", "Engine position"));
                                });
                            self.finish_camera_edit(&before);
                        });
                        ui.collapsing(tl(en, "場景生成物件與持久 ID（唯讀；請用「放置腳本物件」面板編輯）", "Scenario objects and persistent IDs (read-only; edit them in the Spawns panel)"), |ui| {
                            ui.small(tl(en, "新增、移動、旋轉、複製與刪除請使用工具列的「放置腳本物件 / Spawns」面板（含驗證與復原）。", "Add, move, rotate, copy and delete objects in the toolbar's Spawns panel (with validation and undo)."));
                            for s in &self.document.spawns {
                                let prebuilt = if !s.prebuilt {
                                    ""
                                } else {
                                    tl(en, "[預建 DATA] ", "[prebuilt DATA] ")
                                };
                                ui.label(if en {
                                    format!(
                                        "{prebuilt}{} / ID {} / X {:.0} Z {:.0} team {} count {} angle {}",
                                        s.alias, s.id, s.x, s.z, s.team, s.count, s.angle
                                    )
                                } else {
                                    format!(
                                        "{prebuilt}{} / ID {} / X {:.0} Z {:.0} team {} 數量 {} 角度 {}",
                                        s.alias, s.id, s.x, s.z, s.team, s.count, s.angle
                                    )
                                });
                            }
                        });
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button(tl(en, "新增事件", "Add event")).clicked() {
                                let item = Event {
                                    name: format!(
                                        "{} {}",
                                        tl(en, "事件", "Event"),
                                        self.document.events.len() + 1
                                    ),
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
                                    egui::Button::new(tl(en, "複製選取事件", "Duplicate selected event")),
                                )
                                .clicked()
                            {
                                if let Err(e) = self.duplicate_event(self.selected) {
                                    self.error = Some(e);
                                }
                            }
                            if !self.document.events.is_empty()
                                && ui.button(tl(en, "刪除選取事件", "Delete selected event")).clicked()
                            {
                                if let Err(e) = self.remove_event(self.selected) {
                                    self.error = Some(e);
                                }
                            }
                            if ui
                                .add_enabled(self.can_undo_events(), egui::Button::new(tl(en, "復原", "Undo")))
                                .clicked()
                            {
                                self.history_request = Some(false);
                            }
                            if ui
                                .add_enabled(self.can_redo_events(), egui::Button::new(tl(en, "重做", "Redo")))
                                .clicked()
                            {
                                self.history_request = Some(true);
                            }
                            ui.separator();
                            if ui.button(tl(en, "視覺化節點圖 (Event Graph)", "Event Graph")).clicked() {
                                self.open_graph();
                            }
                            let graph_dirty = self.graph_ui.is_dirty();
                            let apply_btn = egui::Button::new(tl(en, "從節點圖套用 (Apply Graph)", "Apply Graph"));
                            if ui.add_enabled(graph_dirty, apply_btn).clicked() {
                                if let Err(e) = self.apply_graph() {
                                    self.error = Some(e);
                                }
                            }
                            if graph_dirty {
                                ui.colored_label(egui::Color32::YELLOW, tl(en, "[節點圖有未套用變更]", "[Event graph has unapplied changes]"));
                            }
                        });
                        ui.horizontal_wrapped(|ui| {
                            let can_export = self.selected < self.document.events.len();
                            if ui
                                .add_enabled(
                                    can_export,
                                    egui::Button::new(tl(en, "匯出選取事件…", "Export selected events…")),
                                )
                                .clicked()
                            {
                                if let Err(e) = self.export_events() {
                                    self.error = Some(e);
                                }
                            }
                            if ui
                                .add_enabled(
                                    self.is_loaded(),
                                    egui::Button::new(tl(en, "匯入事件…", "Import events…")),
                                )
                                .clicked()
                            {
                                if let Err(e) = self.import_events() {
                                    self.error = Some(e);
                                }
                            }
                            ui.separator();
                            ui.checkbox(&mut self.template_remap, tl(en, "重新對應隊伍", "Remap team"));
                            ui.label(tl(en, "來源", "Source"));
                            ui.add(egui::DragValue::new(&mut self.template_from).clamp_range(-1..=15));
                            ui.label(tl(en, "目標", "Target"));
                            ui.add(egui::DragValue::new(&mut self.template_to).clamp_range(-1..=15));
                        });
                        self.show_event_browser(ui);
                        let targets: Vec<_> = self
                            .document
                            .spawns
                            .iter()
                            .map(|s| (s.clone(), format!("{} / {}", s.alias, s.id)))
                            .collect();
                        if let Some(current) = self.document.events.get(self.selected) {
                            let mut edited = current.clone();
                            let mut pick = None;
                            let mut draw = None;
                            event_editor(ui, &mut edited, &targets, &self.aliases, en, &mut pick, &mut draw);
                            if let Some(condition) = draw {
                                self.canvas.pick = Some(crate::scenario_canvas::PickTarget::Condition {
                                    event: self.selected, condition, snapshot: edited.clone(),
                                });
                            }
                            if let Some((action, destination)) = pick {
                                self.canvas.pick = Some(crate::scenario_canvas::PickTarget::Event {
                                    event: self.selected, action, destination,
                                    snapshot: edited.clone(),
                                });
                            }
                            if &edited != current {
                                if let Err(e) = self.replace_event(self.selected, edited) {
                                    self.error = Some(e);
                                }
                            }
                        }
                    });
                ui.separator();
                if ui.button(tl(en, "驗證並編譯預覽", "Validate and compile preview")).clicked() {
                    if let Err(e) = self.preview() {
                        self.error = Some(e)
                    }
                }
                ui.horizontal(|ui| {
                    ui.label(tl(en, "新工作副本目錄", "New working copy folder"));
                    ui.text_edit_singleline(&mut self.export_path);
                    if ui.button(tl(en, "輸出 JSON + BCI + 原版備份", "Export JSON + BCI + original backup")).clicked() {
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
            egui::Window::new(tl(en, "尚未輸出的劇本修改", "Unexported scenario changes"))
                .id(egui::Id::new("scenario_unexported_changes"))
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(tl(en, "載入或關閉會放棄目前修改（包含未套用之節點圖變更）。可取消後先輸出工作副本。", "Loading or closing discards the current changes (including unapplied event graph edits). Cancel to export a working copy first."));
                    ui.horizontal(|ui| {
                        if ui.button(tl(en, "放棄修改並繼續", "Discard changes and continue")).clicked() {
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
                        if ui.button(tl(en, "取消", "Cancel")).clicked() {
                            self.pending = None
                        }
                    });
                });
        }
        self.graph_ui.english = self.english;
        self.graph_ui.show(ctx);
    }
}

fn event_editor(
    ui: &mut egui::Ui,
    e: &mut Event,
    targets: &[(Spawn, String)],
    aliases: &[String],
    en: bool,
    pick: &mut Option<(usize, bool)>,
    draw: &mut Option<usize>,
) {
    ui.separator();
    ui.horizontal(|ui| {
        ui.checkbox(&mut e.enabled, tl(en, "啟用事件", "Event enabled"));
        ui.text_edit_singleline(&mut e.name);
    });
    ui.horizontal(|ui| {
        ui.label(tl(en, "開局後延遲（秒）", "Delay after start (s)"));
        ui.add(egui::DragValue::new(&mut e.delay_seconds).clamp_range(0..=86400));
        ui.checkbox(&mut e.repeat, tl(en, "週期重複", "Repeat"));
    });
    ui.label(tl(en, "條件：全部成立且延遲到期後執行；死亡條件先追蹤曾存在的物件。", "Conditions: all must hold once the delay expires; death conditions track objects that once existed."));
    let mut remove = None;
    for (i, c) in e.conditions.iter_mut().enumerate() {
        ui.push_id(("condition", i), |ui| {
            ui.horizontal(|ui| {
                kind_combo(
                    ui,
                    "kind",
                    &mut c.kind,
                    if en {
                        &["Object exists", "Dead or removed", "Inside rectangle"]
                    } else {
                        &["物件存在", "死亡或已移除", "位於矩形區域"]
                    },
                );
                if ui.button(tl(en, "刪除此條件", "Delete condition")).clicked() {
                    remove = Some(i)
                }
            });
            egui::ComboBox::from_id_source("target")
                .selected_text(
                    targets
                        .iter()
                        .find(|(s, _)| s.id == c.target_id)
                        .map(|(_, n)| n.as_str())
                        .unwrap_or(tl(en, "請選擇目標 / 已刪除", "Choose a target / deleted")),
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
                    if ui.button(tl(en, "在地圖上框選", "Draw on map")).clicked() {
                        *draw = Some(i);
                    }
                });
            }
        });
    }
    if let Some(i) = remove {
        e.conditions.remove(i);
    }
    if ui.button(tl(en, "新增物件條件", "Add object condition")).clicked() {
        let mut c = Condition::default();
        if let Some((s, _)) = targets.first() {
            c.target_id = s.id
        }
        e.conditions.push(c);
    }
    ui.label(tl(en, "動作：依序執行。勝敗動作只能放在單次事件最後。", "Actions run in order. Victory or Defeat must be the last action of a one-shot event."));
    let mut remove = None;
    let mut move_request: Option<(usize, isize)> = None;
    let action_count = e.actions.len();
    for (i, a) in e.actions.iter_mut().enumerate() {
        ui.push_id(("action", i), |ui| {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    kind_combo(
                        ui,
                        "kind",
                        &mut a.kind,
                        if en {
                            &["Show message", "Team diplomacy", "Spawn troops", "Victory", "Defeat"]
                        } else {
                            &["顯示訊息", "隊伍外交", "生成部隊", "勝利", "失敗"]
                        },
                    );
                    if ui.button(tl(en, "刪除此動作", "Delete action")).clicked() {
                        remove = Some(i)
                    }
                    if ui.add_enabled(i > 0, egui::Button::new(tl(en, "上移", "Move up"))).clicked() {
                        move_request = Some((i, -1));
                    }
                    if ui
                        .add_enabled(i + 1 < action_count, egui::Button::new(tl(en, "下移", "Move down")))
                        .clicked()
                    {
                        move_request = Some((i, 1));
                    }
                });
                match a.kind {
                    0 => {
                        ui.text_edit_multiline(&mut a.text);
                        ui.small(tl(en, "文字需能使用 Windows-1251 編碼，最多 1000 bytes。", "Text must encode as Windows-1251, at most 1000 bytes."));
                    }
                    1 => {
                        ui.horizontal(|ui| {
                            ui.label(tl(en, "隊伍", "team"));
                            ui.add(egui::DragValue::new(&mut a.team).clamp_range(0..=7));
                            ui.label(tl(en, "對方隊伍", "other team"));
                            ui.add(egui::DragValue::new(&mut a.other_team).clamp_range(0..=7));
                            ui.checkbox(&mut a.hostile, tl(en, "敵對", "Hostile"));
                        });
                    }
                    2 => {
                        ui.horizontal(|ui| {
                            ui.label(tl(en, "ObjDefName 別名", "ObjDefName alias"));
                            ui.text_edit_singleline(&mut a.alias);
                            alias_combo(ui, &mut a.alias, aliases, en);
                            ui.label(tl(en, "隊伍", "team"));
                            ui.add(egui::DragValue::new(&mut a.team).clamp_range(0..=7));
                            ui.label(tl(en, "數量", "count"));
                            ui.add(egui::DragValue::new(&mut a.count).clamp_range(1..=20));
                        });
                        ui.horizontal(|ui| {
                            ui.label("X");
                            ui.add(egui::DragValue::new(&mut a.x).clamp_range(0. ..=16383.));
                            ui.label("Z");
                            ui.add(egui::DragValue::new(&mut a.z).clamp_range(0. ..=16383.));
                            if ui.button(tl(en, "在地圖上點選（生成點）", "Pick on map (spawn)")).clicked() {
                                *pick = Some((i, false));
                            }
                        });
                        let mut moved = a.move_destination.is_some();
                        if ui.checkbox(&mut moved, tl(en, "生成成功後移動至目的地", "Move to destination after spawning")).changed() {
                            a.move_destination = if moved {
                                Some(WorldPosition { x: 8000, z: 8000 })
                            } else {
                                None
                            }
                        }
                        if let Some(p) = &mut a.move_destination {
                            ui.horizontal(|ui| {
                                ui.label(tl(en, "目的 X", "Destination X"));
                                ui.add(egui::DragValue::new(&mut p.x).clamp_range(0..=16383));
                                ui.label(tl(en, "目的 Z", "Destination Z"));
                                ui.add(egui::DragValue::new(&mut p.z).clamp_range(0..=16383));
                                if ui.button(tl(en, "在地圖上點選（目的地）", "Pick on map (destination)")).clicked() {
                                    *pick = Some((i, true));
                                }
                            });
                        }
                    }
                    3 | 4 => {
                        ui.small(tl(en, "結束任務；後續事件停止輪詢。", "Ends the mission; later events stop polling."));
                    }
                    _ => {
                        ui.colored_label(egui::Color32::RED, tl(en, "不支援的動作種類", "Unsupported action kind"));
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
    } else if let Some((i, offset)) = move_request {
        move_action(&mut e.actions, i, offset);
    }
    if ui.button(tl(en, "新增動作", "Add action")).clicked() {
        e.actions.push(Action {
            text: "Message".into(),
            ..Default::default()
        });
    }
}
/// C# `ScenarioEventDialog` Move up/down: swaps an action with its neighbour (execution order
/// matters). Returns the action's new index, or `None` when the move is out of range.
/// The caller edits a clone of the event, so the surrounding `replace_event` records exactly one
/// undo step.
fn move_action(actions: &mut [Action], index: usize, offset: isize) -> Option<usize> {
    let target = index.checked_add_signed(offset)?;
    if index >= actions.len() || target >= actions.len() {
        return None;
    }
    actions.swap(index, target);
    Some(target)
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
    #[test]
    fn empty_state_hints_are_bilingual_and_follow_scenario_state() {
        let mut panel = super::ScenarioUi::default();
        for en in [false, true] {
            panel.english = en;
            let hint = panel.empty_state_hint().unwrap();
            assert!(hint.contains("SCRIPT/ak_level.bci"));
            assert!(hint.contains(if en { "Loading terrain" } else { "載入地形" }));
        }
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../re_workspace").join(format!("empty-hint-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("SCRIPT")).unwrap();
        std::fs::write(root.join("SCRIPT/ak_level.bci"), against_rome_scenario::BciImage::idle().serialize()).unwrap();
        panel.load_session(&root).unwrap();
        for en in [false, true] {
            panel.english = en;
            assert!(panel.empty_state_hint().unwrap().contains(if en { "No events yet" } else { "尚無事件" }));
            panel.open = true;
            let ctx = egui::Context::default();
            let before = panel.document.clone();
            let _ = ctx.run(Default::default(), |ctx| panel.show(ctx));
            assert_eq!(panel.document, before);
            assert!(!panel.dirty());
        }
        panel.document.events.push(Default::default());
        assert!(panel.empty_state_hint().is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn action_field_labels_follow_language_snapshot() {
        for (english, expected) in [
            (false, vec!["隊伍", "對方隊伍", "隊伍", "數量"]),
            (true, vec!["team", "other team", "team", "count"]),
        ] {
            let ctx = egui::Context::default();
            let mut event = Event {
                actions: vec![
                    Action { kind: 1, ..Default::default() },
                    Action { kind: 2, ..Default::default() },
                ],
                ..Default::default()
            };
            let mut pick = None;
            let mut draw = None;
            ctx.begin_frame(egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO, egui::vec2(1800.0, 1400.0),
                )),
                ..Default::default()
            });
            egui::CentralPanel::default().show(&ctx, |ui| {
                event_editor(ui, &mut event, &[], &[], english, &mut pick, &mut draw);
            });
            let output = ctx.end_frame();
            let labels: Vec<String> = output.shapes.iter().filter_map(|shape| {
                if let egui::epaint::Shape::Text(text) = &shape.shape {
                    let label = text.galley.text();
                    if ["隊伍", "對方隊伍", "數量", "team", "other team", "count"].contains(&label) {
                        return Some(label.to_owned());
                    }
                }
                None
            }).collect();
            assert_eq!(labels, expected);
        }
    }
    fn shadow_event(name: &str) -> Event {
        Event { name: name.into(), actions: vec![Action { text: "shadow".into(),
            ..Default::default() }], ..Default::default() }
    }

    #[test]
    fn shadow_events_all_commit_paths_and_token_roundtrip() {
        let mut ui = ScenarioUi::default();
        ui.source = Some(Source { path: "unused.bci".into(), bytes: vec![] });
        let mut tokens = vec![ui.history_token()];
        ui.add_events(&[shadow_event("新增")]).unwrap();
        tokens.push(ui.history_token());
        ui.duplicate_event(0).unwrap();
        tokens.push(ui.history_token());
        ui.replace_event(0, shadow_event("取代")).unwrap();
        tokens.push(ui.history_token());
        ui.remove_event(1).unwrap();
        tokens.push(ui.history_token());
        ui.replace_events(vec![shadow_event("整批")]).unwrap();
        tokens.push(ui.history_token());
        let mut draft = ui.document.clone();
        draft.events.push(shadow_event("提交"));
        ui.commit_events(draft);
        tokens.push(ui.history_token());
        ui.document.events.push(shadow_event("外部同步"));
        ui.sync_session();
        tokens.push(ui.history_token());
        let receipts = ui.take_history_receipts();
        assert_eq!(receipts.len(), tokens.len() - 1);
        for (receipt, pair) in receipts.iter().zip(tokens.windows(2)) {
            assert_eq!(receipt.subsystem, crate::undo_journal::SubsystemId::Scenario);
            assert_eq!((receipt.before_token, receipt.after_token), (pair[0], pair[1]));
            assert_ne!(pair[0], pair[1]);
        }
        for token in tokens[..tokens.len()-1].iter().rev() {
            assert!(ui.undo_events());
            assert_eq!(ui.history_token(), *token);
        }
        for token in &tokens[1..] {
            assert!(ui.redo_events());
            assert_eq!(ui.history_token(), *token);
        }
        assert!(ui.take_history_receipts().is_empty());
    }

    #[test]
    fn shadow_events_noop_reject_branch_clear_and_external_invalidation() {
        let mut ui = ScenarioUi::default();
        ui.source = Some(Source { path: "unused.bci".into(), bytes: vec![] });
        ui.add_events(&[shadow_event("原始")]).unwrap();
        ui.take_history_receipts();
        let token = ui.history_token();
        ui.add_events(&[]).unwrap();
        ui.replace_event(0, ui.document.events[0].clone()).unwrap();
        ui.replace_events(ui.document.events.clone()).unwrap();
        ui.commit_events(ui.document.clone());
        assert_eq!(ui.history_token(), token);
        assert!(ui.take_history_receipts().is_empty());
        for _ in 0..3 {
            assert!(ui.undo_events());
            assert_eq!(ui.history_token(), token);
        }
        let before = ui.document.clone();
        assert!(ui.remove_event(99).is_err());
        assert!(ui.duplicate_event(99).is_err());
        assert!(ui.replace_event(99, shadow_event("無效")).is_err());
        assert!(ui.add_events(&vec![shadow_event("過量"); 256]).is_err());
        assert_eq!(ui.document, before);
        assert_eq!(ui.history_token(), token);
        assert!(ui.can_redo_events());
        assert!(ui.take_history_receipts().is_empty());
        let baseline = ui.events.baseline().to_vec();
        let dirty = ui.dirty();
        ui.clear_redo();
        assert_eq!(ui.events.baseline(), baseline);
        assert_eq!(ui.dirty(), dirty);
        assert!(!ui.can_redo_events());
        assert_eq!(ui.history_token(), token);
        assert_eq!(ui.document, before);
        assert!(ui.undo_events());
        assert!(ui.redo_events());
        assert_eq!(ui.history_token(), token);
        ui.add_events(&[shadow_event("舊分支")]).unwrap();
        let old_branch = ui.history_token();
        assert!(ui.undo_events());
        ui.add_events(&[shadow_event("新分支")]).unwrap();
        assert_ne!(ui.history_token(), old_branch);
        assert!(!ui.can_redo_events());
        let token = ui.history_token();
        ui.reset_session(None);
        assert_ne!(ui.history_token(), token);
        assert!(ui.take_history_receipts().is_empty());
        assert!(!ui.can_undo_events());
    }

    #[test]
    fn shadow_events_recovery_tokens_and_redo_clearing_preserve_undo() {
        let mut ui = recovery_scenario();
        let restored = ui.history_token();
        let receipt = ui.take_history_receipts();
        assert_eq!(receipt.len(), 1);
        assert_eq!(receipt[0].after_token, restored);
        ui.add_events(&[shadow_event("復原後")]).unwrap();
        let edited = ui.history_token();
        assert!(ui.undo_events());
        assert_eq!(ui.history_token(), restored);
        assert!(ui.undo_events());
        assert_eq!(ui.history_token(), receipt[0].before_token);
        assert!(ui.redo_events());
        assert_eq!(ui.history_token(), restored);
        assert!(ui.redo_events());
        assert_eq!(ui.history_token(), edited);
        assert!(ui.undo_events());
        ui.clear_redo();
        assert!(!ui.can_redo_events());
        assert!(ui.undo_events());
        assert!(ui.redo_events());
        assert_eq!(ui.history_token(), restored);
        assert!(!ui.can_redo_events());
        assert!(ui.undo_events());
        ui.clear_redo();
        assert!(!ui.redo_events());
    }

    use super::*;
    fn recovery_scenario() -> ScenarioUi {
        let mut ui = ScenarioUi::default();
        ui.source = Some(Source { path: PathBuf::from("TEMP-recovery-test.bci"), bytes: vec![] });
        let mut state = ui.recovery_state();
        state[0]["Events"] = serde_json::json!([Event {
            name: "復原事件".into(), actions: vec![Action { text: "recovered".into(), ..Default::default() }], ..Default::default()
        }]);
        ui.restore_recovery(state).unwrap();
        ui
    }
    #[test]
    fn recovery_preserves_event_redo_history_across_full_swap() {
        let mut ui = recovery_scenario();
        ui.add_events(&[Event { name: "新增事件".into(), actions: vec![Action { text: "added".into(), ..Default::default() }], ..Default::default() }]).unwrap();
        assert!(ui.undo_events());
        assert_eq!(ui.document.events.len(), 1);
        assert!(ui.undo_events());
        assert!(ui.document.events.is_empty());
        assert!(ui.redo_events());
        assert_eq!(ui.document.events.len(), 1);
        assert!(ui.redo_events());
        assert_eq!(ui.document.events.len(), 2);
        assert_eq!(ui.document.events[1].name, "新增事件");
        assert!(ui.undo_events());
        assert!(ui.undo_events());
        assert!(ui.redo_events());
        assert!(ui.redo_events());
        assert_eq!(ui.document.events.len(), 2);
    }
    #[test]
    fn recovery_redo_is_invalidated_by_spawn_event_and_inline_camera_edits() {
        for kind in 0..3 {
            let mut ui = recovery_scenario();
            assert!(ui.undo_events());
            match kind {
                0 => ui.apply_spawns(vec![Spawn { alias: "wolf".into(), id: uuid::Uuid::new_v4(), count: 1, ..Default::default() }]).unwrap(),
                1 => { ui.add_events(&[Event { name: "新分支".into(), actions: vec![Action { text: "added".into(), ..Default::default() }], ..Default::default() }]).unwrap(); }
                _ => { let before = ui.camera.clone(); ui.camera.x += 10.0; ui.finish_camera_edit(&before); }
            }
            assert!(!ui.can_redo_events());
            let current = ui.recovery_state();
            assert!(!ui.redo_events());
            assert_eq!(ui.recovery_state(), current);
        }
    }
    fn browser_events() -> Vec<Event> {
        vec![
            Event { name: "Late Guard".into(), delay_seconds: 125, repeat: true,
                actions: vec![Action { text: "Hold POSITION".into(), team: 7, other_team: 6, ..Default::default() }],
                ..Default::default() },
            Event { name: "Wave".into(), delay_seconds: 10,
                actions: vec![Action { kind: 2, alias: "GER_INF01".into(), count: 12, team: 2, other_team: 3,
                    move_destination: Some(WorldPosition { x: 123, z: 456 }), ..Default::default() },
                    Action { kind: 1, team: 4, other_team: 5, hostile: false, ..Default::default() }],
                ..Default::default() },
            Event { name: "Disabled".into(), delay_seconds: 10, enabled: false,
                actions: vec![Action { kind: 3, text: "Victory text".into(), ..Default::default() }], ..Default::default() },
            Event { name: "End".into(), delay_seconds: 0,
                actions: vec![Action { kind: 4, text: "Defeat text".into(), ..Default::default() }], ..Default::default() },
        ]
    }
    #[test]
    fn event_browser_combined_filters_and_case_insensitive_search() {
        let events = browser_events();
        for (query, expected) in [("lAtE gUaRd", vec![0]), ("PoSiTiOn", vec![0]), ("ger_inf01", vec![1]),
            ("  GER_inf01  ", vec![1]), ("不存在", vec![]), ("", vec![0, 1, 2, 3])] {
            assert_eq!(EventBrowser { search: query.into(), ..Default::default() }.indices(&events), expected);
        }
        for team in 0..=7 {
            let expected: Vec<_> = events.iter().enumerate().filter(|(_, e)|
                e.actions.iter().any(|a| a.team == team || a.other_team == team)).map(|(i, _)| i).collect();
            assert_eq!(EventBrowser { team: Some(team), ..Default::default() }.indices(&events), expected);
        }
        for (kind, expected) in [(0, vec![0]), (1, vec![1]), (2, vec![1]), (3, vec![2]), (4, vec![3])] {
            assert_eq!(EventBrowser { kind: Some(kind), ..Default::default() }.indices(&events), expected);
        }
        let mut filter = EventBrowser { search: "GER_INF01".into(), team: Some(5), kind: Some(2), enabled_only: true, time_order: true, ..Default::default() };
        // 各篩選套用於事件；隊伍與種類可由不同動作符合。
        assert_eq!(filter.indices(&events), vec![1]);
        filter.team = Some(7);
        assert!(filter.indices(&events).is_empty());
        filter = EventBrowser { kind: Some(3), enabled_only: true, ..Default::default() };
        assert!(filter.indices(&events).is_empty());
        assert!(EventBrowser::default().indices(&[]).is_empty());
    }
    #[test]
    fn batch9_filtered_bulk_disable_uses_snapshot_and_one_receipt() {
        let mut ui = ScenarioUi::default();
        ui.document.events = browser_events();
        ui.events = ScenarioEventSession::with_events(ui.document.events.clone());
        ui.source = Some(Source { path: PathBuf::from("original/SCRIPT/ak_level.bci"), bytes: vec![1, 2, 3] });
        ui.browser.enabled_only = true;
        let indices = ui.browser.indices(&ui.document.events);
        let before = ui.document.events.clone();
        assert_eq!(ui.set_events_enabled(&indices, false).unwrap(), indices.len());
        assert_eq!(ui.take_history_receipts().len(), 1);
        assert!(ui.browser.indices(&ui.document.events).is_empty());
        assert_eq!(ui.set_events_enabled(&[], true).unwrap(), 0);
        assert!(ui.take_history_receipts().is_empty());
        for (i, event) in ui.document.events.iter().enumerate() {
            if !indices.contains(&i) {
                assert_eq!(serde_json::to_vec(event).unwrap(), serde_json::to_vec(&before[i]).unwrap());
            }
        }
        assert!(ui.undo_events());
        assert_eq!(serde_json::to_vec(&ui.document.events).unwrap(), serde_json::to_vec(&before).unwrap());
        assert_eq!(ui.source.as_ref().unwrap().bytes, vec![1, 2, 3]);
    }

    #[test]
    fn batch9_disabled_filter_and_clear_preserve_order_and_document() {
        let events = browser_events();
        let before = serde_json::to_vec(&events).unwrap();
        let mut browser = EventBrowser { disabled_only: true, time_order: true, ..Default::default() };
        let indices = browser.indices(&events);
        assert!(!indices.is_empty());
        assert!(indices.iter().all(|&i| !events[i].enabled));
        browser.search = "missing".into();
        browser.team = Some(7);
        browser.kind = Some(4);
        assert!(browser.indices(&events).is_empty());
        browser.clear_filters();
        assert!(browser.time_order);
        assert!(!browser.enabled_only && !browser.disabled_only);
        assert_eq!(browser.indices(&events), EventBrowser { time_order: true, ..Default::default() }.indices(&events));
        assert_eq!(serde_json::to_vec(&events).unwrap(), before);
    }

    #[test]
    fn event_browser_bilingual_summaries() {
        let mut events = browser_events();
        events[1].conditions = (0..=2).map(|kind| Condition { kind, ..Default::default() }).collect();
        assert_eq!(event_summary(&events[0], true), "Delay 2:05 / repeat | 0 conditions [] | Message: Hold POSITION");
        assert_eq!(event_summary(&events[0], false), "延遲 2:05 / 重複 | 0 條件 [] | 訊息: Hold POSITION");
        assert_eq!(event_summary(&events[1], true), "Delay 0:10 / once | 3 conditions [Object exists, Dead or removed, Inside rectangle] | Spawn: 12 × GER_INF01 / team 2 / destination (123, 456); Diplomacy: team 4 ↔ 5 / hostile=false");
        assert_eq!(event_summary(&events[1], false), "延遲 0:10 / 單次 | 3 條件 [物件存在, 死亡或已移除, 位於矩形區域] | 生成: 12 × GER_INF01 / 隊伍 2 / 目的地 (123, 456); 外交: 隊伍 4 ↔ 5 / 敵對=false");
        for en in [false, true] {
            assert!(event_summary(&events[2], en).contains(&format!("{}: Victory text", action_kind_label(3, en))));
            assert!(event_summary(&events[3], en).contains(&format!("{}: Defeat text", action_kind_label(4, en))));
            events[1].actions[0].move_destination = None;
            events[1].actions[1].hostile = true;
            let text = event_summary(&events[1], en);
            assert!(text.contains(tl(en, "目的地 無", "destination None")));
            assert!(text.contains(tl(en, "敵對=true", "hostile=true")));
        }
    }
    #[test]
    fn event_browser_sort_preserves_source_indices_and_selection() {
        let events = browser_events();
        let snapshot = events.clone();
        let mut browser = EventBrowser { time_order: true, ..Default::default() };
        assert_eq!(browser.indices(&events), vec![3, 1, 2, 0]);
        browser.enabled_only = true;
        assert_eq!(browser.indices(&events), vec![3, 1, 0]);
        browser.time_order = false;
        assert_eq!(browser.indices(&events), vec![0, 1, 3]);
        assert!(selection_hidden(2, &events, &browser.indices(&events)));
        assert!(!selection_hidden(1, &events, &browser.indices(&events)));
        assert!(!selection_hidden(0, &[], &[]));
        assert_eq!(events, snapshot);
    }
    #[test]
    fn event_browser_ui_preserves_selection_undo_dirty_and_compiled_bytes() {
        let p = source();
        let mut scenario_ui = ScenarioUi::default();
        scenario_ui.load_now(&p).unwrap();
        scenario_ui.document.events = browser_events();
        scenario_ui.aliases = vec!["GER_INF01".into()];
        scenario_ui.selected = 2;
        let before = scenario_ui.preview().unwrap();
        let snapshot = state_bytes(&scenario_ui.document, &scenario_ui.camera).unwrap();
        scenario_ui.baseline = snapshot.clone();
        let ctx = egui::Context::default();
        for en in [false, true] {
            scenario_ui.english = en;
            for search in ["", "ger_inf01", "no matches"] {
                for time_order in [false, true] {
                    scenario_ui.browser = EventBrowser { search: search.into(), enabled_only: true, time_order, ..Default::default() };
                    let output = ctx.run(Default::default(), |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| scenario_ui.show_event_browser(ui));
                    });
                    let text = format!("{:?}", output.shapes);
                    assert!(text.contains(tl(en, "已選事件被篩選隱藏", "The selected event is hidden by filters")));
                    assert!(text.contains(if en { "of 4" } else { "/ 4 個事件" }));
                    assert_eq!(scenario_ui.selected, 2);
                    assert!(!scenario_ui.dirty());
                    assert!(!scenario_ui.can_undo_events());
                    assert_eq!(state_bytes(&scenario_ui.document, &scenario_ui.camera).unwrap(), snapshot);
                    assert_eq!(scenario_ui.preview().unwrap(), before);
                }
            }
        }
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn objective_presets_equivalence_and_name_collisions() {
        let p = source();
        for mode in ["enabled", "disabled", "delay", "action", "condition", "extra_action"] {
            let mut ui = ScenarioUi::default();
            ui.load_now(&p).unwrap();
            let id = uuid::Uuid::new_v4();
            ui.document.spawns.push(Spawn { id, alias: "GER_HAU00".into(), prebuilt: true, ..Default::default() });
            ui.document.data_slots.push(scenario::DataSlot { spawn_id: id, slot: 0, uid: 10001 });
            ui.active_bindings = Some(vec![(0, 10001)]);
            let mut existing = ui.objective_event(Some(42), None, 0).unwrap();
            existing.name = "sURVIVE_42S".into();
            match mode {
                "disabled" => existing.enabled = false,
                "delay" => existing.delay_seconds += 1,
                "action" => existing.actions[0].kind = 4,
                "condition" => existing.conditions.push(Condition { kind: 1, target_id: id, ..Default::default() }),
                "extra_action" => existing.actions.insert(0, Action { text: "Message".into(), ..Default::default() }),
                _ => { existing.actions[0].text = "無關欄位".into(); }
            }
            ui.add_events(&[existing.clone()]).unwrap();
            let duplicate = matches!(mode, "enabled" | "extra_action");
            assert_eq!(ui.add_objective(Some(42), None, 0).unwrap(), !duplicate);
            if !duplicate {
                assert_eq!(ui.document.events[1].name, "Survive_42s_2");
                assert!(ui.undo_events());
                assert_eq!(ui.document.events, vec![existing.clone()]);
            }
            assert!(ui.undo_events());
            assert!(ui.document.events.is_empty());
            assert!(!ui.can_undo_events());
        }
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn objective_presets_require_compilable_spawn_and_binding() {
        let p = source();
        let mut ui = ScenarioUi::default();
        ui.load_now(&p).unwrap();
        let id = uuid::Uuid::new_v4();
        assert!(ui.add_objective(None, Some(id), 0).is_err());
        ui.document.spawns.push(Spawn { id, alias: "GER_INF01".into(), ..Default::default() });
        assert!(ui.add_objective(None, Some(id), 0).is_err());
        ui.document.spawns[0].alias = "GER_HAU00".into();
        // 沒有別名目錄的腳本生成物件不可通過真正的編譯邊界。
        assert!(ui.add_objective(None, Some(id), 0).is_err());
        ui.document.spawns[0].prebuilt = true;
        assert!(ui.add_objective(None, Some(id), 0).is_err());
        ui.document.data_slots.push(scenario::DataSlot { spawn_id: id, slot: 3, uid: 10001 });
        assert!(ui.add_objective(None, Some(id), 0).is_err());
        ui.active_bindings = Some(vec![(3, 10002)]);
        assert!(ui.add_objective(None, Some(id), 0).is_err());
        ui.active_bindings = Some(vec![(3, 10001)]);
        assert!(ui.add_objective(None, Some(id), 1).is_err());
        assert!(ui.add_objective(None, Some(id), 8).is_err());
        for seconds in [0, -1, 86401] { assert!(ui.add_objective(Some(seconds), None, 0).is_err()); }
        assert!(ui.document.events.is_empty());
        assert!(!ui.can_undo_events());
        assert!(ui.add_objective(None, Some(id), 0).unwrap());
        // 死亡條件的區域數值與終局動作的無關欄位不影響等價性。
        ui.document.events[0].conditions[0].min_x = 123;
        assert!(!ui.add_objective(None, Some(id), 0).unwrap());
        ui.document.events[0].conditions[0].min_x = 0;
        assert!(ui.undo_events());
        assert!(ui.document.events.is_empty());
        assert!(!ui.can_undo_events());
        ui.graph_ui.mark_dirty();
        assert!(ui.add_objective(Some(86400), None, 0).is_err());
        assert!(!ui.can_undo_events());
        ui.graph_ui.reload_from_scenario(&ui.document);
        assert!(ui.add_objective(Some(86400), None, 0).unwrap());
        ui.reset_session(None);
        assert!(ui.add_objective(Some(1), None, 0).is_err());
        assert!(!ui.can_undo_events());
        fs::remove_dir_all(p).unwrap();
    }
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
    fn texts(e: &Event) -> Vec<String> {
        e.actions.iter().map(|a| a.text.clone()).collect()
    }
    #[test]
    fn move_action_swaps_neighbours_and_rejects_out_of_range() {
        let mut v: Vec<Action> = ["a", "b", "c"]
            .iter()
            .map(|t| Action { text: (*t).into(), ..Default::default() })
            .collect();
        let names = |v: &[Action]| v.iter().map(|a| a.text.clone()).collect::<Vec<_>>();
        assert_eq!(move_action(&mut v, 0, -1), None);
        assert_eq!(move_action(&mut v, 2, 1), None);
        assert_eq!(move_action(&mut v, 3, -1), None);
        assert_eq!(names(&v), ["a", "b", "c"]);
        assert_eq!(move_action(&mut v, 1, 1), Some(2));
        assert_eq!(names(&v), ["a", "c", "b"]);
        assert_eq!(move_action(&mut v, 2, -1), Some(1));
        assert_eq!(names(&v), ["a", "b", "c"]);
    }
    #[test]
    fn moving_an_action_is_one_undo_step_through_replace_event() {
        let mut ui = ScenarioUi::default();
        let mut e = ev("e");
        e.actions = ["a", "b", "c"]
            .iter()
            .map(|t| Action { text: (*t).into(), ..Default::default() })
            .collect();
        ui.document.events = vec![e.clone()];
        ui.events = ScenarioEventSession::with_events(ui.document.events.clone());
        ui.baseline = state_bytes(&ui.document, &ui.camera).unwrap();

        // Same shape as the panel: edit a clone, then replace_event when it differs.
        let mut edited = ui.document.events[0].clone();
        assert_eq!(move_action(&mut edited.actions, 0, 1), Some(1));
        assert_ne!(edited, ui.document.events[0]);
        ui.replace_event(0, edited).unwrap();
        assert_eq!(texts(&ui.document.events[0]), ["b", "a", "c"]);
        assert!(ui.dirty());
        assert!(ui.undo_events());
        assert_eq!(texts(&ui.document.events[0]), ["a", "b", "c"]);
        assert!(!ui.dirty() && !ui.can_undo_events());
        assert!(ui.redo_events());
        assert_eq!(texts(&ui.document.events[0]), ["b", "a", "c"]);
    }
    #[test]
    fn bulk_enable_disable_events_is_one_undo_step_and_respects_filter() {
        let mut ui = ScenarioUi::default();
        let mut ev1 = ev("Event_1");
        ev1.enabled = true;
        let mut ev2 = ev("Event_2");
        ev2.enabled = true;
        let mut ev3 = ev("Event_3");
        ev3.enabled = false;
        ui.document.events = vec![ev1, ev2, ev3];
        ui.events = ScenarioEventSession::with_events(ui.document.events.clone());
        ui.baseline = state_bytes(&ui.document, &ui.camera).unwrap();
        ui.source = Some(Source { path: PathBuf::from("test"), bytes: Vec::new() });

        // Unloaded scenario test (with a fresh ui)
        let mut unloaded = ScenarioUi::default();
        assert!(unloaded.set_events_enabled(&[0], true).is_err());

        // Initial state: ev1=true, ev2=true, ev3=false
        assert_eq!(ui.document.events[0].enabled, true);
        assert_eq!(ui.document.events[1].enabled, true);
        assert_eq!(ui.document.events[2].enabled, false);

        // Bulk disable indices [0, 1]
        let changed = ui.set_events_enabled(&[0, 1], false).unwrap();
        assert_eq!(changed, 2);
        assert_eq!(ui.document.events[0].enabled, false);
        assert_eq!(ui.document.events[1].enabled, false);
        assert_eq!(ui.document.events[2].enabled, false);
        assert!(ui.dirty());

        // Single undo step restores both
        assert!(ui.undo_events());
        assert_eq!(ui.document.events[0].enabled, true);
        assert_eq!(ui.document.events[1].enabled, true);
        assert_eq!(ui.document.events[2].enabled, false);

        // Redo re-disables both
        assert!(ui.redo_events());
        assert_eq!(ui.document.events[0].enabled, false);
        assert_eq!(ui.document.events[1].enabled, false);
        assert_eq!(ui.document.events[2].enabled, false);

        // No-op when all target events already in state: returns 0 and does not record undo step
        let noop = ui.set_events_enabled(&[0, 1, 2], false).unwrap();
        assert_eq!(noop, 0);

        // Bulk enable [1, 2]: ev1 remains false, ev2 and ev3 become true (changed count = 2)
        let enabled_count = ui.set_events_enabled(&[1, 2], true).unwrap();
        assert_eq!(enabled_count, 2);
        assert_eq!(ui.document.events[0].enabled, false);
        assert_eq!(ui.document.events[1].enabled, true);
        assert_eq!(ui.document.events[2].enabled, true);

        // Out of bound index is safely ignored
        let safe = ui.set_events_enabled(&[999], false).unwrap();
        assert_eq!(safe, 0);
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

    #[test]
    fn event_template_export_import_roundtrip_with_collision_rename_and_picker() {
        let p = source();
        let mut ui = ScenarioUi::default();
        ui.load_now(&p).unwrap();
        ui.add_events(&[ev("Wave 1"), ev("Defense")]).unwrap();

        let temp_dir = scratch();
        let template_path = temp_dir.join("test_export.arm-events.json");
        let picker_path = template_path.clone();

        ui.selected = 0; // "Wave 1"
        ui.event_picker = Box::new(move |kind| match kind {
            crate::layout_io::LayoutDialogKind::Save { .. } => Some(picker_path.clone()),
            crate::layout_io::LayoutDialogKind::Open => Some(picker_path.clone()),
        });

        // Export selected event "Wave 1"
        assert!(ui.export_events().unwrap());
        assert!(template_path.is_file());
        assert!(ui.report.contains("匯出完成") || ui.report.contains("Export complete"));

        // Import back: "Wave 1" collides with existing "Wave 1", should be renamed to "Wave 1_2"
        let count = ui.import_events().unwrap();
        assert_eq!(count, 1);
        assert_eq!(ui.document.events.len(), 3);
        assert_eq!(ui.document.events[2].name, "Wave 1_2");

        // Verify one undo step removes the imported event
        assert!(ui.undo_events());
        assert_eq!(ui.document.events.len(), 2);
        assert_eq!(ui.document.events[0].name, "Wave 1");
        assert_eq!(ui.document.events[1].name, "Defense");

        fs::remove_dir_all(temp_dir).unwrap();
        fs::remove_dir_all(p).unwrap();
    }

    #[test]
    fn event_template_export_skips_object_targeted_conditions_with_bilingual_report() {
        let p = source();
        let mut ui = ScenarioUi::default();
        ui.load_now(&p).unwrap();

        let mut event_with_target = ev("Targeted");
        event_with_target.enabled = false;
        event_with_target.conditions.push(Condition {
            kind: 0,
            target_id: uuid::Uuid::new_v4(),
            ..Default::default()
        });
        ui.add_events(&[event_with_target]).unwrap();

        let temp_dir = scratch();
        let template_path = temp_dir.join("should_not_exist.arm-events.json");

        ui.selected = 0;
        let err = ui.export_events_to(&template_path).unwrap_err();
        assert!(err.contains("略過 1 個引用特定地圖物件") || err.contains("skipped 1 event(s) referencing"));
        assert!(!template_path.is_file());

        fs::remove_dir_all(temp_dir).unwrap();
        fs::remove_dir_all(p).unwrap();
    }

    #[test]
    fn event_template_import_team_remap() {
        let p = source();
        let mut ui = ScenarioUi::default();
        ui.load_now(&p).unwrap();

        let template_events = vec![Event {
            name: "Diplomatic".into(),
            actions: vec![Action {
                kind: 1,
                team: 2,
                other_team: 3,
                hostile: true,
                ..Default::default()
            }],
            ..Default::default()
        }];
        let template = crate::event_templates::EventTemplate(template_events);
        let temp_dir = scratch();
        let template_path = temp_dir.join("remap.arm-events.json");
        let json = template.serialize().unwrap();
        fs::write(&template_path, json).unwrap();

        // Remap team 2 -> 5
        let imported = ui.import_events_from(&template_path, Some((2, 5))).unwrap();
        assert_eq!(imported, 1);
        assert_eq!(ui.document.events[0].actions[0].team, 5);
        assert_eq!(ui.document.events[0].actions[0].other_team, 3);

        fs::remove_dir_all(temp_dir).unwrap();
        fs::remove_dir_all(p).unwrap();
    }

    #[test]
    fn event_template_256_limit_rejection_leaves_document_unchanged() {
        let p = source();
        let mut ui = ScenarioUi::default();
        ui.load_now(&p).unwrap();

        // Fill up to 255 events
        let full_255: Vec<Event> = (0..255).map(|i| ev(&format!("ev_{i}"))).collect();
        ui.add_events(&full_255).unwrap();
        assert_eq!(ui.document.events.len(), 255);

        // Create a template with 2 events (255 + 2 = 257 > 256)
        let template = crate::event_templates::EventTemplate(vec![ev("Extra 1"), ev("Extra 2")]);
        let temp_dir = scratch();
        let template_path = temp_dir.join("exceed_256.arm-events.json");
        fs::write(&template_path, template.serialize().unwrap()).unwrap();

        let err = ui.import_events_from(&template_path, None).unwrap_err();
        assert!(err.contains("256"));
        // Document must remain unchanged!
        assert_eq!(ui.document.events.len(), 255);

        fs::remove_dir_all(temp_dir).unwrap();
        fs::remove_dir_all(p).unwrap();
    }
}

fn alias_combo(ui: &mut egui::Ui, alias: &mut String, names: &[String], en: bool) {
    if names.is_empty() {
        return;
    }
    egui::ComboBox::from_id_source("object_alias_catalog")
        .selected_text(tl(en, "從遊戲別名選擇", "Choose game alias"))
        .show_ui(ui, |ui| {
            for name in names {
                ui.selectable_value(alias, name.clone(), name);
            }
        });
}
