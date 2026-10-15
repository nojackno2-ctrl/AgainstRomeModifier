use crate::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Clone, Debug, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
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
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_rotation: Option<f32>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct DataSlot {
    pub slot: i32,
    pub uid: u32,
    pub spawn_id: Uuid,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WorldPosition {
    pub x: i32,
    pub z: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Condition {
    pub kind: i32,
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
    pub fn from_json(data: &[u8]) -> Result<Self> {
        let data = data.strip_prefix(&[239, 187, 191]).unwrap_or(data);
        let mut d: Self = serde_json::from_slice(data).map_err(|e| e.to_string())?;
        if !(1..=6).contains(&d.version) {
            return Err("unsupported scenario version".into());
        }
        if d.version < 4 {
            for (i, s) in d.spawns.iter_mut().enumerate() {
                if s.id.is_nil() {
                    let h = against_rome_data::sha256::sha256(
                        format!("AgainstRomeScenarioLegacy:{i}").as_bytes(),
                    );
                    let mut bytes: [u8; 16] = h[..16].try_into().unwrap();
                    bytes[..4].reverse();
                    bytes[4..6].reverse();
                    bytes[6..8].reverse();
                    s.id = Uuid::from_bytes(bytes)
                }
            }
            let p: Vec<_> = d
                .spawns
                .iter()
                .filter(|s| s.prebuilt)
                .map(|s| s.id)
                .collect();
            if p.len() == d.data_slots.len() && d.data_slots.iter().all(|s| s.spawn_id.is_nil()) {
                for (s, id) in d.data_slots.iter_mut().zip(p) {
                    s.spawn_id = id
                }
            }
        }
        d.validate(None)?;
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
                || e.name.chars().count() > 100
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
