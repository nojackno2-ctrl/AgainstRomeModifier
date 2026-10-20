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
    ress,
    services::BackupManager,
    team_dat::{TeamDatOptions, TeamDatPatcher},
    troops::{TroopConfig, UnitStatParser},
    ObjdefIndex, PatchProfile,
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
    pub fn detect(game_path: &Path, backup: &BackupManager, profile: &mut PatchProfile) {
        let path = game_path.join("SYSTEM/cl_script.ini");
        if !path.is_file() {
            return;
        }
        let Ok(bytes) = fs::read(&path) else { return };
        let decode = |b: &[u8]| match crate::decompress_if_pfil(b) {
            Ok(d) => crate::decode_cp1251(&d),
            Err(_) => crate::decode_cp1251(b),
        };
        let text = decode(&bytes);
        let original = if backup.has_file("SYSTEM/cl_script.ini") {
            backup
                .get_backup_bytes("SYSTEM/cl_script.ini")
                .ok()
                .map(decode)
        } else {
            None
        };
        Self::detect_text(&text, original.as_deref(), profile);
    }

    /// Mirrors the C# ClScriptFeatureDetector regexes: unanchored first-match for the
    /// morale lines (digits only, Int32 range), all-matches for CiviDelay, line-anchored
    /// case-sensitive matching for Value/Radius lines, and a missing resurrection key
    /// in the current script means "not detected".
    pub fn detect_text(text: &str, original: Option<&str>, profile: &mut PatchProfile) {
        let civi = Self::civi_delays(text);
        profile.set_bool(
            "FastCiviProduction",
            !civi.is_empty() && civi.iter().all(|&d| d > 0.0 && d <= 500.0),
        );

        let lost = Self::first_morale(text, "MoralsDecLostMem");
        let flee = Self::first_morale(text, "MoralsDecFlee");
        let over_pop = Self::first_morale(text, "MoralsDecOverPop");
        let idle = Self::first_morale(text, "MoralsIncIdle");
        let infinite = matches!(
            (lost, flee, over_pop, idle),
            (Some(Some(0)), Some(Some(0)), Some(Some(op)), Some(Some(500))) if op >= 99_999_999
        );
        profile.set_bool("InfiniteMorale", infinite);

        let Some(orig_text) = original else { return };

        let current_spells = Self::read_keyed_values(text, "Spell");
        let orig_spells = Self::read_keyed_values(orig_text, "Spell");
        let current_radius = Self::read_radius_values(text);
        let orig_radius = Self::read_radius_values(orig_text);

        let damage_keys: Vec<String> = crate::cl_script::DAMAGE_SPELL_KEYS
            .iter()
            .map(|k| format!("Value_{k}"))
            .collect();
        let damage_refs: Vec<&str> = damage_keys.iter().map(String::as_str).collect();
        profile.set_bool(
            "SpellDamage5x",
            Self::all_scaled(&current_spells, &orig_spells, &damage_refs, 5),
        );
        profile.set_bool(
            "SpellHealing10x",
            Self::all_scaled(&current_spells, &orig_spells, &["Value_KEL_Spell1"], 10),
        );
        profile.set_bool(
            "SpellResurrection",
            Self::detect_resurrection(&current_spells, &orig_spells),
        );
        profile.set_bool(
            "SpellRange3x",
            Self::radius_scaled_by_three(&current_radius, &orig_radius),
        );

        let current_abilities = Self::read_keyed_values(text, "SAbility");
        let orig_abilities = Self::read_keyed_values(orig_text, "SAbility");
        let ability_keys: Vec<_> = orig_abilities.keys().cloned().collect();
        let ability_refs: Vec<&str> = ability_keys.iter().map(String::as_str).collect();
        profile.set_bool(
            "GeneralSkills",
            Self::all_scaled(&current_abilities, &orig_abilities, &ability_refs, 5),
        );
    }

    fn take_digits(s: &str) -> Option<(&str, &str)> {
        let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
        if end == 0 {
            None
        } else {
            Some(s.split_at(end))
        }
    }

    /// `name\s*=\s*` at the start of `s`, returning the remainder.
    fn assign<'a>(s: &'a str, name: &str) -> Option<&'a str> {
        let rest = s.strip_prefix(name)?.trim_start().strip_prefix('=')?;
        Some(rest.trim_start())
    }

    /// `([A-Z]{3})\s*,\s*` -> (faction, remainder after the comma).
    fn faction_comma(s: &str) -> Option<(&str, &str)> {
        let faction = s
            .get(..3)
            .filter(|f| f.bytes().all(|b| b.is_ascii_uppercase()))?;
        let rest = s[3..].trim_start().strip_prefix(',')?;
        Some((faction, rest.trim_start()))
    }

    /// C# `CiviDelay\s*=\s*([A-Z]{3})\s*,\s*(\d+)`, every match in the text.
    fn civi_delays(text: &str) -> Vec<f64> {
        text.match_indices("CiviDelay")
            .filter_map(|(pos, _)| {
                let rest = Self::assign(&text[pos..], "CiviDelay")?;
                let (_, rest) = Self::faction_comma(rest)?;
                let (digits, _) = Self::take_digits(rest)?;
                digits.parse::<f64>().ok()
            })
            .collect()
    }

    /// First match of `name\s*=\s*GER\s*,\s*(\d+)`: `None` = no match,
    /// `Some(None)` = matched but not a valid Int32 (C# int.TryParse fails).
    fn first_morale(text: &str, name: &str) -> Option<Option<i32>> {
        text.match_indices(name).find_map(|(pos, _)| {
            let rest = Self::assign(&text[pos..], name)?.strip_prefix("GER")?;
            let rest = rest.trim_start().strip_prefix(',')?.trim_start();
            let (digits, _) = Self::take_digits(rest)?;
            Some(digits.parse::<i32>().ok())
        })
    }

    /// Line `^(<head>)\s*=\s*([A-Z]{3})\s*,\s*(<tag>\d+)\s*,\s*([^;\r\n]+)` with no leading
    /// whitespace allowed; `head_digits` allows the `Value\d*` suffix. Returns
    /// (head, faction, spell, raw value).
    fn parse_value_line<'a>(
        line: &'a str,
        head: &str,
        head_digits: bool,
        tag: &str,
    ) -> Option<(&'a str, &'a str, &'a str, &'a str)> {
        let after_head = line.strip_prefix(head)?;
        let suffix = if head_digits {
            after_head.len()
                - after_head
                    .trim_start_matches(|c: char| c.is_ascii_digit())
                    .len()
        } else {
            0
        };
        let head_full = &line[..head.len() + suffix];
        let rest = after_head[suffix..]
            .trim_start()
            .strip_prefix('=')?
            .trim_start();
        let (faction, rest) = Self::faction_comma(rest)?;
        let (digits, after) = Self::take_digits(rest.strip_prefix(tag)?)?;
        let spell = &rest[..tag.len() + digits.len()];
        let after = after.trim_start().strip_prefix(',')?.trim_start();
        let end = after.find([';', '\r', '\n']).unwrap_or(after.len());
        if end == 0 {
            return None;
        }
        Some((head_full, faction, spell, &after[..end]))
    }

    fn parse_value(raw: &str) -> Option<f64> {
        raw.trim().parse::<f64>().ok().filter(|v| v.is_finite())
    }

    fn read_keyed_values(text: &str, tag: &str) -> HashMap<String, f64> {
        let mut map = HashMap::new();
        for line in text.split('\n') {
            let Some((head, faction, spell, raw)) =
                Self::parse_value_line(line, "Value", true, tag)
            else {
                continue;
            };
            if let Some(v) = Self::parse_value(raw) {
                map.insert(format!("{head}_{faction}_{spell}").to_ascii_uppercase(), v);
            }
        }
        map
    }

    fn read_radius_values(text: &str) -> HashMap<String, f64> {
        let mut map = HashMap::new();
        for line in text.split('\n') {
            let Some((_, faction, spell, raw)) =
                Self::parse_value_line(line, "Radius", false, "Spell")
            else {
                continue;
            };
            if let Some(v) = Self::parse_value(raw) {
                map.insert(format!("Radius_{faction}_{spell}").to_ascii_uppercase(), v);
            }
        }
        map
    }

    fn all_scaled(
        current: &HashMap<String, f64>,
        orig: &HashMap<String, f64>,
        keys: &[&str],
        mult: i32,
    ) -> bool {
        let mut any = false;
        for &key in keys {
            let upper = key.to_ascii_uppercase();
            let Some(&orig_v) = orig.get(&upper) else {
                continue;
            };
            if orig_v <= 0.0 {
                continue;
            }
            let expected = (orig_v * mult as f64) as i64;
            if expected == orig_v as i64 {
                continue;
            }
            match current.get(&upper) {
                Some(&cur_v) if cur_v as i64 == expected => any = true,
                _ => return false,
            }
        }
        any
    }

    fn radius_scaled_by_three(current: &HashMap<String, f64>, orig: &HashMap<String, f64>) -> bool {
        let mut any = false;
        for (key, &orig_v) in orig {
            if orig_v <= 0.0 {
                continue;
            }
            let Some(&cur_v) = current.get(key) else {
                return false;
            };
            let ratio = cur_v / orig_v;
            if (ratio - 3.0).abs() > 0.05 && (ratio - 7.5).abs() > 0.1 {
                return false;
            }
            any = true;
        }
        any
    }

    fn detect_resurrection(current: &HashMap<String, f64>, orig: &HashMap<String, f64>) -> bool {
        let mut any = false;
        for key in ["VALUE_KEL_SPELL3", "VALUE2_KEL_SPELL3"] {
            let Some(&orig_v) = orig.get(key) else {
                continue;
            };
            if orig_v as i64 == 100 {
                continue;
            }
            match current.get(key) {
                Some(&cur_v) if cur_v as i64 == 100 => any = true,
                _ => return false,
            }
        }
        any
    }
}

pub struct RessFeatureDetector;
impl RessFeatureDetector {
    pub fn detect(game_path: &Path, profile: &mut PatchProfile) {
        let path = game_path.join("SYSTEM/ress.ini");
        if !path.is_file() {
            return;
        }
        let Ok(bytes) = fs::read(&path) else { return };
        let text = match crate::decompress_if_pfil(&bytes) {
            Ok(b) => crate::decode_cp1251(&b),
            Err(_) => crate::decode_cp1251(&bytes),
        };

        Self::detect_text(&text, profile);
    }

    /// Mirrors C# RessFeatureDetector: only the first matching line of each kind is
    /// inspected, using the RessIndex/VolkresIndex ranges that RessPatcher zeroes.
    pub fn detect_text(text: &str, profile: &mut PatchProfile) {
        let first_line = |matches: &dyn Fn(&str) -> bool| text.split('\n').find(|l| matches(l));
        let starts_with_name = |line: &str, name: &str| {
            line.strip_prefix(name)
                .is_some_and(|rest| rest.trim_start().starts_with(','))
        };

        if let Some(line) = first_line(&|l| starts_with_name(l, "FigRomInf00_Lanze_Schild")) {
            let free = Self::all_zero(
                &parse_csv_line(line),
                ress::FIG_PROD_COST_START,
                ress::FIG_PROD_COST_END,
            );
            profile.set_bool("FreeProduction", free);
        }
        if let Some(line) = first_line(&|l| l.contains("Ger_Kampf")) {
            let free = Self::all_zero(
                &parse_csv_line(line),
                ress::UNIT_UPGRADE_START,
                ress::UNIT_UPGRADE_END,
            );
            profile.set_bool("FreeUpgrade", free);
        }
        if let Some(line) = first_line(&|l| starts_with_name(l, "FigGerPri00_Priester")) {
            let free = Self::all_zero(
                &parse_csv_line(line),
                ress::FIG_PRIEST_SPELL_COST_START,
                ress::FIG_PRIEST_SPELL_COST_END,
            );
            profile.set_bool("NoSpellCost", free);
        }
    }

    fn all_zero(columns: &[String], first: usize, last: usize) -> bool {
        (first..=last).all(|i| columns.get(i).is_some_and(|c| c.trim() == "0"))
    }
}

pub struct TeamDatFeatureDetector;
impl TeamDatFeatureDetector {
    pub fn detect(game_path: &Path, backup: &BackupManager) -> (bool, bool) {
        let mut max_population = false;
        let mut roman_player = false;

        for (key, original) in backup.backup_files() {
            let lower = key.to_ascii_lowercase();
            if !lower.starts_with("maps/") || !lower.ends_with("team.dat") {
                continue;
            }
            let path = game_path.join(key);
            if !path.is_file() {
                continue;
            }
            let Ok(current) = fs::read(&path) else {
                continue;
            };
            let is_endl = lower.starts_with("maps/endl_");

            if let Some((max_pop, rom_p)) = Self::try_detect_options(original, &current, is_endl) {
                max_population |= max_pop;
                roman_player |= rom_p;
            } else {
                max_population = true;
            }
        }

        (max_population, roman_player)
    }

    fn try_detect_options(original: &[u8], current: &[u8], is_endl: bool) -> Option<(bool, bool)> {
        for (candidate_pop, candidate_rom) in
            [(false, false), (true, false), (false, true), (true, true)]
        {
            let effective_rom = candidate_rom && is_endl;
            if let Ok(candidate) = TeamDatPatcher::get_patched_bytes(
                original,
                &TeamDatOptions {
                    max_population: candidate_pop,
                    roman_player: effective_rom,
                    ..Default::default()
                },
            ) {
                if current == candidate {
                    return Some((candidate_pop, effective_rom));
                }
            }
        }
        None
    }
}

pub struct ObjdefFeatureDetector;
impl ObjdefFeatureDetector {
    pub fn detect(game_path: &Path, backup: &BackupManager, profile: &mut PatchProfile) {
        let path = game_path.join("SYSTEM/DATA_MP/DEFAULTS/objdef.dau");
        if !path.is_file() {
            return;
        }
        let Ok(bytes) = fs::read(&path) else { return };
        let Ok(raw) = crate::decompress_pfil(&bytes) else {
            return;
        };
        let current_text = crate::decode_cp1251(&raw);
        let current_rows: Vec<Vec<String>> = current_text
            .lines()
            .filter(|l| l.len() >= 100)
            .map(parse_csv_line)
            .collect();

        // C# only runs the backup-relative checks (and sets their flags) when the backup exists;
        // the remaining flags are always assigned, with an empty original-row set otherwise.
        const BACKUP_KEY: &str = "SYSTEM/DATA_MP/DEFAULTS/objdef.dau";
        let mut orig_rows: Vec<Vec<String>> = Vec::new();
        if backup.has_file(BACKUP_KEY) {
            let Ok(orig_bytes) = backup.get_backup_bytes(BACKUP_KEY) else {
                return;
            };
            let Ok(orig_raw) = crate::decompress_pfil(orig_bytes) else {
                return;
            };
            let orig_text = crate::decode_cp1251(&orig_raw);
            orig_rows = orig_text
                .lines()
                .filter(|l| l.len() >= 100)
                .map(parse_csv_line)
                .collect();

            profile.set_bool(
                "HousingCapacity20x",
                Self::has_capacity_multiplier(
                    &current_rows,
                    &orig_rows,
                    ObjdefIndex::HousingCapacity as usize,
                    20,
                ),
            );
            profile.set_bool(
                "StorageCapacity10x",
                Self::has_storage_multiplier(&current_rows, &orig_rows, 10),
            );
            profile.set_bool(
                "HqHp10x",
                Self::has_hq_hp_multiplier(&current_rows, &orig_rows, 10),
            );
            profile.set_bool(
                "FastBuildUpgradeRepair",
                Self::has_fast_build(&current_rows, &orig_rows),
            );
            profile.set_bool(
                "ProjectileArcHeight",
                Self::has_projectile_scale(&current_rows, &orig_rows),
            );
            profile.set_bool(
                "NoRunHpLoss",
                Self::has_no_run_hp_loss(&current_rows, &orig_rows),
            );
        }
        let entire_vision = Self::has_entire_map_vision(&current_rows, &orig_rows);
        profile.set_bool("AllUnitsEntireMapVision", entire_vision);

        // Unit rows map
        let mut unit_rows = HashMap::new();
        for r in &current_rows {
            if r.len() >= 192 {
                let name = r[52].trim();
                unit_rows.insert(name.to_ascii_uppercase(), r.clone());
            }
        }
        let orig_unit_rows = backup.get_backup_unit_rows().unwrap_or_default();

        // Range 3x probe
        let mut range3x = false;
        if let Some(r) = unit_rows.get("FIGROMSCH01_BOGEN") {
            let cur = UnitStatParser::get_maximum_range(r, "ranged_inf");
            let orig = backup
                .get_original_stats("FigRomSch01_Bogen")
                .map(|s| s[7])
                .unwrap_or(0.0);
            range3x = cur > 0.0 && (cur - orig * 3.0).abs() < 5.0;
        }

        // Balance check
        let mut is_balanced = false;
        for key in TroopConfig::sorted_units() {
            let upper = key.to_ascii_uppercase();
            if let (Some(cur), Some(orig)) = (unit_rows.get(&upper), orig_unit_rows.get(key)) {
                let cur_hp = cur
                    .get(ObjdefIndex::Hp as usize)
                    .and_then(|s| s.trim().parse::<f64>().ok())
                    .unwrap_or(0.0);
                let orig_hp = orig
                    .get(ObjdefIndex::Hp as usize)
                    .and_then(|s| s.trim().parse::<f64>().ok())
                    .unwrap_or(0.0);
                let cur_vw = cur
                    .get(ObjdefIndex::Vw as usize)
                    .and_then(|s| s.trim().parse::<f64>().ok())
                    .unwrap_or(0.0);
                let orig_vw = orig
                    .get(ObjdefIndex::Vw as usize)
                    .and_then(|s| s.trim().parse::<f64>().ok())
                    .unwrap_or(0.0);
                let cur_aw = cur
                    .get(ObjdefIndex::Aw as usize)
                    .and_then(|s| s.trim().parse::<f64>().ok())
                    .unwrap_or(0.0);
                let orig_aw = orig
                    .get(ObjdefIndex::Aw as usize)
                    .and_then(|s| s.trim().parse::<f64>().ok())
                    .unwrap_or(0.0);

                if TroopConfig::get_metadata(key).map(|m| m.tier) != Some("leader") {
                    if (cur_hp - orig_hp).abs() > 0.01
                        || (cur_vw - orig_vw).abs() > 0.01
                        || (cur_aw - orig_aw).abs() > 0.01
                    {
                        is_balanced = true;
                    }
                }
            }
        }
        profile.set_bool("Balance", is_balanced);

        // Leader glory
        profile.set_bool(
            "LeaderGlory",
            Self::detect_leader_glory(&current_rows, &orig_unit_rows),
        );

        // Speed 2x
        let mut speed2x = false;
        if let Some(r) = unit_rows.get("FIGROMINF00_LANZE_SCHILD") {
            let cur_moves = r
                .get(ObjdefIndex::Moves as usize)
                .and_then(|s| s.trim().parse::<f64>().ok())
                .unwrap_or(0.0);
            let expected_base = backup
                .get_base_stats_for_unit("FigRomInf00_Lanze_Schild", profile)
                .map(|s| s[4] / 2.0)
                .unwrap_or(0.0);
            if cur_moves > 0.0 && (cur_moves - expected_base * 2.0).abs() < 0.05 {
                speed2x = true;
            }
        }
        profile.set_bool("UnitMovementSpeed2x", speed2x);
        profile.set_bool("RangedRange3x", range3x);

        // Villager speed 5x
        let mut villager5x = false;
        if let (Some(cur), Some(orig)) = (
            unit_rows.get("FIGZIVMAN00_ZIVILIST"),
            orig_unit_rows.get("FigZivMan00_Zivilist"),
        ) {
            let cur_moves = cur
                .get(ObjdefIndex::Moves as usize)
                .and_then(|s| s.trim().parse::<f64>().ok())
                .unwrap_or(0.0);
            let orig_moves = orig
                .get(ObjdefIndex::Moves as usize)
                .and_then(|s| s.trim().parse::<f64>().ok())
                .unwrap_or(0.0);
            if cur_moves > 0.0 && (cur_moves - orig_moves * 5.0).abs() < 0.05 {
                villager5x = true;
            }
        }
        profile.set_bool("VillagerMovementSpeed5x", villager5x);

        // Priest spell range entire map
        let mut spell_map = false;
        if let Some(r) = unit_rows.get("FIGKELPRI00_PRIESTER") {
            let cur_range = UnitStatParser::get_maximum_range(r, "priest");
            if cur_range > 0.0 && (cur_range - 30000.0).abs() < 100.0 {
                spell_map = true;
            }
        }
        profile.set_bool("SpellEntireMap", spell_map);
    }

    fn has_capacity_multiplier(
        current: &[Vec<String>],
        orig: &[Vec<String>],
        col: usize,
        mult: i64,
    ) -> bool {
        let mut cur_map = HashMap::new();
        for r in current {
            if r.len() > col && r.len() > 52 {
                if let Ok(v) = r[col].trim().parse::<i64>() {
                    cur_map.insert(r[52].trim().to_ascii_uppercase(), v);
                }
            }
        }
        let mut found = false;
        for r in orig {
            if r.len() > col && r.len() > 52 {
                if let Ok(v) = r[col].trim().parse::<i64>() {
                    if v > 0 {
                        found = true;
                        let name = r[52].trim().to_ascii_uppercase();
                        if cur_map.get(&name) != Some(&(v * mult)) {
                            return false;
                        }
                    }
                }
            }
        }
        found
    }

    fn has_storage_multiplier(current: &[Vec<String>], orig: &[Vec<String>], mult: i64) -> bool {
        let col = ObjdefIndex::StorageCapacity as usize;
        let mut cur_map = HashMap::new();
        for r in current {
            if r.len() > col && r.len() > 52 {
                if let Ok(v) = r[col].trim().parse::<i64>() {
                    cur_map.insert(r[52].trim().to_ascii_uppercase(), v);
                }
            }
        }
        let mut found = false;
        for r in orig {
            if r.len() > col && r.len() > 52 {
                let name = r[52].trim();
                if !name.starts_with("Bau") || !(name.contains("Hau") || name.contains("Lag")) {
                    continue;
                }
                if let Ok(v) = r[col].trim().parse::<i64>() {
                    if v > 0 {
                        found = true;
                        if cur_map.get(&name.to_ascii_uppercase()) != Some(&(v * mult)) {
                            return false;
                        }
                    }
                }
            }
        }
        found
    }

    fn has_hq_hp_multiplier(current: &[Vec<String>], orig: &[Vec<String>], mult: i64) -> bool {
        let col = ObjdefIndex::Hp as usize;
        let mut cur_map = HashMap::new();
        for r in current {
            if r.len() > col && r.len() > 52 {
                if let Ok(v) = r[col].trim().parse::<i64>() {
                    cur_map.insert(r[52].trim().to_ascii_uppercase(), v);
                }
            }
        }
        let mut found = false;
        for r in orig {
            if r.len() > col && r.len() > 52 {
                let name = r[52].trim();
                if !name.starts_with("Bau") || !name.contains("Hau") {
                    continue;
                }
                if let Ok(v) = r[col].trim().parse::<i64>() {
                    if v > 0 {
                        found = true;
                        if cur_map.get(&name.to_ascii_uppercase()) != Some(&(v * mult)) {
                            return false;
                        }
                    }
                }
            }
        }
        found
    }

    fn has_fast_build(current: &[Vec<String>], orig: &[Vec<String>]) -> bool {
        let mut cur_build = HashMap::new();
        let mut cur_upg = HashMap::new();
        for r in current {
            if r.len() >= 192 && r[52].trim().starts_with("Bau") {
                let name = r[52].trim().to_ascii_uppercase();
                if let Ok(b) = r[73].trim().parse::<i64>() {
                    cur_build.insert(name.clone(), b);
                }
                if let Ok(u) = r[74].trim().parse::<i64>() {
                    cur_upg.insert(name, u);
                }
            }
        }
        let mut found = false;
        for r in orig {
            if r.len() >= 192 && r[52].trim().starts_with("Bau") {
                let name = r[52].trim().to_ascii_uppercase();
                let b = r[73].trim().parse::<i64>().ok().filter(|&x| x > 0);
                let u = r[74].trim().parse::<i64>().ok().filter(|&x| x > 0);
                if b.is_some() || u.is_some() {
                    found = true;
                    if let Some(bv) = b {
                        let expected = (bv / 10).max(1);
                        if cur_build.get(&name) != Some(&expected) {
                            return false;
                        }
                    }
                    if let Some(uv) = u {
                        let expected = (uv / 10).max(1);
                        if cur_upg.get(&name) != Some(&expected) {
                            return false;
                        }
                    }
                }
            }
        }
        found
    }

    fn has_projectile_scale(current: &[Vec<String>], orig: &[Vec<String>]) -> bool {
        let mut cur_map = HashMap::new();
        for r in current {
            if r.len() > 52 {
                cur_map.insert(r[52].trim().to_ascii_uppercase(), r);
            }
        }
        let mut any = false;
        for orig_r in orig {
            if orig_r.len() <= 52 {
                continue;
            }
            let name = orig_r[52].trim().to_ascii_uppercase();
            let Some(cur_r) = cur_map.get(&name) else {
                continue;
            };
            for w in 1..=8 {
                let active = ObjdefIndex::Weapon1Akti as usize + (w - 1) * 8;
                let emit = ObjdefIndex::Weapon1Emit as usize + (w - 1) * 8;
                if emit >= orig_r.len() || emit >= cur_r.len() {
                    continue;
                }
                if orig_r[active].trim() != "1" {
                    continue;
                }
                let Ok(orig_emit) = orig_r[emit].trim().parse::<i64>() else {
                    continue;
                };
                let Ok(cur_emit) = cur_r[emit].trim().parse::<i64>() else {
                    continue;
                };
                if orig_emit <= 0 {
                    continue;
                }
                let expected = (orig_emit as f64 * 2.0).round() as i64;
                if expected == orig_emit {
                    continue;
                }
                if cur_emit != expected {
                    return false;
                }
                any = true;
            }
        }
        any
    }

    fn has_no_run_hp_loss(current: &[Vec<String>], orig: &[Vec<String>]) -> bool {
        let probe = "FIGROMINF00_LANZE_SCHILD";
        let cur = current
            .iter()
            .find(|r| r.len() > 52 && r[52].trim().eq_ignore_ascii_case(probe));
        let orig = orig
            .iter()
            .find(|r| r.len() > 52 && r[52].trim().eq_ignore_ascii_case(probe));
        if let (Some(c), Some(o)) = (cur, orig) {
            let col = ObjdefIndex::Lpsub as usize;
            if c.len() > col && o.len() > col {
                let cur_lp = c[col].trim().parse::<f64>().unwrap_or(1.0);
                let orig_lp = o[col].trim().parse::<f64>().unwrap_or(0.0);
                return orig_lp > 0.0 && cur_lp.abs() < 0.01;
            }
        }
        false
    }

    fn has_entire_map_vision(current: &[Vec<String>], orig: &[Vec<String>]) -> bool {
        let mut cur_map = HashMap::new();
        for r in current {
            if r.len() > ObjdefIndex::Sirad as usize && r.len() > 52 {
                let name = r[52].trim();
                if ObjdefPatcher::supports_entire_map_vision(name) {
                    cur_map.insert(name.to_ascii_uppercase(), r);
                }
            }
        }
        let mut expected = 0;
        for r in orig {
            if r.len() > ObjdefIndex::Sirad as usize && r.len() > 52 {
                let name = r[52].trim();
                if !ObjdefPatcher::supports_entire_map_vision(name) {
                    continue;
                }
                expected += 1;
                let Some(c) = cur_map.get(&name.to_ascii_uppercase()) else {
                    return false;
                };
                let sight = c[ObjdefIndex::Sirad as usize]
                    .trim()
                    .parse::<f64>()
                    .unwrap_or(0.0);
                if (sight - 30000.0).abs() >= 100.0 {
                    return false;
                }
            }
        }
        expected > 0
    }

    fn detect_leader_glory(
        current: &[Vec<String>],
        orig_map: &HashMap<String, Vec<String>>,
    ) -> bool {
        let leaders = [
            "FigGerAnf00_Anfuehrer",
            "FigKelAnf00_Anfuehrer",
            "FigHunAnf00_Anfuehrer",
            "FigRomAnf00_Anfuehrer",
        ];
        let glory_cols = [
            102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116,
        ];
        let mut any = false;

        for leader in leaders {
            let Some(orig) = orig_map.get(leader) else {
                continue;
            };
            let Some(cur) = current
                .iter()
                .find(|r| r.len() > 52 && r[52].trim().eq_ignore_ascii_case(leader))
            else {
                continue;
            };
            for &idx in &glory_cols {
                if idx >= orig.len() || idx >= cur.len() {
                    continue;
                }
                let Ok(orig_v) = orig[idx].trim().parse::<f64>() else {
                    continue;
                };
                if orig_v <= 0.0 {
                    continue;
                }
                let Ok(cur_v) = cur[idx].trim().parse::<f64>() else {
                    return false;
                };
                if (cur_v - orig_v * 5.0).abs() > 0.01 {
                    return false;
                }
                any = true;
            }
        }
        any
    }
}

pub struct FeatureDetector;
impl FeatureDetector {
    pub fn detect(game_path: &Path, backup_manager: &BackupManager) -> PatchProfile {
        let mut profile = PatchProfile::new();
        let exe_roman = ExeFeatureDetector::detect(game_path, &mut profile);
        ClScriptFeatureDetector::detect(game_path, backup_manager, &mut profile);
        RessFeatureDetector::detect(game_path, &mut profile);
        ObjdefFeatureDetector::detect(game_path, backup_manager, &mut profile);

        let (max_pop, roman_player) = TeamDatFeatureDetector::detect(game_path, backup_manager);
        profile.set_bool("MaxPopulation", max_pop);
        if exe_roman == ExeRomanEndlessPatchState::Unknown {
            profile.set_bool("RomanEndless", roman_player);
        } else {
            profile.set_bool(
                "RomanEndless",
                exe_roman == ExeRomanEndlessPatchState::Patched,
            );
        }

        let mut orchestrator = EndlessAiOrchestrator::new(false);
        BciFeatureDetector::detect(game_path, &mut profile, &mut orchestrator);
        InstallFeatureDetector::detect(game_path, &mut profile);

        profile
    }
}
