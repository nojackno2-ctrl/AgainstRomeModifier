//! Designer-facing material palette logic (port of the C# `FloorMaterialCatalog` player naming,
//! `MapSuitability`, and `MapEditorForm` `LoadPalette` / `LoadStampPalette` / `StampCategory`).
//!
//! Everything here is pure: UI crates feed it catalog data and render the result.

use crate::resolver::{NativeTerrainMaterialResolver, StandardFloorMaterialCatalog};
use std::collections::{BTreeSet, HashMap, HashSet};

/// `(id, zh category, zh name, en name, order)` for the verified `4B?___5?` base materials.
const PLAYER_NAMES: [(&str, &str, &str, &str, u32); 27] = [
    ("BB", "草地", "草地", "Grass", 0),
    ("BA", "草地", "深綠草地", "Dark Green Grass", 1),
    ("BC", "草地", "鮮綠草地", "Bright Green Grass", 2),
    ("BD", "草地", "橄欖草地", "Olive Grass", 3),
    ("BW", "草地", "淺綠草地", "Light Green Grass", 4),
    ("BS", "草地", "野草地", "Weed Grass", 5),
    ("BM", "草地", "稀疏草地", "Sparse Grass", 6),
    ("BT", "草地", "泥濘草地", "Muddy Grass", 7),
    ("BU", "草地", "乾草地", "Dry Grass", 8),
    ("BE", "草地", "枯草地", "Withered Grass", 9),
    ("BX", "草地", "紅土草地", "Red Clay Grass", 10),
    ("BV", "荒地", "灰綠荒地", "Grey Green Wasteland", 20),
    ("BL", "荒地", "淺褐荒地", "Light Brown Wasteland", 21),
    ("B5", "沙地", "沙地", "Sand", 30),
    ("BJ", "土地", "黃土地", "Yellow Soil", 40),
    ("B2", "土地", "乾燥土壤", "Dry Soil", 41),
    ("B4", "土地", "黃褐土壤", "Yellow Brown Soil", 42),
    ("B6", "土地", "深色泥土", "Dark Mud", 43),
    ("B7", "土地", "粗糙泥地", "Rough Mud", 44),
    ("B9", "土地", "淺色泥地", "Light Mud", 45),
    ("B1", "土地", "淺灰泥地", "Light Grey Mud", 46),
    ("BI", "土地", "灰褐土壤", "Grey Brown Soil", 47),
    ("B8", "岩地", "深褐砂礫", "Dark Brown Gravel", 60),
    ("B3", "岩地", "灰色岩地", "Grey Rock", 61),
    ("BG", "岩地", "礫石地", "Gravel", 62),
    ("BK", "岩地", "灰色碎石地", "Grey Scree", 63),
    ("BR", "岩地", "風化岩地", "Weathered Rock", 64),
];
const EXTRA_PLAYER_NAMES: [(&str, &str, &str, &str, u32); 1] =
    [("BO", "岩地", "苔蘚岩地", "Mossy Rock", 65)];

fn player_definition(id: &str) -> Option<&'static (&'static str, &'static str, &'static str, &'static str, u32)> {
    PLAYER_NAMES
        .iter()
        .chain(EXTRA_PLAYER_NAMES.iter())
        .find(|d| d.0.eq_ignore_ascii_case(id))
}

/// One paintable base material in the palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteMaterial {
    pub id: String,
    /// Traditional-Chinese category (`草地`, `荒地`, `沙地`, `土地`, `岩地`, `其他地表`).
    pub category: String,
    pub name_zh: String,
    pub name_en: String,
    pub order: u32,
    /// Texture used for the thumbnail (variant ending `50`, else the first variant).
    pub representative: String,
}

impl PaletteMaterial {
    pub fn name(&self, english: bool) -> &str {
        if english {
            &self.name_en
        } else {
            &self.name_zh
        }
    }
    pub fn category_name(&self, english: bool) -> &str {
        localized_category(&self.category, english)
    }
}

pub fn localized_category(category: &str, english: bool) -> &str {
    if !english {
        return category;
    }
    match category {
        "草地" => "Grassland",
        "荒地" => "Wasteland",
        "沙地" => "Sand",
        "土地" => "Dirt",
        "岩地" => "Rock",
        "道路" => "Roads",
        "其他地表" => "Other terrain",
        other => other,
    }
}

/// Builds the base-material palette from a catalog, ordered like the C# catalog.
pub fn build_material_palette(catalog: &StandardFloorMaterialCatalog) -> Vec<PaletteMaterial> {
    let mut materials: Vec<PaletteMaterial> = catalog
        .materials
        .iter()
        .map(|id| {
            let variants = catalog
                .variants_by_material
                .get(&id.to_ascii_uppercase())
                .cloned()
                .unwrap_or_default();
            let representative = variants
                .iter()
                .find(|v| v.to_ascii_uppercase().ends_with("50"))
                .or_else(|| variants.first())
                .cloned()
                .unwrap_or_default();
            match player_definition(id) {
                Some(def) => PaletteMaterial {
                    id: id.clone(),
                    category: def.1.into(),
                    name_zh: def.2.into(),
                    name_en: def.3.into(),
                    order: def.4,
                    representative,
                },
                None => PaletteMaterial {
                    id: id.clone(),
                    category: "其他地表".into(),
                    name_zh: format!("其他地表 {id}"),
                    name_en: format!("Other terrain {id}"),
                    order: 1000,
                    representative,
                },
            }
        })
        .collect();
    materials.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.to_ascii_uppercase().cmp(&b.id.to_ascii_uppercase())));
    materials
}

/// Materials actually used by the map (resolved from the corner materials of every tile).
pub fn map_material_ids(
    resolver: &dyn NativeTerrainMaterialResolver,
    textures: &[String],
) -> HashSet<String> {
    let mut seen_textures: HashSet<&str> = HashSet::new();
    let mut ids = HashSet::new();
    for texture in textures {
        if !seen_textures.insert(texture.as_str()) {
            continue;
        }
        if let Some(corners) = resolver.try_resolve_native_corners(texture) {
            for corner in corners {
                ids.insert(corner.to_ascii_uppercase());
            }
        }
    }
    ids
}

/// Caches "is there a native transition tile between two materials" lookups.
pub struct TransitionProbe<'a> {
    resolver: &'a dyn NativeTerrainMaterialResolver,
    cache: HashMap<(String, String), bool>,
}

impl<'a> TransitionProbe<'a> {
    pub fn new(resolver: &'a dyn NativeTerrainMaterialResolver) -> Self {
        Self {
            resolver,
            cache: HashMap::new(),
        }
    }

    /// True when at least one native two-material corner combination resolves (C# `HasTransition`).
    pub fn has_transition(&mut self, first: &str, second: &str) -> bool {
        if first.eq_ignore_ascii_case(second) {
            return true;
        }
        let (a, b) = (first.to_ascii_uppercase(), second.to_ascii_uppercase());
        let key = if a < b { (a, b) } else { (b, a) };
        if let Some(&cached) = self.cache.get(&key) {
            return cached;
        }
        let mut found = false;
        for mask in 1..15u8 {
            let corner = |bit: u8| if (mask >> bit) & 1 == 1 { second } else { first };
            let corners = [corner(0), corner(1), corner(2), corner(3)];
            if self.resolver.resolve_native_tile(&corners, 0, 0).is_some() {
                found = true;
                break;
            }
        }
        self.cache.insert(key, found);
        found
    }

    /// 0 = already used on the map; 1 = direct transition to a map material; 2 = reachable through
    /// one bridge material (auto transition); 3 = hard to blend (C# `MapSuitability`).
    pub fn map_suitability(
        &mut self,
        material: &str,
        all_materials: &[PaletteMaterial],
        map_materials: &HashSet<String>,
    ) -> u8 {
        if map_materials.contains(&material.to_ascii_uppercase()) {
            return 0;
        }
        let present: Vec<&String> = map_materials.iter().collect();
        if present.iter().any(|p| self.has_transition(material, p)) {
            return 1;
        }
        for bridge in all_materials {
            if self.has_transition(material, &bridge.id)
                && present.iter().any(|p| self.has_transition(&bridge.id, p))
            {
                return 2;
            }
        }
        3
    }
}

/// One palette row after ranking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedMaterial {
    pub material: PaletteMaterial,
    /// See [`TransitionProbe::map_suitability`]; 3 means "this map is hard to blend with it".
    pub tier: u8,
}

/// Ranks and filters the palette for a map (C# `LoadPalette`): sorted by usability tier, hidden
/// hard-to-blend materials when not searching (unless every material is hard), and text filter on
/// the localized name or category.
pub fn rank_materials(
    materials: &[PaletteMaterial],
    probe: &mut TransitionProbe<'_>,
    map_materials: &HashSet<String>,
    filter: &str,
    english: bool,
) -> Vec<RankedMaterial> {
    let filter = filter.trim();
    let needle = filter.to_lowercase();
    let mut ranked: Vec<(usize, RankedMaterial)> = materials
        .iter()
        .enumerate()
        .filter(|(_, m)| {
            filter.is_empty()
                || m.name(english).to_lowercase().contains(&needle)
                || m.category_name(english).to_lowercase().contains(&needle)
        })
        .map(|(order, m)| {
            let tier = probe.map_suitability(&m.id, materials, map_materials);
            (order, RankedMaterial { material: m.clone(), tier })
        })
        .collect();
    ranked.sort_by_key(|(order, r)| (r.tier, *order));
    let mut ranked: Vec<RankedMaterial> = ranked.into_iter().map(|(_, r)| r).collect();
    if filter.is_empty() && ranked.iter().any(|r| r.tier < 3) {
        ranked.retain(|r| r.tier < 3);
    }
    ranked
}

/// Material that "owns" a texture, for eyedropper sampling: the material of a `4B?___5?` base
/// tile, or the first material of a `4U`/`4T` transition tile (C# `FindByTexture`).
pub fn owner_material(texture: &str) -> Option<String> {
    let b = texture.as_bytes();
    if b.len() != 8 || b[0] != b'4' {
        return None;
    }
    match b[1].to_ascii_uppercase() {
        b'B' if b[3] == b'_' && b[4] == b'_' && b[5] == b'_' && b[6] == b'5' && b[2].is_ascii_alphanumeric() => {
            Some(format!("B{}", (b[2] as char).to_ascii_uppercase()))
        }
        b'U' | b'T' if b[2].is_ascii_alphanumeric() => Some(format!("B{}", (b[2] as char).to_ascii_uppercase())),
        _ => None,
    }
}

/// `L<set>B<nn>T<shape><variant>` regional set number (C# `RegionalTileSet`).
pub fn regional_tile_set(name: &str) -> Option<String> {
    let upper = name.to_ascii_uppercase();
    let rest = upper.strip_prefix('L')?;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let tail = &rest[digits.len()..];
    let t = tail.as_bytes();
    // B + two digits + T + shape(1-9) + variant letter
    let ok = t.len() == 6
        && t[0] == b'B'
        && t[1].is_ascii_digit()
        && t[2].is_ascii_digit()
        && t[3] == b'T'
        && (b'1'..=b'9').contains(&t[4])
        && t[5].is_ascii_uppercase();
    ok.then_some(digits)
}

/// Native floortex grouping for the stamp palette (C# `StampCategory`); lower sorts first.
pub fn stamp_category(name: &str) -> u8 {
    let upper = name.to_ascii_uppercase();
    let any = |prefixes: &[&str]| prefixes.iter().any(|p| upper.starts_with(p));
    let bytes = upper.as_bytes();
    if any(&["WEG", "H_WEG", "V_WEG", "PFAD", "PFLASTER", "LUXUSWEG", "PLATZ"]) {
        0
    } else if any(&["FLUSS", "ERDEFLUSS"]) {
        1
    } else if any(&["FELS", "ITA_FELS"]) {
        2
    } else if any(&["MARMOR", "STEINBODEN", "STADT"]) {
        3
    } else if bytes.len() > 1 && bytes[0] == b'L' && bytes[1].is_ascii_digit() {
        5
    } else if bytes.len() > 1 && bytes[0].is_ascii_digit() {
        6
    } else {
        4
    }
}

pub fn localized_stamp_category(category: u8, english: bool) -> &'static str {
    match (category, english) {
        (0, true) => "Roads & plazas",
        (0, false) => "道路與廣場",
        (1, true) => "Rivers",
        (1, false) => "河流",
        (2, true) => "Cliffs",
        (2, false) => "岩壁",
        (3, true) => "Paving",
        (3, false) => "地板",
        (5, true) => "Regional (L)",
        (5, false) => "地區地表 L",
        (6, true) => "Blend tiles",
        (6, false) => "過渡圖塊",
        (_, true) => "Other",
        (_, false) => "其他",
    }
}

/// Regional tile sets the map belongs to: from its own `L<set>` tiles, else inferred from the
/// landscape-object regions (e.g. `GER` -> L2). Empty means "no evidence": nothing is filtered.
pub fn map_regional_sets(map_textures: &[String], nature_regions: &BTreeSet<String>) -> BTreeSet<String> {
    let mut sets: BTreeSet<String> = map_textures.iter().filter_map(|t| regional_tile_set(t)).collect();
    if sets.is_empty() {
        for region in nature_regions {
            let add: &[&str] = match region.to_ascii_uppercase().as_str() {
                "GER" => &["2"],
                "HUN" => &["3"],
                "BRI" => &["4", "04"],
                "ITA" | "ROM" => &["5", "05", "06"],
                "KAR" => &["13", "15"],
                _ => &[],
            };
            sets.extend(add.iter().map(|s| s.to_string()));
        }
    }
    sets
}

/// One stamp-palette entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StampEntry {
    pub texture: String,
    pub category: u8,
}

/// Stamp palette (C# `LoadStampPalette`): all floortex names, filtered to the map's regional
/// sets unless `other_regions` or a search is active, then sorted by category and name.
pub fn stamp_palette(
    names: &[String],
    map_textures: &[String],
    nature_regions: &BTreeSet<String>,
    other_regions: bool,
    filter: &str,
    english: bool,
) -> Vec<StampEntry> {
    let filter = filter.trim();
    let sets = map_regional_sets(map_textures, nature_regions);
    let needle = filter.to_lowercase();
    let mut entries: Vec<StampEntry> = names
        .iter()
        .filter(|name| {
            if !other_regions && filter.is_empty() && !sets.is_empty() {
                return regional_tile_set(name).map_or(true, |set| sets.contains(&set));
            }
            true
        })
        .map(|name| StampEntry {
            texture: name.clone(),
            category: stamp_category(name),
        })
        .filter(|e| {
            filter.is_empty()
                || e.texture.to_lowercase().contains(&needle)
                || localized_stamp_category(e.category, english).to_lowercase().contains(&needle)
        })
        .collect();
    entries.sort_by(|a, b| {
        a.category
            .cmp(&b.category)
            .then_with(|| a.texture.to_ascii_uppercase().cmp(&b.texture.to_ascii_uppercase()))
    });
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names_with_transition() -> Vec<String> {
        let mut names: Vec<String> = ["4BA___51", "4BA___50", "4BB___51", "4BC___51", "4BV___51"]
            .map(Into::into)
            .into();
        for shape in [1, 2, 3, 4, 6, 7, 8, 9] {
            for pair in ["AC", "CA", "CB", "BC"] {
                names.push(format!("4U{pair}__{shape}1"));
            }
        }
        names
    }

    #[test]
    fn palette_names_categories_and_representative() {
        let catalog = StandardFloorMaterialCatalog::from_texture_names(names_with_transition());
        let palette = build_material_palette(&catalog);
        let ids: Vec<&str> = palette.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["BB", "BA", "BC", "BV"]);
        let ba = &palette[1];
        assert_eq!(ba.name(true), "Dark Green Grass");
        assert_eq!(ba.name(false), "深綠草地");
        assert_eq!(ba.category_name(true), "Grassland");
        assert_eq!(ba.representative, "4BA___50");
        assert_eq!(palette[3].category_name(true), "Wasteland");
    }

    #[test]
    fn suitability_tiers_and_difficulty_hint() {
        let names = names_with_transition();
        let catalog = StandardFloorMaterialCatalog::from_texture_names(&names);
        let palette = build_material_palette(&catalog);
        let mut probe = TransitionProbe::new(&catalog);
        assert!(probe.has_transition("BA", "BC"));
        assert!(probe.has_transition("BC", "BB"));
        assert!(!probe.has_transition("BA", "BB"));
        assert!(probe.has_transition("BA", "BA"));

        let map = HashSet::from(["BA".to_string()]);
        assert_eq!(probe.map_suitability("BA", &palette, &map), 0);
        assert_eq!(probe.map_suitability("BC", &palette, &map), 1);
        // BB only connects through BC which connects to BA.
        assert_eq!(probe.map_suitability("BB", &palette, &map), 2);
        // BV has no transition tiles at all: hard to blend.
        assert_eq!(probe.map_suitability("BV", &palette, &map), 3);

        let ranked = rank_materials(&palette, &mut probe, &map, "", true);
        let ids: Vec<&str> = ranked.iter().map(|r| r.material.id.as_str()).collect();
        assert_eq!(ids, ["BA", "BC", "BB"]); // used, direct, bridged; BV hidden
        let searched = rank_materials(&palette, &mut probe, &map, "wasteland", true);
        assert_eq!(searched.len(), 1);
        assert_eq!(searched[0].tier, 3);
        let zh = rank_materials(&palette, &mut probe, &map, "荒地", false);
        assert_eq!(zh[0].material.id, "BV");
    }

    #[test]
    fn all_hard_palette_is_not_hidden() {
        let catalog = StandardFloorMaterialCatalog::from_texture_names(["4BA___51", "4BB___51"]);
        let palette = build_material_palette(&catalog);
        let mut probe = TransitionProbe::new(&catalog);
        let map = HashSet::from(["XX".to_string()]);
        assert_eq!(rank_materials(&palette, &mut probe, &map, "", false).len(), 2);
    }

    #[test]
    fn map_materials_and_owner() {
        let names = names_with_transition();
        let catalog = StandardFloorMaterialCatalog::from_texture_names(&names);
        let textures = vec!["4BA___51".to_string(), "4UCB__21".to_string(), "unknown".to_string()];
        let ids = map_material_ids(&catalog, &textures);
        assert!(ids.contains("BA") && ids.contains("BC") && ids.contains("BB"));
        assert_eq!(owner_material("4BA___51").as_deref(), Some("BA"));
        assert_eq!(owner_material("4UCB__21").as_deref(), Some("BC"));
        assert_eq!(owner_material("4TXYZ_21").as_deref(), Some("BX"));
        assert_eq!(owner_material("H_WEG1"), None);
    }

    #[test]
    fn stamp_categories_and_region_filter() {
        assert_eq!(stamp_category("H_WEG1"), 0);
        assert_eq!(stamp_category("pfad1"), 0);
        assert_eq!(stamp_category("FLUSS_H1"), 1);
        assert_eq!(stamp_category("ITA_FELS1"), 2);
        assert_eq!(stamp_category("MARMOR1"), 3);
        assert_eq!(stamp_category("L2B02T5A"), 5);
        assert_eq!(stamp_category("4UAC__21"), 6);
        assert_eq!(stamp_category("random"), 4);
        assert_eq!(regional_tile_set("L2B02T5A").as_deref(), Some("2"));
        assert_eq!(regional_tile_set("L13B02T1B").as_deref(), Some("13"));
        assert_eq!(regional_tile_set("L2B02T0A"), None);

        let names: Vec<String> = ["H_WEG1", "L2B02T5A", "L3B01T5A", "FLUSS1"].map(Into::into).into();
        let none = BTreeSet::new();
        // No regional evidence: nothing is filtered.
        assert_eq!(stamp_palette(&names, &[], &none, false, "", true).len(), 4);
        // Map has L2 tiles: L3 hidden unless "other regions" or searching.
        let map = vec!["L2B02T5A".to_string()];
        let filtered = stamp_palette(&names, &map, &none, false, "", true);
        assert!(filtered.iter().all(|e| e.texture != "L3B01T5A"));
        assert_eq!(filtered.first().unwrap().texture, "H_WEG1");
        assert_eq!(stamp_palette(&names, &map, &none, true, "", true).len(), 4);
        assert_eq!(stamp_palette(&names, &map, &none, false, "L3", true).len(), 1);
        assert_eq!(stamp_palette(&names, &map, &none, false, "rivers", true)[0].texture, "FLUSS1");
        // Region inferred from landscape objects.
        let regions = BTreeSet::from(["GER".to_string()]);
        let by_region = stamp_palette(&names, &[], &regions, false, "", false);
        assert!(by_region.iter().all(|e| e.texture != "L3B01T5A"));
    }
}
