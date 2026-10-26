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
pub const LAGER_CAPACITY: usize = 3200;
pub const LAGER_RECORD_SIZE: usize = 43;
pub const LAGER_PAYLOAD_LEN: usize = 144016;
pub const BIG_LAGER_CAPACITY: usize = 32;
pub const BIG_LAGER_MEMBERS: usize = 800;
pub const BIG_LAGER_RECORD_SIZE: usize = 1601;
pub const BIG_LAGER_PAYLOAD_LEN: usize = 51244;
pub const PARTICLE_CAPACITY: usize = 1024;
pub const PARTICLE_CHILDREN: usize = 64;
pub const PARTICLE_RECORD_SIZE: usize = 2252;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FamilyDetails {
    pub object_slots: Vec<usize>,
    pub light_slots: Vec<usize>,
    pub particle_slots: Vec<usize>,
    pub lager_slots: Vec<usize>,
    pub big_lager_slots: Vec<usize>,
}

fn u16at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}
fn i16at(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}
fn u32at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn i32at(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
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

    // Expected values frozen from the original C# oracle (tests/fixtures/placement_oracle_hashes.tsv):
    // columns case, stage, pool, decoded length, sha256; `text` rows carry oracle side-files.
    const FROZEN_HASHES: &str = include_str!("../tests/fixtures/placement_oracle_hashes.tsv");
    fn frozen_text(case: &str, name: &str) -> String {
        FROZEN_HASHES
            .lines()
            .map(|l| l.split('\t').collect::<Vec<_>>())
            .find(|c| c[0] == case && c[1] == "text" && c[2] == name)
            .unwrap_or_else(|| panic!("missing frozen text {case}/{name}"))[4]
            .replace('|', "\n")
    }
    fn assert_pools_match_frozen(s: &ObjectStore, case: &str, stage: &str) {
        for (n, b) in &s.pools {
            let row = FROZEN_HASHES
                .lines()
                .map(|l| l.split('\t').collect::<Vec<_>>())
                .find(|c| c[0] == case && c[1] == stage && c[2] == n.as_str())
                .unwrap_or_else(|| panic!("missing frozen {case}/{stage}/{n}"));
            assert_eq!(row[3].parse::<usize>().unwrap(), b.bytes.len(), "{case}/{stage}/{n} length");
            assert_eq!(row[4], against_rome_data::sha256::sha256_hex(&b.bytes), "{case}/{stage}/{n} hash");
        }
    }
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
        store.root = super::temp_fixture::dir(&format!("ar-placement-{label}"));
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
    fn clone_keeps_inactive_unmanaged_light_reference_and_payload_reserved() {
        let mut s = fixture_count(4);
        for i in 2..4 {
            let r = s.record(i);
            s.pools.get_mut("objects").unwrap().bytes[r] = 0;
            s.pools.get_mut("position").unwrap().bytes[8 + i * 17] = 0;
            s.pools.get_mut("objdata").unwrap().bytes[8 + i * 9] = 0;
        }

        add_light(&mut s);
        let widths = against_rome_data::pool::WIDTHS;
        let active_offset = 8 + 2 * widths[0];
        let owner_offset = 8 + 1024 * widths[..13].iter().sum::<usize>() + 2 * widths[13];
        {
            let light = s.pools.get_mut("light").unwrap();
            light.bytes[active_offset..active_offset + widths[0]].fill(0x5A);
            light.bytes[owner_offset..owner_offset + 2].copy_from_slice(&2u16.to_le_bytes());
            light.raw = light.encoded();
            light.initial = light.bytes.clone();
        }

        let object_light = s.column_offset(2, 12);
        s.pools.get_mut("objects").unwrap().bytes[object_light..object_light + 2]
            .copy_from_slice(&2u16.to_le_bytes());
        let unmanaged_before = s.pools["light"].bytes
            [active_offset..active_offset + widths[0]]
            .to_vec();
        let before = s.snapshot();

        let (copy, uid) = s.clone_family_fixed(0, 1, [65536, 0, 0]).unwrap();

        assert_eq!(s.col(copy, 12), 1, "managed copy must allocate around the inactive orphan");
        assert_eq!(s.col(2, 12), 2, "unmanaged reference must remain intact");
        assert_eq!(
            &s.pools["light"].bytes[active_offset..active_offset + widths[0]],
            unmanaged_before.as_slice(),
            "inactive orphan payload must not be reused or rewritten"
        );
        assert!(s.family_details(0, 1).is_ok());
        assert!(s.family_details(copy, uid).is_ok());
        assert!(s.undo());
        assert_eq!(s.snapshot(), before, "one undo restores every pool byte");
    }
    #[test]
    fn clone_does_not_reallocate_inactive_position_still_referenced_by_native_object() {
        let mut s = fixture_count(4);
        for i in [1usize, 3] {
            let r = s.record(i);
            s.pools.get_mut("objects").unwrap().bytes[r] = 0;
            s.pools.get_mut("position").unwrap().bytes[8 + i * 17] = 0;
            s.pools.get_mut("objdata").unwrap().bytes[8 + i * 9] = 0;
        }
        let unmanaged_position_link = s.record(2) + 67;
        s.pools.get_mut("objects").unwrap().bytes
            [unmanaged_position_link..unmanaged_position_link + 2]
            .copy_from_slice(&1u16.to_le_bytes());
        let unmanaged_position = 8 + 1 * 17;
        s.pools.get_mut("position").unwrap().bytes
            [unmanaged_position..unmanaged_position + 17]
            .copy_from_slice(&[
                 0, 0xA5, 0x5A, 0xC3, 0x3C, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99,
                0xAB, 0xCD, 0xEF,
            ]);
        let position_before = s.pools["position"].bytes
            [unmanaged_position..unmanaged_position + 17]
            .to_vec();
        let before = s.snapshot();

        let (copy, uid) = s.clone_family_fixed(0, 1, [0; 3]).unwrap();

        assert_ne!(s.link(copy, 67), 1);
        assert_ne!(s.link(copy, 69), 1);
        assert_eq!(
            &s.pools["position"].bytes[unmanaged_position..unmanaged_position + 17],
            position_before.as_slice(),
            "the referenced inactive record must not be rewritten"
        );
        assert!(s.family_details(0, 1).is_ok());
        assert!(s.family_details(copy, uid).is_ok());
        assert!(s.undo());
        assert_eq!(s.snapshot(), before);
    }
    #[test]
    fn save_second_publish_failure_restores_all_and_retry() {
        let mut s = disk_fixture("failure");
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
        let mut s = disk_fixture("conflict");
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
        let mut s = disk_fixture("recovery");
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
    fn add_particle(
        s: &mut ObjectStore,
        emitter_slot: usize,
        owner_obj: usize,
        light_slot: Option<u16>,
    ) {
        if !s.pools.contains_key("particle") {
            let mut p = vec![0; against_rome_data::particle::PAYLOAD_LEN];
            p[0..4].copy_from_slice(&1u32.to_le_bytes());
            p[4..8].copy_from_slice(&(PARTICLE_CAPACITY as u32).to_le_bytes());
            p[8..12].copy_from_slice(&(PARTICLE_CHILDREN as u32).to_le_bytes());
            for slot in 0..PARTICLE_CAPACITY {
                let rec = 12 + slot * PARTICLE_RECORD_SIZE;
                p[rec + 6..rec + 8].copy_from_slice(&(-1i16).to_le_bytes());
                for i in 0..PARTICLE_CHILDREN {
                    p[rec + 12 + i * 35 + 33..rec + 12 + i * 35 + 35]
                        .copy_from_slice(&(-1i16).to_le_bytes());
                }
                for col in 0..4 {
                    let off = 12 + PARTICLE_CAPACITY * PARTICLE_RECORD_SIZE + col * PARTICLE_CAPACITY * 4 + slot * 4;
                    let val = if col == 3 { 30i32 } else { 0i32 };
                    p[off..off + 4].copy_from_slice(&val.to_le_bytes());
                }
            }
            s.pools.insert("particle".into(), Blob::decode(p).unwrap());
        }
        let b = s.pools.get_mut("particle").unwrap();
        let rec = 12 + emitter_slot * PARTICLE_RECORD_SIZE;
        b.bytes[rec..rec + 2].copy_from_slice(&1u16.to_le_bytes());
        b.bytes[rec + 2..rec + 6].copy_from_slice(&1u32.to_le_bytes());
        b.bytes[rec + 6..rec + 8].copy_from_slice(&(owner_obj as u16).to_le_bytes());
        b.bytes[rec + 12] = 1;
        b.bytes[rec + 13..rec + 17].copy_from_slice(&65536i32.to_le_bytes());
        b.bytes[rec + 17..rec + 21].copy_from_slice(&131072i32.to_le_bytes());
        b.bytes[rec + 21..rec + 25].copy_from_slice(&196608i32.to_le_bytes());
        let light_val = light_slot.map(|x| x as i16).unwrap_or(-1);
        b.bytes[rec + 45..rec + 47].copy_from_slice(&light_val.to_le_bytes());
        b.raw = b.encoded();
        b.initial = b.bytes.clone();

        if let Some(l) = light_slot {
            let l_usize = l as usize;
            if !s.pools.contains_key("light") {
                let mut lb = vec![0; against_rome_data::pool::PAYLOAD_LEN];
                lb[0..4].copy_from_slice(&1u32.to_le_bytes());
                lb[4..8].copy_from_slice(&1024u32.to_le_bytes());
                lb[8..12].copy_from_slice(&1u32.to_le_bytes());
                let widths = against_rome_data::pool::WIDTHS;
                let mut base = 8;
                for (c, w) in widths.iter().enumerate() {
                    if c >= 13 {
                        lb[base..base + 1024 * w].fill(255);
                    }
                    base += 1024 * w;
                }
                s.pools.insert("light".into(), Blob::decode(lb).unwrap());
            }
            let lb = s.pools.get_mut("light").unwrap();
            let widths = against_rome_data::pool::WIDTHS;
            lb.bytes[8 + l_usize * 4..8 + l_usize * 4 + 4].copy_from_slice(&1u32.to_le_bytes());
            for a in 0..3 {
                let off = 8 + 1024 * widths[..10 + a].iter().sum::<usize>() + l_usize * 4;
                let v = ((a + 1) as f32).to_le_bytes();
                lb.bytes[off..off + 4].copy_from_slice(&v);
            }
            lb.raw = lb.encoded();
            lb.initial = lb.bytes.clone();
        }

        let o = s.column_offset(owner_obj, 11);
        s.pools.get_mut("objects").unwrap().bytes[o..o + 2]
            .copy_from_slice(&(emitter_slot as u16).to_le_bytes());
    }

    fn add_lager(
        s: &mut ObjectStore,
        camp_slot: usize,
        owner_obj: usize,
        members: &[usize],
        big_slot: Option<usize>,
    ) {
        if !s.pools.contains_key("lager") {
            let mut l = vec![0; LAGER_PAYLOAD_LEN];
            l[0..4].copy_from_slice(&1u32.to_le_bytes());
            l[4..8].copy_from_slice(&(LAGER_CAPACITY as u32).to_le_bytes());
            l[8..12].copy_from_slice(&6u32.to_le_bytes());
            l[12..16].copy_from_slice(&10u32.to_le_bytes());
            let big_base = 16 + LAGER_CAPACITY * LAGER_RECORD_SIZE;
            for i in 0..LAGER_CAPACITY {
                l[big_base + i * 2..big_base + i * 2 + 2].copy_from_slice(&(-1i16).to_le_bytes());
            }
            for i in 0..LAGER_CAPACITY {
                let rec = 16 + i * LAGER_RECORD_SIZE;
                for m in 0..10 {
                    l[rec + 23 + m * 2..rec + 23 + m * 2 + 2].copy_from_slice(&(-1i16).to_le_bytes());
                }
            }
            s.pools.insert("lager".into(), Blob::decode(l).unwrap());
        }
        let b = s.pools.get_mut("lager").unwrap();
        let rec = 16 + camp_slot * LAGER_RECORD_SIZE;
        b.bytes[rec] = 1;
        b.bytes[rec + 9..rec + 11].copy_from_slice(&(members.len() as i16).to_le_bytes());
        if big_slot.is_none() {
            for (i, &m) in members.iter().enumerate() {
                if i < 10 {
                    b.bytes[rec + 23 + i * 2..rec + 23 + i * 2 + 2]
                        .copy_from_slice(&(m as i16).to_le_bytes());
                }
            }
        }
        let big_val = big_slot.map(|x| x as i16).unwrap_or(-1);
        let big_base = 16 + LAGER_CAPACITY * LAGER_RECORD_SIZE;
        b.bytes[big_base + camp_slot * 2..big_base + camp_slot * 2 + 2]
            .copy_from_slice(&big_val.to_le_bytes());
        b.raw = b.encoded();
        b.initial = b.bytes.clone();

        let o = s.column_offset(owner_obj, 5);
        s.pools.get_mut("objects").unwrap().bytes[o..o + 2]
            .copy_from_slice(&(camp_slot as u16).to_le_bytes());
    }

    fn add_biglager(s: &mut ObjectStore, big_slot: usize, members: &[usize]) {
        if !s.pools.contains_key("biglager") {
            let mut bl = vec![0; BIG_LAGER_PAYLOAD_LEN];
            bl[0..4].copy_from_slice(&1u32.to_le_bytes());
            bl[4..8].copy_from_slice(&(BIG_LAGER_CAPACITY as u32).to_le_bytes());
            bl[8..12].copy_from_slice(&(BIG_LAGER_MEMBERS as u32).to_le_bytes());
            for i in 0..BIG_LAGER_CAPACITY {
                let rec = 12 + i * BIG_LAGER_RECORD_SIZE;
                for m in 0..BIG_LAGER_MEMBERS {
                    bl[rec + 1 + m * 2..rec + 1 + m * 2 + 2].copy_from_slice(&(-1i16).to_le_bytes());
                }
            }
            s.pools.insert("biglager".into(), Blob::decode(bl).unwrap());
        }
        let b = s.pools.get_mut("biglager").unwrap();
        let rec = 12 + big_slot * BIG_LAGER_RECORD_SIZE;
        b.bytes[rec] = 1;
        for (i, &m) in members.iter().enumerate() {
            if i < BIG_LAGER_MEMBERS {
                b.bytes[rec + 1 + i * 2..rec + 1 + i * 2 + 2]
                    .copy_from_slice(&(m as i16).to_le_bytes());
            }
        }
        b.raw = b.encoded();
        b.initial = b.bytes.clone();
    }

    #[test]
    fn garrison_occupancy_mismatch_rejected() {
        let mut s = fixture();
        add_lager(&mut s, 0, 0, &[1], None);
        let rec = 16;
        s.pools.get_mut("lager").unwrap().bytes[rec + 9..rec + 11]
            .copy_from_slice(&2i16.to_le_bytes());
        let err = s.family_details(0, 1).unwrap_err();
        assert!(err.contains("camp occupancy mismatch"), "{err}");
    }

    #[test]
    fn garrison_cycle_rejected() {
        let mut s = fixture();
        add_lager(&mut s, 0, 0, &[1], None);
        add_lager(&mut s, 1, 1, &[0], None);
        let err = s.family_details(0, 1).unwrap_err();
        assert!(err.contains("dependency cycle detected"), "{err}");
    }

    #[test]
    fn garrison_duplicate_occupant_rejected() {
        let mut s = fixture();
        add_lager(&mut s, 0, 0, &[1], None);
        let rec = 16;
        s.pools.get_mut("lager").unwrap().bytes[rec + 25..rec + 27]
            .copy_from_slice(&1i16.to_le_bytes());
        s.pools.get_mut("lager").unwrap().bytes[rec + 9..rec + 11]
            .copy_from_slice(&2i16.to_le_bytes());
        let err = s.family_details(0, 1).unwrap_err();
        assert!(err.contains("duplicate small member"), "{err}");
    }

    #[test]
    fn particle_emitter_owner_mismatch_rejected() {
        let mut s = fixture();
        add_particle(&mut s, 0, 0, None);
        let rec = 12;
        s.pools.get_mut("particle").unwrap().bytes[rec + 6..rec + 8]
            .copy_from_slice(&1i16.to_le_bytes());
        let err = s.family_details(0, 1).unwrap_err();
        assert!(err.contains("particle emitter owner mismatch"), "{err}");
    }


    #[test]
    fn particle_duplicate_emitter_reference_rejected() {
        let mut s = fixture();
        add_particle(&mut s, 0, 0, None);
        let p1 = s.column_offset(1, 13);
        s.pools.get_mut("objects").unwrap().bytes[p1..p1 + 2].copy_from_slice(&0u16.to_le_bytes());
        let o1 = s.column_offset(1, 11);
        s.pools.get_mut("objects").unwrap().bytes[o1..o1 + 2].copy_from_slice(&0u16.to_le_bytes());
        let err = s.family_details(0, 1).unwrap_err();
        assert!(err.contains("particle emitter owner mismatch") || err.contains("duplicate particle emitter"), "{err}");
    }

    #[test]
    fn particle_external_object_references_family_emitter_rejected() {
        let mut s = fixture();
        add_particle(&mut s, 0, 0, None);
        let o2 = s.column_offset(2, 11);
        s.pools.get_mut("objects").unwrap().bytes[o2..o2 + 2].copy_from_slice(&0u16.to_le_bytes());
        let err = s.family_details(0, 1).unwrap_err();
        assert!(err.contains("external object references family particle emitter"), "{err}");
    }

    #[test]
    fn garrison_external_object_references_family_camp_rejected() {
        let mut s = fixture();
        add_lager(&mut s, 0, 0, &[1], None);
        let o2 = s.column_offset(2, 5);
        s.pools.get_mut("objects").unwrap().bytes[o2..o2 + 2].copy_from_slice(&0u16.to_le_bytes());
        let err = s.family_details(0, 1).unwrap_err();
        assert!(err.contains("external object references family lager"), "{err}");
    }

    #[test]
    fn garrison_small_and_big_move_clone_delete_and_undo_redo() {
        let mut s = fixture_count(6);
        add_light(&mut s);
        for i in 3..6 {
            let r = s.record(i);
            s.pools.get_mut("objects").unwrap().bytes[r] = 0;
            s.pools.get_mut("position").unwrap().bytes[8 + i * 17] = 0;
            s.pools.get_mut("objdata").unwrap().bytes[8 + i * 9] = 0;
        }
        let p1 = s.column_offset(1, 13);
        s.pools.get_mut("objects").unwrap().bytes[p1..p1 + 2].copy_from_slice(&0u16.to_le_bytes());
        add_lager(&mut s, 0, 0, &[1], None);
        add_biglager(&mut s, 0, &[2]);
        add_lager(&mut s, 1, 1, &[2], Some(0));

        let details = s.family_details(0, 1).unwrap();
        assert_eq!(details.object_slots, vec![0, 1, 2]);
        assert_eq!(details.lager_slots, vec![0, 1]);
        assert_eq!(details.big_lager_slots, vec![0]);

        let before = s.snapshot();
        s.translate_family_fixed(0, 1, [65536, 131072, 196608]).unwrap();
        let objs = s.objects();
        assert_eq!(objs[0].position, [1., 2., 3.]);
        assert_eq!(objs[1].position, [21., 2., 3.]);
        assert_eq!(objs[2].position, [41., 2., 3.]);
        assert!(s.undo());
        assert_eq!(s.snapshot(), before);
        assert!(s.redo());

        let (copy, uid) = s.clone_family_fixed(0, 1, [65536, 0, 0]).unwrap();
        assert_eq!(copy, 3);
        assert_eq!(uid, 4);
        let copy_details = s.family_details(copy, uid).unwrap();
        assert_eq!(copy_details.object_slots, vec![3, 4, 5]);
        assert_eq!(copy_details.lager_slots, vec![2, 3]);
        assert_eq!(copy_details.big_lager_slots, vec![1]);

        let l_blob = &s.pools["lager"].bytes;
        let c2 = 16 + 2 * LAGER_RECORD_SIZE;
        assert_eq!(i16at(l_blob, c2 + 23), 4);
        let c3 = 16 + 3 * LAGER_RECORD_SIZE;
        assert_eq!(i16at(l_blob, c3 + 23), -1);
        let big_link = i16at(l_blob, 16 + LAGER_CAPACITY * LAGER_RECORD_SIZE + 3 * 2);
        assert_eq!(big_link, 1);
        let b_blob = &s.pools["biglager"].bytes;
        assert_eq!(i16at(b_blob, 12 + 1 * BIG_LAGER_RECORD_SIZE + 1), 5);

        s.remove_family(copy, uid).unwrap();
        assert!(!s.active(3));
        assert!(!s.active(4));
        assert!(!s.active(5));
        let l_blob = &s.pools["lager"].bytes;
        assert_eq!(l_blob[16 + 2 * LAGER_RECORD_SIZE], 0);
        assert_eq!(l_blob[16 + 3 * LAGER_RECORD_SIZE], 0);
        let b_blob = &s.pools["biglager"].bytes;
        assert_eq!(b_blob[12 + 1 * BIG_LAGER_RECORD_SIZE], 0);

        assert!(s.undo());
        assert!(s.active(3));
        assert_eq!(s.pools["lager"].bytes[16 + 2 * LAGER_RECORD_SIZE], 1);
        assert!(s.undo());
        assert!(!s.active(3));
        assert!(s.undo());
        assert_eq!(s.snapshot(), before);
    }

    #[test]
    fn particle_with_child_light_move_clone_delete_and_undo_redo() {
        let mut s = fixture_count(4);
        for i in 2..4 {
            let r = s.record(i);
            s.pools.get_mut("objects").unwrap().bytes[r] = 0;
            s.pools.get_mut("position").unwrap().bytes[8 + i * 17] = 0;
            s.pools.get_mut("objdata").unwrap().bytes[8 + i * 9] = 0;
        }
        let p1 = s.column_offset(1, 13);
        s.pools.get_mut("objects").unwrap().bytes[p1..p1 + 2].copy_from_slice(&0u16.to_le_bytes());
        add_particle(&mut s, 0, 0, Some(0));

        let details = s.family_details(0, 1).unwrap();
        assert_eq!(details.object_slots, vec![0, 1]);
        assert_eq!(details.particle_slots, vec![0]);
        assert_eq!(details.light_slots, vec![0]);

        let before = s.snapshot();
        s.translate_family_fixed(0, 1, [65536, 131072, 196608]).unwrap();
        let lights = LightPool::from_payload(s.payload("light").unwrap()).unwrap();
        assert_eq!(lights.capture(0).unwrap().position(), [2., 4., 6.]);

        assert!(s.undo());
        assert_eq!(s.snapshot(), before);
        assert!(s.redo());

        let (copy, uid) = s.clone_family_fixed(0, 1, [65536, 0, 0]).unwrap();
        assert_eq!(copy, 2);
        assert_eq!(uid, 3);
        let copy_details = s.family_details(copy, uid).unwrap();
        assert_eq!(copy_details.object_slots, vec![2, 3]);
        assert_eq!(copy_details.particle_slots, vec![1]);
        assert_eq!(copy_details.light_slots, vec![1]);

        let p_blob = &s.pools["particle"].bytes;
        let p1_rec = 12 + 1 * PARTICLE_RECORD_SIZE;
        assert_eq!(i16at(p_blob, p1_rec + 6), 2);
        assert_eq!(i16at(p_blob, p1_rec + 12 + 33), 1);

        let lights = LightPool::from_payload(s.payload("light").unwrap()).unwrap();
        assert!(lights.capture(1).is_some());

        s.remove_family(copy, uid).unwrap();
        assert!(!s.active(2));
        assert!(!s.active(3));
        let p_pool = ParticlePool::from_payload(s.payload("particle").unwrap()).unwrap();
        assert!(p_pool.capture(1).is_none());
        let l_pool = LightPool::from_payload(s.payload("light").unwrap()).unwrap();
        assert!(l_pool.capture(1).is_none());

        assert!(s.undo());
        assert!(s.active(2));
        let p_pool = ParticlePool::from_payload(s.payload("particle").unwrap()).unwrap();
        assert!(p_pool.capture(1).is_some());
        assert!(s.undo());
        assert!(!s.active(2));
        assert!(s.undo());
        assert_eq!(s.snapshot(), before);
    }

    #[test]
    #[ignore = "requires installed retail map; read only source, edits TEMP"]
    fn retail_temp_roundtrip() {
        let source = Path::new(r"C:\Program Files (x86)\Against Rome\MAPS\TUTOR_01");
        let dir = super::temp_fixture::dir("ar-rust-placement");
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
    #[ignore = "requires installed retail map TUTOR_01; compares against frozen C# oracle hashes"]
    fn retail_managed_differential() {
        let source = Path::new(r"C:\Program Files (x86)\Against Rome\MAPS\TUTOR_01");
        let root = super::temp_fixture::dir("ar-placement-oracle");
        let rust = root.join("rust");
        fs::create_dir_all(rust.join("DATA")).unwrap();
        let before: Vec<_> = fs::read_dir(source.join("DATA"))
            .unwrap()
            .map(|e| {
                let path = e.unwrap().path();
                let bytes = fs::read(&path).unwrap();
                fs::write(rust.join("DATA").join(path.file_name().unwrap()), &bytes).unwrap();
                (path, bytes)
            })
            .collect();
        let mut s = ObjectStore::load(&rust).unwrap();
        let object = s
            .objects()
            .into_iter()
            .find(|o| s.family(o.slot, o.uid).is_ok())
            .unwrap();
        assert_eq!((object.slot, object.uid), (0, 13136), "frozen oracle root");
        s.translate_family_fixed(object.slot, object.uid, [65536, 32768, -123])
            .unwrap();
        assert_pools_match_frozen(&s, "retail", "move");
        let (copy, uid) = s
            .clone_family_fixed(object.slot, object.uid, [131072, -65536, 4321])
            .unwrap();
        assert_eq!(frozen_text("retail", "copy.txt"), format!("{copy} {uid}"));
        assert_pools_match_frozen(&s, "retail", "clone");
        s.remove_family(copy, uid).unwrap();
        assert_pools_match_frozen(&s, "retail", "delete");
        for (p, b) in before {
            assert_eq!(fs::read(p).unwrap(), b);
        }
        let _ = fs::remove_dir_all(root);
    }
    #[test]
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
        let compare = |s: &ObjectStore, stage: &str| assert_pools_match_frozen(s, "light_aux", stage);
        s.translate_family_fixed(0, 1, [65536, 32768, -123])
            .unwrap();
        compare(&s, "move");
        let (copy, uid) = s.clone_family_fixed(0, 1, [131072, -65536, 4321]).unwrap();
        assert_eq!(uid, 100);
        assert_eq!(
            frozen_text("light_aux", "copy.txt"),
            format!("{copy} {uid}")
        );
        compare(&s, "clone");
        s.remove_family(copy, uid).unwrap();
        compare(&s, "delete");
    }
    fn particle_garrison_fixture() -> ObjectStore {
        let mut s = fixture_count(10);
        for i in 4..10 {
            let r = s.record(i);
            s.pools.get_mut("objects").unwrap().bytes[r] = 0;
            s.pools.get_mut("position").unwrap().bytes[8 + i * 17] = 0;
            s.pools.get_mut("objdata").unwrap().bytes[8 + i * 9] = 0;
        }
        for i in 0..4 {
            s.pools.get_mut("objdata").unwrap().bytes[8 + i * 9] = 1;
        }
        let p1 = s.column_offset(1, 13);
        s.pools.get_mut("objects").unwrap().bytes[p1..p1 + 2].copy_from_slice(&0u16.to_le_bytes());

        add_light(&mut s);
        add_particle(&mut s, 0, 0, Some(1));
        add_lager(&mut s, 0, 0, &[2], None);
        add_biglager(&mut s, 0, &[3]);
        add_lager(&mut s, 1, 1, &[3], Some(0));

        for (name, h, stride, tails) in [
            ("anim", 8, 21, 2),
            ("gfxtype", 8, 15, 1),
            ("action", 12, 25, 0),
        ] {
            let mut b = vec![0; h + 10 * (stride + tails * 2)];
            b[..4].copy_from_slice(&1u32.to_le_bytes());
            b[4..8].copy_from_slice(&10u32.to_le_bytes());
            if h == 12 {
                b[8..12].copy_from_slice(&6u32.to_le_bytes());
            }
            for i in 0..10 {
                for j in 1..stride {
                    b[h + i * stride + j] = (i * 31 + j) as u8;
                }
                b[h + i * stride] = if i < 4 { 1 } else { 0 };
                for t in 0..tails {
                    let o = h + 10 * stride + t * 20 + i * 2;
                    b[o..o + 2]
                        .copy_from_slice(&(0x1100u16 + t as u16 * 256 + i as u16).to_le_bytes());
                }
            }
            s.pools.insert(name.into(), Blob::decode(b).unwrap());
            for i in 0..4 {
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
        s
    }

    #[test]
    fn particle_garrison_family_golden_parity() {
        use against_rome_data::sha256::sha256_hex;

        let mut s = particle_garrison_fixture();
        let d = s.family_details(0, 1).unwrap();
        assert_eq!(d.object_slots, vec![0, 1, 2, 3]);
        assert_eq!(d.particle_slots, vec![0]);
        assert_eq!(d.lager_slots, vec![0, 1]);
        assert_eq!(d.big_lager_slots, vec![0]);
        assert_eq!(d.light_slots, vec![1, 0]);

        // 1. Move phase
        s.translate_family_fixed(0, 1, [65536, 32768, -123]).unwrap();
        let expected_move: [(&str, &str); 11] = [
            ("action", "C03AF0DE675DC8248D53EB7808C3001782BCCD8E1AFB4CFD614D593CF66669C2"),
            ("anim", "12642FBD6C36B92539CBECA3A43B4FB796FBCDEC35BE2800787F8F908468C2AD"),
            ("biglager", "C224E11FABCA4A5C5023CE29A3B2E47EBBDB67760098FAAE2833F63FE55482FE"),
            ("engine", "F3D4931422025448114CA2405E7709933099FBA13B95EC1F32099B4B5AE64039"),
            ("gfxtype", "AD93ED8599C30E98CB3986AC84CB15BD8C309A717498CD0759E9C64805F69691"),
            ("lager", "02DD00852DBA82F3C2C35904BA5CF3BA6D7FCB1DCB2F0367B50381B18581F097"),
            ("light", "460EB31203CD49FF1E416FF6A4C8D080663572136C36F011E838B4E57AE6A521"),
            ("objdata", "069A455AB53314F44CDA1FB620EC132B825E5EC92EE8159220711C3A61032865"),
            ("objects", "14DC73541D550A6935950B8416883157A6C07E5DD572748E7D6E54335A6761DD"),
            ("particle", "B66BEFFA69DD729ECFEAEB9D6BFE51F870910A5817077691C00AB1EE74730BCE"),
            ("position", "E4CA1B49BCBA0D12C47FC114F76295864F25776717CA83A762278421B30E25A0"),
        ];
        for (pool, expected_hash) in expected_move {
            let actual_hash = sha256_hex(s.payload(pool).unwrap()).to_uppercase();
            assert_eq!(actual_hash, expected_hash, "move/{pool} hash mismatch");
        }

        // 2. Clone phase
        let (copy, uid) = s.clone_family_fixed(0, 1, [131072, -65536, 4321]).unwrap();
        assert_eq!((copy, uid), (4, 100));
        let cloned_details = s.family_details(copy, uid).unwrap();
        assert_eq!(cloned_details.object_slots, vec![4, 5, 6, 7]);
        assert_eq!(cloned_details.particle_slots, vec![1]);
        assert_eq!(cloned_details.lager_slots, vec![2, 3]);
        assert_eq!(cloned_details.big_lager_slots, vec![1]);
        assert_eq!(cloned_details.light_slots, vec![2, 3]);

        let expected_clone: [(&str, &str); 11] = [
            ("action", "197E7F46A45D521829602F89BFF006E506ED4D01AB9B38F2F4B99255D5968D81"),
            ("anim", "903A47C71564961C34286FDF29BBC3A33ED7190D38FA3FA4C4871BD89E190A33"),
            ("biglager", "F0ECB3D820DAAC1F88F9506D672626F6B9FABCA7B62ABF1D28289FBE833506EE"),
            ("engine", "73277040E010090C93CF4C7A577EF73E277DC2E18AC294268A2C14C253F73E29"),
            ("gfxtype", "EF701BA4D5CAEAA9C5CE3EBF4A94D889507089C85ACB48524D7199CB0A3E7094"),
            ("lager", "52477780576627941788290D6420117D74189DB1232BE08C4FE353DF89589B91"),
            ("light", "9D536F17C6679535703D4FA55A80DFA23EA853AB7099CC75EAD93AA6FB4482B2"),
            ("objdata", "96397A6C2438772BFE93CDB935015CBAE378F5F2702C346D5B35433BE7F460E5"),
            ("objects", "859374A47D7C73E27E5484B63B96ADD7B6486EC49CF25E09FFB19CDD11B61824"),
            ("particle", "1BBE4D67442795D5FED33B4E392D7C63E8B5F2783EC6C8ED23688C3FE0FA4566"),
            ("position", "0A8BE67924AE3370C56B427FDCB4BF6813F93DBBF99E298B1F90BBEDFD61F8F0"),
        ];
        for (pool, expected_hash) in expected_clone {
            let actual_hash = sha256_hex(s.payload(pool).unwrap()).to_uppercase();
            assert_eq!(actual_hash, expected_hash, "clone/{pool} hash mismatch");
        }

        // 3. Delete phase
        s.remove_family(copy, uid).unwrap();
        let expected_delete: [(&str, &str); 11] = [
            ("action", "AB7E3591B5B4EFEBC493EC9C3C1E4A0EB52324A15E1570C00CD128020D05B7DD"),
            ("anim", "F1136D9F964C7D6743EF6C37DA90ACCF5796EE82740B3D9965F3F7072F10260A"),
            ("biglager", "C224E11FABCA4A5C5023CE29A3B2E47EBBDB67760098FAAE2833F63FE55482FE"),
            ("engine", "73277040E010090C93CF4C7A577EF73E277DC2E18AC294268A2C14C253F73E29"),
            ("gfxtype", "A8276B2A0B595488B3F5169334C876AC3174D72A2F9872A850D8ACE882CDE475"),
            ("lager", "02DD00852DBA82F3C2C35904BA5CF3BA6D7FCB1DCB2F0367B50381B18581F097"),
            ("light", "460EB31203CD49FF1E416FF6A4C8D080663572136C36F011E838B4E57AE6A521"),
            ("objdata", "069A455AB53314F44CDA1FB620EC132B825E5EC92EE8159220711C3A61032865"),
            ("objects", "E32556D32FAC2DDD6593A8C3E43C7B2463E10195921E1AA16BFC6B0C2A948812"),
            ("particle", "B66BEFFA69DD729ECFEAEB9D6BFE51F870910A5817077691C00AB1EE74730BCE"),
            ("position", "803E37D7FB034594C8B609AB08418C3EA5C70E99B8C537221EDF449327C68053"),
        ];
        for (pool, expected_hash) in expected_delete {
            let actual_hash = sha256_hex(s.payload(pool).unwrap()).to_uppercase();
            assert_eq!(actual_hash, expected_hash, "delete/{pool} hash mismatch");
        }

        // Original family remains valid and intact
        assert_eq!(s.family_details(0, 1).unwrap(), d);
    }

    #[test]
    fn managed_family_particle_garrison_differential() {
        let mut s = particle_garrison_fixture();

        let compare = |s: &ObjectStore, stage: &str| assert_pools_match_frozen(s, "particle_garrison", stage);
        s.translate_family_fixed(0, 1, [65536, 32768, -123]).unwrap();
        compare(&s, "move");
        let (copy, uid) = s.clone_family_fixed(0, 1, [131072, -65536, 4321]).unwrap();
        assert_eq!(uid, 100);
        assert_eq!(
            frozen_text("particle_garrison", "copy.txt"),
            format!("{copy} {uid}")
        );
        compare(&s, "clone");
        s.remove_family(copy, uid).unwrap();
        compare(&s, "delete");
    }

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    struct TsvRow {
        index: usize,
        name: String,
        x: f32,
        y: f32,
        z: f32,
        team: u16,
        angle: f32,
        unit_count: i32,
    }

    fn workflow_fixture(total: usize) -> ObjectStore {
        let mut s = fixture_count(total);
        for i in 3..total {
            let r = s.record(i);
            s.pools.get_mut("objects").unwrap().bytes[r] = 0;
            s.pools.get_mut("position").unwrap().bytes[8 + i * 17] = 0;
            s.pools.get_mut("objdata").unwrap().bytes[8 + i * 9] = 0;
        }
        for i in 0..3 {
            s.pools.get_mut("objdata").unwrap().bytes[8 + i * 9] = 1;
        }
        let initial_positions = [
            [400.0f32, 20.0f32, 600.0f32],
            [850.0f32, 10.0f32, 1200.0f32],
            [1500.0f32, 15.0f32, 2000.0f32],
        ];
        let initial_angles = [90.0f32, 180.0f32, 270.0f32];
        let initial_teams = [1u16, 1u16, 2u16];
        for i in 0..3 {
            let r = s.record(i);
            s.pools.get_mut("objects").unwrap().bytes[r + 1..r + 3].copy_from_slice(&initial_teams[i].to_le_bytes());
            let p_off = 8 + i * 17;
            let p_bytes = &mut s.pools.get_mut("position").unwrap().bytes;
            p_bytes[p_off + 1..p_off + 5].copy_from_slice(&initial_positions[i][0].to_le_bytes());
            p_bytes[p_off + 5..p_off + 9].copy_from_slice(&initial_positions[i][1].to_le_bytes());
            p_bytes[p_off + 9..p_off + 13].copy_from_slice(&initial_positions[i][2].to_le_bytes());
            p_bytes[p_off + 13..p_off + 17].copy_from_slice(&initial_angles[i].to_le_bytes());
        }
        for (name, h, stride, tails) in [
            ("anim", 8, 21, 2),
            ("gfxtype", 8, 15, 1),
            ("action", 12, 25, 0),
        ] {
            let mut b = vec![0; h + total * (stride + tails * 2)];
            b[..4].copy_from_slice(&1u32.to_le_bytes());
            b[4..8].copy_from_slice(&(total as u32).to_le_bytes());
            if h == 12 {
                b[8..12].copy_from_slice(&6u32.to_le_bytes());
            }
            for i in 0..total {
                for j in 1..stride {
                    b[h + i * stride + j] = (i * 31 + j) as u8;
                }
                b[h + i * stride] = if i < 3 { 1 } else { 0 };
                for t in 0..tails {
                    let o = h + total * stride + t * total * 2 + i * 2;
                    b[o..o + 2]
                        .copy_from_slice(&(0x1100u16 + t as u16 * 256 + i as u16).to_le_bytes());
                }
            }
            s.pools.insert(name.into(), Blob::decode(b).unwrap());
            for i in 0..3 {
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
        s.pools.insert("engine".into(), Blob::decode(engine).unwrap());
        s
    }

    #[test]
    fn managed_placement_workflow_and_batch_dialog_differential() {
let tsv_content = include_str!("../tests/fixtures/placement_workflow_oracle.tsv");
        let mut snap_delta = [0.0f32; 3];
        let mut stages: BTreeMap<String, Vec<TsvRow>> = BTreeMap::new();
        for line in tsv_content.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.is_empty() || parts[0].is_empty() {
                continue;
            }
            if parts[0] == "SNAP_DELTA" && parts.len() >= 4 {
                snap_delta = [
                    parts[1].parse().unwrap(),
                    parts[2].parse().unwrap(),
                    parts[3].parse().unwrap(),
                ];
                continue;
            }
            if parts.len() >= 9 {
                let row = TsvRow {
                    index: parts[1].parse().unwrap(),
                    name: parts[2].to_string(),
                    x: parts[3].parse().unwrap(),
                    y: parts[4].parse().unwrap(),
                    z: parts[5].parse().unwrap(),
                    team: parts[6].parse().unwrap(),
                    angle: parts[7].parse().unwrap(),
                    unit_count: parts[8].parse().unwrap(),
                };
                stages.entry(parts[0].to_string()).or_default().push(row);
            }
        }

        let mut s = workflow_fixture(8);

        // Stage 0: Initial state verification
        let initial_tsv = stages.get("initial").expect("missing initial stage");
        assert_eq!(initial_tsv.len(), 3);
        let objs = s.objects();
        for (i, tsv) in initial_tsv.iter().enumerate() {
            let o = &objs[i];
            assert!((o.position[0] - tsv.x).abs() < 1e-2);
            assert!((o.position[2] - tsv.z).abs() < 1e-2);
            assert_eq!(o.team, tsv.team);
            assert!((o.rotation_degrees - tsv.angle).abs() < 1e-2);
        }

        // Stage 1: Batch Translate differential (MoveSelectedPlacements dx=256, dz=-64)
        s.translate_families(&[(0, 1), (1, 2)], [256.0, 0.0, -64.0]).unwrap();
        let trans_tsv = stages.get("translate").expect("missing translate stage");
        let objs = s.objects();
        for (i, tsv) in trans_tsv.iter().enumerate() {
            let o = &objs[i];
            assert!((o.position[0] - tsv.x).abs() < 1e-2, "translate mismatch obj {i}: rust={}, c#={}", o.position[0], tsv.x);
            assert!((o.position[2] - tsv.z).abs() < 1e-2, "translate mismatch obj {i}: rust={}, c#={}", o.position[2], tsv.z);
            assert_eq!(o.team, tsv.team);
            assert!((o.rotation_degrees - tsv.angle).abs() < 1e-2);
        }

        // Stage 2: Batch Snap to Grid differential (MoveSelectedPlacements snap=true)
        let rust_snap_delta = s.snap_families_to_grid(&[(0, 1), (1, 2)]).unwrap();
        assert!((rust_snap_delta[0] - snap_delta[0]).abs() < 1e-2, "snap delta X mismatch: rust={}, c#={}", rust_snap_delta[0], snap_delta[0]);
        assert!((rust_snap_delta[2] - snap_delta[2]).abs() < 1e-2, "snap delta Z mismatch: rust={}, c#={}", rust_snap_delta[2], snap_delta[2]);
        let snap_tsv = stages.get("snap").expect("missing snap stage");
        let objs = s.objects();
        for (i, tsv) in snap_tsv.iter().enumerate() {
            let o = &objs[i];
            assert!((o.position[0] - tsv.x).abs() < 1e-2, "snap mismatch obj {i}: rust={}, c#={}", o.position[0], tsv.x);
            assert!((o.position[2] - tsv.z).abs() < 1e-2, "snap mismatch obj {i}: rust={}, c#={}", o.position[2], tsv.z);
            assert_eq!(o.team, tsv.team);
            assert!((o.rotation_degrees - tsv.angle).abs() < 1e-2);
        }

        // Stage 3: Batch Rotation differential (HandlePlacementRotationKey R key +45 deg)
        s.rotate_families(&[(0, 1), (1, 2), (2, 3)], 45.0).unwrap();
        let rot_tsv = stages.get("rotate").expect("missing rotate stage");
        let objs = s.objects();
        for (i, tsv) in rot_tsv.iter().enumerate() {
            let o = &objs[i];
            assert!((o.rotation_degrees - tsv.angle).abs() < 1e-2, "rotate mismatch obj {i}: rust={}, c#={}", o.rotation_degrees, tsv.angle);
        }

        // Stage 4: PlacementBatchEditDialog differential (Team=4, Angle=315 on [0, 1])
        s.batch_edit_families(&[(0, 1), (1, 2)], Some(4), Some(315.0)).unwrap();
        let batch_tsv = stages.get("batch_edit").expect("missing batch_edit stage");
        let objs = s.objects();
        for (i, tsv) in batch_tsv.iter().enumerate() {
            let o = &objs[i];
            assert_eq!(o.team, tsv.team, "batch_edit team mismatch obj {i}: rust={}, c#={}", o.team, tsv.team);
            assert!((o.rotation_degrees - tsv.angle).abs() < 1e-2, "batch_edit angle mismatch obj {i}: rust={}, c#={}", o.rotation_degrees, tsv.angle);
        }

        // Stage 5: Batch Clone differential (DuplicateSelectedPlacedObjects +256 X/Z)
        let cloned = s.clone_families_fixed(&[(0, 1), (1, 2)], [256 << 16, 0, 256 << 16]).unwrap();
        assert_eq!(cloned.len(), 2);
        let clone_tsv = stages.get("clone").expect("missing clone stage");
        assert_eq!(s.objects().len(), clone_tsv.len(), "cloned object count mismatch");

        // Stage 6: Batch Remove differential (DeleteSelectedPlacedObjects)
        s.remove_families(&cloned).unwrap();
        let remove_tsv = stages.get("remove").expect("missing remove stage");
        let objs = s.objects();
        assert_eq!(objs.len(), remove_tsv.len(), "remaining object count mismatch");
        for (i, tsv) in remove_tsv.iter().enumerate() {
            let o = &objs[i];
            assert!((o.position[0] - tsv.x).abs() < 1e-2);
            assert!((o.position[2] - tsv.z).abs() < 1e-2);
            assert_eq!(o.team, tsv.team);
            assert!((o.rotation_degrees - tsv.angle).abs() < 1e-2);
        }

        println!("C# PlacementWorkflow and PlacementBatchEditDialog differential verified: 6 stages identical across translate/snap/rotate/batch_edit/clone/remove");
    }

    #[test]
    fn managed_batch_data_pools_differential() {
        let mut s = workflow_fixture(8);
let compare = |s: &ObjectStore, stage: &str| assert_pools_match_frozen(s, "batch_data", stage);

        s.translate_families_fixed(&[(0, 1), (1, 2)], [65536, 0, -32768]).unwrap();
        compare(&s, "translate");

        let cloned = s.clone_families_fixed(&[(0, 1), (1, 2)], [131072, 0, 65536]).unwrap();
        let cloned_lines: Vec<_> = frozen_text("batch_data", "cloned.txt")
            .lines()
            .map(|l| {
                let parts: Vec<&str> = l.split_whitespace().collect();
                (parts[0].parse::<usize>().unwrap(), parts[1].parse::<u32>().unwrap())
            })
            .collect();
        assert_eq!(cloned, cloned_lines);
        compare(&s, "clone");

        s.remove_families(&cloned).unwrap();
        compare(&s, "remove");
    }

    // ---- particle / garrison family slice: shared fixture, rejection matrix, exhaustion, disk ----
    fn poke(s: &mut ObjectStore, pool: &str, offset: usize, bytes: &[u8]) {
        s.pools.get_mut(pool).unwrap().bytes[offset..offset + bytes.len()].copy_from_slice(bytes);
    }
    fn set_col(s: &mut ObjectStore, slot: usize, col: usize, value: u16) {
        let o = s.column_offset(slot, col);
        poke(s, "objects", o, &value.to_le_bytes());
    }
    fn light_offset(column: usize, slot: usize) -> usize {
        let w = against_rome_data::pool::WIDTHS;
        8 + 1024 * w[..column].iter().sum::<usize>() + slot * w[column]
    }
    fn lager_rec(slot: usize) -> usize {
        16 + slot * LAGER_RECORD_SIZE
    }
    fn big_link_offset(slot: usize) -> usize {
        16 + LAGER_CAPACITY * LAGER_RECORD_SIZE + slot * 2
    }
    fn biglager_rec(slot: usize) -> usize {
        12 + slot * BIG_LAGER_RECORD_SIZE
    }
    fn particle_rec(slot: usize) -> usize {
        12 + slot * PARTICLE_RECORD_SIZE
    }
    fn deactivate_object(s: &mut ObjectStore, i: usize) {
        let r = s.record(i);
        s.pools.get_mut("objects").unwrap().bytes[r] = 0;
        s.pools.get_mut("position").unwrap().bytes[8 + i * 17] = 0;
        s.pools.get_mut("objdata").unwrap().bytes[8 + i * 9] = 0;
    }
    /// Makes the current in-memory pools the "on disk" baseline (raw == bytes == initial).
    fn commit_pools(s: &mut ObjectStore) {
        for b in s.pools.values_mut() {
            b.raw = b.bytes.clone();
            b.initial = b.bytes.clone();
        }
    }
    /// Objects 0..=4 active, the rest free. 0 is the family root (UID 1) with direct light 0 and camp 0
    /// (small occupant 2); 1 is its child with emitter 0 (child light 1) and camp 1 (biglager 0, occupant 3);
    /// 4 is unrelated.
    fn rich_fixture_n(n: usize) -> ObjectStore {
        let mut s = fixture_count(n);
        for i in 5..n {
            deactivate_object(&mut s, i);
        }
        set_col(&mut s, 1, 13, 0);
        add_light(&mut s);
        add_particle(&mut s, 0, 1, Some(1));
        add_lager(&mut s, 0, 0, &[2], None);
        add_biglager(&mut s, 0, &[3]);
        add_lager(&mut s, 1, 1, &[3], Some(0));
        commit_pools(&mut s);
        s
    }
    fn rich_fixture() -> ObjectStore {
        rich_fixture_n(14)
    }
    fn sorted(v: &[usize]) -> Vec<usize> {
        let mut v = v.to_vec();
        v.sort();
        v
    }
    /// Bytes of every pool record the family owns, for "original untouched" comparisons.
    fn family_fingerprint(s: &ObjectStore, d: &FamilyDetails) -> Vec<u8> {
        let mut out = Vec::new();
        for &o in &d.object_slots {
            let r = s.record(o);
            out.extend_from_slice(&s.pools["objects"].bytes[r..r + 79]);
            for c in 0..18 {
                out.extend_from_slice(&s.col(o, c).to_le_bytes());
            }
            let p = &s.pools["position"].bytes;
            for pos in s.poslinks(o) {
                out.extend_from_slice(&p[8 + pos * 17..25 + pos * 17]);
            }
        }
        for &p in &d.particle_slots {
            let b = &s.pools["particle"].bytes;
            out.extend_from_slice(&b[particle_rec(p)..particle_rec(p) + PARTICLE_RECORD_SIZE]);
            for col in 0..4 {
                let o = 12 + PARTICLE_CAPACITY * PARTICLE_RECORD_SIZE + col * PARTICLE_CAPACITY * 4 + p * 4;
                out.extend_from_slice(&b[o..o + 4]);
            }
        }
        for &l in &d.lager_slots {
            let b = &s.pools["lager"].bytes;
            out.extend_from_slice(&b[lager_rec(l)..lager_rec(l) + LAGER_RECORD_SIZE]);
            out.extend_from_slice(&b[big_link_offset(l)..big_link_offset(l) + 2]);
        }
        for &g in &d.big_lager_slots {
            let b = &s.pools["biglager"].bytes;
            out.extend_from_slice(&b[biglager_rec(g)..biglager_rec(g) + BIG_LAGER_RECORD_SIZE]);
        }
        for &l in &d.light_slots {
            for (c, w) in against_rome_data::pool::WIDTHS.iter().enumerate() {
                let o = light_offset(c, l);
                out.extend_from_slice(&s.pools["light"].bytes[o..o + w]);
            }
        }
        out
    }
    fn assert_fail_closed(s: &mut ObjectStore, slot: usize, uid: u32, needle: &str, label: &str) {
        let before = s.snapshot();
        let err = s.family_details(slot, uid).unwrap_err();
        assert!(err.contains(needle), "{label}: expected {needle:?}, got {err:?}");
        assert!(s.translate_family_fixed(slot, uid, [1, 2, 3]).is_err(), "{label}: move accepted");
        assert!(s.clone_family_fixed(slot, uid, [1, 2, 3]).is_err(), "{label}: clone accepted");
        assert!(s.remove_family(slot, uid).is_err(), "{label}: remove accepted");
        assert!(s.set_family_team(slot, uid, 1).is_err(), "{label}: team accepted");
        assert!(s.rotate_family(slot, uid, 10.0).is_err(), "{label}: rotate accepted");
        assert!(s.set_family_vitals(slot, uid, Vitals::new(1.0, 1.0)).is_err(), "{label}: vitals accepted");
        assert_eq!(s.snapshot(), before, "{label}: rejected edit mutated pools");
        assert!(!s.can_undo() && !s.can_redo(), "{label}: rejected edit recorded history");
    }
    type Corrupt = fn(&mut ObjectStore);
    fn case(label: &'static str, needle: &'static str, f: Corrupt) -> (&'static str, &'static str, Corrupt) {
        (label, needle, f)
    }
    /// (label, expected rejection text, corruption applied to `rich_fixture`). Shared with the C# parity probe.
    fn corruption_cases() -> Vec<(&'static str, &'static str, Corrupt)> {
        vec![
            case("light-backlink-missing", "lacks matching direct-light backlink", |s| set_col(s, 0, 12, 65535)),
            case("second-owned-light", "lacks matching direct-light backlink", |s| {
                poke(s, "light", light_offset(0, 2), &1u32.to_le_bytes());
                poke(s, "light", light_offset(13, 2), &1i16.to_le_bytes());
            }),
            case("unreferenced-member-emitter", "unreferenced active particle emitter", |s| {
                add_particle(s, 5, 2, None);
                set_col(s, 2, 11, 65535);
            }),
            case("external-emitter-references-family-light", "external particle emitter references family light", |s| {
                add_particle(s, 6, 4, None);
                poke(s, "particle", particle_rec(6) + 12 + 33, &1i16.to_le_bytes());
            }),
            case("particle-light-owned-outside", "particle light owned outside family", |s| {
                poke(s, "light", light_offset(13, 1), &4i16.to_le_bytes())
            }),
            case("external-lager-member", "external lager references family member", |s| {
                add_lager(s, 5, 4, &[2], None)
            }),
            case("external-biglager-member", "external biglager references family member", |s| {
                add_biglager(s, 3, &[3])
            }),
            case("biglager-link-range", "biglager slot out of range", |s| {
                poke(s, "lager", big_link_offset(1), &40i16.to_le_bytes())
            }),
            case("biglager-link-inactive", "inactive biglager", |s| {
                poke(s, "lager", big_link_offset(1), &7i16.to_le_bytes())
            }),
            case("camp-inactive", "inactive lager camp", |s| poke(s, "lager", lager_rec(0), &[0])),
            case("camp-slot-range", "lager slot out of range", |s| set_col(s, 0, 5, 4000)),
            case("camp-member-inactive-object", "invalid camp member slot", |s| {
                poke(s, "lager", lager_rec(0) + 23, &5i16.to_le_bytes())
            }),
            case("camp-member-is-owner", "invalid camp member slot", |s| {
                poke(s, "lager", lager_rec(0) + 23, &0i16.to_le_bytes())
            }),
            case("camp-member-two-camps", "belongs to multiple camps", |s| {
                poke(s, "biglager", biglager_rec(0) + 1, &2i16.to_le_bytes());
                poke(s, "biglager", biglager_rec(0) + 3, &3i16.to_le_bytes());
                poke(s, "lager", lager_rec(1) + 9, &2i16.to_le_bytes());
            }),
            case("camp-occupancy-high", "camp occupancy mismatch", |s| {
                poke(s, "lager", lager_rec(0) + 9, &3i16.to_le_bytes())
            }),
            case("camp-occupancy-negative", "camp occupancy mismatch", |s| {
                poke(s, "lager", lager_rec(0) + 9, &(-1i16).to_le_bytes())
            }),
            case("garrison-cycle", "dependency cycle", |s| {
                poke(s, "biglager", biglager_rec(0) + 1, &0i16.to_le_bytes())
            }),
            case("member-external-parent", "member references external parent", |s| set_col(s, 2, 13, 4)),
            case("unsupported-column", "unsupported related pool column", |s| set_col(s, 2, 2, 0)),
            case("shared-objdata", "external shared position/objdata", |s| set_col(s, 4, 1, 2)),
            case("shared-direct-light", "external direct light", |s| set_col(s, 4, 12, 0)),
            case("emitter-slot-range", "particle slot out of range", |s| set_col(s, 1, 11, 2000)),
            case("emitter-inactive", "inactive or missing particle emitter", |s| {
                poke(s, "particle", particle_rec(0), &[0, 0])
            }),
            case("emitter-owner-mismatch", "particle emitter owner mismatch", |s| {
                poke(s, "particle", particle_rec(0) + 6, &0i16.to_le_bytes())
            }),
            case("particle-light-inactive", "particle child references inactive light", |s| {
                poke(s, "light", light_offset(0, 1), &0u32.to_le_bytes())
            }),
            case("attached-light-bad-definition", "invalid attached light definition", |s| {
                poke(s, "light", light_offset(0, 2), &1u32.to_le_bytes());
                poke(s, "light", light_offset(14, 2), &2i16.to_le_bytes());
                poke(s, "light", light_offset(15, 2), &300i16.to_le_bytes());
            }),
            case("mixed-light-owner-and-parent", "mixed or invalid attached light", |s| {
                poke(s, "light", light_offset(0, 2), &1u32.to_le_bytes());
                poke(s, "light", light_offset(13, 2), &2i16.to_le_bytes());
                poke(s, "light", light_offset(14, 2), &3i16.to_le_bytes());
            }),
        ]
    }
    #[test]
    fn rich_family_closure_and_edit_cycle() {
        let mut s = rich_fixture();
        let d = s.family_details(0, 1).unwrap();
        assert_eq!(sorted(&d.object_slots), vec![0, 1, 2, 3]);
        assert_eq!(d.particle_slots, vec![0]);
        assert_eq!(d.lager_slots, vec![0, 1]);
        assert_eq!(d.big_lager_slots, vec![0]);
        assert_eq!(sorted(&d.light_slots), vec![0, 1]);
        assert_eq!(s.family(0, 1).unwrap(), d.object_slots);
        // The unrelated object is its own family and stays editable.
        assert_eq!(s.family_details(4, 5).unwrap().object_slots, vec![4]);

        let original = family_fingerprint(&s, &d);
        let baseline = s.snapshot();
        let team = |s: &ObjectStore, slot: usize| s.objects().into_iter().find(|o| o.slot == slot).unwrap().team;
        let unrelated_team = team(&s, 4);
        let unrelated_vitals = s.vitals(4);
        s.set_family_team(0, 1, 3).unwrap();
        s.set_family_vitals(0, 1, Vitals::new(50.0, 25.0)).unwrap();
        s.rotate_family(0, 1, 30.0).unwrap();
        for slot in 0..4 {
            assert_eq!(team(&s, slot), 3, "slot {slot}");
        }
        assert_eq!(team(&s, 4), unrelated_team);
        assert_eq!(s.vitals(4), unrelated_vitals);
        assert_eq!(s.vitals(2).map(|v| v.hp), Some(50.0));
        for _ in 0..3 {
            assert!(s.undo());
        }
        assert_eq!(s.snapshot(), baseline);

        let (c1, u1) = s.clone_family_fixed(0, 1, [65536, 0, 0]).unwrap();
        let (c2, u2) = s.clone_family_fixed(0, 1, [0, 65536, 0]).unwrap();
        let d1 = s.family_details(c1, u1).unwrap();
        let d2 = s.family_details(c2, u2).unwrap();
        let uids = |d: &FamilyDetails| {
            let mut v: Vec<_> = d.object_slots.iter().map(|&o| s.objects().iter().find(|w| w.slot == o).unwrap().uid).collect();
            v.sort();
            v
        };
        assert_eq!(uids(&d1), vec![6, 7, 8, 9]);
        assert_eq!(uids(&d2), vec![10, 11, 12, 13]);
        assert_eq!((d1.particle_slots.len(), d1.lager_slots.len(), d1.big_lager_slots.len(), d1.light_slots.len()), (1, 2, 1, 2));
        // Pairwise disjoint pool ownership across the original and both copies.
        let sets = |d: &FamilyDetails| [d.object_slots.clone(), d.particle_slots.clone(), d.lager_slots.clone(), d.big_lager_slots.clone(), d.light_slots.clone()];
        for (a, b) in [(&d, &d1), (&d, &d2), (&d1, &d2)] {
            for (x, y) in sets(a).iter().zip(sets(b).iter()) {
                assert!(x.iter().all(|v| !y.contains(v)), "overlap {x:?} {y:?}");
            }
        }
        assert_eq!(s.family_details(0, 1).unwrap(), d);
        assert_eq!(family_fingerprint(&s, &d), original, "cloning disturbed the source family");
        // Moved by the requested fixed delta; particle child light is re-derived from the child position.
        let source = s.objects().into_iter().find(|o| o.slot == 0).unwrap();
        let moved = s.objects().into_iter().find(|o| o.slot == c1).unwrap();
        assert_eq!(moved.position, [source.position[0] + 1., source.position[1], source.position[2]]);
        let lights = LightPool::from_payload(s.payload("light").unwrap()).unwrap();
        let child_light = |dd: &FamilyDetails| *dd.light_slots.iter().find(|&&l| lights.capture(l as u32).unwrap().owner() < 0).unwrap();
        assert_eq!(lights.capture(child_light(&d) as u32).unwrap().position(), [0., 0., 0.], "source child light must not move");
        assert_eq!(lights.capture(child_light(&d1) as u32).unwrap().position(), [2., 2., 3.]);
        assert_eq!(lights.capture(child_light(&d2) as u32).unwrap().position(), [1., 3., 3.]);

        s.remove_family(c2, u2).unwrap();
        s.remove_family(c1, u1).unwrap();
        assert!(s.family_details(c1, u1).is_err());
        assert_eq!(s.family_details(0, 1).unwrap(), d);
        assert_eq!(family_fingerprint(&s, &d), original);
        // Released pool slots are reusable and fully reset.
        assert_eq!(s.pools["particle"].bytes[particle_rec(d1.particle_slots[0])], 0);
        assert_eq!(s.pools["lager"].bytes[lager_rec(d1.lager_slots[0])], 0);
        assert_eq!(s.pools["biglager"].bytes[biglager_rec(d1.big_lager_slots[0])], 0);
        let after_remove = s.snapshot();
        assert!(s.undo() && s.undo());
        assert!(s.family_details(c1, u1).is_ok() && s.family_details(c2, u2).is_ok());
        assert!(s.redo() && s.redo());
        assert_eq!(s.snapshot(), after_remove);
        // Undo everything back to the pre-clone state.
        for _ in 0..4 {
            assert!(s.undo());
        }
        assert_eq!(s.snapshot(), baseline);
    }
    #[test]
    fn dangling_reference_reserves_free_object_slot() {
        let mut s = rich_fixture();
        // An unrelated camp still lists the free slot 5; that stale occupant must not be adopted by a copy.
        add_lager(&mut s, 5, 4, &[5], None);
        commit_pools(&mut s);
        s.family_details(0, 1).unwrap();
        let (root, uid) = s.clone_family_fixed(0, 1, [0, 0, 0]).unwrap();
        let copy = s.family_details(root, uid).unwrap();
        assert!(!copy.object_slots.contains(&5), "{:?}", copy.object_slots);
        assert_eq!(s.pools["lager"].bytes[lager_rec(5) + 23..lager_rec(5) + 25], 5i16.to_le_bytes());
    }
    #[test]
    fn corrupted_families_fail_closed_without_mutation() {
        let cases = corruption_cases();
        assert!(cases.len() >= 25);
        for (label, needle, corrupt) in cases {
            let mut s = rich_fixture();
            corrupt(&mut s);
            assert_fail_closed(&mut s, 0, 1, needle, label);
        }
        let mut s = rich_fixture();
        assert_fail_closed(&mut s, 0, 99, "stale object UID", "stale uid");
        assert_fail_closed(&mut s, 1, 2, "select family root", "non-root");
        assert_fail_closed(&mut s, 6, 7, "stale object UID", "inactive slot");
        assert_fail_closed(&mut s, 999, 1, "stale object UID", "slot beyond capacity");
        // Missing pools can never prove a complete family.
        for (pool, needle) in [("particle", "missing particle pool"), ("lager", "missing lager pool"), ("biglager", "missing biglager pool"), ("light", "missing")] {
            let mut s = rich_fixture();
            s.pools.remove(pool);
            assert_fail_closed(&mut s, 0, 1, needle, pool);
        }
    }
    #[test]
    fn exhausted_pools_refuse_clone_without_mutation() {
        type Fill = fn(&mut ObjectStore);
        let fills: [(&str, Fill); 6] = [
            ("particle", |s| (1..PARTICLE_CAPACITY).for_each(|i| poke(s, "particle", particle_rec(i), &[1, 0]))),
            ("lager", |s| (2..LAGER_CAPACITY).for_each(|i| poke(s, "lager", lager_rec(i), &[1]))),
            ("biglager", |s| (1..BIG_LAGER_CAPACITY).for_each(|i| poke(s, "biglager", biglager_rec(i), &[1]))),
            ("light", |s| (2..1024).for_each(|i| poke(s, "light", light_offset(0, i), &1u32.to_le_bytes()))),
            ("position", |s| (5..14).for_each(|i| s.pools.get_mut("position").unwrap().bytes[8 + i * 17] = 1)),
            ("objdata", |s| (5..14).for_each(|i| s.pools.get_mut("objdata").unwrap().bytes[8 + i * 9] = 1)),
        ];
        for (pool, fill) in fills {
            let mut s = rich_fixture();
            fill(&mut s);
            let before = s.snapshot();
            s.family_details(0, 1).unwrap_or_else(|e| panic!("{pool}: filler broke the family: {e}"));
            let err = s.clone_family_fixed(0, 1, [0, 0, 0]).unwrap_err();
            assert!(err.contains("insufficient free native pool slots"), "{pool}: {err}");
            assert_eq!(s.snapshot(), before, "{pool}");
            assert!(!s.can_undo(), "{pool}");
        }
        // Object capacity: 8 slots leave 3 free for a 4 object family.
        let mut s = rich_fixture_n(8);
        let before = s.snapshot();
        let err = s.clone_family_fixed(0, 1, [0, 0, 0]).unwrap_err();
        assert!(err.contains("insufficient free native pool slots"), "{err}");
        assert_eq!(s.snapshot(), before);
    }
    #[test]
    fn uid_exhaustion_and_numeric_overflow_refuse_without_mutation() {
        let mut s = rich_fixture();
        let mut engine = vec![0u8; 94];
        engine[..4].copy_from_slice(&1u32.to_le_bytes());
        engine[34..38].copy_from_slice(&0x00FF_FFFEu32.to_le_bytes());
        s.pools.insert("engine".into(), Blob::decode(engine).unwrap());
        let before = s.snapshot();
        assert!(s.clone_family_fixed(0, 1, [0, 0, 0]).unwrap_err().contains("UID exhausted"));
        assert_eq!(s.snapshot(), before);

        let mut s = rich_fixture();
        let before = s.snapshot();
        poke(&mut s, "particle", particle_rec(0) + 12 + 1, &(i32::MAX - 1).to_le_bytes());
        let poked = s.snapshot();
        assert!(s.translate_family_fixed(0, 1, [65536, 0, 0]).unwrap_err().contains("particle fixed overflow"));
        assert!(s.clone_family_fixed(0, 1, [65536, 0, 0]).is_err());
        assert_eq!(s.snapshot(), poked);
        assert_ne!(poked, before);

        let mut s = rich_fixture();
        poke(&mut s, "light", light_offset(1, 0), &f32::NAN.to_le_bytes());
        let poked = s.snapshot();
        assert!(s.translate_family_fixed(0, 1, [65536, 0, 0]).unwrap_err().contains("nonfinite"));
        assert_eq!(s.snapshot(), poked);
        assert!(!s.can_undo());
    }
    fn rich_disk(label: &str) -> (PathBuf, ObjectStore) {
        let s = rich_fixture();
        let root = std::env::temp_dir().join(format!(
            "ar-placement-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(root.join("DATA")).unwrap();
        for (name, blob) in &s.pools {
            fs::write(root.join("DATA").join(format!("{name}.dat")), &blob.raw).unwrap();
        }
        let store = ObjectStore::load(&root).unwrap();
        (root, store)
    }
    fn read_pools(root: &Path) -> BTreeMap<String, Vec<u8>> {
        ["objects", "objdata", "position", "light", "particle", "lager", "biglager"]
            .into_iter()
            .map(|n| (n.to_string(), fs::read(root.join("DATA").join(format!("{n}.dat"))).unwrap()))
            .collect()
    }
    #[test]
    fn particle_garrison_stage_save_reload_and_undo_after_save() {
        let (root, mut s) = rich_disk("family-save");
        let original = read_pools(&root);
        let base = s.family_details(0, 1).unwrap();
        let base_print = family_fingerprint(&s, &base);

        let (c, u) = s.clone_family_fixed(0, 1, [65536, 0, 0]).unwrap();
        s.save().unwrap();
        assert!(!s.is_dirty());
        let reload = ObjectStore::load(&root).unwrap();
        assert_eq!(reload.snapshot(), s.snapshot());
        let copy = reload.family_details(c, u).unwrap();
        assert_eq!(reload.family_details(0, 1).unwrap(), base);
        assert_eq!(family_fingerprint(&reload, &base), base_print);

        // An objects-only edit rewrites only objects.dat; every other pool file stays byte-identical.
        let cloned = read_pools(&root);
        s.set_family_team(c, u, 2).unwrap();
        s.save().unwrap();
        let team_saved = read_pools(&root);
        for name in ["objdata", "position", "light", "particle", "lager", "biglager"] {
            assert_eq!(team_saved[name], cloned[name], "{name} was rewritten");
        }
        assert_ne!(team_saved["objects"], cloned["objects"]);

        s.translate_family_fixed(c, u, [0, 0, 131072]).unwrap();
        s.save().unwrap();
        let mut reload = ObjectStore::load(&root).unwrap();
        assert_eq!(reload.snapshot(), s.snapshot());
        let lights = LightPool::from_payload(reload.payload("light").unwrap()).unwrap();
        let clone_child_light = *copy.light_slots.iter().find(|&&l| lights.capture(l as u32).unwrap().owner() < 0).unwrap();
        assert_eq!(lights.capture(clone_child_light as u32).unwrap().position(), [2., 2., 5.]);

        reload.remove_family(c, u).unwrap();
        reload.save().unwrap();
        let removed = ObjectStore::load(&root).unwrap();
        assert!(removed.family_details(c, u).is_err());
        assert_eq!(removed.family_details(0, 1).unwrap(), base);
        assert_eq!(family_fingerprint(&removed, &base), base_print);
        for pool in ["particle", "lager", "biglager"] {
            assert!(removed.payload(pool).is_some());
        }
        assert_eq!(removed.payload("particle").unwrap()[particle_rec(copy.particle_slots[0])], 0);
        assert_eq!(removed.payload("lager").unwrap()[lager_rec(copy.lager_slots[0])], 0);
        assert_eq!(removed.payload("biglager").unwrap()[biglager_rec(copy.big_lager_slots[0])], 0);

        // Undo survives saves: stepping back past a save restores the earlier state and re-saves it.
        reload.undo();
        reload.save().unwrap();
        assert_eq!(ObjectStore::load(&root).unwrap().snapshot(), reload.snapshot());
        assert!(ObjectStore::load(&root).unwrap().family_details(c, u).is_ok());
        assert_ne!(read_pools(&root), original);
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn multi_pool_publish_failure_restores_every_family_pool() {
        let (root, mut s) = rich_disk("family-rollback");
        let original = read_pools(&root);
        let (c, u) = s.clone_family_fixed(0, 1, [65536, 0, 0]).unwrap();
        let pending = s.snapshot();
        let error = s
            .save_using(|i, from, to| {
                if i == 3 {
                    Err(std::io::Error::other("injected fourth publication failure"))
                } else {
                    fs::rename(from, to)
                }
            })
            .unwrap_err();
        assert!(error.contains("injected fourth publication failure"), "{error}");
        assert_eq!(read_pools(&root), original, "rollback left a half written family");
        assert_eq!(s.snapshot(), pending);
        assert!(s.is_dirty());
        assert!(!root.join("DATA").join(".ar-placement.lock").exists());
        s.save().unwrap();
        let reload = ObjectStore::load(&root).unwrap();
        assert_eq!(reload.snapshot(), pending);
        assert!(reload.family_details(c, u).is_ok());
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn external_edit_to_family_pool_blocks_save() {
        let (root, mut s) = rich_disk("family-conflict");
        s.set_family_team(0, 1, 2).unwrap();
        let particle = root.join("DATA").join("particle.dat");
        let mut changed = fs::read(&particle).unwrap();
        changed[particle_rec(0) + 12] ^= 1;
        fs::write(&particle, &changed).unwrap();
        let before = read_pools(&root);
        assert!(s.save().unwrap_err().contains("external file change"));
        assert_eq!(read_pools(&root), before);
        assert!(s.is_dirty());
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    #[ignore = "requires installed retail maps; read only scan, no writes"]
    fn retail_scan_particle_garrison_families() {
        let maps = Path::new(r"C:\Program Files (x86)\Against Rome\MAPS");
        let mut dirs: Vec<_> = fs::read_dir(maps).unwrap().map(|e| e.unwrap().path()).filter(|p| p.join("DATA").join("objects.dat").is_file()).collect();
        dirs.sort();
        let (mut loaded, mut related_roots, mut accepted_related) = (0, 0, 0);
        let mut load_errors: BTreeMap<String, usize> = BTreeMap::new();
        let mut rejections: BTreeMap<String, usize> = BTreeMap::new();
        let mut accepted_samples = Vec::new();
        for dir in &dirs {
            let s = match ObjectStore::load(dir) {
                Ok(s) => s,
                Err(e) => {
                    *load_errors.entry(e).or_default() += 1;
                    continue;
                }
            };
            loaded += 1;
            let family_index = FamilyDetailsIndex::new(&s);
            let mut related_slots = BTreeSet::new();
            for &slot in &family_index.active_slots {
                if s.col(slot, 5) != 65535 || s.col(slot, 11) != 65535 {
                    related_slots.insert(slot);
                    related_slots.insert(s.col(slot, 13));
                }
            }
            for o in s.objects().into_iter().filter(|o| o.parent.is_none() && o.uid != 0) {
                if !related_slots.contains(&o.slot) {
                    continue;
                }
                related_roots += 1;
                match s.family_details_with_index(o.slot, o.uid, &family_index) {
                    Ok(d) => {
                        accepted_related += 1;
                        if accepted_samples.len() < 5 && (!d.particle_slots.is_empty() || !d.lager_slots.is_empty()) {
                            accepted_samples.push((dir.file_name().unwrap().to_string_lossy().to_string(), o.slot, o.uid, d));
                        }
                    }
                    Err(e) => *rejections.entry(e).or_default() += 1,
                }
            }
        }
        println!("retail scan: {} maps, {loaded} loaded, load errors {load_errors:?}", dirs.len());
        println!("roots with particle/garrison links: {related_roots}, accepted {accepted_related}, fail-closed {:?}", rejections);
        for sample in &accepted_samples {
            println!("accepted sample {sample:?}");
        }
        assert!(loaded > 0, "no retail map loaded");
    }
    #[test]
    fn managed_family_rejection_parity() {
        let mut labels = vec![("baseline", "", None)];
        labels.extend(corruption_cases().into_iter().map(|(l, n, f)| (l, n, Some(f))));
        let mut rust = Vec::new();
        for (_, _, corrupt) in &labels {
            let mut s = rich_fixture();
            if let Some(corrupt) = corrupt {
                corrupt(&mut s);
            }
            let index = FamilyDetailsIndex::new(&s);
            let indexed = s.family_details_with_index(0, 1, &index);
            let direct = s.family_details(0, 1);
            assert_eq!(indexed, direct, "shared index changed family result");
            rust.push(indexed.map(|d| {
                (
                    sorted(&d.object_slots),
                    d.particle_slots,
                    d.lager_slots,
                    d.big_lager_slots,
                    sorted(&d.light_slots),
                )
            }));
        }
        // Frozen output of the original C# LevelObjectStore.FamilyAt probe, one line per label.
        let stdout = include_str!("../tests/fixtures/placement_probe_oracle.txt");
        let managed: Vec<_> = stdout.lines().filter(|l| l.starts_with("accept") || l.starts_with("reject")).collect();
        assert_eq!(managed.len(), labels.len(), "frozen probe lines vs corruption matrix");
        let list = |text: &str, key: &str| -> Vec<usize> {
            text.split_whitespace()
                .find_map(|w| w.strip_prefix(key))
                .map(|v| v.split(',').filter(|x| !x.is_empty()).map(|x| x.parse().unwrap()).collect())
                .unwrap_or_default()
        };
        for ((label, needle, _), (rust, managed)) in labels.iter().zip(rust.iter().zip(&managed)) {
            match rust {
                Ok((objects, particles, lagers, bigs, lights)) => {
                    assert!(managed.starts_with("accept"), "{label}: Rust accepted, C# said {managed}");
                    assert_eq!(&sorted(&list(managed, "objects=")), objects, "{label}");
                    assert_eq!(&list(managed, "particles="), particles, "{label}");
                    assert_eq!(&list(managed, "lagers=").into_iter().collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>(), lagers, "{label}");
                    assert_eq!(&list(managed, "biglagers="), bigs, "{label}");
                    assert_eq!(&sorted(&list(managed, "lights=")), lights, "{label}");
                }
                Err(e) => {
                    assert!(e.contains(needle), "{label}: {e}");
                    assert!(*managed == "reject", "{label}: Rust rejected ({e}) but C# FamilyAt said {managed}");
                }
            }
            println!("C# parity {label}: {}", if rust.is_ok() { "both accept" } else { "both reject" });
        }
    }
    #[test]
    fn batch_translation_matches_sequential_pool_bytes_and_preserves_history() {
        for mut store in [rich_fixture(), fixture_count(16)] {
            let roots: Vec<_> = store.objects().into_iter()
                .filter(|o| o.parent.is_none()).map(|o| (o.slot, o.uid)).collect();
            let roots: Vec<_> = roots.into_iter()
                .filter(|&(s, u)| store.family_details(s, u).is_ok()).collect();
            assert!(!roots.is_empty());
            store.translate_family_fixed(roots[0].0, roots[0].1, [1, 2, 3]).unwrap();
            assert!(store.undo());
            let before = store.snapshot();
            let history = store.history.clone();
            let mut sequential = store.clone();
            let delta = [65537, -32769, 131073];
            for &(slot, uid) in &roots {
                sequential.translate_family_fixed(slot, uid, delta).unwrap();
            }
            let mut duplicated = roots.clone();
            duplicated.push(roots[0]);
            store.translate_families_fixed(&duplicated, delta).unwrap();
            assert_eq!(store.snapshot(), sequential.snapshot());
            assert!(store.redo.is_empty());
            let after = store.snapshot();
            assert!(store.undo());
            assert_eq!(store.snapshot(), before);
            assert_eq!(store.history, history);
            assert!(store.redo());
            assert_eq!(store.snapshot(), after);
            assert!(store.undo());
            let redo = store.redo.clone();
            let mut invalid = roots;
            invalid.push((usize::MAX, 999));
            assert!(store.translate_families_fixed(&invalid, delta).is_err());
            assert_eq!(store.snapshot(), before);
            assert_eq!(store.history, history);
            assert_eq!(store.redo, redo);
        }
    }
    #[test]
    fn translate_families_fixed_batch_and_undo_redo() {
        let mut store = fixture_count(4);
        let orig_pos_0 = store.objects()[0].position;
        let orig_pos_1 = store.objects()[1].position;
        let orig_pos_2 = store.objects()[2].position;
        let orig_pos_3 = store.objects()[3].position;

        store
            .translate_families_fixed(&[(0, 1), (1, 2)], [65536, 0, 131072])
            .unwrap();

        let objs = store.objects();
        assert_eq!(objs[0].position, [orig_pos_0[0] + 1.0, orig_pos_0[1], orig_pos_0[2] + 2.0]);
        assert_eq!(objs[1].position, [orig_pos_1[0] + 1.0, orig_pos_1[1], orig_pos_1[2] + 2.0]);
        assert_eq!(objs[2].position, orig_pos_2);
        assert_eq!(objs[3].position, orig_pos_3);

        // Single undo reverts both
        assert!(store.undo());
        let objs_undone = store.objects();
        assert_eq!(objs_undone[0].position, orig_pos_0);
        assert_eq!(objs_undone[1].position, orig_pos_1);

        // Single redo reapplies both
        assert!(store.redo());
        let objs_redone = store.objects();
        assert_eq!(objs_redone[0].position, [orig_pos_0[0] + 1.0, orig_pos_0[1], orig_pos_0[2] + 2.0]);
        assert_eq!(objs_redone[1].position, [orig_pos_1[0] + 1.0, orig_pos_1[1], orig_pos_1[2] + 2.0]);
    }
    #[test]
    fn translate_families_world_and_validation_limits() {
        let mut store = fixture_count(4);
        let orig_pos_0 = store.objects()[0].position;
        let orig_pos_1 = store.objects()[1].position;

        store
            .translate_families(&[(0, 1), (1, 2)], [2.5, 0.0, -1.0])
            .unwrap();
        let objs = store.objects();
        assert!((objs[0].position[0] - (orig_pos_0[0] + 2.5)).abs() < 1e-4);
        assert!((objs[0].position[2] - (orig_pos_0[2] - 1.0)).abs() < 1e-4);
        assert!((objs[1].position[0] - (orig_pos_1[0] + 2.5)).abs() < 1e-4);

        // Nonfinite rejects without mutation
        let err = store
            .translate_families(&[(0, 1)], [f32::NAN, 0.0, 0.0])
            .unwrap_err();
        assert!(err.contains("nonfinite"));

        // Out of signed 16.16 range rejects
        let err_range = store
            .translate_families(&[(0, 1)], [40000.0, 0.0, 0.0])
            .unwrap_err();
        assert!(err_range.contains("outside signed 16.16 range"));
    }
    #[test]
    fn snap_families_to_grid_preserves_spacing_and_undo() {
        let mut store = fixture_count(4);
        // Move slot 0 to (300, 0, 500) and slot 1 to (350, 0, 550) (spacing dx=50, dz=50)
        let orig_0 = store.objects()[0].position;
        let orig_1 = store.objects()[1].position;
        store
            .translate_families(
                &[(0, 1)],
                [300.0 - orig_0[0], 0.0, 500.0 - orig_0[2]],
            )
            .unwrap();
        store
            .translate_families(
                &[(1, 2)],
                [350.0 - orig_1[0], 0.0, 550.0 - orig_1[2]],
            )
            .unwrap();

        // Target for anchor (300, 500): floor(300/256)*256 + 128 = 384.0, floor(500/256)*256 + 128 = 384.0
        let delta = store.snap_families_to_grid(&[(0, 1), (1, 2)]).unwrap();
        assert!((delta[0] - 84.0).abs() < 1e-3);
        assert!((delta[2] - (-116.0)).abs() < 1e-3);

        let objs = store.objects();
        assert!((objs[0].position[0] - 384.0).abs() < 1e-3);
        assert!((objs[0].position[2] - 384.0).abs() < 1e-3);
        // Spacing preserved: 384 + 50 = 434
        assert!((objs[1].position[0] - 434.0).abs() < 1e-3);
        assert!((objs[1].position[2] - 434.0).abs() < 1e-3);

        // Single undo reverts snap
        assert!(store.undo());
        let objs_undone = store.objects();
        assert!((objs_undone[0].position[0] - 300.0).abs() < 1e-3);
        assert!((objs_undone[0].position[2] - 500.0).abs() < 1e-3);
        assert!((objs_undone[1].position[0] - 350.0).abs() < 1e-3);
        assert!((objs_undone[1].position[2] - 550.0).abs() < 1e-3);
    }
    #[test]
    fn batch_edit_families_team_and_rotation_atomic() {
        let mut store = fixture_count(3);
        store
            .batch_edit_families(&[(0, 1), (1, 2)], Some(5), Some(135.0))
            .unwrap();

        let objs = store.objects();
        assert_eq!(objs[0].team, 5);
        assert_eq!(objs[1].team, 5);
        assert_eq!(objs[0].rotation_degrees, 135.0);
        assert_eq!(objs[1].rotation_degrees, 135.0);
        assert_eq!(objs[2].team, 0); // slot 2 unchanged

        // Single undo reverts both team and rotation
        assert!(store.undo());
        let objs_undone = store.objects();
        assert_eq!(objs_undone[0].team, 0);
        assert_eq!(objs_undone[1].team, 0);
        assert_eq!(objs_undone[0].rotation_degrees, 90.0);
        assert_eq!(objs_undone[1].rotation_degrees, 90.0);

        // Single redo reapplies both
        assert!(store.redo());
        let objs_redone = store.objects();
        assert_eq!(objs_redone[0].team, 5);
        assert_eq!(objs_redone[1].team, 5);
        assert_eq!(objs_redone[0].rotation_degrees, 135.0);
        assert_eq!(objs_redone[1].rotation_degrees, 135.0);

        // Validation rejection
        let err_team = store
            .batch_edit_families(&[(0, 1)], Some(20), None)
            .unwrap_err();
        assert!(err_team.contains("invalid team id"));
        let err_rot = store
            .batch_edit_families(&[(0, 1)], None, Some(f32::NAN))
            .unwrap_err();
        assert!(err_rot.contains("nonfinite rotation"));
    }
    #[test]
    fn clone_families_fixed_and_single_undo() {
        let mut store = fixture_count(4);
        for free_slot in [2, 3] {
            let r = store.record(free_slot);
            store.pools.get_mut("objects").unwrap().bytes[r] = 0;
            store.pools.get_mut("position").unwrap().bytes[8 + free_slot * 17] = 0;
            store.pools.get_mut("objdata").unwrap().bytes[8 + free_slot * 9] = 0;
        }
        let initial_count = store.objects().len();
        assert_eq!(initial_count, 2);
        let cloned = store
            .clone_families_fixed(&[(0, 1), (1, 2)], [65536, 0, 0])
            .unwrap();
        assert_eq!(cloned.len(), 2);
        assert_ne!(cloned[0].0, cloned[1].0);
        assert_ne!(cloned[0].1, cloned[1].1);
        assert_eq!(store.objects().len(), initial_count + 2);

        // Single undo deletes both clones
        assert!(store.undo());
        assert_eq!(store.objects().len(), initial_count);
    }
    #[test]
    fn remove_families_and_single_undo() {
        let mut store = fixture_count(4);
        let initial_count = store.objects().len();
        store.remove_families(&[(0, 1), (1, 2)]).unwrap();
        assert_eq!(store.objects().len(), initial_count - 2);

        // Single undo restores both
        assert!(store.undo());
        assert_eq!(store.objects().len(), initial_count);
    }
    #[test]
    fn batch_operations_fail_closed_on_stale_or_invalid_member() {
        let mut store = fixture_count(3);
        let orig_pos_0 = store.objects()[0].position;

        // One valid family (0, 1) and one invalid stale UID (1, 999)
        let err = store
            .translate_families_fixed(&[(0, 1), (1, 999)], [65536, 0, 0])
            .unwrap_err();
        assert!(err.contains("stale object UID"));

        // Slot 0 is NOT modified!
        assert_eq!(store.objects()[0].position, orig_pos_0);
        assert!(!store.can_undo());
    }
    include!("starter_import_tests.rs");
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
struct FamilyDetailsIndex {
    active_slots: Vec<usize>,
    children: BTreeMap<usize, Vec<usize>>,
    object_lagers: BTreeMap<usize, Vec<usize>>,
    object_particles: BTreeMap<usize, Vec<usize>>,
    object_lights: BTreeMap<usize, Vec<usize>>,
    lager_members: BTreeMap<usize, Vec<usize>>,
    lager_big_slots: BTreeMap<usize, Vec<usize>>,
    biglager_members: BTreeMap<usize, Vec<usize>>,
    particle_owners: BTreeMap<usize, Vec<usize>>,
    particle_lights: BTreeMap<usize, Vec<usize>>,
    attached_lights: BTreeMap<usize, Vec<against_rome_data::pool::LightSnapshot>>,
    particle: Option<std::result::Result<ParticlePool, String>>,
    light: Option<std::result::Result<LightPool, String>>,
}
impl FamilyDetailsIndex {
    fn new(store: &ObjectStore) -> Self {
        let mut active_slots = Vec::new();
        let mut children: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        let mut object_lagers: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        let mut object_particles: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        let mut object_lights: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for slot in 0..store.count {
            if !store.active(slot) {
                continue;
            }
            active_slots.push(slot);
            let parent = store.col(slot, 13);
            if parent != 65535 {
                children.entry(parent).or_default().push(slot);
            }
            for (value, map) in [
                (store.col(slot, 5), &mut object_lagers),
                (store.col(slot, 11), &mut object_particles),
                (store.col(slot, 12), &mut object_lights),
            ] {
                if value != 65535 {
                    map.entry(value).or_default().push(slot);
                }
            }
        }
        let mut lager_members: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        let mut lager_big_slots: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        if let Some(blob) = store.pools.get("lager") {
            for camp in 0..LAGER_CAPACITY {
                let rec = 16 + camp * LAGER_RECORD_SIZE;
                if blob.bytes[rec] == 0 {
                    continue;
                }
                for i in 0..10 {
                    let member = i16at(&blob.bytes, rec + 23 + i * 2);
                    if member >= 0 {
                        lager_members.entry(member as usize).or_default().push(camp);
                    }
                }
                let big = i16at(&blob.bytes, 16 + LAGER_CAPACITY * LAGER_RECORD_SIZE + camp * 2);
                if big >= 0 {
                    lager_big_slots.entry(big as usize).or_default().push(camp);
                }
            }
        }
        let mut biglager_members: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        if let Some(blob) = store.pools.get("biglager") {
            for big in 0..BIG_LAGER_CAPACITY {
                let rec = 12 + big * BIG_LAGER_RECORD_SIZE;
                if blob.bytes[rec] == 0 {
                    continue;
                }
                for i in 0..BIG_LAGER_MEMBERS {
                    let member = i16at(&blob.bytes, rec + 1 + i * 2);
                    if member >= 0 {
                        biglager_members.entry(member as usize).or_default().push(big);
                    }
                }
            }
        }
        let particle = store.pools.get("particle").map(|blob| {
            ParticlePool::from_payload(&blob.bytes).map_err(|error| error.to_string())
        });
        let mut particle_owners: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        let mut particle_lights: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        if let Some(Ok(pool)) = particle.as_ref() {
            for slot in pool.active_slots() {
                let Some(snapshot) = pool.capture(slot) else { continue };
                let slot = slot as usize;
                if snapshot.owner() >= 0 {
                    particle_owners.entry(snapshot.owner() as usize).or_default().push(slot);
                }
                for child in 0..PARTICLE_CHILDREN as u32 {
                    if let Ok(child) = snapshot.child(child) {
                        if child.light_link() >= 0 {
                            particle_lights.entry(child.light_link() as usize).or_default().push(slot);
                        }
                    }
                }
            }
        }
        let light = store.pools.get("light").map(|blob| {
            LightPool::from_payload(&blob.bytes).map_err(|error| error.to_string())
        });
        let mut attached_lights: BTreeMap<usize, Vec<against_rome_data::pool::LightSnapshot>> = BTreeMap::new();
        if let Some(Ok(pool)) = light.as_ref() {
            for snapshot in pool.active_snapshots() {
                if snapshot.owner() >= 0 {
                    attached_lights.entry(snapshot.owner() as usize).or_default().push(snapshot);
                }
                if snapshot.parent() >= 0 {
                    attached_lights.entry(snapshot.parent() as usize).or_default().push(snapshot);
                }
            }
        }
        Self {
            active_slots,
            children,
            object_lagers,
            object_particles,
            object_lights,
            lager_members,
            lager_big_slots,
            biglager_members,
            particle_owners,
            particle_lights,
            attached_lights,
            particle,
            light,
        }
    }
    fn particle_pool(&self) -> Result<Option<&ParticlePool>> {
        self.particle
            .as_ref()
            .map(|pool| pool.as_ref().map_err(Clone::clone))
            .transpose()
    }
    fn light_pool(&self) -> Result<Option<&LightPool>> {
        self.light
            .as_ref()
            .map(|pool| pool.as_ref().map_err(Clone::clone))
            .transpose()
    }
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
    /// 僅移除重做歷史，保留目前內容、基準與復原。
    pub fn clear_redo(&mut self) { self.redo.clear(); }
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
    /// Returns detailed family closure including object, light, particle, lager, and biglager slots.
    pub fn family_details(&self, slot: usize, uid: u32) -> Result<FamilyDetails> {
        let index = FamilyDetailsIndex::new(self);
        self.family_details_with_index(slot, uid, &index)
    }
    /// 同一個唯讀批次共用反向索引，避免每個根物件重新解碼全部資料池。
    pub fn family_details_batch(&self, roots: &[(usize, u32)]) -> Vec<Result<FamilyDetails>> {
        let index = FamilyDetailsIndex::new(self);
        roots.iter().map(|&(slot, uid)| self.family_details_with_index(slot, uid, &index)).collect()
    }
    fn family_details_with_index(
        &self,
        slot: usize,
        uid: u32,
        index: &FamilyDetailsIndex,
    ) -> Result<FamilyDetails> {
        if !self.active(slot) || u32at(&self.pools["objects"].bytes, self.record(slot) + 3) != uid {
            return Err("stale object UID".into());
        }
        if uid == 0 {
            return Err("invalid UID 0".into());
        }
        if self.col(slot, 13) != 65535 {
            return Err("select family root".into());
        }
        let mut members = BTreeSet::from([slot]);
        let mut order = vec![slot];
        let mut lager_slots = BTreeSet::new();
        let mut big_slots = BTreeSet::new();
        let mut garrison_owners: BTreeMap<usize, usize> = BTreeMap::new();
        let mut edges: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
        let mut cursor = 0;
        while cursor < order.len() {
            let owner = order[cursor];
            let mut related = BTreeSet::new();
            if let Some(descendants) = index.children.get(&owner) {
                related.extend(descendants.iter().copied());
            }
            let camp_slot = self.col(owner, 5);
            if camp_slot != 65535 {
                if camp_slot >= LAGER_CAPACITY {
                    return Err("lager slot out of range".into());
                }
                let lager_blob = self.pools.get("lager").ok_or("missing lager pool")?;
                let l_bytes = &lager_blob.bytes;
                let camp_rec = 16 + camp_slot * LAGER_RECORD_SIZE;
                if l_bytes[camp_rec] == 0 {
                    return Err("inactive lager camp".into());
                }
                if !lager_slots.insert(camp_slot) {
                    return Err("duplicate lager camp reference".into());
                }
                let mut occupants = BTreeSet::new();
                let mut small_count = 0;
                for i in 0..10 {
                    let m = i16at(l_bytes, camp_rec + 23 + i * 2);
                    if m != -1 {
                        if m < 0 {
                            return Err("invalid negative small member slot".into());
                        }
                        let m_usize = m as usize;
                        if !occupants.insert(m_usize) {
                            return Err("duplicate small member in camp".into());
                        }
                        small_count += 1;
                    }
                }
                let big_link = i16at(l_bytes, 16 + LAGER_CAPACITY * LAGER_RECORD_SIZE + camp_slot * 2);
                let (capacity, occupied_count) = if big_link != -1 {
                    if big_link < 0 || big_link as usize >= BIG_LAGER_CAPACITY {
                        return Err("biglager slot out of range".into());
                    }
                    let big_slot = big_link as usize;
                    let big_blob = self.pools.get("biglager").ok_or("missing biglager pool")?;
                    let b_bytes = &big_blob.bytes;
                    let big_rec = 12 + big_slot * BIG_LAGER_RECORD_SIZE;
                    if b_bytes[big_rec] == 0 {
                        return Err("inactive biglager".into());
                    }
                    if !big_slots.insert(big_slot) {
                        return Err("duplicate biglager reference".into());
                    }
                    let mut big_members = Vec::new();
                    let mut big_set = BTreeSet::new();
                    for i in 0..BIG_LAGER_MEMBERS {
                        let m = i16at(b_bytes, big_rec + 1 + i * 2);
                        if m != -1 {
                            if m < 0 {
                                return Err("invalid negative biglager member slot".into());
                            }
                            let m_usize = m as usize;
                            if !big_set.insert(m_usize) {
                                return Err("duplicate member in biglager".into());
                            }
                            big_members.push(m_usize);
                        }
                    }
                    let count = big_members.len();
                    occupants.extend(big_members);
                    (BIG_LAGER_MEMBERS, count)
                } else {
                    (10, small_count)
                };
                let occupancy = i16at(l_bytes, camp_rec + 9);
                if occupancy < 0 || occupancy as usize > capacity || occupancy as usize != occupied_count {
                    return Err("camp occupancy mismatch".into());
                }
                for &member in &occupants {
                    if member >= self.count || !self.active(member) || member == owner {
                        return Err("invalid camp member slot".into());
                    }
                    if let Some(&prev) = garrison_owners.get(&member) {
                        if prev != camp_slot {
                            return Err("camp member belongs to multiple camps".into());
                        }
                    }
                    garrison_owners.insert(member, camp_slot);
                    related.insert(member);
                }
            }
            edges.insert(owner, related.clone());
            for child in related {
                if members.insert(child) {
                    order.push(child);
                }
            }
            cursor += 1;
        }

        let mut incoming: BTreeMap<usize, usize> = members.iter().map(|&s| (s, 0)).collect();
        for outgoing in edges.values() {
            for &child in outgoing {
                *incoming.get_mut(&child).unwrap() += 1;
            }
        }
        let mut ready: std::collections::VecDeque<usize> =
            order.iter().filter(|&&s| incoming[&s] == 0).copied().collect();
        let mut sorted = Vec::new();
        while let Some(parent) = ready.pop_front() {
            sorted.push(parent);
            if let Some(outgoing) = edges.get(&parent) {
                for &child in outgoing {
                    let cnt = incoming.get_mut(&child).unwrap();
                    *cnt -= 1;
                    if *cnt == 0 {
                        ready.push_back(child);
                    }
                }
            }
        }
        if sorted.len() != members.len() {
            return Err("dependency cycle detected in family".into());
        }
        let order = sorted;
        for &s in &order {
            let parent = self.col(s, 13);
            if parent != 65535 && !members.contains(&parent) {
                return Err("member references external parent".into());
            }
        }

        for &camp in &lager_slots {
            if index.object_lagers.get(&camp).is_some_and(|owners| {
                owners.iter().any(|owner| !members.contains(owner))
            }) {
                return Err("external object references family lager".into());
            }
        }
        for &member in &members {
            if index.lager_members.get(&member).is_some_and(|camps| {
                camps.iter().any(|camp| !lager_slots.contains(camp))
            }) {
                return Err("external lager references family member".into());
            }
        }
        for &big in &big_slots {
            if index.lager_big_slots.get(&big).is_some_and(|camps| {
                camps.iter().any(|camp| !lager_slots.contains(camp))
            }) {
                return Err("external lager references family biglager".into());
            }
        }
        for &member in &members {
            if index.biglager_members.get(&member).is_some_and(|bigs| {
                bigs.iter().any(|big| !big_slots.contains(big))
            }) {
                return Err("external biglager references family member".into());
            }
        }

        let mut positions = BTreeSet::new();
        let mut data = BTreeSet::new();
        for &s in &order {
            for c in [2, 3, 4, 8] {
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
        for &s in &index.active_slots {
            if !members.contains(&s)
                && (self.poslinks(s).iter().any(|p| positions.contains(p))
                    || data.contains(&self.col(s, 1))
                    || [71, 73, 77].into_iter().any(|o| {
                        self.link(s, o) != 65535
                            && members.iter().any(|m| self.link(*m, o) == self.link(s, o))
                    }))
            {
                return Err("external shared position/objdata/auxiliary".into());
            }
        }

        let mut particle_slots = BTreeSet::new();
        let mut particle_lights = BTreeSet::new();
        for &s in &order {
            let target = self.col(s, 11);
            if target == 65535 {
                continue;
            }
            if target >= PARTICLE_CAPACITY {
                return Err("particle slot out of range".into());
            }
            let pool = index.particle_pool()?.ok_or("missing particle pool")?;
            let emitter = pool.capture(target as u32).ok_or("inactive or missing particle emitter")?;
            if emitter.owner() as usize != s {
                return Err("particle emitter owner mismatch".into());
            }
            if !particle_slots.insert(target) {
                return Err("duplicate particle emitter reference in family".into());
            }
            for i in 0..PARTICLE_CHILDREN as u32 {
                let child = emitter.child(i).map_err(|e| e.to_string())?;
                let light = child.light_link();
                if light != -1 {
                    if light < 0 || light as usize >= 1024 {
                        return Err("particle child light out of bounds".into());
                    }
                    let l_pool = index
                        .light_pool()?
                        .ok_or("missing light pool for particle light")?;
                    let saved = l_pool.capture(light as u32).ok_or("particle child references inactive light")?;
                    if saved.owner() >= 0 && !members.contains(&(saved.owner() as usize)) {
                        return Err("particle light owned outside family".into());
                    }
                    if saved.parent() >= 0 && !members.contains(&(saved.parent() as usize)) {
                        return Err("particle light parented outside family".into());
                    }
                    particle_lights.insert(light as usize);
                }
            }
        }
        for &owner in &members {
            if index.particle_owners.get(&owner).is_some_and(|emitters| {
                emitters.iter().any(|emitter| !particle_slots.contains(emitter))
            }) {
                return Err("family member owns unreferenced active particle emitter".into());
            }
        }
        for &particle in &particle_slots {
            if index.object_particles.get(&particle).is_some_and(|owners| {
                owners.iter().any(|owner| !members.contains(owner))
            }) {
                return Err("external object references family particle emitter".into());
            }
        }

        let mut light_slots: Vec<usize> = particle_lights.into_iter().collect();
        if let Some(l_pool) = index.light_pool()? {
            let mut related_lights = BTreeMap::new();
            for &member in &members {
                if let Some(lights) = index.attached_lights.get(&member) {
                    related_lights.extend(lights.iter().map(|light| (light.slot(), *light)));
                }
            }
            for l in related_lights.values() {
                let owner_inside = l.owner() >= 0 && members.contains(&(l.owner() as usize));
                let parent_inside = l.parent() >= 0 && members.contains(&(l.parent() as usize));
                if !owner_inside && !parent_inside {
                    continue;
                }
                if owner_inside && l.parent() != -1 || parent_inside && l.owner() != -1 {
                    return Err("mixed or invalid attached light ownership".into());
                }
                // Managed IsLinked(BrokenDirectLight): an owned light must be the owner's only direct-light backlink.
                if owner_inside && self.col(l.owner() as usize, 12) != l.slot() as usize {
                    return Err("owned light lacks matching direct-light backlink".into());
                }
                if parent_inside && !(0..256).contains(&l.definition()) {
                    return Err("invalid attached light definition".into());
                }
                let l_slot = l.slot() as usize;
                if !light_slots.contains(&l_slot) {
                    light_slots.push(l_slot);
                }
            }
            let light_targets: BTreeSet<usize> = light_slots.iter().copied().collect();
            for &light in &light_targets {
                if index.particle_lights.get(&light).is_some_and(|emitters| {
                    emitters.iter().any(|emitter| !particle_slots.contains(emitter))
                }) {
                    return Err("external particle emitter references family light".into());
                }
            }
            for &light in &light_targets {
                if index.object_lights.get(&light).is_some_and(|owners| {
                    owners.iter().any(|owner| !members.contains(owner))
                }) {
                    return Err("external direct light reference".into());
                }
            }
            for &s in &order {
                let l = self.col(s, 12);
                if l != 65535 {
                    let snap = l_pool.capture(l as u32).ok_or("missing direct light")?;
                    if snap.owner() != s as i16 || snap.parent() != -1 {
                        return Err("direct light owner mismatch".into());
                    }
                }
            }
        } else if order.iter().any(|s| self.col(*s, 12) != 65535) || !particle_slots.is_empty() {
            return Err("missing direct light pool".into());
        }
        if order.len() > 1 && !self.pools.contains_key("light") {
            return Err("missing family light pool".into());
        }

        Ok(FamilyDetails {
            object_slots: order,
            light_slots,
            particle_slots: particle_slots.into_iter().collect(),
            lager_slots: lager_slots.into_iter().collect(),
            big_lager_slots: big_slots.into_iter().collect(),
        })
    }

    /// Parent-child and garrison/particle closure, with strict external ownership rejection.
    pub fn family(&self, slot: usize, uid: u32) -> Result<Vec<usize>> {
        self.family_details(slot, uid).map(|d| d.object_slots)
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
    /// Exact signed 16.16 displacement, matching the managed family translation contract.
    pub fn translate_family_fixed(&mut self, slot: usize, uid: u32, delta: [i32; 3]) -> Result<()> {
        let details = self.family_details(slot, uid)?;
        let mut candidate = self.snapshot();
        self.translate_details_fixed(&details, &mut candidate, delta)?;
        self.history.push(self.snapshot());
        self.redo.clear();
        self.restore(candidate);
        Ok(())
    }
    // 僅修改候選座標，不建立子交易或重複複製資料池。
    fn translate_details_fixed(
        &self,
        details: &FamilyDetails,
        candidate: &mut BTreeMap<String, Vec<u8>>,
        delta: [i32; 3],
    ) -> Result<()> {
        let positions: BTreeSet<_> = details.object_slots.iter().flat_map(|s| self.poslinks(*s)).collect();
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
        let mut particle_child_lights = BTreeSet::new();
        let mut particle_light_coords: Vec<(usize, [f32; 3])> = Vec::new();
        if !details.particle_slots.is_empty() {
            let b = candidate.get_mut("particle").ok_or("missing particle pool")?;
            for &s in &details.particle_slots {
                let rec = 12 + s * PARTICLE_RECORD_SIZE;
                for i in 0..PARTICLE_CHILDREN {
                    let off = rec + 12 + i * 35;
                    if b[off] != 0 {
                        let mut fixed = [0i32; 3];
                        for a in 0..3 {
                            let cur = i32at(b, off + 1 + a * 4);
                            let translated = cur.checked_add(delta[a]).ok_or("particle fixed overflow")?;
                            fixed[a] = translated;
                            b[off + 1 + a * 4..off + 5 + a * 4].copy_from_slice(&translated.to_le_bytes());
                        }
                        let light_link = i16at(b, off + 33);
                        if light_link >= 0 {
                            let l_slot = light_link as usize;
                            particle_child_lights.insert(l_slot);
                            particle_light_coords.push((
                                l_slot,
                                std::array::from_fn(|a| (fixed[a] >> 16) as f32),
                            ));
                        }
                    }
                }
            }
        }
        if !particle_light_coords.is_empty() {
            if let Some(light_bytes) = candidate.get_mut("light") {
                for (l_slot, coords) in particle_light_coords {
                    for (a, &coord) in coords.iter().enumerate() {
                        let l_off = 8 + 1024 * 4 * (1 + a) + l_slot * 4;
                        light_bytes[l_off..l_off + 4].copy_from_slice(&coord.to_le_bytes());
                    }
                }
            }
        }
        if !details.light_slots.is_empty() {
            let light_bytes = candidate.get_mut("light").ok_or("missing light pool")?;
            for &l in &details.light_slots {
                if !particle_child_lights.contains(&l) {
                    for a in 0..3 {
                        translate(
                            light_bytes,
                            8 + 1024 * 4 * (1 + a) + l * 4,
                            a,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }
    /// Exact signed 16.16 displacement for multiple object families as a single atomic transaction.
    /// If any family fails validation, the entire batch is rejected and no mutations occur.
    pub fn translate_families_fixed(
        &mut self,
        families: &[(usize, u32)],
        delta: [i32; 3],
    ) -> Result<()> {
        if families.is_empty() || (delta[0] == 0 && delta[1] == 0 && delta[2] == 0) {
            return Ok(());
        }
        let mut seen = BTreeSet::new();
        let mut unique_families = Vec::new();
        for &(slot, uid) in families {
            if seen.insert((slot, uid)) {
                unique_families.push((slot, uid));
            }
        }
        // 平移不改變家族所有權與連結；共用唯讀索引，依原順序套用座標。
        let index = FamilyDetailsIndex::new(self);
        let mut candidate = self.snapshot();
        for &(slot, uid) in &unique_families {
            let details = self.family_details_with_index(slot, uid, &index)?;
            self.translate_details_fixed(&details, &mut candidate, delta)?;
        }
        self.history.push(self.snapshot());
        self.redo.clear();
        self.restore(candidate);
        Ok(())
    }
    /// World displacement for multiple object families as a single atomic transaction.
    /// Converts float world coordinates to signed 16.16 fixed point, rejecting nonfinite or out-of-range deltas.
    pub fn translate_families(
        &mut self,
        families: &[(usize, u32)],
        delta_world: [f32; 3],
    ) -> Result<()> {
        let mut delta_fixed = [0i32; 3];
        for axis in 0..3 {
            let val = delta_world[axis];
            if !val.is_finite() {
                return Err("nonfinite coordinate".into());
            }
            let scaled = (val as f64 * 65536.0).round();
            if !scaled.is_finite() || scaled < i32::MIN as f64 || scaled > i32::MAX as f64 {
                return Err("displacement outside signed 16.16 range".into());
            }
            delta_fixed[axis] = scaled as i32;
        }
        self.translate_families_fixed(families, delta_fixed)
    }
    /// World displacement for a single object family.
    pub fn translate_family(
        &mut self,
        slot: usize,
        uid: u32,
        delta_world: [f32; 3],
    ) -> Result<()> {
        self.translate_families(&[(slot, uid)], delta_world)
    }
    /// Snaps the first family (anchor) to a 256-unit tile center (128 mod 256) and moves all
    /// selected families by the exact same delta to preserve their arrangement.
    /// Returns the applied world delta [dx, dy, dz].
    pub fn snap_families_to_grid(
        &mut self,
        families: &[(usize, u32)],
    ) -> Result<[f32; 3]> {
        if families.is_empty() {
            return Ok([0.0, 0.0, 0.0]);
        }
        let (anchor_slot, anchor_uid) = families[0];
        let anchor = self
            .objects()
            .into_iter()
            .find(|o| o.slot == anchor_slot && o.uid == anchor_uid)
            .ok_or("anchor object not found or stale UID")?;
        let wx = anchor.position[0];
        let wz = anchor.position[2];
        if !wx.is_finite() || !wz.is_finite() {
            return Err("anchor position is nonfinite".into());
        }
        let target_x = (wx as f64 / 256.0).floor() * 256.0 + 128.0;
        let target_z = (wz as f64 / 256.0).floor() * 256.0 + 128.0;
        let dx = (target_x - wx as f64) as f32;
        let dz = (target_z - wz as f64) as f32;
        let delta_world = [dx, 0.0, dz];
        self.translate_families(families, delta_world)?;
        Ok(delta_world)
    }
    /// Snaps a single family to a 256-unit tile center. Returns the applied world delta [dx, dy, dz].
    pub fn snap_family_to_grid(&mut self, slot: usize, uid: u32) -> Result<[f32; 3]> {
        self.snap_families_to_grid(&[(slot, uid)])
    }
    /// Copies saved parent/light/garrison/particle families into independently allocated native pools.
    /// Returns the new root slot and UID. Definitions/native creation are deliberately not inferred.
    pub fn clone_family_fixed(
        &mut self,
        slot: usize,
        uid: u32,
        delta: [i32; 3],
    ) -> Result<(usize, u32)> {
        let details = self.family_details(slot, uid)?;
        let family = details.object_slots;
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
                if l.owner() >= 0 { reserved.insert(l.owner() as usize); }
                if l.parent() >= 0 { reserved.insert(l.parent() as usize); }
            }
        }
        if let Some(b) = self.pools.get("particle") {
            let p = ParticlePool::from_payload(&b.bytes).map_err(|e| e.to_string())?;
            for s in p.active_slots() {
                let owner = p.capture(s).unwrap().owner();
                if owner >= 0 { reserved.insert(owner as usize); }
            }
        }
        if let Some(b) = self.pools.get("lager") {
            for s in 0..LAGER_CAPACITY {
                let o = 16 + s * LAGER_RECORD_SIZE;
                if b.bytes[o] != 0 {
                    for i in 0..10 {
                        let m = i16at(&b.bytes, o + 23 + i * 2);
                        if m >= 0 { reserved.insert(m as usize); }
                    }
                }
            }
        }
        if let Some(b) = self.pools.get("biglager") {
            for s in 0..BIG_LAGER_CAPACITY {
                let o = 12 + s * BIG_LAGER_RECORD_SIZE;
                if b.bytes[o] != 0 {
                    for i in 0..BIG_LAGER_MEMBERS {
                        let m = i16at(&b.bytes, o + 1 + i * 2);
                        if m >= 0 { reserved.insert(m as usize); }
                    }
                }
            }
        }
        fn allocation(
            source: Vec<usize>,
            free: impl Iterator<Item = usize>,
        ) -> Result<BTreeMap<usize, usize>> {
            let targets: Vec<_> = free.take(source.len()).collect();
            if targets.len() != source.len() {
                return Err("insufficient free native pool slots".into());
            }
            Ok(source.into_iter().zip(targets).collect())
        }
        let objects = allocation(
            family.clone(),
            (0..self.count).filter(|s| !self.active(*s) && !reserved.contains(s)),
        )?;
        let all_active: Vec<_> = (0..self.count).filter(|s| self.active(*s)).collect();
        let used_positions: BTreeSet<_> = all_active
            .iter()
            .flat_map(|s| self.poslinks(*s))
            .filter(|p| *p < self.positions && self.pools["position"].bytes[8 + p * 17] == 0)
            .collect();
        let mut source_positions = Vec::new();
        for &s in &family {
            for p in [self.link(s, 67), self.link(s, 69), self.col(s, 10)] {
                if p != 65535 && !source_positions.contains(&p) {
                    source_positions.push(p);
                }
            }
        }
        let position = allocation(
            source_positions,
            (0..self.positions).filter(|s| {
                self.pools["position"].bytes[8 + s * 17] == 0 && !used_positions.contains(s)
            }),
        )?;
        let used_data: BTreeSet<_> = all_active.iter().map(|s| self.col(*s, 1)).collect();
        let mut source_data = Vec::new();
        for &s in &family {
            let d = self.col(s, 1);
            if d != 65535 && !source_data.contains(&d) {
                source_data.push(d);
            }
        }
        let data = allocation(
            source_data,
            (0..self.count)
                .filter(|s| self.pools["objdata"].bytes[8 + s * 9] == 0 && !used_data.contains(s)),
        )?;
        let mut aux = BTreeMap::new();
        for (name, o, h, stride) in [
            ("anim", 71, 8, 21),
            ("gfxtype", 73, 8, 15),
            ("action", 77, 12, 25),
        ] {
            let mut source = Vec::new();
            for &s in &family {
                let v = self.link(s, o);
                if v != 65535 && !source.contains(&v) {
                    source.push(v);
                }
            }
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
        let mut used_lights: BTreeSet<_> = all_active.iter().map(|s| self.col(*s, 12)).collect();
        if let Some(b) = self.pools.get("particle") {
            let p = ParticlePool::from_payload(&b.bytes).map_err(|e| e.to_string())?;
            for s in p.active_slots() {
                for ordinal in 0..PARTICLE_CHILDREN as u32 {
                    let child = p
                        .capture(s)
                        .unwrap()
                        .child(ordinal)
                        .map_err(|e| e.to_string())?;
                    if child.active_raw() != 0 && child.light_link() != -1 {
                        used_lights.insert(child.light_link() as usize);
                    }
                }
            }
        }
        let light = allocation(
            details.light_slots.clone(),
            (0..1024).filter(|s| {
                lights
                    .as_ref()
                    .is_none_or(|p| p.capture(*s as u32).is_none())
                    && !used_lights.contains(s)
            }),
        )?;
        let particle = if !details.particle_slots.is_empty() {
            let b = self.pools.get("particle").ok_or("missing particle pool")?;
            let p = ParticlePool::from_payload(&b.bytes).map_err(|e| e.to_string())?;
            let used_particles: BTreeSet<_> = all_active.iter().map(|s| self.col(*s, 11)).filter(|&x| x != 65535).collect();
            allocation(
                details.particle_slots.clone(),
                (0..PARTICLE_CAPACITY).filter(|&s| p.capture(s as u32).is_none() && !used_particles.contains(&s)),
            )?
        } else {
            BTreeMap::new()
        };
        let lager = if !details.lager_slots.is_empty() {
            let b = self.pools.get("lager").ok_or("missing lager pool")?;
            let used_lagers: BTreeSet<_> = all_active.iter().map(|s| self.col(*s, 5)).filter(|&x| x != 65535).collect();
            allocation(
                details.lager_slots.clone(),
                (0..LAGER_CAPACITY).filter(|&s| b.bytes[16 + s * LAGER_RECORD_SIZE] == 0 && !used_lagers.contains(&s)),
            )?
        } else {
            BTreeMap::new()
        };
        let biglager = if !details.big_lager_slots.is_empty() {
            let b = self.pools.get("biglager").ok_or("missing biglager pool")?;
            let mut used_big = BTreeSet::new();
            if let Some(l) = self.pools.get("lager") {
                for s in 0..LAGER_CAPACITY {
                    if l.bytes[16 + s * LAGER_RECORD_SIZE] != 0 {
                        let bl = i16at(&l.bytes, 16 + LAGER_CAPACITY * LAGER_RECORD_SIZE + s * 2);
                        if bl != -1 {
                            used_big.insert(bl as usize);
                        }
                    }
                }
            }
            allocation(
                details.big_lager_slots.clone(),
                (0..BIG_LAGER_CAPACITY).filter(|&s| b.bytes[12 + s * BIG_LAGER_RECORD_SIZE] == 0 && !used_big.contains(&s)),
            )?
        } else {
            BTreeMap::new()
        };
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
        // 暫存候選只發布資料池；複製舊復原快照會讓大量地景複製呈平方成本。
        let mut candidate = Self {
            pools: self.pools.clone(), count: self.count, positions: self.positions,
            root: self.root.clone(), history: Vec::new(), redo: Vec::new(),
        };
        let snapshot = self.snapshot();
        fn remap(v: usize, map: &BTreeMap<usize, usize>) -> Result<usize> {
            if v == 65535 {
                Ok(v)
            } else {
                map.get(&v).copied().ok_or_else(|| "missing family mapping".into())
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
            for (c, &col_w) in COLUMNS.iter().enumerate() {
                let v = self.col(src, c);
                let value = match c {
                    1 => remap(v, &data)?,
                    5 => remap(v, &lager)?,
                    10 => remap(v, &position)?,
                    11 => remap(v, &particle)?,
                    12 => remap(v, &light)?,
                    13 => remap(v, &objects)?,
                    _ => v,
                } as u32;
                let o = self.column_offset(dst, c);
                candidate.pools.get_mut("objects").unwrap().bytes[o..o + col_w]
                    .copy_from_slice(&value.to_le_bytes()[..col_w]);
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
                    let raw_link = u16at(&snapshot["light"], source);
                    let v = if raw_link != 65535 {
                        remap(raw_link as usize, &objects)?
                    } else {
                        65535
                    };
                    b[o..o + 2].copy_from_slice(&(v as u16).to_le_bytes());
                }
                base += 1024 * w;
            }
        }
        for (&src, &dst) in &particle {
            let src_off = 12 + src * PARTICLE_RECORD_SIZE;
            let dst_off = 12 + dst * PARTICLE_RECORD_SIZE;
            let b = candidate.pools.get_mut("particle").unwrap();
            b.bytes[dst_off..dst_off + PARTICLE_RECORD_SIZE]
                .copy_from_slice(&snapshot["particle"][src_off..src_off + PARTICLE_RECORD_SIZE]);
            let src_owner = i16at(&snapshot["particle"], src_off + 6);
            if src_owner >= 0 {
                let dst_owner = remap(src_owner as usize, &objects)?;
                b.bytes[dst_off + 6..dst_off + 8].copy_from_slice(&(dst_owner as i16).to_le_bytes());
            }
            for i in 0..PARTICLE_CHILDREN {
                let child_off = dst_off + 12 + i * 35;
                let src_light = i16at(&snapshot["particle"], src_off + 12 + i * 35 + 33);
                if src_light != -1 {
                    let dst_light = remap(src_light as usize, &light)?;
                    b.bytes[child_off + 33..child_off + 35].copy_from_slice(&(dst_light as i16).to_le_bytes());
                }
            }
            for col in 0..4 {
                let src_state = 12 + PARTICLE_CAPACITY * PARTICLE_RECORD_SIZE + col * PARTICLE_CAPACITY * 4 + src * 4;
                let dst_state = 12 + PARTICLE_CAPACITY * PARTICLE_RECORD_SIZE + col * PARTICLE_CAPACITY * 4 + dst * 4;
                b.bytes[dst_state..dst_state + 4].copy_from_slice(&snapshot["particle"][src_state..src_state + 4]);
            }
        }
        for (&src, &dst) in &biglager {
            let src_off = 12 + src * BIG_LAGER_RECORD_SIZE;
            let dst_off = 12 + dst * BIG_LAGER_RECORD_SIZE;
            let b = candidate.pools.get_mut("biglager").unwrap();
            b.bytes[dst_off..dst_off + BIG_LAGER_RECORD_SIZE]
                .copy_from_slice(&snapshot["biglager"][src_off..src_off + BIG_LAGER_RECORD_SIZE]);
            for i in 0..BIG_LAGER_MEMBERS {
                let off = dst_off + 1 + i * 2;
                let src_m = i16at(&snapshot["biglager"], src_off + 1 + i * 2);
                if src_m != -1 {
                    let dst_m = remap(src_m as usize, &objects)?;
                    b.bytes[off..off + 2].copy_from_slice(&(dst_m as i16).to_le_bytes());
                }
            }
        }
        for (&src, &dst) in &lager {
            let src_off = 16 + src * LAGER_RECORD_SIZE;
            let dst_off = 16 + dst * LAGER_RECORD_SIZE;
            let b = candidate.pools.get_mut("lager").unwrap();
            b.bytes[dst_off..dst_off + LAGER_RECORD_SIZE]
                .copy_from_slice(&snapshot["lager"][src_off..src_off + LAGER_RECORD_SIZE]);
            for i in 0..10 {
                let off = dst_off + 23 + i * 2;
                let src_m = i16at(&snapshot["lager"], src_off + 23 + i * 2);
                if src_m != -1 {
                    let dst_m = remap(src_m as usize, &objects)?;
                    b.bytes[off..off + 2].copy_from_slice(&(dst_m as i16).to_le_bytes());
                }
            }
            let src_big = i16at(&snapshot["lager"], 16 + LAGER_CAPACITY * LAGER_RECORD_SIZE + src * 2);
            let dst_big = if src_big != -1 {
                remap(src_big as usize, &biglager)? as i16
            } else {
                -1
            };
            b.bytes[16 + LAGER_CAPACITY * LAGER_RECORD_SIZE + dst * 2..16 + LAGER_CAPACITY * LAGER_RECORD_SIZE + dst * 2 + 2]
                .copy_from_slice(&dst_big.to_le_bytes());
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
    /// Copies multiple object families into independently allocated native pools as a single atomic transaction.
    /// Returns the new root (slot, UID) pairs.
    pub fn clone_families_fixed(
        &mut self,
        families: &[(usize, u32)],
        delta: [i32; 3],
    ) -> Result<Vec<(usize, u32)>> {
        if families.is_empty() {
            return Ok(Vec::new());
        }
        let mut seen = BTreeSet::new();
        let mut unique_families = Vec::new();
        for &(slot, uid) in families {
            if seen.insert((slot, uid)) {
                unique_families.push((slot, uid));
            }
        }
        let mut candidate = self.clone();
        let mut cloned = Vec::new();
        for &(slot, uid) in &unique_families {
            let key = candidate.clone_family_fixed(slot, uid, delta)?;
            cloned.push(key);
        }
        self.history.push(self.snapshot());
        self.redo.clear();
        self.restore(candidate.snapshot());
        Ok(cloned)
    }
    /// Removes a fully validated supported closure. Reset templates mirror LevelObjectStore.RemoveCore.
    pub fn remove_family(&mut self, slot: usize, uid: u32) -> Result<()> {
        let details = self.family_details(slot, uid)?;
        let family = details.object_slots;
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
        for s in &details.light_slots {
            if let Some(b) = candidate.get_mut("light") {
                let mut base = 8;
                for (c, w) in against_rome_data::pool::WIDTHS.iter().enumerate() {
                    let o = base + s * w;
                    b[o..o + w].fill(if c >= 13 { 255 } else { 0 });
                    base += 1024 * w;
                }
            }
        }
        if let Some(b) = candidate.get_mut("particle") {
            for &s in &details.particle_slots {
                let rec = 12 + s * PARTICLE_RECORD_SIZE;
                b[rec..rec + PARTICLE_RECORD_SIZE].fill(0);
                b[rec + 6..rec + 8].copy_from_slice(&(-1i16).to_le_bytes());
                for i in 0..PARTICLE_CHILDREN {
                    b[rec + 12 + i * 35 + 33..rec + 12 + i * 35 + 35].copy_from_slice(&(-1i16).to_le_bytes());
                }
                for col in 0..4 {
                    let off = 12 + PARTICLE_CAPACITY * PARTICLE_RECORD_SIZE + col * PARTICLE_CAPACITY * 4 + s * 4;
                    let val = if col == 3 { 30i32 } else { 0i32 };
                    b[off..off + 4].copy_from_slice(&val.to_le_bytes());
                }
            }
        }
        if let Some(b) = candidate.get_mut("biglager") {
            for &s in &details.big_lager_slots {
                let rec = 12 + s * BIG_LAGER_RECORD_SIZE;
                b[rec] = 0;
                b[rec + 1..rec + BIG_LAGER_RECORD_SIZE].fill(255);
            }
        }
        if let Some(b) = candidate.get_mut("lager") {
            for &s in &details.lager_slots {
                let rec = 16 + s * LAGER_RECORD_SIZE;
                b[rec..rec + LAGER_RECORD_SIZE].fill(0);
                b[rec + 23..rec + LAGER_RECORD_SIZE].fill(255);
                b[16 + LAGER_CAPACITY * LAGER_RECORD_SIZE + s * 2..16 + LAGER_CAPACITY * LAGER_RECORD_SIZE + s * 2 + 2].copy_from_slice(&(-1i16).to_le_bytes());
            }
        }
        self.history.push(self.snapshot());
        self.redo.clear();
        self.restore(candidate);
        Ok(())
    }
    /// Removes multiple validated object families as a single atomic transaction.
    pub fn remove_families(&mut self, families: &[(usize, u32)]) -> Result<()> {
        if families.is_empty() {
            return Ok(());
        }
        let mut seen = BTreeSet::new();
        let mut unique_families = Vec::new();
        for &(slot, uid) in families {
            if seen.insert((slot, uid)) {
                unique_families.push((slot, uid));
            }
        }
        let mut candidate = self.clone();
        for &(slot, uid) in &unique_families {
            candidate.remove_family(slot, uid)?;
        }
        self.history.push(self.snapshot());
        self.redo.clear();
        self.restore(candidate.snapshot());
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
    /// Sets rotation in degrees for multiple object families as a single atomic transaction.
    pub fn set_families_rotation(
        &mut self,
        families: &[(usize, u32)],
        degrees: f32,
    ) -> Result<()> {
        if !degrees.is_finite() {
            return Err("nonfinite rotation".into());
        }
        if families.is_empty() {
            return Ok(());
        }
        let mut seen = BTreeSet::new();
        let mut unique_families = Vec::new();
        for &(slot, uid) in families {
            if seen.insert((slot, uid)) {
                unique_families.push((slot, uid));
            }
        }
        let mut candidate = self.clone();
        for &(slot, uid) in &unique_families {
            candidate.set_family_rotation(slot, uid, degrees)?;
        }
        self.history.push(self.snapshot());
        self.redo.clear();
        self.restore(candidate.snapshot());
        Ok(())
    }
    /// Rotates multiple object families by delta degrees relative to current orientation as a single atomic transaction.
    pub fn rotate_families(
        &mut self,
        families: &[(usize, u32)],
        delta_degrees: f32,
    ) -> Result<()> {
        if !delta_degrees.is_finite() {
            return Err("nonfinite rotation".into());
        }
        if families.is_empty() || delta_degrees % 360.0 == 0.0 {
            return Ok(());
        }
        let mut seen = BTreeSet::new();
        let mut unique_families = Vec::new();
        for &(slot, uid) in families {
            if seen.insert((slot, uid)) {
                unique_families.push((slot, uid));
            }
        }
        let mut candidate = self.clone();
        for &(slot, uid) in &unique_families {
            candidate.rotate_family(slot, uid, delta_degrees)?;
        }
        self.history.push(self.snapshot());
        self.redo.clear();
        self.restore(candidate.snapshot());
        Ok(())
    }
    /// Sets team ownership for multiple object families as a single atomic transaction.
    pub fn set_families_team(
        &mut self,
        families: &[(usize, u32)],
        team: u16,
    ) -> Result<()> {
        if team > 15 && team != 65535 {
            return Err("invalid team id (must be 0..15 or 65535 for none)".into());
        }
        if families.is_empty() {
            return Ok(());
        }
        let mut seen = BTreeSet::new();
        let mut unique_families = Vec::new();
        for &(slot, uid) in families {
            if seen.insert((slot, uid)) {
                unique_families.push((slot, uid));
            }
        }
        let mut candidate = self.clone();
        for &(slot, uid) in &unique_families {
            candidate.set_family_team(slot, uid, team)?;
        }
        self.history.push(self.snapshot());
        self.redo.clear();
        self.restore(candidate.snapshot());
        Ok(())
    }
    /// Batch edits team and/or rotation for multiple object families as a single atomic transaction,
    /// directly matching the designer batch edit dialog workflow (PlacementBatchEditDialog).
    pub fn batch_edit_families(
        &mut self,
        families: &[(usize, u32)],
        team: Option<u16>,
        degrees: Option<f32>,
    ) -> Result<()> {
        if families.is_empty() || (team.is_none() && degrees.is_none()) {
            return Ok(());
        }
        if let Some(t) = team {
            if t > 15 && t != 65535 {
                return Err("invalid team id (must be 0..15 or 65535 for none)".into());
            }
        }
        if let Some(d) = degrees {
            if !d.is_finite() {
                return Err("nonfinite rotation".into());
            }
        }
        let mut seen = BTreeSet::new();
        let mut unique_families = Vec::new();
        for &(slot, uid) in families {
            if seen.insert((slot, uid)) {
                unique_families.push((slot, uid));
            }
        }
        let mut candidate = self.clone();
        for &(slot, uid) in &unique_families {
            if let Some(t) = team {
                candidate.set_family_team(slot, uid, t)?;
            }
            if let Some(d) = degrees {
                candidate.set_family_rotation(slot, uid, d)?;
            }
        }
        self.history.push(self.snapshot());
        self.redo.clear();
        self.restore(candidate.snapshot());
        Ok(())
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

mod import;
pub use import::fixed_displacement;

#[cfg(test)]
mod temp_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/temp_unique.rs"));
}
