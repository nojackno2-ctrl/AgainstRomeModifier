//! 編輯器專用的版本化最近地圖清單，不寫入地圖目錄。
use std::path::{Path, PathBuf};

#[derive(Default)]
pub(crate) struct RecentMaps {
    pub paths: Vec<PathBuf>,
    file: Option<PathBuf>,
}

impl RecentMaps {
    pub fn load(file: PathBuf) -> Result<Self, String> {
        let mut result = Self { paths: Vec::new(), file: Some(file.clone()) };
        if !file.exists() { return Ok(result); }
        let json: serde_json::Value = serde_json::from_str(&crate::layout_io::read_json_file(&file)?)
            .map_err(|e| e.to_string())?;
        if json["version"].as_u64() != Some(1) {
            return Err("Unsupported recent maps version / 不支援的最近地圖版本".into());
        }
        for path in json["paths"].as_array().ok_or("Invalid recent maps / 最近地圖格式無效")? {
            let path = PathBuf::from(path.as_str().ok_or("Invalid map path / 地圖路徑無效")?);
            if let Ok(path) = std::fs::canonicalize(path) {
                if path.is_dir() && !result.paths.contains(&path) && result.paths.len() < 8 {
                    result.paths.push(path);
                }
            }
        }
        result.save()?;
        Ok(result)
    }

    fn save(&self) -> Result<(), String> {
        if let Some(file) = &self.file {
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            crate::layout_io::write_json_atomic(file,
                &serde_json::json!({"version": 1, "paths": self.paths}).to_string())?;
        }
        Ok(())
    }

    pub fn remember(&mut self, path: &Path) -> Result<(), String> {
        let path = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
        self.paths.retain(|p| p != &path && p.is_dir());
        self.paths.insert(0, path);
        self.paths.truncate(8);
        self.save()
    }

    pub fn prune(&mut self) -> Result<(), String> {
        let count = self.paths.len();
        self.paths.retain(|p| p.is_dir());
        if count != self.paths.len() { self.save()?; }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_mru_roundtrip_prunes_and_preserves_unknown_versions() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../re_workspace")
            .join(format!("recent-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("recent.json");
        let mut recent = RecentMaps::load(file.clone()).unwrap();
        for n in 0..10 {
            let map = root.join(format!("地圖 {n}"));
            std::fs::create_dir(&map).unwrap();
            recent.remember(&map).unwrap();
        }
        recent.remember(&root.join("地圖 5")).unwrap();
        assert_eq!(recent.paths.len(), 8);
        assert_eq!(recent.paths[0].file_name().unwrap(), "地圖 5");
        std::fs::remove_dir(root.join("地圖 9")).unwrap();
        let mut loaded = RecentMaps::load(file.clone()).unwrap();
        assert_eq!(loaded.paths.len(), 7);
        assert_eq!(loaded.paths[0], recent.paths[0]);
        std::fs::remove_dir(root.join("地圖 8")).unwrap();
        loaded.prune().unwrap();
        assert_eq!(RecentMaps::load(file.clone()).unwrap().paths.len(), 6);
        for input in [r#"{"version":2,"paths":[]}"#, "invalid", r#"{"version":1,"paths":[3]}"#] {
            std::fs::write(&file, input).unwrap();
            assert!(RecentMaps::load(file.clone()).is_err());
            assert_eq!(std::fs::read_to_string(&file).unwrap(), input);
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
