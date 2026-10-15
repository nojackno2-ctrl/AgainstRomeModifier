use crate::{compile_script_verified, Result, ScenarioDocument, StartCamera};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Immutable source identity evidence. Re-read immediately before publishing an export.
/// Watches the entire objects.dat file, including inactive records and PFIL metadata.
pub struct DataBindingSource {
    path: PathBuf,
    original: Vec<u8>,
    active: Vec<(i32, u32)>,
}
impl DataBindingSource {
    pub fn load(map: &Path) -> Result<Self> {
        let path = map.join("DATA/objects.dat");
        let original = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let raw = if original.starts_with(b"PFIL") {
            against_rome_data::lzss::decompress_pfil(&original, None).map_err(|e| e.to_string())?
        } else {
            original.clone()
        };
        if raw.len() < 16 {
            return Err("truncated objects.dat header".into());
        }
        let word = |offset| u32::from_le_bytes(raw[offset..offset + 4].try_into().unwrap());
        let count = word(4) as usize;
        // Native objects: 79-byte records followed by 18 parallel columns.
        const COLUMNS: [usize; 18] = [2, 2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 2, 2, 2, 4, 2, 2, 4];
        if word(0) != 1
            || word(8) != 30
            || word(12) != 30
            || !(1..=14000).contains(&count)
            || raw.len() != 16 + count * (79 + COLUMNS.iter().sum::<usize>())
        {
            return Err("invalid objects.dat layout".into());
        }
        let mut active = vec![];
        for slot in 0..count {
            let offset = 16 + slot * 79;
            if raw[offset] != 0 {
                active.push((slot as i32, word(offset + 3)));
            }
        }
        let source = Self {
            path,
            original,
            active,
        };
        source.verify_unchanged()?;
        Ok(source)
    }
    pub fn verify_unchanged(&self) -> Result<()> {
        let current = fs::read(&self.path).map_err(|e| format!("{}: {e}", self.path.display()))?;
        if current != self.original {
            return Err("source DATA objects.dat changed; reload before export".into());
        }
        Ok(())
    }
    pub fn compile_script(
        &self,
        original: &[u8],
        document: &ScenarioDocument,
        camera: Option<&StartCamera>,
        aliases: Option<&[String]>,
    ) -> Result<Vec<u8>> {
        self.verify_unchanged()?;
        let bytes =
            compile_script_verified(original, document, camera, aliases, Some(&self.active))?;
        self.verify_unchanged()?;
        Ok(bytes)
    }
}
