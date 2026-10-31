//! Map Browser, Catalog, Cloner, Deleter, Design Validator, Diagnostics, and Preflight UI.
//! Replicates C# `MapSelectionForm.cs` and integrates `native/against-rome-maps`.

use against_rome_maps::{
    blank_builder::{BlankMapBuildResult, BlankMapBuilder, BlankMapSpec},
    catalog::{EndlessMapCatalog, EndlessMapInfo, GameMapCatalog, GameMapInfo},
    cloner::EndlessMapCloner,
    deleter::EndlessMapDeleter,
    design_validator::{DesignIssueSeverity, MapDesignIssue, MapDesignValidationRequest, MapDesignValidator},
    diagnostics::{MapCheckObject, MapCheckSnapshot, MapDiagnostics, MapIssue, MapIssueSeverity},
    preflight::ScenarioSavePreflight,
};
use against_rome_scenario::model::{Event, ScenarioDocument, Spawn};
use eframe::egui::{self, Color32, Vec2};
use std::path::{Path, PathBuf};

pub type GameFolderPicker = Box<dyn Fn() -> Option<PathBuf> + Send + Sync>;

fn native_game_folder_dialog(current_path: &str) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new()
        .set_title("Select Against Rome Game Folder / 選擇 Against Rome 遊戲資料夾");
    let current = Path::new(current_path.trim());
    if current.is_dir() {
        dialog = dialog.set_directory(current);
    }
    dialog.pick_folder()
}

/// Read-only view of the live editor state handed to the panel every frame.
#[derive(Clone, Copy, Default)]
pub struct MapsContext<'a> {
    pub scenario: Option<&'a ScenarioDocument>,
    pub aliases: &'a [String],
    pub heights: Option<&'a [u8]>,
    /// Height grid vertex count; the collision layer is `height_size - 1` square.
    pub height_size: usize,
    pub collision: Option<&'a [u8]>,
    pub height_step: f32,
    pub water_level: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapsTab {
    Catalog,
    ClonerDeleter,
    NewBlankMap,
    DesignValidator,
    Diagnostics,
    Preflight,
}

#[derive(Clone, Debug)]
pub enum MapsPendingAction {
    DeleteCustom(i32),
    DiscardAndClose,
}

/// Typed intent to build and open a blank map.
/// Execution is deferred to the owning app so dirty sessions can be guarded
/// before any filesystem mutations take place.
#[derive(Clone, Debug, PartialEq)]
pub struct BlankMapBuildIntent {
    pub game_path: PathBuf,
    pub spec: BlankMapSpec,
    /// C# "Flat Template" (`MapSelectionForm.CreateBlankMap`): clone the template as is (settlements,
    /// scripts and DATA kept) and flatten the terrain once it opens, instead of the Empty Scene build.
    pub flat_template: bool,
}

impl BlankMapBuildIntent {
    pub fn new(game_path: impl Into<PathBuf>, spec: BlankMapSpec) -> Self {
        Self {
            game_path: game_path.into(),
            spec,
            flat_template: false,
        }
    }

    /// Marks this intent as the Flat Template flow (clone, open, then flatten the terrain).
    pub fn with_flat_template(mut self) -> Self {
        self.flat_template = true;
        self
    }
}

pub struct MapsUi {
    pub open: bool,
    pub english: bool,
    pub current_tab: MapsTab,
    pub game_path_input: String,
    /// Injectable for UI tests; the desktop default opens the native folder picker.
    pub game_folder_picker: Option<GameFolderPicker>,
    pub endless_maps: Vec<EndlessMapInfo>,
    pub general_maps: Vec<GameMapInfo>,
    pub selected_map_id: Option<String>,
    pub selected_map_path: Option<PathBuf>,
    pub selected_is_custom: bool,

    // Cloner inputs
    pub clone_source_slot: i32,
    pub clone_target_slot: i32,
    pub clone_new_name: String,

    // Deleter confirmation
    pub delete_confirm_slot: Option<i32>,

    // Blank Map Builder inputs
    pub blank_source_slot: i32,
    pub blank_target_slot: i32,
    pub blank_target_name: String,
    pub blank_height_override: u8,
    pub blank_include_starter_kit: bool,
    pub blank_starter_team: i32,
    pub blank_starter_x: f32,
    pub blank_starter_z: f32,
    pub blank_briefing: String,
    pub blank_last_result: Option<BlankMapBuildResult>,

    // Validation & Diagnostics results
    pub design_issues: Vec<MapDesignIssue>,
    pub diagnostic_issues: Vec<MapIssue>,
    pub preflight_result: Option<Result<String, String>>,

    // Pending confirmation & dirty state
    pub pending_action: Option<MapsPendingAction>,
    pub pending_build_intent: Option<BlankMapBuildIntent>,
    pub request_skirmish_wizard: bool,
    pub status_message: String,
    pub status_is_error: bool,
    pub dirty: bool,
}

fn tl<'a>(english: bool, zh: &'a str, en: &'a str) -> &'a str {
    if english {
        en
    } else {
        zh
    }
}

/// Same check as [`map_name_error`] for `Err` paths that are language-agnostic: "English / 中文".
fn map_name_error_both(name: &str) -> Option<String> {
    let en = map_name_error(true, name)?;
    Some(format!("{en} / {}", map_name_error(false, name)?))
}

/// Early, bilingual check of a new map name so the designer sees the problem next to the field
/// instead of after a discard prompt or a rolled-back transaction. Mirrors the cloner/builder rules.
pub fn map_name_error(english: bool, name: &str) -> Option<String> {
    let msg = if name.trim().is_empty() {
        tl(english, "地圖名稱不可空白。", "Map name must not be blank.")
    } else if name.chars().any(|c| c.is_control() || "/\\:*?\"<>|".contains(c)) {
        tl(
            english,
            "地圖名稱含不允許的字元（控制字元或 / \\ : * ? \" < > |）。",
            "Map name contains forbidden characters (control characters or / \\ : * ? \" < > |).",
        )
    } else if !against_rome_maps::text_encoding::can_encode_game_text(name) {
        tl(
            english,
            "地圖名稱包含遊戲無法儲存的字元；請改用英文、數字或原版使用的字元。",
            "Map name has characters the game cannot store; use English letters, digits or characters used by retail maps.",
        )
    } else {
        return None;
    };
    Some(msg.to_string())
}

impl Default for MapsUi {
    fn default() -> Self {
        Self {
            open: false,
            english: false,
            current_tab: MapsTab::Catalog,
            game_path_input: String::new(),
            game_folder_picker: None,
            endless_maps: Vec::new(),
            general_maps: Vec::new(),
            selected_map_id: None,
            selected_map_path: None,
            selected_is_custom: false,
            clone_source_slot: 0,
            clone_target_slot: 5,
            clone_new_name: "Custom Map".to_string(),
            delete_confirm_slot: None,
            blank_source_slot: 0,
            blank_target_slot: 5,
            blank_target_name: "New Blank Map".to_string(),
            blank_height_override: 32,
            blank_include_starter_kit: true,
            blank_starter_team: 0,
            blank_starter_x: 8192.0,
            blank_starter_z: 8192.0,
            blank_briefing: String::new(),
            blank_last_result: None,
            design_issues: Vec::new(),
            diagnostic_issues: Vec::new(),
            preflight_result: None,
            pending_action: None,
            pending_build_intent: None,
            request_skirmish_wizard: false,
            status_message: "Ready.".to_string(),
            status_is_error: false,
            dirty: false,
        }
    }
}

impl MapsUi {
    pub fn is_dirty(&self) -> bool {
        self.dirty || self.pending_action.is_some()
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
        self.pending_action = None;
    }

    /// Takes any pending blank map build intent deferred for the owner application.
    pub fn take_pending_build_intent(&mut self) -> Option<BlankMapBuildIntent> {
        self.pending_build_intent.take()
    }

    /// Constructs a typed BlankMapBuildIntent from the current UI input fields without touching the filesystem.
    pub fn blank_map_build_intent(&self) -> BlankMapBuildIntent {
        let source_id = format!("ENDL_{:03}", self.blank_source_slot);
        let mut spec = BlankMapSpec::new(source_id, self.blank_target_slot, &self.blank_target_name)
            .with_height_override(self.blank_height_override);
        if self.blank_include_starter_kit {
            spec = spec.with_starter_kit(
                self.blank_starter_team,
                self.blank_starter_x,
                self.blank_starter_z,
            );
        }
        let trimmed_briefing = self.blank_briefing.trim();
        if !trimmed_briefing.is_empty() {
            spec = spec.with_briefing(trimmed_briefing);
        }
        let game_path = PathBuf::from(self.game_path_input.trim());
        BlankMapBuildIntent::new(game_path, spec)
    }

    /// C# "Flat Template" request: clones the selected template into the next free custom slot and
    /// asks the owning app to open it with flattened terrain. Does NOT touch the filesystem.
    pub fn request_build_flat_template(&mut self) -> Result<BlankMapBuildIntent, String> {
        let game_path = PathBuf::from(self.game_path_input.trim());
        if !game_path.is_dir() {
            return Err(format!("Game root path is invalid or does not exist: {}", game_path.display()));
        }
        if let Some(e) = map_name_error_both(&self.blank_target_name) {
            return Err(e);
        }
        let name = self.blank_target_name.trim().to_string();
        let slot = EndlessMapCatalog::new().get_next_free_slot(&game_path)?;
        self.blank_target_slot = slot;
        let mut spec = BlankMapSpec::new(format!("ENDL_{:03}", self.blank_source_slot), slot, name);
        let trimmed_briefing = self.blank_briefing.trim();
        if !trimmed_briefing.is_empty() {
            spec = spec.with_briefing(trimmed_briefing);
        }
        let intent = BlankMapBuildIntent::new(game_path, spec).with_flat_template();
        self.pending_build_intent = Some(intent.clone());
        Ok(intent)
    }

    /// 尋找下一個可用的自訂地圖槽位，不建立任何地圖。
    pub fn select_next_blank_slot(&mut self) -> Result<i32, String> {
        let path = Path::new(self.game_path_input.trim());
        if !path.is_dir() {
            return Err("Game root path is invalid or does not exist.".into());
        }
        let slot = EndlessMapCatalog::new().get_next_free_slot(path)?;
        self.blank_target_slot = slot;
        Ok(slot)
    }
    /// Requests building a blank map by queuing a typed build intent.
    /// Does NOT perform any filesystem mutation; the owning app is responsible for
    /// verifying clean/discard state before executing the build.
    pub fn request_build_blank_map(&mut self) -> BlankMapBuildIntent {
        let intent = self.blank_map_build_intent();
        self.pending_build_intent = Some(intent.clone());
        intent
    }

    /// Resets map-bound validation issues, diagnostic reports, preflight results, blank build reports,
    /// and transient confirmation actions.
    /// Preserves user preferences (open state, current tab, game_path_input, game_folder_picker,
    /// cached endless/general maps catalog, cloner/blank tool parameters).
    pub fn reset_session(&mut self, new_map_path: Option<&Path>) {
        self.design_issues.clear();
        self.diagnostic_issues.clear();
        self.preflight_result = None;
        self.blank_last_result = None;
        self.delete_confirm_slot = None;
        self.pending_action = None;
        self.pending_build_intent = None;
        self.dirty = false;
        self.status_message.clear();
        self.status_is_error = false;
        self.selected_map_path = new_map_path.map(|p| p.to_path_buf());
    }

    pub fn request_close(&mut self) {
        if self.is_dirty() {
            self.pending_action = Some(MapsPendingAction::DiscardAndClose);
        } else {
            self.open = false;
        }
    }

    pub fn refresh_catalog(&mut self) {
        let path = Path::new(self.game_path_input.trim());
        if !path.exists() {
            self.status_message = format!("Game path does not exist: {}", path.display());
            self.status_is_error = true;
            return;
        }

        let mut endless_error = None;
        match EndlessMapCatalog::new().list(path) {
            Ok(maps) => {
                self.endless_maps = maps;
                self.status_message = format!("Discovered {} endless maps.", self.endless_maps.len());
                self.status_is_error = false;
            }
            Err(e) => {
                endless_error = Some(e.to_string());
                self.status_message = format!("Failed to list endless maps: {e}");
                self.status_is_error = true;
            }
        }

        match GameMapCatalog::new().list(path) {
            Ok(maps) => {
                self.general_maps = maps;
            }
            Err(e) => {
                let general_error = e.to_string();
                self.status_message = if let Some(endless_error) = endless_error {
                    format!("Failed to list endless maps: {endless_error}; Failed to list general maps: {general_error}")
                } else {
                    format!("Failed to list general maps: {general_error}")
                };
                self.status_is_error = true;
            }
        }
    }

    fn localized_status_message(&self) -> String {
        let message = self.status_message.as_str();
        let fixed = match message {
            "Ready." => Some(("就緒。", "Ready.")),
            "No scenario loaded for diagnostics." => Some(("尚未載入劇本，無法執行診斷。", "No scenario loaded for diagnostics.")),
            "Preflight verification passed." => Some(("存檔預檢通過。", "Preflight verification passed.")),
            _ => None,
        };
        if let Some((chinese, english)) = fixed {
            return tl(self.english, chinese, english).to_string();
        }

        if message.contains("; Failed to list general maps: ") {
            return Self::localized_catalog_errors(message, self.english);
        }

        if let Some((success, catalog_errors)) = message.split_once(" Catalog refresh failed: ") {
            let localized_success = self.localized_success_status(success);
            return format!("{}{}", localized_success, tl(self.english, " 地圖目錄重新整理失敗：", " Catalog refresh failed: "))
                + &Self::localized_catalog_errors(catalog_errors, self.english);
        }

        for (prefix, chinese_prefix) in [
            ("Design Validator completed: ", "設計驗證完成："),
            ("Diagnostics completed: ", "地圖診斷完成："),
        ] {
            if let Some(count) = message.strip_prefix(prefix).and_then(|rest| rest.strip_suffix(" issues found.")) {
                return if self.english {
                    message.to_string()
                } else {
                    format!("{chinese_prefix}{count} 項問題。")
                };
            }
        }

        let dynamic = [
            ("Game path does not exist: ", "遊戲路徑不存在：", "Game path does not exist: "),
            ("Selected game folder does not exist: ", "所選遊戲資料夾不存在：", "Selected game folder does not exist: "),
            ("Failed to list endless maps: ", "列出無盡地圖失敗：", "Failed to list endless maps: "),
            ("Failed to list general maps: ", "列出一般地圖失敗：", "Failed to list general maps: "),
            ("Preflight blocked: ", "存檔預檢未通過：", "Preflight blocked: "),
            ("Delete failed: ", "刪除失敗：", "Delete failed: "),
            ("Copy failed: ", "複製失敗：", "Copy failed: "),
            ("Clone failed: ", "複製地圖失敗：", "Clone failed: "),
            ("Spec validation failed: ", "規格驗證失敗：", "Spec validation failed: "),
            ("規格驗證失敗: ", "規格驗證失敗：", "Spec validation failed: "),
            ("Flat template build failed: ", "平坦範本建置失敗：", "Flat template build failed: "),
            ("平坦範本請求失敗: ", "平坦範本請求失敗：", "Flat template request failed: "),
        ];
        for (prefix, chinese_prefix, english_prefix) in dynamic {
            if let Some(detail) = message.strip_prefix(prefix) {
                return format!("{}{}", tl(self.english, chinese_prefix, english_prefix), detail);
            }
        }

        if let Some(count) = message.strip_prefix("Discovered ").and_then(|rest| rest.strip_suffix(" endless maps.")) {
            return format!("{}{}{}", tl(self.english, "已找到 ", "Discovered "), count, tl(self.english, " 個無盡地圖。", " endless maps."));
        }
        if let Some(count) = message.strip_prefix("Blank map ENDL_").and_then(|rest| rest.strip_suffix(" created and verified successfully!")) {
            return format!("{}{}{}", tl(self.english, "空白地圖 ENDL_", "Blank map ENDL_"), count, tl(self.english, " 已建立並驗證完成！", " created and verified successfully!"));
        }
        if let Some(rest) = message.strip_prefix("Catalog refresh failed: ") {
            return format!("{}{}", tl(self.english, "地圖目錄重新整理失敗：", "Catalog refresh failed: "), Self::localized_catalog_errors(rest, self.english));
        }
        let localized_success = self.localized_success_status(message);
        if localized_success != message {
            return localized_success;
        }
        Self::localized_catalog_errors(message, self.english)
    }

    fn localized_success_status(&self, message: &str) -> String {
        if self.english {
            return message.to_string();
        }
        if let Some(rest) = message.strip_prefix("Copied ") {
            if let Some((source, rest)) = rest.split_once(" to custom map ") {
                if let Some((target, name)) = rest.strip_suffix(").").and_then(|value| value.split_once(" (")) {
                    return format!("已將 {source} 複製至自製地圖 {target}（{name}）。");
                }
            }
        }
        if let Some(rest) = message.strip_prefix("Cloned map to ") {
            if let Some((target, name)) = rest.strip_suffix(')').and_then(|value| value.split_once(" (")) {
                if let Some(target) = target.strip_suffix(" successfully!") {
                    return format!("已成功複製地圖至 {target}（{name}）。");
                }
            }
        }
        if let Some(slot) = message.strip_prefix("Successfully deleted custom map ENDL_").and_then(|value| value.strip_suffix('.')) {
            return format!("已成功刪除自製地圖 ENDL_{slot}。");
        }
        message.to_string()
    }

    fn localized_catalog_errors(message: &str, english: bool) -> String {
        if english {
            return message.to_string();
        }
        message
            .replace("Failed to list endless maps: ", "列出無盡地圖失敗：")
            .replace("; Failed to list general maps: ", "；列出一般地圖失敗：")
    }

    /// Opens the native folder picker and refreshes catalogs after a valid selection.
    /// Cancelling leaves both the current path and map lists untouched.
    pub fn browse_game_folder(&mut self) {
        let selected = if let Some(picker) = &self.game_folder_picker {
            picker()
        } else {
            native_game_folder_dialog(&self.game_path_input)
        };
        let Some(path) = selected else { return };
        if !path.is_dir() {
            self.status_message = format!("Selected game folder does not exist: {}", path.display());
            self.status_is_error = true;
            return;
        }
        self.game_path_input = path.to_string_lossy().into_owned();
        self.refresh_catalog();
    }

    /// C# `SuggestedCopyName`.
    /// C# `MapSelectionForm.CanCloneToCustom`: only Endless (ENDL) maps carry the settlement
    /// definitions a custom map needs, so campaign/other retail maps are refused as sources.
    pub fn can_clone_to_custom(map_id: &str, is_custom: bool) -> bool {
        is_custom || map_id.to_ascii_uppercase().starts_with("ENDL_")
    }

    /// Copy-name suggestion for any catalog entry (`"<name or id> - Copy"`).
    pub fn suggested_copy_name_for(display_name: Option<&str>, map_id: &str) -> String {
        format!("{} - Copy", display_name.unwrap_or(map_id))
    }

    /// "Copy to Custom Map": clones the selected catalog map into the next free custom slot, which
    /// registers its custom-map manifest so it is editable, then selects the new map.
    pub fn execute_copy_selected(&mut self) -> Result<EndlessMapInfo, String> {
        let source_id = self
            .selected_map_id
            .clone()
            .ok_or("Select a map from the catalog first.")?;
        if !Self::can_clone_to_custom(&source_id, self.selected_is_custom) {
            return Err(
                "Only Endless Mode (ENDL) maps can be copied to custom maps. Other map types lack endless settlement definitions."
                    .into(),
            );
        }
        let game_path = Path::new(self.game_path_input.trim()).to_path_buf();
        if !game_path.exists() {
            return Err("Game path does not exist.".into());
        }
        if let Some(e) = map_name_error_both(&self.clone_new_name) {
            return Err(e);
        }
        let name = self.clone_new_name.trim().to_string();
        let slot = EndlessMapCatalog::new().get_next_free_slot(&game_path)?;
        self.clone_target_slot = slot;
        let result = EndlessMapCloner::new().clone_from_map_id(&game_path, &source_id, slot, &name)?;
        self.status_is_error = false;
        self.refresh_catalog();
        self.selected_map_id = Some(result.id());
        self.selected_map_path = Some(result.directory_path.clone());
        self.selected_is_custom = result.is_custom;
        self.finish_disk_action(format!(
            "Copied {source_id} to custom map {} ({}).",
            result.id(),
            result.display_name.as_deref().unwrap_or("Untitled")
        ));
        Ok(result)
    }

    pub fn suggested_copy_name(map: &EndlessMapInfo) -> String {
        format!("{} - Copy", map.display_name.as_deref().unwrap_or(&map.id()))
    }

    /// Selects an endless map; as in C# the clone source and suggested copy name follow the selection.
    pub fn select_endless_map(&mut self, map: &EndlessMapInfo) {
        self.selected_map_id = Some(map.id());
        self.selected_map_path = Some(map.directory_path.clone());
        self.selected_is_custom = map.is_custom;
        self.clone_source_slot = map.slot;
        self.clone_new_name = Self::suggested_copy_name(map);
    }

    fn clear_selection(&mut self) {
        self.selected_map_id = None;
        self.selected_map_path = None;
        self.selected_is_custom = false;
    }

    /// Reports a committed disk operation; a catalog refresh failure (already in `status_message`)
    /// is appended and keeps the error flag so the user sees it.
    fn finish_disk_action(&mut self, success: String) {
        if self.status_is_error {
            self.status_message = format!("{success} Catalog refresh failed: {}", self.status_message);
        } else {
            self.status_message = success;
        }
    }

    /// Clones into the next free custom slot (C# `GetNextFreeSlot`), trimming and requiring a name.
    pub fn execute_clone(&mut self) -> Result<EndlessMapInfo, String> {
        let game_path = Path::new(self.game_path_input.trim());
        if !game_path.exists() {
            return Err("Game path does not exist.".into());
        }
        if let Some(e) = map_name_error_both(&self.clone_new_name) {
            return Err(e);
        }
        let name = self.clone_new_name.trim().to_string();

        let slot = EndlessMapCatalog::new().get_next_free_slot(game_path)?;
        self.clone_target_slot = slot;
        let result = EndlessMapCloner::new().clone_map(game_path, self.clone_source_slot, slot, &name)?;

        // The clone is already committed on disk (transactional, rolled back on failure); it does not
        // leave unsaved in-memory map content, so it must not mark the session dirty or the safe-save
        // transaction would reject saving the open map.
        self.status_is_error = false;
        self.refresh_catalog();
        self.selected_map_id = Some(result.id());
        self.selected_map_path = Some(result.directory_path.clone());
        self.selected_is_custom = result.is_custom;
        self.finish_disk_action(format!(
            "Cloned map to {} successfully! ({})",
            result.id(),
            result.display_name.as_deref().unwrap_or("Untitled")
        ));
        Ok(result)
    }

    pub fn execute_delete(&mut self, slot: i32) -> Result<(), String> {
        let game_path = Path::new(self.game_path_input.trim());
        if !game_path.exists() {
            return Err("Game path does not exist.".into());
        }

        let deleter = EndlessMapDeleter::new();
        deleter.delete(game_path, slot)?;

        // Deletion is committed on disk; nothing unsaved remains, so the session stays clean.
        self.delete_confirm_slot = None;
        if self.selected_map_id.as_deref() == Some(&format!("ENDL_{:03}", slot)) {
            self.clear_selection();
        }
        self.status_is_error = false;
        self.refresh_catalog();
        self.finish_disk_action(format!("Successfully deleted custom map ENDL_{:03}.", slot));
        Ok(())
    }

    /// Builds a new blank endless map using BlankMapBuilder and BlankMapSpec.
    pub fn build_blank_map(&mut self) -> Result<BlankMapBuildResult, String> {
        let intent = self.blank_map_build_intent();
        if !intent.game_path.is_dir() {
            return Err(format!(
                "Game root path is invalid or does not exist: {}",
                intent.game_path.display()
            ));
        }
        let result = BlankMapBuilder::build_with_spec(&intent.game_path, &intent.spec)?;
        result.verify_disk_artifacts(&intent.game_path)?;
        self.refresh_catalog();
        self.blank_last_result = Some(result.clone());
        self.status_message = format!(
            "Blank map ENDL_{:03} created and verified successfully!",
            intent.spec.target_slot
        );
        self.status_is_error = false;
        Ok(result)
    }

    pub fn run_design_validator(
        &mut self,
        objects: &[Spawn],
        events: &[Event],
        heights: Option<&[u8]>,
        height_size: usize,
        collision: Option<&[u8]>,
        collision_size: usize,
        water_level: f32,
    ) {
        let req = MapDesignValidationRequest {
            objects: objects.to_vec(),
            events: events.to_vec(),
            collision: collision.map(|c| c.to_vec()),
            collision_size,
            heights: heights.map(|h| h.to_vec()),
            height_size,
            height_step: 4.0,
            water_level,
            map_dimension: 16384.0,
        };

        self.design_issues = MapDesignValidator::validate(&req);
        self.status_message = format!(
            "Design Validator completed: {} issues found.",
            self.design_issues.len()
        );
        self.status_is_error = false;
    }

    pub fn run_diagnostics(&mut self, cx: &MapsContext<'_>) {
        let Some(scenario) = cx.scenario else {
            self.status_message = "No scenario loaded for diagnostics.".to_string();
            self.status_is_error = true;
            return;
        };
        // Spawns without a troop count are treated as buildings; no template data is available here,
        // so templates are assumed complete rather than raising false alarms.
        let objects = scenario
            .spawns
            .iter()
            .map(|s| MapCheckObject::new(s.clone(), s.count == 0, true))
            .collect();
        let collision_size = cx.height_size.saturating_sub(1);
        let mut snapshot = MapCheckSnapshot::new(
            objects,
            scenario.events.clone(),
            cx.aliases.to_vec(),
            collision_size,
            cx.collision.map(<[u8]>::to_vec),
        );
        snapshot.height_size = cx.height_size;
        snapshot.heights = cx.heights.map(<[u8]>::to_vec);
        snapshot.height_step = cx.height_step;
        snapshot.water_level = cx.water_level;
        self.diagnostic_issues = MapDiagnostics::check(&snapshot);
        self.status_message = format!(
            "Diagnostics completed: {} issues found.",
            self.diagnostic_issues.len()
        );
        self.status_is_error = false;
    }

    pub fn run_preflight(
        &mut self,
        scenario: &ScenarioDocument,
        aliases: &[String],
    ) {
        match ScenarioSavePreflight::validate(scenario, aliases) {
            Ok(()) => {
                self.preflight_result = Some(Ok("Preflight verification passed! Safe to export.".to_string()));
                self.status_message = "Preflight verification passed.".to_string();
                self.status_is_error = false;
            }
            Err(e) => {
                self.preflight_result = Some(Err(e.clone()));
                self.status_message = format!("Preflight blocked: {e}");
                self.status_is_error = true;
            }
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, cx: &MapsContext<'_>, mut on_load_requested: Option<&mut dyn FnMut(PathBuf)>) {
        if !self.open {
            return;
        }

        let mut is_open = self.open;
        egui::Window::new(tl(self.english, "地圖目錄與診斷工作室", "Maps & Diagnostics Studio"))
            .open(&mut is_open)
            .min_size(Vec2::new(720.0, 520.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.current_tab, MapsTab::Catalog, tl(self.english, "地圖目錄", "Map Catalog"));
                    ui.selectable_value(&mut self.current_tab, MapsTab::ClonerDeleter, tl(self.english, "複製 / 刪除", "Clone / Delete"));
                    ui.selectable_value(&mut self.current_tab, MapsTab::NewBlankMap, tl(self.english, "空白場景（實驗）", "Empty Scene (Exp.)"));
                    ui.selectable_value(&mut self.current_tab, MapsTab::DesignValidator, tl(self.english, "設計驗證器", "Design Validator"));
                    ui.selectable_value(&mut self.current_tab, MapsTab::Diagnostics, tl(self.english, "地圖診斷", "Diagnostics"));
                    ui.selectable_value(&mut self.current_tab, MapsTab::Preflight, tl(self.english, "存檔預檢", "Save Preflight"));
                });
                ui.separator();

                // Status banner
                if !self.status_message.is_empty() {
                    let color = if self.status_is_error {
                        Color32::from_rgb(250, 100, 100)
                    } else {
                        Color32::from_rgb(120, 220, 120)
                    };
                    ui.colored_label(color, self.localized_status_message());
                    ui.separator();
                }

                match self.current_tab {
                    MapsTab::Catalog => self.render_catalog_tab(ui, &mut on_load_requested),
                    MapsTab::ClonerDeleter => self.render_cloner_tab(ui),
                    MapsTab::NewBlankMap => self.render_blank_map_tab(ui),
                    MapsTab::DesignValidator => self.render_validator_tab(ui, cx),
                    MapsTab::Diagnostics => self.render_diagnostics_tab(ui, cx),
                    MapsTab::Preflight => self.render_preflight_tab(ui, cx),
                }
            });

        if !is_open {
            self.request_close();
        }

        // Pending discard confirmation modal
        if let Some(pending) = &self.pending_action {
            let mut close_modal = false;
            let mut confirm = false;
            let action_copy = pending.clone();

            egui::Window::new(tl(self.english, "確認操作", "Confirm Action"))
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    match action_copy {
                        MapsPendingAction::DeleteCustom(slot) => {
                            ui.label(format!(
                                "{} ENDL_{:03}?",
                                tl(self.english, "確定要永久刪除自訂地圖", "Are you sure you want to permanently delete custom map"),
                                slot
                            ));
                        }
                        MapsPendingAction::DiscardAndClose => {
                            ui.label(tl(
                                self.english,
                                "您有未提交的地圖變更。確定捨棄並關閉嗎？",
                                "You have uncommitted map changes. Discard and close?",
                            ));
                        }
                    }
                    ui.horizontal(|ui| {
                        if ui.button(tl(self.english, "確認", "Confirm")).clicked() {
                            confirm = true;
                        }
                        if ui.button(tl(self.english, "取消", "Cancel")).clicked() {
                            close_modal = true;
                        }
                    });
                });

            if confirm {
                match action_copy {
                    MapsPendingAction::DeleteCustom(slot) => {
                        if let Err(e) = self.execute_delete(slot) {
                            self.status_message = format!("Delete failed: {e}");
                            self.status_is_error = true;
                        }
                    }
                    MapsPendingAction::DiscardAndClose => {
                        self.clear_dirty();
                        self.open = false;
                    }
                }
                self.pending_action = None;
            } else if close_modal {
                self.pending_action = None;
            }
        }
    }

    fn render_catalog_tab(&mut self, ui: &mut egui::Ui, on_load_requested: &mut Option<&mut dyn FnMut(PathBuf)>) {
        ui.horizontal(|ui| {
            ui.label(tl(self.english, "遊戲主目錄路徑：", "Game Root Path:"));
            ui.text_edit_singleline(&mut self.game_path_input);
            if ui.button(tl(self.english, "瀏覽...", "Browse...")).clicked() {
                self.browse_game_folder();
            }
            if ui.button(tl(self.english, "重新整理目錄", "Refresh Catalog")).clicked() {
                self.refresh_catalog();
            }
        });

        ui.separator();

        egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
            ui.heading(tl(self.english, "無盡模式地圖", "Endless Maps"));
            if self.endless_maps.is_empty() {
                ui.label(tl(self.english, "找不到無盡模式地圖或目錄尚未掃描。", "No endless maps found or catalog not yet scanned."));
            } else {
                let maps = self.endless_maps.clone();
                for map in &maps {
                    let is_selected = self.selected_map_id.as_deref() == Some(&map.id());
                    let label = format!(
                        "[{}] {} ({}) - {}",
                        if map.is_custom { tl(self.english, "自訂", "Custom") } else { tl(self.english, "原版", "Retail") },
                        map.id(),
                        map.display_name.as_deref().unwrap_or_else(|| tl(self.english, "未命名", "Unnamed")),
                        map.directory_path.display()
                    );
                    if ui.selectable_label(is_selected, label).clicked() {
                        self.select_endless_map(map);
                    }
                }
            }

            ui.separator();
            ui.heading(tl(self.english, "一般戰役／任務地圖", "General Game Maps"));
            if self.general_maps.is_empty() {
                ui.label(tl(self.english, "未編錄一般地圖。", "No general maps catalogued."));
            } else {
                for map in &self.general_maps {
                    let is_selected = self.selected_map_id.as_deref() == Some(&map.id);
                    let label = format!(
                        "[{}] {} - {}",
                        if map.is_custom { tl(self.english, "自訂", "Custom") } else { tl(self.english, "原版", "Retail") },
                        map.id,
                        map.directory_path.display()
                    );
                    if ui.selectable_label(is_selected, label).clicked() {
                        self.selected_map_id = Some(map.id.clone());
                        self.selected_map_path = Some(map.directory_path.clone());
                        self.selected_is_custom = map.is_custom;
                        self.clone_new_name =
                            Self::suggested_copy_name_for(map.display_name.as_deref(), &map.id);
                    }
                }
            }
        });

        ui.separator();
        let mut copy_requested = false;
        ui.horizontal(|ui| {
            if let Some(path) = &self.selected_map_path {
                ui.label(format!("{}: {}", tl(self.english, "已選取", "Selected"), path.display()));
                if ui.button(tl(self.english, "載入地圖至編輯器", "Load Map into Editor")).clicked() {
                    if let Some(cb) = on_load_requested.as_deref_mut() {
                        cb(path.clone());
                    }
                }
            } else {
                ui.label(tl(self.english, "請從目錄中選取地圖以檢查或載入。", "Select a map from the catalog to inspect or load."));
            }
        });
        if let Some(id) = self.selected_map_id.clone() {
            if !self.selected_is_custom {
                ui.horizontal(|ui| {
                    ui.label(tl(self.english, "原版地圖唯讀；請複製為自製地圖後編輯：", "Retail maps are read-only. Copy to edit:"));
                    ui.text_edit_singleline(&mut self.clone_new_name);
                    let allowed = Self::can_clone_to_custom(&id, false);
                    if ui
                        .add_enabled(allowed, egui::Button::new(tl(self.english, "複製為自製地圖", "Copy to Custom Map")))
                        .on_disabled_hover_text(tl(
                            self.english,
                            "只有無盡模式（ENDL）地圖能複製。",
                            "Only Endless Mode (ENDL) maps can be copied.",
                        ))
                        .clicked()
                    {
                        copy_requested = true;
                    }
                });
            }
        }
        if copy_requested {
            match self.execute_copy_selected() {
                Ok(created) => {
                    // C# `CloneAndOpen`: the new custom map opens straight away.
                    if let Some(cb) = on_load_requested.as_deref_mut() {
                        cb(created.directory_path.clone());
                    }
                }
                Err(e) => {
                    self.status_message = format!("Copy failed: {e}");
                    self.status_is_error = true;
                }
            }
        }
    }

    fn render_cloner_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading(tl(self.english, "地圖複製器與安全刪除", "Map Cloner & Safe Deleter"));
        ui.label(tl(
            self.english,
            "將任何原版或自訂地圖複製到獨立的自訂槽位 (5..999)。",
            "Duplicate any retail or custom map into an isolated custom slot (5..999).",
        ));

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(tl(self.english, "來源槽位 (ENDL_###)：", "Source Slot (ENDL_###):"));
            ui.add(egui::DragValue::new(&mut self.clone_source_slot).clamp_range(0..=999));
        });

        ui.horizontal(|ui| {
            ui.label(tl(
                self.english,
                "目標槽位：下一個可用的自訂槽位（如同 C# 編輯器自動指定）",
                "Target Slot: next free custom slot (assigned automatically, like the C# editor)",
            ));
        });

        ui.horizontal(|ui| {
            ui.label(tl(self.english, "新地圖名稱：", "New Map Name:"));
            ui.text_edit_singleline(&mut self.clone_new_name);
        });
        if let Some(e) = map_name_error(self.english, &self.clone_new_name) {
            ui.colored_label(Color32::LIGHT_RED, e);
        }

        if ui.button(tl(self.english, "複製地圖", "Clone Map")).clicked() {
            match self.execute_clone() {
                Ok(_) => {}
                Err(e) => {
                    self.status_message = format!("Clone failed: {e}");
                    self.status_is_error = true;
                }
            }
        }

        ui.separator();
        ui.heading(tl(self.english, "自訂地圖刪除器", "Custom Map Deleter"));
        ui.label(tl(
            self.english,
            "僅自訂地圖（槽位 5..999）可刪除。原版地圖受嚴格保護。",
            "Only custom maps (slots 5..999) can be deleted. Retail maps are strictly guarded.",
        ));

        if let Some(slot) = self.selected_map_id.as_deref().and_then(|id| {
            if id.starts_with("ENDL_") {
                id[5..].parse::<i32>().ok()
            } else {
                None
            }
        }) {
            if slot >= 5 && self.selected_is_custom {
                if ui.button(format!("{}: ENDL_{:03}", tl(self.english, "刪除自訂地圖", "Delete Custom Map"), slot)).clicked() {
                    self.pending_action = Some(MapsPendingAction::DeleteCustom(slot));
                }
            } else {
                ui.label(tl(self.english, "目前選取的為原版或非自訂地圖：已停用刪除。", "Current selected map is retail or non-custom: Deletion disabled."));
            }
        } else {
            ui.label(tl(self.english, "請在目錄中選取自訂地圖以進行刪除。", "Select a custom map in the catalog to delete."));
        }
    }

    /// C# `MapSelectionForm` hint distinguishing the two new-map flavours. This tab is the
    /// "Empty Scene" path (`BlankMapBuilder.Create`); the Cloner tab copies a template unchanged.
    pub fn empty_scene_help() -> &'static str {
        "平坦範本保留聚落與腳本；空白場景清除來源物件與生成腳本，遊戲開局待驗證。原版地圖維持唯讀。\n\
         Flat Template keeps settlements and scripts. Empty Scene clears source objects and generation scripts; game startup awaits validation. Original maps are read-only."
    }

    fn render_blank_map_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading(tl(self.english, "空白場景（實驗）", "Empty Scene (Exp.)"));
        ui.label(tl(
            self.english,
            "根據規格建立全新平坦自訂地圖 (ENDL_005..ENDL_999)。自動繼承資產、重設池、清理快取並註冊清單。",
            "Builds a clean flat custom map (ENDL_005..ENDL_999) from spec. Inherits assets, resets pools, clears cache and registers catalog.",
        ));
        ui.label(Self::empty_scene_help());
        ui.collapsing(tl(self.english, "清除／保留內容說明", "What is cleared / kept"), |ui| {
            let source_id = format!("ENDL_{:03}", self.blank_source_slot);
            ui.label(BlankMapBuilder::describe(&source_id, false));
            ui.label(BlankMapBuilder::describe(&source_id, true));
        });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(tl(self.english, "遊戲主目錄路徑：", "Game Root Path:"));
            ui.text_edit_singleline(&mut self.game_path_input);
            if ui.button(tl(self.english, "瀏覽遊戲資料夾...", "Browse Game Folder...")).clicked() {
                self.browse_game_folder();
            }
        });

        ui.horizontal(|ui| {
            ui.label(tl(self.english, "來源模板槽位：", "Source Slot:"));
            ui.add(egui::DragValue::new(&mut self.blank_source_slot).clamp_range(0..=999).prefix("ENDL_"));
            ui.label(tl(self.english, "(預設 ENDL_000 零售模板)", "(default ENDL_000 retail template)"));
        });

        ui.horizontal(|ui| {
            ui.label(tl(self.english, "目標地圖槽位：", "Target Slot:"));
            ui.add(egui::DragValue::new(&mut self.blank_target_slot).clamp_range(5..=999).prefix("ENDL_"));
            ui.label(tl(self.english, "(自訂地圖範圍 5..999)", "(custom map range 5..999)"));
            if ui.button(tl(self.english, "尋找可用槽位", "Find Free Slot")).clicked() {
                match self.select_next_blank_slot() {
                    Ok(slot) => {
                        self.status_message = format!("{} ENDL_{slot:03}", tl(self.english, "可用槽位：", "Available slot:"));
                        self.status_is_error = false;
                    }
                    Err(error) => {
                        self.status_message = format!("{} {error}", tl(self.english, "尋找可用槽位失敗：", "Could not find a free slot:"));
                        self.status_is_error = true;
                    }
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label(tl(self.english, "新地圖名稱：", "New Map Name:"));
            ui.text_edit_singleline(&mut self.blank_target_name);
        });
        if let Some(e) = map_name_error(self.english, &self.blank_target_name) {
            ui.colored_label(Color32::LIGHT_RED, e);
        }

        ui.horizontal(|ui| {
            ui.label(tl(self.english, "基礎地形高度：", "Base Height:"));
            ui.add(egui::Slider::new(&mut self.blank_height_override, 0..=255));
        });

        ui.separator();
        ui.checkbox(&mut self.blank_include_starter_kit, tl(self.english, "配置起始聚落 (主城與初始部隊)", "Starter Kit: Town Center & Initial Units"));
        if self.blank_include_starter_kit {
            ui.horizontal(|ui| {
                ui.label(tl(self.english, "起始陣營 (Team 0..7)：", "Starter Team (0..7):"));
                ui.add(egui::DragValue::new(&mut self.blank_starter_team).clamp_range(0..=7));
            });
            ui.horizontal(|ui| {
                ui.label(tl(self.english, "聚落中心位置 X, Z (0..16383)：", "Settlement Center X, Z (0..16383):"));
                ui.add(egui::DragValue::new(&mut self.blank_starter_x).prefix("X: "));
                ui.add(egui::DragValue::new(&mut self.blank_starter_z).prefix("Z: "));
            });
        }

        ui.horizontal(|ui| {
            ui.label(tl(self.english, "任務簡報說明（選填）：", "Briefing (Optional):"));
            ui.text_edit_singleline(&mut self.blank_briefing);
        });

        ui.separator();
        ui.horizontal(|ui| {
            if ui.button(tl(self.english, "驗證規格", "Validate Spec")).clicked() {
                let game_path = Path::new(self.game_path_input.trim());
                let source_id = format!("ENDL_{:03}", self.blank_source_slot);
                let mut spec = BlankMapSpec::new(source_id, self.blank_target_slot, &self.blank_target_name)
                    .with_height_override(self.blank_height_override);
                if self.blank_include_starter_kit {
                    spec = spec.with_starter_kit(self.blank_starter_team, self.blank_starter_x, self.blank_starter_z);
                }
                if !self.blank_briefing.trim().is_empty() {
                    spec = spec.with_briefing(self.blank_briefing.trim());
                }
                match spec.validate(game_path) {
                    Ok(()) => {
                        self.status_message = tl(self.english, "規格驗證通過，隨時可建置地圖。", "Spec validation passed, ready to build map.").to_string();
                        self.status_is_error = false;
                    }
                    Err(e) => {
                        self.status_message = format!("規格驗證失敗: {e}");
                        self.status_is_error = true;
                    }
                }
            }

            if ui
                .button(tl(self.english, "建立平坦範本地圖", "Build Flat Template & Open"))
                .on_hover_text(tl(
                    self.english,
                    "複製範本（保留聚落、腳本與 DATA），開啟後立即整平地形並重設地表材質、阻擋、光照；儲存前仍可還原。使用下一個可用的自製地圖槽位。",
                    "Copies template (keeps settlements, scripts, DATA), levels terrain and resets ground materials, collision, lighting on open; undoable before saving. Uses next available custom slot.",
                ))
                .clicked()
            {
                match self.request_build_flat_template() {
                    Ok(intent) => {
                        self.status_message = format!(
                            "{}: ENDL_{:03}...",
                            tl(self.english, "已提出平坦範本建置請求", "Flat template build intent requested"),
                            intent.spec.target_slot
                        );
                        self.status_is_error = false;
                    }
                    Err(e) => {
                        self.status_message = format!("平坦範本請求失敗: {e}");
                        self.status_is_error = true;
                    }
                }
            }

            if ui.button(tl(self.english, "建立並載入空白場景", "Build Empty Scene & Open")).clicked() {
                if let Some(e) = map_name_error(self.english, &self.blank_target_name) {
                    self.status_message = e;
                    self.status_is_error = true;
                    return;
                }
                let intent = self.request_build_blank_map();
                self.status_message = format!(
                    "{}: ENDL_{:03}...",
                    tl(self.english, "已提出空白地圖建置請求", "Blank map build intent requested"),
                    intent.spec.target_slot
                );
                self.status_is_error = false;
            }

            ui.separator();
            if ui.button(tl(self.english, "新增對戰地圖…", "New Skirmish Map Wizard…")).clicked() {
                self.request_skirmish_wizard = true;
            }
        });

        if let Some(res) = &self.blank_last_result {
            ui.separator();
            ui.colored_label(Color32::LIGHT_GREEN, format!("{}: {} ({})", tl(self.english, "最近建置地圖", "Recent Map"), res.info.id(), res.info.display_name.as_deref().unwrap_or("")));
            ui.label(format!("{}: {}", tl(self.english, "路徑", "Path"), res.map_path.display()));
            ui.label(format!("{} {} | {} {}", tl(self.english, "套用地形高度:", "Applied height:"), res.applied_height, tl(self.english, "代表性材質:", "Representative texture:"), res.representative_texture));
            ui.label(format!("{} {} ({} {})", tl(self.english, "起始聚落配置:", "Starter kit:"), res.starter_kit_applied, tl(self.english, "生成物件數:", "Spawns:"), res.starter_spawns_count));
            ui.collapsing(tl(self.english, "檢視空白初始化檔案報告", "Inspect blank initialization file report"), |ui| {
                let report = &res.reset_report;
                ui.label(format!("{} ({})", tl(self.english, "已重設的 DATA 檔", "Reset DATA files"), report.reset_data_files.len()));
                for path in &report.reset_data_files {
                    ui.small(path);
                }
                ui.add_space(4.0);
                ui.label(format!("{} ({})", tl(self.english, "沿用範本且未由空白初始化重設的 DATA 檔案", "Template DATA files not reset by blank initialization"), report.preserved_data_files.len()));
                if report.preserved_data_files.is_empty() {
                    ui.small(tl(self.english, "無", "None"));
                }
                for path in &report.preserved_data_files {
                    ui.small(path);
                }
                ui.add_space(4.0);
                ui.label(format!("{} ({})", tl(self.english, "保留的 SDL 範本", "Preserved SDL templates"), report.preserved_sdl_templates.len()));
                for path in &report.preserved_sdl_templates {
                    ui.small(path);
                }
                ui.label(format!("{}: {}", tl(self.english, "已初始化場景文件", "Initialized scenario file"), report.initialized_scenario_file));
                ui.label(tl(self.english, "重新建立的關卡腳本檔", "Recreated level script files"));
                for path in &report.generated_script_files {
                    ui.small(path);
                }
                ui.colored_label(
                    Color32::YELLOW,
                    tl(
                        self.english,
                        "保留的 DATA 檔案與 SDL 範本尚未證明可在遊戲內相容；空白場景開局仍未驗證。",
                        "Compatibility of preserved DATA files and SDL templates has not been proven in game; startup on an empty scene remains unverified.",
                    ),
                );
            });
        }
    }

    fn render_validator_tab(&mut self, ui: &mut egui::Ui, cx: &MapsContext<'_>) {
        ui.heading(tl(self.english, "設計驗證器", "Design Validator"));
        ui.label(tl(
            self.english,
            "靜態遊戲玩法完整性規則：邊界、水中／阻擋生成、主城與單位存在性。",
            "Static gameplay sanity rules: bounds, water/blocked spawn, main house & unit presence.",
        ));
        if ui.button(tl(self.english, "在目前地圖上執行設計驗證", "Run Design Validator on current map")).clicked() {
            match cx.scenario {
                Some(sc) => self.run_design_validator(
                    &sc.spawns,
                    &sc.events,
                    cx.heights,
                    cx.height_size,
                    cx.collision,
                    cx.height_size.saturating_sub(1),
                    cx.water_level,
                ),
                None => {
                    self.status_message = tl(self.english, "未載入劇本。", "No scenario loaded.").to_string();
                    self.status_is_error = true;
                }
            }
        }

        egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
            if self.design_issues.is_empty() {
                ui.colored_label(Color32::LIGHT_GREEN, tl(self.english, "未發現設計問題或驗證器尚未執行。", "No design issues found or validator has not been run yet."));
            } else {
                for (idx, issue) in self.design_issues.iter().enumerate() {
                    let color = match issue.severity {
                        DesignIssueSeverity::Error => Color32::from_rgb(255, 90, 90),
                        DesignIssueSeverity::Warning => Color32::from_rgb(255, 200, 70),
                    };
                    ui.horizontal(|ui| {
                        ui.colored_label(color, format!("[{:?}]", issue.severity));
                        ui.label(format!("{}. [{}] {}", idx + 1, issue.code, issue.chinese));
                        if let (Some(x), Some(z)) = (issue.world_x, issue.world_z) {
                            ui.label(format!("({} {:.1}, {:.1})", tl(self.english, "位置：", "Pos:"), x, z));
                        }
                    });
                }
            }
        });
    }

    fn render_diagnostics_tab(&mut self, ui: &mut egui::Ui, cx: &MapsContext<'_>) {
        ui.heading(tl(self.english, "地圖連通與通行診斷", "Map Diagnostics"));
        ui.label(tl(
            self.english,
            "檢查孤立通行區域、連通瓶頸、道路中斷與事件問題。",
            "Checks isolated passability regions, connectivity chokepoints, road gaps, and event issues.",
        ));
        if ui.button(tl(self.english, "在目前地圖上執行診斷", "Run Diagnostics on current map")).clicked() {
            self.run_diagnostics(cx);
        }

        egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
            if self.diagnostic_issues.is_empty() {
                ui.colored_label(Color32::LIGHT_GREEN, tl(self.english, "未發現診斷問題。", "No diagnostic issues found."));
            } else {
                for (idx, issue) in self.diagnostic_issues.iter().enumerate() {
                    let color = match issue.severity {
                        MapIssueSeverity::Error => Color32::from_rgb(255, 90, 90),
                        MapIssueSeverity::Warning => Color32::from_rgb(255, 200, 70),
                    };
                    ui.horizontal(|ui| {
                        ui.colored_label(color, format!("[{:?}]", issue.severity));
                        ui.label(format!("{}. [{}] {}", idx + 1, issue.code, issue.chinese));
                    });
                }
            }
        });
    }

    fn render_preflight_tab(&mut self, ui: &mut egui::Ui, cx: &MapsContext<'_>) {
        ui.heading(tl(self.english, "劇本存檔合約預檢", "Scenario Save Preflight"));
        ui.label(tl(
            self.english,
            "存檔前合約檢查：BCI 位元組碼邊界、劇本 UUID 一致性與動作定義。",
            "Pre-save contract checks for BCI bytecode bounds, scenario UUID consistency, and actions.",
        ));
        if ui.button(tl(self.english, "在目前劇本上執行存檔預檢", "Run Save Preflight on current scenario")).clicked() {
            match cx.scenario {
                Some(sc) => self.run_preflight(sc, cx.aliases),
                None => {
                    self.status_message = tl(self.english, "未載入劇本。", "No scenario loaded.").to_string();
                    self.status_is_error = true;
                }
            }
        }

        if let Some(res) = &self.preflight_result {
            match res {
                Ok(msg) => {
                    ui.colored_label(Color32::LIGHT_GREEN, format!("{}: {msg}", tl(self.english, "預檢成功", "Success")));
                }
                Err(err) => {
                    ui.colored_label(Color32::from_rgb(255, 90, 90), format!("{}: {err}", tl(self.english, "驗證錯誤", "Validation Error")));
                }
            }
        } else {
            ui.label(tl(self.english, "目前劇本文件尚未執行存檔預檢。", "Preflight check has not been run for the current scenario document."));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

    #[test]
    fn test_maps_ui_initial_state_and_dirty_lifecycle() {
        let mut ui = MapsUi::default();
        assert!(!ui.open);
        assert!(!ui.is_dirty());

        ui.mark_dirty();
        assert!(ui.is_dirty());

        ui.request_close();
        assert!(ui.pending_action.is_some());

        ui.clear_dirty();
        assert!(!ui.is_dirty());
        assert!(ui.pending_action.is_none());
    }

    #[test]
    fn native_game_folder_picker_updates_path_and_refreshes_catalog() {
        let mut ui = MapsUi::default();
        let selected_path = std::env::temp_dir();
        let selected_by_picker = selected_path.clone();
        let was_called = Arc::new(AtomicBool::new(false));
        let called = was_called.clone();
        ui.game_folder_picker = Some(Box::new(move || {
            called.store(true, Ordering::SeqCst);
            Some(selected_by_picker.clone())
        }));

        ui.browse_game_folder();

        assert!(was_called.load(Ordering::SeqCst));
        assert_eq!(PathBuf::from(&ui.game_path_input), selected_path);
        assert_ne!(ui.status_message, "Ready.", "selecting a folder refreshes the map catalogs");
    }

    #[test]
    fn game_folder_picker_cancel_and_invalid_selection_preserve_current_path() {
        let mut ui = MapsUi::default();
        ui.game_path_input = "existing path".to_string();
        ui.game_folder_picker = Some(Box::new(|| None));
        ui.browse_game_folder();
        assert_eq!(ui.game_path_input, "existing path");
        assert_eq!(ui.status_message, "Ready.");

        let mut ui = MapsUi::default();
        ui.game_path_input = "existing path".to_string();
        ui.game_folder_picker = Some(Box::new(|| Some(PathBuf::from("missing-game-folder-12345"))));
        ui.browse_game_folder();
        assert_eq!(ui.game_path_input, "existing path");
        assert!(ui.status_is_error);
    }

    #[test]
    fn test_maps_ui_validator_detects_out_of_bounds() {
        let mut ui = MapsUi::default();
        let mut spawn = Spawn::default();
        spawn.x = 99999.0; // out of map bounds
        spawn.z = 100.0;

        ui.run_design_validator(&[spawn], &[], None, 0, None, 0, 0.0);
        assert!(!ui.design_issues.is_empty());
        assert!(ui.design_issues.iter().any(|i| i.code == "bounds"));
    }

    #[test]
    fn test_maps_ui_preflight_passes_valid_scenario() {
        let mut ui = MapsUi::default();
        let mut scenario = ScenarioDocument::default();
        let mut spawn = Spawn::default();
        spawn.id = uuid::Uuid::new_v4();
        spawn.alias = "GER_INF00".to_string();
        spawn.x = 1000.0;
        spawn.z = 1000.0;
        spawn.count = 5;
        spawn.team = 0;
        scenario.spawns.push(spawn);

        let aliases = vec!["GER_INF00".to_string()];
        ui.run_preflight(&scenario, &aliases);
        assert!(ui.preflight_result.as_ref().unwrap().is_ok());
    }

    fn info(slot: i32, name: Option<&str>) -> EndlessMapInfo {
        EndlessMapInfo {
            slot,
            directory_path: PathBuf::from(format!("MAPS/ENDL_{slot:03}")),
            is_custom: slot >= 5,
            display_name: name.map(str::to_string),
        }
    }

    #[test]
    fn test_maps_ui_selection_suggests_copy_name_and_source() {
        let mut ui = MapsUi::default();
        ui.select_endless_map(&info(7, Some("Nordic")));
        assert_eq!(ui.clone_source_slot, 7);
        assert_eq!(ui.clone_new_name, "Nordic - Copy");
        assert!(ui.selected_is_custom);
        ui.select_endless_map(&info(1, None));
        assert_eq!(ui.clone_new_name, "ENDL_001 - Copy");
    }

    #[test]
    fn map_name_error_rejects_bad_names_in_both_languages_before_any_disk_work() {
        assert!(map_name_error(true, "Nordic Plains").is_none());
        assert!(map_name_error(true, "  ").unwrap().contains("blank"));
        assert!(map_name_error(false, "").unwrap().contains("空白"));
        for bad in ["a/b", "a\\b", "a:b", "a*b", "a?b", "a\"b", "a<b", "a>b", "a|b", "tab\there", "nl\nx"] {
            assert!(map_name_error(true, bad).unwrap().contains("forbidden"), "{bad:?}");
            assert!(map_name_error(false, bad).unwrap().contains("不允許"), "{bad:?}");
        }
        assert!(map_name_error(true, "map \u{1F600}").unwrap().contains("cannot store"));

        let root = std::env::temp_dir().join(format!("arm_maps_ui_name_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut ui = MapsUi::default();
        ui.game_path_input = root.to_string_lossy().into_owned();
        ui.blank_target_name = "bad|name".into();
        assert!(ui.request_build_flat_template().unwrap_err().contains("forbidden"));
        assert!(ui.pending_build_intent.is_none());
        ui.clone_new_name = "bad\u{7}name".into();
        assert!(ui.execute_clone().unwrap_err().contains("forbidden"));
        assert!(ui.execute_copy_selected().is_err());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn test_maps_ui_clone_rejects_blank_name_and_delete_clears_selection() {
        let mut ui = MapsUi::default();
        ui.game_path_input = std::env::temp_dir().to_string_lossy().into_owned();
        ui.clone_new_name = "   ".to_string();
        assert!(ui.execute_clone().unwrap_err().contains("blank"));

        ui.select_endless_map(&info(9, Some("X")));
        ui.clear_selection();
        assert!(ui.selected_map_id.is_none() && ui.selected_map_path.is_none() && !ui.selected_is_custom);
    }

    #[test]
    fn test_maps_ui_blank_map_tab_and_spec_validation() {
        let mut ui = MapsUi::default();
        ui.current_tab = MapsTab::NewBlankMap;
        ui.blank_target_slot = 77;
        ui.blank_target_name = "Nordic Plains".to_string();
        ui.blank_height_override = 45;

        // Spec validation on non-existent path properly fails
        let non_existent = Path::new("non_existent_arm_game_root_12345");
        let spec = BlankMapSpec::new("ENDL_000", ui.blank_target_slot, &ui.blank_target_name)
            .with_height_override(ui.blank_height_override);
        assert!(spec.validate(non_existent).is_err());
    }

    #[test]
    fn test_maps_ui_request_build_blank_map_creates_intent_without_filesystem_mutation() {
        let temp_dir = std::env::temp_dir().join(format!("arm_maps_ui_intent_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let mut ui = MapsUi::default();
        ui.game_path_input = temp_dir.to_string_lossy().to_string();
        ui.blank_source_slot = 0;
        ui.blank_target_slot = 88;
        ui.blank_target_name = "Deferred Map".to_string();
        ui.blank_height_override = 50;
        ui.blank_include_starter_kit = true;
        ui.blank_starter_team = 2;
        ui.blank_starter_x = 5000.0;
        ui.blank_starter_z = 5000.0;
        ui.blank_briefing = "Deferred mission briefing".to_string();

        assert!(ui.pending_build_intent.is_none());
        assert!(!ui.is_dirty());

        let intent = ui.request_build_blank_map();
        assert_eq!(intent.game_path, temp_dir);
        assert_eq!(intent.spec.source_map_id, "ENDL_000");
        assert_eq!(intent.spec.target_slot, 88);
        assert_eq!(intent.spec.target_name, "Deferred Map");
        assert_eq!(intent.spec.height_override, Some(50));
        assert!(intent.spec.starter_kit.is_some());
        assert_eq!(intent.spec.briefing_text.as_deref(), Some("Deferred mission briefing"));

        // Intent is staged in UI without mutating disk
        assert_eq!(ui.pending_build_intent, Some(intent.clone()));
        assert!(!temp_dir.join("MAPS").exists(), "no MAPS directory created on request");
        assert!(!temp_dir.join("MAPS/ENDL_088").exists(), "no target directory created on request");
        assert_eq!(std::fs::read_dir(&temp_dir).unwrap().count(), 0, "directory remains empty");
        assert!(!ui.is_dirty(), "intent staging must not falsely dirty clean session");

        // take_pending_build_intent clears staging
        let taken = ui.take_pending_build_intent();
        assert_eq!(taken, Some(intent));
        assert!(ui.pending_build_intent.is_none());

        // reset_session clears pending intent
        ui.request_build_blank_map();
        assert!(ui.pending_build_intent.is_some());
        ui.reset_session(None);
        assert!(ui.pending_build_intent.is_none());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    fn disk_fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!("arm_maps_ui_disk_{}", uuid::Uuid::new_v4()));
        let src = root.join("MAPS").join("ENDL_001");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("level.put"), "var:map_name =\"Template\";
").unwrap();
        std::fs::write(src.join("briefing.put"), "var:briefing_titel_1 =\"Brief\";
").unwrap();
        std::fs::write(src.join("map.sdl"), "[settlement]
refpos=100.0,0.0,100.0
").unwrap();
        std::fs::write(src.join("boden.txt"), "[Dimension]
64

[Texturen]
").unwrap();
        std::fs::write(src.join("minimap.bmp"), "BM").unwrap();
        root
    }

    #[test]
    fn blank_map_finds_free_slot_without_creating_a_map() {
        let root = disk_fixture();
        std::fs::create_dir_all(root.join("MAPS/ENDL_005")).unwrap();
        let mut ui = MapsUi::default();
        ui.game_path_input = root.to_string_lossy().into_owned();
        assert_eq!(ui.select_next_blank_slot().unwrap(), 6);
        assert_eq!(ui.blank_map_build_intent().spec.target_slot, 6);
        assert!(!root.join("MAPS/ENDL_006").exists());
        ui.game_path_input = root.join("missing").to_string_lossy().into_owned();
        assert!(ui.select_next_blank_slot().is_err());
        assert_eq!(ui.blank_target_slot, 6);
        let _ = std::fs::remove_dir_all(root);
    }
    #[test]
    fn test_maps_ui_clone_and_delete_do_not_mark_session_dirty() {
        let root = disk_fixture();
        let mut ui = MapsUi::default();
        ui.game_path_input = root.to_string_lossy().into_owned();
        ui.clone_source_slot = 1;
        ui.clone_new_name = "Disk Copy".to_string();

        let cloned = ui.execute_clone().unwrap();
        assert!(!ui.is_dirty(), "committed clone must not block safe save");
        assert!(!ui.status_is_error);
        assert!(cloned.directory_path.exists());

        ui.execute_delete(cloned.slot).unwrap();
        assert!(!ui.is_dirty(), "committed delete must not block safe save");
        assert!(!cloned.directory_path.exists());
        assert!(ui.status_message.contains("deleted"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_maps_ui_failed_delete_stays_clean_and_error_is_reportable() {
        let root = disk_fixture();
        let mut ui = MapsUi::default();
        ui.game_path_input = root.to_string_lossy().into_owned();
        assert!(ui.execute_delete(1).is_err(), "retail slot must be refused");
        assert!(!ui.is_dirty());
        assert!(root.join("MAPS").join("ENDL_001").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_maps_ui_can_clone_to_custom_rules_and_suggested_name() {
        assert!(MapsUi::can_clone_to_custom("ENDL_001", false));
        assert!(MapsUi::can_clone_to_custom("endl_002", false));
        assert!(!MapsUi::can_clone_to_custom("KAMP_001", false));
        assert!(MapsUi::can_clone_to_custom("KAMP_001", true));
        assert_eq!(MapsUi::suggested_copy_name_for(Some("Forest"), "ENDL_001"), "Forest - Copy");
        assert_eq!(MapsUi::suggested_copy_name_for(None, "ENDL_001"), "ENDL_001 - Copy");
    }

    #[test]
    fn test_maps_ui_copy_selected_retail_map_registers_editable_custom_map() {
        let root = disk_fixture();
        let mut ui = MapsUi::default();
        ui.game_path_input = root.to_string_lossy().into_owned();
        // Nothing selected, then a non-ENDL source: both refused without touching disk.
        assert!(ui.execute_copy_selected().is_err());
        ui.selected_map_id = Some("KAMP_001".into());
        assert!(ui.execute_copy_selected().unwrap_err().contains("ENDL"));

        ui.selected_map_id = Some("ENDL_001".into());
        ui.selected_is_custom = false;
        ui.clone_new_name = "  Retail Copy ".into();
        let copy = ui.execute_copy_selected().unwrap();
        assert!(copy.is_custom, "copy must be a registered custom map");
        assert!(copy.slot >= 5);
        assert!(copy.directory_path.join(against_rome_maps::custom_map::CustomMapManifest::MARKER_FILE_NAME).exists());
        assert_eq!(ui.selected_map_id.as_deref(), Some(copy.id().as_str()));
        assert!(ui.selected_is_custom);
        assert!(!ui.is_dirty());
        assert!(root.join("MAPS").join("ENDL_001").exists(), "retail source untouched");
        let manifest = std::fs::read_to_string(root.join("MAPS").join("arm_custom_maps.json")).unwrap();
        assert!(manifest.contains(&format!("{}", copy.slot)));

        ui.clone_new_name = "   ".into();
        ui.selected_map_id = Some("ENDL_001".into());
        ui.selected_is_custom = false;
        assert!(ui.execute_copy_selected().unwrap_err().contains("blank"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_maps_ui_finish_disk_action_surfaces_refresh_failure() {
        let mut ui = MapsUi::default();
        ui.status_message = "Failed to list endless maps: boom".into();
        ui.status_is_error = true;
        ui.finish_disk_action("Cloned.".into());
        assert!(ui.status_is_error);
        assert!(ui.status_message.starts_with("Cloned.") && ui.status_message.contains("boom"));
    }

    #[test]
    fn empty_scene_tab_is_the_blank_builder_path_and_documents_the_flat_template_difference() {
        let help = MapsUi::empty_scene_help();
        assert!(help.contains("空白場景") && help.contains("平坦範本"));
        assert!(help.contains("Empty Scene clears source objects"));
        let zh = BlankMapBuilder::describe("ENDL_000", false);
        assert!(zh.contains("清除") && zh.contains("ENDL_000"));
        // The tab only queues an intent; it never touches the filesystem itself.
        let mut ui = MapsUi::default();
        ui.game_path_input = "does-not-exist".into();
        let intent = ui.request_build_blank_map();
        assert_eq!(intent.spec.target_slot, ui.blank_target_slot);
        assert!(ui.pending_build_intent.is_some());
    }

    #[test]
    fn test_flat_template_intent_preserves_briefing_text() {
        let root = disk_fixture();
        let mut ui = MapsUi::default();
        ui.game_path_input = root.to_string_lossy().to_string();
        ui.blank_source_slot = 1;
        ui.blank_target_name = "Flat Mission".to_string();
        ui.blank_briefing = "Custom flat mission briefing".to_string();

        let intent = ui.request_build_flat_template().expect("flat template intent");
        assert_eq!(
            intent.spec.briefing_text.as_deref(),
            Some("Custom flat mission briefing"),
            "request_build_flat_template must carry through the user's typed briefing text"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    fn collect_shape_texts(shape: &egui::epaint::Shape, out: &mut Vec<String>) {
        match shape {
            egui::epaint::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::epaint::Shape::Vec(shapes) => {
                for s in shapes {
                    collect_shape_texts(s, out);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn test_maps_ui_chinese_localization_renders_no_untranslated_literals() {
        let tabs = [
            MapsTab::Catalog,
            MapsTab::ClonerDeleter,
            MapsTab::NewBlankMap,
            MapsTab::DesignValidator,
            MapsTab::Diagnostics,
            MapsTab::Preflight,
        ];
        let english_literals_to_forbid = [
            "Maps & Diagnostics Studio",
            "Map Catalog",
            "Clone / Delete",
            "Save Preflight",
            "Game Root Path:",
            "Endless Maps",
            "No endless maps found or catalog not yet scanned.",
            "General Game Maps",
            "No general maps catalogued.",
            "Load Map into Editor",
            "Select a map from the catalog to inspect or load.",
            "Map Cloner & Safe Deleter",
            "Duplicate any retail or custom map into an isolated custom slot (5..999).",
            "Source Slot (ENDL_###):",
            "Target Slot: next free custom slot (assigned automatically, like the C# editor)",
            "New Map Name:",
            "Clone Map",
            "Custom Map Deleter",
            "Only custom maps (slots 5..999) can be deleted. Retail maps are strictly guarded.",
            "Select a custom map in the catalog to delete.",
            "Static gameplay sanity rules: bounds, water/blocked spawn, main house & unit presence.",
            "Run Design Validator on current map",
            "No design issues found or validator has not been run yet.",
            "Checks isolated passability regions, connectivity chokepoints, road gaps, and event issues.",
            "Run Diagnostics on current map",
            "No diagnostic issues found.",
            "Scenario Save Preflight",
            "Pre-save contract checks for BCI bytecode bounds, scenario UUID consistency, and actions.",
            "Run Save Preflight on current scenario",
            "Preflight check has not been run for the current scenario document.",
        ];

        let ctx = egui::Context::default();
        let heights = vec![20u8; 33 * 33];
        let cx = MapsContext {
            scenario: None,
            aliases: &[],
            heights: Some(&heights),
            height_size: 33,
            collision: None,
            height_step: 4.0,
            water_level: 0.0,
        };

        for tab in tabs {
            let mut ui = MapsUi::default();
            ui.open = true;
            ui.english = false;
            ui.current_tab = tab;

            // egui windows measure on first frame and paint on subsequent frames
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0))),
                    ..Default::default()
                },
                |ctx| {
                    ui.show(ctx, &cx, None);
                },
            );

            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0))),
                    ..Default::default()
                },
                |ctx| {
                    ui.show(ctx, &cx, None);
                },
            );

            let mut texts = Vec::new();
            for shape in &output.shapes {
                collect_shape_texts(&shape.shape, &mut texts);
            }
            let all_text = texts.join(" ");

            // Assert Chinese header is present
            assert!(all_text.contains("地圖目錄與診斷工作室"), "tab {:?} missing Chinese window title", tab);

            // Assert no untranslated literals remain in rendered UI
            for &forbidden in &english_literals_to_forbid {
                assert!(
                    !all_text.contains(forbidden),
                    "Tab {:?} rendered untranslated literal: '{}'",
                    tab,
                    forbidden
                );
            }
        }
    }

    #[test]
    fn maps_status_banner_localizes_messages_and_preserves_error_details() {
        let heights = vec![20u8; 33 * 33];
        let cx = MapsContext {
            scenario: None,
            aliases: &[],
            heights: Some(&heights),
            height_size: 33,
            collision: None,
            height_step: 4.0,
            water_level: 0.0,
        };
        let mut ui = MapsUi::default();
        ui.open = true;
        ui.current_tab = MapsTab::Catalog;
        let cases = [
            ("Ready.", "就緒。", "Ready."),
            ("Discovered 4 endless maps.", "已找到 4 個無盡地圖。", "Discovered 4 endless maps."),
            ("Design Validator completed: 3 issues found.", "設計驗證完成：3 項問題。", "Design Validator completed: 3 issues found."),
            ("Diagnostics completed: 2 issues found.", "地圖診斷完成：2 項問題。", "Diagnostics completed: 2 issues found."),
            ("Copied ENDL_001 to custom map ENDL_005 (New Map).", "已將 ENDL_001 複製至自製地圖 ENDL_005（New Map）。", "Copied ENDL_001 to custom map ENDL_005 (New Map)."),
            ("Cloned map to ENDL_005 successfully! (New Map)", "已成功複製地圖至 ENDL_005（New Map）。", "Cloned map to ENDL_005 successfully! (New Map)"),
            ("Successfully deleted custom map ENDL_005.", "已成功刪除自製地圖 ENDL_005。", "Successfully deleted custom map ENDL_005."),
            ("Failed to list endless maps: permission denied; Failed to list general maps: malformed catalog", "列出無盡地圖失敗：permission denied；列出一般地圖失敗：malformed catalog", "Failed to list endless maps: permission denied; Failed to list general maps: malformed catalog"),
            ("Preflight blocked: event 7 has invalid target", "存檔預檢未通過：event 7 has invalid target", "Preflight blocked: event 7 has invalid target"),
        ];
        for (status, chinese_expected, english_expected) in cases {
            ui.status_message = status.to_string();
            ui.english = false;
            let chinese = crate::batch20_localization_tests::render(|ctx| ui.show(ctx, &cx, None));
            assert!(chinese.contains(chinese_expected), "expected {chinese_expected:?} in {chinese}");
            ui.english = true;
            let english = crate::batch20_localization_tests::render(|ctx| ui.show(ctx, &cx, None));
            assert!(english.contains(english_expected), "expected {english_expected:?} in {english}");
        }
    }
}
