use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormationLayoutSegment {
    pub start: (f32, f32),
    pub end: (f32, f32),
}

impl FormationLayoutSegment {
    pub fn new(start: (f32, f32), end: (f32, f32)) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormationLayoutDefinition {
    segments: Vec<FormationLayoutSegment>,
}

impl FormationLayoutDefinition {
    pub fn new(segments: Vec<FormationLayoutSegment>) -> Result<Self, String> {
        if segments.is_empty() || segments.len() > 20 {
            return Err("隊形線段數必須介於 1 與 20。".to_string());
        }
        for s in &segments {
            if !s.start.0.is_finite()
                || !s.start.1.is_finite()
                || !s.end.0.is_finite()
                || !s.end.1.is_finite()
            {
                return Err("隊形座標必須為有限數值。".to_string());
            }
        }
        Ok(Self { segments })
    }

    pub fn segments(&self) -> &[FormationLayoutSegment] {
        &self.segments
    }

    /// 原版 id 0 All_Haufen 的九條有效線段（離線 formdef.dau）。
    pub fn native_default() -> Self {
        Self {
            segments: vec![
                FormationLayoutSegment::new((0.05, -0.05), (0.20, 0.05)),
                FormationLayoutSegment::new((-0.20, 0.15), (-0.05, 0.05)),
                FormationLayoutSegment::new((0.20, -0.20), (0.05, -0.15)),
                FormationLayoutSegment::new((-0.10, -0.05), (-0.20, -0.20)),
                FormationLayoutSegment::new((0.30, -0.15), (0.40, 0.10)),
                FormationLayoutSegment::new((-0.25, 0.25), (-0.40, 0.05)),
                FormationLayoutSegment::new((0.05, 0.30), (0.30, 0.20)),
                FormationLayoutSegment::new((0.34, -0.25), (0.10, -0.34)),
                FormationLayoutSegment::new((-0.20, -0.34), (-0.40, -0.15)),
            ],
        }
    }

    pub fn matches_embedded(&self) -> bool {
        // C# uses record-struct SequenceEqual, which compares each f32 exactly.
        self.segments == Self::native_default().segments
    }

    /// 解析已解壓之 [FormationDefault] CSV 文字，並讀取指定隊形 id（預設 0）。
    pub fn from_form_def_text(text: &str, id: i32) -> Result<Self, String> {
        if !(0..100).contains(&id) {
            return Err("隊形 ID 超出範圍。".to_string());
        }

        let mut in_section = false;
        let mut result: Option<Self> = None;

        for raw in text.lines() {
            let line = raw.split(';').next().unwrap_or("").trim();
            if line.starts_with('[') {
                in_section = line.eq_ignore_ascii_case("[FormationDefault]");
                continue;
            }
            if !in_section || line.is_empty() {
                continue;
            }

            let fields: Vec<&str> = line.split(',').collect();
            let row_id: i32 = match fields.first().and_then(|f| f.trim().parse().ok()) {
                Some(v) => v,
                None => continue,
            };

            if row_id != id {
                continue;
            }

            if result.is_some() {
                return Err("隊形 ID 重複。".to_string());
            }

            if fields.len() < 83 {
                return Err("不完整或無效的隊形幾何欄位。".to_string());
            }

            let active: i32 = fields[1]
                .trim()
                .parse()
                .map_err(|_| "不完整或無效的隊形幾何欄位。".to_string())?;
            if !(1..=255).contains(&active) {
                return Err("不完整或無效的隊形幾何欄位。".to_string());
            }

            let count: usize = fields[2]
                .trim()
                .parse()
                .map_err(|_| "不完整或無效的隊形幾何欄位。".to_string())?;
            if !(1..=20).contains(&count) {
                return Err("不完整或無效的隊形幾何欄位。".to_string());
            }

            let parse_val = |index: usize| -> Result<f32, String> {
                fields
                    .get(index)
                    .and_then(|s| s.trim().parse::<f32>().ok())
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| "無效的隊形座標。".to_string())
            };

            let mut segments = Vec::with_capacity(count);
            for i in 0..count {
                let offset = 3 + i * 4;
                let sx = parse_val(offset)?;
                let sy = parse_val(offset + 1)?;
                let ex = parse_val(offset + 2)?;
                let ey = parse_val(offset + 3)?;
                segments.push(FormationLayoutSegment::new((sx, sy), (ex, ey)));
            }

            result = Some(Self::new(segments)?);
        }

        result.ok_or_else(|| format!("找不到隊形 ID {id}。"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OffsetPoint {
    pub x: f32,
    pub z: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CountLayout {
    pub count: usize,
    pub offsets: Vec<OffsetPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutReport {
    pub sha256: String,
    #[serde(rename = "segmentCount")]
    pub segment_count: usize,
    #[serde(rename = "matchesEmbedded")]
    pub matches_embedded: bool,
    pub layouts: Vec<CountLayout>,
}

pub struct FormationLayout;

impl FormationLayout {
    pub const INITIAL_SPACING: f32 = 320.0;
    pub const MAXIMUM_SCENARIO_MEMBERS: usize = 20;

    pub fn banner_offset() -> (f32, f32) {
        (0.0, 0.0)
    }

    /// 依士兵數量取樣隊形幾何偏移，回傳成員世界 X/Z 座標。
    pub fn create(
        count: usize,
        angle_degrees: f32,
        definition: &FormationLayoutDefinition,
        spacing: f32,
    ) -> Result<Vec<OffsetPoint>, String> {
        if count > Self::MAXIMUM_SCENARIO_MEMBERS {
            return Err("成員數超出上限。".to_string());
        }
        if !angle_degrees.is_finite() {
            return Err("角度必須為有限數值。".to_string());
        }
        if !spacing.is_finite() || spacing <= 0.0 {
            return Err("間距必須為大於零的有限數值。".to_string());
        }
        if count == 0 {
            return Ok(Vec::new());
        }

        let radians = (90.0 - (angle_degrees % 360.0) as f64) * std::f64::consts::PI / 180.0;
        let sine = radians.sin();
        let cosine = radians.cos();

        let ratio = definition.segments.len() as f32 / count as f32;
        let mut result = Vec::with_capacity(count);

        for member in 0..count {
            let position = ratio * (member as f32 + 0.5);
            let segment_index = (position.floor() as usize).min(definition.segments.len() - 1);
            let segment = definition.segments[segment_index];
            let fraction = position - segment_index as f32;

            let local_x = segment.start.0 * (1.0 - fraction) + segment.end.0 * fraction;
            let local_y = segment.start.1 * (1.0 - fraction) + segment.end.1 * fraction;

            let x = -local_x as f64 * spacing as f64;
            let z = local_y as f64 * spacing as f64;

            let world_x = (x * cosine - z * sine) as f32;
            let world_z = (x * sine + z * cosine) as f32;

            result.push(OffsetPoint {
                x: world_x,
                z: world_z,
            });
        }

        Ok(result)
    }
}
