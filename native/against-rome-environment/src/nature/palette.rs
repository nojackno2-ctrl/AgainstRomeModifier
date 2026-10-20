//! Designer-facing nature palette, regional filtering, species mixing, and brush workflows.
//! Faithful port of C# `src.MapEditor/MapEditorForm.Nature.cs`.

use super::biome::BiomeEcologyProfile;
use super::engine::NatureAddition;
use super::scatter::{NatureDensity, NatureScatter};
use crate::random::DotNetRandom;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 地景物件名稱的地區代碼（例如 LanGerNad18 -> "Ger"）。
/// 若名稱過短（小於 6 字元）或非有效 UTF-8 字元邊界則回傳空字串。
pub fn nature_region(name: &str) -> &str {
    if name.len() >= 6 && name.is_char_boundary(3) && name.is_char_boundary(6) {
        &name[3..6]
    } else {
        ""
    }
}

/// 判斷自然物件分類（比照 C# `MapEditorForm.Nature.cs:NatureCategory`）。
pub fn nature_category(name: &str) -> NatureCategory {
    NatureCategory::classify(name)
}

/// 自然物件分類類別。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum NatureCategory {
    Tree,
    Grass,
    Bush,
    Water,
    Rock,
    Other,
}

impl NatureCategory {
    pub const ALL: [NatureCategory; 6] = [
        NatureCategory::Tree,
        NatureCategory::Grass,
        NatureCategory::Bush,
        NatureCategory::Water,
        NatureCategory::Rock,
        NatureCategory::Other,
    ];

    /// 根據物件名稱代碼判斷分類（比照 C# `MapEditorForm.Nature.cs:NatureCategory`）。
    pub fn classify(name: &str) -> Self {
        let code = if name.len() >= 9 && name.is_char_boundary(6) && name.is_char_boundary(9) {
            &name[6..9]
        } else {
            ""
        };
        match code {
            "Nad" | "Lau" | "Bau" | "Pal" | "Bir" | "Eic" | "Buc" | "Kie" | "Tan" | "Fic"
            | "Obs" | "Zyp" | "Pin" => Self::Tree,
            "Gra" | "Bli" | "Blu" => Self::Grass,
            "Dor" | "Bod" | "Bus" | "Str" | "Ger" | "Hec" => Self::Bush,
            "Sch" | "Ent" | "Wei" | "Ser" => Self::Water,
            "Ste" | "Fel" | "Sto" => Self::Rock,
            _ => Self::Other,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Tree => "tree",
            Self::Grass => "grass",
            Self::Bush => "bush",
            Self::Water => "water",
            Self::Rock => "rock",
            Self::Other => "other",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key.trim() {
            "tree" => Some(Self::Tree),
            "grass" => Some(Self::Grass),
            "bush" => Some(Self::Bush),
            "water" => Some(Self::Water),
            "rock" => Some(Self::Rock),
            "other" => Some(Self::Other),
            _ => None,
        }
    }

    pub fn display_name(self, is_en: bool) -> &'static str {
        match (self, is_en) {
            (Self::Tree, true) => "Trees",
            (Self::Tree, false) => "樹木",
            (Self::Grass, true) => "Grass & flowers",
            (Self::Grass, false) => "草叢與花",
            (Self::Bush, true) => "Bushes",
            (Self::Bush, false) => "灌木",
            (Self::Water, true) => "Reeds & water plants",
            (Self::Water, false) => "蘆葦與水生植物",
            (Self::Rock, true) => "Rocks",
            (Self::Rock, false) => "岩石",
            (Self::Other, true) => "Other landscape",
            (Self::Other, false) => "其他地景",
        }
    }
}

/// 自然物件操作模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NatureOperation {
    Plant,
    Remove,
}

impl NatureOperation {
    pub fn display_name(self, is_en: bool) -> &'static str {
        match (self, is_en) {
            (Self::Plant, true) => "Plant",
            (Self::Plant, false) => "種植",
            (Self::Remove, true) => "Remove",
            (Self::Remove, false) => "移除",
        }
    }
}

/// 地圖原有地景物件中佔多數的地區代碼（至少占 15% 的地區都算，涵蓋混合地區地圖）；地圖沒有地景物件時為空集合。
/// 比照 C# `MapEditorForm.Nature.cs:MapNatureRegions`。
pub fn map_nature_regions<'a, I>(landscape_names: I) -> BTreeSet<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut total = 0;

    for name in landscape_names {
        if BiomeEcologyProfile::is_landscape(name) {
            let reg = nature_region(name);
            if reg.len() == 3 {
                let canonical_key = reg.to_ascii_uppercase();
                *counts.entry(canonical_key).or_insert(0) += 1;
                total += 1;
            }
        }
    }

    if total == 0 {
        return BTreeSet::new();
    }

    let mut result = BTreeSet::new();
    for (reg, count) in counts {
        if count * 100 >= total * 15 {
            result.insert(reg);
        }
    }

    result
}

/// 自然物件範本條目。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NatureCatalogItem {
    pub type_id: i32,
    pub name: String,
}

impl NatureCatalogItem {
    pub fn new(type_id: i32, name: impl Into<String>) -> Self {
        Self {
            type_id,
            name: name.into(),
        }
    }

    #[inline]
    pub fn region(&self) -> &str {
        nature_region(&self.name)
    }

    #[inline]
    pub fn category(&self) -> NatureCategory {
        NatureCategory::classify(&self.name)
    }
}

/// 編輯器自然物件選單過濾器。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NaturePaletteFilter {
    /// 類別過濾（None 為 "all"）。
    pub category: Option<NatureCategory>,
    /// 地圖主要地區代碼（大寫，例如 `["GER"]`）。
    pub dominant_regions: BTreeSet<String>,
    /// 是否顯示其他地區物件。若為 true，則忽略地區過濾。
    pub show_other_regions: bool,
}

impl NaturePaletteFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_category(mut self, category: Option<NatureCategory>) -> Self {
        self.category = category;
        self
    }

    pub fn with_dominant_regions(mut self, regions: BTreeSet<String>) -> Self {
        self.dominant_regions = regions;
        self
    }

    pub fn with_show_other_regions(mut self, show: bool) -> Self {
        self.show_other_regions = show;
        self
    }

    /// 根據分類、主要地區與顯示選項過濾目錄物件，並按名稱不分大小寫排序。
    /// 比照 C# `MapEditorForm.Nature.cs:RefreshNatureTypes`。
    pub fn filter_and_sort<'a>(&self, items: &'a [NatureCatalogItem]) -> Vec<&'a NatureCatalogItem> {
        let use_region_filter = !self.show_other_regions && !self.dominant_regions.is_empty();

        let mut filtered: Vec<&'a NatureCatalogItem> = items
            .iter()
            .filter(|item| {
                if let Some(cat) = self.category {
                    if item.category() != cat {
                        return false;
                    }
                }
                if use_region_filter {
                    let reg = item.region();
                    if !self
                        .dominant_regions
                        .iter()
                        .any(|dominant| dominant.eq_ignore_ascii_case(reg))
                    {
                        return false;
                    }
                }
                true
            })
            .collect();

        filtered.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        filtered
    }
}

/// 自然物件混合物種選擇器。
/// 混合時只取與所選物種同地區（LanGer、LanIta…）的同類物件，避免日耳曼森林混進義大利柏樹。
/// 比照 C# `MapEditorForm.Nature.cs:PaintNatureTile` 的 `_natureMix` 機制。
pub struct NatureSpeciesMixer;

impl NatureSpeciesMixer {
    /// 找出與選取物件同地區（例如 "Ger"）且同類別（例如 Tree）的候選清單，
    /// 且按照名稱不區分大小寫排序，對齊 C# ListBox Items 的既有排序。
    pub fn matching_species<'a>(
        selected: &NatureCatalogItem,
        palette_items: &'a [NatureCatalogItem],
    ) -> Vec<&'a NatureCatalogItem> {
        let sel_region = selected.region();
        let sel_category = selected.category();
        let mut candidates: Vec<&'a NatureCatalogItem> = palette_items
            .iter()
            .filter(|item| {
                sel_region.eq_ignore_ascii_case(item.region()) && item.category() == sel_category
            })
            .collect();
        candidates.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        candidates
    }

    /// 依據 `mix_species` 開關抽選物種。
    /// 在 C# 中，未勾選混合時 `species` 為 `[type]`（長度為 1），必定調用一次 `_natureRandom.Next(1)`
    /// 推進 PRNG 狀態後才由後續 `NextDouble()` 計算旋轉角度；此處如實推進以維持 100% 隨機序列對拍。
    pub fn pick_species<'a>(
        selected: &'a NatureCatalogItem,
        palette_items: &'a [NatureCatalogItem],
        mix_species: bool,
        random: &mut DotNetRandom,
    ) -> &'a NatureCatalogItem {
        if !mix_species {
            let _ = random.next_i32(1);
            return selected;
        }
        let pool = Self::matching_species(selected, palette_items);
        if pool.is_empty() {
            selected
        } else {
            let idx = random.next_i32(pool.len() as i32) as usize;
            pool[idx]
        }
    }
}

/// 已放置於地圖中的世界物件參照。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlacedNatureObject {
    pub slot: i32,
    pub type_id: i32,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rotation: f32,
    pub team: i32,
    pub linked: bool,
}

impl PlacedNatureObject {
    pub fn new(slot: i32, type_id: i32, name: impl Into<String>, x: f32, z: f32, linked: bool) -> Self {
        Self {
            slot,
            type_id,
            name: name.into(),
            x,
            y: 0.0,
            z,
            rotation: 0.0,
            team: 8,
            linked,
        }
    }

    pub fn with_transform(
        slot: i32,
        type_id: i32,
        name: impl Into<String>,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
        team: i32,
        linked: bool,
    ) -> Self {
        Self {
            slot,
            type_id,
            name: name.into(),
            x,
            y,
            z,
            rotation,
            team,
            linked,
        }
    }

    /// 判斷是否為地景物件（Lan*）。
    #[inline]
    pub fn is_landscape(&self) -> bool {
        BiomeEcologyProfile::is_landscape(&self.name)
    }

    /// 判斷是否為可移除的自然物件（未被腳本或結構連結，且為 Lan* 地景物件）。
    /// 比照 C# `MapEditorForm.Nature.cs:IsRemovableNature`。
    #[inline]
    pub fn is_removable_nature(&self) -> bool {
        !self.linked && self.is_landscape()
    }
}

/// 自然物件筆刷移除規劃器。
/// 在指定圓形筆刷半徑內清除未被鎖定的樹木、草叢、岩石等自然物件，同時清除草稿中對應的新增物。
/// 比照 C# `MapEditorForm.Nature.cs:PaintNatureTile` 移除分支。
pub struct NatureBrushRemover;

impl NatureBrushRemover {
    pub fn plan_removal(
        center_world: (f32, f32),
        radius_world: f32,
        level_objects: &[PlacedNatureObject],
        pending_additions: &[NatureAddition],
        already_removed_slots: &BTreeSet<i32>,
    ) -> (Vec<i32>, Vec<NatureAddition>) {
        let (cx, cz) = center_world;
        let radius_sq = radius_world * radius_world;

        let removed_slots: Vec<i32> = level_objects
            .iter()
            .filter(|item| {
                item.is_removable_nature()
                    && !already_removed_slots.contains(&item.slot)
                    && {
                        let dx = item.x - cx;
                        let dz = item.z - cz;
                        dx * dx + dz * dz <= radius_sq
                    }
            })
            .map(|item| item.slot)
            .collect();

        let removed_additions: Vec<NatureAddition> = pending_additions
            .iter()
            .filter(|item| {
                let dx = item.x - cx;
                let dz = item.z - cz;
                dx * dx + dz * dz <= radius_sq
            })
            .cloned()
            .collect();

        (removed_slots, removed_additions)
    }

    /// 以 tile 座標與筆刷尺寸規劃筆刷移除。
    /// 中心點偏移 (tile + 0.5) * tile_world_size，半徑為 (brush_size / 2 + 0.26) * tile_world_size。
    /// 比照 C# `MapEditorForm.Nature.cs:PaintNatureTile` 移除分支。
    pub fn plan_removal_tile(
        tile_x: i32,
        tile_y: i32,
        brush_size: i32,
        tile_world_size: f32,
        level_objects: &[PlacedNatureObject],
        pending_additions: &[NatureAddition],
        already_removed_slots: &BTreeSet<i32>,
    ) -> (Vec<i32>, Vec<NatureAddition>) {
        let cx = (tile_x as f32 + 0.5) * tile_world_size;
        let cz = (tile_y as f32 + 0.5) * tile_world_size;
        let radius = (brush_size as f32 / 2.0 + 0.26) * tile_world_size;
        Self::plan_removal(
            (cx, cz),
            radius,
            level_objects,
            pending_additions,
            already_removed_slots,
        )
    }
}

/// 自然物件筆刷種植規劃器。
/// 支援單格精確放置或大筆刷密度散佈，格內隨機偏移、旋轉，以及同地區物種隨機混種。
/// 比照 C# `MapEditorForm.Nature.cs:PaintNatureTile` 種植分支。
pub struct NatureBrushPainter;

impl NatureBrushPainter {
    pub fn plan_plant(
        tile_x: i32,
        tile_y: i32,
        brush_size: i32,
        density: NatureDensity,
        dimension: i32,
        tile_world_size: f32,
        existing_additions: &[NatureAddition],
        level_objects: &[PlacedNatureObject],
        removed_slots: &BTreeSet<i32>,
        selected_item: &NatureCatalogItem,
        palette_items: &[NatureCatalogItem],
        mix_species: bool,
        sample_height: impl Fn(f32, f32) -> f32,
        random: &mut DotNetRandom,
    ) -> Vec<NatureAddition> {
        let ncx = (tile_x as f32 + 0.5) * tile_world_size;
        let ncz = (tile_y as f32 + 0.5) * tile_world_size;
        let near = (brush_size as f32 / 2.0 + 2.0) * tile_world_size;

        let mut existing_tile_points = Vec::new();
        for item in existing_additions {
            if (item.x - ncx).abs() <= near && (item.z - ncz).abs() <= near {
                existing_tile_points.push((item.x / tile_world_size, item.z / tile_world_size));
            }
        }
        for item in level_objects {
            if item.is_removable_nature() && !removed_slots.contains(&item.slot) {
                if (item.x - ncx).abs() <= near && (item.z - ncz).abs() <= near {
                    existing_tile_points.push((item.x / tile_world_size, item.z / tile_world_size));
                }
            }
        }

        let points = NatureScatter::plan(
            tile_x,
            tile_y,
            brush_size,
            density,
            dimension,
            &existing_tile_points,
            random,
        );

        let mut additions = Vec::with_capacity(points.len());
        for (px, pz) in points {
            let world_x = px * tile_world_size;
            let world_z = pz * tile_world_size;
            let world_y = sample_height(world_x, world_z);
            let pick = NatureSpeciesMixer::pick_species(
                selected_item,
                palette_items,
                mix_species,
                random,
            );
            let rotation = (random.next_double() * std::f64::consts::PI * 2.0) as f32;
            additions.push(NatureAddition::new(
                pick.type_id,
                pick.name.clone(),
                world_x,
                world_y,
                world_z,
                rotation,
            ));
        }

        additions
    }
}
