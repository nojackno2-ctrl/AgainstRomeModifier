//! Parser and byte-preserving document editor for Against Rome `boden.txt` texture grids.
//!
//! Directly mirrors the contract of C# `BodenTexturesDocument` while guaranteeing
//! zero-loss preservation of comments, formatting, line endings, PFIL compression, and unknown tokens.

use crate::error::TextureError;
use crate::lzss::{compress_pfil, decompress_if_pfil, is_pfil};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureEntry {
    pub start: usize,
    pub end: usize,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct BodenTexturesDocument {
    raw_text: String,
    dimension: usize,
    entries: Vec<TextureEntry>,
    use_crlf: bool,
    pfil_header: Option<Vec<u8>>,
    original_bytes: Option<Vec<u8>>,
    loaded_path: Option<PathBuf>,
    modified: bool,
}

impl BodenTexturesDocument {
    /// Loads a `boden.txt` document from a file path.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, TextureError> {
        let p = path.as_ref();
        let bytes = fs::read(p)?;
        let mut doc = Self::from_bytes(&bytes)?;
        doc.loaded_path = Some(p.to_path_buf());
        Ok(doc)
    }

    /// Parses a `boden.txt` document from raw bytes (supporting PFIL wrapped, UTF-8, or CP1251).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TextureError> {
        if bytes.starts_with(b"PFIL") && bytes.len() < crate::lzss::PFIL_HEADER_LEN {
            return Err(TextureError::InvalidFormat(format!(
                "PFIL header truncated: expected at least {} bytes, got {}",
                crate::lzss::PFIL_HEADER_LEN,
                bytes.len()
            )));
        }

        let pfil_header = if is_pfil(bytes) {
            Some(bytes[..crate::lzss::PFIL_HEADER_LEN].to_vec())
        } else {
            None
        };

        let decompressed = decompress_if_pfil(bytes)?;
        let text = match std::str::from_utf8(&decompressed) {
            Ok(s) => s.to_string(),
            Err(_) => {
                let (cow, _, _) = encoding_rs::WINDOWS_1251.decode(&decompressed);
                cow.into_owned()
            }
        };

        let mut doc = Self::parse(text)?;
        doc.pfil_header = pfil_header;
        doc.original_bytes = Some(bytes.to_vec());
        doc.loaded_path = None;
        doc.modified = false;
        Ok(doc)
    }

    /// Parses a `boden.txt` document from a string.
    pub fn from_str(text: &str) -> Result<Self, TextureError> {
        Self::parse(text.to_string())
    }

    fn parse(raw_text: String) -> Result<Self, TextureError> {
        let use_crlf = raw_text.contains("\r\n");

        // Parse [Dimension]
        let dimension = Self::extract_dimension(&raw_text)?;

        // Parse [Texturen] entries
        let entries = Self::extract_entries(&raw_text)?;

        let expected_count = dimension * dimension;
        if entries.len() != expected_count {
            return Err(TextureError::DimensionMismatch {
                expected: expected_count,
                actual: entries.len(),
            });
        }

        Ok(Self {
            raw_text,
            dimension,
            entries,
            use_crlf,
            pfil_header: None,
            original_bytes: None,
            loaded_path: None,
            modified: false,
        })
    }

    fn extract_dimension(text: &str) -> Result<usize, TextureError> {
        let mut lines = text.lines();
        while let Some(line) = lines.next() {
            let trimmed = line.trim();
            if trimmed.eq_ignore_ascii_case("[dimension]") {
                if let Some(val_line) = lines.next() {
                    let val_str = val_line.trim();
                    if let Ok(dim) = val_str.parse::<usize>() {
                        if dim > 0 {
                            return Ok(dim);
                        }
                    }
                }
            }
        }
        Err(TextureError::InvalidFormat(
            "boden.txt 缺少有效的 [Dimension]".to_string(),
        ))
    }

    fn extract_entries(text: &str) -> Result<Vec<TextureEntry>, TextureError> {
        // Find [Texturen] case-insensitively
        let lower = text.to_lowercase();
        let section_tag = "[texturen]";
        let tag_pos = lower
            .find(section_tag)
            .ok_or_else(|| TextureError::InvalidFormat("boden.txt 缺少 [Texturen]".to_string()))?;

        // Find the newline immediately following [Texturen]
        let mut start_pos = tag_pos + section_tag.len();
        if let Some(nl) = text[start_pos..].find('\n') {
            start_pos += nl + 1;
        }

        let mut entries = Vec::new();
        let mut position = start_pos;
        let bytes = text.as_bytes();
        let len = bytes.len();

        while position < len {
            let next_nl = match text[position..].find('\n') {
                Some(idx) => position + idx,
                None => len,
            };

            let mut value_start = position;
            let mut value_end = next_nl;

            // Trim carriage return
            if value_end > value_start && bytes[value_end - 1] == b'\r' {
                value_end -= 1;
            }

            // Trim leading ASCII whitespace
            while value_start < value_end && (bytes[value_start] == b' ' || bytes[value_start] == b'\t') {
                value_start += 1;
            }

            // Trim trailing ASCII whitespace
            while value_end > value_start && (bytes[value_end - 1] == b' ' || bytes[value_end - 1] == b'\t') {
                value_end -= 1;
            }

            if value_start < value_end {
                // If it starts a new section, stop parsing textures
                if bytes[value_start] == b'[' {
                    break;
                }
                // Skip comment lines starting with ';'
                if bytes[value_start] != b';' {
                    entries.push(TextureEntry {
                        start: value_start,
                        end: value_end,
                        value: text[value_start..value_end].to_string(),
                    });
                }
            }

            position = if next_nl == len { len } else { next_nl + 1 };
        }

        Ok(entries)
    }

    /// Whether the document originally used CRLF line endings.
    #[inline]
    pub fn is_crlf(&self) -> bool {
        self.use_crlf
    }

    /// Whether the document is wrapped in a PFIL container.
    #[inline]
    pub fn is_pfil(&self) -> bool {
        self.pfil_header.is_some()
    }

    /// The grid dimension (e.g. 64 for 64x64 tiles).
    #[inline]
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// Total tile count (`dimension * dimension`).
    #[inline]
    pub fn tile_count(&self) -> usize {
        self.dimension * self.dimension
    }

    /// Returns a slice of all texture names in row-major order (y * dimension + x).
    pub fn textures(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.value.clone()).collect()
    }

    /// Gets the texture token at tile coordinates (x, y).
    pub fn get_texture(&self, x: usize, y: usize) -> Result<&str, TextureError> {
        if x >= self.dimension || y >= self.dimension {
            return Err(TextureError::OutOfRange {
                x,
                y,
                dimension: self.dimension,
            });
        }
        let index = y * self.dimension + x;
        Ok(&self.entries[index].value)
    }

    /// Updates the single texture at tile coordinates (x, y).
    pub fn set_texture(&mut self, x: usize, y: usize, texture: &str) -> Result<(), TextureError> {
        if x >= self.dimension || y >= self.dimension {
            return Err(TextureError::OutOfRange {
                x,
                y,
                dimension: self.dimension,
            });
        }
        let trimmed = texture.trim();
        if trimmed.is_empty() || trimmed.contains('\n') || trimmed.contains('\r') {
            return Err(TextureError::InvalidFormat(
                "材質名稱不可為空或包含換行。".to_string(),
            ));
        }

        let index = y * self.dimension + x;
        let entry = &self.entries[index];
        if entry.value == trimmed {
            return Ok(());
        }

        let before = &self.raw_text[..entry.start];
        let after = &self.raw_text[entry.end..];

        let mut new_text = String::with_capacity(before.len() + trimmed.len() + after.len());
        new_text.push_str(before);
        new_text.push_str(trimmed);
        new_text.push_str(after);

        let pfil_header = self.pfil_header.clone();
        let original_bytes = self.original_bytes.clone();
        let loaded_path = self.loaded_path.clone();

        *self = Self::parse(new_text)?;
        self.pfil_header = pfil_header;
        self.original_bytes = original_bytes;
        self.loaded_path = loaded_path;
        self.modified = true;
        Ok(())
    }

    /// Batch writes the entire grid of textures in a single string replacement pass.
    /// Preserves all text outside the texture tokens (headers, comments, section tags, blank lines).
    pub fn set_textures(&mut self, new_textures: &[String]) -> Result<(), TextureError> {
        if new_textures.len() != self.entries.len() {
            return Err(TextureError::DimensionMismatch {
                expected: self.entries.len(),
                actual: new_textures.len(),
            });
        }

        for tex in new_textures {
            let trimmed = tex.trim();
            if trimmed.is_empty() || trimmed.contains('\n') || trimmed.contains('\r') {
                return Err(TextureError::InvalidFormat(
                    "材質名稱不可為空或包含換行。".to_string(),
                ));
            }
        }

        // Check if any texture actually changed
        let unchanged = self
            .entries
            .iter()
            .zip(new_textures.iter())
            .all(|(entry, new_val)| entry.value == *new_val.trim());

        if unchanged {
            // Keep byte-for-byte exactness if no values changed
            return Ok(());
        }

        let mut builder = String::with_capacity(self.raw_text.len() + 64);
        let mut position = 0;

        for (entry, new_val) in self.entries.iter().zip(new_textures.iter()) {
            builder.push_str(&self.raw_text[position..entry.start]);
            builder.push_str(new_val.trim());
            position = entry.end;
        }

        builder.push_str(&self.raw_text[position..]);

        let pfil_header = self.pfil_header.clone();
        let original_bytes = self.original_bytes.clone();
        let loaded_path = self.loaded_path.clone();

        *self = Self::parse(builder)?;
        self.pfil_header = pfil_header;
        self.original_bytes = original_bytes;
        self.loaded_path = loaded_path;
        self.modified = true;
        Ok(())
    }

    /// Serializes the document back to bytes (preserving PFIL compression if originally compressed).
    pub fn to_bytes(&self) -> Vec<u8> {
        if !self.modified {
            if let Some(orig) = &self.original_bytes {
                return orig.clone();
            }
        }

        let (bytes, _, _) = encoding_rs::WINDOWS_1251.encode(&self.raw_text);
        let uncompressed = bytes.into_owned();

        if let Some(header) = &self.pfil_header {
            compress_pfil(&uncompressed, header).unwrap_or(uncompressed)
        } else {
            uncompressed
        }
    }

    /// Raw text content of the document.
    #[inline]
    pub fn text(&self) -> &str {
        &self.raw_text
    }

    /// Path from which the document was loaded, if any.
    #[inline]
    pub fn loaded_path(&self) -> Option<&Path> {
        self.loaded_path.as_deref()
    }

    /// Safely saves the document to a file with readonly preflight checks, external modification
    /// conflict detection, atomic temporary staging, and a `.bak` backup of any existing file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), TextureError> {
        self.save_internal(path.as_ref(), true)
    }

    /// Safely saves the document to a file without creating a `.bak` backup.
    pub fn save_without_backup(&self, path: impl AsRef<Path>) -> Result<(), TextureError> {
        self.save_internal(path.as_ref(), false)
    }

    fn save_internal(&self, target: &Path, create_backup: bool) -> Result<(), TextureError> {
        let new_bytes = self.to_bytes();

        // 1. Preflight checks on existing target
        if target.exists() {
            let metadata = fs::metadata(target)?;

            if !metadata.is_file() {
                return Err(TextureError::InvalidFormat(format!(
                    "Target is not a regular file: {}",
                    target.display()
                )));
            }

            if metadata.permissions().readonly() {
                return Err(TextureError::Io(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("Target file is read-only: {}", target.display()),
                )));
            }

            // If file already has the exact same content, skip rewrite
            let existing_bytes = fs::read(target)?;
            if existing_bytes == new_bytes {
                return Ok(());
            }

            // Conflict check: if saving back to loaded_path, ensure file hasn't changed externally
            if let Some(loaded) = &self.loaded_path {
                if loaded == target {
                    if let Some(orig) = &self.original_bytes {
                        if existing_bytes != *orig {
                            return Err(TextureError::InvalidFormat(format!(
                                "External file modification detected on: {}",
                                target.display()
                            )));
                        }
                    }
                }
            }
        }

        let dir = target.parent().unwrap_or_else(|| Path::new("."));
        if !dir.exists() {
            fs::create_dir_all(dir)?;
        }

        // 2. Stage file in the same directory for atomic rename
        static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seq = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let pid = std::process::id();
        let file_name = target
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("boden.txt");
        let stage_path = dir.join(format!(".{file_name}.stage-{pid}-{seq}.tmp"));

        fs::write(&stage_path, &new_bytes)?;

        // If target exists, preserve its permissions on the stage file
        if let Ok(meta) = fs::metadata(target) {
            let _ = fs::set_permissions(&stage_path, meta.permissions());
        }

        // 3. Backup creation & publishing
        if target.exists() {
            let existing_bytes = fs::read(target)?;
            let backup_path = dir.join(format!("{file_name}.bak"));

            if create_backup {
                if let Err(e) = fs::write(&backup_path, &existing_bytes) {
                    let _ = fs::remove_file(&stage_path);
                    return Err(TextureError::Io(e));
                }
            }

            // Replace target with stage_path
            if let Err(e) = fs::remove_file(target) {
                let _ = fs::remove_file(&stage_path);
                return Err(TextureError::Io(e));
            }

            if let Err(e) = fs::rename(&stage_path, target) {
                // Rollback from existing_bytes
                let _ = fs::write(target, &existing_bytes);
                let _ = fs::remove_file(&stage_path);
                return Err(TextureError::Io(e));
            }
        } else {
            // Target does not exist, simply rename stage to target
            if let Err(e) = fs::rename(&stage_path, target) {
                let _ = fs::remove_file(&stage_path);
                return Err(TextureError::Io(e));
            }
        }

        Ok(())
    }
}
