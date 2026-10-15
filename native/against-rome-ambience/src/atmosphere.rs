//! Atmosphere authoring, presets, serialization, and preview.
use serde::{Deserialize, Serialize};

pub type Vec3 = [f32; 3];
pub type Vec4 = [f32; 4];

fn normalize(v: Vec3) -> Vec3 {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n > 1e-6 {
        [v[0] / n, v[1] / n, v[2] / n]
    } else {
        [0.0, 1.0, 0.0]
    }
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.001
}

macro_rules! boden_fields {
    ($($field:ident : $ty:ty = $default:expr => $key:literal),* $(,)?) => {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct BodenIniData {
            $(pub $field: $ty,)*
            pub preserved_sections: Vec<(String, String)>,
        }

        impl Default for BodenIniData {
            fn default() -> Self {
                Self {
                    $($field: $default,)*
                    preserved_sections: Vec::new(),
                }
            }
        }

        impl BodenIniData {
            fn apply_value(&mut self, key: &str, value: &str) -> bool {
                $(
                    if key.eq_ignore_ascii_case($key) {
                        if let Some(v) = <$ty as IniValue>::parse_ini(value) {
                            self.$field = v;
                        }
                        return true;
                    }
                )*
                false
            }

            pub fn entries(&self) -> Vec<(&'static str, String)> {
                vec![$(($key, self.$field.ini_value())),*]
            }
        }

        pub fn is_known_key(key: &str) -> bool {
            $(
                if key.eq_ignore_ascii_case($key) {
                    return true;
                }
            )*
            false
        }
    };
}

trait IniValue: Sized {
    fn parse_ini(s: &str) -> Option<Self>;
    fn ini_value(&self) -> String;
}

impl IniValue for String {
    fn parse_ini(s: &str) -> Option<Self> {
        Some(s.trim().into())
    }
    fn ini_value(&self) -> String {
        self.trim().into()
    }
}

impl IniValue for bool {
    fn parse_ini(s: &str) -> Option<Self> {
        Some(s.trim() == "1")
    }
    fn ini_value(&self) -> String {
        if *self { "1" } else { "0" }.into()
    }
}

impl IniValue for i32 {
    fn parse_ini(s: &str) -> Option<Self> {
        s.trim().parse().ok()
    }
    fn ini_value(&self) -> String {
        self.to_string()
    }
}

impl IniValue for f32 {
    fn parse_ini(s: &str) -> Option<Self> {
        s.trim().parse().ok()
    }
    fn ini_value(&self) -> String {
        self.to_string()
    }
}

boden_fields! {
    waterlevel: f32 = 120.0 => "Waterlevel",
    heightmapstep: f32 = 4.0 => "Heightmapstep",
    water_bump_amplitude: i32 = 256 => "WaterBumpAmplitude",
    water_bump_frequency: i32 = 4 => "WaterBumpFrequency",
    water_warp_shift: i32 = 12 => "WaterWarpShift",
    rain_drops_on_water: bool = true => "RainDropsOnWater",
    water_color: String = "0xffdfbf".into() => "WaterColor",
    wasser_textur_name: String = "wassAW".into() => "WasserTexturName",
    caustic_textur_name: String = "caustA".into() => "CausticTexturName",
    sky_textur_name: String = "sky".into() => "SkyTexturName",
    flash_propability: i32 = 8 => "FlashPropability",
    flash_object_default_index: i32 = 803 => "FlashObjectDefaultIndex",
    flash_object_default2_index: i32 = 1099 => "FlashObjectDefault2Index",
    flash_light_default_index: i32 = 6 => "FlashLightDefaultIndex",
    snow_alr_index: i32 = 837 => "SnowAlrIndex",
    snow_shadow_index: i32 = 152 => "SnowShadowIndex",
    snow_shadow_size: i32 = 7 => "SnowShadowSize",
    hagel_shadow_index: i32 = 152 => "HagelShadowIndex",
    hagel_shadow_size: i32 = 7 => "HagelShadowSize",
    move_list_amplitude: i32 = 127 => "MoveListAmplitude",
    day_start_time: f32 = 6.0 => "DayStartTime",
    day_end_time: f32 = 20.0 => "DayEndTime",
    shadow_mesh_mode: i32 = 0 => "ShadowMeshMode",
    shadow_mesh_accuracy: i32 = 6 => "ShadowMeshAccuracy",
    shadow_mesh_xzsize: i32 = 16000 => "ShadowMeshXZsize",
    shadow_mesh_ysize: i32 = 5000 => "ShadowMeshYsize",
    skydensspread: f32 = 16.0 => "Skydensspread",
    skydensaccuracy: i32 = 2 => "Skydensaccuracy",
    handle_sky_dens_map: i32 = 1 => "HandleSkyDensMap",
    handle_visible_map: i32 = 1 => "HandleVisibleMap",
    handle_clip_rect_map: i32 = 1 => "HandleClipRectMap",
    handle_shadow_meshes: i32 = 1 => "HandleShadowMeshes",
    show_collision_mesh: i32 = 0 => "ShowCollisionMesh",
}

impl BodenIniData {
    pub fn get_water_rgb(&self) -> Vec3 {
        let hex = self.water_color.trim().replace("0x", "").replace("0X", "");
        if hex.len() == 6 {
            if let Ok(v) = u32::from_str_radix(&hex, 16) {
                return [
                    (v & 255) as f32 / 255.0,
                    ((v >> 8) & 255) as f32 / 255.0,
                    ((v >> 16) & 255) as f32 / 255.0,
                ];
            }
        }
        [0.25, 0.5, 0.8]
    }

    pub fn set_water_rgb(&mut self, r: f32, g: f32, b: f32) {
        let cv = |v: f32| ((v * 255.0).round().clamp(0.0, 255.0)) as u8;
        self.water_color = format!("0x{:02x}{:02x}{:02x}", cv(b), cv(g), cv(r));
    }
}

pub struct BodenIniSerializer;

impl BodenIniSerializer {
    pub fn parse_text(ini_text: &str) -> BodenIniData {
        let mut data = BodenIniData::default();
        let normalized = ini_text.replace("\r\n", "\n");
        let lines: Vec<&str> = normalized.split('\n').collect();
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i].trim();
            if line.starts_with('[') && line.contains(']') {
                let end_bracket = line.find(']').unwrap();
                let section = line[1..end_bracket].trim();
                let mut value_line = "";
                if i + 1 < lines.len() {
                    i += 1;
                    value_line = lines[i];
                }
                let raw_val = if let Some((before_semi, _)) = value_line.split_once(';') {
                    before_semi.trim()
                } else {
                    value_line.trim()
                };

                if is_known_key(section) {
                    data.apply_value(section, raw_val);
                } else {
                    if let Some(existing) = data.preserved_sections.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(section)) {
                        existing.1 = raw_val.to_string();
                    } else {
                        data.preserved_sections.push((section.to_string(), raw_val.to_string()));
                    }
                }
            }
            i += 1;
        }
        data
    }

    pub fn generate_default_ini(data: &BodenIniData) -> String {
        let mut s = String::new();
        macro_rules! append_key {
            ($key:literal, $val:expr, $comment:expr) => {
                s.push('[');
                s.push_str($key);
                s.push(']');
                let c: &str = $comment;
                if !c.is_empty() {
                    s.push_str(" ;");
                    s.push_str(c);
                }
                s.push_str("\r\n");
                s.push_str(&$val.to_string());
                s.push_str("\r\n");
            };
            ($key:literal, $val:expr) => {
                append_key!($key, $val, "");
            };
        }

        append_key!("Waterlevel", data.waterlevel, "0..");
        append_key!("Heightmapstep", data.heightmapstep, "1..");
        append_key!("ShadowMeshMode", data.shadow_mesh_mode, "0=太陽只上下移動, 1=太陽東南西原始移動");
        append_key!("WasserTexturName", data.wasser_textur_name, "水面貼圖基底名 name00.bmp,...");
        append_key!("CausticTexturName", data.caustic_textur_name);
        append_key!("SkyTexturName", data.sky_textur_name);
        append_key!("RainDropsOnWater", if data.rain_drops_on_water { "1" } else { "0" }, "0/1");
        append_key!("WaterWarpShift", data.water_warp_shift, "0=關閉, 18(低)..10(高), 註解預設14、樣本12");
        append_key!("WaterBumpAmplitude", data.water_bump_amplitude, "0..1024, 256=default");
        append_key!("WaterBumpFrequency", data.water_bump_frequency, "1..16, 4=default");
        append_key!("FlashPropability", data.flash_propability, "最大降雨強度時閃電參數 0..1000，實際單位待確認");
        append_key!("WaterColor", data.water_color, "水色 Hex (bgr), 註解預設0xffbf7f、樣本0xffdfbf");
        append_key!("DayStartTime", data.day_start_time, "白天開始(時)");
        append_key!("DayEndTime", data.day_end_time, "夜晚開始(時)");

        if data.flash_object_default_index >= 0 { append_key!("FlashObjectDefaultIndex", data.flash_object_default_index); }
        if data.flash_object_default2_index >= 0 { append_key!("FlashObjectDefault2Index", data.flash_object_default2_index); }
        if data.flash_light_default_index >= 0 { append_key!("FlashLightDefaultIndex", data.flash_light_default_index); }
        if data.snow_alr_index >= 0 { append_key!("SnowAlrIndex", data.snow_alr_index); }
        if data.snow_shadow_index >= 0 { append_key!("SnowShadowIndex", data.snow_shadow_index); }
        if data.snow_shadow_size > 0 { append_key!("SnowShadowSize", data.snow_shadow_size); }
        if data.hagel_shadow_index >= 0 { append_key!("HagelShadowIndex", data.hagel_shadow_index); }
        if data.hagel_shadow_size > 0 { append_key!("HagelShadowSize", data.hagel_shadow_size); }
        if data.move_list_amplitude > 0 { append_key!("MoveListAmplitude", data.move_list_amplitude); }

        append_key!("ShadowMeshAccuracy", data.shadow_mesh_accuracy);
        append_key!("ShadowMeshXZsize", data.shadow_mesh_xzsize);
        append_key!("ShadowMeshYsize", data.shadow_mesh_ysize);
        append_key!("Skydensspread", data.skydensspread);
        append_key!("Skydensaccuracy", data.skydensaccuracy);
        append_key!("HandleSkyDensMap", data.handle_sky_dens_map);
        append_key!("HandleVisibleMap", data.handle_visible_map);
        append_key!("HandleClipRectMap", data.handle_clip_rect_map);
        append_key!("HandleShadowMeshes", data.handle_shadow_meshes);
        append_key!("ShowCollisionMesh", data.show_collision_mesh);

        for (k, v) in &data.preserved_sections {
            if !is_known_key(k) {
                s.push('[');
                s.push_str(k);
                s.push_str("]\r\n");
                s.push_str(v);
                s.push_str("\r\n");
            }
        }
        s
    }

    pub fn validate(data: &BodenIniData) -> (bool, Vec<String>) {
        let mut errors = Vec::new();

        if !(0..=1024).contains(&data.water_bump_amplitude) {
            errors.push(format!("WaterBumpAmplitude 數值 ({}) 超出安全範圍 0..1024。", data.water_bump_amplitude));
        }
        if !(1..=16).contains(&data.water_bump_frequency) {
            errors.push(format!("WaterBumpFrequency 數值 ({}) 超出安全範圍 1..16。", data.water_bump_frequency));
        }
        if data.water_warp_shift != 0 && !(10..=18).contains(&data.water_warp_shift) {
            errors.push(format!("WaterWarpShift 數值 ({}) 必須為 0 (關閉) 或 10..18。", data.water_warp_shift));
        }
        if !(0..=1000).contains(&data.flash_propability) {
            errors.push(format!("FlashPropability 數值 ({}) 超出安全範圍 0..1000。", data.flash_propability));
        }
        if !data.day_start_time.is_finite() || !(0.0..=24.0).contains(&data.day_start_time) {
            errors.push(format!("DayStartTime 數值 ({}) 必須介於 0..24 之間。", data.day_start_time));
        }
        if !data.day_end_time.is_finite() || !(0.0..=24.0).contains(&data.day_end_time) {
            errors.push(format!("DayEndTime 數值 ({}) 必須介於 0..24 之間。", data.day_end_time));
        }
        if !data.heightmapstep.is_finite() || data.heightmapstep < 1.0 {
            errors.push(format!("Heightmapstep 數值 ({}) 必須為有限數值且至少為 1。", data.heightmapstep));
        }
        if data.shadow_mesh_mode != 0 && data.shadow_mesh_mode != 1 {
            errors.push(format!("ShadowMeshMode 數值 ({}) 僅支援 0 或 1。", data.shadow_mesh_mode));
        }
        if !data.waterlevel.is_finite() || data.waterlevel < 0.0 {
            errors.push(format!("Waterlevel 數值 ({}) 必須為有限的非負數值。", data.waterlevel));
        }
        if !(0..=7).contains(&data.shadow_mesh_accuracy) {
            errors.push("ShadowMeshAccuracy 必須介於 0..7。".into());
        }
        if !(8192..=16384).contains(&data.shadow_mesh_xzsize) {
            errors.push("ShadowMeshXZsize 必須介於 8192..16384。".into());
        }
        if !(4000..=16000).contains(&data.shadow_mesh_ysize) {
            errors.push("ShadowMeshYsize 必須介於 4000..16000。".into());
        }
        let valid_spreads = [4.0, 8.0, 16.0, 32.0, 64.0, 128.0, 256.0, 512.0, 1024.0];
        if !valid_spreads.iter().any(|&s| (data.skydensspread - s).abs() < 1e-4) {
            errors.push("Skydensspread 必須為 4..1024 中的二次方值。".into());
        }
        if !(0..=2).contains(&data.skydensaccuracy) {
            errors.push("Skydensaccuracy 必須介於 0..2。".into());
        }
        if !(0..=127).contains(&data.move_list_amplitude) {
            errors.push("MoveListAmplitude 必須介於 0..127。".into());
        }
        if data.handle_sky_dens_map != 0 && data.handle_sky_dens_map != 1 {
            errors.push("HandleSkyDensMap 必須為 0 或 1。".into());
        }
        if data.handle_visible_map != 0 && data.handle_visible_map != 1 {
            errors.push("HandleVisibleMap 必須為 0 或 1。".into());
        }
        if data.handle_clip_rect_map != 0 && data.handle_clip_rect_map != 1 {
            errors.push("HandleClipRectMap 必須為 0 或 1。".into());
        }
        if data.handle_shadow_meshes != 0 && data.handle_shadow_meshes != 1 {
            errors.push("HandleShadowMeshes 必須為 0 或 1。".into());
        }
        if data.show_collision_mesh != 0 && data.show_collision_mesh != 1 {
            errors.push("ShowCollisionMesh 必須為 0 或 1。".into());
        }

        let clean_hex = data.water_color.trim().replace("0x", "").replace("0X", "");
        if clean_hex.len() != 6 || u32::from_str_radix(&clean_hex, 16).is_err() {
            errors.push(format!("WaterColor 格式錯誤 ({})，必須為 6 碼十六進位 BGR 色碼 (例: 0xffdfbf)。", data.water_color));
        }

        let is_valid = errors.is_empty();
        (is_valid, errors)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PrecipitationType {
    #[default]
    None = 0,
    Rain = 1,
    Snow = 2,
    Hail = 3,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AtmosphereProfile {
    pub preset_id: String,
    pub name: String,
    pub english_name: String,
    pub description: String,
    pub boden: BodenIniData,
    pub fog_enabled: bool,
    pub fog_start_distance: f32,
    pub fog_end_distance: f32,
    pub fog_density: f32,
    pub fog_color: Vec3,
    pub sky_scattering_color: Vec3,
    pub ambient_tint: Vec3,
    pub exposure: f32,
    pub sun_light_color: Vec3,
    pub sun_light_direction: Vec3,
    pub precipitation: PrecipitationType,
    pub precipitation_intensity: f32,
    pub wind_speed: f32,
    pub wind_direction: f32,
}

impl Default for AtmosphereProfile {
    fn default() -> Self {
        Self {
            preset_id: "custom".into(),
            name: "自訂環境".into(),
            english_name: "Custom Atmosphere".into(),
            description: String::new(),
            boden: BodenIniData::default(),
            fog_enabled: true,
            fog_start_distance: 40.0,
            fog_end_distance: 120.0,
            fog_density: 0.5,
            fog_color: [0.7, 0.75, 0.8],
            sky_scattering_color: [1.0, 1.0, 1.0],
            ambient_tint: [1.0, 1.0, 1.0],
            exposure: 1.0,
            sun_light_color: [1.0, 0.98, 0.92],
            sun_light_direction: normalize([0.4, 0.85, 0.3]),
            precipitation: PrecipitationType::None,
            precipitation_intensity: 0.0,
            wind_speed: 5.0,
            wind_direction: 45.0,
        }
    }
}

impl AtmosphereProfile {
    pub fn value_equals(&self, b: &Self) -> bool {
        let a = self;
        let x = &a.boden;
        let y = &b.boden;
        a.preset_id == b.preset_id
            && a.name == b.name
            && a.english_name == b.english_name
            && a.fog_enabled == b.fog_enabled
            && near(a.fog_start_distance, b.fog_start_distance)
            && near(a.fog_end_distance, b.fog_end_distance)
            && near(a.fog_density, b.fog_density)
            && a.fog_color == b.fog_color
            && a.sky_scattering_color == b.sky_scattering_color
            && a.ambient_tint == b.ambient_tint
            && near(a.exposure, b.exposure)
            && a.precipitation == b.precipitation
            && near(a.precipitation_intensity, b.precipitation_intensity)
            && near(a.wind_speed, b.wind_speed)
            && near(a.wind_direction, b.wind_direction)
            && near(x.waterlevel, y.waterlevel)
            && near(x.heightmapstep, y.heightmapstep)
            && x.water_bump_amplitude == y.water_bump_amplitude
            && x.water_bump_frequency == y.water_bump_frequency
            && x.water_warp_shift == y.water_warp_shift
            && x.rain_drops_on_water == y.rain_drops_on_water
            && x.water_color.eq_ignore_ascii_case(&y.water_color)
            && x.wasser_textur_name.eq_ignore_ascii_case(&y.wasser_textur_name)
            && x.flash_propability == y.flash_propability
            && x.flash_light_default_index == y.flash_light_default_index
            && x.snow_alr_index == y.snow_alr_index
            && near(x.day_start_time, y.day_start_time)
            && near(x.day_end_time, y.day_end_time)
            && x.shadow_mesh_mode == y.shadow_mesh_mode
    }
}

pub struct WeatherPresets;

impl WeatherPresets {
    pub const CLEAR_SKY_ID: &'static str = "clear_sky";
    pub const STORM_ID: &'static str = "storm";
    pub const SUNSET_DUSK_ID: &'static str = "sunset_dusk";
    pub const DENSE_FOG_ID: &'static str = "dense_fog";
    pub const MOUNTAIN_SNOW_ID: &'static str = "mountain_snow";

    pub fn create_clear_sky() -> AtmosphereProfile {
        AtmosphereProfile {
            preset_id: Self::CLEAR_SKY_ID.into(),
            name: "晴朗豔陽".into(),
            english_name: "Clear Sky".into(),
            description: "晴空萬里，陽光明媚。水面呈現柔和微瀾，遠山呈現自然的空氣透視淡藍微霞。".into(),
            boden: BodenIniData {
                water_bump_amplitude: 224,
                water_bump_frequency: 4,
                water_warp_shift: 14,
                water_color: "0xffdfbf".into(),
                rain_drops_on_water: false,
                flash_propability: 0,
                day_start_time: 6.0,
                day_end_time: 20.0,
                shadow_mesh_mode: 0,
                ..BodenIniData::default()
            },
            fog_enabled: true,
            fog_start_distance: 60.0,
            fog_end_distance: 180.0,
            fog_density: 0.20,
            fog_color: [0.72, 0.82, 0.95],
            sky_scattering_color: [1.05, 1.02, 0.98],
            ambient_tint: [1.0, 1.0, 1.0],
            exposure: 1.0,
            sun_light_color: [1.0, 0.98, 0.92],
            sun_light_direction: normalize([0.4, 0.85, 0.3]),
            precipitation: PrecipitationType::None,
            precipitation_intensity: 0.0,
            wind_speed: 4.0,
            wind_direction: 45.0,
            ..AtmosphereProfile::default()
        }
    }

    pub fn create_storm() -> AtmosphereProfile {
        AtmosphereProfile {
            preset_id: Self::STORM_ID.into(),
            name: "雷鳴暴風雨".into(),
            english_name: "Thunderstorm".into(),
            description: "烏雲蔽日，風雨交加。水面狂浪翻騰且泛起密集雨滴漣漪，天候伴隨頻繁雷鳴爆閃。".into(),
            boden: BodenIniData {
                water_bump_amplitude: 320,
                water_bump_frequency: 5,
                water_warp_shift: 12,
                water_color: "0xe0c8aa".into(),
                rain_drops_on_water: true,
                flash_propability: 8,
                flash_light_default_index: 6,
                day_start_time: 6.0,
                day_end_time: 20.0,
                shadow_mesh_mode: 0,
                ..BodenIniData::default()
            },
            fog_enabled: true,
            fog_start_distance: 12.0,
            fog_end_distance: 60.0,
            fog_density: 0.85,
            fog_color: [0.22, 0.26, 0.28],
            sky_scattering_color: [0.50, 0.55, 0.60],
            ambient_tint: [0.65, 0.70, 0.68],
            exposure: 0.75,
            sun_light_color: [0.55, 0.58, 0.62],
            sun_light_direction: normalize([0.2, 0.95, 0.2]),
            precipitation: PrecipitationType::Rain,
            precipitation_intensity: 0.95,
            wind_speed: 22.0,
            wind_direction: 75.0,
            ..AtmosphereProfile::default()
        }
    }

    pub fn create_sunset_dusk() -> AtmosphereProfile {
        AtmosphereProfile {
            preset_id: Self::SUNSET_DUSK_ID.into(),
            name: "暮色日落".into(),
            english_name: "Sunset Dusk".into(),
            description: "殘陽如血，暮靄四起。遠景瀰漫橘紅霞霧，水面反照深邃晚霞光斑，沉靜而蒼茫。".into(),
            boden: BodenIniData {
                water_bump_amplitude: 256,
                water_bump_frequency: 4,
                water_warp_shift: 13,
                water_color: "0xe0d0d0".into(),
                rain_drops_on_water: false,
                flash_propability: 0,
                day_start_time: 6.0,
                day_end_time: 20.0,
                shadow_mesh_mode: 0,
                ..BodenIniData::default()
            },
            fog_enabled: true,
            fog_start_distance: 30.0,
            fog_end_distance: 110.0,
            fog_density: 0.50,
            fog_color: [0.85, 0.45, 0.30],
            sky_scattering_color: [1.20, 0.75, 0.45],
            ambient_tint: [1.10, 0.85, 0.70],
            exposure: 0.95,
            sun_light_color: [1.25, 0.70, 0.35],
            sun_light_direction: normalize([0.85, 0.35, 0.4]),
            precipitation: PrecipitationType::None,
            precipitation_intensity: 0.0,
            wind_speed: 5.0,
            wind_direction: 200.0,
            ..AtmosphereProfile::default()
        }
    }

    pub fn create_dense_fog() -> AtmosphereProfile {
        AtmosphereProfile {
            preset_id: Self::DENSE_FOG_ID.into(),
            name: "濃霧迷漫".into(),
            english_name: "Dense Fog".into(),
            description: "晨霧如幔，視線受限。近距離即被迷離白霧遮蔽，水波平緩，適合營造神秘森林或埋伏戰術。".into(),
            boden: BodenIniData {
                water_bump_amplitude: 224,
                water_bump_frequency: 4,
                water_warp_shift: 14,
                water_color: "0xf0d8bf".into(),
                rain_drops_on_water: false,
                flash_propability: 0,
                day_start_time: 6.0,
                day_end_time: 20.0,
                shadow_mesh_mode: 0,
                ..BodenIniData::default()
            },
            fog_enabled: true,
            fog_start_distance: 4.0,
            fog_end_distance: 38.0,
            fog_density: 0.96,
            fog_color: [0.68, 0.73, 0.76],
            sky_scattering_color: [0.75, 0.78, 0.80],
            ambient_tint: [0.78, 0.82, 0.84],
            exposure: 0.85,
            sun_light_color: [0.70, 0.72, 0.75],
            sun_light_direction: normalize([0.3, 0.9, 0.3]),
            precipitation: PrecipitationType::None,
            precipitation_intensity: 0.0,
            wind_speed: 2.0,
            wind_direction: 90.0,
            ..AtmosphereProfile::default()
        }
    }

    pub fn create_mountain_snow() -> AtmosphereProfile {
        AtmosphereProfile {
            preset_id: Self::MOUNTAIN_SNOW_ID.into(),
            name: "高山霜雪".into(),
            english_name: "Mountain Snow".into(),
            description: "冰天雪地，寒風刺骨。大氣瀰漫晶瑩冰晶，水體泛起冰川碧藍，地面與遠山覆蓋霜雪清輝。".into(),
            boden: BodenIniData {
                water_bump_amplitude: 224,
                water_bump_frequency: 4,
                water_warp_shift: 14,
                water_color: "0xffe8cc".into(),
                rain_drops_on_water: false,
                flash_propability: 0,
                snow_alr_index: 837,
                snow_shadow_index: 152,
                snow_shadow_size: 7,
                day_start_time: 6.0,
                day_end_time: 20.0,
                shadow_mesh_mode: 0,
                ..BodenIniData::default()
            },
            fog_enabled: true,
            fog_start_distance: 25.0,
            fog_end_distance: 95.0,
            fog_density: 0.48,
            fog_color: [0.82, 0.88, 0.98],
            sky_scattering_color: [0.90, 0.95, 1.15],
            ambient_tint: [0.92, 0.96, 1.10],
            exposure: 1.12,
            sun_light_color: [0.95, 0.98, 1.05],
            sun_light_direction: normalize([0.5, 0.75, 0.4]),
            precipitation: PrecipitationType::Snow,
            precipitation_intensity: 0.70,
            wind_speed: 12.0,
            wind_direction: 315.0,
            ..AtmosphereProfile::default()
        }
    }

    pub fn all() -> Vec<AtmosphereProfile> {
        vec![
            Self::create_clear_sky(),
            Self::create_storm(),
            Self::create_sunset_dusk(),
            Self::create_dense_fog(),
            Self::create_mountain_snow(),
        ]
    }

    pub fn try_get_preset(id: &str) -> Option<AtmosphereProfile> {
        Self::all().into_iter().find(|p| p.preset_id.eq_ignore_ascii_case(id))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AtmosphereShaderUniforms {
    pub effective_ambient: Vec3,
    pub fog_color: Vec3,
    pub fog_params: Vec4,
    pub sky_scattering: Vec3,
    pub sun_color: Vec3,
    pub sun_direction: Vec3,
    pub water_dynamics: Vec4,
    pub flash_intensity: f32,
    pub simulation_time: f32,
}

#[derive(Clone, Debug)]
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }
    fn next_f32(&mut self) -> f32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.state >> 40) as f32) / 16777216.0
    }
}

#[derive(Clone, Debug)]
pub struct AtmosphereLightingBridge {
    simulation_time: f32,
    flash_intensity: f32,
    next_flash_timer: f32,
    rng: SimpleRng,
}

impl Default for AtmosphereLightingBridge {
    fn default() -> Self {
        Self {
            simulation_time: 0.0,
            flash_intensity: 0.0,
            next_flash_timer: 5.0,
            rng: SimpleRng::new(42),
        }
    }
}

impl AtmosphereLightingBridge {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn simulation_time(&self) -> f32 {
        self.simulation_time
    }

    pub fn flash_intensity(&self) -> f32 {
        self.flash_intensity
    }

    pub fn update(&mut self, delta_time_seconds: f32, profile: &AtmosphereProfile) {
        if delta_time_seconds <= 0.0 {
            return;
        }
        self.simulation_time += delta_time_seconds;

        if self.flash_intensity > 0.0 {
            self.flash_intensity = (self.flash_intensity - delta_time_seconds * 6.5).max(0.0);
        }

        let flash_prob = profile.boden.flash_propability;
        if flash_prob > 0 {
            self.next_flash_timer -= delta_time_seconds;
            if self.next_flash_timer <= 0.0 {
                let average_interval = (1000.0 / flash_prob as f32).max(0.5);
                self.next_flash_timer = average_interval * (0.5 + self.rng.next_f32());
                self.flash_intensity = 0.8 + self.rng.next_f32() * 0.2;
            }
        } else {
            self.flash_intensity = 0.0;
            self.next_flash_timer = 5.0;
        }
    }

    pub fn trigger_manual_flash(&mut self, intensity: f32) {
        self.flash_intensity = intensity.clamp(0.0, 1.0);
    }

    pub fn compute_uniforms(
        &self,
        profile: &AtmosphereProfile,
        base_ambient: Vec3,
    ) -> AtmosphereShaderUniforms {
        let mut lit_ambient = [
            base_ambient[0] * profile.ambient_tint[0] * profile.exposure,
            base_ambient[1] * profile.ambient_tint[1] * profile.exposure,
            base_ambient[2] * profile.ambient_tint[2] * profile.exposure,
        ];
        if self.flash_intensity > 0.001 {
            let boost = self.flash_intensity * 0.75;
            lit_ambient[0] = (lit_ambient[0] + boost).min(1.6);
            lit_ambient[1] = (lit_ambient[1] + boost).min(1.6);
            lit_ambient[2] = (lit_ambient[2] + boost).min(1.6);
        }
        let boden = &profile.boden;
        let water_dynamics = [
            boden.water_bump_amplitude as f32,
            boden.water_bump_frequency as f32,
            boden.water_warp_shift as f32,
            if boden.rain_drops_on_water { 1.0 } else { 0.0 },
        ];
        let fog_params = [
            profile.fog_start_distance,
            profile.fog_end_distance,
            profile.fog_density,
            if profile.fog_enabled { 1.0 } else { 0.0 },
        ];
        AtmosphereShaderUniforms {
            effective_ambient: lit_ambient,
            fog_color: profile.fog_color,
            fog_params,
            sky_scattering: profile.sky_scattering_color,
            sun_color: profile.sun_light_color,
            sun_direction: profile.sun_light_direction,
            water_dynamics,
            flash_intensity: self.flash_intensity,
            simulation_time: self.simulation_time,
        }
    }

    pub const GLSL_ATMOSPHERE_HEADER: &'static str = r#"// 大氣與天氣系統動態 Uniforms (由 AtmosphereLightingBridge 提供)
uniform int uAtmosphereFogEnabled;
uniform vec4 uAtmosphereFogParams; // x: startDistance, y: endDistance, z: density, w: unused
uniform vec3 uAtmosphereFogColor;
uniform vec3 uAtmosphereSkyScattering;
uniform float uAtmosphereFlashIntensity;
uniform vec4 uAtmosphereWaterDynamics; // x: bumpAmp, y: bumpFreq, z: warpShift, w: rainDrops
uniform float uAtmosphereTime;

// 空氣透視與距離迷霧混合函式
vec3 applyAtmosphereFog(vec3 litColor, float viewDist) {
    if (uAtmosphereFogEnabled == 0) return litColor;
    float factor = clamp((viewDist - uAtmosphereFogParams.x) / max(0.001, uAtmosphereFogParams.y - uAtmosphereFogParams.x), 0.0, 1.0);
    factor = factor * uAtmosphereFogParams.z;
    return mix(litColor, uAtmosphereFogColor, factor);
}

// 閃電瞬時強光增益
vec3 applyAtmosphereFlash(vec3 litColor) {
    return litColor + vec3(uAtmosphereFlashIntensity * 0.7);
}
"#;

    pub const GLSL_WATER_DYNAMICS: &'static str = r#"vec2 computeWaterRippleOffset(vec2 worldPos, float time, float amplitude, float frequency) {
    float phase = time * (frequency * 0.4);
    float wave1 = sin(worldPos.x * 0.15 + phase) * cos(worldPos.y * 0.15 + phase * 0.8);
    float wave2 = sin(worldPos.x * 0.3 - phase * 1.2) * cos(worldPos.y * 0.25 + phase * 1.1);
    float normAmp = amplitude / 1024.0 * 0.08;
    return vec2(wave1 + wave2 * 0.5, wave2 + wave1 * 0.5) * normAmp;
}
"#;
}

#[derive(Clone, Debug)]
pub struct AtmosphereEditSession {
    current: AtmosphereProfile,
    baseline: AtmosphereProfile,
}

impl Default for AtmosphereEditSession {
    fn default() -> Self {
        let p = WeatherPresets::create_clear_sky();
        Self {
            current: p.clone(),
            baseline: p,
        }
    }
}

impl AtmosphereEditSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_initial(initial: Option<AtmosphereProfile>) -> Self {
        let cur = initial.unwrap_or_else(WeatherPresets::create_clear_sky);
        let base = cur.clone();
        Self {
            current: cur,
            baseline: base,
        }
    }

    pub fn module_id(&self) -> &'static str {
        "atmosphere"
    }

    pub fn is_dirty(&self) -> bool {
        !self.current.value_equals(&self.baseline)
    }

    pub fn current(&self) -> &AtmosphereProfile {
        &self.current
    }

    pub fn load(&mut self, snapshot: AtmosphereProfile) {
        self.current = snapshot;
        self.accept_changes();
    }

    pub fn capture(&self) -> AtmosphereProfile {
        self.current.clone()
    }

    pub fn accept_changes(&mut self) {
        self.baseline = self.current.clone();
    }

    pub fn reset(&mut self) {
        self.current = self.baseline.clone();
    }

    pub fn apply_preset(&mut self, preset_id: &str) {
        if let Some(preset) = WeatherPresets::try_get_preset(preset_id) {
            let orig_waterlevel = self.current.boden.waterlevel;
            let orig_heightstep = self.current.boden.heightmapstep;

            self.current = preset;
            self.current.boden.waterlevel = orig_waterlevel;
            self.current.boden.heightmapstep = orig_heightstep;
        }
    }

    pub fn set_water_bump(&mut self, amplitude: i32, frequency: i32, warp_shift: i32) {
        self.current.boden.water_bump_amplitude = amplitude.clamp(0, 1024);
        self.current.boden.water_bump_frequency = frequency.clamp(1, 16);
        self.current.boden.water_warp_shift = warp_shift.clamp(8, 20);
    }

    pub fn set_water_color(&mut self, hex_color: &str) {
        let trimmed = hex_color.trim();
        if !trimmed.is_empty() {
            self.current.boden.water_color = trimmed.to_string();
        }
    }

    pub fn set_flash_probability(&mut self, flash_probability: i32) {
        self.current.boden.flash_propability = flash_probability.clamp(0, 1000);
    }

    pub fn set_day_night_times(&mut self, day_start_time: f32, day_end_time: f32) {
        self.current.boden.day_start_time = day_start_time.clamp(0.0, 24.0);
        self.current.boden.day_end_time = day_end_time.clamp(0.0, 24.0);
    }

    pub fn set_fog(
        &mut self,
        enabled: bool,
        start_distance: f32,
        end_distance: f32,
        density: f32,
        color: Vec3,
    ) {
        self.current.fog_enabled = enabled;
        let start = start_distance.max(0.0);
        self.current.fog_start_distance = start;
        self.current.fog_end_distance = end_distance.max(start + 0.1);
        self.current.fog_density = density.clamp(0.0, 1.0);
        self.current.fog_color = [
            color[0].clamp(0.0, 1.0),
            color[1].clamp(0.0, 1.0),
            color[2].clamp(0.0, 1.0),
        ];
    }

    pub fn set_lighting_modulation(&mut self, ambient_tint: Vec3, exposure: f32) {
        self.current.ambient_tint = [
            ambient_tint[0].clamp(0.0, 2.0),
            ambient_tint[1].clamp(0.0, 2.0),
            ambient_tint[2].clamp(0.0, 2.0),
        ];
        self.current.exposure = exposure.clamp(0.2, 3.0);
    }

    pub fn set_precipitation(&mut self, precip_type: PrecipitationType, intensity: f32) {
        self.current.precipitation = precip_type;
        self.current.precipitation_intensity = intensity.clamp(0.0, 1.0);
        self.current.boden.rain_drops_on_water =
            precip_type == PrecipitationType::Rain && intensity > 0.1;
        self.current.boden.snow_alr_index = if precip_type == PrecipitationType::Snow {
            1
        } else {
            -1
        };
    }
}
