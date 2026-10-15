//! Biome types, ecological layers, and species profiles.

use crate::random::DotNetRandom;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 《反抗羅馬》(Against Rome) 歷史戰區與地理生態圈類型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i32)]
pub enum BiomeType {
    /// 日耳曼原始森林
    GermanicForest = 0,
    /// 多瑙河溫帶闊葉林
    DanubeBroadleaf = 1,
    /// 義大利地中海灌木丘陵
    ItalianHills = 2,
    /// 不列顛沼澤荒原
    BritishMarsh = 3,
}

/// 植被生態垂直分層與生境類別。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i32)]
pub enum VegetationLayer {
    /// 喬木層（林冠）
    Canopy = 0,
    /// 灌木層（林下/林緣）
    Understory = 1,
    /// 地表草花層（林窗/原野）
    GroundFlora = 2,
    /// 水岸生境層（濱水帶）
    Riparian = 3,
    /// 地質岩石層（坡地/懸崖）
    Rock = 4,
}

impl VegetationLayer {
    pub const ALL: [VegetationLayer; 5] = [
        VegetationLayer::Canopy,
        VegetationLayer::Understory,
        VegetationLayer::GroundFlora,
        VegetationLayer::Riparian,
        VegetationLayer::Rock,
    ];
}

/// 生態圈單一物種定義項。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BiomeSpeciesEntry {
    pub name_prefix: String,
    pub layer: VegetationLayer,
    pub weight: f64,
    pub footprint_radius_tiles: f32,
    pub min_elevation: u8,
    pub max_elevation: u8,
    pub max_slope: f32,
    pub description_zh: Option<String>,
}

impl BiomeSpeciesEntry {
    pub fn new(
        name_prefix: impl Into<String>,
        layer: VegetationLayer,
        weight: f64,
        footprint_radius_tiles: f32,
        max_slope: f32,
        description_zh: Option<&str>,
    ) -> Self {
        Self {
            name_prefix: name_prefix.into(),
            layer,
            weight,
            footprint_radius_tiles,
            min_elevation: 0,
            max_elevation: 255,
            max_slope,
            description_zh: description_zh.map(|s| s.to_string()),
        }
    }
}

/// 已成功映射至遊戲實體範本的解析物種。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedSpecies {
    pub type_id: i32,
    pub name: String,
    pub layer: VegetationLayer,
    pub weight: f64,
    pub footprint_radius_tiles: f32,
    pub min_elevation: u8,
    pub max_elevation: u8,
    pub max_slope: f32,
}

/// 生態圈已解析之物種名冊。提供按生態分層進行高效加權隨機抽選。
#[derive(Debug, Clone, Default)]
pub struct ResolvedEcologyRoster {
    by_layer: HashMap<VegetationLayer, Vec<ResolvedSpecies>>,
}

impl ResolvedEcologyRoster {
    pub fn new(species: impl IntoIterator<Item = ResolvedSpecies>) -> Self {
        let mut by_layer = HashMap::new();
        for &layer in &VegetationLayer::ALL {
            by_layer.insert(layer, Vec::new());
        }
        for item in species {
            by_layer.entry(item.layer).or_default().push(item);
        }
        Self { by_layer }
    }

    pub fn get_layer_species(&self, layer: VegetationLayer) -> &[ResolvedSpecies] {
        self.by_layer
            .get(&layer)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn has_layer(&self, layer: VegetationLayer) -> bool {
        self.by_layer
            .get(&layer)
            .map(|list| !list.is_empty())
            .unwrap_or(false)
    }

    pub fn total_species_count(&self) -> usize {
        self.by_layer.values().map(|list| list.len()).sum()
    }

    pub fn try_pick(&self, layer: VegetationLayer, random: &mut DotNetRandom) -> Option<ResolvedSpecies> {
        let list = self.by_layer.get(&layer)?;
        if list.is_empty() {
            return None;
        }
        if list.len() == 1 {
            return Some(list[0].clone());
        }

        let total_weight: f64 = list.iter().map(|s| s.weight).sum();
        if total_weight <= 0.0 {
            let idx = random.next_i32(list.len() as i32) as usize;
            return Some(list[idx].clone());
        }

        let roll = random.next_double() * total_weight;
        let mut current = 0.0;
        for item in list {
            current += item.weight;
            if roll <= current {
                return Some(item.clone());
            }
        }
        list.last().cloned()
    }
}

/// 生態圈群落設定檔。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BiomeEcologyProfile {
    pub biome: BiomeType,
    pub region_code: String,
    pub display_name_zh: String,
    pub display_name_en: String,
    pub canopy_threshold: f32,
    pub understory_threshold: f32,
    pub ground_flora_threshold: f32,
    pub riparian_elevation_delta: f32,
    pub cliff_slope_threshold: f32,
    pub species: Vec<BiomeSpeciesEntry>,
}

impl BiomeEcologyProfile {
    pub fn germanic_forest() -> Self {
        Self {
            biome: BiomeType::GermanicForest,
            region_code: "Ger".to_string(),
            display_name_zh: "日耳曼針葉黑森林".to_string(),
            display_name_en: "Germanic Black Forest".to_string(),
            canopy_threshold: 0.55,
            understory_threshold: 0.38,
            ground_flora_threshold: 0.20,
            riparian_elevation_delta: 3.0,
            cliff_slope_threshold: 26.0,
            species: vec![
                BiomeSpeciesEntry::new("LanGerNad", VegetationLayer::Canopy, 60.0, 1.15, 32.0, Some("日耳曼針葉高樹 (Tanne/Fichte)")),
                BiomeSpeciesEntry::new("LanGerBir", VegetationLayer::Canopy, 18.0, 0.90, 35.0, Some("高地白樺 (Birke)")),
                BiomeSpeciesEntry::new("LanGerLau", VegetationLayer::Canopy, 12.0, 1.05, 30.0, Some("混交闊葉樹 (Laubbaum)")),
                BiomeSpeciesEntry::new("LanGerEic", VegetationLayer::Canopy, 10.0, 1.25, 28.0, Some("古老巨橡 (Eiche)")),
                BiomeSpeciesEntry::new("LanGerBus", VegetationLayer::Understory, 45.0, 0.65, 40.0, Some("日耳曼林下灌木 (Busch)")),
                BiomeSpeciesEntry::new("LanGerDor", VegetationLayer::Understory, 30.0, 0.55, 42.0, Some("黑森林荊棘 (Dornen)")),
                BiomeSpeciesEntry::new("LanGerBod", VegetationLayer::Understory, 25.0, 0.50, 45.0, Some("地被矮灌木 (Bodenbusch)")),
                BiomeSpeciesEntry::new("LanGerGra", VegetationLayer::GroundFlora, 55.0, 0.35, 50.0, Some("日耳曼林間野草 (Gras)")),
                BiomeSpeciesEntry::new("LanGerBlu", VegetationLayer::GroundFlora, 30.0, 0.35, 45.0, Some("耐陰野花 (Blumen)")),
                BiomeSpeciesEntry::new("LanGerBli", VegetationLayer::GroundFlora, 15.0, 0.35, 45.0, Some("盛開林花 (Blüten)")),
                BiomeSpeciesEntry::new("LanGerSch", VegetationLayer::Riparian, 50.0, 0.45, 20.0, Some("水岸蘆葦 (Schilf)")),
                BiomeSpeciesEntry::new("LanGerWei", VegetationLayer::Riparian, 30.0, 0.85, 22.0, Some("濱水垂柳 (Weide)")),
                BiomeSpeciesEntry::new("LanGerEnt", VegetationLayer::Riparian, 20.0, 0.40, 15.0, Some("澤地水草 (Entengrütze)")),
                BiomeSpeciesEntry::new("LanGerSte", VegetationLayer::Rock, 60.0, 0.80, 90.0, Some("黑森林岩石塊 (Stein)")),
                BiomeSpeciesEntry::new("LanGerFel", VegetationLayer::Rock, 40.0, 1.20, 90.0, Some("斷崖花崗巨石 (Fels)")),
            ],
        }
    }

    pub fn danube_broadleaf() -> Self {
        Self {
            biome: BiomeType::DanubeBroadleaf,
            region_code: "Ger".to_string(),
            display_name_zh: "多瑙河溫帶闊葉林".to_string(),
            display_name_en: "Danubian Broadleaf Forest".to_string(),
            canopy_threshold: 0.52,
            understory_threshold: 0.36,
            ground_flora_threshold: 0.18,
            riparian_elevation_delta: 3.5,
            cliff_slope_threshold: 28.0,
            species: vec![
                BiomeSpeciesEntry::new("LanGerEic", VegetationLayer::Canopy, 45.0, 1.25, 30.0, Some("溫帶巨橡 (Eiche)")),
                BiomeSpeciesEntry::new("LanGerBuc", VegetationLayer::Canopy, 25.0, 1.15, 32.0, Some("多瑙河山毛櫸 (Buche)")),
                BiomeSpeciesEntry::new("LanGerLau", VegetationLayer::Canopy, 20.0, 1.05, 30.0, Some("沖積平原闊葉樹 (Laubbaum)")),
                BiomeSpeciesEntry::new("LanGerBir", VegetationLayer::Canopy, 10.0, 0.90, 35.0, Some("丘陵樺木 (Birke)")),
                BiomeSpeciesEntry::new("LanGerBus", VegetationLayer::Understory, 50.0, 0.65, 40.0, Some("闊葉林灌木 (Busch)")),
                BiomeSpeciesEntry::new("LanGerHec", VegetationLayer::Understory, 30.0, 0.60, 38.0, Some("自然樹籬灌木 (Hecke)")),
                BiomeSpeciesEntry::new("LanGerStr", VegetationLayer::Understory, 20.0, 0.55, 42.0, Some("原野矮灌木 (Strauch)")),
                BiomeSpeciesEntry::new("LanGerGra", VegetationLayer::GroundFlora, 40.0, 0.35, 50.0, Some("豐茂草地 (Gras)")),
                BiomeSpeciesEntry::new("LanGerBlu", VegetationLayer::GroundFlora, 35.0, 0.35, 45.0, Some("谷地野花 (Blumen)")),
                BiomeSpeciesEntry::new("LanGerBli", VegetationLayer::GroundFlora, 25.0, 0.35, 45.0, Some("河谷繽紛花叢 (Blüten)")),
                BiomeSpeciesEntry::new("LanGerWei", VegetationLayer::Riparian, 40.0, 0.85, 22.0, Some("河岸垂柳 (Weide)")),
                BiomeSpeciesEntry::new("LanGerSch", VegetationLayer::Riparian, 35.0, 0.45, 20.0, Some("沿岸蘆葦 (Schilf)")),
                BiomeSpeciesEntry::new("LanGerEnt", VegetationLayer::Riparian, 25.0, 0.40, 15.0, Some("淺灘浮萍 (Entengrütze)")),
                BiomeSpeciesEntry::new("LanGerSte", VegetationLayer::Rock, 65.0, 0.75, 90.0, Some("河床卵石岩塊 (Stein)")),
                BiomeSpeciesEntry::new("LanGerFel", VegetationLayer::Rock, 35.0, 1.10, 90.0, Some("丘陵風化岩壁 (Fels)")),
            ],
        }
    }

    pub fn italian_hills() -> Self {
        Self {
            biome: BiomeType::ItalianHills,
            region_code: "Ita".to_string(),
            display_name_zh: "義大利地中海灌木丘陵".to_string(),
            display_name_en: "Italian Mediterranean Hills".to_string(),
            canopy_threshold: 0.62,
            understory_threshold: 0.42,
            ground_flora_threshold: 0.24,
            riparian_elevation_delta: 2.5,
            cliff_slope_threshold: 22.0,
            species: vec![
                BiomeSpeciesEntry::new("LanItaZyp", VegetationLayer::Canopy, 50.0, 0.70, 35.0, Some("義大利絲柏 (Zypresse)")),
                BiomeSpeciesEntry::new("LanItaPin", VegetationLayer::Canopy, 35.0, 1.15, 28.0, Some("地中海石松 (Pinie)")),
                BiomeSpeciesEntry::new("LanItaObs", VegetationLayer::Canopy, 15.0, 0.85, 25.0, Some("丘陵果木 (Obstbaum)")),
                BiomeSpeciesEntry::new("LanItaBus", VegetationLayer::Understory, 55.0, 0.60, 45.0, Some("地中海硬葉灌木 (Busch)")),
                BiomeSpeciesEntry::new("LanItaDor", VegetationLayer::Understory, 25.0, 0.50, 45.0, Some("乾燥棘刺叢 (Dornen)")),
                BiomeSpeciesEntry::new("LanItaBod", VegetationLayer::Understory, 20.0, 0.45, 45.0, Some("旱生矮灌叢 (Bodenbusch)")),
                BiomeSpeciesEntry::new("LanItaGra", VegetationLayer::GroundFlora, 65.0, 0.35, 50.0, Some("旱地草叢 (Gras)")),
                BiomeSpeciesEntry::new("LanItaBlu", VegetationLayer::GroundFlora, 35.0, 0.35, 45.0, Some("陽生野花 (Blumen)")),
                BiomeSpeciesEntry::new("LanItaSch", VegetationLayer::Riparian, 60.0, 0.45, 20.0, Some("溪谷水草 (Schilf)")),
                BiomeSpeciesEntry::new("LanItaSer", VegetationLayer::Riparian, 40.0, 0.45, 15.0, Some("泉池睡蓮 (Seerose)")),
                BiomeSpeciesEntry::new("LanItaSte", VegetationLayer::Rock, 55.0, 0.80, 90.0, Some("石灰岩塊 (Stein)")),
                BiomeSpeciesEntry::new("LanItaFel", VegetationLayer::Rock, 45.0, 1.15, 90.0, Some("裸露石灰岩壁 (Fels)")),
            ],
        }
    }

    pub fn british_marsh() -> Self {
        Self {
            biome: BiomeType::BritishMarsh,
            region_code: "Kel".to_string(),
            display_name_zh: "不列顛沼澤荒原".to_string(),
            display_name_en: "British Bog & Heathland".to_string(),
            canopy_threshold: 0.70,
            understory_threshold: 0.45,
            ground_flora_threshold: 0.20,
            riparian_elevation_delta: 4.0,
            cliff_slope_threshold: 24.0,
            species: vec![
                BiomeSpeciesEntry::new("LanKelBir", VegetationLayer::Canopy, 40.0, 0.85, 30.0, Some("荒原矮樺 (Birke)")),
                BiomeSpeciesEntry::new("LanKelEic", VegetationLayer::Canopy, 30.0, 1.05, 28.0, Some("風蝕孤櫟 (Eiche)")),
                BiomeSpeciesEntry::new("LanKelSto", VegetationLayer::Canopy, 30.0, 0.60, 35.0, Some("泥炭枯樹樁 (Stock)")),
                BiomeSpeciesEntry::new("LanKelBus", VegetationLayer::Understory, 55.0, 0.60, 40.0, Some("石楠灌叢 (Busch)")),
                BiomeSpeciesEntry::new("LanKelDor", VegetationLayer::Understory, 45.0, 0.55, 42.0, Some("荒原荊棘叢 (Dornen)")),
                BiomeSpeciesEntry::new("LanKelGra", VegetationLayer::GroundFlora, 65.0, 0.35, 45.0, Some("沼澤薹草 (Gras)")),
                BiomeSpeciesEntry::new("LanKelBlu", VegetationLayer::GroundFlora, 35.0, 0.35, 45.0, Some("荒原苔蘚花 (Blumen)")),
                BiomeSpeciesEntry::new("LanKelSch", VegetationLayer::Riparian, 50.0, 0.45, 18.0, Some("泥沼蘆葦 (Schilf)")),
                BiomeSpeciesEntry::new("LanKelEnt", VegetationLayer::Riparian, 30.0, 0.40, 15.0, Some("泥塘浮萍 (Entengrütze)")),
                BiomeSpeciesEntry::new("LanKelWei", VegetationLayer::Riparian, 20.0, 0.80, 20.0, Some("澤地矮柳 (Weide)")),
                BiomeSpeciesEntry::new("LanKelSte", VegetationLayer::Rock, 50.0, 0.90, 90.0, Some("荒原玄武岩塊 (Stein)")),
                BiomeSpeciesEntry::new("LanKelFel", VegetationLayer::Rock, 50.0, 1.30, 90.0, Some("巨石陣荒原石柱 (Fels)")),
            ],
        }
    }

    pub fn get_preset(biome_type: BiomeType) -> Self {
        match biome_type {
            BiomeType::GermanicForest => Self::germanic_forest(),
            BiomeType::DanubeBroadleaf => Self::danube_broadleaf(),
            BiomeType::ItalianHills => Self::italian_hills(),
            BiomeType::BritishMarsh => Self::british_marsh(),
        }
    }

    /// 將生態圈物種前綴映射至地圖中實際載入的範本 ID 與名稱。
    pub fn resolve_templates(
        &self,
        type_ids: &[i32],
        objdef_names: &HashMap<i32, String>,
    ) -> ResolvedEcologyRoster {
        struct AvailableItem<'a> {
            id: i32,
            name: &'a str,
            region: &'a str,
            category: &'static str,
        }

        let mut available = Vec::new();
        for &id in type_ids {
            if let Some(name) = objdef_names.get(&id) {
                if Self::is_landscape(name) {
                    let region = if name.len() >= 6 { &name[3..6] } else { "" };
                    let category = Self::classify_category(name);
                    available.push(AvailableItem {
                        id,
                        name: name.as_str(),
                        region,
                        category,
                    });
                }
            }
        }

        if available.is_empty() {
            return ResolvedEcologyRoster::default();
        }

        let mut resolved_list = Vec::new();

        for entry in &self.species {
            let category_code = Self::layer_to_category_string(entry.layer);

            // 1. 精確前綴匹配
            let exact_matches: Vec<_> = available
                .iter()
                .filter(|item| {
                    item.name.len() >= entry.name_prefix.len()
                        && item.name[..entry.name_prefix.len()]
                            .eq_ignore_ascii_case(&entry.name_prefix)
                })
                .collect();

            if !exact_matches.is_empty() {
                let weight_per_match = entry.weight / exact_matches.len() as f64;
                for m in exact_matches {
                    resolved_list.push(ResolvedSpecies {
                        type_id: m.id,
                        name: m.name.to_string(),
                        layer: entry.layer,
                        weight: weight_per_match,
                        footprint_radius_tiles: entry.footprint_radius_tiles,
                        min_elevation: entry.min_elevation,
                        max_elevation: entry.max_elevation,
                        max_slope: entry.max_slope,
                    });
                }
                continue;
            }

            // 2. 退回同地區同類別匹配
            let regional_matches: Vec<_> = available
                .iter()
                .filter(|item| {
                    item.region.eq_ignore_ascii_case(&self.region_code)
                        && item.category == category_code
                })
                .collect();

            if !regional_matches.is_empty() {
                let weight_per_match = entry.weight / regional_matches.len() as f64;
                for m in regional_matches {
                    resolved_list.push(ResolvedSpecies {
                        type_id: m.id,
                        name: m.name.to_string(),
                        layer: entry.layer,
                        weight: weight_per_match,
                        footprint_radius_tiles: entry.footprint_radius_tiles,
                        min_elevation: entry.min_elevation,
                        max_elevation: entry.max_elevation,
                        max_slope: entry.max_slope,
                    });
                }
                continue;
            }

            // 3. 退回全局同類別匹配
            let global_matches: Vec<_> = available
                .iter()
                .filter(|item| item.category == category_code)
                .collect();

            if !global_matches.is_empty() {
                let weight_per_match = entry.weight / global_matches.len() as f64;
                for m in global_matches {
                    resolved_list.push(ResolvedSpecies {
                        type_id: m.id,
                        name: m.name.to_string(),
                        layer: entry.layer,
                        weight: weight_per_match,
                        footprint_radius_tiles: entry.footprint_radius_tiles,
                        min_elevation: entry.min_elevation,
                        max_elevation: entry.max_elevation,
                        max_slope: entry.max_slope,
                    });
                }
            }
        }

        // 去重並合併權重 (type_id, layer)
        let mut deduped: HashMap<(i32, VegetationLayer), ResolvedSpecies> = HashMap::new();
        for item in resolved_list {
            let key = (item.type_id, item.layer);
            deduped
                .entry(key)
                .and_modify(|existing| existing.weight += item.weight)
                .or_insert(item);
        }

        ResolvedEcologyRoster::new(deduped.into_values())
    }

    pub fn is_landscape(name: &str) -> bool {
        name.len() >= 3 && name[..3].eq_ignore_ascii_case("Lan")
    }

    pub fn layer_to_category_string(layer: VegetationLayer) -> &'static str {
        match layer {
            VegetationLayer::Canopy => "tree",
            VegetationLayer::Understory => "bush",
            VegetationLayer::GroundFlora => "grass",
            VegetationLayer::Riparian => "water",
            VegetationLayer::Rock => "rock",
        }
    }

    pub fn classify_category(name: &str) -> &'static str {
        let code = if name.len() >= 9 { &name[6..9] } else { "" };
        match code {
            "Nad" | "Lau" | "Bau" | "Pal" | "Bir" | "Eic" | "Buc" | "Kie" | "Tan" | "Fic"
            | "Obs" | "Zyp" | "Pin" => "tree",
            "Gra" | "Bli" | "Blu" => "grass",
            "Dor" | "Bod" | "Bus" | "Str" | "Ger" | "Hec" => "bush",
            "Sch" | "Ent" | "Wei" | "Ser" => "water",
            "Ste" | "Fel" | "Sto" => "rock",
            _ => "other",
        }
    }
}
