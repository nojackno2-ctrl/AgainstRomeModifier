use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Debug)]
pub struct Row {
    pub id: i32,
    pub line: usize,
    pub raw: String,
    pub values: Vec<String>,
}
impl Row {
    pub fn json(&self) -> Value {
        json!({"id":self.id,"line":self.line,"raw_bytes_hex":hex(self.raw.as_bytes()),"raw_values":self.values})
    }
}
pub struct Table {
    pub section: String,
    pub header: String,
    pub columns: Vec<String>,
    pub rows: BTreeMap<i32, Vec<Row>>,
    pub issues: Vec<String>,
}
impl Table {
    pub fn parse(bytes: &[u8], section: &str, names: bool) -> Result<Self, String> {
        // Every installed input was ASCII. Do not guess a legacy code page or use lossy UTF-8.
        if !bytes.is_ascii() {
            return Err("non-ASCII encoding unresolved; raw source fingerprint retained".into());
        }
        let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        let mut t = Self {
            section: section.into(),
            header: String::new(),
            columns: Vec::new(),
            rows: BTreeMap::new(),
            issues: Vec::new(),
        };
        let mut inside = false;
        let mut sections = 0;
        for (i, raw) in text.split_inclusive('\n').enumerate() {
            let s = raw.trim();
            if s.starts_with('[') {
                inside = s == format!("[{section}]");
                if inside {
                    sections += 1;
                }
                continue;
            }
            if !inside || s.is_empty() {
                continue;
            }
            if s.starts_with(';') {
                let header = s.trim_start_matches(';');
                if header.split(',').next().unwrap_or("").trim() == "idx" {
                    if !t.columns.is_empty() {
                        return Err("duplicate column header".into());
                    }
                    t.header = raw.into();
                    t.columns = header
                        .split(',')
                        .map(|s| s.trim().trim_matches('-').to_string())
                        .collect();
                    let mut seen = std::collections::BTreeSet::new();
                    if t.columns
                        .iter()
                        .any(|s| s.is_empty() || !seen.insert(s.clone()))
                    {
                        return Err("empty or duplicate column name".into());
                    }
                }
                continue;
            }
            let body = s.split(';').next().unwrap();
            let values: Vec<String> = body.split(',').map(str::to_string).collect();
            let id: i32 = values[0]
                .trim()
                .parse()
                .map_err(|_| format!("invalid row id at line {}", i + 1))?;
            if id < 0 {
                return Err(format!("negative row id at line {}", i + 1));
            }
            if !names && t.columns.is_empty() {
                return Err("row before header".into());
            }
            if !names && values.len() != t.columns.len() {
                t.issues.push(format!(
                    "row {id} line {}: {} values vs {} header columns",
                    i + 1,
                    values.len(),
                    t.columns.len()
                ));
            }
            if names && values.len() < 2 {
                return Err("truncated name row".into());
            }
            let rows = t.rows.entry(id).or_default();
            if !rows.is_empty() {
                t.issues
                    .push(format!("duplicate id {id}; ambiguous, no overwrite"));
            }
            rows.push(Row {
                id,
                line: i + 1,
                raw: raw.into(),
                values,
            });
        }
        if sections != 1 || (!names && t.columns.is_empty()) {
            return Err("missing/duplicate section or missing header".into());
        }
        Ok(t)
    }
    pub fn one(&self, id: i32) -> Result<&Row, String> {
        match self.rows.get(&id) {
            None => Err(format!("missing id {id}")),
            Some(v) if v.len() != 1 => Err(format!("ambiguous id {id}")),
            Some(v) => Ok(&v[0]),
        }
    }
    pub fn index(&self, key: &str) -> Result<usize, String> {
        self.columns
            .iter()
            .position(|s| s == key)
            .ok_or_else(|| format!("missing header column {key}"))
    }
    pub fn value<'a>(&self, row: &'a Row, key: &str) -> Result<&'a str, String> {
        row.values
            .get(self.index(key)?)
            .map(|s| s.trim())
            .ok_or_else(|| format!("missing {key} in row {}", row.id))
    }
    pub fn int(&self, row: &Row, key: &str) -> Result<i32, String> {
        self.value(row, key)?
            .parse()
            .map_err(|_| format!("invalid integer {key} row {}", row.id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn header_boundaries() {
        assert!(Table::parse(b"[X]\n;idx,name,name\n0,a,b\n", "X", false).is_err());
        assert!(Table::parse(b"[X]\n0,a\n", "X", false).is_err());
        assert!(Table::parse(b"[X]\n;idx,name\nno,a\n", "X", false).is_err());
        assert!(Table::parse(b"[X]\n;idx,name\n-1,a\n", "X", false).is_err());
        assert!(Table::parse(b"[X]\n;idx,name\n0,\xff\n", "X", false).is_err());
    }
    #[test]
    fn truncated_duplicate_and_raw() {
        let t = Table::parse(
            b"[X]\r\n;idx,activ,name\r\n0,1\r\n1,1,a\r\n1,0,b\r\n",
            "X",
            false,
        )
        .unwrap();
        assert_eq!(t.issues.len(), 2);
        assert!(t.value(t.one(0).unwrap(), "name").is_err());
        assert!(t.one(1).is_err());
        assert!(t.one(2).is_err());
        assert_eq!(t.one(0).unwrap().raw, "0,1\r\n");
    }
    #[test]
    fn names_are_not_invented_headers() {
        let t = Table::parse(b"[AlrNames]\n0001,a.alr,-1\n", "AlrNames", true).unwrap();
        assert!(t.columns.is_empty());
        assert_eq!(t.one(1).unwrap().values[1], "a.alr");
    }
}
