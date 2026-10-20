//! Detached Spawn -> DATA transaction. No filesystem writes.
use super::native::{fixed_displacement, ObjectStore};
use crate::placement_edit::ScenarioFamilyCopyRequest;
use against_rome_scenario::model::{DataSlot, ScenarioDocument, Spawn};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use uuid::Uuid;
#[derive(Clone)]
pub struct Template { pub store: ObjectStore, pub slot: usize, pub uid: u32 }
pub struct ScenarioCandidateResult {
    pub store: ObjectStore,
    pub scenario: ScenarioDocument,
    pub skipped: Vec<Spawn>,
}
fn angle(r: f32) -> i32 {
    ((r as f64 * 180. / std::f64::consts::PI).rem_euclid(360.).round_ties_even() as i32).rem_euclid(360)
}
/// Ordinary template reconstruction and exact native-family retention. Inputs remain detached
/// even if a later allocation fails. Explicit family-copy requests are a separate API/slice.
pub fn apply_to_candidate(store: &ObjectStore, previous: &ScenarioDocument,
    next: &ScenarioDocument, template_for: &dyn Fn(&Spawn) -> Option<Template>)
    -> Result<ScenarioCandidateResult, String> {
    apply_core(store, previous, next, template_for, None)
}

fn norm(angle: i32) -> i32 { angle.rem_euclid(360) }

/// Port of C# `ScenarioLevelObjects.ApplyToCandidate` with family copies: every request is
/// preflighted against the transaction-start snapshot, then the source families are cloned (in
/// sorted target-id order) into a detached store before the ordinary retain/rebuild core runs.
/// Inputs are never mutated; any failure leaves nothing to publish.
pub fn apply_to_candidate_with_copies(store: &ObjectStore, previous: &ScenarioDocument,
    next: &ScenarioDocument, template_for: &dyn Fn(&Spawn) -> Option<Template>,
    copies: Option<&HashMap<Uuid, ScenarioFamilyCopyRequest>>,
    native_alias: Option<&dyn Fn(i32) -> Option<String>>)
    -> Result<ScenarioCandidateResult, String> {
    let Some(copies) = copies.filter(|c| !c.is_empty()) else {
        return apply_core(store, previous, next, template_for, None);
    };
    previous.validate(None)?;
    next.validate(None)?;
    let mut prev = previous.clone();
    let mut working = store.clone();
    let initial: HashMap<usize, _> = store.objects().into_iter().map(|o| (o.slot, o)).collect();
    for (target, req) in copies {
        if target.is_nil() { return Err("副本請求的目標 Guid 不能為空。".into()); }
        if previous.spawns.iter().any(|s| s.id == *target) || previous.data_slots.iter().any(|b| b.spawn_id == *target) {
            return Err("副本目標 Guid 不能是既有場景物件。".into());
        }
        let Some(t) = next.spawns.iter().find(|s| s.id == *target).filter(|s| s.prebuilt) else {
            return Err("副本請求必須對應至 next 中新增且有效的預建物件。".into());
        };
        if !t.x.is_finite() || !t.y.is_finite() || !t.z.is_finite() { return Err("副本目標座標必須為有限數值。".into()); }
        if !(0..=8).contains(&t.team) { return Err("預建物件的隊伍必須介於 0 與 8。".into()); }
        let root = usize::try_from(req.source_slot).ok().and_then(|s| initial.get(&s))
            .filter(|r| r.uid == req.source_uid)
            .ok_or_else(|| format!("副本來源槽位 {} 的 root UID 無效。", req.source_slot))?;
        if store.family(root.slot, root.uid).is_err() {
            return Err(format!("副本來源槽位 {} 的群組無效或非獨立支援群組。", req.source_slot));
        }
        let radians = store.native_rotation(root.slot, root.uid)?;
        let translate_only = || "群組副本目前只能平移；變更隊伍或方向尚未支援。".to_string();
        if req.is_native_source {
            if previous.data_slots.iter().any(|b| b.slot == req.source_slot && b.uid == req.source_uid && !b.spawn_id.is_nil()) {
                return Err(format!("原生副本來源槽位 {} 已存在場景綁定，應使用一般副本請求。", req.source_slot));
            }
            let resolver = native_alias.ok_or("複製原生群組時必須提供可信原生別名解析器。")?;
            match resolver(root.type_id as i32) {
                Some(a) if !a.trim().is_empty() && a.eq_ignore_ascii_case(&t.alias) => {}
                _ => return Err(format!("原生副本來源 TypeId {} 解析的別名與目標別名不一致。", root.type_id)),
            }
            if root.team as i32 != t.team { return Err(translate_only()); }
            let raw = t.raw_rotation.filter(|r| r.is_finite())
                .ok_or("原生群組副本目標必須明確提供有限的原生旋轉弧度。")?;
            if !radians.is_finite() || raw.to_bits() != radians.to_bits() {
                return Err("原生群組副本目標旋轉弧度必須與來源 bit-exact 一致。".into());
            }
            if norm(t.angle) != angle(radians) { return Err(translate_only()); }
        } else {
            let matching: Vec<_> = previous.data_slots.iter()
                .filter(|b| b.slot == req.source_slot && b.uid == req.source_uid && !b.spawn_id.is_nil()).collect();
            if matching.len() != 1 {
                return Err(format!("副本來源槽位 {} 在 previous 中沒有唯一有效的綁定。", req.source_slot));
            }
            let src = previous.spawns.iter().find(|s| s.id == matching[0].spawn_id).filter(|s| s.prebuilt)
                .ok_or("副本來源在 previous 中必須是預建物件。")?;
            if !src.alias.eq_ignore_ascii_case(&t.alias) { return Err("副本物件的別名必須與來源一致。".into()); }
            if src.team != t.team || root.team as i32 != t.team { return Err(translate_only()); }
            if src.raw_rotation.is_some() || t.raw_rotation.is_some() {
                match (src.raw_rotation, t.raw_rotation) {
                    (Some(a), Some(b)) if a.is_finite() && b.is_finite() && a.to_bits() == b.to_bits()
                        && radians.to_bits() == b.to_bits() && norm(src.angle) == norm(t.angle) => {}
                    _ => return Err(translate_only()),
                }
            } else {
                let actual = radians as f64 * 180. / std::f64::consts::PI;
                let desired = norm(t.angle) as f64;
                let diff = ((actual - desired + 540.).rem_euclid(360.) - 180.).abs();
                if norm(src.angle) as f64 != desired || !actual.is_finite() || diff > 0.001 { return Err(translate_only()); }
            }
        }
    }
    let mut ordered: Vec<_> = copies.iter().collect();
    ordered.sort_by_key(|(id, _)| **id);
    let (mut slots, mut uids) = (HashSet::new(), HashSet::new());
    for (target, req) in ordered {
        let t = next.spawns.iter().find(|s| s.id == *target).expect("preflighted").clone();
        let root = &initial[&(req.source_slot as usize)];
        let delta = fixed_displacement(root.position, [t.x, t.y, t.z])?;
        let (slot, uid) = working.clone_family_fixed(req.source_slot as usize, req.source_uid, delta)
            .map_err(|e| format!("建立群組副本候選失敗（池容量不足或配置無法滿足）：{e}"))?;
        if !slots.insert(slot) || !uids.insert(uid) { return Err("多個群組副本之間發生槽位或 UID 衝突。".into()); }
        prev.spawns.push(t);
        prev.data_slots.push(DataSlot { slot: slot as i32, uid, spawn_id: *target });
    }
    let ids: HashSet<Uuid> = copies.keys().copied().collect();
    apply_core(&working, &prev, next, template_for, Some(&ids))
}

fn apply_core(store: &ObjectStore, previous: &ScenarioDocument,
    next: &ScenarioDocument, template_for: &dyn Fn(&Spawn) -> Option<Template>,
    copied: Option<&HashSet<Uuid>>)
    -> Result<ScenarioCandidateResult, String> {
    previous.validate(None)?;
    next.validate(None)?;
    let mut working = store.clone();
    let mut scenario = next.clone();
    scenario.data_slots.clear();
    let world = store.objects();
    let mut retained = BTreeMap::new();
    let mut occupied = BTreeSet::new();
    let mut owned = BTreeMap::new();
    for binding in &previous.data_slots {
        let old = previous.spawns.iter().find(|s| s.id == binding.spawn_id && s.prebuilt);
        let current = next.spawns.iter().find(|s| s.id == binding.spawn_id && s.prebuilt);
        let (Some(old), Some(current)) = (old, current) else { continue; };
        if !old.alias.eq_ignore_ascii_case(&current.alias) { continue; }
        let Some(root) = world.iter().find(|o| o.slot as i32 == binding.slot && o.uid == binding.uid) else { continue; };
        if copied.is_none_or(|c| !c.contains(&current.id)) && old.raw_rotation.is_none() && current.raw_rotation.is_none()
            && store.singleton_template(root.slot, root.uid).is_ok() {
            owned.insert(current.id, Template { store: store.clone(), slot: root.slot, uid: root.uid });
            continue;
        }
        let family = store.family(root.slot, root.uid)?;
        let radians = store.native_rotation(root.slot, root.uid)?;
        if old.team != current.team || root.team as i32 != current.team || old.angle.rem_euclid(360) != current.angle.rem_euclid(360) {
            return Err("retained family only supports translation".into());
        }
        match (old.raw_rotation, current.raw_rotation) {
            (Some(a), Some(b)) if a.to_bits() == b.to_bits() && b.to_bits() == radians.to_bits()
                && angle(b) == current.angle.rem_euclid(360) => {},
            (None, None) if radians.is_finite() && {
                let actual = radians as f64 * 180. / std::f64::consts::PI;
                ((actual-current.angle as f64+180.).rem_euclid(360.)-180.).abs() <= 0.001
            } => {},
            _ => return Err("unverified native rotation".into()),
        }
        if family.iter().any(|s| !occupied.insert(*s)) || retained.contains_key(&current.id) {
            return Err("overlapping family bindings".into());
        }
        retained.insert(current.id, (binding.clone(), fixed_displacement(root.position, [current.x,current.y,current.z])?));
    }
    for s in &next.spawns {
        if s.prebuilt && (!(0..=8).contains(&s.team) || (s.raw_rotation.is_some() && !retained.contains_key(&s.id))) {
            return Err("invalid prebuilt team or unverified native rotation".into());
        }
    }
    for old in &previous.spawns {
        if old.prebuilt && old.raw_rotation.is_some() && next.spawns.iter().any(|s| s.id == old.id)
            && !retained.contains_key(&old.id) { return Err("native family binding cannot be dropped or changed".into()); }
    }
    for b in &previous.data_slots {
        if retained.values().any(|(r,_)| r.slot == b.slot) { continue; }
        if world.iter().any(|o| o.slot as i32 == b.slot && o.uid == b.uid) {
            // Linked families may be deleted, but never silently rebuilt from a singleton.
            if store.family(b.slot as usize,b.uid)?.len() > 1 && next.spawns.iter().any(|s| s.id == b.spawn_id && s.prebuilt) {
                return Err("linked family cannot be reconstructed".into());
            }
            working.remove_family(b.slot as usize,b.uid)?;
        }
    }
    let mut skipped = vec![];
    for s in next.spawns.iter().filter(|s| s.prebuilt) {
        if let Some((b,delta)) = retained.get(&s.id) {
            if *delta != [0;3] { working.translate_family_fixed(b.slot as usize,b.uid,*delta)?; }
            scenario.data_slots.push(b.clone());
        } else if let Some(t) = owned.get(&s.id).cloned().or_else(|| template_for(s)) {
            let radians = (s.angle.rem_euclid(360) as f64 * std::f64::consts::PI / 180.) as f32;
            let (candidate,slot,uid) = working.add_singleton_candidate(&t.store,t.slot,t.uid,[s.x,s.y,s.z],radians,s.team as u16)?;
            working = candidate;
            scenario.data_slots.push(DataSlot { slot: slot as i32, uid, spawn_id:s.id });
        } else { skipped.push(s.clone()); }
    }
    Ok(ScenarioCandidateResult { store: working, scenario, skipped })
}
// Public for source-selection parity tests and callers displaying exact native angles.
pub fn display_angle(radians: f32) -> i32 { angle(radians) }
