//! Modifier Studio: persistent feature/profile selection with a safe preview -> apply -> undo workflow.
//!
//! Data comes from the native `against-rome-modifier` crate (`FeatureRegistry`, `PatchProfile`,
//! `PatchEngine`, `commit_plan`). Planning never writes to the game directory. Every committed
//! operation first stores the pre-change bytes of each touched file in a snapshot directory, so
//! it can be undone later (and the undo itself is previewable and snapshotted).

use against_rome_ambience::app_localization::{Language, Loc};
use against_rome_modifier::{
    icons::{self, UnitIcons, ICON_INI_KEY},
    cli::{commit_plan, profile_from_json, profile_to_json, ENABLE_ALL_EXCLUDED},
    features::{FeatureCategory, FeatureControlKind, FeatureDefinition, FeatureRegistry, PatchProfile},
    install::{plan_install, sha256, InstallResources},
    services::{BackupManager, GameDirectoryLocator, PatchEngine, PatchFilePlan},
    troops::TroopConfig,
};
use eframe::egui::{self, Color32};
use serde_json::{json, Value};
use std::{
    fs, io,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

pub type ModResult<T> = Result<T, String>;

/// Process spawner used by "Start Game": `(exe path, working directory)`. Injectable so tests never spawn.
pub type GameLauncher = Box<dyn Fn(&Path, &Path) -> io::Result<()> + Send + Sync>;

/// Real launcher: `Against_Rome.exe` with the game directory as working directory (C# `ProcessStartInfo`).
pub fn spawn_game_process(exe: &Path, work_dir: &Path) -> io::Result<()> {
    std::process::Command::new(exe).current_dir(work_dir).spawn().map(|_| ())
}

/// Features shown on the Experimental page until promoted (C# `InitializeExperimentalFeatures`).
pub const EXPERIMENTAL_FEATURES: [&str; 8] = [
    "Balance",
    "SpellHealing10x",
    "SpellResurrection",
    "GeneralSkills",
    "LeaderGlory",
    "VillageGarrisonQuota3x",
    "ArgmTrace",
    "CorpseRetention",
];

pub fn is_experimental_feature(id: &str) -> bool {
    EXPERIMENTAL_FEATURES.iter().any(|x| x.eq_ignore_ascii_case(id))
}
const APP_SETTINGS_FILE: &str = "app_settings.json";

fn io_err(e: io::Error) -> String {
    e.to_string()
}

const SPEED_ID: &str = "GameSpeed";
const QUOTA_ID: &str = "VillageGarrisonQuotaMultiplier";
const STATE_FILE: &str = "modifier_state.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RestoreScope {
    #[default]
    All,
    Stats,
    Compat,
    Language,
}

impl RestoreScope {
    pub const ALL: [RestoreScope; 4] = [Self::All, Self::Stats, Self::Compat, Self::Language];
    pub fn key(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Stats => "stats",
            Self::Compat => "compat",
            Self::Language => "language",
        }
    }
    pub fn from_key(s: &str) -> Self {
        Self::ALL.into_iter().find(|k| k.key() == s).unwrap_or_default()
    }
    /// Categories whose toggles are reset after a successful restore (C# `RestoreAll*` `resetCategories`).
    pub fn categories(self) -> &'static [FeatureCategory] {
        match self {
            Self::All => &[FeatureCategory::Stats, FeatureCategory::Compat, FeatureCategory::Language],
            Self::Stats => &[FeatureCategory::Stats],
            Self::Compat => &[FeatureCategory::Compat],
            Self::Language => &[FeatureCategory::Language],
        }
    }
    pub fn label_for(self, en: bool) -> &'static str {
        match (self, en) {
            (Self::All, true) => "Restore all (Stats+Compat+Language)",
            (Self::Stats, true) => "Stats only",
            (Self::Compat, true) => "Compat only",
            (Self::Language, true) => "Language pack only",
            _ => self.label(),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "全部還原 (Stats+Compat+Language)",
            Self::Stats => "僅數值 (Stats)",
            Self::Compat => "僅相容性 (Compat)",
            Self::Language => "僅語言包 (Language)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub severity: Severity,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Create,
    Modify,
    Delete,
    Unchanged,
}

#[derive(Clone, Debug)]
pub struct PlanEntry {
    pub path: PathBuf,
    pub kind: ChangeKind,
    pub old_len: Option<usize>,
    pub new_len: Option<usize>,
    /// Differing byte positions when old/new have equal length.
    pub changed_bytes: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreviewKind {
    Apply,
    RestoreOriginal(RestoreScope),
    RestoreSnapshot(String),
}

impl PreviewKind {
    fn label(&self) -> String {
        match self {
            Self::Apply => "apply".into(),
            Self::RestoreOriginal(s) => format!("restore-original:{}", s.key()),
            Self::RestoreSnapshot(id) => format!("undo:{id}"),
        }
    }
}

/// A reviewed plan. `commit_preview` writes exactly this plan, after re-checking that the game files still match.
#[derive(Clone, Debug)]
pub struct ModifierPreview {
    pub kind: PreviewKind,
    pub game: PathBuf,
    pub entries: Vec<PlanEntry>,
    /// File count of the unpruned plan (equals the CLI `plannedFiles`).
    pub planned_files: usize,
    pub enabled_features: Vec<String>,
    pub feature_changes: Vec<String>,
    pub blockers: Vec<String>,
    plan: PatchFilePlan,
    fingerprint: Vec<(PathBuf, Option<String>)>,
}

fn disk_sha(path: &Path) -> Option<String> {
    fs::read(path).ok().map(|b| sha256(&b))
}

impl ModifierPreview {
    fn build(
        kind: PreviewKind,
        game: PathBuf,
        full: PatchFilePlan,
        enabled_features: Vec<String>,
        feature_changes: Vec<String>,
        blockers: Vec<String>,
    ) -> Self {
        let planned_files = full.files.len();
        let fingerprint = full.files.keys().chain(full.deletes.iter()).map(|p| (p.clone(), disk_sha(p))).collect();
        let mut plan = PatchFilePlan::default();
        let mut entries = Vec::new();
        for (path, new) in &full.files {
            let old = fs::read(path).ok();
            let (kind, changed) = match &old {
                None => (ChangeKind::Create, None),
                Some(o) if o == new => (ChangeKind::Unchanged, None),
                Some(o) => (ChangeKind::Modify, (o.len() == new.len()).then(|| o.iter().zip(new).filter(|(a, b)| a != b).count())),
            };
            if kind != ChangeKind::Unchanged {
                plan.files.insert(path.clone(), new.clone());
            }
            entries.push(PlanEntry { path: path.clone(), kind, old_len: old.as_ref().map(Vec::len), new_len: Some(new.len()), changed_bytes: changed });
        }
        for path in &full.deletes {
            if let Ok(old) = fs::read(path) {
                plan.deletes.push(path.clone());
                entries.push(PlanEntry { path: path.clone(), kind: ChangeKind::Delete, old_len: Some(old.len()), new_len: None, changed_bytes: None });
            }
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Self { kind, game, entries, planned_files, enabled_features, feature_changes, blockers, plan, fingerprint }
    }
    pub fn changed_count(&self) -> usize {
        self.plan.files.len() + self.plan.deletes.len()
    }
    pub fn can_commit(&self) -> bool {
        self.blockers.is_empty() && self.changed_count() > 0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitReport {
    pub snapshot_id: String,
    pub written: usize,
    pub deleted: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotInfo {
    pub id: String,
    pub label: String,
    pub game: PathBuf,
    pub created_unix: u64,
    pub files: usize,
    pub state: String,
}

// ------------------------------------------------------------------ snapshots

mod snapshot {
    use super::*;

    pub struct Entry {
        pub rel: PathBuf,
        pub pre_file: Option<String>,
        pub pre_sha: Option<String>,
        pub post_sha: Option<String>,
    }
    pub struct Manifest {
        pub info: SnapshotInfo,
        pub entries: Vec<Entry>,
    }

    pub fn valid_id(id: &str) -> bool {
        !id.is_empty() && id.len() <= 64 && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
    }
    fn safe_rel(rel: &str) -> Option<PathBuf> {
        let p = PathBuf::from(rel);
        (!rel.is_empty() && p.components().all(|c| matches!(c, Component::Normal(_)))).then_some(p)
    }
    pub fn now() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
    }
    pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension(format!("tmp-{}", &Uuid::new_v4().simple().to_string()[..8]));
        let r = (|| {
            let mut f = fs::File::create(&tmp)?;
            io::Write::write_all(&mut f, bytes)?;
            f.sync_all()?;
            fs::rename(&tmp, path)
        })();
        if r.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        r
    }

    fn manifest_json(m: &Manifest) -> Value {
        json!({
            "version": 1, "id": m.info.id, "label": m.info.label, "game": m.info.game.to_string_lossy(),
            "createdUnix": m.info.created_unix, "state": m.info.state,
            "entries": m.entries.iter().map(|e| json!({
                "path": e.rel.to_string_lossy().replace('\\', "/"), "pre": e.pre_file, "preSha": e.pre_sha, "postSha": e.post_sha,
            })).collect::<Vec<_>>(),
        })
    }
    fn save(root: &Path, m: &Manifest) -> io::Result<()> {
        write_atomic(&root.join(&m.info.id).join("manifest.json"), serde_json::to_string_pretty(&manifest_json(m)).unwrap().as_bytes())
    }

    /// Stores the current bytes of every path touched by `plan`. Written before the commit, state `pending`.
    pub fn create(root: &Path, game: &Path, label: &str, plan: &PatchFilePlan) -> io::Result<Manifest> {
        let id = format!("{:010}-{}", now(), &Uuid::new_v4().simple().to_string()[..8]);
        let dir = root.join(&id);
        fs::create_dir_all(dir.join("files"))?;
        let mut entries = Vec::new();
        for (i, path) in plan.files.keys().chain(plan.deletes.iter()).enumerate() {
            let rel = path
                .strip_prefix(game)
                .ok()
                .map(Path::to_path_buf)
                .filter(|r| safe_rel(&r.to_string_lossy()).is_some())
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("路徑不在遊戲目錄內: {}", path.display())))?;
            let pre = fs::read(path).ok();
            let pre_file = match &pre {
                Some(bytes) => {
                    let name = format!("files/{i}.bin");
                    write_atomic(&dir.join(&name), bytes)?;
                    Some(name)
                }
                None => None,
            };
            entries.push(Entry {
                rel,
                pre_file,
                pre_sha: pre.as_ref().map(|b| sha256(b)),
                post_sha: plan.files.get(path).map(|b| sha256(b)),
            });
        }
        let info = SnapshotInfo { id, label: label.into(), game: game.into(), created_unix: now(), files: entries.len(), state: "pending".into() };
        let m = Manifest { info, entries };
        save(root, &m)?;
        Ok(m)
    }
    pub fn set_state(root: &Path, id: &str, state: &str) -> io::Result<()> {
        let mut m = read(root, id)?;
        m.info.state = state.into();
        save(root, &m)
    }
    pub fn remove(root: &Path, id: &str) {
        if valid_id(id) {
            let _ = fs::remove_dir_all(root.join(id));
        }
    }

    pub fn read(root: &Path, id: &str) -> io::Result<Manifest> {
        let bad = |m: &str| io::Error::new(io::ErrorKind::InvalidData, format!("快照 {id} 無效: {m}"));
        if !valid_id(id) {
            return Err(bad("id"));
        }
        let v: Value = serde_json::from_slice(&fs::read(root.join(id).join("manifest.json"))?).map_err(|e| bad(&e.to_string()))?;
        let text = |k: &str| v[k].as_str().map(str::to_owned);
        if text("id").as_deref() != Some(id) || v["version"] != 1 {
            return Err(bad("id/version mismatch"));
        }
        let mut entries = Vec::new();
        for e in v["entries"].as_array().ok_or_else(|| bad("entries"))? {
            let rel = e["path"].as_str().and_then(safe_rel).ok_or_else(|| bad("unsafe entry path"))?;
            let pre_file = e["pre"].as_str().map(str::to_owned);
            if pre_file.as_deref().is_some_and(|p| !p.starts_with("files/") || p.contains("..") || p.contains('\\')) {
                return Err(bad("unsafe blob path"));
            }
            entries.push(Entry { rel, pre_file, pre_sha: e["preSha"].as_str().map(str::to_owned), post_sha: e["postSha"].as_str().map(str::to_owned) });
        }
        let info = SnapshotInfo {
            id: id.into(),
            label: text("label").unwrap_or_default(),
            game: PathBuf::from(text("game").ok_or_else(|| bad("game"))?),
            created_unix: v["createdUnix"].as_u64().unwrap_or(0),
            files: entries.len(),
            state: text("state").unwrap_or_default(),
        };
        Ok(Manifest { info, entries })
    }

    pub fn list(root: &Path) -> Vec<SnapshotInfo> {
        let mut out: Vec<_> = fs::read_dir(root)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| read(root, &e.file_name().to_string_lossy()).ok())
            .map(|m| m.info)
            .collect();
        out.sort_by(|a, b| b.id.cmp(&a.id));
        out
    }

    /// Plan that puts every file back to its pre-change bytes. A file whose current content is neither the
    /// recorded pre nor post state was edited externally and blocks the undo.
    pub fn undo_plan(root: &Path, m: &Manifest) -> (PatchFilePlan, Vec<String>) {
        let (game, mut plan, mut blockers) = (&m.info.game, PatchFilePlan::default(), Vec::new());
        for e in &m.entries {
            let path = game.join(&e.rel);
            let cur = disk_sha(&path);
            if cur == e.pre_sha {
                continue;
            }
            if cur != e.post_sha {
                blockers.push(format!("{} 在套用後被外部修改，拒絕覆寫", e.rel.display()));
                continue;
            }
            match &e.pre_file {
                Some(blob) => match fs::read(root.join(&m.info.id).join(blob)) {
                    Ok(bytes) if Some(sha256(&bytes)) == e.pre_sha => {
                        plan.files.insert(path, bytes);
                    }
                    Ok(_) => blockers.push(format!("快照備份已損毀 (hash 不符): {}", e.rel.display())),
                    Err(err) => blockers.push(format!("無法讀取快照備份 {}: {err}", e.rel.display())),
                },
                None => plan.deletes.push(path),
            }
        }
        (plan, blockers)
    }
}

// ---------------------------------------------------------------------- model

pub fn default_state_dir() -> PathBuf {
    ["LOCALAPPDATA", "APPDATA"]
        .iter()
        .find_map(|k| std::env::var_os(k).filter(|v| !v.is_empty()))
        .map_or_else(std::env::temp_dir, PathBuf::from)
        .join("AgainstRomeEditor")
        .join("modifier")
}

fn valid_profile_name(name: &str) -> bool {
    const RESERVED: [&str; 8] = ["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "LPT1", "LPT2"];
    (1..=48).contains(&name.chars().count())
        && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        && !RESERVED.iter().any(|r| r.eq_ignore_ascii_case(name))
}

const EDITABLE_STATS: [(usize, &str); 6] = [(0, "HP"), (1, "damage"), (2, "VW"), (3, "AW"), (5, "sight"), (6, "reload")];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilePickerKind {
    ImportPreset,
    ExportPreset,
}

pub type PresetPickerRunner = Box<dyn Fn(FilePickerKind) -> Option<PathBuf> + Send + Sync>;

pub fn native_preset_dialog(kind: FilePickerKind, is_en: bool) -> Option<PathBuf> {
    let filter_name = if is_en { "Troop Preset Files (*.artroop)" } else { "兵種自訂屬性檔 (*.artroop)" };
    match kind {
        FilePickerKind::ImportPreset => {
            let title = if is_en { "Import Custom Troop Stats" } else { "匯入自訂兵種屬性" };
            rfd::FileDialog::new()
                .set_title(title)
                .add_filter(filter_name, &["artroop"])
                .pick_file()
        }
        FilePickerKind::ExportPreset => {
            let title = if is_en { "Export Custom Troop Stats" } else { "匯出自訂兵種屬性" };
            rfd::FileDialog::new()
                .set_title(title)
                .add_filter(filter_name, &["artroop"])
                .set_file_name("custom_troop_preset.artroop")
                .save_file()
        }
    }
}

/// Local draft; opening, typing and cancelling never alter the profile or game files.
#[derive(Clone, Debug)]
pub struct UnitStatsDraft {
    source_profile: String,
    source_game: PathBuf,
    pub rows: Vec<UnitStatsRow>,
    pub preset_path: String,
    pub loaded_file_name: String,
    pub preset_status: Option<(String, bool)>,
}
#[derive(Clone, Debug)]
pub struct UnitStatsRow {
    pub key: String,
    pub original: [f64; 9],
    pub balanced: [f64; 9],
    pub cells: [String; 6],
}
impl UnitStatsDraft {
    pub fn load(model: &ModifierModel) -> ModResult<Self> {
        let game = model.game_dir()?;
        let backup = model.load_backup(&game, false)?;
        let available = backup.get_backup_unit_rows().map_err(io_err)?;
        let mut rows = Vec::new();
        for key in TroopConfig::sorted_units() {
            // Native getters return zeroes for missing rows; never accept that as a baseline.
            if !available.keys().any(|k| k.eq_ignore_ascii_case(key)) {
                return Err(format!("原版備份缺少兵種 {key}，無法安全編輯。"));
            }
            let original = backup.get_original_stats(key).map_err(io_err)?;
            if !original.iter().all(|v| v.is_finite()) || original[0] <= 0.0 {
                return Err(format!("{key} 原版數值無效。"));
            }
            let balanced = backup.get_default_balanced_stats(key).map_err(io_err)?;
            let custom = model.profile.custom_unit_stats.as_ref().and_then(|m| m.iter().find(|(k, _)| k.eq_ignore_ascii_case(key))).map(|(_, v)| v);
            let priest = TroopConfig::get_metadata(key).is_some_and(|m| m.unit_type == "priest");
            let cells = std::array::from_fn(|i| {
                let index = EDITABLE_STATS[i].0;
                let value = if priest && index == 5 { original[5] }
                    else { custom.and_then(|v| v.get(index)).copied().unwrap_or(balanced[index]) };
                value.to_string()
            });
            rows.push(UnitStatsRow { key: key.into(), original, balanced, cells });
        }
        Ok(Self {
            source_profile: model.profile_json_text(),
            source_game: game,
            rows,
            preset_path: String::new(),
            loaded_file_name: String::new(),
            preset_status: None,
        })
    }
    pub fn validate_inputs(&self) -> ModResult<()> {
        for row in &self.rows {
            row.values()?;
        }
        Ok(())
    }
    pub fn import_preset_from_path(&mut self, path: &Path) -> ModResult<String> {
        let text = fs::read_to_string(path).map_err(|e| format!("匯入 {} 失敗：{e}；草稿未變更。", path.display()))?;
        let (parsed, skipped) = against_rome_modifier::cli::parse_preset(text.trim_start_matches('\u{feff}'), |key| {
            self.rows.iter().find(|r| r.key.eq_ignore_ascii_case(key)).map(|r| r.balanced.to_vec()).unwrap_or_else(|| vec![0.0; 9])
        });
        let mut rows = self.rows.clone();
        let mut count = 0;
        for row in &mut rows {
            let Some((_, values)) = parsed.iter().find(|(k, _)| row.key.eq_ignore_ascii_case(k)) else { continue };
            count += 1;
            for (i, &(index, _)) in EDITABLE_STATS.iter().enumerate() {
                let priest_sight = index == 5 && TroopConfig::get_metadata(&row.key).is_some_and(|m| m.unit_type == "priest");
                row.cells[i] = if priest_sight { row.original[5] } else { values[index] }.to_string();
            }
            row.values()?;
        }
        if count == 0 { return Err(format!("沒有可匯入的已知兵種；略過 {skipped} 行格式錯誤。草稿未變更。")); }
        self.rows = rows;
        self.preset_path = path.to_string_lossy().into_owned();
        self.loaded_file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        Ok(format!("已匯入 {count} 個兵種至草稿；略過 {skipped} 行格式錯誤。尚未更新設定檔。"))
    }
    pub fn import_preset(&mut self) -> ModResult<String> {
        if self.preset_path.trim().is_empty() { return Err("請輸入 .artroop 檔案路徑。".into()); }
        let path = PathBuf::from(self.preset_path.trim());
        self.import_preset_from_path(&path)
    }
    pub fn export_preset_to_path(&mut self, path: &Path) -> ModResult<String> {
        if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("artroop")) {
            return Err("匯出路徑必須使用 .artroop 副檔名。".into());
        }
        // Resolve links and '..' before writing; exporting must never overwrite game data.
        let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        let parent = fs::canonicalize(parent).map_err(|e| format!("匯出目錄無法開啟：{e}"))?;
        let target = parent.join(path.file_name().ok_or("無效的匯出檔名。")?);
        let resolved = if target.exists() { fs::canonicalize(&target).map_err(io_err)? } else { target.clone() };
        let game = fs::canonicalize(&self.source_game).map_err(io_err)?;
        if resolved.starts_with(&game) || parent.starts_with(&game) {
            return Err("請選擇遊戲目錄以外的匯出路徑；未寫入任何檔案。".into());
        }
        let stats = self.rows.iter().map(|r| Ok((r.key.clone(), r.values()?))).collect::<ModResult<Vec<_>>>()?;
        let text = against_rome_modifier::unit_stats::write_preset(&stats, &preset_timestamp());
        snapshot::write_atomic(&target, text.as_bytes()).map_err(|e| format!("匯出 {} 失敗：{e}", target.display()))?;
        self.preset_path = target.to_string_lossy().into_owned();
        self.loaded_file_name = target.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        Ok(format!("已匯出草稿至 {}；設定檔未變更。", target.display()))
    }
    pub fn export_preset(&mut self) -> ModResult<String> {
        if self.preset_path.trim().is_empty() { return Err("請輸入 .artroop 檔案路徑。".into()); }
        let path = PathBuf::from(self.preset_path.trim());
        self.export_preset_to_path(&path)
    }
    fn apply(&self, model: &mut ModifierModel) -> ModResult<()> {
        if self.source_profile != model.profile_json_text() || self.source_game != model.game_dir()? {
            return Err("設定或遊戲目錄已變更，請重新開啟兵種編輯器。".into());
        }
        let fresh = Self::load(model)?;
        if self.rows.len() != fresh.rows.len() || self.rows.iter().zip(&fresh.rows).any(|(a, b)| a.key != b.key || a.original != b.original) {
            return Err("原版備份已變更，請重新開啟兵種編輯器。".into());
        }
        let mut stats = model.profile.custom_unit_stats.clone().unwrap_or_default();
        for row in &self.rows {
            let values = row.values()?;
            stats.retain(|k, _| !k.eq_ignore_ascii_case(&row.key));
            stats.insert(row.key.clone(), values.to_vec());
        }
        model.profile.custom_unit_stats = Some(stats);
        model.touch();
        Ok(())
    }
}

impl UnitStatsRow {
    fn values(&self) -> ModResult<[f64; 9]> {
        let mut values = self.original;
        for (cell, &(index, label)) in self.cells.iter().zip(&EDITABLE_STATS) {
            if index == 5 && TroopConfig::get_metadata(&self.key).is_some_and(|m| m.unit_type == "priest") { continue; }
            let value: f64 = cell.trim().parse().map_err(|_| format!("{} {label}: 請輸入有效數字；草稿未套用。", self.key))?;
            if !value.is_finite() || value < 0.0 || (index == 0 && value == 0.0) {
                return Err(format!("{} {label}: HP 必須大於 0，其餘數值不得小於 0，且必須有限；草稿未套用。", self.key));
            }
            values[index] = value;
        }
        Ok(values)
    }
}

/// Draws the unit icon cell (24x24); an empty cell when the icon is unavailable. Textures are cached per unit.
fn show_unit_icon(ui: &mut egui::Ui, ctx: &egui::Context, icons: &Option<UnitIcons>, textures: &mut std::collections::HashMap<String, egui::TextureHandle>, key: &str) {
    let size = egui::vec2(24.0, 24.0);
    let Some(icon) = icons.as_ref().and_then(|i| i.get(key)) else {
        ui.allocate_space(size);
        return;
    };
    let tex = textures.entry(key.to_ascii_lowercase()).or_insert_with(|| {
        let image = egui::ColorImage::from_rgba_unmultiplied([icon.width, icon.height], &icon.rgba);
        ctx.load_texture(format!("unit_icon_{key}"), image, egui::TextureOptions::LINEAR)
    });
    ui.image((tex.id(), size));
}

/// Gregorian UTC timestamp, without adding a date/time dependency.
fn preset_timestamp() -> String {
    let seconds = snapshot::now();
    let z = (seconds / 86400) as i64 + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}", seconds / 3600 % 24, seconds / 60 % 60, seconds % 60)
}

pub struct ModifierModel {
    pub game_path: String,
    pub profile: PatchProfile,
    pub restore_scope: RestoreScope,
    pub preserve_custom_maps: bool,
    pub english: bool,
    /// Directory holding persisted state and profiles; `None` keeps everything in memory.
    pub state_dir: Option<PathBuf>,
    pub snapshot_dir: Option<PathBuf>,
    /// Directory searched for `Backup.zip` (the CLI uses the executable's directory).
    pub backup_base: PathBuf,
    pub preview: Option<ModifierPreview>,
    pub log: Vec<String>,
    /// Preset label shown next to the troop stats (C# `presetFileName`); empty = default.
    pub preset_label: String,
    /// App-wide preferences (promoted experimental features); persisted by `Loc` when it has a path.
    pub app_loc: Loc,
    settings_dirty: bool,
}

impl Default for ModifierModel {
    fn default() -> Self {
        let mut profile = PatchProfile::new();
        profile.set_int(SPEED_ID, 1);
        profile.set_int(QUOTA_ID, 1);
        Self {
            game_path: String::new(),
            profile,
            restore_scope: RestoreScope::All,
            preserve_custom_maps: true,
            english: false,
            state_dir: None,
            snapshot_dir: None,
            backup_base: std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_default(),
            preview: None,
            log: Vec::new(),
            preset_label: String::new(),
            app_loc: Loc::default(),
            settings_dirty: false,
        }
    }
}

impl ModifierModel {
    pub fn with_state_dir(dir: PathBuf) -> Self {
        let app_loc = Loc::load(dir.join(APP_SETTINGS_FILE), "zh-TW");
        Self { state_dir: Some(dir), app_loc, ..Self::default() }
    }
    pub fn loc(&self) -> Loc {
        Loc::new(if self.english { Language::English } else { Language::TraditionalChinese })
    }
    pub fn snapshot_root(&self) -> Option<PathBuf> {
        self.snapshot_dir.clone().or_else(|| self.state_dir.as_ref().map(|d| d.join("snapshots")))
    }
    fn touch(&mut self) {
        self.preview = None;
        self.settings_dirty = true;
    }
    pub fn take_settings_dirty(&mut self) -> bool {
        std::mem::take(&mut self.settings_dirty)
    }
    fn note(&mut self, msg: impl Into<String>) {
        self.log.push(msg.into());
        if self.log.len() > 200 {
            self.log.drain(..self.log.len() - 200);
        }
    }

    // ---- Start Game (C# BtnStartGame_Click)
    /// Launches `<game>/Against_Rome.exe` through `launch`. Errors use the C# messages and are also logged.
    pub fn start_game_with(&mut self, launch: &dyn Fn(&Path, &Path) -> io::Result<()>) -> ModResult<PathBuf> {
        let loc = self.loc();
        let game = PathBuf::from(self.game_path.trim());
        if self.game_path.trim().is_empty() || !game.is_dir() {
            return Err(loc.get("MsgSelectGameDir").to_owned());
        }
        let exe = game.join("Against_Rome.exe");
        if !exe.is_file() {
            return Err(loc.get("MsgExeNotFound").to_owned());
        }
        match launch(&exe, &game) {
            Ok(()) => {
                self.note(loc.get("LogGameStarted"));
                Ok(exe)
            }
            Err(e) => {
                let msg = format!("{}{e}", loc.get("MsgLaunchFailed"));
                self.note(msg.clone());
                Err(msg)
            }
        }
    }

    // ---- Experimental grouping (C# InitializeExperimentalFeatures / UpdateExperimentalLayout)
    pub fn is_promoted(&self, id: &str) -> bool {
        self.app_loc.is_promoted(id)
    }
    /// Moves an experimental feature to the main list (promote) or back (demote). Returns the new promoted state.
    pub fn toggle_promotion(&mut self, id: &str) -> ModResult<bool> {
        let def = FeatureRegistry::get(id).filter(|d| is_experimental_feature(d.id)).ok_or_else(|| format!("'{id}' 不是實驗性功能。"))?;
        if self.app_loc.is_promoted(def.id) {
            self.app_loc.demote_feature(def.id);
            self.note(format!("已將功能 {} 移回實驗性頁面。", def.id));
            Ok(false)
        } else {
            self.app_loc.promote_feature(def.id);
            self.note(format!("已將功能 {} 移至主控制台。", def.id));
            Ok(true)
        }
    }

    // ---- Balanced Base Stats template (C# CbTroopTemplate_SelectedIndexChanged)
    /// Replaces every custom unit stat with the built-in balanced values (speed/range/radius, and
    /// priest sight, kept at original like `NormalizeIndependentFields`). Caller must have confirmed.
    /// Returns the number of units written. Nothing is changed on error.
    pub fn apply_balanced_template(&mut self) -> ModResult<usize> {
        let game = self.game_dir()?;
        let backup = self.load_backup(&game, false)?;
        let mut stats = std::collections::BTreeMap::new();
        for key in TroopConfig::sorted_units() {
            if TroopConfig::get_metadata(key).is_none() {
                continue;
            }
            let balanced = backup.get_default_balanced_stats(key).map_err(io_err)?;
            let normalized = against_rome_modifier::unit_stats::normalize_independent_fields(&backup, key, &balanced).map_err(io_err)?;
            stats.insert(key.to_owned(), normalized.to_vec());
        }
        let n = stats.len();
        self.profile.custom_unit_stats = Some(stats.into_iter().collect());
        self.preset_label = self.loc().get("TroopTemplateBalanced").to_owned();
        self.touch();
        self.note(if self.english { "Applied built-in balanced troop stats template." } else { "已套用修改器內建平衡兵種屬性範本。" });
        Ok(n)
    }

    // ---- unit icons (C# LoadIcons)
    /// Reads `icon.ini` from the original backup and TGAs from `gui.dat`; logs like C#. Never writes.
    pub fn load_unit_icons(&mut self) -> UnitIcons {
        let game = Some(PathBuf::from(self.game_path.trim())).filter(|p| !self.game_path.trim().is_empty() && p.is_dir());
        let ini = game
            .as_ref()
            .filter(|g| g.join("gui.dat").is_file())
            .and_then(|g| self.load_backup(g, false).ok())
            .and_then(|b| b.get_backup_bytes(ICON_INI_KEY).ok().map(<[u8]>::to_vec));
        let icons = icons::load_unit_icons(game.as_deref(), ini.as_deref());
        let loc = self.loc();
        for entry in &icons.log {
            self.note(entry.format(|k| loc.get(k).to_owned()));
        }
        icons
    }

    // ---- selection
    pub fn set_game_path(&mut self, path: impl Into<String>) {
        self.game_path = path.into();
        self.touch();
    }
    pub fn feature_enabled(&self, id: &str) -> bool {
        self.profile.get_bool(id)
    }
    pub fn set_feature(&mut self, id: &str, on: bool) -> ModResult<()> {
        let def = FeatureRegistry::get(id).ok_or_else(|| format!("未知的修改功能 ID: '{id}'"))?;
        if def.control_kind != FeatureControlKind::Toggle {
            return Err(format!("'{id}' 不是開關型功能，請使用專用控制項"));
        }
        self.profile.set_bool(def.id, on);
        self.touch();
        Ok(())
    }
    pub fn game_speed(&self) -> i32 {
        self.profile.get_int(SPEED_ID).max(1)
    }
    pub fn quota(&self) -> i32 {
        self.profile.get_int(QUOTA_ID).max(1)
    }
    pub fn set_game_speed(&mut self, n: i32) -> ModResult<()> {
        if !(1..=10).contains(&n) {
            return Err("遊戲速度倍率必須介於 1 到 10 之間。".into());
        }
        self.profile.set_int(SPEED_ID, n);
        self.touch();
        Ok(())
    }
    pub fn set_quota(&mut self, n: i32) -> ModResult<()> {
        if !(1..=10).contains(&n) {
            return Err("駐軍配額倍率必須介於 1 到 10 之間。".into());
        }
        self.profile.set_int(QUOTA_ID, n);
        self.touch();
        Ok(())
    }
    /// Enables every toggle outside `ENABLE_ALL_EXCLUDED`, like C# `BtnEnableAll_Click` / `apply --all`.
    /// Speed/quota multipliers and custom unit stats are left untouched. Returns the toggle count.
    pub fn enable_all(&mut self) -> usize {
        let mut n = 0;
        for f in FeatureRegistry::toggle_features().filter(|f| !ENABLE_ALL_EXCLUDED.contains(&f.id)) {
            self.profile.set_bool(f.id, true);
            n += 1;
        }
        self.touch();
        self.note("已開啟所有功能。");
        n
    }
    /// Disables every toggle and resets speed/quota, like `apply --disable-all`.
    pub fn clear_selection(&mut self) {
        let units = self.profile.custom_unit_stats.take();
        self.profile = Self::default().profile;
        self.profile.custom_unit_stats = units;
        self.touch();
    }
    pub fn enabled_ids(&self) -> Vec<String> {
        FeatureRegistry::toggle_features().filter(|f| self.profile.get_bool(f.id)).map(|f| f.id.to_string()).collect()
    }
    pub fn custom_unit_count(&self) -> usize {
        self.profile.custom_unit_stats.as_ref().map_or(0, |m| m.len())
    }

    // ---- validation
    pub fn game_dir(&self) -> ModResult<PathBuf> {
        let g = self.game_path.trim();
        if g.is_empty() {
            return Err("尚未指定遊戲目錄。".into());
        }
        let p = PathBuf::from(g);
        if !p.is_dir() {
            return Err(format!("指定的遊戲目錄不存在: {}", p.display()));
        }
        if !p.join("Against_Rome.exe").is_file() {
            return Err("遊戲目錄下未找到主執行檔 Against_Rome.exe".into());
        }
        fs::canonicalize(&p).map_err(io_err)
    }
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        let mut push = |severity, message: String| out.push(Issue { severity, message });
        if let Err(e) = self.game_dir() {
            push(Severity::Error, e);
        }
        for (label, id) in [("遊戲速度", SPEED_ID), ("駐軍配額", QUOTA_ID)] {
            let n = self.profile.get_int(id);
            if !(1..=10).contains(&n) {
                push(Severity::Error, format!("{label}倍率 {n} 超出 1..10 範圍。"));
            }
        }
        for f in FeatureRegistry::toggle_features() {
            if self.profile.get_bool(f.id) && ENABLE_ALL_EXCLUDED.contains(&f.id) {
                push(Severity::Warning, format!("{} 屬於高風險/需明確啟用的功能，請確認確實需要。", f.id));
            }
        }
        if self.snapshot_root().is_none() {
            push(Severity::Error, "未設定快照目錄，無法在寫入前保存還原點。".into());
        }
        out
    }
    fn require_valid(&self) -> ModResult<()> {
        let errors: Vec<_> = self.validate().into_iter().filter(|i| i.severity == Severity::Error).map(|i| i.message).collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("\n"))
        }
    }

    // ---- game detection / current state
    pub fn detect_game(&mut self) -> ModResult<PathBuf> {
        let found = GameDirectoryLocator::resolve_initial_game_path(None);
        if found.as_os_str().is_empty() || !found.join("Against_Rome.exe").is_file() {
            return Err("找不到遊戲安裝目錄，請手動輸入。".into());
        }
        self.set_game_path(found.to_string_lossy().into_owned());
        Ok(found)
    }
    /// Port of C# `PatchEngine.RunStartupSafeMigrations` at form start: only the targeted repairs, never a full apply.
    /// Plans against a read-only backup, keeps only files that differ from disk (so a second run is a no-op),
    /// and refuses any path outside the validated game directory. Returns the number of files written.
    pub fn run_startup_migrations(&mut self) -> ModResult<usize> {
        let game = self.game_dir()?;
        let b = self.load_backup(&game, false)?;
        let full = PatchEngine::run_startup_safe_migrations(&game, &b).map_err(io_err)?;
        if !full.deletes.is_empty() {
            return Err("啟動遷移不應刪除檔案，已中止。".into());
        }
        let mut plan = PatchFilePlan::default();
        for (path, bytes) in full.files {
            if !path.starts_with(&game) || path.components().any(|c| matches!(c, Component::ParentDir)) {
                return Err(format!("啟動遷移拒絕遊戲目錄外的路徑: {}", path.display()));
            }
            if fs::read(&path).ok().as_deref() != Some(bytes.as_slice()) {
                plan.files.insert(path, bytes);
            }
        }
        let n = plan.files.len();
        if n > 0 {
            commit_plan(&plan).map_err(|e| format!("寫入失敗: {e}"))?;
            self.note(format!("啟動安全遷移已更新 {n} 個檔案"));
        }
        Ok(n)
    }
    fn load_backup(&self, game: &Path, allow_write: bool) -> ModResult<BackupManager> {
        let mut b = BackupManager::new();
        b.load_backup_zip_to_memory_opt(game, None, &self.backup_base, allow_write).map_err(io_err)?;
        if !b.ensure_backup_loaded_for_game_path_opt(game, allow_write).map_err(io_err)? {
            return Err("無法載入或建立遊戲備份基準。請先確認遊戲目錄包含原版檔案。".into());
        }
        Ok(b)
    }
    /// Replaces the selection with what is currently installed in the game directory (read-only).
    pub fn load_current_state(&mut self) -> ModResult<usize> {
        let game = self.game_dir()?;
        let b = self.load_backup(&game, false)?;
        let mut p = PatchEngine::detect_current_patch_state(&game, &b);
        let units = self.profile.custom_unit_stats.clone();
        if p.custom_unit_stats.is_none() {
            p.custom_unit_stats = units;
        }
        Self::normalize_ranges(&mut p);
        self.profile = p;
        self.touch();
        let n = self.enabled_ids().len();
        self.note(format!("已載入遊戲目前狀態: {n} 項功能啟用"));
        Ok(n)
    }
    fn normalize_ranges(p: &mut PatchProfile) {
        for id in [SPEED_ID, QUOTA_ID] {
            if p.get_int(id) < 1 {
                p.set_int(id, 1);
            }
        }
    }

    // ---- profiles
    pub fn profiles_dir(&self) -> Option<PathBuf> {
        self.state_dir.as_ref().map(|d| d.join("profiles"))
    }
    pub fn profile_json_text(&self) -> String {
        serde_json::to_string_pretty(&profile_to_json(&self.profile)).unwrap()
    }
    pub fn list_profiles(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .profiles_dir()
            .and_then(|d| fs::read_dir(d).ok())
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.path().file_stem().filter(|_| e.path().extension().is_some_and(|x| x == "json")).map(|s| s.to_string_lossy().into_owned()))
            .filter(|n| valid_profile_name(n))
            .collect();
        v.sort();
        v
    }
    pub fn save_profile(&mut self, name: &str) -> ModResult<PathBuf> {
        if !valid_profile_name(name) {
            return Err("設定檔名稱只能包含文字、數字、底線與連字號 (1-48 字)。".into());
        }
        let path = self.profiles_dir().ok_or("未設定狀態目錄，無法保存設定檔。")?.join(format!("{name}.json"));
        self.export_profile(&path)?;
        Ok(path)
    }
    pub fn load_profile(&mut self, name: &str) -> ModResult<()> {
        if !valid_profile_name(name) {
            return Err("設定檔名稱無效。".into());
        }
        let path = self.profiles_dir().ok_or("未設定狀態目錄。")?.join(format!("{name}.json"));
        self.import_profile(&path)
    }
    /// CLI-compatible profile JSON (`apply --profile`).
    pub fn export_profile(&mut self, path: &Path) -> ModResult<()> {
        snapshot::write_atomic(path, self.profile_json_text().as_bytes()).map_err(io_err)?;
        self.note(format!("已匯出設定檔 {}", path.display()));
        Ok(())
    }
    pub fn import_profile(&mut self, path: &Path) -> ModResult<()> {
        let bytes = fs::read(path).map_err(|e| format!("無法讀取設定檔 {}: {e}", path.display()))?;
        let p = Self::parse_profile(&bytes)?;
        self.profile = p;
        self.touch();
        self.note(format!("已匯入設定檔 {}", path.display()));
        Ok(())
    }
    fn parse_profile(bytes: &[u8]) -> ModResult<PatchProfile> {
        let v: Value = serde_json::from_slice(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(bytes)).map_err(|e| format!("設定檔 JSON 無效: {e}"))?;
        if let Some(features) = v.as_object().and_then(|o| o.iter().find(|(k, _)| k.eq_ignore_ascii_case("Features"))).and_then(|(_, f)| f.as_object()) {
            let unknown: Vec<_> = features.keys().filter(|k| FeatureRegistry::get(k).is_none()).cloned().collect();
            if !unknown.is_empty() {
                return Err(format!("設定檔包含未知的功能 ID: {}", unknown.join(", ")));
            }
        }
        let mut p = profile_from_json(&v).map_err(io_err)?;
        Self::normalize_ranges(&mut p);
        for (label, id) in [("GameSpeed", SPEED_ID), ("VillageGarrisonQuotaMultiplier", QUOTA_ID)] {
            if !(1..=10).contains(&p.get_int(id)) {
                return Err(format!("設定檔 {label} 超出 1..10 範圍。"));
            }
        }
        Ok(p)
    }

    // ---- persistence of the selection
    pub fn settings_json(&self) -> Value {
        json!({
            "gamePath": self.game_path, "snapshotDir": self.snapshot_dir.as_ref().map(|p| p.to_string_lossy().into_owned()),
            "english": self.english, "restoreScope": self.restore_scope.key(),
            "preserveCustomMaps": self.preserve_custom_maps, "profile": profile_to_json(&self.profile),
        })
    }
    pub fn apply_settings_json(&mut self, v: &Value) -> ModResult<()> {
        let profile = match v.get("profile") {
            Some(p) => Self::parse_profile(&serde_json::to_vec(p).unwrap())?,
            None => Self::default().profile,
        };
        self.game_path = v["gamePath"].as_str().unwrap_or_default().to_owned();
        self.snapshot_dir = v["snapshotDir"].as_str().filter(|s| !s.is_empty()).map(PathBuf::from);
        self.english = v["english"].as_bool().unwrap_or(false);
        self.restore_scope = RestoreScope::from_key(v["restoreScope"].as_str().unwrap_or(""));
        self.preserve_custom_maps = v["preserveCustomMaps"].as_bool().unwrap_or(true);
        self.profile = profile;
        self.preview = None;
        Ok(())
    }
    pub fn save_settings(&self) -> ModResult<()> {
        let dir = self.state_dir.as_ref().ok_or("未設定狀態目錄。")?;
        snapshot::write_atomic(&dir.join(STATE_FILE), serde_json::to_string_pretty(&self.settings_json()).unwrap().as_bytes()).map_err(io_err)
    }
    /// A missing state file is fine (first run); a corrupt one is reported and ignored.
    pub fn load_settings(&mut self) -> ModResult<bool> {
        let Some(dir) = &self.state_dir else { return Ok(false) };
        let path = dir.join(STATE_FILE);
        let Ok(bytes) = fs::read(&path) else { return Ok(false) };
        let v: Value = serde_json::from_slice(&bytes).map_err(|e| format!("{} 無效: {e}", path.display()))?;
        self.apply_settings_json(&v)?;
        Ok(true)
    }

    /// Applies the choices confirmed in the restore-options dialog. Only called on confirm; cancel never reaches the model.
    pub fn set_restore_options(&mut self, scope: RestoreScope, preserve_custom_maps: bool) {
        self.restore_scope = scope;
        self.preserve_custom_maps = preserve_custom_maps;
        self.touch();
    }
    /// After a committed restore: untoggle the restored categories like C# `ResetTogglesForCategory`.
    /// Compat also resets speed/quota, and the DgVoodoo / ArgmTrace toggles follow what is installed on disk.
    fn reset_after_restore(&mut self, scope: RestoreScope, game: &Path) {
        for &cat in scope.categories() {
            for f in FeatureRegistry::by_category(cat).filter(|f| f.control_kind == FeatureControlKind::Toggle) {
                self.profile.set_bool(f.id, false);
            }
            if cat == FeatureCategory::Compat {
                self.profile.set_int(SPEED_ID, 1);
                self.profile.set_int(QUOTA_ID, 1);
                self.profile.set_bool("DgVoodoo", PatchEngine::is_dgvoodoo_installed(game));
                self.profile.set_bool("ArgmTrace", PatchEngine::is_argm_trace_installed(game));
            }
        }
        self.touch();
    }

    // ---- preview
    fn require_game_and_valid(&self) -> ModResult<PathBuf> {
        self.require_valid()?;
        self.game_dir()
    }
    /// Plans the selected profile against the game directory. Never writes.
    pub fn preview_apply(&mut self) -> ModResult<()> {
        self.preview = None;
        let game = self.require_game_and_valid()?;
        let b = self.load_backup(&game, false)?;
        let current = PatchEngine::detect_current_patch_state(&game, &b);
        let mut plan = PatchEngine::compose(&game, &self.profile, &b).map_err(io_err)?;
        let install = plan_install(&game, &self.profile, &InstallResources::discover()).map_err(io_err)?;
        plan.files.extend(install.writes);
        plan.deletes.extend(install.deletes);
        let mut changes = Vec::new();
        for f in FeatureRegistry::toggle_features() {
            let (was, now) = (current.get_bool(f.id), self.profile.get_bool(f.id));
            if was != now {
                changes.push(format!("{}{}", if now { "+" } else { "-" }, f.id));
            }
        }
        for (label, id) in [("GameSpeed", SPEED_ID), ("Quota", QUOTA_ID)] {
            let (was, now) = (current.get_int(id).max(1), self.profile.get_int(id).max(1));
            if was != now {
                changes.push(format!("{label} {was}→{now}"));
            }
        }
        let preview = ModifierPreview::build(PreviewKind::Apply, game, plan, self.enabled_ids(), changes, Vec::new());
        self.note(format!("預覽套用: {} 個檔案將變更 (計畫 {} 個)", preview.changed_count(), preview.planned_files));
        self.preview = Some(preview);
        Ok(())
    }
    /// Plans a restore of the original files (from `Backup.zip` / `.bak`). Never writes.
    pub fn preview_restore_original(&mut self) -> ModResult<()> {
        self.preview = None;
        let game = self.game_dir()?;
        let b = self.load_backup(&game, false)?;
        let plan = match self.restore_scope {
            RestoreScope::All => PatchEngine::restore_original_files(&game, &b, self.preserve_custom_maps),
            RestoreScope::Stats => PatchEngine::restore_stats_only(&game, &b),
            RestoreScope::Compat => PatchEngine::restore_compat_only(&game, &b),
            RestoreScope::Language => PatchEngine::restore_language_only(&game),
        }
        .map_err(io_err)?;
        let preview = ModifierPreview::build(PreviewKind::RestoreOriginal(self.restore_scope), game, plan, Vec::new(), Vec::new(), Vec::new());
        self.note(format!("預覽還原原版 ({}): {} 個檔案將變更", self.restore_scope.key(), preview.changed_count()));
        self.preview = Some(preview);
        Ok(())
    }
    pub fn list_snapshots(&self) -> Vec<SnapshotInfo> {
        self.snapshot_root().map(|r| snapshot::list(&r)).unwrap_or_default()
    }
    /// Plans an undo of one committed operation. Externally edited files become blockers.
    pub fn preview_undo(&mut self, id: &str) -> ModResult<()> {
        self.preview = None;
        let root = self.snapshot_root().ok_or("未設定快照目錄。")?;
        let m = snapshot::read(&root, id).map_err(io_err)?;
        if !m.info.game.is_dir() {
            return Err(format!("快照對應的遊戲目錄已不存在: {}", m.info.game.display()));
        }
        let (plan, blockers) = snapshot::undo_plan(&root, &m);
        let preview = ModifierPreview::build(PreviewKind::RestoreSnapshot(id.into()), m.info.game.clone(), plan, Vec::new(), Vec::new(), blockers);
        self.note(format!("預覽復原快照 {id}: {} 個檔案將變更", preview.changed_count()));
        self.preview = Some(preview);
        Ok(())
    }

    // ---- commit
    /// Writes the reviewed preview. Takes the preview: after any attempt a fresh preview is required.
    pub fn commit_preview(&mut self) -> ModResult<CommitReport> {
        let preview = self.preview.take().ok_or("沒有可套用的預覽，請先執行預覽。")?;
        if let Some(b) = preview.blockers.first() {
            return Err(format!("預覽含阻擋項，無法寫入: {b}"));
        }
        if preview.changed_count() == 0 {
            return Err("沒有需要寫入的變更。".into());
        }
        if preview.kind == PreviewKind::Apply {
            self.require_valid()?;
            if self.game_dir()? != preview.game {
                return Err("遊戲目錄在預覽後已變更，請重新預覽。".into());
            }
        }
        if preview.fingerprint.iter().any(|(p, sha)| disk_sha(p) != *sha) {
            return Err("預覽後遊戲檔案已被修改，請重新預覽。".into());
        }
        let root = self.snapshot_root().ok_or("未設定快照目錄，拒絕在沒有還原點的情況下寫入。")?;
        if matches!(preview.kind, PreviewKind::Apply | PreviewKind::RestoreOriginal(_)) {
            // Establish the `.bak` baseline of the original files before the first modification (as the CLI does).
            self.load_backup(&preview.game, true)?;
        }
        let m = snapshot::create(&root, &preview.game, &preview.kind.label(), &preview.plan).map_err(io_err)?;
        let id = m.info.id.clone();
        if let Err(e) = commit_plan(&preview.plan) {
            // commit_plan rolls back on failure; keep the snapshot only if the disk no longer matches the pre-state.
            let untouched = m.entries.iter().all(|en| disk_sha(&preview.game.join(&en.rel)) == en.pre_sha);
            if untouched {
                snapshot::remove(&root, &id);
            } else {
                let _ = snapshot::set_state(&root, &id, "failed");
            }
            return Err(format!("寫入失敗: {e}"));
        }
        snapshot::set_state(&root, &id, "committed").map_err(io_err)?;
        let report = CommitReport { snapshot_id: id, written: preview.plan.files.len(), deleted: preview.plan.deletes.len() };
        if let PreviewKind::RestoreOriginal(scope) = preview.kind {
            self.reset_after_restore(scope, &preview.game);
        }
        self.note(format!("已寫入 {} 個檔案、刪除 {} 個；還原點 {}", report.written, report.deleted, report.snapshot_id));
        Ok(report)
    }
}

// ------------------------------------------------------------------------ UI

pub struct ModifierUi {
    pub open: bool,
    pub model: ModifierModel,
    search: String,
    category: Option<FeatureCategory>,
    profile_name: String,
    export_path: String,
    selected_profile: Option<String>,
    confirm_write: bool,
    pub unit_stats_draft: Option<UnitStatsDraft>,
    /// Open restore-options dialog: `(scope, keep custom maps)` draft, copied to the model only on confirm.
    pub restore_dialog: Option<(RestoreScope, bool)>,
    pub preset_file_picker: Option<PresetPickerRunner>,
    snapshots: Vec<SnapshotInfo>,
    snapshots_stale: bool,
    startup_migrated: bool,
    status: Option<(String, bool)>,
    /// Process spawner for "Start Game"; replaceable via [`ModifierUi::with_game_launcher`].
    launcher: GameLauncher,
    /// Confirmation dialog for the Balanced Base Stats template is open.
    pub template_confirm: bool,
    icons: Option<UnitIcons>,
    icon_textures: std::collections::HashMap<String, egui::TextureHandle>,
}

impl Default for ModifierUi {
    fn default() -> Self {
        Self {
            open: false,
            model: ModifierModel::default(),
            search: String::new(),
            category: None,
            profile_name: String::new(),
            export_path: String::new(),
            selected_profile: None,
            confirm_write: false,
            unit_stats_draft: None,
            restore_dialog: None,
            preset_file_picker: None,
            snapshots: Vec::new(),
            snapshots_stale: true,
            startup_migrated: false,
            status: None,
            launcher: Box::new(spawn_game_process),
            template_confirm: false,
            icons: None,
            icon_textures: std::collections::HashMap::new(),
        }
    }
}

enum Action {
    DetectGame,
    LoadCurrent,
    EnableAll,
    Clear,
    PreviewApply,
    OpenRestore,
    PreviewRestore(RestoreScope, bool),
    PreviewUndo(String),
    Commit,
    SaveProfile,
    LoadProfile(String),
    Export,
    Import,
    StartGame,
    ConfirmTemplate,
    TogglePromotion(String),
}

fn category_label(c: Option<FeatureCategory>) -> &'static str {
    match c {
        None => "全部",
        Some(FeatureCategory::Stats) => "Stats 數值",
        Some(FeatureCategory::Compat) => "Compat 相容性",
        Some(FeatureCategory::Language) => "Language 語言",
    }
}

impl ModifierUi {
    /// Restores the persisted selection from `state_dir`; a corrupt file is reported, not fatal.
    pub fn persisted(state_dir: PathBuf) -> Self {
        let mut ui = Self { model: ModifierModel::with_state_dir(state_dir), ..Self::default() };
        // Promoted features live in the settings file shared with the C# tools
        // and the editor (`%APPDATA%\AgainstRomeModifier\settings.json`).
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let shared = PathBuf::from(appdata).join("AgainstRomeModifier").join("settings.json");
            ui.model.app_loc = Loc::load(shared, "zh-TW");
        }
        if let Err(e) = ui.model.load_settings() {
            ui.status = Some((e, true));
        }
        ui
    }
    /// Runs the C# form-start safe migrations once per `ModifierUi`; call at startup, never from `show`.
    /// Without a valid game directory nothing is written and the reason is shown as an error status.
    pub fn run_startup_migrations(&mut self) {
        if std::mem::replace(&mut self.startup_migrated, true) {
            return;
        }
        match self.model.run_startup_migrations() {
            Ok(0) => {}
            Ok(n) => self.set_status(Ok(format!("啟動安全遷移完成: 更新 {n} 個檔案"))),
            Err(e) => self.set_status(Err(format!("啟動安全遷移失敗: {e}"))),
        }
    }
    pub fn with_preset_picker<F>(mut self, picker: F) -> Self
    where
        F: Fn(FilePickerKind) -> Option<PathBuf> + Send + Sync + 'static,
    {
        self.preset_file_picker = Some(Box::new(picker));
        self
    }
    /// Replaces the process spawner (tests inject a fake so nothing is ever launched).
    pub fn with_game_launcher<F>(mut self, launcher: F) -> Self
    where
        F: Fn(&Path, &Path) -> io::Result<()> + Send + Sync + 'static,
    {
        self.launcher = Box::new(launcher);
        self
    }
    /// "Start Game" action; also usable headlessly.
    pub fn start_game(&mut self) -> ModResult<String> {
        let r = self.model.start_game_with(&*self.launcher);
        r.map(|exe| format!("已啟動 {}", exe.display()))
    }
    /// Loads unit icons once per (re)open of the stats editor; textures are created lazily in `show_unit_stats`.
    fn reload_icons(&mut self) {
        self.icon_textures.clear();
        self.icons = Some(self.model.load_unit_icons());
    }
    pub fn set_status(&mut self, r: ModResult<String>) {
        self.status = Some(match r {
            Ok(m) => (m, false),
            Err(e) => (e, true),
        });
    }
    fn run(&mut self, action: Action) {
        let r: ModResult<String> = match action {
            Action::DetectGame => self.model.detect_game().map(|p| format!("偵測到遊戲目錄 {}", p.display())),
            Action::LoadCurrent => self.model.load_current_state().map(|n| format!("已載入遊戲目前狀態 ({n} 項啟用)")),
            Action::EnableAll => {
                let n = self.model.enable_all();
                Ok(format!("已開啟 {n} 項功能 (不含高風險/需手動啟用項，速度/配額不變)"))
            }
            Action::Clear => {
                self.model.clear_selection();
                Ok("已清除選擇 (全部停用，速度/配額 1x)".into())
            }
            Action::PreviewApply => self.model.preview_apply().map(|_| "預覽完成，請檢視變更後再套用。".into()),
            Action::OpenRestore => {
                // C# defaults: restore everything, keep custom maps.
                self.restore_dialog = Some((RestoreScope::All, true));
                return;
            }
            Action::PreviewRestore(scope, preserve) => {
                self.model.set_restore_options(scope, preserve);
                self.model.preview_restore_original().map(|_| "還原預覽完成。".into())
            }
            Action::PreviewUndo(id) => self.model.preview_undo(&id).map(|_| "復原預覽完成。".into()),
            Action::Commit => {
                let r = self.model.commit_preview().map(|c| format!("完成: 寫入 {}、刪除 {}；還原點 {}", c.written, c.deleted, c.snapshot_id));
                self.snapshots_stale = true;
                r
            }
            Action::SaveProfile => {
                let name = self.profile_name.trim().to_owned();
                self.model.save_profile(&name).map(|p| format!("已保存設定檔 {}", p.display()))
            }
            Action::LoadProfile(name) => self.model.load_profile(&name).map(|_| format!("已載入設定檔 {name}")),
            Action::Export => {
                let p = PathBuf::from(self.export_path.trim());
                self.model.export_profile(&p).map(|_| format!("已匯出 {}", p.display()))
            }
            Action::Import => {
                let p = PathBuf::from(self.export_path.trim());
                self.model.import_profile(&p).map(|_| format!("已匯入 {}", p.display()))
            }
            Action::StartGame => self.start_game(),
            Action::ConfirmTemplate => {
                self.template_confirm = false;
                let r = self.model.apply_balanced_template().map(|n| format!("已套用修改器內建平衡範本 ({n} 個兵種)，請重新預覽套用。"));
                if r.is_ok() {
                    // An open draft was built from the previous profile and would be rejected on apply.
                    self.unit_stats_draft = None;
                }
                r
            }
            Action::TogglePromotion(id) => self.model.toggle_promotion(&id).map(|promoted| {
                if promoted { format!("{id} 已移至主控制台") } else { format!("{id} 已移回實驗性") }
            }),
        };
        self.confirm_write = false;
        self.set_status(r);
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        if self.snapshots_stale {
            self.snapshots = self.model.list_snapshots();
            self.snapshots_stale = false;
        }
        let mut open = self.open;
        let mut action = None;
        egui::Window::new("Modifier Studio (遊戲修改器)").open(&mut open).default_width(760.0).default_height(640.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| action = self.draw(ui));
        });
        self.open = open;
        self.show_unit_stats(ctx);
        if let Some(a) = self.show_restore_dialog(ctx) {
            action = Some(a);
        }
        if let Some(a) = self.show_template_dialog(ctx) {
            action = Some(a);
        }
        if let Some(a) = action {
            self.run(a);
        }
        if self.model.take_settings_dirty() && self.model.state_dir.is_some() {
            if let Err(e) = self.model.save_settings() {
                self.status = Some((format!("無法保存 Modifier 設定: {e}"), true));
            }
        }
    }

    fn draw(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let mut action = None;
        ui.horizontal(|ui| {
            ui.label("遊戲目錄:");
            let mut path = self.model.game_path.clone();
            if ui.add(egui::TextEdit::singleline(&mut path).desired_width(380.0)).changed() {
                self.model.set_game_path(path);
            }
            if ui.button("偵測").clicked() {
                action = Some(Action::DetectGame);
            }
            if ui.button("載入遊戲目前狀態").on_hover_text("唯讀掃描遊戲檔案，將偵測到的功能設為目前選擇").clicked() {
                action = Some(Action::LoadCurrent);
            }
            if ui.button(if self.model.english { "Start Game" } else { "啟動遊戲" }).on_hover_text("以遊戲目錄為工作目錄啟動 Against_Rome.exe").clicked() {
                action = Some(Action::StartGame);
            }
        });
        ui.horizontal(|ui| {
            let mut en = self.model.english;
            if ui.checkbox(&mut en, "English 功能名稱").changed() {
                self.model.english = en;
                self.model.settings_dirty = true;
            }
            ui.label(format!("已啟用 {} 項功能 · 自訂兵種數值 {} 筆", self.model.enabled_ids().len(), self.model.custom_unit_count()));
        });

        ui.separator();
        ui.collapsing("設定檔 (Profile)", |ui| {
            ui.horizontal(|ui| {
                ui.label("名稱:");
                ui.text_edit_singleline(&mut self.profile_name);
                if ui.button("保存").clicked() {
                    action = Some(Action::SaveProfile);
                }
                let names = self.model.list_profiles();
                egui::ComboBox::from_id_source("modifier_profiles")
                    .selected_text(self.selected_profile.clone().unwrap_or_else(|| "(選擇已存設定檔)".into()))
                    .show_ui(ui, |ui| {
                        for n in names {
                            ui.selectable_value(&mut self.selected_profile, Some(n.clone()), n);
                        }
                    });
                if ui.add_enabled(self.selected_profile.is_some(), egui::Button::new("載入")).clicked() {
                    action = self.selected_profile.clone().map(Action::LoadProfile);
                }
            });
            ui.horizontal(|ui| {
                ui.label("JSON 路徑 (CLI 相容):");
                ui.add(egui::TextEdit::singleline(&mut self.export_path).desired_width(320.0));
                if ui.button("匯出").clicked() {
                    action = Some(Action::Export);
                }
                if ui.button("匯入").clicked() {
                    action = Some(Action::Import);
                }
            });
        });

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("分類:");
            egui::ComboBox::from_id_source("modifier_category").selected_text(category_label(self.category)).show_ui(ui, |ui| {
                for c in [None, Some(FeatureCategory::Stats), Some(FeatureCategory::Compat), Some(FeatureCategory::Language)] {
                    ui.selectable_value(&mut self.category, c, category_label(c));
                }
            });
            ui.label("搜尋:");
            ui.text_edit_singleline(&mut self.search);
            if ui.button("全部啟用").on_hover_text("不含高風險/需手動啟用的功能 (標示 ⚠)；不改變速度/配額").clicked() {
                action = Some(Action::EnableAll);
            }
            if ui.button("全部停用").clicked() {
                action = Some(Action::Clear);
            }
        });
        if let Some(a) = self.draw_features(ui) {
            action = Some(a);
        }

        ui.separator();
        for issue in self.model.validate() {
            let (color, tag) = match issue.severity {
                Severity::Error => (Color32::LIGHT_RED, "錯誤"),
                Severity::Warning => (Color32::YELLOW, "警告"),
                Severity::Info => (Color32::GRAY, "提示"),
            };
            ui.colored_label(color, format!("[{tag}] {}", issue.message));
        }

        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("預覽套用").on_hover_text("只規劃，不寫入任何檔案").clicked() {
                action = Some(Action::PreviewApply);
            }
            if ui.button("還原原版…").on_hover_text("開啟還原選項視窗；只規劃，不寫入任何檔案").clicked() {
                action = Some(Action::OpenRestore);
            }
        });
        if let Some(a) = self.draw_preview(ui) {
            action = Some(a);
        }
        if let Some(a) = self.draw_snapshots(ui) {
            action = Some(a);
        }

        if let Some((msg, err)) = &self.status {
            ui.separator();
            ui.colored_label(if *err { Color32::LIGHT_RED } else { Color32::LIGHT_GREEN }, msg);
        }
        ui.collapsing("操作紀錄", |ui| {
            for line in self.model.log.iter().rev().take(30) {
                ui.label(line);
            }
        });
        action
    }

    fn draw_features(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let loc = self.model.loc();
        let search = self.search.trim().to_lowercase();
        let mut action = None;
        egui::ScrollArea::vertical().id_source("modifier_features").max_height(240.0).show(ui, |ui| {
            let visible = |category: Option<FeatureCategory>, def: &FeatureDefinition| -> bool {
                if category.is_some_and(|c| c != def.category) {
                    return false;
                }
                let name = loc.get(def.id);
                search.is_empty() || def.id.to_lowercase().contains(&search) || name.to_lowercase().contains(&search)
            };
            let category = self.category;
            // Main list: everything except experimental features that have not been promoted.
            for def in FeatureRegistry::ALL {
                let experimental = is_experimental_feature(def.id);
                if (experimental && !self.model.is_promoted(def.id)) || !visible(category, def) {
                    continue;
                }
                if let Some(a) = self.draw_feature_row(ui, def, &loc, experimental.then_some("移回實驗性 (Demote)")) {
                    action = Some(a);
                }
            }
            // Experimental group: features that have not been promoted yet.
            let pending: Vec<&FeatureDefinition> =
                FeatureRegistry::ALL.iter().filter(|d| is_experimental_feature(d.id) && !self.model.is_promoted(d.id) && visible(category, d)).collect();
            if !pending.is_empty() {
                ui.separator();
                ui.strong(loc.get("ExperimentalCardTitle"));
                ui.small(loc.get("ExperimentalSubtitle"));
                for def in pending {
                    if let Some(a) = self.draw_feature_row(ui, def, &loc, Some("移至主控制台 (Promote)")) {
                        action = Some(a);
                    }
                }
            }
        });
        action
    }

    /// One feature row; `promote_label` adds the Promote/Demote button (experimental features only).
    fn draw_feature_row(&mut self, ui: &mut egui::Ui, def: &FeatureDefinition, loc: &Loc, promote_label: Option<&str>) -> Option<Action> {
        let mut action = None;
        let name = loc.get(def.id);
        let tip_key = format!("{}Tip", def.id);
        let tip = loc.get(&tip_key);
        ui.horizontal(|ui| {
            match (def.control_kind, def.id) {
                (FeatureControlKind::Toggle, _) => {
                    let mut on = self.model.feature_enabled(def.id);
                    let risky = ENABLE_ALL_EXCLUDED.contains(&def.id);
                    let text = if risky { format!("{name} ({}) ⚠", def.id) } else { format!("{name} ({})", def.id) };
                    if ui.checkbox(&mut on, text).on_hover_text(tip).changed() {
                        let _ = self.model.set_feature(def.id, on);
                    }
                }
                (_, "GameSpeed") => {
                    let mut n = self.model.game_speed();
                    if ui.add(egui::DragValue::new(&mut n).clamp_range(1..=10).suffix("x")).on_hover_text(tip).changed() {
                        let _ = self.model.set_game_speed(n);
                    }
                    ui.label(format!("{name} ({})", def.id));
                }
                (_, "VillageGarrisonQuota3x") => {
                    let mut n = self.model.quota();
                    if ui.add(egui::DragValue::new(&mut n).clamp_range(1..=10).suffix("x")).on_hover_text(tip).changed() {
                        let _ = self.model.set_quota(n);
                    }
                    ui.label(format!("{name} ({})", def.id));
                }
                (_, "CustomUnitStats") => {
                    if ui.button(format!("{name} ({}): {} 筆 — 編輯", def.id, self.model.custom_unit_count())).on_hover_text(tip).clicked() {
                        match UnitStatsDraft::load(&self.model) {
                            Ok(draft) => {
                                self.unit_stats_draft = Some(draft);
                                self.reload_icons();
                            }
                            Err(e) => self.set_status(Err(e)),
                        }
                    }
                    if ui.button(loc.get("TroopTemplateBalanced")).on_hover_text("以內建平衡數值覆蓋所有自訂兵種屬性 (需確認)").clicked() {
                        self.template_confirm = true;
                    }
                    if !self.model.preset_label.is_empty() {
                        ui.label(format!("範本: {}", self.model.preset_label));
                    }
                }
                _ => {
                    // Composite Endless-AI modules mirror EndlessAi.Core / RomanReinforcementGarrison.
                    let mut on = self.model.profile.get_endless_ai_module(def.id.trim_start_matches("EndlessAi."));
                    ui.add_enabled(false, egui::Checkbox::new(&mut on, format!("{name} ({}) — 隨 EndlessAi.Core / M6 連動", def.id))).on_hover_text(tip);
                }
            }
            if let Some(label) = promote_label {
                if ui.small_button(label).clicked() {
                    action = Some(Action::TogglePromotion(def.id.to_owned()));
                }
            }
        });
        action
    }

    /// Confirmation for the Balanced Base Stats template (C# `MessageBox` YesNo); No changes nothing.
    fn show_template_dialog(&mut self, ctx: &egui::Context) -> Option<Action> {
        if !self.template_confirm {
            return None;
        }
        let en = self.model.english;
        let (mut yes, mut no) = (false, false);
        let mut open = true;
        egui::Window::new(if en { "Confirm Template Overwrite" } else { "確認範本覆蓋" }).open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.label(if en {
                "Are you sure you want to apply the 'Balanced Base Stats' template? This will overwrite your current custom unit stats."
            } else {
                "確定要套用「修改器內建平衡」範本嗎？這將覆蓋目前的自訂兵種屬性。"
            });
            ui.horizontal(|ui| {
                yes = ui.button(if en { "Yes" } else { "是" }).clicked();
                no = ui.button(if en { "No" } else { "否" }).clicked();
            });
        });
        if yes {
            return Some(Action::ConfirmTemplate);
        }
        if no || !open {
            self.template_confirm = false;
        }
        None
    }

    /// C# `RestoreAllOptionsDialog` plus the per-category restores: pick a scope, keep/delete custom maps,
    /// then 還原 (plans a reviewable preview; nothing is written) or 取消 (closes with no model change).
    fn show_restore_dialog(&mut self, ctx: &egui::Context) -> Option<Action> {
        let (mut scope, mut preserve) = self.restore_dialog?;
        let en = self.model.english;
        let (mut open, mut confirm, mut cancel) = (true, false, false);
        egui::Window::new(if en { "Restore All" } else { "完整還原" }).open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.set_max_width(460.0);
            ui.label(if en {
                "Restore the original game settings. Custom maps are kept by default.\n\nUncheck below to delete registered custom maps and their contents. If restoration fails, the transaction restores the maps."
            } else {
                "還原遊戲的原版設定，預設保留自製地圖。\n\n取消勾選會刪除已登記的自製地圖及其內容；還原失敗時，交易會回復地圖。"
            });
            ui.label(if en { "Category:" } else { "還原類別:" });
            for s in RestoreScope::ALL {
                ui.radio_value(&mut scope, s, s.label_for(en));
            }
            ui.add_enabled(scope == RestoreScope::All, egui::Checkbox::new(&mut preserve, if en { "Keep custom maps" } else { "保留自製地圖" }));
            ui.horizontal(|ui| {
                confirm = ui.button(if en { "Restore" } else { "還原" }).clicked();
                cancel = ui.button(if en { "Cancel" } else { "取消" }).clicked();
            });
        });
        if confirm {
            self.restore_dialog = None;
            return Some(Action::PreviewRestore(scope, preserve));
        }
        self.restore_dialog = (open && !cancel).then_some((scope, preserve));
        None
    }

    fn show_unit_stats(&mut self, ctx: &egui::Context) {
        if self.unit_stats_draft.is_none() { return; }
        let mut open = true;
        let (mut apply, mut cancel) = (false, false);
        let mut triggered_import = false;
        let mut triggered_export = false;
        let is_en = self.model.english;
        let import_label = if is_en { "Load Stats File (Import)" } else { "載入屬性檔 (匯入)" };
        let export_label = if is_en { "Save Stats File (Export)" } else { "儲存屬性檔 (匯出)" };

        {
            let draft = self.unit_stats_draft.as_mut().unwrap();
            let (icons, textures) = (&self.icons, &mut self.icon_textures);
            egui::Window::new(if is_en { "CustomUnitStats — 6 Editable Troop Stats" } else { "CustomUnitStats — 六項兵種屬性" })
                .open(&mut open)
                .default_width(980.0)
                .show(ctx, |ui| {
                    ui.label(if is_en {
                        "Only updates draft/profile; applying to game requires Preview -> Commit. Speed, Range, Radius, and Priest sight are preserved from original."
                    } else {
                        "僅更新設定檔；寫入遊戲仍需預覽套用。速度、射程、半徑與 priest 視野由原版保留。"
                    });
                    if !self.model.feature_enabled("Balance") {
                        ui.colored_label(
                            Color32::YELLOW,
                            if is_en { "CustomUnitStats requires Balance feature enabled to apply in-game." } else { "CustomUnitStats 需要啟用 Balance 才會套用至遊戲。" },
                        );
                    }
                    ui.horizontal(|ui| {
                        if ui.button(import_label).clicked() {
                            triggered_import = true;
                        }
                        if ui.button(export_label).clicked() {
                            triggered_export = true;
                        }
                        if !draft.loaded_file_name.is_empty() {
                            let file_label = if is_en { "Loaded file: " } else { "已載入檔案：" };
                            ui.label(format!("{file_label}{}", draft.loaded_file_name));
                        }
                    });
                    ui.small(if is_en {
                        "Import only updates draft and can be cancelled. Export saves to chosen .artroop file outside game directory. Timestamps are UTC."
                    } else {
                        "匯入僅更新草稿，可取消；匯出會儲存至選定的 .artroop 檔案，請使用遊戲目錄以外的路徑。檔頭時間為 UTC。"
                    });
                    if let Some((message, error)) = &draft.preset_status {
                        ui.colored_label(if *error { Color32::LIGHT_RED } else { Color32::LIGHT_GREEN }, message);
                    }
                    egui::ScrollArea::both().id_source("custom_unit_stats_scroll").max_height(450.0).show(ui, |ui| {
                        egui::Grid::new("custom_unit_stats_grid").striped(true).show(ui, |ui| {
                            ui.label("");
                            ui.label("兵種 / faction / tier");
                            for (_, label) in EDITABLE_STATS { ui.label(label); }
                            ui.end_row();
                            for row in &mut draft.rows {
                                let meta = TroopConfig::get_metadata(&row.key).unwrap();
                                show_unit_icon(ui, ctx, icons, textures, &row.key);
                                ui.label(format!("{} / {} / {}", TroopConfig::get_name_zh(&row.key).unwrap_or(&row.key), meta.faction, meta.tier)).on_hover_text(&row.key);
                                for (i, cell) in row.cells.iter_mut().enumerate() {
                                    ui.add_enabled(!(meta.unit_type == "priest" && EDITABLE_STATS[i].0 == 5), egui::TextEdit::singleline(cell).desired_width(65.0));
                                }
                                ui.end_row();
                            }
                        });
                    });
                    ui.horizontal(|ui| {
                        apply = ui.button(if is_en { "Apply to Profile" } else { "更新設定檔" }).clicked();
                        cancel = ui.button(if is_en { "Cancel" } else { "取消" }).clicked();
                    });
                });
        }

        if triggered_import {
            let chosen = if let Some(picker) = &self.preset_file_picker {
                picker(FilePickerKind::ImportPreset)
            } else {
                native_preset_dialog(FilePickerKind::ImportPreset, is_en)
            };
            if let Some(path) = chosen {
                if let Some(draft) = self.unit_stats_draft.as_mut() {
                    draft.preset_status = Some(match draft.import_preset_from_path(&path) {
                        Ok(s) => (s, false),
                        Err(e) => (e, true),
                    });
                }
            }
        }

        if triggered_export {
            let is_valid = self.unit_stats_draft.as_ref().map(|d| d.validate_inputs()).unwrap_or(Ok(()));
            if let Err(e) = is_valid {
                if let Some(draft) = self.unit_stats_draft.as_mut() {
                    draft.preset_status = Some((format!("數值驗證失敗，無法開啟匯出對話框：{e}"), true));
                }
            } else {
                let chosen = if let Some(picker) = &self.preset_file_picker {
                    picker(FilePickerKind::ExportPreset)
                } else {
                    native_preset_dialog(FilePickerKind::ExportPreset, is_en)
                };
                if let Some(path) = chosen {
                    if let Some(draft) = self.unit_stats_draft.as_mut() {
                        draft.preset_status = Some(match draft.export_preset_to_path(&path) {
                            Ok(s) => (s, false),
                            Err(e) => (e, true),
                        });
                    }
                }
            }
        }

        if apply {
            let result = if let Some(draft) = self.unit_stats_draft.as_ref() {
                draft.apply(&mut self.model)
            } else {
                Ok(())
            };
            if result.is_ok() { self.unit_stats_draft = None; self.confirm_write = false; }
            self.set_status(result.map(|_| "兵種數值已更新，請重新預覽套用。".into()));
        } else if !open || cancel {
            self.unit_stats_draft = None;
        }
    }

    fn draw_preview(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let mut action = None;
        let Some(p) = &self.model.preview else {
            ui.label("尚無預覽。寫入前必須先預覽。");
            return None;
        };
        ui.separator();
        ui.strong(format!("預覽 ({}) — {} 個檔案將變更，計畫 {} 個", p.kind.label(), p.changed_count(), p.planned_files));
        if !p.feature_changes.is_empty() {
            ui.label(format!("功能變更: {}", p.feature_changes.join("  ")));
        }
        for b in &p.blockers {
            ui.colored_label(Color32::LIGHT_RED, format!("阻擋: {b}"));
        }
        egui::ScrollArea::vertical().id_source("modifier_preview").max_height(150.0).show(ui, |ui| {
            egui::Grid::new("modifier_preview_grid").striped(true).show(ui, |ui| {
                for e in p.entries.iter().filter(|e| e.kind != ChangeKind::Unchanged) {
                    let tag = match e.kind {
                        ChangeKind::Create => "新增",
                        ChangeKind::Modify => "修改",
                        ChangeKind::Delete => "刪除",
                        ChangeKind::Unchanged => "",
                    };
                    ui.label(tag);
                    ui.label(e.path.strip_prefix(&p.game).unwrap_or(&e.path).display().to_string());
                    ui.label(format!("{} → {} bytes", e.old_len.map_or("-".into(), |n| n.to_string()), e.new_len.map_or("-".into(), |n| n.to_string())));
                    ui.label(e.changed_bytes.map_or(String::new(), |n| format!("{n} bytes 不同")));
                    ui.end_row();
                }
            });
        });
        let can = p.can_commit();
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.confirm_write, "我了解這會寫入遊戲檔案（寫入前會自動保存還原點，首次套用會建立 .bak 原版備份）");
            if ui.add_enabled(can && self.confirm_write, egui::Button::new("確認寫入")).clicked() {
                action = Some(Action::Commit);
            }
        });
        action
    }

    fn draw_snapshots(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let mut action = None;
        ui.collapsing(format!("還原點 ({})", self.snapshots.len()), |ui| {
            if ui.button("重新整理").clicked() {
                self.snapshots_stale = true;
            }
            for s in &self.snapshots {
                ui.horizontal(|ui| {
                    ui.label(format!("{} · {} · {} 個檔案 · {}", s.id, s.label, s.files, s.state));
                    if ui.button("預覽復原").clicked() {
                        action = Some(Action::PreviewUndo(s.id.clone()));
                    }
                });
            }
        });
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use against_rome_modifier::{cli::run_cli, compress_pfil, endless_ai::words_bytes, FOCUS};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    /// Synthetic game directory with `.bak` baselines (no installed game, no Backup.zip).
    struct Sandbox(PathBuf);
    impl Sandbox {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("arm-modifier-ui-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            let _ = fs::remove_dir_all(&dir);
            let game = dir.join("game");
            fs::create_dir_all(&game).unwrap();
            let sb = Self(dir);
            let mut exe = vec![0x55; FOCUS.offset + FOCUS.original.len() + 1];
            exe[FOCUS.offset..FOCUS.offset + FOCUS.original.len()].copy_from_slice(FOCUS.original);
            for (rel, bytes) in [
                ("Against_Rome.exe", &exe[..]),
                ("SYSTEM/cl_script.ini", b"[CiviDelay]\r\nDelay=100\r\n"),
                ("SYSTEM/ress.ini", b"[objres]\r\n"),
                ("SYSTEM/DATA_MP/DEFAULTS/objdef.dau", b""),
                ("SYSTEM/DATA_MP/DEFAULTS/partgeo.dau", b"PARTGEO"),
                ("SYSTEM/CLMK/icon.ini", b""),
                ("SYSTEM/CLAK/cl_scint.ini", b"SCINT"),
                ("MAPS/ENDL_000/team.dat", b"TEAMS"),
            ] {
                sb.put(rel, bytes);
                sb.put(&format!("{rel}.bak"), bytes);
            }
            sb.put("SYSTEM/cl_epara.ini", b"[General]\r\n");
            let sites = [
                ("ak_anfuehrer", 87), ("ak_artillerie", 69), ("ak_geisterreiter", 68), ("ak_kampfverband", 89), ("ak_krieger", 73), ("ak_kundschafterwolf", 77),
                ("ak_landtier", 72), ("ak_packpferd", 57), ("ak_priester", 92), ("ak_verbandswolf", 68), ("ak_zivilist", 83), ("ak_zivilverband", 87),
            ];
            for (file, symbol) in sites {
                let mut bytes = words_bytes(&[66, 1, 81, 10, 81, 98, 128, symbol, 73, -3, 86]);
                bytes.resize(128, 0);
                let raw = compress_pfil(&bytes, None).unwrap();
                sb.put(&format!("SYSTEM/CLAK/SCRIPT/{file}.bci"), &raw);
            }
            let anf = fs::read(sb.game().join("SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci")).unwrap();
            sb.put("SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci.bak", &anf);
            sb
        }
        fn game(&self) -> PathBuf {
            self.0.join("game")
        }
        fn put(&self, rel: &str, bytes: &[u8]) {
            let p = self.game().join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, bytes).unwrap();
        }
        fn model(&self) -> ModifierModel {
            let mut m = ModifierModel::with_state_dir(self.0.join("state"));
            m.backup_base = self.0.join("no-backup-zip-here");
            m.set_game_path(self.game().to_string_lossy().into_owned());
            m
        }
        /// Every file under the game directory except lock/temp leftovers.
        fn tree(&self) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
            fn walk(d: &Path, out: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
                for e in fs::read_dir(d).unwrap().flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        walk(&p, out)
                    } else {
                        out.insert(p.clone(), fs::read(&p).unwrap());
                    }
                }
            }
            let mut out = Default::default();
            walk(&self.game(), &mut out);
            out
        }
    }
    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn startup_migrations_on_sandbox_game_are_idempotent_and_stay_inside_game_dir() {
        let sb = Sandbox::new();
        let mut m = sb.model();
        let before = sb.tree();
        let first = m.run_startup_migrations();
        let after_first = sb.tree();
        let second = m.run_startup_migrations();
        assert_eq!(sb.tree(), after_first, "second run must not change anything ({first:?} / {second:?})");
        assert!(matches!(second, Ok(0)) || second.is_err(), "{second:?}");
        assert_eq!(first.is_ok(), second.is_ok(), "{first:?} / {second:?}");
        // Nothing outside the game directory (e.g. the state dir) is touched by the migration.
        assert!(!sb.0.join("state").join("snapshots").exists());
        assert!(before.keys().all(|p| p.starts_with(sb.game())));
    }

    fn cli(args: &[&str]) -> (i32, Value) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_cli(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>(), &mut out, &mut err);
        let v = serde_json::from_slice(&out).unwrap_or_else(|_| panic!("cli output not JSON: {} / {}", String::from_utf8_lossy(&out), String::from_utf8_lossy(&err)));
        (code, v)
    }

    fn unit_stats_fixture(sb: &Sandbox) {
        let mut text = String::from("header\r\nheader\r\n");
        for key in TroopConfig::sorted_units() {
            let mut columns = vec!["0".to_owned(); 220];
            for (i, value) in [(4, "1.75"), (19, "400"), (24, "1234"), (142, "25"), (146, "30")] {
                columns[i] = value.into();
            }
            columns[52] = key.into();
            text.push_str(&columns.join(","));
            text.push_str("\r\n");
        }
        sb.put("SYSTEM/DATA_MP/DEFAULTS/objdef.dau", text.as_bytes());
        sb.put("SYSTEM/DATA_MP/DEFAULTS/objdef.dau.bak", text.as_bytes());
    }

    #[test]
    fn custom_stats_updates_roundtrip_and_preserves_independent_fields() {
        let sb = Sandbox::new();
        unit_stats_fixture(&sb);
        let mut m = sb.model();
        let before_tree = sb.tree();
        let key = "FigRomInf00_Lanze_Schild";
        m.profile.custom_unit_stats = Some([(key.to_lowercase(), vec![100., 2., 3., 4., 999., 6., 7., 888., 777.])].into());
        m.preview_apply().unwrap();
        m.take_settings_dirty();
        let mut draft = UnitStatsDraft::load(&m).unwrap();
        let row = draft.rows.iter_mut().find(|r| r.key == key).unwrap();
        assert_eq!(row.cells[0], "100");
        let original = row.original;
        row.cells = ["321.25", "12.75", "41.5", "42.25", "1500.5", "234.75"].map(str::to_owned);
        // Even a forged edit to a disabled priest field cannot change its original sight.
        let priest = draft.rows.iter_mut().find(|r| r.key == "FigKelPri00_Priester").unwrap();
        let priest_original = priest.original;
        priest.cells[4] = "999999".into();
        draft.apply(&mut m).unwrap();
        assert_eq!(sb.tree(), before_tree, "editing only changes the in-memory profile");
        assert!(!sb.0.join("state").exists());
        assert!(m.preview.is_none() && m.take_settings_dirty());
        let stats = m.profile.custom_unit_stats.as_ref().unwrap();
        assert!(!stats.contains_key(&key.to_lowercase()));
        assert_eq!(stats[key], [321.25, 12.75, 41.5, 42.25, original[4], 1500.5, 234.75, original[7], original[8]]);
        for i in [4, 5, 7, 8] { assert_eq!(stats["FigKelPri00_Priester"][i], priest_original[i]); }
        let path = sb.0.join("units.json");
        m.export_profile(&path).unwrap();
        let mut imported = sb.model();
        imported.import_profile(&path).unwrap();
        assert_eq!(imported.profile_json_text(), m.profile_json_text());
        m.save_settings().unwrap();
        assert_eq!(ModifierUi::persisted(sb.0.join("state")).model.profile_json_text(), m.profile_json_text());
        m.set_feature("Balance", true).unwrap();
        let backup = m.load_backup(&sb.game(), false).unwrap();
        assert_eq!(backup.get_base_stats_for_unit(key, &m.profile).unwrap()[0], 321.25);
        let projected = against_rome_modifier::unit_stats::project(&backup, key, &m.profile).unwrap();
        m.set_feature("UnitMovementSpeed2x", true).unwrap();
        assert_eq!(against_rome_modifier::unit_stats::project(&backup, key, &m.profile).unwrap()[4], projected[4] * 2.0);
    }

    #[test]
    fn custom_stats_rejects_invalid_cells_atomically_and_keeps_review() {
        let sb = Sandbox::new();
        unit_stats_fixture(&sb);
        let mut m = sb.model();
        m.preview_apply().unwrap();
        m.take_settings_dirty();
        let before = m.profile_json_text();
        for (cell, value) in [(0, "0"), (0, "-1"), (1, "-0.1"), (2, "NaN"), (3, "inf"), (4, "oops"), (5, "-1")] {
            let mut draft = UnitStatsDraft::load(&m).unwrap();
            draft.rows[0].cells[0] = "555".into();
            let row = draft.rows.iter_mut().rfind(|r| TroopConfig::get_metadata(&r.key).unwrap().unit_type != "priest").unwrap();
            row.cells[cell] = value.into();
            assert!(draft.apply(&mut m).is_err(), "{cell}={value}");
            assert_eq!(m.profile_json_text(), before);
            assert!(m.preview.is_some() && !m.take_settings_dirty());
        }
        let mut valid = UnitStatsDraft::load(&m).unwrap();
        valid.rows[0].cells = ["0.1", "0", "0", "0", "0", "0"].map(str::to_owned);
        valid.apply(&mut m).unwrap();
    }

    #[test]
    fn custom_stats_editing_and_cancel_are_readonly_and_stale_drafts_fail() {
        let sb = Sandbox::new();
        unit_stats_fixture(&sb);
        let mut m = sb.model();
        let tree = sb.tree();
        let profile = m.profile_json_text();
        let mut draft = UnitStatsDraft::load(&m).unwrap();
        draft.rows[0].cells[0] = "800".into();
        assert_eq!(m.profile_json_text(), profile);
        drop(draft); // cancel
        assert_eq!(sb.tree(), tree);
        assert!(!sb.0.join("state").exists());
        let draft = UnitStatsDraft::load(&m).unwrap();
        m.set_game_speed(2).unwrap();
        assert!(draft.apply(&mut m).unwrap_err().contains("設定"));
        let draft = UnitStatsDraft::load(&m).unwrap();
        let original_path = sb.game().join("SYSTEM/DATA_MP/DEFAULTS/objdef.dau.bak");
        let changed = fs::read_to_string(&original_path).unwrap().replace(",400,", ",401,");
        fs::write(original_path, changed).unwrap();
        assert!(draft.apply(&mut m).unwrap_err().contains("備份已變更"));
        let sb = Sandbox::new();
        assert!(UnitStatsDraft::load(&sb.model()).is_err(), "missing unit rows must not become zero baselines");
    }

    #[test]
    fn artroop_legacy_bom_partial_fallback_and_skipped_rows() {
        let sb = Sandbox::new();
        unit_stats_fixture(&sb);
        let mut m = sb.model();
        m.take_settings_dirty();
        let profile = m.profile_json_text();
        let tree = sb.tree();
        let mut draft = UnitStatsDraft::load(&m).unwrap();
        let keys: Vec<_> = draft.rows.iter().filter(|r| TroopConfig::get_metadata(&r.key).unwrap().unit_type != "priest").take(3).map(|r| r.key.clone()).collect();
        let priest = "FigKelPri00_Priester";
        let path = sb.0.join("legacy.artroop");
        fs::write(&path, format!("\u{feff}{}=321.25,12.75,41.5,42.25,1500.5,234.75\n{}=100,11,22,33\n{priest}=200,2,3,4,999999,55\nbad=1,2\nbad2=no,2,3,4\n", keys[0].to_uppercase(), keys[1])).unwrap();
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        io::Write::write_all(&mut file, format!("{}=250,2,3,4,99999,1250,75,88888,77777\n", keys[2]).as_bytes()).unwrap();
        drop(file);
        draft.preset_path = path.to_string_lossy().into_owned();
        assert!(draft.import_preset().unwrap().contains("略過 2 行"));
        let a = draft.rows.iter().find(|r| r.key == keys[0]).unwrap();
        assert_eq!(a.cells, ["321.25", "12.75", "41.5", "42.25", "1500.5", "234.75"].map(str::to_owned));
        for index in [4, 7, 8] { assert_eq!(a.values().unwrap()[index], a.original[index]); }
        let b = draft.rows.iter().find(|r| r.key == keys[1]).unwrap();
        assert_eq!(b.cells[4], b.balanced[5].to_string());
        assert_eq!(b.cells[5], b.balanced[6].to_string());
        let c = draft.rows.iter().find(|r| r.key == keys[2]).unwrap();
        assert_eq!(c.cells, ["250", "2", "3", "4", "1250", "75"].map(str::to_owned));
        for index in [4, 7, 8] { assert_eq!(c.values().unwrap()[index], c.original[index]); }
        let p = draft.rows.iter().find(|r| r.key == priest).unwrap();
        assert_eq!(p.cells[4], p.original[5].to_string());
        assert_eq!(m.profile_json_text(), profile);
        assert!(!m.take_settings_dirty());
        assert_eq!(sb.tree(), tree);
    }

    #[test]
    fn artroop_export_roundtrips_draft_six_columns_without_profile_or_game_writes() {
        let sb = Sandbox::new();
        unit_stats_fixture(&sb);
        let mut m = sb.model();
        m.take_settings_dirty();
        let profile = m.profile_json_text();
        let tree = sb.tree();
        let mut draft = UnitStatsDraft::load(&m).unwrap();
        draft.rows[0].cells[0] = "321.1234567890123".into();
        let path = sb.0.join("export.artroop");
        fs::write(&path, "old preset").unwrap();
        draft.preset_path = path.to_string_lossy().into_owned();
        draft.export_preset().unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# Against Rome Modifier - Custom Troop Preset File (independent modifiers removed)\r\n# Generated on: "));
        assert!(text.contains("# Format: UnitKey=HP,Dmg,VW,AW,Sight,Relt\r\n\r\n"));
        assert!(text.contains("321.1234567890123"));
        for line in text.lines().filter(|l| l.contains('=') && !l.starts_with('#')) {
            assert_eq!(line.split_once('=').unwrap().1.split(',').count(), 6);
        }
        let mut imported = UnitStatsDraft::load(&m).unwrap();
        imported.preset_path = draft.preset_path.clone();
        imported.import_preset().unwrap();
        for (a, b) in draft.rows.iter().zip(&imported.rows) { assert_eq!(a.values().unwrap(), b.values().unwrap()); }
        assert_eq!(m.profile_json_text(), profile);
        assert!(!m.take_settings_dirty());
        drop(imported); // Cancel after import/export: no profile change or persistence.
        assert_eq!(UnitStatsDraft::load(&m).unwrap().rows[0].cells[0], draft.rows[0].balanced[0].to_string());
        assert_eq!(sb.tree(), tree);
        assert!(!sb.0.join("state").exists());
        draft.apply(&mut m).unwrap();
        assert_eq!(m.profile.custom_unit_stats.as_ref().unwrap()[&draft.rows[0].key][0], 321.1234567890123);
        assert_eq!(sb.tree(), tree);
    }

    #[test]
    fn artroop_failed_import_and_export_are_atomic_and_game_paths_are_refused() {
        let sb = Sandbox::new();
        unit_stats_fixture(&sb);
        let m = sb.model();
        let mut draft = UnitStatsDraft::load(&m).unwrap();
        draft.rows[0].cells[0] = "777".into();
        let before: Vec<_> = draft.rows.iter().map(|r| r.cells.clone()).collect();
        let profile = m.profile_json_text();
        let tree = sb.tree();
        let input = sb.0.join("invalid.artroop");
        draft.preset_path = input.to_string_lossy().into_owned();
        assert!(draft.import_preset().is_err()); // missing file
        for bytes in [vec![0xff, 0xfe], b"unknown=1,2,3,4\nbad=1,2".to_vec(),
            format!("{}=200,1,2,3,4,5\n{}=NaN,1,2,3,4,5", draft.rows[0].key, draft.rows.last().unwrap().key).into_bytes()] {
            fs::write(&input, bytes).unwrap();
            assert!(draft.import_preset().is_err());
            assert_eq!(draft.rows.iter().map(|r| r.cells.clone()).collect::<Vec<_>>(), before);
        }
        draft.preset_path.clear();
        assert!(draft.import_preset().is_err() && draft.export_preset().is_err());
        draft.preset_path = sb.game().join("forbidden.artroop").to_string_lossy().into_owned();
        assert!(draft.export_preset().unwrap_err().contains("遊戲目錄"));
        draft.preset_path = sb.0.join("absent/output.artroop").to_string_lossy().into_owned();
        assert!(draft.export_preset().is_err());
        let output = sb.0.join("keep.artroop");
        fs::write(&output, "keep existing").unwrap();
        draft.preset_path = output.to_string_lossy().into_owned();
        draft.rows[0].cells[0] = "-1".into();
        assert!(draft.export_preset().is_err());
        assert_eq!(fs::read_to_string(output).unwrap(), "keep existing");
        assert_eq!(m.profile_json_text(), profile);
        assert_eq!(sb.tree(), tree);
        assert!(!sb.0.join("state").exists());
    }

    #[test]
    fn artroop_window_buttons_import_export_and_cancel_without_persistence() {
        fn frame(ui: &mut ModifierUi, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
            ctx.run(egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1200.0))),
                events, ..Default::default()
            }, |ctx| ui.show_unit_stats(ctx))
        }
        fn click(ui: &mut ModifierUi, ctx: &egui::Context, label: &str) {
            let output = frame(ui, ctx, vec![]);
            let pos = output.shapes.iter().find_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) if t.galley.text() == label => Some(t.pos + t.galley.size() * 0.5),
                _ => None,
            }).unwrap_or_else(|| panic!("button {label} not rendered"));
            frame(ui, ctx, vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton {
                pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE,
            }]);
            frame(ui, ctx, vec![egui::Event::PointerButton {
                pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE,
            }]);
        }
        let sb = Sandbox::new();
        unit_stats_fixture(&sb);
        let mut ui = ModifierUi { model: sb.model(), ..Default::default() };
        ui.model.take_settings_dirty();
        let profile = ui.model.profile_json_text();
        let tree = sb.tree();
        let draft = UnitStatsDraft::load(&ui.model).unwrap();
        let path = sb.0.join("buttons.artroop");
        fs::write(&path, format!("{}=777,2,3,4,5,6", draft.rows[0].key)).unwrap();
        let export_path = sb.0.join("exported_buttons.artroop");
        let import_target = path.clone();
        let export_target = export_path.clone();
        ui.preset_file_picker = Some(Box::new(move |kind| match kind {
            FilePickerKind::ImportPreset => Some(import_target.clone()),
            FilePickerKind::ExportPreset => Some(export_target.clone()),
        }));
        ui.unit_stats_draft = Some(draft);
        let ctx = egui::Context::default();
        for _ in 0..3 { frame(&mut ui, &ctx, vec![]); }
        click(&mut ui, &ctx, "載入屬性檔 (匯入)");
        assert_eq!(ui.unit_stats_draft.as_ref().unwrap().rows[0].cells[0], "777");
        assert_eq!(ui.unit_stats_draft.as_ref().unwrap().loaded_file_name, "buttons.artroop");
        assert!(ui.unit_stats_draft.as_ref().unwrap().preset_status.as_ref().is_some_and(|(_, e)| !e));
        click(&mut ui, &ctx, "儲存屬性檔 (匯出)");
        assert_eq!(ui.unit_stats_draft.as_ref().unwrap().loaded_file_name, "exported_buttons.artroop");
        assert!(fs::read_to_string(&export_path).unwrap().contains("# Format: UnitKey=HP,Dmg,VW,AW,Sight,Relt"));
        click(&mut ui, &ctx, "取消");
        assert!(ui.unit_stats_draft.is_none());
        assert_eq!(ui.model.profile_json_text(), profile);
        assert!(!ui.model.take_settings_dirty());
        assert_eq!(sb.tree(), tree);
        assert!(!sb.0.join("state").exists());
    }

    #[test]
    fn artroop_file_picker_cancel_preserves_draft_and_writes_nothing() {
        fn frame(ui: &mut ModifierUi, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
            ctx.run(egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1200.0))),
                events, ..Default::default()
            }, |ctx| ui.show_unit_stats(ctx))
        }
        fn click(ui: &mut ModifierUi, ctx: &egui::Context, label: &str) {
            let output = frame(ui, ctx, vec![]);
            let pos = output.shapes.iter().find_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) if t.galley.text() == label => Some(t.pos + t.galley.size() * 0.5),
                _ => None,
            }).unwrap_or_else(|| panic!("button {label} not rendered"));
            frame(ui, ctx, vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton {
                pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE,
            }]);
            frame(ui, ctx, vec![egui::Event::PointerButton {
                pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE,
            }]);
        }
        let sb = Sandbox::new();
        unit_stats_fixture(&sb);
        let mut ui = ModifierUi { model: sb.model(), ..Default::default() };
        let mut draft = UnitStatsDraft::load(&ui.model).unwrap();
        draft.rows[0].cells[0] = "999".into();
        ui.unit_stats_draft = Some(draft);
        ui.preset_file_picker = Some(Box::new(|_| None));
        let ctx = egui::Context::default();
        for _ in 0..3 { frame(&mut ui, &ctx, vec![]); }
        click(&mut ui, &ctx, "載入屬性檔 (匯入)");
        assert_eq!(ui.unit_stats_draft.as_ref().unwrap().rows[0].cells[0], "999");
        assert!(ui.unit_stats_draft.as_ref().unwrap().preset_status.is_none());
        click(&mut ui, &ctx, "儲存屬性檔 (匯出)");
        assert_eq!(ui.unit_stats_draft.as_ref().unwrap().rows[0].cells[0], "999");
        assert!(ui.unit_stats_draft.as_ref().unwrap().preset_status.is_none());
    }

    #[test]
    fn artroop_file_picker_export_validates_inputs_before_dialog() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        fn frame(ui: &mut ModifierUi, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
            ctx.run(egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1200.0))),
                events, ..Default::default()
            }, |ctx| ui.show_unit_stats(ctx))
        }
        fn click(ui: &mut ModifierUi, ctx: &egui::Context, label: &str) {
            let output = frame(ui, ctx, vec![]);
            let pos = output.shapes.iter().find_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) if t.galley.text() == label => Some(t.pos + t.galley.size() * 0.5),
                _ => None,
            }).unwrap_or_else(|| panic!("button {label} not rendered"));
            frame(ui, ctx, vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton {
                pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE,
            }]);
            frame(ui, ctx, vec![egui::Event::PointerButton {
                pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE,
            }]);
        }
        let sb = Sandbox::new();
        unit_stats_fixture(&sb);
        let mut ui = ModifierUi { model: sb.model(), ..Default::default() };
        let mut draft = UnitStatsDraft::load(&ui.model).unwrap();
        draft.rows[0].cells[0] = "-1".into();
        ui.unit_stats_draft = Some(draft);
        let picker_called = Arc::new(AtomicBool::new(false));
        let flag = picker_called.clone();
        ui.preset_file_picker = Some(Box::new(move |_| {
            flag.store(true, Ordering::SeqCst);
            None
        }));
        let ctx = egui::Context::default();
        for _ in 0..3 { frame(&mut ui, &ctx, vec![]); }
        click(&mut ui, &ctx, "儲存屬性檔 (匯出)");
        assert!(!picker_called.load(Ordering::SeqCst), "file dialog must NOT be opened when grid inputs are invalid");
        assert!(ui.unit_stats_draft.as_ref().unwrap().preset_status.as_ref().is_some_and(|(msg, err)| *err && msg.contains("數值驗證失敗")));
    }

    #[test]
    fn artroop_file_picker_english_localization() {
        fn frame(ui: &mut ModifierUi, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
            ctx.run(egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1200.0))),
                events, ..Default::default()
            }, |ctx| ui.show_unit_stats(ctx))
        }
        fn click(ui: &mut ModifierUi, ctx: &egui::Context, label: &str) {
            let output = frame(ui, ctx, vec![]);
            let pos = output.shapes.iter().find_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) if t.galley.text() == label => Some(t.pos + t.galley.size() * 0.5),
                _ => None,
            }).unwrap_or_else(|| panic!("button {label} not rendered"));
            frame(ui, ctx, vec![egui::Event::PointerMoved(pos), egui::Event::PointerButton {
                pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE,
            }]);
            frame(ui, ctx, vec![egui::Event::PointerButton {
                pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE,
            }]);
        }
        let sb = Sandbox::new();
        unit_stats_fixture(&sb);
        let mut ui = ModifierUi { model: sb.model(), ..Default::default() };
        ui.model.english = true;
        let draft = UnitStatsDraft::load(&ui.model).unwrap();
        let path = sb.0.join("english_test.artroop");
        fs::write(&path, format!("{}=555,2,3,4,5,6", draft.rows[0].key)).unwrap();
        let export_path = sb.0.join("english_export.artroop");
        let import_path = path.clone();
        let export_target = export_path.clone();
        ui.preset_file_picker = Some(Box::new(move |kind| match kind {
            FilePickerKind::ImportPreset => Some(import_path.clone()),
            FilePickerKind::ExportPreset => Some(export_target.clone()),
        }));
        ui.model.take_settings_dirty();
        let profile = ui.model.profile_json_text();
        ui.unit_stats_draft = Some(draft);
        let ctx = egui::Context::default();
        for _ in 0..3 { frame(&mut ui, &ctx, vec![]); }
        click(&mut ui, &ctx, "Load Stats File (Import)");
        assert_eq!(ui.unit_stats_draft.as_ref().unwrap().rows[0].cells[0], "555");
        assert_eq!(ui.unit_stats_draft.as_ref().unwrap().loaded_file_name, "english_test.artroop");
        click(&mut ui, &ctx, "Save Stats File (Export)");
        assert_eq!(ui.unit_stats_draft.as_ref().unwrap().loaded_file_name, "english_export.artroop");
        assert!(fs::read_to_string(export_path).unwrap().contains("# Format: UnitKey=HP,Dmg,VW,AW,Sight,Relt"));
        assert_eq!(ui.model.profile_json_text(), profile);
        assert!(!ui.model.take_settings_dirty());
    }

    #[test]
    fn custom_stats_window_renders_draft_without_applying() {
        let sb = Sandbox::new();
        unit_stats_fixture(&sb);
        let mut ui = ModifierUi { model: sb.model(), open: true, ..ModifierUi::default() };
        ui.model.take_settings_dirty();
        ui.unit_stats_draft = Some(UnitStatsDraft::load(&ui.model).unwrap());
        let before = ui.model.profile_json_text();
        let tree = sb.tree();
        let ctx = egui::Context::default();
        for _ in 0..2 { let _ = ctx.run(Default::default(), |ctx| ui.show(ctx)); }
        assert!(ui.unit_stats_draft.is_some());
        assert_eq!(ui.model.profile_json_text(), before);
        assert_eq!(sb.tree(), tree);
        assert!(!sb.0.join("state").exists());
    }

    #[test]
    fn selection_validation_and_state_roundtrip() {
        let sb = Sandbox::new();
        let mut m = sb.model();
        assert!(m.validate().iter().all(|i| i.severity != Severity::Error));
        assert!(m.set_feature("NoSuchFeature", true).is_err());
        assert!(m.set_feature("GameSpeed", true).is_err(), "specialized features need their own control");
        assert!(m.set_game_speed(0).is_err() && m.set_game_speed(11).is_err() && m.set_quota(11).is_err());
        m.set_feature("fastciviproduction", true).unwrap();
        m.set_feature("ArgmTrace", true).unwrap();
        m.set_game_speed(4).unwrap();
        m.set_quota(3).unwrap();
        assert!(m.validate().iter().any(|i| i.severity == Severity::Warning && i.message.contains("ArgmTrace")));
        m.set_game_path("Z:/definitely/missing");
        assert!(m.validate().iter().any(|i| i.severity == Severity::Error));
        m.set_game_path(sb.game().to_string_lossy().into_owned());
        m.snapshot_dir = None;
        m.state_dir = None;
        assert!(m.validate().iter().any(|i| i.message.contains("快照")), "no snapshot dir blocks writing");
        m.state_dir = Some(sb.0.join("state"));

        m.save_settings().unwrap();
        let reloaded = ModifierUi::persisted(sb.0.join("state")).model;
        assert_eq!(reloaded.game_path, m.game_path);
        assert!(reloaded.feature_enabled("FastCiviProduction") && reloaded.feature_enabled("ArgmTrace"));
        assert_eq!((reloaded.game_speed(), reloaded.quota()), (4, 3));

        // Corrupt state is reported and falls back to defaults.
        fs::write(sb.0.join("state").join(STATE_FILE), b"{ not json").unwrap();
        let ui = ModifierUi::persisted(sb.0.join("state"));
        assert!(ui.status.as_ref().is_some_and(|(_, err)| *err));
        assert!(ui.model.enabled_ids().is_empty());

        m.clear_selection();
        assert!(m.enabled_ids().is_empty() && m.game_speed() == 1 && m.quota() == 1);
    }

    #[test]
    fn enable_all_skips_exclusions_and_preserves_multipliers_and_custom_stats() {
        let sb = Sandbox::new();
        let mut m = sb.model();
        m.set_game_speed(4).unwrap();
        m.set_quota(3).unwrap();
        let tree = sb.tree();
        let n = m.enable_all();
        let toggles: Vec<_> = FeatureRegistry::toggle_features().collect();
        assert_eq!(n, toggles.len() - ENABLE_ALL_EXCLUDED.len());
        for id in ENABLE_ALL_EXCLUDED {
            assert!(toggles.iter().any(|f| f.id == *id), "unknown exclusion {id}");
        }
        for f in &toggles {
            assert_eq!(m.feature_enabled(f.id), !ENABLE_ALL_EXCLUDED.contains(&f.id), "{}", f.id);
        }
        assert_eq!((m.game_speed(), m.quota()), (4, 3));
        assert!(m.validate().iter().all(|i| i.severity != Severity::Warning), "no risky feature enabled");
        assert_eq!(sb.tree(), tree, "enable all never writes game files");

        // Idempotent, and an explicitly enabled excluded feature is left alone.
        m.set_feature("ArgmTrace", true).unwrap();
        assert_eq!(m.enable_all(), n);
        assert!(m.feature_enabled("ArgmTrace"));
        m.clear_selection();
        assert!(m.enabled_ids().is_empty());
    }

    #[test]
    fn profiles_persist_validate_and_reject_bad_input() {
        let sb = Sandbox::new();
        let mut m = sb.model();
        m.set_feature("InfiniteMorale", true).unwrap();
        m.set_game_speed(2).unwrap();
        for bad in ["", "../evil", "a/b", "con", "x.json", &"n".repeat(49)] {
            assert!(m.save_profile(bad).is_err(), "{bad:?}");
        }
        m.save_profile("my_profile").unwrap();
        assert_eq!(m.list_profiles(), ["my_profile"]);
        m.clear_selection();
        m.load_profile("my_profile").unwrap();
        assert!(m.feature_enabled("InfiniteMorale") && m.game_speed() == 2);

        let bad = sb.0.join("bad.json");
        for (text, needle) in [
            (r#"{"Features":{"Bogus":true}}"#, "Bogus"),
            (r#"{"GameSpeed":99}"#, "1..10"),
            (r#"{"Features":{"FocusLoss":"yes"}}"#, "boolean"),
            ("not json", "JSON"),
        ] {
            fs::write(&bad, text).unwrap();
            let before = m.profile_json_text();
            let err = m.import_profile(&bad).unwrap_err();
            assert!(err.contains(needle), "{err}");
            assert_eq!(m.profile_json_text(), before, "failed import must not change the selection");
        }
        // BOM-prefixed CLI exports are accepted.
        let mut bom = vec![0xEF, 0xBB, 0xBF];
        bom.extend(br#"{"Features":{"FocusLoss":true},"GameSpeed":3}"#);
        fs::write(&bad, bom).unwrap();
        m.import_profile(&bad).unwrap();
        assert!(m.feature_enabled("FocusLoss") && m.game_speed() == 3 && !m.feature_enabled("InfiniteMorale"));
    }

    #[test]
    fn preview_is_read_only_and_matches_cli_dry_run() {
        let sb = Sandbox::new();
        let before = sb.tree();
        let mut m = sb.model();
        m.set_feature("FocusLoss", true).unwrap();
        m.set_feature("InfiniteMorale", true).unwrap();
        m.preview_apply().unwrap();
        assert_eq!(sb.tree(), before, "preview must not touch the game directory (no .bak, no locks)");
        assert!(!sb.0.join("state").join("snapshots").exists());
        let p = m.preview.as_ref().unwrap();
        assert!(p.can_commit() && p.blockers.is_empty());
        assert!(p.feature_changes.contains(&"+FocusLoss".to_string()));
        let exe = p.entries.iter().find(|e| e.path.ends_with("Against_Rome.exe")).unwrap();
        assert_eq!((exe.kind, exe.changed_bytes), (ChangeKind::Modify, Some(FOCUS.original.len())));

        let game = sb.game().to_string_lossy().into_owned();
        let (code, v) = cli(&["apply", "--game", &game, "--enable", "FocusLoss,InfiniteMorale", "--dry-run", "--json"]);
        assert_eq!(code, 0, "{v}");
        assert_eq!(v["data"]["plannedFiles"].as_u64().unwrap() as usize, p.planned_files);
        let mut cli_enabled: Vec<_> = v["data"]["enabledFeatures"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_owned()).collect();
        let mut ui_enabled = p.enabled_features.clone();
        cli_enabled.sort();
        ui_enabled.sort();
        assert_eq!(ui_enabled, cli_enabled);
        assert_eq!(sb.tree(), before);

        // Editing the selection invalidates the reviewed plan.
        m.set_feature("InfiniteMorale", false).unwrap();
        assert!(m.preview.is_none());
        assert!(m.commit_preview().unwrap_err().contains("預覽"));
    }

    #[test]
    fn exported_profile_roundtrips_through_cli() {
        let sb = Sandbox::new();
        let mut m = sb.model();
        m.set_feature("FastCiviProduction", true).unwrap();
        m.set_game_speed(2).unwrap();
        m.preview_apply().unwrap();
        let ui_plan = m.preview.as_ref().unwrap().planned_files;
        let file = sb.0.join("exported.json");
        m.export_profile(&file).unwrap();
        let (code, v) = cli(&["apply", "--game", &sb.game().to_string_lossy(), "--profile", &file.to_string_lossy(), "--dry-run", "--json"]);
        assert_eq!(code, 0, "{v}");
        assert_eq!(v["data"]["plannedFiles"].as_u64().unwrap() as usize, ui_plan);
        assert_eq!(v["data"]["gameSpeed"], 2);
        assert_eq!(v["data"]["enabledFeatures"], json!(["FastCiviProduction"]));
    }

    #[test]
    fn apply_status_undo_and_restore_roundtrip_via_cli() {
        let sb = Sandbox::new();
        let original = sb.tree();
        let mut m = sb.model();
        m.set_feature("FocusLoss", true).unwrap();
        m.preview_apply().unwrap();
        let report = m.commit_preview().unwrap();
        assert!(report.written > 0);
        assert!(m.preview.is_none(), "preview is consumed by a commit");
        assert!(m.commit_preview().is_err(), "cannot commit twice from one preview");

        let after = sb.tree();
        assert_ne!(after[&sb.game().join("Against_Rome.exe")], original[&sb.game().join("Against_Rome.exe")]);
        let game = sb.game().to_string_lossy().into_owned();
        let (code, status) = cli(&["status", "--game", &game, "--json"]);
        assert_eq!(code, 0, "{status}");
        assert_eq!(status["data"]["features"]["FocusLoss"], true, "CLI sees the UI-applied feature");

        // Re-planning the same selection finds nothing left to do.
        m.preview_apply().unwrap();
        assert_eq!(m.preview.as_ref().unwrap().changed_count(), 0);
        assert!(m.commit_preview().is_err());

        // Snapshot listing + undo restores the pre-apply bytes exactly.
        let snaps = m.list_snapshots();
        assert_eq!(snaps.len(), 1);
        assert_eq!((snaps[0].id.as_str(), snaps[0].state.as_str()), (report.snapshot_id.as_str(), "committed"));
        m.preview_undo(&report.snapshot_id).unwrap();
        assert!(m.preview.as_ref().unwrap().can_commit());
        m.commit_preview().unwrap();
        let undone = sb.tree();
        for (path, bytes) in &original {
            assert_eq!(&undone[path], bytes, "{} not restored", path.display());
        }
        let (_, status) = cli(&["status", "--game", &game, "--json"]);
        assert_eq!(status["data"]["features"]["FocusLoss"], false);

        // Restore-original is also previewed and snapshotted; the scope decides which files are touched.
        m.preview_apply().unwrap();
        m.commit_preview().unwrap();
        m.restore_scope = RestoreScope::Stats;
        m.preview_restore_original().unwrap();
        assert!(m.preview.as_ref().unwrap().entries.iter().all(|e| !e.path.ends_with("Against_Rome.exe")));
        m.restore_scope = RestoreScope::Compat;
        m.preview_restore_original().unwrap();
        assert!(m.preview.as_ref().unwrap().entries.iter().any(|e| e.path.ends_with("Against_Rome.exe")));
        m.commit_preview().unwrap();
        assert_eq!(sb.tree()[&sb.game().join("Against_Rome.exe")], original[&sb.game().join("Against_Rome.exe")]);
        let (_, status) = cli(&["status", "--game", &game, "--json"]);
        assert_eq!(status["data"]["features"]["FocusLoss"], false);
        assert_eq!(m.list_snapshots().len(), 4);
    }

    #[test]
    fn stale_preview_external_edits_and_tampered_snapshots_are_refused() {
        let sb = Sandbox::new();
        let mut m = sb.model();
        m.set_feature("FocusLoss", true).unwrap();
        m.preview_apply().unwrap();
        // The game files change between preview and commit.
        sb.put("SYSTEM/ress.ini", b"[objres]\r\nEXTERNAL=1\r\n");
        let before = sb.tree();
        assert!(m.commit_preview().unwrap_err().contains("重新預覽"));
        assert_eq!(sb.tree(), before, "refused commit must not write");
        assert!(m.list_snapshots().is_empty(), "no snapshot for a refused commit");
        sb.put("SYSTEM/ress.ini", b"[objres]\r\n");

        m.preview_apply().unwrap();
        let id = m.commit_preview().unwrap().snapshot_id;
        // An external edit after apply blocks undo instead of being overwritten.
        sb.put("Against_Rome.exe", b"user edited this\r\n");
        m.preview_undo(&id).unwrap();
        let p = m.preview.as_ref().unwrap();
        assert!(!p.can_commit() && p.blockers.iter().any(|b| b.contains("Against_Rome.exe")));
        let before = sb.tree();
        assert!(m.commit_preview().unwrap_err().contains("阻擋"));
        assert_eq!(sb.tree(), before);

        // Tampered manifests / ids never escape the snapshot directory.
        let root = m.snapshot_root().unwrap();
        assert!(m.preview_undo("..").is_err() && m.preview_undo("a/b").is_err());
        let manifest = root.join(&id).join("manifest.json");
        let text = fs::read_to_string(&manifest).unwrap().replacen("SYSTEM/", "../SYSTEM/", 1);
        fs::write(&manifest, text).unwrap();
        assert!(m.preview_undo(&id).unwrap_err().contains("unsafe"));
        assert!(m.list_snapshots().iter().all(|s| s.id != id), "unreadable snapshots are not listed as usable");
    }

    #[test]
    fn corrupted_snapshot_blob_blocks_undo() {
        let sb = Sandbox::new();
        let mut m = sb.model();
        m.set_feature("FastCiviProduction", true).unwrap();
        m.preview_apply().unwrap();
        let id = m.commit_preview().unwrap().snapshot_id;
        let files = m.snapshot_root().unwrap().join(&id).join("files");
        for e in fs::read_dir(&files).unwrap().flatten() {
            fs::write(e.path(), b"garbage").unwrap();
        }
        m.preview_undo(&id).unwrap();
        let p = m.preview.as_ref().unwrap();
        assert!(!p.can_commit() && p.blockers.iter().any(|b| b.contains("損毀")));
    }

    #[test]
    fn missing_prerequisites_are_reported_without_side_effects() {
        let sb = Sandbox::new();
        let mut m = sb.model();
        m.set_game_path("");
        assert!(m.preview_apply().unwrap_err().contains("遊戲目錄"));
        assert!(m.preview_restore_original().is_err());
        m.set_game_path(sb.game().to_string_lossy().into_owned());
        m.set_feature("FocusLoss", true).unwrap();
        m.state_dir = None;
        assert!(m.preview_apply().unwrap_err().contains("快照"), "writing needs a restore point location");
        assert!(m.commit_preview().is_err());
        assert!(!sb.0.join("state").exists());
    }

    fn restore_frame(ui: &mut ModifierUi, ctx: &egui::Context, events: Vec<egui::Event>) -> (egui::FullOutput, Option<Action>) {
        let mut action = None;
        let out = ctx.run(
            egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1200.0))), events, ..Default::default() },
            |ctx| action = ui.show_restore_dialog(ctx),
        );
        (out, action)
    }
    /// Clicks the button/radio whose text is `label`; returns the action emitted on release.
    fn restore_click(ui: &mut ModifierUi, ctx: &egui::Context, label: &str) -> Option<Action> {
        restore_frame(ui, ctx, vec![]);
        let (out, _) = restore_frame(ui, ctx, vec![]);
        let pos = out.shapes.iter().find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.text() == label => Some(t.pos + t.galley.size() * 0.5),
            _ => None,
        }).unwrap_or_else(|| panic!("{label} not rendered"));
        let btn = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        restore_frame(ui, ctx, vec![egui::Event::PointerMoved(pos), btn(true)]);
        restore_frame(ui, ctx, vec![btn(false)]).1
    }

    #[test]
    fn restore_dialog_defaults_cancel_and_confirm_semantics() {
        let sb = Sandbox::new();
        let mut ui = ModifierUi { model: sb.model(), ..Default::default() };
        ui.model.set_feature("FocusLoss", true).unwrap();
        ui.model.take_settings_dirty();
        let (profile, tree) = (ui.model.profile_json_text(), sb.tree());
        let ctx = egui::Context::default();

        assert!(restore_frame(&mut ui, &ctx, vec![]).1.is_none() && ui.restore_dialog.is_none(), "closed dialog renders nothing");
        ui.run(Action::OpenRestore);
        assert_eq!(ui.restore_dialog, Some((RestoreScope::All, true)), "C# defaults: restore all, keep custom maps");

        // Choosing a different category/uncheck then cancelling leaves model, disk and settings untouched.
        restore_click(&mut ui, &ctx, "僅數值 (Stats)");
        assert_eq!(ui.restore_dialog, Some((RestoreScope::Stats, true)));
        assert!(restore_click(&mut ui, &ctx, "取消").is_none());
        assert!(ui.restore_dialog.is_none());
        assert_eq!((ui.model.restore_scope, ui.model.preserve_custom_maps), (RestoreScope::All, true));
        assert_eq!(ui.model.profile_json_text(), profile);
        assert!(ui.model.preview.is_none() && !ui.model.take_settings_dirty());
        assert_eq!(sb.tree(), tree);
        assert!(!sb.0.join("state").exists());

        // Confirm hands the draft to the model and only plans a preview.
        ui.run(Action::OpenRestore);
        restore_click(&mut ui, &ctx, "僅相容性 (Compat)");
        let Some(Action::PreviewRestore(scope, preserve)) = restore_click(&mut ui, &ctx, "還原") else { panic!("confirm emits PreviewRestore") };
        assert_eq!((scope, preserve), (RestoreScope::Compat, true));
        assert!(ui.restore_dialog.is_none());
        ui.run(Action::PreviewRestore(scope, preserve));
        assert_eq!(ui.model.restore_scope, RestoreScope::Compat);
        assert!(ui.model.preview.as_ref().is_some_and(|p| p.kind == PreviewKind::RestoreOriginal(RestoreScope::Compat)));
        assert_eq!(sb.tree(), tree, "preview never writes");

        // Closing with the window X is also a cancel.
        ui.run(Action::OpenRestore);
        ui.restore_dialog = None;
        assert_eq!(sb.tree(), tree);
    }

    #[test]
    fn restore_commit_resets_matching_toggles_and_failed_commit_keeps_them() {
        let sb = Sandbox::new();
        let mut m = sb.model();
        m.set_feature("FocusLoss", true).unwrap();
        m.preview_apply().unwrap();
        m.commit_preview().unwrap();
        // Multipliers only live in the selection here (the fixture has no Endless-AI sites to patch).
        m.set_game_speed(5).unwrap();
        m.set_quota(4).unwrap();

        // Language scope does not touch Compat state.
        m.set_restore_options(RestoreScope::Language, true);
        m.preview_restore_original().unwrap();
        let _ = m.commit_preview();
        assert!(m.feature_enabled("FocusLoss") && m.game_speed() == 5 && m.quota() == 4);

        // A stale preview fails closed: files are untouched and the selection is not reset.
        m.set_restore_options(RestoreScope::Compat, true);
        m.preview_restore_original().unwrap();
        let exe = sb.game().join("Against_Rome.exe");
        let mut bytes = fs::read(&exe).unwrap();
        bytes.push(0);
        fs::write(&exe, &bytes).unwrap();
        let tree = sb.tree();
        assert!(m.commit_preview().is_err());
        assert_eq!(sb.tree(), tree);
        assert!(m.feature_enabled("FocusLoss") && m.game_speed() == 5 && m.quota() == 4);
        fs::write(&exe, &bytes[..bytes.len() - 1]).unwrap();

        m.set_restore_options(RestoreScope::Compat, true);
        m.preview_restore_original().unwrap();
        m.commit_preview().unwrap();
        assert!(!m.feature_enabled("FocusLoss"));
        assert_eq!((m.game_speed(), m.quota()), (1, 1));
        assert!(m.take_settings_dirty(), "reset selection is persisted");
        assert_eq!(m.list_snapshots().len(), 2, "restore commits stay snapshotted via commit_plan transaction");
    }

    #[test]
    fn window_renders_all_states_and_persists_on_change() {
        let sb = Sandbox::new();
        let mut ui = ModifierUi::persisted(sb.0.join("state"));
        ui.model.backup_base = sb.0.join("none");
        ui.open = true;
        let ctx = egui::Context::default();
        let frame = |ui: &mut ModifierUi| {
            let _ = ctx.run(Default::default(), |ctx| ui.show(ctx));
        };
        frame(&mut ui);
        ui.model.set_game_path(sb.game().to_string_lossy().into_owned());
        ui.model.set_feature("FocusLoss", true).unwrap();
        ui.category = Some(FeatureCategory::Compat);
        ui.search = "speed".into();
        frame(&mut ui);
        assert!(sb.0.join("state").join(STATE_FILE).is_file(), "selection changes are persisted when the window runs");
        ui.run(Action::PreviewApply);
        assert!(ui.status.as_ref().is_some_and(|(_, err)| !*err));
        frame(&mut ui);
        ui.run(Action::Commit);
        assert!(ui.status.as_ref().is_some_and(|(m, err)| !*err && m.contains("完成")), "{:?}", ui.status);
        assert_eq!(ui.snapshots_stale, true);
        frame(&mut ui);
        assert_eq!(ui.snapshots.len(), 1);
        let id = ui.snapshots[0].id.clone();
        ui.run(Action::PreviewUndo(id));
        frame(&mut ui);
        ui.run(Action::Commit);
        assert!(ui.status.as_ref().is_some_and(|(_, err)| !*err));
        let fresh = ModifierUi::persisted(sb.0.join("state"));
        assert!(fresh.model.feature_enabled("FocusLoss"), "selection survives a restart");
    }

    // ---- parity: Start Game / Experimental promote / Balanced template

    #[test]
    fn start_game_missing_exe_or_dir_errors_without_spawning() {
        use std::sync::atomic::AtomicBool;
        let spawned = AtomicBool::new(false);
        let fake = |_: &Path, _: &Path| -> io::Result<()> {
            spawned.store(true, Ordering::SeqCst);
            Ok(())
        };
        let sb = Sandbox::new();
        let mut m = sb.model();
        m.set_game_path("");
        assert_eq!(m.start_game_with(&fake).unwrap_err(), m.loc().get("MsgSelectGameDir"));
        m.set_game_path(sb.0.join("does-not-exist").to_string_lossy().into_owned());
        assert_eq!(m.start_game_with(&fake).unwrap_err(), m.loc().get("MsgSelectGameDir"));
        fs::remove_file(sb.game().join("Against_Rome.exe")).unwrap();
        m.set_game_path(sb.game().to_string_lossy().into_owned());
        assert_eq!(m.start_game_with(&fake).unwrap_err(), m.loc().get("MsgExeNotFound"));
        assert!(!spawned.load(Ordering::SeqCst), "launcher must not run when the exe is missing");
        // The real spawner reports a missing exe as an io error rather than panicking.
        assert!(spawn_game_process(&sb.game().join("Against_Rome.exe"), &sb.game()).is_err());
    }

    #[test]
    fn start_game_passes_exe_and_game_dir_to_launcher_and_reports_failure() {
        use std::sync::{Arc, Mutex};
        let sb = Sandbox::new();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let rec = calls.clone();
        let mut ui = ModifierUi::default().with_game_launcher(move |exe: &Path, dir: &Path| {
            rec.lock().unwrap().push((exe.to_path_buf(), dir.to_path_buf()));
            Ok(())
        });
        ui.model = sb.model();
        ui.start_game().unwrap();
        assert_eq!(*calls.lock().unwrap(), vec![(sb.game().join("Against_Rome.exe"), sb.game())]);
        assert_eq!(ui.model.log.last().unwrap(), ui.model.loc().get("LogGameStarted"));

        let mut failing = ModifierUi::default().with_game_launcher(|_: &Path, _: &Path| Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied")));
        failing.model = sb.model();
        let err = failing.start_game().unwrap_err();
        assert!(err.starts_with(failing.model.loc().get("MsgLaunchFailed")) && err.contains("denied"), "{err}");
        assert_eq!(failing.model.log.last().unwrap(), &err);
    }

    #[test]
    fn experimental_promote_demote_persists_through_loc_settings() {
        let sb = Sandbox::new();
        let mut m = sb.model();
        for id in EXPERIMENTAL_FEATURES {
            assert!(FeatureRegistry::get(id).is_some(), "{id} must be a real feature id");
            assert!(!m.is_promoted(id));
        }
        assert!(m.toggle_promotion("Balance").unwrap());
        assert!(m.is_promoted("balance"));
        assert!(m.toggle_promotion("ArgmTrace").unwrap());
        assert!(m.toggle_promotion("FocusLoss").is_err(), "non-experimental features cannot be promoted");
        assert!(m.toggle_promotion("Nope").is_err());
        let reloaded = sb.model();
        assert!(reloaded.is_promoted("Balance") && reloaded.is_promoted("ArgmTrace") && !reloaded.is_promoted("GeneralSkills"));
        assert!(!m.toggle_promotion("Balance").unwrap());
        assert!(!sb.model().is_promoted("Balance") && sb.model().is_promoted("ArgmTrace"));
        assert!(m.log.iter().any(|l| l.contains("Balance") && l.contains("移回實驗性")));
        assert!(m.log.iter().any(|l| l.contains("ArgmTrace") && l.contains("移至主控制台")));
    }

    #[test]
    fn balanced_template_replaces_all_custom_stats_with_normalized_balanced_values() {
        let sb = Sandbox::new();
        unit_stats_fixture(&sb);
        let mut m = sb.model();
        let key = "FigRomInf00_Lanze_Schild";
        m.profile.custom_unit_stats = Some([(key.to_lowercase(), vec![1.0; 9]), ("stale_unit".into(), vec![2.0; 9])].into());
        m.preview_apply().unwrap();
        m.take_settings_dirty();
        let before_tree = sb.tree();
        let n = m.apply_balanced_template().unwrap();
        assert_eq!(n, TroopConfig::sorted_units().len());
        assert_eq!(sb.tree(), before_tree, "template only edits the in-memory profile");
        assert!(m.preview.is_none() && m.take_settings_dirty());
        assert_eq!(m.preset_label, m.loc().get("TroopTemplateBalanced"));
        assert!(m.log.last().unwrap().contains("內建平衡"));
        let stats = m.profile.custom_unit_stats.clone().unwrap();
        assert_eq!(stats.len(), n);
        assert!(!stats.contains_key(&key.to_lowercase()) && !stats.contains_key("stale_unit"));
        let backup = m.load_backup(&sb.game(), false).unwrap();
        for unit in TroopConfig::sorted_units() {
            let balanced = backup.get_default_balanced_stats(unit).unwrap();
            let original = backup.get_original_stats(unit).unwrap();
            let got = &stats[unit];
            for i in 0..9 {
                let expect = if [4, 7, 8].contains(&i) || (i == 5 && TroopConfig::get_metadata(unit).unwrap().unit_type == "priest") { original[i] } else { balanced[i] };
                assert_eq!(got[i], expect, "{unit}[{i}]");
            }
        }
        // English mode logs the English message and the failure path changes nothing.
        let mut bad = sb.model();
        bad.english = true;
        bad.profile.custom_unit_stats = Some([("keep".into(), vec![3.0; 9])].into());
        bad.set_game_path(sb.0.join("missing").to_string_lossy().into_owned());
        assert!(bad.apply_balanced_template().is_err());
        assert_eq!(bad.profile.custom_unit_stats.as_ref().unwrap().len(), 1);
        assert!(bad.preset_label.is_empty());
        bad.set_game_path(sb.game().to_string_lossy().into_owned());
        bad.apply_balanced_template().unwrap();
        assert_eq!(bad.preset_label, "Balanced Base Stats");
        assert_eq!(bad.log.last().unwrap(), "Applied built-in balanced troop stats template.");
    }

    #[test]
    fn unit_icons_degrade_gracefully_with_csharp_log_messages() {
        let sb = Sandbox::new();
        let mut m = sb.model();
        let icons = m.load_unit_icons();
        assert!(icons.is_empty());
        assert_eq!(m.log.last().unwrap(), m.loc().get("LogGuiDatNotFound"));
        sb.put("gui.dat", b"not a zip");
        let icons = m.load_unit_icons();
        assert!(icons.is_empty());
        let last = m.log.last().unwrap();
        assert!(last.starts_with(m.loc().get("LogLoadIconFailed")) || last == m.loc().get("LogIconIniNotFound"), "{last}");
        let mut none = ModifierModel::default();
        none.load_unit_icons();
        assert_eq!(none.log.last().unwrap(), none.loc().get("LogGamePathNotSetIcon"));
    }
}

#[cfg(test)]
mod startup_migration_tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("arm-modifier-startup-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn missing_or_invalid_game_path_is_a_noop_with_error_status() {
        let dir = temp("invalid");
        let mut ui = ModifierUi::persisted(dir.join("state"));
        ui.model.backup_base = dir.join("no-backup-zip-here");
        ui.run_startup_migrations();
        assert!(ui.status.as_ref().is_some_and(|(m, err)| *err && m.contains("啟動安全遷移失敗")), "{:?}", ui.status);

        // A directory without Against_Rome.exe is not a game install: nothing may be written into it.
        let fake = dir.join("fake");
        fs::create_dir_all(&fake).unwrap();
        let mut m = ModifierModel::with_state_dir(dir.join("state"));
        m.backup_base = dir.join("no-backup-zip-here");
        m.set_game_path(fake.to_string_lossy().into_owned());
        assert!(m.run_startup_migrations().is_err());
        assert_eq!(fs::read_dir(&fake).unwrap().count(), 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn startup_migrations_run_once_per_ui_and_are_idempotent_on_disk() {
        let dir = temp("idem");
        let game = dir.join("game");
        fs::create_dir_all(&game).unwrap();
        fs::write(game.join("Against_Rome.exe"), b"MZ").unwrap();
        let mut m = ModifierModel::with_state_dir(dir.join("state"));
        m.backup_base = dir.join("no-backup-zip-here");
        m.set_game_path(game.to_string_lossy().into_owned());
        // No retail BCI/backup baseline in the synthetic dir: must fail closed or be a no-op, never write.
        let before = fs::read_dir(&game).unwrap().count();
        let first = m.run_startup_migrations();
        let second = m.run_startup_migrations();
        assert_eq!(first.is_ok(), second.is_ok());
        assert!(matches!(first, Ok(0) | Err(_)));
        assert_eq!(fs::read_dir(&game).unwrap().count(), before, "{first:?}");

        let mut ui = ModifierUi { model: m, ..ModifierUi::default() };
        ui.run_startup_migrations();
        let status = ui.status.clone();
        ui.status = None;
        ui.run_startup_migrations();
        assert!(ui.startup_migrated && ui.status.is_none(), "second call must not re-run (was {status:?})");
        let _ = fs::remove_dir_all(&dir);
    }
}
