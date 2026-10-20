//! Synthetic-data helpers for the unit tests (no game files involved).

use crate::util::COLUMN_WIDTHS;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ar-re-tools-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn u32s(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// Wraps `payload` in a literal-only `PFIL@` container (store mode: 0xFF flag + 8 literals).
pub fn pfil_store(payload: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; 64];
    out[..5].copy_from_slice(b"PFIL@");
    out[16..20].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    for chunk in payload.chunks(8) {
        out.push(0xff >> (8 - chunk.len()));
        out.extend_from_slice(chunk);
    }
    out
}

/// A zeroed `objects.dat` with `count` slots.
pub fn objects(count: usize) -> Vec<u8> {
    let mut data = u32s(&[1, count as u32, 30, 30]);
    data.resize(16 + count * (79 + COLUMN_WIDTHS.iter().sum::<usize>()), 0);
    data
}

pub fn obj_activate(data: &mut [u8], slot: usize, type_id: u16) {
    data[16 + slot * 79] = 1;
    data[16 + slot * 79 + 75..16 + slot * 79 + 77].copy_from_slice(&type_id.to_le_bytes());
}

pub fn obj_column(data: &mut [u8], count: usize, column: usize, slot: usize, value: u16) {
    let offset = 16 + count * (79 + COLUMN_WIDTHS[..column].iter().sum::<usize>()) + slot * 2;
    data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

/// Fills every u16 of a column with 0xffff ("no link").
pub fn obj_unlink_all(data: &mut [u8], count: usize, column: usize) {
    for slot in 0..count {
        obj_column(data, count, column, slot, 0xffff);
    }
}

pub fn obj_record_i16(data: &mut [u8], slot: usize, offset: usize, value: i16) {
    let at = 16 + slot * 79 + offset;
    data[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

pub fn write(dir: &Path, relative: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, bytes).unwrap();
    path
}
