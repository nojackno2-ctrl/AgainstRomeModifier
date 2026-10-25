//! Modifier command line contract, ported from src.Modifier/Cli.
use crate::{FeatureControlKind, FeatureRegistry, PatchProfile};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CliOptions {
    pub command: String,
    pub sub_action: Option<String>,
    pub sub_arg: Option<String>,
    pub sub_target: Option<String>,
    pub game_path: Option<String>,
    pub json: bool,
    pub verbose: bool,
    pub enable: Vec<String>,
    pub disable: Vec<String>,
    pub enable_all: bool,
    pub disable_all: bool,
    pub balance: Option<bool>,
    pub game_speed: Option<i32>,
    pub quota: Option<i32>,
    pub preset: Option<String>,
    pub profile: Option<String>,
    pub dry_run: bool,
    pub restore_all: bool,
    pub restore_stats: bool,
    pub restore_compat: bool,
    pub compat: bool,
    pub restore_language: bool,
    pub preserve_custom_maps: bool,
    pub output: Option<String>,
    pub format: Option<String>,
    pub category: Option<String>,
    pub language: Option<String>,
    pub original_path: Option<String>,
}
fn canonical_command(s: &str) -> Option<&'static str> {
    Some(match s.to_ascii_lowercase().as_str() {
        "detect-game" | "game" => "detect-game",
        "status" | "detect" | "info" => "status",
        "features" | "list" => "features",
        "apply" => "apply",
        "restore" => "restore",
        "backup" => "backup",
        "stats" => "stats",
        "profile" => "profile",
        "saves" => "saves",
        "maps" => "maps",
        "repair-language-backup" | "repair-language" => "repair-language-backup",
        "help" => "help",
        _ => return None,
    })
}
impl CliOptions {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut o = Self {
            command: "help".into(),
            preserve_custom_maps: true,
            ..Self::default()
        };
        let mut pos = Vec::new();
        let mut i = 0;
        while i < args.len() {
            let arg = args[i].to_ascii_lowercase();
            let mut value = || -> Result<String, String> {
                i += 1;
                args.get(i)
                    .cloned()
                    .ok_or_else(|| format!("{} 缺少參數。", arg))
            };
            match arg.as_str() {
                "--cli" => {}
                "--json" => o.json = true,
                "--verbose" | "-v" => o.verbose = true,
                "--game" | "-g" => o.game_path = Some(value()?),
                "--enable" | "--disable" => {
                    let v = value()?;
                    let target = if arg == "--enable" {
                        &mut o.enable
                    } else {
                        &mut o.disable
                    };
                    target.extend(
                        v.split([',', ';'])
                            .filter(|x| !x.is_empty())
                            .map(|x| x.trim().into()),
                    );
                }
                "--all" => {
                    o.enable_all = true;
                    o.restore_all = true;
                }
                "--disable-all" => o.disable_all = true,
                "--balance" | "--preserve-custom-maps" => {
                    let mut b = true;
                    if let Some(v) = args.get(i + 1) {
                        if v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("false") {
                            b = v.eq_ignore_ascii_case("true");
                            i += 1;
                        }
                    }
                    if arg == "--balance" {
                        o.balance = Some(b)
                    } else {
                        o.preserve_custom_maps = b
                    }
                }
                "--game-speed" | "--speed" | "--garrison-quota" | "--quota" => {
                    let n = value()?
                        .parse::<i32>()
                        .map_err(|_| format!("{} 缺少有效的數值倍率 (1..10)。", arg))?;
                    if !(1..=10).contains(&n) {
                        return Err("倍率必須介於 1 到 10 之間。".into());
                    }
                    if arg == "--speed" || arg == "--game-speed" {
                        o.game_speed = Some(n)
                    } else {
                        o.quota = Some(n)
                    }
                }
                "--preset" => o.preset = Some(value()?),
                "--profile" => o.profile = Some(value()?),
                "--dry-run" => o.dry_run = true,
                "--stats" => o.restore_stats = true,
                "--compat" => {
                    o.compat = true;
                    o.restore_compat = true;
                }
                "--language" | "--lang-pack" => o.restore_language = true,
                "--delete-custom-maps" => o.preserve_custom_maps = false,
                "--output" | "-o" => o.output = Some(value()?),
                "--format" | "-f" => o.format = Some(value()?),
                "--category" | "-c" => o.category = Some(value()?),
                "--unit" | "-u" => o.sub_arg = Some(value()?),
                "--lang" => o.language = Some(value()?),
                "--original" | "--original-path" => o.original_path = Some(value()?),
                "--help" | "-h" | "/?" => return Ok(o),
                _ => {
                    if !args[i].starts_with('-') {
                        pos.push(args[i].clone());
                    }
                }
            }
            i += 1;
        }
        if let Some(c) = pos.first() {
            o.command = canonical_command(c)
                .ok_or_else(|| {
                    format!("未知的指令: '{c}'。使用 'AgainstRomeModifier.exe help' 檢視說明。")
                })?
                .into();
        }
        if pos.first().is_some_and(|c| c.eq_ignore_ascii_case("saves")) && pos.len() > 4 {
            return Err("saves 子指令有多餘的位置參數。".into());
        }
        o.sub_action = pos.get(1).cloned();
        if o.sub_arg.is_none() {
            o.sub_arg = pos.get(2).cloned();
        }
        o.sub_target = pos.get(3).cloned();
        Ok(o)
    }
}
pub const ENABLE_ALL_EXCLUDED: &[&str] = &[
    "Balance",
    "CameraZoomOut1",
    "CorpseRetention",
    "ArgmTrace",
    "SpellHealing10x",
    "SpellResurrection",
    "GeneralSkills",
    "LeaderGlory",
];
fn invalid(s: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, s.into())
}
use crate::cli_commands::arg_error;
fn read_json(path: &str) -> io::Result<Value> {
    let bytes = fs::read(path)?;
    serde_json::from_slice(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes))
        .map_err(|e| invalid(e.to_string()))
}
pub fn profile_to_json(p: &PatchProfile) -> Value {
    let mut features: BTreeMap<_, Value> = FeatureRegistry::toggle_features()
        .map(|f| (f.id, json!(p.get_bool(f.id))))
        .collect();
    for id in ["EndlessAi.M2", "EndlessAi.M3", "EndlessAi.M4"] {
        features.insert(id, json!(p.get_bool(id)));
    }
    features.insert(
        "VillageGarrisonQuota3x",
        json!(p.get_int("VillageGarrisonQuota3x")),
    );
    let mut endless_ai: BTreeMap<String, bool> = BTreeMap::new();
    for m in ["M1", "Core", "M2", "M3", "M4", "M5", "M6"] {
        endless_ai.insert(m.to_string(), p.get_endless_ai_module(m));
    }
    endless_ai.insert(
        "VillageGarrisonQuota3x".to_string(),
        p.get_int("VillageGarrisonQuotaMultiplier") > 1,
    );
    json!({
        "Features":features,
        "GameSpeed":p.get_int("GameSpeed"),
        "VillageGarrisonQuotaMultiplier":p.get_int("VillageGarrisonQuotaMultiplier"),
        "Balance":p.get_bool("Balance"),
        "CustomUnitStats":p.custom_unit_stats,
        "EndlessAiModules":endless_ai,
        "VillageGarrisonQuota3x":p.get_int("VillageGarrisonQuotaMultiplier")>1,
    })
}
pub fn profile_from_json(v: &Value) -> io::Result<PatchProfile> {
    let Some(obj) = v.as_object() else {
        return Err(invalid("設定檔格式無效。"));
    };
    let mut p = PatchProfile::new();
    let f_val = obj
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("Features"))
        .map(|(_, v)| v);
    if let Some(f_val) = f_val {
        let f_obj = f_val
            .as_object()
            .ok_or_else(|| invalid("Features must be an object"))?;
        for (id, value) in f_obj {
            if id.eq_ignore_ascii_case("VillageGarrisonQuota3x")
                || id.eq_ignore_ascii_case("VillageGarrisonQuotaMultiplier")
            {
                continue;
            }
            p.set_bool(
                id,
                value
                    .as_bool()
                    .ok_or_else(|| invalid("Feature values must be boolean"))?,
            );
        }
        let quota = ["VillageGarrisonQuotaMultiplier", "VillageGarrisonQuota3x"]
            .iter()
            .find_map(|id| f_obj.iter().find(|(key, _)| key.eq_ignore_ascii_case(id)));
        if let Some((id, value)) = quota {
            if let Some(b) = value.as_bool() {
                p.set_bool(id, b);
            } else {
                p.set_int(
                    id,
                    i32::try_from(
                        value
                            .as_i64()
                            .ok_or_else(|| invalid("Expected integer or boolean"))?,
                    )
                    .map_err(|_| invalid("Integer overflow"))?,
                );
            }
        }
    }
    for (key, val) in obj {
        if let Some(b) = val.as_bool() {
            if let Some(def) = FeatureRegistry::get(key) {
                if def.control_kind == FeatureControlKind::Toggle && !p.has_explicit_value(def.id) {
                    p.set_bool(def.id, b);
                }
            }
        }
    }
    let m_val = obj
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("EndlessAiModules"))
        .map(|(_, v)| v);
    if let Some(m_val) = m_val {
        if let Some(m_obj) = m_val.as_object() {
            for (id, val) in m_obj {
                if let Some(b) = val.as_bool() {
                    p.endless_ai_modules.insert(id.clone(), b);
                }
            }
        }
    }
    // 優先序：頂層 Multiplier > Features Multiplier > Features registry 鍵 > 頂層舊鍵 > 舊模組 map。
    if !p.has_explicit_value("VillageGarrisonQuota3x") {
        if let Some((_, value)) = obj
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("VillageGarrisonQuota3x"))
        {
            if let Some(b) = value.as_bool() {
                p.set_bool("VillageGarrisonQuota3x", b);
            } else {
                p.set_int(
                    "VillageGarrisonQuota3x",
                    i32::try_from(
                        value
                            .as_i64()
                            .ok_or_else(|| invalid("Expected integer or boolean"))?,
                    )
                    .map_err(|_| invalid("Integer overflow"))?,
                );
            }
        }
    }
    for id in ["GameSpeed", "VillageGarrisonQuotaMultiplier"] {
        if let Some((_, n)) = obj.iter().find(|(k, _)| k.eq_ignore_ascii_case(id)) {
            p.set_int(
                id,
                i32::try_from(n.as_i64().ok_or_else(|| invalid("Expected integer"))?)
                    .map_err(|_| invalid("Integer overflow"))?,
            );
        }
    }
    if let Some((_, b)) = obj.iter().find(|(k, _)| k.eq_ignore_ascii_case("Balance")) {
        p.set_bool(
            "Balance",
            b.as_bool().ok_or_else(|| invalid("Expected boolean"))?,
        );
    }
    if let Some((_, stats)) = obj
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("CustomUnitStats"))
    {
        if !stats.is_null() {
            p.custom_unit_stats =
                Some(serde_json::from_value(stats.clone()).map_err(|e| invalid(e.to_string()))?);
        }
    }
    p.normalize_composite_values();
    Ok(p)
}
/// Preserves the six-column legacy preset mapping, including its zero speed.
pub fn parse_preset(
    text: &str,
    fallback: impl Fn(&str) -> Vec<f64>,
) -> (BTreeMap<String, Vec<f64>>, usize) {
    let mut out = BTreeMap::new();
    let mut skipped = 0;
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with(['#', ';']) {
            continue;
        }
        let Some((key, tail)) = line.split_once('=') else {
            continue;
        };
        if key.trim().is_empty() {
            continue;
        }
        let values: Vec<_> = tail.split(',').collect();
        if values.len() < 4 {
            skipped += 1;
            continue;
        }
        let parsed: Result<Vec<f64>, _> = values
            .iter()
            .take(9)
            .map(|s| s.trim().parse::<f64>())
            .collect();
        let Ok(parsed) = parsed else {
            skipped += 1;
            continue;
        };
        let mut a = vec![0.; 9];
        a[..4].copy_from_slice(&parsed[..4]);
        if values.len() == 6 {
            a[5] = parsed[4];
            a[6] = parsed[5];
        } else {
            for j in 5..9 {
                a[j] = parsed.get(j).copied().unwrap_or(0.);
            }
            if values.len() < 9 {
                let base = fallback(key.trim());
                for j in 4..9 {
                    if values.len() <= j {
                        a[j] = base.get(j).copied().unwrap_or(0.);
                    }
                }
            }
        }
        let key = key.trim();
        if let Some(old) = out
            .keys()
            .find(|k: &&String| k.eq_ignore_ascii_case(key))
            .cloned()
        {
            out.remove(&old);
        }
        out.insert(key.into(), a);
    }
    (out, skipped)
}
pub fn apply_options(o: &CliOptions, mut p: PatchProfile) -> io::Result<PatchProfile> {
    if let Some(c) = &o.category {
        if !["Stats", "Compat", "Language"]
            .iter()
            .any(|s| s.eq_ignore_ascii_case(c))
        {
            return Err(arg_error(format!("未知的修改分類: '{c}'。")));
        }
    }
    for id in o.enable.iter().chain(o.disable.iter()) {
        if FeatureRegistry::get(id).is_none() {
            return Err(arg_error(format!("未知的修改功能 ID: '{id}'。")));
        }
    }
    if let Some(path) = &o.profile {
        return profile_from_json(&read_json(path)?);
    }
    if o.disable_all {
        p = PatchProfile::new();
    }
    let category = o
        .category
        .as_deref()
        .or(if o.compat { Some("Compat") } else { None });
    for f in FeatureRegistry::toggle_features() {
        let excluded = ENABLE_ALL_EXCLUDED.contains(&f.id);
        if let Some(c) = category {
            if format!("{:?}", f.category).eq_ignore_ascii_case(c) && (o.enable_all || !excluded) {
                p.set_bool(f.id, true);
            }
        } else if o.enable_all && !excluded {
            p.set_bool(f.id, true);
        }
    }
    for id in &o.enable {
        p.set_bool(FeatureRegistry::get(id).unwrap().id, true);
    }
    for id in &o.disable {
        p.set_bool(FeatureRegistry::get(id).unwrap().id, false);
    }
    if let Some(b) = o.balance {
        p.set_bool("Balance", b);
    }
    if let Some(n) = o.game_speed {
        p.set_int("GameSpeed", n);
    }
    if let Some(n) = o.quota {
        p.set_int("VillageGarrisonQuotaMultiplier", n);
    }
    Ok(p)
}

/// Mirrors C# `Path.GetFullPath` output: Windows `fs::canonicalize` returns verbatim
/// (`\\?\`) paths that the C# CLI never prints, so strip them when the plain form is equivalent.
pub fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        let b = rest.as_bytes();
        if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\' {
            return PathBuf::from(rest);
        }
    }
    path
}

/// Game directory resolution with the C# `CliCommands.ResolveGamePath` contract: an explicit
/// `--game` must exist (no silent fallback); otherwise auto-detection must yield an existing
/// directory or the command fails with the "specify --game" guidance.
pub fn resolve_game_path(game_path: Option<&str>) -> io::Result<PathBuf> {
    let candidate = if let Some(p) = game_path.filter(|s| !s.trim().is_empty()) {
        let p = PathBuf::from(p);
        if !p.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("指定的遊戲目錄不存在: {}", p.display()),
            ));
        }
        p
    } else {
        let detected = crate::services::GameDirectoryLocator::resolve_initial_game_path(None);
        if detected.as_os_str().is_empty() || !detected.is_dir() {
            return Err(invalid(
                "無法自動偵測到遊戲目錄。請使用 --game <路徑> 明確指定 Against Rome 安裝目錄。",
            ));
        }
        detected
    };
    Ok(strip_verbatim_prefix(fs::canonicalize(candidate)?))
}
fn resolve_game(o: &CliOptions) -> io::Result<PathBuf> {
    resolve_game_path(o.game_path.as_deref())
}
fn validate_exe(game: &Path) -> io::Result<()> {
    if !game.join("Against_Rome.exe").is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "遊戲目錄下未找到主執行檔 Against_Rome.exe",
        ));
    }
    Ok(())
}
pub fn features_json(
    o: &CliOptions,
    p: Option<&PatchProfile>,
    loc: &against_rome_ambience::app_localization::Loc,
) -> Value {
    Value::Array(FeatureRegistry::ALL.iter().filter(|f|o.category.as_ref().map(|c|format!("{:?}",f.category).eq_ignore_ascii_case(c)).unwrap_or(true)).map(|f|{
        let tip=format!("{}Tip",f.id);let mut v=json!({"id":f.id,"name":loc.get(f.id),"description":loc.get(&tip),"category":format!("{:?}",f.category),"controlKind":format!("{:?}",f.control_kind),"isExcludedFromEnableAll":ENABLE_ALL_EXCLUDED.contains(&f.id)});
        if let Some(p)=p.filter(|_|f.control_kind==FeatureControlKind::Toggle){v["currentValue"]=json!(p.get_bool(f.id));} v
    }).collect())
}

pub fn maps_json(game: &Path) -> io::Result<Value> {
    let root = game.join("MAPS");
    if !root.is_dir() {
        return Ok(json!([]));
    }
    let mut maps = Vec::new();
    for entry in fs::read_dir(root)? {
        let e = entry?;
        let id = e.file_name().to_string_lossy().into_owned();
        let p = e.path();
        if !p.is_dir()
            || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
            || !p.join("boden.txt").is_file()
            || !p.join("minimap.bmp").is_file()
        {
            continue;
        }
        let upper = id.to_ascii_uppercase();
        let (order, category) = if upper.starts_with("KAMP_") {
            (0, "劇情戰役")
        } else if upper.starts_with("HIST_") {
            (1, "歷史戰役")
        } else if upper.starts_with("TUTOR_") {
            (2, "教學")
        } else if upper.starts_with("ENDL_") {
            (3, "無盡模式")
        } else if upper.starts_with("MP_") {
            (4, "多人地圖")
        } else {
            (5, "其他地圖")
        };
        let slot = upper
            .strip_prefix("ENDL_")
            .and_then(|s| s.parse::<i32>().ok());
        let custom = upper.len() == 8
            && slot.is_some_and(|n| (5..=999).contains(&n))
            && p.join(".arm_custom_map").is_file();
        let mut v = json!({"id":id,"category":category,"isCustom":custom});
        if let Some(n) = slot {
            v["endlessSlot"] = json!(n);
        }
        let title = p.join("TEXT/US/briefing.put");
        if let Ok(b) = fs::read(title) {
            let text = crate::decode_cp1251(&crate::decompress_if_pfil(&b).unwrap_or(b));
            for line in text.lines() {
                if let Some((k, vv)) = line.split_once('=') {
                    if k.trim().eq_ignore_ascii_case("briefing_titel_1") {
                        v["displayName"] = json!(vv.trim().trim_matches('"'));
                    }
                }
            }
        }
        maps.push((order, upper, v));
    }
    maps.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    Ok(Value::Array(maps.into_iter().map(|x| x.2).collect()))
}

pub fn is_cli_invocation(args: &[String]) -> bool {
    let mut i = 0;
    while i < args.len() {
        let a = args[i].to_ascii_lowercase();
        if ["--cli", "--help", "-h", "/?", "--version", "-v", "--json"].contains(&a.as_str()) {
            return true;
        }
        if a.starts_with('-') {
            if [
                "--game",
                "-g",
                "--map",
                "--preset",
                "--profile",
                "--output",
                "-o",
                "--format",
                "-f",
                "--category",
                "-c",
                "--lang",
                "--unit",
                "-u",
                "--enable",
                "--disable",
                "--game-speed",
                "--speed",
                "--garrison-quota",
                "--quota",
            ]
            .contains(&a.as_str())
            {
                i += 1;
            }
        } else if canonical_command(&a).is_some() {
            return true;
        }
        i += 1;
    }
    false
}

use std::sync::atomic::{AtomicU64, Ordering};
static COMMIT_COUNTER: AtomicU64 = AtomicU64::new(0);

struct LockListGuard(Vec<PathBuf>);
impl Drop for LockListGuard {
    fn drop(&mut self) {
        for lock in &self.0 {
            let _ = fs::remove_file(lock);
        }
    }
}

struct TempListGuard(Vec<PathBuf>, bool);
impl Drop for TempListGuard {
    fn drop(&mut self) {
        if !self.1 {
            for tmp in &self.0 {
                let _ = fs::remove_file(tmp);
            }
        }
    }
}

enum AppliedAction {
    Created {
        path: PathBuf,
        new_bytes: Vec<u8>,
    },
    Replaced {
        path: PathBuf,
        old_bytes: Vec<u8>,
        new_bytes: Vec<u8>,
        temp_path: PathBuf,
    },
    Deleted {
        path: PathBuf,
        old_bytes: Vec<u8>,
    },
}

/// Writes every planned file via temp+replace; rejects concurrent external modifications
/// and stale locks; rolls back safely on failure while preserving external edits and temporary files.
pub fn commit_plan(plan: &crate::services::PatchFilePlan) -> io::Result<()> {
    commit_plan_internal(plan, None)
}

pub fn commit_plan_internal(
    plan: &crate::services::PatchFilePlan,
    injected_failure_after_step: Option<usize>,
) -> io::Result<()> {
    if plan.files.is_empty() && plan.deletes.is_empty() {
        return Ok(());
    }

    // 1. Acquire sibling locks for all target paths
    let mut locks = Vec::new();
    let lock_guard = (|| -> io::Result<LockListGuard> {
        let mut list = Vec::new();
        for path in plan.files.keys().chain(plan.deletes.iter()) {
            let name = path
                .file_name()
                .ok_or_else(|| invalid("無效的檔案路徑名稱"))?;
            let mut lock_name = name.to_os_string();
            lock_name.push(".arm-rust-lock");
            let lock_path = path.with_file_name(lock_name);
            if !list.contains(&lock_path) {
                if let Some(parent) = lock_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                let _file = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&lock_path)
                    .map_err(|e| {
                        io::Error::new(
                            e.kind(),
                            format!(
                                "無法取得檔案鎖 (可能另有程序正在執行修改): {}",
                                lock_path.display()
                            ),
                        )
                    })?;
                list.push(lock_path);
            }
        }
        Ok(LockListGuard(list))
    })()?;
    locks.push(lock_guard);

    // 2. Pre-transaction snapshot of originals and associated backup files
    let mut originals: BTreeMap<PathBuf, Option<Vec<u8>>> = BTreeMap::new();
    let mut bak_snapshots: BTreeMap<PathBuf, Option<Vec<u8>>> = BTreeMap::new();
    for (path, _) in &plan.files {
        let orig = if path.is_file() {
            Some(fs::read(path)?)
        } else {
            None
        };
        originals.insert(path.clone(), orig);
        let bak = PathBuf::from(format!("{}.bak", path.display()));
        let bak_bytes = if bak.is_file() {
            Some(fs::read(&bak)?)
        } else {
            None
        };
        bak_snapshots.insert(bak, bak_bytes);
    }
    for path in &plan.deletes {
        let orig = if path.is_file() {
            Some(fs::read(path)?)
        } else {
            None
        };
        originals.insert(path.clone(), orig);
        let bak = PathBuf::from(format!("{}.bak", path.display()));
        let bak_bytes = if bak.is_file() {
            Some(fs::read(&bak)?)
        } else {
            None
        };
        bak_snapshots.insert(bak, bak_bytes);
    }

    // 3. Prepare synced temp files with unique names
    let mut temp_paths: Vec<(PathBuf, PathBuf, Vec<u8>)> = Vec::new();
    let mut temp_guard = TempListGuard(Vec::new(), false);
    for (path, bytes) in &plan.files {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let name = path.file_name().unwrap();
        let mut tmp_name = name.to_os_string();
        let seq = COMMIT_COUNTER.fetch_add(1, Ordering::Relaxed);
        tmp_name.push(format!(".arm-tmp-{}-{}", std::process::id(), seq));
        let tmp_path = path.with_file_name(tmp_name);

        crate::create_synced(&tmp_path, bytes)?;
        if let Ok(meta) = fs::metadata(path) {
            let _ = fs::set_permissions(&tmp_path, meta.permissions());
        }
        temp_guard.0.push(tmp_path.clone());
        temp_paths.push((path.clone(), tmp_path, bytes.clone()));
    }

    // 4. Pre-commit concurrency verification (external file/backup modification check)
    for (path, orig_opt) in &originals {
        match orig_opt {
            Some(expected) => {
                if !path.is_file() {
                    return Err(invalid(format!(
                        "目標檔案在交易準備期間被外部刪除: {}",
                        path.display()
                    )));
                }
                let current = fs::read(path)?;
                if &current != expected {
                    return Err(invalid(format!(
                        "目標檔案在交易準備期間被外部修改: {}",
                        path.display()
                    )));
                }
            }
            None => {
                if path.exists() {
                    return Err(invalid(format!(
                        "目標檔案在交易準備期間被外部建立: {}",
                        path.display()
                    )));
                }
            }
        }
    }
    for (bak_path, bak_opt) in &bak_snapshots {
        match bak_opt {
            Some(expected) => {
                if !bak_path.is_file() {
                    return Err(invalid(format!(
                        "關聯備份在交易準備期間被外部刪除: {}",
                        bak_path.display()
                    )));
                }
                let current = fs::read(bak_path)?;
                if &current != expected {
                    return Err(invalid(format!(
                        "關聯備份在交易準備期間被外部修改: {}",
                        bak_path.display()
                    )));
                }
            }
            None => {
                if bak_path.exists() {
                    return Err(invalid(format!(
                        "關聯備份在交易準備期間被外部建立: {}",
                        bak_path.display()
                    )));
                }
            }
        }
    }

    // 5. Two-phase commit execution with action tracking
    let mut applied_actions: Vec<AppliedAction> = Vec::new();
    let mut step_count = 0;
    let commit_result = (|| -> io::Result<()> {
        for (path, tmp_path, bytes) in &temp_paths {
            if let Some(limit) = injected_failure_after_step {
                if step_count >= limit {
                    return Err(io::Error::new(io::ErrorKind::Other, "測試注入之交易失敗"));
                }
            }
            let is_existing = originals.get(path).and_then(|x| x.as_ref()).is_some();
            if is_existing {
                let old_bytes = originals.get(path).unwrap().as_ref().unwrap().clone();
                crate::replace(path, tmp_path)?;
                applied_actions.push(AppliedAction::Replaced {
                    path: path.clone(),
                    old_bytes,
                    new_bytes: bytes.clone(),
                    temp_path: tmp_path.clone(),
                });
            } else {
                fs::rename(tmp_path, path)?;
                applied_actions.push(AppliedAction::Created {
                    path: path.clone(),
                    new_bytes: bytes.clone(),
                });
            }
            step_count += 1;
        }

        for path in &plan.deletes {
            if let Some(limit) = injected_failure_after_step {
                if step_count >= limit {
                    return Err(io::Error::new(io::ErrorKind::Other, "測試注入之刪除失敗"));
                }
            }
            if path.is_file() {
                let old_bytes = originals
                    .get(path)
                    .and_then(|x| x.as_ref())
                    .cloned()
                    .unwrap_or_default();
                fs::remove_file(path)?;
                applied_actions.push(AppliedAction::Deleted {
                    path: path.clone(),
                    old_bytes,
                });
                step_count += 1;
            }
        }
        Ok(())
    })();

    // 6. Fail-safe rollback on error
    if let Err(primary_err) = commit_result {
        let mut rollback_errors = Vec::new();
        for action in applied_actions.into_iter().rev() {
            match action {
                AppliedAction::Created { path, new_bytes } => {
                    if path.is_file() {
                        if let Ok(cur) = fs::read(&path) {
                            if cur == new_bytes {
                                if let Err(e) = fs::remove_file(&path) {
                                    rollback_errors
                                        .push(format!("無法刪除新建檔案 {}: {e}", path.display()));
                                }
                            } else {
                                rollback_errors.push(format!(
                                    "目標檔案已被外部程式變更，保留外部檔案: {}",
                                    path.display()
                                ));
                            }
                        }
                    }
                }
                AppliedAction::Replaced {
                    path,
                    old_bytes,
                    new_bytes,
                    temp_path,
                } => {
                    let mut preserved_temp = false;
                    if path.is_file() {
                        match fs::read(&path) {
                            Ok(cur) if cur == new_bytes => {
                                // Content matches our replacement, restore original bytes safely
                                if let Err(e) = fs::remove_file(&path)
                                    .and_then(|_| crate::create_synced(&path, &old_bytes))
                                {
                                    preserved_temp = true;
                                    rollback_errors.push(format!(
                                        "還原檔案 {} 失敗: {e}; 保留替換檔 {}",
                                        path.display(),
                                        temp_path.display()
                                    ));
                                }
                            }
                            Ok(_) => {
                                // File changed externally; preserve external changes and keep temp
                                preserved_temp = true;
                                rollback_errors.push(format!(
                                    "目標檔案在替換後被外部程式變更，保留外部變更與替換檔: {}",
                                    path.display()
                                ));
                            }
                            Err(e) => {
                                preserved_temp = true;
                                rollback_errors
                                    .push(format!("無法讀取目標檔案 {}: {e}", path.display()));
                            }
                        }
                    } else {
                        // File missing, recreate from original bytes
                        if let Err(e) = crate::create_synced(&path, &old_bytes) {
                            preserved_temp = true;
                            rollback_errors.push(format!(
                                "重建原檔案 {} 失敗: {e}; 保留替換檔 {}",
                                path.display(),
                                temp_path.display()
                            ));
                        }
                    }
                    if !preserved_temp {
                        let _ = fs::remove_file(&temp_path);
                    }
                }
                AppliedAction::Deleted { path, old_bytes } => {
                    if path.exists() {
                        rollback_errors.push(format!(
                            "被刪除之路徑已被外部程式重新建立，保留外部檔案: {}",
                            path.display()
                        ));
                    } else if let Err(e) = crate::create_synced(&path, &old_bytes) {
                        rollback_errors
                            .push(format!("重建被刪除檔案 {} 失敗: {e}", path.display()));
                    }
                }
            }
        }

        if rollback_errors.is_empty() {
            return Err(primary_err);
        } else {
            return Err(io::Error::new(
                primary_err.kind(),
                format!(
                    "交易執行失敗 ({primary_err})，且回滾時偵測到外部變更或復原障礙:\r\n{}",
                    rollback_errors.join("\r\n")
                ),
            ));
        }
    }

    temp_guard.1 = true; // Mark as committed successfully so Drop won't delete renamed temp files
    Ok(())
}

fn load_backup(game: &Path) -> io::Result<crate::services::BackupManager> {
    load_backup_opt(game, true)
}
fn load_backup_readonly(game: &Path) -> io::Result<crate::services::BackupManager> {
    load_backup_opt(game, false)
}
fn load_backup_opt(
    game: &Path,
    allow_write_bak: bool,
) -> io::Result<crate::services::BackupManager> {
    let mut b = crate::services::BackupManager::new();
    let base = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    b.load_backup_zip_to_memory_opt(game, None, &base, allow_write_bak)?;
    Ok(b)
}

fn envelope(command: &str, data: io::Result<Value>, error: Option<String>) -> Value {
    match error {
        Some(e) => json!({"success":false,"command":command,"error":e}),
        None => json!({"success":true,"command":command,"data":data.unwrap_or(Value::Null)}),
    }
}

/// Version response. The C# CLI defines no version output (its parser treats `-v` as verbose and
/// `--version` falls through to help), so this reports the crate's Cargo package name/version.
pub fn version_json() -> Value {
    json!({"name":env!("CARGO_PKG_NAME"),"version":env!("CARGO_PKG_VERSION")})
}
/// Plain-text form: `<package-name> <cargo-version>`.
pub fn version_text() -> String {
    format!("{} {}\n", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
}
/// `--version` anywhere, or `-v` alone (bare `-v` otherwise stays the `--verbose` alias of a command).
fn is_version_request(args: &[String]) -> bool {
    if args.iter().any(|a| a.eq_ignore_ascii_case("--version")) {
        return true;
    }
    let rest: Vec<_> = args
        .iter()
        .filter(|a| !a.eq_ignore_ascii_case("--json") && !a.eq_ignore_ascii_case("--cli"))
        .collect();
    rest.len() == 1 && rest[0].eq_ignore_ascii_case("-v")
}

/// Runs a parsed command line; returns the process exit code (0 ok, 1 runtime failure, 2 argument error).
pub fn run_cli(args: &[String], out: &mut dyn io::Write, err: &mut dyn io::Write) -> i32 {
    if is_version_request(args) {
        if args.iter().any(|a| a.eq_ignore_ascii_case("--json")) {
            let _ = writeln!(
                out,
                "{}",
                serde_json::to_string_pretty(&envelope("version", Ok(version_json()), None))
                    .unwrap()
            );
        } else {
            let _ = write!(out, "{}", version_text());
        }
        return 0;
    }
    let o = match CliOptions::parse(args) {
        Ok(o) => o,
        Err(e) => {
            if args.iter().any(|a| a.eq_ignore_ascii_case("--json")) {
                let _ = writeln!(
                    out,
                    "{}",
                    serde_json::to_string_pretty(&envelope("unknown", Ok(Value::Null), Some(e)))
                        .unwrap()
                );
            } else {
                let _ = writeln!(err, "參數錯誤: {e}");
                let _ = writeln!(err, "執行 'AgainstRomeModifier.exe help' 可檢視說明。");
            }
            return 2;
        }
    };
    use crate::cli_commands as cc;
    let loc = cc::loc_for(&o);
    let name = o.command.clone();
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let result: io::Result<Value> = match name.as_str() {
        "help" => Ok(cc::help_json()),
        "detect-game" => Ok(detect_game_json(&o)),
        "status" => status_json(&o),
        "features" => features_command(&o),
        "maps" => resolve_game(&o).and_then(|g| maps_json(&g)),
        "profile" => cc::profile_command(&o, resolve_game, &loc),
        "apply" => apply_command(&o),
        "restore" => restore_command(&o),
        "backup" => cc::backup_command(&o, resolve_game),
        "stats" => cc::stats_command(&o, resolve_game, &loc, &now),
        "saves" => cc::saves_command(&o, resolve_game),
        "repair-language-backup" => cc::repair_language_backup_command(&o, resolve_game),
        other => Err(arg_error(format!("指令 '{other}' 不存在。"))),
    };
    match result {
        Ok(v) => {
            if o.json {
                let _ = writeln!(
                    out,
                    "{}",
                    serde_json::to_string_pretty(&envelope(&name, Ok(v.clone()), None)).unwrap()
                );
            } else {
                let _ = write!(out, "{}", cc::render_text(&name, &v, &loc));
            }
            if name == "backup" && !v["success"].as_bool().unwrap_or(true) {
                1
            } else {
                0
            }
        }
        Err(e) => {
            let code = if e.kind() == io::ErrorKind::InvalidInput {
                2
            } else {
                1
            };
            if o.json {
                let _ = writeln!(
                    out,
                    "{}",
                    serde_json::to_string_pretty(&envelope(
                        &name.replace('-', ""),
                        Ok(Value::Null),
                        Some(e.to_string())
                    ))
                    .unwrap()
                );
            } else {
                let _ = writeln!(err, "錯誤 ({code}): {e}");
            }
            code
        }
    }
}
fn detect_game_json(o: &CliOptions) -> Value {
    use crate::services::GameDirectoryLocator as L;
    let detected = L::resolve_initial_game_path(o.game_path.as_deref().map(Path::new));
    let valid = !detected.as_os_str().is_empty() && detected.is_dir();
    let registry = L::detect_from_registry();
    let mut v = json!({"isValid":valid,"hasExe":valid&&detected.join("Against_Rome.exe").is_file(),"defaultPath":crate::services::DEFAULT_INSTALL_PATH});
    if valid {
        v["detectedPath"] = json!(strip_verbatim_prefix(
            fs::canonicalize(&detected).unwrap_or(detected)
        )
        .display()
        .to_string());
    }
    if !registry.as_os_str().is_empty() {
        v["registryPath"] = json!(registry.display().to_string());
    }
    v
}
fn status_json(o: &CliOptions) -> io::Result<Value> {
    let game = resolve_game(o)?;
    validate_exe(&game)?;
    let b = load_backup(&game)?;
    let missing = b.find_missing_backup_resources();
    let p = crate::services::PatchEngine::detect_current_patch_state(&game, &b);
    let features: BTreeMap<_, _> = FeatureRegistry::toggle_features()
        .map(|f| (f.id, p.get_bool(f.id)))
        .collect();
    Ok(
        json!({"gamePath":game.display().to_string(),"backup":{"loaded":!b.backup_files().is_empty(),"missingCount":missing.len(),"missingFiles":missing},
        "gameSpeed":p.get_int("GameSpeed"),"villageGarrisonQuota":p.get_int("VillageGarrisonQuotaMultiplier"),
        "dgVoodooInstalled":crate::services::PatchEngine::is_dgvoodoo_installed(&game),"argmTraceInstalled":crate::services::PatchEngine::is_argm_trace_installed(&game),
        "hasCustomUnitStats":p.custom_unit_stats.as_ref().is_some_and(|m|!m.is_empty()),"balance":p.get_bool("Balance"),
        "activeFeaturesCount":features.values().filter(|v|**v).count(),"features":features}),
    )
}
fn features_command(o: &CliOptions) -> io::Result<Value> {
    let mut current = None;
    if let Ok(game) = resolve_game(o) {
        if game.join("Against_Rome.exe").is_file() {
            if let Ok(b) = load_backup(&game) {
                current = Some(crate::services::PatchEngine::detect_current_patch_state(
                    &game, &b,
                ));
            }
        }
    }
    Ok(features_json(
        o,
        current.as_ref(),
        &crate::cli_commands::loc_for(o),
    ))
}
fn apply_command(o: &CliOptions) -> io::Result<Value> {
    if let Some(c) = &o.category {
        if !["Stats", "Compat", "Language"]
            .iter()
            .any(|s| s.eq_ignore_ascii_case(c))
        {
            return Err(arg_error(format!("未知的修改分類: '{c}'。")));
        }
    }
    for id in o.enable.iter().chain(o.disable.iter()) {
        if FeatureRegistry::get(id).is_none() {
            return Err(arg_error(format!("未知的修改功能 ID: '{id}'。")));
        }
    }
    if let Some(path) = &o.profile {
        if !Path::new(path).is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("找不到設定檔: {path}"),
            ));
        }
    }
    if let Some(path) = &o.preset {
        if !Path::new(path).is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("找不到兵種預設檔: {path}"),
            ));
        }
    }
    let game = resolve_game(o)?;
    validate_exe(&game)?;
    let mut b = if o.dry_run {
        load_backup_readonly(&game)?
    } else {
        load_backup(&game)?
    };
    let backup_ok = if o.dry_run {
        b.ensure_backup_loaded_for_game_path_readonly(&game)?
    } else {
        b.ensure_backup_loaded_for_game_path(&game)?
    };
    if !backup_ok {
        return Err(invalid(
            "無法載入或建立遊戲備份基準。請先確認遊戲目錄包含原版檔案。",
        ));
    }
    let start = if o.disable_all || o.profile.is_some() {
        PatchProfile::new()
    } else {
        crate::services::PatchEngine::detect_current_patch_state(&game, &b)
    };
    let mut profile = apply_options(o, start)?;
    if o.profile.is_none() {
        if let Some(path) = &o.preset {
            let raw = fs::read(path)?;
            let text =
                String::from_utf8_lossy(raw.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(&raw))
                    .into_owned();
            let lines: Vec<&str> = text.lines().collect();
            let (stats, _) = parse_preset(
                &lines.join(
                    "
",
                ),
                |k| {
                    b.get_original_stats(k)
                        .map(|a| a.to_vec())
                        .unwrap_or_default()
                },
            );
            profile.custom_unit_stats = Some(stats);
        }
    }
    if o.disable_all && o.profile.is_none() {
        if o.game_speed.is_none() {
            profile.set_int("GameSpeed", 1);
        }
        if o.quota.is_none() {
            profile.set_int("VillageGarrisonQuotaMultiplier", 1);
        }
    }
    let resources = crate::install::InstallResources::discover();
    let plan = crate::services::PatchEngine::compose_with_install(&game, &profile, &b, &resources)?;
    if !o.dry_run {
        commit_plan(&plan)?;
    }
    let enabled: Vec<_> = FeatureRegistry::toggle_features()
        .filter(|f| profile.get_bool(f.id))
        .map(|f| f.id)
        .collect();
    Ok(
        json!({"gamePath":game.display().to_string(),"isDryRun":o.dry_run,"appliedFeaturesCount":enabled.len(),"enabledFeatures":enabled,
        "gameSpeed":profile.get_int("GameSpeed"),"villageGarrisonQuotaMultiplier":profile.get_int("VillageGarrisonQuotaMultiplier"),"balance":profile.get_bool("Balance"),
        "customStatsUnitsCount":profile.custom_unit_stats.as_ref().map_or(0,|m|m.len()),"plannedFiles":plan.files.len()}),
    )
}
fn restore_command(o: &CliOptions) -> io::Result<Value> {
    use crate::services::{PatchEngine, PatchFilePlan};
    let game = resolve_game(o)?;
    validate_exe(&game)?;
    let b = if o.dry_run {
        load_backup_readonly(&game)?
    } else {
        load_backup(&game)?
    };
    let specific = o.restore_stats || o.restore_compat || o.restore_language;
    let all = o.restore_all || !specific;
    let custom_map_deletion_slots = if all {
        crate::custom_map_restore::deletion_slots(&game, o.preserve_custom_maps)?
    } else {
        Vec::new()
    };
    let mut cats = Vec::new();
    let mut plan = PatchFilePlan::default();
    let mut merge = |p: PatchFilePlan| {
        plan.files.extend(p.files);
        plan.deletes.extend(p.deletes);
    };
    if all {
        merge(PatchEngine::restore_original_files(
            &game,
            &b,
            o.preserve_custom_maps,
        )?);
        cats = vec!["Stats", "Compat", "Language"];
    } else {
        if o.restore_stats {
            merge(PatchEngine::restore_stats_only(&game, &b)?);
            cats.push("Stats");
        }
        if o.restore_compat {
            merge(PatchEngine::restore_compat_only(&game, &b)?);
            cats.push("Compat");
        }
        if o.restore_language {
            merge(PatchEngine::restore_language_only(&game)?);
            cats.push("Language");
        }
    }
    if !o.dry_run {
        crate::custom_map_restore::commit_restore(
            &game,
            &plan,
            o.preserve_custom_maps,
            &custom_map_deletion_slots,
        )?;
    }
    Ok(
        json!({"gamePath":game.display().to_string(),"categoriesRestored":cats,"preserveCustomMaps":o.preserve_custom_maps,"isDryRun":o.dry_run,"plannedFiles":plan.files.len(),"plannedCustomMapsDeleted":custom_map_deletion_slots.len()}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }
    #[test]
    fn parser_aliases_and_precedence() {
        let o = CliOptions::parse(&args(&[
            "--cli",
            "apply",
            "--enable",
            "a,b;c",
            "--all",
            "--balance",
            "false",
            "--quota",
            "10",
            "--unit",
            "x",
            "--json",
        ]))
        .unwrap();
        assert_eq!(o.enable, ["a", "b", "c"]);
        assert_eq!(o.balance, Some(false));
        assert_eq!(o.quota, Some(10));
        assert!(o.enable_all && o.restore_all && o.json);
    }
    #[test]
    fn parser_missing_range_and_unknown_command() {
        for a in [
            vec!["apply", "--game"],
            vec!["apply", "--speed", "11"],
            vec!["wat"],
        ] {
            assert!(CliOptions::parse(&args(&a)).is_err());
        }
    }
    #[test]
    fn legacy_preset_and_fallback() {
        let (v, n) = parse_preset("u=1,2,3,4,50,60\nv=1,2,3,4\nbad=1,2\n", |_| vec![7.; 9]);
        assert_eq!(n, 1);
        assert_eq!(v["u"], vec![1., 2., 3., 4., 0., 50., 60., 0., 0.]);
        assert_eq!(&v["v"][4..], &[7.; 5]);
    }
    #[test]
    fn all_exclusions_category_override_and_disable_order() {
        let o = CliOptions::parse(&args(&["apply", "--all", "--disable", "FocusLoss"])).unwrap();
        let p = apply_options(&o, PatchProfile::new()).unwrap();
        assert!(!p.get_bool("FocusLoss"));
        assert!(!p.get_bool("ArgmTrace"));
        let o = CliOptions::parse(&args(&["apply", "--category", "Compat", "--all"])).unwrap();
        assert!(apply_options(&o, p).unwrap().get_bool("ArgmTrace"));
    }
    #[test]
    fn profile_pascal_case_roundtrip_and_case_replacement() {
        let mut p = PatchProfile::new();
        p.set_bool("FocusLoss", true);
        p.set_bool("focusloss", false);
        assert!(!p.get_bool("FocusLoss"));
        let p = profile_from_json(&profile_to_json(&p)).unwrap();
        assert_eq!(p.get_int("GameSpeed"), 1);
    }
    #[test]
    fn cli_invocation_detection() {
        assert!(is_cli_invocation(&args(&["status"])));
        assert!(is_cli_invocation(&args(&["--game", "x", "--json"])));
        assert!(!is_cli_invocation(&args(&["--game", "status"])));
        assert!(!is_cli_invocation(&[]));
    }
    #[test]
    fn run_cli_error_paths_and_help() {
        let (mut o, mut e) = (Vec::new(), Vec::new());
        assert_eq!(run_cli(&args(&["wat", "--json"]), &mut o, &mut e), 2);
        assert!(String::from_utf8(o).unwrap().contains("\"success\": false"));
        let (mut o, mut e) = (Vec::new(), Vec::new());
        assert_eq!(run_cli(&args(&["help"]), &mut o, &mut e), 0);
        assert!(!o.is_empty());
        let (mut o, mut e) = (Vec::new(), Vec::new());
        assert_eq!(
            run_cli(
                &args(&["status", "--game", "Z:/nonexistent-arm"]),
                &mut o,
                &mut e
            ),
            1
        );
        assert!(!e.is_empty());
    }
    #[test]
    fn commit_plan_writes_deletes_and_rolls_back() {
        let d = crate::cli::temp_fixture::dir("arm-cli-commit");
        fs::create_dir_all(&d).unwrap();
        let (a, b, c) = (d.join("a.bin"), d.join("b.bin"), d.join("c.bin"));
        fs::write(&a, b"old").unwrap();
        fs::write(&c, b"del").unwrap();
        let mut plan = crate::services::PatchFilePlan::default();
        plan.files.insert(a.clone(), b"new".to_vec());
        plan.files.insert(b.clone(), b"b".to_vec());
        plan.deletes.push(c.clone());
        commit_plan(&plan).unwrap();
        assert_eq!(fs::read(&a).unwrap(), b"new");
        assert!(b.is_file() && !c.is_file());
        let bad = d.join("dir");
        fs::create_dir(&bad).unwrap();
        let mut plan = crate::services::PatchFilePlan::default();
        plan.files.insert(a.clone(), b"zzz".to_vec());
        plan.files.insert(bad.clone(), b"x".to_vec());
        assert!(commit_plan(&plan).is_err());
        assert_eq!(fs::read(&a).unwrap(), b"new");
        let _ = fs::remove_dir_all(&d);
    }
}

#[cfg(test)]
mod temp_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/temp_unique.rs"));
}
