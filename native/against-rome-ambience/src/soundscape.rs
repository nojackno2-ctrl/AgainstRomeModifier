//! Editor-only soundscape prototype. SNDZ is not a native game sound format.
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, String>;

macro_rules! enumeration {
    ($name:ident {$($v:ident),* $(,)?}) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[repr(u8)]
        pub enum $name { $($v),* }
    };
}

enumeration!(SoundscapeShapeType { Point, Circle, OrientedRectangle, ConvexPolygon });
enumeration!(AttenuationCurveKind { Linear, SmoothStep, Logarithmic, Exponential });
enumeration!(AudioCategory { AmbientGlobal, AmbientZone, PointEmitter, Hydrology, Vegetation, Settlement, WeatherFx });
enumeration!(SoundTriggerMode { ContinuousLoop, RandomStochasticInterval, TimeOfDayConditional });

pub struct SoundscapeZoneAttributes;
impl SoundscapeZoneAttributes {
    pub const NONE: u32 = 0;
    pub const IS_LOOPING: u32 = 1 << 0;
    pub const IS_ACTIVE: u32 = 1 << 1;
    pub const IS_AUTO_GENERATED: u32 = 1 << 2;
    pub const OCCLUSION_ENABLED: u32 = 1 << 3;
    pub const DAY_ONLY: u32 = 1 << 4;
    pub const NIGHT_ONLY: u32 = 1 << 5;
}
pub type SoundscapeZoneFlags = SoundscapeZoneAttributes;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SoundscapeVertex {
    pub x: f32,
    pub z: f32,
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

pub struct SoundscapeBinaryStorage;

impl SoundscapeBinaryStorage {
    pub fn serialize_to_binary(s: &SoundscapeSnapshot) -> Result<Vec<u8>> {
        if s.zones.len() > u16::MAX as usize {
            return Err("too many SNDZ zones".into());
        }
        let mut b = Vec::new();
        b.extend(b"SNDZ");
        b.extend(1u16.to_le_bytes());
        b.extend((s.zones.len() as u16).to_le_bytes());
        b.extend(s.global_ambience_volume.to_le_bytes());
        fixed(&mut b, &s.global_ambience_id);
        b.extend([0; 20]);
        for z in &s.zones {
            let verts = if z.shape_type == SoundscapeShapeType::ConvexPolygon {
                z.polygon_vertices.as_slice()
            } else {
                &[]
            };
            if verts.len() > u16::MAX as usize {
                return Err("too many SNDZ polygon vertices".into());
            }
            b.extend(z.id.to_bytes_le());
            fixed(&mut b, &z.name);
            fixed(&mut b, &z.sound_def_id);
            b.extend([z.shape_type as u8, z.attenuation_curve as u8, z.category as u8, 0]);
            b.extend(z.attributes.to_le_bytes());
            b.extend(z.priority.to_le_bytes());
            b.extend([0; 2]);
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
                b.extend(f.to_le_bytes());
            }
            b.extend((verts.len() as u16).to_le_bytes());
            for v in verts {
                b.extend(v.x.to_le_bytes());
                b.extend(v.z.to_le_bytes());
            }
        }
        Ok(b)
    }

    pub fn deserialize_from_binary(data: &[u8]) -> Result<SoundscapeSnapshot> {
        if data.len() < 64 {
            return Err("音效資料長度不足 64 位元組標頭。".into());
        }
        let raw;
        if data.starts_with(b"PFIL") {
            raw = against_rome_data::lzss::decompress_pfil(data, None).map_err(|e| e.to_string())?;
        } else {
            raw = data.to_vec();
        }
        let mut r = Reader { bytes: &raw, pos: 0 };
        if r.take(4)? != b"SNDZ" {
            return Err("非有效的 SNDZ 音效二進位檔案魔數。".into());
        }
        let version = r.u16()?;
        if version != 1 {
            return Err(format!("不支援的 SNDZ 版本: {version}"));
        }
        let n = r.u16()? as usize;
        let volume = r.f32()?;
        let id = r.string()?;
        r.take(20)?;
        let mut zones = Vec::with_capacity(n);
        for _ in 0..n {
            let id = Uuid::from_bytes_le(r.take(16)?.try_into().unwrap());
            let name = r.string()?;
            let sound_def_id = r.string()?;
            let shape_type = match r.byte()? {
                0 => SoundscapeShapeType::Point,
                1 => SoundscapeShapeType::Circle,
                2 => SoundscapeShapeType::OrientedRectangle,
                3 => SoundscapeShapeType::ConvexPolygon,
                _ => return Err("invalid SNDZ shape".into()),
            };
            let attenuation_curve = match r.byte()? {
                0 => AttenuationCurveKind::Linear,
                1 => AttenuationCurveKind::SmoothStep,
                2 => AttenuationCurveKind::Logarithmic,
                3 => AttenuationCurveKind::Exponential,
                _ => return Err("invalid SNDZ attenuation".into()),
            };
            let category = match r.byte()? {
                0 => AudioCategory::AmbientGlobal,
                1 => AudioCategory::AmbientZone,
                2 => AudioCategory::PointEmitter,
                3 => AudioCategory::Hydrology,
                4 => AudioCategory::Vegetation,
                5 => AudioCategory::Settlement,
                6 => AudioCategory::WeatherFx,
                _ => return Err("invalid SNDZ category".into()),
            };
            r.byte()?;
            let attributes = r.u32()?;
            let priority = r.u16()?;
            r.u16()?;
            let mut z = SoundscapeZone {
                id,
                name,
                sound_def_id,
                shape_type,
                attenuation_curve,
                category,
                attributes,
                priority,
                base_volume: r.f32()?,
                inner_radius: r.f32()?,
                outer_radius: r.f32()?,
                center_world_x: r.f32()?,
                center_world_y: r.f32()?,
                center_world_z: r.f32()?,
                param_a: r.f32()?,
                param_b: r.f32()?,
                param_angle_deg: r.f32()?,
                polygon_vertices: vec![],
            };
            let vert_count = r.u16()?;
            for _ in 0..vert_count {
                z.polygon_vertices.push(SoundscapeVertex {
                    x: r.f32()?,
                    z: r.f32()?,
                });
            }
            zones.push(z);
        }
        Ok(SoundscapeSnapshot {
            global_ambience_id: id,
            global_ambience_volume: volume,
            zones,
        })
    }

    pub fn load_from_sound_dat(path: impl AsRef<std::path::Path>) -> Result<SoundscapeSnapshot> {
        Self::deserialize_from_binary(&std::fs::read(path).map_err(|e| e.to_string())?)
    }

    pub fn save_to_sound_dat(
        _path: impl AsRef<std::path::Path>,
        _snapshot: &SoundscapeSnapshot,
        _compress_pfil: bool,
        _orig_header: Option<&[u8]>,
    ) -> Result<()> {
        Err("Native sound-zone export is unwired. SNDZ is an editor prototype, not DATA/sound.dat.".into())
    }

    pub fn export_to_script(_snapshot: &SoundscapeSnapshot) -> Result<String> {
        Err("Native sound-zone script export is unwired; no verified spatial playback contract exists.".into())
    }

    pub fn export_to_json(snapshot: &SoundscapeSnapshot) -> Result<String> {
        let mut value = serde_json::to_value(snapshot).map_err(|e| e.to_string())?;
        if let Some(zones) = value.get_mut("Zones").and_then(|v| v.as_array_mut()) {
            for zone in zones {
                if let Some(attrs) = zone.get("Attributes").and_then(|v| v.as_u64()) {
                    let flags = flags_string(attrs as u32);
                    zone["Attributes"] = flags.clone();
                    zone["Flags"] = flags;
                }
            }
        }
        serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
    }

    pub fn import_from_json(json: &str) -> Result<SoundscapeSnapshot> {
        let mut value: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if let Some(zones) = value.get_mut("Zones").and_then(|v| v.as_array_mut()) {
            for z in zones {
                for key in ["Attributes", "Flags"] {
                    if let Some(v) = z.get(key) {
                        let attrs = if let Some(s) = v.as_str() {
                            parse_flags(s)?
                        } else {
                            v.as_u64().ok_or("invalid zone attributes")? as u32
                        };
                        z["Attributes"] = attrs.into();
                    }
                }
            }
        }
        serde_json::from_value(value).map_err(|e| e.to_string())
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
            _ => return Err(format!("invalid zone attribute: {p}")),
        };
    }
    Ok(v)
}

fn fixed(b: &mut Vec<u8>, s: &str) {
    let mut v = [0; 32];
    let n = s.len().min(32);
    v[..n].copy_from_slice(&s.as_bytes()[..n]);
    b.extend(v);
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or("SNDZ offset overflow")?;
        let s = self.bytes.get(self.pos..end).ok_or("truncated SNDZ record")?;
        self.pos = end;
        Ok(s)
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn string(&mut self) -> Result<String> {
        let b = self.take(32)?;
        let n = b.iter().position(|&x| x == 0).unwrap_or(32);
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

/// Preserves C# dirty comparison, including its omission of CenterWorldY.
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
