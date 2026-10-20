use std::fmt;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafetyError {
    EmptyPath,
    NullByteInPath,
    PathTraversal(PathBuf),
    OutputAlreadyExists(PathBuf),
    InvalidAssetName(String),
}

impl fmt::Display for SafetyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SafetyError::EmptyPath => write!(f, "路徑不可為空。"),
            SafetyError::NullByteInPath => write!(f, "路徑不可包含空字元 (null byte)。"),
            SafetyError::PathTraversal(p) => {
                write!(f, "路徑包含非法路徑遍歷元件 (..)：{}", p.display())
            }
            SafetyError::OutputAlreadyExists(p) => {
                write!(f, "輸出目錄已存在，拒絕覆寫：{}", p.display())
            }
            SafetyError::InvalidAssetName(name) => {
                write!(f, "無效的資產名稱或包含目錄遍歷：{name}")
            }
        }
    }
}

impl std::error::Error for SafetyError {}

/// 驗證明確指定之輸入路徑的安全限制：不可為空、不可有空字元、不可越界逃逸。
pub fn validate_explicit_path(path: &Path) -> Result<PathBuf, SafetyError> {
    let s = path.to_string_lossy();
    if s.is_empty() {
        return Err(SafetyError::EmptyPath);
    }
    if s.contains('\0') {
        return Err(SafetyError::NullByteInPath);
    }
    for comp in path.components() {
        if comp == Component::ParentDir {
            return Err(SafetyError::PathTraversal(path.to_path_buf()));
        }
    }
    Ok(path.to_path_buf())
}

/// 驗證輸出目標必須為全新路徑（不可為已存在的檔案或資料夾）。
pub fn validate_new_output_dir(path: &Path) -> Result<PathBuf, SafetyError> {
    let clean = validate_explicit_path(path)?;
    if clean.exists() {
        return Err(SafetyError::OutputAlreadyExists(clean));
    }
    Ok(clean)
}

/// 驗證 ALR 資產名稱：不可為空、不可包含空字元或父層遍歷。
pub fn validate_asset_name(name: &str) -> Result<String, SafetyError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(SafetyError::InvalidAssetName(name.to_string()));
    }
    if trimmed.contains('\0') || trimmed.contains("..") {
        return Err(SafetyError::InvalidAssetName(name.to_string()));
    }
    Ok(trimmed.to_string())
}
