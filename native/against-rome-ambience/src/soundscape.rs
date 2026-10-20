//! Editor-only soundscape prototype. SNDZ is not a native game sound format.
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum SoundscapeError {
    InsufficientHeaderLength {
        expected: usize,
        actual: usize,
    },
    InvalidMagic {
        actual: [u8; 4],
    },
    UnsupportedVersion {
        version: u16,
        expected: u16,
    },
    TruncatedData {
        reason: String,
    },
    InvalidShapeType(u8),
    InvalidAttenuationCurve(u8),
    InvalidCategory(u8),
    InvalidTriggerMode(u8),
    InvalidFloat {
        field: &'static str,
        value: f32,
    },
    InvalidAttribute(String),
    MalformedPolygon {
        reason: String,
    },
    CompressionError(String),
    InvalidArgument(String),
    NotFound(String),
    NotSupported(String),
    Serialization(String),
    Deserialization(String),
}

impl std::fmt::Display for SoundscapeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InsufficientHeaderLength { expected, actual } => {
                write!(
                    f,
                    "音效資料長度不足 64 位元組標頭。 (預期至少 {expected} 位元組，實際僅有 {actual} 位元組)"
                )
            }
            Self::InvalidMagic { actual } => {
                write!(
                    f,
                    "非有效的 SNDZ 音效二進位檔案魔數。 (預期 'SNDZ'，實際為 {:?})",
                    String::from_utf8_lossy(actual)
                )
            }
            Self::UnsupportedVersion { version, expected } => {
                write!(f, "不支援的 SNDZ 版本: {version} (當前預期: {expected})")
            }
            Self::TruncatedData { reason } => write!(f, "SNDZ 資料截斷: {reason}"),
            Self::InvalidShapeType(v) => write!(f, "無效的形狀類型 (SoundscapeShapeType): {v}"),
            Self::InvalidAttenuationCurve(v) => {
                write!(f, "無效的衰減曲線類型 (AttenuationCurveKind): {v}")
            }
            Self::InvalidCategory(v) => write!(f, "無效的音效類別 (AudioCategory): {v}"),
            Self::InvalidTriggerMode(v) => write!(f, "無效的觸發模式 (SoundTriggerMode): {v}"),
            Self::InvalidFloat { field, value } => {
                write!(f, "非法的浮點數數值 ({value}) 於欄位 '{field}'")
            }
            Self::InvalidAttribute(s) => write!(f, "無效的區域屬性旗標: {s}"),
            Self::MalformedPolygon { reason } => write!(f, "多邊形資料損壞: {reason}"),
            Self::CompressionError(e) => write!(f, "PFIL 壓縮/解壓縮失敗: {e}"),
            Self::InvalidArgument(msg) => write!(f, "{msg}"),
            Self::NotFound(msg) => write!(f, "{msg}"),
            Self::NotSupported(msg) => write!(f, "{msg}"),
            Self::Serialization(msg) => write!(f, "序列化失敗: {msg}"),
            Self::Deserialization(msg) => write!(f, "反序列化失敗: {msg}"),
        }
    }
}

impl std::error::Error for SoundscapeError {}

impl From<String> for SoundscapeError {
    fn from(s: String) -> Self {
        SoundscapeError::Deserialization(s)
    }
}

impl From<&str> for SoundscapeError {
    fn from(s: &str) -> Self {
        SoundscapeError::Deserialization(s.to_string())
    }
}

pub type Result<T> = std::result::Result<T, SoundscapeError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum SoundscapeShapeType {
    Point = 0,
    Circle = 1,
    OrientedRectangle = 2,
    ConvexPolygon = 3,
}

impl Default for SoundscapeShapeType {
    fn default() -> Self {
        Self::Circle
    }
}

impl SoundscapeShapeType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Point => "Point",
            Self::Circle => "Circle",
            Self::OrientedRectangle => "OrientedRectangle",
            Self::ConvexPolygon => "ConvexPolygon",
        }
    }
}

impl TryFrom<u8> for SoundscapeShapeType {
    type Error = SoundscapeError;
    fn try_from(v: u8) -> std::result::Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Point),
            1 => Ok(Self::Circle),
            2 => Ok(Self::OrientedRectangle),
            3 => Ok(Self::ConvexPolygon),
            _ => Err(SoundscapeError::InvalidShapeType(v)),
        }
    }
}

impl std::fmt::Display for SoundscapeShapeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum AttenuationCurveKind {
    Linear = 0,
    SmoothStep = 1,
    Logarithmic = 2,
    Exponential = 3,
}

impl Default for AttenuationCurveKind {
    fn default() -> Self {
        Self::SmoothStep
    }
}

impl AttenuationCurveKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::SmoothStep => "SmoothStep",
            Self::Logarithmic => "Logarithmic",
            Self::Exponential => "Exponential",
        }
    }
}

impl TryFrom<u8> for AttenuationCurveKind {
    type Error = SoundscapeError;
    fn try_from(v: u8) -> std::result::Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Linear),
            1 => Ok(Self::SmoothStep),
            2 => Ok(Self::Logarithmic),
            3 => Ok(Self::Exponential),
            _ => Err(SoundscapeError::InvalidAttenuationCurve(v)),
        }
    }
}

impl std::fmt::Display for AttenuationCurveKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum AudioCategory {
    AmbientGlobal = 0,
    AmbientZone = 1,
    PointEmitter = 2,
    Hydrology = 3,
    Vegetation = 4,
    Settlement = 5,
    WeatherFx = 6,
}

impl Default for AudioCategory {
    fn default() -> Self {
        Self::AmbientZone
    }
}

impl AudioCategory {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::AmbientGlobal => "AmbientGlobal",
            Self::AmbientZone => "AmbientZone",
            Self::PointEmitter => "PointEmitter",
            Self::Hydrology => "Hydrology",
            Self::Vegetation => "Vegetation",
            Self::Settlement => "Settlement",
            Self::WeatherFx => "WeatherFx",
        }
    }
}

impl TryFrom<u8> for AudioCategory {
    type Error = SoundscapeError;
    fn try_from(v: u8) -> std::result::Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::AmbientGlobal),
            1 => Ok(Self::AmbientZone),
            2 => Ok(Self::PointEmitter),
            3 => Ok(Self::Hydrology),
            4 => Ok(Self::Vegetation),
            5 => Ok(Self::Settlement),
            6 => Ok(Self::WeatherFx),
            _ => Err(SoundscapeError::InvalidCategory(v)),
        }
    }
}

impl std::fmt::Display for AudioCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum SoundTriggerMode {
    ContinuousLoop = 0,
    RandomStochasticInterval = 1,
    TimeOfDayConditional = 2,
}

impl Default for SoundTriggerMode {
    fn default() -> Self {
        Self::ContinuousLoop
    }
}

impl SoundTriggerMode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ContinuousLoop => "ContinuousLoop",
            Self::RandomStochasticInterval => "RandomStochasticInterval",
            Self::TimeOfDayConditional => "TimeOfDayConditional",
        }
    }
}

impl TryFrom<u8> for SoundTriggerMode {
    type Error = SoundscapeError;
    fn try_from(v: u8) -> std::result::Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::ContinuousLoop),
            1 => Ok(Self::RandomStochasticInterval),
            2 => Ok(Self::TimeOfDayConditional),
            _ => Err(SoundscapeError::InvalidTriggerMode(v)),
        }
    }
}

impl std::fmt::Display for SoundTriggerMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SoundscapeZoneAttributes;

impl SoundscapeZoneAttributes {
    pub const NONE: u32 = 0;
    pub const IS_LOOPING: u32 = 1 << 0;
    pub const IS_ACTIVE: u32 = 1 << 1;
    pub const IS_AUTO_GENERATED: u32 = 1 << 2;
    pub const OCCLUSION_ENABLED: u32 = 1 << 3;
    pub const DAY_ONLY: u32 = 1 << 4;
    pub const NIGHT_ONLY: u32 = 1 << 5;

    pub const fn has_flag(attributes: u32, flag: u32) -> bool {
        (attributes & flag) == flag
    }
}
pub type SoundscapeZoneFlags = SoundscapeZoneAttributes;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SoundscapeVertex {
    pub x: f32,
    pub z: f32,
}

impl SoundscapeVertex {
    pub const fn new(x: f32, z: f32) -> Self {
        Self { x, z }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SoundDef {
    pub sound_id: String,
    pub display_name: String,
    pub category: AudioCategory,
    pub trigger_mode: SoundTriggerMode,
    pub default_volume: f32,
    pub default_inner_radius: f32,
    pub default_outer_radius: f32,
    pub min_interval_seconds: f32,
    pub max_interval_seconds: f32,
    pub day_only: bool,
    pub night_only: bool,
}

impl SoundDef {
    pub fn new(id: &str, name: &str, category: AudioCategory, trigger_mode: SoundTriggerMode) -> Self {
        Self {
            sound_id: id.into(),
            display_name: name.into(),
            category,
            trigger_mode,
            default_volume: 1.0,
            default_inner_radius: 200.0,
            default_outer_radius: 800.0,
            min_interval_seconds: 4.0,
            max_interval_seconds: 15.0,
            day_only: false,
            night_only: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct SoundscapeZone {
    pub id: Uuid,
    pub name: String,
    pub sound_def_id: String,
    pub shape_type: SoundscapeShapeType,
    pub attenuation_curve: AttenuationCurveKind,
    pub category: AudioCategory,
    pub base_volume: f32,
    pub priority: u16,
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub center_world_x: f32,
    pub center_world_y: f32,
    pub center_world_z: f32,
    pub param_a: f32,
    pub param_b: f32,
    pub param_angle_deg: f32,
    pub polygon_vertices: Vec<SoundscapeVertex>,
    pub attributes: u32,
}

impl Default for SoundscapeZone {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: "New Soundscape Zone".into(),
            sound_def_id: "Amb_Forest_Light".into(),
            shape_type: SoundscapeShapeType::Circle,
            attenuation_curve: AttenuationCurveKind::SmoothStep,
            category: AudioCategory::AmbientZone,
            base_volume: 1.0,
            priority: 128,
            inner_radius: 150.0,
            outer_radius: 600.0,
            center_world_x: 0.0,
            center_world_y: 0.0,
            center_world_z: 0.0,
            param_a: 200.0,
            param_b: 200.0,
            param_angle_deg: 0.0,
            polygon_vertices: vec![],
            attributes: SoundscapeZoneAttributes::IS_LOOPING | SoundscapeZoneAttributes::IS_ACTIVE,
        }
    }
}

impl SoundscapeZone {
    pub fn flags(&self) -> u32 {
        self.attributes
    }

    pub fn set_flags(&mut self, flags: u32) {
        self.attributes = flags;
    }

    pub fn clone_zone(&self) -> Self {
        self.clone()
    }

    pub fn validate(&self) -> Result<()> {
        let floats = [
            ("BaseVolume", self.base_volume),
            ("InnerRadius", self.inner_radius),
            ("OuterRadius", self.outer_radius),
            ("CenterWorldX", self.center_world_x),
            ("CenterWorldY", self.center_world_y),
            ("CenterWorldZ", self.center_world_z),
            ("ParamA", self.param_a),
            ("ParamB", self.param_b),
            ("ParamAngleDeg", self.param_angle_deg),
        ];
        for (field, val) in floats {
            if !val.is_finite() {
                return Err(SoundscapeError::InvalidFloat { field, value: val });
            }
        }
        for v in &self.polygon_vertices {
            if !v.x.is_finite() {
                return Err(SoundscapeError::InvalidFloat {
                    field: "PolygonVertex.X",
                    value: v.x,
                });
            }
            if !v.z.is_finite() {
                return Err(SoundscapeError::InvalidFloat {
                    field: "PolygonVertex.Z",
                    value: v.z,
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SoundscapeSnapshot {
    pub global_ambience_id: String,
    pub global_ambience_volume: f32,
    pub zones: Vec<SoundscapeZone>,
}

impl Default for SoundscapeSnapshot {
    fn default() -> Self {
        Self {
            global_ambience_id: "Amb_Plains_Wind".into(),
            global_ambience_volume: 0.6,
            zones: vec![],
        }
    }
}

impl SoundscapeSnapshot {
    pub fn new(
        global_ambience_id: impl Into<String>,
        global_ambience_volume: f32,
        zones: Vec<SoundscapeZone>,
    ) -> Self {
        Self {
            global_ambience_id: global_ambience_id.into(),
            global_ambience_volume,
            zones,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if !self.global_ambience_volume.is_finite() {
            return Err(SoundscapeError::InvalidFloat {
                field: "GlobalAmbienceVolume",
                value: self.global_ambience_volume,
            });
        }
        for z in &self.zones {
            z.validate()?;
        }
        Ok(())
    }
}

pub struct SoundscapeBinaryStorage;

impl SoundscapeBinaryStorage {
    pub const MAGIC: &'static [u8; 4] = b"SNDZ";
    pub const CURRENT_VERSION: u16 = 1;
    pub const HEADER_SIZE: usize = 64;
    pub const ZONE_RECORD_FIXED_SIZE: usize = 130;

    pub fn serialize_to_binary(s: &SoundscapeSnapshot) -> Result<Vec<u8>> {
        s.validate()?;
        if s.zones.len() > u16::MAX as usize {
            return Err(SoundscapeError::Serialization("too many SNDZ zones".into()));
        }
        let mut b = Vec::with_capacity(Self::HEADER_SIZE + s.zones.len() * Self::ZONE_RECORD_FIXED_SIZE);
        b.extend_from_slice(Self::MAGIC);
        b.extend_from_slice(&Self::CURRENT_VERSION.to_le_bytes());
        b.extend_from_slice(&(s.zones.len() as u16).to_le_bytes());
        b.extend_from_slice(&s.global_ambience_volume.to_le_bytes());
        fixed_string_write(&mut b, &s.global_ambience_id, 32);
        b.extend_from_slice(&[0u8; 20]);
        for z in &s.zones {
            let verts = if z.shape_type == SoundscapeShapeType::ConvexPolygon {
                z.polygon_vertices.as_slice()
            } else {
                &[]
            };
            if verts.len() > u16::MAX as usize {
                return Err(SoundscapeError::Serialization("too many SNDZ polygon vertices".into()));
            }
            b.extend_from_slice(&z.id.to_bytes_le());
            fixed_string_write(&mut b, &z.name, 32);
            fixed_string_write(&mut b, &z.sound_def_id, 32);
            b.push(z.shape_type as u8);
            b.push(z.attenuation_curve as u8);
            b.push(z.category as u8);
            b.push(0); // 1 byte reserved
            b.extend_from_slice(&z.attributes.to_le_bytes());
            b.extend_from_slice(&z.priority.to_le_bytes());
            b.extend_from_slice(&0u16.to_le_bytes()); // 2 bytes reserved
            for f in [
                z.base_volume,
                z.inner_radius,
                z.outer_radius,
                z.center_world_x,
                z.center_world_y,
                z.center_world_z,
                z.param_a,
                z.param_b,
                z.param_angle_deg,
            ] {
                b.extend_from_slice(&f.to_le_bytes());
            }
            b.extend_from_slice(&(verts.len() as u16).to_le_bytes());
            for v in verts {
                b.extend_from_slice(&v.x.to_le_bytes());
                b.extend_from_slice(&v.z.to_le_bytes());
            }
        }
        Ok(b)
    }

    pub fn deserialize_from_binary(data: &[u8]) -> Result<SoundscapeSnapshot> {
        if data.len() < Self::HEADER_SIZE && !data.starts_with(b"PFIL") {
            return Err(SoundscapeError::InsufficientHeaderLength {
                expected: Self::HEADER_SIZE,
                actual: data.len(),
            });
        }
        let raw;
        if data.starts_with(b"PFIL") {
            raw = against_rome_data::lzss::decompress_pfil(data, None)
                .map_err(|e| SoundscapeError::CompressionError(e.to_string()))?;
        } else {
            raw = data.to_vec();
        }
        if raw.len() < Self::HEADER_SIZE {
            return Err(SoundscapeError::InsufficientHeaderLength {
                expected: Self::HEADER_SIZE,
                actual: raw.len(),
            });
        }
        let mut r = Reader { bytes: &raw, pos: 0 };
        let magic = r.take(4)?;
        if magic != Self::MAGIC {
            return Err(SoundscapeError::InvalidMagic {
                actual: [magic[0], magic[1], magic[2], magic[3]],
            });
        }
        let version = r.u16()?;
        if version != Self::CURRENT_VERSION {
            return Err(SoundscapeError::UnsupportedVersion {
                version,
                expected: Self::CURRENT_VERSION,
            });
        }
        let zone_count = r.u16()? as usize;
        let global_vol = r.f32("GlobalAmbienceVolume")?;
        let global_amb_id = r.fixed_string(32)?;
        r.take(20)?; // 20 bytes padding

        let min_required = r.pos.saturating_add(zone_count.saturating_mul(Self::ZONE_RECORD_FIXED_SIZE));
        if raw.len() < min_required {
            return Err(SoundscapeError::TruncatedData {
                reason: format!(
                    "SNDZ 區域資料截斷：需要至少 {} 位元組以容納 {} 個區域記錄，實際僅有 {} 位元組",
                    min_required, zone_count, raw.len()
                ),
            });
        }

        let mut zones = Vec::with_capacity(zone_count);
        for i in 0..zone_count {
            let guid_bytes = r.take(16)?;
            let id = Uuid::from_bytes_le(guid_bytes.try_into().unwrap());
            let name = r.fixed_string(32)?;
            let sound_def_id = r.fixed_string(32)?;
            let shape_type = SoundscapeShapeType::try_from(r.byte()?)?;
            let attenuation_curve = AttenuationCurveKind::try_from(r.byte()?)?;
            let category = AudioCategory::try_from(r.byte()?)?;
            r.byte()?; // reserved
            let attributes = r.u32()?;
            let priority = r.u16()?;
            r.u16()?; // reserved
            let base_volume = r.f32("BaseVolume")?;
            let inner_radius = r.f32("InnerRadius")?;
            let outer_radius = r.f32("OuterRadius")?;
            let center_world_x = r.f32("CenterWorldX")?;
            let center_world_y = r.f32("CenterWorldY")?;
            let center_world_z = r.f32("CenterWorldZ")?;
            let param_a = r.f32("ParamA")?;
            let param_b = r.f32("ParamB")?;
            let param_angle_deg = r.f32("ParamAngleDeg")?;

            let vert_count = r.u16()? as usize;
            let vert_bytes = vert_count.checked_mul(8).ok_or_else(|| {
                SoundscapeError::TruncatedData {
                    reason: "頂點數量乘積溢出".into(),
                }
            })?;
            if r.remaining() < vert_bytes {
                return Err(SoundscapeError::TruncatedData {
                    reason: format!(
                        "區域 #{} ('{}') 的多邊形頂點截斷：預期 {} 個頂點 ({} 位元組)，剩餘僅有 {} 位元組",
                        i, name, vert_count, vert_bytes, r.remaining()
                    ),
                });
            }

            let mut polygon_vertices = Vec::with_capacity(vert_count);
            for _ in 0..vert_count {
                polygon_vertices.push(SoundscapeVertex::new(
                    r.f32("Vertex.X")?,
                    r.f32("Vertex.Z")?,
                ));
            }

            zones.push(SoundscapeZone {
                id,
                name,
                sound_def_id,
                shape_type,
                attenuation_curve,
                category,
                base_volume,
                priority,
                inner_radius,
                outer_radius,
                center_world_x,
                center_world_y,
                center_world_z,
                param_a,
                param_b,
                param_angle_deg,
                polygon_vertices,
                attributes,
            });
        }

        Ok(SoundscapeSnapshot {
            global_ambience_id: global_amb_id,
            global_ambience_volume: global_vol,
            zones,
        })
    }

    pub fn load_from_sound_dat(path: impl AsRef<std::path::Path>) -> Result<SoundscapeSnapshot> {
        let p = path.as_ref();
        if p.as_os_str().is_empty() || p.to_string_lossy().trim().is_empty() {
            return Err(SoundscapeError::InvalidArgument(
                "檔案路徑不得為空或僅包含空白 (ArgumentException)".into(),
            ));
        }
        if !p.exists() {
            return Err(SoundscapeError::NotFound(format!(
                "找不到檔案: {} (FileNotFoundException)",
                p.display()
            )));
        }
        let bytes = std::fs::read(p).map_err(|e| SoundscapeError::NotFound(e.to_string()))?;
        Self::deserialize_from_binary(&bytes)
    }

    pub fn save_to_sound_dat(
        path: impl AsRef<std::path::Path>,
        _snapshot: &SoundscapeSnapshot,
        _compress_pfil: bool,
        _orig_header: Option<&[u8]>,
    ) -> Result<()> {
        let p = path.as_ref();
        if p.as_os_str().is_empty() || p.to_string_lossy().trim().is_empty() {
            return Err(SoundscapeError::InvalidArgument(
                "檔案路徑不得為空或僅包含空白 (ArgumentException)".into(),
            ));
        }
        Err(SoundscapeError::NotSupported(
            "Native sound-zone export is unwired. SNDZ is an editor prototype, not DATA/sound.dat.".into(),
        ))
    }

    pub fn export_to_script(_snapshot: &SoundscapeSnapshot) -> Result<String> {
        Err(SoundscapeError::NotSupported(
            "Native sound-zone script export is unwired; no verified spatial playback contract exists.".into(),
        ))
    }

    pub fn export_to_json(snapshot: &SoundscapeSnapshot) -> Result<String> {
        snapshot.validate()?;
        let mut value = serde_json::to_value(snapshot)
            .map_err(|e| SoundscapeError::Serialization(e.to_string()))?;
        if let Some(zones) = value.get_mut("Zones").and_then(|v| v.as_array_mut()) {
            for zone in zones {
                if let Some(attrs) = zone.get("Attributes").and_then(|v| v.as_u64()) {
                    let flags = flags_string(attrs as u32);
                    zone["Attributes"] = flags.clone();
                    zone["Flags"] = flags;
                }
            }
        }
        serde_json::to_string_pretty(&value)
            .map_err(|e| SoundscapeError::Serialization(e.to_string()))
    }

    pub fn import_from_json(json: &str) -> Result<SoundscapeSnapshot> {
        if json.trim().is_empty() {
            return Err(SoundscapeError::InvalidArgument(
                "JSON 字串不得為空或僅包含空白 (ArgumentException)".into(),
            ));
        }
        let mut value: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| SoundscapeError::Deserialization(e.to_string()))?;
        if let Some(zones) = value.get_mut("Zones").and_then(|v| v.as_array_mut()) {
            for z in zones {
                for key in ["Attributes", "Flags"] {
                    if let Some(v) = z.get(key) {
                        let attrs = if let Some(s) = v.as_str() {
                            parse_flags(s)?
                        } else {
                            v.as_u64()
                                .and_then(|n| u32::try_from(n).ok())
                                .ok_or_else(|| {
                                    SoundscapeError::Deserialization(
                                        "zone attributes must be a UInt32".into(),
                                    )
                                })?
                        };
                        z["Attributes"] = attrs.into();
                    }
                }
            }
        }
        let snap: SoundscapeSnapshot = serde_json::from_value(value)
            .map_err(|e| SoundscapeError::Deserialization(e.to_string()))?;
        snap.validate()?;
        Ok(snap)
    }
}

fn flags_string(v: u32) -> serde_json::Value {
    if v & !63 != 0 {
        return v.into();
    }
    if v == 0 {
        return "None".into();
    }
    let names = [
        "IsLooping",
        "IsActive",
        "IsAutoGenerated",
        "OcclusionEnabled",
        "DayOnly",
        "NightOnly",
    ];
    names
        .iter()
        .enumerate()
        .filter(|(i, _)| v & (1 << i) != 0)
        .map(|(_, n)| *n)
        .collect::<Vec<_>>()
        .join(", ")
        .into()
}

fn parse_flags(s: &str) -> Result<u32> {
    let mut v = 0;
    for p in s.split(',').map(str::trim) {
        v |= match p {
            "None" => 0,
            "IsLooping" => SoundscapeZoneAttributes::IS_LOOPING,
            "IsActive" => SoundscapeZoneAttributes::IS_ACTIVE,
            "IsAutoGenerated" => SoundscapeZoneAttributes::IS_AUTO_GENERATED,
            "OcclusionEnabled" => SoundscapeZoneAttributes::OCCLUSION_ENABLED,
            "DayOnly" => SoundscapeZoneAttributes::DAY_ONLY,
            "NightOnly" => SoundscapeZoneAttributes::NIGHT_ONLY,
            _ => return Err(SoundscapeError::InvalidAttribute(p.to_string())),
        };
    }
    Ok(v)
}

fn fixed_string_write(b: &mut Vec<u8>, s: &str, fixed_len: usize) {
    let bytes = s.as_bytes();
    let to_copy = bytes.len().min(fixed_len);
    b.extend_from_slice(&bytes[..to_copy]);
    if to_copy < fixed_len {
        b.resize(b.len() + (fixed_len - to_copy), 0);
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or_else(|| SoundscapeError::TruncatedData {
            reason: "SNDZ offset overflow".into(),
        })?;
        if end > self.bytes.len() {
            return Err(SoundscapeError::TruncatedData {
                reason: format!("截斷的 SNDZ 記錄：需要 {} 位元組，剩餘僅有 {} 位元組", n, self.remaining()),
            });
        }
        let s = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16> {
        let slice = self.take(2)?;
        Ok(u16::from_le_bytes(slice.try_into().unwrap()))
    }

    fn u32(&mut self) -> Result<u32> {
        let slice = self.take(4)?;
        Ok(u32::from_le_bytes(slice.try_into().unwrap()))
    }

    fn f32(&mut self, field: &'static str) -> Result<f32> {
        let slice = self.take(4)?;
        let val = f32::from_le_bytes(slice.try_into().unwrap());
        if !val.is_finite() {
            return Err(SoundscapeError::InvalidFloat { field, value: val });
        }
        Ok(val)
    }

    fn fixed_string(&mut self, len: usize) -> Result<String> {
        let b = self.take(len)?;
        let n = b.iter().position(|&x| x == 0).unwrap_or(len);
        Ok(String::from_utf8_lossy(&b[..n]).into_owned())
    }
}

#[derive(Clone, Debug, Default)]
pub struct SoundscapeZoneCatalog {
    definitions: Vec<SoundDef>,
}

impl SoundscapeZoneCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, def: SoundDef) {
        if let Some(i) = self
            .definitions
            .iter()
            .position(|x| x.sound_id.eq_ignore_ascii_case(&def.sound_id))
        {
            self.definitions[i] = def;
        } else {
            self.definitions.push(def);
        }
    }

    pub fn resolve(&self, id: &str) -> SoundDef {
        self.definitions
            .iter()
            .find(|d| d.sound_id.eq_ignore_ascii_case(id))
            .cloned()
            .unwrap_or_else(|| {
                SoundDef::new(id, id, AudioCategory::AmbientZone, SoundTriggerMode::ContinuousLoop)
            })
    }

    pub fn get_all_definitions(&self) -> &[SoundDef] {
        &self.definitions
    }

    pub fn shared() -> Self {
        Self::create_default_catalog()
    }

    pub fn create_default_catalog() -> Self {
        use AudioCategory::*;
        use SoundTriggerMode::*;
        let mut c = Self::new();
        for (id, name, cat, mode, v, i, o, lo, hi, day, night) in [
            ("Amb_Forest_Dense_Day", "茂密森林（日間鳥鳴風動）", Vegetation, ContinuousLoop, 0.85, 300., 1000., 4., 15., true, false),
            ("Amb_Forest_Dense_Night", "茂密森林（夜間蟋蟀貓頭鷹）", Vegetation, ContinuousLoop, 0.75, 300., 1000., 4., 15., false, true),
            ("Amb_Forest_Light", "稀疏林地（微風枝葉作響）", Vegetation, ContinuousLoop, 0.65, 200., 750., 4., 15., false, false),
            ("Amb_Forest_Birds", "林冠飛鳥間歇鳴叫", Vegetation, RandomStochasticInterval, 0.9, 150., 600., 6., 22., false, false),
            ("Amb_River_Gentle", "平緩河流潺潺水聲", Hydrology, ContinuousLoop, 0.70, 160., 550., 4., 15., false, false),
            ("Amb_River_Rapid", "峽谷急流湍急咆哮", Hydrology, ContinuousLoop, 0.88, 220., 750., 4., 15., false, false),
            ("Amb_Waterfall_Roar", "斷崖瀑布萬鈞轟鳴", Hydrology, ContinuousLoop, 1.0, 350., 1200., 4., 15., false, false),
            ("Amb_Coast_Waves", "海岸邊際浪濤拍岸", Hydrology, ContinuousLoop, 0.82, 250., 900., 4., 15., false, false),
            ("Amb_Pond_Marsh", "靜水池塘水泡蛙鳴", Hydrology, ContinuousLoop, 0.60, 120., 450., 4., 15., false, false),
            ("Amb_Plains_Wind", "開闊荒野平原長風", AmbientGlobal, ContinuousLoop, 0.55, 500., 1800., 4., 15., false, false),
            ("Amb_Tundra_Blizzard", "極北雪原酷寒暴風", AmbientZone, ContinuousLoop, 0.90, 400., 1400., 4., 15., false, false),
            ("Amb_Mountain_Wind", "山嶽之巔孤寂寒風", AmbientZone, ContinuousLoop, 0.75, 300., 1100., 4., 15., false, false),
            ("Snd_Campfire_Crackling", "營火柴薪劈啪燃燒", PointEmitter, ContinuousLoop, 0.75, 80., 350., 4., 15., false, false),
            ("Snd_Blacksmith_Anvil", "鐵匠鋪鍛鐵打砧迴響", Settlement, RandomStochasticInterval, 0.85, 120., 500., 3., 9., false, false),
            ("Snd_Shrine_Chant", "祭壇遠古呢喃神諭", Settlement, ContinuousLoop, 0.65, 100., 400., 4., 15., false, false),
        ] {
            let mut d = SoundDef::new(id, name, cat, mode);
            d.default_volume = v;
            d.default_inner_radius = i;
            d.default_outer_radius = o;
            d.min_interval_seconds = lo;
            d.max_interval_seconds = hi;
            d.day_only = day;
            d.night_only = night;
            c.register(d);
        }
        c
    }

    pub fn calculate_attenuation(d: f32, i: f32, o: f32, c: AttenuationCurveKind) -> f32 {
        if d <= i {
            return 1.0;
        }
        if d >= o || o <= i {
            return 0.0;
        }
        let t = ((d - i) / (o - i)).clamp(0.0, 1.0);
        match c {
            AttenuationCurveKind::Linear => 1.0 - t,
            AttenuationCurveKind::SmoothStep => 1.0 - (3.0 * t * t - 2.0 * t * t * t),
            AttenuationCurveKind::Logarithmic => {
                (1.0 - ((1.0 + 9.0 * t as f64).ln() / 10f64.ln()) as f32).clamp(0.0, 1.0)
            }
            AttenuationCurveKind::Exponential => (1.0 - t) * (1.0 - t),
        }
    }

    pub fn distance_point_to_segment(px: f32, pz: f32, x1: f32, z1: f32, x2: f32, z2: f32) -> f32 {
        let l = (x2 - x1) * (x2 - x1) + (z2 - z1) * (z2 - z1);
        let t = if l <= 1e-6 {
            0.0
        } else {
            (((px - x1) * (x2 - x1) + (pz - z1) * (z2 - z1)) / l).clamp(0.0, 1.0)
        };
        distance(px, pz, x1 + t * (x2 - x1), z1 + t * (z2 - z1))
    }

    pub fn is_point_inside_polygon(px: f32, pz: f32, v: &[SoundscapeVertex]) -> bool {
        if v.len() < 3 {
            return false;
        }
        let mut inside = false;
        let mut j = v.len() - 1;
        for i in 0..v.len() {
            if (v[i].z > pz) != (v[j].z > pz)
                && px < (v[j].x - v[i].x) * (pz - v[i].z) / (v[j].z - v[i].z + 1e-7) + v[i].x
            {
                inside = !inside;
            }
            j = i;
        }
        inside
    }

    pub fn calculate_distance_to_zone(px: f32, pz: f32, z: &SoundscapeZone) -> f32 {
        match z.shape_type {
            SoundscapeShapeType::Point => distance(px, pz, z.center_world_x, z.center_world_z),
            SoundscapeShapeType::Circle => {
                (distance(px, pz, z.center_world_x, z.center_world_z) - z.param_a.max(0.0)).max(0.0)
            }
            SoundscapeShapeType::OrientedRectangle => {
                let r = -z.param_angle_deg * (std::f32::consts::PI / 180.0);
                let dx = px - z.center_world_x;
                let dz = pz - z.center_world_z;
                let x = ((dx * r.cos() - dz * r.sin()).abs() - z.param_a.max(0.0)).max(0.0);
                let y = ((dx * r.sin() + dz * r.cos()).abs() - z.param_b.max(0.0)).max(0.0);
                (x * x + y * y).sqrt()
            }
            SoundscapeShapeType::ConvexPolygon => {
                let v = &z.polygon_vertices;
                if v.len() < 3 {
                    return f32::MAX;
                }
                if Self::is_point_inside_polygon(px, pz, v) {
                    return 0.0;
                }
                (0..v.len())
                    .map(|i| {
                        let a = v[i];
                        let b = v[(i + 1) % v.len()];
                        Self::distance_point_to_segment(px, pz, a.x, a.z, b.x, b.z)
                    })
                    .fold(f32::MAX, f32::min)
            }
        }
    }
}

fn distance(x: f32, z: f32, cx: f32, cz: f32) -> f32 {
    ((x - cx) * (x - cx) + (z - cz) * (z - cz)).sqrt()
}

#[derive(Clone, Debug)]
pub struct SoundscapeZonePlanner {
    pub catalog: SoundscapeZoneCatalog,
}

impl Default for SoundscapeZonePlanner {
    fn default() -> Self {
        Self {
            catalog: SoundscapeZoneCatalog::shared(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct WaterCompanionContext {
    pub dimension: usize,
    pub tile_world_size: f32,
    pub heights: Option<Vec<u8>>,
    pub vertex_size: usize,
    pub height_map_step: f32,
    pub water_level: u8,
    pub river_tile_indices: Option<HashSet<usize>>,
}

impl SoundscapeZonePlanner {
    #[allow(clippy::too_many_arguments)]
    pub fn create_point_zone(
        &self,
        name: &str,
        id: &str,
        cx: f32,
        cy: f32,
        cz: f32,
        inner: f32,
        outer: f32,
        volume: f32,
        curve: AttenuationCurveKind,
    ) -> SoundscapeZone {
        let d = self.catalog.resolve(id);
        SoundscapeZone {
            name: name.into(),
            sound_def_id: id.into(),
            shape_type: SoundscapeShapeType::Point,
            category: d.category,
            center_world_x: cx,
            center_world_y: cy,
            center_world_z: cz,
            inner_radius: inner,
            outer_radius: outer,
            base_volume: volume,
            attenuation_curve: curve,
            attributes: SoundscapeZoneAttributes::IS_ACTIVE
                | if d.trigger_mode == SoundTriggerMode::ContinuousLoop {
                    SoundscapeZoneAttributes::IS_LOOPING
                } else {
                    0
                },
            ..Default::default()
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_circle_zone(
        &self,
        name: &str,
        id: &str,
        cx: f32,
        cy: f32,
        cz: f32,
        radius: f32,
        inner: f32,
        outer: f32,
        volume: f32,
        curve: AttenuationCurveKind,
    ) -> SoundscapeZone {
        let mut z = self.create_point_zone(name, id, cx, cy, cz, inner, outer, volume, curve);
        z.shape_type = SoundscapeShapeType::Circle;
        z.param_a = radius.max(10.0);
        z
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_rectangle_zone(
        &self,
        name: &str,
        id: &str,
        cx: f32,
        cy: f32,
        cz: f32,
        half_w: f32,
        half_h: f32,
        angle: f32,
        inner: f32,
        outer: f32,
        volume: f32,
        curve: AttenuationCurveKind,
    ) -> SoundscapeZone {
        let mut z = self.create_point_zone(name, id, cx, cy, cz, inner, outer, volume, curve);
        z.shape_type = SoundscapeShapeType::OrientedRectangle;
        z.param_a = half_w.max(10.0);
        z.param_b = half_h.max(10.0);
        z.param_angle_deg = angle;
        z
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_convex_polygon_zone(
        &self,
        name: &str,
        id: &str,
        points: &[SoundscapeVertex],
        cy: f32,
        inner: f32,
        outer: f32,
        volume: f32,
        curve: AttenuationCurveKind,
    ) -> Result<SoundscapeZone> {
        let hull = Self::compute_convex_hull(points);
        if hull.len() < 3 {
            return Err("凸多邊形至少需要 3 個非共線點。".into());
        }
        let cx = hull.iter().map(|v| v.x).sum::<f32>() / hull.len() as f32;
        let cz = hull.iter().map(|v| v.z).sum::<f32>() / hull.len() as f32;
        let mut z = self.create_point_zone(name, id, cx, cy, cz, inner, outer, volume, curve);
        z.shape_type = SoundscapeShapeType::ConvexPolygon;
        z.polygon_vertices = hull;
        Ok(z)
    }

    pub fn compute_convex_hull(points: &[SoundscapeVertex]) -> Vec<SoundscapeVertex> {
        let mut p = points.to_vec();
        p.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.z.total_cmp(&b.z)));
        p.dedup();
        if p.len() <= 2 {
            return p;
        }
        let cross = |a: SoundscapeVertex, b: SoundscapeVertex, c: SoundscapeVertex| {
            (b.x - a.x) * (c.z - a.z) - (b.z - a.z) * (c.x - a.x)
        };
        let mut l = vec![];
        for &v in &p {
            while l.len() >= 2 && cross(l[l.len() - 2], l[l.len() - 1], v) <= 0.0 {
                l.pop();
            }
            l.push(v);
        }
        let mut u = vec![];
        for &v in p.iter().rev() {
            while u.len() >= 2 && cross(u[u.len() - 2], u[u.len() - 1], v) <= 0.0 {
                u.pop();
            }
            u.push(v);
        }
        l.pop();
        u.pop();
        l.extend(u);
        l
    }

    pub fn generate_hydrology_soundscapes(
        &self,
        c: &WaterCompanionContext,
    ) -> Result<Vec<SoundscapeZone>> {
        let n = c.dimension;
        let count = n
            .checked_mul(n)
            .filter(|&x| x <= 16_777_216)
            .ok_or("invalid hydrology dimension")?;
        if let Some(h) = &c.heights {
            if c.vertex_size < n + 1
                || h.len()
                    < c.vertex_size
                        .checked_mul(c.vertex_size)
                        .ok_or("height dimensions overflow")?
            {
                return Err("height grid does not cover hydrology tiles".into());
            }
        }
        let mut water = vec![false; count];
        for y in 0..n {
            for x in 0..n {
                let i = y * n + x;
                water[i] = c
                    .river_tile_indices
                    .as_ref()
                    .is_some_and(|s| s.contains(&i))
                    || c.heights.as_ref().is_some_and(|h| {
                        let v = c.vertex_size;
                        (h[y * v + x] as f32
                            + h[y * v + x + 1] as f32
                            + h[(y + 1) * v + x] as f32
                            + h[(y + 1) * v + x + 1] as f32)
                            * 0.25
                            < c.water_level as f32
                    });
            }
        }
        let neighbors = |x: usize, y: usize| -> Vec<(usize, usize)> {
            [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .filter_map(|&(dx, dy)| {
                    let a = x as isize + dx;
                    let b = y as isize + dy;
                    (a >= 0 && b >= 0 && a < n as isize && b < n as isize)
                        .then_some((a as usize, b as usize))
                })
                .collect()
        };
        let mut visited = vec![false; count];
        let mut out = vec![];
        let mut counter = 1;
        let size = c.tile_world_size;
        for y in 0..n {
            for x in 0..n {
                if !water[y * n + x] || visited[y * n + x] {
                    continue;
                }
                let mut queue = VecDeque::from([(x, y)]);
                visited[y * n + x] = true;
                let mut tiles = vec![];
                let (mut minx, mut maxx, mut miny, mut maxy) = (x, x, y, y);
                let mut border = false;
                while let Some((cx, cy)) = queue.pop_front() {
                    tiles.push((cx, cy));
                    minx = minx.min(cx);
                    maxx = maxx.max(cx);
                    miny = miny.min(cy);
                    maxy = maxy.max(cy);
                    border |= cx == 0 || cy == 0 || cx == n - 1 || cy == n - 1;
                    for (a, b) in neighbors(cx, cy) {
                        if water[b * n + a] && !visited[b * n + a] {
                            visited[b * n + a] = true;
                            queue.push_back((a, b));
                        }
                    }
                }
                if tiles.len() < 20 {
                    let mut z = self.create_circle_zone(
                        &format!("水潭蛙鳴 #{counter}"),
                        "Amb_Pond_Marsh",
                        (minx + maxx) as f32 * 0.5 * size,
                        0.0,
                        (miny + maxy) as f32 * 0.5 * size,
                        ((maxx - minx + 1).max(maxy - miny + 1)) as f32 * size * 0.5,
                        100.0,
                        400.0,
                        0.65,
                        AttenuationCurveKind::SmoothStep,
                    );
                    counter += 1;
                    z.attributes |= SoundscapeZoneAttributes::IS_AUTO_GENERATED;
                    out.push(z);
                } else if border && tiles.len() > 800 {
                    let coast: Vec<_> = tiles
                        .iter()
                        .copied()
                        .filter(|&(tx, ty)| neighbors(tx, ty).iter().any(|&(a, b)| !water[b * n + a]))
                        .collect();
                    for &(tx, ty) in coast.iter().step_by((coast.len() / 12).max(1)) {
                        let mut z = self.create_circle_zone(
                            &format!("海岸潮聲 #{counter}"),
                            "Amb_Coast_Waves",
                            tx as f32 * size,
                            0.0,
                            ty as f32 * size,
                            size * 3.0,
                            200.0,
                            800.0,
                            0.8,
                            AttenuationCurveKind::SmoothStep,
                        );
                        counter += 1;
                        z.attributes |= SoundscapeZoneAttributes::IS_AUTO_GENERATED;
                        out.push(z);
                    }
                } else {
                    if let Some(h) = &c.heights {
                        let mut found = false;
                        for &(tx, ty) in &tiles {
                            let center = h[ty * c.vertex_size + tx] as f32;
                            for (a, b) in neighbors(tx, ty) {
                                if water[b * n + a]
                                    && (center - h[b * c.vertex_size + a] as f32) * c.height_map_step > 16.0
                                {
                                    let mut z = self.create_point_zone(
                                        &format!("萬鈞瀑布 #{counter}"),
                                        "Amb_Waterfall_Roar",
                                        tx as f32 * size,
                                        center * c.height_map_step,
                                        ty as f32 * size,
                                        350.0,
                                        1200.0,
                                        1.0,
                                        AttenuationCurveKind::SmoothStep,
                                    );
                                    counter += 1;
                                    z.attributes |= SoundscapeZoneAttributes::IS_AUTO_GENERATED;
                                    out.push(z);
                                    found = true;
                                    break;
                                }
                            }
                            if found {
                                break;
                            }
                        }
                    }
                    for &(tx, ty) in tiles.iter().step_by((tiles.len() / 8).max(1)) {
                        let mut z = self.create_circle_zone(
                            &format!("流水潺潺 #{counter}"),
                            "Amb_River_Gentle",
                            tx as f32 * size,
                            0.0,
                            ty as f32 * size,
                            size * 2.0,
                            150.0,
                            550.0,
                            0.7,
                            AttenuationCurveKind::SmoothStep,
                        );
                        counter += 1;
                        z.attributes |= SoundscapeZoneAttributes::IS_AUTO_GENERATED;
                        out.push(z);
                    }
                }
            }
        }
        Ok(out)
    }

    pub fn generate_forest_soundscapes(
        &self,
        positions: &[(f32, f32)],
        bin_size: f32,
        dense_threshold: usize,
    ) -> Result<Vec<SoundscapeZone>> {
        if positions.len() < 3 {
            return Ok(vec![]);
        }
        if !bin_size.is_finite() || bin_size <= 0.0 {
            return Err("invalid forest bin size".into());
        }
        let mut bins: Vec<((i32, i32), Vec<SoundscapeVertex>)> = vec![];
        for &(x, z) in positions {
            let key = ((x / bin_size).floor() as i32, (z / bin_size).floor() as i32);
            let v = SoundscapeVertex { x, z };
            if let Some((_, points)) = bins.iter_mut().find(|(k, _)| *k == key) {
                points.push(v);
            } else {
                bins.push((key, vec![v]));
            }
        }
        let mut counter = 1;
        let mut out = vec![];
        for (_, trees) in bins {
            if trees.len() >= dense_threshold {
                let hull = Self::compute_convex_hull(&trees);
                if hull.len() >= 3 {
                    let mut forest = self.create_convex_polygon_zone(
                        &format!("茂密林區 #{counter}"),
                        "Amb_Forest_Dense_Day",
                        &hull,
                        0.0,
                        250.0,
                        850.0,
                        0.85,
                        AttenuationCurveKind::SmoothStep,
                    )?;
                    counter += 1;
                    forest.attributes |= SoundscapeZoneAttributes::IS_AUTO_GENERATED;
                    let mut birds = self.create_point_zone(
                        &format!("林冠飛鳥鳴啼 #{counter}"),
                        "Amb_Forest_Birds",
                        forest.center_world_x,
                        0.0,
                        forest.center_world_z,
                        120.0,
                        600.0,
                        0.9,
                        AttenuationCurveKind::SmoothStep,
                    );
                    birds.attributes |= SoundscapeZoneAttributes::IS_AUTO_GENERATED;
                    out.push(forest);
                    out.push(birds);
                }
            } else if trees.len() >= 3 {
                let x = (trees.iter().map(|v| v.x as f64).sum::<f64>() / trees.len() as f64) as f32;
                let z = (trees.iter().map(|v| v.z as f64).sum::<f64>() / trees.len() as f64) as f32;
                let mut zone = self.create_circle_zone(
                    &format!("疏林伴隨音 #{counter}"),
                    "Amb_Forest_Light",
                    x,
                    0.0,
                    z,
                    bin_size * 0.4,
                    180.0,
                    650.0,
                    0.65,
                    AttenuationCurveKind::SmoothStep,
                );
                counter += 1;
                zone.attributes |= SoundscapeZoneAttributes::IS_AUTO_GENERATED;
                out.push(zone);
            }
        }
        Ok(out)
    }

    pub fn generate_settlement_soundscapes(
        &self,
        buildings: &[(String, f32, f32, f32)],
    ) -> Vec<SoundscapeZone> {
        let mut out = vec![];
        let mut counter = 1;
        for (name, x, y, z) in buildings {
            let t = name.to_lowercase();
            let spec = if t.contains("schmied") || t.contains("blacksmith") {
                Some(("鐵砧鍛打聲", "Snd_Blacksmith_Anvil", 100.0, 450.0, 0.85))
            } else if t.contains("tempel") || t.contains("shrine") || t.contains("heiligtum") {
                Some(("神龕祈禱低語", "Snd_Shrine_Chant", 120.0, 400.0, 0.70))
            } else if t.contains("feuer") || t.contains("campfire") {
                Some(("營火劈啪聲", "Snd_Campfire_Crackling", 80.0, 350.0, 0.75))
            } else {
                None
            };
            if let Some((label, id, inner, outer, vol)) = spec {
                let mut zone = self.create_point_zone(
                    &format!("{label} #{counter}"),
                    id,
                    *x,
                    *y,
                    *z,
                    inner,
                    outer,
                    vol,
                    AttenuationCurveKind::SmoothStep,
                );
                counter += 1;
                zone.attributes |= SoundscapeZoneAttributes::IS_AUTO_GENERATED;
                out.push(zone);
            }
        }
        out
    }
}

#[derive(Clone, Debug)]
pub struct SoundscapeEditorModule {
    state: SoundscapeSnapshot,
    baseline: SoundscapeSnapshot,
    undo: Vec<SoundscapeSnapshot>,
    redo: Vec<SoundscapeSnapshot>,
}

impl Default for SoundscapeEditorModule {
    fn default() -> Self {
        let state = SoundscapeSnapshot::default();
        Self {
            baseline: state.clone(),
            state,
            undo: vec![],
            redo: vec![],
        }
    }
}

impl SoundscapeEditorModule {
    pub const MODULE_ID: &'static str = "SoundscapeZones";

    pub fn capture(&self) -> SoundscapeSnapshot {
        self.state.clone()
    }

    pub fn zones(&self) -> &[SoundscapeZone] {
        &self.state.zones
    }

    pub fn global_ambience_id(&self) -> &str {
        &self.state.global_ambience_id
    }

    pub fn global_ambience_volume(&self) -> f32 {
        self.state.global_ambience_volume
    }

    pub fn load(&mut self, s: &SoundscapeSnapshot) {
        self.state = s.clone();
        self.baseline = s.clone();
        self.undo.clear();
        self.redo.clear();
    }

    pub fn accept_changes(&mut self) {
        self.baseline = self.capture();
    }

    pub fn reset(&mut self) {
        let s = self.baseline.clone();
        self.load(&s);
    }

    fn push_undo(&mut self) {
        self.undo.push(self.capture());
        self.redo.clear();
    }

    pub fn set_global_ambience_id(&mut self, id: String) {
        if self.state.global_ambience_id != id {
            self.push_undo();
            self.state.global_ambience_id = id;
        }
    }

    pub fn set_global_ambience_volume(&mut self, v: f32) {
        if (self.state.global_ambience_volume - v).abs() > 1e-4 {
            self.push_undo();
            self.state.global_ambience_volume = v;
        }
    }

    pub fn add_zone(&mut self, z: &SoundscapeZone) {
        self.push_undo();
        self.state.zones.push(z.clone());
    }

    pub fn remove_zone(&mut self, id: Uuid) -> bool {
        if let Some(i) = self.state.zones.iter().position(|z| z.id == id) {
            self.push_undo();
            self.state.zones.remove(i);
            true
        } else {
            false
        }
    }

    pub fn update_zone(&mut self, z: &SoundscapeZone) -> bool {
        if let Some(i) = self.state.zones.iter().position(|v| v.id == z.id) {
            self.push_undo();
            self.state.zones[i] = z.clone();
            true
        } else {
            false
        }
    }

    pub fn add_auto_generated_zones(&mut self, z: &[SoundscapeZone]) {
        if z.is_empty() {
            return;
        }
        self.push_undo();
        self.state.zones.extend(z.iter().cloned().map(|mut zone| {
            zone.attributes |= SoundscapeZoneAttributes::IS_AUTO_GENERATED;
            zone
        }));
    }

    pub fn clear_auto_generated_zones(&mut self) -> usize {
        let n = self
            .state
            .zones
            .iter()
            .filter(|z| z.attributes & SoundscapeZoneAttributes::IS_AUTO_GENERATED != 0)
            .count();
        if n > 0 {
            self.push_undo();
            self.state
                .zones
                .retain(|z| z.attributes & SoundscapeZoneAttributes::IS_AUTO_GENERATED == 0);
        }
        n
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        if let Some(s) = self.undo.pop() {
            self.redo.push(self.capture());
            self.state = s;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(s) = self.redo.pop() {
            self.undo.push(self.capture());
            self.state = s;
            true
        } else {
            false
        }
    }

    pub fn is_dirty(&self) -> bool {
        !snapshots_equal(&self.state, &self.baseline)
    }
}

/// Dirty comparison. Unlike the C# original, CenterWorldY is compared too, so a
/// height-only zone edit still marks the session unsaved.
fn snapshots_equal(a: &SoundscapeSnapshot, b: &SoundscapeSnapshot) -> bool {
    let close = |x: f32, y: f32| (x - y).abs() <= 1e-4;
    a.global_ambience_id == b.global_ambience_id
        && close(a.global_ambience_volume, b.global_ambience_volume)
        && a.zones.len() == b.zones.len()
        && a.zones.iter().zip(&b.zones).all(|(a, b)| {
            a.id == b.id
                && a.name == b.name
                && a.sound_def_id == b.sound_def_id
                && a.shape_type == b.shape_type
                && a.attenuation_curve == b.attenuation_curve
                && a.category == b.category
                && a.priority == b.priority
                && a.attributes == b.attributes
                && [
                    (a.base_volume, b.base_volume),
                    (a.inner_radius, b.inner_radius),
                    (a.outer_radius, b.outer_radius),
                    (a.center_world_x, b.center_world_x),
                    (a.center_world_y, b.center_world_y),
                    (a.center_world_z, b.center_world_z),
                    (a.param_a, b.param_a),
                    (a.param_b, b.param_b),
                    (a.param_angle_deg, b.param_angle_deg),
                ]
                .iter()
                .all(|&(x, y)| close(x, y))
                && a.polygon_vertices.len() == b.polygon_vertices.len()
                && a.polygon_vertices
                    .iter()
                    .zip(&b.polygon_vertices)
                    .all(|(a, b)| close(a.x, b.x) && close(a.z, b.z))
        })
}

// =========================================================================
// SoundscapeMixEvaluator and Spatial Audio Evaluation Models
// =========================================================================

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudibleChannelEvaluation {
    pub zone_id: Uuid,
    pub sound_def_id: String,
    pub category: AudioCategory,
    pub distance_to_zone: f32,
    pub falloff_factor: f32,
    pub effective_volume: f32,
    pub panning_left: f32,
    pub panning_right: f32,
    pub azimuth_degrees: f32,
    pub priority: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SoundscapeMixResult {
    pub listener_x: f32,
    pub listener_y: f32,
    pub listener_z: f32,
    pub listener_yaw_deg: f32,
    pub total_perceived_volume: f32,
    pub master_left_volume: f32,
    pub master_right_volume: f32,
    pub active_audible_count: usize,
    pub channel_budget_exceeded: bool,
    pub channels: Vec<AudibleChannelEvaluation>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioGizmoRepresentation {
    pub zone_id: Uuid,
    pub name: String,
    pub sound_def_id: String,
    pub category: AudioCategory,
    pub shape_type: SoundscapeShapeType,
    pub center_x: f32,
    pub center_y: f32,
    pub center_z: f32,
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub core_boundary: Vec<SoundscapeVertex>,
    pub falloff_boundary: Vec<SoundscapeVertex>,
    pub color_rgba: u32,
    pub is_selected: bool,
}

#[derive(Clone, Debug)]
pub struct SoundscapeMixEvaluator {
    catalog: SoundscapeZoneCatalog,
}

impl Default for SoundscapeMixEvaluator {
    fn default() -> Self {
        Self {
            catalog: SoundscapeZoneCatalog::shared(),
        }
    }
}

impl SoundscapeMixEvaluator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_catalog(catalog: SoundscapeZoneCatalog) -> Self {
        Self { catalog }
    }

    pub fn catalog(&self) -> &SoundscapeZoneCatalog {
        &self.catalog
    }

    pub fn evaluate_mix(
        &self,
        listener_x: f32,
        listener_y: f32,
        listener_z: f32,
        listener_yaw_deg: f32,
        zones: &[SoundscapeZone],
        max_hardware_channels: usize,
    ) -> SoundscapeMixResult {
        let mut audible_list = Vec::new();
        let mut sum_left_sq = 0.0f32;
        let mut sum_right_sq = 0.0f32;
        let mut sum_total_sq = 0.0f32;

        for zone in zones {
            if zone.attributes & SoundscapeZoneAttributes::IS_ACTIVE == 0 {
                continue;
            }

            let dist = SoundscapeZoneCatalog::calculate_distance_to_zone(listener_x, listener_z, zone);
            let falloff = SoundscapeZoneCatalog::calculate_attenuation(
                dist,
                zone.inner_radius,
                zone.outer_radius,
                zone.attenuation_curve,
            );

            let effective_vol = zone.base_volume * falloff;
            if effective_vol < 0.001 {
                continue;
            }

            let dx = zone.center_world_x - listener_x;
            let dz = zone.center_world_z - listener_z;
            let world_angle_deg = dx.atan2(dz) * (180.0 / std::f32::consts::PI);
            let rel_angle_deg = normalize_angle(world_angle_deg - listener_yaw_deg);

            let rad = rel_angle_deg * (std::f32::consts::PI / 180.0);
            let lateral = rad.sin();
            let pan_r = (0.5 + 0.45 * lateral).clamp(0.05, 0.95);
            let pan_l = 1.0 - pan_r;

            let vol_l = effective_vol * pan_l;
            let vol_r = effective_vol * pan_r;

            sum_left_sq += vol_l * vol_l;
            sum_right_sq += vol_r * vol_r;
            sum_total_sq += effective_vol * effective_vol;

            audible_list.push(AudibleChannelEvaluation {
                zone_id: zone.id,
                sound_def_id: zone.sound_def_id.clone(),
                category: zone.category,
                distance_to_zone: dist,
                falloff_factor: falloff,
                effective_volume: effective_vol,
                panning_left: pan_l,
                panning_right: pan_r,
                azimuth_degrees: rel_angle_deg,
                priority: zone.priority,
            });
        }

        let budget_exceeded = audible_list.len() > max_hardware_channels;
        if budget_exceeded {
            audible_list.sort_by(|a, b| {
                b.priority
                    .cmp(&a.priority)
                    .then_with(|| b.effective_volume.total_cmp(&a.effective_volume))
            });
            audible_list.truncate(max_hardware_channels);
        }

        let perceived_total = sum_total_sq.sqrt().min(1.0);
        let master_l = sum_left_sq.sqrt().min(1.0);
        let master_r = sum_right_sq.sqrt().min(1.0);

        SoundscapeMixResult {
            listener_x,
            listener_y,
            listener_z,
            listener_yaw_deg,
            total_perceived_volume: perceived_total,
            master_left_volume: master_l,
            master_right_volume: master_r,
            active_audible_count: audible_list.len(),
            channel_budget_exceeded: budget_exceeded,
            channels: audible_list,
        }
    }

    pub fn build_gizmos(
        &self,
        zones: &[SoundscapeZone],
        selected_zone_id: Option<Uuid>,
    ) -> Vec<AudioGizmoRepresentation> {
        let mut gizmos = Vec::with_capacity(zones.len());
        for z in zones {
            let is_selected = selected_zone_id.map_or(false, |id| id == z.id);
            let color_rgba = get_category_color(z.category, is_selected);
            let core = build_core_boundary(z);
            let falloff = build_falloff_boundary(z);

            gizmos.push(AudioGizmoRepresentation {
                zone_id: z.id,
                name: z.name.clone(),
                sound_def_id: z.sound_def_id.clone(),
                category: z.category,
                shape_type: z.shape_type,
                center_x: z.center_world_x,
                center_y: z.center_world_y,
                center_z: z.center_world_z,
                inner_radius: z.inner_radius,
                outer_radius: z.outer_radius,
                core_boundary: core,
                falloff_boundary: falloff,
                color_rgba,
                is_selected,
            });
        }
        gizmos
    }

    pub fn generate_acoustic_heatmap(
        &self,
        zones: &[SoundscapeZone],
        world_min_x: f32,
        world_min_z: f32,
        world_max_x: f32,
        world_max_z: f32,
        sample_grid_resolution: usize,
    ) -> Vec<Vec<f32>> {
        let res = sample_grid_resolution.clamp(8, 256);
        let mut heatmap = vec![vec![0.0f32; res]; res];

        let step_x = (world_max_x - world_min_x) / res as f32;
        let step_z = (world_max_z - world_min_z) / res as f32;

        for gy in 0..res {
            let pz = world_min_z + (gy as f32 + 0.5) * step_z;
            for gx in 0..res {
                let px = world_min_x + (gx as f32 + 0.5) * step_x;
                let mut sum_sq = 0.0f32;

                for z in zones {
                    if z.attributes & SoundscapeZoneAttributes::IS_ACTIVE == 0 {
                        continue;
                    }
                    let dist = SoundscapeZoneCatalog::calculate_distance_to_zone(px, pz, z);
                    let att = SoundscapeZoneCatalog::calculate_attenuation(
                        dist,
                        z.inner_radius,
                        z.outer_radius,
                        z.attenuation_curve,
                    );
                    let vol = z.base_volume * att;
                    sum_sq += vol * vol;
                }

                heatmap[gy][gx] = sum_sq.sqrt().min(1.0);
            }
        }

        heatmap
    }
}

fn build_core_boundary(zone: &SoundscapeZone) -> Vec<SoundscapeVertex> {
    match zone.shape_type {
        SoundscapeShapeType::Point => vec![SoundscapeVertex {
            x: zone.center_world_x,
            z: zone.center_world_z,
        }],
        SoundscapeShapeType::Circle => generate_circle_vertices(
            zone.center_world_x,
            zone.center_world_z,
            zone.param_a.max(10.0),
            24,
        ),
        SoundscapeShapeType::OrientedRectangle => generate_oriented_box_vertices(
            zone.center_world_x,
            zone.center_world_z,
            zone.param_a,
            zone.param_b,
            zone.param_angle_deg,
        ),
        SoundscapeShapeType::ConvexPolygon => zone.polygon_vertices.clone(),
    }
}

fn build_falloff_boundary(zone: &SoundscapeZone) -> Vec<SoundscapeVertex> {
    let total_r = zone.outer_radius;
    match zone.shape_type {
        SoundscapeShapeType::Point => generate_circle_vertices(
            zone.center_world_x,
            zone.center_world_z,
            total_r,
            24,
        ),
        SoundscapeShapeType::Circle => generate_circle_vertices(
            zone.center_world_x,
            zone.center_world_z,
            zone.param_a.max(10.0) + total_r,
            24,
        ),
        SoundscapeShapeType::OrientedRectangle => generate_oriented_box_vertices(
            zone.center_world_x,
            zone.center_world_z,
            zone.param_a + total_r,
            zone.param_b + total_r,
            zone.param_angle_deg,
        ),
        SoundscapeShapeType::ConvexPolygon => expand_polygon(&zone.polygon_vertices, total_r),
    }
}

fn generate_circle_vertices(cx: f32, cz: f32, radius: f32, segments: usize) -> Vec<SoundscapeVertex> {
    let mut list = Vec::with_capacity(segments);
    for i in 0..segments {
        let theta = (i as f32) * (2.0 * std::f32::consts::PI / segments as f32);
        list.push(SoundscapeVertex {
            x: cx + theta.cos() * radius,
            z: cz + theta.sin() * radius,
        });
    }
    list
}

fn generate_oriented_box_vertices(
    cx: f32,
    cz: f32,
    half_w: f32,
    half_h: f32,
    angle_deg: f32,
) -> Vec<SoundscapeVertex> {
    let rad = angle_deg * (std::f32::consts::PI / 180.0);
    let cos = rad.cos();
    let sin = rad.sin();
    let local_corners = [
        (-half_w, -half_h),
        (half_w, -half_h),
        (half_w, half_h),
        (-half_w, half_h),
    ];
    local_corners
        .iter()
        .map(|&(lx, lz)| {
            let wx = cx + (lx * cos - lz * sin);
            let wz = cz + (lx * sin + lz * cos);
            SoundscapeVertex { x: wx, z: wz }
        })
        .collect()
}

fn expand_polygon(verts: &[SoundscapeVertex], expansion: f32) -> Vec<SoundscapeVertex> {
    if verts.len() < 3 {
        return Vec::new();
    }
    let sum_x: f32 = verts.iter().map(|v| v.x).sum();
    let sum_z: f32 = verts.iter().map(|v| v.z).sum();
    let avg_x = sum_x / verts.len() as f32;
    let avg_z = sum_z / verts.len() as f32;
    verts
        .iter()
        .map(|v| {
            let dx = v.x - avg_x;
            let dz = v.z - avg_z;
            let len = (dx * dx + dz * dz).sqrt();
            if len > 1e-4 {
                let factor = (len + expansion) / len;
                SoundscapeVertex {
                    x: avg_x + dx * factor,
                    z: avg_z + dz * factor,
                }
            } else {
                *v
            }
        })
        .collect()
}

fn get_category_color(category: AudioCategory, is_selected: bool) -> u32 {
    if is_selected {
        return 0xFFFFE000;
    }
    match category {
        AudioCategory::Hydrology => 0xFF00BFFF,
        AudioCategory::Vegetation => 0xFF32CD32,
        AudioCategory::PointEmitter => 0xFFFF4500,
        AudioCategory::Settlement => 0xFFFF8C00,
        AudioCategory::WeatherFx => 0xFF4682B4,
        AudioCategory::AmbientGlobal => 0xFFBA55D3,
        _ => 0xFF87CEEB,
    }
}

fn normalize_angle(mut deg: f32) -> f32 {
    while deg > 180.0 {
        deg -= 360.0;
    }
    while deg <= -180.0 {
        deg += 360.0;
    }
    deg
}
