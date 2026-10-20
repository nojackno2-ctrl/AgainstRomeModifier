//! Editable PFIL-wrapped BCI script. Port of `BciScriptFile`
//! (src.Core/Core/Bci/BciScriptFile.cs) with fail-closed loading, verified recompression and
//! atomic pattern patches. The compressed form is literal-only LZSS (as in `compile_script`):
//! deterministic and loadable by the game, favouring safety over size.
use crate::bci::BciImage;
use crate::bci_pattern::{self, WordPattern};
use crate::Result;
use against_rome_data::lzss::{decompress_pfil, MAX_DECODED_LEN, PFIL_HEADER_LEN};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct BciScriptFile {
    path: PathBuf,
    raw: Vec<u8>,
    decompressed: Vec<u8>,
    modified: bool,
}

fn unpack(raw: &[u8]) -> Result<Vec<u8>> {
    decompress_pfil(raw, None).map_err(|e| e.to_string())
}

/// Rebuilds a PFIL file from `plain`, keeping the 64-byte header of `template` (size field updated).
pub fn compress_pfil_literal(plain: &[u8], template: &[u8]) -> Result<Vec<u8>> {
    if plain.len() > MAX_DECODED_LEN {
        return Err("decompressed BCI exceeds PFIL size limit".into());
    }
    let header = template
        .get(..PFIL_HEADER_LEN)
        .filter(|h| h.starts_with(b"PFIL"))
        .ok_or("PFIL header template required")?;
    let mut out = header.to_vec();
    out[16..20].copy_from_slice(&(plain.len() as u32).to_le_bytes());
    out.reserve(plain.len() + plain.len() / 8 + 8);
    for chunk in plain.chunks(8) {
        out.push(((1u16 << chunk.len()) - 1) as u8);
        out.extend(chunk);
    }
    Ok(out)
}

impl BciScriptFile {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let raw = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_bytes(path, raw)
    }
    pub fn from_bytes(path: impl Into<PathBuf>, raw: Vec<u8>) -> Result<Self> {
        let decompressed = unpack(&raw)?;
        Ok(Self {
            path: path.into(),
            raw,
            decompressed,
            modified: false,
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn raw_bytes(&self) -> &[u8] {
        &self.raw
    }
    pub fn decompressed_bytes(&self) -> &[u8] {
        &self.decompressed
    }
    pub fn is_modified(&self) -> bool {
        self.modified
    }
    /// Parses the current decompressed bytes as a structured BCI image.
    pub fn image(&self) -> Result<BciImage> {
        BciImage::parse(&self.decompressed)
    }
    /// Replaces the decompressed bytes. Rejects data that is not a valid BCI image.
    pub fn update_decompressed(&mut self, bytes: Vec<u8>) -> Result<()> {
        BciImage::parse(&bytes)?;
        self.decompressed = bytes;
        self.modified = true;
        Ok(())
    }
    /// Replaces the on-disk bytes; the state is unchanged when they do not decode.
    pub fn replace_raw(&mut self, raw: Vec<u8>) -> Result<()> {
        self.decompressed = unpack(&raw)?;
        self.raw = raw;
        self.modified = true;
        Ok(())
    }
    /// Current PFIL bytes. Pending edits are recompressed and verified to decode back to the
    /// edited bytes before they replace the retained raw copy.
    pub fn compressed_bytes(&mut self) -> Result<&[u8]> {
        if self.modified {
            let raw = compress_pfil_literal(&self.decompressed, &self.raw)?;
            if unpack(&raw)? != self.decompressed {
                return Err("PFIL recompression did not round-trip".into());
            }
            self.raw = raw;
            self.modified = false;
        }
        Ok(&self.raw)
    }
    /// Verified replacement of the unique pattern match; marks modified only if bytes change.
    pub fn replace_unique(
        &mut self,
        pattern: &WordPattern,
        replacement: &WordPattern,
        patch_name: &str,
    ) -> Result<usize> {
        let mut staged = self.decompressed.clone();
        let site = bci_pattern::replace_unique_word_pattern(
            &mut staged,
            pattern,
            replacement,
            patch_name,
        )?;
        self.commit(staged);
        Ok(site)
    }
    pub fn replace_all(
        &mut self,
        pattern: &WordPattern,
        replacement: &WordPattern,
        expected_count: usize,
        patch_name: &str,
    ) -> Result<Vec<usize>> {
        let mut staged = self.decompressed.clone();
        let sites = bci_pattern::replace_all_word_pattern_sites(
            &mut staged,
            pattern,
            replacement,
            expected_count,
            patch_name,
        )?;
        self.commit(staged);
        Ok(sites)
    }
    pub fn write_i32(
        &mut self,
        offset: usize,
        expected: i32,
        value: i32,
        patch_name: &str,
    ) -> Result<()> {
        let mut staged = self.decompressed.clone();
        bci_pattern::write_i32_verified(&mut staged, offset, expected, value, patch_name)?;
        self.commit(staged);
        Ok(())
    }
    fn commit(&mut self, staged: Vec<u8>) {
        if staged != self.decompressed {
            self.decompressed = staged;
            self.modified = true;
        }
    }
    /// Writes the current PFIL bytes to `target` through a temporary sibling file.
    pub fn save_as(&mut self, target: impl AsRef<Path>) -> Result<()> {
        let target = target.as_ref();
        let bytes = self.compressed_bytes()?.to_vec();
        let mut tmp = target.as_os_str().to_owned();
        tmp.push(".arm-tmp");
        let tmp = PathBuf::from(tmp);
        std::fs::write(&tmp, &bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, target).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("{}: {e}", target.display())
        })
    }
    pub fn save(&mut self) -> Result<()> {
        let path = self.path.clone();
        self.save_as(path)
    }
}
