use chrono::{DateTime, Utc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapHealthGrade {
    Healthy,
    Caution,
    Warning,
    Critical,
    EngineExceeded,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapHealthBottleneck {
    pub category: String,
    pub code: String,
    pub chinese_description: String,
    pub english_description: String,
    pub world_x: Option<f32>,
    pub world_z: Option<f32>,
    pub is_engine_limit_violation: bool,
}

impl MapHealthBottleneck {
    pub fn new(
        category: impl Into<String>,
        code: impl Into<String>,
        chinese: impl Into<String>,
        english: impl Into<String>,
        is_engine_limit_violation: bool,
    ) -> Self {
        Self {
            category: category.into(),
            code: code.into(),
            chinese_description: chinese.into(),
            english_description: english.into(),
            world_x: None,
            world_z: None,
            is_engine_limit_violation,
        }
    }

    pub fn with_location(mut self, world_x: f32, world_z: f32) -> Self {
        self.world_x = Some(world_x);
        self.world_z = Some(world_z);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectSlotBudget {
    pub total_active_objects: usize,
    pub objects_dat_limit: usize,
    pub objects_usage_percent: f32,
    pub estimated_position_records: usize,
    pub position_limit: usize,
    pub position_usage_percent: f32,
    pub scenario_spawns_count: usize,
    pub landscape_count: usize,
    pub building_count: usize,
    pub unit_count: usize,
    pub fx_count: usize,
    pub other_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LightClusterHotspot {
    pub world_x: f32,
    pub world_z: f32,
    pub light_count: usize,
    pub radius: f32,
    pub exceeds_hard_limit: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LightDensityBudget {
    pub total_scene_lights: usize,
    pub peak_cluster_lights: usize,
    pub peak_density_ratio: f32,
    pub hotspots: Vec<LightClusterHotspot>,
    pub cluster_grid_resolution: usize,
    pub density_grid: Vec<Vec<f32>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DrawCallBudget {
    pub estimated_total_draw_calls: usize,
    pub terrain_draw_calls: usize,
    pub sprite_batch_draw_calls: usize,
    pub shadow_batch_draw_calls: usize,
    pub estimated_atlas_pages: usize,
    pub estimated_atlas_switches: usize,
    pub batch_fragmentation_score: f32,
    pub total_sprite_quads: usize,
    pub total_shadow_quads: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChokepointLocation {
    pub cell_x: usize,
    pub cell_z: usize,
    pub world_x: f32,
    pub world_z: f32,
    pub passage_width_cells: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PathfindingBudget {
    pub obstacle_density_percent: f32,
    pub passable_regions_count: usize,
    pub chokepoint_count: usize,
    pub chokepoints: Vec<ChokepointLocation>,
    pub dead_end_count: usize,
    pub pathfinding_complexity_score: i32,
    pub complexity_rating: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapBudgetReport {
    pub timestamp: DateTime<Utc>,
    pub overall_grade: MapHealthGrade,
    pub overall_score: i32,
    pub object_budget: ObjectSlotBudget,
    pub light_budget: LightDensityBudget,
    pub draw_budget: DrawCallBudget,
    pub path_budget: PathfindingBudget,
    pub bottlenecks: Vec<MapHealthBottleneck>,
}
