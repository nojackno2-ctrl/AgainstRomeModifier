//! Ambience and atmosphere studio UI: Weather presets, Boden.ini atmospheric lighting,
//! soundscape zones (SNDZ), and map briefing localization.
//! Directly integrates native `against-rome-ambience` and `against-rome-scene` crates with safe persistence.

use against_rome_ambience::{
    app_localization::Language as AppLanguage,
    atmosphere::{
        AtmosphereLightingBridge, AtmosphereProfile, AtmosphereShaderUniforms, BodenIniSerializer,
        PrecipitationType, WeatherPresets,
    },
    localization::{
        encode_strict, strict_encoding, BriefingSaveTransaction, BriefingTextTable,
        PutScriptSyntaxValidator, SupportedLanguage,
    },
    soundscape::{
        AttenuationCurveKind, AudioCategory, SoundscapeBinaryStorage, SoundscapeShapeType,
        SoundscapeSnapshot, SoundscapeZone, SoundscapeZoneFlags,
    },
};
use against_rome_scene::lighting::{MapLightingDayNight, SceneLightingContext};
use eframe::egui::{self, Color32};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

/// Native briefing location (C# `MapEditorForm.Persistence.cs`): `TEXT/US/briefing.put`.
const BRIEFING_NATIVE_RELATIVE: [&str; 3] = ["TEXT", "US", "briefing.put"];
/// Number of `briefing_text_teamname{n}` slots edited by the C# editor.
pub const BRIEFING_TEAM_COUNT: usize = 8;
/// The game reads `boden.ini` and `.put` files as CP1251 regardless of UI language.
const GAME_ENCODING_NAME: &str = "WINDOWS-1251";

/// Decodes a game text file: unwraps PFIL (LZSS) when present, then strict CP1251.
fn decode_game_text(bytes: &[u8]) -> Result<String, String> {
    let raw = if against_rome_data::lzss::has_pfil_magic(bytes) {
        against_rome_data::lzss::decompress_pfil(bytes, None).map_err(|e| e.to_string())?
    } else {
        bytes.to_vec()
    };
    strict_encoding(GAME_ENCODING_NAME)
        .decode_without_bom_handling_and_without_replacement(&raw)
        .map(|text| text.into_owned())
        .ok_or_else(|| "invalid CP1251 byte sequence".to_string())
}

/// Literal-only LZSS repack reusing the 64-byte PFIL header of `template`.
fn repack_pfil(raw: &[u8], template: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = template.get(..64).ok_or("truncated PFIL header")?.to_vec();
    out[16..20].copy_from_slice(&(raw.len() as u32).to_le_bytes());
    for chunk in raw.chunks(8) {
        out.push(((1u16 << chunk.len()) - 1) as u8);
        out.extend_from_slice(chunk);
    }
    Ok(out)
}

/// `[Section]` -> value pairs of a boden.ini text (comments after `;` dropped).
fn ini_sections(text: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        if let (true, Some(end)) = (line.starts_with('['), line.find(']')) {
            let value = lines
                .get(i + 1)
                .map(|l| l.split(';').next().unwrap_or("").trim())
                .unwrap_or("");
            out.push((line[1..end].trim().to_string(), value.to_string()));
            i += 1;
        }
        i += 1;
    }
    out
}

/// Replaces the value line under `[section]` (keeping indentation, trailing comment and newline
/// style), or appends the section when missing. Mirrors C# `BodenIniDocument.SetValue`.
fn patch_ini_value(text: &str, section: &str, value: &str) -> String {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = text.split_inclusive('\n').map(str::to_string).collect();
    for i in 0..lines.len().saturating_sub(1) {
        let is_header = lines[i]
            .trim()
            .strip_prefix('[')
            .and_then(|h| h.find(']').map(|e| h[..e].trim() == section))
            .unwrap_or(false);
        if !is_header {
            continue;
        }
        let old = &lines[i + 1];
        let body = old.trim_end_matches(['\r', '\n']);
        let eol = &old[body.len()..];
        let (code, comment) = match body.find(';') {
            Some(p) => (&body[..p], &body[p..]),
            None => (body, ""),
        };
        let indent = &code[..code.len() - code.trim_start().len()];
        let pad = if comment.is_empty() { "" } else { " " };
        lines[i + 1] = format!("{indent}{value}{pad}{comment}{eol}");
        return lines.concat();
    }
    let mut out = text.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push_str(newline);
    }
    out.push_str(&format!("[{section}]{newline}{value}{newline}"));
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AmbienceTab {
    #[default]
    Atmosphere,
    Soundscape,
    Localization,
    AppLanguage,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AmbienceSaveReport {
    pub map_path: PathBuf,
    pub boden_ini_saved: bool,
    pub boden_ini_backup: Option<PathBuf>,
    pub soundscape_saved: bool,
    pub briefing_saved: bool,
    pub briefing_file: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AmbienceLoadReport {
    pub map_path: PathBuf,
    pub boden_ini_loaded: bool,
    pub daynight_loaded: bool,
    pub soundscape_loaded: bool,
    pub soundscape_zones_count: usize,
    pub briefing_loaded: bool,
}

pub struct AmbienceUi {
    pub open: bool,
    pub current_tab: AmbienceTab,

    // --- Atmosphere / Weather ---
    pub profile: AtmosphereProfile,
    pub selected_preset: String,
    pub validation_errors: Vec<String>,

    // --- Native Scene / Lighting Bridge ---
    pub bridge: AtmosphereLightingBridge,
    pub lighting_context: SceneLightingContext,
    pub game_hour: f32,
    pub last_computed_uniforms: Option<AtmosphereShaderUniforms>,

    // --- Soundscape Zones ---
    pub zones: Vec<SoundscapeZone>,
    pub selected_zone_index: Option<usize>,
    pub new_zone_name: String,
    pub new_zone_sound_id: String,
    pub new_zone_category: AudioCategory,
    pub new_zone_shape: SoundscapeShapeType,
    pub global_sound_id: String,
    pub global_volume: f32,

    // --- Briefing Localization ---
    pub selected_language: SupportedLanguage,
    pub briefing_title: String,
    pub briefing_body: String,
    pub briefing_subtitle: String,
    pub briefing_team_names: [String; BRIEFING_TEAM_COUNT],

    // --- App Language ---
    pub app_language: AppLanguage,

    // Dirty / guards
    pub dirty: bool,
    pub pending_discard: bool,
    pub status_message: String,
    pub status_is_error: bool,
    /// Raised by the Save button; `EditorApp` consumes it and routes to the central Safe Save.
    save_requested: bool,
}

impl Default for AmbienceUi {
    fn default() -> Self {
        let mut ui = Self {
            open: false,
            current_tab: AmbienceTab::Atmosphere,
            profile: WeatherPresets::create_clear_sky(),
            selected_preset: WeatherPresets::CLEAR_SKY_ID.to_string(),
            validation_errors: Vec::new(),
            bridge: AtmosphereLightingBridge::new(),
            lighting_context: SceneLightingContext::new(None, None, None, HashMap::new()),
            game_hour: 12.0,
            last_computed_uniforms: None,
            zones: Vec::new(),
            selected_zone_index: None,
            new_zone_name: "Zone 1".to_string(),
            new_zone_sound_id: "SND_FOREST_BIRDS".to_string(),
            new_zone_category: AudioCategory::Vegetation,
            new_zone_shape: SoundscapeShapeType::Circle,
            global_sound_id: "Amb_Plains_Wind".to_string(),
            global_volume: 0.6,
            selected_language: SupportedLanguage::TraditionalChinese,
            briefing_title: String::new(),
            briefing_body: String::new(),
            briefing_subtitle: String::new(),
            briefing_team_names: Default::default(),
            app_language: AppLanguage::TraditionalChinese,
            dirty: false,
            pending_discard: false,
            status_message: "Ready.".to_string(),
            status_is_error: false,
            save_requested: false,
        };
        ui.evaluate_scene_lighting();
        ui.dirty = false;
        ui
    }
}

impl AmbienceUi {
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Ask the host to run the whole-map Safe Save; this panel never writes the map itself.
    pub fn request_save(&mut self) {
        self.save_requested = true;
    }

    /// Takes (and clears) a pending Safe Save request.
    pub fn take_save_request(&mut self) -> bool {
        std::mem::take(&mut self.save_requested)
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
        self.pending_discard = false;
    }

    /// Reset map-bound ambience, soundscape, lighting context, and briefing documents to clean defaults.
    /// Preserves user preferences such as app_language, selected_language, current_tab, open state,
    /// game_hour, and new-zone creation templates.
    pub fn reset_session(&mut self) {
        self.profile = WeatherPresets::create_clear_sky();
        self.selected_preset = WeatherPresets::CLEAR_SKY_ID.to_string();
        self.validation_errors.clear();
        self.bridge = AtmosphereLightingBridge::new();
        self.lighting_context.day_night = None;
        self.zones.clear();
        self.selected_zone_index = None;
        self.global_sound_id = "Amb_Plains_Wind".to_string();
        self.global_volume = 0.6;
        self.briefing_title.clear();
        self.briefing_body.clear();
        self.briefing_subtitle.clear();
        self.briefing_team_names = Default::default();
        self.dirty = false;
        self.pending_discard = false;
        self.status_message = "Ready.".to_string();
        self.status_is_error = false;
        self.evaluate_scene_lighting();
    }

    pub fn request_close(&mut self) {
        if self.is_dirty() {
            self.pending_discard = true;
        } else {
            self.open = false;
        }
    }

    pub fn load_preset(&mut self, preset_id: &str) {
        self.selected_preset = preset_id.to_string();
        if let Some(preset) = WeatherPresets::try_get_preset(preset_id) {
            let orig_waterlevel = self.profile.boden.waterlevel;
            let orig_heightstep = self.profile.boden.heightmapstep;
            self.profile = preset;
            self.profile.boden.waterlevel = orig_waterlevel;
            self.profile.boden.heightmapstep = orig_heightstep;
        } else {
            self.profile.preset_id = "custom".into();
            self.profile.name = "自訂環境 (Custom)".into();
        }
        self.dirty = true;
        self.validate_current();
        self.evaluate_scene_lighting();
    }

    pub fn validate_current(&mut self) -> bool {
        let (valid, errors) = BodenIniSerializer::validate(&self.profile.boden);
        self.validation_errors = errors;
        valid
    }

    pub fn sync_water_level(&mut self, water_level: f32) {
        self.profile.boden.waterlevel = water_level;
        self.dirty = true;
        self.validate_current();
        self.evaluate_scene_lighting();
        self.status_message = format!("Water level synchronized to {water_level:.1}.");
        self.status_is_error = false;
    }

    /// Evaluates scene lighting with `against_rome_scene::SceneLightingContext` and
    /// native `AtmosphereLightingBridge`, computing live `AtmosphereShaderUniforms`.
    pub fn evaluate_scene_lighting(&mut self) -> AtmosphereShaderUniforms {
        let ambient_vec = self.lighting_context.get_ambient_color(self.game_hour);
        let base_ambient = [ambient_vec.x, ambient_vec.y, ambient_vec.z];
        let uniforms = self.bridge.compute_uniforms(&self.profile, base_ambient);
        self.last_computed_uniforms = Some(uniforms.clone());
        uniforms
    }

    pub fn trigger_manual_flash(&mut self, intensity: f32) {
        self.bridge.trigger_manual_flash(intensity);
        self.evaluate_scene_lighting();
    }

    pub fn update_bridge(&mut self, delta_seconds: f32) {
        self.bridge.update(delta_seconds, &self.profile);
        self.evaluate_scene_lighting();
    }

    pub fn add_sound_zone(
        &mut self,
        name: &str,
        sound_id: &str,
        category: AudioCategory,
        shape: SoundscapeShapeType,
    ) -> Uuid {
        let id = Uuid::new_v4();
        let zone = SoundscapeZone {
            id,
            name: name.to_string(),
            sound_def_id: sound_id.to_string(),
            shape_type: shape,
            category,
            attenuation_curve: AttenuationCurveKind::SmoothStep,
            base_volume: 1.0,
            inner_radius: 150.0,
            outer_radius: 600.0,
            attributes: SoundscapeZoneFlags::IS_ACTIVE | SoundscapeZoneFlags::IS_LOOPING,
            ..Default::default()
        };
        self.zones.push(zone);
        self.selected_zone_index = Some(self.zones.len() - 1);
        self.dirty = true;
        self.status_message = format!("Added sound zone '{name}'.");
        self.status_is_error = false;
        id
    }

    pub fn remove_sound_zone(&mut self, index: usize) {
        if index < self.zones.len() {
            let removed = self.zones.remove(index);
            self.selected_zone_index = if self.zones.is_empty() {
                None
            } else {
                Some(index.min(self.zones.len() - 1))
            };
            self.dirty = true;
            self.status_message = format!("Removed sound zone '{}'.", removed.name);
            self.status_is_error = false;
        }
    }

    /// Bytes to write as `boden.ini` for `map_path`. Pure: touches no disk state.
    /// With an existing file only changed values are patched into the original text, which is
    /// re-encoded as CP1251 and re-wrapped as PFIL when the original was PFIL; an unchanged
    /// profile returns the original bytes verbatim. A missing file yields a fresh CP1251 ini.
    pub fn stage_boden_ini_bytes(&self, map_path: &Path) -> Result<Vec<u8>, String> {
        let encoding = strict_encoding(GAME_ENCODING_NAME);
        let ini_path = map_path.join("boden.ini");
        let current_text = BodenIniSerializer::generate_default_ini(&self.profile.boden);
        if !ini_path.is_file() {
            // The generator's CJK comments are not CP1251-encodable; emit values only.
            let plain: String = ini_sections(&current_text)
                .iter()
                .map(|(key, value)| format!("[{key}]\r\n{value}\r\n"))
                .collect();
            return encode_strict(&plain, encoding).map_err(|e| format!("boden.ini {e}"));
        }
        let original = fs::read(&ini_path)
            .map_err(|e| format!("Failed to read {}: {e}", ini_path.display()))?;
        let original_text = decode_game_text(&original)
            .map_err(|e| format!("Failed to decode {}: {e}", ini_path.display()))?;
        let baseline = BodenIniSerializer::generate_default_ini(&BodenIniSerializer::parse_text(
            &original_text,
        ));
        let before = ini_sections(&baseline);
        let mut patched = original_text.clone();
        for (section, value) in ini_sections(&current_text) {
            if before.iter().find(|(k, _)| *k == section).map(|(_, v)| v) != Some(&value) {
                patched = patch_ini_value(&patched, &section, &value);
            }
        }
        if patched == original_text {
            return Ok(original);
        }
        let raw = encode_strict(&patched, encoding).map_err(|e| format!("boden.ini {e}"))?;
        if against_rome_data::lzss::has_pfil_magic(&original) {
            repack_pfil(&raw, &original)
        } else {
            Ok(raw)
        }
    }

    /// Bytes for `TEXT/US/briefing.put`, or `None` when nothing changed. Edited values are merged
    /// into the existing native document, so extra variables (objectives, debriefing, video...)
    /// and unedited text are preserved; PFIL wrapping and CP1251 encoding are kept.
    pub fn stage_briefing_bytes(&self, map_path: &Path) -> Result<Option<Vec<u8>>, String> {
        let path = map_path.join(BRIEFING_NATIVE_RELATIVE.iter().collect::<PathBuf>());
        let existing = if path.is_file() {
            Some(fs::read(&path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?)
        } else {
            None
        };
        let current = match &existing {
            Some(bytes) => BriefingTextTable::parse(
                &decode_game_text(bytes)
                    .map_err(|e| format!("Failed to decode {}: {e}", path.display()))?,
            )
            .map_err(|e| format!("Corrupted {}: {e}", path.display()))?,
            None => BriefingTextTable::create_empty(),
        };
        let mut edits = BriefingTextTable::create_empty();
        let mut want = |key: &str, value: &str, composite: bool| {
            let old = current.get_value(key);
            if old.unwrap_or_default() != value && !(old.is_none() && value.is_empty()) {
                edits.set_value(key, value, composite);
            }
        };
        want("briefing_titel_1", self.briefing_title.trim(), false);
        want("briefing_titel_2", self.briefing_subtitle.trim(), false);
        want("briefing_text", self.briefing_body.trim(), true);
        for (index, name) in self.briefing_team_names.iter().enumerate() {
            want(
                &format!("briefing_text_teamname{index}"),
                name.trim(),
                false,
            );
        }
        if edits.all_keys().is_empty() {
            return Ok(None);
        }
        let mut tables = std::collections::BTreeMap::new();
        tables.insert(SupportedLanguage::Russian, edits.clone());
        BriefingSaveTransaction::preflight_validate(&tables)?;
        BriefingSaveTransaction::prepare_briefing_bytes(
            existing.as_deref(),
            &edits,
            strict_encoding(GAME_ENCODING_NAME),
        )
        .map(Some)
    }

    /// Safely writes all ambience, soundscape, and briefing configurations to the target map directory.
    pub fn save_to_map(&mut self, map_path: &Path) -> Result<AmbienceSaveReport, String> {
        if crate::storage::is_forbidden_install_path(map_path) {
            return Err(format!(
                "Forbidden: Cannot save ambience directly to game installation folder: {}",
                map_path.display()
            ));
        }
        if !map_path.is_dir() {
            return Err(format!(
                "Target map path does not exist or is not a directory: {}",
                map_path.display()
            ));
        }

        let mut report = AmbienceSaveReport {
            map_path: map_path.to_path_buf(),
            ..Default::default()
        };

        // Stage everything that can fail on content before the first write.
        let ini_bytes = self.stage_boden_ini_bytes(map_path)?;
        let briefing_bytes = self.stage_briefing_bytes(map_path)?;

        // 1. Save boden.ini (unedited original text/PFIL/CP1251 are preserved)
        let ini_path = map_path.join("boden.ini");
        if ini_path.is_file() {
            let bak_path = map_path.join("boden.ini.bak");
            let _ = fs::copy(&ini_path, &bak_path);
            report.boden_ini_backup = Some(bak_path);
        }
        let ini_tmp = map_path.join("boden.ini.tmp_arm");
        fs::write(&ini_tmp, &ini_bytes)
            .map_err(|e| format!("Failed to write temporary boden.ini: {e}"))?;
        fs::rename(&ini_tmp, &ini_path).map_err(|e| format!("Failed to commit boden.ini: {e}"))?;
        report.boden_ini_saved = true;

        // 2. Save Soundscape SNDZ binary
        let snapshot = SoundscapeSnapshot {
            global_ambience_id: self.global_sound_id.clone(),
            global_ambience_volume: self.global_volume,
            zones: self.zones.clone(),
        };
        let sndz_bytes = SoundscapeBinaryStorage::serialize_to_binary(&snapshot)
            .map_err(|e| format!("Failed to serialize soundscape: {e}"))?;
        let sndz_path = map_path.join("soundscape.sndz");
        let sndz_tmp = map_path.join("soundscape.sndz.tmp_arm");
        fs::write(&sndz_tmp, &sndz_bytes)
            .map_err(|e| format!("Failed to write temporary soundscape.sndz: {e}"))?;
        fs::rename(&sndz_tmp, &sndz_path)
            .map_err(|e| format!("Failed to commit soundscape.sndz: {e}"))?;
        report.soundscape_saved = true;

        // 3. Save Briefing PUT script (native TEXT/US/briefing.put)
        if let Some(bytes) = briefing_bytes {
            let briefing_path = map_path.join(BRIEFING_NATIVE_RELATIVE.iter().collect::<PathBuf>());
            let briefing_dir = briefing_path.parent().expect("briefing path has a parent");
            fs::create_dir_all(briefing_dir)
                .map_err(|e| format!("Failed to create {}: {e}", briefing_dir.display()))?;
            let briefing_tmp = briefing_dir.join("briefing.put.tmp_arm");
            fs::write(&briefing_tmp, &bytes)
                .map_err(|e| format!("Failed to write briefing PUT script: {e}"))?;
            fs::rename(&briefing_tmp, &briefing_path)
                .map_err(|e| format!("Failed to commit briefing PUT script: {e}"))?;
            report.briefing_saved = true;
            report.briefing_file = Some(briefing_path);
        }

        self.dirty = false;
        self.status_message = format!(
            "Successfully saved ambience & soundscape to {}.",
            map_path.display()
        );
        self.status_is_error = false;
        Ok(report)
    }

    /// Loads boden.ini, day/night lighting, soundscape SNDZ, and briefing PUT files from the map directory.
    /// Operates transactionally: missing optional files reset to clean defaults; any corrupt or
    /// unreadable file returns an Err immediately and leaves the current ambience session completely untouched.
    pub fn load_from_map(&mut self, map_path: &Path) -> Result<AmbienceLoadReport, String> {
        if !map_path.is_dir() {
            return Err(format!(
                "Target map path is not a directory: {}",
                map_path.display()
            ));
        }

        let mut report = AmbienceLoadReport {
            map_path: map_path.to_path_buf(),
            ..Default::default()
        };

        // 1. Stage boden.ini (or fresh default)
        let ini_path = map_path.join("boden.ini");
        let staged_boden = if ini_path.is_file() {
            let bytes = fs::read(&ini_path)
                .map_err(|e| format!("Failed to read {}: {e}", ini_path.display()))?;
            let content = decode_game_text(&bytes)
                .map_err(|e| format!("Failed to decode {}: {e}", ini_path.display()))?;
            report.boden_ini_loaded = true;
            BodenIniSerializer::parse_text(&content)
        } else {
            WeatherPresets::create_clear_sky().boden
        };

        // 2. Stage Day/Night BMP table for scene lighting (or fresh None)
        let daynight_path = map_path.join("daynight.bmp");
        let staged_daynight = if daynight_path.is_file() {
            let bytes = fs::read(&daynight_path)
                .map_err(|e| format!("Failed to read {}: {e}", daynight_path.display()))?;
            let dn = MapLightingDayNight::parse(&bytes)
                .map_err(|e| format!("Corrupted {}: {e}", daynight_path.display()))?;
            report.daynight_loaded = true;
            Some(dn)
        } else {
            None
        };

        // 3. Stage Soundscape SNDZ (or fresh default)
        let sndz_path = map_path.join("soundscape.sndz");
        let (staged_zones, staged_global_id, staged_global_vol) = if sndz_path.is_file() {
            let bytes = fs::read(&sndz_path)
                .map_err(|e| format!("Failed to read {}: {e}", sndz_path.display()))?;
            let snapshot = SoundscapeBinaryStorage::deserialize_from_binary(&bytes)
                .map_err(|e| format!("Corrupted {}: {e}", sndz_path.display()))?;
            report.soundscape_loaded = true;
            report.soundscape_zones_count = snapshot.zones.len();
            (
                snapshot.zones,
                snapshot.global_ambience_id,
                snapshot.global_ambience_volume,
            )
        } else {
            (Vec::new(), "Amb_Plains_Wind".to_string(), 0.6)
        };

        // 4. Stage Briefing PUT: native TEXT/US/briefing.put first, then legacy SCRIPT/ copies
        let script_dir = map_path.join("SCRIPT");
        let candidates = [
            map_path.join(BRIEFING_NATIVE_RELATIVE.iter().collect::<PathBuf>()),
            script_dir.join(format!(
                "briefing_{}.put",
                self.selected_language.to_directory_code()
            )),
            script_dir.join("briefing.put"),
        ];
        let mut staged_title = String::new();
        let mut staged_body = String::new();
        let mut staged_subtitle = String::new();
        let mut staged_teams: [String; BRIEFING_TEAM_COUNT] = Default::default();
        if let Some(path) = candidates.iter().find(|p| p.is_file()) {
            let bytes =
                fs::read(path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
            let text = decode_game_text(&bytes)
                .map_err(|e| format!("Failed to decode {}: {e}", path.display()))?;
            let table = BriefingTextTable::parse(&text)
                .map_err(|e| format!("Corrupted {}: {e}", path.display()))?;
            staged_title = table.title1().unwrap_or_default().to_string();
            staged_body = table.briefing_text().unwrap_or_default().to_string();
            staged_subtitle = table.title2().unwrap_or_default().to_string();
            for (index, name) in staged_teams.iter_mut().enumerate() {
                *name = table
                    .get_team_name(index as i32)
                    .unwrap_or_default()
                    .to_string();
            }
            report.briefing_loaded = true;
        }

        // Staged parsing succeeded completely without errors. Now atomically commit to self.
        self.profile.boden = staged_boden;
        self.lighting_context.day_night = staged_daynight;
        self.zones = staged_zones;
        self.selected_zone_index = None;
        self.global_sound_id = staged_global_id;
        self.global_volume = staged_global_vol;
        self.briefing_title = staged_title;
        self.briefing_body = staged_body;
        self.briefing_subtitle = staged_subtitle;
        self.briefing_team_names = staged_teams;
        self.dirty = false;
        self.pending_discard = false;
        self.validate_current();
        self.evaluate_scene_lighting();
        self.status_message = format!("Loaded ambience settings from {}.", map_path.display());
        self.status_is_error = false;
        Ok(report)
    }

    /// Legacy direct writer; the product UI saves only through `EditorApp` Safe Save.
    #[cfg(test)]
    pub fn quick_save(&mut self, map_path: &Path) -> bool {
        match self.save_to_map(map_path) {
            Ok(_) => true,
            Err(err) => {
                self.status_message = format!("Ambience save failed (unsaved changes kept): {err}");
                self.status_is_error = true;
                false
            }
        }
    }

    /// Quick-reload button behavior: failures are shown in the status line; state is untouched.
    pub fn quick_reload(&mut self, map_path: &Path) -> bool {
        match self.load_from_map(map_path) {
            Ok(_) => true,
            Err(err) => {
                self.status_message = format!("Ambience reload failed: {err}");
                self.status_is_error = true;
                false
            }
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, map_path: Option<&Path>, water_surface: f32) {
        if !self.open {
            return;
        }

        let mut open = self.open;
        egui::Window::new("Ambience & Localization Studio (環境氛圍與在地化)")
            .open(&mut open)
            .default_width(740.0)
            .show(ctx, |ui| {
                if self.pending_discard {
                    ui.colored_label(
                        Color32::YELLOW,
                        "您有未儲存的環境氛圍設定變更。確定關閉並放棄變更嗎？",
                    );
                    ui.horizontal(|ui| {
                        if ui.button("放棄變更並關閉").clicked() {
                            self.clear_dirty();
                            self.open = false;
                        }
                        if ui.button("繼續編輯").clicked() {
                            self.pending_discard = false;
                        }
                    });
                    ui.separator();
                }

                // 標籤切換
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.current_tab,
                        AmbienceTab::Atmosphere,
                        "Atmosphere & Weather (天候光影)",
                    );
                    ui.selectable_value(
                        &mut self.current_tab,
                        AmbienceTab::Soundscape,
                        "Soundscape Zones (音效區域)",
                    );
                    ui.selectable_value(
                        &mut self.current_tab,
                        AmbienceTab::Localization,
                        "Briefing PUT (任務簡報在地化)",
                    );
                    ui.selectable_value(
                        &mut self.current_tab,
                        AmbienceTab::AppLanguage,
                        "App Language (介面語言)",
                    );
                });
                ui.separator();

                // 持久化快速操作列
                if let Some(path) = map_path {
                    ui.horizontal(|ui| {
                        ui.label(format!("地圖目錄: {}", path.display()));
                        if ui.button("儲存氛圍到地圖 (Save)").clicked() {
                            self.request_save();
                        }
                        if ui.button("從地圖重新載入 (Reload)").clicked() {
                            self.quick_reload(path);
                        }
                    });
                    ui.separator();
                }

                egui::ScrollArea::vertical()
                    .max_height(550.0)
                    .show(ui, |ui| match self.current_tab {
                        AmbienceTab::Atmosphere => {
                            self.render_atmosphere_tab(ui, water_surface, map_path)
                        }
                        AmbienceTab::Soundscape => self.render_soundscape_tab(ui, map_path),
                        AmbienceTab::Localization => self.render_localization_tab(ui),
                        AmbienceTab::AppLanguage => self.render_app_language_tab(ui),
                    });

                ui.separator();
                let status_color = if self.status_is_error {
                    Color32::from_rgb(240, 80, 80)
                } else {
                    Color32::from_rgb(100, 200, 100)
                };
                ui.colored_label(status_color, &self.status_message);
            });

        self.open = open;
    }

    fn render_atmosphere_tab(
        &mut self,
        ui: &mut egui::Ui,
        water_surface: f32,
        map_path: Option<&Path>,
    ) {
        ui.heading("Atmospheric Lighting & Weather Profile");
        ui.label("設定地圖天候、光影、霧氣以及底層 boden.ini 渲染參數。");

        ui.horizontal(|ui| {
            ui.label("Weather Preset (預設天候):");
            let mut changed = false;
            for (id, label) in [
                (WeatherPresets::CLEAR_SKY_ID, "Clear Sky (晴空)"),
                (WeatherPresets::STORM_ID, "Storm (暴雨)"),
                (WeatherPresets::SUNSET_DUSK_ID, "Sunset (黃昏)"),
                (WeatherPresets::DENSE_FOG_ID, "Dense Fog (濃霧)"),
                (WeatherPresets::MOUNTAIN_SNOW_ID, "Mountain Snow (霜雪)"),
            ] {
                if ui
                    .selectable_label(self.selected_preset == id, label)
                    .clicked()
                {
                    self.load_preset(id);
                    changed = true;
                }
            }
            if changed {
                self.status_message = format!("Loaded preset '{}'.", self.selected_preset);
                self.status_is_error = false;
            }
        });

        ui.separator();
        egui::Grid::new("atmosphere_grid")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Precipitation (降水型態):");
                egui::ComboBox::from_id_source("precip_select")
                    .selected_text(format!("{:?}", self.profile.precipitation))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.profile.precipitation,
                            PrecipitationType::None,
                            "None (無)",
                        );
                        ui.selectable_value(
                            &mut self.profile.precipitation,
                            PrecipitationType::Rain,
                            "Rain (雨)",
                        );
                        ui.selectable_value(
                            &mut self.profile.precipitation,
                            PrecipitationType::Snow,
                            "Snow (雪)",
                        );
                        ui.selectable_value(
                            &mut self.profile.precipitation,
                            PrecipitationType::Hail,
                            "Hail (冰雹)",
                        );
                    });
                ui.end_row();

                ui.label("Precipitation Intensity:");
                if ui
                    .add(egui::Slider::new(
                        &mut self.profile.precipitation_intensity,
                        0.0..=1.0,
                    ))
                    .changed()
                {
                    self.dirty = true;
                    self.evaluate_scene_lighting();
                }
                ui.end_row();

                ui.label("Wind Speed (風速):");
                if ui
                    .add(egui::Slider::new(&mut self.profile.wind_speed, 0.0..=30.0).suffix(" m/s"))
                    .changed()
                {
                    self.dirty = true;
                }
                ui.end_row();

                ui.label("Wind Direction (風向角度):");
                if ui
                    .add(
                        egui::Slider::new(&mut self.profile.wind_direction, 0.0..=360.0)
                            .suffix("°"),
                    )
                    .changed()
                {
                    self.dirty = true;
                }
                ui.end_row();

                ui.label("Fog Enabled (迷霧):");
                if ui.checkbox(&mut self.profile.fog_enabled, "").changed() {
                    self.dirty = true;
                    self.evaluate_scene_lighting();
                }
                ui.end_row();

                if self.profile.fog_enabled {
                    ui.label("Fog Distance (Start / End):");
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::DragValue::new(&mut self.profile.fog_start_distance)
                                    .prefix("Start: "),
                            )
                            .changed()
                        {
                            self.dirty = true;
                            self.evaluate_scene_lighting();
                        }
                        if ui
                            .add(
                                egui::DragValue::new(&mut self.profile.fog_end_distance)
                                    .prefix("End: "),
                            )
                            .changed()
                        {
                            self.dirty = true;
                            self.evaluate_scene_lighting();
                        }
                    });
                    ui.end_row();

                    ui.label("Fog Density:");
                    if ui
                        .add(egui::Slider::new(&mut self.profile.fog_density, 0.0..=1.0))
                        .changed()
                    {
                        self.dirty = true;
                        self.evaluate_scene_lighting();
                    }
                    ui.end_row();
                }

                ui.label("Sun / Ambient Exposure:");
                if ui
                    .add(egui::Slider::new(&mut self.profile.exposure, 0.2..=2.0))
                    .changed()
                {
                    self.dirty = true;
                    self.evaluate_scene_lighting();
                }
                ui.end_row();
            });

        ui.separator();
        ui.heading("Scene Lighting Bridge (原生場景光照計算)");
        ui.horizontal(|ui| {
            ui.label("Game Clock (遊戲時間 0..24h):");
            if ui
                .add(egui::Slider::new(&mut self.game_hour, 0.0..=24.0).suffix("h"))
                .changed()
            {
                self.evaluate_scene_lighting();
            }
            if ui.button("Trigger Flash (閃電爆閃)").clicked() {
                self.trigger_manual_flash(1.0);
            }
        });

        if let Some(uniforms) = &self.last_computed_uniforms {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "Effective Ambient: [{:.2}, {:.2}, {:.2}] | Fog: [{:.2}, {:.2}, {:.2}] | Flash: {:.2}",
                    uniforms.effective_ambient[0],
                    uniforms.effective_ambient[1],
                    uniforms.effective_ambient[2],
                    uniforms.fog_color[0],
                    uniforms.fog_color[1],
                    uniforms.fog_color[2],
                    uniforms.flash_intensity,
                ));
            });
        }

        ui.separator();
        ui.heading("boden.ini Environment Parameters");
        egui::Grid::new("boden_ini_grid")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Water Level (水位):");
                ui.horizontal(|ui| {
                    if ui
                        .add(egui::DragValue::new(&mut self.profile.boden.waterlevel).speed(0.5))
                        .changed()
                    {
                        self.dirty = true;
                        self.validate_current();
                    }
                    if ui.button("Sync with Editor Water").clicked() {
                        self.sync_water_level(water_surface);
                    }
                });
                ui.end_row();

                ui.label("Water Color (BGR Hex, 例 0xffdfbf):");
                ui.horizontal(|ui| {
                    // Color picker over the BGR hex string (C# ChooseWaterColor); text stays editable.
                    let rgb = self.profile.boden.get_water_rgb();
                    let mut color = rgb;
                    if ui.color_edit_button_rgb(&mut color).changed() && color != rgb {
                        self.profile.boden.set_water_rgb(color[0], color[1], color[2]);
                        self.dirty = true;
                        self.validate_current();
                    }
                    if ui
                        .text_edit_singleline(&mut self.profile.boden.water_color)
                        .changed()
                    {
                        self.dirty = true;
                        self.validate_current();
                    }
                });
                ui.end_row();

                ui.label("Water Bump Amplitude:");
                if ui
                    .add(egui::Slider::new(
                        &mut self.profile.boden.water_bump_amplitude,
                        0..=1024,
                    ))
                    .changed()
                {
                    self.dirty = true;
                    self.validate_current();
                    self.evaluate_scene_lighting();
                }
                ui.end_row();

                ui.label("Water Bump Frequency:");
                if ui
                    .add(egui::Slider::new(
                        &mut self.profile.boden.water_bump_frequency,
                        1..=16,
                    ))
                    .changed()
                {
                    self.dirty = true;
                    self.validate_current();
                    self.evaluate_scene_lighting();
                }
                ui.end_row();

                ui.label("Water Warp Shift (0 = 關閉, 10..18):");
                if ui
                    .add(egui::Slider::new(
                        &mut self.profile.boden.water_warp_shift,
                        0..=18,
                    ))
                    .changed()
                {
                    self.dirty = true;
                    self.validate_current();
                    self.evaluate_scene_lighting();
                }
                ui.end_row();

                ui.label("Flash Probability (雷電機率 0..1000):");
                if ui
                    .add(egui::Slider::new(
                        &mut self.profile.boden.flash_propability,
                        0..=1000,
                    ))
                    .changed()
                {
                    self.dirty = true;
                    self.validate_current();
                }
                ui.end_row();

                ui.label("Day Start / End Time (0..24h):");
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::DragValue::new(&mut self.profile.boden.day_start_time)
                                .speed(0.1)
                                .prefix("Start: "),
                        )
                        .changed()
                    {
                        self.dirty = true;
                        self.validate_current();
                    }
                    if ui
                        .add(
                            egui::DragValue::new(&mut self.profile.boden.day_end_time)
                                .speed(0.1)
                                .prefix("End: "),
                        )
                        .changed()
                    {
                        self.dirty = true;
                        self.validate_current();
                    }
                });
                ui.end_row();
            });

        if !self.validation_errors.is_empty() {
            ui.separator();
            ui.colored_label(Color32::from_rgb(255, 120, 50), "參數診斷警告：");
            for err in &self.validation_errors {
                ui.colored_label(Color32::from_rgb(255, 180, 80), format!("• {err}"));
            }
        }

        ui.add_space(8.0);
        if let Some(path) = map_path {
            ui.label(format!("Active map directory: {}", path.display()));
        } else {
            ui.label("No active map folder loaded. Parameters are in-memory.");
        }
    }

    fn render_soundscape_tab(&mut self, ui: &mut egui::Ui, _map_path: Option<&Path>) {
        ui.heading("Soundscape Zone Studio (SNDZ)");
        ui.label("配置地圖環境音效區域：水體流動、森林鳥鳴、定居點人聲、戰場氛圍。");

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Global Ambience Sound ID:");
            if ui.text_edit_singleline(&mut self.global_sound_id).changed() {
                self.dirty = true;
            }
            ui.label("Volume:");
            if ui
                .add(egui::Slider::new(&mut self.global_volume, 0.0..=1.0))
                .changed()
            {
                self.dirty = true;
            }
        });

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("New Zone Name:");
            ui.text_edit_singleline(&mut self.new_zone_name);

            ui.label("Sound ID:");
            ui.text_edit_singleline(&mut self.new_zone_sound_id);

            egui::ComboBox::from_id_source("new_cat_select")
                .selected_text(format!("{:?}", self.new_zone_category))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.new_zone_category,
                        AudioCategory::Hydrology,
                        "Hydrology (水文)",
                    );
                    ui.selectable_value(
                        &mut self.new_zone_category,
                        AudioCategory::Vegetation,
                        "Vegetation (植被/森林)",
                    );
                    ui.selectable_value(
                        &mut self.new_zone_category,
                        AudioCategory::Settlement,
                        "Settlement (城鎮/村落)",
                    );
                    ui.selectable_value(
                        &mut self.new_zone_category,
                        AudioCategory::AmbientGlobal,
                        "AmbientGlobal (全域)",
                    );
                    ui.selectable_value(
                        &mut self.new_zone_category,
                        AudioCategory::WeatherFx,
                        "WeatherFx (天候音效)",
                    );
                });

            if ui.button("Add Sound Zone").clicked() {
                let name = self.new_zone_name.clone();
                let sound_id = self.new_zone_sound_id.clone();
                let category = self.new_zone_category;
                let shape = self.new_zone_shape;
                self.add_sound_zone(&name, &sound_id, category, shape);
            }
        });

        ui.separator();
        ui.label(format!("Configured Sound Zones: {}", self.zones.len()));

        egui::ScrollArea::vertical()
            .max_height(200.0)
            .show(ui, |ui| {
                let mut remove_idx = None;
                for (i, zone) in self.zones.iter().enumerate() {
                    let is_sel = self.selected_zone_index == Some(i);
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(
                                is_sel,
                                format!(
                                    "#{i}: {} ({:?}) - {}",
                                    zone.name, zone.category, zone.sound_def_id
                                ),
                            )
                            .clicked()
                        {
                            self.selected_zone_index = Some(i);
                        }
                        if ui.small_button("Delete").clicked() {
                            remove_idx = Some(i);
                        }
                    });
                }
                if let Some(i) = remove_idx {
                    self.remove_sound_zone(i);
                }
            });

        if let Some(idx) = self.selected_zone_index {
            if idx < self.zones.len() {
                let zone = &mut self.zones[idx];
                ui.separator();
                ui.heading(format!("Edit Zone: {}", zone.name));
                egui::Grid::new("zone_edit_grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Zone Name:");
                        if ui.text_edit_singleline(&mut zone.name).changed() {
                            self.dirty = true;
                        }
                        ui.end_row();

                        ui.label("Sound Definition ID:");
                        if ui.text_edit_singleline(&mut zone.sound_def_id).changed() {
                            self.dirty = true;
                        }
                        ui.end_row();

                        ui.label("Base Volume (基礎音量):");
                        if ui
                            .add(egui::Slider::new(&mut zone.base_volume, 0.0..=2.0))
                            .changed()
                        {
                            self.dirty = true;
                        }
                        ui.end_row();

                        ui.label("Inner / Outer Radius:");
                        ui.horizontal(|ui| {
                            if ui
                                .add(egui::DragValue::new(&mut zone.inner_radius).prefix("Inner: "))
                                .changed()
                            {
                                self.dirty = true;
                            }
                            if ui
                                .add(egui::DragValue::new(&mut zone.outer_radius).prefix("Outer: "))
                                .changed()
                            {
                                self.dirty = true;
                            }
                        });
                        ui.end_row();

                        ui.label("Center Position (X, Z):");
                        ui.horizontal(|ui| {
                            if ui
                                .add(egui::DragValue::new(&mut zone.center_world_x).prefix("X: "))
                                .changed()
                            {
                                self.dirty = true;
                            }
                            if ui
                                .add(egui::DragValue::new(&mut zone.center_world_z).prefix("Z: "))
                                .changed()
                            {
                                self.dirty = true;
                            }
                        });
                        ui.end_row();

                        ui.label("Attenuation Curve:");
                        egui::ComboBox::from_id_source("atten_curve_select")
                            .selected_text(format!("{:?}", zone.attenuation_curve))
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_value(
                                        &mut zone.attenuation_curve,
                                        AttenuationCurveKind::SmoothStep,
                                        "SmoothStep",
                                    )
                                    .clicked()
                                {
                                    self.dirty = true;
                                }
                                if ui
                                    .selectable_value(
                                        &mut zone.attenuation_curve,
                                        AttenuationCurveKind::Linear,
                                        "Linear",
                                    )
                                    .clicked()
                                {
                                    self.dirty = true;
                                }
                                if ui
                                    .selectable_value(
                                        &mut zone.attenuation_curve,
                                        AttenuationCurveKind::Logarithmic,
                                        "Logarithmic",
                                    )
                                    .clicked()
                                {
                                    self.dirty = true;
                                }
                                if ui
                                    .selectable_value(
                                        &mut zone.attenuation_curve,
                                        AttenuationCurveKind::Exponential,
                                        "Exponential",
                                    )
                                    .clicked()
                                {
                                    self.dirty = true;
                                }
                            });
                        ui.end_row();
                    });
            }
        }
    }

    fn render_localization_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Map Briefing Localization (任務簡報多語言)");
        ui.label(
            "編輯地圖任務標題、副標題、說明文字與隊伍名稱（原生 TEXT/US/briefing.put，CP1251）。",
        );

        ui.separator();
        ui.label(format!(
            "Path: {} | Encoding: CP1251 (PFIL preserved)",
            BRIEFING_NATIVE_RELATIVE.join("/")
        ));

        ui.label("Briefing Title:");
        if ui.text_edit_singleline(&mut self.briefing_title).changed() {
            self.dirty = true;
        }

        ui.label("Briefing Subtitle:");
        if ui
            .text_edit_singleline(&mut self.briefing_subtitle)
            .changed()
        {
            self.dirty = true;
        }

        ui.label("Briefing Description / Objectives:");
        if ui
            .add(egui::TextEdit::multiline(&mut self.briefing_body).desired_rows(6))
            .changed()
        {
            self.dirty = true;
        }

        ui.collapsing("Team Names", |ui| {
            for (index, name) in self.briefing_team_names.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!("Team {index}:"));
                    if ui.text_edit_singleline(name).changed() {
                        self.dirty = true;
                    }
                });
            }
        });

        // 語法診斷
        let mut table = BriefingTextTable::create_empty();
        table.set_title1(&self.briefing_title);
        table.set_title2(&self.briefing_subtitle);
        table.set_briefing_text(&self.briefing_body);
        let validation = PutScriptSyntaxValidator::validate_table(&table, GAME_ENCODING_NAME);
        if !validation.is_valid() {
            ui.separator();
            ui.colored_label(
                Color32::from_rgb(255, 90, 90),
                format!("PUT 語法錯誤 ({} 項):", validation.error_count()),
            );
            for issue in &validation.issues {
                ui.colored_label(
                    Color32::from_rgb(255, 120, 120),
                    format!("• [{}] {}", issue.key, issue.message),
                );
            }
        }
    }

    fn render_app_language_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Application Language (介面語言)");
        ui.label("切換地圖編輯器介面顯示語系。");

        ui.horizontal(|ui| {
            if ui
                .selectable_label(
                    self.app_language == AppLanguage::TraditionalChinese,
                    "繁體中文 (Traditional Chinese)",
                )
                .clicked()
            {
                self.app_language = AppLanguage::TraditionalChinese;
                self.status_message = "介面語言已設為繁體中文。".to_string();
                self.status_is_error = false;
            }
            if ui
                .selectable_label(self.app_language == AppLanguage::English, "English")
                .clicked()
            {
                self.app_language = AppLanguage::English;
                self.status_message = "Application language set to English.".to_string();
                self.status_is_error = false;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ambience_ui_initial_state_and_dirty_guards() {
        let mut ui = AmbienceUi::default();
        assert!(!ui.open);
        assert!(!ui.is_dirty());

        ui.mark_dirty();
        assert!(ui.is_dirty());

        ui.request_close();
        assert!(ui.pending_discard);

        ui.clear_dirty();
        assert!(!ui.is_dirty());
        assert!(!ui.pending_discard);
    }

    #[test]
    fn test_ambience_ui_preset_loading() {
        let mut ui = AmbienceUi::default();
        ui.load_preset(WeatherPresets::STORM_ID);
        assert_eq!(ui.profile.preset_id, WeatherPresets::STORM_ID);
        assert_eq!(ui.profile.precipitation, PrecipitationType::Rain);
        assert!(ui.profile.wind_speed > 10.0);
        assert!(ui.is_dirty());

        ui.load_preset(WeatherPresets::CLEAR_SKY_ID);
        assert_eq!(ui.profile.preset_id, WeatherPresets::CLEAR_SKY_ID);
        assert_eq!(ui.profile.precipitation, PrecipitationType::None);
    }

    #[test]
    fn test_ambience_ui_boden_validation() {
        let mut ui = AmbienceUi::default();
        assert!(ui.validate_current());
        assert!(ui.validation_errors.is_empty());

        // Set invalid water bump amplitude
        ui.profile.boden.water_bump_amplitude = 2000;
        assert!(!ui.validate_current());
        assert!(!ui.validation_errors.is_empty());
    }

    #[test]
    fn test_water_color_picker_roundtrips_bgr_hex() {
        let mut ui = AmbienceUi::default();
        ui.profile.boden.set_water_rgb(1.0, 0.0, 0.5);
        // BGR order: blue 0x80, green 0x00, red 0xff.
        assert_eq!(ui.profile.boden.water_color, "0x8000ff");
        assert_eq!(ui.profile.boden.get_water_rgb(), [1.0, 0.0, 128.0 / 255.0]);
    }

    #[test]
    fn test_ambience_ui_sync_water_level() {
        let mut ui = AmbienceUi::default();
        ui.sync_water_level(42.5);
        assert_eq!(ui.profile.boden.waterlevel, 42.5);
        assert!(ui.is_dirty());
    }

    #[test]
    fn test_ambience_ui_sound_zone_management() {
        let mut ui = AmbienceUi::default();
        assert!(ui.zones.is_empty());

        let id = ui.add_sound_zone(
            "River Flow",
            "SND_WATER_FLOW",
            AudioCategory::Hydrology,
            SoundscapeShapeType::OrientedRectangle,
        );
        assert_eq!(ui.zones.len(), 1);
        assert_eq!(ui.zones[0].id, id);
        assert_eq!(ui.zones[0].category, AudioCategory::Hydrology);
        assert!(ui.is_dirty());

        ui.remove_sound_zone(0);
        assert!(ui.zones.is_empty());
        assert_eq!(ui.selected_zone_index, None);
    }

    #[test]
    fn test_ambience_ui_scene_lighting_bridge() {
        let mut ui = AmbienceUi::default();
        ui.game_hour = 14.0;
        let uniforms = ui.evaluate_scene_lighting();
        assert_eq!(
            uniforms.water_dynamics[0],
            ui.profile.boden.water_bump_amplitude as f32
        );
        assert!(uniforms.effective_ambient[0] > 0.0);

        ui.trigger_manual_flash(0.9);
        let lit_uniforms = ui.evaluate_scene_lighting();
        assert!(lit_uniforms.flash_intensity > 0.8);
    }

    #[test]
    fn test_ambience_ui_persistence_roundtrip() {
        let temp_dir = std::env::temp_dir().join(format!("arm_test_ambience_{}", Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).expect("create temp dir");

        let mut ui = AmbienceUi::default();
        ui.load_preset(WeatherPresets::STORM_ID);
        ui.profile.boden.waterlevel = 55.0;
        ui.profile.boden.water_bump_amplitude = 310;
        ui.add_sound_zone(
            "Waterfall",
            "SND_WATERFALL",
            AudioCategory::Hydrology,
            SoundscapeShapeType::Circle,
        );
        ui.briefing_title = "Defend the Pass".to_string();
        ui.briefing_body = "Hold against barbarian horde.".to_string();

        let save_rep = ui.save_to_map(&temp_dir).expect("save should succeed");
        assert!(save_rep.boden_ini_saved);
        assert!(save_rep.soundscape_saved);
        assert!(save_rep.briefing_saved);
        assert!(!ui.is_dirty());

        // Verify disk files exist
        assert!(temp_dir.join("boden.ini").is_file());
        assert!(temp_dir.join("soundscape.sndz").is_file());
        assert!(temp_dir.join("TEXT/US/briefing.put").is_file());

        // Load into clean UI
        let mut loaded_ui = AmbienceUi::default();
        let load_rep = loaded_ui
            .load_from_map(&temp_dir)
            .expect("load should succeed");
        assert!(load_rep.boden_ini_loaded);
        assert!(load_rep.soundscape_loaded);
        assert_eq!(load_rep.soundscape_zones_count, 1);
        assert!(load_rep.briefing_loaded);

        assert_eq!(loaded_ui.profile.boden.waterlevel, 55.0);
        assert_eq!(loaded_ui.profile.boden.water_bump_amplitude, 310);
        assert_eq!(loaded_ui.zones.len(), 1);
        assert_eq!(loaded_ui.zones[0].name, "Waterfall");
        assert_eq!(loaded_ui.briefing_title, "Defend the Pass");
        assert_eq!(loaded_ui.briefing_body, "Hold against barbarian horde.");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn save_request_lifecycle_does_not_touch_disk_or_dirty() {
        let mut ui = AmbienceUi::default();
        ui.mark_dirty();
        assert!(!ui.take_save_request());
        ui.request_save();
        ui.request_save();
        assert!(ui.is_dirty());
        assert!(ui.take_save_request());
        assert!(!ui.take_save_request());
        assert!(ui.is_dirty());
    }

    #[test]
    fn quick_save_and_reload_report_errors_and_keep_dirty() {
        let missing = std::env::temp_dir().join(format!("arm_missing_ambience_{}", Uuid::new_v4()));
        let mut ui = AmbienceUi::default();
        ui.mark_dirty();
        assert!(!ui.quick_save(&missing));
        assert!(ui.status_is_error);
        assert!(ui.status_message.contains("save failed"));
        assert!(ui.is_dirty());

        ui.status_is_error = false;
        assert!(!ui.quick_reload(&missing));
        assert!(ui.status_is_error);
        assert!(ui.status_message.contains("reload failed"));
        assert!(
            ui.is_dirty(),
            "failed reload must not discard unsaved edits"
        );

        let dir = std::env::temp_dir().join(format!("arm_quick_ambience_{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        assert!(ui.quick_save(&dir));
        assert!(!ui.status_is_error);
        assert!(!ui.is_dirty());
        assert!(ui.quick_reload(&dir));
        assert!(!ui.status_is_error);
        let _ = fs::remove_dir_all(&dir);
    }

    fn temp_map(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("arm_{tag}_{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create temp map");
        dir
    }

    fn cp1251(text: &str) -> Vec<u8> {
        encode_strict(text, strict_encoding(GAME_ENCODING_NAME)).expect("encodable as CP1251")
    }

    fn pfil(text: &str) -> Vec<u8> {
        let mut header = vec![0u8; 64];
        header[..4].copy_from_slice(b"PFIL");
        repack_pfil(&cp1251(text), &header).expect("pack pfil")
    }

    fn unpack(bytes: &[u8]) -> String {
        decode_game_text(bytes).expect("decode game text")
    }

    const RU_INI: &str = "; \u{43f}\u{440}\u{438}\u{432}\u{435}\u{442}\r\n[Waterlevel] ;0..\r\n30.5\r\n[Heightmapstep]\r\n2.0\r\n[CustomNote]\r\n\u{41f}\u{440}\u{438}\u{432}\u{435}\u{442}\r\n";

    #[test]
    fn boden_ini_loads_pfil_cp1251_and_unchanged_save_keeps_original_bytes() {
        let dir = temp_map("boden_pfil");
        let original = pfil(RU_INI);
        assert!(against_rome_data::lzss::has_pfil_magic(&original));
        fs::write(dir.join("boden.ini"), &original).unwrap();

        let mut ui = AmbienceUi::default();
        let report = ui.load_from_map(&dir).expect("PFIL boden.ini must load");
        assert!(report.boden_ini_loaded);
        assert_eq!(ui.profile.boden.waterlevel, 30.5);
        assert_eq!(ui.profile.boden.heightmapstep, 2.0);

        // Raw CP1251 (non-PFIL) loads too.
        let raw_dir = temp_map("boden_raw1251");
        fs::write(raw_dir.join("boden.ini"), cp1251(RU_INI)).unwrap();
        let mut raw_ui = AmbienceUi::default();
        raw_ui
            .load_from_map(&raw_dir)
            .expect("raw CP1251 boden.ini must load");
        assert_eq!(raw_ui.profile.boden.waterlevel, 30.5);

        // Nothing edited: the file stays byte-identical (no reformatting, no re-encode).
        ui.mark_dirty();
        ui.save_to_map(&dir).expect("save");
        assert_eq!(fs::read(dir.join("boden.ini")).unwrap(), original);

        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&raw_dir);
    }

    #[test]
    fn boden_ini_edit_patches_value_and_keeps_pfil_cp1251_text() {
        let dir = temp_map("boden_edit");
        fs::write(dir.join("boden.ini"), pfil(RU_INI)).unwrap();

        let mut ui = AmbienceUi::default();
        ui.load_from_map(&dir).unwrap();
        ui.sync_water_level(77.0);
        let report = ui.save_to_map(&dir).expect("save");
        assert!(report.boden_ini_backup.is_some());

        let saved = fs::read(dir.join("boden.ini")).unwrap();
        assert!(
            against_rome_data::lzss::has_pfil_magic(&saved),
            "PFIL wrapper must be kept"
        );
        let text = unpack(&saved);
        assert!(
            text.starts_with("; \u{43f}\u{440}\u{438}\u{432}\u{435}\u{442}\r\n"),
            "leading comment kept: {text:?}"
        );
        assert!(
            text.contains("[Waterlevel] ;0..\r\n77\r\n"),
            "value patched in place: {text:?}"
        );
        assert!(
            text.contains("[Heightmapstep]\r\n2.0\r\n"),
            "untouched value text kept: {text:?}"
        );
        assert!(
            text.contains("[CustomNote]\r\n\u{41f}\u{440}\u{438}\u{432}\u{435}\u{442}\r\n"),
            "unknown CP1251 section kept"
        );

        let mut reloaded = AmbienceUi::default();
        reloaded.load_from_map(&dir).unwrap();
        assert_eq!(reloaded.profile.boden.waterlevel, 77.0);
        assert_eq!(reloaded.profile.boden.heightmapstep, 2.0);
        // The pre-edit original survives as boden.ini.bak.
        assert_eq!(fs::read(dir.join("boden.ini.bak")).unwrap(), pfil(RU_INI));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn boden_ini_new_file_is_plain_cp1251_and_unencodable_text_is_rejected() {
        let dir = temp_map("boden_new");
        let mut ui = AmbienceUi::default();
        ui.sync_water_level(12.0);
        ui.save_to_map(&dir).unwrap();
        let bytes = fs::read(dir.join("boden.ini")).unwrap();
        assert!(!against_rome_data::lzss::has_pfil_magic(&bytes));
        assert!(unpack(&bytes).contains("[Waterlevel]"));

        // CP1251 cannot hold CJK: refuse to save instead of writing substitute characters.
        ui.profile.boden.sky_textur_name = "\u{5929}\u{7a7a}".into();
        let before = fs::read(dir.join("boden.ini")).unwrap();
        assert!(ui.save_to_map(&dir).is_err());
        assert_eq!(fs::read(dir.join("boden.ini")).unwrap(), before);
        let _ = fs::remove_dir_all(&dir);
    }

    const RU_PUT: &str = "var: briefing_titel_1 = \"\u{41c}\u{438}\u{441}\u{441}\u{438}\u{44f}\";\r\nvar: briefing_titel_2 = \"old sub\";\r\nvar: briefing_text = \"\u{422}\u{435}\u{43a}\u{441}\u{442}\";\r\nvar: briefing_text_teamname1 = \"Rome\";\r\nvar: debriefing_text_win = \"\u{41f}\u{43e}\u{431}\u{435}\u{434}\u{430}\";\r\nvar: briefing_video_nummmer = 3;\r\n";

    #[test]
    fn briefing_uses_native_text_us_path_not_script() {
        let dir = temp_map("briefing_path");
        let mut ui = AmbienceUi::default();
        ui.briefing_title = "Defend".into();
        ui.briefing_subtitle = "Act I".into();
        ui.briefing_body = "Hold the pass.".into();
        ui.briefing_team_names[0] = "Romans".into();
        ui.briefing_team_names[3] = "Goths".into();
        // Language selection must not change the native location.
        ui.selected_language = SupportedLanguage::German;

        let report = ui.save_to_map(&dir).unwrap();
        let native = dir.join("TEXT").join("US").join("briefing.put");
        assert_eq!(report.briefing_file.as_deref(), Some(native.as_path()));
        assert!(native.is_file());
        assert!(
            !dir.join("SCRIPT").exists(),
            "legacy SCRIPT/briefing*.put must not be created"
        );
        assert!(!dir.join("TEXT").join("DE").exists());

        let mut loaded = AmbienceUi::default();
        loaded.selected_language = SupportedLanguage::English;
        assert!(loaded.load_from_map(&dir).unwrap().briefing_loaded);
        assert_eq!(loaded.briefing_title, "Defend");
        assert_eq!(loaded.briefing_subtitle, "Act I");
        assert_eq!(loaded.briefing_body, "Hold the pass.");
        assert_eq!(loaded.briefing_team_names[0], "Romans");
        assert_eq!(loaded.briefing_team_names[3], "Goths");
        assert_eq!(loaded.briefing_team_names[1], "");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn briefing_edit_preserves_pfil_cp1251_and_extra_variables() {
        let dir = temp_map("briefing_extras");
        let native_dir = dir.join("TEXT").join("US");
        fs::create_dir_all(&native_dir).unwrap();
        fs::write(native_dir.join("briefing.put"), pfil(RU_PUT)).unwrap();

        let mut ui = AmbienceUi::default();
        ui.load_from_map(&dir).unwrap();
        assert_eq!(
            ui.briefing_title,
            "\u{41c}\u{438}\u{441}\u{441}\u{438}\u{44f}"
        );
        assert_eq!(ui.briefing_subtitle, "old sub");
        assert_eq!(ui.briefing_team_names[1], "Rome");

        // Nothing edited -> nothing staged, file byte-identical after save.
        assert!(ui.stage_briefing_bytes(&dir).unwrap().is_none());
        ui.mark_dirty();
        ui.save_to_map(&dir).unwrap();
        assert_eq!(
            fs::read(native_dir.join("briefing.put")).unwrap(),
            pfil(RU_PUT)
        );

        ui.briefing_subtitle = "new sub".into();
        ui.briefing_team_names[2] = "\u{413}\u{43e}\u{442}\u{44b}".into();
        ui.save_to_map(&dir).unwrap();

        let saved = fs::read(native_dir.join("briefing.put")).unwrap();
        assert!(against_rome_data::lzss::has_pfil_magic(&saved));
        let text = unpack(&saved);
        assert!(text.contains("new sub") && !text.contains("old sub"));

        // Values are compared semantically: the shared serializer normalizes `var:` spacing.
        let table = BriefingTextTable::parse(&text).unwrap();
        assert_eq!(
            table.title1(),
            Some("\u{41c}\u{438}\u{441}\u{441}\u{438}\u{44f}")
        );
        assert_eq!(
            table.briefing_text(),
            Some("\u{422}\u{435}\u{43a}\u{441}\u{442}")
        );
        assert_eq!(table.get_team_name(1), Some("Rome"));
        assert_eq!(
            table.document.get_integer_value("briefing_video_nummmer"),
            Some(3)
        );
        assert_eq!(
            table.debriefing_win(),
            Some("\u{41f}\u{43e}\u{431}\u{435}\u{434}\u{430}")
        );
        assert_eq!(table.get_team_name(2), Some("\u{413}\u{43e}\u{442}\u{44b}"));

        let mut reloaded = AmbienceUi::default();
        reloaded.load_from_map(&dir).unwrap();
        assert_eq!(reloaded.briefing_subtitle, "new sub");
        assert_eq!(
            reloaded.briefing_team_names[2],
            "\u{413}\u{43e}\u{442}\u{44b}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn briefing_load_falls_back_to_legacy_script_copy_and_migrates_on_save() {
        let dir = temp_map("briefing_legacy");
        fs::create_dir_all(dir.join("SCRIPT")).unwrap();
        fs::write(dir.join("SCRIPT").join("briefing.put"), RU_PUT.as_bytes()).unwrap();

        let mut ui = AmbienceUi::default();
        ui.load_from_map(&dir).unwrap();
        assert_eq!(ui.briefing_subtitle, "old sub");
        ui.briefing_title = "Moved".into();
        ui.save_to_map(&dir).unwrap();
        assert!(dir.join("TEXT").join("US").join("briefing.put").is_file());
        let mut reloaded = AmbienceUi::default();
        reloaded.load_from_map(&dir).unwrap();
        assert_eq!(reloaded.briefing_title, "Moved");
        let _ = fs::remove_dir_all(&dir);
    }
}
