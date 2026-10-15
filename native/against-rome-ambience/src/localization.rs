//! Briefing PUT AST, validation, translation studio and safe multi-language persistence.
use encoding_rs::{Encoding, BIG5, UTF_8, WINDOWS_1251, WINDOWS_1252};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    time::SystemTime,
};

pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SupportedLanguage {
    TraditionalChinese,
    English,
    German,
    Russian,
}

impl SupportedLanguage {
    pub fn all() -> &'static [SupportedLanguage] {
        &[
            SupportedLanguage::TraditionalChinese,
            SupportedLanguage::English,
            SupportedLanguage::German,
            SupportedLanguage::Russian,
        ]
    }

    pub fn to_directory_code(&self) -> &'static str {
        match self {
            SupportedLanguage::TraditionalChinese => "ZH",
            SupportedLanguage::English => "US",
            SupportedLanguage::German => "DE",
            SupportedLanguage::Russian => "RU",
        }
    }

    pub fn to_encoding_name(&self) -> &'static str {
        match self {
            SupportedLanguage::TraditionalChinese => "BIG5",
            SupportedLanguage::English => "WINDOWS-1252",
            SupportedLanguage::German => "WINDOWS-1252",
            SupportedLanguage::Russian => "WINDOWS-1251",
        }
    }

    pub fn to_display_name(&self) -> &'static str {
        match self {
            SupportedLanguage::TraditionalChinese => "繁體中文 (Traditional Chinese)",
            SupportedLanguage::English => "英文 (English)",
            SupportedLanguage::German => "德文 (German)",
            SupportedLanguage::Russian => "俄文 (Russian)",
        }
    }
}

pub fn strict_encoding(name: &str) -> &'static Encoding {
    match name.to_uppercase().as_str() {
        "UTF-8" | "UTF8" => UTF_8,
        "BIG5" | "950" | "ZH" | "TW" | "CHT" | "TRADITIONALCHINESE" => BIG5,
        "1251" | "CP1251" | "WINDOWS-1251" | "RU" | "RUSSIAN" => WINDOWS_1251,
        _ => WINDOWS_1252,
    }
}

pub fn encode_strict(text: &str, encoding: &'static Encoding) -> Result<Vec<u8>> {
    let (bytes, _, errors) = encoding.encode(text);
    if errors {
        Err(format!("text cannot be encoded as {}", encoding.name()))
    } else {
        Ok(bytes.into_owned())
    }
}

pub struct PutScriptFormatting;

impl PutScriptFormatting {
    pub const DEFAULT_MAX_LITERAL_BYTES: usize = 100;

    pub fn escape(value: &str) -> String {
        value
            .replace('\\', "\\\\")
            .replace('\r', "\\r")
            .replace('\n', "\\n")
            .replace('\t', "\\t")
            .replace('"', "\\\"")
    }

    pub fn unescape(value: &str) -> String {
        let mut out = String::new();
        let mut chars = value.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => out.push('\n'),
                    Some('r') => out.push('\r'),
                    Some('t') => out.push('\t'),
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some(other) => {
                        out.push('\\');
                        out.push(other);
                    }
                    None => out.push('\\'),
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    pub fn split_escaped_into_safe_chunks(
        text: &str,
        encoding: &'static Encoding,
        max: usize,
    ) -> Result<Vec<String>> {
        let mut out = Vec::new();
        let mut start = 0;
        while start < text.len() {
            if encode_strict(&text[start..], encoding)?.len() <= max {
                out.push(Self::unescape(&text[start..]));
                break;
            }
            let mut pos = start;
            let mut last = None;
            while pos < text.len() {
                let c = text[pos..].chars().next().unwrap();
                let mut end = pos + c.len_utf8();
                if c == '\\' && end < text.len() {
                    end += text[end..].chars().next().unwrap().len_utf8();
                }
                if encode_strict(&text[start..end], encoding)?.len() > max {
                    break;
                }
                if text[pos..end] == *" " || text[pos..end] == *"\\n" {
                    last = Some(end);
                }
                pos = end;
            }
            let cut = last.filter(|&b| b - start >= (pos - start) / 2).unwrap_or(pos);
            if cut == start {
                return Err("literal byte limit is smaller than one encoded token".into());
            }
            out.push(Self::unescape(&text[start..cut]));
            start = cut;
        }
        if out.is_empty() {
            out.push(String::new());
        }
        Ok(out)
    }
}

pub fn escape(value: &str) -> String {
    PutScriptFormatting::escape(value)
}

pub fn unescape(value: &str) -> String {
    PutScriptFormatting::unescape(value)
}

pub fn split_escaped_into_safe_chunks(
    text: &str,
    encoding: &'static Encoding,
    max: usize,
) -> Result<Vec<String>> {
    PutScriptFormatting::split_escaped_into_safe_chunks(text, encoding, max)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PutVariableNode {
    pub key: String,
    pub string_value: Option<String>,
    pub integer_value: Option<i32>,
    pub is_composite: bool,
    pub literal_fragments: Vec<String>,
    pub indentation: String,
    pub trailing_comment: String,
    pub has_semicolon: bool,
}

impl PutVariableNode {
    pub fn new(key: &str) -> Self {
        Self {
            key: key.into(),
            string_value: None,
            integer_value: None,
            is_composite: false,
            literal_fragments: vec![],
            indentation: String::new(),
            trailing_comment: String::new(),
            has_semicolon: true,
        }
    }

    pub fn to_script_text(&self) -> String {
        let mut s = format!("{}var:{} =", self.indentation, self.key);
        if let Some(i) = self.integer_value {
            s.push_str(&format!(" {i}"));
        } else if let Some(text) = &self.string_value {
            if self.is_composite || self.literal_fragments.len() > 1 {
                s.push('\n');
                s.push_str(
                    &self
                        .literal_fragments
                        .iter()
                        .map(|f| format!("{}    \"{}\"", self.indentation, escape(f)))
                        .collect::<Vec<_>>()
                        .join("\n"),
                );
            } else {
                s.push_str(&format!("\"{}\"", escape(text)));
            }
        } else {
            s.push_str("\"\"");
        }
        if self.has_semicolon {
            s.push(';');
        }
        if !self.trailing_comment.is_empty() {
            s.push(' ');
            s.push_str(&self.trailing_comment);
        }
        s
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PutScriptNode {
    Comment(String),
    Whitespace(String),
    Variable(PutVariableNode),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PutScriptDocument {
    pub nodes: Vec<PutScriptNode>,
    pub line_ending: String,
}

impl Default for PutScriptDocument {
    fn default() -> Self {
        Self {
            nodes: vec![],
            line_ending: "\r\n".into(),
        }
    }
}

fn declaration(line: &str) -> Option<(&str, &str, &str)> {
    let rest = line.trim_start().strip_prefix("var:")?.trim_start();
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    let key = &rest[..end];
    let value = rest[end..].trim_start().strip_prefix('=')?.trim_start();
    Some((&line[..line.len() - line.trim_start().len()], key, value))
}

fn literals(text: &str) -> Vec<(usize, String)> {
    let mut out = vec![];
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'"' {
            i += 1;
            continue;
        }
        let begin = i;
        i += 1;
        let start = i;
        while i < bytes.len() && bytes[i] != b'"' {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                i += 2;
            } else {
                i += 1;
            }
        }
        if i < bytes.len() {
            out.push((begin, text[start..i].into()));
            i += 1;
        } else {
            break;
        }
    }
    out
}

impl PutScriptDocument {
    pub fn parse(content: &str) -> Result<Self> {
        let mut doc = Self::default();
        if content.is_empty() {
            return Ok(doc);
        }
        doc.line_ending = if content.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        }
        .into();
        let normalized = content.replace("\r\n", "\n");
        let lines: Vec<&str> = normalized.split('\n').collect();
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();
            if trimmed.is_empty() {
                doc.nodes.push(PutScriptNode::Whitespace(line.into()));
                i += 1;
                continue;
            }
            if trimmed.starts_with("//") || trimmed.starts_with(';') {
                doc.nodes.push(PutScriptNode::Comment(line.into()));
                i += 1;
                continue;
            }
            if let Some((indent, key, rest)) = declaration(line) {
                let mut node = PutVariableNode::new(key);
                node.indentation = indent.into();
                let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
                if digits > 0 {
                    node.integer_value = Some(
                        rest[..digits]
                            .parse::<i32>()
                            .map_err(|e| e.to_string())?,
                    );
                    let trail = rest[digits..].trim_start();
                    node.trailing_comment = trail
                        .strip_prefix(';')
                        .unwrap_or(trail)
                        .trim()
                        .into();
                    node.has_semicolon = rest.contains(';');
                    i += 1;
                } else {
                    let mut expr = rest.to_string();
                    let mut current = i;
                    while !expr.contains(';') && current + 1 < lines.len() {
                        current += 1;
                        expr.push('\n');
                        expr.push_str(lines[current]);
                        if lines[current].trim_start().starts_with("var:") {
                            current -= 1;
                            break;
                        }
                    }
                    i = current + 1;
                    node.has_semicolon = expr.contains(';');
                    let code = if let Some((code, trail)) = expr.split_once(';') {
                        node.trailing_comment = trail.trim().into();
                        code
                    } else {
                        &expr
                    };
                    node.literal_fragments = literals(code)
                        .into_iter()
                        .map(|(_, v)| unescape(&v))
                        .collect();
                    node.string_value = Some(node.literal_fragments.concat());
                    node.is_composite = node.literal_fragments.len() > 1 || expr.contains('\n');
                }
                doc.nodes.push(PutScriptNode::Variable(node));
            } else {
                doc.nodes.push(PutScriptNode::Comment(line.into()));
                i += 1;
            }
        }
        Ok(doc)
    }

    pub fn variables(&self) -> impl Iterator<Item = &PutVariableNode> {
        self.nodes.iter().filter_map(|n| {
            if let PutScriptNode::Variable(v) = n {
                Some(v)
            } else {
                None
            }
        })
    }

    pub fn get_variable(&self, key: &str) -> Option<&PutVariableNode> {
        self.variables()
            .find(|v| v.key.eq_ignore_ascii_case(key))
    }

    pub fn get_string_value(&self, key: &str) -> Option<&str> {
        self.get_variable(key)?.string_value.as_deref()
    }

    pub fn get_integer_value(&self, key: &str) -> Option<i32> {
        self.get_variable(key)?.integer_value
    }

    fn variable_mut(&mut self, key: &str) -> &mut PutVariableNode {
        let index = self
            .nodes
            .iter()
            .position(|n| matches!(n, PutScriptNode::Variable(v) if v.key.eq_ignore_ascii_case(key)))
            .unwrap_or_else(|| {
                self.nodes
                    .push(PutScriptNode::Variable(PutVariableNode::new(key)));
                self.nodes.len() - 1
            });
        match &mut self.nodes[index] {
            PutScriptNode::Variable(v) => v,
            _ => unreachable!(),
        }
    }

    pub fn set_variable(
        &mut self,
        key: &str,
        value: &str,
        composite: bool,
        encoding: &'static Encoding,
        max: usize,
    ) -> Result<()> {
        let escaped = escape(value);
        let (fragments, composite) = if composite || encode_strict(&escaped, encoding)?.len() > max {
            let f = split_escaped_into_safe_chunks(&escaped, encoding, max)?;
            let c = f.len() > 1;
            (f, c)
        } else {
            (vec![value.into()], false)
        };
        let node = self.variable_mut(key);
        node.string_value = Some(value.into());
        node.integer_value = None;
        node.is_composite = composite;
        node.literal_fragments = fragments;
        Ok(())
    }

    pub fn set_integer(&mut self, key: &str, value: i32) {
        let node = self.variable_mut(key);
        node.integer_value = Some(value);
        node.string_value = None;
        node.is_composite = false;
        node.literal_fragments.clear();
    }

    pub fn remove_variable(&mut self, key: &str) -> bool {
        if let Some(i) = self
            .nodes
            .iter()
            .position(|n| matches!(n, PutScriptNode::Variable(v) if v.key.eq_ignore_ascii_case(key)))
        {
            self.nodes.remove(i);
            true
        } else {
            false
        }
    }

    pub fn to_script_text(&self) -> String {
        let ending = if self.line_ending == "\n" {
            "\n"
        } else {
            "\r\n"
        };
        let mut text = self
            .nodes
            .iter()
            .map(|n| match n {
                PutScriptNode::Comment(s) | PutScriptNode::Whitespace(s) => s.clone(),
                PutScriptNode::Variable(v) => v.to_script_text(),
            })
            .collect::<Vec<_>>()
            .join(ending);
        if !self.nodes.is_empty() && !text.ends_with(ending) {
            text.push_str(ending);
        }
        text
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BriefingVariableCategory {
    MapHeader,
    BriefingBody,
    AudioVideo,
    TeamName,
    Debriefing,
    Objective,
    Custom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BriefingVariableType {
    SingleLineString,
    CompositeString,
    Integer,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BriefingVariableDescriptor {
    pub key: String,
    pub name_zh: String,
    pub name_en: String,
    pub name_de: String,
    pub description_zh: String,
    pub description_en: String,
    pub description_de: String,
    pub category: BriefingVariableCategory,
    pub value_type: BriefingVariableType,
    pub is_required: bool,
    pub default_value: String,
    pub max_literal_bytes: usize,
}

impl BriefingVariableDescriptor {
    pub fn display_name(&self, code: &str) -> &str {
        match code.to_uppercase().as_str() {
            "DE" | "GR" => &self.name_de,
            "EN" | "US" => &self.name_en,
            _ => &self.name_zh,
        }
    }
}

pub const MAX_TEAM_COUNT: usize = 8;

pub struct BriefingTextTableCatalog;

impl BriefingTextTableCatalog {
    pub fn get_team_key(index: i32) -> String {
        team_key(index)
    }

    pub fn try_get_team_index(key: &str) -> Option<i32> {
        team_index(key)
    }

    pub fn get_objective_key(slot: &str, index: i32) -> String {
        objective_key(slot, index)
    }

    pub fn try_get_objective_index(key: &str) -> Option<(String, i32)> {
        objective_index(key)
    }

    pub fn get_descriptor(key: &str) -> BriefingVariableDescriptor {
        descriptor(key)
    }

    pub fn try_get_descriptor(key: &str) -> Option<BriefingVariableDescriptor> {
        standard_descriptors()
            .into_iter()
            .find(|d| d.key.eq_ignore_ascii_case(key))
            .or_else(|| {
                if team_index(key).is_some() || objective_index(key).is_some() {
                    Some(descriptor(key))
                } else {
                    None
                }
            })
    }

    pub fn get_all_standard_descriptors() -> Vec<BriefingVariableDescriptor> {
        standard_descriptors()
    }
}

pub fn team_key(index: i32) -> String {
    format!("briefing_text_teamname{index}")
}

pub fn team_index(key: &str) -> Option<i32> {
    let lower = key.to_ascii_lowercase();
    let n = lower.strip_prefix("briefing_text_teamname")?;
    if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
        None
    } else {
        n.parse().ok()
    }
}

pub fn objective_key(slot: &str, index: i32) -> String {
    let slot = if slot.trim().is_empty() {
        "000".into()
    } else {
        slot.to_lowercase().replace("endl_", "")
    };
    format!("endl_{slot}_text_{index:02}")
}

pub fn objective_index(key: &str) -> Option<(String, i32)> {
    let lower = key.to_ascii_lowercase();
    let rest = lower.strip_prefix("endl_")?;
    let (slot, index) = rest.split_once("_text_")?;
    if slot.is_empty()
        || !slot.bytes().all(|b| b.is_ascii_alphanumeric())
        || index.is_empty()
        || !index.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    Some((key[5..5 + slot.len()].into(), index.parse().ok()?))
}

include!("localization/catalog.rs");

pub fn descriptor(key: &str) -> BriefingVariableDescriptor {
    if let Some(d) = standard_descriptors()
        .into_iter()
        .find(|d| d.key.eq_ignore_ascii_case(key))
    {
        return d;
    }
    let mut d = BriefingVariableDescriptor {
        key: key.into(),
        name_zh: format!("自訂變數 ({key})"),
        name_en: format!("Custom ({key})"),
        name_de: format!("Benutzerdefiniert ({key})"),
        description_zh: "使用者自訂或腳本專用變數".into(),
        description_en: "User defined or script specific variable".into(),
        description_de: "Benutzerdefinierte oder skriptspezifische Variable".into(),
        category: BriefingVariableCategory::Custom,
        value_type: BriefingVariableType::CompositeString,
        is_required: false,
        default_value: String::new(),
        max_literal_bytes: 100,
    };
    if let Some(i) = team_index(key) {
        d.category = BriefingVariableCategory::TeamName;
        d.value_type = BriefingVariableType::SingleLineString;
        d.name_zh = format!("隊伍 {i} 名稱");
        d.name_en = format!("Team {i} Name");
        d.name_de = d.name_en.clone();
        d.description_zh.clear();
        d.description_en.clear();
        d.description_de.clear();
        d.default_value = format!("Team {i}");
    } else if let Some((slot, i)) = objective_index(key) {
        d.category = BriefingVariableCategory::Objective;
        d.name_zh = format!("任務目標 {i}");
        d.name_en = format!("Objective {i}");
        d.name_de = format!("Missionsziel {i}");
        d.description_zh = format!("槽位 {slot} 任務目標第 {i} 項敘述");
        d.description_en = format!("Objective description {i} for slot {slot}");
        d.description_de = format!("Zielbeschreibung {i} für Slot {slot}");
    }
    d
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BriefingTextTable {
    pub document: PutScriptDocument,
}

impl BriefingTextTable {
    pub fn create_empty() -> Self {
        Self::default()
    }

    pub fn parse(text: &str) -> Result<Self> {
        Ok(Self {
            document: PutScriptDocument::parse(text)?,
        })
    }

    pub fn get_value(&self, key: &str) -> Option<&str> {
        self.document.get_string_value(key)
    }

    pub fn set_value(&mut self, key: &str, value: &str, is_composite: bool) {
        let _ = self.document.set_variable(
            key,
            value,
            is_composite || descriptor(key).value_type == BriefingVariableType::CompositeString,
            UTF_8,
            100,
        );
    }

    pub fn all_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = vec![];
        for v in self.document.variables() {
            if !keys.iter().any(|k| k.eq_ignore_ascii_case(&v.key)) {
                keys.push(v.key.clone());
            }
        }
        keys
    }

    pub fn to_script_text(&self) -> String {
        self.document.to_script_text()
    }

    // Properties for convenient editor access
    pub fn title1(&self) -> Option<&str> {
        self.get_value("briefing_titel_1")
    }
    pub fn set_title1(&mut self, val: &str) {
        self.set_value("briefing_titel_1", val, false);
    }

    pub fn title2(&self) -> Option<&str> {
        self.get_value("briefing_titel_2")
    }
    pub fn set_title2(&mut self, val: &str) {
        self.set_value("briefing_titel_2", val, false);
    }

    pub fn briefing_text(&self) -> Option<&str> {
        self.get_value("briefing_text")
    }
    pub fn set_briefing_text(&mut self, val: &str) {
        self.set_value("briefing_text", val, true);
    }

    pub fn sample_name(&self) -> Option<&str> {
        self.get_value("briefing_text_sample_name")
    }
    pub fn set_sample_name(&mut self, val: &str) {
        self.set_value("briefing_text_sample_name", val, false);
    }

    pub fn video_number(&self) -> Option<i32> {
        self.document.get_integer_value("briefing_video_nummmer")
    }
    pub fn set_video_number(&mut self, val: i32) {
        self.document.set_integer("briefing_video_nummmer", val);
    }

    pub fn debriefing_win(&self) -> Option<&str> {
        self.get_value("debriefing_text_win")
    }
    pub fn set_debriefing_win(&mut self, val: &str) {
        self.set_value("debriefing_text_win", val, true);
    }

    pub fn debriefing_lose(&self) -> Option<&str> {
        self.get_value("debriefing_text_loss")
    }
    pub fn set_debriefing_lose(&mut self, val: &str) {
        self.set_value("debriefing_text_loss", val, true);
    }

    pub fn get_team_name(&self, index: i32) -> Option<&str> {
        self.get_value(&team_key(index))
    }
    pub fn set_team_name(&mut self, index: i32, name: &str) {
        self.set_value(&team_key(index), name, false);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PutIssueSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PutScriptIssue {
    pub key: String,
    pub severity: PutIssueSeverity,
    pub message: String,
    pub line_number: usize,
    pub column: usize,
    pub offending_fragment: Option<String>,
    pub suggested_fix: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PutScriptValidationReport {
    pub issues: Vec<PutScriptIssue>,
}

impl PutScriptValidationReport {
    pub fn is_valid(&self) -> bool {
        self.error_count() == 0
    }
    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == PutIssueSeverity::Error)
            .count()
    }
    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == PutIssueSeverity::Warning)
            .count()
    }
    fn add(
        &mut self,
        key: &str,
        severity: PutIssueSeverity,
        message: String,
        line: usize,
        col: usize,
        fragment: Option<String>,
    ) {
        self.issues.push(PutScriptIssue {
            key: key.into(),
            severity,
            message,
            line_number: line,
            column: col,
            offending_fragment: fragment,
            suggested_fix: None,
        });
    }
    fn error(&mut self, key: &str, message: String, line: usize, col: usize) {
        self.add(key, PutIssueSeverity::Error, message, line, col, None);
    }
}

pub struct PutScriptSyntaxValidator;

impl PutScriptSyntaxValidator {
    pub fn get_strict_encoding(name: &str) -> &'static Encoding {
        strict_encoding(name)
    }

    pub fn validate_script_text(script: &str, target: &str) -> PutScriptValidationReport {
        validate_script_text(script, target)
    }

    pub fn validate_table(table: &BriefingTextTable, target: &str) -> PutScriptValidationReport {
        validate_table(table, target)
    }
}

fn check_encoding(
    key: &str,
    text: &str,
    enc: &'static Encoding,
    line: usize,
    col: usize,
    report: &mut PutScriptValidationReport,
) {
    let mut chars = text.chars();
    let mut offset = 0;
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                offset += 1 + next.len_utf16();
                continue;
            }
        }
        if encode_strict(&c.to_string(), enc).is_err() {
            report.add(
                key,
                PutIssueSeverity::Error,
                format!(
                    "字元 '{c}' (U+{:04X}) 無法由目標編碼 {} 編碼，遊戲將顯示亂碼或崩潰！",
                    c as u32,
                    enc.name().to_ascii_lowercase()
                ),
                line,
                col + offset,
                Some(c.to_string()),
            );
        }
        offset += c.len_utf16();
    }
}

fn check_backslashes(key: &str, text: &str, line: usize, report: &mut PutScriptValidationReport) {
    let mut chars = text.chars();
    let mut col = 1;
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                None => report.error(key, "行末存在懸空的反斜線 (\\)。".into(), line, col),
                Some(n) => {
                    if !matches!(n, 'n' | 'r' | 't' | '"' | '\\') {
                        report.add(
                            key,
                            PutIssueSeverity::Warning,
                            format!("非標準的跳脫序列 '\\{n}'。"),
                            line,
                            col,
                            None,
                        );
                    }
                    col += n.len_utf16();
                }
            }
        }
        col += c.len_utf16();
    }
}

fn quote_count(text: &str) -> usize {
    let mut last = '\0';
    let mut count = 0;
    for c in text.chars() {
        if c == '"' && last != '\\' {
            count += 1;
        }
        last = c;
    }
    count
}

pub fn validate_script_text(script: &str, target: &str) -> PutScriptValidationReport {
    let mut report = PutScriptValidationReport::default();
    if script.trim().is_empty() {
        report.add("", PutIssueSeverity::Warning, "腳本內容為空。".into(), 0, 0, None);
        return report;
    }
    let encoding = strict_encoding(target);
    let mut multi = false;
    let mut key = String::new();
    let mut start = 0;
    for (i, line) in script.replace("\r\n", "\n").split('\n').enumerate() {
        let n = i + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with(';') {
            continue;
        }
        if let Some(p) = line.find('\0') {
            report.error(
                &key,
                "字串中包含非法的 NUL (\\0) 字元，會造成 C 字串提前截斷。".into(),
                n,
                line[..p].encode_utf16().count() + 1,
            );
        }
        if let Some((_, k, rest)) = declaration(line) {
            key = k.into();
            check_backslashes(&key, rest, n, &mut report);
            if quote_count(rest) % 2 != 0 {
                multi = true;
                start = n;
            } else if !rest.contains(';') {
                report.error(
                    &key,
                    format!("變數 '{key}' 宣告缺少結尾分號 (;)。"),
                    n,
                    line.encode_utf16().count(),
                );
            }
        } else if multi {
            check_backslashes(&key, line, n, &mut report);
            if quote_count(line) % 2 != 0 {
                multi = false;
                if !line.contains(';') {
                    report.error(
                        &key,
                        format!("多行字串於第 {n} 行閉合但缺少分號 (;)。"),
                        n,
                        line.encode_utf16().count(),
                    );
                }
            }
        }
        for (p, literal) in literals(line) {
            let col = line[..p].encode_utf16().count() + 1;
            check_encoding(&key, &literal, encoding, n, col, &mut report);
            let len = encode_strict(&literal, encoding)
                .map(|v| v.len())
                .unwrap_or(literal.len());
            if len > 100 {
                report.add(
                    &key,
                    PutIssueSeverity::Error,
                    format!("字串常值位元組長度 ({len} bytes) 超過遊戲緩衝區上限 (100 bytes)，進入關卡會導致堆疊/堆積崩潰！"),
                    n,
                    col,
                    Some(literal.chars().take(20).collect()),
                );
            }
        }
    }
    if multi {
        report.error(
            &key,
            format!("字串從第 {start} 行開始未正確閉合雙引號 (\") 直至檔案結尾。"),
            start,
            1,
        );
    }
    report
}

pub fn validate_table(table: &BriefingTextTable, target: &str) -> PutScriptValidationReport {
    let mut r = PutScriptValidationReport::default();
    let enc = strict_encoding(target);
    for node in table.document.variables() {
        let d = descriptor(&node.key);
        if d.is_required && node.string_value.as_deref().unwrap_or("").trim().is_empty() {
            r.error(
                &node.key,
                format!("必要變數 '{}' ({}) 不得為空。", node.key, d.name_zh),
                0,
                0,
            );
        }
        if let Some(s) = &node.string_value {
            if d.value_type == BriefingVariableType::SingleLineString && s.contains(['\r', '\n']) {
                r.error(
                    &node.key,
                    format!("單行文字欄位 '{}' 不得包含換行字元。", node.key),
                    0,
                    0,
                );
            }
            check_encoding(&node.key, s, enc, 0, 0, &mut r);
            for f in &node.literal_fragments {
                let e = escape(f);
                let n = encode_strict(&e, enc).map(|b| b.len()).unwrap_or(e.len());
                if n > d.max_literal_bytes {
                    r.add(
                        &node.key,
                        PutIssueSeverity::Error,
                        format!(
                            "變數 '{}' 片段位元組長度 ({n}) 超過上限 ({})。",
                            node.key, d.max_literal_bytes
                        ),
                        0,
                        0,
                        Some(f.chars().take(15).collect()),
                    );
                }
            }
        }
    }
    r
}

// =========================================================================
// MultiLanguageTextStudio & TranslationGlossary
// =========================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlotSyncStatus {
    Synchronized,
    Missing,
    Stale,
    EncodingError,
    SyntaxError,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StudioLanguageSlot {
    pub language: SupportedLanguage,
    pub text: String,
    pub original_text: String,
    pub status: SlotSyncStatus,
    pub last_modified: SystemTime,
}

impl StudioLanguageSlot {
    pub fn new(language: SupportedLanguage, initial_text: &str) -> Self {
        let status = if initial_text.trim().is_empty() {
            SlotSyncStatus::Missing
        } else {
            SlotSyncStatus::Synchronized
        };
        Self {
            language,
            text: initial_text.into(),
            original_text: initial_text.into(),
            status,
            last_modified: SystemTime::now(),
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.text != self.original_text
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StudioTextEntry {
    pub key: String,
    pub descriptor: BriefingVariableDescriptor,
    pub slots: BTreeMap<SupportedLanguage, StudioLanguageSlot>,
    pub master_version: i32,
    pub aligned_versions: BTreeMap<SupportedLanguage, i32>,
}

impl StudioTextEntry {
    pub fn new(key: &str, descriptor: BriefingVariableDescriptor) -> Self {
        let mut slots = BTreeMap::new();
        let mut aligned_versions = BTreeMap::new();
        for &lang in SupportedLanguage::all() {
            slots.insert(lang, StudioLanguageSlot::new(lang, ""));
            aligned_versions.insert(lang, 0);
        }
        Self {
            key: key.into(),
            descriptor,
            slots,
            master_version: 1,
            aligned_versions,
        }
    }

    pub fn get_slot(&self, lang: SupportedLanguage) -> &StudioLanguageSlot {
        &self.slots[&lang]
    }

    pub fn get_slot_mut(&mut self, lang: SupportedLanguage) -> &mut StudioLanguageSlot {
        self.slots.get_mut(&lang).unwrap()
    }

    pub fn get_text(&self, lang: SupportedLanguage) -> &str {
        &self.slots[&lang].text
    }

    pub fn set_text(&mut self, lang: SupportedLanguage, text: &str) {
        let slot = self.slots.get_mut(&lang).unwrap();
        slot.text = text.into();
        slot.last_modified = SystemTime::now();
    }
}

#[derive(Clone, Debug)]
pub struct MultiLanguageTextStudio {
    entries: BTreeMap<String, StudioTextEntry>,
    pub master_language: SupportedLanguage,
}

impl Default for MultiLanguageTextStudio {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
            master_language: SupportedLanguage::TraditionalChinese,
        }
    }
}

impl MultiLanguageTextStudio {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn entries(&self) -> impl Iterator<Item = &StudioTextEntry> {
        self.entries.values()
    }

    pub fn get_entry(&self, key: &str) -> Option<&StudioTextEntry> {
        self.entries.values().find(|e| e.key.eq_ignore_ascii_case(key))
    }

    pub fn get_or_create_entry(&mut self, key: &str) -> &mut StudioTextEntry {
        if !self.entries.iter().any(|(k, _)| k.eq_ignore_ascii_case(key)) {
            let desc = BriefingTextTableCatalog::get_descriptor(key);
            let entry = StudioTextEntry::new(key, desc);
            self.entries.insert(key.to_string(), entry);
        }
        self.entries.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, e)| e).unwrap()
    }

    pub fn load_from_tables(&mut self, tables: &BTreeMap<SupportedLanguage, BriefingTextTable>) {
        let mut all_keys = Vec::new();
        for desc in BriefingTextTableCatalog::get_all_standard_descriptors() {
            if !all_keys.iter().any(|k: &String| k.eq_ignore_ascii_case(&desc.key)) {
                all_keys.push(desc.key);
            }
        }
        for table in tables.values() {
            for k in table.all_keys() {
                if !all_keys.iter().any(|existing: &String| existing.eq_ignore_ascii_case(&k)) {
                    all_keys.push(k);
                }
            }
        }

        for key in all_keys {
            let entry = self.get_or_create_entry(&key);
            for (&lang, table) in tables {
                let val = table.get_value(&key).unwrap_or("");
                let slot = entry.get_slot_mut(lang);
                slot.text = val.to_string();
                slot.original_text = val.to_string();
                slot.status = if val.trim().is_empty() {
                    SlotSyncStatus::Missing
                } else {
                    SlotSyncStatus::Synchronized
                };
            }
        }

        self.refresh_parity_status();
    }

    pub fn export_to_tables(&self) -> BTreeMap<SupportedLanguage, BriefingTextTable> {
        let mut result = BTreeMap::new();
        for &lang in SupportedLanguage::all() {
            let mut table = BriefingTextTable::create_empty();
            for entry in self.entries.values() {
                let text = entry.get_text(lang);
                if !text.trim().is_empty() || entry.descriptor.is_required {
                    table.set_value(
                        &entry.key,
                        text,
                        entry.descriptor.value_type == BriefingVariableType::CompositeString,
                    );
                }
            }
            result.insert(lang, table);
        }
        result
    }

    pub fn update_text(&mut self, key: &str, lang: SupportedLanguage, new_text: &str) {
        let master = self.master_language;
        let entry = self.get_or_create_entry(key);
        entry.set_text(lang, new_text);

        if lang == master {
            entry.master_version += 1;
            entry.aligned_versions.insert(lang, entry.master_version);

            for &other_lang in SupportedLanguage::all() {
                if other_lang == master {
                    continue;
                }
                let other_slot = entry.get_slot_mut(other_lang);
                if other_slot.text.trim().is_empty() {
                    other_slot.status = SlotSyncStatus::Missing;
                } else {
                    other_slot.status = SlotSyncStatus::Stale;
                }
            }
        } else {
            entry.aligned_versions.insert(lang, entry.master_version);
            let slot = entry.get_slot_mut(lang);
            slot.status = if new_text.trim().is_empty() {
                SlotSyncStatus::Missing
            } else {
                SlotSyncStatus::Synchronized
            };
        }

        validate_studio_slot(entry, lang);
    }

    pub fn refresh_parity_status(&mut self) {
        let master = self.master_language;
        for entry in self.entries.values_mut() {
            let has_master = !entry.get_text(master).trim().is_empty();

            for &lang in SupportedLanguage::all() {
                if lang == master {
                    entry.get_slot_mut(lang).status = if has_master {
                        SlotSyncStatus::Synchronized
                    } else {
                        SlotSyncStatus::Missing
                    };
                } else {
                    let aligned = entry.aligned_versions.get(&lang).copied().unwrap_or(0);
                    let stale = aligned < entry.master_version;
                    let slot = entry.get_slot_mut(lang);
                    if slot.text.trim().is_empty() {
                        slot.status = SlotSyncStatus::Missing;
                    } else if stale {
                        slot.status = SlotSyncStatus::Stale;
                    } else {
                        slot.status = SlotSyncStatus::Synchronized;
                    }
                }
                validate_studio_slot(entry, lang);
            }
        }
    }

    pub fn calculate_coverage(&self, lang: SupportedLanguage) -> f64 {
        let total = self.entries.len();
        if total == 0 {
            return 100.0;
        }
        let filled = self
            .entries
            .values()
            .filter(|e| !e.get_text(lang).trim().is_empty())
            .count();
        (filled as f64) / (total as f64) * 100.0
    }

    pub fn get_missing_or_stale_keys(&self, lang: SupportedLanguage) -> Vec<String> {
        self.entries
            .values()
            .filter(|e| matches!(e.get_slot(lang).status, SlotSyncStatus::Missing | SlotSyncStatus::Stale))
            .map(|e| e.key.clone())
            .collect()
    }
}

fn validate_studio_slot(entry: &mut StudioTextEntry, lang: SupportedLanguage) {
    let single_line = entry.descriptor.value_type == BriefingVariableType::SingleLineString;
    let slot = entry.get_slot_mut(lang);
    if slot.text.trim().is_empty() {
        return;
    }
    let encoding = strict_encoding(lang.to_encoding_name());
    if encode_strict(&slot.text, encoding).is_err() {
        slot.status = SlotSyncStatus::EncodingError;
        return;
    }
    if single_line
        && (slot.text.contains('\r') || slot.text.contains('\n'))
    {
        slot.status = SlotSyncStatus::SyntaxError;
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlossaryTerm {
    pub zh: &'static str,
    pub en: &'static str,
    pub de: &'static str,
}

impl GlossaryTerm {
    pub fn get_text(&self, lang: SupportedLanguage) -> &'static str {
        match lang {
            SupportedLanguage::TraditionalChinese => self.zh,
            SupportedLanguage::English => self.en,
            SupportedLanguage::German => self.de,
            _ => self.en,
        }
    }
}

pub struct TranslationGlossary;

const GLOSSARY_TERMS: &[GlossaryTerm] = &[
    // 陣營與派系
    GlossaryTerm { zh: "羅馬人", en: "Romans", de: "Römer" },
    GlossaryTerm { zh: "日耳曼人", en: "Germanics", de: "Germanen" },
    GlossaryTerm { zh: "凱爾特人", en: "Celts", de: "Kelten" },
    GlossaryTerm { zh: "匈人", en: "Huns", de: "Hunnen" },
    // 建築與聚落
    GlossaryTerm { zh: "主屋", en: "Chieftain House", de: "Haupthaus" },
    GlossaryTerm { zh: "聚落", en: "Settlement", de: "Siedlung" },
    GlossaryTerm { zh: "兵營", en: "Barracks", de: "Kaserne" },
    GlossaryTerm { zh: "瞭望塔", en: "Watchtower", de: "Wachturm" },
    GlossaryTerm { zh: "木樁柵欄", en: "Palisade", de: "Palisade" },
    GlossaryTerm { zh: "鐵匠鋪", en: "Blacksmith", de: "Schmiede" },
    GlossaryTerm { zh: "神殿", en: "Temple", de: "Tempel" },
    // 部隊與角色
    GlossaryTerm { zh: "步兵", en: "Infantry", de: "Infanterie" },
    GlossaryTerm { zh: "弓箭手", en: "Archers", de: "Bogenschützen" },
    GlossaryTerm { zh: "騎兵", en: "Cavalry", de: "Kavallerie" },
    GlossaryTerm { zh: "首領", en: "Chieftain", de: "Häuptling" },
    GlossaryTerm { zh: "祭司", en: "Druid", de: "Priester" },
    // 任務目標與遊戲狀態
    GlossaryTerm { zh: "消滅所有敵方部隊", en: "Eliminate all enemy troops", de: "Vernichte alle feindlichen Truppen" },
    GlossaryTerm { zh: "摧毀敵方聚落", en: "Destroy enemy settlement", de: "Zerstöre die feindliche Siedlung" },
    GlossaryTerm { zh: "保護首領生存", en: "Protect the chieftain", de: "Beschütze den Häuptling" },
    GlossaryTerm { zh: "佔領祭壇", en: "Capture the altar", de: "Erobere den Altar" },
    GlossaryTerm { zh: "勝利", en: "Victory", de: "Sieg" },
    GlossaryTerm { zh: "失敗", en: "Defeat", de: "Niederlage" },
    GlossaryTerm { zh: "平手", en: "Draw", de: "Unentschieden" },
];

impl TranslationGlossary {
    pub fn translate(source_text: &str, from: SupportedLanguage, to: SupportedLanguage) -> String {
        if source_text.trim().is_empty() || from == to {
            return source_text.into();
        }
        let mut result = source_text.to_string();
        for term in GLOSSARY_TERMS {
            let from_str = term.get_text(from);
            let to_str = term.get_text(to);
            if !from_str.is_empty() && !to_str.is_empty() {
                result = replace_case_insensitive(&result, from_str, to_str);
            }
        }
        result
    }

    pub fn find_matching_terms(text: &str) -> Vec<GlossaryTerm> {
        if text.trim().is_empty() {
            return vec![];
        }
        let text_lower = text.to_lowercase();
        GLOSSARY_TERMS
            .iter()
            .filter(|t| {
                text_lower.contains(&t.zh.to_lowercase())
                    || text_lower.contains(&t.en.to_lowercase())
                    || text_lower.contains(&t.de.to_lowercase())
            })
            .cloned()
            .collect()
    }
}

fn replace_case_insensitive(haystack: &str, from: &str, to: &str) -> String {
    let mut result = String::new();
    let mut last_idx = 0;
    let haystack_lower = haystack.to_lowercase();
    let from_lower = from.to_lowercase();

    while let Some(idx) = haystack_lower[last_idx..].find(&from_lower) {
        let abs_idx = last_idx + idx;
        result.push_str(&haystack[last_idx..abs_idx]);
        result.push_str(to);
        last_idx = abs_idx + from.len();
    }
    result.push_str(&haystack[last_idx..]);
    result
}

// =========================================================================
// BriefingSaveTransaction
// =========================================================================

pub struct BriefingSaveTransaction;

impl BriefingSaveTransaction {
    pub fn preflight_validate(
        language_tables: &BTreeMap<SupportedLanguage, BriefingTextTable>,
    ) -> Result<()> {
        for (&lang, table) in language_tables {
            let encoding_name = lang.to_encoding_name();
            let report = validate_table(table, encoding_name);
            if !report.is_valid() {
                let first_error = report
                    .issues
                    .iter()
                    .find(|i| i.severity == PutIssueSeverity::Error)
                    .unwrap();
                let fragment_info = first_error
                    .offending_fragment
                    .as_deref()
                    .map(|f| format!(" (片段: {f})"))
                    .unwrap_or_default();
                return Err(format!(
                    "[{}] 簡報變數 '{}' 驗證失敗: {}{}",
                    lang.to_display_name(),
                    first_error.key,
                    first_error.message,
                    fragment_info
                ));
            }
        }
        Ok(())
    }

    pub fn prepare_briefing_bytes(
        existing_bytes: Option<&[u8]>,
        new_table: &BriefingTextTable,
        encoding: &'static Encoding,
    ) -> Result<Vec<u8>> {
        let mut doc_to_save;
        let is_pfil = existing_bytes
            .map(|b| b.len() >= 64 && &b[..4] == b"PFIL")
            .unwrap_or(false);

        if let Some(orig) = existing_bytes {
            let text_bytes = if is_pfil {
                against_rome_data::lzss::decompress_pfil(orig, None).map_err(|e| e.to_string())?
            } else {
                orig.to_vec()
            };
            let (original_text, _, _) = encoding.decode(&text_bytes);
            doc_to_save = PutScriptDocument::parse(&original_text)?;

            for new_var in new_table.document.variables() {
                if let Some(i) = new_var.integer_value {
                    doc_to_save.set_integer(&new_var.key, i);
                } else if let Some(s) = &new_var.string_value {
                    doc_to_save.set_variable(&new_var.key, s, new_var.is_composite, encoding, 100)?;
                }
            }
        } else {
            doc_to_save = new_table.document.clone();
        }

        let serialized_text = doc_to_save.to_script_text();
        let raw_bytes = encode_strict(&serialized_text, encoding)?;

        if is_pfil {
            if let Some(orig) = existing_bytes {
                return pack_pfil(&raw_bytes, &orig);
            }
        }

        Ok(raw_bytes)
    }

    pub fn save_multi_language_briefings(
        map_directory: impl AsRef<Path>,
        language_tables: &BTreeMap<SupportedLanguage, BriefingTextTable>,
    ) -> Result<()> {
        Self::preflight_validate(language_tables)?;

        let text_root = map_directory.as_ref().join("TEXT");
        for (&lang, table) in language_tables {
            let lang_dir = text_root.join(lang.to_directory_code());
            fs::create_dir_all(&lang_dir).map_err(|e| e.to_string())?;
            let briefing_path = lang_dir.join("briefing.put");

            let existing_bytes = fs::read(&briefing_path).ok();
            let encoding = strict_encoding(lang.to_encoding_name());
            let final_bytes =
                Self::prepare_briefing_bytes(existing_bytes.as_deref(), table, encoding)?;

            fs::write(&briefing_path, final_bytes).map_err(|e| e.to_string())?;
        }

        Ok(())
    }
}

/// Literal-only LZSS repack of a PFIL file, reusing the 64-byte header of `template`.
fn pack_pfil(raw: &[u8], template: &[u8]) -> std::result::Result<Vec<u8>, String> {
    let mut out = template.get(..64).ok_or_else(|| "truncated PFIL header".to_string())?.to_vec();
    out[16..20].copy_from_slice(&(raw.len() as u32).to_le_bytes());
    for chunk in raw.chunks(8) {
        out.push(((1u16 << chunk.len()) - 1) as u8);
        out.extend_from_slice(chunk);
    }
    Ok(out)
}
