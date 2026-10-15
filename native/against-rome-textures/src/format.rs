//! Format analysis and reality contract for Against Rome terrain textures.
//!
//! # Format Truth Boundary (真格式覆蓋界線)
//!
//! 1. **Map Terrain Texture Grid (`boden.txt`)**:
//!    Each map directory (e.g. `MAPS/TUTOR_01/boden.txt`, `MAPS/ENDL_000/boden.txt`) contains
//!    a text document defining the map dimensions and texture tile names:
//!    ```text
//!    [Dimension]
//!    64
//!    [Texturen]
//!    4BB___50
//!    4BB___50
//!    ...
//!    ```
//!    In retail maps, the dimension is typically 64 (or 48 in tutorial maps), yielding
//!    `Dimension * Dimension` texture entries (e.g. 4096 or 2304 lines).
//!    The file may include header comments, arbitrary indentation, and Windows CRLF (`\r\n`)
//!    or Unix LF (`\n`) line endings.
//!
//! 2. **Global Terrain Texture Archive (`floortex.dat`)**:
//!    Located in the game root directory (`<GamePath>/floortex.dat`), this file is a standard
//!    ZIP archive containing 3,005 entries of 128x128 24-bit BMP floor textures matching the
//!    texture tokens specified in `boden.txt` (e.g. `4BB___50.bmp`, `4UBC__11.bmp`).
//!
//! 3. **Non-existence of `textures.dat` / `texture.dat`**:
//!    Reverse engineering analysis of `Against_Rome.exe` and all installed retail maps
//!    confirms that neither `textures.dat` nor `texture.dat` exists in Against Rome.
//!    The executable references `floortex.dat` and `boden.txt` / `Texturen`.
//!    The only `*tex*.dat` in the game's `MAPS/*/DATA/` subdirectories is `hitex.dat`,
//!    which is a 40,008-byte pool for particle projectile hit effects (`0x48e4d0`),
//!    not terrain texturing.
//!    Consequently, any request to read or write map textures operates on `boden.txt`.
//!    When paths pointing to `textures.dat` or directories are received, our parser
//!    gracefully resolves the actual `boden.txt` document.

use std::path::{Path, PathBuf};

/// Identifies whether a given path is an Against Rome terrain texture file or directory,
/// resolving the authoritative file on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextureResourceKind {
    /// Authoritative map texture grid document (`boden.txt`).
    BodenText(PathBuf),
    /// Global floor texture ZIP archive (`floortex.dat`).
    FloorTextureArchive(PathBuf),
    /// Hypothetical or alias `textures.dat` / `texture.dat` path redirected to `boden.txt` if adjacent.
    AliasRedirected { original: PathBuf, target: PathBuf },
    /// Unknown or unsupported file.
    Unknown(PathBuf),
}

impl TextureResourceKind {
    /// Detects and resolves the resource kind from a path.
    pub fn resolve(path: impl AsRef<Path>) -> Self {
        let p = path.as_ref();
        if p.is_dir() {
            let boden = p.join("boden.txt");
            if boden.is_file() {
                return Self::BodenText(boden);
            }
            let floortex = p.join("floortex.dat");
            if floortex.is_file() {
                return Self::FloorTextureArchive(floortex);
            }
        }

        let file_name = p.file_name().and_then(|f| f.to_str()).unwrap_or("");
        if file_name.eq_ignore_ascii_case("boden.txt") {
            Self::BodenText(p.to_path_buf())
        } else if file_name.eq_ignore_ascii_case("floortex.dat") {
            Self::FloorTextureArchive(p.to_path_buf())
        } else if file_name.eq_ignore_ascii_case("textures.dat")
            || file_name.eq_ignore_ascii_case("texture.dat")
        {
            // If user or tool specifies textures.dat, check if boden.txt exists in the same directory
            if let Some(parent) = p.parent() {
                let boden = parent.join("boden.txt");
                if boden.is_file() {
                    return Self::AliasRedirected {
                        original: p.to_path_buf(),
                        target: boden,
                    };
                }
            }
            Self::Unknown(p.to_path_buf())
        } else {
            Self::Unknown(p.to_path_buf())
        }
    }
}
