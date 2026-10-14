//! Effective loose-file definition snapshots. Selected fields only; not native VFS or a live watcher.
use crate::{lzss, particle_definitions::*, sha256::sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Clone)]
pub struct EffectiveDefinitions {
    pub parameters: BTreeMap<i32, ParameterDefinition>,
    pub geometries: BTreeMap<i32, GeometryDefinition>,
    pub grafiks: BTreeMap<i32, GrafikDefinition>,
    pub objects: BTreeMap<i32, ObjectParticleDefinition>,
    pub sources: Vec<SourceFingerprint>,
}
#[derive(Clone)]
pub struct SourceFingerprint {
    pub path: PathBuf,
    pub sha256: [u8; 32],
    pub length: u64,
    pub modified: SystemTime,
}
#[derive(Clone, Copy)]
enum Table {
    Object,
    Parameter,
    Geometry,
    Grafik,
}
impl Table {
    fn name(self) -> &'static str {
        match self {
            Self::Object => "objdef",
            Self::Parameter => "partpar",
            Self::Geometry => "partgeo",
            Self::Grafik => "partgfx",
        }
    }
    fn section(self) -> &'static str {
        match self {
            Self::Object => "ObjectDefaults",
            Self::Parameter => "ParticleParameterDefault",
            Self::Geometry => "ParticleGeometrieDefault",
            Self::Grafik => "ParticleGrafikDefault",
        }
    }
    fn columns(self) -> usize {
        match self {
            Self::Object => 209,
            Self::Grafik => 9,
            _ => 16,
        }
    }
}
impl EffectiveDefinitions {
    pub fn initialized() -> Self {
        Self {
            parameters: (0..512)
                .map(|id| {
                    (
                        id,
                        ParameterDefinition {
                            id,
                            name: "PP_NoNameEntered".into(),
                            numeric_raw: [0; 14],
                        },
                    )
                })
                .collect(),
            geometries: (0..512)
                .map(|id| {
                    (
                        id,
                        GeometryDefinition {
                            id,
                            name: "PG_NoNameEntered".into(),
                            numeric_raw: [0; 14],
                        },
                    )
                })
                .collect(),
            grafiks: (0..512)
                .map(|id| {
                    (
                        id,
                        GrafikDefinition {
                            id,
                            name: "PX_NoNameEntered".into(),
                            numeric_raw: [0, -1, -1, -1, -1, 0, 0],
                        },
                    )
                })
                .collect(),
            objects: (0..2500)
                .map(|id| {
                    (
                        id,
                        ObjectParticleDefinition {
                            id,
                            name: "OD_NoNameEntered".into(),
                            numeric_raw: [0, 1, -1, -1, -1, -1],
                            mode_raw: None,
                        },
                    )
                })
                .collect(),
            sources: Vec::new(),
        }
    }
    fn apply(&mut self, table: Table, input: &[u8]) -> Result<(), String> {
        let text = decode_text(input)?;
        let mut staged = self.clone();
        let mut inside = false;
        let mut sections = 0;
        for line in text.lines() {
            let row = line.split(';').next().unwrap_or("").trim();
            if row.starts_with('[') {
                inside = row == format!("[{}]", table.section());
                if inside {
                    sections += 1;
                    if sections > 1 {
                        return Err("duplicate definition section".into());
                    }
                }
                continue;
            }
            if !inside || row.is_empty() {
                continue;
            }
            // 567D50 skips delimiter runs: interior empty comma tokens do not preserve a column.
            let values: Vec<_> = row
                .split(',')
                .filter(|s| !s.is_empty())
                .map(str::trim)
                .collect();
            if values.is_empty() || values.len() > table.columns() {
                return Err("unsupported definition column count".into());
            }
            let integer = |i: usize| {
                values[i]
                    .parse::<i32>()
                    .map_err(|_| format!("invalid {} signed32 column {i}", table.name()))
            };
            let id = integer(0)?;
            let capacity = if matches!(table, Table::Object) {
                2500
            } else {
                512
            };
            if !(0..capacity).contains(&id) {
                return Err("definition ID outside table capacity".into());
            }
            if matches!(table, Table::Object) {
                let entry = staged
                    .objects
                    .get_mut(&id)
                    .expect("initialized object capacity");
                for (slot, column) in [1, 2, 5, 63, 64, 65].into_iter().enumerate() {
                    if column < values.len() {
                        entry.numeric_raw[slot] = integer(column)?;
                    }
                }
                if values.len() > 7 {
                    entry.mode_raw = Some(integer(7)?);
                }
                if values.len() > 52 {
                    entry.name = name(values[52])?;
                }
            } else {
                let (words, label): (&mut [i32], &mut String) = match table {
                    Table::Parameter => {
                        let e = staged.parameters.get_mut(&id).unwrap();
                        (&mut e.numeric_raw, &mut e.name)
                    }
                    Table::Geometry => {
                        let e = staged.geometries.get_mut(&id).unwrap();
                        (&mut e.numeric_raw, &mut e.name)
                    }
                    Table::Grafik => {
                        let e = staged.grafiks.get_mut(&id).unwrap();
                        (&mut e.numeric_raw, &mut e.name)
                    }
                    Table::Object => unreachable!(),
                };
                if values.len() > 1 {
                    words[0] = integer(1)?;
                }
                if values.len() > 2 {
                    *label = name(values[2])?;
                }
                for column in 3..values.len() {
                    words[column - 2] = integer(column)?;
                }
            }
        }
        *self = staged;
        Ok(())
    }
}
fn name(value: &str) -> Result<String, String> {
    let token = value
        .split_ascii_whitespace()
        .next()
        .ok_or("empty definition name")?;
    Ok(token.chars().take(30).collect())
}
fn decode_text(input: &[u8]) -> Result<String, String> {
    let bytes = if lzss::has_pfil_magic(input) {
        lzss::decompress_pfil(input, None).map_err(|e| e.to_string())?
    } else {
        input.to_vec()
    };
    if !bytes.is_ascii() || bytes.contains(&0) {
        return Err("unsupported definition text encoding/NUL".into());
    }
    String::from_utf8(bytes).map_err(|e| e.to_string())
}
fn prefix(input: Option<&[u8]>) -> Result<String, String> {
    let Some(input) = input else {
        return Ok("SYSTEM/DATA_MP/DEFAULTS/".into());
    };
    let text = decode_text(input)?;
    let mut inside = false;
    let mut found = None;
    let mut sections = 0;
    for line in text.lines() {
        let row = line.split(';').next().unwrap_or("").trim();
        if row.starts_with('[') {
            inside = row.eq_ignore_ascii_case("[DefaultPath]");
            if inside {
                sections += 1;
            }
            continue;
        }
        if inside && !row.is_empty() {
            if found.is_some() {
                return Err("ambiguous DefaultPath".into());
            }
            found = Some(row.to_owned());
        }
    }
    if sections != 1 {
        return Err("missing/duplicate DefaultPath".into());
    }
    let value = found.ok_or("empty DefaultPath")?;
    if value.contains(':') || value.starts_with('/') || value.starts_with('\\') {
        return Err("DefaultPath must be relative to game root".into());
    }
    let separator = std::path::MAIN_SEPARATOR.to_string();
    Ok(value.replace(['\\', '/'], &separator))
}
fn read_source(
    root: &Path,
    path: &Path,
    sources: &mut Vec<SourceFingerprint>,
) -> Result<Option<Vec<u8>>, String> {
    if !path.try_exists().map_err(|e| e.to_string())? {
        return Ok(None);
    }
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    if !path.starts_with(root) {
        return Err("definition source outside game root".into());
    }
    let metadata = fs::metadata(&path).map_err(|e| e.to_string())?;
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    sources.push(SourceFingerprint {
        path,
        sha256: sha256(&bytes),
        length: metadata.len(),
        modified: metadata.modified().map_err(|e| e.to_string())?,
    });
    Ok(Some(bytes))
}
/// Selected loose DefaultPath + dau, then d00..d09. Missing optional files are no-ops.
/// Unknown VFS/archive selection and non-ASCII source encodings are not silently substituted.
pub fn load(game_root: &Path) -> Result<EffectiveDefinitions, String> {
    if !game_root.is_absolute() {
        return Err("game root must be absolute".into());
    }
    let root = game_root.canonicalize().map_err(|e| e.to_string())?;
    let mut result = EffectiveDefinitions::initialized();
    let engine = read_source(
        &root,
        &root.join("SYSTEM/clengine.ini"),
        &mut result.sources,
    )?;
    let selected = prefix(engine.as_deref())?;
    let tables = [
        Table::Object,
        Table::Parameter,
        Table::Geometry,
        Table::Grafik,
    ];
    let mut loaded = 0;
    for suffix in std::iter::once("dau".to_owned()).chain((0..10).map(|i| format!("d{i:02}"))) {
        for table in tables {
            let path = root.join(format!("{selected}{}.{suffix}", table.name()));
            if let Some(bytes) = read_source(&root, &path, &mut result.sources)? {
                result
                    .apply(table, &bytes)
                    .map_err(|e| format!("{}.{suffix}: {e}", table.name()))?;
                loaded += 1;
            }
        }
    }
    if loaded == 0 {
        return Err("no loose definition sources found".into());
    }
    for source in &result.sources {
        let after = fs::metadata(&source.path).map_err(|e| e.to_string())?;
        if after.len() != source.length
            || after.modified().map_err(|e| e.to_string())? != source.modified
            || sha256(&fs::read(&source.path).map_err(|e| e.to_string())?) != source.sha256
        {
            return Err("definition source changed while loading".into());
        }
    }
    Ok(result)
}
