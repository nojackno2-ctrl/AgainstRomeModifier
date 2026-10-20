use against_rome_placement::{ObjectStore, Result};
fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 3 {
        return Err(
            "usage: ar-placement inspect MAP | family MAP SLOT UID | move MAP SLOT UID DX_FIXED DY_FIXED DZ_FIXED | snap MAP SLOT UID | clone MAP SLOT UID DX_FIXED DY_FIXED DZ_FIXED | remove MAP SLOT UID | rotate MAP SLOT UID DEGREES | set-team MAP SLOT UID TEAM | set-vitals MAP SLOT UID HP MORALE".into(),
        );
    }
    let mut store = ObjectStore::load(&args[2])?;
    match args[1].as_str() {
        "inspect" => {
            println!(
                "capacity={} active={}",
                store.capacity(),
                store.objects().len()
            );
            for o in store.objects() {
                let family = if args.get(3).is_some_and(|v| v == &o.slot.to_string()) {
                    Some(store.family(o.slot, o.uid))
                } else {
                    None
                };
                println!("slot={} uid={} type={} team={} xyz={:?} rotation={} parent={:?} editable_family={:?}",o.slot,o.uid,o.type_id,o.team,o.position,o.rotation_degrees,o.parent,family);
            }
        }
        "move" if args.len() == 8 => {
            let parse = |n: usize| args[n].parse::<i64>().map_err(|e| e.to_string());
            let slot = usize::try_from(parse(3)?).map_err(|e| e.to_string())?;
            let uid = u32::try_from(parse(4)?).map_err(|e| e.to_string())?;
            let delta = [
                i32::try_from(parse(5)?).map_err(|e| e.to_string())?,
                i32::try_from(parse(6)?).map_err(|e| e.to_string())?,
                i32::try_from(parse(7)?).map_err(|e| e.to_string())?,
            ];
            store.translate_family_fixed(slot, uid, delta)?;
            store.save()?;
            let reload = ObjectStore::load(&args[2])?;
            println!(
                "saved and reloaded {:?}",
                reload.objects().into_iter().find(|o| o.slot == slot)
            );
        }
        "snap" if args.len() == 5 => {
            let slot = args[3].parse::<usize>().map_err(|e| e.to_string())?;
            let uid = args[4].parse::<u32>().map_err(|e| e.to_string())?;
            let delta = store.snap_family_to_grid(slot, uid)?;
            store.save()?;
            let reload = ObjectStore::load(&args[2])?;
            println!(
                "snapped by delta {:?}; saved and reloaded {:?}",
                delta,
                reload.objects().into_iter().find(|o| o.slot == slot)
            );
        }
        "family" if args.len() == 5 => {
            let slot = args[3].parse::<usize>().map_err(|e| e.to_string())?;
            let uid = args[4].parse::<u32>().map_err(|e| e.to_string())?;
            println!("{:?}", store.family_details(slot, uid)?);
        }
        "clone" if args.len() == 8 => {
            let parse = |n: usize| args[n].parse::<i64>().map_err(|e| e.to_string());
            let slot = usize::try_from(parse(3)?).map_err(|e| e.to_string())?;
            let uid = u32::try_from(parse(4)?).map_err(|e| e.to_string())?;
            let delta = [
                i32::try_from(parse(5)?).map_err(|e| e.to_string())?,
                i32::try_from(parse(6)?).map_err(|e| e.to_string())?,
                i32::try_from(parse(7)?).map_err(|e| e.to_string())?,
            ];
            let (root, new_uid) = store.clone_family_fixed(slot, uid, delta)?;
            store.save()?;
            let reload = ObjectStore::load(&args[2])?;
            println!(
                "cloned to slot={root} uid={new_uid}; saved and reloaded {:?}",
                reload.family_details(root, new_uid)?
            );
        }
        "remove" if args.len() == 5 => {
            let slot = args[3].parse::<usize>().map_err(|e| e.to_string())?;
            let uid = args[4].parse::<u32>().map_err(|e| e.to_string())?;
            store.remove_family(slot, uid)?;
            store.save()?;
            let reload = ObjectStore::load(&args[2])?;
            println!(
                "removed; saved and reloaded, slot {slot} active={}",
                reload.objects().iter().any(|o| o.slot == slot)
            );
        }
        "rotate" if args.len() == 6 => {
            let slot = args[3].parse::<usize>().map_err(|e| e.to_string())?;
            let uid = args[4].parse::<u32>().map_err(|e| e.to_string())?;
            let degrees = args[5].parse::<f32>().map_err(|e| e.to_string())?;
            store.set_family_rotation(slot, uid, degrees)?;
            store.save()?;
            let reload = ObjectStore::load(&args[2])?;
            println!(
                "saved and reloaded {:?}",
                reload.objects().into_iter().find(|o| o.slot == slot)
            );
        }
        "set-team" if args.len() == 6 => {
            let slot = args[3].parse::<usize>().map_err(|e| e.to_string())?;
            let uid = args[4].parse::<u32>().map_err(|e| e.to_string())?;
            let team = args[5].parse::<u16>().map_err(|e| e.to_string())?;
            store.set_family_team(slot, uid, team)?;
            store.save()?;
            let reload = ObjectStore::load(&args[2])?;
            println!(
                "saved and reloaded {:?}",
                reload.objects().into_iter().find(|o| o.slot == slot)
            );
        }
        "set-vitals" if args.len() == 7 => {
            let slot = args[3].parse::<usize>().map_err(|e| e.to_string())?;
            let uid = args[4].parse::<u32>().map_err(|e| e.to_string())?;
            let hp = args[5].parse::<f32>().map_err(|e| e.to_string())?;
            let morale = args[6].parse::<f32>().map_err(|e| e.to_string())?;
            store.set_hp_morale(slot, uid, hp, morale)?;
            store.save()?;
            let reload = ObjectStore::load(&args[2])?;
            println!(
                "saved and reloaded {:?}",
                reload.objects().into_iter().find(|o| o.slot == slot)
            );
        }
        _ => return Err("invalid arguments".into()),
    }
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1)
    }
}
