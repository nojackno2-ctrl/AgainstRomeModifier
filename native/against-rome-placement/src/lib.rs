//! Native DATA saved-state editing. Unknown bytes and untouched encoded files survive.
use against_rome_data::{lzss, particle::ParticlePool, LightPool};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
pub type Result<T> = std::result::Result<T, String>;
pub const COLUMNS: [usize; 18] = [2, 2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 2, 2, 2, 4, 2, 2, 4];
pub const SEGMENTS: [usize; 36] = [
    9, 8, 4, 4, 2, 16, 4, 4, 2, 2, 2, 2, 4, 2, 2, 2, 4, 4, 4, 4, 2, 4, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 4, 4, 2, 2,
];
fn u16at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}
fn u32at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn f32at(b: &[u8], o: usize) -> f32 {
    f32::from_bits(u32at(b, o))
}
fn write_stage(path: &Path, bytes: &[u8], permissions: fs::Permissions) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::set_permissions(path, permissions)?;
    file.sync_all()
}
#[derive(Clone)]
struct Blob {
    raw: Vec<u8>,
    bytes: Vec<u8>,
    initial: Vec<u8>,
    header: Option<[u8; 64]>,
}
impl Blob {
    fn read(p: &Path) -> Result<Self> {
        let raw = fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
        Self::decode(raw)
    }
    fn decode(raw: Vec<u8>) -> Result<Self> {
        let (bytes, header) = if raw.starts_with(b"PFIL") {
            let b = lzss::decompress_pfil(&raw, None).map_err(|e| e.to_string())?;
            (b, Some(raw[..64].try_into().unwrap()))
        } else {
            (raw.clone(), None)
        };
        Ok(Self {
            raw,
            initial: bytes.clone(),
            bytes,
            header,
        })
    }
    fn encoded(&self) -> Vec<u8> {
        if self.bytes == self.initial {
            return self.raw.clone();
        }
        let Some(mut h) = self.header else {
            return self.bytes.clone();
        };
        h[16..20].copy_from_slice(&(self.bytes.len() as u32).to_le_bytes());
        let mut out = h.to_vec();
        for part in self.bytes.chunks(8) {
            out.push(((1u16 << part.len()) - 1) as u8);
            out.extend_from_slice(part)
        }
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> ObjectStore {
        fixture_count(3)
    }
    fn fixture_count(n: usize) -> ObjectStore {
        let mut o = vec![0; 16 + n * (79 + COLUMNS.iter().sum::<usize>())];
        o[0..4].copy_from_slice(&1u32.to_le_bytes());
        o[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        o[8..12].copy_from_slice(&30u32.to_le_bytes());
        o[12..16].copy_from_slice(&30u32.to_le_bytes());
        for s in 0..n {
            let r = 16 + s * 79;
            o[r] = 1;
            o[r + 3..r + 7].copy_from_slice(&(s as u32 + 1).to_le_bytes());
            for offset in [67, 69] {
                o[r + offset..r + offset + 2].copy_from_slice(&(s as u16).to_le_bytes());
            }
            for offset in [71, 73, 77] {
                o[r + offset..r + offset + 2].fill(255);
            }
            for c in 0..18 {
                let p = 16 + n * 79 + n * COLUMNS[..c].iter().sum::<usize>() + s * COLUMNS[c];
                if COLUMNS[c] == 2 {
                    o[p..p + 2].fill(255)
                }
                if c == 1 {
                    o[p..p + 2].copy_from_slice(&(s as u16).to_le_bytes());
                }
            }
        }
        let mut p = vec![0; 8 + n * 17];
        p[0..4].copy_from_slice(&1u32.to_le_bytes());
        p[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        for s in 0..n {
            p[8 + s * 17] = 1;
            p[9 + s * 17..13 + s * 17].copy_from_slice(&(s as f32 * 20.).to_le_bytes());
            p[21 + s * 17..25 + s * 17].copy_from_slice(&90f32.to_le_bytes());
        }
        let mut d = vec![0; 8 + n * SEGMENTS.iter().sum::<usize>()];
        d[0..4].copy_from_slice(&1u32.to_le_bytes());
        d[4..8].copy_from_slice(&(n as u32).to_le_bytes());
        for s in 0..n {
            d[8 + s * 9] = 1;
        }
        let mut b = BTreeMap::new();
        for (name, bytes) in [("objects", o), ("objdata", d), ("position", p)] {
            b.insert(name.into(), Blob::decode(bytes).unwrap());
        }
        ObjectStore::build(b, PathBuf::new()).unwrap()
    }
    fn disk_fixture(label: &str) -> ObjectStore {
        let mut store = fixture();
        store.root = std::env::temp_dir().join(format!(
            "ar-placement-{label}-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir(&store.root).unwrap();
        for (name, blob) in &store.pools {
            fs::write(store.root.join(format!("{name}.dat")), &blob.raw).unwrap();
        }
        store
    }
    fn stages(store: &ObjectStore) -> Vec<PathBuf> {
        fs::read_dir(&store.root).unwrap().map(|e| e.unwrap().path())
            .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(".ar-placement-stage-")).collect()
    }
    fn assert_original_files(store: &ObjectStore) {
        for (name, blob) in &store.pools {
            assert_eq!(fs::read(store.root.join(format!("{name}.dat"))).unwrap(), blob.raw, "{name}");
        }
        assert!(!store.root.join(".ar-placement.lock").exists());
    }
    #[test]
    fn save_readonly_preflight_never_changes_originals_or_creates_stage() {
        let mut store = disk_fixture("readonly");
        store.remove_family(0, 1).unwrap();
        let pending = store.snapshot();
        let path = store.root.join("position.dat");
        let writable = fs::metadata(&path).unwrap().permissions();
        let mut readonly = writable.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&path, readonly).unwrap();
        let error = store.save().unwrap_err();
        assert!(error.contains("read-only pool file"), "{error}");
        assert_original_files(&store);
        assert_eq!(store.snapshot(), pending);
        assert!(store.is_dirty());
        assert!(fs::metadata(&path).unwrap().permissions().readonly());
        assert!(stages(&store).is_empty());
        fs::set_permissions(path, writable).unwrap();
    }
    #[test]
    fn save_preserves_permissions_and_exact_backup_bytes() {
        let mut store = disk_fixture("permissions");
        let path = store.root.join("position.dat");
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        }
        let expected = fs::metadata(&path).unwrap().permissions();
        let original = fs::read(&path).unwrap();
        store.translate_family_fixed(0, 1, [65536, 0, 0]).unwrap();
        let pending = store.payload("position").unwrap().to_vec();
        store.save().unwrap();
        assert_eq!(fs::read(&path).unwrap(), pending);
        assert!(!store.is_dirty());
        let actual = fs::metadata(&path).unwrap().permissions();
        assert_eq!(actual.readonly(), expected.readonly());
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(actual.mode(), expected.mode());
        }
        let stage = stages(&store);
        assert_eq!(stage.len(), 1);
        let backup = stage[0].join("position.old");
        assert_eq!(fs::read(&backup).unwrap(), original);
        assert_eq!(fs::metadata(backup).unwrap().permissions().readonly(), expected.readonly());
    }
    #[test]
    fn stage_failure_preserves_partial_candidates_and_all_originals() {
        let mut store = disk_fixture("stage-failure");
        store.remove_family(0, 1).unwrap();
        let mut staged = 0;
        let error = store.save_using_io(
            |_, _, _| panic!("must not publish before every stage is complete"),
            |path, bytes, permissions| {
                staged += 1;
                if staged == 2 {
                    fs::write(path, b"partial candidate evidence")?;
                    Err(std::io::Error::other("injected staging write failure"))
                } else { write_stage(path, bytes, permissions) }
            }
        ).unwrap_err();
        assert!(error.contains("injected staging write failure"), "{error}");
        assert!(error.contains("recovery files retained at"), "{error}");
        assert_original_files(&store);
        assert!(store.is_dirty());
        let stage = stages(&store);
        assert_eq!(stage.len(), 1);
        assert!(error.contains(&stage[0].display().to_string()));
        assert!(stage[0].join("objdata.new").is_file());
        assert_eq!(fs::read(stage[0].join("objects.new")).unwrap(), b"partial candidate evidence");
        store.save().unwrap();
        assert!(stage[0].join("objects.new").is_file());
    }
    #[test]
    fn backup_failure_preserves_existing_evidence_and_all_originals() {
        let mut store = disk_fixture("backup-failure");
        store.remove_family(0, 1).unwrap();
        let error = store.save_using_io(
            |_, _, _| panic!("blocked backup must prevent publication"),
            |path, bytes, permissions| {
                write_stage(path, bytes, permissions)?;
                if path.file_stem().unwrap() == "objdata" {
                    let blocked = path.with_extension("old");
                    fs::create_dir(&blocked)?;
                    fs::write(blocked.join("keep.txt"), b"existing recovery evidence")?;
                }
                Ok(())
            }
        ).unwrap_err();
        assert!(error.contains("recovery files retained at"), "{error}");
        assert_original_files(&store);
        assert!(store.is_dirty());
        let stage = stages(&store);
        assert_eq!(stage.len(), 1);
        assert_eq!(fs::read(stage[0].join("objdata.old/keep.txt")).unwrap(), b"existing recovery evidence");
        assert!(stage[0].join("objdata.new").is_file());
    }
    #[test]
    fn rollback_preserves_external_target_and_reports_original_backup() {
        let mut store = disk_fixture("external-rollback");
        store.remove_family(0, 1).unwrap();
        let original = store.pools["objdata"].raw.clone();
        let first = store.root.join("objdata.dat");
        let error = store.save_using(|index, from, to| {
            if index == 1 {
                fs::write(&first, b"another editor's unrelated work")?;
                Err(std::io::Error::other("injected second publication failure"))
            } else { fs::rename(from, to) }
        }).unwrap_err();
        assert!(error.contains("rollback unexpected target preserved"), "{error}");
        assert_eq!(fs::read(first).unwrap(), b"another editor's unrelated work");
        let stage = stages(&store);
        assert_eq!(stage.len(), 1);
        let backup = stage[0].join("objdata.old");
        assert_eq!(fs::read(&backup).unwrap(), original);
        assert!(error.contains(&backup.display().to_string()));
        for name in ["objects", "position"] {
            assert_eq!(fs::read(store.root.join(format!("{name}.dat"))).unwrap(), store.pools[name].raw);
        }
        assert!(store.is_dirty());
        assert!(!store.root.join(".ar-placement.lock").exists());
    }
    #[test]
    fn rollback_never_uses_externally_changed_backup() {
        let mut store = disk_fixture("changed-backup");
        store.remove_family(0, 1).unwrap();
        let pending = store.payload("objdata").unwrap().to_vec();
        let mut first_backup = None;
        let error = store.save_using(|index, from, to| {
            if index == 0 {
                first_backup = Some(from.with_file_name("objdata.old"));
                fs::rename(from, to)
            } else {
                fs::write(first_backup.as_ref().unwrap(), b"changed backup")?;
                Err(std::io::Error::other("injected backup corruption"))
            }
        }).unwrap_err();
        assert!(error.contains("rollback backup is unreadable or changed"), "{error}");
        assert!(error.contains("recovery files retained at"), "{error}");
        assert_eq!(fs::read(store.root.join("objdata.dat")).unwrap(), pending);
        assert_eq!(fs::read(first_backup.unwrap()).unwrap(), b"changed backup");
        assert!(store.is_dirty());
    }
    #[test]
    fn move_preserves_unknown_rotation_and_undo() {
        let mut s = fixture();
        let before = s.snapshot();
        s.translate_family_fixed(1, 2, [65536, -32768, 123])
            .unwrap();
        let o = s.objects();
        assert_eq!(o[1].position, [21., -0.5, 123. / 65536.]);
        assert_eq!(o[1].rotation_degrees, 90.0);
        assert_eq!(s.payload("objects"), Some(before["objects"].as_slice()));
        assert_eq!(s.payload("objdata"), Some(before["objdata"].as_slice()));
        assert!(s.undo());
        assert_eq!(s.snapshot(), before);
        assert!(s.redo());
        assert_eq!(s.objects()[1].position[0], 21.);
    }
    #[test]
    fn stale_and_external_position_refuse_without_mutation() {
        let mut s = fixture();
        let before = s.snapshot();
        assert!(s.translate_family_fixed(1, 99, [1, 2, 3]).is_err());
        assert_eq!(s.snapshot(), before);
        let r = s.record(2) + 67;
        s.pools.get_mut("objects").unwrap().bytes[r..r + 2].copy_from_slice(&1u16.to_le_bytes());
        let before = s.snapshot();
        assert!(s.translate_family_fixed(1, 2, [1, 2, 3]).is_err());
        assert_eq!(s.snapshot(), before);
    }
    #[test]
    fn rotation_edits_normalize_and_undo_redo() {
        let mut s = fixture();
        let before = s.snapshot();
        // Initial rotation is 90.0
        assert_eq!(s.objects()[0].rotation_degrees, 90.0);
        // Absolute rotation with normalization
        s.set_family_rotation(0, 1, 375.0).unwrap();
        assert_eq!(s.objects()[0].rotation_degrees, 15.0);
        // Relative rotation with negative wrap
        s.rotate_family(0, 1, -45.0).unwrap();
        assert_eq!(s.objects()[0].rotation_degrees, 330.0);
        // Nonfinite rotation is rejected with snapshot preserved
        let state = s.snapshot();
        assert!(s.set_family_rotation(0, 1, f32::NAN).is_err());
        assert!(s.rotate_family(0, 1, f32::INFINITY).is_err());
        assert_eq!(s.snapshot(), state);
        // Undo relative rotation
        assert!(s.undo());
        assert_eq!(s.objects()[0].rotation_degrees, 15.0);
        // Undo absolute rotation
        assert!(s.undo());
        assert_eq!(s.objects()[0].rotation_degrees, 90.0);
        assert_eq!(s.snapshot(), before);
        // Redo chain
        assert!(s.redo());
        assert_eq!(s.objects()[0].rotation_degrees, 15.0);
        assert!(s.redo());
        assert_eq!(s.objects()[0].rotation_degrees, 330.0);
    }
    #[test]
    fn team_edits_validate_propagate_and_undo_redo() {
        let mut s = fixture();
        add_light(&mut s);
        // Attach slot 1 as child of slot 0
        let parent = s.column_offset(1, 13);
        s.pools.get_mut("objects").unwrap().bytes[parent..parent + 2]
            .copy_from_slice(&0u16.to_le_bytes());
        assert_eq!(s.family(0, 1).unwrap(), vec![0, 1]);
        let before = s.snapshot();
        // Set team on family: both parent (0) and child (1) receive team 5
        s.set_family_team(0, 1, 5).unwrap();
        assert_eq!(s.objects()[0].team, 5);
        assert_eq!(s.objects()[1].team, 5);
        // Unassigned team (65535) is allowed
        s.set_family_team(0, 1, 65535).unwrap();
        assert_eq!(s.objects()[0].team, 65535);
        assert_eq!(s.objects()[1].team, 65535);
        // Team > 15 (except 65535) is rejected without mutation
        let state = s.snapshot();
        assert!(s.set_family_team(0, 1, 16).is_err());
        assert!(s.set_family_team(0, 1, 100).is_err());
        assert_eq!(s.snapshot(), state);
        // Undo to team 5
        assert!(s.undo());
        assert_eq!(s.objects()[0].team, 5);
        assert_eq!(s.objects()[1].team, 5);
        // Undo to initial
        assert!(s.undo());
        assert_eq!(s.snapshot(), before);
        // Redo
        assert!(s.redo());
        assert_eq!(s.objects()[0].team, 5);
    }
    #[test]
    fn vitals_edits_read_write_propagate_and_undo_redo() {
        let mut s = fixture();
        // Set HP and morale via convenience method
        s.set_hp_morale(0, 1, 500.0, 80.0).unwrap();
        let v = s.vitals(0).unwrap();
        assert_eq!(v.hp, 500.0);
        assert_eq!(v.max_hp_ratio, 1.0);
        assert_eq!(v.morale, 80.0);
        assert_eq!(v.max_morale_ratio, 1.0);
        assert_eq!(s.objects()[0].vitals, Some(v));
        // Set custom vitals with custom ratios
        let custom = Vitals {
            hp: 250.0,
            max_hp_ratio: 0.5,
            morale: 40.0,
            max_morale_ratio: 0.75,
        };
        s.set_family_vitals(0, 1, custom).unwrap();
        assert_eq!(s.vitals(0), Some(custom));
        // Reject nonfinite and negative ratios
        let state = s.snapshot();
        assert!(s.set_hp_morale(0, 1, f32::NAN, 50.0).is_err());
        assert!(s.set_family_vitals(0, 1, Vitals { hp: 100.0, max_hp_ratio: -0.1, morale: 50.0, max_morale_ratio: 1.0 }).is_err());
        assert_eq!(s.snapshot(), state);
        // Stale UID rejection
        assert!(s.set_hp_morale(0, 999, 100.0, 50.0).is_err());
        // Undo restores previous vitals
        assert!(s.undo());
        assert_eq!(s.vitals(0).unwrap().hp, 500.0);
        // Undo to initial
        assert!(s.undo());
        assert_eq!(s.vitals(0).unwrap().hp, 0.0);
        // Redo
        assert!(s.redo());
        assert_eq!(s.vitals(0).unwrap().hp, 500.0);
    }
    #[test]
    fn placement_edits_disk_save_and_reload() {
        let mut store = fixture();
        let map_root = std::env::temp_dir().join(format!(
            "ar-placement-edits-disk-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let data_dir = map_root.join("DATA");
        fs::create_dir_all(&data_dir).unwrap();
        store.root = data_dir.clone();
        for (name, blob) in &store.pools {
            fs::write(data_dir.join(format!("{name}.dat")), &blob.raw).unwrap();
        }
        store.set_family_rotation(0, 1, 180.0).unwrap();
        store.set_family_team(0, 1, 7).unwrap();
        store.set_hp_morale(0, 1, 750.0, 95.0).unwrap();
        assert!(store.is_dirty());
        store.save().unwrap();
        assert!(!store.is_dirty());
        let reloaded = ObjectStore::load(&map_root).unwrap();
        let obj = reloaded.objects().into_iter().find(|o| o.slot == 0).unwrap();
        assert_eq!(obj.rotation_degrees, 180.0);
        assert_eq!(obj.team, 7);
        let vitals = obj.vitals.unwrap();
        assert_eq!(vitals.hp, 750.0);
        assert_eq!(vitals.morale, 95.0);
    }
    #[test]
    fn pfil_literal_encoder_keeps_header_and_roundtrips() {
        let mut h = [0x79; 64];
        h[..4].copy_from_slice(b"PFIL");
        h[16..20].copy_from_slice(&3u32.to_le_bytes());
        let mut raw = h.to_vec();
        raw.extend_from_slice(&[7, 1, 2, 3]);
        let mut b = Blob::decode(raw.clone()).unwrap();
        assert_eq!(b.encoded(), raw);
        b.bytes = (0..251).collect();
        let encoded = b.encoded();
        assert_eq!(encoded[20..64], h[20..64]);
        assert_eq!(lzss::decompress_pfil(&encoded, None).unwrap(), b.bytes);
    }
    #[test]
    fn nonfinite_rejects_transaction() {
        let mut s = fixture();
        s.pools.get_mut("position").unwrap().bytes[9..13].copy_from_slice(&f32::NAN.to_le_bytes());
        let before = s.snapshot();
        assert!(s.translate_family_fixed(0, 1, [1, 2, 3]).is_err());
        assert_eq!(s.snapshot(), before);
    }
    #[test]
    fn clone_delete_reallocate_and_undo() {
        let mut s = fixture();
        let r = s.record(2);
        s.pools.get_mut("objects").unwrap().bytes[r] = 0;
        s.pools.get_mut("position").unwrap().bytes[8 + 2 * 17] = 0;
        s.pools.get_mut("objdata").unwrap().bytes[8 + 2 * 9] = 0;
        let before = s.snapshot();
        let (slot, uid) = s.clone_family_fixed(0, 1, [65536, 0, 0]).unwrap();
        assert_eq!(slot, 2);
        assert_eq!(uid, 3);
        let copy = s.objects().into_iter().find(|o| o.slot == slot).unwrap();
        assert_eq!(copy.position[0], 1.);
        assert_eq!(copy.rotation_degrees, 90.0);
        assert_eq!(s.objects()[0].position[0], 0.);
        s.remove_family(slot, uid).unwrap();
        assert!(!s.active(slot));
        assert!(s.undo());
        assert!(s.active(slot));
        assert!(s.undo());
        assert_eq!(s.snapshot(), before);
    }
    fn add_light(s: &mut ObjectStore) {
        let mut b = vec![0; against_rome_data::pool::PAYLOAD_LEN];
        b[0..4].copy_from_slice(&1u32.to_le_bytes());
        b[4..8].copy_from_slice(&1024u32.to_le_bytes());
        b[8..12].copy_from_slice(&1u32.to_le_bytes());
        let widths = against_rome_data::pool::WIDTHS;
        let mut base = 8;
        for (c, w) in widths.iter().enumerate() {
            if c >= 13 {
                b[base..base + 1024 * w].fill(255);
            }
            base += 1024 * w;
        }
        let owner_offset = 8 + 1024 * widths[..13].iter().sum::<usize>();
        b[owner_offset..owner_offset + 2].copy_from_slice(&0u16.to_le_bytes());
        let o = s.column_offset(0, 12);
        s.pools.get_mut("objects").unwrap().bytes[o..o + 2].copy_from_slice(&0u16.to_le_bytes());
        s.pools.insert("light".into(), Blob::decode(b).unwrap());
    }
    #[test]
    fn parent_and_direct_light_move_copy_release() {
        let mut s = fixture();
        add_light(&mut s);
        let parent = s.column_offset(1, 13);
        s.pools.get_mut("objects").unwrap().bytes[parent..parent + 2]
            .copy_from_slice(&0u16.to_le_bytes());
        assert_eq!(s.family(0, 1).unwrap(), vec![0, 1]);
        let original = s.snapshot();
        s.translate_family_fixed(0, 1, [65536, 131072, 196608])
            .unwrap();
        assert_eq!(s.objects()[0].position, [1., 2., 3.]);
        assert_eq!(s.objects()[1].position, [21., 2., 3.]);
        let lights = LightPool::from_payload(s.payload("light").unwrap()).unwrap();
        assert_eq!(lights.capture(0).unwrap().position(), [1., 2., 3.]);
        assert!(s.undo());
        assert_eq!(s.snapshot(), original);
        // Single-root copy exercises independent light and object allocations.
        s.pools.get_mut("objects").unwrap().bytes[parent..parent + 2].fill(255);
        let r = s.record(2);
        s.pools.get_mut("objects").unwrap().bytes[r] = 0;
        s.pools.get_mut("position").unwrap().bytes[8 + 2 * 17] = 0;
        s.pools.get_mut("objdata").unwrap().bytes[8 + 2 * 9] = 0;
        let (copy, uid) = s.clone_family_fixed(0, 1, [65536, 0, 0]).unwrap();
        assert_eq!(copy, 2);
        let light = s.col(copy, 12);
        assert_eq!(light, 1);
        let lights = LightPool::from_payload(s.payload("light").unwrap()).unwrap();
        assert_eq!(lights.capture(1).unwrap().owner(), 2);
        assert_eq!(lights.capture(1).unwrap().position(), [1., 0., 0.]);
        s.remove_family(copy, uid).unwrap();
        assert!(LightPool::from_payload(s.payload("light").unwrap())
            .unwrap()
            .capture(1)
            .is_none());
        assert!(LightPool::from_payload(s.payload("light").unwrap())
            .unwrap()
            .capture(0)
            .is_some());
    }
    #[test]
    fn save_second_publish_failure_restores_all_and_retry() {
        let mut s = fixture();
        let dir = std::env::temp_dir().join(format!("ar-placement-failure-{}", std::process::id()));
        let _ = fs::create_dir(&dir);
        s.root = dir;
        for (n, b) in &s.pools {
            fs::write(s.root.join(format!("{n}.dat")), &b.raw).unwrap();
        }
        let original = s.snapshot();
        s.remove_family(0, 1).unwrap();
        let pending = s.snapshot();
        let result = s.save_using(|i, from, to| {
            if i == 1 {
                Err(std::io::Error::other("injected second publish"))
            } else {
                fs::rename(from, to)
            }
        });
        assert!(result.unwrap_err().contains("injected second publish"));
        assert_eq!(s.snapshot(), pending);
        for (n, bytes) in original {
            assert_eq!(
                Blob::read(&s.root.join(format!("{n}.dat"))).unwrap().bytes,
                bytes
            );
        }
        assert!(!s.root.join(".ar-placement.lock").exists());
        s.save().unwrap();
        for (n, bytes) in pending {
            assert_eq!(
                Blob::read(&s.root.join(format!("{n}.dat"))).unwrap().bytes,
                bytes
            );
        }
    }
    #[test]
    fn save_checks_unchanged_dependency_and_exclusive_lock() {
        let mut s = fixture();
        s.root = std::env::temp_dir().join(format!("ar-placement-conflict-{}", std::process::id()));
        fs::create_dir_all(&s.root).unwrap();
        for (n, b) in &s.pools {
            fs::write(s.root.join(format!("{n}.dat")), &b.raw).unwrap();
        }
        s.translate_family_fixed(0, 1, [65536, 0, 0]).unwrap();
        let position = fs::read(s.root.join("position.dat")).unwrap();
        let objects = s.root.join("objects.dat");
        let mut changed = fs::read(&objects).unwrap();
        changed[20] ^= 1;
        fs::write(&objects, &changed).unwrap();
        assert!(s.save().unwrap_err().contains("external file change"));
        assert_eq!(fs::read(s.root.join("position.dat")).unwrap(), position);
        assert_eq!(fs::read(objects).unwrap(), changed);
        fs::write(s.root.join(".ar-placement.lock"), b"another editor").unwrap();
        assert!(s.save().unwrap_err().contains("save lock"));
        assert_eq!(
            fs::read(s.root.join(".ar-placement.lock")).unwrap(),
            b"another editor"
        );
    }
    #[test]
    fn failed_rollback_retains_original_recovery_evidence() {
        let mut s = fixture();
        s.root = std::env::temp_dir().join(format!("ar-placement-recovery-{}", std::process::id()));
        fs::create_dir_all(&s.root).unwrap();
        for (n, b) in &s.pools {
            fs::write(s.root.join(format!("{n}.dat")), &b.raw).unwrap();
        }
        let original = s.snapshot();
        s.remove_family(0, 1).unwrap();
        let error = s
            .save_using(|i, from, to| {
                if i == 1 {
                    fs::create_dir(to)?;
                    Err(std::io::Error::other(
                        "injected inaccessible rollback target",
                    ))
                } else {
                    fs::rename(from, to)
                }
            })
            .unwrap_err();
        assert!(error.contains("recovery files retained at"));
        let stage = fs::read_dir(&s.root)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| {
                p.is_dir()
                    && p.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with(".ar-placement-stage-")
            })
            .unwrap();
        let backup = fs::read_dir(&stage)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p.extension().is_some_and(|s| s == "old"))
            .unwrap();
        assert_eq!(
            fs::read(&backup).unwrap(),
            original[backup.file_stem().unwrap().to_str().unwrap()]
        );
        assert!(s.is_dirty());
        assert!(!s.root.join(".ar-placement.lock").exists());
    }
    #[test]
    fn direct_light_external_alias_and_mixed_attachment_rejected() {
        let mut s = fixture();
        add_light(&mut s);
        let alias = s.column_offset(2, 12);
        s.pools.get_mut("objects").unwrap().bytes[alias..alias + 2]
            .copy_from_slice(&0u16.to_le_bytes());
        assert!(s
            .family(0, 1)
            .unwrap_err()
            .contains("external direct light"));
        s.pools.get_mut("objects").unwrap().bytes[alias..alias + 2].fill(255);
        let o = 8 + 1024 * against_rome_data::pool::WIDTHS[..14].iter().sum::<usize>();
        s.pools.get_mut("light").unwrap().bytes[o..o + 2].copy_from_slice(&0u16.to_le_bytes());
        assert!(s.family(0, 1).unwrap_err().contains("mixed"));
    }
    #[test]
    #[ignore = "requires installed retail map; read only source, edits TEMP"]
    fn retail_temp_roundtrip() {
        let source = Path::new(r"C:\Program Files (x86)\Against Rome\MAPS\TUTOR_01");
        let dir = std::env::temp_dir().join(format!("ar-rust-placement-{}", std::process::id()));
        fs::create_dir_all(dir.join("DATA")).unwrap();
        let before: Vec<_> = fs::read_dir(source.join("DATA"))
            .unwrap()
            .map(|e| {
                let p = e.unwrap().path();
                let bytes = fs::read(&p).unwrap();
                fs::write(dir.join("DATA").join(p.file_name().unwrap()), &bytes).unwrap();
                (p, bytes)
            })
            .collect();
        let mut s = ObjectStore::load(&dir).unwrap();
        let object = s
            .objects()
            .into_iter()
            .find(|o| s.family(o.slot, o.uid).is_ok())
            .expect("editable real object");
        let original = s.snapshot();
        s.translate_family_fixed(object.slot, object.uid, [65536, 32768, 0])
            .unwrap();
        let changed = s.snapshot();
        assert_ne!(changed, original);
        s.save().unwrap();
        let reload = ObjectStore::load(&dir).unwrap();
        assert_eq!(reload.snapshot(), changed);
        assert_eq!(
            reload
                .objects()
                .into_iter()
                .find(|o| o.slot == object.slot)
                .unwrap()
                .uid,
            object.uid
        );
        assert!(s.undo());
        s.save().unwrap();
        assert_eq!(ObjectStore::load(&dir).unwrap().snapshot(), original);
        let (root, uid) = s
            .clone_family_fixed(object.slot, object.uid, [131072, 0, 0])
            .unwrap();
        s.save().unwrap();
        let mut cloned = ObjectStore::load(&dir).unwrap();
        assert!(cloned
            .objects()
            .iter()
            .any(|o| o.slot == root && o.uid == uid));
        cloned.remove_family(root, uid).unwrap();
        cloned.save().unwrap();
        assert!(!ObjectStore::load(&dir).unwrap().active(root));
        for (p, b) in before {
            assert_eq!(fs::read(&p).unwrap(), b, "source changed {}", p.display());
        }
        println!(
            "retail slot={} UID={} TEMP={} source bytes unchanged",
            object.slot,
            object.uid,
            dir.display()
        );
    }
    #[test]
    #[ignore = "requires dotnet and installed retail map; real C# differential in TEMP"]
    fn retail_managed_differential() {
        let source = Path::new(r"C:\Program Files (x86)\Against Rome\MAPS\TUTOR_01");
        let root = std::env::temp_dir().join(format!("ar-placement-oracle-{}", std::process::id()));
        let rust = root.join("rust");
        let managed = root.join("managed");
        let output = root.join("oracle");
        for map in [&rust, &managed] {
            fs::create_dir_all(map.join("DATA")).unwrap();
        }
        let before: Vec<_> = fs::read_dir(source.join("DATA"))
            .unwrap()
            .map(|e| {
                let path = e.unwrap().path();
                let bytes = fs::read(&path).unwrap();
                for map in [&rust, &managed] {
                    fs::write(map.join("DATA").join(path.file_name().unwrap()), &bytes).unwrap();
                }
                (path, bytes)
            })
            .collect();
        let mut s = ObjectStore::load(&rust).unwrap();
        let object = s
            .objects()
            .into_iter()
            .find(|o| s.family(o.slot, o.uid).is_ok())
            .unwrap();
        let project =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/oracle/PlacementOracle.csproj");
        let status = std::process::Command::new("dotnet")
            .args(["run", "--project"])
            .arg(project)
            .args(["--configuration", "Release", "--"])
            .arg(&managed)
            .arg(&output)
            .arg(object.slot.to_string())
            .arg(object.uid.to_string())
            .status()
            .unwrap();
        assert!(status.success());
        let compare = |s: &ObjectStore, stage: &str| {
            let mut count = 0;
            for (n, b) in &s.pools {
                let expected = fs::read(output.join(stage).join(format!("{n}.bin"))).unwrap();
                if expected != b.bytes {
                    let first = expected.iter().zip(&b.bytes).position(|(a, b)| a != b);
                    panic!(
                        "C# differential {stage}/{n}: lengths {}/{} first mismatch {:?}",
                        expected.len(),
                        b.bytes.len(),
                        first
                    );
                }
                count += b.bytes.len();
            }
            println!(
                "C# {stage}: {count} decoded bytes identical across {} native pools",
                s.pools.len()
            );
        };
        s.translate_family_fixed(object.slot, object.uid, [65536, 32768, -123])
            .unwrap();
        compare(&s, "move");
        let (copy, uid) = s
            .clone_family_fixed(object.slot, object.uid, [131072, -65536, 4321])
            .unwrap();
        assert_eq!(
            fs::read_to_string(output.join("copy.txt")).unwrap(),
            format!("{copy} {uid}")
        );
        compare(&s, "clone");
        s.remove_family(copy, uid).unwrap();
        compare(&s, "delete");
        for (p, b) in before {
            assert_eq!(fs::read(p).unwrap(), b);
        }
        println!("retail C# oracle source unchanged; TEMP={}", root.display());
    }
    #[test]
    #[ignore = "requires dotnet; real C# family/light/auxiliary differential in TEMP"]
    fn managed_family_light_auxiliary_differential() {
        let mut s = fixture_count(5);
        for i in 2..5 {
            let r = s.record(i);
            s.pools.get_mut("objects").unwrap().bytes[r] = 0;
            s.pools.get_mut("position").unwrap().bytes[8 + i * 17] = 0;
            s.pools.get_mut("objdata").unwrap().bytes[8 + i * 9] = 0;
        }
        let o = s.column_offset(1, 13);
        s.pools.get_mut("objects").unwrap().bytes[o..o + 2].copy_from_slice(&0u16.to_le_bytes());
        for i in 0..2 {
            s.pools.get_mut("objdata").unwrap().bytes[8 + i * 9] = 1;
        }
        add_light(&mut s);
        let b = &mut s.pools.get_mut("light").unwrap().bytes;
        b[12..16].copy_from_slice(&1u32.to_le_bytes());
        let widths = against_rome_data::pool::WIDTHS;
        for (c, value) in [(14, 1u16), (15, 7u16)] {
            let o = 8 + 1024 * widths[..c].iter().sum::<usize>() + 2;
            b[o..o + 2].copy_from_slice(&value.to_le_bytes());
        }
        for (name, h, stride, tails) in [
            ("anim", 8, 21, 2),
            ("gfxtype", 8, 15, 1),
            ("action", 12, 25, 0),
        ] {
            let mut b = vec![0; h + 5 * (stride + tails * 2)];
            b[..4].copy_from_slice(&1u32.to_le_bytes());
            b[4..8].copy_from_slice(&5u32.to_le_bytes());
            if h == 12 {
                b[8..12].copy_from_slice(&6u32.to_le_bytes());
            }
            for i in 0..5 {
                for j in 1..stride {
                    b[h + i * stride + j] = (i * 31 + j) as u8;
                }
                b[h + i * stride] = if i < 2 { 1 } else { 0 };
                for t in 0..tails {
                    let o = h + 5 * stride + t * 10 + i * 2;
                    b[o..o + 2]
                        .copy_from_slice(&(0x1100u16 + t as u16 * 256 + i as u16).to_le_bytes());
                }
            }
            s.pools.insert(name.into(), Blob::decode(b).unwrap());
            for i in 0..2 {
                let o = s.record(i)
                    + match name {
                        "anim" => 71,
                        "gfxtype" => 73,
                        _ => 77,
                    };
                s.pools.get_mut("objects").unwrap().bytes[o..o + 2]
                    .copy_from_slice(&(i as u16).to_le_bytes());
            }
        }
        let mut engine = vec![0xA3; 94];
        engine[..4].copy_from_slice(&1u32.to_le_bytes());
        engine[34..38].copy_from_slice(&100u32.to_le_bytes());
        s.pools
            .insert("engine".into(), Blob::decode(engine).unwrap());
        let root =
            std::env::temp_dir().join(format!("ar-placement-family-oracle-{}", std::process::id()));
        let managed = root.join("managed");
        let output = root.join("oracle");
        fs::create_dir_all(managed.join("DATA")).unwrap();
        for (n, b) in &s.pools {
            fs::write(managed.join("DATA").join(format!("{n}.dat")), &b.bytes).unwrap();
        }
        let project =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/oracle/PlacementOracle.csproj");
        let status = std::process::Command::new("dotnet")
            .args(["run", "--project"])
            .arg(project)
            .args(["--configuration", "Release", "--"])
            .arg(managed)
            .arg(&output)
            .args(["0", "1"])
            .status()
            .unwrap();
        assert!(status.success());
        let compare = |s: &ObjectStore, stage: &str| {
            for (n, b) in &s.pools {
                let expected = fs::read(output.join(stage).join(format!("{n}.bin"))).unwrap();
                if expected != b.bytes {
                    panic!(
                        "C# {stage}/{n} first mismatch {:?}",
                        expected.iter().zip(&b.bytes).position(|(a, b)| a != b)
                    )
                }
            }
            println!("C# synthetic {stage}: {} pools bitwise identical; parent family, direct+attached lights, anim/gfx tails and engineUID",s.pools.len());
        };
        s.translate_family_fixed(0, 1, [65536, 32768, -123])
            .unwrap();
        compare(&s, "move");
        let (copy, uid) = s.clone_family_fixed(0, 1, [131072, -65536, 4321]).unwrap();
        assert_eq!(uid, 100);
        assert_eq!(
            fs::read_to_string(output.join("copy.txt")).unwrap(),
            format!("{copy} {uid}")
        );
        compare(&s, "clone");
        s.remove_family(copy, uid).unwrap();
        compare(&s, "delete");
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vitals {
    pub hp: f32,
    pub max_hp_ratio: f32,
    pub morale: f32,
    pub max_morale_ratio: f32,
}

impl Vitals {
    pub fn new(hp: f32, morale: f32) -> Self {
        Self {
            hp,
            max_hp_ratio: 1.0,
            morale,
            max_morale_ratio: 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorldObject {
    pub slot: usize,
    pub type_id: u16,
    pub team: u16,
    pub uid: u32,
    pub position: [f32; 3],
    pub rotation_degrees: f32,
    pub parent: Option<usize>,
    pub vitals: Option<Vitals>,
}
#[derive(Clone)]
pub struct ObjectStore {
    pools: BTreeMap<String, Blob>,
    count: usize,
    positions: usize,
    root: PathBuf,
    history: Vec<BTreeMap<String, Vec<u8>>>,
    redo: Vec<BTreeMap<String, Vec<u8>>>,
}
impl ObjectStore {
    pub fn load(map: impl AsRef<Path>) -> Result<Self> {
        let root = map.as_ref().join("DATA");
        let mut pools = BTreeMap::new();
        for name in [
            "objects", "objdata", "position", "anim", "gfxtype", "action", "engine", "light",
            "particle", "lager", "biglager",
        ] {
            let p = root.join(format!("{name}.dat"));
            if p.exists() || ["objects", "objdata", "position"].contains(&name) {
                pools.insert(name.into(), Blob::read(&p)?);
            }
        }
        Self::build(pools, root)
    }
    fn build(pools: BTreeMap<String, Blob>, root: PathBuf) -> Result<Self> {
        let b = &pools["objects"].bytes;
        if b.len() < 16 || u32at(b, 0) != 1 || u32at(b, 8) != 30 || u32at(b, 12) != 30 {
            return Err("objects header".into());
        }
        let count = u32at(b, 4) as usize;
        if count == 0
            || count > 14000
            || b.len() != 16 + count * (79 + COLUMNS.iter().sum::<usize>())
        {
            return Err("objects capacity/length".into());
        }
        let d = &pools["objdata"].bytes;
        if d.len() != 8 + count * SEGMENTS.iter().sum::<usize>()
            || u32at(d, 0) != 1
            || u32at(d, 4) as usize != count
        {
            return Err("objdata layout".into());
        }
        let p = &pools["position"].bytes;
        if p.len() < 8 || u32at(p, 0) != 1 {
            return Err("position header".into());
        }
        let positions = u32at(p, 4) as usize;
        if positions > 65535 || p.len() != 8 + positions * 17 {
            return Err("position layout".into());
        }
        for (name, h, stride) in [("anim", 8, 25), ("gfxtype", 8, 17), ("action", 12, 25)] {
            if let Some(b) = pools.get(name) {
                if b.bytes.len() != h + count * stride
                    || u32at(&b.bytes, 0) != 1
                    || u32at(&b.bytes, 4) as usize != count
                {
                    return Err(format!("{name} layout"));
                }
            }
        }
        if pools.get("action").is_some_and(|b| u32at(&b.bytes, 8) != 6) {
            return Err("action column count".into());
        }
        if pools
            .get("engine")
            .is_some_and(|b| b.bytes.len() != 94 || u32at(&b.bytes, 0) != 1)
        {
            return Err("engine layout".into());
        }
        for (name, len, count, width) in [("lager", 144016, 3200, 6), ("biglager", 51244, 32, 800)]
        {
            if let Some(b) = pools.get(name) {
                if b.bytes.len() != len
                    || u32at(&b.bytes, 0) != 1
                    || u32at(&b.bytes, 4) != count
                    || u32at(&b.bytes, 8) != width
                    || name == "lager" && u32at(&b.bytes, 12) != 10
                {
                    return Err(format!("{name} header/layout"));
                }
            }
        }
        if let Some(b) = pools.get("light") {
            LightPool::from_payload(&b.bytes).map_err(|e| e.to_string())?;
        }
        if let Some(b) = pools.get("particle") {
            ParticlePool::from_payload(&b.bytes).map_err(|e| e.to_string())?;
        }
        Ok(Self {
            pools,
            count,
            positions,
            root,
            history: vec![],
            redo: vec![],
        })
    }
    pub fn capacity(&self) -> usize {
        self.count
    }
    pub fn is_dirty(&self) -> bool {
        self.pools.values().any(|b| b.bytes != b.initial)
    }
    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn payload(&self, name: &str) -> Option<&[u8]> {
        self.pools.get(name).map(|b| b.bytes.as_slice())
    }
    fn record(&self, s: usize) -> usize {
        16 + s * 79
    }
    fn active(&self, s: usize) -> bool {
        s < self.count && self.pools["objects"].bytes[self.record(s)] != 0
    }
    fn link(&self, s: usize, o: usize) -> usize {
        u16at(&self.pools["objects"].bytes, self.record(s) + o) as usize
    }
    fn col(&self, s: usize, c: usize) -> usize {
        let o =
            16 + self.count * 79 + self.count * COLUMNS[..c].iter().sum::<usize>() + s * COLUMNS[c];
        let b = &self.pools["objects"].bytes;
        if COLUMNS[c] == 2 {
            u16at(b, o) as usize
        } else {
            u32at(b, o) as usize
        }
    }
    fn poslinks(&self, s: usize) -> BTreeSet<usize> {
        [self.link(s, 67), self.link(s, 69), self.col(s, 10)]
            .into_iter()
            .filter(|p| *p != 65535)
            .collect()
    }
    pub fn objects(&self) -> Vec<WorldObject> {
        (0..self.count)
            .filter(|s| self.active(*s))
            .map(|s| {
                let p = self.link(s, 67);
                let b = &self.pools["position"].bytes;
                let xyz = if p < self.positions {
                    std::array::from_fn(|a| f32at(b, 9 + p * 17 + a * 4))
                } else {
                    [f32::NAN; 3]
                };
                let r = self.record(s);
                let o = &self.pools["objects"].bytes;
                WorldObject {
                    slot: s,
                    type_id: u16at(o, r + 75),
                    team: u16at(o, r + 1),
                    uid: u32at(o, r + 3),
                    position: xyz,
                    rotation_degrees: if p < self.positions {
                        f32at(b, 21 + p * 17)
                    } else {
                        f32::NAN
                    },
                    parent: (self.col(s, 13) != 65535).then(|| self.col(s, 13)),
                    vitals: self.vitals(s),
                }
            })
            .collect()
    }
    /// Reads decoded vitals (HP, morale and their ratios) for an object slot, if present and active.
    pub fn vitals(&self, slot: usize) -> Option<Vitals> {
        if !self.active(slot) {
            return None;
        }
        let d = self.col(slot, 1);
        if d >= self.count {
            return None;
        }
        let objdata = self.pools.get("objdata")?;
        let base0 = 8;
        let target0 = base0 + d * 9;
        let base1 = 8 + self.count * 9;
        let target1 = base1 + d * 8;
        if target0 + 9 > objdata.bytes.len() || target1 + 8 > objdata.bytes.len() {
            return None;
        }
        if objdata.bytes[target0] == 0 {
            return None;
        }
        let hp = f32at(&objdata.bytes, target0 + 1);
        let max_hp_ratio = f32at(&objdata.bytes, target0 + 5);
        let morale = f32at(&objdata.bytes, target1);
        let max_morale_ratio = f32at(&objdata.bytes, target1 + 4);
        Some(Vitals {
            hp,
            max_hp_ratio,
            morale,
            max_morale_ratio,
        })
    }
    /// Parent-child closure, with strict external ownership and unsupported pool rejection.
    pub fn family(&self, slot: usize, uid: u32) -> Result<Vec<usize>> {
        if !self.active(slot) || u32at(&self.pools["objects"].bytes, self.record(slot) + 3) != uid {
            return Err("stale object UID".into());
        }
        if self.col(slot, 13) != 65535 {
            return Err("select family root".into());
        }
        let mut family = BTreeSet::from([slot]);
        loop {
            let before = family.len();
            for s in 0..self.count {
                if self.active(s) && family.contains(&self.col(s, 13)) {
                    family.insert(s);
                }
            }
            if family.len() == before {
                break;
            }
        }
        let mut positions = BTreeSet::new();
        let mut data = BTreeSet::new();
        for &s in &family {
            for c in [2, 3, 4, 5, 8, 11] {
                if self.col(s, c) != 65535 {
                    return Err(format!("unsupported related pool column {c}, slot {s}"));
                }
            }
            if self.col(s, 1) >= self.count {
                return Err("objdata link out of range".into());
            }
            data.insert(self.col(s, 1));
            let ps = self.poslinks(s);
            if self.link(s, 67) == 65535 || self.link(s, 69) == 65535 {
                return Err("missing primary positions".into());
            }
            for p in ps {
                if p >= self.positions || self.pools["position"].bytes[8 + p * 17] == 0 {
                    return Err("inactive position".into());
                }
                positions.insert(p);
            }
            for (name, o, h, stride) in [
                ("anim", 71, 8, 21),
                ("gfxtype", 73, 8, 15),
                ("action", 77, 12, 25),
            ] {
                let p = self.link(s, o);
                if p != 65535 {
                    let b = self.pools.get(name).ok_or(format!("missing {name}"))?;
                    if p >= self.count || b.bytes[h + p * stride] == 0 {
                        return Err(format!("invalid {name} link"));
                    }
                }
            }
        }
        for s in 0..self.count {
            if self.active(s)
                && !family.contains(&s)
                && (self.poslinks(s).iter().any(|p| positions.contains(p))
                    || data.contains(&self.col(s, 1))
                    || [71, 73, 77].into_iter().any(|o| {
                        self.link(s, o) != 65535
                            && family.iter().any(|m| self.link(*m, o) == self.link(s, o))
                    }))
            {
                return Err("external shared position/objdata/auxiliary".into());
            }
        }
        // Garrison membership is a reverse link, including unreferenced active camps.
        for (name, count, h, stride, start, n) in [
            ("lager", 3200, 16, 43, 23, 10),
            ("biglager", 32, 12, 1601, 1, 800),
        ] {
            if let Some(b) = self.pools.get(name) {
                let expected = if name == "lager" { 144016 } else { 51244 };
                if b.bytes.len() != expected || u32at(&b.bytes, 0) != 1 {
                    return Err(format!("invalid {name}"));
                }
                for s in 0..count {
                    let o = h + s * stride;
                    if b.bytes[o] != 0
                        && (0..n).any(|i| {
                            family.contains(&(u16at(&b.bytes, o + start + i * 2) as usize))
                        })
                    {
                        return Err("garrison family not yet supported".into());
                    }
                }
            }
        }
        if let Some(b) = self.pools.get("particle") {
            let p = ParticlePool::from_payload(&b.bytes).map_err(|e| e.to_string())?;
            for s in p.active_slots() {
                if family.contains(&(p.capture(s).unwrap().owner() as usize)) {
                    return Err("particle family not yet supported".into());
                }
            }
        }
        if family.len() > 1 && !self.pools.contains_key("light") {
            return Err("missing family light pool".into());
        }
        if let Some(b) = self.pools.get("light") {
            let p = LightPool::from_payload(&b.bytes).map_err(|e| e.to_string())?;
            let family_lights: BTreeSet<_> = p
                .active_snapshots()
                .filter(|l| {
                    family.contains(&(l.owner() as usize))
                        || family.contains(&(l.parent() as usize))
                })
                .map(|l| l.slot() as usize)
                .collect();
            for s in 0..self.count {
                if self.active(s)
                    && !family.contains(&s)
                    && family_lights.contains(&self.col(s, 12))
                {
                    return Err("external direct light reference".into());
                }
            }
            if let Some(bytes) = self.pools.get("particle") {
                let particles =
                    ParticlePool::from_payload(&bytes.bytes).map_err(|e| e.to_string())?;
                for slot in particles.active_slots() {
                    let emitter = particles.capture(slot).unwrap();
                    for i in 0..64 {
                        let child = emitter.child(i).map_err(|e| e.to_string())?;
                        if child.active_raw() != 0
                            && family_lights.contains(&(child.light_link() as usize))
                        {
                            return Err("external particle light reference".into());
                        }
                    }
                }
            }
            for l in p.active_snapshots() {
                let own = family.contains(&(l.owner() as usize));
                let par = family.contains(&(l.parent() as usize));
                if own || par {
                    if own && l.parent() != -1
                        || par && l.owner() != -1
                        || par && !(0..256).contains(&l.definition())
                    {
                        return Err("mixed or invalid attached light ownership".into());
                    }
                    if l.owner() >= 0 && !own || l.parent() >= 0 && !par {
                        return Err("external light ownership".into());
                    }
                    if l.owner() >= 0 && self.col(l.owner() as usize, 12) != l.slot() as usize {
                        return Err("light backlink mismatch".into());
                    }
                }
            }
            for &s in &family {
                let l = self.col(s, 12);
                if l != 65535 {
                    let snap = p.capture(l as u32).ok_or("missing direct light")?;
                    if snap.owner() != s as i16 || snap.parent() != -1 {
                        return Err("direct light owner mismatch".into());
                    }
                }
            }
        } else if family.iter().any(|s| self.col(*s, 12) != 65535) {
            return Err("missing direct light pool".into());
        }
        Ok(family.into_iter().collect())
    }
    fn snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        self.pools
            .iter()
            .map(|(n, b)| (n.clone(), b.bytes.clone()))
            .collect()
    }
    fn column_offset(&self, s: usize, c: usize) -> usize {
        16 + self.count * 79 + self.count * COLUMNS[..c].iter().sum::<usize>() + s * COLUMNS[c]
    }
    fn family_lights(&self, family: &[usize]) -> Result<Vec<usize>> {
        if let Some(b) = self.pools.get("light") {
            let p = LightPool::from_payload(&b.bytes).map_err(|e| e.to_string())?;
            Ok(p.active_snapshots()
                .filter(|l| {
                    family.contains(&(l.owner() as usize))
                        || family.contains(&(l.parent() as usize))
                })
                .map(|l| l.slot() as usize)
                .collect())
        } else {
            Ok(vec![])
        }
    }
    /// Copies saved parent/light families into independently allocated native pools.
    /// Returns the new root slot and UID. Definitions/native creation are deliberately not inferred.
    pub fn clone_family_fixed(
        &mut self,
        slot: usize,
        uid: u32,
        delta: [i32; 3],
    ) -> Result<(usize, u32)> {
        let family = self.family(slot, uid)?;
        let mut reserved: BTreeSet<_> = (0..self.count)
            .filter(|s| self.active(*s))
            .map(|s| self.col(s, 13))
            .collect();
        let lights = if let Some(b) = self.pools.get("light") {
            Some(LightPool::from_payload(&b.bytes).map_err(|e| e.to_string())?)
        } else {
            None
        };
        if let Some(p) = &lights {
            for l in p.active_snapshots() {
                reserved.insert(l.owner() as usize);
                reserved.insert(l.parent() as usize);
            }
        }
        if let Some(b) = self.pools.get("particle") {
            let p = ParticlePool::from_payload(&b.bytes).map_err(|e| e.to_string())?;
            for s in p.active_slots() {
                reserved.insert(p.capture(s).unwrap().owner() as usize);
            }
        }
        for (name, count, h, stride, start, n) in [
            ("lager", 3200, 16, 43, 23, 10),
            ("biglager", 32, 12, 1601, 1, 800),
        ] {
            if let Some(b) = self.pools.get(name) {
                for s in 0..count {
                    let o = h + s * stride;
                    if b.bytes[o] != 0 {
                        for i in 0..n {
                            reserved.insert(u16at(&b.bytes, o + start + i * 2) as usize);
                        }
                    }
                }
            }
        }
        fn allocation(
            source: BTreeSet<usize>,
            free: impl Iterator<Item = usize>,
        ) -> Result<BTreeMap<usize, usize>> {
            let targets: Vec<_> = free.take(source.len()).collect();
            if targets.len() != source.len() {
                return Err("insufficient free native pool slots".into());
            }
            Ok(source.into_iter().zip(targets).collect())
        }
        let objects = allocation(
            family.iter().copied().collect(),
            (0..self.count).filter(|s| !self.active(*s) && !reserved.contains(s)),
        )?;
        let all_active: Vec<_> = (0..self.count).filter(|s| self.active(*s)).collect();
        let used_positions: BTreeSet<_> =
            all_active.iter().flat_map(|s| self.poslinks(*s)).collect();
        let position = allocation(
            family.iter().flat_map(|s| self.poslinks(*s)).collect(),
            (0..self.positions).filter(|s| {
                self.pools["position"].bytes[8 + s * 17] == 0 && !used_positions.contains(s)
            }),
        )?;
        let used_data: BTreeSet<_> = all_active.iter().map(|s| self.col(*s, 1)).collect();
        let data = allocation(
            family.iter().map(|s| self.col(*s, 1)).collect(),
            (0..self.count)
                .filter(|s| self.pools["objdata"].bytes[8 + s * 9] == 0 && !used_data.contains(s)),
        )?;
        let mut aux = BTreeMap::new();
        for (name, o, h, stride) in [
            ("anim", 71, 8, 21),
            ("gfxtype", 73, 8, 15),
            ("action", 77, 12, 25),
        ] {
            let source = family
                .iter()
                .map(|s| self.link(*s, o))
                .filter(|s| *s != 65535)
                .collect::<BTreeSet<_>>();
            if !source.is_empty() {
                let b = self.pools.get(name).ok_or(format!("missing {name}"))?;
                let used: BTreeSet<_> = all_active.iter().map(|s| self.link(*s, o)).collect();
                aux.insert(
                    name,
                    allocation(
                        source,
                        (0..self.count)
                            .filter(|s| b.bytes[h + s * stride] == 0 && !used.contains(s)),
                    )?,
                );
            }
        }
        let light_sources: BTreeSet<_> = self.family_lights(&family)?.into_iter().collect();
        let mut used_lights: BTreeSet<_> = all_active.iter().map(|s| self.col(*s, 12)).collect();
        if let Some(b) = self.pools.get("particle") {
            let p = ParticlePool::from_payload(&b.bytes).map_err(|e| e.to_string())?;
            for s in p.active_slots() {
                for ordinal in 0..64 {
                    let light = p
                        .capture(s)
                        .unwrap()
                        .child(ordinal)
                        .map_err(|e| e.to_string())?;
                    if light.active_raw() != 0 {
                        used_lights.insert(light.light_link() as usize);
                    }
                }
            }
        }
        let light = allocation(
            light_sources,
            (0..1024).filter(|s| {
                lights
                    .as_ref()
                    .is_none_or(|p| p.capture(*s as u32).is_none())
                    && !used_lights.contains(s)
            }),
        )?;
        let active_max = all_active
            .iter()
            .map(|s| u32at(&self.pools["objects"].bytes, self.record(*s) + 3) & 0xFFFFFF)
            .max()
            .unwrap_or(0);
        let persisted = self
            .pools
            .get("engine")
            .filter(|b| b.bytes.len() >= 38)
            .map(|b| u32at(&b.bytes, 34))
            .unwrap_or(1);
        let next = (active_max + 1).max(persisted);
        if next == 0 || next as u64 + family.len() as u64 > 0x1000000 {
            return Err("24-bit UID exhausted".into());
        }
        let mut candidate = self.clone();
        let snapshot = self.snapshot();
        fn remap(v: usize, map: &BTreeMap<usize, usize>) -> Result<usize> {
            if v == 65535 {
                Ok(v)
            } else {
                map.get(&v).copied().ok_or("missing family mapping".into())
            }
        }
        for (&src, &dst) in &position {
            candidate.pools.get_mut("position").unwrap().bytes[8 + dst * 17..25 + dst * 17]
                .copy_from_slice(&snapshot["position"][8 + src * 17..25 + src * 17]);
        }
        for (&src, &dst) in &data {
            for (seg, w) in SEGMENTS.iter().enumerate() {
                let base = 8 + self.count * SEGMENTS[..seg].iter().sum::<usize>();
                candidate.pools.get_mut("objdata").unwrap().bytes
                    [base + dst * w..base + (dst + 1) * w]
                    .copy_from_slice(&snapshot["objdata"][base + src * w..base + (src + 1) * w]);
            }
        }
        for (name, h, stride, tails) in [
            ("anim", 8, 21, 2),
            ("gfxtype", 8, 15, 1),
            ("action", 12, 25, 0),
        ] {
            if let Some(map) = aux.get(name) {
                for (&src, &dst) in map {
                    let b = &mut candidate.pools.get_mut(name).unwrap().bytes;
                    b[h + dst * stride..h + (dst + 1) * stride]
                        .copy_from_slice(&snapshot[name][h + src * stride..h + (src + 1) * stride]);
                    for t in 0..tails {
                        let base = h + self.count * stride + t * self.count * 2;
                        b[base + dst * 2..base + dst * 2 + 2]
                            .copy_from_slice(&snapshot[name][base + src * 2..base + src * 2 + 2]);
                    }
                }
            }
        }
        for (index, (&src, &dst)) in objects.iter().enumerate() {
            let r = self.record(dst);
            let from = self.record(src);
            let b = &mut candidate.pools.get_mut("objects").unwrap().bytes;
            b[r..r + 79].copy_from_slice(&snapshot["objects"][from..from + 79]);
            b[r + 3..r + 7].copy_from_slice(&(next + index as u32).to_le_bytes());
            for (o, map) in [(67, &position), (69, &position)] {
                let v = remap(self.link(src, o), map)?;
                candidate.pools.get_mut("objects").unwrap().bytes[r + o..r + o + 2]
                    .copy_from_slice(&(v as u16).to_le_bytes());
            }
            for (name, o) in [("anim", 71), ("gfxtype", 73), ("action", 77)] {
                let v = self.link(src, o);
                if v != 65535 {
                    let v = remap(v, &aux[name])?;
                    candidate.pools.get_mut("objects").unwrap().bytes[r + o..r + o + 2]
                        .copy_from_slice(&(v as u16).to_le_bytes());
                }
            }
            for c in 0..18 {
                let v = self.col(src, c);
                let value = match c {
                    1 => remap(v, &data)?,
                    10 => remap(v, &position)?,
                    12 => remap(v, &light)?,
                    13 => remap(v, &objects)?,
                    _ => v,
                } as u32;
                let o = self.column_offset(dst, c);
                candidate.pools.get_mut("objects").unwrap().bytes[o..o + COLUMNS[c]]
                    .copy_from_slice(&value.to_le_bytes()[..COLUMNS[c]]);
            }
        }
        for (&src, &dst) in &light {
            let mut base = 8;
            for (c, w) in against_rome_data::pool::WIDTHS.iter().enumerate() {
                let o = base + dst * w;
                let source = base + src * w;
                let b = &mut candidate.pools.get_mut("light").unwrap().bytes;
                b[o..o + w].copy_from_slice(&snapshot["light"][source..source + w]);
                if c == 13 || c == 14 {
                    let v = remap(u16at(&snapshot["light"], source) as usize, &objects)?;
                    b[o..o + 2].copy_from_slice(&(v as u16).to_le_bytes());
                }
                base += 1024 * w;
            }
        }
        if let Some(b) = candidate.pools.get_mut("engine") {
            if b.bytes.len() < 38 {
                return Err("engine missing UID counter".into());
            }
            b.bytes[34..38].copy_from_slice(&(next + family.len() as u32).to_le_bytes());
        }
        let root = objects[&slot];
        let new_uid = u32at(
            &candidate.pools["objects"].bytes,
            candidate.record(root) + 3,
        );
        candidate.translate_family_fixed(root, new_uid, delta)?;
        self.history.push(self.snapshot());
        self.redo.clear();
        self.restore(candidate.snapshot());
        Ok((root, new_uid))
    }
    /// Removes a fully validated supported closure. Reset templates mirror LevelObjectStore.RemoveCore.
    pub fn remove_family(&mut self, slot: usize, uid: u32) -> Result<()> {
        let family = self.family(slot, uid)?;
        let mut candidate = self.snapshot();
        let free = (0..self.count).find(|s| !self.active(*s));
        let empty_record = free
            .map(|s| self.pools["objects"].bytes[self.record(s)..self.record(s) + 79].to_vec())
            .unwrap_or_else(|| {
                let mut b = vec![0; 79];
                for o in [67, 69, 71, 73, 75, 77] {
                    b[o..o + 2].fill(255)
                }
                b
            });
        let empty_cols: Vec<_> = (0..18)
            .map(|c| {
                free.map(|s| self.col(s, c) as u32)
                    .unwrap_or(if COLUMNS[c] == 2 { 65535 } else { 0 })
            })
            .collect();
        let p = &self.pools["position"].bytes;
        let empty_position = (0..self.positions)
            .find(|s| p[8 + s * 17] == 0)
            .map(|s| p[8 + s * 17..25 + s * 17].to_vec())
            .unwrap_or(vec![0; 17]);
        let data = &self.pools["objdata"].bytes;
        let empty_data = (0..self.count).find(|s| data[8 + s * 9] == 0);
        for &s in &family {
            let r = self.record(s);
            candidate.get_mut("objects").unwrap()[r..r + 79].copy_from_slice(&empty_record);
            for (c, v) in empty_cols.iter().enumerate() {
                let o = self.column_offset(s, c);
                let v = if [5, 11, 12, 13].contains(&c) {
                    65535
                } else {
                    *v
                };
                candidate.get_mut("objects").unwrap()[o..o + COLUMNS[c]]
                    .copy_from_slice(&v.to_le_bytes()[..COLUMNS[c]]);
            }
            for p in self.poslinks(s) {
                candidate.get_mut("position").unwrap()[8 + p * 17..25 + p * 17]
                    .copy_from_slice(&empty_position);
            }
            let d = self.col(s, 1);
            for (seg, w) in SEGMENTS.iter().enumerate() {
                let base = 8 + self.count * SEGMENTS[..seg].iter().sum::<usize>();
                let target = base + d * w;
                let blank = empty_data
                    .map(|i| data[base + i * w..base + (i + 1) * w].to_vec())
                    .unwrap_or(vec![0; *w]);
                candidate.get_mut("objdata").unwrap()[target..target + w].copy_from_slice(&blank);
            }
            for (name, o, h, stride, tails) in [
                ("anim", 71, 8, 21, 2),
                ("gfxtype", 73, 8, 15, 1),
                ("action", 77, 12, 25, 0),
            ] {
                let source = self.link(s, o);
                if source == 65535 {
                    continue;
                }
                let b = &self.pools[name].bytes;
                let empty = (0..self.count).find(|i| b[h + i * stride] == 0);
                let blank = empty
                    .map(|i| b[h + i * stride..h + (i + 1) * stride].to_vec())
                    .unwrap_or(vec![0; stride]);
                let target = candidate.get_mut(name).unwrap();
                target[h + source * stride..h + (source + 1) * stride].copy_from_slice(&blank);
                for t in 0..tails {
                    let base = h + self.count * stride + t * self.count * 2;
                    let val = empty
                        .map(|i| u16at(b, base + i * 2))
                        .unwrap_or(if name == "anim" && t == 1 { 1 } else { 0 });
                    target[base + source * 2..base + source * 2 + 2]
                        .copy_from_slice(&val.to_le_bytes());
                }
            }
        }
        for s in self.family_lights(&family)? {
            let b = candidate.get_mut("light").unwrap();
            let mut base = 8;
            for (c, w) in against_rome_data::pool::WIDTHS.iter().enumerate() {
                let o = base + s * w;
                b[o..o + w].fill(if c >= 13 { 255 } else { 0 });
                base += 1024 * w;
            }
        }
        self.history.push(self.snapshot());
        self.redo.clear();
        self.restore(candidate);
        Ok(())
    }
    fn restore(&mut self, s: BTreeMap<String, Vec<u8>>) {
        for (n, b) in s {
            self.pools.get_mut(&n).unwrap().bytes = b
        }
    }
    pub fn undo(&mut self) -> bool {
        if let Some(s) = self.history.pop() {
            let now = self.snapshot();
            self.restore(s);
            self.redo.push(now);
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self) -> bool {
        if let Some(s) = self.redo.pop() {
            let now = self.snapshot();
            self.restore(s);
            self.history.push(now);
            true
        } else {
            false
        }
    }
    /// Exact signed 16.16 displacement, matching the managed family translation contract.
    pub fn translate_family_fixed(&mut self, slot: usize, uid: u32, delta: [i32; 3]) -> Result<()> {
        let family = self.family(slot, uid)?;
        let mut candidate = self.snapshot();
        let positions: BTreeSet<_> = family.iter().flat_map(|s| self.poslinks(*s)).collect();
        let translate = |bytes: &mut [u8], offset: usize, axis: usize| -> Result<()> {
            let old = f32at(bytes, offset);
            let new = (old as f64 + delta[axis] as f64 / 65536.0) as f32;
            if !old.is_finite() || !new.is_finite() {
                return Err("nonfinite coordinate".into());
            }
            bytes[offset..offset + 4].copy_from_slice(&new.to_le_bytes());
            Ok(())
        };
        for p in positions {
            for a in 0..3 {
                translate(
                    candidate.get_mut("position").unwrap(),
                    9 + p * 17 + a * 4,
                    a,
                )?;
            }
        }
        if let Some(b) = self.pools.get("light") {
            let p = LightPool::from_payload(&b.bytes).map_err(|e| e.to_string())?;
            for l in p.active_snapshots() {
                if family.contains(&(l.owner() as usize)) || family.contains(&(l.parent() as usize))
                {
                    for a in 0..3 {
                        translate(
                            candidate.get_mut("light").unwrap(),
                            8 + 1024 * 4 * (1 + a) + l.slot() as usize * 4,
                            a,
                        )?;
                    }
                }
            }
        }
        self.history.push(self.snapshot());
        self.redo.clear();
        self.restore(candidate);
        Ok(())
    }
    /// Sets rotation in degrees for an object family.
    pub fn set_family_rotation(&mut self, slot: usize, uid: u32, degrees: f32) -> Result<()> {
        if !degrees.is_finite() {
            return Err("nonfinite rotation".into());
        }
        let family = self.family(slot, uid)?;
        let mut candidate = self.snapshot();
        let normalized = ((degrees % 360.0) + 360.0) % 360.0;
        let positions: BTreeSet<_> = family.iter().flat_map(|s| self.poslinks(*s)).collect();
        for p in positions {
            candidate.get_mut("position").unwrap()[21 + p * 17..21 + p * 17 + 4]
                .copy_from_slice(&normalized.to_le_bytes());
        }
        self.history.push(self.snapshot());
        self.redo.clear();
        self.restore(candidate);
        Ok(())
    }
    /// Rotates an object family by delta degrees relative to current orientation.
    pub fn rotate_family(&mut self, slot: usize, uid: u32, delta_degrees: f32) -> Result<()> {
        if !delta_degrees.is_finite() {
            return Err("nonfinite rotation".into());
        }
        let family = self.family(slot, uid)?;
        let mut candidate = self.snapshot();
        let positions: BTreeSet<_> = family.iter().flat_map(|s| self.poslinks(*s)).collect();
        for p in positions {
            let current = f32at(&self.pools["position"].bytes, 21 + p * 17);
            if !current.is_finite() {
                return Err("nonfinite coordinate".into());
            }
            let normalized = (((current + delta_degrees) % 360.0) + 360.0) % 360.0;
            candidate.get_mut("position").unwrap()[21 + p * 17..21 + p * 17 + 4]
                .copy_from_slice(&normalized.to_le_bytes());
        }
        self.history.push(self.snapshot());
        self.redo.clear();
        self.restore(candidate);
        Ok(())
    }
    pub fn set_rotation(&mut self, slot: usize, uid: u32, degrees: f32) -> Result<()> {
        self.set_family_rotation(slot, uid, degrees)
    }
    /// Sets team ownership (0..15 or 65535 for unassigned/-1) for an object family.
    pub fn set_family_team(&mut self, slot: usize, uid: u32, team: u16) -> Result<()> {
        if team > 15 && team != 65535 {
            return Err("invalid team id (must be 0..15 or 65535 for none)".into());
        }
        let family = self.family(slot, uid)?;
        let mut candidate = self.snapshot();
        for &s in &family {
            let r = self.record(s);
            candidate.get_mut("objects").unwrap()[r + 1..r + 3]
                .copy_from_slice(&team.to_le_bytes());
        }
        self.history.push(self.snapshot());
        self.redo.clear();
        self.restore(candidate);
        Ok(())
    }
    pub fn set_team(&mut self, slot: usize, uid: u32, team: u16) -> Result<()> {
        self.set_family_team(slot, uid, team)
    }
    /// Sets vitals (HP, morale and their ratios) for an object family.
    pub fn set_family_vitals(&mut self, slot: usize, uid: u32, vitals: Vitals) -> Result<()> {
        if !vitals.hp.is_finite()
            || !vitals.max_hp_ratio.is_finite()
            || !vitals.morale.is_finite()
            || !vitals.max_morale_ratio.is_finite()
        {
            return Err("nonfinite vitals".into());
        }
        if vitals.max_hp_ratio < 0.0 || vitals.max_morale_ratio < 0.0 {
            return Err("negative vital ratio".into());
        }
        let family = self.family(slot, uid)?;
        let mut candidate = self.snapshot();
        for &s in &family {
            let d = self.col(s, 1);
            if d >= self.count {
                return Err("objdata link out of range".into());
            }
            let base0 = 8;
            let target0 = base0 + d * 9;
            let base1 = 8 + self.count * 9;
            let target1 = base1 + d * 8;
            let objdata = candidate.get_mut("objdata").unwrap();
            objdata[target0] = 1;
            objdata[target0 + 1..target0 + 5].copy_from_slice(&vitals.hp.to_le_bytes());
            objdata[target0 + 5..target0 + 9].copy_from_slice(&vitals.max_hp_ratio.to_le_bytes());
            objdata[target1..target1 + 4].copy_from_slice(&vitals.morale.to_le_bytes());
            objdata[target1 + 4..target1 + 8].copy_from_slice(&vitals.max_morale_ratio.to_le_bytes());
        }
        self.history.push(self.snapshot());
        self.redo.clear();
        self.restore(candidate);
        Ok(())
    }
    pub fn set_vitals(&mut self, slot: usize, uid: u32, vitals: Vitals) -> Result<()> {
        self.set_family_vitals(slot, uid, vitals)
    }
    pub fn set_hp_morale(&mut self, slot: usize, uid: u32, hp: f32, morale: f32) -> Result<()> {
        self.set_family_vitals(slot, uid, Vitals::new(hp, morale))
    }
    /// Stages and syncs candidates, retains exclusive backups, and rolls back ordinary errors.
    /// A DATA lock serializes cooperating editors. Not crash atomic across files.
    pub fn save(&mut self) -> Result<()> {
        self.save_using(|_, from, to| fs::rename(from, to))
    }
    fn save_using(
        &mut self,
        publish: impl FnMut(usize, &Path, &Path) -> std::io::Result<()>,
    ) -> Result<()> {
        self.save_using_io(publish, write_stage)
    }
    fn save_using_io(
        &mut self,
        mut publish: impl FnMut(usize, &Path, &Path) -> std::io::Result<()>,
        mut stage_file: impl FnMut(&Path, &[u8], fs::Permissions) -> std::io::Result<()>,
    ) -> Result<()> {
        use std::fs::OpenOptions;
        let canonical = fs::canonicalize(&self.root).map_err(|e| e.to_string())?;
        for var in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(p) = std::env::var_os(var) {
                if let Ok(p) = fs::canonicalize(p) {
                    if canonical.starts_with(p) {
                        return Err(
                            "save to installed game is forbidden; copy map to a working directory"
                                .into(),
                        );
                    }
                }
            }
        }
        struct Lock(PathBuf, Option<std::fs::File>);
        impl Drop for Lock {
            fn drop(&mut self) {
                if let Some(file) = self.1.take() {
                    let _ = file.sync_all();
                    drop(file);
                }
                let _ = fs::remove_file(&self.0);
            }
        }
        let lockpath = self.root.join(".ar-placement.lock");
        let lockfile = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lockpath)
            .map_err(|e| format!("save lock {}: {e}", lockpath.display()))?;
        let _lock = Lock(lockpath, Some(lockfile));
        let mut changes = vec![];
        for (n, b) in &self.pools {
            let path = self.root.join(format!("{n}.dat"));
            if fs::read(&path).map_err(|e| e.to_string())? != b.raw {
                return Err(format!("external file change: {}", path.display()));
            }
            if b.bytes != b.initial {
                let path = self.root.join(format!("{n}.dat"));
                let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
                if !metadata.is_file() || metadata.file_type().is_symlink() {
                    return Err(format!("not a regular pool file: {}", path.display()));
                }
                if metadata.permissions().readonly() {
                    return Err(format!("read-only pool file: {}", path.display()));
                }
                if fs::read(&path).map_err(|e| e.to_string())? != b.raw {
                    return Err(format!("external file change: {}", path.display()));
                }
                changes.push((n.clone(), path, b.encoded(), b.raw.clone(), metadata.permissions()));
            }
        }
        if changes.is_empty() {
            return Ok(());
        }
        static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let stage = self.root.join(format!(
            ".ar-placement-stage-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&stage).map_err(|e| format!("create stage {}: {e}", stage.display()))?;
        for (n, _, bytes, _, permissions) in &changes {
            let p = stage.join(format!("{n}.new"));
            stage_file(&p, bytes, permissions.clone()).map_err(|e| {
                format!("stage {}: {e}; recovery files retained at {}", p.display(), stage.display())
            })?;
        }
        for (i, (n, p, _, old, _)) in changes.iter().enumerate() {
            let backup = stage.join(format!("{n}.old"));
            let result = (|| -> std::io::Result<()> {
                if fs::read(p)? != *old {
                    return Err(std::io::Error::other("external file changed during save"));
                }
                fs::rename(p, &backup)?;
                publish(i, &stage.join(format!("{n}.new")), p)
            })();
            if let Err(e) = result {
                let mut errors = vec![format!("publish {}: {e}", p.display())];
                for (name, target, candidate, original, _) in changes[..=i].iter().rev() {
                    let backup = stage.join(format!("{name}.old"));
                    let backup_present = match backup.try_exists() {
                        Ok(present) => present,
                        Err(e) => {
                            errors.push(format!("rollback inspect {}: {e}", backup.display()));
                            continue;
                        }
                    };
                    if backup_present {
                        match fs::read(&backup) {
                            Ok(bytes) if bytes == *original => {}
                            result => {
                                errors.push(format!("rollback backup is unreadable or changed: {} ({:?}); original target and recovery preserved", backup.display(), result.err()));
                                continue;
                            }
                        }
                        let target_present = match target.try_exists() {
                            Ok(present) => present,
                            Err(e) => {
                                errors.push(format!("rollback inspect {}: {e}", target.display()));
                                continue;
                            }
                        };
                        if target_present {
                            // Do not remove a file created or edited by an unrelated writer.
                            // The DATA lock excludes cooperating writers only.
                            match fs::read(target) {
                                Ok(bytes) if bytes == *candidate => {}
                                _ => {
                                    errors.push(format!("rollback unexpected target preserved: {}; original backup: {}", target.display(), backup.display()));
                                    continue;
                                }
                            }
                            if let Err(e) = fs::remove_file(target) {
                                errors.push(format!("rollback remove {}: {e}", target.display()));
                                continue;
                            }
                        }
                        if let Err(e) = fs::rename(&backup, target) {
                            errors.push(format!("rollback restore {}: {e}", target.display()));
                        }
                    }
                }
                // Preserve every remaining candidate/backup on any failure, including
                // staging/backup failures. Recovery evidence is never cleanup fodder.
                errors.push(format!("recovery files retained at {}", stage.display()));
                return Err(errors.join("; "));
            }
        }
        // Successful backups remain as recovery evidence; each save owns an exclusive stage.
        for (n, _, raw, _, _) in changes {
            let b = self.pools.get_mut(&n).unwrap();
            b.raw = raw;
            b.initial = b.bytes.clone();
        }
        Ok(())
    }
}
