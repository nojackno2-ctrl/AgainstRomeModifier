use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use zip::ZipArchive;

use super::document::ShadowDocument;
use super::error::ShadowError;

/// Reads one shadow mask BMP by file name; `None` when the asset is missing.
pub type TextureReader = Box<dyn Fn(&str) -> Option<Vec<u8>> + Send + Sync>;

/// Upper bound for a single shadow BMP read from `shad.dat` (real masks are < 200 KiB).
const MAX_TEXTURE_BYTES: u64 = 16 * 1024 * 1024;

/// Shadow footprint shape (objdef `shtyp`): 0 = axis-aligned box quad, 1 = circle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowType {
    Box = 0,
    Circle = 1,
}

/// objdef shadow fields plus the BMP resolved through `cl_shado.ini`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowDefinition {
    /// objdef `shidx`.
    pub shadow_id: i32,
    /// objdef `shsiz`: world half-extent / radius.
    pub shadow_size: i32,
    /// objdef `shtyp`.
    pub shadow_type: ShadowType,
    /// objdef `shacx`: world X offset of the shadow center from the object anchor.
    pub correction_x: i32,
    /// objdef `shacz`: world Z offset of the shadow center from the object anchor.
    pub correction_z: i32,
    pub texture_file_name: String,
}

/// World-space axis-aligned shadow quad (corner order NW, NE, SE, SW).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowWorldQuad {
    pub center_x: f64,
    pub center_z: f64,
    pub half_extent: f64,
    pub corner_nw: (f64, f64),
    pub corner_ne: (f64, f64),
    pub corner_se: (f64, f64),
    pub corner_sw: (f64, f64),
}

impl ShadowWorldQuad {
    pub fn from_center_and_size(center_x: f64, center_z: f64, half_extent: f64) -> Self {
        Self {
            center_x,
            center_z,
            half_extent,
            corner_nw: (center_x - half_extent, center_z - half_extent),
            corner_ne: (center_x + half_extent, center_z - half_extent),
            corner_se: (center_x + half_extent, center_z + half_extent),
            corner_sw: (center_x - half_extent, center_z + half_extent),
        }
    }

    /// Corners in NW, NE, SE, SW order.
    pub fn corners(&self) -> [(f64, f64); 4] {
        [self.corner_nw, self.corner_ne, self.corner_se, self.corner_sw]
    }

    /// Whether a world point lies inside the quad (edges inclusive). Non-finite points are outside.
    pub fn contains(&self, x: f64, z: f64) -> bool {
        x.is_finite()
            && z.is_finite()
            && (x - self.center_x).abs() <= self.half_extent
            && (z - self.center_z).abs() <= self.half_extent
    }
}

/// An object's resolved shadow: definition, mask document, and projection helpers.
#[derive(Debug, Clone)]
pub struct ObjectShadow {
    pub object_name: String,
    pub definition: ShadowDefinition,
    pub document: Arc<ShadowDocument>,
}

impl ObjectShadow {
    /// World quad for an object anchored at `(anchor_x, anchor_z)`.
    ///
    /// Pure arithmetic exactly like the C# oracle: non-finite anchors propagate as NaN/inf,
    /// callers that need validation should check `is_finite` first.
    pub fn compute_world_quad(&self, anchor_x: f64, anchor_z: f64) -> ShadowWorldQuad {
        let cx = anchor_x + f64::from(self.definition.correction_x);
        let cz = anchor_z + f64::from(self.definition.correction_z);
        ShadowWorldQuad::from_center_and_size(cx, cz, f64::from(self.definition.shadow_size))
    }

    /// 2:1 isometric screen-pixel offset of the shadow center from the anchor:
    /// `dx = (shacx - shacz) / 2`, `dy = (shacx + shacz) / 4`.
    pub fn compute_screen_offset(&self) -> (f64, f64) {
        let (x, z) = (f64::from(self.definition.correction_x), f64::from(self.definition.correction_z));
        ((x - z) / 2.0, (x + z) / 4.0)
    }
}

/// Catalog of object shadows: objdef definitions + `cl_shado.ini` names + `shad.dat` masks.
pub struct ShadowCatalog {
    definitions: HashMap<String, ShadowDefinition>,
    shadow_name_count: usize,
    read_texture: TextureReader,
    documents: Mutex<HashMap<String, Option<Arc<ShadowDocument>>>>,
}

impl std::fmt::Debug for ShadowCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShadowCatalog")
            .field("definitions", &self.definitions.len())
            .field("shadow_names", &self.shadow_name_count)
            .finish()
    }
}

/// Ordinal-ignore-case key (per-char simple uppercase, like .NET `OrdinalIgnoreCase`).
fn fold(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            let mut upper = c.to_uppercase();
            match (upper.next(), upper.next()) {
                (Some(u), None) => u,
                _ => c,
            }
        })
        .collect()
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl ShadowCatalog {
    /// Builds a catalog from parsed definitions (first duplicate name, ignoring case, wins).
    pub fn new(
        definitions: impl IntoIterator<Item = (String, ShadowDefinition)>,
        shadow_name_count: usize,
        read_texture: TextureReader,
    ) -> Self {
        let mut map = HashMap::new();
        for (name, definition) in definitions {
            map.entry(fold(&name)).or_insert(definition);
        }
        Self {
            definitions: map,
            shadow_name_count,
            read_texture,
            documents: Mutex::new(HashMap::new()),
        }
    }

    /// Builds from objdef text, `cl_shado.ini` text and a texture reader.
    pub fn from_text(
        objdef_text: &str,
        shadow_names_ini_text: &str,
        read_texture: TextureReader,
    ) -> Self {
        let names = parse_shadow_names(shadow_names_ini_text);
        let definitions = parse_objdef_shadows(objdef_text, &names);
        Self::new(definitions, names.len(), read_texture)
    }

    /// Opens the shadow assets read-only from a flat asset directory or a game install layout
    /// (`SYSTEM/cl_shado.ini`, `SYSTEM/DATA_MP/DEFAULTS/objdef.dau`, `shad.dat`).
    ///
    /// `Ok(None)` when the directory or any required file is missing; `Err` when a present
    /// file cannot be read or decoded.
    pub fn open(assets_path: impl AsRef<Path>) -> Result<Option<Self>, ShadowError> {
        let root = assets_path.as_ref();
        if !root.is_dir() {
            return Ok(None);
        }
        let shad_path = root.join("shad.dat");
        if !shad_path.is_file() {
            return Ok(None);
        }
        let Some(cl_shado) = find_file(
            root,
            &["cl_shado.ini", "cl_shado.txt", "SYSTEM/cl_shado.ini"],
        ) else {
            return Ok(None);
        };
        let Some(objdef) = find_file(
            root,
            &["objdef.dau", "objdef.txt", "SYSTEM/DATA_MP/DEFAULTS/objdef.dau"],
        ) else {
            return Ok(None);
        };

        let names = parse_shadow_names(&read_game_text(&cl_shado)?);
        let definitions = parse_objdef_shadows(&read_game_text(&objdef)?, &names);

        let file = File::open(&shad_path).map_err(|e| ShadowError::Io(format!("{}: {e}", shad_path.display())))?;
        let mut archive =
            ZipArchive::new(file).map_err(|e| ShadowError::invalid(format!("shad.dat is not a valid ZIP: {e}")))?;
        let mut index: HashMap<String, usize> = HashMap::new();
        for i in 0..archive.len() {
            let name = archive
                .by_index_raw(i)
                .map_err(|e| ShadowError::invalid(format!("shad.dat entry {i}: {e}")))?
                .name()
                .to_owned();
            index.entry(fold(&name)).or_insert(i);
            let leaf = name.rsplit(['/', '\\']).next().unwrap_or("");
            if !leaf.is_empty() {
                index.entry(fold(leaf)).or_insert(i);
            }
        }
        let archive = Mutex::new(archive);
        let reader: TextureReader = Box::new(move |texture: &str| {
            let slot = index
                .get(&fold(&format!("SYSTEM/DATA/SHADOWTEXTURE/{texture}")))
                .or_else(|| index.get(&fold(texture)))
                .copied()?;
            let mut archive = lock(&archive);
            let entry = archive.by_index(slot).ok()?;
            if entry.is_dir() || entry.size() > MAX_TEXTURE_BYTES {
                return None;
            }
            let mut bytes = Vec::with_capacity(entry.size() as usize);
            entry.take(MAX_TEXTURE_BYTES + 1).read_to_end(&mut bytes).ok()?;
            (bytes.len() as u64 <= MAX_TEXTURE_BYTES).then_some(bytes)
        });
        Ok(Some(Self::new(definitions, names.len(), reader)))
    }

    pub fn definition_count(&self) -> usize {
        self.definitions.len()
    }

    pub fn shadow_name_mapping_count(&self) -> usize {
        self.shadow_name_count
    }

    /// Definition for an object name (ignoring case), without touching the mask archive.
    pub fn try_get_definition(&self, object_name: &str) -> Option<&ShadowDefinition> {
        self.definitions.get(&fold(object_name))
    }

    /// Resolved shadow, or `None` for no shadow (`shidx < 0`), unknown objects, and missing or
    /// invalid mask assets.
    pub fn get_shadow(&self, object_name: &str) -> Option<ObjectShadow> {
        let definition = self.try_get_definition(object_name)?;
        let document = self.get_document(&definition.texture_file_name)?;
        Some(ObjectShadow {
            object_name: object_name.to_owned(),
            definition: definition.clone(),
            document,
        })
    }

    /// Loads and caches one mask BMP. Missing, empty, or invalid assets cache a `None`.
    pub fn get_document(&self, texture_file_name: &str) -> Option<Arc<ShadowDocument>> {
        let key = fold(texture_file_name);
        let mut cache = lock(&self.documents);
        if let Some(cached) = cache.get(&key) {
            return cached.clone();
        }
        let document = (self.read_texture)(texture_file_name)
            .filter(|data| !data.is_empty())
            .and_then(|data| ShadowDocument::parse(&data).ok())
            .map(Arc::new);
        cache.insert(key, document.clone());
        document
    }
}

fn find_file(directory: &Path, candidates: &[&str]) -> Option<PathBuf> {
    candidates.iter().map(|c| directory.join(c)).find(|p| p.is_file())
}

fn read_game_text(path: &Path) -> Result<String, ShadowError> {
    let mut bytes = std::fs::read(path).map_err(|e| ShadowError::Io(format!("{}: {e}", path.display())))?;
    if bytes.starts_with(b"PFIL") {
        bytes = against_rome_data::lzss::decompress_pfil(&bytes, None)
            .map_err(|e| ShadowError::invalid(format!("{}: {e}", path.display())))?;
    }
    let (text, _) = encoding_rs::WINDOWS_1251.decode_without_bom_handling(&bytes);
    Ok(text.into_owned())
}

/// Parses the `[ShadowNames]` section of `cl_shado.ini` (`0000,small_round_shadow.bmp`).
/// IDs outside `0..2000`, empty names and names containing a path separator are skipped;
/// the first duplicate ID wins.
pub fn parse_shadow_names(text: &str) -> HashMap<i32, String> {
    let mut result = HashMap::new();
    let mut in_section = false;
    for raw in text.split('\n') {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') {
            in_section = line.eq_ignore_ascii_case("[ShadowNames]");
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((id, file_name)) = line.split_once(',') else { continue };
        let Ok(id) = id.trim().parse::<i32>() else { continue };
        if !(0..2000).contains(&id) {
            continue;
        }
        let file_name = file_name.trim();
        if !file_name.is_empty() && !file_name.contains('/') && !file_name.contains('\\') {
            result.entry(id).or_insert_with(|| file_name.to_owned());
        }
    }
    result
}

/// Parses objdef shadow columns (`shidx 11, shsiz 12, shtyp 13, shacx 15, shacz 16, name 52`
/// unless a `;idx` header line relocates them), resolving `shidx` through `shadow_names`.
/// Objects with `shidx < 0`, unmapped ids, or malformed numbers are skipped. Results keep file
/// order; the first duplicate name (ignoring case) wins.
pub fn parse_objdef_shadows(
    text: &str,
    shadow_names: &HashMap<i32, String>,
) -> Vec<(String, ShadowDefinition)> {
    let mut result: Vec<(String, ShadowDefinition)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let (mut c_idx, mut c_siz, mut c_typ, mut c_acx, mut c_acz, c_name) =
        (11usize, 12usize, 13usize, 15usize, 16usize, 52usize);

    for raw in text.split('\n') {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('[') {
            continue;
        }
        if line.get(..4).is_some_and(|p| p.eq_ignore_ascii_case(";idx")) {
            let headers: Vec<&str> = line.split(',').map(|h| h.trim().trim_start_matches(';')).collect();
            let find = |name: &str| headers.iter().position(|h| *h == name);
            if let Some(i) = find("shidx") {
                c_idx = i;
            }
            if let Some(i) = find("shsiz") {
                c_siz = i;
            }
            if let Some(i) = find("shtyp") {
                c_typ = i;
            }
            if let Some(i) = find("shacx") {
                c_acx = i;
            }
            if let Some(i) = find("shacz") {
                c_acz = i;
            }
            continue;
        }
        if line.starts_with(';') {
            continue;
        }

        let cols: Vec<&str> = line.split(',').collect();
        let min_cols = [c_name, c_idx, c_siz, c_typ, c_acx, c_acz].into_iter().max().unwrap_or(0);
        if cols.len() <= min_cols {
            continue;
        }
        let name = cols[c_name].trim();
        if name.is_empty() {
            continue;
        }
        let parse = |i: usize| cols[i].trim().parse::<i32>().ok();
        let Some(shidx) = parse(c_idx).filter(|v| *v >= 0) else { continue };
        let (Some(shsiz), Some(shtyp), Some(shacx), Some(shacz)) =
            (parse(c_siz), parse(c_typ), parse(c_acx), parse(c_acz))
        else {
            continue;
        };
        let Some(texture) = shadow_names.get(&shidx).filter(|t| !t.is_empty()) else { continue };
        if !seen.insert(fold(name)) {
            continue;
        }
        result.push((
            name.to_owned(),
            ShadowDefinition {
                shadow_id: shidx,
                shadow_size: shsiz,
                shadow_type: if shtyp == 1 { ShadowType::Circle } else { ShadowType::Box },
                correction_x: shacx,
                correction_z: shacz,
                texture_file_name: texture.clone(),
            },
        ));
    }
    result
}
