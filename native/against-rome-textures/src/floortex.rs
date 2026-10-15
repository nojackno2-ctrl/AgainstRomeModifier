//! Reader for `floortex.dat` (the global Against Rome terrain texture archive).

use crate::error::TextureError;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

/// Provides access to the floor texture bitmaps stored in `floortex.dat`.
pub struct FloortexLibrary {
    archive: Option<ZipArchive<File>>,
    names: Vec<String>,
    name_to_index: HashMap<String, usize>,
}

impl FloortexLibrary {
    /// Opens the `floortex.dat` ZIP archive from disk.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, TextureError> {
        let p = path.as_ref();
        if !p.exists() {
            return Err(TextureError::InvalidFormat(format!(
                "floortex.dat file does not exist: {}",
                p.display()
            )));
        }

        let file = File::open(p)?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| TextureError::InvalidFormat(format!("Failed to open ZIP archive: {}", e)))?;

        let mut names = Vec::new();
        let mut name_to_index = HashMap::new();

        for i in 0..archive.len() {
            if let Ok(entry) = archive.by_index(i) {
                let entry_name = entry.name().to_string();
                if entry_name.to_lowercase().ends_with(".bmp") {
                    let base_name = entry_name
                        .rsplit('/')
                        .next()
                        .unwrap_or(&entry_name)
                        .strip_suffix(".bmp")
                        .or_else(|| entry_name.strip_suffix(".BMP"))
                        .unwrap_or(&entry_name)
                        .to_string();

                    if !base_name.is_empty() {
                        let lower_key = base_name.to_lowercase();
                        if !name_to_index.contains_key(&lower_key) {
                            name_to_index.insert(lower_key, i);
                            names.push(base_name);
                        }
                    }
                }
            }
        }

        names.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));

        Ok(Self {
            archive: Some(archive),
            names,
            name_to_index,
        })
    }

    /// Sorted list of all available texture names.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Checks if a texture exists in the library (case-insensitive).
    pub fn contains(&self, name: &str) -> bool {
        self.name_to_index.contains_key(&name.to_lowercase())
    }

    /// Reads raw BMP bytes for a texture.
    pub fn get_bmp_bytes(&mut self, name: &str) -> Option<Vec<u8>> {
        let idx = *self.name_to_index.get(&name.to_lowercase())?;
        let archive = self.archive.as_mut()?;
        let mut entry = archive.by_index(idx).ok()?;
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes).ok()?;
        Some(bytes)
    }
}
