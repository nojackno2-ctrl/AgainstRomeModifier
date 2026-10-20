//! Interactive 3D nature scene projection, picking, deletion, and blank-terrain clearance.
//! Faithful port of C# `MapEditorForm.Scene.cs`, `MapEditorForm.Nature.cs`, and `MapEditorForm.Terrain.cs`.

use super::engine::NatureAddition;
use super::palette::PlacedNatureObject;
use super::session::NatureEditSession;
use serde::{Deserialize, Serialize};

/// 原版地圖 Level 物件索引偏移基底（C# `-100000 - slot`）。
pub const LEVEL_OBJECT_INDEX_BASE: i32 = -100_000;

/// 編輯器待存新增物件索引偏移基底（C# `-200000 - index`）。
pub const ADDITION_OBJECT_INDEX_BASE: i32 = -200_000;

/// Level 資料物件來源字串標記（C# `"DATA/objects.dat"`）。
pub const NATIVE_DATA_OBJECTS_SOURCE: &str = "DATA/objects.dat";

/// 判斷是否為 Level 資料物件（比照 C# `Map3DViewControl.IsLevelDataObject`）。
#[inline]
pub fn is_level_data_object(object_index: i32, source: &str) -> bool {
    object_index < 0 && source == NATIVE_DATA_OBJECTS_SOURCE
}

/// 判斷是否為待存新增物件索引（`<= -200000`）。
#[inline]
pub fn is_addition_index(object_index: i32) -> bool {
    object_index <= ADDITION_OBJECT_INDEX_BASE
}

/// 判斷是否為既有地圖槽位索引（`<= -100000` 且 `> -200000`）。
#[inline]
pub fn is_existing_slot_index(object_index: i32) -> bool {
    object_index <= LEVEL_OBJECT_INDEX_BASE && object_index > ADDITION_OBJECT_INDEX_BASE
}

/// 將槽位轉換為合成場景物件索引（`-100000 - slot`）。
#[inline]
pub fn object_index_for_slot(slot: i32) -> i32 {
    LEVEL_OBJECT_INDEX_BASE - slot
}

/// 將待存新增索引轉換為合成場景物件索引（`-200000 - index`）。
#[inline]
pub fn object_index_for_addition(addition_index: usize) -> i32 {
    ADDITION_OBJECT_INDEX_BASE - (addition_index as i32)
}

/// 從場景物件索引提取原版槽位號（若非既有槽位則回傳 `None`）。
#[inline]
pub fn slot_from_object_index(object_index: i32) -> Option<i32> {
    if is_existing_slot_index(object_index) {
        Some(LEVEL_OBJECT_INDEX_BASE - object_index)
    } else {
        None
    }
}

/// 從場景物件索引提取待存新增物件之索引（若非新增物件則回傳 `None`）。
#[inline]
pub fn addition_index_from_object_index(object_index: i32) -> Option<usize> {
    if is_addition_index(object_index) {
        let raw = ADDITION_OBJECT_INDEX_BASE - object_index;
        if raw >= 0 {
            Some(raw as usize)
        } else {
            None
        }
    } else {
        None
    }
}

/// 投影至 2D/3D 場景的自然物件顯示資料結構。
/// 比照 C# `MapEditorForm.Nature.cs:NatureDisplayObjects`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NatureDisplayObject {
    pub object_index: i32,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub team: i32,
    pub source: String,
    pub angle_degrees: Option<f32>,
    pub is_addition: bool,
}

impl NatureDisplayObject {
    #[inline]
    pub fn is_level_data_object(&self) -> bool {
        is_level_data_object(self.object_index, &self.source)
    }
}

/// 建立當前有效之自然場景顯示物件列表。
/// 過濾已移除槽位，並將既有物件與草稿新增物分別投影至 `-100000 - slot` 與 `-200000 - index`。
/// 比照 C# `MapEditorForm.Nature.cs:NatureDisplayObjects`。
pub fn build_nature_display_objects(
    level_objects: &[PlacedNatureObject],
    session: &NatureEditSession,
) -> Vec<NatureDisplayObject> {
    let removals = session.removed_slots();
    let additions = session.additions();

    let mut result = Vec::with_capacity(level_objects.len() + additions.len());

    // 1. 地圖既有地景物件（未被移除且為 Lan* 地景）
    for item in level_objects {
        if !removals.contains(&item.slot) && item.is_landscape() {
            let angle = if item.rotation.is_finite() {
                Some(item.rotation * (180.0 / std::f32::consts::PI))
            } else {
                None
            };
            result.push(NatureDisplayObject {
                object_index: object_index_for_slot(item.slot),
                name: item.name.clone(),
                x: item.x,
                y: item.y,
                z: item.z,
                team: item.team,
                source: NATIVE_DATA_OBJECTS_SOURCE.to_string(),
                angle_degrees: angle,
                is_addition: false,
            });
        }
    }

    // 2. 編輯會話中的待存新增物件
    for (index, addition) in additions.iter().enumerate() {
        let angle = if addition.rotation.is_finite() {
            Some(addition.rotation * (180.0 / std::f32::consts::PI))
        } else {
            None
        };
        result.push(NatureDisplayObject {
            object_index: object_index_for_addition(index),
            name: addition.name.clone(),
            x: addition.x,
            y: addition.y,
            z: addition.z,
            team: 8,
            source: NATIVE_DATA_OBJECTS_SOURCE.to_string(),
            angle_degrees: angle,
            is_addition: true,
        });
    }

    result
}

/// 3D 選取的自然物件目標類型。
#[derive(Debug, Clone, PartialEq)]
pub enum NaturePickTarget {
    ExistingSlot {
        slot: i32,
        object: PlacedNatureObject,
    },
    Addition {
        index: usize,
        addition: NatureAddition,
    },
}

impl NaturePickTarget {
    #[inline]
    pub fn name(&self) -> &str {
        match self {
            Self::ExistingSlot { object, .. } => &object.name,
            Self::Addition { addition, .. } => &addition.name,
        }
    }

    #[inline]
    pub fn is_addition(&self) -> bool {
        matches!(self, Self::Addition { .. })
    }
}

/// 3D 自然物件選取結果狀態。
#[derive(Debug, Clone, PartialEq)]
pub enum NaturePickResult {
    /// 成功解析出可操作之自然物件目標。
    Success(NaturePickTarget),
    /// 目標為被腳本或結構連結之保護物件（不可移除）。
    ProtectedLinkedObject { slot: i32, name: String },
    /// 目標非 Lan* 地景物件（例如建築、部隊、固定標記）。
    NotLandscape { slot: i32, name: String },
    /// 該槽位已被目前會話標記移除。
    AlreadyRemoved { slot: i32 },
    /// 待存新增物件索引超出範圍。
    AdditionNotFound { index: usize },
    /// 地圖中無此槽位物件。
    SlotNotFound { slot: i32 },
    /// 非 DATA/objects.dat 或非負數合成索引。
    NotLevelDataObject,
}

/// 解析 3D 視角中被點選的場景物件是否為可編輯自然物件。
/// 比照 C# `MapEditorForm.Scene.cs:PickedNatureTarget`。
pub fn resolve_picked_nature(
    object_index: i32,
    source: &str,
    level_objects: &[PlacedNatureObject],
    session: &NatureEditSession,
) -> NaturePickResult {
    if !is_level_data_object(object_index, source) {
        return NaturePickResult::NotLevelDataObject;
    }

    if is_addition_index(object_index) {
        let index = match addition_index_from_object_index(object_index) {
            Some(i) => i,
            None => return NaturePickResult::NotLevelDataObject,
        };
        let additions = session.additions();
        if index < additions.len() {
            NaturePickResult::Success(NaturePickTarget::Addition {
                index,
                addition: additions[index].clone(),
            })
        } else {
            NaturePickResult::AdditionNotFound { index }
        }
    } else if is_existing_slot_index(object_index) {
        let slot = match slot_from_object_index(object_index) {
            Some(s) => s,
            None => return NaturePickResult::NotLevelDataObject,
        };
        let item = match level_objects.iter().find(|entry| entry.slot == slot) {
            Some(it) => it,
            None => return NaturePickResult::SlotNotFound { slot },
        };
        if item.linked {
            return NaturePickResult::ProtectedLinkedObject {
                slot,
                name: item.name.clone(),
            };
        }
        if !item.is_landscape() {
            return NaturePickResult::NotLandscape {
                slot,
                name: item.name.clone(),
            };
        }
        if session.removed_slots().contains(&slot) {
            return NaturePickResult::AlreadyRemoved { slot };
        }
        NaturePickResult::Success(NaturePickTarget::ExistingSlot {
            slot,
            object: item.clone(),
        })
    } else {
        NaturePickResult::NotLevelDataObject
    }
}

/// 刪除選取的自然物件，並作為單一可撤銷之筆觸寫入歷史。
/// 比照 C# `MapEditorForm.Scene.cs:DeletePickedNature`。
pub fn delete_picked_nature(
    target: &NaturePickTarget,
    session: &mut NatureEditSession,
) -> bool {
    let removed = match target {
        NaturePickTarget::ExistingSlot { slot, .. } => session.remove(&[*slot], &[]),
        NaturePickTarget::Addition { addition, .. } => session.remove(&[], &[addition.clone()]),
    };
    if !removed {
        return false;
    }
    session.commit_stroke();
    true
}

/// 產生物件移除成功後的本地化狀態提示文字。
/// 比照 C# `MapEditorForm.Scene.cs:337`。
pub fn nature_deleted_status_message(name: &str, is_en: bool) -> String {
    if is_en {
        format!("Removed {name}; Undo restores it.")
    } else {
        format!("已移除 {name}；可用「復原」還原。")
    }
}

/// 產生物件被選取時的本地化狀態提示文字。
/// 比照 C# `MapEditorForm.Scene.cs:302, 305`。
pub fn nature_selected_status_message(name: &str, is_removable: bool, is_en: bool) -> String {
    if is_removable {
        if is_en {
            format!("Selected {name}; press Delete to remove it (undo restores it).")
        } else {
            format!("已選取 {name}；按 Delete 移除（可復原）。")
        }
    } else if is_en {
        format!("{name} is a placed or fixed map object; edit it with the Place tool.")
    } else {
        format!("{name} 屬於放置物件或地圖固定物件，請用「放置」工具編輯。")
    }
}

/// 空白地形自然物件清除執行報告。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct NatureBlankClearanceReport {
    pub removed_slots: Vec<i32>,
    pub removed_additions_count: usize,
    pub preserved_linked_slots: Vec<i32>,
    pub preserved_non_landscape_slots: Vec<i32>,
}

/// 執行空白地形（Blank Terrain）對自然物件之同步清除。
/// 清除所有可移除地景物件與所有待存新增物件，並將本次清除作為單一筆觸記錄至歷史（可完整 Undo/Redo）。
/// 嚴格保留被腳本或結構連結（linked）之地景與非地景結構（建築物、標記）。
/// 比照 C# `MapEditorForm.Terrain.cs:163`。
pub fn apply_blank_terrain_nature_clearance(
    level_objects: &[PlacedNatureObject],
    session: &mut NatureEditSession,
) -> NatureBlankClearanceReport {
    let mut removable_slots = Vec::new();
    let mut preserved_linked_slots = Vec::new();
    let mut preserved_non_landscape_slots = Vec::new();

    let current_removals = session.removed_slots();

    for item in level_objects {
        if item.is_removable_nature() {
            if !current_removals.contains(&item.slot) {
                removable_slots.push(item.slot);
            }
        } else if item.linked && item.is_landscape() {
            preserved_linked_slots.push(item.slot);
        } else {
            preserved_non_landscape_slots.push(item.slot);
        }
    }

    let pending_additions = session.additions().to_vec();
    let removed_additions_count = pending_additions.len();

    let changed = session.remove(&removable_slots, &pending_additions);
    if changed {
        session.commit_stroke();
    }

    NatureBlankClearanceReport {
        removed_slots: removable_slots,
        removed_additions_count,
        preserved_linked_slots,
        preserved_non_landscape_slots,
    }
}
