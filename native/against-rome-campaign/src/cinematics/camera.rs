use serde::{Deserialize, Serialize};
use std::path::Path;

pub use super::previewer::{CameraFlightPreviewer, CameraPreviewState};
pub use super::spline::{CameraEasingType, CameraPose, CameraTrackSplinePlanner, CameraWaypoint};

/// 開局相機使用的底層原生原語。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StartCameraPrimitive {
    /// 使用原版戰役/多人地圖開局呼叫的 s_centerToPos(x, z) 搭配 s_lgcSetEngineZoom(zoom)。
    CenterToPos = 0,
    /// 使用底層原生原語 s_lgcSetEnginePos(x, y, z) 搭配 s_lgcSetEngineZoom(zoom)。
    DirectEnginePos = 1,
}

impl Serialize for StartCameraPrimitive {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i32(*self as i32)
    }
}

impl<'de> Deserialize<'de> for StartCameraPrimitive {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = StartCameraPrimitive;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("an integer (0 or 1) or string name ('CenterToPos' or 'DirectEnginePos')")
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match v {
                    0 => Ok(StartCameraPrimitive::CenterToPos),
                    1 => Ok(StartCameraPrimitive::DirectEnginePos),
                    _ => Err(E::custom(format!("invalid StartCameraPrimitive integer value: {v}"))),
                }
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match v {
                    0 => Ok(StartCameraPrimitive::CenterToPos),
                    1 => Ok(StartCameraPrimitive::DirectEnginePos),
                    _ => Err(E::custom(format!("invalid StartCameraPrimitive integer value: {v}"))),
                }
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match v {
                    "CenterToPos" | "centerToPos" => Ok(StartCameraPrimitive::CenterToPos),
                    "DirectEnginePos" | "directEnginePos" => Ok(StartCameraPrimitive::DirectEnginePos),
                    _ => Err(E::custom(format!("unknown StartCameraPrimitive variant: {v}"))),
                }
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

impl Default for StartCameraPrimitive {
    fn default() -> Self {
        Self::CenterToPos
    }
}

/// 地圖開局相機設定 (Map Start Camera)，由企劃在編輯器中指定開局視角焦點與縮放。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MapStartCamera {
    pub enabled: bool,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub zoom: f64,
    pub primitive: StartCameraPrimitive,
}

impl MapStartCamera {
    pub const FILE_NAME: &'static str = "arm_start_camera.json";
    pub const DEFAULT_WORLD_X: f64 = 8192.0;
    pub const DEFAULT_WORLD_Z: f64 = 8192.0;
    pub const DEFAULT_ELEVATION: f64 = 0.0;
    pub const DEFAULT_ZOOM: f64 = 5.0;

    pub fn validate(&self) -> Result<(), String> {
        if !self.x.is_finite() || !(0.0..=16383.0).contains(&self.x) {
            return Err("X 座標必須在世界地圖範圍 0..16383 內。".into());
        }
        if !self.z.is_finite() || !(0.0..=16383.0).contains(&self.z) {
            return Err("Z 座標必須在世界地圖範圍 0..16383 內。".into());
        }
        if !self.y.is_finite() || self.y.abs() > f32::MAX as f64 {
            return Err("高度必須為有限浮點數。".into());
        }
        if !self.zoom.is_finite() || !(0.0..=9.0).contains(&self.zoom) {
            return Err("原生縮放範圍必須為 0..9。".into());
        }
        Ok(())
    }

    pub fn load(map_directory: &Path) -> Self {
        let path = map_directory.join(Self::FILE_NAME);
        if !path.exists() {
            return Self {
                enabled: false,
                ..Default::default()
            };
        }
        let data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(_) => {
                return Self {
                    enabled: false,
                    ..Default::default()
                }
            }
        };
        serde_json::from_slice(&data).unwrap_or(Self {
            enabled: false,
            ..Default::default()
        })
    }

    pub fn save(&self, map_directory: &Path) -> Result<(), String> {
        self.validate()?;
        let path = map_directory.join(Self::FILE_NAME);
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, bytes).map_err(|e| e.to_string())
    }
}

impl Default for MapStartCamera {
    fn default() -> Self {
        Self {
            enabled: true,
            x: Self::DEFAULT_WORLD_X,
            y: Self::DEFAULT_ELEVATION,
            z: Self::DEFAULT_WORLD_Z,
            zoom: Self::DEFAULT_ZOOM,
            primitive: StartCameraPrimitive::CenterToPos,
        }
    }
}

impl From<&MapStartCamera> for against_rome_scenario::StartCamera {
    fn from(c: &MapStartCamera) -> Self {
        Self {
            enabled: c.enabled,
            x: c.x,
            y: c.y,
            z: c.z,
            zoom: c.zoom,
            primitive: c.primitive as i32,
        }
    }
}

impl From<MapStartCamera> for against_rome_scenario::StartCamera {
    fn from(c: MapStartCamera) -> Self {
        Self::from(&c)
    }
}

impl From<&against_rome_scenario::StartCamera> for MapStartCamera {
    fn from(c: &against_rome_scenario::StartCamera) -> Self {
        Self {
            enabled: c.enabled,
            x: c.x,
            y: c.y,
            z: c.z,
            zoom: c.zoom,
            primitive: if c.primitive == 1 {
                StartCameraPrimitive::DirectEnginePos
            } else {
                StartCameraPrimitive::CenterToPos
            },
        }
    }
}

impl From<against_rome_scenario::StartCamera> for MapStartCamera {
    fn from(c: against_rome_scenario::StartCamera) -> Self {
        Self::from(&c)
    }
}
