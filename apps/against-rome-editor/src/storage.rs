//! Staged, synced two-layer save with rollback on ordinary failures.
//! Multi-file publication is not crash/power-loss atomic.
use crate::{bmp::BmpImage, terrain::TerrainSession};
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

#[derive(Debug)]
pub struct SaveReport {
    pub boden_path: PathBuf,
    pub boden_backup: Option<PathBuf>,
    pub collision_path: PathBuf,
    pub collision_backup: Option<PathBuf>,
    pub height_vertex_size: usize,
    pub collision_size: usize,
}

fn resolved_path(path: &Path) -> io::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    // Canonicalize components sequentially: resolving a junction before '..'
    // matches filesystem traversal instead of lexical parent removal.
    let mut result = PathBuf::new();
    for part in absolute.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                // Resolve a junction before traversing its parent, but do not
                // open unrelated ancestors (e.g. a restricted user profile).
                if result.try_exists()? {
                    result = fs::canonicalize(&result)?;
                }
                result.pop();
            }
            _ => result.push(part.as_os_str()),
        }
    }
    // Canonicalize the deepest existing path; preserve any not-yet-created tail.
    let mut tail = Vec::new();
    while !result.try_exists()? {
        let name = result
            .file_name()
            .ok_or_else(|| io::Error::other("path has no existing ancestor"))?;
        tail.push(name.to_os_string());
        result.pop();
    }
    result = fs::canonicalize(result)?;
    for name in tail.into_iter().rev() {
        result.push(name);
    }
    Ok(result)
}
pub fn is_forbidden_install_path(path: &Path) -> bool {
    is_forbidden_install_path_opt(path, false)
}

fn strip_unc(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{stripped}"))
    } else if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path
    }
}

pub fn is_forbidden_install_path_opt(path: &Path, allow_staged_marker: bool) -> bool {
    let resolved = resolved_path(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(path)
        }
    });
    let resolved = strip_unc(resolved);

    let Some(game_root) = against_rome_maps::detect_game_root(&resolved, None) else {
        return false;
    };
    let game_root = strip_unc(game_root);

    let Ok(rel) = resolved.strip_prefix(&game_root) else {
        return true;
    };

    let mut comps = rel.components();
    let Some(std::path::Component::Normal(first)) = comps.next() else {
        // Target is game root itself
        return true;
    };

    if !first.to_string_lossy().eq_ignore_ascii_case("MAPS") {
        // Any file or directory not in MAPS is forbidden
        return true;
    }

    let Some(std::path::Component::Normal(map_folder)) = comps.next() else {
        // Target is MAPS directory itself
        return true;
    };

    let folder_name = map_folder.to_string_lossy();
    let Some(slot) = against_rome_maps::parse_endl_slot(&folder_name) else {
        // Retail map (TUTOR, CAMP, etc.) or unauthorized folder
        return true;
    };

    if !(5..=999).contains(&slot) {
        // Retail endless ENDL_000..004 or out of range
        return true;
    }

    // For ENDL_005..999: check marker
    let map_dir = game_root.join("MAPS").join(&*folder_name);
    let has_marker = map_dir.join(against_rome_maps::CustomMapManifest::MARKER_FILE_NAME).is_file();
    if !has_marker && !allow_staged_marker {
        return true;
    }

    false
}

/// Read-only preflight for destinations used by the native whole-map coordinator.
#[allow(dead_code)]
pub(crate) fn preflight_transaction_target(path: &Path) -> Result<(), String> {
    preflight_transaction_target_opt(path, false)
}

pub(crate) fn preflight_transaction_target_opt(path: &Path, allow_staged_marker: bool) -> Result<(), String> {
    if is_forbidden_install_path_opt(path, allow_staged_marker) {
        return Err(format!("Security Guard: installation path forbidden: {}", path.display()));
    }
    for ancestor in path.ancestors().filter(|p| !p.as_os_str().is_empty()) {
        match fs::symlink_metadata(ancestor) {
            Ok(meta) => {
                let mut linked = meta.file_type().is_symlink();
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    linked |= meta.file_attributes() & 0x400 != 0;
                }
                if linked {
                    return Err(format!("Save preflight rejects symlink/reparse point: {}", ancestor.display()));
                }
                if ancestor == path && meta.is_dir() {
                    return Err(format!("Save target is a directory: {}", path.display()));
                }
                if ancestor != path && !meta.is_dir() {
                    return Err(format!("Save ancestor is not a directory: {}", ancestor.display()));
                }
                if meta.permissions().readonly() && (!meta.is_dir() || cfg!(unix)) {
                    return Err(format!("Save preflight rejects read-only path: {}", ancestor.display()));
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("Save preflight {}: {e}", ancestor.display())),
        }
    }
    Ok(())
}

#[allow(dead_code)]
pub(crate) fn preflight_transaction_tree(dir: &Path) -> Result<(), String> {
    preflight_transaction_tree_opt(dir, false)
}

pub(crate) fn preflight_transaction_tree_opt(dir: &Path, allow_staged_marker: bool) -> Result<(), String> {
    // Checking a fictitious child also validates the root as an ancestor directory.
    preflight_transaction_target_opt(&dir.join(".arm-preflight"), allow_staged_marker)?;
    if !dir.try_exists().map_err(|e| e.to_string())? {
        return Ok(());
    }
    fn visit(dir: &Path, allow_staged_marker: bool) -> Result<(), String> {
        for entry in fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.contains(".tmp_tx") || name.ends_with(".arm_temp")
                || name.ends_with(".arm_old") || name.ends_with(".tmp_arm") || name == ".arm-save.lock" {
                return Err(format!("Save preflight preserves existing recovery/lock path: {}", path.display()));
            }
            // Use a child to validate directories without rejecting them as file targets.
            preflight_transaction_target_opt(&if meta.is_dir() { path.join(".arm-preflight") } else { path.clone() }, allow_staged_marker)?;
            if meta.is_dir() {
                visit(&path, allow_staged_marker)?;
            }
        }
        Ok(())
    }
    visit(dir, allow_staged_marker)
}
pub fn load_map(dir: &Path) -> Result<TerrainSession, String> {
    if !dir.is_dir() {
        return Err(format!(
            "Specified path is not a directory: {}",
            dir.display()
        ));
    }
    let bytes = fs::read(dir.join("boden.bmp")).map_err(|e| {
        format!("Failed to read required boden.bmp: {e}; create a blank map explicitly instead")
    })?;
    let raw = BmpImage::parse(&bytes).map_err(|e| e.to_string())?;
    if raw.width != raw.height {
        return Err("boden.bmp must be square".into());
    }
    let vertex_size = raw.width;
    let path = dir.join("collision.bmp");
    let (size, collision, col) = if path.exists() {
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        let col = BmpImage::parse(&bytes).map_err(|e| e.to_string())?;
        if col.width != col.height {
            return Err("collision.bmp must be square".into());
        }
        (col.width, col.extract_collision_channel(), Some(col))
    } else {
        let size = vertex_size.checked_sub(1).ok_or("height size is zero")?;
        (size, vec![0; size * size], None)
    };
    TerrainSession::try_new(
        vertex_size,
        raw.extract_green_channel(),
        size,
        collision,
        Some(raw),
        col,
    )
    .map_err(|e| e.to_string())
}
struct Lock {
    file: Option<fs::File>,
    path: PathBuf,
}
impl Drop for Lock {
    fn drop(&mut self) {
        drop(self.file.take());
        let _ = fs::remove_file(&self.path);
    }
}
struct Staged {
    target: PathBuf,
    temp: PathBuf,
    old: PathBuf,
    original: Option<Vec<u8>>,
    backup: Option<PathBuf>,
    published: bool,
    owns_old: bool,
}
impl Drop for Staged {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.temp);
        if self.owns_old && !self.published {
            let _ = fs::remove_file(&self.old);
        }
    }
}
fn exclusive_sync(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    if let Err(e) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(e);
    }
    Ok(())
}
fn backup(target: &Path, original: &[u8]) -> io::Result<PathBuf> {
    for n in 0..10000 {
        let extension = if n == 0 {
            "bmp.arm_backup".to_owned()
        } else {
            format!("bmp.arm_backup.{n}")
        };
        let path = target.with_extension(extension);
        match exclusive_sync(&path, original) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "backup slots exhausted",
    ))
}
pub fn safe_save_map(dir: &Path, terrain: &mut TerrainSession) -> Result<SaveReport, String> {
    save_using(dir, terrain, |_, from, to| fs::rename(from, to))
}
/// Encode without changing the baseline or touching disk (whole-map transaction staging).
pub(crate) fn encode_terrain(terrain: &TerrainSession) -> Result<(BmpImage, BmpImage), String> {
    let mut height = terrain.raw_boden_bmp.clone().unwrap_or_else(|| {
        BmpImage::from_green_channel_grayscale(terrain.vertex_size, terrain.vertex_size, terrain.heights())
    });
    if height.width != terrain.vertex_size || height.height != terrain.vertex_size {
        return Err("raw height dimensions disagree with terrain".into());
    }
    height.update_green_channel(terrain.heights());
    let mut collision = terrain.raw_collision_bmp.clone().unwrap_or_else(|| {
        BmpImage::from_collision_channel(terrain.collision_size, terrain.collision_size, terrain.collision())
    });
    if collision.width != terrain.collision_size || collision.height != terrain.collision_size {
        return Err("raw collision dimensions disagree with terrain".into());
    }
    collision.update_collision_channel(terrain.collision());
    Ok((height, collision))
}
fn save_using(
    dir: &Path,
    terrain: &mut TerrainSession,
    mut publish: impl FnMut(usize, &Path, &Path) -> io::Result<()>,
) -> Result<SaveReport, String> {
    for path in [
        dir.to_path_buf(),
        dir.join("boden.bmp"),
        dir.join("collision.bmp"),
    ] {
        if is_forbidden_install_path(&path) {
            return Err(format!(
                "Security Guard: save to installation path '{}' is forbidden",
                path.display()
            ));
        }
    }
    let (height, collision) = encode_terrain(terrain)?;
    let payloads = [
        ("boden.bmp", height.encode_bmp()),
        ("collision.bmp", collision.encode_bmp()),
    ];
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let lock_path = dir.join(".arm-save.lock");
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&lock_path)
        .map_err(|e| format!("save lock: {e}"))?;
    let _lock = Lock {
        file: Some(file),
        path: lock_path,
    };
    let mut stages = Vec::new();
    for (name, _) in &payloads {
        let target = dir.join(name);
        for p in [
            target.with_extension("bmp.arm_temp"),
            target.with_extension("bmp.arm_old"),
        ] {
            if p.exists() {
                return Err(format!(
                    "existing transaction recovery file must be inspected: {}",
                    p.display()
                ));
            }
        }
    }
    for (name, bytes) in &payloads {
        let target = dir.join(name);
        let original = match fs::read(&target) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        let temp = target.with_extension("bmp.arm_temp");
        let old = target.with_extension("bmp.arm_old");
        exclusive_sync(&temp, bytes).map_err(|e| e.to_string())?;
        stages.push(Staged {
            target,
            temp,
            old,
            original,
            backup: None,
            published: false,
            owns_old: false,
        });
        let stage = stages.last_mut().unwrap();
        if let Some(original) = &stage.original {
            exclusive_sync(&stage.old, original).map_err(|e| e.to_string())?;
            stage.owns_old = true;
        }
    }
    for stage in &mut stages {
        if let Some(original) = &stage.original {
            stage.backup = Some(backup(&stage.target, original).map_err(|e| e.to_string())?);
        }
    }
    for index in 0..stages.len() {
        let stage = &stages[index];
        let result = (|| {
            let current = match fs::read(&stage.target) {
                Ok(b) => Some(b),
                Err(e) if e.kind() == io::ErrorKind::NotFound => None,
                Err(e) => return Err(e),
            };
            if current != stage.original {
                return Err(io::Error::other("source changed during save"));
            }
            publish(index, &stage.temp, &stage.target)
        })();
        if let Err(error) = result {
            let mut failures = Vec::new();
            for previous in stages.iter_mut().take(index).rev() {
                let rollback = if previous.original.is_some() {
                    fs::rename(&previous.old, &previous.target)
                } else {
                    fs::remove_file(&previous.target)
                };
                match rollback {
                    Ok(()) => previous.published = false,
                    Err(e) => failures.push(format!(
                        "{}: {e}; recovery {}",
                        previous.target.display(),
                        previous.old.display()
                    )),
                }
            }
            return Err(if failures.is_empty() {
                format!("save failed; all published layers rolled back: {error}")
            } else {
                format!(
                    "save failed: {error}; rollback errors: {}",
                    failures.join("; ")
                )
            });
        }
        stages[index].published = true;
    }
    let report = SaveReport {
        boden_path: stages[0].target.clone(),
        boden_backup: stages[0].backup.clone(),
        collision_path: stages[1].target.clone(),
        collision_backup: stages[1].backup.clone(),
        height_vertex_size: terrain.vertex_size,
        collision_size: terrain.collision_size,
    };
    for stage in &mut stages {
        stage.published = false;
    }
    terrain.raw_boden_bmp = Some(height);
    terrain.raw_collision_bmp = Some(collision);
    terrain.commit_baseline();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;


    struct Dir(PathBuf);
    impl Dir {
        fn new() -> Self {
            let p = crate::storage::temp_fixture::dir("arm-storage");
            fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn edit(s: &mut TerrainSession) {
        let before = s.heights()[0];
        s.apply_height_changes(&[against_rome_editor::terrain::SampleChange {
            index: 0,
            before,
            after: 99,
        }]);
        s.commit_stroke();
    }
    #[test]
    fn missing_height_is_not_blank() {
        let d = Dir::new();
        assert!(load_map(&d.0).err().unwrap().contains("required boden.bmp"));
    }
    #[test]
    fn guard_checks_components() {
        assert!(is_forbidden_install_path(Path::new(
            "C:/Program Files (x86)/Against Rome/MAPS/TEST"
        )));
        assert!(is_forbidden_install_path(Path::new(
            r"C:\Program Files\Against Rome\MAPS"
        )));
        assert!(!is_forbidden_install_path(Path::new(
            "D:/MyCustomMaps/Custom_001"
        )));
        assert!(!is_forbidden_install_path(Path::new(
            "D:/Program Files Copy/Against Rome"
        )));
        // Non-standard install path tests
        assert!(is_forbidden_install_path(Path::new(
            "D:/Games/Against Rome/MAPS/ENDL_001"
        )));
        assert!(is_forbidden_install_path(Path::new(
            "D:/Games/Against Rome/MAPS/TUTOR_01"
        )));
        assert!(is_forbidden_install_path(Path::new(
            "D:/Games/Against Rome/MAPS/CAMP_01"
        )));
        assert!(is_forbidden_install_path(Path::new(
            "D:/Games/Against Rome/Against_Rome.exe"
        )));
        assert!(is_forbidden_install_path(Path::new(
            "C:/Program Files (x86)/Against Rome/Against_Rome.exe"
        )));
        // Unmarked ENDL_998 in game maps is forbidden without marker
        assert!(is_forbidden_install_path(Path::new(
            "C:/Program Files (x86)/Against Rome/MAPS/ENDL_998"
        )));
        assert!(is_forbidden_install_path(Path::new(
            "D:/Games/Against Rome/MAPS/ENDL_005"
        )));
        // Genuine marker-backed ENDL_005 in Program Files is permitted!
        assert!(!is_forbidden_install_path(Path::new(
            "C:/Program Files (x86)/Against Rome/MAPS/ENDL_005"
        )));
        // With staged marker override during creation, any ENDL_005..999 is allowed!
        assert!(!is_forbidden_install_path_opt(
            Path::new("C:/Program Files (x86)/Against Rome/MAPS/ENDL_998"),
            true
        ));
        assert!(!is_forbidden_install_path_opt(
            Path::new("D:/Games/Against Rome/MAPS/ENDL_005"),
            true
        ));
        // Staged marker override still strictly rejects retail maps
        assert!(is_forbidden_install_path_opt(
            Path::new("D:/Games/Against Rome/MAPS/ENDL_001"),
            true
        ));
        assert!(is_forbidden_install_path_opt(
            Path::new("D:/Games/Against Rome/MAPS/TUTOR_01"),
            true
        ));
    }

    #[test]
    fn guard_permits_marker_backed_custom_map_in_synthetic_installs() {
        let d = Dir::new();
        let game = d.0.join("Games").join("Against Rome");
        let maps = game.join("MAPS");
        let custom_map = maps.join("ENDL_005");
        let retail_map = maps.join("ENDL_001");
        let tutor_map = maps.join("TUTOR_01");
        fs::create_dir_all(&custom_map).unwrap();
        fs::create_dir_all(&retail_map).unwrap();
        fs::create_dir_all(&tutor_map).unwrap();

        // 1. Without marker, custom_map is forbidden
        assert!(is_forbidden_install_path(&custom_map));

        // 2. Add .arm_custom_map marker to ENDL_005
        fs::write(custom_map.join(".arm_custom_map"), b"{}").unwrap();
        assert!(!is_forbidden_install_path(&custom_map));
        assert!(!is_forbidden_install_path(&custom_map.join("boden.bmp")));

        // 3. Retail maps are forbidden even if someone created .arm_custom_map in them
        fs::write(retail_map.join(".arm_custom_map"), b"{}").unwrap();
        assert!(is_forbidden_install_path(&retail_map));
        assert!(is_forbidden_install_path(&retail_map.join("boden.bmp")));

        fs::write(tutor_map.join(".arm_custom_map"), b"{}").unwrap();
        assert!(is_forbidden_install_path(&tutor_map));
    }
    #[test]
    fn second_publish_failure_rollback_retry_preserves_backup() {
        let d = Dir::new();
        let mut s = TerrainSession::create_blank(5, 25);
        safe_save_map(&d.0, &mut s).unwrap();
        let h = fs::read(d.0.join("boden.bmp")).unwrap();
        let c = fs::read(d.0.join("collision.bmp")).unwrap();
        edit(&mut s);
        let raw_h = s.raw_boden_bmp.clone();
        let raw_c = s.raw_collision_bmp.clone();
        let baseline = s.original_heights().to_vec();
        let result = save_using(&d.0, &mut s, |i, from, to| {
            if i == 1 {
                Err(io::Error::other("injected second publish"))
            } else {
                fs::rename(from, to)
            }
        });
        assert!(result.unwrap_err().contains("rolled back"));
        assert_eq!(fs::read(d.0.join("boden.bmp")).unwrap(), h);
        assert_eq!(fs::read(d.0.join("collision.bmp")).unwrap(), c);
        assert_eq!(s.raw_boden_bmp, raw_h);
        assert_eq!(s.raw_collision_bmp, raw_c);
        assert_eq!(s.original_heights(), baseline);
        assert!(s.is_dirty());
        assert!(s.can_undo());
        assert!(s.undo());
        assert_eq!(s.heights()[0], 25);
        assert!(s.redo());
        assert_eq!(s.heights()[0], 99);
        let bak = fs::read(d.0.join("boden.bmp.arm_backup")).unwrap();
        let report = safe_save_map(&d.0, &mut s).unwrap();
        assert!(!s.is_dirty());
        assert!(!s.can_undo()); // Existing core baseline policy clears saved history.
        assert_eq!(fs::read(d.0.join("boden.bmp.arm_backup")).unwrap(), bak);
        assert_ne!(
            report.boden_backup.unwrap(),
            d.0.join("boden.bmp.arm_backup")
        );
        assert_eq!(load_map(&d.0).unwrap().heights()[0], 99);
    }
    #[test]
    fn stale_recovery_files_are_preserved() {
        for suffix in ["bmp.arm_temp", "bmp.arm_old"] {
            let d = Dir::new();
            let mut s = TerrainSession::create_blank(5, 25);
            let stale = d.0.join("collision.bmp").with_extension(suffix);
            fs::write(&stale, b"precious").unwrap();
            assert!(safe_save_map(&d.0, &mut s).is_err());
            assert_eq!(fs::read(stale).unwrap(), b"precious");
            assert!(!d.0.join("boden.bmp").exists());
        }
    }
    #[test]
    fn failed_new_map_save_restores_absent_layers_and_raw_state() {
        let d = Dir::new();
        let mut s = TerrainSession::create_blank(5, 25);
        edit(&mut s);
        assert!(save_using(&d.0, &mut s, |i, from, to| if i == 1 {
            Err(io::Error::other("second publish"))
        } else {
            fs::rename(from, to)
        })
        .is_err());
        assert!(!d.0.join("boden.bmp").exists());
        assert!(!d.0.join("collision.bmp").exists());
        assert!(s.raw_boden_bmp.is_none());
        assert!(s.raw_collision_bmp.is_none());
        assert!(s.is_dirty());
        assert!(s.can_undo());
        safe_save_map(&d.0, &mut s).unwrap();
        assert_eq!(load_map(&d.0).unwrap().heights()[0], 99);
    }
    #[test]
    #[cfg(windows)]
    fn guard_resolves_junctions_before_testing_components() {
        let d = Dir::new();
        let installed = d.0.join("Program Files (x86)").join("Against Rome");
        fs::create_dir_all(&installed).unwrap();
        let link = d.0.join("workspace");
        let output = std::process::Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&installed)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(is_forbidden_install_path(
            &link.join("MAPS").join("new-map")
        ));
        assert!(is_forbidden_install_path(
            &link.join("..").join("Against Rome").join("new-map")
        ));
        fs::remove_dir(link).unwrap();
    }
    #[test]
    fn bmp_metadata_colored_collision_and_padding_roundtrip() {
        let d = Dir::new();
        let mut h = BmpImage::new_solid(5, 5, 13, 25, 91).encode_bmp();
        let mut c = BmpImage::new_solid(4, 4, 7, 24, 80).encode_bmp();
        h[6] = 37;
        h[38] = 83;
        c[6] = 61;
        c[42] = 92;
        h.extend_from_slice(b"height trailer");
        c.extend_from_slice(b"collision trailer");
        h[54 + 15] = 0xa5;
        fs::write(d.0.join("boden.bmp"), &h).unwrap();
        fs::write(d.0.join("collision.bmp"), &c).unwrap();
        let mut s = load_map(&d.0).unwrap();
        safe_save_map(&d.0, &mut s).unwrap();
        assert_eq!(fs::read(d.0.join("boden.bmp")).unwrap(), h);
        assert_eq!(fs::read(d.0.join("collision.bmp")).unwrap(), c);
        edit(&mut s);
        safe_save_map(&d.0, &mut s).unwrap();
        let mut expected = h;
        expected[54 + 4 * 16 + 1] = 99;
        assert_eq!(fs::read(d.0.join("boden.bmp")).unwrap(), expected);
        assert_eq!(fs::read(d.0.join("collision.bmp")).unwrap(), c);
    }
}

/// 編輯器自己的資料目錄，與共用設定位於相同產品根目錄，禁止使用地圖目錄。
pub fn editor_data_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("AgainstRomeModifier").join("editor"))
}

#[cfg(test)]
mod temp_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/temp_unique.rs"));
}
