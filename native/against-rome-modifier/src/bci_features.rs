//! Managed BCI features, food healing and migrations, ported from Core.Features.Bci.
use crate::{
    endless_ai::{self, EndlessAiOrchestrator, PatchState},
    PatchProfile,
};
use sha2::{Digest, Sha256};
use std::{fs, io, path::Path};

fn error(s: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, s.into())
}

pub struct FoodHealingFeature;

const FOOD_HEALING_SITES: &[(&str, i32)] = &[
    ("ak_anfuehrer", 87),
    ("ak_artillerie", 69),
    ("ak_geisterreiter", 68),
    ("ak_kampfverband", 89),
    ("ak_krieger", 73),
    ("ak_kundschafterwolf", 77),
    ("ak_landtier", 72),
    ("ak_packpferd", 57),
    ("ak_priester", 92),
    ("ak_verbandswolf", 68),
    ("ak_zivilist", 83),
    ("ak_zivilverband", 87),
];

const RETIRED_LEADER_SHA: &str = "42b07605d20c81a93be983252df1cb34104eb7ae9f8456ab31a842d4cd9232dd";
const VANILLA_LEADER_SHA: &str = "778a6e01b99664136ac0420b9f48212c41a5d6297a9952ea9d7d3a5d4851272c";

fn food_signature(symbol: i32, amount: i32) -> [Option<i32>; 11] {
    [
        Some(66),
        Some(amount),
        Some(81),
        Some(10),
        Some(81),
        Some(98),
        Some(128),
        Some(symbol),
        Some(73),
        Some(-3),
        Some(86),
    ]
}

impl FoodHealingFeature {
    pub fn try_detect(game_path: &Path) -> Option<bool> {
        match Self::probe(game_path) {
            Ok((enabled, true)) => Some(enabled),
            _ => None,
        }
    }

    /// Mirrors C# `TryDetect(out enabled)`: returns `(enabled, detected)` where `enabled`
    /// is always false unless every site is consistently ultimate. `Err` corresponds to a
    /// C# exception (unreadable/undecodable script), where the caller must not assign.
    fn probe(game_path: &Path) -> io::Result<(bool, bool)> {
        let mut state: Option<bool> = None;
        let root = game_path.join("SYSTEM/CLAK/SCRIPT");
        for &(file, symbol) in FOOD_HEALING_SITES {
            let path = root.join(format!("{file}.bci"));
            if !path.is_file() {
                return Ok((false, false));
            }
            let data = crate::decompress_pfil(&fs::read(&path)?)?;

            let orig = endless_ai::find_sites(&data, &food_signature(symbol, 1)).len();
            let ult = endless_ai::find_sites(&data, &food_signature(symbol, 10)).len();
            if orig + ult != 1 {
                return Ok((false, false));
            }
            let cur = ult == 1;
            if state.is_some_and(|prev| prev != cur) {
                return Ok((false, false));
            }
            state = Some(cur);
        }
        Ok((state == Some(true), state.is_some()))
    }

    pub fn apply(
        game_path: &Path,
        enabled: bool,
        backup_ak_anfuehrer: Option<&[u8]>,
        orchestrator: &mut EndlessAiOrchestrator,
    ) -> io::Result<bool> {
        let root = game_path.join("SYSTEM/CLAK/SCRIPT");
        let target = if enabled { 10 } else { 1 };
        let mut changed = false;

        for &(file, symbol) in FOOD_HEALING_SITES {
            let path = root.join(format!("{file}.bci"));
            if !path.is_file() {
                return Err(error(format!("missing script {}", path.display())));
            }

            if file.eq_ignore_ascii_case("ak_anfuehrer") {
                let mut normalized = orchestrator.get_or_load_file(&path)?.0.clone();
                let old = endless_ai::find_sites(&normalized, &food_signature(symbol, 1));
                let ult_sites = endless_ai::find_sites(&normalized, &food_signature(symbol, 10));
                if old.len() + ult_sites.len() == 1 {
                    if ult_sites.len() == 1 {
                        endless_ai::write_word(&mut normalized, ult_sites[0] + 4, 1)?;
                    }
                    if format!("{:x}", Sha256::digest(&normalized))
                        .eq_ignore_ascii_case(RETIRED_LEADER_SHA)
                    {
                        let bak = backup_ak_anfuehrer.ok_or_else(|| {
                            error("記憶體備份中找不到 SYSTEM/CLAK/SCRIPT/ak_anfuehrer.bci。")
                        })?;
                        orchestrator.replace_raw_bytes(&path, bak)?;
                        let data = &orchestrator.get_or_load_file(&path)?.0;
                        if !format!("{:x}", Sha256::digest(data))
                            .eq_ignore_ascii_case(VANILLA_LEADER_SHA)
                        {
                            return Err(error("乾淨的原版 ak_anfuehrer.bci 備份不存在或版本不符，已取消安全遷移。"));
                        }
                        changed = true;
                    }
                }
            }

            let script_file = orchestrator.get_or_load_file(&path)?;
            let data = &mut script_file.0;
            let originals = endless_ai::find_sites(data, &food_signature(symbol, 1));
            let ultimates = endless_ai::find_sites(data, &food_signature(symbol, 10));
            if originals.len() + ultimates.len() != 1 {
                return Err(error(format!(
                    "Food healing signature count mismatch: {}",
                    path.display()
                )));
            }
            let current = if originals.len() == 1 { 1 } else { 10 };
            let offset = if originals.len() == 1 {
                originals[0]
            } else {
                ultimates[0]
            } + 4;
            if current != target {
                endless_ai::write_word(data, offset, target)?;
                script_file.1 = true;
                changed = true;
            }
        }

        Ok(changed)
    }
}

pub struct CiviProduce20Feature;
impl CiviProduce20Feature {
    pub fn restore_ak_npc_original(
        game_path: &Path,
        orchestrator: &mut EndlessAiOrchestrator,
    ) -> io::Result<bool> {
        let path = game_path.join("SYSTEM/CLAK/SCRIPT/ak_npc.bci");
        if !path.is_file() {
            return Ok(false);
        }
        let script = orchestrator.get_or_load_file(&path)?;
        let sig = [
            Some(66),
            Some(20),
            Some(90),
            Some(3),
            Some(90),
            Some(2),
            Some(90),
            Some(8),
            Some(128),
            Some(46),
            Some(73),
            Some(-4),
            Some(86),
        ];
        let sites = endless_ai::find_sites(&script.0, &sig);
        if sites.len() == 1 {
            endless_ai::write_word(&mut script.0, sites[0] + 4, 1)?;
            script.1 = true;
            return Ok(true);
        }
        Ok(false)
    }
}

pub struct RetiredDefaultSpecialArrowsMigration;
impl RetiredDefaultSpecialArrowsMigration {
    fn sig(weapon: i32) -> [Option<i32>; 17] {
        [
            Some(66),
            Some(1000001),
            Some(87),
            Some(130),
            Some(66),
            Some(1),
            Some(66),
            Some(weapon),
            Some(81),
            Some(10),
            Some(81),
            Some(98),
            Some(128),
            Some(106),
            Some(73),
            Some(-4),
            Some(86),
        ]
    }

    pub fn restore_if_present(
        game_path: &Path,
        orchestrator: &mut EndlessAiOrchestrator,
    ) -> io::Result<bool> {
        let path = game_path.join("SYSTEM/CLAK/SCRIPT/ak_krieger.bci");
        if !path.is_file() {
            return Ok(false);
        }
        let script = orchestrator.get_or_load_file(&path)?;
        let originals = endless_ai::find_sites(&script.0, &Self::sig(0));
        let retired = endless_ai::find_sites(&script.0, &Self::sig(1));
        if originals.is_empty() && retired.len() == 1 {
            let offset = retired[0] + 28;
            endless_ai::write_word(&mut script.0, offset, 0)?;
            script.1 = true;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn try_detect_legacy(game_path: &Path) -> Option<bool> {
        let path = game_path.join("SYSTEM/CLAK/SCRIPT/ak_krieger.bci");
        if !path.is_file() {
            return None;
        }
        let raw = fs::read(&path).ok()?;
        let data = crate::decompress_pfil(&raw).ok()?;
        let orig = endless_ai::find_sites(&data, &Self::sig(0)).len();
        let ret = endless_ai::find_sites(&data, &Self::sig(1)).len();
        if orig + ret != 1 {
            return None;
        }
        Some(ret == 1)
    }
}

pub struct BciFeatureDetector;
impl BciFeatureDetector {
    pub fn detect(
        game_path: &Path,
        profile: &mut PatchProfile,
        orchestrator: &mut EndlessAiOrchestrator,
    ) {
        for module_id in &orchestrator.user_modules.clone() {
            if let Some(state) = orchestrator.detect_module_by_id(game_path, module_id) {
                let migrate_legacy = module_id == "Core" || module_id == "M6";
                let selected = state == PatchState::Ultimate
                    || (migrate_legacy && state == PatchState::Legacy);
                if module_id == "M6" {
                    profile.set_bool("RomanReinforcementGarrison", selected);
                    profile.set_bool("EndlessAi.M6", selected);
                } else if module_id == "VillageGarrisonQuota3x" {
                    let multiplier = if state == PatchState::Ultimate || state == PatchState::Legacy
                    {
                        orchestrator.detect_village_garrison_quota_multiplier(game_path)
                    } else {
                        1
                    };
                    profile.set_int("VillageGarrisonQuotaMultiplier", multiplier);
                } else {
                    profile.set_bool(format!("EndlessAi.{}", module_id), selected);
                }
            }
        }

        // C# assigns the out value even when TryDetect reports false; only an exception skips it.
        if let Ok((food_healing, _)) = FoodHealingFeature::probe(game_path) {
            profile.set_bool("FoodHealing10x", food_healing);
        }
    }
}
