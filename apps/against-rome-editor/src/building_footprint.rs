//! 從有效物件定義解析唯讀 APT，占用清單與失敗結果皆依 type 快取。
use against_rome_data::apt_footprint::{AptFootprint, FootprintCell};
use against_rome_maps::native_objdef::{NativeObjectDefinition, NativeObjectDefinitions};
use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

pub const GROUP_LABEL: &str = "group 0 (default state) / 群組 0（預設狀態）";
#[derive(Clone, Debug)]
pub struct ResolvedFootprint {
    pub member: String,
    pub sha256: String,
    pub group_label: &'static str,
    pub cells: Vec<FootprintCell>,
}
pub struct FootprintResolver {
    root: PathBuf,
    definitions: HashMap<i32, NativeObjectDefinition>,
    names: HashMap<i16, String>,
    prefix: String,
    cache: HashMap<i32, Result<Arc<ResolvedFootprint>, String>>,
}
fn text(path: &Path) -> Result<String, String> {
    let b = std::fs::read(path).map_err(|e| e.to_string())?;
    let b = against_rome_maps::text_encoding::decompress_pfil_if_needed(&b)?;
    against_rome_maps::text_encoding::decode_cp1251(&b)
}
fn rows<'a>(text: &'a str, section: &str) -> Vec<&'a str> {
    let mut active = false;
    let mut out = Vec::new();
    for line in text.lines() {
        let row = line.split(';').next().unwrap_or("").trim();
        if row.starts_with('[') {
            active = row.eq_ignore_ascii_case(section);
        } else if active && !row.is_empty() {
            out.push(row);
        }
    }
    out
}
fn relative(s: &str) -> Result<String, String> {
    let normalized = s.replace('\\', "/");
    if Path::new(&normalized)
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("APT 路徑必須位於遊戲目錄內".into());
    }
    Ok(normalized)
}
impl FootprintResolver {
    pub fn load(root: &Path) -> Result<Self, String> {
        let definitions = NativeObjectDefinitions::load(root)?;
        let config = text(&root.join("SYSTEM/clengine.ini"))?;
        let prefix = relative(rows(&config, "[AptPath]").first().ok_or("缺少 AptPath")?)?;
        let catalog = text(&root.join("SYSTEM/cl_apt.ini"))?;
        let mut names = HashMap::new();
        for row in rows(&catalog, "[AptNames]") {
            let mut fields = row.split(',').map(str::trim);
            let id = fields
                .next()
                .ok_or("缺少 APT 編號")?
                .parse::<i16>()
                .map_err(|e| e.to_string())?;
            let name = relative(fields.next().ok_or("缺少 APT 名稱")?)?;
            names.insert(id, name);
        }
        Ok(Self {
            root: root.into(),
            definitions,
            names,
            prefix,
            cache: HashMap::new(),
        })
    }
    #[cfg(test)]
    pub(crate) fn synthetic(type_id: i32, cells: Vec<FootprintCell>) -> Self {
        let stencil = ResolvedFootprint {
            member: "synthetic.apt".into(),
            sha256: String::new(),
            group_label: GROUP_LABEL,
            cells,
        };
        Self {
            root: PathBuf::new(),
            definitions: HashMap::new(),
            names: HashMap::new(),
            prefix: String::new(),
            cache: HashMap::from([(type_id, Ok(Arc::new(stencil)))]),
        }
    }
    pub fn resolve(&mut self, type_id: i32) -> Result<Arc<ResolvedFootprint>, String> {
        if let Some(hit) = self.cache.get(&type_id) {
            return hit.clone();
        }
        let result = self.resolve_uncached(type_id).map(Arc::new);
        self.cache.insert(type_id, result.clone());
        result
    }
    fn resolve_uncached(&self, type_id: i32) -> Result<ResolvedFootprint, String> {
        let definition = self
            .definitions
            .get(&type_id)
            .filter(|d| d.active)
            .ok_or("無有效物件定義")?;
        let aptix = definition.aptix.ok_or("物件缺少 aptix")?;
        let name = self.names.get(&aptix).ok_or("AptNames 缺少此 aptix")?;
        let member = format!("{}/{}", self.prefix.trim_end_matches('/'), name);
        let bytes =
            against_rome_textures::catalog::read_apt_member(&self.root.join("apt.dat"), &member)
                .ok_or_else(|| format!("apt.dat 缺少 {member}"))?;
        let document = AptFootprint::parse(&bytes)?;
        let cells = document.groups.into_iter().next().ok_or("APT 無群組 0")?;
        Ok(ResolvedFootprint {
            member,
            sha256: against_rome_data::sha256::sha256_hex(&bytes),
            group_label: GROUP_LABEL,
            cells,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "需要唯讀零售遊戲資產"]
    fn retail_apt_footprint_gate() {
        let root = PathBuf::from(
            std::env::var_os("AGAINST_ROME_GAME")
                .unwrap_or_else(|| r"C:\Program Files (x86)\Against Rome".into()),
        );
        assert!(root.is_dir(), "零售遊戲不存在：{}", root.display());
        let mut resolver = FootprintResolver::load(&root).unwrap();
        for (id, count, bounds, hash) in [
            (
                232,
                101,
                [-5, 7, -5, 3],
                "a2a57992a0c89a23a35ea580d92f039b4013e42b88109da44323887b4d9e29d2",
            ),
            (
                37,
                36,
                [-4, 1, -3, 2],
                "6c596a0025190d205d599efb468b8004f90a42b685a16f475b10d46666a9e262",
            ),
            (
                34,
                49,
                [-4, 2, -4, 2],
                "578ff6b135b2c112cdae63a7ab1701eab6617b18395f72801111951c08ec3884",
            ),
        ] {
            let stencil = resolver.resolve(id).unwrap();
            assert_eq!(stencil.cells.len(), count);
            assert_eq!(stencil.sha256, hash);
            let xs: Vec<_> = stencil.cells.iter().map(|c| c.relative[0]).collect();
            let zs: Vec<_> = stencil.cells.iter().map(|c| c.relative[1]).collect();
            assert_eq!(
                [
                    *xs.iter().min().unwrap(),
                    *xs.iter().max().unwrap(),
                    *zs.iter().min().unwrap(),
                    *zs.iter().max().unwrap()
                ],
                bounds
            );
            assert!(Arc::ptr_eq(&stencil, &resolver.resolve(id).unwrap()));
            println!(
                "{}：{} 格，{}×{}，SHA256 {}",
                stencil.member,
                count,
                bounds[1] - bounds[0] + 1,
                bounds[3] - bounds[2] + 1,
                hash
            );
        }
    }
    #[test]
    fn aptix_layers_preserve_and_override() {
        let mut columns = vec!["0"; 15];
        columns[0] = "34";
        columns[1] = "1";
        columns[14] = "9";
        let first = format!("[ObjectDefaults]\n{}\n", columns.join(","));
        let defs = NativeObjectDefinitions::parse(&first).unwrap();
        assert_eq!(defs[&34].aptix, Some(9));
        assert!(relative("../outside").is_err());
        assert!(relative(r"C:\outside").is_err());
    }
}
