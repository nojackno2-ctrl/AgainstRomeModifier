//! Scenario event templates (portable `.arm-events.json` files).
//!
//! Provides pure serialization, deserialization, and planning for reusable scenario event
//! templates. Events that reference specific map objects (via `Condition.target_id`) are
//! skipped on capture/export. On import, event names are deduplicated to avoid collisions,
//! optional team remapping is applied to action teams, and size/version limits are enforced.

use against_rome_scenario::Event;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// File extension used for exported scenario event templates.
pub const EVENT_TEMPLATE_EXTENSION: &str = "arm-events.json";

/// Injectable file picker type, matching `crate::layout_io::LayoutFilePicker`.
pub type EventFilePicker =
    Box<dyn Fn(&crate::layout_io::LayoutDialogKind) -> Option<PathBuf> + Send + Sync>;

/// Default native file picker for event templates.
pub fn native_event_picker() -> EventFilePicker {
    Box::new(|kind| match kind {
        crate::layout_io::LayoutDialogKind::Save { suggested_name } => rfd::FileDialog::new()
            .set_title("Export scenario events / 匯出劇本事件")
            .add_filter("Scenario events (*.arm-events.json)", &["json"])
            .set_file_name(suggested_name.as_str())
            .save_file(),
        crate::layout_io::LayoutDialogKind::Open => rfd::FileDialog::new()
            .set_title("Import scenario events / 匯入劇本事件")
            .add_filter("Scenario events / JSON", &["json"])
            .pick_file(),
    })
}

/// Appends `.arm-events.json` if the path has no extension or doesn't end with `.json` or `.arm-events.json`.
pub fn with_default_extension(path: &Path) -> PathBuf {
    let str_path = path.to_string_lossy();
    if str_path.ends_with(EVENT_TEMPLATE_EXTENSION) || path.extension().is_some() {
        path.to_path_buf()
    } else {
        let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
        name.push(format!(".{EVENT_TEMPLATE_EXTENSION}"));
        path.with_file_name(name)
    }
}

/// Checks whether an event references a specific map object (i.e. has any condition with a non-nil target_id).
pub fn event_references_object(event: &Event) -> bool {
    event.conditions.iter().any(|c| !c.target_id.is_nil())
}

/// A container of portable scenario events.
#[derive(Clone, Debug, PartialEq)]
pub struct EventTemplate(pub Vec<Event>);

impl EventTemplate {
    pub const KIND: &'static str = "ScenarioEvents";
    pub const CURRENT_VERSION: u32 = 1;

    /// Captures events for export, skipping any events that reference specific map objects.
    /// Returns the template and the count of skipped events.
    pub fn capture(events: &[Event]) -> (Self, usize) {
        let mut kept = Vec::new();
        let mut skipped = 0;
        for event in events {
            if event_references_object(event) {
                skipped += 1;
            } else {
                kept.push(event.clone());
            }
        }
        (Self(kept), skipped)
    }

    /// Serializes the template to pretty-printed JSON text.
    pub fn serialize(&self) -> Result<String, String> {
        self.validate()?;
        let payload = serde_json::json!({
            "version": Self::CURRENT_VERSION,
            "kind": Self::KIND,
            "events": self.0,
        });
        let text = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
        if text.len() > crate::layout_io::MAX_LAYOUT_BYTES {
            return Err("Event template too large / 事件範本過大".into());
        }
        Ok(text)
    }

    /// Deserializes a template from JSON text, enforcing size, version, kind, and event structure.
    pub fn deserialize(text: &str) -> Result<Self, String> {
        let invalid = || "Invalid event template / 事件範本格式無效".to_string();
        if text.len() > crate::layout_io::MAX_LAYOUT_BYTES {
            return Err("Event template too large / 事件範本過大".into());
        }
        let root: Value =
            serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|_| invalid())?;
        let obj = root.as_object().ok_or_else(invalid)?;
        if obj.len() != 3 {
            return Err(invalid());
        }
        let version = root.get("version").and_then(|v| v.as_u64()).ok_or_else(invalid)?;
        if version != Self::CURRENT_VERSION as u64 {
            return Err(format!(
                "Unsupported event template version: {version} (expected {}) / 不支援的事件範本版本：{version}（預期 {}）",
                Self::CURRENT_VERSION,
                Self::CURRENT_VERSION
            ));
        }
        let kind = root.get("kind").and_then(|k| k.as_str()).ok_or_else(invalid)?;
        if kind != Self::KIND {
            return Err(format!(
                "Unsupported event template kind: {kind} (expected {}) / 不支援的事件範本種類：{kind}（預期 {}）",
                Self::KIND,
                Self::KIND
            ));
        }
        let events_val = root.get("events").ok_or_else(invalid)?;
        if !events_val.is_array() {
            return Err(invalid());
        }
        let events: Vec<Event> =
            serde_json::from_value(events_val.clone()).map_err(|_| invalid())?;
        let template = Self(events);
        template.validate()?;
        Ok(template)
    }

    /// Validates basic structural invariants of the template.
    pub fn validate(&self) -> Result<(), String> {
        if self.0.is_empty() {
            return Err("Event template contains no events / 事件範本未包含任何事件".into());
        }
        if self.0.len() > 256 {
            return Err(
                "Too many events in template (max 256) / 事件範本包含過多事件（上限 256）".into(),
            );
        }
        for event in &self.0 {
            if event.name.trim().is_empty() || event.actions.is_empty() {
                return Err("Invalid event in template / 事件範本內含無效事件".into());
            }
        }
        Ok(())
    }

    /// Plans the import of events into a scenario document:
    /// - renames colliding event names using `crate::scenario_ui::unused_event_name`
    /// - optionally remaps team numbers in actions (action.team and action.other_team)
    /// - ensures total event count does not exceed the 256 limit
    pub fn plan(
        &self,
        existing_events: &[Event],
        remap: Option<(i32, i32)>,
    ) -> Result<Vec<Event>, String> {
        self.validate()?;
        if existing_events.len() + self.0.len() > 256 {
            return Err("Exceeds 256-event limit / 超過 256 個事件上限".into());
        }
        let mut accumulated: Vec<Event> = existing_events.to_vec();
        let mut planned = Vec::with_capacity(self.0.len());
        for event in &self.0 {
            let mut e = event.clone();
            if let Some((from, to)) = remap {
                for a in &mut e.actions {
                    if a.team == from {
                        a.team = to;
                    }
                    if a.other_team == from {
                        a.other_team = to;
                    }
                }
            }
            if accumulated.iter().any(|existing| existing.name.eq_ignore_ascii_case(&e.name)) {
                e.name = crate::scenario_ui::unused_event_name(&e.name, &accumulated);
            }
            accumulated.push(e.clone());
            planned.push(e);
        }
        Ok(planned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use against_rome_scenario::{Action, Condition, Event};
    use uuid::Uuid;

    fn sample_event(name: &str, team: i32, other_team: i32) -> Event {
        Event {
            name: name.to_string(),
            delay_seconds: 30,
            repeat: false,
            enabled: true,
            actions: vec![
                Action {
                    kind: 0,
                    text: "Greetings".into(),
                    team,
                    other_team,
                    ..Default::default()
                },
                Action {
                    kind: 1,
                    team,
                    other_team,
                    hostile: true,
                    ..Default::default()
                },
            ],
            conditions: vec![],
        }
    }

    #[test]
    fn export_import_roundtrip_preserves_events_and_renames_on_collision() {
        let events = vec![
            sample_event("Wave Alpha", 1, 0),
            sample_event("Wave Beta", 2, 0),
        ];
        let (template, skipped) = EventTemplate::capture(&events);
        assert_eq!(skipped, 0);
        let json = template.serialize().expect("serialize should succeed");
        assert!(json.contains("\"version\": 1"));
        assert!(json.contains("\"kind\": \"ScenarioEvents\""));

        let loaded = EventTemplate::deserialize(&json).expect("deserialize should succeed");
        assert_eq!(loaded.0.len(), 2);

        // Test planning with name collisions against existing events
        let existing = vec![
            sample_event("Wave Alpha", 0, 1),
            sample_event("Wave Alpha_2", 0, 1),
        ];
        let planned = loaded.plan(&existing, None).expect("plan should succeed");
        assert_eq!(planned.len(), 2);
        assert_eq!(planned[0].name, "Wave Alpha_3");
        assert_eq!(planned[1].name, "Wave Beta");
        assert_eq!(planned[0].actions[0].text, "Greetings");
        assert_eq!(planned[0].delay_seconds, 30);
    }

    #[test]
    fn object_targeted_events_skipped_on_export_with_count() {
        let normal_event = sample_event("Normal", 1, 0);
        let mut object_targeted_event_1 = sample_event("Kill Target", 1, 0);
        object_targeted_event_1.conditions.push(Condition {
            kind: 1,
            target_id: Uuid::new_v4(),
            ..Default::default()
        });
        let mut object_targeted_event_2 = sample_event("Area Guard", 1, 0);
        object_targeted_event_2.conditions.push(Condition {
            kind: 0,
            target_id: Uuid::new_v4(),
            ..Default::default()
        });
        let mut nil_target_event = sample_event("Timer Only", 1, 0);
        nil_target_event.conditions.push(Condition {
            kind: 2,
            target_id: Uuid::nil(),
            ..Default::default()
        });

        let events = vec![
            normal_event,
            object_targeted_event_1,
            object_targeted_event_2,
            nil_target_event,
        ];
        let (template, skipped) = EventTemplate::capture(&events);
        assert_eq!(skipped, 2);
        assert_eq!(template.0.len(), 2);
        assert_eq!(template.0[0].name, "Normal");
        assert_eq!(template.0[1].name, "Timer Only");
    }

    #[test]
    fn team_remap_applies_to_action_team_and_other_team() {
        let events = vec![sample_event("Ambush", 1, 2)];
        let template = EventTemplate(events);
        let planned = template.plan(&[], Some((1, 4))).expect("plan should succeed");
        assert_eq!(planned[0].actions[0].team, 4);
        assert_eq!(planned[0].actions[0].other_team, 2);
        assert_eq!(planned[0].actions[1].team, 4);
        assert_eq!(planned[0].actions[1].other_team, 2);

        // Remap other_team
        let planned_other = template.plan(&[], Some((2, 5))).expect("plan should succeed");
        assert_eq!(planned_other[0].actions[0].team, 1);
        assert_eq!(planned_other[0].actions[0].other_team, 5);
        assert_eq!(planned_other[0].actions[1].team, 1);
        assert_eq!(planned_other[0].actions[1].other_team, 5);
    }

    #[test]
    fn future_version_wrong_kind_oversized_rejected() {
        // Future version
        let future_json = r#"{
            "version": 2,
            "kind": "ScenarioEvents",
            "events": [{"Name":"E","DelaySeconds":10,"Repeat":false,"Enabled":true,"Actions":[{"Kind":0,"Text":"hi","Team":0,"OtherTeam":1,"Hostile":false,"Alias":"","X":0.0,"Z":0.0,"Count":0,"MoveDestination":null}],"Conditions":[]}]
        }"#;
        let err = EventTemplate::deserialize(future_json).unwrap_err();
        assert!(err.contains("Unsupported event template version: 2"));

        // Wrong kind
        let wrong_kind_json = r#"{
            "version": 1,
            "kind": "WrongKind",
            "events": [{"Name":"E","DelaySeconds":10,"Repeat":false,"Enabled":true,"Actions":[{"Kind":0,"Text":"hi","Team":0,"OtherTeam":1,"Hostile":false,"Alias":"","X":0.0,"Z":0.0,"Count":0,"MoveDestination":null}],"Conditions":[]}]
        }"#;
        let err = EventTemplate::deserialize(wrong_kind_json).unwrap_err();
        assert!(err.contains("Unsupported event template kind: WrongKind"));

        // Oversized
        let huge = " ".repeat(crate::layout_io::MAX_LAYOUT_BYTES + 10);
        let err = EventTemplate::deserialize(&huge).unwrap_err();
        assert!(err.contains("Event template too large"));
    }
}
