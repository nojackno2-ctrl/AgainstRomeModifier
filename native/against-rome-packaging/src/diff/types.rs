use against_rome_scenario::model::Event;
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffStatus {
    Unchanged,
    Added,
    Deleted,
    Modified,
    Moved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RollbackLayerFlags(pub u32);

impl RollbackLayerFlags {
    pub const NONE: Self = Self(0);
    pub const HEIGHT: Self = Self(1 << 0);
    pub const TEXTURES: Self = Self(1 << 1);
    pub const COLLISION: Self = Self(1 << 2);
    pub const OBJECTS: Self = Self(1 << 3);
    pub const ALL: Self = Self(Self::HEIGHT.0 | Self::TEXTURES.0 | Self::COLLISION.0 | Self::OBJECTS.0);

    pub fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for RollbackLayerFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DiffObjectItem {
    pub id: Uuid,
    pub slot: i32,
    pub uid: u32,
    pub alias: String,
    pub type_id: i32,
    pub team: i32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rotation: f32,
    pub unit_count: i32,
    pub prebuilt: bool,
}

impl DiffObjectItem {
    pub fn new(
        id: Uuid,
        slot: i32,
        uid: u32,
        alias: impl Into<String>,
        type_id: i32,
        team: i32,
        x: f32,
        y: f32,
        z: f32,
        rotation: f32,
        unit_count: i32,
        prebuilt: bool,
    ) -> Self {
        Self {
            id,
            slot,
            uid,
            alias: alias.into(),
            type_id,
            team,
            x,
            y,
            z,
            rotation,
            unit_count,
            prebuilt,
        }
    }

    pub fn distance_to(&self, other: &DiffObjectItem) -> f32 {
        let dx = self.x - other.x;
        let dz = self.z - other.z;
        (dx * dx + dz * dz).sqrt()
    }
}

#[derive(Clone, Debug)]
pub struct MapVersionSnapshot {
    pub version_label: String,
    pub tile_dimension: usize,
    pub height_dimension: usize,
    pub heights: Option<Vec<u8>>,
    pub height_step: f32,
    pub textures: Option<Vec<String>>,
    pub collision_dimension: usize,
    pub collision: Option<Vec<u8>>,
    pub objects: Option<Vec<DiffObjectItem>>,
    pub events: Option<Vec<Event>>,
}

impl MapVersionSnapshot {
    pub fn new(
        version_label: impl Into<String>,
        tile_dimension: usize,
        height_dimension: usize,
        heights: Option<Vec<u8>>,
        height_step: f32,
    ) -> Self {
        Self {
            version_label: version_label.into(),
            tile_dimension,
            height_dimension,
            heights,
            height_step,
            textures: None,
            collision_dimension: 0,
            collision: None,
            objects: None,
            events: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiffTerrainSampleChange {
    pub index: usize,
    pub before: u8,
    pub after: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeightVertexChange {
    pub vertex_x: usize,
    pub vertex_y: usize,
    pub old_height: u8,
    pub new_height: u8,
    pub delta: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeightDiffRecord {
    pub dimension: usize,
    pub delta_grid: Vec<f32>,
    pub changed_vertex_count: usize,
    pub max_raise: f32,
    pub max_lower: f32,
    pub volume_delta: f32,
    pub sample_changes: Vec<HeightVertexChange>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TileTextureChange {
    pub tile_x: usize,
    pub tile_y: usize,
    pub old_texture: String,
    pub new_texture: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextureDiffRecord {
    pub dimension: usize,
    pub changed_tile_count: usize,
    pub changed_tiles: Vec<TileTextureChange>,
    pub transitions: BTreeMap<(String, String), usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollisionTileChange {
    pub tile_x: usize,
    pub tile_y: usize,
    pub old_value: u8,
    pub new_value: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CollisionDiffRecord {
    pub dimension: usize,
    pub newly_blocked_count: usize,
    pub newly_cleared_count: usize,
    pub changes: Vec<CollisionTileChange>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectModification {
    pub baseline: DiffObjectItem,
    pub current: DiffObjectItem,
    pub distance_moved: f32,
    pub rotation_delta: f32,
    pub changed_attributes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectDiffRecord {
    pub added: Vec<DiffObjectItem>,
    pub deleted: Vec<DiffObjectItem>,
    pub modified: Vec<ObjectModification>,
}

impl ObjectDiffRecord {
    pub fn total_changes(&self) -> usize {
        self.added.len() + self.deleted.len() + self.modified.len()
    }
}

#[derive(Clone, Debug)]
pub struct EventModification {
    pub event_name: String,
    pub baseline: Event,
    pub current: Event,
    pub detail_changes: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct EventDiffRecord {
    pub added: Vec<Event>,
    pub deleted: Vec<Event>,
    pub modified: Vec<EventModification>,
}

impl EventDiffRecord {
    pub fn total_changes(&self) -> usize {
        self.added.len() + self.deleted.len() + self.modified.len()
    }
}

#[derive(Clone, Debug)]
pub struct MapDiffReport {
    pub compared_at: DateTime<Utc>,
    pub baseline_label: String,
    pub current_label: String,
    pub height: HeightDiffRecord,
    pub textures: TextureDiffRecord,
    pub collision: CollisionDiffRecord,
    pub objects: ObjectDiffRecord,
    pub events: EventDiffRecord,
}

impl MapDiffReport {
    pub fn has_differences(&self) -> bool {
        self.height.changed_vertex_count > 0
            || self.textures.changed_tile_count > 0
            || self.collision.newly_blocked_count > 0
            || self.collision.newly_cleared_count > 0
            || self.objects.total_changes() > 0
            || self.events.total_changes() > 0
    }

    pub fn generate_summary_text(&self) -> String {
        let mut s = String::new();
        s.push_str("【地圖版本比對報告】\n");
        s.push_str(&format!("基準版本 (Baseline): {}\n", self.baseline_label));
        s.push_str(&format!("當前版本 (Current): {}\n", self.current_label));
        s.push_str(&format!("比對時間: {}\n", self.compared_at.format("%Y-%m-%d %H:%M:%S")));
        s.push_str(&format!("總體狀態: {}\n\n", if self.has_differences() { "檢測到差異變更" } else { "兩版本完全一致" }));

        s.push_str("--- 1. 地形起伏 (Heightmap) ---\n");
        s.push_str(&format!("  變更頂點數: {} 個\n", self.height.changed_vertex_count));
        s.push_str(&format!("  最大高度抬升: +{:.1} (世界單位)\n", self.height.max_raise));
        s.push_str(&format!("  最大深度下陷: -{:.1} (世界單位)\n", self.height.max_lower));
        let sign = if self.height.volume_delta >= 0.0 { "+" } else { "" };
        s.push_str(&format!("  淨體積變化: {}{:.1}\n\n", sign, self.height.volume_delta));

        s.push_str("--- 2. 地表材質 (Floor Materials) ---\n");
        s.push_str(&format!("  變更格位數: {} 格\n", self.textures.changed_tile_count));
        if !self.textures.transitions.is_empty() {
            s.push_str("  主要材質轉移:\n");
            let mut trans_vec: Vec<_> = self.textures.transitions.iter().collect();
            trans_vec.sort_by(|a, b| b.1.cmp(a.1));
            for ((from, to), count) in trans_vec.into_iter().take(5) {
                s.push_str(&format!("    {} -> {}: {} 格\n", from, to, count));
            }
        }
        s.push('\n');

        s.push_str("--- 3. 通行碰撞 (Collision) ---\n");
        s.push_str(&format!("  新增阻擋格: {} 格\n", self.collision.newly_blocked_count));
        s.push_str(&format!("  新增通行格: {} 格\n\n", self.collision.newly_cleared_count));

        s.push_str("--- 4. 場景物件 (Level Objects) ---\n");
        s.push_str(&format!("  新增物件: {} 個\n", self.objects.added.len()));
        s.push_str(&format!("  刪除物件: {} 個\n", self.objects.deleted.len()));
        s.push_str(&format!("  修改/移動物件: {} 個\n\n", self.objects.modified.len()));

        s.push_str("--- 5. 劇本事件 (Scenario Events) ---\n");
        s.push_str(&format!("  新增事件: {} 個\n", self.events.added.len()));
        s.push_str(&format!("  刪除事件: {} 個\n", self.events.deleted.len()));
        s.push_str(&format!("  修改事件: {} 個\n", self.events.modified.len()));

        s
    }
}
