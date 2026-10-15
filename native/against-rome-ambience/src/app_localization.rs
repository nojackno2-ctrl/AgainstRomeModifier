//! Application strings and preference behavior from src.Core/Core/Localization.cs.
//! Explicit instances replace the C# global singleton so callers can isolate settings.
use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}};
#[path = "app_tables.rs"]
mod tables;
pub use tables::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language { #[default] TraditionalChinese, English }
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AppSettings { pub language: Language, pub promoted_features: Vec<String> }
#[derive(Clone, Debug)]
pub struct Loc { pub settings: AppSettings, settings_path: Option<PathBuf> }
impl Default for Loc {
    fn default() -> Self { Self { settings: AppSettings::default(), settings_path: None } }
}
impl Loc {
    pub fn new(language: Language) -> Self { Self { settings: AppSettings { language, ..Default::default() }, ..Default::default() } }
    /// Read errors fall back to UI culture; both legacy numeric and current string languages work.
    pub fn load(path: impl AsRef<Path>, ui_culture: &str) -> Self {
        let mut loc = Self::new(if ui_culture.to_ascii_lowercase().starts_with("en") { Language::English } else { Language::TraditionalChinese });
        loc.settings_path = Some(path.as_ref().to_owned());
        if let Ok(text) = fs::read_to_string(path) {
            if let Ok(serde_json::Value::Object(map)) = serde_json::from_str(&text) {
                if let Some(v) = map.get("Language") {
                    if let Some(n) = v.as_i64() { loc.settings.language = if n == 1 { Language::English } else { Language::TraditionalChinese }; }
                    if let Some(s) = v.as_str() {
                        if s.eq_ignore_ascii_case("English") { loc.settings.language = Language::English; }
                        if s.eq_ignore_ascii_case("TraditionalChinese") { loc.settings.language = Language::TraditionalChinese; }
                    }
                }
                if let Some(items) = map.get("PromotedFeatures").and_then(|v| v.as_array()) {
                    for id in items.iter().filter_map(|v| v.as_str()).filter(|s| !s.is_empty()) {
                        if !loc.is_promoted(id) { loc.settings.promoted_features.push(id.to_owned()); }
                    }
                }
            }
        }
        loc
    }
    /// Same best-effort persistence as C#; no implicit environment/global settings writes.
    pub fn save_settings(&self) {
        if let Some(path) = &self.settings_path {
            let _ = (|| -> std::io::Result<()> {
                if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
                fs::write(path, serde_json::to_vec_pretty(&self.settings)?)
            })();
        }
    }
    pub fn set_language(&mut self, language: Language) {
        if self.settings.language != language { self.settings.language = language; self.save_settings(); }
    }
    pub fn current_language(&self) -> Language { self.settings.language }
    pub fn promoted_features(&self) -> &[String] { &self.settings.promoted_features }
    pub fn reload_language(&mut self, ui_culture: &str) {
        if let Some(path) = &self.settings_path {
            let mut loaded = Self::load(path, ui_culture);
            // Managed code clears promotions only after parsing a JSON object.
            let valid_object = fs::read_to_string(path).ok()
                .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
                .is_some_and(|value| value.is_object());
            if !valid_object { loaded.settings.promoted_features = self.settings.promoted_features.clone(); }
            *self = loaded;
        } else {
            self.settings.language = if ui_culture.to_ascii_lowercase().starts_with("en") { Language::English } else { Language::TraditionalChinese };
        }
    }
    pub fn is_promoted(&self, id: &str) -> bool { self.settings.promoted_features.iter().any(|x| x.eq_ignore_ascii_case(id)) }
    pub fn promote_feature(&mut self, id: &str) { if !self.is_promoted(id) { self.settings.promoted_features.push(id.into()); self.save_settings(); } }
    pub fn demote_feature(&mut self, id: &str) { if self.is_promoted(id) { self.settings.promoted_features.retain(|x| !x.eq_ignore_ascii_case(id)); self.save_settings(); } }
    pub fn get<'a>(&self, key: &'a str) -> &'a str {
        let table = if self.settings.language == Language::English { EN } else { ZH };
        table.iter().find(|(k,_)| *k == key).map(|(_,v)| *v).unwrap_or(key)
    }
    pub fn get_unit_name<'a>(&self, key: &'a str) -> &'a str {
        if self.settings.language == Language::English {
            if let Some((_,v)) = EN_UNIT_NAMES.iter().find(|(k,_)| k.eq_ignore_ascii_case(key)) { return v; }
        }
        ZH_UNIT_NAMES.iter().find(|(k,_)| *k == key).map(|(_,v)| *v).unwrap_or(key)
    }
    fn choose<'a>(&self, en: &'a str, zh: &'a str) -> &'a str { if self.settings.language == Language::English { en } else { zh } }
    pub fn get_unit_type(&self, value: &str) -> &str {
        let (en,zh) = match value {
            "melee_inf" => ("Melee Infantry","近戰步兵"), "ranged_inf" => ("Ranged Infantry","遠程部隊"),
            "ranged_cav" => ("Ranged Cavalry","遠程部隊"), "hybrid_inf" => ("Hybrid Infantry","混合步兵"),
            "cav" => ("Cavalry","騎兵部隊"), "leader_melee" => ("Leader (Melee)","領袖"),
            "leader_cav" => ("Leader (Cavalry)","領袖"), "priest" => ("Priest","祭司"),
            "siege" => ("Siege Weapon","攻城武器"), _ => ("Unknown","未知") };
        self.choose(en,zh)
    }
    pub fn get_style_text(&self, value: &str) -> &str {
        let (en,zh) = match value { "shield" => ("Shielded","持盾"), "two_handed" => ("Two-Handed","雙手武器"), "dual_wield" => ("Dual-Wield","雙持武器"), "ranged" => ("Ranged","遠程"), _ => ("None","無") }; self.choose(en,zh)
    }
    pub fn get_tier_text(&self, value: &str) -> &str {
        let (en,zh) = match value { "low" => ("Low Tier","低階"), "mid" => ("Mid Tier","中階"), "high" => ("High Tier","高階"), "ace" => ("Ace","王牌"), "leader" => ("Leader","領袖"), "siege" => ("Siege","攻城武器"), _ => ("Unknown","未知") }; self.choose(en,zh)
    }
    pub fn get_faction_name<'a>(&self, value: &'a str) -> &'a str {
        if self.settings.language == Language::English { return value; }
        match value { "Roman" => "羅馬", "Teuton" => "條頓", "Celt" => "塞爾特", "Hun" => "匈奴", _ => value }
    }
}
