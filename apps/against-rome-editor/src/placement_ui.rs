//! DATA object-family editor. Its Save is independent of terrain/image saving.
use crate::design_apply::{apply_placements, PlacementBatch, PlacementRequest};
use against_rome_placement::{ObjectStore, WorldObject};
use eframe::egui;
use std::{
    fs,
    path::{Path, PathBuf},
};

type Result<T> = std::result::Result<T, String>;

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

pub struct PlacementUi {
    pub open: bool,
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
    pending: Option<Pending>,
    status: String,
    /// History entries per user-visible edit (a batch apply is one undo step).
    undo_groups: Vec<usize>,
    redo_groups: Vec<usize>,
    /// Raised by the Save button; `EditorApp` consumes it and routes to the central Safe Save.
    save_requested: bool,
}
impl Default for PlacementUi {
    fn default() -> Self {
        Self {
            open: false,
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
            pending: None,
            status: String::new(),
            undo_groups: Vec::new(),
            redo_groups: Vec::new(),
            save_requested: false,
        }
    }
}
impl PlacementUi {
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
        self.baseline = None;
        self.loaded_map = None;
        self.selected = None;
        self.selection.clear();
        self.filter.clear();
        self.pending = None;
        self.status.clear();
        self.undo_groups.clear();
        self.redo_groups.clear();
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
    fn record_edit(&mut self, entries: usize) {
        if entries > 0 {
            self.undo_groups.push(entries);
            self.redo_groups.clear();
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
        self.pending = None;
        Ok(())
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
        ui.label("矩形選取 tiles：[left, right) × [top, bottom)，world X/Z，每 tile 256；取代選取（忽略 Filter）");
        ui.horizontal_wrapped(|ui| {
            for (index, name) in ["left", "top", "right", "bottom"].iter().enumerate() {
                ui.label(*name);
                ui.add(
                    egui::TextEdit::singleline(&mut self.tile_bounds[index])
                        .id_source(("placement-tile-bound", index))
                        .desired_width(60.0),
                );
            }
            if ui
                .add_enabled(self.is_loaded(), egui::Button::new("Select rectangle"))
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
        egui::Window::new("Rust 物件 / Family 工具").open(&mut visible)
            .default_width(760.0).show(ctx, |ui| {
            ui.label("DATA 獨立編輯階段：此處 Save 只保存 DATA 物件與關聯池，不保存 terrain、材質或劇本。");
            ui.label("只編輯明確指定的 MAP；安裝目錄允許唯讀載入，保存請使用工作副本。");
            ui.label("MAP 完整路徑（包含 DATA 的資料夾）");
            ui.text_edit_singleline(&mut self.map_input);
            ui.horizontal(|ui| {
                if ui.button("Load DATA").clicked() { let r = self.request_load(); self.report(r); }
                if ui.button("Close tool").clicked() { self.request_close(); }
            });
            if let Some(path) = &self.loaded_map {
                ui.label(format!("實際載入：{} {}", path.display(), if self.is_dirty() {"（未保存）"} else {"（已保存）"}));
            }
            if let Some(store) = &self.store {
                ui.label(format!("Active objects={} / capacity={}",store.objects().len(),store.capacity()));
            }
            ui.horizontal(|ui| {
                ui.label("Filter slot / UID / type / team / parent");
                ui.text_edit_singleline(&mut self.filter);
                if ui.button("Select all").clicked() {
                    self.select_all();
                }
                if ui.button("Clear selection").clicked() {
                    self.clear_selection();
                }
            });
            self.tile_bounds_form(ui);
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
            ui.label("Move / Clone 的相對 world 位移；X/Y/Z → signed 16.16（四捨五入至 1/65536）。");
            ui.horizontal(|ui| {for axis in 0..3 {ui.label(["ΔX","ΔY","ΔZ"][axis]);ui.add(egui::TextEdit::singleline(&mut self.delta[axis]).desired_width(95.0));}});
            if let Ok(delta)=fixed_delta(&self.delta) {ui.label(format!("16.16 delta: {delta:?}"));}
            ui.label("地圖外座標沿用原有 family 契約，不自動 clamp。未知／不支援關聯由 core 拒絕，錯誤原樣顯示。");
            ui.horizontal(|ui| {
                ui.label("角度（度）");ui.add(egui::TextEdit::singleline(&mut self.angle).desired_width(70.0));
                if ui.button("Set rotation").clicked() {let r=self.rotate_family(true);self.report(r);}
                if ui.button("Rotate by").clicked() {let r=self.rotate_family(false);self.report(r);}
                ui.label("隊伍 0..15 / -1");ui.add(egui::TextEdit::singleline(&mut self.team).desired_width(40.0));
                if ui.button("Set team").clicked() {let r=self.set_team();self.report(r);}
                if ui.button("Snap to tile").clicked() {let r=self.snap_to_grid();self.report(r);}
            });
            ui.horizontal_wrapped(|ui| {
                let count = self.selected_families().len();
                let move_label = if count > 1 { format!("Move {count} families") } else { "Move family".into() };
                let clone_label = if count > 1 { format!("Clone {count} families") } else { "Clone family".into() };
                let remove_label = if count > 1 { format!("Remove {count} families…") } else { "Remove family…".into() };

                if ui.button(move_label).clicked() {let r=self.move_family();self.report(r);}
                if ui.button(clone_label).clicked() {let r=self.clone_family();self.report(r);}
                if ui.button(remove_label).clicked() {let r=self.request_remove();self.report(r);}
                if count > 1 {
                    if ui.button("Batch edit team & rotation").clicked() {
                        let r = self.apply_batch_edit_fields();
                        self.report(r);
                    }
                }
                let can_undo=!self.undo_groups.is_empty();
                let can_redo=!self.redo_groups.is_empty();
                if ui.add_enabled(can_undo,egui::Button::new("Undo")).clicked() {let r=self.undo();self.report(r);}
                if ui.add_enabled(can_redo,egui::Button::new("Redo")).clicked() {let r=self.redo();self.report(r);}
                if ui.add_enabled(self.is_dirty(),egui::Button::new("Save DATA")).clicked() {self.request_save();}
                if ui.add_enabled(self.is_loaded(),egui::Button::new("Reset DATA…")).clicked() {let r=self.request_reset();self.report(r);}
            });
            ui.separator();ui.label(&self.status);
        });
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
                egui::epaint::Shape::Text(text) if text.galley.text() == "Select rectangle" => {
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
        for label in ["left", "top", "right", "bottom"] {
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
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
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
        let data = map.join("DATA");
        fs::create_dir_all(&data).unwrap();
        let n = 3usize;
        let mut objects = vec![0u8; 16 + n * (79 + COLUMNS.iter().sum::<usize>())];
        objects[..4].copy_from_slice(&1u32.to_le_bytes());
        objects[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        objects[8..12].copy_from_slice(&30u32.to_le_bytes());
        objects[12..16].copy_from_slice(&30u32.to_le_bytes());
        for slot in 0..n {
            let r = 16 + slot * 79;
            objects[r] = 1;
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
            position[8 + slot * 17] = 1;
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
}
