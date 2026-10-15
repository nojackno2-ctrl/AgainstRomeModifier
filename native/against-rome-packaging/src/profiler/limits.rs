pub struct MapBudgetLimits;

impl MapBudgetLimits {
    // 1. Entity & Pool Limits
    pub const MAX_OBJECTS_DAT_SLOTS: usize = 14_000;
    pub const REDLINE_OBJECTS_DAT_SLOTS: usize = 10_000;
    pub const RECOMMENDED_OBJECTS_DAT_SLOTS: usize = 8_000;

    pub const MAX_POSITION_DAT_RECORDS: usize = 33_000;
    pub const REDLINE_POSITION_DAT_RECORDS: usize = 24_000;

    pub const RECOMMENDED_SCENARIO_SPAWNS: usize = 500;
    pub const REDLINE_SCENARIO_SPAWNS: usize = 1_000;

    pub const MAX_LAGER_SLOTS: usize = 3_200;
    pub const MAX_BIG_LAGER_SLOTS: usize = 800;
    pub const MAX_PARTICLE_SLOTS: usize = 64;
    pub const MAX_EXPLOS_SLOTS: usize = 46;
    pub const MAX_FLASH_SLOTS: usize = 16;
    pub const MAX_WAY_POINTS: usize = 256;

    // 2. Rendering & Draw Call Limits
    pub const MAX_GLOBAL_RUNTIME_LIGHTS: usize = 1_024;
    pub const MAX_ACTIVE_LIGHTS_IN_VIEW: usize = 64;
    pub const HARD_LIMIT_LIGHTS_PER_CLUSTER: usize = 64;
    pub const REDLINE_LIGHTS_PER_CLUSTER: usize = 48;
    pub const RECOMMENDED_LIGHTS_PER_CLUSTER: usize = 32;

    pub const MAX_SPRITE_QUADS_PER_BATCH: usize = 16_384;
    pub const SAFE_SPRITE_QUADS_PER_DRAW_CALL: usize = 10_922;
    pub const RECOMMENDED_ATLAS_PAGES: usize = 2;
    pub const REDLINE_ATLAS_PAGES: usize = 4;
    pub const BENCHMARK_SHADOW_QUADS: usize = 2_050;
    pub const REDLINE_SHADOW_QUADS: usize = 5_000;

    // 3. Map Geometry & Pathfinding Limits
    pub const MAP_GRID_DIMENSION: usize = 256;
    pub const MAP_WORLD_SIZE: f32 = 16384.0;
    pub const WORLD_UNITS_PER_TILE: f32 = 64.0;
    pub const HEIGHT_GRID_DIMENSION: usize = 257;
    pub const TOTAL_PATHFINDING_NODES: usize = 65_536;

    pub const CHOKEPOINT_WIDTH_THRESHOLD_CELLS: usize = 2;
    pub const REDLINE_CHOKEPOINTS: usize = 30;
    pub const DEAD_END_MIN_DEPTH_CELLS: usize = 8;
    pub const REDLINE_DEAD_ENDS: usize = 15;
    pub const REDLINE_ISOLATED_REGIONS: usize = 10;
}
