//! 編輯器專用的腳本生成配置；不攜帶 DATA 綁定或原始識別碼。
use against_rome_scenario::{ScenarioDocument, Spawn};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashSet};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub(crate) struct SpawnLayout(pub Vec<Spawn>);

impl SpawnLayout {
    pub fn capture(doc: &ScenarioDocument, ids: &HashSet<Uuid>) -> (Self, usize) {
        let selected: Vec<_> = doc.spawns.iter().filter(|s| ids.contains(&s.id)).collect();
        let mut spawns: Vec<_> = selected
            .iter()
            .copied()
            .filter(|s| !s.prebuilt && !doc.data_slots.iter().any(|b| b.spawn_id == s.id))
            .cloned()
            .collect();
        let skipped = selected.len() - spawns.len();
        if !spawns.is_empty() {
            let n = spawns.len() as f64;
            let x = spawns.iter().map(|s| s.x as f64).sum::<f64>() / n;
            let z = spawns.iter().map(|s| s.z as f64).sum::<f64>() / n;
            for s in &mut spawns {
                s.x = (s.x as f64 - x) as f32;
                s.z = (s.z as f64 - z) as f32;
            }
        }
        (Self(spawns), skipped)
    }

    pub fn serialize(&self) -> Result<String, String> {
        self.validate()?;
        let entries: Vec<_> = self
            .0
            .iter()
            .map(|s| {
                json!({"alias":s.alias,"x":s.x,"z":s.z,
            "y":s.y,"team":s.team,"count":s.count,"angle":s.angle})
            })
            .collect();
        let text = serde_json::to_string_pretty(
            &json!({"version":1,"kind":"ScriptSpawns","entries":entries}),
        )
        .map_err(|e| e.to_string())?;
        if text.len() > crate::layout_io::MAX_LAYOUT_BYTES {
            return Err("Layout too large / 配置過大".into());
        }
        Ok(text)
    }

    pub fn deserialize(text: &str) -> Result<Self, String> {
        let invalid = || "Invalid script layout / 腳本配置格式無效".to_string();
        if text.len() > crate::layout_io::MAX_LAYOUT_BYTES {
            return Err(invalid());
        }
        let root: Value =
            serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|_| invalid())?;
        let obj = root.as_object().ok_or_else(invalid)?;
        if obj.len() != 3 || root["version"].as_u64() != Some(1) || root["kind"] != "ScriptSpawns" {
            return Err("Unsupported layout version or kind / 不支援的配置版本或種類".into());
        }
        let mut spawns = Vec::new();
        for e in root["entries"].as_array().ok_or_else(invalid)? {
            let obj = e.as_object().ok_or_else(invalid)?;
            // 七個設計欄位必須完整且不得夾帶識別碼或綁定。
            if obj.len() != 7
                || obj.keys().any(|k| {
                    !["alias", "x", "z", "y", "team", "count", "angle"].contains(&k.as_str())
                })
            {
                return Err(invalid());
            }
            let f = |k: &str| e[k].as_f64().map(|v| v as f32).ok_or_else(invalid);
            let i = |k: &str| {
                e[k].as_i64()
                    .and_then(|v| i32::try_from(v).ok())
                    .ok_or_else(invalid)
            };
            spawns.push(Spawn {
                alias: e["alias"].as_str().ok_or_else(invalid)?.into(),
                x: f("x")?,
                z: f("z")?,
                y: f("y")?,
                team: i("team")?,
                count: i("count")?,
                angle: i("angle")?,
                id: Uuid::nil(),
                prebuilt: false,
                raw_rotation: None,
            });
        }
        let layout = Self(spawns);
        layout.validate()?;
        Ok(layout)
    }

    fn validate(&self) -> Result<(), String> {
        if self.0.is_empty()
            || self.0.len() > 4096
            || self.0.iter().any(|s| {
                s.alias.trim().is_empty()
                    || !s.x.is_finite()
                    || !s.z.is_finite()
                    || !s.y.is_finite()
                    || s.prebuilt
                    || s.raw_rotation.is_some()
            })
        {
            return Err("Invalid script layout / 腳本配置格式無效".into());
        }
        Ok(())
    }

    pub fn plan(
        &self,
        aliases: &[String],
        anchor: (f32, f32),
        remap: Option<(i32, i32)>,
    ) -> Result<(Vec<Spawn>, usize), String> {
        self.validate()?;
        let unknown: BTreeSet<_> = self
            .0
            .iter()
            .filter(|s| !aliases.iter().any(|a| a.eq_ignore_ascii_case(&s.alias)))
            .map(|s| s.alias.as_str())
            .collect();
        if !unknown.is_empty() {
            return Err(format!(
                "Unknown aliases / 未知別名：{}",
                unknown.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }
        if !anchor.0.is_finite() || !anchor.1.is_finite() {
            return Err("Invalid anchor / 錨點無效".into());
        }
        let mut result = self.0.clone();
        let mut clamped = 0;
        for s in &mut result {
            let (x, z) = (anchor.0 as f64 + s.x as f64, anchor.1 as f64 + s.z as f64);
            let (cx, cz) = (x.clamp(0., 16383.), z.clamp(0., 16383.));
            if x != cx || z != cz {
                clamped += 1;
            }
            s.x = cx as f32;
            s.z = cz as f32;
            s.id = Uuid::new_v4();
            if let Some((from, to)) = remap {
                if s.team == from {
                    s.team = to;
                }
            }
        }
        Ok((result, clamped))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn document() -> ScenarioDocument {
        ScenarioDocument {
            spawns: vec![
                Spawn {
                    alias: "house".into(),
                    x: 100.,
                    z: 200.,
                    y: 13.5,
                    team: 2,
                    count: 0,
                    angle: 405,
                    id: Uuid::new_v4(),
                    ..Default::default()
                },
                Spawn {
                    alias: "legion".into(),
                    x: 300.,
                    z: 600.,
                    y: -2.,
                    team: 3,
                    count: 17,
                    angle: 90,
                    id: Uuid::new_v4(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }
    #[test]
    fn spawn_layout_roundtrip_fields_positions_and_fresh_ids() {
        let doc = document();
        let (layout, skipped) =
            SpawnLayout::capture(&doc, &doc.spawns.iter().map(|s| s.id).collect());
        assert_eq!(skipped, 0);
        let json = layout.serialize().unwrap();
        assert!(!json.contains(&doc.spawns[0].id.to_string()));
        let layout = SpawnLayout::deserialize(&json).unwrap();
        assert_eq!((layout.0[0].x, layout.0[0].z), (-100., -200.));
        let aliases = vec!["house".into(), "legion".into()];
        let (first, clamped) = layout.plan(&aliases, (200., 400.), None).unwrap();
        let (second, _) = layout.plan(&aliases, (200., 400.), None).unwrap();
        assert_eq!(clamped, 0);
        for (i, s) in first.iter().enumerate() {
            assert_ne!(s.id, doc.spawns[i].id);
            assert_ne!(s.id, second[i].id);
            let mut expected = doc.spawns[i].clone();
            expected.id = s.id;
            assert_eq!(*s, expected);
        }
    }
    #[test]
    fn spawn_layout_remap_and_count_clamped_spawns_once() {
        let doc = document();
        let (layout, _) = SpawnLayout::capture(&doc, &doc.spawns.iter().map(|s| s.id).collect());
        let (spawns, n) = layout
            .plan(&["house".into(), "legion".into()], (0., 0.), Some((2, 5)))
            .unwrap();
        assert_eq!(n, 1);
        assert_eq!((spawns[0].x, spawns[0].z), (0., 0.));
        assert_eq!((spawns[0].team, spawns[1].team), (5, 3));
        let (spawns, n) = layout
            .plan(&["house".into(), "legion".into()], (16383., 16383.), None)
            .unwrap();
        assert_eq!(n, 1);
        assert_eq!((spawns[1].x, spawns[1].z), (16383., 16383.));
    }
    #[test]
    fn spawn_layout_skips_bound_and_prebuilt_before_centroid() {
        let mut doc = document();
        let mut prebuilt = doc.spawns[0].clone();
        prebuilt.id = Uuid::new_v4();
        prebuilt.prebuilt = true;
        doc.data_slots.push(against_rome_scenario::model::DataSlot {
            spawn_id: doc.spawns[0].id,
            ..Default::default()
        });
        doc.spawns.push(prebuilt);
        let (layout, n) = SpawnLayout::capture(&doc, &doc.spawns.iter().map(|s| s.id).collect());
        assert_eq!(n, 2);
        assert_eq!(layout.0.len(), 1);
        assert_eq!((layout.0[0].x, layout.0[0].z), (0., 0.));
    }
    #[test]
    fn spawn_layout_rejects_malformed_and_future_versions() {
        let doc = document();
        let (layout, _) = SpawnLayout::capture(&doc, &doc.spawns.iter().map(|s| s.id).collect());
        let text = layout.serialize().unwrap();
        for text in [
            "{".into(),
            text.replace("\"version\": 1", "\"version\": 2"),
            text.replace("\"team\": 2", "\"team\": null"),
            text.replace("\"alias\":", "\"id\":"),
        ] {
            assert!(SpawnLayout::deserialize(&text).is_err());
        }
    }
}
