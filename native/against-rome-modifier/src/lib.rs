//! Executable patch core ported from src.Core/Core/Patches/ExePatchModel.cs.
//! No installation discovery or implicit writes: callers supply all paths.
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

pub mod cl_script;
pub mod epara;
pub mod exe_patches;
pub mod features;
pub mod lzss;
pub mod objdef;
pub mod patch_text;
pub mod partgeo;
pub mod ress;
pub mod team_dat;
pub mod troops;
pub mod cli;
pub mod services;
pub mod install;
pub mod detectors;
pub mod endless_ai;
pub mod bci_features;
pub mod exe_features;

pub use cl_script::{ClScriptOptions, ClScriptPatcher};
pub use epara::{CleanEparaBaseline, EparaPatcher};
pub use exe_patches::{ALL_EXE_PATCHES, CIVI_PRODUCE_20, CORPSE_RETENTION};
pub use features::{
    FeatureCategory, FeatureControlKind, FeatureDefinition, FeatureRegistry, FeatureValue,
    PatchProfile,
};
pub use lzss::{compress_pfil, decompress_if_pfil, decompress_pfil, is_pfil};
pub use objdef::{ObjdefOptions, ObjdefPatcher};
pub use partgeo::{PartgeoOptions, PartgeoPatcher};
pub use patch_text::{decode_cp1251, encode_cp1251, parse_csv_line, to_csv_string, TextDocument};
pub use ress::{RessOptions, RessPatcher};
pub use team_dat::{TeamDatOptions, TeamDatPatcher, DEFAULT_POPULATION_LIMIT};
pub use troops::{ObjdefIndex, TroopConfig, UnitMetadata, UnitStatParser};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Original,
    Patched,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub struct Patch {
    pub name: &'static str,
    pub offset: usize,
    pub original: &'static [u8],
    pub replacement: &'static [u8],
}

pub const FOCUS: Patch = Patch {
    name: "focus",
    offset: 0x161a88,
    original: &[0x89, 0x15, 0xc4, 0x7d, 0x9e, 0x02],
    replacement: &[0x90; 6],
};
pub const ROMAN_ENDLESS: Patch = Patch {
    name: "roman-endless",
    offset: 0x5bd60,
    original: &[
        0x53, 0x8b, 0x5c, 0x24, 0x08, 0x53, 0xe8, 0x25, 0x3b, 0xfe, 0xff, 0x83, 0xc4, 0x04, 0x53,
        0x89, 0x1d, 0x78, 0x74, 0x73, 0x00,
    ],
    replacement: &[
        0x53, 0x6a, 0x03, 0x5b, 0x90, 0x53, 0xe8, 0x25, 0x3b, 0xfe, 0xff, 0x83, 0xc4, 0x04, 0x53,
        0x89, 0x1d, 0x78, 0x74, 0x73, 0x00,
    ],
};
pub const UNIT_RECRUIT_20: Patch = Patch {
    name: "unit-recruit-20",
    offset: 0x4c7dd,
    original: &[0x29, 0xfd, 0x01, 0xfe],
    replacement: &[0x6a, 0x14, 0x5e, 0x90],
};

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

impl Patch {
    pub fn detect(&self, bytes: &[u8]) -> State {
        let Some(end) = self.offset.checked_add(self.original.len()) else {
            return State::Unknown;
        };
        if self.original.is_empty() || self.original.len() != self.replacement.len() {
            return State::Unknown;
        }
        match bytes.get(self.offset..end) {
            Some(b) if b == self.original => State::Original,
            Some(b) if b == self.replacement => State::Patched,
            _ => State::Unknown,
        }
    }
}

/// Validate the ENTIRE plan before producing any output. Overlapping ranges are
/// rejected, including idempotent selections, so order can never change results.
pub fn plan(bytes: &[u8], selections: &[(Patch, bool)]) -> io::Result<Vec<u8>> {
    let mut ranges = Vec::new();
    for (patch, _) in selections {
        if patch.detect(bytes) == State::Unknown {
            return Err(invalid(format!(
                "{}: unsupported bytes or range",
                patch.name
            )));
        }
        let end = patch
            .offset
            .checked_add(patch.original.len())
            .ok_or_else(|| invalid("offset overflow"))?;
        if ranges
            .iter()
            .any(|&(start, stop)| patch.offset < stop && start < end)
        {
            return Err(invalid("overlapping patch ranges"));
        }
        ranges.push((patch.offset, end));
    }
    let mut output = bytes.to_vec();
    for (patch, enabled) in selections {
        let value = if *enabled {
            patch.replacement
        } else {
            patch.original
        };
        output[patch.offset..patch.offset + value.len()].copy_from_slice(value);
    }
    Ok(output)
}

struct Cleanup(PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
struct LockGuard {
    file: Option<fs::File>,
    path: PathBuf,
}
impl Drop for LockGuard {
    fn drop(&mut self) {
        drop(self.file.take());
        let _ = fs::remove_file(&self.path);
    }
}

fn create_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(error);
    }
    Ok(())
}

/// Atomic single-file transaction. The backup must be a new caller-chosen path;
/// it contains the exact pre-transaction bytes and is never overwritten. A
/// sibling lock excludes cooperating writers. External writers must be stopped
/// by the caller (e.g. close the game); this is not a filesystem-wide lock.
/// On replacement failure, verify and restore source bytes from the exclusive
/// backup. If recovery fails, preserve the replacement and report every path.
pub fn apply_file(path: &Path, backup: &Path, selections: &[(Patch, bool)]) -> io::Result<bool> {
    apply_file_using(path, backup, selections, replace)
}

fn apply_file_using(
    path: &Path,
    backup: &Path,
    selections: &[(Patch, bool)],
    commit: impl FnOnce(&Path, &Path) -> io::Result<()>,
) -> io::Result<bool> {
    let name = path
        .file_name()
        .ok_or_else(|| invalid("missing filename"))?;
    let mut lock_name = name.to_os_string();
    lock_name.push(".arm-rust-lock");
    let lock_path = path.with_file_name(lock_name);
    let lock = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)?;
    let _lock_cleanup = LockGuard {
        file: Some(lock),
        path: lock_path,
    };
    let original = fs::read(path)?;
    let output = plan(&original, selections)?;
    if output == original {
        return Ok(false);
    }
    // A fixed sibling temp name also rejects stale interrupted transactions.
    let mut temp_name = name.to_os_string();
    temp_name.push(".arm-rust-new");
    let temp = path.with_file_name(temp_name);
    create_synced(&temp, &output)?;
    let temp_cleanup = Cleanup(temp.clone());
    fs::set_permissions(&temp, fs::metadata(path)?.permissions())?;
    create_synced(backup, &original)?;
    if fs::read(path)? != original {
        return Err(invalid(
            "source changed during transaction; backup retained",
        ));
    }
    if let Err(error) = commit(path, &temp) {
        // ReplaceFileW can fail after removing/renaming the original (1176/1177).
        // A failure is therefore not evidence that the target still exists.
        if fs::read(path).is_ok_and(|current| current == original) {
            return Err(error);
        }
        let recovery = restore_original(path, backup, &original);
        if let Err(recovery_error) = recovery {
            // Keep all remaining evidence, including a replacement that the OS
            // may have partially moved or merged. Never delete an unknown target.
            std::mem::forget(temp_cleanup);
            return Err(io::Error::new(error.kind(), format!(
                "replacement failed: {error}; recovery failed: {recovery_error}; target={}, backup={}, replacement={} (preserved if present)",
                path.display(), backup.display(), temp.display()
            )));
        }
        return Err(io::Error::new(
            error.kind(),
            format!(
                "replacement failed: {error}; original bytes recovered from {}",
                backup.display()
            ),
        ));
    }
    Ok(true)
}

fn restore_original(path: &Path, backup: &Path, original: &[u8]) -> io::Result<()> {
    if fs::read(backup)? != original {
        return Err(invalid("recovery backup no longer matches original bytes"));
    }
    // Do not overwrite an unexpected file: another writer or partial OS state
    // needs manual inspection. Missing targets can be recreated exclusively.
    if path.try_exists()? {
        return Err(invalid(
            "target contains unexpected bytes; manual recovery required",
        ));
    }
    create_synced(path, original)?;
    if fs::read(path)? != original {
        return Err(invalid("recovered target verification failed"));
    }
    Ok(())
}

#[cfg(windows)]
fn replace(path: &Path, replacement: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn ReplaceFileW(
            replaced: *const u16,
            replacement: *const u16,
            backup: *const u16,
            flags: u32,
            exclude: *mut core::ffi::c_void,
            reserved: *mut core::ffi::c_void,
        ) -> i32;
    }
    let wide = |p: &Path| -> io::Result<Vec<u16>> {
        let mut value: Vec<u16> = p.as_os_str().encode_wide().collect();
        if value.contains(&0) {
            return Err(invalid("path contains NUL"));
        }
        value.push(0);
        Ok(value)
    };
    let p = wide(path)?;
    let r = wide(replacement)?;
    // Both buffers remain alive throughout this synchronous Win32 call.
    if unsafe {
        ReplaceFileW(
            p.as_ptr(),
            r.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } == 0
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
#[cfg(not(windows))]
fn replace(path: &Path, replacement: &Path) -> io::Result<()> {
    fs::rename(replacement, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    fn fixture() -> Vec<u8> {
        let mut bytes = vec![0x5a; FOCUS.offset + FOCUS.original.len() + 12];
        for p in [FOCUS, ROMAN_ENDLESS, UNIT_RECRUIT_20] {
            bytes[p.offset..p.offset + p.original.len()].copy_from_slice(p.original);
        }
        bytes
    }
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "arm-rust-patch-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn real_definitions_roundtrip_and_preserve_unselected_bytes() {
        let original = fixture();
        let patches = [FOCUS, ROMAN_ENDLESS, UNIT_RECRUIT_20];
        let selections: Vec<_> = patches.iter().map(|p| (*p, true)).collect();
        let patched = plan(&original, &selections).unwrap();
        for p in patches {
            assert_eq!(p.detect(&patched), State::Patched);
        }
        assert_eq!(plan(&patched, &selections).unwrap(), patched);
        let reverse: Vec<_> = patches.iter().map(|p| (*p, false)).collect();
        assert_eq!(plan(&patched, &reverse).unwrap(), original);
        for i in 0..original.len() {
            if !patches
                .iter()
                .any(|p| (p.offset..p.offset + p.original.len()).contains(&i))
            {
                assert_eq!(original[i], patched[i]);
            }
        }
    }
    #[test]
    fn rejects_unknown_truncated_overlap_and_overflow() {
        let mut bytes = fixture();
        bytes[FOCUS.offset] = 0;
        assert!(plan(&bytes, &[(UNIT_RECRUIT_20, true), (FOCUS, true)]).is_err());
        assert!(plan(&[], &[(FOCUS, true)]).is_err());
        assert!(plan(&fixture(), &[(FOCUS, true), (FOCUS, false)]).is_err());
        let overflow = Patch {
            offset: usize::MAX,
            ..FOCUS
        };
        assert_eq!(overflow.detect(&bytes), State::Unknown);
    }
    #[test]
    fn disk_transaction_backup_disable_and_noop() {
        let d = Directory::new();
        let p = d.0.join("game.exe");
        let b = d.0.join("before.bin");
        let original = fixture();
        fs::write(&p, &original).unwrap();
        assert!(apply_file(&p, &b, &[(FOCUS, true), (ROMAN_ENDLESS, true)]).unwrap());
        assert_eq!(fs::read(&b).unwrap(), original);
        assert!(!apply_file(&p, &b, &[(FOCUS, true), (ROMAN_ENDLESS, true)]).unwrap());
        assert!(apply_file(
            &p,
            &d.0.join("patched.bin"),
            &[(FOCUS, false), (ROMAN_ENDLESS, false)]
        )
        .unwrap());
        assert_eq!(fs::read(&p).unwrap(), original);
        assert_eq!(fs::read(&b).unwrap(), original);
        assert!(!d.0.join("game.exe.arm-rust-lock").exists());
    }
    #[test]
    fn existing_backup_and_unknown_source_never_mutate_source() {
        let d = Directory::new();
        let p = d.0.join("game.exe");
        let b = d.0.join("before.bin");
        let original = fixture();
        fs::write(&p, &original).unwrap();
        fs::write(&b, b"precious backup").unwrap();
        assert!(apply_file(&p, &b, &[(FOCUS, true)]).is_err());
        assert_eq!(fs::read(&p).unwrap(), original);
        assert_eq!(fs::read(&b).unwrap(), b"precious backup");
        fs::write(&p, b"unsupported executable").unwrap();
        assert!(apply_file(&p, &d.0.join("unknown.bin"), &[(FOCUS, true)]).is_err());
        assert!(!d.0.join("unknown.bin").exists());
        assert!(!d.0.join("game.exe.arm-rust-new").exists());
        assert!(!d.0.join("game.exe.arm-rust-lock").exists());
    }
    #[test]
    fn stale_lock_and_temp_fail_closed() {
        let d = Directory::new();
        let p = d.0.join("game.exe");
        let b = d.0.join("before.bin");
        let original = fixture();
        fs::write(&p, &original).unwrap();
        let lock = d.0.join("game.exe.arm-rust-lock");
        fs::write(&lock, b"other writer").unwrap();
        assert!(apply_file(&p, &b, &[(FOCUS, true)]).is_err());
        assert_eq!(fs::read(&lock).unwrap(), b"other writer");
        fs::remove_file(lock).unwrap();
        let temp = d.0.join("game.exe.arm-rust-new");
        fs::write(&temp, b"recovery data").unwrap();
        assert!(apply_file(&p, &b, &[(FOCUS, true)]).is_err());
        assert_eq!(fs::read(&temp).unwrap(), b"recovery data");
        assert_eq!(fs::read(&p).unwrap(), original);
        assert!(!b.exists());
    }

    #[test]
    fn failed_replacement_keeps_original_and_recovery_backup() {
        let d = Directory::new();
        let p = d.0.join("game.exe");
        let b = d.0.join("before.bin");
        let original = fixture();
        fs::write(&p, &original).unwrap();
        let result = apply_file_using(&p, &b, &[(FOCUS, true)], |_, temp| {
            assert_eq!(FOCUS.detect(&fs::read(temp)?), State::Patched);
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "injected replacement failure",
            ))
        });
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(fs::read(&p).unwrap(), original);
        assert_eq!(fs::read(&b).unwrap(), original);
        assert!(!d.0.join("game.exe.arm-rust-lock").exists());
        assert!(!d.0.join("game.exe.arm-rust-new").exists());
    }

    #[test]
    fn win32_1176_after_original_removed_recovers_from_backup() {
        let d = Directory::new();
        let p = d.0.join("game.exe");
        let b = d.0.join("original.bin");
        let original = fixture();
        fs::write(&p, &original).unwrap();
        let error = apply_file_using(&p, &b, &[(FOCUS, true)], |path, temp| {
            assert_eq!(FOCUS.detect(&fs::read(temp)?), State::Patched);
            fs::remove_file(path)?;
            Err(io::Error::from_raw_os_error(1176))
        })
        .unwrap_err();
        assert!(error.to_string().contains("original bytes recovered"));
        assert_eq!(fs::read(&p).unwrap(), original);
        assert_eq!(fs::read(&b).unwrap(), original);
        assert!(!d.0.join("game.exe.arm-rust-new").exists());
        assert!(!d.0.join("game.exe.arm-rust-lock").exists());
    }

    #[test]
    fn win32_1177_after_original_renamed_recovers_and_keeps_os_evidence() {
        let d = Directory::new();
        let p = d.0.join("game.exe");
        let b = d.0.join("original.bin");
        let displaced = d.0.join("os-renamed-original");
        let original = fixture();
        fs::write(&p, &original).unwrap();
        let error = apply_file_using(&p, &b, &[(FOCUS, true)], |path, _| {
            fs::rename(path, &displaced)?;
            Err(io::Error::from_raw_os_error(1177))
        })
        .unwrap_err();
        assert!(error.to_string().contains("original bytes recovered"));
        assert_eq!(fs::read(&p).unwrap(), original);
        assert_eq!(fs::read(&displaced).unwrap(), original);
        assert_eq!(fs::read(&b).unwrap(), original);
    }

    #[test]
    fn partial_failure_with_unknown_target_preserves_all_evidence() {
        let d = Directory::new();
        let p = d.0.join("game.exe");
        let b = d.0.join("original.bin");
        let original = fixture();
        fs::write(&p, &original).unwrap();
        let error = apply_file_using(&p, &b, &[(FOCUS, true)], |path, _| {
            fs::write(path, b"unexpected partial result")?;
            Err(io::Error::from_raw_os_error(1177))
        })
        .unwrap_err();
        assert!(error.to_string().contains("manual recovery required"));
        assert_eq!(fs::read(&p).unwrap(), b"unexpected partial result");
        assert_eq!(fs::read(&b).unwrap(), original);
        assert_eq!(
            FOCUS.detect(&fs::read(d.0.join("game.exe.arm-rust-new")).unwrap()),
            State::Patched
        );
        assert!(!d.0.join("game.exe.arm-rust-lock").exists());
    }

    #[test]
    fn backup_aliases_target_temp_and_lock_fail_closed() {
        for backup_name in [
            "game.exe",
            "game.exe.arm-rust-new",
            "game.exe.arm-rust-lock",
        ] {
            let d = Directory::new();
            let p = d.0.join("game.exe");
            let original = fixture();
            fs::write(&p, &original).unwrap();
            assert!(apply_file(&p, &d.0.join(backup_name), &[(FOCUS, true)]).is_err());
            assert_eq!(fs::read(&p).unwrap(), original);
            assert!(!d.0.join("game.exe.arm-rust-lock").exists());
            assert!(!d.0.join("game.exe.arm-rust-new").exists());
        }
    }

    #[test]
    fn changed_backup_is_not_used_for_recovery() {
        let d = Directory::new();
        let p = d.0.join("game.exe");
        let b = d.0.join("original.bin");
        fs::write(&p, fixture()).unwrap();
        let error = apply_file_using(&p, &b, &[(FOCUS, true)], |path, _| {
            fs::remove_file(path)?;
            fs::write(&b, b"externally changed backup")?;
            Err(io::Error::from_raw_os_error(1176))
        })
        .unwrap_err();
        assert!(error.to_string().contains("backup no longer matches"));
        assert!(!p.exists());
        assert_eq!(fs::read(&b).unwrap(), b"externally changed backup");
        assert_eq!(
            FOCUS.detect(&fs::read(d.0.join("game.exe.arm-rust-new")).unwrap()),
            State::Patched
        );
    }
}
