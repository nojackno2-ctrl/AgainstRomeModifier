//! DSL 巨集指令資料模型 (Macro Command Models)
//!
//! 定義指令引數、幾何區域、執行結果、上下文與執行報告。

use std::collections::HashMap;
use uuid::Uuid;

/// 表示矩形區域座標 (Tile 或 World 空間)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapRegionRect {
    pub min_x: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_z: i32,
}

impl MapRegionRect {
    pub fn new(x1: i32, z1: i32, x2: i32, z2: i32) -> Self {
        Self {
            min_x: x1.min(x2),
            min_z: z1.min(z2),
            max_x: x1.max(x2),
            max_z: z1.max(z2),
        }
    }

    pub fn width(&self) -> i32 {
        (self.max_x - self.min_x + 1).max(0)
    }

    pub fn height(&self) -> i32 {
        (self.max_z - self.min_z + 1).max(0)
    }

    pub fn area(&self) -> i32 {
        self.width() * self.height()
    }

    pub fn contains(&self, x: i32, z: i32) -> bool {
        x >= self.min_x && x <= self.max_x && z >= self.min_z && z <= self.max_z
    }
}

/// 表示圓形區域座標。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapRegionCircle {
    pub center_x: f32,
    pub center_z: f32,
    pub radius: f32,
}

impl MapRegionCircle {
    pub fn new(center_x: f32, center_z: f32, radius: f32) -> Self {
        Self {
            center_x,
            center_z,
            radius,
        }
    }

    pub fn contains(&self, x: f32, z: f32) -> bool {
        let dx = x - self.center_x;
        let dz = z - self.center_z;
        (dx * dx + dz * dz) <= (self.radius * self.radius)
    }
}

/// 控制台與腳本執行日誌等級。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Success,
    Warning,
    Error,
    Command,
}

/// 指令參數型別列舉。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandParameterType {
    String,
    Integer,
    Float,
    Boolean,
    Rectangle,
    Circle,
    TextureName,
    TemplateName,
    TeamId,
    Flag,
}

/// 指令參數定義中繼資料。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandParameter {
    pub name: String,
    pub param_type: CommandParameterType,
    pub desc_zh: String,
    pub desc_en: String,
    pub is_required: bool,
    pub default_value: Option<String>,
    pub possible_values: Option<Vec<String>>,
}

impl CommandParameter {
    pub fn new(
        name: impl Into<String>,
        param_type: CommandParameterType,
        desc_zh: impl Into<String>,
        desc_en: impl Into<String>,
        is_required: bool,
    ) -> Self {
        Self {
            name: name.into(),
            param_type,
            desc_zh: desc_zh.into(),
            desc_en: desc_en.into(),
            is_required,
            default_value: None,
            possible_values: None,
        }
    }

    pub fn with_default(mut self, def: impl Into<String>) -> Self {
        self.default_value = Some(def.into());
        self
    }

    pub fn with_possible(mut self, values: Vec<String>) -> Self {
        self.possible_values = Some(values);
        self
    }
}

/// 解析後的結構化指令物件。
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedCommand {
    pub raw_text: String,
    pub command_name: String,
    pub positional_args: Vec<String>,
    pub named_flags: HashMap<String, Option<String>>,
}

impl ParsedCommand {
    pub fn new(
        raw_text: impl Into<String>,
        command_name: impl Into<String>,
        positional_args: Vec<String>,
        named_flags: HashMap<String, Option<String>>,
    ) -> Self {
        Self {
            raw_text: raw_text.into(),
            command_name: command_name.into(),
            positional_args,
            named_flags,
        }
    }

    pub fn has_flag(&self, flag_name: &str) -> bool {
        self.named_flags
            .keys()
            .any(|k| k.eq_ignore_ascii_case(flag_name))
    }

    pub fn get_flag(&self, flag_name: &str) -> Option<&str> {
        self.named_flags
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(flag_name))
            .and_then(|(_, v)| v.as_deref())
    }

    pub fn try_get_flag_i32(&self, flag_name: &str) -> Option<i32> {
        self.get_flag(flag_name)?.parse().ok()
    }

    pub fn try_get_flag_f32(&self, flag_name: &str) -> Option<f32> {
        self.get_flag(flag_name)?.parse().ok()
    }

    pub fn get_positional(&self, index: usize) -> Option<&str> {
        self.positional_args.get(index).map(|s| s.as_str())
    }

    pub fn try_get_positional_i32(&self, index: usize) -> Option<i32> {
        self.get_positional(index)?.parse().ok()
    }

    pub fn try_get_positional_f32(&self, index: usize) -> Option<f32> {
        self.get_positional(index)?.parse().ok()
    }

    /// 解析矩形參數：支援 "x1,z1,x2,z2" 或連續 4 個數值。
    pub fn try_parse_rect(raw: &str) -> Option<MapRegionRect> {
        if raw.trim().is_empty() {
            return None;
        }
        let parts: Vec<&str> = raw
            .split(&[',', ' ', ':', ';'][..])
            .filter(|s| !s.is_empty())
            .collect();
        if parts.len() != 4 {
            return None;
        }

        let x1: i32 = parts[0].parse().ok()?;
        let z1: i32 = parts[1].parse().ok()?;
        let x2: i32 = parts[2].parse().ok()?;
        let z2: i32 = parts[3].parse().ok()?;

        Some(MapRegionRect::new(x1, z1, x2, z2))
    }

    /// 解析圓形參數：支援 "cx,cz,radius" 或連續 3 個數值。
    pub fn try_parse_circle(raw: &str) -> Option<MapRegionCircle> {
        if raw.trim().is_empty() {
            return None;
        }
        let parts: Vec<&str> = raw
            .split(&[',', ' ', ':', ';'][..])
            .filter(|s| !s.is_empty())
            .collect();
        if parts.len() != 3 {
            return None;
        }

        let cx: f32 = parts[0].parse().ok()?;
        let cz: f32 = parts[1].parse().ok()?;
        let r: f32 = parts[2].parse().ok()?;
        if r <= 0.0 {
            return None;
        }

        Some(MapRegionCircle::new(cx, cz, r))
    }
}

/// 指令或巨集執行結果模型。
#[derive(Clone, Debug, PartialEq)]
pub struct CommandResult {
    pub success: bool,
    pub message: String,
    pub affected_count: usize,
    pub details: Option<Vec<String>>,
    pub error: Option<String>,
}

impl CommandResult {
    pub fn ok(message: impl Into<String>, affected_count: usize) -> Self {
        Self {
            success: true,
            message: message.into(),
            affected_count,
            details: None,
            error: None,
        }
    }

    pub fn ok_with_details(
        message: impl Into<String>,
        affected_count: usize,
        details: Vec<String>,
    ) -> Self {
        Self {
            success: true,
            message: message.into(),
            affected_count,
            details: Some(details),
            error: None,
        }
    }

    pub fn fail(message: impl Into<String>) -> Self {
        let msg = message.into();
        Self {
            success: false,
            message: msg.clone(),
            affected_count: 0,
            details: None,
            error: Some(msg),
        }
    }
}

/// 巨集腳本執行後之完整審計報告。
#[derive(Clone, Debug, PartialEq)]
pub struct MacroExecutionReport {
    pub script_path: Option<String>,
    pub success: bool,
    pub total_commands: usize,
    pub succeeded_commands: usize,
    pub failed_commands: usize,
    pub elapsed_milliseconds: u64,
    pub affected_elements: usize,
    pub was_rolled_back: bool,
    pub error_message: Option<String>,
    pub logs: Vec<(String, LogLevel)>,
}

/// 巨集操作中的放置物件模型
#[derive(Clone, Debug, PartialEq)]
pub struct MacroPlacedObject {
    pub id: Uuid,
    pub type_name: String,
    pub world_x: f32,
    pub world_y: f32,
    pub world_z: f32,
    pub team: i32,
    pub unit_count: i32,
}

/// 巨集單一步驟的狀態儲存（用於原子性回滾與撤銷/重做）
#[derive(Clone, Debug)]
pub enum MacroStepSnapshot {
    HeightDeltas {
        changes: Vec<(usize, u8, u8)>, // (index, before, after)
    },
    TextureDeltas {
        changes: Vec<(usize, String, String)>, // (index, before, after)
    },
    PlacementAdd {
        added_indices: Vec<usize>,
    },
    PlacementModify {
        changes: Vec<(usize, MacroPlacedObject, MacroPlacedObject)>, // (index, before, after)
    },
    PlacementRemove {
        removed: Vec<(usize, MacroPlacedObject)>,
    },
    Custom {
        name: String,
    },
}

/// 指令與巨集執行時的共享上下文環境。
#[derive(Clone, Debug)]
pub struct CommandExecutionContext {
    // 地形高程
    pub heights: Option<Vec<u8>>,
    pub vertex_size: usize,
    pub height_step: f32,

    // 地形阻擋
    pub collision: Option<Vec<u8>>,
    pub collision_size: usize,

    // 材質地表
    pub textures: Option<Vec<String>>,
    pub map_tile_dimension: usize,

    // 世界維度
    pub world_dimension: f32,
    pub water_level: f32,

    // 物件放置清單
    pub placed_objects: Vec<MacroPlacedObject>,
    pub selected_placement_indices: Vec<usize>,
    pub available_object_types: Vec<String>,

    // 知名標籤與材質
    pub known_textures: Vec<String>,
    pub known_aliases: Vec<String>,

    // 變數符號表 ($var -> value)
    pub variables: HashMap<String, String>,

    // 執行日誌收集
    pub logs: Vec<(String, LogLevel)>,
}

impl Default for CommandExecutionContext {
    fn default() -> Self {
        Self {
            heights: None,
            vertex_size: 257,
            height_step: 4.0,
            collision: None,
            collision_size: 256,
            textures: None,
            map_tile_dimension: 64,
            world_dimension: 16384.0,
            water_level: 0.0,
            placed_objects: Vec::new(),
            selected_placement_indices: Vec::new(),
            available_object_types: Vec::new(),
            known_textures: Vec::new(),
            known_aliases: Vec::new(),
            variables: HashMap::new(),
            logs: Vec::new(),
        }
    }
}

impl CommandExecutionContext {
    pub fn log(&mut self, message: impl Into<String>, level: LogLevel) {
        self.logs.push((message.into(), level));
    }

    pub fn get_height_at_world(&self, world_x: f32, world_z: f32) -> f32 {
        let heights = match &self.heights {
            Some(h) if !h.is_empty() => h,
            _ => return 0.0,
        };
        let vertex_size = self.vertex_size;
        if vertex_size <= 1 {
            return 0.0;
        }

        let step = self.world_dimension / (vertex_size - 1) as f32;
        let vx = ((world_x / step).round() as usize).clamp(0, vertex_size - 1);
        let vz = ((world_z / step).round() as usize).clamp(0, vertex_size - 1);
        let idx = vz * vertex_size + vx;

        if idx < heights.len() {
            heights[idx] as f32 * self.height_step
        } else {
            0.0
        }
    }

    pub fn find_object_type(&self, name_or_alias: &str) -> Option<&str> {
        self.available_object_types
            .iter()
            .find(|t| t.eq_ignore_ascii_case(name_or_alias))
            .map(|s| s.as_str())
    }
}
