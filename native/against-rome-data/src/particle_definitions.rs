//! Strict ASCII base tables bound to parameter/geometry/gfx and selected objdef callback writers.
//! No overlays, native text-error emulation, geometry simulation, or asset residency inference.
use crate::lzss;
use std::collections::BTreeMap;

const PARAMETER_COLUMNS: &[&str] = &[
    "idx", "activ", "name", "maxam", "timer", "reflec", "birth", "fadet", "windf", "floorc",
    "swarm", "timdie", "swarad", "swarmtim", "heisen", "refint",
];
const GFX_COLUMNS: &[&str] = &[
    "idx", "activ", "name", "lolig", "obdef", "expde", "objpa", "objpv", "objpp",
];
const GEOMETRY_COLUMNS: &[&str] = &[
    "idx", "activ", "name", "exsiz", "ezsiz", "heigh", "exadd", "eyadd", "ezadd", "exvar", "eyvar",
    "ezvar", "ysub", "xadd", "yadd", "zadd",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeometryDefinition {
    pub id: i32,
    pub name: String,
    pub numeric_raw: [i32; 14],
}
impl GeometryDefinition {
    pub fn active_raw(&self) -> i32 {
        self.numeric_raw[0]
    }
}
pub fn geometry_definitions(bytes: &[u8]) -> Result<BTreeMap<i32, GeometryDefinition>, String> {
    Ok(
        parse_rows(bytes, "ParticleGeometrieDefault", GEOMETRY_COLUMNS)?
            .into_iter()
            .map(|(id, (name, numeric))| {
                (
                    id,
                    GeometryDefinition {
                        id,
                        name,
                        numeric_raw: numeric.try_into().expect("validated geometry columns"),
                    },
                )
            })
            .collect(),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectParticleDefinition {
    pub id: i32,
    pub name: String,
    /// activ, alen, alrid, papar, pageo, pagfx, before native byte/word narrowing.
    pub numeric_raw: [i32; 6],
    /// Explicit disps column7 only; unknown initialization is not guessed as0.
    pub mode_raw: Option<i32>,
}
impl ObjectParticleDefinition {
    pub fn mode_native(&self) -> Option<i16> {
        self.mode_raw.map(|mode| mode as i16)
    }
    pub fn active_native(&self) -> u8 {
        self.numeric_raw[0] as u8
    }
    pub fn animation_length(&self) -> i32 {
        self.numeric_raw[1]
    }
    pub fn alr_id(&self) -> i16 {
        self.numeric_raw[2] as i16
    }
    pub fn parameter_id(&self) -> i16 {
        self.numeric_raw[3] as i16
    }
    pub fn geometry_id(&self) -> i16 {
        self.numeric_raw[4] as i16
    }
    pub fn grafik_id(&self) -> i16 {
        self.numeric_raw[5] as i16
    }
}

/// Strict full base rows, interpreting only the six source-bound numeric fields and name.
/// Unselected columns may contain floats/strings; this does not validate or rebuild them.
pub fn object_particle_definitions(
    input: &[u8],
) -> Result<BTreeMap<i32, ObjectParticleDefinition>, String> {
    let decoded;
    let bytes = if lzss::has_pfil_magic(input) {
        decoded = lzss::decompress_pfil(input, None).map_err(|e| e.to_string())?;
        decoded.as_slice()
    } else {
        input
    };
    if !bytes.is_ascii() {
        return Err("object definition encoding is not ASCII".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let required = [
        (0, "idx"),
        (1, "activ"),
        (2, "alen"),
        (5, "alrid"),
        (52, "name"),
        (63, "papar"),
        (64, "pageo"),
        (65, "pagfx"),
    ];
    let mut inside = false;
    let mut sections = 0;
    let mut header = false;
    let mut mode_header = false;
    let mut result = BTreeMap::new();
    for raw in text.lines() {
        let row = raw.trim();
        if row.starts_with('[') {
            inside = row == "[ObjectDefaults]";
            if inside {
                sections += 1;
                if sections > 1 {
                    return Err("duplicate object section".into());
                }
            }
            continue;
        }
        if !inside || row.is_empty() {
            continue;
        }
        if let Some(comment) = row.strip_prefix(';') {
            let names: Vec<_> = comment
                .split(',')
                .map(|s| s.trim().trim_matches('-'))
                .collect();
            if names.first() == Some(&"idx") {
                if header || names.len() != 209 || required.iter().any(|(i, n)| names[*i] != *n) {
                    return Err("object column contract mismatch".into());
                }
                header = true;
                mode_header = names[7] == "disps";
            }
            continue;
        }
        if !header {
            return Err("object row before column header".into());
        }
        let values: Vec<_> = row.split(',').map(str::trim).collect();
        if values.len() != 209 {
            return Err("object column count".into());
        }
        let integer = |i: usize| {
            values[i]
                .parse::<i32>()
                .map_err(|_| format!("invalid object signed32 column {i}"))
        };
        let id = integer(0)?;
        if !(0..2500).contains(&id) {
            return Err("object ID outside 0..2499".into());
        }
        if values[52].is_empty() {
            return Err("empty object name".into());
        }
        let def = ObjectParticleDefinition {
            id,
            name: values[52].into(),
            mode_raw: if mode_header { Some(integer(7)?) } else { None },
            numeric_raw: [
                integer(1)?,
                integer(2)?,
                integer(5)?,
                integer(63)?,
                integer(64)?,
                integer(65)?,
            ],
        };
        if result.insert(id, def).is_some() {
            return Err("duplicate object ID".into());
        }
    }
    if sections != 1 || !header {
        return Err("missing object section/header".into());
    }
    Ok(result)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefinitionChainError {
    MissingOwner,
    InactiveOwner,
    MissingParameter(i16),
    InactiveParameter,
    MissingGeometry(i16),
    InactiveGeometry,
    MissingGrafik(i16),
    InactiveGrafik,
    NoPrimaryGrafikObject,
    MissingGrafikObject(i32),
    InactiveGrafikObject,
}
pub struct DefinitionChain<'a> {
    pub owner: &'a ObjectParticleDefinition,
    pub parameter: &'a ParameterDefinition,
    pub geometry: &'a GeometryDefinition,
    pub grafik: &'a GrafikDefinition,
    pub grafik_object: &'a ObjectParticleDefinition,
}
impl DefinitionChain<'_> {
    /// Caller supplies resident metadata for this grafik_object.alr_id(), never an owner fallback.
    pub fn frame_context(
        &self,
        columns: i32,
        directions: i32,
        frame_count: u32,
    ) -> Result<crate::particle_frames::FrameContext, &'static str> {
        self.parameter.frame_context(
            self.grafik_object.animation_length(),
            columns,
            directions,
            frame_count,
        )
    }
}
/// Base-table identity resolution and product active guards, not full native draw/visibility gates.
pub fn definition_chain<'a>(
    owner_type: i32,
    objects: &'a BTreeMap<i32, ObjectParticleDefinition>,
    parameters: &'a BTreeMap<i32, ParameterDefinition>,
    geometry: &'a BTreeMap<i32, GeometryDefinition>,
    grafiks: &'a BTreeMap<i32, GrafikDefinition>,
) -> Result<DefinitionChain<'a>, DefinitionChainError> {
    use DefinitionChainError::*;
    let owner = objects.get(&owner_type).ok_or(MissingOwner)?;
    if owner.active_native() == 0 {
        return Err(InactiveOwner);
    }
    let parameter = parameters
        .get(&(owner.parameter_id() as i32))
        .ok_or(MissingParameter(owner.parameter_id()))?;
    if parameter.active_raw() == 0 {
        return Err(InactiveParameter);
    }
    let geometry = geometry
        .get(&(owner.geometry_id() as i32))
        .ok_or(MissingGeometry(owner.geometry_id()))?;
    if geometry.active_raw() == 0 {
        return Err(InactiveGeometry);
    }
    let grafik = grafiks
        .get(&(owner.grafik_id() as i32))
        .ok_or(MissingGrafik(owner.grafik_id()))?;
    if grafik.active_raw() == 0 {
        return Err(InactiveGrafik);
    }
    if grafik.object_definition() == -1 {
        return Err(NoPrimaryGrafikObject);
    }
    let grafik_object = objects
        .get(&grafik.object_definition())
        .ok_or(MissingGrafikObject(grafik.object_definition()))?;
    if grafik_object.active_native() == 0 {
        return Err(InactiveGrafikObject);
    }
    Ok(DefinitionChain {
        owner,
        parameter,
        geometry,
        grafik,
        grafik_object,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterDefinition {
    pub id: i32,
    pub name: String,
    /// Runtime +0..+34 dwords: active then columns3..15. Names occupy +38 separately.
    pub numeric_raw: [i32; 14],
}
impl ParameterDefinition {
    pub fn active_raw(&self) -> i32 {
        self.numeric_raw[0]
    }
    pub fn lifetime(&self) -> i32 {
        self.numeric_raw[2]
    }
    pub fn skip_first_raw(&self) -> i32 {
        self.numeric_raw[8]
    }
    pub fn velocity_branch_raw(&self) -> i32 {
        self.numeric_raw[12]
    }
    /// Build the existing frame resolver's context from this exact active parameter row.
    /// Divisor and resident asset observations still come from the caller's resolved gfx object.
    pub fn frame_context(
        &self,
        type_divisor: i32,
        columns: i32,
        directions: i32,
        frame_count: u32,
    ) -> Result<crate::particle_frames::FrameContext, &'static str> {
        if self.active_raw() == 0 {
            return Err("inactive particle parameter definition");
        }
        Ok(crate::particle_frames::FrameContext {
            definition_lifetime: self.lifetime(),
            type_divisor,
            velocity_branch_raw: self.velocity_branch_raw(),
            skip_first_raw: self.skip_first_raw(),
            columns,
            directions,
            frame_count,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrafikDefinition {
    pub id: i32,
    pub name: String,
    /// Runtime +0..+18 dwords: active then columns3..8. Names occupy +1C separately.
    pub numeric_raw: [i32; 7],
}
impl GrafikDefinition {
    pub fn active_raw(&self) -> i32 {
        self.numeric_raw[0]
    }
    pub fn object_definition(&self) -> i32 {
        self.numeric_raw[2]
    }
}

pub fn parameter_definitions(bytes: &[u8]) -> Result<BTreeMap<i32, ParameterDefinition>, String> {
    let rows = parse_rows(bytes, "ParticleParameterDefault", PARAMETER_COLUMNS)?;
    Ok(rows
        .into_iter()
        .map(|(id, (name, numeric))| {
            (
                id,
                ParameterDefinition {
                    id,
                    name,
                    numeric_raw: numeric.try_into().expect("validated parameter columns"),
                },
            )
        })
        .collect())
}
pub fn grafik_definitions(bytes: &[u8]) -> Result<BTreeMap<i32, GrafikDefinition>, String> {
    let rows = parse_rows(bytes, "ParticleGrafikDefault", GFX_COLUMNS)?;
    Ok(rows
        .into_iter()
        .map(|(id, (name, numeric))| {
            (
                id,
                GrafikDefinition {
                    id,
                    name,
                    numeric_raw: numeric.try_into().expect("validated gfx columns"),
                },
            )
        })
        .collect())
}

type Rows = BTreeMap<i32, (String, Vec<i32>)>;
fn parse_rows(input: &[u8], section: &str, columns: &[&str]) -> Result<Rows, String> {
    let decoded;
    let bytes = if lzss::has_pfil_magic(input) {
        decoded = lzss::decompress_pfil(input, None).map_err(|e| e.to_string())?;
        decoded.as_slice()
    } else {
        input
    };
    if !bytes.is_ascii() {
        return Err("definition encoding is not ASCII".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let mut inside = false;
    let mut sections = 0;
    let mut header = false;
    let mut rows = Rows::new();
    for (line, raw) in text.lines().enumerate() {
        let row = raw.trim();
        if row.starts_with('[') {
            inside = row == format!("[{section}]");
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
        if let Some(comment) = row.strip_prefix(';') {
            let names: Vec<_> = comment
                .split(',')
                .map(|s| s.trim().trim_matches('-'))
                .collect();
            if names.first() == Some(&"idx") {
                if header || names != columns {
                    return Err("definition column contract mismatch".into());
                }
                header = true;
            }
            continue;
        }
        if !header {
            return Err("definition row before column header".into());
        }
        let values: Vec<_> = row.split(',').map(str::trim).collect();
        if values.len() != columns.len() {
            return Err(format!("definition column count at line {}", line + 1));
        }
        let integer = |i: usize| {
            values[i]
                .parse::<i32>()
                .map_err(|_| format!("invalid signed32 column {i} at line {}", line + 1))
        };
        let id = integer(0)?;
        if !(0..512).contains(&id) {
            return Err("definition ID outside 0..511".into());
        }
        if values[2].is_empty() || values[2].bytes().any(|b| b.is_ascii_whitespace() || b == 0) {
            return Err("definition name must be one nonempty ASCII token".into());
        }
        let mut numeric = vec![integer(1)?];
        for i in 3..values.len() {
            numeric.push(integer(i)?);
        }
        if rows.insert(id, (values[2].into(), numeric)).is_some() {
            return Err("duplicate definition ID".into());
        }
    }
    if sections != 1 || !header {
        return Err("missing definition section/header".into());
    }
    Ok(rows)
}
