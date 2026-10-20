//! Designer-facing material palette logic (port of the C# `FloorMaterialCatalog` player naming,
//! `MapSuitability`, and `MapEditorForm` `LoadPalette` / `LoadStampPalette` / `StampCategory`).
//!
//! Everything here is pure: UI crates feed it catalog data and render the result.

use crate::resolver::{corner_key, NativeTerrainMaterialResolver, StandardFloorMaterialCatalog};
use crate::terrain_layer::TerrainLayer;
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
    /// English category label (regional `L` sets carry their set number).
    pub category_en: String,
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
        if english {
            &self.category_en
        } else {
            &self.category
        }
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
            if let Some((set, index)) = regional_material_parts(id) {
                return PaletteMaterial {
                    id: id.clone(),
                    category: format!("地區地表 L{set}"),
                    category_en: format!("Regional terrain L{set}"),
                    name_zh: format!("L{set} 地表 {index}"),
                    name_en: format!("L{set} terrain {index}"),
                    order: 2000,
                    representative,
                };
            }
            if id.eq_ignore_ascii_case(PATH_MATERIAL_ID) {
                return PaletteMaterial {
                    id: id.clone(),
                    category: "道路".into(),
                    category_en: localized_category("道路", true).into(),
                    name_zh: "土路".into(),
                    name_en: "Dirt Road".into(),
                    order: 3000,
                    representative,
                };
            }
            match player_definition(id) {
                Some(def) => PaletteMaterial {
                    id: id.clone(),
                    category: def.1.into(),
                    category_en: localized_category(def.1, true).into(),
                    name_zh: def.2.into(),
                    name_en: def.3.into(),
                    order: def.4,
                    representative,
                },
                None => PaletteMaterial {
                    id: id.clone(),
                    category: "其他地表".into(),
                    category_en: "Other terrain".into(),
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

/// Material id of the original dirt road (`PFAD1`-`PFAD9` pure road surfaces; C# `PathMaterialId`).
pub const PATH_MATERIAL_ID: &str = "PFAD";

/// Maximum mean corner colour distance (RGB Euclidean) for accepting an inferred transition
/// (C# `RegionalTransitionMaxCornerDistance`).
pub const REGIONAL_TRANSITION_MAX_CORNER_DISTANCE: f64 = 32.0;
/// Looser outer-material match for dirt road borders (C# `PathApproximateMaxCornerDistance`).
pub const PATH_APPROXIMATE_MAX_CORNER_DISTANCE: f64 = 16.0;
/// Same outer material keeps only the best-fitting border groups within this tolerance.
const PATH_GROUP_TOLERANCE: f64 = 2.0;

type Rgb = [f64; 3];

/// Regional material id (C# `RegionalMaterialId`): `L{set}:{index}`.
pub fn regional_material_id(set: &str, index: &str) -> String {
    format!("L{set}:{index}")
}

/// Splits `L{set}:{index}` back into `(set, index)`.
pub fn regional_material_parts(id: &str) -> Option<(&str, &str)> {
    let rest = id.strip_prefix('L').or_else(|| id.strip_prefix('l'))?;
    let (set, index) = rest.split_once(':')?;
    (!set.is_empty() && set.bytes().all(|b| b.is_ascii_digit()) && !index.is_empty()).then_some((set, index))
}

/// `L<set>B<nn>T<shape><variant>` -> `(set, index, shape)` (C# `RegionalTileName`).
fn parse_regional_tile(name: &str) -> Option<(String, String, u8)> {
    let upper = name.to_ascii_uppercase();
    let rest = upper.strip_prefix('L')?;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let t = rest[digits.len()..].as_bytes();
    let ok = t.len() == 6
        && t[0] == b'B'
        && t[1].is_ascii_digit()
        && t[2].is_ascii_digit()
        && t[3] == b'T'
        && (b'1'..=b'9').contains(&t[4])
        && t[5].is_ascii_uppercase();
    ok.then(|| (digits, String::from_utf8_lossy(&t[1..3]).into_owned(), t[4] - b'0'))
}

fn is_path_base(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() == 5 && name[..4].eq_ignore_ascii_case("PFAD") && (b'1'..=b'9').contains(&b[4])
}

/// `PFAD[_Erde]<shape 1-4,6-9><A|B><variant 0-9>` -> `(group, shape, inner)`.
fn parse_path_transition(name: &str) -> Option<(String, u8, bool)> {
    let upper = name.to_ascii_uppercase();
    let rest = upper.strip_prefix("PFAD")?;
    let (erde, rest) = match rest.strip_prefix("_ERDE") {
        Some(r) => (true, r),
        None => (false, rest),
    };
    let b = rest.as_bytes();
    if b.len() != 3
        || !((b'1'..=b'9').contains(&b[0]) && b[0] != b'5')
        || !(b[1] == b'A' || b[1] == b'B')
        || !b[2].is_ascii_digit()
    {
        return None;
    }
    Some((format!("{}{}", if erde { "Erde" } else { "" }, b[2] as char), b[0] - b'0', b[1] == b'B'))
}

fn layer_valid(layer: &TerrainLayer) -> bool {
    layer.width > 0 && layer.height > 0 && layer.argb.len() >= layer.width * layer.height
}

/// Port of C# `MeanColor`: sparse 8x8 sampling of an area.
fn mean_color(layer: &TerrainLayer, x: usize, y: usize, width: usize, height: usize) -> Rgb {
    let (mut r, mut g, mut b, mut count) = (0u64, 0u64, 0u64, 0u64);
    let step_x = (width / 8).max(1);
    let step_y = (height / 8).max(1);
    let mut sy = y + step_y / 2;
    while sy < y + height {
        let mut sx = x + step_x / 2;
        while sx < x + width {
            let px = layer.argb[sy.min(layer.height - 1) * layer.width + sx.min(layer.width - 1)];
            r += ((px >> 16) & 0xff) as u64;
            g += ((px >> 8) & 0xff) as u64;
            b += (px & 0xff) as u64;
            count += 1;
            sx += step_x;
        }
        sy += step_y;
    }
    let n = count.max(1) as f64;
    [r as f64 / n, g as f64 / n, b as f64 / n]
}

fn mean_texture_color(layer: &TerrainLayer) -> Option<Rgb> {
    layer_valid(layer).then(|| mean_color(layer, 0, 0, layer.width, layer.height))
}

/// Four corner samples TL, TR, BR, BL (each 1/4 x 1/4 of the image).
fn corner_colors(layer: &TerrainLayer) -> Option<[Rgb; 4]> {
    if !layer_valid(layer) {
        return None;
    }
    let w = (layer.width / 4).max(1);
    let h = (layer.height / 4).max(1);
    Some([
        mean_color(layer, 0, 0, w, h),
        mean_color(layer, layer.width - w, 0, w, h),
        mean_color(layer, layer.width - w, layer.height - h, w, h),
        mean_color(layer, 0, layer.height - h, w, h),
    ])
}

fn dist2(a: &Rgb, b: &Rgb) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

/// Texture-pixel provider used to infer transition corners (C# `textureResolver`).
pub type TextureProbeFn<'a> = &'a mut dyn FnMut(&str) -> Option<TerrainLayer>;

/// Builds the complete standard catalog: the `4B`/`4U`/`4T` families of
/// [`StandardFloorMaterialCatalog::from_texture_names`] plus the C# `FloorMaterialCatalog` extras:
/// L-series regional materials, the `PFAD` dirt road and (when `probe` supplies texture pixels)
/// 0-prefixed corner-mask `4U` families, regional `L` transitions and `PFAD` border tiles whose
/// corners are inferred from the bitmaps.
pub fn build_full_catalog(names: &[String], probe: Option<TextureProbeFn<'_>>) -> StandardFloorMaterialCatalog {
    let mut seen = HashSet::new();
    let unique: Vec<String> = names
        .iter()
        .filter(|n| seen.insert(n.to_ascii_uppercase()))
        .cloned()
        .collect();
    let mut catalog = StandardFloorMaterialCatalog::from_texture_names(&unique);
    extend_catalog(&mut catalog, &unique, probe);
    catalog
}

type RegionalTile = (String, String, String, u8);

/// Adds the L-series / PFAD / mask materials to a base catalog (names must be unique).
pub fn extend_catalog(catalog: &mut StandardFloorMaterialCatalog, names: &[String], mut probe: Option<TextureProbeFn<'_>>) {
    let mut path_owned: HashSet<String> = HashSet::new();
    let mut path_resolution: Vec<(String, [String; 4])> = Vec::new();

    // Regional pure materials: every tile of (set, index) is shape 5.
    let regional: Vec<RegionalTile> = names
        .iter()
        .filter_map(|n| parse_regional_tile(n).map(|(s, i, shape)| (n.clone(), s, i, shape)))
        .collect();
    let mut groups: Vec<((String, String), Vec<&RegionalTile>)> = Vec::new();
    for tile in &regional {
        let key = (tile.1.clone(), tile.2.clone());
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, tiles)) => tiles.push(tile),
            None => groups.push((key, vec![tile])),
        }
    }
    groups.sort_by(|a, b| a.0.cmp(&b.0));
    for ((set, index), tiles) in &groups {
        if !tiles.iter().all(|t| t.3 == 5) {
            continue;
        }
        let id = regional_material_id(set, index);
        let mut variants: Vec<String> = tiles.iter().map(|t| t.0.clone()).collect();
        variants.sort_by_key(|v| v.to_ascii_uppercase());
        for v in &variants {
            catalog
                .native_corners_by_texture
                .insert(v.to_ascii_uppercase(), [id.clone(), id.clone(), id.clone(), id.clone()]);
        }
        catalog.variants_by_material.insert(id.to_ascii_uppercase(), variants);
        catalog.materials.push(id);
    }
    // Dirt road pure surfaces.
    let mut path_bases: Vec<String> = names.iter().filter(|n| is_path_base(n)).cloned().collect();
    path_bases.sort_by_key(|v| v.to_ascii_uppercase());
    if !path_bases.is_empty() {
        let id = PATH_MATERIAL_ID.to_string();
        for v in &path_bases {
            catalog
                .native_corners_by_texture
                .insert(v.to_ascii_uppercase(), [id.clone(), id.clone(), id.clone(), id.clone()]);
        }
        catalog.variants_by_material.insert(id.to_ascii_uppercase(), path_bases.clone());
        catalog.materials.push(id);
    }

    if let Some(probe) = probe.as_deref_mut() {
        register_mask_transitions(catalog, names, probe);
        register_regional_transitions(catalog, &groups, probe);
        if !path_bases.is_empty() {
            register_path_transitions(catalog, names, &path_bases[0], probe, &mut path_owned, &mut path_resolution);
        }
    }

    // Rebuild the corner index; path border tiles resolve only through their approximate-outer corners.
    catalog.native_textures_by_corners.clear();
    let mut entries: Vec<(String, [String; 4])> = catalog
        .native_corners_by_texture
        .iter()
        .filter(|(tex, _)| !path_owned.contains(*tex))
        .map(|(tex, corners)| (tex.clone(), corners.clone()))
        .collect();
    entries.extend(path_resolution);
    for (tex, corners) in entries {
        let key = corner_key(&[&corners[0], &corners[1], &corners[2], &corners[3]]);
        let list = catalog.native_textures_by_corners.entry(key).or_default();
        if !list.contains(&tex) {
            list.push(tex);
        }
    }
    for list in catalog.native_textures_by_corners.values_mut() {
        list.sort();
    }
}

fn material_known(catalog: &StandardFloorMaterialCatalog, id: &str) -> bool {
    catalog.materials.iter().any(|m| m.eq_ignore_ascii_case(id))
}

fn representative_color(
    catalog: &StandardFloorMaterialCatalog,
    id: &str,
    probe: &mut dyn FnMut(&str) -> Option<TerrainLayer>,
) -> Option<Rgb> {
    let variants = catalog.variants_by_material.get(&id.to_ascii_uppercase())?;
    // C# representative: first sorted variant for L / PFAD materials, the `...50` variant for 4B.
    let rep = if regional_material_parts(id).is_some() || id.eq_ignore_ascii_case(PATH_MATERIAL_ID) {
        variants.first()?
    } else {
        variants
            .iter()
            .find(|v| v.to_ascii_uppercase().ends_with("50"))
            .or_else(|| variants.first())?
    };
    probe(rep).as_ref().and_then(mean_texture_color)
}

/// Nearest-reference corner assignment for the given material colours (C# `InferCorners`).
fn infer_corners(layer: &TerrainLayer, refs: &[(String, Rgb)]) -> Option<[String; 4]> {
    let samples = corner_colors(layer)?;
    let pick = |s: &Rgb| {
        let mut best = &refs[0];
        for r in &refs[1..] {
            if dist2(s, &r.1) < dist2(s, &best.1) {
                best = r;
            }
        }
        best.0.clone()
    };
    Some([pick(&samples[0]), pick(&samples[1]), pick(&samples[2]), pick(&samples[3])])
}

fn distinct_count(corners: &[String; 4]) -> usize {
    let mut seen: Vec<&String> = Vec::new();
    for c in corners {
        if !seen.iter().any(|s| s.eq_ignore_ascii_case(c)) {
            seen.push(c);
        }
    }
    seen.len()
}

fn register_mask_transitions(
    catalog: &mut StandardFloorMaterialCatalog,
    names: &[String],
    probe: &mut dyn FnMut(&str) -> Option<TerrainLayer>,
) {
    let families = crate::resolver::mask_transition_families(names);
    let mut color_cache: HashMap<String, Option<Rgb>> = HashMap::new();
    for name in names {
        let Some((first, second, _)) = crate::resolver::parse_mask_transition(name) else {
            continue;
        };
        if !families.contains(&format!("{first}|{second}"))
            || !material_known(catalog, &first)
            || !material_known(catalog, &second)
        {
            continue;
        }
        let mut refs = Vec::new();
        for id in [&first, &second] {
            if !color_cache.contains_key(id) {
                let color = representative_color(catalog, id, probe);
                color_cache.insert(id.clone(), color);
            }
            if let Some(color) = color_cache[id] {
                refs.push((id.clone(), color));
            }
        }
        if refs.len() != 2 {
            continue;
        }
        let Some(layer) = probe(name) else { continue };
        let Some(corners) = infer_corners(&layer, &refs) else { continue };
        if distinct_count(&corners) != 2 {
            continue;
        }
        catalog.native_corners_by_texture.insert(name.to_ascii_uppercase(), corners);
    }
}

type RegionalGroups<'a> = [((String, String), Vec<&'a RegionalTile>)];

fn register_regional_transitions(
    catalog: &mut StandardFloorMaterialCatalog,
    groups: &RegionalGroups<'_>,
    probe: &mut dyn FnMut(&str) -> Option<TerrainLayer>,
) {
    let mut base_colors: Vec<(String, String, Rgb)> = Vec::new();
    for id in catalog.materials.clone() {
        if let Some((set, _)) = regional_material_parts(&id) {
            let set = set.to_string();
            if let Some(color) = representative_color(catalog, &id, probe) {
                base_colors.push((id, set, color));
            }
        }
    }
    for ((set, index), tiles) in groups {
        if material_known(catalog, &regional_material_id(set, index)) {
            continue;
        }
        let corners: Vec<(&str, [Rgb; 4])> = tiles
            .iter()
            .filter(|t| t.3 != 5)
            .filter_map(|t| probe(&t.0).and_then(|l| corner_colors(&l)).map(|c| (t.0.as_str(), c)))
            .collect();
        if corners.is_empty() {
            continue;
        }
        let mut candidates: Vec<&(String, String, Rgb)> = base_colors.iter().filter(|b| &b.1 == set).collect();
        if candidates.len() < 2 {
            candidates = base_colors.iter().collect();
        }
        let mut best: Option<(usize, usize, f64)> = None;
        for a in 0..candidates.len() {
            for b in a + 1..candidates.len() {
                let mut total = 0.0;
                for (_, cs) in &corners {
                    for c in cs {
                        total += dist2(c, &candidates[a].2).min(dist2(c, &candidates[b].2)).sqrt();
                    }
                }
                if best.map_or(true, |(_, _, t)| total < t) {
                    best = Some((a, b, total));
                }
            }
        }
        let Some((a, b, total)) = best else { continue };
        if total / (corners.len() * 4) as f64 > REGIONAL_TRANSITION_MAX_CORNER_DISTANCE {
            continue;
        }
        let (first, second) = (candidates[a], candidates[b]);
        for (name, cs) in &corners {
            let assigned: [String; 4] = std::array::from_fn(|i| {
                if dist2(&cs[i], &first.2) <= dist2(&cs[i], &second.2) {
                    first.0.clone()
                } else {
                    second.0.clone()
                }
            });
            if distinct_count(&assigned) == 2 {
                catalog.native_corners_by_texture.insert(name.to_ascii_uppercase(), assigned);
            }
        }
    }
}

struct PathTile {
    name: String,
    pattern: [String; 4],
    corners: [Rgb; 4],
}

fn register_path_transitions(
    catalog: &mut StandardFloorMaterialCatalog,
    names: &[String],
    path_base: &str,
    probe: &mut dyn FnMut(&str) -> Option<TerrainLayer>,
    path_owned: &mut HashSet<String>,
    path_resolution: &mut Vec<(String, [String; 4])>,
) {
    let Some(path_color) = probe(path_base).as_ref().and_then(mean_texture_color) else {
        return;
    };
    let mut outer_colors: Vec<(String, Rgb)> = Vec::new();
    for id in catalog.materials.clone() {
        if id.eq_ignore_ascii_case(PATH_MATERIAL_ID) {
            continue;
        }
        if let Some(color) = representative_color(catalog, &id, probe) {
            outer_colors.push((id, color));
        }
    }
    let mut groups: Vec<(String, Vec<PathTile>)> = Vec::new();
    for name in names {
        let Some((group, shape, inner)) = parse_path_transition(name) else {
            continue;
        };
        let Some(corners) = probe(name).and_then(|l| corner_colors(&l)) else {
            continue;
        };
        let pattern = if inner {
            crate::resolver::two_material_corners("O", "P", shape)
        } else {
            crate::resolver::two_material_corners("P", "O", shape)
        };
        let Some(pattern) = pattern else { continue };
        let tile = PathTile { name: name.clone(), pattern, corners };
        match groups.iter_mut().find(|(g, _)| g.eq_ignore_ascii_case(&group)) {
            Some((_, tiles)) => tiles.push(tile),
            None => groups.push((group, vec![tile])),
        }
    }
    let mut accepted: Vec<(&Vec<PathTile>, Vec<(String, f64)>)> = Vec::new();
    for (_, tiles) in &groups {
        let (mut path_total, mut path_count) = (0.0, 0usize);
        for tile in tiles {
            for c in 0..4 {
                if tile.pattern[c] == "P" {
                    path_total += dist2(&tile.corners[c], &path_color).sqrt();
                    path_count += 1;
                }
            }
        }
        let mut fits: Vec<(String, f64)> = outer_colors
            .iter()
            .map(|(id, color)| {
                let (mut total, mut count) = (0.0, 0usize);
                for tile in tiles {
                    for c in 0..4 {
                        if tile.pattern[c] == "O" {
                            total += dist2(&tile.corners[c], color).sqrt();
                            count += 1;
                        }
                    }
                }
                (id.clone(), if count > 0 { total / count as f64 } else { f64::MAX })
            })
            .collect();
        fits.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let Some(best) = fits.first().cloned() else { continue };
        let usable = best.1 <= REGIONAL_TRANSITION_MAX_CORNER_DISTANCE
            && path_count > 0
            && path_total / path_count as f64 <= REGIONAL_TRANSITION_MAX_CORNER_DISTANCE;
        if !usable {
            continue;
        }
        let outers: Vec<(String, f64)> = std::iter::once(best.clone())
            .chain(fits.iter().skip(1).filter(|f| f.1 <= PATH_APPROXIMATE_MAX_CORNER_DISTANCE).cloned())
            .collect();
        for tile in tiles {
            let corners: [String; 4] = std::array::from_fn(|i| {
                if tile.pattern[i] == "P" {
                    PATH_MATERIAL_ID.to_string()
                } else {
                    best.0.clone()
                }
            });
            let upper = tile.name.to_ascii_uppercase();
            catalog.native_corners_by_texture.insert(upper.clone(), corners);
            path_owned.insert(upper);
        }
        accepted.push((tiles, outers));
    }
    let mut best_by_outer: HashMap<String, f64> = HashMap::new();
    for (_, fits) in &accepted {
        for (outer, avg) in fits {
            let entry = best_by_outer.entry(outer.to_ascii_uppercase()).or_insert(f64::MAX);
            if *avg < *entry {
                *entry = *avg;
            }
        }
    }
    for (tiles, fits) in &accepted {
        for (outer, avg) in fits {
            if *avg > best_by_outer[&outer.to_ascii_uppercase()] + PATH_GROUP_TOLERANCE {
                continue;
            }
            for tile in tiles.iter() {
                let corners: [String; 4] = std::array::from_fn(|i| {
                    if tile.pattern[i] == "P" {
                        PATH_MATERIAL_ID.to_string()
                    } else {
                        outer.clone()
                    }
                });
                path_resolution.push((tile.name.to_ascii_uppercase(), corners));
            }
        }
    }
}

/// Material that owns `texture` in `catalog` (C# `FindByTexture`): base/transition tiles of the
/// `4B`/`4U`/`4T` families, L-series regional tiles and `PFAD` road tiles.
pub fn owner_material_in(catalog: &StandardFloorMaterialCatalog, texture: &str) -> Option<String> {
    if let Some(id) = owner_material(texture) {
        return Some(id);
    }
    let upper = texture.to_ascii_uppercase();
    if is_path_base(&upper) {
        return Some(PATH_MATERIAL_ID.to_string());
    }
    if let Some((set, index, 5)) = parse_regional_tile(&upper) {
        return Some(regional_material_id(&set, &index)).filter(|id| material_known(catalog, id));
    }
    let corners = catalog.native_corners_by_texture.get(&upper)?;
    if parse_path_transition(&upper).is_some() {
        return Some(PATH_MATERIAL_ID.to_string());
    }
    if parse_regional_tile(&upper).is_some() {
        return Some(corners[0].clone());
    }
    // Mask-named 4U tiles belong to their first material.
    crate::resolver::parse_mask_transition(&upper).map(|(first, _, _)| first)
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

    fn layer(corners: [u32; 4]) -> TerrainLayer {
        // 8x8 image: quadrants TL, TR, BR, BL with the given RGB colours.
        let mut argb = vec![0u32; 64];
        for y in 0..8 {
            for x in 0..8 {
                let q = match (x < 4, y < 4) {
                    (true, true) => 0,
                    (false, true) => 1,
                    (false, false) => 2,
                    (true, false) => 3,
                };
                argb[y * 8 + x] = 0xFF00_0000 | corners[q];
            }
        }
        TerrainLayer { width: 8, height: 8, argb, green: vec![0; 64] }
    }

    const GRASS: u32 = 0x30A030;
    const DIRT: u32 = 0xA06030;
    const ROAD: u32 = 0xC8C8B4;

    fn probe_for(name: &str) -> Option<TerrainLayer> {
        Some(match name.to_ascii_uppercase().as_str() {
            "L2B01T5A" => layer([GRASS; 4]),
            "L2B02T5A" => layer([DIRT; 4]),
            // transition number 03: grass on the top half, dirt on the bottom half
            "L2B03T2A" => layer([GRASS, GRASS, DIRT, DIRT]),
            "4BJ___51" => layer([DIRT; 4]),
            "4BX___51" => layer([GRASS; 4]),
            "4UJX__05" => layer([DIRT, GRASS, GRASS, DIRT]),
            "4UJX__14" => layer([GRASS, DIRT, DIRT, DIRT]),
            "PFAD1" => layer([ROAD; 4]),
            // PFAD shape 2 kind A: road on the top half (TwoMaterialCorners(P, O, 2) = P,P,O,O)
            "PFAD2A0" => layer([ROAD, ROAD, GRASS, GRASS]),
            _ => return None,
        })
    }

    #[test]
    fn regional_and_path_materials_join_the_palette() {
        let names: Vec<String> = ["4BB___51", "L2B01T5A", "L2B01T5B", "L2B02T5A", "L2B03T2A", "PFAD1", "PFAD2", "PFAD2A0"]
            .map(Into::into)
            .into();
        let catalog = build_full_catalog(&names, None);
        let palette = build_material_palette(&catalog);
        let ids: Vec<&str> = palette.iter().map(|m| m.id.as_str()).collect();
        // L03 is a transition number (not shape 5 only), so it is not a pure material.
        assert_eq!(ids, ["BB", "L2:01", "L2:02", "PFAD"]);
        let l = &palette[1];
        assert_eq!(l.category, "地區地表 L2");
        assert_eq!(l.category_name(true), "Regional terrain L2");
        assert_eq!(l.name(false), "L2 地表 01");
        assert_eq!(l.representative, "L2B01T5A");
        let road = &palette[3];
        assert_eq!((road.category.as_str(), road.name(false), road.name(true)), ("道路", "土路", "Dirt Road"));
        assert_eq!(road.category_name(true), "Roads");
        assert_eq!(owner_material_in(&catalog, "L2B01T5B").as_deref(), Some("L2:01"));
        assert_eq!(owner_material_in(&catalog, "pfad2").as_deref(), Some("PFAD"));
        assert_eq!(owner_material_in(&catalog, "4BB___51").as_deref(), Some("BB"));
        // Pure regional / road surfaces resolve as all-same corners.
        let corners = catalog.try_resolve_native_corners("L2B02T5A").unwrap();
        assert!(corners.iter().all(|c| c == "L2:02"));
        assert!(catalog
            .resolve_native_tile(&["PFAD"; 4], 0, 0)
            .is_some_and(|t| t.starts_with("PFAD")));
        // Without pixels nothing is guessed for transitions.
        assert!(catalog.try_resolve_native_corners("L2B03T2A").is_none());
        assert!(catalog.try_resolve_native_corners("PFAD2A0").is_none());
    }

    #[test]
    fn pixel_probe_infers_regional_path_and_mask_transitions() {
        let names: Vec<String> =
            ["4BJ___51", "4BX___51", "4UJX__05", "4UJX__14", "L2B01T5A", "L2B02T5A", "L2B03T2A", "PFAD1", "PFAD2A0"]
                .map(Into::into)
                .into();
        // The mask family must not be mistaken for 9-grid shapes (4UMX__14 would parse as shape 1).
        let plain = build_full_catalog(&names, None);
        assert!(plain.try_resolve_native_corners("4UJX__05").is_none());
        assert!(plain.try_resolve_native_corners("4UJX__14").is_none());

        let mut probe = |name: &str| probe_for(name);
        let catalog = build_full_catalog(&names, Some(&mut probe));
        // Mask: first = BJ (dirt), second = BX (grass); corners inferred by colour.
        assert_eq!(
            catalog.try_resolve_native_corners("4UJX__05").unwrap(),
            ["BJ".to_string(), "BX".into(), "BX".into(), "BJ".into()]
        );
        // 4UJX__14 belongs to the same mask family (not shape 1); its corners come from the colours.
        assert_eq!(
            catalog.try_resolve_native_corners("4UJX__14").unwrap(),
            ["BX".to_string(), "BJ".into(), "BJ".into(), "BJ".into()]
        );
        // Regional transition number 03 pairs the two L2 materials by corner colour.
        assert_eq!(
            catalog.try_resolve_native_corners("L2B03T2A").unwrap(),
            ["L2:01".to_string(), "L2:01".into(), "L2:02".into(), "L2:02".into()]
        );
        assert_eq!(owner_material_in(&catalog, "L2B03T2A").as_deref(), Some("L2:01"));
        // Dirt road border: road on the top half, grass outside.
        let road = catalog.try_resolve_native_corners("PFAD2A0").unwrap();
        assert_eq!(&road[..2], ["PFAD".to_string(), "PFAD".into()]);
        assert_eq!(owner_material_in(&catalog, "PFAD2A0").as_deref(), Some("PFAD"));
        // The mask tile is reachable through its corner combination.
        assert!(catalog
            .resolve_native_tile(&["BJ", "BX", "BX", "BJ"], 0, 0)
            .is_some_and(|t| t.eq_ignore_ascii_case("4UJX__05")));
        // The road border resolves via the approximate outer materials as well.
        assert!(catalog.resolve_native_tile(&["PFAD", "PFAD", "BX", "BX"], 3, 4).is_some());
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
