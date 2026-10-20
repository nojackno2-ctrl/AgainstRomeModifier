//! Safe blank map starter kit placement and template bootstrapping (`BlankMapStarterKit`).
//! Replicates C# `BlankMapStarterKit.cs`.

use against_rome_scenario::model::{ScenarioDocument, Spawn};
use uuid::Uuid;
use std::{collections::BTreeMap, path::{Path, PathBuf}};
// Manifest edits are outside this slice. Share the existing DATA source rather than fork it.
// Replace this source link with a crate dependency at the later integration boundary.
#[path = "../../against-rome-placement/src/lib.rs"]
pub mod native;
#[path = "starter_transaction.rs"]
pub mod transaction;
use native::{ObjectStore, WorldObject};
use transaction::{ScenarioCandidateResult, Template};

pub struct BlankMapStarterKit {
    pub spawns: Vec<Spawn>,
    pub house_family_object_count: usize,
    pub native_object_count: usize,
    pub house_source_map_path: Option<PathBuf>,
    pub house_family_placements: Vec<WorldObject>,
    templates: BTreeMap<String, Template>,
    house: Option<Template>,
}

impl BlankMapStarterKit {
    pub const MAXIMUM_OBJECTS: usize = 14000;
    pub const MAIN_HOUSE_ALIAS: &'static str = "GER_HAU00";
    pub const MAIN_HOUSE_NAME: &'static str = "BauGerHau00_Haupthaus";
    pub const MAIN_HOUSE_TYPE_ID: i32 = 232;
    pub const UNIT_ALIAS: &'static str = "GER_INF02";
    pub const UNIT_NAME: &'static str = "FigGerInf02_Zweihandaxt";
    pub const UNIT_TYPE_ID: i32 = 218;

    /// Creates a detached main-house family and four safe individual unit placements.
    /// Validates team (0..=7) and coordinates (0..=16383).
    pub fn create(
        team: i32,
        x: f32,
        z: f32,
        height: Option<&dyn Fn(f32, f32) -> f32>,
        existing_spawns: Option<&[Spawn]>,
    ) -> Result<Self, String> {
        if !(0..=7).contains(&team) {
            return Err("Starter kit team must be 0–7.".into());
        }
        check_position(x, 0.0, z)?;

        if let Some(existing) = existing_spawns {
            check_budget(existing, 5)?;
        }

        let layout: [(&str, f32, f32); 5] = [
            (Self::MAIN_HOUSE_ALIAS, 0.0, 0.0),
            (Self::UNIT_ALIAS, -480.0, 800.0),
            (Self::UNIT_ALIAS, -160.0, 800.0),
            (Self::UNIT_ALIAS, 160.0, 800.0),
            (Self::UNIT_ALIAS, 480.0, 800.0),
        ];

        let mut spawns = Vec::with_capacity(5);
        for (alias, dx, dz) in layout {
            let px = x + dx;
            let pz = z + dz;
            check_position(px, 0.0, pz)?;
            let py = if let Some(h) = height {
                let y_val = h(px, pz);
                check_position(px, y_val, pz)?;
                y_val
            } else {
                0.0
            };

            spawns.push(Spawn {
                id: Uuid::new_v4(),
                alias: alias.to_string(),
                x: px,
                z: pz,
                y: py,
                team,
                count: 0,
                prebuilt: true,
                ..Default::default()
            });
        }

        Ok(Self {
            spawns,
            house_family_object_count: 1,
            native_object_count: 5,
            house_source_map_path: None,
            house_family_placements: vec![],
            templates: BTreeMap::new(),
            house: None,
        })
    }

    /// Safely applies starter kit placements to a scenario document.
    /// Verifies that none of the kit spawns have already been applied.
    pub fn apply_to_scenario(&self, scenario: &mut ScenarioDocument) -> Result<(), String> {
        if scenario.spawns.iter().any(|s| self.spawns.iter().any(|k| k.id == s.id)) {
            return Err("Starter kit has already been applied to this scenario.".into());
        }

        check_budget(&scenario.spawns, self.native_object_count)?;

        scenario.spawns.extend(self.spawns.clone());
        Ok(())
    }
}

fn check_position(x: f32, y: f32, z: f32) -> Result<(), String> {
    if !x.is_finite() || !y.is_finite() || !z.is_finite() || !(0.0..=16383.0).contains(&x) || !(0.0..=16383.0).contains(&z) {
        return Err("Starter kit positions must be finite with X/Z between 0 and 16383.".into());
    }
    Ok(())
}

fn check_budget(existing: &[Spawn], added: usize) -> Result<(), String> {
    let mut count = added;
    for spawn in existing {
        if spawn.count < 0 || spawn.count > 20 {
            return Err("Existing placements contain an invalid troop count.".into());
        }
        count += if spawn.count > 0 { spawn.count as usize + 1 } else { 1 };
        if count > BlankMapStarterKit::MAXIMUM_OBJECTS {
            return Err("Starter kit exceeds 14000 objects.".into());
        }
    }
    Ok(())
}

impl BlankMapStarterKit {
    /// Read-only verified catalogs and official DATA discovery. TUTOR_01 wins over path order;
    /// custom maps are excluded even when they carry a complete linked house.
    pub fn create_from_game(game: &Path, template_map: &Path, team: i32, x: f32, z: f32,
        height: Option<&dyn Fn(f32,f32)->f32>, existing: Option<&[Spawn]>) -> Result<Self,String> {
        let read_text = |p: PathBuf| -> Result<String,String> {
            let raw = std::fs::read(p).map_err(|e| e.to_string())?;
            crate::text_encoding::decode_cp1251(&crate::text_encoding::decompress_pfil_if_needed(&raw)?)
        };
        let aliases = read_text(game.join("SYSTEM/CLAK/cl_scint.ini"))?;
        let mut section = false;
        let mut pairs = vec![];
        for raw in aliases.lines() {
            let line = raw.trim();
            if line.starts_with('[') { section = line.eq_ignore_ascii_case("[ObjDefName]"); continue; }
            if section && !line.starts_with(';') {
                if let Some((a,n)) = line.split_once('=') { pairs.push((a.trim(),n.trim())); }
            }
        }
        let definitions = read_text(game.join("SYSTEM/DATA_MP/DEFAULTS/objdef.dau"))?;
        let mut names = BTreeMap::new();
        for line in definitions.lines() {
            if line.len() < 100 || line.trim_start().starts_with(';') || line.starts_with('[') { continue; }
            let fields: Vec<_> = line.split(',').collect();
            if fields.len()>52 { if let Ok(id) = fields[0].trim().parse::<u16>() { names.insert(id,fields[52].trim()); } }
        }
        let resolve = |alias: &str,name: &str| -> Result<u16,String> {
            let a: Vec<_> = pairs.iter().filter(|(a,_)| a.eq_ignore_ascii_case(alias)).collect();
            let ids: Vec<_> = names.iter().filter(|(_,n)| n.eq_ignore_ascii_case(name)).collect();
            if a.len()!=1 || !a[0].1.eq_ignore_ascii_case(name) || ids.len()!=1 { return Err(format!("unverified or nonunique starter alias {alias}")); }
            Ok(*ids[0].0)
        };
        let house_id = resolve(Self::MAIN_HOUSE_ALIAS,Self::MAIN_HOUSE_NAME)?;
        let unit_id = resolve(Self::UNIT_ALIAS,Self::UNIT_NAME)?;
        let source = ObjectStore::load(template_map)?;
        if source.capacity()<5 { return Err("template capacity below starter kit size".into()); }
        let mut maps: Vec<_> = std::fs::read_dir(game.join("MAPS")).map_err(|e|e.to_string())?
            .filter_map(|e|e.ok()).map(|e|e.path()).filter(|p|p.is_dir()).collect();
        maps.sort_by_key(|p| (if p.file_name().unwrap_or_default().to_string_lossy().eq_ignore_ascii_case("TUTOR_01") {0} else {1},p.to_string_lossy().to_lowercase()));
        let mut kit = Self::create(team,x,z,height,None)?;
        let collect = |store: &ObjectStore, templates: &mut BTreeMap<String,Template>| {
            for o in store.objects() {
                let alias = if o.type_id==house_id {Self::MAIN_HOUSE_ALIAS} else if o.type_id==unit_id {Self::UNIT_ALIAS} else {continue;};
                if store.singleton_template(o.slot,o.uid).is_ok() {
                    templates.entry(alias.into()).or_insert_with(|| Template {store:store.clone(),slot:o.slot,uid:o.uid});
                }
            }
        };
        collect(&source,&mut kit.templates);
        // Safe templates use ordinary path order; linked-house choice separately prefers tutorial.
        let mut ordinary = maps.clone(); ordinary.sort_by_key(|p|p.to_string_lossy().to_lowercase());
        for path in ordinary {
            if crate::CustomMapManifest::is_custom_map_directory(&path) {continue;}
            if let Ok(store) = ObjectStore::load(&path) { collect(&store,&mut kit.templates); }
        }
        for path in maps {
            if crate::CustomMapManifest::is_custom_map_directory(&path) {continue;}
            let Ok(store) = ObjectStore::load(&path) else {continue;};
            let root = store.objects().into_iter().find(|o|o.type_id==house_id
                && store.family(o.slot,o.uid).is_ok_and(|f|f.len()>1));
            let Some(root) = root else {continue;};
            let r = store.native_rotation(root.slot,root.uid)?;
            if !r.is_finite() {continue;}
            kit.spawns[0].raw_rotation=Some(r); kit.spawns[0].angle=transaction::display_angle(r);
            let family=store.family(root.slot,root.uid)?;
            let h=&kit.spawns[0];
            let delta=native::fixed_displacement(root.position,[h.x,h.y,h.z])?;
            kit.house_family_placements=store.objects().into_iter().filter(|o|family.contains(&o.slot)).map(|mut o| {
                for a in 0..3 {o.position[a]=(o.position[a] as f64+delta[a] as f64/65536.) as f32;}
                o.team=team as u16; o
            }).collect();
            for o in &kit.house_family_placements { check_position(o.position[0],o.position[1],o.position[2])?; }
            kit.house_family_object_count=family.len(); kit.native_object_count=family.len()+4;
            kit.house_source_map_path=Some(path.canonicalize().map_err(|e|e.to_string())?);
            kit.house=Some(Template {store,slot:root.slot,uid:root.uid});
            kit.templates.remove(Self::MAIN_HOUSE_ALIAS);
            break;
        }
        if !kit.templates.contains_key(Self::UNIT_ALIAS) || (kit.house.is_none() && !kit.templates.contains_key(Self::MAIN_HOUSE_ALIAS)) {
            return Err("missing safe starter DATA templates or official house family".into());
        }
        check_budget(existing.unwrap_or_default(),kit.native_object_count)?;
        Ok(kit)
    }
    pub fn template_for(&self, spawn: &Spawn) -> Option<Template> {
        self.templates.iter().find(|(a,_)|a.eq_ignore_ascii_case(&spawn.alias)).map(|(_,t)|t.clone())
    }
    pub fn apply_to_candidate(&self, store: &ObjectStore, previous: &ScenarioDocument,
        existing_template_for: Option<&dyn Fn(&Spawn)->Option<Template>>) -> Result<ScenarioCandidateResult,String> {
        check_budget(&previous.spawns,self.native_object_count)?;
        let objects=store.objects();
        let mut budget=objects.len()+self.native_object_count;
        for s in &previous.spawns {
            if !s.prebuilt || !previous.data_slots.iter().any(|b|b.spawn_id==s.id
                && objects.iter().any(|o|o.slot as i32==b.slot && o.uid==b.uid)) {
                budget+=if s.count>0 {s.count as usize+1} else {1};
            }
        }
        if budget>store.capacity().min(Self::MAXIMUM_OBJECTS) {return Err("starter kit exceeds native DATA capacity".into());}
        if previous.spawns.iter().any(|s|self.spawns.iter().any(|k|k.id==s.id)) {return Err("starter kit already applied".into());}
        let mut seeded=previous.clone();
        let mut working=store.clone();
        if let Some(t)=&self.house {
            let root=t.store.objects().into_iter().find(|o|o.slot==t.slot && o.uid==t.uid).ok_or("missing house root")?;
            let h=&self.spawns[0];
            let (candidate,slot,uid)=working.import_family_candidate(&t.store,t.slot,t.uid,
                native::fixed_displacement(root.position,[h.x,h.y,h.z])?,h.team as u16)?;
            working=candidate;
            seeded.spawns.push(h.clone());
            seeded.data_slots.push(against_rome_scenario::model::DataSlot {slot:slot as i32,uid,spawn_id:h.id});
        }
        let mut next=previous.clone(); next.spawns.extend(self.spawns.clone()); next.data_slots=seeded.data_slots.clone();
        let result=transaction::apply_to_candidate(&working,&seeded,&next,&|s|self.template_for(s)
            .or_else(||existing_template_for.and_then(|f|f(s))))?;
        if !result.skipped.is_empty() || self.spawns.iter().any(|s|result.scenario.data_slots.iter().filter(|b|b.spawn_id==s.id).count()!=1) {
            return Err("starter kit transaction did not bind all required DATA objects".into());
        }
        Ok(result)
    }
}

#[cfg(test)]
#[path = "starter_kit_tests.rs"]
mod tests;
