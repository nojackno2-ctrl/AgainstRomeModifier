//! AI map plan models, normalization and serialization.

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use crate::scope::AiMapEditScope;

pub const MAX_FEATURES: usize = 24;

pub const FEATURE_TYPES: &[&str] = &[
    "hill", "mountain", "valley", "plateau", "lake", "ridge", "river", "material", "blocked", "passable",
];

pub fn get_location_coordinates(name: &str) -> Option<(i32, i32)> {
    match name.trim().to_ascii_lowercase().as_str() {
        "center" => Some((32, 32)),
        "north" => Some((32, 9)),
        "north-east" => Some((52, 12)),
        "east" => Some((55, 32)),
        "south-east" => Some((52, 52)),
        "south" => Some((32, 55)),
        "south-west" => Some((12, 52)),
        "west" => Some((9, 32)),
        "north-west" => Some((12, 12)),
        "north-center" => Some((32, 20)),
        "east-center" => Some((44, 32)),
        "south-center" => Some((32, 44)),
        "west-center" => Some((20, 32)),
        _ => None,
    }
}

pub const LOCATION_NAMES: &[&str] = &[
    "center", "north", "north-east", "east", "south-east", "south", "south-west", "west",
    "north-west", "north-center", "east-center", "south-center", "west-center",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiMapPlan {
    #[serde(skip)]
    pub edit_scope: Option<AiMapEditScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(rename = "baseHeight", default, skip_serializing_if = "Option::is_none")]
    pub base_height: Option<i32>,
    #[serde(rename = "baseMaterial", default, skip_serializing_if = "Option::is_none")]
    pub base_material: Option<String>,
    #[serde(default)]
    pub features: Vec<AiMapFeature>,
}

impl Default for AiMapPlan {
    fn default() -> Self {
        Self {
            edit_scope: None,
            summary: None,
            base_height: None,
            base_material: None,
            features: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AiMapFeature {
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub feature_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(rename = "toLocation", default, skip_serializing_if = "Option::is_none")]
    pub to_location: Option<String>,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x2: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y2: Option<i32>,
    #[serde(default)]
    pub radius: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
}

impl Default for AiMapFeature {
    fn default() -> Self {
        Self {
            feature_type: None,
            location: None,
            to_location: None,
            x: 0,
            y: 0,
            x2: None,
            y2: None,
            radius: 0,
            amount: None,
            material: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AiMapApplyResult {
    pub height_samples_changed: usize,
    pub material_strokes: usize,
    pub rejected_material_strokes: usize,
    pub collision_pixels_changed: usize,
    pub operation_log: Vec<String>,
}

impl AiMapApplyResult {
    pub fn new(
        height_samples_changed: usize,
        material_strokes: usize,
        rejected_material_strokes: usize,
        collision_pixels_changed: usize,
        operation_log: Vec<String>,
    ) -> Self {
        Self {
            height_samples_changed,
            material_strokes,
            rejected_material_strokes,
            collision_pixels_changed,
            operation_log,
        }
    }

    pub fn operation_count(&self) -> usize {
        self.operation_log.len()
    }
}

impl AiMapPlan {
    /// Parse AI response (tolerant to markdown fences and surrounding text),
    /// normalize all values to legal ranges, or return Err.
    pub fn parse(response: &str, material_ids: &[&str]) -> Result<Self, String> {
        let trimmed = response.trim();
        if trimmed.is_empty() {
            return Err("AI 沒有回傳內容。".to_string());
        }

        let start = response.find('{');
        let end = response.rfind('}');
        let (start, end) = match (start, end) {
            (Some(s), Some(e)) if e > s => (s, e),
            _ => return Err("AI 回應不是 JSON 地圖計畫。".to_string()),
        };

        let json_slice = &response[start..=end];
        let mut plan: AiMapPlan = serde_json::from_str(json_slice)
            .map_err(|e| format!("AI 回應的 JSON 無法解析：{}", e))?;

        plan.normalize(material_ids);

        if plan.features.is_empty() && plan.base_height.is_none() && plan.base_material.is_none() {
            return Err("AI 計畫沒有任何可套用的地形特徵。".to_string());
        }

        Ok(plan)
    }

    pub fn normalize(&mut self, material_ids: &[&str]) {
        let known_materials: HashMap<String, String> = material_ids
            .iter()
            .map(|id| (id.trim().to_ascii_lowercase(), id.trim().to_string()))
            .collect();

        let resolve_material = |mat: Option<&str>| -> Option<String> {
            mat.and_then(|m| known_materials.get(&m.trim().to_ascii_lowercase()).cloned())
        };

        self.base_height = self.base_height.map(|h| h.clamp(0, 255));
        self.base_material = resolve_material(self.base_material.as_deref());
        self.summary = self.summary.as_ref().map(|s| s.trim().to_string());

        let mut normalized_features = Vec::new();
        let mut seen = HashSet::new();

        for mut feature in std::mem::take(&mut self.features) {
            let ftype = match feature.feature_type.as_ref() {
                Some(t) => t.trim().to_ascii_lowercase(),
                None => continue,
            };

            if !FEATURE_TYPES.iter().any(|&allowed| allowed == ftype) {
                continue;
            }

            if let Some(loc) = feature.location.as_ref() {
                if let Some((x, y)) = get_location_coordinates(loc) {
                    feature.x = x;
                    feature.y = y;
                }
            }
            if let Some(to_loc) = feature.to_location.as_ref() {
                if let Some((x2, y2)) = get_location_coordinates(to_loc) {
                    feature.x2 = Some(x2);
                    feature.y2 = Some(y2);
                }
            }

            feature.feature_type = Some(ftype.clone());
            feature.x = feature.x.clamp(0, 63);
            feature.y = feature.y.clamp(0, 63);
            feature.x2 = feature.x2.map(|v| v.clamp(0, 63));
            feature.y2 = feature.y2.map(|v| v.clamp(0, 63));
            feature.radius = if feature.radius <= 0 { 4 } else { feature.radius }.clamp(1, 32);
            feature.amount = if ftype == "lake" || ftype == "river" {
                Some(feature.amount.unwrap_or(16).clamp(5, 30))
            } else {
                feature.amount.map(|amount| amount.clamp(0, 255))
            };
            feature.material = resolve_material(feature.material.as_deref());

            // Filter out invalid material features
            if ftype == "material" && feature.material.is_none() {
                continue;
            }

            // River and ridge must have different start and end points
            if ftype == "river" || ftype == "ridge" {
                match (feature.x2, feature.y2) {
                    (Some(x2), Some(y2)) if x2 != feature.x || y2 != feature.y => {}
                    _ => continue,
                }
            }

            // Deduplicate
            let dedup_key = (
                ftype,
                feature.x,
                feature.y,
                feature.x2,
                feature.y2,
                feature.radius,
                feature.amount,
                feature.material.clone(),
            );
            if seen.insert(dedup_key) {
                normalized_features.push(feature);
                if normalized_features.len() >= MAX_FEATURES {
                    break;
                }
            }
        }

        self.features = normalized_features;
    }

    /// JSON schema for Ollama structured format output
    pub fn json_schema(material_ids: &[&str]) -> serde_json::Value {
        let mut schema = serde_json::json!({
            "type": "object",
            "properties": {
                "summary": { "type": "string" },
                "baseHeight": { "type": "integer", "minimum": 0, "maximum": 255 },
                "baseMaterial": { "type": "string", "enum": material_ids },
                "features": {
                    "type": "array",
                    "maxItems": MAX_FEATURES,
                    "items": {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": FEATURE_TYPES },
                            "location": { "type": "string", "enum": LOCATION_NAMES },
                            "toLocation": { "type": "string", "enum": LOCATION_NAMES },
                            "x": { "type": "integer", "minimum": 0, "maximum": 63 },
                            "y": { "type": "integer", "minimum": 0, "maximum": 63 },
                            "x2": { "type": "integer", "minimum": 0, "maximum": 63 },
                            "y2": { "type": "integer", "minimum": 0, "maximum": 63 },
                            "radius": { "type": "integer", "minimum": 1, "maximum": 32 },
                            "amount": { "type": "integer", "minimum": 0, "maximum": 255 },
                            "material": { "type": "string", "enum": material_ids }
                        },
                        "required": ["type", "location", "radius"]
                    }
                }
            },
            "required": ["summary", "features"]
        });
        if material_ids.is_empty() {
            if let Some(properties) = schema
                .get_mut("properties")
                .and_then(serde_json::Value::as_object_mut)
            {
                properties.remove("baseMaterial");
                if let Some(feature_properties) = properties
                    .get_mut("features")
                    .and_then(|features| features.get_mut("items"))
                    .and_then(|items| items.get_mut("properties"))
                    .and_then(serde_json::Value::as_object_mut)
                {
                    feature_properties.remove("material");
                }
            }
        }
        schema
    }
}
