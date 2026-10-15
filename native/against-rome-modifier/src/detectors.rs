//! Feature detectors ported from src.Core/Core/Features.
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};
use crate::{
    bci_features::BciFeatureDetector,
    cl_script::ClScriptPatcher,
    endless_ai::EndlessAiOrchestrator,
    exe_features::{self, ExePatchState, ExeRomanEndlessPatchState, ExeSpellAltarPatchState, ExeVillageSetterPatchState},
    install::InstallFeatureDetector,
    objdef::ObjdefPatcher,
    partgeo,
    patch_text::{self, parse_csv_line},
    ress::RessPatcher,
    services::BackupManager,
    team_dat::{TeamDatOptions, TeamDatPatcher},
    troops::{TroopConfig, UnitStatParser},
    ObjdefIndex, PatchProfile,
};

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
            profile.set_bool("VillageBuildRange", setter != ExeVillageSetterPatchState::Original);
        }

        let altar = exe_features::get_spell_altar_patch_state(&bytes);
        if altar != ExeSpellAltarPatchState::Unknown {
            profile.set_bool("NoSpellAltar", altar == ExeSpellAltarPatchState::Patched);
        }

        let speed = exe_features::get_game_speed_multiplier(&bytes);
        if speed > 0 {
            profile.set_int("GameSpeed", speed);
        }

        profile.set_bool("CiviProduce20", exe_features::get_civi_produce_20_patch_state(&bytes) == exe_features::ExeCiviProduce20PatchState::Patched);
        profile.set_bool("UnitRecruit20", exe_features::get_unit_recruit_20_patch_state(&bytes) == exe_features::ExeUnitRecruit20PatchState::Patched);
        profile.set_bool("IdleSelect999", exe_features::get_idle_select_999_patch_state(&bytes) == exe_features::ExeIdleSelect999PatchState::Patched);
        profile.set_bool("CorpseRetention", exe_features::get_corpse_retention_patch_state(&bytes) == exe_features::ExeCorpseRetentionPatchState::Patched);

        let ws = exe_features::get_native_widescreen_patch_state(&bytes);
        profile.set_bool("NativeWidescreen1920x1080", matches!(ws, exe_features::ExeNativeWidescreenPatchState::LegacyUnforced | exe_features::ExeNativeWidescreenPatchState::LegacyForcedStaleUi | exe_features::ExeNativeWidescreenPatchState::Patched));

        let zoom = exe_features::get_camera_zoom_out_patch_state(&bytes);
        profile.set_bool("CameraZoomOut1", matches!(zoom, exe_features::ExeCameraZoomOutPatchState::LegacyZoomHalf | exe_features::ExeCameraZoomOutPatchState::Patched));

        exe_features::get_roman_endless_patch_state(&bytes)
    }
}

pub struct ClScriptFeatureDetector;
impl ClScriptFeatureDetector {
    pub fn detect(game_path: &Path, backup: &BackupManager, profile: &mut PatchProfile) {
        let path = game_path.join("SYSTEM/cl_script.ini");
        if !path.is_file() { return; }
        let Ok(bytes) = fs::read(&path) else { return };
        let text = match crate::decompress_if_pfil(&bytes) {
            Ok(b) => crate::decode_cp1251(&b),
            Err(_) => crate::decode_cp1251(&bytes),
        };

        // Parse CiviDelay
        let mut civi_delays = Vec::new();
        for line in text.lines() {
            let t = line.trim();
            if let Some((left, right)) = t.split_once('=') {
                if left.trim().eq_ignore_ascii_case("CiviDelay") {
                    let parts: Vec<_> = right.split(',').collect();
                    if parts.len() >= 2 {
                        if let Ok(d) = parts[1].trim().parse::<f64>() {
                            civi_delays.push(d);
                        }
                    }
                }
            }
        }
        if !civi_delays.is_empty() && civi_delays.iter().all(|&d| d > 0.0 && d <= 500.0) {
            profile.set_bool("FastCiviProduction", true);
        }

        // Parse Morals
        let mut lost = None;
        let mut flee = None;
        let mut over_pop = None;
        let mut idle = None;
        for line in text.lines() {
            let t = line.trim();
            if let Some((left, right)) = t.split_once('=') {
                let l = left.trim();
                let parts: Vec<_> = right.split(',').collect();
                if parts.len() >= 2 && parts[0].trim().eq_ignore_ascii_case("GER") {
                    let val = parts[1].trim().parse::<i64>().ok();
                    if l.eq_ignore_ascii_case("MoralsDecLostMem") { lost = val; }
                    else if l.eq_ignore_ascii_case("MoralsDecFlee") { flee = val; }
                    else if l.eq_ignore_ascii_case("MoralsDecOverPop") { over_pop = val; }
                    else if l.eq_ignore_ascii_case("MoralsIncIdle") { idle = val; }
                }
            }
        }
        if lost == Some(0) && flee == Some(0) && over_pop.is_some_and(|v| v >= 99999999) && idle == Some(500) {
            profile.set_bool("InfiniteMorale", true);
        }

        if !backup.has_file("SYSTEM/cl_script.ini") { return; }
        let Ok(orig_bytes) = backup.get_backup_bytes("SYSTEM/cl_script.ini") else { return };
        let orig_text = match crate::decompress_if_pfil(orig_bytes) {
            Ok(b) => crate::decode_cp1251(&b),
            Err(_) => crate::decode_cp1251(orig_bytes),
        };

        let current_spells = Self::read_keyed_values(&text, "Value");
        let orig_spells = Self::read_keyed_values(&orig_text, "Value");
        let current_radius = Self::read_radius_values(&text);
        let orig_radius = Self::read_radius_values(&orig_text);

        profile.set_bool("SpellDamage5x", Self::all_scaled(&current_spells, &orig_spells, &["Value_GER_Spell3", "Value_KEL_Spell2", "Value_HUN_Spell3"], 5));
        profile.set_bool("SpellHealing10x", Self::all_scaled(&current_spells, &orig_spells, &["Value_KEL_Spell1"], 10));
        profile.set_bool("SpellResurrection", Self::detect_resurrection(&current_spells, &orig_spells));
        profile.set_bool("SpellRange3x", Self::radius_scaled_by_three(&current_radius, &orig_radius));

        let current_abilities = Self::read_keyed_values(&text, "Value");
        let orig_abilities = Self::read_keyed_values(&orig_text, "Value");
        let ability_keys: Vec<_> = orig_abilities.keys().filter(|k| k.contains("SAbility")).cloned().collect();
        let ability_refs: Vec<&str> = ability_keys.iter().map(String::as_str).collect();
        profile.set_bool("GeneralSkills", Self::all_scaled(&current_abilities, &orig_abilities, &ability_refs, 5));
    }

    fn read_keyed_values(text: &str, prefix: &str) -> HashMap<String, f64> {
        let mut map = HashMap::new();
        for line in text.lines() {
            let t = line.trim();
            if let Some((left, right)) = t.split_once('=') {
                let l = left.trim();
                if l.starts_with(prefix) {
                    let parts: Vec<_> = right.split(',').collect();
                    if parts.len() >= 3 {
                        let faction = parts[0].trim();
                        let spell = parts[1].trim();
                        let val = parts[2].trim().split(';').next().unwrap_or("").trim().parse::<f64>();
                        if let Ok(v) = val {
                            map.insert(format!("{l}_{faction}_{spell}").to_ascii_uppercase(), v);
                        }
                    }
                }
            }
        }
        map
    }

    fn read_radius_values(text: &str) -> HashMap<String, f64> {
        let mut map = HashMap::new();
        for line in text.lines() {
            let t = line.trim();
            if let Some((left, right)) = t.split_once('=') {
                if left.trim().eq_ignore_ascii_case("Radius") {
                    let parts: Vec<_> = right.split(',').collect();
                    if parts.len() >= 3 {
                        let faction = parts[0].trim();
                        let spell = parts[1].trim();
                        let val = parts[2].trim().split(';').next().unwrap_or("").trim().parse::<f64>();
                        if let Ok(v) = val {
                            map.insert(format!("Radius_{faction}_{spell}").to_ascii_uppercase(), v);
                        }
                    }
                }
            }
        }
        map
    }

    fn all_scaled(current: &HashMap<String, f64>, orig: &HashMap<String, f64>, keys: &[&str], mult: i32) -> bool {
        let mut any = false;
        for &key in keys {
            let upper = key.to_ascii_uppercase();
            if let Some(&orig_v) = orig.get(&upper) {
                if orig_v <= 0.0 { continue; }
                let expected = (orig_v * mult as f64) as i64;
                if expected == orig_v as i64 { continue; }
                if let Some(&cur_v) = current.get(&upper) {
                    if cur_v as i64 != expected { return false; }
                    any = true;
                } else {
                    return false;
                }
            }
        }
        any
    }

    fn radius_scaled_by_three(current: &HashMap<String, f64>, orig: &HashMap<String, f64>) -> bool {
        let mut any = false;
        for (key, &orig_v) in orig {
            if orig_v <= 0.0 { continue; }
            if let Some(&cur_v) = current.get(key) {
                let ratio = cur_v / orig_v;
                if (ratio - 3.0).abs() > 0.05 && (ratio - 7.5).abs() > 0.1 { return false; }
                any = true;
            } else {
                return false;
            }
        }
        any
    }

    fn detect_resurrection(current: &HashMap<String, f64>, orig: &HashMap<String, f64>) -> bool {
        let mut any = false;
        for key in ["VALUE_KEL_SPELL3", "VALUE2_KEL_SPELL3"] {
            if let Some(&orig_v) = orig.get(key) {
                if orig_v as i64 == 100 { continue; }
                if let Some(&cur_v) = current.get(key) {
                    if cur_v as i64 != 100 { return false; }
                    any = true;
                }
            }
        }
        any
    }
}

pub struct RessFeatureDetector;
impl RessFeatureDetector {
    pub fn detect(game_path: &Path, profile: &mut PatchProfile) {
        let path = game_path.join("SYSTEM/ress.ini");
        if !path.is_file() { return; }
        let Ok(bytes) = fs::read(&path) else { return };
        let text = match crate::decompress_if_pfil(&bytes) {
            Ok(b) => crate::decode_cp1251(&b),
            Err(_) => crate::decode_cp1251(&bytes),
        };

        for line in text.lines() {
            let cols = parse_csv_line(line);
            if cols.is_empty() { continue; }
            let name = cols[0].trim();
            if name.eq_ignore_ascii_case("FigRomInf00_Lanze_Schild") {
                if cols.len() > 10 && cols[4..=10].iter().all(|c| c.trim() == "0") {
                    profile.set_bool("FreeProduction", true);
                }
            } else if name.contains("Ger_Kampf") {
                if cols.len() > 7 && cols[1..=7].iter().all(|c| c.trim() == "0") {
                    profile.set_bool("FreeUpgrade", true);
                }
            } else if name.eq_ignore_ascii_case("FigGerPri00_Priester") {
                if cols.len() > 14 && cols[11..=14].iter().all(|c| c.trim() == "0") {
                    profile.set_bool("NoSpellCost", true);
                }
            }
        }
    }
}

pub struct TeamDatFeatureDetector;
impl TeamDatFeatureDetector {
    pub fn detect(game_path: &Path, backup: &BackupManager) -> (bool, bool) {
        let mut max_population = false;
        let mut roman_player = false;

        for (key, original) in backup.backup_files() {
            let lower = key.to_ascii_lowercase();
            if !lower.starts_with("maps/") || !lower.ends_with("team.dat") { continue; }
            let path = game_path.join(key);
            if !path.is_file() { continue; }
            let Ok(current) = fs::read(&path) else { continue };
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
        for (candidate_pop, candidate_rom) in [(false, false), (true, false), (false, true), (true, true)] {
            let effective_rom = candidate_rom && is_endl;
            if let Ok(candidate) = TeamDatPatcher::get_patched_bytes(
                original,
                &TeamDatOptions { max_population: candidate_pop, roman_player: effective_rom, ..Default::default() },
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
        if !path.is_file() { return; }
        let Ok(bytes) = fs::read(&path) else { return };
        let Ok(raw) = crate::decompress_pfil(&bytes) else { return };
        let current_text = crate::decode_cp1251(&raw);
        let current_rows: Vec<Vec<String>> = current_text.lines().filter(|l| l.len() >= 100).map(parse_csv_line).collect();

        const BACKUP_KEY: &str = "SYSTEM/DATA_MP/DEFAULTS/objdef.dau";
        if !backup.has_file(BACKUP_KEY) { return; }
        let Ok(orig_bytes) = backup.get_backup_bytes(BACKUP_KEY) else { return };
        let Ok(orig_raw) = crate::decompress_pfil(orig_bytes) else { return };
        let orig_text = crate::decode_cp1251(&orig_raw);
        let orig_rows: Vec<Vec<String>> = orig_text.lines().filter(|l| l.len() >= 100).map(parse_csv_line).collect();

        // 1. Housing capacity 20x
        profile.set_bool("HousingCapacity20x", Self::has_capacity_multiplier(&current_rows, &orig_rows, ObjdefIndex::HousingCapacity as usize, 20));
        // 2. Storage capacity 10x
        profile.set_bool("StorageCapacity10x", Self::has_storage_multiplier(&current_rows, &orig_rows, 10));
        // 3. HQ HP 10x
        profile.set_bool("HqHp10x", Self::has_hq_hp_multiplier(&current_rows, &orig_rows, 10));
        // 4. Fast build upgrade repair
        profile.set_bool("FastBuildUpgradeRepair", Self::has_fast_build(&current_rows, &orig_rows));
        // 5. Projectile arc height
        profile.set_bool("ProjectileArcHeight", Self::has_projectile_scale(&current_rows, &orig_rows));
        // 6. No run HP loss
        profile.set_bool("NoRunHpLoss", Self::has_no_run_hp_loss(&current_rows, &orig_rows));
        // 7. All units entire map vision
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
            let orig = backup.get_original_stats("FigRomSch01_Bogen").map(|s| s[7]).unwrap_or(0.0);
            range3x = cur > 0.0 && (cur - orig * 3.0).abs() < 5.0;
        }

        // Balance check
        let mut is_balanced = false;
        for key in TroopConfig::sorted_units() {
            let upper = key.to_ascii_uppercase();
            if let (Some(cur), Some(orig)) = (unit_rows.get(&upper), orig_unit_rows.get(key)) {
                let utype = TroopConfig::get_metadata(key).map(|m| m.unit_type).unwrap_or("melee_inf");
                let cur_hp = cur.get(ObjdefIndex::Hp as usize).and_then(|s| s.trim().parse::<f64>().ok()).unwrap_or(0.0);
                let orig_hp = orig.get(ObjdefIndex::Hp as usize).and_then(|s| s.trim().parse::<f64>().ok()).unwrap_or(0.0);
                let cur_vw = cur.get(ObjdefIndex::Vw as usize).and_then(|s| s.trim().parse::<f64>().ok()).unwrap_or(0.0);
                let orig_vw = orig.get(ObjdefIndex::Vw as usize).and_then(|s| s.trim().parse::<f64>().ok()).unwrap_or(0.0);
                let cur_aw = cur.get(ObjdefIndex::Aw as usize).and_then(|s| s.trim().parse::<f64>().ok()).unwrap_or(0.0);
                let orig_aw = orig.get(ObjdefIndex::Aw as usize).and_then(|s| s.trim().parse::<f64>().ok()).unwrap_or(0.0);

                if TroopConfig::get_metadata(key).map(|m| m.tier) != Some("leader") {
                    if (cur_hp - orig_hp).abs() > 0.01 || (cur_vw - orig_vw).abs() > 0.01 || (cur_aw - orig_aw).abs() > 0.01 {
                        is_balanced = true;
                    }
                }
            }
        }
        profile.set_bool("Balance", is_balanced);

        // Leader glory
        profile.set_bool("LeaderGlory", Self::detect_leader_glory(&current_rows, &orig_unit_rows));

        // Speed 2x
        let mut speed2x = false;
        if let Some(r) = unit_rows.get("FIGROMINF00_LANZE_SCHILD") {
            let cur_moves = r.get(ObjdefIndex::Moves as usize).and_then(|s| s.trim().parse::<f64>().ok()).unwrap_or(0.0);
            let expected_base = backup.get_base_stats_for_unit("FigRomInf00_Lanze_Schild", profile).map(|s| s[4] / 2.0).unwrap_or(0.0);
            if cur_moves > 0.0 && (cur_moves - expected_base * 2.0).abs() < 0.05 {
                speed2x = true;
            }
        }
        profile.set_bool("UnitMovementSpeed2x", speed2x);
        profile.set_bool("RangedRange3x", range3x);

        // Villager speed 5x
        let mut villager5x = false;
        if let (Some(cur), Some(orig)) = (unit_rows.get("FIGZIVMAN00_ZIVILIST"), orig_unit_rows.get("FigZivMan00_Zivilist")) {
            let cur_moves = cur.get(ObjdefIndex::Moves as usize).and_then(|s| s.trim().parse::<f64>().ok()).unwrap_or(0.0);
            let orig_moves = orig.get(ObjdefIndex::Moves as usize).and_then(|s| s.trim().parse::<f64>().ok()).unwrap_or(0.0);
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

    fn has_capacity_multiplier(current: &[Vec<String>], orig: &[Vec<String>], col: usize, mult: i64) -> bool {
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
                if !name.starts_with("Bau") || !(name.contains("Hau") || name.contains("Lag")) { continue; }
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
                if !name.starts_with("Bau") || !name.contains("Hau") { continue; }
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
                if let Ok(b) = r[73].trim().parse::<i64>() { cur_build.insert(name.clone(), b); }
                if let Ok(u) = r[74].trim().parse::<i64>() { cur_upg.insert(name, u); }
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
                        if cur_build.get(&name) != Some(&expected) { return false; }
                    }
                    if let Some(uv) = u {
                        let expected = (uv / 10).max(1);
                        if cur_upg.get(&name) != Some(&expected) { return false; }
                    }
                }
            }
        }
        found
    }

    fn has_projectile_scale(current: &[Vec<String>], orig: &[Vec<String>]) -> bool {
        let mut cur_map = HashMap::new();
        for r in current {
            if r.len() > 52 { cur_map.insert(r[52].trim().to_ascii_uppercase(), r); }
        }
        let mut any = false;
        for orig_r in orig {
            if orig_r.len() <= 52 { continue; }
            let name = orig_r[52].trim().to_ascii_uppercase();
            let Some(cur_r) = cur_map.get(&name) else { continue };
            for w in 1..=8 {
                let active = ObjdefIndex::Weapon1Akti as usize + (w - 1) * 8;
                let emit = ObjdefIndex::Weapon1Emit as usize + (w - 1) * 8;
                if emit >= orig_r.len() || emit >= cur_r.len() { continue; }
                if orig_r[active].trim() != "1" { continue; }
                let Ok(orig_emit) = orig_r[emit].trim().parse::<i64>() else { continue };
                let Ok(cur_emit) = cur_r[emit].trim().parse::<i64>() else { continue };
                if orig_emit <= 0 { continue; }
                let expected = (orig_emit as f64 * 2.0).round() as i64;
                if expected == orig_emit { continue; }
                if cur_emit != expected { return false; }
                any = true;
            }
        }
        any
    }

    fn has_no_run_hp_loss(current: &[Vec<String>], orig: &[Vec<String>]) -> bool {
        let probe = "FIGROMINF00_LANZE_SCHILD";
        let cur = current.iter().find(|r| r.len() > 52 && r[52].trim().eq_ignore_ascii_case(probe));
        let orig = orig.iter().find(|r| r.len() > 52 && r[52].trim().eq_ignore_ascii_case(probe));
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
                if !ObjdefPatcher::supports_entire_map_vision(name) { continue; }
                expected += 1;
                let Some(c) = cur_map.get(&name.to_ascii_uppercase()) else { return false };
                let sight = c[ObjdefIndex::Sirad as usize].trim().parse::<f64>().unwrap_or(0.0);
                if (sight - 30000.0).abs() >= 100.0 { return false; }
            }
        }
        expected > 0
    }

    fn detect_leader_glory(current: &[Vec<String>], orig_map: &HashMap<String, Vec<String>>) -> bool {
        let leaders = ["FigGerAnf00_Anfuehrer", "FigKelAnf00_Anfuehrer", "FigHunAnf00_Anfuehrer", "FigRomAnf00_Anfuehrer"];
        let glory_cols = [102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116];
        let mut any = false;

        for leader in leaders {
            let Some(orig) = orig_map.get(leader) else { continue };
            let Some(cur) = current.iter().find(|r| r.len() > 52 && r[52].trim().eq_ignore_ascii_case(leader)) else { continue };
            for &idx in &glory_cols {
                if idx >= orig.len() || idx >= cur.len() { continue; }
                let Ok(orig_v) = orig[idx].trim().parse::<f64>() else { continue };
                if orig_v <= 0.0 { continue; }
                let Ok(cur_v) = cur[idx].trim().parse::<f64>() else { return false };
                if (cur_v - orig_v * 5.0).abs() > 0.01 { return false; }
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
            profile.set_bool("RomanEndless", exe_roman == ExeRomanEndlessPatchState::Patched);
        }

        let mut orchestrator = EndlessAiOrchestrator::new(false);
        BciFeatureDetector::detect(game_path, &mut profile, &mut orchestrator);
        InstallFeatureDetector::detect(game_path, &mut profile);

        profile
    }
}
