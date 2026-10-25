//! DATA object-family editor. Its Save is independent of terrain/image saving.
use crate::design_apply::{apply_placements, PlacementBatch, PlacementRequest};
use crate::layout_io::{
    self, LayoutApplyOptions, LayoutContext, LayoutDialogKind, LayoutFilePicker, MapLayoutKind,
    MapLayoutPreset,
};
use against_rome_placement::{ObjectStore, WorldObject};
use eframe::egui;
use std::{
    fs,
    path::{Path, PathBuf},
};

type Result<T> = std::result::Result<T, String>;

/// Barrier 必須使整合端的 journal 失效，不可當成可復原步驟。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlacementHistoryReceipt {
    Commit(crate::undo_journal::SubsystemStep),
    Barrier { after_token: u64 },
}

impl PlacementHistoryReceipt {
    pub fn after_token(&self) -> u64 {
        match self {
            Self::Commit(step) => step.after_token,
            Self::Barrier { after_token } => *after_token,
        }
    }
}

/// 與既有分組步數同步；核心接受的相同值操作不產生新 token。
#[derive(Clone, Debug, Default)]
struct PlacementHistory {
    tokens: against_rome_textures::history::HistoryTokens,
    undo: Vec<bool>,
    redo: Vec<bool>,
}

impl PlacementHistory {
    fn token(&self) -> u64 { self.tokens.token() }
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

fn history_payloads(store: Option<&ObjectStore>) -> Vec<Option<Vec<u8>>> {
    ["objects", "objdata", "position", "anim", "gfxtype", "action", "engine",
        "light", "particle", "lager", "biglager"]
        .iter().map(|name| store.and_then(|s| s.payload(name)).map(<[u8]>::to_vec)).collect()
}

/// Encodes raw uncompressed payload into Against Rome's native PFIL container format.
/// Preserves the 64-byte header while updating the uncompressed length at offset 16..20,
/// and packages chunks with uncompressed control bytes.
fn encode_pfil_container(header_64: &[u8; 64], uncompressed: &[u8]) -> Vec<u8> {
    let mut header = *header_64;
    header[16..20].copy_from_slice(&(uncompressed.len() as u32).to_le_bytes());
    let mut out = header.to_vec();
    for part in uncompressed.chunks(8) {
        let control = ((1u16 << part.len()) - 1) as u8;
        out.push(control);
        out.extend_from_slice(part);
    }
    out
}

/// World displacement rounded to the nearest signed 16.16 unit. Never saturates.
fn fixed_delta(values: &[String; 3]) -> Result<[i32; 3]> {
    let mut result = [0; 3];
    for axis in 0..3 {
        let world = values[axis]
            .trim()
            .parse::<f64>()
            .map_err(|_| format!("{} 位移不是有效數值", ["X", "Y", "Z"][axis]))?;
        let scaled = world * 65536.0;
        if !world.is_finite()
            || !scaled.is_finite()
            || scaled < i32::MIN as f64
            || scaled > i32::MAX as f64
        {
            return Err(format!(
                "{} 位移超出 signed 16.16 範圍 [-32768, 32767.99998474121]",
                ["X", "Y", "Z"][axis]
            ));
        }
        let rounded = scaled.round();
        if rounded < i32::MIN as f64 || rounded > i32::MAX as f64 {
            return Err("16.16 四捨五入後超出範圍".into());
        }
        result[axis] = rounded as i32;
    }
    Ok(result)
}

fn explicit_map(text: &str) -> Result<PathBuf> {
    let path = PathBuf::from(text.trim());
    if text.trim().is_empty()
        || !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("請輸入 MAP 資料夾的完整絕對路徑，不接受 ..".into());
    }
    reject_links(&path)?;
    Ok(path)
}
fn reject_links(path: &Path) -> Result<()> {
    for p in path.ancestors() {
        match fs::symlink_metadata(p) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(format!("拒絕 symbolic link: {}", p.display()));
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if metadata.file_attributes() & 0x400 != 0 {
                        return Err(format!("拒絕 junction/reparse point: {}", p.display()));
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{}: {e}", p.display())),
        }
    }
    Ok(())
}
fn allow_save(map: &Path) -> Result<()> {
    reject_links(map)?;
    let text = map.to_string_lossy().replace('/', "\\").to_lowercase();
    if text
        .split('\\')
        .any(|p| p == "program files" || p == "program files (x86)")
    {
        return Err("禁止保存到 Program Files；請先複製 MAP 到工作資料夾".into());
    }
    let canonical = fs::canonicalize(map).map_err(|e| e.to_string())?;
    for variable in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Some(root) = std::env::var_os(variable) {
            if let Ok(root) = fs::canonicalize(root) {
                if canonical.starts_with(root) {
                    return Err("禁止保存到安裝目錄；請操作 MAP 工作副本".into());
                }
            }
        }
    }
    let data = map.join("DATA");
    reject_links(&data)?;
    // Validate every existing DATA entry before the core creates its save lock or stages.
    for entry in fs::read_dir(&data).map_err(|e| e.to_string())? {
        reject_links(&entry.map_err(|e| e.to_string())?.path())?;
    }
    Ok(())
}

#[derive(Clone)]
enum Pending {
    Load(String),
    Close,
    Reset,
    Remove { families: Vec<(usize, u32)> },
}

#[derive(Clone)]
pub(crate) struct PlacementState {
    store: Option<ObjectStore>,
    history: PlacementHistory,
    undo: Vec<usize>,
    redo: Vec<usize>,
    selected: Option<(usize, u32)>,
    selection: Vec<(usize, u32)>,
    pending: Option<Pending>,
}

pub struct PlacementUi {
    pub english: bool,
    pub open: bool,
    pub(crate) selection_tools_blocked: bool,
    map_input: String,
    loaded_map: Option<PathBuf>,
    store: Option<ObjectStore>,
    /// Last successfully loaded/saved store, including raw bytes for save conflict checks.
    baseline: Option<ObjectStore>,
    selected: Option<(usize, u32)>,
    selection: Vec<(usize, u32)>,
    filter: String,
    tile_bounds: [String; 4],
    delta: [String; 3],
    angle: String,
    team: String,
    /// Object type id -> objdef name, used to apply per-category team limits (C# `ValidateBounds`).
    type_names: std::collections::HashMap<i32, String>,
    pending: Option<Pending>,
    status: String,
    /// History entries per user-visible edit (a batch apply is one undo step).
    undo_groups: Vec<usize>,
    history: PlacementHistory,
    history_payloads: Vec<Option<Vec<u8>>>,
    history_receipts: Vec<PlacementHistoryReceipt>,
    redo_groups: Vec<usize>,
    /// Raised by the Save button; `EditorApp` consumes it and routes to the central Safe Save.
    save_requested: bool,
    pub(crate) settlement_history_routing: bool,
    pub(crate) settlement_history_available: (bool, bool),
    pub(crate) history_request: Option<bool>,
    /// Injectable layout file picker; `None` uses the native `rfd` dialogs.
    pub layout_picker: Option<LayoutFilePicker>,
    /// Loaded layout waiting for the apply dialog (C# `LayoutApplyDialog`).
    layout_pending: Option<MapLayoutPreset>,
    layout_options: LayoutApplyOptions,
    layout_team_override: bool,
    layout_team: i32,
    /// Layout work that needs terrain/name context; drained by `process_layout_requests`.
    layout_queue: Vec<LayoutRequest>,
    layout_status: String,
}

/// Layout actions queued by the UI and executed with an app-provided [`LayoutContext`].
#[derive(Clone, Debug, PartialEq)]
pub enum LayoutRequest {
    ExportSelection,
    ExportNature(TileRect),
    Apply {
        preset: MapLayoutPreset,
        options: LayoutApplyOptions,
    },
}

fn tl(english: bool, zh: &'static str, en: &'static str) -> &'static str {
    if english { en } else { zh }
}

impl Default for PlacementUi {
    fn default() -> Self {
        Self {
            open: false,
            english: false,
            selection_tools_blocked: false,
            map_input: String::new(),
            loaded_map: None,
            store: None,
            baseline: None,
            selected: None,
            selection: Vec::new(),
            filter: String::new(),
            tile_bounds: ["0".into(), "0".into(), "1".into(), "1".into()],
            delta: ["0".into(), "0".into(), "0".into()],
            angle: "0".into(),
            team: "0".into(),
            type_names: std::collections::HashMap::new(),
            pending: None,
            status: String::new(),
            undo_groups: Vec::new(),
            history: Default::default(),
            history_payloads: history_payloads(None),
            history_receipts: Vec::new(),
            redo_groups: Vec::new(),
            save_requested: false,
            settlement_history_routing: false,
            settlement_history_available: (false, false),
            history_request: None,
            layout_picker: None,
            layout_pending: None,
            layout_options: LayoutApplyOptions::default(),
            layout_team_override: false,
            layout_team: 0,
            layout_queue: Vec::new(),
            layout_status: String::new(),
        }
    }
}
impl PlacementUi {
    pub fn history_token(&self) -> u64 { self.history.token() }

    pub fn take_history_receipts(&mut self) -> Vec<PlacementHistoryReceipt> {
        std::mem::take(&mut self.history_receipts)
    }

    /// 僅清除面板分組與 token 的重做，保留復原紀錄。
    /// ObjectStore 未提供 clear_redo；直接操作共用 store 的面板仍須通知外部編輯 barrier。
    pub fn clear_redo(&mut self) {
        self.redo_groups.clear();
        if let Some(store) = self.store.as_mut() { store.clear_redo(); }
        self.history.clear_redo();
    }

    fn history_barrier(&mut self) {
        self.history = Default::default();
        self.history_payloads = history_payloads(self.store.as_ref());
        self.history_receipts.push(PlacementHistoryReceipt::Barrier {
            after_token: self.history.token(),
        });
    }

    pub(crate) fn capture_state(&self) -> PlacementState {
        PlacementState {
            store: self.store.clone(),
            history: self.history.clone(),
            undo: self.undo_groups.clone(),
            redo: self.redo_groups.clone(),
            selected: self.selected,
            selection: self.selection.clone(),
            pending: self.pending.clone(),
        }
    }

    pub(crate) fn matches_state(&self, state: &PlacementState) -> bool {
        self.undo_groups == state.undo
            && self.redo_groups == state.redo
            && match (&self.store, &state.store) {
                (Some(a), Some(b)) => [
                    "objects", "objdata", "position", "anim", "gfxtype", "action", "engine",
                    "light", "particle", "lager", "biglager",
                ]
                .iter()
                .all(|n| a.payload(n) == b.payload(n)),
                (None, None) => true,
                _ => false,
            }
    }

    pub(crate) fn restore_state(&mut self, state: &PlacementState) {
        // 快照交換的複合紀錄留待後續整合；目前先使旁錄失效。
        if self.history.token() != state.history.token() {
            self.history_barrier();
        }
        self.history = state.history.clone();
        self.store = state.store.clone();
        self.history_payloads = history_payloads(self.store.as_ref());
        self.undo_groups = state.undo.clone();
        self.redo_groups = state.redo.clone();
        self.selected = state.selected;
        self.selection = state.selection.clone();
        self.pending = state.pending.clone();
    }

    pub(crate) fn matches_undo_boundary(&self, state: &PlacementState) -> bool {
        let mut boundary = state.clone();
        boundary.redo = self.redo_groups.clone();
        self.matches_state(&boundary)
    }

    pub(crate) fn history_candidate(&self, redo: bool) -> Result<PlacementState> {
        let mut candidate = Self::default();
        candidate.restore_state(&self.capture_state());
        if redo {
            candidate.redo()?;
        } else {
            candidate.undo()?;
        }
        Ok(candidate.capture_state())
    }

    /// Uses the existing grouped implementation on an isolated session.
    pub(crate) fn settlement_candidate(
        &self,
        requests: &[PlacementRequest],
    ) -> Result<PlacementState> {
        let mut candidate = Self::default();
        candidate.restore_state(&self.capture_state());
        let batch = candidate.apply_requests_grouped(requests)?;
        if batch.placed != requests.len() || !batch.skipped.is_empty() {
            return Err(format!(
                "Settlement placement rejected: {}",
                batch.skipped.join("; ")
            ));
        }
        Ok(candidate.capture_state())
    }

    pub(crate) fn detached_store(&self) -> Result<ObjectStore> {
        self.store.clone().ok_or_else(|| "Load DATA first".into())
    }

    pub(crate) fn stage_candidate_store(
        &self,
        map: &Path,
        store: ObjectStore,
    ) -> Result<Vec<(PathBuf, Vec<u8>)>> {
        let disk = ObjectStore::load(map)?;
        let baseline = self
            .baseline
            .as_ref()
            .ok_or("Placement: missing loaded baseline")?;
        for name in [
            "objects", "objdata", "position", "anim", "gfxtype", "action", "engine", "light",
            "particle", "lager", "biglager",
        ] {
            if disk.payload(name) != baseline.payload(name) {
                return Err(format!(
                    "Placement: DATA/{name}.dat changed on disk; reload before saving"
                ));
            }
        }
        let mut candidate = Self::default();
        candidate.loaded_map = self.loaded_map.clone();
        candidate.baseline = self.baseline.clone();
        candidate.store = Some(store);
        candidate.stage_transaction_files(map)
    }

    /// Ask the host to run the whole-map Safe Save; this panel never writes DATA itself.
    pub fn request_save(&mut self) {
        self.save_requested = true;
    }
    /// Takes (and clears) a pending Safe Save request.
    pub fn take_save_request(&mut self) -> bool {
        std::mem::take(&mut self.save_requested)
    }
    pub fn loaded_map(&self) -> Option<&Path> {
        self.loaded_map.as_deref()
    }
    pub fn delta(&self) -> &[String; 3] {
        &self.delta
    }
    pub fn set_delta(&mut self, delta: [String; 3]) {
        self.delta = delta;
    }
    pub fn undo_depth(&self) -> usize {
        self.undo_groups.len()
    }
    pub fn redo_depth(&self) -> usize {
        self.redo_groups.len()
    }
    pub fn is_dirty(&self) -> bool {
        self.store.as_ref().is_some_and(ObjectStore::is_dirty)
    }

    /// Resets all map-bound object documents, loaded paths, selections, undo/redo history, and pending actions.
    /// Preserves user preferences (open state, delta translations, angle, team).
    pub fn reset_session(&mut self, map_path: Option<&Path>) {
        self.store = None;
        self.type_names.clear();
        self.baseline = None;
        self.loaded_map = None;
        self.selected = None;
        self.selection.clear();
        self.filter.clear();
        self.pending = None;
        self.status.clear();
        self.undo_groups.clear();
        self.redo_groups.clear();
        self.history_barrier();
        self.map_input = map_path
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
    }

    /// Transactionally loads or resets placement session for the target map.
    /// The target map must be an existing directory.
    /// If the map contains a DATA directory, loads the ObjectStore. If the DATA directory does not
    /// exist, cleanly resets to an unloaded session. If DATA exists but is invalid, returns Err
    /// and leaves the current session untouched.
    pub fn load_session(&mut self, map_path: &Path) -> Result<()> {
        if !map_path.is_dir() {
            return Err(format!(
                "Target map path is not a directory: {}",
                map_path.display()
            ));
        }
        let data_dir = map_path.join("DATA");
        if data_dir.is_dir() {
            if Self::uses_native_game_object_pools(map_path)? {
                self.reset_session(Some(map_path));
                self.status =
                    "Native game DATA pools are not an editable Placement document.".into();
                return Ok(());
            }
            let store = ObjectStore::load(map_path)?;
            self.baseline = Some(store.clone());
            self.store = Some(store);
            self.loaded_map = Some(map_path.to_path_buf());
            self.map_input = map_path.to_string_lossy().to_string();
            self.selected = None;
            self.selection.clear();
            self.pending = None;
            self.status.clear();
            self.undo_groups.clear();
            self.redo_groups.clear();
            self.history_barrier();
            Ok(())
        } else {
            self.reset_session(Some(map_path));
            Ok(())
        }
    }

    /// Validates map placement data without changing an existing panel session.
    /// Native retail DATA pools are valid maps but are not the editor's ObjectStore format.
    pub fn validate_map_session(map_path: &Path) -> Result<()> {
        if !map_path.is_dir() {
            return Err(format!(
                "Target map path is not a directory: {}",
                map_path.display()
            ));
        }
        if !map_path.join("DATA").is_dir() || Self::uses_native_game_object_pools(map_path)? {
            return Ok(());
        }
        ObjectStore::load(map_path).map(|_| ())
    }

    fn uses_native_game_object_pools(map_path: &Path) -> Result<bool> {
        let objects_path = map_path.join("DATA/objects.dat");
        if !objects_path.is_file() {
            return Ok(false);
        }
        let raw = std::fs::read(&objects_path)
            .map_err(|e| format!("Failed to read {}: {e}", objects_path.display()))?;
        let bytes = if raw.starts_with(b"PFIL") {
            against_rome_data::lzss::decompress_pfil(&raw, None)
                .map_err(|e| format!("Failed to decompress {}: {e}", objects_path.display()))?
        } else {
            raw
        };
        if bytes.len() < 8 || u32::from_le_bytes(bytes[0..4].try_into().unwrap()) != 1 {
            return Ok(false);
        }
        let count = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let Some(expected_len) = count.checked_mul(56).and_then(|n| n.checked_add(8)) else {
            return Ok(false);
        };
        Ok(bytes.len() == expected_len)
    }

    fn load_now(&mut self, text: &str) -> Result<()> {
        let map = explicit_map(text)?;
        self.load_session(&map)
    }
    pub fn is_loaded(&self) -> bool {
        self.store.is_some()
    }

    /// Matches PlacementEditSession.Reset: restore the accepted in-memory baseline,
    /// keep the document loaded, and establish a fresh UI undo/redo boundary.
    /// Never reads disk; external changes must remain conflicts on a later save.
    /// Validate everything before replacing state so failures preserve pending edits.
    pub fn reset_to_baseline(&mut self) -> Result<()> {
        if self.store.is_none() || self.loaded_map.is_none() {
            return Err("請先載入 MAP".into());
        }
        let baseline = self.baseline.as_ref().ok_or("沒有可還原的 DATA 基準")?;
        if baseline.is_dirty() {
            return Err("DATA 基準含有未保存修改；拒絕還原".into());
        }
        let restored = baseline.clone();
        self.store = Some(restored);
        self.undo_groups.clear();
        self.redo_groups.clear();
        self.history_barrier();
        self.pending = None;
        self.check_selection();
        Ok(())
    }

    fn request_reset(&mut self) -> Result<()> {
        if !self.is_loaded() {
            return Err("請先載入 MAP".into());
        }
        self.pending = Some(Pending::Reset);
        Ok(())
    }

    /// Stages dirty placement pool files for transactional saving without modifying in-memory state.
    /// Returns pairs of relative path within the map (e.g. `DATA/position.dat`) and serialized bytes.
    /// Fails closed if the document cannot be losslessly saved, if paths do not match, or if
    /// external modifications / invalid formats are detected.
    pub fn stage_transaction_files(&self, target_map: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>> {
        if !self.is_dirty() {
            return Ok(Vec::new());
        }
        let store = self
            .store
            .as_ref()
            .ok_or("Placement is marked dirty but has no loaded store")?;
        let loaded = self
            .loaded_map
            .as_ref()
            .ok_or("Placement has unsaved changes but no map path is loaded")?;

        let is_same_path = match (fs::canonicalize(loaded), fs::canonicalize(target_map)) {
            (Ok(c1), Ok(c2)) => c1 == c2,
            _ => loaded == target_map,
        };
        if !is_same_path {
            return Err(format!(
                "Placement document loaded from '{}' cannot be saved to different map path '{}'",
                loaded.display(),
                target_map.display()
            ));
        }

        allow_save(target_map)?;

        if Self::uses_native_game_object_pools(target_map)? {
            return Err(
                "Native game DATA pools cannot be overwritten as an editor Placement document."
                    .into(),
            );
        }

        let mut staged = Vec::new();
        let pool_names = [
            "objects", "objdata", "position", "anim", "gfxtype", "action", "engine", "light",
            "particle", "lager", "biglager",
        ];

        let data_dir = target_map.join("DATA");
        if !data_dir.is_dir() {
            return Err(format!(
                "Placement DATA directory not found: {}",
                data_dir.display()
            ));
        }

        for name in pool_names {
            if let Some(current_bytes) = store.payload(name) {
                let pool_path = data_dir.join(format!("{name}.dat"));
                if pool_path.is_file() {
                    reject_links(&pool_path)?;
                    let disk_raw = fs::read(&pool_path)
                        .map_err(|e| format!("Failed to read {}: {e}", pool_path.display()))?;

                    if disk_raw.starts_with(b"PFIL") {
                        if disk_raw.len() < 64 {
                            return Err(format!(
                                "Corrupted PFIL header in {}",
                                pool_path.display()
                            ));
                        }
                        let initial_bytes =
                            against_rome_data::lzss::decompress_pfil(&disk_raw, None).map_err(
                                |e| format!("Failed to decompress {}: {e}", pool_path.display()),
                            )?;
                        if initial_bytes != current_bytes {
                            let encoded = encode_pfil_container(
                                &disk_raw[..64].try_into().unwrap(),
                                current_bytes,
                            );
                            staged.push((PathBuf::from(format!("DATA/{name}.dat")), encoded));
                        }
                    } else {
                        if disk_raw != current_bytes {
                            staged.push((
                                PathBuf::from(format!("DATA/{name}.dat")),
                                current_bytes.to_vec(),
                            ));
                        }
                    }
                } else {
                    if ["objects", "objdata", "position"].contains(&name) {
                        return Err(format!(
                            "Required placement pool missing from disk: {}",
                            pool_path.display()
                        ));
                    } else {
                        staged.push((
                            PathBuf::from(format!("DATA/{name}.dat")),
                            current_bytes.to_vec(),
                        ));
                    }
                }
            }
        }

        if staged.is_empty() {
            return Err(
                "Placement reported dirty, but no dirty pool files could be serialized".into(),
            );
        }

        Ok(staged)
    }

    /// Commits the in-memory baseline after a successful atomic MapSaveTransaction.
    /// Reloads the updated files from disk into ObjectStore so that baseline matches disk,
    /// is_dirty() becomes false, and selection is preserved if still valid.
    pub fn commit_baseline(&mut self, target_map: &Path) -> Result<()> {
        let prev_selected = self.selected;
        let prev_selection = self.selection.clone();
        self.load_session(target_map)?;
        self.select_object(prev_selected);
        if prev_selection.len() > 1 {
            self.select_objects(&prev_selection);
        }
        self.status = "Placement DATA saved in whole-map transaction.".into();
        Ok(())
    }
    pub(crate) fn record_edit(&mut self, entries: usize) {
        if entries > 0 {
            self.undo_groups.push(entries);
            self.redo_groups.clear();
            let payloads = history_payloads(self.store.as_ref());
            self.history.commit(payloads != self.history_payloads);
            self.history_payloads = payloads;
            self.history_receipts.extend(self.history.tokens.take_receipts().into_iter().map(
                |(before_token, after_token)| PlacementHistoryReceipt::Commit(crate::undo_journal::SubsystemStep {
                    subsystem: crate::undo_journal::SubsystemId::Placement,
                    before_token,
                    after_token,
                }),
            ));
        }
    }
    /// Applies design-panel placements as one undoable group (bounded per call, see
    /// `MAX_OBJECTS_PER_BATCH`). Returns the batch outcome; `consumed` requests may be dropped by
    /// the caller. A core error leaves the session untouched.
    pub fn apply_requests(&mut self, requests: &[PlacementRequest]) -> Result<PlacementBatch> {
        let store = self
            .store
            .as_mut()
            .ok_or("請先載入 MAP（Objects 面板 Load DATA）")?;
        let batch = apply_placements(store, requests)?;
        self.record_edit(batch.ops);
        if batch.ops > 0 {
            self.selected = None;
            self.selection.clear();
            self.pending = None;
        }
        Ok(batch)
    }
    /// Mutable DATA store for panels that edit it directly (Families & Effects).
    pub fn store_mut(&mut self) -> Option<&mut ObjectStore> {
        self.store.as_mut()
    }
    /// Call after a panel edited/undid the store behind this session's back: the grouped
    /// undo/redo bookkeeping no longer matches the store history, so it is reset (the store's own
    /// history stays intact) and selection / pending actions are revalidated.
    pub fn note_external_store_edit(&mut self) {
        self.undo_groups.clear();
        self.redo_groups.clear();
        self.history_barrier();
        self.pending = None;
        self.check_selection();
    }
    /// Supplies objdef names so team edits can apply the per-category limits.
    pub fn set_type_names(&mut self, names: std::collections::HashMap<i32, String>) {
        self.type_names = names;
    }
    /// 使用既有建築分類，合併整個 family 的目前位置。
    pub(crate) fn navigation_buildings(&self, objects: &[WorldObject]) -> Vec<[f32; 4]> {
        use against_rome_maps::{MapDesignValidator, SdlObjectCatalog, SdlObjectCategory};
        objects.iter().filter(|o| o.parent.is_none()).filter_map(|root| {
            let name = self.type_names.get(&i32::from(root.type_id))?;
            if SdlObjectCatalog::categorize(name) != SdlObjectCategory::Building { return None; }
            let margin = against_rome_maps::placement_edit::spawn_data::approximate_family_margin(
                MapDesignValidator::is_main_house(name));
            let mut e = [root.position[0], root.position[0], root.position[2], root.position[2]];
            for object in objects {
                let mut parent = object.parent;
                for _ in 0..objects.len() {
                    let Some(slot) = parent else { break };
                    if slot == root.slot {
                        e[0] = e[0].min(object.position[0]); e[1] = e[1].max(object.position[0]);
                        e[2] = e[2].min(object.position[2]); e[3] = e[3].max(object.position[2]);
                        break;
                    }
                    parent = objects.iter().find(|o| o.slot == slot).and_then(|o| o.parent);
                }
            }
            Some([e[0] - margin, e[1] + margin, e[2] - margin, e[3] + margin])
        }).collect()
    }
    pub fn has_type_names(&self) -> bool {
        !self.type_names.is_empty()
    }
    pub fn type_names(&self) -> &std::collections::HashMap<i32, String> {
        &self.type_names
    }
    /// Per-category team limits and unit-count rule of an object type (C# `ValidateBounds`):
    /// squads (figures) team 0..=7, animals count 0, others team -1..=15.
    pub fn category_limits(&self, type_id: i32) -> Option<String> {
        let name = self.type_names.get(&type_id)?;
        Some(match placement_category(name) {
            PlacementCategory::Squad => "team 0..7, unit count 1..20".to_string(),
            PlacementCategory::Animal => "team -1..15, unit count 0".to_string(),
            PlacementCategory::UnitGroup => "team -1..15, unit count 1..20 (SDL)".to_string(),
            PlacementCategory::Other => "team -1..15".to_string(),
        })
    }
    /// Validates a team value for every selected family root against its category limits.
    /// Types without a known name are checked with the generic -1..15 object rule.
    fn validate_team_for_families(&self, families: &[(usize, u32)], team: i32) -> Result<()> {
        let Some(store) = self.store.as_ref() else { return Ok(()) };
        let objects = store.objects();
        for &(slot, uid) in families {
            let Some(o) = objects.iter().find(|o| o.slot == slot && o.uid == uid) else { continue };
            let Some(name) = self.type_names.get(&(o.type_id as i32)) else { continue };
            validate_team_for_name(name, team).map_err(|e| format!("slot {slot}: {e}"))?;
        }
        Ok(())
    }
    pub fn object_count(&self) -> usize {
        self.store.as_ref().map_or(0, |s| s.objects().len())
    }
    /// Read-only snapshot for the 3D viewport.
    pub fn objects(&self) -> Vec<WorldObject> {
        self.store
            .as_ref()
            .map(ObjectStore::objects)
            .unwrap_or_default()
    }
    pub fn selected_object(&self) -> Option<(usize, u32)> {
        self.selected
    }
    pub fn selected_objects(&self) -> Vec<(usize, u32)> {
        self.selected_families()
    }
    pub fn is_selected(&self, key: (usize, u32)) -> bool {
        self.selection.contains(&key) || self.selected == Some(key)
    }
    /// Selects a family root by slot/UID; stale keys are ignored.
    pub fn select_object(&mut self, key: Option<(usize, u32)>) {
        let valid_key = key.filter(|k| {
            self.store
                .as_ref()
                .is_some_and(|s| s.objects().iter().any(|o| (o.slot, o.uid) == *k))
        });
        self.selected = valid_key;
        self.selection = valid_key.map(|k| vec![k]).unwrap_or_default();
        if matches!(self.pending, Some(Pending::Remove { .. })) {
            self.pending = None;
        }
    }
    /// Sets multi-selection from an explicit list of keys; non-existent keys are filtered out.
    pub fn select_objects(&mut self, keys: &[(usize, u32)]) {
        let valid = self.store.as_ref().map(|s| s.objects()).unwrap_or_default();
        let mut filtered = Vec::new();
        for &k in keys {
            if valid.iter().any(|o| (o.slot, o.uid) == k) && !filtered.contains(&k) {
                filtered.push(k);
            }
        }
        self.selection = filtered;
        self.selected = self.selection.last().copied();
        if matches!(self.pending, Some(Pending::Remove { .. })) {
            self.pending = None;
        }
    }
    /// Toggles selection of an individual object (Ctrl-click behavior).
    pub fn toggle_select_object(&mut self, key: (usize, u32)) {
        let exists = self
            .store
            .as_ref()
            .is_some_and(|s| s.objects().iter().any(|o| (o.slot, o.uid) == key));
        if !exists {
            return;
        }
        if self.selection.is_empty() {
            if let Some(sel) = self.selected {
                self.selection.push(sel);
            }
        }
        if let Some(pos) = self.selection.iter().position(|k| *k == key) {
            self.selection.remove(pos);
            self.selected = self.selection.last().copied();
        } else {
            self.selection.push(key);
            self.selected = Some(key);
        }
        if matches!(self.pending, Some(Pending::Remove { .. })) {
            self.pending = None;
        }
    }
    /// Range selects objects between anchor and target from the given list of objects (Shift-click behavior).
    pub fn select_range_from_visible(&mut self, visible: &[WorldObject], target: (usize, u32)) {
        let target_idx = visible.iter().position(|o| (o.slot, o.uid) == target);
        let anchor = self.selected.or_else(|| self.selection.last().copied());
        let anchor_idx = anchor.and_then(|a| visible.iter().position(|o| (o.slot, o.uid) == a));

        if let (Some(a_idx), Some(t_idx)) = (anchor_idx, target_idx) {
            let start = a_idx.min(t_idx);
            let end = a_idx.max(t_idx);
            let mut new_selection = self.selection.clone();
            for obj in &visible[start..=end] {
                let key = (obj.slot, obj.uid);
                if !new_selection.contains(&key) {
                    new_selection.push(key);
                }
            }
            self.selection = new_selection;
            self.selected = Some(target);
        } else {
            self.select_object(Some(target));
        }
        if matches!(self.pending, Some(Pending::Remove { .. })) {
            self.pending = None;
        }
    }
    /// Box-select result (C# `TerrainRegionOperation.SelectObjects`): replaces the selection, or
    /// adds to it when `additive` (Shift). Returns how many objects the rectangle contains.
    pub fn select_in_tile_rect(&mut self, rect: TileRect, additive: bool) -> usize {
        let hits = select_objects_in_tile_rect(&self.objects(), rect);
        if additive {
            let mut keys = self.selected_objects();
            for key in &hits {
                if !keys.contains(key) {
                    keys.push(*key);
                }
            }
            self.select_objects(&keys);
        } else {
            self.select_objects(&hits);
        }
        hits.len()
    }
    /// Selects all active objects currently in the loaded store.
    pub fn select_all(&mut self) {
        if let Some(store) = &self.store {
            let all: Vec<(usize, u32)> = store.objects().iter().map(|o| (o.slot, o.uid)).collect();
            self.selected = all.last().copied();
            self.selection = all;
            if matches!(self.pending, Some(Pending::Remove { .. })) {
                self.pending = None;
            }
        }
    }
    /// Clears any object selection.
    pub fn clear_selection(&mut self) {
        self.selected = None;
        self.selection.clear();
        if matches!(self.pending, Some(Pending::Remove { .. })) {
            self.pending = None;
        }
    }

    /// 選取全地圖同類型物件；與全選一樣保留 DATA 子物件。
    pub fn select_all_of_type(&mut self) -> usize {
        let objects = self.objects();
        let selected_types: std::collections::HashSet<u16> = self.selected_families()
            .iter().filter_map(|key| objects.iter().find(|o| (o.slot, o.uid) == *key).map(|o| o.type_id)).collect();
        if selected_types.is_empty() { return 0; }
        let matching: Vec<_> = objects.iter().filter(|o| selected_types.contains(&o.type_id))
            .map(|o| (o.slot, o.uid)).collect();
        self.select_objects(&matching);
        matching.len()
    }

    /// 反向選取全地圖 DATA 物件；不修改物件或復原歷史。
    pub fn invert_selection(&mut self) -> usize {
        if self.pending.is_some() || self.selection_tools_blocked { return 0; }
        let selected: std::collections::HashSet<_> = self.selected_objects().into_iter().collect();
        let keys: Vec<_> = self.objects().iter().map(|o| (o.slot, o.uid))
            .filter(|k| !selected.contains(k)).collect();
        self.select_objects(&keys);
        keys.len()
    }

    /// 選取目前所選隊伍的所有 DATA 物件，包含子物件與中立隊伍。
    pub fn select_same_teams(&mut self) -> usize {
        if self.pending.is_some() || self.selection_tools_blocked { return 0; }
        let objects = self.objects();
        let selected: std::collections::HashSet<_> = self.selected_objects().into_iter().collect();
        let teams: std::collections::HashSet<_> = objects.iter()
            .filter(|o| selected.contains(&(o.slot, o.uid))).map(|o| o.team).collect();
        if teams.is_empty() { return 0; }
        let keys: Vec<_> = objects.iter().filter(|o| teams.contains(&o.team))
            .map(|o| (o.slot, o.uid)).collect();
        self.select_objects(&keys);
        keys.len()
    }

    /// 僅在單選時提供座標，避免多選時誤複製最後選取的物件。
    pub fn selected_coordinates(&self) -> Option<(f32, f32)> {
        let keys = self.selected_families();
        if keys.len() != 1 { return None; }
        self.objects().iter().find(|o| (o.slot, o.uid) == keys[0])
            .map(|o| (o.position[0], o.position[2]))
    }
    fn request_load(&mut self) -> Result<()> {
        explicit_map(&self.map_input)?;
        if self.is_dirty() {
            self.pending = Some(Pending::Load(self.map_input.clone()));
            Ok(())
        } else {
            self.load_now(&self.map_input.clone())
        }
    }
    fn request_close(&mut self) {
        if self.is_dirty() {
            self.open = true;
            self.pending = Some(Pending::Close);
        } else {
            self.open = false;
            self.pending = None;
        }
    }
    fn confirm_pending(&mut self) -> Result<()> {
        // Reset failure must retain the confirmation and the entire current session.
        if matches!(self.pending, Some(Pending::Reset)) {
            return self.reset_to_baseline();
        }
        match self.pending.take().ok_or("沒有待確認操作")? {
            Pending::Load(map) => self.load_now(&map),
            Pending::Reset => unreachable!(),
            Pending::Close => {
                // Closing is an explicit discard; reopening starts with a fresh session.
                self.store = None;
                self.baseline = None;
                self.loaded_map = None;
                self.selected = None;
                self.selection.clear();
                self.undo_groups.clear();
                self.redo_groups.clear();
                self.history_barrier();
                self.open = false;
                Ok(())
            }
            Pending::Remove { families } => {
                let current = self.current_families()?;
                if current != families {
                    return Err("選擇已變更；請重新確認移除".into());
                }
                let store = self.store.as_mut().ok_or("請先載入 MAP")?;
                store.remove_families(&families)?;
                self.record_edit(1);
                self.clear_selection();
                Ok(())
            }
        }
    }
    fn selected_families(&self) -> Vec<(usize, u32)> {
        if self.selection.len() > 1 {
            if let Some(key) = self.selected {
                if !self.selection.contains(&key) {
                    return vec![key];
                }
            }
            self.selection.clone()
        } else if let Some(key) = self.selected {
            vec![key]
        } else if !self.selection.is_empty() {
            self.selection.clone()
        } else {
            Vec::new()
        }
    }
    fn current_families(&self) -> Result<Vec<(usize, u32)>> {
        let store = self.store.as_ref().ok_or("請先載入 MAP")?;
        let families = self.selected_families();
        if families.is_empty() {
            return Err("請選擇 family root".into());
        }
        let objects = store.objects();
        for &(slot, uid) in &families {
            if !objects.iter().any(|o| o.slot == slot && o.uid == uid) {
                return Err("stale object UID".into());
            }
        }
        Ok(families)
    }
    pub fn current(&self) -> Result<(usize, u32)> {
        let families = self.current_families()?;
        Ok(families[0])
    }
    pub fn move_family(&mut self) -> Result<()> {
        let families = self.current_families()?;
        let delta = fixed_delta(&self.delta)?;
        if delta == [0, 0, 0] {
            return Ok(());
        }
        self.store
            .as_mut()
            .unwrap()
            .translate_families_fixed(&families, delta)?;
        self.record_edit(1);
        Ok(())
    }
    pub fn clone_family(&mut self) -> Result<()> {
        let families = self.current_families()?;
        let delta = fixed_delta(&self.delta)?;
        let cloned = self
            .store
            .as_mut()
            .unwrap()
            .clone_families_fixed(&families, delta)?;
        self.record_edit(1);
        if !cloned.is_empty() {
            self.selected = cloned.last().copied();
            self.selection = cloned;
        }
        Ok(())
    }
    /// Keyboard nudge (C# `MoveSelectedPlacements`): shifts the selection by whole world units
    /// without touching the panel's delta text fields. One undoable group.
    pub fn nudge_selection(&mut self, dx: f32, dz: f32) -> Result<()> {
        let families = self.current_families()?;
        let fixed = |v: f32| (f64::from(v) * 65536.0).round() as i32;
        let delta = [fixed(dx), 0, fixed(dz)];
        if delta == [0, 0, 0] {
            return Ok(());
        }
        self.store
            .as_mut()
            .unwrap()
            .translate_families_fixed(&families, delta)?;
        self.record_edit(1);
        Ok(())
    }
    /// Keyboard duplicate (C# `DuplicateSelectedPlacedObjects`): copies at +256/+256 and selects the copies.
    pub fn duplicate_selection(&mut self) -> Result<()> {
        let families = self.current_families()?;
        let offset = (256.0f64 * 65536.0) as i32;
        let cloned = self
            .store
            .as_mut()
            .unwrap()
            .clone_families_fixed(&families, [offset, 0, offset])?;
        self.record_edit(1);
        if !cloned.is_empty() {
            self.selected = cloned.last().copied();
            self.selection = cloned;
        }
        Ok(())
    }
    /// Keyboard Delete (C# `DeleteSelectedPlacedObjects`): removes the selection immediately
    /// (undoable) without the panel's confirmation step.
    pub fn delete_selection(&mut self) -> Result<()> {
        let families = self.current_families()?;
        self.store.as_mut().unwrap().remove_families(&families)?;
        self.record_edit(1);
        self.clear_selection();
        Ok(())
    }
    /// `absolute` sets the family heading; otherwise the angle is added to the current heading.
    pub fn rotate_family(&mut self, absolute: bool) -> Result<()> {
        let families = self.current_families()?;
        let degrees = self
            .angle
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|d| d.is_finite())
            .ok_or("角度不是有效數值")?;
        let store = self.store.as_mut().unwrap();
        if absolute {
            store.set_families_rotation(&families, degrees)?;
        } else {
            if degrees % 360.0 == 0.0 {
                return Ok(());
            }
            store.rotate_families(&families, degrees)?;
        }
        self.record_edit(1);
        Ok(())
    }
    /// Team 0..15, or -1 / 65535 for unassigned (as in the C# batch-edit dialog).
    pub fn set_team(&mut self) -> Result<()> {
        let families = self.current_families()?;
        let text = self.team.trim();
        let team = if text == "-1" {
            65535
        } else {
            text.parse::<u16>().map_err(|_| "隊伍必須是 0..15 或 -1")?
        };
        self.validate_team_for_families(&families, if team == 65535 { -1 } else { i32::from(team) })?;
        self.store
            .as_mut()
            .unwrap()
            .set_families_team(&families, team)?;
        self.record_edit(1);
        Ok(())
    }
    pub fn snap_to_grid(&mut self) -> Result<()> {
        let families = self.current_families()?;
        let delta = self
            .store
            .as_mut()
            .unwrap()
            .snap_families_to_grid(&families)?;
        // C# SnapManyToGrid delegates to MoveMany, which ignores zero offsets.
        // The core also skips that command: do not invent a UI history entry
        // that could undo an older command across the saved baseline.
        if delta != [0.0; 3] {
            self.record_edit(1);
        }
        Ok(())
    }
    /// Batch edits team and/or rotation for the selected families in a single atomic transaction.
    pub fn batch_edit(&mut self, team: Option<u16>, degrees: Option<f32>) -> Result<()> {
        let families = self.current_families()?;
        if team.is_none() && degrees.is_none() {
            return Ok(());
        }
        self.store
            .as_mut()
            .unwrap()
            .batch_edit_families(&families, team, degrees)?;
        self.record_edit(1);
        Ok(())
    }
    /// Applies batch edit using UI field inputs for team and angle.
    pub fn apply_batch_edit_fields(&mut self) -> Result<()> {
        let families = self.current_families()?;
        let team_parsed = {
            let t = self.team.trim();
            if t.is_empty() {
                None
            } else if t == "-1" {
                Some(65535u16)
            } else {
                Some(t.parse::<u16>().map_err(|_| "隊伍必須是 0..15 或 -1")?)
            }
        };
        let angle_parsed = {
            let a = self.angle.trim();
            if a.is_empty() {
                None
            } else {
                Some(
                    a.parse::<f32>()
                        .ok()
                        .filter(|d| d.is_finite())
                        .ok_or("角度不是有效數值")?,
                )
            }
        };
        if team_parsed.is_none() && angle_parsed.is_none() {
            return Err("請輸入要批次設定的隊伍或角度".into());
        }
        if let Some(team) = team_parsed {
            self.validate_team_for_families(&families, if team == 65535 { -1 } else { i32::from(team) })?;
        }
        self.store
            .as_mut()
            .unwrap()
            .batch_edit_families(&families, team_parsed, angle_parsed)?;
        self.record_edit(1);
        Ok(())
    }
    pub fn request_remove(&mut self) -> Result<()> {
        let families = self.current_families()?;
        let store = self.store.as_ref().unwrap();
        for &(slot, uid) in &families {
            store.family(slot, uid)?;
        }
        self.pending = Some(Pending::Remove { families });
        Ok(())
    }
    pub fn undo(&mut self) -> Result<()> {
        let store = self.store.as_mut().ok_or("請先載入 MAP")?;
        let group = self.undo_groups.pop().ok_or("沒有可復原操作")?;
        let mut done = 0;
        for _ in 0..group {
            if !store.undo() {
                break;
            }
            done += 1;
        }
        if done == 0 {
            return Err("沒有可復原操作".into());
        }
        self.redo_groups.push(done);
        if done == group { self.history.undo(); } else { self.history_barrier(); }
        self.history_payloads = history_payloads(self.store.as_ref());
        self.check_selection();
        self.pending = None;
        Ok(())
    }
    pub fn redo(&mut self) -> Result<()> {
        let store = self.store.as_mut().ok_or("請先載入 MAP")?;
        let group = self.redo_groups.pop().ok_or("沒有可重做操作")?;
        let mut done = 0;
        for _ in 0..group {
            if !store.redo() {
                break;
            }
            done += 1;
        }
        if done == 0 {
            return Err("沒有可重做操作".into());
        }
        self.undo_groups.push(done);
        if done == group { self.history.redo(); } else { self.history_barrier(); }
        self.history_payloads = history_payloads(self.store.as_ref());
        self.check_selection();
        self.pending = None;
        Ok(())
    }
    fn check_selection(&mut self) {
        if let Some(store) = &self.store {
            let valid = store.objects();
            self.selection
                .retain(|k| valid.iter().any(|o| (o.slot, o.uid) == *k));
            if let Some(key) = self.selected {
                if !valid.iter().any(|o| (o.slot, o.uid) == key) {
                    self.selected = self.selection.last().copied();
                }
            } else {
                self.selected = self.selection.last().copied();
            }
        } else {
            self.selected = None;
            self.selection.clear();
        }
    }
    /// Legacy direct writer; the product UI saves only through `EditorApp` Safe Save.
    #[cfg(test)]
    fn save(&mut self) -> Result<()> {
        let path = self.loaded_map.as_ref().ok_or("請先載入 MAP")?;
        allow_save(path)?;
        self.store.as_mut().ok_or("請先載入 MAP")?.save()?;
        self.baseline = self.store.clone();
        // Match PlacementEditSession.AcceptSavedChanges: successful Save establishes
        // a new baseline. ObjectStore retains its raw history, so the UI command
        // groups are the authoritative boundary for both undo/redo and buttons.
        self.undo_groups.clear();
        self.redo_groups.clear();
        self.history_barrier();
        self.pending = None;
        Ok(())
    }
    // ---------------------------------------------------------------- layouts
    pub fn layout_status(&self) -> &str {
        &self.layout_status
    }
    pub fn has_pending_layout(&self) -> bool {
        self.layout_pending.is_some()
    }
    pub fn layout_options(&self) -> LayoutApplyOptions {
        self.layout_options
    }
    pub fn set_layout_options(&mut self, options: LayoutApplyOptions) {
        self.layout_options = options;
        self.layout_team_override = options.team.is_some();
        if let Some(team) = options.team {
            self.layout_team = team;
        }
    }
    /// Queues a layout action; it runs on the next `process_layout_requests`.
    pub fn request_layout(&mut self, request: LayoutRequest) {
        self.layout_queue.push(request);
    }
    pub fn has_layout_requests(&self) -> bool {
        !self.layout_queue.is_empty()
    }
    fn pick_layout_file(&self, kind: &LayoutDialogKind) -> Option<PathBuf> {
        match &self.layout_picker {
            Some(picker) => picker(kind),
            None => layout_io::native_layout_picker()(kind),
        }
    }
    /// "Import layout": pick a file, validate it, then open the apply dialog.
    pub fn begin_layout_import(&mut self) {
        let Some(path) = self.pick_layout_file(&LayoutDialogKind::Open) else {
            return;
        };
        self.load_layout_from(&path);
    }
    /// Reads and validates `path`; on success the apply dialog opens (C# `RunLayoutApply`).
    pub fn load_layout_from(&mut self, path: &Path) {
        match layout_io::read_layout_file(path) {
            Ok(preset) => {
                self.layout_status = format!(
                    "已載入配置：{:?}，{} 個物件 / Loaded layout ({:?}, {} objects)",
                    preset.kind,
                    preset.entries.len(),
                    preset.kind,
                    preset.entries.len()
                );
                self.layout_pending = Some(preset);
            }
            Err(error) => {
                self.layout_pending = None;
                self.layout_status = format!("配置操作失敗 / Layout failed: {error}");
            }
        }
    }
    /// Confirms the apply dialog: validates the options and queues the apply.
    pub fn confirm_layout_apply(&mut self) -> Result<()> {
        let mut options = self.layout_options;
        let placement = self
            .layout_pending
            .as_ref()
            .is_some_and(|p| p.kind == MapLayoutKind::Placement);
        options.team = (placement && self.layout_team_override).then_some(self.layout_team);
        options.validate()?;
        let preset = self.layout_pending.take().ok_or("沒有待套用的配置")?;
        self.layout_options = options;
        self.layout_queue.push(LayoutRequest::Apply { preset, options });
        Ok(())
    }
    pub fn cancel_layout_apply(&mut self) {
        self.layout_pending = None;
    }
    /// Consumes all requests in bounded batches on a candidate store, then publishes one
    /// undoable group. Any batch failure leaves the entire live session unchanged.
    pub fn apply_requests_grouped(
        &mut self,
        requests: &[PlacementRequest],
    ) -> Result<PlacementBatch> {
        let mut candidate = self
            .store
            .as_ref()
            .ok_or("請先載入 MAP（Objects 面板 Load DATA）")?
            .clone();
        let mut total = PlacementBatch::default();
        for chunk in requests.chunks(crate::design_apply::MAX_OBJECTS_PER_BATCH) {
            let batch = apply_placements(&mut candidate, chunk)?;
            total.consumed += batch.consumed;
            total.ops += batch.ops;
            total.placed += batch.placed;
            total.skipped.extend(batch.skipped);
        }
        if total.ops > 0 {
            self.store = Some(candidate);
            self.record_edit(total.ops);
            self.selected = None;
            self.selection.clear();
            self.pending = None;
        }
        Ok(total)
    }
    /// Runs queued layout exports/applies (called by `EditorApp` with terrain + objdef context).
    pub fn process_layout_requests(&mut self, ctx: &LayoutContext<'_>) {
        for request in std::mem::take(&mut self.layout_queue) {
            let result = match request {
                LayoutRequest::ExportSelection => self.run_export_selection(ctx),
                LayoutRequest::ExportNature(rect) => self.run_export_nature(rect, ctx),
                LayoutRequest::Apply { preset, options } => self.run_apply_layout(&preset, &options, ctx),
            };
            self.layout_status = match result {
                Ok(message) => message,
                Err(error) => format!("配置操作失敗 / Layout failed: {error}"),
            };
        }
    }
    fn save_layout(&self, preset: &MapLayoutPreset) -> Result<String> {
        let kind = LayoutDialogKind::Save {
            suggested_name: layout_io::suggested_file_name(preset.kind),
        };
        let Some(path) = self.pick_layout_file(&kind) else {
            return Ok("已取消保存配置 / Layout save cancelled".into());
        };
        let written = layout_io::write_layout_file(&path, preset)?;
        Ok(format!(
            "已保存配置，共 {} 個物件 / Saved {} layout objects: {}",
            preset.entries.len(),
            preset.entries.len(),
            written.display()
        ))
    }
    fn run_export_selection(&mut self, ctx: &LayoutContext<'_>) -> Result<String> {
        if !self.is_loaded() {
            return Err("請先載入 MAP".into());
        }
        let preset = layout_io::capture_placement(
            &self.objects(),
            &self.selected_objects(),
            &|id| ctx.name_of(id),
            ctx.ground,
        )?;
        self.save_layout(&preset)
    }
    fn run_export_nature(&mut self, rect: TileRect, ctx: &LayoutContext<'_>) -> Result<String> {
        if !self.is_loaded() {
            return Err("請先載入 MAP".into());
        }
        let preset = layout_io::capture_nature(&self.objects(), rect, &|id| ctx.name_of(id), ctx.ground)?;
        self.save_layout(&preset)
    }
    fn run_apply_layout(
        &mut self,
        preset: &MapLayoutPreset,
        options: &LayoutApplyOptions,
        ctx: &LayoutContext<'_>,
    ) -> Result<String> {
        if !self.is_loaded() {
            return Err("請先載入 MAP".into());
        }
        let requests = layout_io::plan_requests(preset, &|name| ctx.resolve(name), options, ctx.ground)?;
        let before: Vec<(usize, u32)> = self.objects().iter().map(|o| (o.slot, o.uid)).collect();
        let batch = self.apply_requests_grouped(&requests)?;
        if batch.placed == 0 && !batch.skipped.is_empty() {
            return Err(batch.skipped.join("; "));
        }
        // Select what was just placed (C# selects the new rows).
        let fresh: Vec<(usize, u32)> = self
            .objects()
            .iter()
            .map(|o| (o.slot, o.uid))
            .filter(|key| !before.contains(key))
            .collect();
        self.select_objects(&fresh);
        let note = if batch.skipped.is_empty() {
            String::new()
        } else {
            format!("（略過：{}）", batch.skipped.join("; "))
        };
        Ok(format!(
            "已套用配置：{} 個物件（可 Undo）{note} / Applied {} layout objects ({:?})",
            batch.placed, batch.placed, preset.kind
        ))
    }
    fn layout_section(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.label(tl(self.english, "配置 (.arm-layout.json)", "Layout (.arm-layout.json)"));
            let has_selection = !self.selected_families().is_empty();
            if ui
                .add_enabled(
                    self.is_loaded() && has_selection,
                    egui::Button::new(tl(self.english, "儲存選取的聚落／物件…", "Export selection…")),
                )
                .clicked()
            {
                self.request_layout(LayoutRequest::ExportSelection);
            }
            if ui
                .add_enabled(self.is_loaded(), egui::Button::new(tl(self.english, "載入並套用配置…", "Import layout…")))
                .clicked()
            {
                self.begin_layout_import();
            }
        });
        if !self.layout_status.is_empty() {
            ui.small(&self.layout_status);
        }
    }
    fn show_layout_apply(&mut self, ctx: &egui::Context) {
        let Some(preset) = self.layout_pending.clone() else {
            return;
        };
        let mut confirm = false;
        let mut cancel = false;
        let mut open = true;
        egui::Window::new(tl(self.english, "套用配置", "Apply layout"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!(
                    "{:?}：{} 個物件。錨點使用圖格座標；繞保存的原點旋轉，高度貼合地形。類型缺失或位置越界時整批拒絕。",
                    preset.kind,
                    preset.entries.len()
                ));
                ui.label(tl(self.english, "錨點使用圖格座標；繞已儲存原點旋轉並貼合地形；類型或位置無效時拒絕整批套用。", "Anchor is in tile units; rotates about the saved origin; heights follow terrain; the whole layout is rejected if any type or position is invalid."));
                ui.horizontal(|ui| {
                    ui.label(tl(self.english, "錨點 X", "Anchor X"));
                    ui.add(egui::DragValue::new(&mut self.layout_options.anchor_tile_x).clamp_range(0.0..=63.99).speed(0.1));
                    ui.label(tl(self.english, "錨點 Z", "Anchor Z"));
                    ui.add(egui::DragValue::new(&mut self.layout_options.anchor_tile_z).clamp_range(0.0..=63.99).speed(0.1));
                });
                ui.horizontal(|ui| {
                    ui.label(tl(self.english, "旋轉（度）", "Rotation (degrees)"));
                    ui.add(egui::DragValue::new(&mut self.layout_options.rotation).clamp_range(0.0..=359.0).speed(45.0));
                });
                let placement = preset.kind == MapLayoutKind::Placement;
                ui.horizontal(|ui| {
                    ui.add_enabled(placement, egui::Checkbox::new(&mut self.layout_team_override, tl(self.english, "指定隊伍", "Override team")));
                    ui.add_enabled(
                        placement && self.layout_team_override,
                        egui::DragValue::new(&mut self.layout_team).clamp_range(-1..=15),
                    );
                });
                ui.horizontal(|ui| {
                    if ui.button(tl(self.english, "套用", "Apply")).clicked() {
                        confirm = true;
                    }
                    if ui.button(tl(self.english, "取消", "Cancel")).clicked() {
                        cancel = true;
                    }
                });
            });
        if confirm {
            if let Err(error) = self.confirm_layout_apply() {
                self.layout_status = format!("配置操作失敗 / Layout failed: {error}");
            }
        } else if cancel || !open {
            self.cancel_layout_apply();
        }
    }
    fn report(&mut self, result: Result<()>) {
        self.status = match result {
            Ok(()) => "完成（DATA 保存與 terrain 保存獨立）".into(),
            Err(e) => e,
        };
    }
    fn visible(&self, objects: Vec<WorldObject>) -> Vec<WorldObject> {
        let filter = self.filter.trim().to_lowercase();
        objects
            .into_iter()
            .filter(|o| {
                filter.is_empty()
                    || format!(
                        "slot={} uid={} type={} team={} parent={:?}",
                        o.slot, o.uid, o.type_id, o.team, o.parent
                    )
                    .to_lowercase()
                    .contains(&filter)
            })
            .collect()
    }
    fn select_tile_bounds(&mut self) -> Result<()> {
        let mut bounds = [0; 4];
        for (index, text) in self.tile_bounds.iter().enumerate() {
            bounds[index] = text.trim().parse::<i32>().map_err(|_| {
                format!(
                    "矩形選取：{} 必須是 i32 整數",
                    ["left", "top", "right", "bottom"][index]
                )
            })?;
        }
        let [left, top, right, bottom] = bounds;
        if left >= right || top >= bottom {
            return Err("矩形選取：範圍不可為空，須 left < right 且 top < bottom".into());
        }
        let store = self.store.as_ref().ok_or("矩形選取：請先載入 MAP")?;
        let keys = select_objects_in_tile_rect(
            &store.objects(),
            TileRect {
                left,
                top,
                right,
                bottom,
            },
        );
        self.select_objects(&keys);
        Ok(())
    }

    fn tile_bounds_form(&mut self, ui: &mut egui::Ui) {
        ui.label(tl(self.english, "矩形選取圖格：[左, 右) × [上, 下)，世界座標 X/Z，每格 256；取代選取（忽略篩選）", "Rectangle tiles: [left, right) x [top, bottom), world X/Z, 256 per tile; replaces selection (ignores filter)"));
        ui.horizontal_wrapped(|ui| {
            for (index, name) in ["left", "top", "right", "bottom"].iter().enumerate() {
                ui.label(if self.english { *name } else { ["左", "上", "右", "下"][index] });
                ui.add(
                    egui::TextEdit::singleline(&mut self.tile_bounds[index])
                        .id_source(("placement-tile-bound", index))
                        .desired_width(60.0),
                );
            }
            if ui
                .add_enabled(self.is_loaded(), egui::Button::new(tl(self.english, "選取矩形範圍", "Select rectangle")))
                .clicked()
            {
                self.status = match self.select_tile_bounds() {
                    Ok(()) => format!("矩形選取：已選取 {} 個物件", self.selection.len()),
                    Err(error) => error,
                };
            }
        });
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        let mut visible = self.open;
        egui::Window::new(tl(self.english, "物件與關聯群組工具", "Rust 物件 / Family 工具")).open(&mut visible)
            .default_width(760.0).show(ctx, |ui| {
            ui.label(tl(self.english, "DATA 獨立編輯階段：此處儲存僅保存 DATA 物件與關聯池，不保存地形、材質或劇本。", "Independent DATA session: saves objects and related pools only; terrain, materials and scenarios are saved separately."));
            ui.label("只編輯明確指定的 MAP；安裝目錄允許唯讀載入，保存請使用工作副本。");
            ui.label("MAP 完整路徑（包含 DATA 的資料夾）");
            ui.text_edit_singleline(&mut self.map_input);
            ui.horizontal(|ui| {
                if ui.button(tl(self.english, "載入 DATA", "Load DATA")).clicked() { let r = self.request_load(); self.report(r); }
                if ui.button(tl(self.english, "關閉工具", "Close tool")).clicked() { self.request_close(); }
            });
            if let Some(path) = &self.loaded_map {
                ui.label(format!("實際載入：{} {}", path.display(), if self.is_dirty() {"（未保存）"} else {"（已保存）"}));
            }
            if let Some(store) = &self.store {
                ui.label(format!("{} {} / {} {}", tl(self.english, "有效物件：", "Active objects:"), store.objects().len(), tl(self.english, "容量：", "Capacity:"), store.capacity()));
            }
            ui.horizontal(|ui| {
                ui.label(tl(self.english, "篩選槽位／UID／類型／隊伍／父物件", "Filter slot / UID / type / team / parent"));
                ui.text_edit_singleline(&mut self.filter);
                if ui.button(tl(self.english, "全選", "Select all")).clicked() {
                    self.select_all();
                }
                if ui.add_enabled(!self.selection.is_empty(), egui::Button::new(tl(self.english, "選取同類型", "Select type / 選取同類型")))
                    .on_hover_text(tl(self.english, "選取所有與目前選取物件具相同類型的物件", "Select all objects of same type"))
                    .clicked()
                {
                    let n = self.select_all_of_type();
                    self.status = format!("已選取 {n} 個同類型物件 / Selected {n} objects of same type");
                }
                if ui.button(tl(self.english, "清除選取", "Clear selection")).clicked() {
                    self.clear_selection();
                }
            });
            self.tile_bounds_form(ui);
            ui.horizontal(|ui| {
                if ui.add_enabled(self.pending.is_none() && !self.selection_tools_blocked, egui::Button::new(tl(self.english, "反向選取", "Invert selection"))).clicked() {
                    self.invert_selection();
                }
                if ui.add_enabled(self.pending.is_none() && !self.selection_tools_blocked && !self.selected_objects().is_empty(), egui::Button::new(tl(self.english, "選取同隊伍", "Select same teams"))).clicked() {
                    self.select_same_teams();
                }
                ui.small(tl(self.english, "全地圖，包含隱藏項目", "Whole map, including filtered objects"));
            });
            let objects = self.visible(self.store.as_ref().map(ObjectStore::objects).unwrap_or_default());
            let selected_count = self.selected_families().len();
            if selected_count > 1 {
                ui.colored_label(
                    egui::Color32::from_rgb(100, 200, 255),
                    format!("已多選 {selected_count} 個物件（按住 Ctrl 點選切換、按住 Shift 點選範圍）"),
                );
            }
            egui::ScrollArea::vertical().id_source("placement-list").max_height(250.0).show(ui, |ui| {
                for o in &objects {
                    let key = (o.slot, o.uid);
                    let is_sel = self.is_selected(key);
                    let label = format!("slot={} UID={} type={} team={} XYZ=({:.4}, {:.4}, {:.4}) parent={:?}",
                        o.slot, o.uid, o.type_id, o.team, o.position[0], o.position[1], o.position[2], o.parent);
                    let resp = ui.selectable_label(is_sel, label);
                    if resp.clicked() {
                        let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
                        let shift = ui.input(|i| i.modifiers.shift);
                        if ctrl {
                            self.toggle_select_object(key);
                        } else if shift {
                            self.select_range_from_visible(&objects, key);
                        } else {
                            self.select_object(Some(key));
                        }
                    }
                }
            });
            if let Some(o) = self.selected.and_then(|k| objects.iter().find(|o| (o.slot, o.uid) == k)) {
                ui.horizontal(|ui| {
                    if let Some(limits) = self.category_limits(o.type_id as i32) {
                        ui.label(format!("類別限制：{limits}（單位數量存於 SDL，不在 DATA）"));
                    }
                    if ui.add_enabled(self.selected_coordinates().is_some(), egui::Button::new(tl(self.english, "複製 X/Z", "Copy X/Z / 複製 X/Z")))
                        .on_hover_text(tl(self.english, "複製選取物件的世界座標 (X, Z) 至剪貼簿", "Copy world X and Z coordinates of the selected object to clipboard"))
                        .clicked()
                    {
                        let text = format!("{:.1}, {:.1}", o.position[0], o.position[2]);
                        ctx.copy_text(text.clone());
                        self.status = format!("已複製 X/Z 座標至剪貼簿：{text} / Copied X/Z to clipboard: {text}");
                    }
                });
            }
            if let Some(store) = &self.store {
                let families = self.selected_families();
                if families.len() == 1 {
                    let (slot, uid) = families[0];
                    match store.family(slot, uid) {
                        Ok(family) => { ui.label(format!("整個 family slots: {family:?}")); }
                        Err(error) => { ui.colored_label(egui::Color32::YELLOW, error); }
                    }
                } else if families.len() > 1 {
                    ui.label(format!("多選中的 family roots: {families:?}"));
                }
            }
            ui.label(tl(self.english, "移動／複製的相對世界位移；X/Y/Z → 帶正負號的 16.16（四捨五入至 1/65536）。", "Move / Clone 的相對 world 位移；X/Y/Z → signed 16.16（四捨五入至 1/65536）。"));
            ui.horizontal(|ui| {for axis in 0..3 {ui.label(["ΔX","ΔY","ΔZ"][axis]);ui.add(egui::TextEdit::singleline(&mut self.delta[axis]).desired_width(95.0));}});
            if let Ok(delta)=fixed_delta(&self.delta) {ui.label(format!("{} {delta:?}", tl(self.english, "16.16 位移：", "16.16 delta:")));}
            ui.label("地圖外座標沿用原有 family 契約，不自動 clamp。未知／不支援關聯由 core 拒絕，錯誤原樣顯示。");
            ui.horizontal(|ui| {
                ui.label("角度（度）");ui.add(egui::TextEdit::singleline(&mut self.angle).desired_width(70.0));
                if ui.button(tl(self.english, "設定旋轉角度", "Set rotation")).clicked() {let r=self.rotate_family(true);self.report(r);}
                if ui.button(tl(self.english, "增加旋轉角度", "Rotate by")).clicked() {let r=self.rotate_family(false);self.report(r);}
                ui.label("隊伍 0..15 / -1");ui.add(egui::TextEdit::singleline(&mut self.team).desired_width(40.0));
                if ui.button(tl(self.english, "設定隊伍", "Set team")).clicked() {let r=self.set_team();self.report(r);}
                if ui.button(tl(self.english, "對齊圖格", "Snap to tile")).clicked() {let r=self.snap_to_grid();self.report(r);}
            });
            ui.horizontal_wrapped(|ui| {
                let count = self.selected_families().len();
                let move_label = if count > 1 { format!("{} {count} {}", tl(self.english, "移動", "Move"), tl(self.english, "個關聯群組", "families")) } else { tl(self.english, "移動關聯群組", "Move family").into() };
                let clone_label = if count > 1 { format!("{} {count} {}", tl(self.english, "複製", "Clone"), tl(self.english, "個關聯群組", "families")) } else { tl(self.english, "複製關聯群組", "Clone family").into() };
                let remove_label = if count > 1 { format!("{} {count} {}", tl(self.english, "移除", "Remove"), tl(self.english, "個關聯群組", "families")) } else { tl(self.english, "移除關聯群組…", "Remove family…").into() };

                if ui.button(move_label).clicked() {let r=self.move_family();self.report(r);}
                if ui.button(clone_label).clicked() {let r=self.clone_family();self.report(r);}
                if ui.button(remove_label).clicked() {let r=self.request_remove();self.report(r);}
                if count > 1 {
                    if ui.button(tl(self.english, "批次編輯隊伍與旋轉角度", "Batch edit team & rotation")).clicked() {
                        let r = self.apply_batch_edit_fields();
                        self.report(r);
                    }
                }
                let can_undo=!self.undo_groups.is_empty();
                let can_redo=!self.redo_groups.is_empty();
                if ui.add_enabled(can_undo,egui::Button::new(tl(self.english, "復原", "Undo"))).clicked() {
                    self.history_request = Some(false);
                }
                if ui.add_enabled(can_redo,egui::Button::new(tl(self.english, "重做", "Redo"))).clicked() {
                    self.history_request = Some(true);
                }
                if ui.add_enabled(self.is_dirty(),egui::Button::new(tl(self.english, "儲存 DATA", "Save DATA"))).clicked() {self.request_save();}
                if ui.add_enabled(self.is_loaded(),egui::Button::new(tl(self.english, "還原 DATA…", "Reset DATA…"))).clicked() {let r=self.request_reset();self.report(r);}
            });
            self.layout_section(ui);
            ui.separator();ui.label(&self.status);
        });
        self.show_layout_apply(ctx);
        // The title-bar close button follows the same dirty guard as the explicit Close tool.
        if self.open && !visible {
            self.request_close();
        } else if self.open {
            self.open = visible;
        }
        if let Some(pending) = self.pending.clone() {
            egui::Window::new("確認物件操作").collapsible(false).resizable(false).show(ctx,|ui| {
                let text=match &pending {
                    Pending::Load(path)=>format!("DATA 尚未保存。捨棄目前物件修改並載入：\n{path}？"),
                    Pending::Close=>"DATA 尚未保存。捨棄目前物件修改並關閉？".into(),
                    Pending::Reset=>"捨棄目前 DATA 物件修改並還原至上次載入／保存的基準？\n不重新讀取磁碟；Undo／Redo 歷史將清除，無法撤銷此還原。".into(),
                    Pending::Remove { families } => {
                        if families.len() == 1 {
                            let (slot, uid) = families[0];
                            format!("移除 slot={slot} UID={uid} 的完整 family？\n只先變更記憶體，可 Undo；Save DATA 後才寫入工作副本。")
                        } else {
                            format!("移除選取的 {} 個完整 family？\n只先變更記憶體，可 Undo；Save DATA 後才寫入工作副本。", families.len())
                        }
                    }
                };
                ui.label(text);
                ui.horizontal(|ui| {
                    if ui.button("取消／繼續編輯").clicked(){self.pending=None;self.open=true;}
                    if ui.button("確認").clicked(){let r=self.confirm_pending();self.report(r);}
                });
            });
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacementCategory {
    Squad,
    Animal,
    UnitGroup,
    Other,
}

/// Classifies an objdef name like the C# SDL catalog (`FigTie*` = animal, other `Fig*` = squad).
pub fn placement_category(name_def: &str) -> PlacementCategory {
    use against_rome_maps::{SdlObjectCatalog, SdlObjectCategory};
    match SdlObjectCatalog::categorize(name_def) {
        SdlObjectCategory::Figure if name_def.to_ascii_lowercase().starts_with("figtie") => PlacementCategory::Animal,
        SdlObjectCategory::Figure => PlacementCategory::Squad,
        SdlObjectCategory::UnitGroup => PlacementCategory::UnitGroup,
        _ => PlacementCategory::Other,
    }
}

/// Runs the C# `ValidateBounds` team rule for an object of this type via the maps crate.
/// Position/angle are neutral because only team and unit count are being checked.
pub fn validate_team_for_name(name_def: &str, team: i32) -> std::result::Result<(), String> {
    use against_rome_maps::{placement_edit::validate_bounds, SdlObjectCategory, SdlObjectType, SdlPlacedObject};
    let category = against_rome_maps::SdlObjectCatalog::categorize(name_def);
    let unit_count = i32::from(category == SdlObjectCategory::Figure && placement_category(name_def) == PlacementCategory::Squad);
    let obj_type = SdlObjectType::new(name_def, 0, category, "", 0, Default::default());
    validate_bounds(&SdlPlacedObject::new(obj_type, 0.0, 0.0, 0.0, team, 0.0, unit_count), true)
}

/// Validates a squad edit (team, unit count) before it is written to an SDL placement, using the
/// maps crate's `edit_placement_values` rules. Returns the accepted `(team, unit_count)`.
pub fn validate_squad_values(name_def: &str, team: i32, unit_count: i32) -> std::result::Result<(i32, i32), String> {
    use against_rome_maps::{placement_edit::edit_placement_values, SdlObjectCatalog, SdlObjectType, SdlPlacedObject};
    let category = SdlObjectCatalog::categorize(name_def);
    let obj_type = SdlObjectType::new(name_def, 0, category, "", 0, Default::default());
    let before = SdlPlacedObject::new(obj_type, 0.0, 0.0, 0.0, team, 0.0, unit_count.max(1));
    let edited = edit_placement_values(&before, team, 0.0, 0.0, 0.0, 0.0, unit_count)?;
    Ok((edited.team, edited.unit_count))
}

/// Half-open tile rectangle: `left <= x < right`, `top <= y < bottom`.
/// World X maps to tile X and world Z to tile Y (256 world units per tile).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Returns `(slot, uid)` keys, in input order, of objects whose tile position lies
/// inside `rect` (rectangle only; matches C# `TerrainRegionOperation.SelectObjects`).
pub fn select_objects_in_tile_rect(objects: &[WorldObject], rect: TileRect) -> Vec<(usize, u32)> {
    objects
        .iter()
        .filter(|o| {
            let x = o.position[0] / 256.0;
            let y = o.position[2] / 256.0;
            x >= rect.left as f32
                && x < rect.right as f32
                && y >= rect.top as f32
                && y < rect.bottom as f32
        })
        .map(|o| (o.slot, o.uid))
        .collect()
}

/// Normalized (0..1) map position to a tile index, clamped to the map.
pub fn norm_to_tile(norm: f32, dimension: i32) -> i32 {
    if !norm.is_finite() || dimension <= 0 {
        return 0;
    }
    ((norm * dimension as f32).floor() as i32).clamp(0, dimension - 1)
}

/// Box-select rectangle from two screen points over the displayed map, inclusive of the tile under
/// each corner like C# `Rectangle.FromLTRB(min, min, max + 1, max + 1)`.
/// `map_min`/`map_size` describe the on-screen map image.
pub fn screen_box_to_tile_rect(
    a: (f32, f32),
    b: (f32, f32),
    map_min: (f32, f32),
    map_size: (f32, f32),
    dimension: i32,
) -> TileRect {
    let tile = |p: (f32, f32)| {
        (
            norm_to_tile((p.0 - map_min.0) / map_size.0, dimension),
            norm_to_tile((p.1 - map_min.1) / map_size.1, dimension),
        )
    };
    let (ta, tb) = (tile(a), tile(b));
    TileRect {
        left: ta.0.min(tb.0),
        top: ta.1.min(tb.1),
        right: ta.0.max(tb.0) + 1,
        bottom: ta.1.max(tb.1) + 1,
    }
}

#[cfg(test)]
mod tests {
    fn world_obj(slot: usize, uid: u32, x: f32, z: f32) -> WorldObject {
        WorldObject {
            slot,
            type_id: 0,
            team: 0,
            uid,
            position: [x, 99.0, z],
            rotation_degrees: 0.0,
            parent: None,
            vitals: None,
        }
    }

    #[test]
    fn tile_rect_selection_is_half_open() {
        let rect = TileRect {
            left: 2,
            top: 3,
            right: 4,
            bottom: 5,
        };
        let objs = vec![
            world_obj(0, 10, 2.0 * 256.0, 3.0 * 256.0), // top-left corner: in
            world_obj(1, 11, 4.0 * 256.0, 3.0 * 256.0), // x == right: out
            world_obj(2, 12, 2.0 * 256.0, 5.0 * 256.0), // z == bottom: out
            world_obj(3, 13, 4.0 * 256.0 - 0.01, 5.0 * 256.0 - 0.01), // just inside
            world_obj(4, 14, 2.0 * 256.0 - 0.01, 3.0 * 256.0), // just left: out
            world_obj(5, 15, 2.0 * 256.0, 3.0 * 256.0 - 0.01), // just above: out
        ];
        assert_eq!(
            select_objects_in_tile_rect(&objs, rect),
            vec![(0, 10), (3, 13)]
        );
    }

    #[test]
    fn tile_rect_selection_maps_x_to_x_and_z_to_y_on_non_square_rect() {
        // 1 tile wide (x=1), 3 tiles tall (y=0..3).
        let rect = TileRect {
            left: 1,
            top: 0,
            right: 2,
            bottom: 3,
        };
        let objs = vec![
            world_obj(0, 1, 1.5 * 256.0, 2.5 * 256.0), // in
            world_obj(1, 2, 2.5 * 256.0, 1.5 * 256.0), // swapped axes would be out of x range
            world_obj(2, 3, 1.5 * 256.0, 3.5 * 256.0), // z beyond bottom
            world_obj(3, 4, 0.5 * 256.0, 1.5 * 256.0), // x left of rect
        ];
        assert_eq!(select_objects_in_tile_rect(&objs, rect), vec![(0, 1)]);
        // Height (Y) must be ignored.
        let mut high = world_obj(9, 9, 1.5 * 256.0, 0.5 * 256.0);
        high.position[1] = 1.0e6;
        assert_eq!(select_objects_in_tile_rect(&[high], rect), vec![(9, 9)]);
    }

    #[test]
    fn tile_rect_selection_preserves_input_order_and_handles_empty() {
        let rect = TileRect {
            left: 0,
            top: 0,
            right: 2,
            bottom: 2,
        };
        let objs = vec![
            world_obj(7, 70, 300.0, 300.0),
            world_obj(2, 20, 10.0, 10.0),
            world_obj(5, 50, 9999.0, 10.0),
            world_obj(2, 21, 10.0, 500.0),
        ];
        assert_eq!(
            select_objects_in_tile_rect(&objs, rect),
            vec![(7, 70), (2, 20), (2, 21)]
        );
        let empty = TileRect {
            left: 1,
            top: 1,
            right: 1,
            bottom: 1,
        };
        assert!(select_objects_in_tile_rect(&objs, empty).is_empty());
        assert!(select_objects_in_tile_rect(&[], rect).is_empty());
    }

    #[test]
    fn save_request_lifecycle_is_take_once() {
        let mut ui = PlacementUi::default();
        assert!(!ui.take_save_request());
        ui.request_save();
        ui.request_save();
        assert!(ui.take_save_request());
        assert!(!ui.take_save_request());
        assert!(!ui.is_dirty());
    }

    fn rectangle_frame(
        panel: &mut PlacementUi,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1600.0, 1200.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| panel.show(ctx),
        )
    }

    fn rectangle_button(output: &egui::FullOutput) -> egui::Pos2 {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) if ["選取矩形範圍", "Select rectangle"].contains(&text.galley.text()) => {
                    Some(text.pos + text.galley.rect.center().to_vec2())
                }
                _ => None,
            })
            .expect("rectangle selection button must render in PlacementUi::show")
    }

    fn click_rectangle(panel: &mut PlacementUi, ctx: &egui::Context) {
        // Allow the egui window to establish its layout before locating the button.
        rectangle_frame(panel, ctx, vec![]);
        let output = rectangle_frame(panel, ctx, vec![]);
        let pos = rectangle_button(&output);
        for pressed in [true, false] {
            rectangle_frame(
                panel,
                ctx,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
    }

    fn rectangle_disk_snapshot(map: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut files: Vec<_> = fs::read_dir(map.join("DATA"))
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                let bytes = fs::read(&path).unwrap();
                (path, bytes)
            })
            .collect();
        files.sort_by(|a, b| a.0.cmp(&b.0));
        files
    }

    #[test]
    fn tile_rect_ui_renders_and_unloaded_button_preserves_state() {
        let mut panel = PlacementUi::default();
        let ctx = egui::Context::default();
        // The API/default state must not implicitly open the tool or require a map.
        let output = rectangle_frame(&mut panel, &ctx, vec![]);
        assert!(output.shapes.is_empty());
        panel.open = true;
        panel.status = "unchanged".into();
        click_rectangle(&mut panel, &ctx);
        let output = rectangle_frame(&mut panel, &ctx, vec![]);
        for label in if panel.english { ["left", "top", "right", "bottom"] } else { ["左", "上", "右", "下"] } {
            assert!(output.shapes.iter().any(|shape| matches!(
                &shape.shape,
                egui::epaint::Shape::Text(text) if text.galley.text() == label
            )));
        }
        assert_eq!(panel.status, "unchanged", "unloaded button is disabled");
        assert!(panel.selected_objects().is_empty());
        assert!(!panel.is_dirty());
        assert!(!panel.take_save_request());
    }

    #[test]
    fn tile_rect_ui_invalid_bounds_preserve_selection_and_edits() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut panel = PlacementUi::default();
        panel.load_session(&temp.0).unwrap();
        panel.open = true;
        panel.select_objects(&[(0, 1), (1, 2)]);
        panel.delta[0] = "1".into();
        panel.move_family().unwrap();
        panel.move_family().unwrap();
        panel.undo().unwrap();
        panel.request_remove().unwrap();
        let objects = panel.objects();
        let disk = rectangle_disk_snapshot(&temp.0);
        let ctx = egui::Context::default();
        for (bounds, error) in [
            (["", "0", "1", "1"], "left"),
            (["0", "1.5", "1", "2"], "top"),
            (["0", "0", "NaN", "1"], "right"),
            (["0", "0", "1", "2147483648"], "bottom"),
            (["1", "0", "1", "1"], "left < right"),
            (["2", "0", "1", "1"], "left < right"),
            (["0", "1", "1", "1"], "top < bottom"),
            (["0", "2", "1", "1"], "top < bottom"),
        ] {
            panel.tile_bounds = bounds.map(String::from);
            click_rectangle(&mut panel, &ctx);
            assert!(panel.status.contains(error), "{}", panel.status);
            assert_eq!(panel.selected_objects(), vec![(0, 1), (1, 2)]);
            assert_eq!(panel.selected, Some((1, 2)));
            assert!(matches!(panel.pending, Some(Pending::Remove { .. })));
            assert_eq!(panel.objects(), objects);
            assert!(panel.is_dirty());
            assert_eq!((panel.undo_depth(), panel.redo_depth()), (1, 1));
            assert!(!panel.take_save_request());
            let output = rectangle_frame(&mut panel, &ctx, vec![]);
            assert!(
                output.shapes.iter().any(|shape| matches!(
                    &shape.shape,
                    egui::epaint::Shape::Text(text) if text.galley.text() == panel.status
                )),
                "invalid bounds status must be visible"
            );
        }
        assert_eq!(rectangle_disk_snapshot(&temp.0), disk);
    }

    #[test]
    fn tile_rect_ui_valid_bounds_replace_selection_without_editing() {
        let temp = Temp::new();
        fixture(&temp.0);
        // Place roots on distinct tile edges with height unrelated to tile Y.
        let path = temp.0.join("DATA/position.dat");
        let mut bytes = fs::read(&path).unwrap();
        for (slot, (x, z)) in [(0.0f32, 0.0f32), (256.0, 256.0), (512.0, 0.0)]
            .into_iter()
            .enumerate()
        {
            let start = 9 + slot * 17;
            for (axis, value) in [x, 999.0, z].into_iter().enumerate() {
                bytes[start + axis * 4..start + axis * 4 + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        fs::write(&path, bytes).unwrap();
        let mut panel = PlacementUi::default();
        panel.load_session(&temp.0).unwrap();
        panel.open = true;
        panel.filter = "no matching objects".into();
        let objects = panel.objects();
        let disk = rectangle_disk_snapshot(&temp.0);
        let ctx = egui::Context::default();
        for (bounds, expected) in [
            ([" 0 ", "0", "+2", "2"], vec![(0, 1), (1, 2)]),
            (["1", "1", "2", "2"], vec![(1, 2)]),
            (["-2", "-2", "-1", "-1"], vec![]),
        ] {
            panel.select_objects(&[(2, 3)]);
            panel.request_remove().unwrap();
            panel.tile_bounds = bounds.map(String::from);
            click_rectangle(&mut panel, &ctx);
            assert_eq!(panel.selected_objects(), expected);
            assert_eq!(panel.selected, expected.last().copied());
            assert!(panel.pending.is_none(), "use select_objects removal guard");
            assert_eq!(
                panel.status,
                format!("矩形選取：已選取 {} 個物件", expected.len())
            );
            assert_eq!(panel.objects(), objects);
            assert!(!panel.is_dirty());
            assert_eq!((panel.undo_depth(), panel.redo_depth()), (0, 0));
            assert!(!panel.take_save_request());
        }
        panel.select_objects(&[(2, 3)]);
        panel.delta[0] = "1".into();
        panel.move_family().unwrap();
        panel.move_family().unwrap();
        panel.undo().unwrap();
        let edited_objects = panel.objects();
        panel.tile_bounds = ["0", "0", "2", "2"].map(String::from);
        click_rectangle(&mut panel, &ctx);
        assert_eq!(panel.selected_objects(), vec![(0, 1), (1, 2)]);
        assert_eq!(panel.objects(), edited_objects);
        assert!(panel.is_dirty(), "selection must preserve existing edits");
        assert_eq!((panel.undo_depth(), panel.redo_depth()), (1, 1));
        assert!(!panel.take_save_request());
        assert_eq!(rectangle_disk_snapshot(&temp.0), disk);
    }

    use super::*;
    use against_rome_placement::{COLUMNS, SEGMENTS};
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "ar-placement-ui-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4()
            ));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn fixture(map: &Path) {
        fixture_with_capacity(map, 3, 3);
    }
    fn fixture_with_capacity(map: &Path, n: usize, active: usize) {
        assert!(active <= n);
        let data = map.join("DATA");
        fs::create_dir_all(&data).unwrap();
        let mut objects = vec![0u8; 16 + n * (79 + COLUMNS.iter().sum::<usize>())];
        objects[..4].copy_from_slice(&1u32.to_le_bytes());
        objects[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        objects[8..12].copy_from_slice(&30u32.to_le_bytes());
        objects[12..16].copy_from_slice(&30u32.to_le_bytes());
        for slot in 0..n {
            let r = 16 + slot * 79;
            objects[r] = u8::from(slot < active);
            objects[r + 3..r + 7].copy_from_slice(&(slot as u32 + 1).to_le_bytes());
            for offset in [67, 69] {
                objects[r + offset..r + offset + 2].copy_from_slice(&(slot as u16).to_le_bytes());
            }
            for offset in [71, 73, 77] {
                objects[r + offset..r + offset + 2].fill(255);
            }
            for col in 0..18 {
                let offset =
                    16 + n * 79 + n * COLUMNS[..col].iter().sum::<usize>() + slot * COLUMNS[col];
                if COLUMNS[col] == 2 {
                    objects[offset..offset + 2].fill(255);
                }
                if col == 1 {
                    objects[offset..offset + 2].copy_from_slice(&(slot as u16).to_le_bytes());
                }
            }
        }
        let mut position = vec![0u8; 8 + n * 17];
        position[..4].copy_from_slice(&1u32.to_le_bytes());
        position[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        for slot in 0..n {
            position[8 + slot * 17] = u8::from(slot < active);
            position[9 + slot * 17..13 + slot * 17]
                .copy_from_slice(&(slot as f32 * 20.0).to_le_bytes());
        }
        let mut objdata = vec![0u8; 8 + n * SEGMENTS.iter().sum::<usize>()];
        objdata[..4].copy_from_slice(&1u32.to_le_bytes());
        objdata[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        for (name, bytes) in [
            ("objects", objects),
            ("position", position),
            ("objdata", objdata),
        ] {
            fs::write(data.join(format!("{name}.dat")), bytes).unwrap();
        }
    }
    #[test]
    fn fixed_conversion_rejects_invalid_and_overflow() {
        assert_eq!(
            fixed_delta(&["1".into(), "-0.5".into(), "0.0000152587890625".into()]).unwrap(),
            [65536, -32768, 1]
        );
        for bad in [
            "NaN",
            "inf",
            "-inf",
            "32768",
            "-32768.00001",
            "32767.99999",
            "junk",
            "",
        ] {
            assert!(
                fixed_delta(&[bad.into(), "0".into(), "0".into()]).is_err(),
                "{bad}"
            );
        }
        assert_eq!(
            fixed_delta(&["-32768".into(), "0".into(), "32767.9999847412109375".into()]).unwrap(),
            [i32::MIN, 0, i32::MAX]
        );
    }
    #[test]
    fn temp_data_move_undo_redo_save_reload_and_stale_uid() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi {
            map_input: temp.0.display().to_string(),
            ..Default::default()
        };
        ui.request_load().unwrap();
        ui.selected = Some((1, 2));
        ui.delta = ["1".into(), "-0.5".into(), "0.25".into()];
        ui.move_family().unwrap();
        assert!(ui.is_dirty());
        assert_eq!(
            ui.store.as_ref().unwrap().objects()[1].position,
            [21.0, -0.5, 0.25]
        );
        ui.undo().unwrap();
        assert!(!ui.is_dirty());
        ui.redo().unwrap();
        assert!(ui.is_dirty());
        ui.save().unwrap();
        assert!(!ui.is_dirty());
        assert_eq!(
            ObjectStore::load(&temp.0).unwrap().objects()[1].position,
            [21.0, -0.5, 0.25]
        );
        ui.selected = Some((1, 99));
        assert!(ui.move_family().is_err());
        assert!(!ui.is_dirty());
    }
    #[test]
    fn reset_restores_loaded_baseline_without_reading_or_writing_disk() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_session(&temp.0).unwrap();
        let baseline = ui.objects();
        ui.select_objects(&[(0, 1), (1, 2)]);
        ui.delta[0] = "5".into();
        ui.move_family().unwrap();
        ui.team = "3".into();
        ui.set_team().unwrap();
        ui.undo().unwrap();
        assert_eq!((ui.undo_depth(), ui.redo_depth()), (1, 1));
        ui.filter = "team".into();
        ui.open = true;
        // Changing input is not a change to the loaded document or reset target.
        ui.map_input = "another map".into();
        let position = temp.0.join("DATA/position.dat");
        fs::write(&position, b"externally modified").unwrap();
        ui.request_reset().unwrap();
        assert!(ui.is_dirty(), "request alone must not discard edits");
        ui.confirm_pending().unwrap();
        assert_eq!(ui.objects(), baseline);
        assert!(ui.is_loaded());
        assert_eq!(ui.loaded_map(), Some(temp.0.as_path()));
        assert!(!ui.is_dirty());
        assert_eq!(ui.selected_objects(), vec![(0, 1), (1, 2)]);
        assert_eq!(ui.filter, "team");
        assert_eq!(ui.delta[0], "5");
        assert_eq!(ui.team, "3");
        assert_eq!(ui.map_input, "another map");
        assert!(ui.open);
        assert!(ui.pending.is_none());
        assert_eq!((ui.undo_depth(), ui.redo_depth()), (0, 0));
        assert!(ui.undo().is_err());
        assert!(ui.redo().is_err());
        assert_eq!(fs::read(&position).unwrap(), b"externally modified");
        // Reset must retain the original raw disk baseline, not bless external changes.
        ui.move_family().unwrap();
        assert!(ui.save().unwrap_err().contains("external file change"));
        assert!(ui.is_dirty());
        assert_eq!(ui.undo_depth(), 1);
        assert_eq!(fs::read(position).unwrap(), b"externally modified");
    }

    #[test]
    fn reset_is_a_history_boundary_and_subsequent_save_advances_baseline() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_session(&temp.0).unwrap();
        let original = ui.objects();
        ui.select_object(Some((0, 1)));
        ui.delta[0] = "5".into();
        ui.move_family().unwrap();
        ui.reset_to_baseline().unwrap();
        ui.move_family().unwrap();
        ui.undo().unwrap();
        assert_eq!(ui.objects(), original);
        assert!(!ui.is_dirty());
        assert!(ui.undo().is_err());
        ui.redo().unwrap();
        assert!(ui.redo().is_err());
        ui.save().unwrap();
        let saved = ui.objects();
        assert_eq!(ObjectStore::load(&temp.0).unwrap().objects(), saved);
        ui.move_family().unwrap();
        ui.reset_to_baseline().unwrap();
        assert_eq!(ui.objects(), saved);
        assert!(!ui.is_dirty());
        assert!(ui.undo().is_err());
        assert!(ui.redo().is_err());
        // Reset also clears redo when the current payload already equals baseline.
        ui.move_family().unwrap();
        ui.undo().unwrap();
        assert_eq!(ui.redo_depth(), 1);
        ui.reset_to_baseline().unwrap();
        assert!(ui.redo().is_err());
        assert_eq!(ui.objects(), saved);
    }

    #[test]
    fn failed_reset_preserves_edits_history_selection_and_confirmation() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        assert!(ui.reset_to_baseline().is_err());
        assert!(ui.request_reset().is_err());
        ui.load_session(&temp.0).unwrap();
        ui.select_object(Some((0, 1)));
        ui.delta[0] = "5".into();
        ui.move_family().unwrap();
        ui.move_family().unwrap();
        ui.undo().unwrap();
        let edited = ui.objects();
        let baseline = ui.baseline.take();
        ui.request_reset().unwrap();
        assert!(ui.confirm_pending().is_err());
        assert_eq!(ui.objects(), edited);
        assert!(ui.is_dirty());
        assert_eq!((ui.undo_depth(), ui.redo_depth()), (1, 1));
        assert_eq!(ui.selected_object(), Some((0, 1)));
        assert_eq!(ui.loaded_map(), Some(temp.0.as_path()));
        assert!(matches!(ui.pending, Some(Pending::Reset)));
        ui.baseline = baseline;
        ui.confirm_pending().unwrap();
        assert!(!ui.is_dirty());
        assert_eq!(ui.objects()[0].position[0], 0.0);
    }

    #[test]
    fn reset_filters_removed_clone_selection_and_unload_forgets_baseline() {
        let temp = Temp::new();
        fixture(&temp.0);
        free_last_slot(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_session(&temp.0).unwrap();
        let baseline = ui.objects();
        ui.select_object(Some((0, 1)));
        ui.clone_family().unwrap();
        assert_eq!(ui.selected_object(), Some((2, 3)));
        ui.reset_to_baseline().unwrap();
        assert_eq!(ui.objects(), baseline);
        assert!(ui.selected_objects().is_empty());
        ui.reset_session(None);
        assert!(ui.baseline.is_none());
        assert!(ui.reset_to_baseline().is_err());
        assert!(!ui.is_loaded());
    }

    #[test]
    fn whole_map_commit_refreshes_reset_baseline_but_failed_load_does_not() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_session(&temp.0).unwrap();
        ui.select_object(Some((0, 1)));
        ui.delta[0] = "5".into();
        ui.move_family().unwrap();
        for (relative, bytes) in ui.stage_transaction_files(&temp.0).unwrap() {
            fs::write(temp.0.join(relative), bytes).unwrap();
        }
        ui.commit_baseline(&temp.0).unwrap();
        let saved = ui.objects();
        ui.move_family().unwrap();
        let invalid = Temp::new();
        fs::create_dir(invalid.0.join("DATA")).unwrap();
        assert!(ui.load_session(&invalid.0).is_err());
        ui.reset_to_baseline().unwrap();
        assert_eq!(ui.objects(), saved);
        assert!(!ui.is_dirty());
        assert_eq!(ui.loaded_map(), Some(temp.0.as_path()));
    }

    #[test]
    fn save_accepts_grouped_changes_and_new_undo_stops_at_saved_baseline() {
        let temp = Temp::new();
        fixture(&temp.0);
        free_last_slot(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_session(&temp.0).unwrap();
        let batch = ui.apply_requests(&[request(300.0, 400.0)]).unwrap();
        assert!(batch.ops > 1);
        assert_eq!(ui.undo_depth(), 1);
        ui.select_object(Some((2, 3)));
        ui.save().unwrap();
        let saved = ui.objects();
        assert!(!ui.is_dirty());
        assert_eq!((ui.undo_depth(), ui.redo_depth()), (0, 0));
        assert_eq!(ui.selected_object(), Some((2, 3)));
        assert!(ui.undo().is_err());
        assert!(ui.redo().is_err());
        assert_eq!(ui.objects(), saved);

        ui.delta[0] = "5".into();
        ui.move_family().unwrap();
        assert_eq!((ui.undo_depth(), ui.redo_depth()), (1, 0));
        ui.undo().unwrap();
        assert_eq!(ui.objects(), saved);
        assert!(!ui.is_dirty());
        assert!(ui.undo().is_err(), "cannot cross the saved baseline");
        assert_eq!((ui.undo_depth(), ui.redo_depth()), (0, 1));
        ui.redo().unwrap();
        assert_eq!(ui.objects()[2].position[0], 305.0);
        assert!(ui.redo().is_err());
    }

    #[test]
    fn save_accepts_current_state_and_discards_redo_branch() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_session(&temp.0).unwrap();
        ui.select_object(Some((0, 1)));
        ui.delta[0] = "1".into();
        ui.move_family().unwrap();
        ui.move_family().unwrap();
        ui.undo().unwrap();
        assert_eq!((ui.undo_depth(), ui.redo_depth()), (1, 1));
        ui.save().unwrap();
        assert_eq!((ui.undo_depth(), ui.redo_depth()), (0, 0));
        assert!(ui.undo().is_err());
        assert!(ui.redo().is_err());
        assert_eq!(ui.objects()[0].position[0], 1.0);
        assert!(!ui.is_dirty());

        ui.move_family().unwrap();
        ui.undo().unwrap();
        ui.redo().unwrap();
        assert_eq!(ui.objects()[0].position[0], 2.0);
        assert!(ui.redo().is_err(), "old redo branch stays inaccessible");
    }

    #[test]
    fn already_snapped_selection_preserves_saved_boundary_and_redo() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_session(&temp.0).unwrap();
        ui.select_object(Some((0, 1)));
        ui.snap_to_grid().unwrap();
        ui.save().unwrap();
        let saved = ui.objects();
        ui.snap_to_grid().unwrap();
        assert_eq!((ui.undo_depth(), ui.redo_depth()), (0, 0));
        assert!(ui.undo().is_err());
        assert_eq!(ui.objects(), saved);
        assert!(!ui.is_dirty());

        ui.delta[0] = "1".into();
        ui.move_family().unwrap();
        ui.undo().unwrap();
        ui.snap_to_grid().unwrap();
        assert_eq!((ui.undo_depth(), ui.redo_depth()), (0, 1));
        ui.redo().unwrap();
        assert_eq!(ui.objects()[0].position[0], 129.0);
        ui.undo().unwrap();
        assert_eq!(ui.objects(), saved);
        assert!(ui.undo().is_err());
    }

    #[test]
    fn failed_save_preserves_undo_redo_selection_and_pending_command() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_session(&temp.0).unwrap();
        ui.select_object(Some((0, 1)));
        ui.delta[0] = "1".into();
        ui.move_family().unwrap();
        ui.move_family().unwrap();
        ui.undo().unwrap();
        ui.request_remove().unwrap();
        let before = ui.objects();
        let path = temp.0.join("DATA/position.dat");
        let mut external = fs::read(&path).unwrap();
        external[9..13].copy_from_slice(&123.0f32.to_le_bytes());
        fs::write(&path, &external).unwrap();
        let error = ui.save().unwrap_err();
        assert!(error.contains("external file change"), "{error}");
        assert_eq!(ui.objects(), before);
        assert_eq!((ui.undo_depth(), ui.redo_depth()), (1, 1));
        assert_eq!(ui.selected_objects(), vec![(0, 1)]);
        assert!(matches!(ui.pending, Some(Pending::Remove { .. })));
        assert!(ui.is_dirty());
        assert_eq!(fs::read(&path).unwrap(), external);
        ui.redo().unwrap();
        assert_eq!(ui.objects()[0].position[0], 2.0);
        ui.undo().unwrap();
        ui.undo().unwrap();
        assert!(!ui.is_dirty());
    }

    #[test]
    fn dirty_load_and_close_require_confirmation_and_failed_load_retains_work() {
        let temp = Temp::new();
        let first = temp.0.join("first");
        let second = temp.0.join("second");
        fixture(&first);
        fixture(&second);
        let mut ui = PlacementUi {
            open: true,
            map_input: first.display().to_string(),
            ..Default::default()
        };
        ui.request_load().unwrap();
        ui.selected = Some((0, 1));
        ui.delta[0] = "2".into();
        ui.move_family().unwrap();
        ui.map_input = second.display().to_string();
        ui.request_load().unwrap();
        assert!(matches!(ui.pending, Some(Pending::Load(_))));
        assert_eq!(ui.loaded_map.as_ref(), Some(&first));
        assert!(ui.is_dirty());
        ui.pending = None;
        ui.request_close();
        assert!(ui.open);
        assert!(matches!(ui.pending, Some(Pending::Close)));
        ui.pending = None;
        ui.map_input = temp.0.join("missing").display().to_string();
        ui.request_load().unwrap();
        assert!(ui.confirm_pending().is_err());
        assert_eq!(ui.loaded_map.as_ref(), Some(&first));
        assert!(ui.is_dirty());
        ui.map_input = second.display().to_string();
        ui.request_load().unwrap();
        ui.confirm_pending().unwrap();
        assert_eq!(ui.loaded_map.as_ref(), Some(&second));
        assert!(!ui.is_dirty());
        ui.selected = Some((0, 1));
        ui.move_family().unwrap();
        ui.request_close();
        ui.confirm_pending().unwrap();
        assert!(!ui.open);
        assert!(ui.store.is_none());
    }
    #[test]
    fn load_session_rejects_invalid_map_paths_but_resets_for_existing_map_without_data() {
        let temp = Temp::new();
        let first = temp.0.join("first");
        fixture(&first);
        let mut ui = PlacementUi {
            open: true,
            ..Default::default()
        };
        ui.load_session(&first).unwrap();
        ui.selected = Some((0, 1));
        ui.delta[0] = "2".into();
        ui.move_family().unwrap();
        let objects = ui.objects();
        let map_input = ui.map_input.clone();
        let file = temp.0.join("not-a-directory");
        fs::write(&file, b"map path must be a directory").unwrap();
        for invalid in [temp.0.join("missing"), file] {
            let error = ui.load_session(&invalid).unwrap_err();
            assert!(error.contains("not a directory"), "{error}");
            assert_eq!(ui.loaded_map(), Some(first.as_path()));
            assert_eq!(ui.map_input, map_input);
            assert_eq!(ui.objects(), objects);
            assert_eq!(ui.selected_object(), Some((0, 1)));
            assert_eq!(ui.undo_depth(), 1);
            assert!(ui.is_dirty());
        }

        let without_data = temp.0.join("without-data");
        fs::create_dir(&without_data).unwrap();
        ui.load_session(&without_data).unwrap();
        assert!(!ui.is_loaded());
        assert_eq!(ui.loaded_map(), None);
        assert_eq!(ui.map_input, without_data.to_string_lossy());
        assert_eq!(ui.selected_object(), None);
        assert_eq!(ui.undo_depth(), 0);
        assert_eq!(ui.redo_depth(), 0);
        assert!(!ui.is_dirty());
        assert!(ui.open);
        assert_eq!(ui.delta()[0], "2");
    }

    #[test]
    fn removal_confirmation_binds_selection_and_is_undoable() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();
        ui.selected = Some((0, 1));
        ui.request_remove().unwrap();
        ui.selected = Some((1, 2));
        assert!(ui.confirm_pending().is_err());
        assert_eq!(ui.store.as_ref().unwrap().objects().len(), 3);
        ui.request_remove().unwrap();
        ui.confirm_pending().unwrap();
        assert_eq!(ui.store.as_ref().unwrap().objects().len(), 2);
        ui.undo().unwrap();
        assert_eq!(ui.store.as_ref().unwrap().objects().len(), 3);
    }
    #[test]
    fn save_path_is_bound_to_loaded_map_and_installed_path_refused() {
        assert!(explicit_map("").is_err());
        assert!(explicit_map("relative").is_err());
        assert!(allow_save(Path::new(
            "C:\\Program Files (x86)\\Against Rome\\MAPS\\TEST"
        ))
        .is_err());
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();
        ui.selected = Some((0, 1));
        ui.delta[0] = "1".into();
        ui.move_family().unwrap();
        ui.map_input = temp.0.join("unloaded").display().to_string();
        ui.save().unwrap();
        assert!(!Path::new(&ui.map_input).exists());
        assert_eq!(
            ObjectStore::load(&temp.0).unwrap().objects()[0].position[0],
            1.0
        );
    }

    #[test]
    fn clone_allocates_independent_object_and_core_errors_are_preserved() {
        let temp = Temp::new();
        fixture(&temp.0);
        let objects_path = temp.0.join("DATA/objects.dat");
        let position_path = temp.0.join("DATA/position.dat");
        let mut objects = fs::read(&objects_path).unwrap();
        objects[16 + 2 * 79] = 0;
        fs::write(objects_path, objects).unwrap();
        let mut positions = fs::read(&position_path).unwrap();
        positions[8 + 2 * 17] = 0;
        fs::write(position_path, positions).unwrap();
        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();
        ui.selected = Some((0, 1));
        ui.delta[0] = "7.5".into();
        ui.clone_family().unwrap();
        let (slot, uid) = ui.selected.unwrap();
        assert_eq!(slot, 2);
        let list = ui.store.as_ref().unwrap().objects();
        assert_eq!(
            list.iter().find(|o| o.slot == slot).unwrap().position[0],
            7.5
        );
        assert_eq!(list.iter().find(|o| o.slot == 0).unwrap().position[0], 0.0);
        assert_eq!(list.iter().find(|o| o.slot == slot).unwrap().uid, uid);
        ui.undo().unwrap();
        assert!(!ui.is_dirty());
        assert!(ui.selected.is_none());
        ui.redo().unwrap();
        ui.save().unwrap();
        assert_eq!(ObjectStore::load(&temp.0).unwrap().objects().len(), 3);
        ui.selected = Some((0, 1));
        let expected = ui
            .store
            .as_ref()
            .unwrap()
            .clone()
            .clone_family_fixed(0, 1, [491520, 0, 0])
            .unwrap_err();
        assert_eq!(ui.clone_family().unwrap_err(), expected);
        assert!(!ui.is_dirty());
    }

    #[test]
    fn external_data_change_refuses_save_and_keeps_unsaved_session() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();
        ui.selected = Some((0, 1));
        ui.delta[0] = "1".into();
        ui.move_family().unwrap();
        let path = temp.0.join("DATA/position.dat");
        let mut external = fs::read(&path).unwrap();
        external[9..13].copy_from_slice(&123.0f32.to_le_bytes());
        fs::write(&path, &external).unwrap();
        let error = ui.save().unwrap_err();
        assert!(error.contains("external file change"), "{error}");
        assert!(ui.is_dirty());
        assert_eq!(fs::read(path).unwrap(), external);
        assert_eq!(ui.store.as_ref().unwrap().objects()[0].position[0], 1.0);
    }

    fn request(x: f32, z: f32) -> PlacementRequest {
        PlacementRequest {
            type_id: 0,
            name: "t".into(),
            x,
            y: 1.5,
            z,
            rotation_deg: 90.0,
            team: Some(3),
        }
    }

    fn history_fixture() -> (Temp, PlacementUi) {
        let temp = Temp::new();
        fixture(&temp.0);
        free_last_slot(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();
        ui.take_history_receipts();
        (temp, ui)
    }

    #[test]
    fn shadow_history_group_tokens_round_trip_and_branch() {
        let (_temp, mut ui) = history_fixture();
        let original = ui.history_token();
        let before = history_payloads(ui.store.as_ref());
        assert!(ui.apply_requests(&[request(10.0, 10.0)]).unwrap().ops > 0);
        let first = ui.history_token();
        let after = history_payloads(ui.store.as_ref());
        assert_ne!(original, first);
        assert_eq!(ui.take_history_receipts(), vec![PlacementHistoryReceipt::Commit(
            crate::undo_journal::SubsystemStep {
                subsystem: crate::undo_journal::SubsystemId::Placement,
                before_token: original, after_token: first,
            },
        )]);
        ui.undo().unwrap();
        assert_eq!(ui.history_token(), original);
        assert_eq!(history_payloads(ui.store.as_ref()), before);
        ui.redo().unwrap();
        assert_eq!(ui.history_token(), first);
        assert_eq!(history_payloads(ui.store.as_ref()), after);
        assert!(ui.take_history_receipts().is_empty());
        ui.undo().unwrap();
        ui.apply_requests(&[request(20.0, 20.0)]).unwrap();
        assert_ne!(ui.history_token(), first);
        assert_eq!(ui.redo_depth(), 0);
        assert!(!ui.store.as_ref().unwrap().can_redo());
        assert_eq!(ui.take_history_receipts().len(), 1);
    }

    #[test]
    fn shadow_history_noop_and_rejection_emit_nothing() {
        let (_temp, mut ui) = history_fixture();
        let original = ui.history_token();
        ui.apply_requests(&[]).unwrap();
        ui.apply_requests(&[request(-1.0, 0.0)]).unwrap();
        assert!(ui.apply_requests(&[request(10.0, 10.0), request(20.0, 20.0)]).is_err());
        ui.select_object(Some((0, 1)));
        ui.move_family().unwrap();
        assert!(ui.batch_edit(Some(99), None).is_err());
        assert_eq!(ui.history_token(), original);
        assert!(ui.take_history_receipts().is_empty());
        // 核心仍接受相同值命令並保留既有 undo 行為，旁錄不虛構提交。
        let team = ui.objects().iter().find(|o| o.slot == 0).unwrap().team;
        ui.batch_edit(Some(team), None).unwrap();
        assert_eq!(ui.undo_depth(), 1);
        assert_eq!(ui.history_token(), original);
        ui.undo().unwrap();
        ui.redo().unwrap();
        assert_eq!(ui.history_token(), original);
        assert!(ui.take_history_receipts().is_empty());
    }

    #[test]
    fn shadow_history_external_store_barrier_invalidates_journal() {
        let (_temp, mut ui) = history_fixture();
        ui.apply_requests(&[request(10.0, 10.0)]).unwrap();
        let mut journal = crate::undo_journal::UndoJournal::default();
        for receipt in ui.take_history_receipts() {
            if let PlacementHistoryReceipt::Commit(step) = receipt {
                assert!(journal.record_compound(vec![step]));
            } else { panic!("預期提交通知"); }
        }
        let prior = ui.history_token();
        // Families／Effects 使用同一個 store，沿用既有編輯後通知入口。
        ui.store_mut().unwrap().translate_families_fixed(&[(0, 1)], [65536, 0, 0]).unwrap();
        ui.note_external_store_edit();
        assert_ne!(ui.history_token(), prior);
        let receipts = ui.take_history_receipts();
        assert_eq!(receipts, vec![PlacementHistoryReceipt::Barrier { after_token: ui.history_token() }]);
        for receipt in receipts {
            if let PlacementHistoryReceipt::Barrier { .. } = receipt { journal.new_epoch(); }
        }
        assert!(!journal.can_undo() && !journal.can_redo());
        assert!(ui.store.as_ref().unwrap().can_undo(), "保留共用 store 的既有歷史");
        assert_eq!((ui.undo_depth(), ui.redo_depth()), (0, 0));
        ui.select_object(Some((0, 1)));
        ui.nudge_selection(1.0, 0.0).unwrap();
        assert!(matches!(ui.take_history_receipts().as_slice(), [PlacementHistoryReceipt::Commit(_)]));
    }

    #[test]
    fn shadow_history_clear_redo_preserves_undo() {
        let (_temp, mut ui) = history_fixture();
        ui.select_object(Some((0, 1)));
        ui.nudge_selection(1.0, 0.0).unwrap();
        let first = ui.history_token();
        ui.nudge_selection(1.0, 0.0).unwrap();
        ui.undo().unwrap();
        ui.take_history_receipts();
        ui.clear_redo();
        assert_eq!(ui.history_token(), first);
        assert_eq!((ui.undo_depth(), ui.redo_depth()), (1, 0));
        assert!(ui.redo().is_err());
        assert!(!ui.store.as_ref().unwrap().can_redo());
        ui.undo().unwrap();
        assert_ne!(ui.history_token(), first);
        assert!(ui.take_history_receipts().is_empty());
    }
    fn free_last_slot(map: &Path) {
        let objects_path = map.join("DATA/objects.dat");
        let position_path = map.join("DATA/position.dat");
        let mut objects = fs::read(&objects_path).unwrap();
        objects[16 + 2 * 79] = 0;
        fs::write(objects_path, objects).unwrap();
        let mut positions = fs::read(&position_path).unwrap();
        positions[8 + 2 * 17] = 0;
        fs::write(position_path, positions).unwrap();
    }

    #[test]
    fn design_batch_clones_templates_marks_dirty_and_undoes_as_one_step() {
        let temp = Temp::new();
        fixture(&temp.0);
        free_last_slot(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();
        assert!(!ui.is_dirty());
        // One valid, one outside the world, one with an unknown template type.
        let mut unknown = request(100.0, 100.0);
        unknown.type_id = 999;
        let batch = ui
            .apply_requests(&[request(300.0, 400.0), request(-5.0, 10.0), unknown])
            .unwrap();
        assert_eq!((batch.consumed, batch.placed), (3, 1));
        assert!(batch.ops >= 2, "clone + rotation (+ team)");
        assert_eq!(batch.skipped.len(), 2, "{:?}", batch.skipped);
        assert!(ui.is_dirty());
        let placed = ui.objects().into_iter().find(|o| o.slot == 2).unwrap();
        assert_eq!(placed.position, [300.0, 1.5, 400.0]);
        assert_eq!(placed.rotation_degrees, 90.0);
        assert_eq!(placed.team, 3);
        ui.undo().unwrap();
        assert!(!ui.is_dirty(), "the whole batch is one undo step");
        assert_eq!(ui.object_count(), 2);
        ui.redo().unwrap();
        assert_eq!(ui.object_count(), 3);
        ui.save().unwrap();
        assert!(!ui.is_dirty());
        let reloaded = ObjectStore::load(&temp.0).unwrap().objects();
        assert_eq!(
            reloaded.iter().find(|o| o.slot == 2).unwrap().position,
            [300.0, 1.5, 400.0]
        );
    }

    #[test]
    fn design_batch_is_atomic_when_the_pool_is_exhausted_and_requires_a_session() {
        let temp = Temp::new();
        fixture(&temp.0);
        free_last_slot(&temp.0);
        let mut ui = PlacementUi::default();
        assert!(ui.apply_requests(&[request(1.0, 1.0)]).is_err());
        ui.load_now(&temp.0.display().to_string()).unwrap();
        // Only one free slot: the second clone fails and nothing from the batch survives.
        let err = ui
            .apply_requests(&[request(10.0, 10.0), request(20.0, 20.0)])
            .unwrap_err();
        assert!(err.contains("insufficient"), "{err}");
        assert!(!ui.is_dirty());
        assert_eq!(ui.object_count(), 2);
        assert!(!ui.store.as_ref().unwrap().can_undo());
    }

    #[test]
    fn design_batch_is_bounded_and_reports_consumed_prefix() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();
        // No free slot: out-of-world requests are skipped, never reaching the core.
        let many: Vec<_> = (0..crate::design_apply::MAX_OBJECTS_PER_BATCH + 10)
            .map(|_| request(-1.0, 0.0))
            .collect();
        let batch = ui.apply_requests(&many).unwrap();
        assert_eq!(batch.consumed, crate::design_apply::MAX_OBJECTS_PER_BATCH);
        assert_eq!(batch.placed, 0);
        assert!(!ui.is_dirty());
    }

    fn placement_payloads(store: &ObjectStore) -> Vec<Option<Vec<u8>>> {
        [
            "objects", "objdata", "position", "anim", "gfxtype", "action", "engine", "light",
            "particle", "lager", "biglager",
        ]
        .iter()
        .map(|name| store.payload(name).map(<[u8]>::to_vec))
        .collect()
    }

    #[test]
    fn design_grouped_requests_consume_all_chunks_with_one_undo_redo() {
        let cap = crate::design_apply::MAX_OBJECTS_PER_BATCH;
        let placed_count = cap + 10;
        let temp = Temp::new();
        fixture_with_capacity(&temp.0, placed_count + 2, 2);
        let mut ui = PlacementUi::default();
        ui.load_session(&temp.0).unwrap();
        ui.select_objects(&[(0, 1), (1, 2)]);
        ui.request_remove().unwrap();
        let before = placement_payloads(ui.store.as_ref().unwrap());
        let mut requests: Vec<_> = (0..placed_count)
            .map(|i| request(300.0 + i as f32, 400.0))
            .collect();
        // Skips in both chunks must be consumed and reported as well.
        requests.insert(0, request(-1.0, 0.0));
        let mut unknown = request(100.0, 100.0);
        unknown.type_id = 999;
        requests.insert(cap, unknown);
        requests.push(request(f32::NAN, 0.0));
        let batch = ui.apply_requests_grouped(&requests).unwrap();
        assert_eq!((batch.consumed, batch.placed), (requests.len(), placed_count));
        assert_eq!(batch.ops, placed_count * 3, "clone + rotation + team");
        assert_eq!(batch.skipped.len(), 3);
        for reason in ["1 requests outside", "type 999 ×1", "1 non-finite"] {
            assert!(batch.skipped.iter().any(|s| s.contains(reason)), "{batch:?}");
        }
        assert_eq!(ui.undo_groups, vec![batch.ops]);
        assert_eq!(ui.redo_depth(), 0);
        assert!(ui.selected.is_none() && ui.selection.is_empty() && ui.pending.is_none());
        assert!(ui.is_dirty());
        assert_eq!(ui.object_count(), placed_count + 2);
        for (i, object) in ui.objects().iter().skip(2).enumerate() {
            assert_eq!(object.position, [300.0 + i as f32, 1.5, 400.0]);
            assert_eq!((object.rotation_degrees, object.team), (90.0, 3));
        }
        let after = placement_payloads(ui.store.as_ref().unwrap());
        ui.undo().unwrap();
        assert_eq!(placement_payloads(ui.store.as_ref().unwrap()), before);
        assert_eq!((ui.undo_depth(), ui.redo_depth()), (0, 1));
        assert!(!ui.is_dirty());
        assert!(!ui.store.as_ref().unwrap().can_undo());
        ui.redo().unwrap();
        assert_eq!(placement_payloads(ui.store.as_ref().unwrap()), after);
        assert_eq!((ui.undo_depth(), ui.redo_depth()), (1, 0));
        assert!(!ui.store.as_ref().unwrap().can_redo());
    }

    #[test]
    fn design_grouped_requests_late_pool_failure_preserves_entire_session() {
        let cap = crate::design_apply::MAX_OBJECTS_PER_BATCH;
        let temp = Temp::new();
        // Exactly one full chunk fits; the next request fails real core allocation.
        fixture_with_capacity(&temp.0, cap + 2, 2);
        let mut ui = PlacementUi::default();
        ui.load_session(&temp.0).unwrap();
        ui.select_objects(&[(0, 1), (1, 2)]);
        ui.delta[0] = "1".into();
        ui.move_family().unwrap();
        ui.move_family().unwrap();
        ui.undo().unwrap();
        ui.request_remove().unwrap();
        let mut expected = ui.store.as_ref().unwrap().clone();
        let selected = ui.selected;
        let selection = ui.selection.clone();
        let undo = ui.undo_groups.clone();
        let redo = ui.redo_groups.clone();
        assert_eq!((undo.len(), redo.len()), (1, 1));
        let requests = vec![request(300.0, 400.0); cap + 1];

        // Establish that this is a late failure, rather than a rejected fixture/template.
        let mut probe = expected.clone();
        let first = apply_placements(&mut probe, &requests[..cap]).unwrap();
        assert_eq!((first.consumed, first.placed), (cap, cap));
        assert_eq!(probe.objects().len(), cap + 2);
        let error = apply_placements(&mut probe, &requests[cap..]).unwrap_err();
        assert!(error.contains("insufficient free native pool slots"), "{error}");
        let error = ui.apply_requests_grouped(&requests).unwrap_err();
        assert!(error.contains("insufficient free native pool slots"), "{error}");

        assert_eq!(placement_payloads(ui.store.as_ref().unwrap()), placement_payloads(&expected));
        assert_eq!(ui.objects(), expected.objects());
        assert_eq!(ui.is_dirty(), expected.is_dirty());
        assert_eq!(ui.selected, selected);
        assert_eq!(ui.selection, selection);
        assert!(matches!(&ui.pending, Some(Pending::Remove { families }) if *families == selection));
        assert_eq!(ui.undo_groups, undo);
        assert_eq!(ui.redo_groups, redo);
        // Replay both histories against the untouched clone, comparing all pool bytes at
        // every step so retained depth alone cannot hide lost or altered history entries.
        ui.redo().unwrap();
        assert!(expected.redo());
        assert_eq!(placement_payloads(ui.store.as_ref().unwrap()), placement_payloads(&expected));
        for _ in 0..2 {
            ui.undo().unwrap();
            assert!(expected.undo());
            assert_eq!(placement_payloads(ui.store.as_ref().unwrap()), placement_payloads(&expected));
        }
        assert!(!ui.is_dirty());
        assert!(!ui.store.as_ref().unwrap().can_undo());
        assert!(!expected.can_undo());
        for _ in 0..2 {
            ui.redo().unwrap();
            assert!(expected.redo());
            assert_eq!(placement_payloads(ui.store.as_ref().unwrap()), placement_payloads(&expected));
        }
        assert!(!ui.store.as_ref().unwrap().can_redo());
        assert!(!expected.can_redo());
    }

    #[test]
    fn design_grouped_requests_without_edits_preserve_history_and_pending_state() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        assert!(ui.apply_requests_grouped(&[request(1.0, 1.0)]).is_err());
        ui.load_session(&temp.0).unwrap();
        ui.select_object(Some((0, 1)));
        ui.delta[0] = "1".into();
        ui.move_family().unwrap();
        ui.move_family().unwrap();
        ui.undo().unwrap();
        ui.request_remove().unwrap();
        let before = placement_payloads(ui.store.as_ref().unwrap());
        let undo = ui.undo_groups.clone();
        let redo = ui.redo_groups.clone();
        let requests = vec![request(-1.0, 0.0); crate::design_apply::MAX_OBJECTS_PER_BATCH + 1];
        let batch = ui.apply_requests_grouped(&requests).unwrap();
        assert_eq!((batch.consumed, batch.placed, batch.ops), (requests.len(), 0, 0));
        assert_eq!(batch.skipped.len(), 2, "one skip summary per internal chunk");
        assert_eq!(ui.apply_requests_grouped(&[]).unwrap(), PlacementBatch::default());
        assert_eq!(placement_payloads(ui.store.as_ref().unwrap()), before);
        assert_eq!(ui.selected, Some((0, 1)));
        assert_eq!(ui.selection, vec![(0, 1)]);
        assert!(matches!(&ui.pending, Some(Pending::Remove { families }) if *families == vec![(0, 1)]));
        assert_eq!(ui.undo_groups, undo);
        assert_eq!(ui.redo_groups, redo);
        assert!(ui.store.as_ref().unwrap().can_redo());
    }

    #[test]
    fn rotation_team_and_snap_are_single_undo_steps_and_validated() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();
        assert!(ui.rotate_family(true).is_err(), "needs a selection");
        ui.selected = Some((1, 2));
        let find = |ui: &PlacementUi| ui.objects().into_iter().find(|o| o.slot == 1).unwrap();
        ui.angle = "-90".into();
        ui.rotate_family(true).unwrap();
        ui.angle = "45".into();
        ui.rotate_family(false).unwrap();
        assert_eq!(find(&ui).rotation_degrees, 315.0);
        ui.angle = "nan".into();
        assert!(ui.rotate_family(true).is_err());
        ui.team = "3".into();
        ui.set_team().unwrap();
        assert_eq!(find(&ui).team, 3);
        ui.team = "16".into();
        assert!(ui.set_team().is_err());
        ui.team = "-1".into();
        ui.set_team().unwrap();
        assert_eq!(find(&ui).team, 65535);
        ui.snap_to_grid().unwrap();
        assert_eq!(find(&ui).position[0].rem_euclid(256.0), 128.0);
        assert_eq!(ui.undo_groups.len(), 5);
        ui.undo().unwrap();
        assert_eq!(find(&ui).position[0], 20.0);
        ui.save().unwrap();
        let saved = ObjectStore::load(&temp.0).unwrap();
        assert_eq!(
            saved.objects().iter().find(|o| o.slot == 1).unwrap().team,
            65535
        );
    }

    #[test]
    fn selecting_ignores_stale_keys() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();
        ui.select_object(Some((1, 2)));
        assert_eq!(ui.selected_object(), Some((1, 2)));
        ui.select_object(Some((1, 77)));
        assert_eq!(ui.selected_object(), None);
    }

    #[test]
    fn selection_tools_preserve_data_and_history_and_respect_confirmation() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();
        let objects = ui.objects();
        let dirty = ui.is_dirty();
        let history = (ui.undo_depth(), ui.redo_depth());
        ui.filter = "no matching objects".into();
        ui.select_object(Some((1, 2)));
        let expected: Vec<_> = objects.iter().filter(|o| o.team == objects.iter().find(|o| (o.slot, o.uid) == (1, 2)).unwrap().team)
            .map(|o| (o.slot, o.uid)).collect();
        assert_eq!(ui.select_same_teams(), expected.len());
        assert_eq!(ui.selected_objects(), expected);
        ui.invert_selection();
        assert!(ui.selected_objects().iter().all(|k| !expected.contains(k)));
        ui.invert_selection();
        assert_eq!(ui.selected_objects(), expected);
        ui.clear_selection();
        assert_eq!(ui.select_same_teams(), 0);
        assert_eq!(ui.invert_selection(), objects.len());
        assert_eq!(ui.invert_selection(), 0);
        ui.selection_tools_blocked = true;
        assert_eq!(ui.invert_selection(), 0);
        ui.selection_tools_blocked = false;
        ui.pending = Some(Pending::Close);
        assert_eq!(ui.invert_selection(), 0);
        assert_eq!(ui.select_same_teams(), 0);
        assert!(ui.selected_objects().is_empty());
        assert_eq!(ui.objects(), objects);
        assert_eq!(ui.is_dirty(), dirty);
        assert_eq!((ui.undo_depth(), ui.redo_depth()), history);
    }

    #[test]
    fn test_multi_selection_workflow_toggle_range_all_and_clear() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();

        // 1. Single selection
        ui.select_object(Some((0, 1)));
        assert_eq!(ui.selected_object(), Some((0, 1)));
        assert_eq!(ui.selected_objects(), vec![(0, 1)]);
        assert!(ui.is_selected((0, 1)));
        assert!(!ui.is_selected((1, 2)));

        // 2. Toggle selection (Ctrl-click)
        ui.toggle_select_object((1, 2));
        assert_eq!(ui.selected_objects(), vec![(0, 1), (1, 2)]);
        assert!(ui.is_selected((0, 1)));
        assert!(ui.is_selected((1, 2)));
        assert_eq!(ui.selected_object(), Some((1, 2)));

        // Toggle deselect
        ui.toggle_select_object((0, 1));
        assert_eq!(ui.selected_objects(), vec![(1, 2)]);
        assert_eq!(ui.selected_object(), Some((1, 2)));

        // 3. Range selection (Shift-click)
        let objs = ui.objects();
        ui.select_object(Some((0, 1)));
        ui.select_range_from_visible(&objs, (2, 3));
        assert_eq!(ui.selected_objects(), vec![(0, 1), (1, 2), (2, 3)]);
        assert_eq!(ui.selected_object(), Some((2, 3)));

        // 4. Clear selection
        ui.clear_selection();
        assert!(ui.selected_objects().is_empty());
        assert_eq!(ui.selected_object(), None);

        // 5. Select all
        ui.select_all();
        assert_eq!(ui.selected_objects().len(), 3);

        // 6. Non-existent keys filtered out safely
        ui.select_objects(&[(0, 1), (99, 99)]);
        assert_eq!(ui.selected_objects(), vec![(0, 1)]);

        // 7. Toggle on non-existent key is safely ignored
        ui.toggle_select_object((99, 99));
        assert_eq!(ui.selected_objects(), vec![(0, 1)]);

        // 8. Select all of type and copy coordinates
        let coords = ui.selected_coordinates();
        assert!(coords.is_some());
        let count_type = ui.select_all_of_type();
        assert!(count_type >= 1);
        let first_type = ui.store.as_ref().unwrap().objects().iter().find(|o| o.slot == 0 && o.uid == 1).unwrap().type_id;
        for &(slot, uid) in &ui.selection {
            let t = ui.store.as_ref().unwrap().objects().iter().find(|o| o.slot == slot && o.uid == uid).unwrap().type_id;
            assert_eq!(t, first_type);
        }
    }

    #[test]
    fn batch9_type_selection_matches_all_for_data_children() {
        let temp = Temp::new();
        fixture(&temp.0);
        let path = temp.0.join("DATA/objects.dat");
        let mut bytes = fs::read(&path).unwrap();
        let parent = 16 + 3 * 79 + 3 * COLUMNS[..13].iter().sum::<usize>() + COLUMNS[13];
        bytes[parent..parent + 2].copy_from_slice(&0u16.to_le_bytes());
        for slot in [1, 2] {
            bytes[16 + slot * 79 + 75..16 + slot * 79 + 77].copy_from_slice(&1u16.to_le_bytes());
        }
        fs::write(path, bytes).unwrap();
        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();
        assert_eq!(ui.objects()[1].parent, Some(0));
        ui.select_all();
        let all = ui.selected_objects();
        ui.select_object(Some((1, 2)));
        assert_eq!(ui.select_all_of_type(), 2);
        assert_eq!(ui.selected_objects(), vec![(1, 2), (2, 3)]);
        assert!(all.contains(&(1, 2)));
        assert!(!ui.is_dirty());
    }

    #[test]
    fn batch9_type_selection_cancels_delete_and_coordinates_require_single() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();
        ui.select_objects(&[(0, 1), (1, 2)]);
        assert_eq!(ui.selected_coordinates(), None);
        ui.pending = Some(Pending::Remove { families: vec![(0, 1)] });
        ui.filter = "no visible matches".into();
        assert_eq!(ui.select_all_of_type(), 3);
        assert!(ui.pending.is_none());
        assert_eq!(ui.selected_objects(), vec![(0, 1), (1, 2), (2, 3)]);
        assert!(!ui.is_dirty());
        ui.select_object(Some((0, 1)));
        assert_eq!(ui.selected_coordinates(), Some((0., 0.)));
    }

    #[test]
    fn test_batch_move_atomic_and_undoable() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();

        ui.select_objects(&[(0, 1), (1, 2)]);
        ui.delta = ["5.0".into(), "0.0".into(), "-10.0".into()];
        ui.move_family().unwrap();

        assert!(ui.is_dirty());
        assert_eq!(ui.undo_depth(), 1);

        let objs = ui.objects();
        assert_eq!(objs[0].position, [5.0, 0.0, -10.0]);
        assert_eq!(objs[1].position, [25.0, 0.0, -10.0]); // initial X was 20.0
        assert_eq!(objs[2].position, [40.0, 0.0, 0.0]); // untouched

        // Zero delta does not record spurious undo step
        ui.delta = ["0.0".into(), "0.0".into(), "0.0".into()];
        ui.move_family().unwrap();
        assert_eq!(ui.undo_depth(), 1);

        // One atomic undo restores both
        ui.undo().unwrap();
        assert!(!ui.is_dirty());
        assert_eq!(ui.undo_depth(), 0);
        let objs = ui.objects();
        assert_eq!(objs[0].position, [0.0, 0.0, 0.0]);
        assert_eq!(objs[1].position, [20.0, 0.0, 0.0]);

        // Redo restores both
        ui.redo().unwrap();
        assert!(ui.is_dirty());
        let objs = ui.objects();
        assert_eq!(objs[0].position, [5.0, 0.0, -10.0]);
        assert_eq!(objs[1].position, [25.0, 0.0, -10.0]);
    }

    #[test]
    fn test_batch_rotation_team_and_batch_edit_atomic() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();

        ui.select_objects(&[(0, 1), (1, 2)]);

        // 1. Batch set team
        ui.team = "7".into();
        ui.set_team().unwrap();
        assert_eq!(ui.undo_depth(), 1);
        let objs = ui.objects();
        assert_eq!(objs[0].team, 7);
        assert_eq!(objs[1].team, 7);
        assert_eq!(objs[2].team, 0); // untouched (default 0 in fixture)

        // Undo reverts both
        ui.undo().unwrap();
        let objs = ui.objects();
        assert_eq!(objs[0].team, 0);
        assert_eq!(objs[1].team, 0);

        // 2. Batch relative rotate
        ui.angle = "90.0".into();
        ui.rotate_family(false).unwrap();
        assert_eq!(ui.undo_depth(), 1);
        let objs = ui.objects();
        assert_eq!(objs[0].rotation_degrees, 90.0);
        assert_eq!(objs[1].rotation_degrees, 90.0);

        // 3. Batch edit both team and rotation at once
        ui.batch_edit(Some(3), Some(180.0)).unwrap();
        assert_eq!(ui.undo_depth(), 2);
        let objs = ui.objects();
        assert_eq!(objs[0].team, 3);
        assert_eq!(objs[1].team, 3);
        assert_eq!(objs[0].rotation_degrees, 180.0);
        assert_eq!(objs[1].rotation_degrees, 180.0);

        // Single undo reverts batch_edit
        ui.undo().unwrap();
        let objs = ui.objects();
        assert_eq!(objs[0].team, 0);
        assert_eq!(objs[1].team, 0);
        assert_eq!(objs[0].rotation_degrees, 90.0);
        assert_eq!(objs[1].rotation_degrees, 90.0);
    }

    #[test]
    fn test_batch_clone_atomic_and_undoable() {
        let temp = Temp::new();
        fixture(&temp.0);
        let objects_path = temp.0.join("DATA/objects.dat");
        let position_path = temp.0.join("DATA/position.dat");
        let mut objects = fs::read(&objects_path).unwrap();
        objects[16 + 2 * 79] = 0; // free slot 2
        fs::write(&objects_path, objects).unwrap();
        let mut positions = fs::read(&position_path).unwrap();
        positions[8 + 2 * 17] = 0;
        fs::write(position_path, positions).unwrap();

        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();

        // Single free slot, but trying to clone 2 objects: must fail closed atomically
        ui.select_objects(&[(0, 1), (1, 2)]);
        ui.delta = ["10.0".into(), "0.0".into(), "0.0".into()];
        assert!(ui.clone_family().is_err());
        assert!(!ui.is_dirty());
        assert_eq!(ui.object_count(), 2);
        assert_eq!(ui.undo_depth(), 0);

        // Now with single object clone
        ui.select_object(Some((0, 1)));
        ui.clone_family().unwrap();
        assert!(ui.is_dirty());
        assert_eq!(ui.object_count(), 3);
        assert_eq!(ui.undo_depth(), 1);
        assert_eq!(ui.selected_object(), Some((2, 3)));

        ui.undo().unwrap();
        assert_eq!(ui.object_count(), 2);
        assert!(!ui.is_dirty());
    }

    #[test]
    fn test_batch_snap_to_grid_atomic_and_undoable() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();

        ui.select_objects(&[(0, 1), (1, 2)]);
        let orig0 = ui.objects()[0].position;
        let orig1 = ui.objects()[1].position;

        ui.snap_to_grid().unwrap();
        assert_eq!(ui.undo_depth(), 1);

        let objs = ui.objects();
        assert_eq!(objs[0].position[0].rem_euclid(256.0), 128.0);
        // Relative distance between object 0 and object 1 is preserved
        assert_eq!(
            objs[1].position[0] - objs[0].position[0],
            orig1[0] - orig0[0]
        );

        ui.undo().unwrap();
        let objs = ui.objects();
        assert_eq!(objs[0].position, orig0);
        assert_eq!(objs[1].position, orig1);
    }

    #[test]
    fn test_batch_remove_confirmation_and_undoable() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();

        ui.select_objects(&[(0, 1), (1, 2)]);
        ui.request_remove().unwrap();

        // Mutating selection causes pending confirmation to fail closed
        ui.select_object(Some((2, 3)));
        assert!(ui.confirm_pending().is_err());
        assert_eq!(ui.object_count(), 3);

        // Re-request and confirm
        ui.select_objects(&[(0, 1), (1, 2)]);
        ui.request_remove().unwrap();
        ui.confirm_pending().unwrap();

        assert_eq!(ui.object_count(), 1);
        assert!(ui.is_dirty());
        assert_eq!(ui.undo_depth(), 1);
        assert!(ui.selected_objects().is_empty());

        // One undo restores both removed objects
        ui.undo().unwrap();
        assert_eq!(ui.object_count(), 3);
        assert!(!ui.is_dirty());
    }

    #[test]
    fn test_batch_operations_empty_selection_and_stale_objects_safety() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi::default();
        ui.load_now(&temp.0.display().to_string()).unwrap();

        // Empty selection fails closed on all batch actions
        ui.clear_selection();
        assert!(ui.move_family().is_err());
        assert!(ui.clone_family().is_err());
        assert!(ui.rotate_family(true).is_err());
        assert!(ui.rotate_family(false).is_err());
        assert!(ui.set_team().is_err());
        assert!(ui.snap_to_grid().is_err());
        assert!(ui.request_remove().is_err());
        assert!(ui.batch_edit(Some(1), None).is_err());
        assert!(!ui.is_dirty());
        assert_eq!(ui.undo_depth(), 0);

        // Stale object in selection fails closed without mutations
        ui.selected = Some((0, 1));
        ui.selection = vec![(0, 1), (1, 999)]; // (1, 999) has wrong UID
        assert_eq!(ui.move_family().unwrap_err(), "stale object UID");
        assert_eq!(ui.clone_family().unwrap_err(), "stale object UID");
        assert_eq!(ui.rotate_family(true).unwrap_err(), "stale object UID");
        assert_eq!(ui.set_team().unwrap_err(), "stale object UID");
        assert_eq!(ui.snap_to_grid().unwrap_err(), "stale object UID");
        assert_eq!(ui.request_remove().unwrap_err(), "stale object UID");
        assert_eq!(
            ui.batch_edit(Some(2), None).unwrap_err(),
            "stale object UID"
        );
        assert!(!ui.is_dirty());
        assert_eq!(ui.undo_depth(), 0);
    }

    #[test]
    fn screen_box_maps_to_inclusive_tile_rect_regardless_of_drag_direction() {
        // 64 tiles over a 640 px map at (100, 50): 10 px per tile.
        let (min, size) = ((100.0, 50.0), (640.0, 640.0));
        let forward = screen_box_to_tile_rect((125.0, 75.0), (161.0, 133.0), min, size, 64);
        assert_eq!(forward, TileRect { left: 2, top: 2, right: 7, bottom: 9 });
        let backward = screen_box_to_tile_rect((161.0, 133.0), (125.0, 75.0), min, size, 64);
        assert_eq!(forward, backward);
        // A click without drag selects its own tile (C# FromLTRB(.., max + 1)).
        let single = screen_box_to_tile_rect((105.0, 55.0), (105.0, 55.0), min, size, 64);
        assert_eq!(single, TileRect { left: 0, top: 0, right: 1, bottom: 1 });
        // Dragging past the map clamps to the border tiles.
        let wide = screen_box_to_tile_rect((-500.0, -500.0), (9999.0, 9999.0), min, size, 64);
        assert_eq!(wide, TileRect { left: 0, top: 0, right: 64, bottom: 64 });
        assert_eq!(norm_to_tile(f32::NAN, 64), 0);
        assert_eq!(norm_to_tile(1.0, 64), 63);
    }

    #[test]
    fn box_select_replaces_or_adds_with_shift() {
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi {
            map_input: temp.0.display().to_string(),
            ..Default::default()
        };
        ui.request_load().unwrap();
        let tile0 = TileRect { left: 0, top: 0, right: 1, bottom: 1 };
        let far = TileRect { left: 10, top: 10, right: 12, bottom: 12 };
        assert_eq!(ui.select_in_tile_rect(tile0, false), 3, "fixture objects sit in tile (0,0)");
        assert_eq!(ui.selected_objects().len(), 3);
        // An empty replace clears; an empty additive box keeps the selection.
        assert_eq!(ui.select_in_tile_rect(far, true), 0);
        assert_eq!(ui.selected_objects().len(), 3);
        assert_eq!(ui.select_in_tile_rect(far, false), 0);
        assert!(ui.selected_objects().is_empty());
        ui.select_object(Some((0, 1)));
        ui.select_in_tile_rect(tile0, true);
        assert_eq!(ui.selected_objects().len(), 3, "additive union has no duplicates");
        assert!(!ui.is_dirty(), "selection never dirties DATA");
    }

    #[test]
    fn layout_export_import_apply_roundtrip_is_one_undo_step() {
        use crate::layout_io::{LayoutApplyOptions, LayoutContext, LayoutDialogKind};
        let temp = Temp::new();
        fixture(&temp.0);
        free_last_slot(&temp.0); // slot 2 becomes the single free native slot
        let file = temp.0.join("saved.arm-layout.json");
        let mut ui = PlacementUi {
            map_input: temp.0.display().to_string(),
            ..Default::default()
        };
        let target = file.clone();
        ui.layout_picker = Some(Box::new(move |kind| match kind {
            LayoutDialogKind::Save { suggested_name } => {
                assert_eq!(suggested_name, "settlement.arm-layout.json");
                Some(target.clone())
            }
            LayoutDialogKind::Open => Some(target.clone()),
        }));
        let names = vec![(0, "RomA".to_string())];
        let ground = |_: f32, _: f32| 0.0;
        let ctx = LayoutContext { names: &names, ground: &ground };

        ui.request_load().unwrap();
        // Nothing selected: export is refused and nothing is written.
        ui.request_layout(LayoutRequest::ExportSelection);
        ui.process_layout_requests(&ctx);
        assert!(ui.layout_status().contains("Select placed objects"), "{}", ui.layout_status());
        assert!(!file.exists());

        ui.select_object(Some((1, 2)));
        ui.request_layout(LayoutRequest::ExportSelection);
        ui.process_layout_requests(&ctx);
        let saved = crate::layout_io::read_layout_file(&file).unwrap();
        assert_eq!(saved.entries.len(), 1);
        assert_eq!(saved.entries[0].type_name, "RomA");
        assert_eq!((saved.entries[0].x, saved.entries[0].z), (0.0, 0.0), "relative to the min corner");
        assert_eq!(saved.entries[0].team, 0);

        // Import opens the dialog; apply is queued only after confirmation.
        let before = ui.object_count();
        ui.begin_layout_import();
        assert!(ui.has_pending_layout());
        ui.set_layout_options(LayoutApplyOptions { anchor_tile_x: 10.0, anchor_tile_z: 12.0, rotation: 90.0, team: Some(4) });
        ui.layout_team_override = true;
        ui.confirm_layout_apply().unwrap();
        assert!(!ui.has_pending_layout() && ui.has_layout_requests());
        let undo_before = ui.undo_depth();
        ui.process_layout_requests(&ctx);
        assert!(ui.layout_status().contains("已套用配置"), "{}", ui.layout_status());
        assert_eq!(ui.object_count(), before + 1);
        assert_eq!(ui.undo_depth(), undo_before + 1, "layout apply is one undo step");
        assert_eq!(ui.selected_objects().len(), 1, "new object is selected");
        let placed = ui.objects().into_iter().find(|o| o.slot == 2).unwrap();
        assert!((placed.position[0] - 2560.0).abs() < 0.1 && (placed.position[2] - 3072.0).abs() < 0.1, "{:?}", placed.position);
        assert_eq!(placed.team, 4);
        ui.undo().unwrap();
        assert_eq!(ui.object_count(), before);
    }

    #[test]
    fn layout_apply_rejects_unknown_types_and_bad_options_without_editing() {
        use crate::layout_io::{LayoutApplyOptions, LayoutContext};
        let temp = Temp::new();
        fixture(&temp.0);
        let mut ui = PlacementUi {
            map_input: temp.0.display().to_string(),
            ..Default::default()
        };
        ui.request_load().unwrap();
        let names = vec![(0, "RomA".to_string())];
        let ground = |_: f32, _: f32| 0.0;
        let ctx = LayoutContext { names: &names, ground: &ground };
        let preset = crate::layout_io::deserialize(
            r#"{"version":1,"kind":"Placement","entries":[{"type":"RomA"},{"type":"Missing"}]}"#,
        )
        .unwrap();
        ui.request_layout(LayoutRequest::Apply { preset: preset.clone(), options: LayoutApplyOptions::default() });
        ui.process_layout_requests(&ctx);
        assert!(ui.layout_status().contains("Missing placement type: Missing"), "{}", ui.layout_status());
        assert!(!ui.is_dirty() && ui.undo_depth() == 0, "whole layout rejected");

        // Out-of-range dialog values are refused at confirmation and keep the dialog open.
        let path = temp.0.join("a.arm-layout.json");
        crate::layout_io::write_layout_file(&path, &preset).unwrap();
        ui.load_layout_from(&path);
        ui.set_layout_options(LayoutApplyOptions { anchor_tile_x: 99.0, ..Default::default() });
        assert!(ui.confirm_layout_apply().is_err());
        assert!(ui.has_pending_layout());
        ui.cancel_layout_apply();
        assert!(!ui.has_pending_layout());

        // Corrupt files never open the dialog.
        fs::write(&path, "{ not json").unwrap();
        ui.load_layout_from(&path);
        assert!(!ui.has_pending_layout());
        assert!(ui.layout_status().contains("Layout failed"));
    }

    #[test]
    fn nature_layout_export_uses_landscape_names_inside_tile_rect() {
        use crate::layout_io::{LayoutContext, LayoutDialogKind};
        let temp = Temp::new();
        fixture(&temp.0);
        let file = temp.0.join("forest.arm-layout.json");
        let mut ui = PlacementUi {
            map_input: temp.0.display().to_string(),
            ..Default::default()
        };
        let target = file.clone();
        ui.layout_picker = Some(Box::new(move |kind| {
            if let LayoutDialogKind::Save { suggested_name } = kind {
                assert_eq!(suggested_name, "forest.arm-layout.json");
            }
            Some(target.clone())
        }));
        ui.request_load().unwrap();
        let names = vec![(0, "LanTree".to_string())];
        let ground = |_: f32, _: f32| 0.0;
        let ctx = LayoutContext { names: &names, ground: &ground };
        ui.request_layout(LayoutRequest::ExportNature(TileRect { left: 0, top: 0, right: 1, bottom: 1 }));
        ui.process_layout_requests(&ctx);
        assert!(file.exists(), "{}", ui.layout_status());
        let saved = crate::layout_io::read_layout_file(&file).unwrap();
        assert_eq!(saved.kind, MapLayoutKind::Nature);
        assert_eq!(saved.entries.len(), 3);
        assert!(saved.entries.iter().all(|e| e.type_name == "LanTree" && e.team == 0 && e.count == 0));
        assert_eq!(saved.entries[2].x, 40.0, "relative to the rectangle corner");
        // Outside the rectangle, or non-landscape names: error and nothing written.
        fs::remove_file(&file).unwrap();
        ui.request_layout(LayoutRequest::ExportNature(TileRect { left: 20, top: 20, right: 21, bottom: 21 }));
        ui.process_layout_requests(&ctx);
        assert!(!file.exists());
        assert!(ui.layout_status().contains("No landscape objects"));
        let buildings = vec![(0, "RomA".to_string())];
        let ctx = LayoutContext { names: &buildings, ground: &ground };
        ui.request_layout(LayoutRequest::ExportNature(TileRect { left: 0, top: 0, right: 1, bottom: 1 }));
        ui.process_layout_requests(&ctx);
        assert!(!file.exists(), "buildings are not nature");
    }
}

#[cfg(test)]
mod navigation_building_tests {
    use super::*;
    #[test]
    fn classification_and_pending_family_positions_use_shared_margins() {
        let mut ui = PlacementUi::default();
        ui.set_type_names([(1, "BauGerHaus".into()), (2, "FigGer".into())].into());
        let mut objects = vec![
            WorldObject { slot: 0, type_id: 1, team: 0, uid: 1, position: [8192.0, 0.0, 8192.0],
                rotation_degrees: 0.0, parent: None, vitals: None },
            WorldObject { slot: 1, type_id: 2, team: 0, uid: 2, position: [8256.0, 0.0, 8192.0],
                rotation_degrees: 0.0, parent: Some(0), vitals: None },
        ];
        let margin = against_rome_maps::placement_edit::spawn_data::approximate_family_margin(
            against_rome_maps::MapDesignValidator::is_main_house("BauGerHaus"));
        assert_eq!(ui.navigation_buildings(&objects),
            vec![[8192.0-margin, 8256.0+margin, 8192.0-margin, 8192.0+margin]]);
        objects[0].position[0] -= 1024.0;
        assert_eq!(ui.navigation_buildings(&objects)[0][0], 7168.0-margin);
        objects[0].type_id = 2;
        assert!(ui.navigation_buildings(&objects).is_empty());
    }
}

#[cfg(test)]
mod batch20_tests {
    use super::*;
    #[test]
    fn chinese_ui_has_no_untranslated_literals() {
        let mut panel = PlacementUi::default();
        panel.open = true;
        panel.map_input = "DATA-test-path".into();
        for english in [true, false, true, false] {
            panel.english = english;
            let text = crate::batch20_localization_tests::render(|ctx| panel.show(ctx));
            assert!(text.contains(if english { "Load DATA" } else { "載入 DATA" }), "{text}");
            if !english {
                for literal in ["Load DATA", "Close tool", "Filter slot", "Select all", "Clear selection", "Select rectangle", "Set rotation", "Rotate by", "Set team", "Snap to tile", "Move family", "Clone family", "Remove family", "Undo", "Redo", "Save DATA", "Reset DATA"] {
                    assert!(!text.contains(literal), "未翻譯：{literal}\n{text}");
                }
            }
            assert_eq!(panel.map_input, "DATA-test-path");
            assert!(!panel.is_loaded());
            assert!(!panel.is_dirty());
            assert!(panel.selection.is_empty());
        }
    }
}
