//! Placement edit rules and family-copy request mapping, port of the pure logic of C#
//! `PlacementEditSession` (`Edit`, `ValidateBounds`, `IsValidUnitTeam/Count`,
//! `BuildFamilyCopyRequests`). Undo/redo history is intentionally not modelled here.

use crate::sdl_objects::{NativeObjectIdentity, SdlObjectCategory, SdlPlacedObject};
use crate::starter_kit::native::ObjectStore;
use against_rome_scenario::model::ScenarioDocument;
use std::collections::HashMap;
use uuid::Uuid;

// Spawn -> DATA save planning lives beside the edit rules it shares (registered here so the crate
// root module list stays untouched); public path: `against_rome_maps::spawn_data`.
#[path = "spawn_data.rs"]
pub mod spawn_data;

/// Squad (figure) team range, 0..=7 (C# `IsValidUnitTeam`).
pub fn is_valid_unit_team(team: i32) -> bool {
    (0..=7).contains(&team)
}

/// Squad unit count range, 1..=20 (C# `IsValidUnitCount`).
pub fn is_valid_unit_count(count: i32) -> bool {
    (1..=20).contains(&count)
}

fn is_animal(item: &SdlPlacedObject) -> bool {
    item.obj_type.category == SdlObjectCategory::Figure
        && item.obj_type.name_def.len() >= 6
        && item.obj_type.name_def.as_bytes()[..6].eq_ignore_ascii_case(b"FigTie")
}

/// C# `ValidateBounds`: figures use team 0..7 and unit count (1..20 strict, 0..20 otherwise);
/// animals must have count 0; everything else uses team -1..15.
pub fn validate_bounds(item: &SdlPlacedObject, strict_unit_count: bool) -> Result<(), String> {
    if !item.world_x.is_finite()
        || !item.world_y.is_finite()
        || !item.world_z.is_finite()
        || !(0.0..=16383.0).contains(&item.world_x)
        || !(0.0..=16383.0).contains(&item.world_z)
    {
        return Err("Position must be finite with X/Z between 0 and 16383.".into());
    }
    if !item.angle.is_finite() {
        return Err("Angle must be finite.".into());
    }
    let animal = is_animal(item);
    if item.obj_type.category == SdlObjectCategory::Figure && !animal && item.raw_rotation.is_none() {
        if !is_valid_unit_team(item.team) {
            return Err(format!("Figure team must be between 0 and 7, got {}.", item.team));
        }
        if strict_unit_count && !is_valid_unit_count(item.unit_count) {
            return Err(format!("Figure unit count must be between 1 and 20, got {}.", item.unit_count));
        }
        if !strict_unit_count && !(0..=20).contains(&item.unit_count) {
            return Err(format!("Figure unit count must be between 0 and 20, got {}.", item.unit_count));
        }
    } else {
        if animal && item.unit_count != 0 {
            return Err("Animals are individual objects and must have unit count 0.".into());
        }
        if !(-1..=15).contains(&item.team) {
            return Err(format!("Object team must be between -1 and 15, got {}.", item.team));
        }
    }
    Ok(())
}

fn normalize_angle(angle: i32) -> i32 {
    angle.rem_euclid(360)
}

fn rounded_angle(angle: f32) -> i32 {
    normalize_angle(angle.round_ties_even() as i32)
}

fn is_family_copy(item: &SdlPlacedObject) -> bool {
    !item.copy_source_scenario_id.is_nil() || item.native_source_identity.is_some()
}

fn raw_rotation_from_degrees(angle: f32) -> f32 {
    let normalized = angle.rem_euclid(360.0);
    (normalized as f64 * (std::f64::consts::PI / 180.0)) as f32
}

fn check_native_translation_only(before: &SdlPlacedObject, team: i32, angle: f32) -> Result<(), String> {
    if before.raw_rotation.is_some() && !is_family_copy(before) {
        if team != before.team {
            return Err("原生群組物件目前僅支援平移；變更隊伍尚未支援。".into());
        }
        if rounded_angle(angle) != rounded_angle(before.angle) {
            return Err("原生群組物件目前僅支援平移；變更方向尚未支援。".into());
        }
    }
    Ok(())
}

/// C# `Edit(index, updated)`: returns the validated replacement for `before`, preserving identity
/// and provenance. A copied native family may change team and orientation; its raw rotation is
/// regenerated from the edited angle while the original DATA source remains immutable.
pub fn edit_placement(before: &SdlPlacedObject, updated: &SdlPlacedObject) -> Result<SdlPlacedObject, String> {
    check_native_translation_only(before, updated.team, updated.angle)?;
    let mut fixed = updated.clone();
    fixed.scenario_id = before.scenario_id;
    fixed.copy_source_scenario_id = before.copy_source_scenario_id;
    fixed.native_source_identity = before.native_source_identity;
    fixed.raw_rotation = if is_family_copy(before)
        && before.raw_rotation.is_some()
        && rounded_angle(updated.angle) != rounded_angle(before.angle)
    {
        Some(raw_rotation_from_degrees(updated.angle))
    } else {
        before.raw_rotation
    };
    validate_bounds(&fixed, true)?;
    Ok(fixed)
}

/// C# `Edit(index, team, x, y, z, angle, unitCount)`: the unit count only applies to figures
/// without a native raw rotation; every other object is forced to 0.
pub fn edit_placement_values(
    before: &SdlPlacedObject,
    team: i32,
    world_x: f32,
    world_y: f32,
    world_z: f32,
    angle: f32,
    unit_count: i32,
) -> Result<SdlPlacedObject, String> {
    check_native_translation_only(before, team, angle)?;
    let is_figure = before.obj_type.category == SdlObjectCategory::Figure;
    let mut updated = before.clone();
    updated.team = team;
    updated.world_x = world_x;
    updated.world_y = world_y;
    updated.world_z = world_z;
    updated.angle = angle;
    updated.unit_count = if is_figure && before.raw_rotation.is_none() { unit_count } else { 0 };
    edit_placement(before, &updated)
}

/// C# `ScenarioFamilyCopyRequest`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScenarioFamilyCopyRequest {
    pub source_slot: i32,
    pub source_uid: u32,
    pub is_native_source: bool,
    pub target_team: u16,
    pub target_rotation_degrees: f32,
    pub target_raw_rotation: f32,
}

fn item_alias(item: &SdlPlacedObject) -> &str {
    item.obj_type
        .template_fields
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("alias"))
        .map(|(_, v)| v.as_str())
        .unwrap_or(&item.obj_type.name_def)
}

fn radians_to_display_angle(radians: f32) -> i32 {
    let deg = radians as f64 * (180.0 / std::f64::consts::PI);
    let rounded = (((deg % 360.0) + 360.0) % 360.0).round_ties_even() as i32;
    rounded % 360
}

/// C# `BuildFamilyCopyRequests`: maps copied placements with a valid previous prebuilt binding or
/// native source to family-copy requests. Unbound/script copies yield no request; bound copies whose
/// source state no longer matches (team or direction) are rejected rather than silently downgraded.
pub fn build_family_copy_requests(
    placed: &[SdlPlacedObject],
    previous: &ScenarioDocument,
    store: &ObjectStore,
    native_alias_resolver: Option<&dyn Fn(i32) -> Option<String>>,
) -> Result<HashMap<Uuid, ScenarioFamilyCopyRequest>, String> {
    let mut bindings: HashMap<Uuid, Vec<&against_rome_scenario::model::DataSlot>> = HashMap::new();
    for b in previous.data_slots.iter().filter(|b| !b.spawn_id.is_nil()) {
        bindings.entry(b.spawn_id).or_default().push(b);
    }
    let spawns: HashMap<Uuid, &against_rome_scenario::model::Spawn> =
        previous.spawns.iter().map(|s| (s.id, s)).collect();
    let world: HashMap<usize, _> = store.objects().into_iter().map(|o| (o.slot, o)).collect();
    let root_rotation = |deg: f32| (deg as f64 * (std::f64::consts::PI / 180.0)) as f32;
    let mut requests = HashMap::new();

    for item in placed {
        if item.copy_source_scenario_id.is_nil() && item.native_source_identity.is_none() {
            continue;
        }
        if let Some(NativeObjectIdentity { slot, uid }) = item.native_source_identity {
            let resolver = native_alias_resolver.ok_or("複製原生群組時必須提供可信原生別名解析器。")?;
            if previous
                .data_slots
                .iter()
                .any(|b| b.slot == slot && b.uid == uid && !b.spawn_id.is_nil())
            {
                return Err(format!("原生副本來源槽位 {slot} 已存在場景綁定，應使用一般副本請求。"));
            }
            let root = usize::try_from(slot)
                .ok()
                .and_then(|s| world.get(&s))
                .filter(|r| r.uid == uid)
                .ok_or_else(|| format!("原生副本來源槽位 {slot} 的 root UID 無效或物件已不存在。"))?;
            if store.family(root.slot, uid).is_err() {
                return Err(format!("副本來源槽位 {slot} 的群組無效或非獨立支援群組。"));
            }
            let resolved = resolver(root.type_id as i32);
            let alias = item_alias(item);
            match resolved {
                Some(r) if !r.trim().is_empty() && r.eq_ignore_ascii_case(alias) => {}
                _ => {
                    return Err(format!(
                        "原生副本來源 TypeId {} 解析的別名與目標別名不一致。",
                        root.type_id
                    ))
                }
            }
            if !(0..=8).contains(&item.team) { return Err("預建物件的隊伍必須介於 0 與 8。".into()); }
            let target_raw = match item.raw_rotation {
                Some(r) if r.is_finite() => r,
                _ => return Err("原生群組副本目標必須明確提供有限的原生旋轉弧度。".into()),
            };
            let source_rotation = root_rotation(root.rotation_degrees);
            if !source_rotation.is_finite() { return Err("原生來源旋轉無效。".into()); }
            let target_degrees = rounded_angle(item.angle);
            let expected_raw = raw_rotation_from_degrees(target_degrees as f32);
            if target_raw.to_bits() != expected_raw.to_bits() {
                return Err("原生群組副本的目標旋轉弧度與顯示方向不一致。".into());
            }
            requests.insert(
                item.scenario_id,
                ScenarioFamilyCopyRequest { source_slot: slot, source_uid: uid, is_native_source: true,
                    target_team: item.team as u16, target_rotation_degrees: target_degrees as f32,
                    target_raw_rotation: target_raw },
            );
        } else {
            let is_prebuilt = (item.obj_type.category == SdlObjectCategory::Building
                && (0..=8).contains(&item.team))
                || item.raw_rotation.is_some();
            let Some(matching) = bindings.get(&item.copy_source_scenario_id) else {
                continue;
            };
            if matching.len() != 1 {
                return Err("群組副本來源具有重複的 DATA 綁定。".into());
            }
            if !is_prebuilt {
                return Err("已綁定的預建群組副本不能降級為腳本物件或使用不支援的隊伍。".into());
            }
            let binding = matching[0];
            let spawn = match spawns.get(&item.copy_source_scenario_id) {
                Some(s) if s.prebuilt => *s,
                _ => return Err("群組副本的 DATA 綁定缺少有效預建來源物件。".into()),
            };
            let root = usize::try_from(binding.slot)
                .ok()
                .and_then(|s| world.get(&s))
                .filter(|r| r.uid == binding.uid)
                .ok_or_else(|| format!("副本來源槽位 {} 的 root UID 無效或物件已不存在。", binding.slot))?;
            if store.family(root.slot, binding.uid).is_err() {
                return Err(format!("副本來源槽位 {} 的群組無效或非獨立支援群組。", binding.slot));
            }
            if !spawn.alias.eq_ignore_ascii_case(item_alias(item)) {
                return Err("副本物件的別名必須與來源一致。".into());
            }
            if !(0..=8).contains(&item.team) { return Err("預建物件的隊伍必須介於 0 與 8。".into()); }
            let root_rot = root_rotation(root.rotation_degrees);
            if spawn.raw_rotation.is_some() || item.raw_rotation.is_some() {
                let source_ok = match spawn.raw_rotation {
                    Some(r) => r.is_finite() && root_rot.to_bits() == r.to_bits()
                        && normalize_angle(spawn.angle) == radians_to_display_angle(r),
                    None => false,
                };
                if !source_ok || !item.raw_rotation.is_some_and(f32::is_finite) {
                    return Err("已綁定群組的原生旋轉資料無效。".into());
                }
            } else {
                let actual = root_rot as f64 * (180.0 / std::f64::consts::PI);
                let source_angle = ((spawn.angle as f64 % 360.0) + 360.0) % 360.0;
                let diff = ((((actual - source_angle + 540.0) % 360.0) + 360.0) % 360.0 - 180.0).abs();
                if !actual.is_finite() || diff > 0.001 { return Err("來源群組旋轉資料無效。".into()); }
            }
            let target_degrees = rounded_angle(item.angle);
            let target_raw = item.raw_rotation.unwrap_or_else(|| raw_rotation_from_degrees(target_degrees as f32));
            requests.insert(
                item.scenario_id,
                ScenarioFamilyCopyRequest {
                    source_slot: binding.slot,
                    source_uid: binding.uid,
                    is_native_source: false,
                    target_team: item.team as u16,
                    target_rotation_degrees: target_degrees as f32,
                    target_raw_rotation: target_raw,
                },
            );
        }
    }
    Ok(requests)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdl_objects::SdlObjectType;
    use std::collections::HashMap;

    fn obj(name: &str, cat: SdlObjectCategory, team: i32, count: i32) -> SdlPlacedObject {
        SdlPlacedObject::new(
            SdlObjectType::new(name, 1, cat, "", 1, HashMap::new()),
            100.0,
            0.0,
            100.0,
            team,
            0.0,
            count,
        )
    }

    #[test]
    fn unit_rules() {
        assert!(is_valid_unit_team(0) && is_valid_unit_team(7));
        assert!(!is_valid_unit_team(-1) && !is_valid_unit_team(8));
        assert!(is_valid_unit_count(1) && is_valid_unit_count(20));
        assert!(!is_valid_unit_count(0) && !is_valid_unit_count(21));
    }

    #[test]
    fn bounds_by_category() {
        let fig = obj("FigRoemLegionaer", SdlObjectCategory::Figure, 3, 5);
        assert!(validate_bounds(&fig, true).is_ok());
        assert!(validate_bounds(&obj("FigRoemLegionaer", SdlObjectCategory::Figure, 8, 5), false).is_err());
        let zero = obj("FigRoemLegionaer", SdlObjectCategory::Figure, 3, 0);
        assert!(validate_bounds(&zero, false).is_ok());
        assert!(validate_bounds(&zero, true).is_err());
        let wolf = obj("FigTieWolf", SdlObjectCategory::Figure, 8, 0);
        assert!(validate_bounds(&wolf, true).is_ok());
        assert!(validate_bounds(&obj("FigTieWolf", SdlObjectCategory::Figure, 8, 2), false).is_err());
        assert!(validate_bounds(&obj("BauX", SdlObjectCategory::Building, -1, 0), true).is_ok());
        assert!(validate_bounds(&obj("BauX", SdlObjectCategory::Building, 16, 0), true).is_err());
        let mut far = obj("BauX", SdlObjectCategory::Building, 1, 0);
        far.world_x = 16384.0;
        assert!(validate_bounds(&far, true).is_err());
        far.world_x = f32::NAN;
        assert!(validate_bounds(&far, true).is_err());
    }

    #[test]
    fn edit_preserves_identity_and_forces_count() {
        let mut before = obj("BauX", SdlObjectCategory::Building, 1, 0);
        before.scenario_id = Uuid::from_u128(7);
        let edited = edit_placement_values(&before, 2, 5.0, 1.0, 6.0, 90.0, 9).unwrap();
        assert_eq!((edited.team, edited.unit_count, edited.scenario_id), (2, 0, before.scenario_id));
        let fig = obj("FigRoemLegionaer", SdlObjectCategory::Figure, 1, 3);
        assert_eq!(edit_placement_values(&fig, 1, 5.0, 0.0, 5.0, 0.0, 12).unwrap().unit_count, 12);
        assert!(edit_placement_values(&fig, 1, 5.0, 0.0, 5.0, 0.0, 0).is_err());
    }

    #[test]
    fn native_group_translate_only() {
        let mut before = obj("BauX", SdlObjectCategory::Building, 1, 0);
        before.raw_rotation = Some(1.0);
        assert!(edit_placement_values(&before, 2, 1.0, 0.0, 1.0, 0.0, 0).is_err());
        assert!(edit_placement_values(&before, 1, 1.0, 0.0, 1.0, 45.0, 0).is_err());
        let ok = edit_placement_values(&before, 1, 9.0, 0.0, 9.0, 0.0, 4).unwrap();
        assert_eq!((ok.world_x, ok.unit_count, ok.raw_rotation), (9.0, 0, Some(1.0)));
    }

    fn synthetic_store(team: u16) -> (ObjectStore, std::path::PathBuf) {
        use crate::starter_kit::native::{COLUMNS, SEGMENTS};
        let n = 4usize;
        let dir = std::env::temp_dir().join(format!("arm-placement-edit-{}", Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("DATA")).unwrap();
        let mut o = vec![0u8; 16 + n * (79 + COLUMNS.iter().sum::<usize>())];
        for (i, v) in [1u32, n as u32, 30, 30].iter().enumerate() {
            o[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        let mut p = vec![0u8; 8 + n * 3 * 17];
        p[..4].copy_from_slice(&1u32.to_le_bytes());
        p[4..8].copy_from_slice(&((n * 3) as u32).to_le_bytes());
        let mut d = vec![0u8; 8 + n * SEGMENTS.iter().sum::<usize>()];
        d[..4].copy_from_slice(&1u32.to_le_bytes());
        d[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        for sl in 0..n {
            let r = 16 + sl * 79;
            for off in [67, 69, 71, 73, 77] {
                o[r + off..r + off + 2].fill(255);
            }
            for c in 0..18 {
                if COLUMNS[c] == 2 {
                    let off = 16 + n * 79 + n * COLUMNS[..c].iter().sum::<usize>() + sl * COLUMNS[c];
                    o[off..off + 2].fill(255);
                }
            }
        }
        // slot 0: live root, type 232, uid 1
        o[16] = 1;
        o[17..19].copy_from_slice(&team.to_le_bytes());
        o[19..23].copy_from_slice(&1u32.to_le_bytes());
        o[16 + 75..16 + 77].copy_from_slice(&232u16.to_le_bytes());
        for off in [67, 69] {
            o[16 + off..16 + off + 2].copy_from_slice(&0u16.to_le_bytes());
        }
        let c1 = 16 + n * 79 + n * COLUMNS[..1].iter().sum::<usize>();
        o[c1..c1 + 2].copy_from_slice(&0u16.to_le_bytes());
        p[8] = 1;
        for (a, v) in [4000f32, 20.0, 4000.0].iter().enumerate() {
            p[9 + a * 4..13 + a * 4].copy_from_slice(&v.to_le_bytes());
        }
        p[21..25].copy_from_slice(&90f32.to_le_bytes());
        d[8] = 1;
        let mut l = vec![0u8; against_rome_data::pool::PAYLOAD_LEN];
        l[..4].copy_from_slice(&1u32.to_le_bytes());
        l[4..8].copy_from_slice(&1024u32.to_le_bytes());
        let mut e = vec![0u8; 94];
        e[..4].copy_from_slice(&1u32.to_le_bytes());
        for (name, b) in [("objects", o), ("position", p), ("objdata", d), ("light", l), ("engine", e)] {
            std::fs::write(dir.join(format!("DATA/{name}.dat")), b).unwrap();
        }
        (ObjectStore::load(&dir).unwrap(), dir)
    }

    #[test]
    fn family_copy_requests() {
        let (store, dir) = synthetic_store(0);
        let prev = ScenarioDocument::default();
        let plain = obj("BauX", SdlObjectCategory::Building, 0, 0);
        assert!(build_family_copy_requests(&[plain.clone()], &prev, &store, None).unwrap().is_empty());
        let mut unbound = plain.clone();
        unbound.copy_source_scenario_id = Uuid::from_u128(1);
        assert!(build_family_copy_requests(&[unbound], &prev, &store, None).unwrap().is_empty());

        let radians = (90f32 as f64 * (std::f64::consts::PI / 180.0)) as f32;
        let mut native = plain;
        native.scenario_id = Uuid::from_u128(5);
        native.native_source_identity = Some(NativeObjectIdentity { slot: 0, uid: 1 });
        native.raw_rotation = Some(radians);
        native.angle = 90.0;
        assert!(build_family_copy_requests(&[native.clone()], &prev, &store, None).is_err());
        let resolver = |t: i32| (t == 232).then(|| "BauX".to_string());
        let req = build_family_copy_requests(&[native.clone()], &prev, &store, Some(&resolver)).unwrap();
        assert_eq!(req[&Uuid::from_u128(5)].source_slot, 0);
        assert_eq!(req[&Uuid::from_u128(5)].source_uid, 1);
        assert_eq!(req[&Uuid::from_u128(5)].target_team, 0);
        assert_eq!(req[&Uuid::from_u128(5)].target_rotation_degrees, 90.0);
        assert_eq!(req[&Uuid::from_u128(5)].target_raw_rotation.to_bits(), radians.to_bits());
        let mut turned = native.clone();
        turned.angle = 180.0;
        turned.raw_rotation = Some(raw_rotation_from_degrees(180.0));
        assert_eq!(build_family_copy_requests(&[turned], &prev, &store, Some(&resolver)).unwrap()[&Uuid::from_u128(5)].target_rotation_degrees, 180.0);
        let mut other_team = native.clone();
        other_team.team = 3;
        assert_eq!(build_family_copy_requests(&[other_team], &prev, &store, Some(&resolver)).unwrap()[&Uuid::from_u128(5)].target_team, 3);
        let mut stale = native;
        stale.native_source_identity = Some(NativeObjectIdentity { slot: 0, uid: 2 });
        assert!(build_family_copy_requests(&[stale], &prev, &store, Some(&resolver)).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn copied_native_family_can_change_team_and_direction_but_source_cannot() {
        let mut source = obj("BauX", SdlObjectCategory::Building, 0, 0);
        source.raw_rotation = Some(raw_rotation_from_degrees(90.0));
        source.angle = 90.0;
        assert!(edit_placement_values(&source, 3, source.world_x, source.world_y, source.world_z, 180.0, 0).is_err());

        let mut copy = source.clone();
        copy.native_source_identity = Some(NativeObjectIdentity { slot: 0, uid: 1 });
        let updated = edit_placement_values(&copy, 3, copy.world_x, copy.world_y, copy.world_z, 180.0, 0).unwrap();
        assert_eq!(updated.team, 3);
        assert_eq!(updated.angle, 180.0);
        assert_eq!(updated.raw_rotation.unwrap().to_bits(), raw_rotation_from_degrees(180.0).to_bits());
    }
}
