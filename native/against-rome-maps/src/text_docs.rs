//! Faithful ports of `MapTextDocument`, `BodenIniDocument`, `PutTextDocument`, `SdlDocument`, and `BodenTexturesDocument`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use crate::text_encoding::{
    can_encode_game_text, compress_pfil, decode_cp1251, decompress_pfil_if_needed, encode_cp1251, is_pfil,
};

/// 3D vector used in SDL scene and settlement files.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SdlVector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl SdlVector3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() != 3 {
            return Err(format!("無效的 SDL 三維座標：{s}"));
        }
        let x = parts[0].trim().parse::<f32>().map_err(|_| format!("無效的 SDL 三維座標：{s}"))?;
        let y = parts[1].trim().parse::<f32>().map_err(|_| format!("無效的 SDL 三維座標：{s}"))?;
        let z = parts[2].trim().parse::<f32>().map_err(|_| format!("無效的 SDL 三維座標：{s}"))?;
        Ok(Self { x, y, z })
    }

    pub fn to_string_2dp(&self) -> String {
        format!("{:.2},{:.2},{:.2}", self.x, self.y, self.z)
    }

    pub fn to_string_concise(&self) -> String {
        let fmt = |v: f32| {
            let s = format!("{:.2}", v);
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        };
        format!("{},{},{}", fmt(self.x), fmt(self.y), fmt(self.z))
    }
}

impl std::fmt::Display for SdlVector3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string_concise())
    }
}

// =========================================================================
// Base MapTextDocument helper
// =========================================================================

#[derive(Clone, Debug)]
pub struct RawTextDocument {
    pub path: PathBuf,
    pub original_bytes: Vec<u8>,
    pub text: String,
}

impl RawTextDocument {
    pub fn load(path: &Path) -> Result<Self, String> {
        let original_bytes = fs::read(path).map_err(|e| format!("讀取檔案失敗 ({}): {e}", path.display()))?;
        let decompressed = decompress_pfil_if_needed(&original_bytes)?;
        let text = decode_cp1251(&decompressed)?;
        Ok(Self {
            path: path.to_path_buf(),
            original_bytes,
            text,
        })
    }

    pub fn from_text(path: &Path, text: String, original_bytes: Vec<u8>) -> Self {
        Self {
            path: path.to_path_buf(),
            original_bytes,
            text,
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let plain_bytes = encode_cp1251(&self.text)?;
        if is_pfil(&self.original_bytes) {
            compress_pfil(&plain_bytes, &self.original_bytes)
        } else {
            Ok(plain_bytes)
        }
    }

    pub fn save(&self, destination: Option<&Path>) -> Result<(), String> {
        let target_path = destination.unwrap_or(&self.path);
        let output_bytes = self.to_bytes()?;

        // Write safely via tmp file
        if let Some(parent) = target_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| format!("建立目錄失敗: {e}"))?;
            }
        }
        let tmp_path = target_path.with_extension("tmp_arm_text");
        fs::write(&tmp_path, &output_bytes).map_err(|e| format!("寫入暫存檔案失敗: {e}"))?;
        fs::rename(&tmp_path, target_path).map_err(|e| format!("覆蓋目標檔案失敗: {e}"))?;
        Ok(())
    }
}

// =========================================================================
// BodenIniDocument
// =========================================================================

#[derive(Clone, Debug)]
pub struct BodenIniDocument {
    raw: RawTextDocument,
}

impl BodenIniDocument {
    pub fn load(path: &Path) -> Result<Self, String> {
        Ok(Self {
            raw: RawTextDocument::load(path)?,
        })
    }

    pub fn from_text(text: &str) -> Self {
        Self {
            raw: RawTextDocument::from_text(Path::new("boden.ini"), text.to_string(), text.as_bytes().to_vec()),
        }
    }

    pub fn text(&self) -> &str {
        &self.raw.text
    }

    pub fn get_value(&self, section: &str) -> Option<String> {
        self.find_value(section).map(|(_, _, val)| val)
    }

    pub fn set_value(&mut self, section: &str, value: &str) -> Result<(), String> {
        let (start, end, _) = self
            .find_value(section)
            .ok_or_else(|| format!("找不到 boden.ini 區段: {section}"))?;
        self.raw.text = format!("{}{}{}", &self.raw.text[..start], value, &self.raw.text[end..]);
        Ok(())
    }

    pub fn dimension(&self) -> Option<u32> {
        self.get_value("Dimension").and_then(|v| v.parse().ok())
    }

    pub fn water_level(&self) -> Option<f32> {
        self.get_value("Waterlevel").and_then(|v| v.parse().ok())
    }

    pub fn height_step(&self) -> Option<f32> {
        self.get_value("Heightmapstep").and_then(|v| v.parse().ok())
    }

    pub fn save(&self) -> Result<(), String> {
        self.raw.save(None)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        self.raw.to_bytes()
    }

    fn find_value(&self, section: &str) -> Option<(usize, usize, String)> {
        let lines: Vec<&str> = self.raw.text.lines().collect();
        let mut byte_offset = 0;
        let mut found_section = false;

        for line in lines {
            let line_len_bytes = line.len();
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let sec_name = &trimmed[1..trimmed.len() - 1].trim();
                if sec_name.eq_ignore_ascii_case(section) {
                    found_section = true;
                    // move past line and newline
                    byte_offset += self.line_total_bytes(byte_offset, line_len_bytes);
                    continue;
                } else if found_section {
                    break;
                }
            }

            if found_section {
                // If it's not empty and not comment, this is the value line
                if !trimmed.is_empty() && !trimmed.starts_with(';') {
                    let val_no_comment = line.split(';').next().unwrap_or(line);
                    let val_trimmed = val_no_comment.trim();
                    if !val_trimmed.is_empty() {
                        let rel_start = line.find(val_trimmed).unwrap();
                        let start = byte_offset + rel_start;
                        let end = start + val_trimmed.len();
                        return Some((start, end, val_trimmed.to_string()));
                    }
                }
            }
            byte_offset += self.line_total_bytes(byte_offset, line_len_bytes);
        }
        None
    }

    fn line_total_bytes(&self, offset: usize, line_len: usize) -> usize {
        let rem = &self.raw.text[offset + line_len..];
        if rem.starts_with("\r\n") {
            line_len + 2
        } else if rem.starts_with('\n') || rem.starts_with('\r') {
            line_len + 1
        } else {
            line_len
        }
    }
}

// =========================================================================
// PutTextDocument
// =========================================================================

#[derive(Clone, Debug)]
pub struct PutTextDocument {
    raw: RawTextDocument,
}

impl PutTextDocument {
    pub const MAX_LITERAL_BYTES: usize = 100;

    pub fn load(path: &Path) -> Result<Self, String> {
        Ok(Self {
            raw: RawTextDocument::load(path)?,
        })
    }

    pub fn from_text(text: &str) -> Self {
        Self {
            raw: RawTextDocument::from_text(Path::new("briefing.put"), text.to_string(), text.as_bytes().to_vec()),
        }
    }

    pub fn text(&self) -> &str {
        &self.raw.text
    }

    pub fn save(&self) -> Result<(), String> {
        self.raw.save(None)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        self.raw.to_bytes()
    }

    pub fn get_value(&self, key: &str) -> Option<String> {
        self.find_single(key).map(|(_, _, val)| unescape(&val))
    }

    pub fn set_value(&mut self, key: &str, value: &str) -> Result<(), String> {
        if value.contains('\0') || value.contains('\r') || value.contains('\n') {
            return Err("地圖標題與隊伍名稱不可含 NUL 或換行。".into());
        }
        if !can_encode_game_text(value) {
            return Err("地圖文字需使用遊戲支援的 CP1251 字元（拉丁／西里爾字母）；中文或 emoji 無法儲存。".into());
        }
        let escaped = escape(value);
        let escaped_bytes_len = encode_cp1251(&escaped)?.len();
        if escaped_bytes_len > Self::MAX_LITERAL_BYTES {
            return Err(format!(
                "地圖文字跳脫後最多 {} 位元組（遊戲的文字緩衝區限制）。",
                Self::MAX_LITERAL_BYTES
            ));
        }

        let (val_start, val_end, existing) = self
            .find_single(key)
            .ok_or_else(|| format!("找不到 .put 變數: {key}"))?;

        if unescape(&existing) == value && encode_cp1251(&existing)?.len() <= Self::MAX_LITERAL_BYTES {
            return Ok(());
        }

        self.raw.text = format!("{}{}{}", &self.raw.text[..val_start], escaped, &self.raw.text[val_end..]);
        Ok(())
    }

    pub fn get_composite_value(&self, key: &str) -> Option<String> {
        let (_, _, expr) = self.find_assignment(key)?;
        let fragments = extract_quoted_literals(&expr);
        if fragments.is_empty() {
            None
        } else {
            let mut combined = String::new();
            for f in fragments {
                combined.push_str(&unescape(&f));
            }
            Some(combined)
        }
    }

    pub fn set_composite_value(&mut self, key: &str, value: &str) -> Result<(), String> {
        if value.contains('\0') {
            return Err("地圖文字不可包含 NUL 字元。".into());
        }
        if !can_encode_game_text(value) {
            return Err("地圖文字需使用遊戲支援的 CP1251 字元（拉丁／西里爾字母）；中文或 emoji 無法儲存。".into());
        }
        let (expr_start, expr_end, expr) = self
            .find_assignment(key)
            .ok_or_else(|| format!("找不到 .put 變數: {key}"))?;

        let unchanged = self.get_composite_value(key).as_deref() == Some(value);
        let safe = extract_quoted_literals(&expr)
            .into_iter()
            .all(|f| encode_cp1251(&f).map(|b| b.len() <= Self::MAX_LITERAL_BYTES).unwrap_or(false));
        if unchanged && safe {
            return Ok(());
        }

        let escaped = escape(value);
        let chunks = split_literals(&escaped);
        let newline = if self.raw.text.contains("\r\n") { "\r\n" } else { "\n" };
        let replacement: Vec<String> = chunks.into_iter().map(|c| format!("\"{c}\"")).collect();
        let replacement_str = replacement.join(newline);

        self.raw.text = format!("{}{}{}", &self.raw.text[..expr_start], replacement_str, &self.raw.text[expr_end..]);
        Ok(())
    }

    fn find_single(&self, key: &str) -> Option<(usize, usize, String)> {
        // Looks for lines: `var: <key> = "<value>"`
        let key_lower = key.to_ascii_lowercase();
        let mut byte_offset = 0;
        for line in self.raw.text.lines() {
            let line_len_bytes = line.len();
            let trimmed = line.trim();
            if trimmed.to_ascii_lowercase().starts_with("var:") {
                let rest = trimmed[4..].trim_start();
                if let Some((k, after_eq)) = rest.split_once('=') {
                    if k.trim().eq_ignore_ascii_case(&key_lower) {
                        if let Some(first_quote) = after_eq.find('"') {
                            if let Some(last_quote) = after_eq.rfind('"') {
                                if last_quote > first_quote {
                                    let inside = &after_eq[first_quote + 1..last_quote];
                                    let rel_quote = line.find(inside).unwrap();
                                    let start = byte_offset + rel_quote;
                                    let end = start + inside.len();
                                    return Some((start, end, inside.to_string()));
                                }
                            }
                        }
                    }
                }
            }
            byte_offset += self.line_total_bytes(byte_offset, line_len_bytes);
        }
        None
    }

    fn find_assignment(&self, key: &str) -> Option<(usize, usize, String)> {
        // Multi-line assignment: `var: <key> = <expression>;`
        let text_lower = self.raw.text.to_ascii_lowercase();
        let mut pos = 0;
        while let Some(var_idx) = text_lower[pos..].find("var:") {
            let actual_var = pos + var_idx;
            // Check if at start of line
            let is_line_start = actual_var == 0
                || self.raw.text.as_bytes()[actual_var - 1] == b'\n'
                || self.raw.text.as_bytes()[actual_var - 1] == b'\r'
                || self.raw.text[..actual_var].chars().rev().take_while(|c| *c == ' ' || *c == '\t').count() == actual_var;
            if !is_line_start {
                pos = actual_var + 4;
                continue;
            }

            let eq_idx = match text_lower[actual_var..].find('=') {
                Some(i) => actual_var + i,
                None => { pos = actual_var + 4; continue; }
            };

            let key_cand = self.raw.text[actual_var + 4..eq_idx].trim();
            if key_cand.eq_ignore_ascii_case(key) {
                // Found var:<key> = ... find ';'
                let semi_idx = match self.raw.text[eq_idx..].find(';') {
                    Some(i) => eq_idx + i,
                    None => { pos = eq_idx + 1; continue; }
                };
                let expr_raw = &self.raw.text[eq_idx + 1..semi_idx];
                return Some((eq_idx + 1, semi_idx, expr_raw.to_string()));
            }
            pos = actual_var + 4;
        }
        None
    }

    fn line_total_bytes(&self, offset: usize, line_len: usize) -> usize {
        let rem = &self.raw.text[offset + line_len..];
        if rem.starts_with("\r\n") {
            line_len + 2
        } else if rem.starts_with('\n') || rem.starts_with('\r') {
            line_len + 1
        } else {
            line_len
        }
    }
}

pub fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 8);
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '"' => out.push_str("\\\""),
            other => out.push(other),
        }
    }
    out
}

pub fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                match next {
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    other => {
                        out.push('\\');
                        out.push(other);
                    }
                }
            } else {
                out.push('\\');
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn split_literals(escaped: &str) -> Vec<String> {
    let max = PutTextDocument::MAX_LITERAL_BYTES;
    let mut chunks = Vec::new();
    let mut start = 0;

    while escaped.len() - start > max {
        let limit = start + max;
        let mut position = start;
        let mut last_break = -1i32;

        while position < limit {
            let length = if position + 1 < escaped.len() && escaped.as_bytes()[position] == b'\\' {
                2
            } else {
                1
            };
            if position + length > limit {
                break;
            }
            position += length;
            if escaped.as_bytes()[position - 1] == b' '
                || (length == 2 && escaped.as_bytes()[position - 1] == b'n')
            {
                last_break = position as i32;
            }
        }

        let cut = if last_break >= 0 && (last_break as usize - start) >= max / 2 {
            last_break as usize
        } else {
            position
        };
        chunks.push(escaped[start..cut].to_string());
        start = cut;
    }
    chunks.push(escaped[start..].to_string());
    chunks
}

fn extract_quoted_literals(expr: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut in_quote = false;
    let mut escaped = false;
    let mut current = String::new();

    for c in expr.chars() {
        if in_quote {
            if escaped {
                current.push(c);
                escaped = false;
            } else if c == '\\' {
                current.push(c);
                escaped = true;
            } else if c == '"' {
                result.push(current.clone());
                current.clear();
                in_quote = false;
            } else {
                current.push(c);
            }
        } else if c == '"' {
            in_quote = true;
            current.clear();
        }
    }
    result
}

// =========================================================================
// SdlDocument
// =========================================================================

#[derive(Clone, Debug)]
pub struct SdlObjectSection {
    pub index: i32,
    pub fields: HashMap<String, String>,
}

impl SdlObjectSection {
    pub fn get_value(&self, key: &str) -> Option<&str> {
        self.fields.get(&key.to_ascii_lowercase()).map(|s| s.as_str())
    }
}

#[derive(Clone, Debug)]
pub struct SdlDocument {
    raw: RawTextDocument,
}

impl SdlDocument {
    pub fn load(path: &Path) -> Result<Self, String> {
        Ok(Self {
            raw: RawTextDocument::load(path)?,
        })
    }

    pub fn from_text(text: &str) -> Self {
        Self {
            raw: RawTextDocument::from_text(Path::new("scene.sdl"), text.to_string(), text.as_bytes().to_vec()),
        }
    }

    pub fn text(&self) -> &str {
        &self.raw.text
    }

    pub fn save(&self) -> Result<(), String> {
        self.raw.save(None)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        self.raw.to_bytes()
    }

    pub fn settlement(&self) -> HashMap<String, String> {
        self.read_section("settlement").unwrap_or_default()
    }

    pub fn objects(&self) -> Vec<SdlObjectSection> {
        let mut result = Vec::new();
        for sec in self.sections() {
            if let Some(idx) = parse_object_index(&sec.name) {
                result.push(SdlObjectSection {
                    index: idx,
                    fields: read_fields(&sec.body),
                });
            }
        }
        result
    }

    pub fn rewrite_map_path(&mut self, old_map_id: &str, new_map_id: &str) {
        // Rewrite `name = MAPS/<old_map_id>/...` or `name=MAPS/<old_map_id>/`
        let target = format!("maps/{}/", old_map_id.to_ascii_lowercase());
        let replacement = format!("MAPS/{}/", new_map_id);

        let lines: Vec<&str> = self.raw.text.lines().collect();
        let mut new_lines = Vec::with_capacity(lines.len());
        for line in lines {
            let trimmed = line.trim();
            if trimmed.to_ascii_lowercase().starts_with("name") {
                if let Some((k, v)) = line.split_once('=') {
                    if k.trim().eq_ignore_ascii_case("name") {
                        let v_trimmed = v.trim();
                        if v_trimmed.to_ascii_lowercase().starts_with(&target) {
                            let rewritten = format!("{}{}{}", k, "=", v.replacen(&v_trimmed[..target.len()], &replacement, 1));
                            new_lines.push(rewritten);
                            continue;
                        }
                    }
                }
            }
            new_lines.push(line.to_string());
        }
        let newline = if self.raw.text.contains("\r\n") { "\r\n" } else { "\n" };
        self.raw.text = new_lines.join(newline);
        if !self.raw.text.ends_with('\n') {
            self.raw.text.push_str(newline);
        }
    }

    pub fn get_settlement_refpos(&self) -> Result<SdlVector3, String> {
        let settlement = self.settlement();
        let refpos_str = settlement.get("refpos").ok_or("SDL 缺少欄位 refpos。")?;
        SdlVector3::parse(refpos_str)
    }

    pub fn set_settlement_value(&mut self, key: &str, value: &str) -> Result<(), String> {
        self.set_section_value("settlement", key, value)
    }

    pub fn translate_settlement(&mut self, delta_x: f32, delta_y: f32, delta_z: f32) -> Result<(), String> {
        let settlement = self.settlement();
        let refpos_str = settlement.get("refpos").ok_or("SDL 缺少欄位 refpos。")?;
        let refpos = SdlVector3::parse(refpos_str)?;
        let translated = SdlVector3::new(refpos.x + delta_x, refpos.y + delta_y, refpos.z + delta_z);
        self.set_settlement_value("refpos", &translated.to_string_concise())
    }

    pub fn set_object_value(&mut self, index: i32, key: &str, value: &str) -> Result<(), String> {
        self.set_section_value(&object_name(index), key, value)
    }

    /// Removes every `key=` line from an object section, leaving all other bytes untouched.
    /// A missing key is a no-op; a missing section is an error.
    pub fn remove_object_value(&mut self, index: i32, key: &str) -> Result<(), String> {
        let section_name = object_name(index);
        let sec = self.find_section(&section_name)?;
        let key_lower = key.trim().to_ascii_lowercase();
        let mut kept = String::with_capacity(sec.body.len());
        let mut removed = false;
        for line in sec.body.split_inclusive('\n') {
            if let Some((k, _)) = line.trim().split_once('=') {
                if k.trim().to_ascii_lowercase() == key_lower {
                    removed = true;
                    continue;
                }
            }
            kept.push_str(line);
        }
        if removed {
            let body_end = sec.body_start + sec.body.len();
            self.raw.text.replace_range(sec.body_start..body_end, &kept);
        }
        Ok(())
    }

    pub fn set_object_team(&mut self, index: i32, team: i32) -> Result<(), String> {
        self.set_object_value(index, "team", &team.to_string())
    }

    pub fn set_object_position(&mut self, index: i32, position: SdlVector3) -> Result<(), String> {
        self.set_object_value(index, "pos", &position.to_string_2dp())
    }

    pub fn set_object_angle(&mut self, index: i32, angle: f32) -> Result<(), String> {
        self.set_object_value(index, "angle", &format!("{:.2}", angle))
    }

    pub fn set_object_definition(&mut self, index: i32, definition: i32, name: &str) -> Result<(), String> {
        self.set_object_value(index, "def", &definition.to_string())?;
        self.set_object_value(index, "namedef", name)
    }

    pub fn add_object(&mut self, fields: &HashMap<String, String>) -> Result<i32, String> {
        let index = self.objects().len() as i32;
        let newline = if self.raw.text.contains("\r\n") { "\r\n" } else { "\n" };
        let mut block = String::new();
        if !self.raw.text.is_empty() && !self.raw.text.ends_with('\n') {
            block.push_str(newline);
        }
        block.push('[');
        block.push_str(&object_name(index));
        block.push(']');
        block.push_str(newline);

        for (k, v) in fields {
            validate_key_value(k, v)?;
            block.push_str(k);
            block.push('=');
            block.push_str(v);
            block.push_str(newline);
        }

        self.raw.text.push_str(&block);
        Ok(index)
    }

    pub fn remove_object(&mut self, index: i32) -> Result<(), String> {
        let sec = self.find_section(&object_name(index))?;
        self.raw.text.drain(sec.start..sec.start + sec.length);
        self.renumber_objects();
        Ok(())
    }

    pub fn clear_objects(&mut self) {
        let sections = self.sections();
        let mut new_text = String::with_capacity(self.raw.text.len());
        let mut last_pos = 0;
        for s in sections {
            if parse_object_index(&s.name).is_some() {
                // skip this section
                new_text.push_str(&self.raw.text[last_pos..s.start]);
                last_pos = s.start + s.length;
            }
        }
        new_text.push_str(&self.raw.text[last_pos..]);
        self.raw.text = new_text;
    }

    fn read_section(&self, name: &str) -> Option<HashMap<String, String>> {
        self.find_section(name).ok().map(|s| read_fields(&s.body))
    }

    fn set_section_value(&mut self, section_name: &str, key: &str, value: &str) -> Result<(), String> {
        validate_key_value(key, value)?;
        let sec = self.find_section(section_name)?;
        let key_lower = key.to_ascii_lowercase();

        let mut pos = 0;
        for line in sec.body.lines() {
            let line_len = line.len();
            let trimmed = line.trim();
            if let Some((k, _)) = trimmed.split_once('=') {
                if k.trim().to_ascii_lowercase() == key_lower {
                    let eq_idx = line.find('=').unwrap();
                    let field_val_start = sec.body_start + pos + eq_idx + 1;
                    let existing_line_end = sec.body_start + pos + line_len;
                    self.raw.text = format!(
                        "{}{}{}",
                        &self.raw.text[..field_val_start],
                        value,
                        &self.raw.text[existing_line_end..]
                    );
                    return Ok(());
                }
            }
            pos += if sec.body[pos + line_len..].starts_with("\r\n") {
                line_len + 2
            } else if sec.body[pos + line_len..].starts_with('\n') {
                line_len + 1
            } else {
                line_len
            };
        }
        Err(format!("SDL 區段 [{section_name}] 缺少欄位 {key}。"))
    }

    fn renumber_objects(&mut self) {
        let mut next = 0;
        let mut lines = Vec::new();
        for line in self.raw.text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let name = &trimmed[1..trimmed.len() - 1].trim();
                if parse_object_index(name).is_some() {
                    lines.push(format!("[object{:04}]", next));
                    next += 1;
                    continue;
                }
            }
            lines.push(line.to_string());
        }
        let newline = if self.raw.text.contains("\r\n") { "\r\n" } else { "\n" };
        self.raw.text = lines.join(newline);
        if !self.raw.text.ends_with('\n') {
            self.raw.text.push_str(newline);
        }
    }

    fn find_section(&self, requested: &str) -> Result<TextSection, String> {
        for sec in self.sections() {
            if section_name_matches(&sec.name, requested) {
                return Ok(sec);
            }
        }
        Err(format!("SDL 缺少 [{requested}] 區段。"))
    }

    fn sections(&self) -> Vec<TextSection> {
        let mut sections = Vec::new();
        let lines: Vec<&str> = self.raw.text.lines().collect();
        let mut byte_offset = 0;
        let mut current_name: Option<String> = None;
        let mut current_start = 0;
        let mut body_start = 0;

        for line in lines {
            let line_len_bytes = line.len();
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let name = trimmed[1..trimmed.len() - 1].trim().to_string();
                if let Some(prev_name) = current_name.take() {
                    let length = byte_offset - current_start;
                    let body = self.raw.text[body_start..byte_offset].to_string();
                    sections.push(TextSection {
                        name: prev_name,
                        start: current_start,
                        length,
                        body_start,
                        body,
                    });
                }
                current_name = Some(name);
                current_start = byte_offset;
                let line_bytes = self.line_total_bytes(byte_offset, line_len_bytes);
                body_start = byte_offset + line_bytes;
                byte_offset += line_bytes;
                continue;
            }
            byte_offset += self.line_total_bytes(byte_offset, line_len_bytes);
        }

        if let Some(name) = current_name {
            let length = byte_offset - current_start;
            let body = self.raw.text[body_start..byte_offset].to_string();
            sections.push(TextSection {
                name,
                start: current_start,
                length,
                body_start,
                body,
            });
        }

        sections
    }

    fn line_total_bytes(&self, offset: usize, line_len: usize) -> usize {
        let rem = &self.raw.text[offset + line_len..];
        if rem.starts_with("\r\n") {
            line_len + 2
        } else if rem.starts_with('\n') || rem.starts_with('\r') {
            line_len + 1
        } else {
            line_len
        }
    }
}

#[derive(Clone, Debug)]
struct TextSection {
    name: String,
    start: usize,
    length: usize,
    body_start: usize,
    body: String,
}

fn parse_object_index(name: &str) -> Option<i32> {
    if name.len() >= 7 && name[..6].eq_ignore_ascii_case("object") {
        name[6..].parse::<i32>().ok()
    } else {
        None
    }
}

fn section_name_matches(actual: &str, requested: &str) -> bool {
    if actual.eq_ignore_ascii_case(requested) {
        return true;
    }
    match (parse_object_index(actual), parse_object_index(requested)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

fn object_name(index: i32) -> String {
    format!("object{:04}", index)
}

fn read_fields(body: &str) -> HashMap<String, String> {
    let mut fields = HashMap::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            fields.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    fields
}

fn validate_key_value(key: &str, value: &str) -> Result<(), String> {
    if key.trim().is_empty() || key.contains('=') || key.contains('\r') || key.contains('\n') {
        return Err("SDL 欄位名稱無效。".into());
    }
    if value.contains('\r') || value.contains('\n') {
        return Err("SDL 欄位值不可包含換行。".into());
    }
    Ok(())
}

// =========================================================================
// BodenTexturesDocument
// =========================================================================

#[derive(Clone, Debug)]
pub struct BodenTexturesDocument {
    raw: RawTextDocument,
}

impl BodenTexturesDocument {
    pub fn load(path: &Path) -> Result<Self, String> {
        Ok(Self {
            raw: RawTextDocument::load(path)?,
        })
    }

    pub fn from_text(text: &str) -> Self {
        Self {
            raw: RawTextDocument::from_text(Path::new("boden.txt"), text.to_string(), text.as_bytes().to_vec()),
        }
    }

    pub fn text(&self) -> &str {
        &self.raw.text
    }

    pub fn save(&self) -> Result<(), String> {
        self.raw.save(None)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        self.raw.to_bytes()
    }

    pub fn dimension(&self) -> Result<usize, String> {
        let lines: Vec<&str> = self.raw.text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if line.trim().eq_ignore_ascii_case("[dimension]") {
                if let Some(next_line) = lines.get(i + 1) {
                    if let Ok(dim) = next_line.trim().parse::<usize>() {
                        if dim > 0 {
                            return Ok(dim);
                        }
                    }
                }
            }
        }
        Err("boden.txt 缺少有效的 [Dimension]。".into())
    }

    pub fn textures(&self) -> Result<Vec<String>, String> {
        let entries = self.entries()?;
        Ok(entries.into_iter().map(|e| e.value).collect())
    }

    pub fn get_texture(&self, x: usize, y: usize) -> Result<String, String> {
        let entry = self.entry_at(x, y)?;
        Ok(entry.value)
    }

    pub fn set_texture(&mut self, x: usize, y: usize, texture: &str) -> Result<(), String> {
        validate_texture_name(texture)?;
        let entry = self.entry_at(x, y)?;
        self.raw.text = format!("{}{}{}", &self.raw.text[..entry.start], texture.trim(), &self.raw.text[entry.end..]);
        Ok(())
    }

    /// Bulk write the entire 64x64 texture table.
    pub fn set_textures(&mut self, textures: &[String]) -> Result<(), String> {
        let dim = self.dimension()?;
        if dim != 64 {
            return Err("目前只支援已驗證的 64×64 boden.txt 材質格。".into());
        }
        let entries = self.entries()?;
        let expected = dim * dim;
        if entries.len() != expected {
            return Err(format!("boden.txt 應有 {expected} 個材質格，實際為 {}。", entries.len()));
        }
        if textures.len() != expected {
            return Err(format!("材質數量應為 {expected}，實際為 {}。", textures.len()));
        }
        for t in textures {
            validate_texture_name(t)?;
        }

        let mut out = String::with_capacity(self.raw.text.len() + 64);
        let mut pos = 0;
        for (i, entry) in entries.iter().enumerate() {
            out.push_str(&self.raw.text[pos..entry.start]);
            out.push_str(textures[i].trim());
            pos = entry.end;
        }
        out.push_str(&self.raw.text[pos..]);
        self.raw.text = out;
        Ok(())
    }

    fn entry_at(&self, x: usize, y: usize) -> Result<TextureEntry, String> {
        let dim = self.dimension()?;
        if x >= 64 || y >= 64 || dim != 64 {
            return Err("目前只支援已驗證的 64×64 boden.txt 材質格。".into());
        }
        let entries = self.entries()?;
        let idx = y * dim + x;
        if entries.len() != dim * dim {
            return Err(format!("boden.txt 應有 {} 個材質格，實際為 {}。", dim * dim, entries.len()));
        }
        Ok(entries[idx].clone())
    }

    fn entries(&self) -> Result<Vec<TextureEntry>, String> {
        let lines: Vec<&str> = self.raw.text.lines().collect();
        let mut in_texturen = false;
        let mut entries = Vec::new();
        let mut byte_offset = 0;

        for line in lines {
            let line_len_bytes = line.len();
            let trimmed = line.trim();
            if trimmed.eq_ignore_ascii_case("[texturen]") {
                in_texturen = true;
                byte_offset += self.line_total_bytes(byte_offset, line_len_bytes);
                continue;
            }

            if in_texturen {
                if trimmed.starts_with('[') && trimmed.ends_with(']') {
                    break;
                }
                if !trimmed.is_empty() {
                    let rel_start = line.find(trimmed).unwrap();
                    let start = byte_offset + rel_start;
                    let end = start + trimmed.len();
                    entries.push(TextureEntry {
                        start,
                        end,
                        value: trimmed.to_string(),
                    });
                }
            }
            byte_offset += self.line_total_bytes(byte_offset, line_len_bytes);
        }

        if !in_texturen {
            return Err("boden.txt 缺少 [Texturen]。".into());
        }
        Ok(entries)
    }

    fn line_total_bytes(&self, offset: usize, line_len: usize) -> usize {
        let rem = &self.raw.text[offset + line_len..];
        if rem.starts_with("\r\n") {
            line_len + 2
        } else if rem.starts_with('\n') || rem.starts_with('\r') {
            line_len + 1
        } else {
            line_len
        }
    }
}

#[derive(Clone, Debug)]
struct TextureEntry {
    start: usize,
    end: usize,
    value: String,
}

fn validate_texture_name(texture: &str) -> Result<(), String> {
    if texture.trim().is_empty() || texture.contains('\r') || texture.contains('\n') {
        Err("材質名稱不可為空或包含換行。".into())
    } else {
        Ok(())
    }
}
