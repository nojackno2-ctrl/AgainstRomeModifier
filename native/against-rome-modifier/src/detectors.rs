//! Feature detectors ported from src.Core/Core/Features.
use crate::{
    bci_features::BciFeatureDetector,
    endless_ai::EndlessAiOrchestrator,
    exe_features::{
        self, ExePatchState, ExeRomanEndlessPatchState, ExeSpellAltarPatchState,
        ExeVillageSetterPatchState,
    },
    install::InstallFeatureDetector,
    objdef::ObjdefPatcher,
    patch_text::parse_csv_line,
    services::BackupManager,
    troops::TroopConfig,
    PatchProfile,
};
use std::{collections::HashMap, fs, path::Path};

pub fn is_exe_original(bytes: &[u8]) -> bool {
    exe_features::is_exe_original(bytes)
}

pub struct ExeFeatureDetector;
impl ExeFeatureDetector {
    pub fn detect(game_path: &Path, profile: &mut PatchProfile) -> ExeRomanEndlessPatchState {
        let exe_path = game_path.join("Against_Rome.exe");
        if !exe_path.is_file() {
            return ExeRomanEndlessPatchState::Unknown;
        }
        let Ok(bytes) = fs::read(&exe_path) else {
            return ExeRomanEndlessPatchState::Unknown;
        };

        let focus = exe_features::get_focus_patch_state(&bytes);
        if focus != ExePatchState::Unknown {
            profile.set_bool("FocusLoss", focus == ExePatchState::FocusPatched);
        }

        let setter = exe_features::get_village_setter_patch_state(&bytes);
        if setter != ExeVillageSetterPatchState::Unknown {
            profile.set_bool(
                "VillageBuildRange",
                setter != ExeVillageSetterPatchState::Original,
            );
        }

        let altar = exe_features::get_spell_altar_patch_state(&bytes);
        if altar != ExeSpellAltarPatchState::Unknown {
            profile.set_bool("NoSpellAltar", altar == ExeSpellAltarPatchState::Patched);
        }

        // C# assigns GameSpeed unconditionally (0 = unknown), so a stale value must not survive.
        profile.set_int("GameSpeed", exe_features::get_game_speed_multiplier(&bytes));

        profile.set_bool(
            "CiviProduce20",
            exe_features::get_civi_produce_20_patch_state(&bytes)
                == exe_features::ExeCiviProduce20PatchState::Patched,
        );
        profile.set_bool(
            "UnitRecruit20",
            exe_features::get_unit_recruit_20_patch_state(&bytes)
                == exe_features::ExeUnitRecruit20PatchState::Patched,
        );
        profile.set_bool(
            "IdleSelect999",
            exe_features::get_idle_select_999_patch_state(&bytes)
                == exe_features::ExeIdleSelect999PatchState::Patched,
        );
        profile.set_bool(
            "CorpseRetention",
            exe_features::get_corpse_retention_patch_state(&bytes)
                == exe_features::ExeCorpseRetentionPatchState::Patched,
        );

        let ws = exe_features::get_native_widescreen_patch_state(&bytes);
        profile.set_bool(
            "NativeWidescreen1920x1080",
            matches!(
                ws,
                exe_features::ExeNativeWidescreenPatchState::LegacyUnforced
                    | exe_features::ExeNativeWidescreenPatchState::LegacyForcedStaleUi
                    | exe_features::ExeNativeWidescreenPatchState::Patched
            ),
        );

        let zoom = exe_features::get_camera_zoom_out_patch_state(&bytes);
        profile.set_bool(
            "CameraZoomOut1",
            matches!(
                zoom,
                exe_features::ExeCameraZoomOutPatchState::LegacyZoomHalf
                    | exe_features::ExeCameraZoomOutPatchState::Patched
            ),
        );

        exe_features::get_roman_endless_patch_state(&bytes)
    }
}

pub struct ClScriptFeatureDetector;
impl ClScriptFeatureDetector {
    const IDS: [&'static str; 7] = [
        "FastCiviProduction",
        "InfiniteMorale",
        "SpellDamage5x",
        "SpellHealing10x",
        "SpellResurrection",
        "SpellRange3x",
        "GeneralSkills",
    ];

    pub fn detect(game_path: &Path, backup: &BackupManager, profile: &mut PatchProfile) {
        for id in Self::IDS {
            profile.set_bool(id, false);
        }
        let key = "SYSTEM/cl_script.ini";
        let Ok(bytes) = fs::read(game_path.join(key)) else {
            return;
        };
        let Ok(doc) = crate::patch_text::TextDocument::read(&bytes) else {
            return;
        };
        let original = backup
            .get_backup_bytes(key)
            .ok()
            .and_then(|b| crate::patch_text::TextDocument::read(b).ok());
        Self::detect_text(
            &doc.text,
            original.as_ref().map(|d| d.text.as_str()),
            profile,
        );
    }

    pub fn detect_text(text: &str, original: Option<&str>, profile: &mut PatchProfile) {
        for id in Self::IDS {
            profile.set_bool(id, false);
        }
        let current = Self::values(text);
        let orig = original.map(Self::values);
        for id in Self::IDS {
            if orig.is_none() && !matches!(id, "FastCiviProduction" | "InfiniteMorale") {
                continue;
            }
            let source = orig.as_ref().unwrap_or(&current);
            let mut any = false;
            let mut evidence = false;
            let mut valid = true;
            // 同時驗證來源與目前的受支援列，新增／遺失／重複／損毀列皆不能略過。
            let keys: std::collections::HashSet<_> = source.keys().chain(current.keys()).collect();
            let mut morale_factions = std::collections::HashSet::new();
            for key in keys {
                let parts: Vec<_> = key.split('|').collect();
                let head = parts[0];
                let target = match id {
                    "FastCiviProduction" if head == "CIVIDELAY" => Some(500.0),
                    "InfiniteMorale" => match head {
                        "MORALSDECLOSTMEM" | "MORALSDECFLEE" => Some(0.0),
                        "MORALSDECOVERPOP" => Some(99_999_999.0),
                        "MORALSINCIDLE" => Some(500.0),
                        _ => None,
                    },
                    "SpellDamage5x"
                        if head == "Value"
                            && parts.len() == 3
                            && crate::cl_script::DAMAGE_SPELL_KEYS
                                .contains(&format!("{}_{}", parts[1], parts[2]).as_str()) =>
                    {
                        source
                            .get(key)
                            .or_else(|| current.get(key))
                            .map(|v| (v * 5.0).trunc())
                    }
                    "SpellHealing10x" if key == "Value|KEL|Spell1" => source
                        .get(key)
                        .or_else(|| current.get(key))
                        .map(|v| (v * 10.0).trunc()),
                    "SpellResurrection"
                        if matches!(key.as_str(), "Value|KEL|Spell3" | "Value2|KEL|Spell3") =>
                    {
                        Some(100.0)
                    }
                    "SpellRange3x" if head == "RADIUS" => source
                        .get(key)
                        .or_else(|| current.get(key))
                        .map(|v| (v * 3.0).trunc()),
                    "GeneralSkills"
                        if head.starts_with("Value")
                            && parts.get(2).is_some_and(|p| p.starts_with("SAbility")) =>
                    {
                        source
                            .get(key)
                            .or_else(|| current.get(key))
                            .map(|v| (v * 5.0).trunc())
                    }
                    _ => None,
                };
                if let Some(expected) = target {
                    any = true;
                    evidence |= orig.is_none()
                        || source
                            .get(key)
                            .is_some_and(|v| (*v - expected).abs() > 0.00001);
                    valid &= source.get(key).is_some_and(|v| v.is_finite())
                        && current
                            .get(key)
                            .is_some_and(|v| v.is_finite() && (*v - expected).abs() < 0.00001);
                    if id == "InfiniteMorale" && parts.len() >= 2 {
                        morale_factions.insert(parts[1]);
                    }
                }
            }
            if id == "InfiniteMorale" {
                valid &= morale_factions.iter().all(|f| {
                    [
                        "MORALSDECLOSTMEM",
                        "MORALSDECFLEE",
                        "MORALSDECOVERPOP",
                        "MORALSINCIDLE",
                    ]
                    .iter()
                    .all(|h| current.contains_key(&format!("{h}|{f}")))
                });
            }
            profile.set_bool(id, any && valid && evidence);
        }
    }

    fn values(text: &str) -> HashMap<String, f64> {
        let mut result = HashMap::new();
        for line in text.lines() {
            let body = line.split(';').next().unwrap_or("").trim();
            let Some((head, value)) = body.split_once('=') else {
                continue;
            };
            let head = head.trim();
            let parts: Vec<_> = value.split(',').map(str::trim).collect();
            let keyed = head.starts_with("Value") || head.eq_ignore_ascii_case("Radius");
            if parts.is_empty() {
                continue;
            }
            let normalized = if head.starts_with("Value") {
                head.to_owned()
            } else {
                head.to_ascii_uppercase()
            };
            let key = if keyed {
                format!(
                    "{normalized}|{}|{}",
                    parts[0],
                    parts.get(1).copied().unwrap_or("")
                )
            } else {
                format!("{normalized}|{}", parts[0])
            };
            let parsed = parts
                .get(if keyed { 2 } else { 1 })
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(f64::NAN);
            if result.insert(key.clone(), parsed).is_some() {
                result.insert(key, f64::NAN);
            }
        }
        result
    }
}
pub struct RessFeatureDetector;
type Costs = [HashMap<(String, String, usize), Option<f64>>; 3];
impl RessFeatureDetector {
    const IDS: [&'static str; 3] = ["FreeProduction", "FreeUpgrade", "NoSpellCost"];
    pub fn detect(game_path: &Path, profile: &mut PatchProfile) {
        for id in Self::IDS {
            profile.set_bool(id, false);
        }
        let Ok(bytes) = fs::read(game_path.join("SYSTEM/ress.ini")) else {
            return;
        };
        let Ok(doc) = crate::patch_text::TextDocument::read(&bytes) else {
            return;
        };
        Self::detect_text(&doc.text, profile);
    }

    /// 公開文字介面驗證所有現存列；整套偵測另以備份檢查必要列是否遺失。
    pub fn detect_text(text: &str, profile: &mut PatchProfile) {
        Self::detect_relative(text, None, profile);
    }

    fn detect_relative(text: &str, original: Option<&str>, profile: &mut PatchProfile) {
        let current = Self::costs(text);
        let original = original.map(Self::costs);
        for (i, id) in Self::IDS.iter().enumerate() {
            let complete = original
                .as_ref()
                .is_none_or(|o| o[i].keys().all(|k| current[i].contains_key(k)));
            profile.set_bool(
                *id,
                complete && !current[i].is_empty() && current[i].values().all(|v| *v == Some(0.0)),
            );
        }
    }

    fn costs(text: &str) -> Costs {
        let mut section = "";
        let mut result: Costs = Default::default();
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                section = line;
                continue;
            }
            if line.is_empty() || line.starts_with(';') {
                continue;
            }
            let cols = parse_csv_line(line);
            let Some(name) = cols.first().map(|s| s.trim()) else {
                continue;
            };
            let mut fields = [Vec::new(), Vec::new(), Vec::new()];
            if section == "[objres]" {
                if name.starts_with("Bau") {
                    fields[0].extend(1..=6);
                    fields[1].extend(7..=12);
                } else if name.starts_with("Fig")
                    && !name.eq_ignore_ascii_case("FigTiePac00_Packpferd")
                {
                    fields[0].extend(13..=18);
                    if name.contains("Art") || name.contains("Bar") || name.contains("Fal") {
                        fields[0].extend(1..=6);
                    }
                    if name.contains("Pri") || name.contains("Dru") {
                        fields[2].extend(25..=28);
                    }
                }
            } else if section == "[volkres]" {
                fields[1].extend([8, 10, 12, 14]);
                fields[1].extend((24..=263).step_by(2));
                fields[1].extend(264..=295);
            }
            for i in 0..3 {
                for &c in &fields[i] {
                    let key = (section.to_owned(), name.to_owned(), c);
                    let value = number(&cols, c);
                    if result[i].insert(key.clone(), value).is_some() {
                        result[i].insert(key, None);
                    }
                }
            }
        }
        result
    }
}
pub struct TeamDatFeatureDetector;
impl TeamDatFeatureDetector {
    pub fn detect(game_path: &Path, backup: &BackupManager) -> (bool, bool) {
        let mut keys: std::collections::BTreeSet<String> = backup
            .backup_files()
            .keys()
            .filter(|k| k.starts_with("maps/") && k.ends_with("/team.dat"))
            .cloned()
            .collect();
        fn visit(
            root: &Path,
            dir: &Path,
            keys: &mut std::collections::BTreeSet<String>,
        ) -> Option<()> {
            if !dir.exists() {
                return Some(());
            }
            for entry in fs::read_dir(dir).ok()? {
                let entry = entry.ok()?;
                let kind = entry.file_type().ok()?;
                if kind.is_symlink() {
                    continue;
                }
                if kind.is_dir() {
                    visit(root, &entry.path(), keys)?;
                } else if entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case("team.dat")
                    && !crate::services::is_custom_map_file(&root.join("MAPS"), &entry.path())
                {
                    keys.insert(
                        entry
                            .path()
                            .strip_prefix(root)
                            .ok()?
                            .to_string_lossy()
                            .replace('\\', "/")
                            .to_ascii_lowercase(),
                    );
                }
            }
            Some(())
        }
        if visit(game_path, &game_path.join("MAPS"), &mut keys).is_none() {
            return (false, false);
        }
        let mut pop_ok = true;
        let mut rom_ok = true;
        let mut pop_any = false;
        let mut rom_any = false;
        for key in keys {
            if crate::services::is_custom_map_file(&game_path.join("MAPS"), &game_path.join(&key)) {
                continue;
            }
            let endless = key.starts_with("maps/endl_");
            let pair = || {
                Some((
                    Self::teams(backup.get_backup_bytes(&key).ok()?)?,
                    Self::teams(&fs::read(game_path.join(&key)).ok()?)?,
                ))
            };
            let Some((original, current)) = pair() else {
                pop_ok = false;
                if endless {
                    rom_ok = false;
                }
                continue;
            };
            for (team, row) in &original {
                let Some(value) = row.get(4).and_then(|v| v.trim().parse::<i32>().ok()) else {
                    pop_ok = false;
                    continue;
                };
                if value > 0 {
                    pop_any = true;
                    pop_ok &= current
                        .get(team)
                        .and_then(|r| r.get(4))
                        .and_then(|v| v.trim().parse::<i32>().ok())
                        == Some(1600);
                }
            }
            // 新增隊伍也必須符合上限；未知或遺失必要欄位不能被忽略。
            for row in current.values() {
                match row.get(4).and_then(|v| v.trim().parse::<i32>().ok()) {
                    Some(v) if v > 0 => {
                        pop_any = true;
                        pop_ok &= v == 1600;
                    }
                    Some(_) => {}
                    None => pop_ok = false,
                }
            }
            if endless {
                rom_any = true;
                rom_ok &= original.contains_key("0")
                    && current
                        .get("0")
                        .and_then(|r| r.get(1))
                        .is_some_and(|f| f.trim() == "ROM");
            }
        }
        (pop_ok && pop_any, rom_ok && rom_any)
    }

    fn teams(bytes: &[u8]) -> Option<Rows> {
        let doc = crate::patch_text::TextDocument::read(bytes).ok()?;
        let mut active = false;
        let mut result = HashMap::new();
        for line in &doc.lines {
            let line = line.trim();
            if line.starts_with('[') {
                active = line == "[teamdata]";
                continue;
            }
            if !active || line.is_empty() || line.starts_with(';') {
                continue;
            }
            let cols = parse_csv_line(line);
            if cols.len() < 5 {
                return None;
            }
            let id = cols[0].trim().to_owned();
            if id.is_empty() || result.insert(id, cols).is_some() {
                return None;
            }
        }
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }
}
/// 偵測只回報磁碟可證明的完整功能，不從任意自訂數值猜測設定。
pub struct ObjdefFeatureDetector;
type Rows = HashMap<String, Vec<String>>;
type Fields = Vec<(String, usize)>;

fn rows(bytes: &[u8], name_col: usize) -> Option<Rows> {
    let doc = crate::patch_text::TextDocument::read(bytes).ok()?;
    let mut result = HashMap::new();
    for line in doc.lines.iter().skip(2) {
        let cols = parse_csv_line(line);
        let Some(name) = cols
            .get(name_col)
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
        else {
            continue;
        };
        if result.insert(name.to_owned(), cols.clone()).is_some() {
            return None;
        }
    }
    Some(result)
}

fn number(row: &[String], col: usize) -> Option<f64> {
    row.get(col)?
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
}

/// 僅比較功能擁有的欄位，無關欄位、空白與行序均可不同；必要列或欄位缺失即拒絕。
fn matches_fields(current: &Rows, expected: &Rows, original: &Rows, fields: &Fields) -> bool {
    let mut changed = false;
    for (name, col) in fields {
        let values = || {
            Some((
                number(current.get(name)?, *col)?,
                number(expected.get(name)?, *col)?,
                number(original.get(name)?, *col)?,
            ))
        };
        let Some((c, e, o)) = values() else {
            return false;
        };
        if (c - e).abs() > 0.00001 {
            return false;
        }
        changed |= (e - o).abs() > 0.00001;
    }
    changed
}

impl ObjdefFeatureDetector {
    const KEY: &'static str = "SYSTEM/DATA_MP/DEFAULTS/objdef.dau";
    const IDS: [&'static str; 13] = [
        "HousingCapacity20x",
        "StorageCapacity10x",
        "HqHp10x",
        "FastBuildUpgradeRepair",
        "ProjectileArcHeight",
        "NoRunHpLoss",
        "AllUnitsEntireMapVision",
        "Balance",
        "LeaderGlory",
        "UnitMovementSpeed2x",
        "RangedRange3x",
        "VillagerMovementSpeed5x",
        "SpellEntireMap",
    ];

    pub fn detect(game_path: &Path, backup: &BackupManager, profile: &mut PatchProfile) {
        for id in Self::IDS {
            profile.set_bool(id, false);
        }
        // 相同結果可由多組自訂設定產生，不能唯一反推；磁碟偵測必須清除記憶體殘值。
        profile.custom_unit_stats = None;
        profile.set_string("CustomUnitStats", "");
        let Ok(original_bytes) = backup.get_backup_bytes(Self::KEY) else {
            return;
        };
        let Ok(current_bytes) = fs::read(game_path.join(Self::KEY)) else {
            return;
        };
        let (Some(original), Some(current)) = (rows(original_bytes, 52), rows(&current_bytes, 52))
        else {
            return;
        };
        // 預期值只在記憶體產生，避免為每個候選重壓縮整份 PFIL。
        let Ok(doc) = crate::patch_text::TextDocument::read(original_bytes) else {
            return;
        };
        let original_raw = crate::patch_text::encode_cp1251(&doc.text);
        let original_bytes = original_raw.as_slice();
        let mut base = crate::objdef::ObjdefOptions::default();
        for name in original
            .keys()
            .filter(|n| TroopConfig::get_metadata(n).is_some())
        {
            let Ok(stats) = backup.get_original_stats(name) else {
                return;
            };
            base.unit_stats.insert(name.clone(), stats);
        }
        // 獨立功能不受自訂戰鬥數值影響，所有必要欄位仍須逐列吻合。
        for id in [
            "HousingCapacity20x",
            "StorageCapacity10x",
            "HqHp10x",
            "FastBuildUpgradeRepair",
            "ProjectileArcHeight",
            "NoRunHpLoss",
            "LeaderGlory",
            "UnitMovementSpeed2x",
            "VillagerMovementSpeed5x",
        ] {
            let mut options = base.clone();
            match id {
                "HousingCapacity20x" => options.housing_capacity_20x = true,
                "StorageCapacity10x" => options.storage_capacity_10x = true,
                "HqHp10x" => options.hq_hp_10x = true,
                "FastBuildUpgradeRepair" => options.fast_build_upgrade_repair = true,
                "ProjectileArcHeight" => options.projectile_arc_height = true,
                "NoRunHpLoss" => options.no_run_hp_loss = true,
                "LeaderGlory" => options.leader_glory_5x = true,
                "UnitMovementSpeed2x" => options.unit_movement_speed_2x = true,
                _ => options.villager_movement_speed_5x = true,
            }
            let fields = Self::fields(&original, id);
            let valid = ObjdefPatcher::get_patched_bytes(original_bytes, &options)
                .ok()
                .and_then(|b| rows(&b, 52))
                .is_some_and(|e| matches_fields(&current, &e, &original, &fields));
            profile.set_bool(
                id,
                valid && (id != "ProjectileArcHeight" || Self::partgeo(game_path, backup)),
            );
        }

        let epara = Self::epara(game_path, backup);
        // 視野覆寫射程與祭司視野，Balance 又會改動射程基準。
        // 枚舉既有功能的組合，但只有每個已啟用功能皆完整吻合才接受；
        // 不能用局部視野值假裝另一功能已啟用，也不猜測未知自訂覆寫。
        for bits in 1..16 {
            let mut options = base.clone();
            options.balance = bits & 1 != 0;
            options.ranged_range_3x = bits & 2 != 0;
            options.spell_entire_map = bits & 4 != 0;
            options.all_units_entire_map_vision = bits & 8 != 0;
            if options.ranged_range_3x && !epara {
                continue;
            }
            if options.balance {
                for (name, stat) in &mut options.unit_stats {
                    let Ok(value) = backup.get_default_balanced_stats(name) else {
                        return;
                    };
                    *stat = value;
                }
            }
            let Some(expected) = ObjdefPatcher::get_patched_bytes(original_bytes, &options)
                .ok()
                .and_then(|b| rows(&b, 52))
            else {
                continue;
            };
            let ids = [
                (1, "Balance"),
                (2, "RangedRange3x"),
                (4, "SpellEntireMap"),
                (8, "AllUnitsEntireMapVision"),
            ];
            if ids
                .iter()
                .filter(|(bit, _)| bits & bit != 0)
                .all(|(_, id)| {
                    matches_fields(&current, &expected, &original, &Self::fields(&original, id))
                })
            {
                for (bit, id) in ids {
                    if bits & bit == 0 {
                        continue;
                    }
                    let mut without = options.clone();
                    match id {
                        "Balance" => {
                            without.balance = false;
                            without.unit_stats = base.unit_stats.clone();
                        }
                        "RangedRange3x" => without.ranged_range_3x = false,
                        "SpellEntireMap" => without.spell_entire_map = false,
                        _ => without.all_units_entire_map_vision = false,
                    }
                    let distinct = ObjdefPatcher::get_patched_bytes(original_bytes, &without)
                        .ok()
                        .and_then(|b| rows(&b, 52))
                        .is_some_and(|other| {
                            Self::fields(&original, id).iter().any(|(n, c)| {
                                expected.get(n).and_then(|r| number(r, *c))
                                    != other.get(n).and_then(|r| number(r, *c))
                            })
                        });
                    if distinct {
                        profile.set_bool(id, true);
                    }
                }
            }
        }
        // 全圖視野遮蔽祭司視野，無法從磁碟證明祭司開關。
        if profile.get_bool("AllUnitsEntireMapVision") {
            profile.set_bool("SpellEntireMap", false);
        }
    }

    fn fields(original: &Rows, id: &str) -> Fields {
        let mut fields = Vec::new();
        for (name, row) in original {
            let meta = TroopConfig::get_metadata(name);
            let civilian = matches!(
                name.as_str(),
                "FigZivMan00_Zivilist" | "FigZivWei00_Zivilistin" | "FigTiePac00_Packpferd"
            );
            let mut cols = Vec::new();
            match id {
                "HousingCapacity20x" => cols.push(156),
                "StorageCapacity10x"
                    if name.starts_with("Bau")
                        && (name.contains("Hau") || name.contains("Lag")) =>
                {
                    cols.push(42)
                }
                "HqHp10x" if name.starts_with("Bau") && name.contains("Hau") => cols.push(19),
                "FastBuildUpgradeRepair" if name.starts_with("Bau") => cols.extend([73, 74]),
                "LeaderGlory" if crate::objdef::GLORY_LEADERS.contains(&name.as_str()) => {
                    cols.extend(crate::objdef::GLORY_COLUMNS)
                }
                "UnitMovementSpeed2x" if meta.is_some() => cols.extend([4, 23, 191]),
                "VillagerMovementSpeed5x" if civilian => cols.extend([4, 23, 191]),
                "NoRunHpLoss" if meta.is_some() || civilian => cols.push(208),
                "AllUnitsEntireMapVision" if meta.is_some() || civilian => cols.push(24),
                "SpellEntireMap" if meta.is_some_and(|m| m.unit_type == "priest") => cols.push(24),
                "Balance" if meta.is_some() => {
                    cols.extend([19, 142, 146, 24]);
                    for w in 0..8 {
                        if row.get(78 + w * 8).is_some_and(|v| v.trim() == "1") {
                            cols.extend([78 + w * 8, 79 + w * 8, 81 + w * 8, 84 + w * 8]);
                        }
                    }
                }
                _ => {}
            }
            if id == "ProjectileArcHeight" || id == "RangedRange3x" {
                if id == "RangedRange3x"
                    && meta.is_some_and(|m| TroopConfig::supports_ranged_range_3x(m.unit_type))
                {
                    cols.push(24);
                    for w in 0..8 {
                        if row.get(78 + w * 8).is_some_and(|v| v.trim() == "1") {
                            cols.extend([78 + w * 8, 81 + w * 8]);
                        }
                    }
                }
                for w in 0..8 {
                    // 啟用狀態是必要條件；不能靠刪除後段武器欄位縮小檢查集合。
                    cols.push(78 + w * 8);
                    if row.get(78 + w * 8).is_some_and(|v| v.trim() == "1") {
                        if id == "ProjectileArcHeight" {
                            cols.push(85 + w * 8);
                        } else if number(row, 85 + w * 8).is_some_and(|v| v > 0.0) {
                            cols.push(164 + w * 2);
                        }
                    }
                }
            }
            fields.extend(cols.into_iter().map(|c| (name.clone(), c)));
        }
        fields
    }

    fn partgeo(game: &Path, backup: &BackupManager) -> bool {
        let key = "SYSTEM/DATA_MP/DEFAULTS/partgeo.dau";
        let valid = || {
            let original = backup.get_backup_bytes(key).ok()?;
            let current = rows(&fs::read(game.join(key)).ok()?, 2)?;
            let expected = crate::partgeo::PartgeoPatcher::get_patched_bytes(
                original,
                &crate::partgeo::PartgeoOptions {
                    projectile_arc_height: true,
                },
            )
            .ok()?;
            let original = rows(original, 2)?;
            let expected = rows(&expected, 2)?;
            let fields = crate::partgeo::PROJECTILE_GEO_NAMES
                .iter()
                .map(|n| (n.to_string(), 12))
                .collect();
            Some(matches_fields(&current, &expected, &original, &fields))
        };
        valid() == Some(true)
    }

    fn epara(game: &Path, backup: &BackupManager) -> bool {
        let valid = || {
            let key = "SYSTEM/cl_epara.ini";
            let original = backup.get_backup_bytes(key).ok()?;
            let expected = crate::epara::EparaPatcher::get_patched_bytes(original, true).ok()?;
            let current = fs::read(game.join(key)).ok()?;
            let values = |bytes: &[u8]| -> Option<Vec<f64>> {
                let doc = crate::patch_text::TextDocument::read(bytes).ok()?;
                let mut out = Vec::new();
                for section in [
                    crate::epara::VARIANCE_SECTION,
                    crate::epara::INIT_SPEED_SECTION,
                ] {
                    let mut matches = doc
                        .lines
                        .iter()
                        .enumerate()
                        .filter(|(_, l)| l.trim().eq_ignore_ascii_case(section));
                    let (i, _) = matches.next()?;
                    if matches.next().is_some() {
                        return None;
                    }
                    let value = doc.lines[i + 1..]
                        .iter()
                        .map(|l| l.trim())
                        .find(|l| !l.is_empty() && !l.starts_with(';'))?;
                    out.push(value.parse::<f64>().ok().filter(|v| v.is_finite())?);
                }
                Some(out)
            };
            Some(values(&current)? == values(&expected)?)
        };
        valid() == Some(true)
    }
}
pub struct FeatureDetector;
impl FeatureDetector {
    pub fn detect(game_path: &Path, backup_manager: &BackupManager) -> PatchProfile {
        let mut profile = PatchProfile::new();
        let exe_roman = ExeFeatureDetector::detect(game_path, &mut profile);
        ClScriptFeatureDetector::detect(game_path, backup_manager, &mut profile);
        RessFeatureDetector::detect(game_path, &mut profile);
        if let (Ok(current), Ok(original)) = (
            fs::read(game_path.join("SYSTEM/ress.ini")),
            backup_manager.get_backup_bytes("SYSTEM/ress.ini"),
        ) {
            if let (Ok(c), Ok(o)) = (
                crate::patch_text::TextDocument::read(&current),
                crate::patch_text::TextDocument::read(original),
            ) {
                RessFeatureDetector::detect_relative(&c.text, Some(&o.text), &mut profile);
            } else {
                for id in RessFeatureDetector::IDS {
                    profile.set_bool(id, false);
                }
            }
        }
        ObjdefFeatureDetector::detect(game_path, backup_manager, &mut profile);

        let (max_pop, roman_player) = TeamDatFeatureDetector::detect(game_path, backup_manager);
        profile.set_bool("MaxPopulation", max_pop);
        // 此開關需要 EXE 與所有無盡地圖一致；單一來源不是整項成功證據。
        profile.set_bool(
            "RomanEndless",
            exe_roman == ExeRomanEndlessPatchState::Patched && roman_player,
        );
        let mut orchestrator = EndlessAiOrchestrator::new(false);
        BciFeatureDetector::detect(game_path, &mut profile, &mut orchestrator);
        InstallFeatureDetector::detect(game_path, &mut profile);

        profile
    }
}
