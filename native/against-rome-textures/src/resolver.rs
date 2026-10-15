use std::collections::{HashMap, HashSet};

/// Native terrain tile lookup contract.
pub trait NativeTerrainMaterialResolver: Send + Sync {
    /// Attempts to determine the four corner material IDs for a given texture.
    /// Order is texture-space: [TL (top-left), TR (top-right), BR (bottom-right), BL (bottom-left)].
    fn try_resolve_native_corners(&self, texture: &str) -> Option<[String; 4]>;

    /// Compiles four corner material IDs into an existing native floor tile name at (x, y).
    /// Returns `None` if the junction is unsupported.
    fn resolve_native_tile(&self, corners: &[&str; 4], x: usize, y: usize) -> Option<String>;

    /// List of materials eligible for auto-bridge transitions (empty by default).
    fn material_ids(&self) -> Vec<String> {
        Vec::new()
    }
}

/// A simple rule-based resolver for unit tests and synthetic workflows.
/// Can parse slash-delimited textures like `"grass/mud/mud/grass"` or rule tables.
#[derive(Debug, Clone, Default)]
pub struct SimpleRuleResolver {
    pub materials: Vec<String>,
    pub supported_pairs: HashSet<String>,
    pub unsupported_materials: HashSet<String>,
}

impl SimpleRuleResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_materials(mut self, materials: &[&str]) -> Self {
        self.materials = materials.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn with_chain_pairs(mut self, pairs: &[&str]) -> Self {
        for pair in pairs {
            let parts: Vec<&str> = pair.split('|').collect();
            if parts.len() == 2 {
                self.supported_pairs
                    .insert(format!("{}|{}", parts[0].to_lowercase(), parts[1].to_lowercase()));
                self.supported_pairs
                    .insert(format!("{}|{}", parts[1].to_lowercase(), parts[0].to_lowercase()));
            }
        }
        self
    }

    pub fn with_unsupported(mut self, unsupported: &[&str]) -> Self {
        for u in unsupported {
            self.unsupported_materials.insert(u.to_lowercase());
        }
        self
    }
}

impl NativeTerrainMaterialResolver for SimpleRuleResolver {
    fn try_resolve_native_corners(&self, texture: &str) -> Option<[String; 4]> {
        let parts: Vec<&str> = texture.split('/').collect();
        if parts.len() == 4 {
            Some([
                parts[0].to_string(),
                parts[1].to_string(),
                parts[2].to_string(),
                parts[3].to_string(),
            ])
        } else {
            None
        }
    }

    fn resolve_native_tile(&self, corners: &[&str; 4], _x: usize, _y: usize) -> Option<String> {
        for c in corners {
            if self.unsupported_materials.contains(&c.to_lowercase()) {
                return None;
            }
        }

        // If chain pairs are defined, verify that all distinct pairs are supported
        if !self.supported_pairs.is_empty() {
            let mut distinct: Vec<&str> = Vec::new();
            for c in corners {
                if !distinct.iter().any(|d| d.eq_ignore_ascii_case(c)) {
                    distinct.push(c);
                }
            }
            if distinct.len() == 1 {
                return Some(corners.join("/"));
            }
            if distinct.len() == 2 {
                let pair_key = format!("{}|{}", distinct[0].to_lowercase(), distinct[1].to_lowercase());
                if self.supported_pairs.contains(&pair_key) {
                    return Some(corners.join("/"));
                }
            }
            return None;
        }

        Some(corners.join("/"))
    }

    fn material_ids(&self) -> Vec<String> {
        self.materials.clone()
    }
}

/// FNV-1a stable hash picker matching C# `PickStable`.
pub fn pick_stable(seed: &str, variants: &[String], x: usize, y: usize) -> String {
    let mut hash: u32 = 2166136261;
    for b in seed.bytes() {
        hash = (hash ^ (b as u32)).wrapping_mul(16777619);
    }
    hash = (hash ^ (x as u32)).wrapping_mul(16777619);
    hash = (hash ^ (y as u32)).wrapping_mul(16777619);
    variants[(hash as usize) % variants.len()].clone()
}

/// Constructs corner key in uppercase: `"C0|C1|C2|C3"`.
pub fn corner_key(corners: &[&str; 4]) -> String {
    format!(
        "{}|{}|{}|{}",
        corners[0].to_ascii_uppercase(),
        corners[1].to_ascii_uppercase(),
        corners[2].to_ascii_uppercase(),
        corners[3].to_ascii_uppercase()
    )
}

/// Standard catalog implementation that parses Against Rome native texture names.
#[derive(Debug, Clone, Default)]
pub struct StandardFloorMaterialCatalog {
    pub materials: Vec<String>,
    pub variants_by_material: HashMap<String, Vec<String>>,
    pub native_corners_by_texture: HashMap<String, [String; 4]>,
    pub native_textures_by_corners: HashMap<String, Vec<String>>,
}

impl StandardFloorMaterialCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a catalog from an enumeration of texture names (e.g. from `floortex.dat` or a map's `boden.txt`).
    pub fn from_texture_names<I, S>(names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut catalog = Self::new();
        let name_list: Vec<String> = names.into_iter().map(|s| s.as_ref().to_string()).collect();

        // 1. Base materials: 4B<code: 1char>___5<variant: 1char>
        for name in &name_list {
            if let Some(mat_id) = parse_base_material(name) {
                if !catalog.materials.iter().any(|m| m.eq_ignore_ascii_case(&mat_id)) {
                    catalog.materials.push(mat_id.clone());
                }
                catalog
                    .variants_by_material
                    .entry(mat_id.to_ascii_uppercase())
                    .or_default()
                    .push(name.clone());

                catalog.native_corners_by_texture.insert(
                    name.to_ascii_uppercase(),
                    [mat_id.clone(), mat_id.clone(), mat_id.clone(), mat_id.clone()],
                );
            }
        }

        // 2. 2-material transitions: 4U<1char><1char>__<shape: 1char><variant: 1char>
        for name in &name_list {
            if let Some((first, second, shape)) = parse_two_material_transition(name) {
                if let Some(corners) = two_material_corners(&first, &second, shape) {
                    catalog
                        .native_corners_by_texture
                        .insert(name.to_ascii_uppercase(), corners);
                }
            }
        }

        // 3. 3-material transitions: 4T<1char><1char><1char>_<shape: 1char><variant: 1char>
        for name in &name_list {
            if let Some((first, second, third, shape)) = parse_three_material_transition(name) {
                if let Some(corners) = three_material_corners(&first, &second, &third, shape) {
                    catalog
                        .native_corners_by_texture
                        .insert(name.to_ascii_uppercase(), corners);
                }
            }
        }

        // Group textures by corner key
        for (tex, corners) in &catalog.native_corners_by_texture {
            let key = corner_key(&[
                &corners[0],
                &corners[1],
                &corners[2],
                &corners[3],
            ]);
            catalog
                .native_textures_by_corners
                .entry(key)
                .or_default()
                .push(tex.clone());
        }

        // Sort variant lists for determinism
        for variants in catalog.native_textures_by_corners.values_mut() {
            variants.sort();
        }
        for variants in catalog.variants_by_material.values_mut() {
            variants.sort();
        }

        catalog
    }
}

impl NativeTerrainMaterialResolver for StandardFloorMaterialCatalog {
    fn try_resolve_native_corners(&self, texture: &str) -> Option<[String; 4]> {
        let key = texture.to_ascii_uppercase();
        self.native_corners_by_texture.get(&key).cloned()
    }

    fn resolve_native_tile(&self, corners: &[&str; 4], x: usize, y: usize) -> Option<String> {
        let all_same = corners[1].eq_ignore_ascii_case(corners[0])
            && corners[2].eq_ignore_ascii_case(corners[0])
            && corners[3].eq_ignore_ascii_case(corners[0]);

        if all_same {
            let mat_key = corners[0].to_ascii_uppercase();
            if let Some(variants) = self.variants_by_material.get(&mat_key) {
                if !variants.is_empty() {
                    return Some(pick_stable(&mat_key, variants, x, y));
                }
            }
        }

        let key = corner_key(corners);
        if let Some(variants) = self.native_textures_by_corners.get(&key) {
            if !variants.is_empty() {
                return Some(pick_stable(&key, variants, x, y));
            }
        }

        None
    }

    fn material_ids(&self) -> Vec<String> {
        self.materials.clone()
    }
}

// Helpers for name pattern parsing
fn parse_base_material(name: &str) -> Option<String> {
    // 4B<code: 1char>___5<variant: 1char>
    let b = name.as_bytes();
    if b.len() == 8
        && (b[0] == b'4' || b[0] == b'4')
        && (b[1] == b'B' || b[1] == b'b')
        && b[2].is_ascii_alphanumeric()
        && b[3] == b'_'
        && b[4] == b'_'
        && b[5] == b'_'
        && b[6] == b'5'
        && b[7].is_ascii_alphanumeric()
    {
        let code = (b[2] as char).to_ascii_uppercase();
        Some(format!("B{}", code))
    } else {
        None
    }
}

fn parse_two_material_transition(name: &str) -> Option<(String, String, u8)> {
    // 4U<first: 1char><second: 1char>__<shape: 1-4,6-9><variant: 1char>
    let b = name.as_bytes();
    if b.len() == 8
        && (b[0] == b'4')
        && (b[1] == b'U' || b[1] == b'u')
        && b[2].is_ascii_alphanumeric()
        && b[3].is_ascii_alphanumeric()
        && b[4] == b'_'
        && b[5] == b'_'
        && (b[6] >= b'1' && b[6] <= b'9' && b[6] != b'5')
        && b[7].is_ascii_alphanumeric()
    {
        let first = format!("B{}", (b[2] as char).to_ascii_uppercase());
        let second = format!("B{}", (b[3] as char).to_ascii_uppercase());
        let shape = b[6] - b'0';
        Some((first, second, shape))
    } else {
        None
    }
}

fn parse_three_material_transition(name: &str) -> Option<(String, String, String, u8)> {
    // 4T<first><second><third>_<shape: 2,4,6,8><variant>
    let b = name.as_bytes();
    if b.len() == 8
        && (b[0] == b'4')
        && (b[1] == b'T' || b[1] == b't')
        && b[2].is_ascii_alphanumeric()
        && b[3].is_ascii_alphanumeric()
        && b[4].is_ascii_alphanumeric()
        && b[5] == b'_'
        && (b[6] == b'2' || b[6] == b'4' || b[6] == b'6' || b[6] == b'8')
        && b[7].is_ascii_alphanumeric()
    {
        let first = format!("B{}", (b[2] as char).to_ascii_uppercase());
        let second = format!("B{}", (b[3] as char).to_ascii_uppercase());
        let third = format!("B{}", (b[4] as char).to_ascii_uppercase());
        let shape = b[6] - b'0';
        Some((first, second, third, shape))
    } else {
        None
    }
}

/// Corner order is texture-space: TL, TR, BR, BL
fn two_material_corners(first: &str, second: &str, shape: u8) -> Option<[String; 4]> {
    let f = first.to_string();
    let s = second.to_string();
    match shape {
        1 => Some([s.clone(), f.clone(), s.clone(), s]),
        2 => Some([f.clone(), f.clone(), s.clone(), s]),
        3 => Some([f.clone(), s.clone(), s.clone(), s]),
        4 => Some([s.clone(), f.clone(), f.clone(), s]),
        6 => Some([f.clone(), s.clone(), s.clone(), f]),
        7 => Some([s.clone(), s.clone(), f.clone(), s]),
        8 => Some([s.clone(), s.clone(), f.clone(), f]),
        9 => Some([s.clone(), s.clone(), s.clone(), f]),
        _ => None,
    }
}

fn three_material_corners(first: &str, second: &str, third: &str, shape: u8) -> Option<[String; 4]> {
    let f = first.to_string();
    let s = second.to_string();
    let t = third.to_string();
    match shape {
        2 => Some([t, s, f.clone(), f]),
        4 => Some([f.clone(), t, s, f]),
        6 => Some([t, f.clone(), f, s]),
        8 => Some([f.clone(), f, s, t]),
        _ => None,
    }
}
