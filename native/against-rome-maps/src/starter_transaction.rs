//! Detached Spawn -> DATA transaction. No filesystem writes.
use super::native::{fixed_displacement, ObjectStore};
use against_rome_scenario::model::{DataSlot, ScenarioDocument, Spawn};
use std::collections::{BTreeMap, BTreeSet};
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
        if old.raw_rotation.is_none() && current.raw_rotation.is_none()
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
