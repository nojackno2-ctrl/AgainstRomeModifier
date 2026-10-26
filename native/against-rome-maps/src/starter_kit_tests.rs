use super::*;
use std::fs;
use native::{COLUMNS,SEGMENTS};
struct Fixture { root: PathBuf, map: PathBuf }
impl Drop for Fixture { fn drop(&mut self) { let _=fs::remove_dir_all(&self.root); } }
fn map(path: &Path,n:usize,live:&[(u16,Option<u16>)],cursor:u32) {
    fs::create_dir_all(path.join("DATA")).unwrap();
    let mut o=vec![0;16+n*(79+COLUMNS.iter().sum::<usize>())];
    for (i,v) in [1,n as u32,30,30].iter().enumerate() {o[i*4..i*4+4].copy_from_slice(&v.to_le_bytes());}
    let mut p=vec![0;8+n*3*17]; p[..4].copy_from_slice(&1u32.to_le_bytes());p[4..8].copy_from_slice(&((n*3) as u32).to_le_bytes());
    let mut d=vec![0;8+n*SEGMENTS.iter().sum::<usize>()];d[..4].copy_from_slice(&1u32.to_le_bytes());d[4..8].copy_from_slice(&(n as u32).to_le_bytes());
    for s in 0..n {
        let r=16+s*79;
        for off in [67,69,71,73,77] {o[r+off..r+off+2].fill(255);}
        for c in 0..18 {if COLUMNS[c]==2 {let off=16+n*79+n*COLUMNS[..c].iter().sum::<usize>()+s*COLUMNS[c];o[off..off+2].fill(255);}}
        if let Some(&(typ,parent))=live.get(s) {
            o[r]=1;o[r+3..r+7].copy_from_slice(&(s as u32+1).to_le_bytes());o[r+75..r+77].copy_from_slice(&typ.to_le_bytes());
            for off in [67,69] {o[r+off..r+off+2].copy_from_slice(&(s as u16).to_le_bytes());}
            for (c,v) in [(1,s as u16),(13,parent.unwrap_or(65535))] {let off=16+n*79+n*COLUMNS[..c].iter().sum::<usize>()+s*COLUMNS[c];o[off..off+2].copy_from_slice(&v.to_le_bytes());}
            let off=8+s*17;p[off]=1;
            for (a,v) in [4000.+s as f32*20.,20.,4000.].iter().enumerate() {p[off+1+a*4..off+5+a*4].copy_from_slice(&v.to_le_bytes());}
            p[off+13..off+17].copy_from_slice(&90f32.to_le_bytes());d[8+s*9]=1;
        }
    }
    let mut l=vec![0;against_rome_data::pool::PAYLOAD_LEN];l[..4].copy_from_slice(&1u32.to_le_bytes());l[4..8].copy_from_slice(&1024u32.to_le_bytes());
    let mut e=vec![0;94];e[..4].copy_from_slice(&1u32.to_le_bytes());e[34..38].copy_from_slice(&cursor.to_le_bytes());
    for (name,b) in [("objects",o),("position",p),("objdata",d),("light",l),("engine",e)] {fs::write(path.join(format!("DATA/{name}.dat")),b).unwrap();}
}
fn fixture() -> Fixture {
    let root=std::env::temp_dir().join(format!("arm-starter-synthetic-{}",Uuid::new_v4()));
    fs::create_dir_all(root.join("SYSTEM/CLAK")).unwrap();fs::create_dir_all(root.join("SYSTEM/DATA_MP/DEFAULTS")).unwrap();
    fs::write(root.join("SYSTEM/CLAK/cl_scint.ini"),"[ObjDefName]\nGER_HAU00=BauGerHau00_Haupthaus\nGER_INF02=FigGerInf02_Zweihandaxt\n").unwrap();
    let names=[(232,BlankMapStarterKit::MAIN_HOUSE_NAME),(218,BlankMapStarterKit::UNIT_NAME)].map(|(id,name)| {let mut fields=vec!["0".to_owned();53]; fields[0]=id.to_string();fields[52]=name.into();fields.join(",")}).join("\n");
    fs::write(root.join("SYSTEM/DATA_MP/DEFAULTS/objdef.dau"),names).unwrap();
    let target=root.join("MAPS/ENDL_005");map(&target,12,&[],1);
    map(&root.join("MAPS/A_FIRST"),5,&[(232,None),(233,Some(0)),(218,None)],10);
    map(&root.join("MAPS/TUTOR_01"),4,&[(232,None),(234,Some(0)),(218,None)],10);
    Fixture {root,map:target}
}
fn payloads(s:&ObjectStore)->Vec<Vec<u8>> { ["objects","position","objdata","light","engine"].map(|n|s.payload(n).unwrap().to_vec()).to_vec() }
#[test]
fn starter_kit_tutorial_family_full_binding_and_atomic_late_failure() {
    let f=fixture();let kit=BlankMapStarterKit::create_from_game(&f.root,&f.map,7,8000.,8000.,Some(&|_,_|120.),None).unwrap();
    assert_eq!(kit.house_source_map_path.as_ref().unwrap().file_name().unwrap(),"TUTOR_01");
    assert_eq!(kit.native_object_count,6);assert_eq!(kit.house_family_object_count,2);
    assert!(kit.template_for(&kit.spawns[0]).is_none());
    let store=ObjectStore::load(&f.map).unwrap();let before=payloads(&store);let previous=ScenarioDocument::default();
    let result=kit.apply_to_candidate(&store,&previous,None).unwrap();
    assert_eq!(payloads(&store),before);assert_eq!(previous,ScenarioDocument::default());
    assert_eq!(result.store.objects().len(),6);assert_eq!(result.scenario.data_slots.len(),5);assert!(result.skipped.is_empty());
    let root=&result.scenario.data_slots[0];assert_eq!(result.store.family(root.slot as usize,root.uid).unwrap().len(),2);
    assert!(result.store.objects().iter().any(|o|o.type_id==234));
    for s in &result.scenario.spawns {let b=result.scenario.data_slots.iter().find(|b|b.spawn_id==s.id).unwrap();let obj=result.store.objects().into_iter().find(|o|o.slot as i32==b.slot).unwrap();assert_eq!(obj.position,[s.x,s.y,s.z]);assert_eq!(obj.team,7);}
    assert!(kit.apply_to_candidate(&result.store,&result.scenario,None).is_err());
    // House consumes the final two UIDs; the first subsequent unit must fail atomically.
    map(&f.map,12,&[],0xFFFFFE);let limited=ObjectStore::load(&f.map).unwrap();let before=payloads(&limited);
    assert!(kit.apply_to_candidate(&limited,&previous,None).is_err());assert_eq!(payloads(&limited),before);
    assert_eq!(payloads(&ObjectStore::load(&f.map).unwrap()),before);
}
#[test]
fn starter_kit_weighted_budget_and_custom_source_exclusion() {
    let f=fixture();let spawn=Spawn {count:20,..Default::default()};let mut existing=vec![spawn;666];existing.extend(vec![Spawn::default();8]);
    assert!(BlankMapStarterKit::create_from_game(&f.root,&f.map,0,8000.,8000.,None,Some(&existing)).is_ok());
    existing.push(Spawn::default());assert!(BlankMapStarterKit::create_from_game(&f.root,&f.map,0,8000.,8000.,None,Some(&existing)).is_err());
    fs::write(f.root.join("MAPS/TUTOR_01/.arm_custom_map"),"{}").unwrap();
    let kit=BlankMapStarterKit::create_from_game(&f.root,&f.map,0,8000.,8000.,None,None).unwrap();
    assert_eq!(kit.house_source_map_path.unwrap().file_name().unwrap(),"A_FIRST");
}
#[test]
fn starter_kit_scenario_weighted_budget_invalid_alias_catalog_and_no_data_fallback() {
    let f=fixture();fs::write(f.root.join("SYSTEM/CLAK/cl_scint.ini"),"[ObjDefName]\nGER_HAU00=wrong\n").unwrap();
    assert!(BlankMapStarterKit::create_from_game(&f.root,&f.map,0,8000.,8000.,None,None).is_err());
    let kit=BlankMapStarterKit::create(0,8000.,8000.,None,None).unwrap();
    assert!(kit.apply_to_candidate(&ObjectStore::load(&f.map).unwrap(),&ScenarioDocument::default(),None).is_err());
    let mut scenario=ScenarioDocument::default();scenario.spawns=vec![Spawn{count:20,..Default::default()};667];let before=scenario.clone();
    assert!(kit.apply_to_scenario(&mut scenario).is_err());assert_eq!(scenario,before);
}
#[test]
fn starter_kit_scenario_retain_move_rebuild_delete_and_skip() {
    let f=fixture();let kit=BlankMapStarterKit::create_from_game(&f.root,&f.map,2,8000.,8000.,None,None).unwrap();
    let empty=ScenarioDocument::default();let first=kit.apply_to_candidate(&ObjectStore::load(&f.map).unwrap(),&empty,None).unwrap();
    let mut next=first.scenario.clone();next.spawns[0].x+=100.;next.spawns[1].angle=45;next.spawns.remove(4);next.data_slots.retain(|b|next.spawns.iter().any(|s|s.id==b.spawn_id));
    let moved=transaction::apply_to_candidate(&first.store,&first.scenario,&next,&|s|kit.template_for(s)).unwrap();
    assert_eq!(moved.store.objects().len(),5);assert_eq!(moved.scenario.data_slots.len(),4);
    assert_eq!(moved.scenario.data_slots[0],first.scenario.data_slots[0]);
    assert_eq!(first.store.objects()[0].position[0],8000.);
    let mut invalid=next.clone();invalid.spawns[0].alias="changed".into();
    assert!(transaction::apply_to_candidate(&first.store,&first.scenario,&invalid,&|s|kit.template_for(s)).is_err());
    let mut skip=ScenarioDocument::default();skip.spawns.push(Spawn{alias:"missing".into(),prebuilt:true,id:Uuid::new_v4(),..Default::default()});
    let result=transaction::apply_to_candidate(&ObjectStore::load(&f.map).unwrap(),&empty,&skip,&|_|None).unwrap();assert_eq!(result.skipped.len(),1);assert!(result.scenario.data_slots.is_empty());
}

#[test]
fn spawn_transaction_preserves_unchanged_legacy_binding_with_inactive_camp_link() {
    use crate::starter_kit::transaction::Template;

    let f = fixture();
    let source_map = f.root.join("MAPS/A_FIRST");
    let mut objects = fs::read(source_map.join("DATA/objects.dat")).unwrap();
    let capacity = u32::from_le_bytes(objects[4..8].try_into().unwrap()) as usize;
    let camp_col = 16 + capacity * 79 + capacity * COLUMNS[..5].iter().sum::<usize>();
    objects[camp_col..camp_col + 2].copy_from_slice(&0u16.to_le_bytes());
    fs::write(source_map.join("DATA/objects.dat"), objects).unwrap();
    let mut lager = vec![0u8; 144016];
    lager[..16].copy_from_slice(&[
        1u32.to_le_bytes(),
        3200u32.to_le_bytes(),
        6u32.to_le_bytes(),
        10u32.to_le_bytes(),
    ].concat());
    fs::write(source_map.join("DATA/lager.dat"), lager).unwrap();
    let source = ObjectStore::load(&source_map).unwrap();
    assert_eq!(source.family(0, 1).unwrap_err(), "inactive lager camp");

    let id = Uuid::from_u128(0x101);
    let copy_id = Uuid::from_u128(0x102);
    let rotation = source.native_rotation(0, 1).unwrap();
    let angle = transaction::display_angle(rotation);
    let root = source.objects().into_iter().find(|o| o.slot == 0).unwrap();
    let original = Spawn {
        alias: "GER_HAU00".into(), x: root.position[0], y: root.position[1], z: root.position[2],
        team: root.team as i32, count: 0, angle, prebuilt: true, id, raw_rotation: None,
    };
    let previous = ScenarioDocument {
        spawns: vec![original.clone()],
        data_slots: vec![against_rome_scenario::model::DataSlot { slot: 0, uid: 1, spawn_id: id }],
        ..Default::default()
    };
    let copy = Spawn {
        alias: original.alias.clone(), x: root.position[0] + 500.0, y: root.position[1],
        z: root.position[2], team: 1, count: 0, angle: 45, prebuilt: true, id: copy_id,
        raw_rotation: None,
    };
    let next = ScenarioDocument { spawns: vec![original, copy], ..previous.clone() };
    let template_map = f.root.join("MAPS/SINGLETON");
    map(&template_map, 3, &[(232, None)], 10);
    let template = Template { store: ObjectStore::load(template_map).unwrap(), slot: 0, uid: 1 };
    let candidate = transaction::apply_to_candidate_with_copies(
        &source, &previous, &next, &|_| Some(template.clone()), None, None,
    ).unwrap();

    assert_eq!(candidate.scenario.data_slots.len(), 2);
    assert_eq!(candidate.scenario.data_slots[0], previous.data_slots[0]);
    assert_eq!(candidate.store.family(0, 1).unwrap_err(), "inactive lager camp");
    assert_eq!(candidate.store.payload("lager"), source.payload("lager"));
    assert_eq!(candidate.store.objects().iter().find(|o| o.slot == 0).unwrap().position, root.position);

    let mut moved = next;
    moved.spawns[0].x += 1.0;
    assert!(transaction::apply_to_candidate_with_copies(
        &source, &previous, &moved, &|_| Some(template.clone()), None, None,
    ).is_err());
}
