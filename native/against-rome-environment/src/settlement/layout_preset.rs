//! MapLayoutPreset: Portable JSON-compatible placement and nature layouts.

use crate::nature::engine::NatureAddition;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum MapLayoutKind {
    Placement,
    Nature,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapLayoutEntry {
    #[serde(rename = "type")]
    pub type_name: String,
    pub x: f32,
    pub z: f32,
    pub height_offset: f32,
    pub angle: f32,
    #[serde(default)]
    pub team: i32,
    #[serde(default)]
    pub count: i32,
}

impl MapLayoutEntry {
    pub fn new(type_name: impl Into<String>, x: f32, z: f32, height_offset: f32, angle: f32) -> Self {
        Self {
            type_name: type_name.into(),
            x,
            z,
            height_offset,
            angle,
            team: 0,
            count: 0,
        }
    }

    pub fn with_team_and_count(
        type_name: impl Into<String>,
        x: f32,
        z: f32,
        height_offset: f32,
        angle: f32,
        team: i32,
        count: i32,
    ) -> Self {
        Self {
            type_name: type_name.into(),
            x,
            z,
            height_offset,
            angle,
            team,
            count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MapLayoutPreset {
    pub version: i32,
    pub kind: MapLayoutKind,
    pub entries: Vec<MapLayoutEntry>,
}

impl MapLayoutPreset {
    pub fn new(kind: MapLayoutKind, entries: Vec<MapLayoutEntry>) -> Self {
        Self {
            version: 1,
            kind,
            entries,
        }
    }
}

pub struct MapLayoutPresets;

impl MapLayoutPresets {
    pub fn serialize(preset: &MapLayoutPreset) -> Result<String, String> {
        Self::validate(preset)?;
        serde_json::to_string_pretty(preset).map_err(|e| e.to_string())
    }

    pub fn deserialize(json: &str) -> Result<MapLayoutPreset, String> {
        if json.len() > 2_000_000 {
            return Err("Layout is too large.".to_string());
        }
        let preset: MapLayoutPreset = serde_json::from_str(json).map_err(|e| e.to_string())?;
        Self::validate(&preset)?;
        Ok(preset)
    }

    pub fn validate(preset: &MapLayoutPreset) -> Result<(), String> {
        if preset.version != 1 {
            return Err(format!("Unsupported layout version: {}", preset.version));
        }
        if preset.entries.is_empty() || preset.entries.len() > 4096 {
            return Err("Object count must be between 1 and 4096.".to_string());
        }

        for entry in &preset.entries {
            if entry.type_name.trim().is_empty() || entry.type_name.len() > 256 {
                return Err("Type name must be between 1 and 256 characters.".to_string());
            }
            if !entry.x.is_finite()
                || !entry.z.is_finite()
                || !entry.height_offset.is_finite()
                || !entry.angle.is_finite()
            {
                return Err("All transforms must be finite numbers.".to_string());
            }
            if entry.x.abs() > 16383.0 || entry.z.abs() > 16383.0 || entry.height_offset.abs() > 16383.0 {
                return Err("Coordinates must be in [-16383, 16383].".to_string());
            }
            if !(-1..=15).contains(&entry.team) {
                return Err("Team must be between -1 and 15.".to_string());
            }
            if !(0..=20).contains(&entry.count) {
                return Err("Count must be between 0 and 20.".to_string());
            }
            if preset.kind == MapLayoutKind::Nature && (entry.team != 0 || entry.count != 0) {
                return Err("Nature layout entries must have team=0 and count=0.".to_string());
            }
        }

        Ok(())
    }

    pub fn plan_nature(
        preset: &MapLayoutPreset,
        template_resolver: impl Fn(&str) -> Option<i32>,
        anchor_x: f32,
        anchor_z: f32,
        rotation: f32,
        ground_height: impl Fn(f32, f32) -> f32,
    ) -> Result<Vec<NatureAddition>, String> {
        Self::validate(preset)?;
        if preset.kind != MapLayoutKind::Nature {
            return Err("Expected a nature layout.".to_string());
        }

        let mut result = Vec::new();
        let rot_rad = rotation * std::f32::consts::PI / 180.0;
        let cos_r = rot_rad.cos();
        let sin_r = rot_rad.sin();

        for entry in &preset.entries {
            let type_id = template_resolver(&entry.type_name)
                .ok_or_else(|| format!("Missing nature template: {}", entry.type_name))?;

            let tx = entry.x * cos_r - entry.z * sin_r + anchor_x;
            let tz = entry.x * sin_r + entry.z * cos_r + anchor_z;
            let ty = ground_height(tx, tz) + entry.height_offset;
            let angle = (entry.angle + rotation) % 360.0;

            result.push(NatureAddition::new(
                type_id,
                entry.type_name.clone(),
                tx,
                ty,
                tz,
                angle * std::f32::consts::PI / 180.0,
            ));
        }

        Ok(result)
    }
}
