//! Shared reader for the loose `objdef.d*` / `typus.d*` default tables and the
//! `[DefaultPath]` lookup of `SYSTEM/clengine.ini` (type-position / typus probes).

use crate::util::*;
use std::path::{Path, PathBuf};

pub const DEFAULT_PREFIX: &str = "SYSTEM/DATA_MP/DEFAULTS/";

/// Python `int(text)` for the plain decimal forms used by the tables.
pub fn py_int(text: &str) -> R<i64> {
    let t = text.trim().replace('_', "");
    t.parse::<i64>().map_err(|_| format!("invalid literal for int() with base 10: '{text}'"))
}

/// Python `int(text, 16)`.
pub fn py_hex(text: &str) -> R<i64> {
    let t = text.trim();
    let (neg, t) = match t.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let t = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")).unwrap_or(t).replace('_', "");
    let value = i64::from_str_radix(&t, 16).map_err(|_| format!("invalid literal for int() with base 16: '{text}'"))?;
    Ok(if neg { -value } else { value })
}

/// Reads `[DefaultPath]` from `SYSTEM/clengine.ini` (PFIL or plain text, cp1252); defaults when absent.
/// `root` must already be resolved.
pub fn default_prefix(root: &Path) -> R<String> {
    let mut prefix = DEFAULT_PREFIX.to_string();
    let config = root.join("SYSTEM/clengine.ini");
    if config.exists() {
        let text = decode_cp1252(&decode(&read(&config)?)?)?;
        let mut section = false;
        let mut found = false;
        for line in splitlines(&text) {
            let row = line.split(';').next().unwrap_or("").trim();
            if row.starts_with('[') {
                section = row.to_lowercase() == "[defaultpath]";
            } else if section && !row.is_empty() {
                let relative = row.replace('\\', "/");
                req(!Path::new(&relative).is_absolute() && !relative.starts_with('/'), "unsupported absolute DefaultPath")?;
                req(is_inside(&resolve(&root.join(&relative)), root), "DefaultPath outside game root")?;
                prefix = relative;
                found = true;
                break;
            }
        }
        req(found, "missing DefaultPath")?;
    }
    Ok(prefix)
}

/// Existing `family.dau` + `family.d00..d09` files below the default path, in load order.
pub fn definition_files(root: &Path, prefix: &str, family: &str) -> R<Vec<PathBuf>> {
    let mut names = vec![format!("{family}.dau")];
    names.extend((0..10).map(|i| format!("{family}.d{i:02}")));
    let mut out = Vec::new();
    for name in names {
        let path = resolve(&root.join(format!("{prefix}{name}")));
        req(is_inside(&path, root), "source outside root")?;
        if path.exists() {
            out.push(path);
        }
    }
    Ok(out)
}

/// Rows of the first section named `heading` (lower case, e.g. `[objectdefaults]`).
/// Columns are the stripped comma separated fields (empty raw fields dropped like the scripts do).
pub fn section_rows(text: &str, heading: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut section = false;
    for line in splitlines(text) {
        let row = line.split(';').next().unwrap_or("").trim();
        if row.starts_with('[') {
            if section {
                break;
            }
            section = row.to_lowercase() == heading;
            continue;
        }
        if !section || row.is_empty() {
            continue;
        }
        let columns: Vec<String> = row.split(',').filter(|c| !c.is_empty()).map(|c| c.trim().to_string()).collect();
        if !columns.is_empty() {
            rows.push(columns);
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_first_section_only_and_drops_comments() {
        let text = "; header\n[Other]\n1,2\n[ObjectDefaults]\n3, 1 ,x ; trailing\n\n4,,5\n[Next]\n9,9\n";
        let rows = section_rows(text, "[objectdefaults]");
        assert_eq!(rows, vec![vec!["3", "1", "x"], vec!["4", "5"]]);
        assert!(section_rows(text, "[missing]").is_empty());
    }

    #[test]
    fn python_number_forms() {
        assert_eq!(py_int(" 42 ").unwrap(), 42);
        assert_eq!(py_int("-7").unwrap(), -7);
        assert!(py_int("1.5").is_err());
        assert_eq!(py_hex("0x400").unwrap(), 0x400);
        assert_eq!(py_hex("1F").unwrap(), 31);
        assert!(py_hex("zz").is_err());
    }
}
