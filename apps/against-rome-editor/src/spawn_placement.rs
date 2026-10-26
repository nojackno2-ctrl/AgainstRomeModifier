//! Scenario spawn placement session: pure Rust port of C# `PlacementEditSession`
//! (`src.MapEditor.Modules/Placement`) plus the spawn <-> placed-object conversions used by
//! `MapEditorForm.Placement.cs` / `MapEditorForm.Persistence.cs`.
//!
//! The session edits `SdlPlacedObject` values with the same command semantics as C#
//! (single-step batch commands, reverse-order undo, persistent `ScenarioId`, raw-rotation rules).
//! Validation delegates to `against_rome_maps::placement_edit`.

use against_rome_maps::placement_edit::{edit_placement, edit_placement_values, validate_bounds};
use against_rome_maps::sdl_objects::{SdlObjectCategory, SdlObjectType, SdlPlacedObject};
use against_rome_maps::script_aliases::ScriptObjectAlias;
use against_rome_scenario::model::Spawn;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// C# `ArgumentOutOfRangeException` vs `InvalidOperationException` vs `ArgumentException`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlacementError {
    /// Index or value out of range (C# `ArgumentOutOfRangeException`).
    OutOfRange(String),
    /// Operation not allowed for this object (C# `InvalidOperationException`).
    InvalidOperation(String),
    /// Bad argument such as duplicate persistent ids (C# `ArgumentException`).
    InvalidArgument(String),
}

impl std::fmt::Display for PlacementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfRange(m) | Self::InvalidOperation(m) | Self::InvalidArgument(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for PlacementError {}

pub type PlacementResult<T> = Result<T, PlacementError>;

/// C# `ScenarioLevelObjects.NormalizeAngle`: wraps any integer angle into `0..360`.
pub fn normalize_angle(angle: i32) -> i32 {
    angle.rem_euclid(360)
}

/// `MathF.Round` (banker's rounding) followed by `(int)` cast.
fn round_angle(angle: f32) -> i32 {
    angle.round_ties_even() as i32
}

/// C# `ScenarioLevelObjects.RadiansToDisplayAngle`.
pub fn radians_to_display_angle(radians: f32) -> i32 {
    let deg = radians as f64 * (180.0 / std::f64::consts::PI);
    (((deg % 360.0) + 360.0) % 360.0).round_ties_even() as i32 % 360
}

fn map_validate(item: &SdlPlacedObject, strict: bool) -> PlacementResult<()> {
    validate_bounds(item, strict).map_err(PlacementError::OutOfRange)
}

/// Independent command that can be executed and undone against the object list.
#[derive(Clone, Debug)]
enum Command {
    Add(SdlPlacedObject),
    Remove { index: usize, item: SdlPlacedObject },
    Edit { index: usize, before: SdlPlacedObject, after: SdlPlacedObject },
    Duplicate { insert_index: usize, item: SdlPlacedObject },
    Batch { description: &'static str, commands: Vec<Command> },
}

impl Command {
    fn execute(&self, objects: &mut Vec<SdlPlacedObject>) {
        match self {
            Self::Add(item) => objects.push(item.clone()),
            Self::Remove { index, .. } => {
                objects.remove(*index);
            }
            Self::Edit { index, after, .. } => objects[*index] = after.clone(),
            Self::Duplicate { insert_index, item } => objects.insert(*insert_index, item.clone()),
            Self::Batch { commands, .. } => {
                for command in commands {
                    command.execute(objects);
                }
            }
        }
    }

    fn undo(&self, objects: &mut Vec<SdlPlacedObject>) {
        match self {
            Self::Add(_) => {
                objects.pop();
            }
            Self::Remove { index, item } => objects.insert(*index, item.clone()),
            Self::Edit { index, before, .. } => objects[*index] = before.clone(),
            Self::Duplicate { insert_index, .. } => {
                objects.remove(*insert_index);
            }
            Self::Batch { commands, .. } => {
                for command in commands.iter().rev() {
                    command.undo(objects);
                }
            }
        }
    }

    fn description(&self) -> String {
        match self {
            Self::Add(item) => format!("Add {}", item.obj_type.name_def),
            Self::Remove { index, item } => format!("Remove {} at {index}", item.obj_type.name_def),
            Self::Edit { index, after, .. } => format!("Edit {} at {index}", after.obj_type.name_def),
            Self::Duplicate { insert_index, item } => {
                format!("Duplicate {} to {insert_index}", item.obj_type.name_def)
            }
            Self::Batch { description, .. } => (*description).to_string(),
        }
    }
}

fn float_bits_equal(a: Option<f32>, b: Option<f32>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a.to_bits() == b.to_bits(),
        _ => false,
    }
}

/// C# `Equal`: identity, provenance, raw rotation bits, position, team, angle, count and type.
pub fn objects_equal(a: &SdlPlacedObject, b: &SdlPlacedObject) -> bool {
    a.scenario_id == b.scenario_id
        && a.copy_source_scenario_id == b.copy_source_scenario_id
        && a.native_source_identity == b.native_source_identity
        && float_bits_equal(a.raw_rotation, b.raw_rotation)
        && a.world_x == b.world_x
        && a.world_y == b.world_y
        && a.world_z == b.world_z
        && a.team == b.team
        && a.angle == b.angle
        && a.unit_count == b.unit_count
        && a.obj_type == b.obj_type
}

/// Persistent identity, content snapshot and save baseline of the placed objects; supports
/// independent commands with Undo/Redo (C# `PlacementEditSession`).
#[derive(Clone, Debug, Default)]
pub struct SpawnPlacementSession {
    objects: Vec<SdlPlacedObject>,
    undo: Vec<Command>,
    redo: Vec<Command>,
    baseline: Vec<SdlPlacedObject>,
}

impl SpawnPlacementSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub const MODULE_ID: &'static str = "placement";

    pub fn is_dirty(&self) -> bool {
        self.objects.len() != self.baseline.len()
            || self.objects.iter().zip(&self.baseline).any(|(a, b)| !objects_equal(a, b))
    }
    pub fn count(&self) -> usize {
        self.objects.len()
    }
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    /// 僅移除重做歷史，保留目前內容、基準與復原。
    pub fn clear_redo(&mut self) { self.redo.clear(); }
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }
    pub fn redo_depth(&self) -> usize {
        self.redo.len()
    }
    /// Description of the command that `undo` would revert.
    pub fn undo_description(&self) -> Option<String> {
        self.undo.last().map(Command::description)
    }

    /// Deep copy of one object (C# indexer).
    pub fn get(&self, index: usize) -> Option<SdlPlacedObject> {
        self.objects.get(index).cloned()
    }

    /// Snapshot of all objects (C# `Capture`).
    pub fn capture(&self) -> Vec<SdlPlacedObject> {
        self.objects.clone()
    }

    pub fn objects(&self) -> &[SdlPlacedObject] {
        &self.objects
    }

    /// Replaces content with a copy of `snapshot`, clears history, accepts it as baseline.
    pub fn load(&mut self, snapshot: &[SdlPlacedObject]) {
        self.objects = snapshot.to_vec();
        self.clear_history();
        self.accept_changes();
    }

    pub fn accept_changes(&mut self) {
        self.baseline = self.capture();
        self.clear_history();
    }

    /// After a successful save: drops copy-source provenance and native identities, then
    /// re-baselines (C# `AcceptSavedChanges`).
    pub fn accept_saved_changes(&mut self) {
        for item in &mut self.objects {
            if !item.copy_source_scenario_id.is_nil() || item.native_source_identity.is_some() {
                item.copy_source_scenario_id = Uuid::nil();
                item.native_source_identity = None;
            }
        }
        self.baseline = self.capture();
        self.clear_history();
    }

    /// Restores the accepted baseline and clears history (C# `Reset`).
    pub fn reset(&mut self) {
        self.objects = self.baseline.clone();
        self.clear_history();
    }

    pub fn clear_history(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    fn execute(&mut self, command: Command) {
        command.execute(&mut self.objects);
        self.undo.push(command);
        self.redo.clear();
    }

    pub fn undo(&mut self) -> bool {
        let Some(command) = self.undo.pop() else { return false };
        command.undo(&mut self.objects);
        self.redo.push(command);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(command) = self.redo.pop() else { return false };
        command.execute(&mut self.objects);
        self.undo.push(command);
        true
    }

    /// Reverts the most recent command and forgets it (no redo entry). Used by callers whose
    /// downstream validation rejected an otherwise valid edit.
    pub fn abort_last(&mut self) -> bool {
        let Some(command) = self.undo.pop() else { return false };
        command.undo(&mut self.objects);
        true
    }

    pub fn add(&mut self, mut item: SdlPlacedObject) -> PlacementResult<()> {
        if item.scenario_id.is_nil() {
            item.scenario_id = Uuid::new_v4();
        }
        map_validate(&item, false)?;
        self.execute(Command::Add(item));
        Ok(())
    }

    pub fn replace(&mut self, index: usize, mut item: SdlPlacedObject) -> PlacementResult<()> {
        let before = self.checked(index)?.clone();
        if item.scenario_id.is_nil() {
            item.scenario_id = before.scenario_id;
        }
        map_validate(&item, false)?;
        self.execute(Command::Edit { index, before, after: item });
        Ok(())
    }

    pub fn remove_at(&mut self, index: usize) -> PlacementResult<()> {
        let removed = self.checked(index)?.clone();
        self.execute(Command::Remove { index, item: removed });
        Ok(())
    }

    /// Edits an object keeping its persistent id and native metadata.
    pub fn edit(&mut self, index: usize, updated: &SdlPlacedObject) -> PlacementResult<()> {
        let before = self.checked(index)?.clone();
        let after = edit_placement(&before, updated).map_err(map_edit_error)?;
        self.execute(Command::Edit { index, before, after });
        Ok(())
    }

    /// Edits team/position/angle/unit count (C# `Edit(index, team, x, y, z, angle, unitCount)`).
    #[allow(clippy::too_many_arguments)]
    pub fn edit_values(
        &mut self,
        index: usize,
        team: i32,
        world_x: f32,
        world_y: f32,
        world_z: f32,
        angle: f32,
        unit_count: i32,
    ) -> PlacementResult<()> {
        let before = self.checked(index)?.clone();
        let after = edit_placement_values(&before, team, world_x, world_y, world_z, angle, unit_count)
            .map_err(map_edit_error)?;
        self.execute(Command::Edit { index, before, after });
        Ok(())
    }

    pub fn duplicate(&mut self, index: usize, offset_x: f32, offset_z: f32) -> PlacementResult<usize> {
        Ok(self.duplicate_many(&[index], offset_x, offset_z)?[0])
    }

    /// Validates every copy first, then inserts them as one undo step. An offset that leaves the
    /// map rejects the whole batch. Returns the indices of the new copies.
    pub fn duplicate_many(&mut self, indices: &[usize], offset_x: f32, offset_z: f32) -> PlacementResult<Vec<usize>> {
        self.duplicate_transformed(indices, |item| {
            item.world_x += offset_x;
            item.world_z += offset_z;
        })
    }

    /// 以世界座標中心 (8192, 8192) 複製位置；保留方向、隊伍與來源關聯。
    pub fn duplicate_opposite_many(&mut self, indices: &[usize], height_at: Option<&dyn Fn(f32, f32) -> f32>) -> PlacementResult<Vec<usize>> {
        self.duplicate_transformed(indices, |item| {
            item.world_x = 16384.0 - item.world_x;
            item.world_z = 16384.0 - item.world_z;
            if let Some(height_at) = height_at {
                item.world_y = height_at(item.world_x, item.world_z);
            }
        })
    }

    fn duplicate_transformed(&mut self, indices: &[usize], transform: impl Fn(&mut SdlPlacedObject)) -> PlacementResult<Vec<usize>> {
        let selected = self.validate_indices(indices)?;
        if selected.is_empty() {
            return Ok(Vec::new());
        }
        let mut commands = Vec::new();
        for &index in selected.iter().rev() {
            let source = &self.objects[index];
            let (source_provenance, native_identity) = if source.native_source_identity.is_some() {
                // Pending native copy: duplicate-of-duplicate keeps the original native identity.
                (Uuid::nil(), source.native_source_identity)
            } else if !source.copy_source_scenario_id.is_nil() {
                (source.copy_source_scenario_id, None)
            } else {
                (source.scenario_id, None)
            };
            let mut duplicate = source.clone();
            transform(&mut duplicate);
            duplicate.scenario_id = Uuid::new_v4();
            duplicate.copy_source_scenario_id = source_provenance;
            duplicate.native_source_identity = native_identity;
            duplicate.raw_rotation = source.raw_rotation;
            map_validate(&duplicate, false)?;
            commands.push(Command::Duplicate { insert_index: index + 1, item: duplicate });
        }
        self.execute(Command::Batch { description: "Duplicate selected objects", commands });
        Ok(selected.iter().enumerate().map(|(shift, index)| index + shift + 1).collect())
    }

    /// Validates all indices, then removes them as one undo step that restores order and identity.
    pub fn remove_many(&mut self, indices: &[usize]) -> PlacementResult<()> {
        let selected = self.validate_indices(indices)?;
        if selected.is_empty() {
            return Ok(());
        }
        let commands = selected
            .iter()
            .rev()
            .map(|&index| Command::Remove { index, item: self.objects[index].clone() })
            .collect();
        self.execute(Command::Batch { description: "Remove selected objects", commands });
        Ok(())
    }

    /// Validates every selected object before changing team/direction as one undoable batch.
    pub fn edit_many(&mut self, indices: &[usize], team: Option<i32>, angle: Option<f32>) -> PlacementResult<()> {
        let selected = self.validate_indices(indices)?;
        if selected.is_empty() || (team.is_none() && angle.is_none()) {
            return Ok(());
        }
        let mut commands = Vec::new();
        for &index in &selected {
            let before = &self.objects[index];
            let family_copy = !before.copy_source_scenario_id.is_nil() || before.native_source_identity.is_some();
            if before.raw_rotation.is_some() && !family_copy {
                if team.is_some_and(|t| t != before.team) {
                    return Err(PlacementError::InvalidOperation(
                        "所選物件中包含原生群組物件，目前僅支援平移；變更隊伍尚未支援。".into(),
                    ));
                }
                if angle.is_some_and(|a| {
                    normalize_angle(round_angle(a)) != normalize_angle(round_angle(before.angle))
                }) {
                    return Err(PlacementError::InvalidOperation(
                        "所選物件中包含原生群組物件，目前僅支援平移；變更方向尚未支援。".into(),
                    ));
                }
            }
            let mut after = before.clone();
            after.team = team.unwrap_or(before.team);
            after.angle = angle.unwrap_or(before.angle);
            if family_copy && before.raw_rotation.is_some()
                && normalize_angle(round_angle(after.angle)) != normalize_angle(round_angle(before.angle))
            {
                after.raw_rotation = Some((normalize_angle(round_angle(after.angle)) as f64
                    * (std::f64::consts::PI / 180.0)) as f32);
            }
            map_validate(&after, true)?;
            if after.team != before.team || after.angle != before.angle {
                commands.push(Command::Edit { index, before: before.clone(), after });
            }
        }
        if !commands.is_empty() {
            self.execute(Command::Batch { description: "Set selected teams/directions", commands });
        }
        Ok(())
    }

    /// Rotates each selected object relative to its own direction as one atomic undo step.
    pub fn rotate_many(&mut self, indices: &[usize], degrees: i32) -> PlacementResult<()> {
        let selected = self.validate_indices(indices)?;
        let mut commands = Vec::new();
        for &index in &selected {
            let before = &self.objects[index];
            let family_copy = !before.copy_source_scenario_id.is_nil() || before.native_source_identity.is_some();
            if before.raw_rotation.is_some() && !family_copy {
                return Err(PlacementError::InvalidOperation(
                    "原生群組物件目前僅支援平移；變更方向尚未支援。".into(),
                ));
            }
            let angle = normalize_angle(round_angle(before.angle) + degrees % 360) as f32;
            let mut after = before.clone();
            after.angle = angle;
            if family_copy && before.raw_rotation.is_some() {
                after.raw_rotation = Some((normalize_angle(round_angle(angle)) as f64 * (std::f64::consts::PI / 180.0)) as f32);
            }
            map_validate(&after, false)?;
            if after.angle != before.angle {
                commands.push(Command::Edit { index, before: before.clone(), after });
            }
        }
        if !commands.is_empty() {
            self.execute(Command::Batch { description: "Rotate selected objects", commands });
        }
        Ok(())
    }

    /// Validates a complete layout before inserting any object; missing ids get fresh identities.
    pub fn add_many(&mut self, items: &[SdlPlacedObject]) -> PlacementResult<Vec<usize>> {
        let prepared: Vec<SdlPlacedObject> = items
            .iter()
            .cloned()
            .map(|mut item| {
                if item.scenario_id.is_nil() {
                    item.scenario_id = Uuid::new_v4();
                }
                item
            })
            .collect();
        let mut ids: HashSet<Uuid> = self.objects.iter().map(|o| o.scenario_id).collect();
        for item in &prepared {
            map_validate(item, true)?;
            if !ids.insert(item.scenario_id) {
                return Err(PlacementError::InvalidArgument("Layout objects require distinct persistent IDs.".into()));
            }
        }
        let start = self.objects.len();
        let count = prepared.len();
        if count > 0 {
            self.execute(Command::Batch {
                description: "Place layout",
                commands: prepared.into_iter().map(Command::Add).collect(),
            });
        }
        Ok((start..start + count).collect())
    }

    /// Translates a selection without changing identity, direction or family provenance. The
    /// whole batch is validated before one undo command is added. `height_at` recomputes Y.
    pub fn move_many(
        &mut self,
        indices: &[usize],
        offset_x: f32,
        offset_z: f32,
        height_at: Option<&dyn Fn(f32, f32) -> f32>,
    ) -> PlacementResult<()> {
        let selected = self.validate_indices(indices)?;
        if !offset_x.is_finite() || !offset_z.is_finite() {
            return Err(PlacementError::OutOfRange("Offsets must be finite.".into()));
        }
        if offset_x == 0.0 && offset_z == 0.0 {
            return Ok(());
        }
        let mut commands = Vec::new();
        for &index in &selected {
            let before = &self.objects[index];
            let mut after = before.clone();
            after.world_x += offset_x;
            after.world_z += offset_z;
            map_validate(&after, false)?;
            if let Some(height_at) = height_at {
                after.world_y = height_at(after.world_x, after.world_z);
            }
            map_validate(&after, false)?;
            commands.push(Command::Edit { index, before: before.clone(), after });
        }
        if !commands.is_empty() {
            self.execute(Command::Batch { description: "Move selected objects", commands });
        }
        Ok(())
    }

    /// Aligns the first selected object's anchor to a 256-unit tile centre and moves the rest by
    /// the same delta (C# `SnapManyToGrid`).
    pub fn snap_many_to_grid(
        &mut self,
        indices: &[usize],
        height_at: Option<&dyn Fn(f32, f32) -> f32>,
    ) -> PlacementResult<()> {
        let selected = self.validate_indices(indices)?;
        let Some(&first) = selected.first() else { return Ok(()) };
        let anchor = &self.objects[first];
        let dx = (anchor.world_x / 256.0).floor() * 256.0 + 128.0 - anchor.world_x;
        let dz = (anchor.world_z / 256.0).floor() * 256.0 + 128.0 - anchor.world_z;
        self.move_many(&selected, dx, dz, height_at)
    }

    fn checked(&self, index: usize) -> PlacementResult<&SdlPlacedObject> {
        self.objects
            .get(index)
            .ok_or_else(|| PlacementError::OutOfRange(format!("index {index} is out of range")))
    }

    /// Distinct, ascending, in-range indices.
    fn validate_indices(&self, indices: &[usize]) -> PlacementResult<Vec<usize>> {
        let mut selected: Vec<usize> = indices.to_vec();
        selected.sort_unstable();
        selected.dedup();
        if selected.iter().any(|&i| i >= self.objects.len()) {
            return Err(PlacementError::OutOfRange("indices out of range".into()));
        }
        Ok(selected)
    }
}

/// `edit_placement*` fails with either a native-translation-only refusal (C#
/// `InvalidOperationException`, Chinese message) or a validation failure (`ArgumentOutOfRange`).
fn map_edit_error(message: String) -> PlacementError {
    if message.contains("原生群組") {
        PlacementError::InvalidOperation(message)
    } else {
        PlacementError::OutOfRange(message)
    }
}

// --- Catalog and spawn conversion (MapEditorForm.Placement.cs / Persistence.cs) ---

/// Palette catalog from `cl_scint.ini [ObjDefName]` (C# `BuildSpawnCatalog`): sorted by category,
/// tribe, then name; `template_fields["alias"]` keeps the script alias.
pub fn build_spawn_catalog(aliases: &[ScriptObjectAlias]) -> Vec<SdlObjectType> {
    let mut catalog: Vec<SdlObjectType> = aliases
        .iter()
        .map(|alias| {
            let mut fields = HashMap::new();
            fields.insert("alias".to_string(), alias.alias.clone());
            SdlObjectType::new(alias.name_def.clone(), -1, alias.category(), alias.tribe(), 0, fields)
        })
        .collect();
    catalog.sort_by(|a, b| {
        a.category
            .cmp(&b.category)
            .then_with(|| a.tribe.cmp(&b.tribe))
            .then_with(|| a.name_def.to_ascii_lowercase().cmp(&b.name_def.to_ascii_lowercase()))
    });
    catalog
}

/// C# `AliasOf`.
pub fn alias_of(object_type: &SdlObjectType) -> &str {
    object_type
        .template_fields
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("alias"))
        .map(|(_, v)| v.as_str())
        .unwrap_or(&object_type.name_def)
}

/// Catalog type for a spawn alias (case-insensitive).
pub fn type_for_alias<'a>(catalog: &'a [SdlObjectType], alias: &str) -> Option<&'a SdlObjectType> {
    catalog.iter().find(|t| alias_of(t).eq_ignore_ascii_case(alias))
}

/// C# `LoadScenarioPlacements` for one spawn; `None` if the alias is not in the catalog.
pub fn spawn_to_placed(spawn: &Spawn, catalog: &[SdlObjectType]) -> Option<SdlPlacedObject> {
    let object_type = type_for_alias(catalog, &spawn.alias)?.clone();
    let angle = match spawn.raw_rotation {
        Some(raw) => (raw as f64 * (180.0 / std::f64::consts::PI)) as f32,
        None => spawn.angle as f32,
    };
    let unit_count = if spawn.raw_rotation.is_some() { 0 } else if spawn.count > 1 { spawn.count } else { 1 };
    let mut placed =
        SdlPlacedObject::new(object_type, spawn.x, spawn.y, spawn.z, spawn.team, angle, unit_count);
    placed.scenario_id = spawn.id;
    placed.raw_rotation = spawn.raw_rotation;
    Some(placed)
}

/// Loads the representable spawns and returns the ones whose alias is not in the catalog
/// (those are preserved untouched on save instead of being dropped).
pub fn load_spawns(spawns: &[Spawn], catalog: &[SdlObjectType]) -> (Vec<SdlPlacedObject>, Vec<Spawn>) {
    let mut placed = Vec::new();
    let mut foreign = Vec::new();
    for spawn in spawns {
        match spawn_to_placed(spawn, catalog) {
            Some(item) => placed.push(item),
            None => foreign.push(spawn.clone()),
        }
    }
    (placed, foreign)
}

/// True for figures that are real squads (not animals, not native raw-rotation objects).
pub fn is_squad(item: &SdlPlacedObject) -> bool {
    item.obj_type.category == SdlObjectCategory::Figure && !item.obj_type.is_animal() && item.raw_rotation.is_none()
}

/// C# `Persistence.cs` spawn mapping. `previous` is the document spawn with the same id, if
/// any: its `prebuilt` flag is kept (new objects are always script-created, see module docs of
/// `spawn_placement_ui`) and a raw-rotation object keeps its display angle.
pub fn placed_to_spawn(item: &SdlPlacedObject, previous: Option<&Spawn>) -> Spawn {
    let raw_rotation = item.raw_rotation;
    let angle = match raw_rotation {
        Some(raw) => radians_to_display_angle(raw),
        None => normalize_angle(round_angle(item.angle)),
    };
    Spawn {
        alias: alias_of(&item.obj_type).to_string(),
        x: item.world_x,
        z: item.world_z,
        team: item.team,
        count: if is_squad(item) { item.unit_count.max(1) } else { 0 },
        angle,
        y: item.world_y,
        // A native raw rotation only exists on prebuilt DATA objects (document validation).
        prebuilt: raw_rotation.is_some() || previous.is_some_and(|s| s.prebuilt),
        id: item.scenario_id,
        raw_rotation,
    }
}

/// Document spawns for the session content followed by the preserved non-representable spawns.
/// An object that still equals the placed form of its original spawn keeps that spawn verbatim
/// (so untouched spawns, e.g. count-0 figures or unnormalised angles, round-trip byte for byte).
pub fn session_to_spawns(
    placed: &[SdlPlacedObject],
    original: &[Spawn],
    foreign: &[Spawn],
    catalog: &[SdlObjectType],
) -> Vec<Spawn> {
    let by_id: HashMap<Uuid, &Spawn> = original.iter().map(|s| (s.id, s)).collect();
    let mut spawns: Vec<Spawn> = placed
        .iter()
        .map(|item| {
            let previous = by_id.get(&item.scenario_id).copied();
            if let Some(prev) = previous {
                if spawn_to_placed(prev, catalog).as_ref().is_some_and(|p| objects_equal(p, item)) {
                    return prev.clone();
                }
            }
            placed_to_spawn(item, previous)
        })
        .collect();
    spawns.extend(foreign.iter().cloned());
    spawns
}

// --- Display helpers (C# CategoryText / TribeText / ObjectDisplayName / MatchesPlacementSearch) ---

pub fn category_text(category: SdlObjectCategory, english: bool) -> &'static str {
    match category {
        SdlObjectCategory::Building => if english { "Buildings" } else { "建築" },
        SdlObjectCategory::UnitGroup => if english { "Unit groups" } else { "部隊" },
        SdlObjectCategory::Figure => if english { "Characters" } else { "人物" },
        SdlObjectCategory::Effect => if english { "Effects" } else { "特效" },
        SdlObjectCategory::Other => if english { "Other" } else { "其他" },
    }
}

pub fn tribe_text(code: &str, english: bool) -> &'static str {
    match code {
        "Ger" => if english { "Germans" } else { "日耳曼" },
        "Hun" => if english { "Huns" } else { "匈人" },
        "Kel" => if english { "Celts" } else { "凱爾特" },
        "Rom" => if english { "Romans" } else { "羅馬" },
        _ => if english { "Common" } else { "通用" },
    }
}

const GERMAN_OBJECT_WORDS: &[(&str, &str)] = &[
    ("Haupthaus", "主屋"), ("Wohnhaus", "住宅"), ("Wohnzelt", "居住帳篷"), ("Lagerhaus", "倉庫"),
    ("Lagerzelt", "倉庫帳篷"), ("Bauernhof", "農場"), ("Schlachterei", "屠宰場"), ("Schreinerei", "木工坊"),
    ("Mine", "礦場"), ("Pferdestall", "馬廄"), ("Waffenschmiede", "兵器鋪"), ("Goldschmiede", "金匠鋪"),
    ("Mauer", "城牆"), ("Mauerecke", "城牆轉角"), ("Mauertor", "城門"), ("Tor", "城門"), ("Turm", "塔樓"),
    ("Palisade", "木柵"), ("Palisadenecke", "木柵轉角"), ("Palisadentor", "木柵門"), ("Opferstaette", "祭壇"),
    ("Anfuehrer", "領主"), ("Kampf_Icon", "戰鬥部隊"), ("Zivil_Icon", "平民隊"),
];

/// C# `ObjectDisplayName`.
pub fn object_display_name(object_type: &SdlObjectType, english: bool) -> String {
    let name = &object_type.name_def;
    let tail = name.split_once('_').map_or(name.as_str(), |(_, rest)| rest);
    let tribe = if object_type.tribe.is_empty() {
        String::new()
    } else {
        format!("{} ", tribe_text(&object_type.tribe, english))
    };
    let plain = tail.replace('_', " ");
    if english {
        return format!("{tribe}{plain}");
    }
    let lower = tail.to_ascii_lowercase();
    let known = GERMAN_OBJECT_WORDS
        .iter()
        .find(|(word, _)| lower == word.to_ascii_lowercase() || lower.ends_with(&word.to_ascii_lowercase()));
    match known {
        Some((_, zh)) => format!("{tribe}{zh}（{tail}）"),
        None => format!("{tribe}{plain}"),
    }
}

/// C# `MatchesPlacementSearch`: every whitespace separated term must occur (case-insensitive).
pub fn matches_search(object_type: &SdlObjectType, query: &str) -> bool {
    let text = [
        object_type.name_def.clone(),
        alias_of(object_type).to_string(),
        object_display_name(object_type, false),
        object_display_name(object_type, true),
        category_text(object_type.category, false).to_string(),
        category_text(object_type.category, true).to_string(),
        object_type.tribe.clone(),
        tribe_text(&object_type.tribe, false).to_string(),
        tribe_text(&object_type.tribe, true).to_string(),
    ]
    .join(" ")
    .to_lowercase();
    query.split_whitespace().all(|term| text.contains(&term.to_lowercase()))
}

// --- Geometry helpers (PlaceObjectAt / MoveSelectedPlacements) ---

/// World units per placement tile (64 x 64 tiles over a 16384 world).
pub const TILE_WORLD: f32 = 256.0;

/// Centre of a tile in world units.
pub fn tile_center(tile_x: i32, tile_y: i32) -> (f32, f32) {
    ((tile_x as f32 + 0.5) * TILE_WORLD, (tile_y as f32 + 0.5) * TILE_WORLD)
}

/// Terrain height used when placing on a clicked tile (C# `PlaceObjectAt`): the vertex at the
/// tile centre times Heightmapstep; 0 without terrain.
pub fn tile_ground_height(heights: &[u8], vertex_size: usize, tile_x: i32, tile_y: i32, height_step: f32) -> f32 {
    if vertex_size < 2 || heights.len() < vertex_size * vertex_size {
        return 0.0;
    }
    let step = ((vertex_size - 1) / 64) as i32;
    let max = vertex_size as i32 - 1;
    let vx = (tile_x * step + step / 2).clamp(0, max) as usize;
    let vy = (tile_y * step + step / 2).clamp(0, max) as usize;
    heights[vy * vertex_size + vx] as f32 * height_step
}

/// Terrain height used when moving objects (C# `MoveSelectedPlacements.HeightAt`).
pub fn world_ground_height(heights: &[u8], vertex_size: usize, world_x: f32, world_z: f32, height_step: f32) -> f32 {
    if vertex_size < 2 || heights.len() < vertex_size * vertex_size {
        return 0.0;
    }
    let step = (vertex_size - 1) as f32 / (64.0 * 256.0);
    let max = vertex_size as i32 - 1;
    let vx = ((world_x * step).round_ties_even() as i32).clamp(0, max) as usize;
    let vz = ((world_z * step).round_ties_even() as i32).clamp(0, max) as usize;
    heights[vz * vertex_size + vx] as f32 * height_step
}

/// Per-category team range for the input widgets (palette limits): squads 0..=7, others -1..=15.
pub fn team_range(object_type: &SdlObjectType) -> (i32, i32) {
    if object_type.category == SdlObjectCategory::Figure && !object_type.is_animal() {
        (0, 7)
    } else {
        (-1, 15)
    }
}

/// Whether the type takes a soldier count (figures that are not animals).
pub fn takes_unit_count(object_type: &SdlObjectType) -> bool {
    object_type.category == SdlObjectCategory::Figure && !object_type.is_animal()
}

/// Builds the object created by a click (C# `PlaceObjectAt`): units need team 0..7.
pub fn new_placement(
    object_type: &SdlObjectType,
    world_x: f32,
    world_y: f32,
    world_z: f32,
    team: i32,
    angle: f32,
    unit_count: i32,
) -> PlacementResult<SdlPlacedObject> {
    let (min_team, max_team) = team_range(object_type);
    if !(min_team..=max_team).contains(&team) {
        let message = if min_team == 0 {
            "Units need a team between 0 and 7.".to_string()
        } else {
            "Object team must be between -1 and 15.".to_string()
        };
        return Err(PlacementError::OutOfRange(message));
    }
    let mut item = SdlPlacedObject::new(
        object_type.clone(),
        world_x,
        world_y,
        world_z,
        team,
        angle,
        if takes_unit_count(object_type) { unit_count } else { 0 },
    );
    item.scenario_id = Uuid::new_v4();
    Ok(item)
}

// --- Landscape clearance warning (C# `PlaceObjectAt` / `MoveSelectedPlacements`) ---

/// A building closer than this (per axis, box test) to a tree/landscape object is rejected by the
/// game's script placement check and clips through it when written to DATA.
pub const LANDSCAPE_CLEARANCE: f32 = 384.0;

/// C# `NatureDisplayObjects().Any(|dx| < 384 && |dz| < 384)`.
pub fn landscape_nearby(landscape: &[(f32, f32)], world_x: f32, world_z: f32) -> bool {
    landscape
        .iter()
        .any(|&(x, z)| (x - world_x).abs() < LANDSCAPE_CLEARANCE && (z - world_z).abs() < LANDSCAPE_CLEARANCE)
}

/// Warning appended to the status line after placing a building (C# wording).
pub fn landscape_place_warning(english: bool) -> &'static str {
    if english {
        " Warning: trees or other landscape objects are very close; clear them with the Nature tool."
    } else {
        " 注意：附近有樹木或其他地景物件，建議用「自然物件 → 移除」清出空地。"
    }
}

/// Warning appended to the status line after moving buildings (C# wording).
pub fn landscape_move_warning(english: bool) -> &'static str {
    if english {
        " Warning: landscape objects are close; clear space with Nature tools."
    } else {
        " 注意：附近有地景物件，建議用自然物件工具清出空地。"
    }
}

/// Landscape (objdef name starts with `Lan`, C# `ObjDefNames.IsLandscape`) object positions of a
/// DATA snapshot: `(slot, x, z)`.
pub fn landscape_points(
    objects: &[against_rome_placement::WorldObject],
    names: &HashMap<i32, String>,
) -> Vec<(i32, f32, f32)> {
    objects
        .iter()
        .filter(|o| {
            names
                .get(&i32::from(o.type_id))
                .is_some_and(|n| n.len() >= 3 && n.as_bytes()[..3].eq_ignore_ascii_case(b"Lan"))
        })
        .filter(|o| o.position[0].is_finite() && o.position[2].is_finite())
        .map(|o| (o.slot as i32, o.position[0], o.position[2]))
        .collect()
}

// --- Canvas picking ---

/// Index of the placement nearest to a world point within `radius` world units (ties: first).
pub fn nearest_placement(objects: &[SdlPlacedObject], world_x: f32, world_z: f32, radius: f32) -> Option<usize> {
    objects
        .iter()
        .enumerate()
        .map(|(index, o)| (index, (o.world_x - world_x).hypot(o.world_z - world_z)))
        .filter(|&(_, distance)| distance <= radius)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(index, _)| index)
}
