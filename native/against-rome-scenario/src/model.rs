use crate::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
/// System.Text.Json only reads/writes GUIDs as 36-character hyphenated "D" text. The `uuid` crate
/// would also accept simple/braced/URN forms, so the on-disk contract is enforced here.
mod guid_d {
    use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};
    use uuid::Uuid;
    pub fn serialize<S: Serializer>(id: &Uuid, s: S) -> Result<S::Ok, S::Error> {
        id.hyphenated().serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Uuid, D::Error> {
        let text = String::deserialize(d)?;
        let b = text.as_bytes();
        if b.len() != 36 || [8, 13, 18, 23].iter().any(|&i| b[i] != b'-') {
            return Err(D::Error::custom("GUID must use the 36-character hyphenated form"));
        }
        Uuid::parse_str(&text).map_err(D::Error::custom)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct ScenarioDocument {
    pub version: i32,
    pub spawns: Vec<Spawn>,
    pub data_slots: Vec<DataSlot>,
    pub events: Vec<Event>,
}
impl Default for ScenarioDocument {
    fn default() -> Self {
        Self {
            version: 6,
            spawns: vec![],
            data_slots: vec![],
            events: vec![],
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Spawn {
    pub alias: String,
    pub x: f32,
    pub z: f32,
    pub team: i32,
    pub count: i32,
    pub angle: i32,
    pub y: f32,
    pub prebuilt: bool,
    #[serde(with = "guid_d")]
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_rotation: Option<f32>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct DataSlot {
    pub slot: i32,
    pub uid: u32,
    #[serde(with = "guid_d")]
    pub spawn_id: Uuid,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Event {
    pub name: String,
    pub delay_seconds: i32,
    pub repeat: bool,
    pub enabled: bool,
    pub actions: Vec<Action>,
    pub conditions: Vec<Condition>,
}
impl Default for Event {
    fn default() -> Self {
        Self {
            name: String::new(),
            delay_seconds: 10,
            repeat: false,
            enabled: true,
            actions: vec![],
            conditions: vec![],
        }
    }
}
/// Numeric kinds preserve System.Text.Json's enum representation: Message=0,
/// Diplomacy=1, SpawnUnit=2, Victory=3, Defeat=4.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Action {
    pub kind: i32,
    pub text: String,
    pub team: i32,
    pub other_team: i32,
    pub hostile: bool,
    pub alias: String,
    pub x: f32,
    pub z: f32,
    pub count: i32,
    pub move_destination: Option<WorldPosition>,
}
impl Default for Action {
    fn default() -> Self {
        Self {
            kind: 0,
            text: String::new(),
            team: 0,
            other_team: 1,
            hostile: true,
            alias: String::new(),
            x: 8000.,
            z: 8000.,
            count: 10,
            move_destination: None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldPosition {
    pub x: i32,
    pub z: i32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Condition {
    pub kind: i32,
    #[serde(with = "guid_d")]
    pub target_id: Uuid,
    pub min_x: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_z: i32,
}
impl Default for Condition {
    fn default() -> Self {
        Self {
            kind: 0,
            target_id: Uuid::nil(),
            min_x: 0,
            min_z: 0,
            max_x: 16383,
            max_z: 16383,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StartCamera {
    pub enabled: bool,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub zoom: f64,
    pub primitive: i32,
}
impl Default for StartCamera {
    fn default() -> Self {
        Self {
            enabled: true,
            x: 8192.,
            y: 0.,
            z: 8192.,
            zoom: 5.,
            primitive: 0,
        }
    }
}
pub fn runtime_key(id: Uuid, suffix: &str) -> String {
    format!("ARM_OBJECT_{}_{}", id.simple(), suffix)
}
/// Computes deterministic legacy UUID matching C# ScenarioObjectIdentity.LegacyId.
pub fn legacy_spawn_id(index: usize) -> Uuid {
    let h = against_rome_data::sha256::sha256(
        format!("AgainstRomeScenarioLegacy:{index}").as_bytes(),
    );
    let mut bytes: [u8; 16] = h[..16].try_into().unwrap();
    bytes[..4].reverse();
    bytes[4..6].reverse();
    bytes[6..8].reverse();
    Uuid::from_bytes(bytes)
}
/// serde's derived structs also accept positional JSON arrays (`[]`, `[6,[],[],[]]`), which System.Text.Json
/// refuses. `arm_scenario.json` is object-only, so every record position must be an object (or null where
/// the field is optional); the typed parse that follows then rejects nulls and wrong types.
fn require_object_shapes(data: &[u8]) -> Result<()> {
    let root: serde_json::Value = serde_json::from_slice(data).map_err(|e| e.to_string())?;
    let is_object = |v: &serde_json::Value| v.is_object();
    let list = |v: &serde_json::Value, key: &str| -> Vec<serde_json::Value> {
        v.get(key).and_then(|x| x.as_array()).cloned().unwrap_or_default()
    };
    if !is_object(&root) {
        return Err("scenario root must be a JSON object".into());
    }
    let mut records = list(&root, "Spawns");
    records.extend(list(&root, "DataSlots"));
    let events = list(&root, "Events");
    for event in &events {
        records.push(event.clone());
        for key in ["Actions", "Conditions"] {
            records.extend(list(event, key));
        }
        for action in list(event, "Actions") {
            if let Some(destination) = action.get("MoveDestination").filter(|d| !d.is_null()) {
                records.push(destination.clone());
            }
        }
    }
    if records.iter().all(is_object) {
        Ok(())
    } else {
        Err("scenario records must be JSON objects".into())
    }
}
impl ScenarioDocument {
    /// Removes editor metadata only; it never deletes an object from native DATA.
    /// Referenced objects must be detached from event conditions first.
    pub fn remove_spawn(&mut self, id: Uuid) -> Result<Spawn> {
        let index = self
            .spawns
            .iter()
            .position(|s| s.id == id)
            .ok_or("spawn not found")?;
        if self
            .events
            .iter()
            .any(|e| e.conditions.iter().any(|c| c.target_id == id))
        {
            return Err("spawn is still referenced by event conditions".into());
        }
        let spawn = self.spawns.remove(index);
        self.data_slots.retain(|b| b.spawn_id != id);
        Ok(spawn)
    }
    /// Returns the DATA slot binding for the given spawn, if any.
    pub fn data_binding(&self, spawn_id: Uuid) -> Option<&DataSlot> {
        if spawn_id.is_nil() {
            return None;
        }
        self.data_slots.iter().find(|b| b.spawn_id == spawn_id)
    }
    /// Safely binds or updates a prebuilt object's DATA slot and UID.
    pub fn bind_prebuilt(&mut self, spawn_id: Uuid, slot: i32, uid: u32) -> Result<()> {
        if spawn_id.is_nil() {
            return Err("cannot bind nil spawn ID".into());
        }
        let spawn = self
            .spawns
            .iter()
            .find(|s| s.id == spawn_id)
            .ok_or("spawn not found")?;
        if !spawn.prebuilt {
            return Err("only prebuilt objects can be bound to DATA slots".into());
        }
        if !(0..14000).contains(&slot) || uid == 0 {
            return Err("invalid DATA slot or UID".into());
        }
        if self
            .data_slots
            .iter()
            .any(|b| b.slot == slot && b.spawn_id != spawn_id)
        {
            return Err(format!("DATA slot {slot} is already bound to another object"));
        }
        if let Some(existing) = self.data_slots.iter_mut().find(|b| b.spawn_id == spawn_id) {
            existing.slot = slot;
            existing.uid = uid;
        } else {
            self.data_slots.push(DataSlot {
                slot,
                uid,
                spawn_id,
            });
        }
        Ok(())
    }
    /// Removes any DATA slot binding for the given spawn.
    pub fn unbind_prebuilt(&mut self, spawn_id: Uuid) {
        if !spawn_id.is_nil() {
            self.data_slots.retain(|b| b.spawn_id != spawn_id);
        }
    }
    /// Ensures all spawns have persistent non-nil unique identities and aligns legacy slot bindings.
    pub fn prepare_identities(&mut self, legacy: bool) -> Result<()> {
        let mut ids = std::collections::HashSet::new();
        for (index, spawn) in self.spawns.iter_mut().enumerate() {
            if spawn.id.is_nil() {
                spawn.id = if legacy {
                    legacy_spawn_id(index)
                } else {
                    uuid::Uuid::new_v4()
                };
            }
            if !ids.insert(spawn.id) {
                return Err("duplicate spawn ID in scenario".into());
            }
        }
        let prebuilt_ids: Vec<_> = self
            .spawns
            .iter()
            .filter(|s| s.prebuilt)
            .map(|s| s.id)
            .collect();
        if legacy
            && self.data_slots.len() == prebuilt_ids.len()
            && self.data_slots.iter().all(|s| s.spawn_id.is_nil())
        {
            for (slot, id) in self.data_slots.iter_mut().zip(prebuilt_ids) {
                slot.spawn_id = id;
            }
        }
        self.validate(None)
    }
    /// Validates all prebuilt bindings against an active DataBindingSource.
    pub fn sync_prebuilt_bindings(&mut self, source: &crate::DataBindingSource) -> Result<usize> {
        source.verify_unchanged()?;
        crate::verify_binding_snapshot(self, source.active_bindings())?;
        Ok(self.spawns.iter().filter(|s| s.prebuilt).count())
    }
    /// Preflight check for scenario save; validates structure and optional DATA bindings.
    pub fn preflight_validate(
        &self,
        aliases: Option<&[String]>,
        data_source: Option<&crate::DataBindingSource>,
    ) -> Result<()> {
        self.validate(aliases)?;
        if let Some(source) = data_source {
            source.verify_unchanged()?;
            crate::verify_binding_snapshot(self, source.active_bindings())?;
        }
        Ok(())
    }
    /// Converts this document's events into a visual EventGraph.
    pub fn to_event_graph(&self) -> crate::graph::EventGraph {
        crate::graph::EventGraphConverter::from_scenario_events(&self.events)
    }
    /// Compiles an EventGraph draft and applies it to this document with fail-closed rollback.
    /// Rejects unsupported constructs (Delay nodes and chained triggers), graph errors, orphan/dangling
    /// elements, and invalid scenario states before mutating `self.events`.
    pub fn apply_event_graph(
        &mut self,
        graph: &crate::graph::EventGraph,
        aliases: Option<&[String]>,
    ) -> Result<()> {
        let valid_targets: std::collections::HashSet<Uuid> =
            self.spawns.iter().map(|s| s.id).collect();
        let diagnostics =
            crate::graph::EventGraphValidator::validate(graph, Some(&valid_targets), aliases);
        if diagnostics.iter().any(|d| {
            d.severity == crate::graph::GraphDiagnosticSeverity::Error
                || matches!(
                    d.code.as_str(),
                    "ORPHAN_ACTION" | "DANGLING_CONDITION" | "ORPHAN_DELAY"
                )
        }) {
            return Err(
                "Fix graph errors and disconnected nodes before applying. Delay nodes and chained triggers are unsupported."
                    .into(),
            );
        }
        if graph
            .nodes
            .iter()
            .any(|n| matches!(n, crate::graph::GraphNode::Delay(_)))
            || graph.has_chained_triggers()
        {
            return Err(
                "Fix graph errors and disconnected nodes before applying. Delay nodes and chained triggers are unsupported."
                    .into(),
            );
        }
        let compiled = crate::graph::EventGraphConverter::to_scenario_events(graph)?;
        let mut draft = self.clone();
        draft.events = compiled.clone();
        draft.validate(aliases)?;
        self.events = compiled;
        Ok(())
    }
    /// Appends an event enforcing the 256 event limit and document invariants.
    pub fn add_event(&mut self, event: Event) -> Result<usize> {
        if self.events.len() >= 256 {
            return Err("事件上限為 256 個。".into());
        }
        let mut draft = self.clone();
        draft.events.push(event.clone());
        draft.validate(None)?;
        let idx = self.events.len();
        self.events.push(event);
        Ok(idx)
    }
    /// Duplicates an event at `index` with `suffix`, enforcing the 256 limit and 100-character name cap.
    pub fn duplicate_event(&mut self, index: usize, suffix: &str) -> Result<usize> {
        if self.events.len() >= 256 {
            return Err("事件上限為 256 個。".into());
        }
        if suffix.encode_utf16().count() > 100 {
            return Err("副本名稱後綴過長。".into());
        }
        let source = self.events.get(index).ok_or("event index out of bounds")?;
        let max_base_len = 100usize.saturating_sub(suffix.encode_utf16().count());
        let mut base_name = String::new();
        let mut utf16_count = 0;
        for c in source.name.chars() {
            let c_len = c.len_utf16();
            if utf16_count + c_len > max_base_len {
                break;
            }
            base_name.push(c);
            utf16_count += c_len;
        }
        base_name.push_str(suffix);
        let mut copy = source.clone();
        copy.name = base_name;
        let insert_idx = index + 1;
        self.events.insert(insert_idx, copy);
        Ok(insert_idx)
    }
    /// Removes an event by index with bounds checking.
    pub fn remove_event(&mut self, index: usize) -> Result<Event> {
        if index >= self.events.len() {
            return Err("event index out of bounds".into());
        }
        Ok(self.events.remove(index))
    }
    /// Atomically replaces an event by index with rollback on invariant failure.
    pub fn replace_event(&mut self, index: usize, event: Event) -> Result<()> {
        if index >= self.events.len() {
            return Err("event index out of bounds".into());
        }
        let old = std::mem::replace(&mut self.events[index], event);
        if let Err(e) = self.validate(None) {
            self.events[index] = old;
            return Err(e);
        }
        Ok(())
    }
    /// Reorders an action within an event with rollback on terminal action invariant violation.
    pub fn move_event_action(
        &mut self,
        event_idx: usize,
        action_idx: usize,
        offset: isize,
    ) -> Result<usize> {
        let event = self
            .events
            .get_mut(event_idx)
            .ok_or("event index out of bounds")?;
        let target = action_idx as isize + offset;
        if target < 0 || target >= event.actions.len() as isize || action_idx >= event.actions.len() {
            return Err("action index or target out of bounds".into());
        }
        let target_idx = target as usize;
        event.actions.swap(action_idx, target_idx);
        if let Err(e) = self.validate(None) {
            let event = &mut self.events[event_idx];
            event.actions.swap(action_idx, target_idx);
            return Err(e);
        }
        Ok(target_idx)
    }
    /// Serializes the document as `arm_scenario.json` (Version 6, indented, numeric enums), the
    /// counterpart of `from_json` and of C# `ScenarioDocument.Save`. Fails closed and never mutates
    /// `self`: every float must be finite (serde would otherwise write `null`, which C# refuses to
    /// load) and the document must pass the full `validate(None)` gate, a superset of the C# Save
    /// checks (identity, DATA bindings, condition targets). Alias membership is checked at compile.
    pub fn to_json(&self) -> Result<Vec<u8>> {
        let finite = self.spawns.iter().all(|s| {
            s.x.is_finite() && s.y.is_finite() && s.z.is_finite() && s.raw_rotation.is_none_or(f32::is_finite)
        }) && self
            .events
            .iter()
            .flat_map(|e| &e.actions)
            .all(|a| a.x.is_finite() && a.z.is_finite());
        if !finite {
            return Err("scenario contains a non-finite coordinate".into());
        }
        let mut document = self.clone();
        document.version = 6;
        document.validate(None)?;
        serde_json::to_vec_pretty(&document).map_err(|e| e.to_string())
    }
    pub fn from_json(data: &[u8]) -> Result<Self> {
        let data = data.strip_prefix(&[239, 187, 191]).unwrap_or(data);
        require_object_shapes(data)?;
        let mut d: Self = serde_json::from_slice(data).map_err(|e| e.to_string())?;
        if !(1..=6).contains(&d.version) {
            return Err("unsupported scenario version".into());
        }
        if d.version < 4 {
            d.prepare_identities(true)?;
        } else {
            d.validate(None)?;
        }
        Ok(d)
    }
    pub fn validate(&self, aliases: Option<&[String]>) -> Result<()> {
        let fail = |s: &str| Err(s.to_string());
        let mut ids = std::collections::HashSet::new();
        for s in &self.spawns {
            if s.id.is_nil() || !ids.insert(s.id) {
                return fail("missing/duplicate spawn ID");
            }
            if !s.prebuilt {
                if let Some(names) = aliases {
                    if !names.iter().any(|n| n.eq_ignore_ascii_case(&s.alias)) {
                        return fail("unknown starting object alias");
                    }
                }
            }
            if s.alias.trim().is_empty() || s.alias.contains('\0') {
                return fail("starting object alias must not be empty or contain NUL");
            }
            if !s.x.is_finite()
                || !s.y.is_finite()
                || !s.z.is_finite()
                || !(0. ..=16383.).contains(&s.x)
                || !(0. ..=16383.).contains(&s.z)
            {
                return fail("spawn position must be finite with X/Z inside 0..16383");
            }
            if !(0..=20).contains(&s.count)
                || if s.count > 0 {
                    !(0..=7).contains(&s.team)
                } else {
                    !(-1..=15).contains(&s.team)
                }
            {
                return fail("invalid starting object count/team");
            }
            if let Some(r) = s.raw_rotation {
                let angle = ((r as f64 * 180. / std::f64::consts::PI)
                    .rem_euclid(360.)
                    .round_ties_even() as i32)
                    .rem_euclid(360);
                if !r.is_finite() || !s.prebuilt || angle != s.angle.rem_euclid(360) {
                    return fail("inconsistent raw rotation");
                }
            }
        }
        let mut bound = std::collections::HashSet::new();
        let mut slots = std::collections::HashSet::new();
        for b in &self.data_slots {
            if !b.spawn_id.is_nil()
                && (!(0..14000).contains(&b.slot)
                    || b.uid == 0
                    || !self.spawns.iter().any(|s| s.id == b.spawn_id && s.prebuilt)
                    || !bound.insert(b.spawn_id)
                    || !slots.insert(b.slot))
            {
                return fail("invalid DATA binding");
            }
        }
        if self.events.len() > 256 {
            return fail("too many events");
        }
        for e in &self.events {
            if e.name.trim().is_empty()
                || e.name.encode_utf16().count() > 100 // C# string.Length counts UTF-16 units
                || !(0..=86400).contains(&e.delay_seconds)
                || (e.repeat && e.delay_seconds == 0)
                || e.actions.is_empty()
                || e.actions.len() > 32
                || e.conditions.len() > 32
            {
                return fail("invalid event");
            }
            for (i, a) in e.actions.iter().enumerate() {
                match a.kind {
                    0 => {
                        let (b, _, bad) = encoding_rs::WINDOWS_1251.encode(&a.text);
                        if bad
                            || a.text.trim().is_empty()
                            || a.text.contains('\0')
                            || b.len() > 1000
                        {
                            return fail("invalid message");
                        }
                    }
                    1 => {
                        if !(0..=7).contains(&a.team)
                            || !(0..=7).contains(&a.other_team)
                            || a.team == a.other_team
                        {
                            return fail("invalid diplomacy");
                        }
                    }
                    2 => {
                        if a.alias.trim().is_empty()
                            || a.alias.contains('\0')
                            || !(0..=7).contains(&a.team)
                            || !(1..=20).contains(&a.count)
                            || ![a.x, a.z]
                                .iter()
                                .all(|v| v.is_finite() && (0. ..=16383.).contains(v))
                        {
                            return fail("invalid unit");
                        }
                        if let Some(names) = aliases {
                            if !names.iter().any(|n| n.eq_ignore_ascii_case(&a.alias)) {
                                return fail("unknown unit alias");
                            }
                        }
                    }
                    3 | 4 => {
                        if e.repeat || i + 1 != e.actions.len() {
                            return fail("terminal action must be last and nonrepeating");
                        }
                    }
                    _ => return fail("unsupported action"),
                };
                if let Some(p) = &a.move_destination {
                    if a.kind != 2 || !(0..=16383).contains(&p.x) || !(0..=16383).contains(&p.z) {
                        return fail("invalid move destination");
                    }
                }
            }
            for c in &e.conditions {
                if !(0..=2).contains(&c.kind) || c.target_id.is_nil() {
                    return fail("invalid condition");
                }
                if c.kind == 2
                    && (c.min_x < 0
                        || c.min_z < 0
                        || c.max_x > 16383
                        || c.max_z > 16383
                        || c.min_x > c.max_x
                        || c.min_z > c.max_z)
                {
                    return fail("invalid area");
                }
                if e.enabled {
                    let s = self
                        .spawns
                        .iter()
                        .find(|s| s.id == c.target_id)
                        .ok_or("missing condition target")?;
                    if s.prebuilt && !self.data_slots.iter().any(|b| b.spawn_id == s.id) {
                        return fail("missing prebuilt DATA binding");
                    }
                }
            }
        }
        Ok(())
    }
}
impl StartCamera {
    pub fn validate(&self) -> Result<()> {
        if ![self.x, self.z]
            .iter()
            .all(|v| v.is_finite() && (0. ..=16383.).contains(v))
            || !self.y.is_finite()
            || self.y.abs() > f32::MAX as f64
            || !self.zoom.is_finite()
            || !(0. ..=9.).contains(&self.zoom)
            || !(0..=1).contains(&self.primitive)
        {
            return Err("invalid start camera".into());
        }
        Ok(())
    }
}
