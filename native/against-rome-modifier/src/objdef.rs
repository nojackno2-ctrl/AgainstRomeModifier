//! objdef.dau patcher ported faithfully from src.Core/Core/Patches/ObjdefPatcher.cs.

use crate::lzss::compress_pfil;
use crate::patch_text::{check_length, parse_csv_line, to_csv_string, TextDocument};
use crate::troops::{ObjdefIndex, TroopConfig};
use std::collections::HashMap;
use std::io::{self, Error, ErrorKind};

pub const GLORY_LEADERS: &[&str] = &[
    "FigRomAnf00_Anfuehrer",
    "FigGerAnf00_Anfuehrer",
    "FigKelAnf00_Anfuehrer",
    "FigHunAnf00_Anfuehrer",
];

pub const GLORY_COLUMNS: &[usize] = &[148, 149, 150, 161];
pub const GLORY_MULTIPLIER: i64 = 5;
pub const ARC_EMIT_MULTIPLIER: f64 = 2.0;
pub const ACCURACY_DRAD_MULTIPLIER: i64 = 2;
pub const ENTIRE_MAP_SIGHT: i32 = 30000;

#[derive(Debug, Clone, Default)]
pub struct ObjdefOptions {
    pub balance: bool,
    pub housing_capacity_20x: bool,
    pub storage_capacity_10x: bool,
    pub fast_build_upgrade_repair: bool,
    pub hq_hp_10x: bool,
    pub leader_glory_5x: bool,
    pub ranged_range_3x: bool,
    pub unit_movement_speed_2x: bool,
    pub spell_entire_map: bool,
    pub spell_range_3x: bool,
    pub projectile_arc_height: bool,
    pub unit_stats: HashMap<String, [f64; 9]>,
    pub all_units_entire_map_vision: bool,
    pub villager_movement_speed_5x: bool,
    pub no_run_hp_loss: bool,
}

pub struct ObjdefPatcher;

impl ObjdefPatcher {
    pub fn supports_entire_map_vision(name: &str) -> bool {
        TroopConfig::get_metadata(name).is_some()
            || matches!(
                name,
                "FigZivMan00_Zivilist" | "FigZivWei00_Zivilistin" | "FigTiePac00_Packpferd"
            )
    }

    pub fn get_patched_bytes(original: &[u8], options: &ObjdefOptions) -> io::Result<Vec<u8>> {
        let doc = TextDocument::read(original)?;
        let original_lines = doc.lines.clone();
        let mut lines = doc.lines.clone();

        for row in 2..lines.len() {
            if lines[row].len() < 100 {
                continue;
            }
            let mut cols = parse_csv_line(&lines[row]);
            if cols.len() < 192 {
                continue;
            }
            let source = parse_csv_line(&original_lines[row]);
            let name = cols[52].trim().to_string();

            if options.leader_glory_5x && GLORY_LEADERS.contains(&name.as_str()) {
                for &index in GLORY_COLUMNS {
                    let value = Self::read_col(&source, index);
                    if value > 0.0 {
                        Self::set_value_if_fits(&mut cols, index, &((value as i64 * GLORY_MULTIPLIER).to_string()));
                    }
                }
            }

            if options.housing_capacity_20x {
                Self::multiply_original_int(&mut cols, &source, ObjdefIndex::HousingCapacity as usize, 20, &name, "housing capacity")?;
            }

            if options.storage_capacity_10x
                && name.starts_with("Bau")
                && (name.contains("Hau") || name.contains("Lag"))
            {
                Self::multiply_original_int(&mut cols, &source, ObjdefIndex::StorageCapacity as usize, 10, &name, "storage capacity")?;
            }

            if options.hq_hp_10x && name.starts_with("Bau") && name.contains("Hau") {
                Self::multiply_original_int(&mut cols, &source, ObjdefIndex::Hp as usize, 10, &name, "hq hp")?;
            }

            if options.fast_build_upgrade_repair && name.starts_with("Bau") {
                Self::divide_original_int(&mut cols, &source, 73, 10);
                Self::divide_original_int(&mut cols, &source, 74, 10);
            }

            if options.projectile_arc_height || options.ranged_range_3x {
                Self::patch_projectile_weapons(&mut cols, &source, &name, options)?;
            }

            if let Some(meta) = TroopConfig::get_metadata(&name) {
                if let Some(stats) = options.unit_stats.get(&name) {
                    Self::patch_unit(&mut cols, &source, &name, meta.unit_type, stats, options)?;
                }
            } else if matches!(
                name.as_str(),
                "FigZivMan00_Zivilist" | "FigZivWei00_Zivilistin" | "FigTiePac00_Packpferd"
            ) {
                let mut civ_mult = 1.0;
                if options.villager_movement_speed_5x {
                    civ_mult *= 5.0;
                }
                Self::patch_civilian_speed(&mut cols, &source, &name, civ_mult)?;
            }

            if options.no_run_hp_loss
                && (TroopConfig::get_metadata(&name).is_some()
                    || matches!(
                        name.as_str(),
                        "FigZivMan00_Zivilist" | "FigZivWei00_Zivilistin" | "FigTiePac00_Packpferd"
                    ))
            {
                let original_lpsub = Self::read_col(&source, ObjdefIndex::Lpsub as usize);
                if (original_lpsub - 1.00).abs() < 0.01 {
                    Self::set_value(&mut cols, ObjdefIndex::Lpsub as usize, "0.00", &name, "lpsub")?;
                }
            }

            // Final override: AllUnitsEntireMapVision owns Sirad last
            if options.all_units_entire_map_vision && Self::supports_entire_map_vision(&name) {
                Self::set_value(&mut cols, ObjdefIndex::Sirad as usize, &ENTIRE_MAP_SIGHT.to_string(), &name, "視野")?;
            }

            lines[row] = to_csv_string(cols.iter().map(|s| s.as_str()));
        }

        let mut new_text = lines.join(doc.line_ending);
        if doc.text.ends_with(doc.line_ending) && !new_text.ends_with(doc.line_ending) {
            new_text.push_str(doc.line_ending);
        }

        if new_text.len() != doc.text.len() {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "objdef.dau 長度不匹配！原始長度: {}, 修改後長度: {}",
                    doc.text.len(),
                    new_text.len()
                ),
            ));
        }

        let encoded = crate::patch_text::encode_cp1251(&new_text);
        if doc.was_pfil {
            compress_pfil(&encoded, doc.raw_header_template.as_deref())
        } else {
            Ok(encoded)
        }
    }

    fn patch_unit(
        cols: &mut [String],
        source: &[String],
        name: &str,
        unit_type: &str,
        stats: &[f64; 9],
        options: &ObjdefOptions,
    ) -> io::Result<()> {
        let moves = Self::read_col(source, ObjdefIndex::Moves as usize);
        let movsf = Self::read_col(source, ObjdefIndex::Movsf as usize);
        let bmovs = Self::read_col(source, ObjdefIndex::Bmovs as usize);

        let (melee_damage, ranged_damage) = Self::get_damage(source, unit_type);
        let primary_damage = if matches!(unit_type, "ranged_inf" | "ranged_cav") {
            ranged_damage
        } else if unit_type == "siege" {
            melee_damage.max(ranged_damage)
        } else {
            melee_damage
        };

        let original_range = Self::get_max_range(source, unit_type);
        let (melee_reload, ranged_reload) = Self::get_reload(source, unit_type);
        let primary_reload = if matches!(unit_type, "ranged_inf" | "ranged_cav") {
            ranged_reload
        } else if unit_type == "siege" {
            melee_reload.max(ranged_reload)
        } else {
            melee_reload
        };

        let speed_scale: f64 = if options.unit_movement_speed_2x { 2.0 } else { 1.0 };
        let mut range_scale = if original_range > 0.0 { stats[7] / original_range } else { 1.0 };
        let supports_ranged_range_3x = TroopConfig::supports_ranged_range_3x(unit_type);
        if supports_ranged_range_3x && options.ranged_range_3x {
            range_scale *= 3.0;
        }

        let is_priest = unit_type == "priest";
        let reload_scale = if primary_reload > 0.0 { stats[6] / primary_reload } else { 1.0 };
        let damage_scale = if primary_damage > 0.0 { stats[1] / primary_damage } else { 1.0 };

        if moves > 0.0 && (speed_scale - 1.0).abs() > 0.001 {
            Self::set_value(cols, ObjdefIndex::Moves as usize, &format!("{:.2}", moves * speed_scale), name, "移動速度")?;
            if movsf > 0.0 {
                Self::set_value(cols, ObjdefIndex::Movsf as usize, &format!("{:.2}", movsf * speed_scale), name, "移動速度")?;
            }
            if bmovs > 0.0 {
                Self::set_value(cols, ObjdefIndex::Bmovs as usize, &format!("{:.2}", bmovs * speed_scale), name, "移動速度")?;
            }
        }

        let mut sight = if options.spell_entire_map && is_priest {
            ENTIRE_MAP_SIGHT as f64
        } else {
            stats[5]
        };
        if supports_ranged_range_3x && options.ranged_range_3x {
            sight = sight.max(original_range * range_scale);
        }

        if (sight - Self::read_col(source, ObjdefIndex::Sirad as usize)).abs() > 0.01 {
            Self::set_value(cols, ObjdefIndex::Sirad as usize, &(sight as i64).to_string(), name, "視野")?;
        }
        if (stats[0] - Self::read_col(source, ObjdefIndex::Hp as usize)).abs() > 0.01 {
            Self::set_value(cols, ObjdefIndex::Hp as usize, &(stats[0] as i64).to_string(), name, "生命值")?;
        }
        if (stats[3] - Self::read_col(source, ObjdefIndex::Aw as usize)).abs() > 0.01 {
            Self::set_value(cols, ObjdefIndex::Aw as usize, &(stats[3] as i64).to_string(), name, "戰鬥")?;
        }
        if (stats[2] - Self::read_col(source, ObjdefIndex::Vw as usize)).abs() > 0.01 {
            Self::set_value(cols, ObjdefIndex::Vw as usize, &(stats[2] as i64).to_string(), name, "防禦")?;
        }

        for w in 1..=8 {
            let active = ObjdefIndex::Weapon1Akti as usize + (w - 1) * 8;
            let damage = active + 1;
            let max_col = active + 3;
            let reload = active + 6;

            if max_col >= source.len() || source[active].trim() != "1" {
                continue;
            }

            if is_priest {
                let priest_weapon_dam = Self::read_col(source, damage);
                let new_priest_dmg = priest_weapon_dam * damage_scale;
                if (new_priest_dmg - priest_weapon_dam).abs() > 0.01 {
                    Self::set_value(cols, damage, &format!("{:.2}", new_priest_dmg), name, "damage")?;
                }
                let orig_priest_reload = Self::read_col(source, reload);
                let new_priest_reload = (orig_priest_reload * reload_scale).round();
                if (new_priest_reload - orig_priest_reload).abs() > 0.01 {
                    Self::set_value(cols, reload, &(new_priest_reload as i64).to_string(), name, "reload")?;
                }
                continue;
            }

            let damage_type_idx = ObjdefIndex::Weapon1Dtyp as usize + (w - 1);
            let is_ranged_weapon = unit_type == "siege"
                || (damage_type_idx < source.len()
                    && matches!(source[damage_type_idx].trim(), "1" | "2" | "3" | "4"));

            if is_ranged_weapon && (range_scale - 1.0).abs() > 0.001 {
                let r = Self::read_col(source, max_col);
                if r > 0.0 {
                    Self::set_value(cols, max_col, &format!("{:.2}", r * range_scale), name, "射程")?;
                }
            }

            let weapon_dam = Self::read_col(source, damage);
            let new_dmg = if matches!(unit_type, "ranged_inf" | "ranged_cav") && w == 1 {
                weapon_dam
            } else {
                weapon_dam * damage_scale
            };
            if (new_dmg - weapon_dam).abs() > 0.01 {
                Self::set_value(cols, damage, &format!("{:.2}", new_dmg), name, "傷害")?;
            }

            let orig_reload = Self::read_col(source, reload);
            let new_reload = (orig_reload * reload_scale).round();
            if (new_reload - orig_reload).abs() > 0.01 {
                Self::set_value(cols, reload, &(new_reload as i64).to_string(), name, "攻擊冷卻")?;
            }
        }

        Ok(())
    }

    fn patch_projectile_weapons(
        cols: &mut [String],
        source: &[String],
        name: &str,
        options: &ObjdefOptions,
    ) -> io::Result<()> {
        for w in 1..=8 {
            let active = ObjdefIndex::Weapon1Akti as usize + (w - 1) * 8;
            let emit = ObjdefIndex::Weapon1Emit as usize + (w - 1) * 8;
            let drad = ObjdefIndex::Weapon1Drad as usize + (w - 1) * 2;

            if emit >= source.len() || source[active].trim() != "1" {
                continue;
            }

            let emit_val: i64 = source[emit].trim().parse().unwrap_or(0);
            if emit_val <= 0 {
                continue;
            }

            if options.projectile_arc_height {
                let new_emit = (emit_val as f64 * ARC_EMIT_MULTIPLIER).round() as i64;
                Self::set_value(cols, emit, &new_emit.to_string(), name, "拋射垂直初速")?;
            }

            if options.ranged_range_3x && drad < source.len() {
                if let Ok(drad_val) = source[drad].trim().parse::<i64>() {
                    if drad_val > 0 {
                        Self::set_value(cols, drad, &(drad_val * ACCURACY_DRAD_MULTIPLIER).to_string(), name, "落點傷害半徑")?;
                    }
                }
            }
        }
        Ok(())
    }

    fn patch_civilian_speed(cols: &mut [String], source: &[String], name: &str, multiplier: f64) -> io::Result<()> {
        for index in [
            ObjdefIndex::Moves as usize,
            ObjdefIndex::Movsf as usize,
            ObjdefIndex::Bmovs as usize,
        ] {
            let val = Self::read_col(source, index);
            if val > 0.0 {
                Self::set_value(cols, index, &format!("{:.2}", val * multiplier), name, "移動速度")?;
            }
        }
        Ok(())
    }

    fn multiply_original_int(
        cols: &mut [String],
        source: &[String],
        index: usize,
        multiplier: i64,
        name: &str,
        label: &str,
    ) -> io::Result<()> {
        if index < cols.len() && index < source.len() {
            if let Ok(value) = source[index].trim().parse::<i64>() {
                if value > 0 {
                    Self::set_value(cols, index, &(value * multiplier).to_string(), name, label)?;
                }
            }
        }
        Ok(())
    }

    fn divide_original_int(cols: &mut [String], source: &[String], index: usize, divisor: i64) {
        if index < cols.len() && index < source.len() {
            if let Ok(value) = source[index].trim().parse::<i64>() {
                if value > 0 {
                    let new_val = 1.max(value / divisor);
                    Self::set_value_if_fits(cols, index, &new_val.to_string());
                }
            }
        }
    }

    fn set_value_if_fits(cols: &mut [String], index: usize, value: &str) {
        let length = cols[index].len();
        if let Some(final_val) = check_length(value, length) {
            cols[index] = format!("{:>width$}", final_val, width = length);
        }
    }

    fn set_value(
        cols: &mut [String],
        index: usize,
        value: &str,
        name: &str,
        label: &str,
    ) -> io::Result<()> {
        let length = cols[index].len();
        if let Some(final_val) = check_length(value, length) {
            cols[index] = format!("{:>width$}", final_val, width = length);
            Ok(())
        } else {
            Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "單位 {} 的 {} 數值 {} 超出 objdef.dau 欄位長度 {}；已取消整次套用。",
                    name, label, value, length
                ),
            ))
        }
    }

    fn read_col(cols: &[String], index: usize) -> f64 {
        if index < cols.len() {
            cols[index].trim().parse::<f64>().unwrap_or(0.0)
        } else {
            0.0
        }
    }

    fn get_damage(cols: &[String], unit_type: &str) -> (f64, f64) {
        let mut melee: f64 = 0.0;
        let mut ranged: f64 = 0.0;
        for w in 1..=8 {
            let active = ObjdefIndex::Weapon1Akti as usize + (w - 1) * 8;
            let damage = active + 1;
            let dtype = ObjdefIndex::Weapon1Dtyp as usize + (w - 1);
            if dtype >= cols.len() || cols[active].trim() != "1" {
                continue;
            }
            let val = Self::read_col(cols, damage);
            if matches!(cols[dtype].trim(), "1" | "2" | "3" | "4") || unit_type == "siege" {
                ranged = ranged.max(val);
            } else {
                melee = melee.max(val);
            }
        }
        (melee, ranged)
    }

    fn get_reload(cols: &[String], unit_type: &str) -> (f64, f64) {
        let mut melee: f64 = 0.0;
        let mut ranged: f64 = 0.0;
        for w in 1..=8 {
            let active = ObjdefIndex::Weapon1Akti as usize + (w - 1) * 8;
            let reload = active + 6;
            let dtype = ObjdefIndex::Weapon1Dtyp as usize + (w - 1);
            if dtype >= cols.len() || cols[active].trim() != "1" {
                continue;
            }
            let val = Self::read_col(cols, reload);
            let is_ranged = matches!(cols[dtype].trim(), "1" | "2" | "3" | "4") || unit_type == "siege";
            if is_ranged {
                if val > 0.0 && (ranged == 0.0 || val < ranged) {
                    ranged = val;
                }
            } else if val > 0.0 && (melee == 0.0 || val < melee) {
                melee = val;
            }
        }
        (melee, ranged)
    }

    fn get_max_range(cols: &[String], unit_type: &str) -> f64 {
        if unit_type == "priest" {
            return Self::read_col(cols, ObjdefIndex::Sirad as usize);
        }
        let mut max_r: f64 = 0.0;
        for w in 1..=8 {
            let active = ObjdefIndex::Weapon1Akti as usize + (w - 1) * 8;
            let min_r = active + 2;
            let max_col = active + 3;
            if max_col >= cols.len() || cols[active].trim() != "1" {
                continue;
            }
            max_r = max_r.max(Self::read_col(cols, min_r)).max(Self::read_col(cols, max_col));
        }
        max_r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objdef_housing_capacity_and_glory() {
        let mut row = vec!["     0".to_string(); 200];
        row[52] = format!("{:>24}", "FigRomAnf00_Anfuehrer");
        row[148] = "     2".to_string();
        row[149] = "     3".to_string();
        row[150] = "     4".to_string();
        row[161] = "     5".to_string();
        row[156] = "    10".to_string();

        let raw = format!("header1\r\nheader2\r\n{}\r\n", row.join(","));
        let options = ObjdefOptions {
            housing_capacity_20x: true,
            leader_glory_5x: true,
            ..Default::default()
        };

        let patched_bytes = ObjdefPatcher::get_patched_bytes(raw.as_bytes(), &options).unwrap();
        let s = String::from_utf8(patched_bytes).unwrap();
        let cols = parse_csv_line(s.lines().nth(2).unwrap());

        assert_eq!(cols[156].trim(), "200");
        assert_eq!(cols[148].trim(), "10");
        assert_eq!(cols[149].trim(), "15");
        assert_eq!(cols[150].trim(), "20");
        assert_eq!(cols[161].trim(), "25");
    }
}
