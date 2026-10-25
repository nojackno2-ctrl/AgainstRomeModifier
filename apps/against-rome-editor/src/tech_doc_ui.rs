//! 技術文件檢視控制項 (TechDoc UI)
//!
//! 對照 C# `src.Modifier/UI/TechDocForm.cs`，以唯讀捲動視窗呈現
//! 根目錄的 `TechDoc.md`（繁體中文）與 `TechDoc_EN.md`（英文）。
//! 透過 `include_str!` 於編譯期內嵌完整文件內容，並支援語系切換與捲動閱讀。

use against_rome_ambience::app_localization::Language;
use eframe::egui::{self, ScrollArea, Vec2};

/// 根目錄繁體中文技術文件
pub const TECH_DOC_ZH: &str = include_str!("../../../TechDoc.md");

/// 根目錄英文技術文件
pub const TECH_DOC_EN: &str = include_str!("../../../TechDoc_EN.md");

/// 技術文件語系選擇
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TechDocLanguage {
    #[default]
    TraditionalChinese,
    English,
}

impl TechDocLanguage {
    pub const ALL: [TechDocLanguage; 2] = [
        TechDocLanguage::TraditionalChinese,
        TechDocLanguage::English,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TechDocLanguage::TraditionalChinese => "繁體中文 (Traditional Chinese)",
            TechDocLanguage::English => "English (英文)",
        }
    }

    /// 對照 C# `TechDocForm` 視窗標題契約:
    /// `Loc.CurrentLanguage == Language.English ? "Technical Document" : "技術文件 (Technical Document)"`
    pub fn window_title(self) -> &'static str {
        match self {
            TechDocLanguage::TraditionalChinese => "技術文件 (Technical Document)",
            TechDocLanguage::English => "Technical Document",
        }
    }
}

impl From<Language> for TechDocLanguage {
    fn from(lang: Language) -> Self {
        match lang {
            Language::TraditionalChinese => Self::TraditionalChinese,
            Language::English => Self::English,
        }
    }
}

impl From<TechDocLanguage> for Language {
    fn from(lang: TechDocLanguage) -> Self {
        match lang {
            TechDocLanguage::TraditionalChinese => Self::TraditionalChinese,
            TechDocLanguage::English => Self::English,
        }
    }
}

/// 技術文件視窗狀態與介面控制項
#[derive(Debug, Clone)]
pub struct TechDocUi {
    pub open: bool,
    pub language: TechDocLanguage,
    search: String,
}

impl Default for TechDocUi {
    fn default() -> Self {
        Self::new()
    }
}

impl TechDocUi {
    pub fn new() -> Self {
        Self {
            open: false,
            language: TechDocLanguage::TraditionalChinese,
            search: String::new(),
        }
    }

    pub fn with_language(language: TechDocLanguage) -> Self {
        Self {
            open: false,
            language,
            search: String::new(),
        }
    }

    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn set_language(&mut self, language: TechDocLanguage) {
        self.language = language;
    }

    /// 取得目前選定語系的內嵌文件內容
    pub fn content(&self) -> &'static str {
        match self.language {
            TechDocLanguage::TraditionalChinese => TECH_DOC_ZH,
            TechDocLanguage::English => TECH_DOC_EN,
        }
    }

    /// 搜尋目前語系，附上原始行號；空白搜尋保留全文。
    fn visible_content(&self) -> String {
        let query = self.search.trim().to_lowercase();
        if query.is_empty() { return self.content().to_owned(); }
        self.content().lines().enumerate()
            .filter(|(_, line)| line.to_lowercase().contains(&query))
            .map(|(index, line)| format!("{}: {}", index + 1, line))
            .collect::<Vec<_>>().join("\n")
    }

    /// 複製目前顯示內容，保留 Markdown 與搜尋行號。
    fn copy_visible(&self, ctx: &egui::Context) {
        ctx.output_mut(|out| out.copied_text = self.visible_content());
    }
    /// 呈現技術文件視窗
    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }

        let mut is_open = self.open;
        let title = self.language.window_title();

        egui::Window::new(title)
            .open(&mut is_open)
            .default_size(Vec2::new(840.0, 620.0))
            .min_size(Vec2::new(400.0, 280.0))
            .resizable(true)
            .show(ctx, |ui| {
                // 頂部控制列：語系切換與文件資訊
                ui.horizontal_wrapped(|ui| {
                    ui.label(match self.language {
                        TechDocLanguage::TraditionalChinese => "語言選擇:",
                        TechDocLanguage::English => "Language:",
                    });
                    for lang in TechDocLanguage::ALL {
                        ui.selectable_value(&mut self.language, lang, lang.label());
                    }

                    ui.separator();

                    let text = self.content();
                    let lines = text.lines().count();
                    let chars = text.chars().count();
                    ui.weak(match self.language {
                        TechDocLanguage::TraditionalChinese => {
                            format!("{lines} 行 · {chars} 字元 (唯讀)")
                        }
                        TechDocLanguage::English => {
                            format!("{lines} lines · {chars} characters (Read-only)")
                        }
                    });
                });

                ui.separator();

                ui.horizontal(|ui| {
                    let english = self.language == TechDocLanguage::English;
                    ui.label(if english { "Search:" } else { "搜尋：" });
                    ui.text_edit_singleline(&mut self.search);
                    if ui.button(if english { "Clear search" } else { "清除搜尋" }).clicked() {
                        self.search.clear();
                    }
                    if ui.button(if english { "Copy displayed text" } else { "複製顯示文字" }).clicked() {
                        self.copy_visible(ctx);
                    }
                });
                let visible = self.visible_content();
                if visible.is_empty() {
                    ui.label(if self.language == TechDocLanguage::English {
                        "No matching lines."
                    } else { "沒有符合的行。" });
                }
                // 唯讀捲動閱讀區：保留換行並支援文字折行與選取複製
                ScrollArea::vertical()
                    .id_source("tech_doc_scroll_area")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(visible)
                                .wrap(true)
                                .selectable(true),
                        );
                    });
            });

        self.open = is_open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_is_case_insensitive_numbered_and_language_specific() {
        let mut doc = TechDocUi::with_language(TechDocLanguage::English);
        doc.search = "  AGAINST ROME  ".into();
        let text = doc.visible_content();
        assert!(!text.is_empty());
        for line in text.lines() {
            let (number, original) = line.split_once(": ").unwrap();
            assert_eq!(doc.content().lines().nth(number.parse::<usize>().unwrap() - 1), Some(original));
            assert!(original.to_lowercase().contains("against rome"));
        }
        doc.search = "找不到的關鍵字__no_match__".into();
        assert!(doc.visible_content().is_empty());
        doc.search.clear();
        assert_eq!(doc.visible_content(), TECH_DOC_EN);
        doc.set_language(TechDocLanguage::TraditionalChinese);
        assert_eq!(doc.visible_content(), TECH_DOC_ZH);
    }

    #[test]
    fn copy_displayed_text_uses_clipboard_output_without_changing_document() {
        let mut doc = TechDocUi::new();
        let ctx = egui::Context::default();
        doc.copy_visible(&ctx);
        assert_eq!(ctx.output(|out| out.copied_text.clone()), TECH_DOC_ZH);
        doc.search = "Against Rome".into();
        doc.copy_visible(&ctx);
        assert_eq!(ctx.output(|out| out.copied_text.clone()), doc.visible_content());
        assert_eq!(doc.content(), TECH_DOC_ZH);
        assert!(!doc.open);
    }


    #[test]
    fn test_embedded_markdown_sources_integrity() {
        assert!(!TECH_DOC_ZH.is_empty(), "TECH_DOC_ZH should not be empty");
        assert!(!TECH_DOC_EN.is_empty(), "TECH_DOC_EN should not be empty");

        assert!(
            TECH_DOC_ZH.contains("# Against Rome Modifier 技術文件"),
            "ZH tech doc should contain header"
        );
        assert!(
            TECH_DOC_ZH.contains("這個修改器還在測試中"),
            "ZH tech doc should contain warning text"
        );

        assert!(
            TECH_DOC_EN.contains("# Against Rome Modifier Complete Technical Document"),
            "EN tech doc should contain header"
        );
        assert!(
            TECH_DOC_EN.contains("This modifier is still in testing"),
            "EN tech doc should contain warning text"
        );

        assert!(TECH_DOC_ZH.lines().count() > 100);
        assert!(TECH_DOC_EN.lines().count() > 100);
    }

    #[test]
    fn test_tech_doc_ui_lifecycle_and_language_switching() {
        let mut ui = TechDocUi::new();
        assert!(!ui.is_open());
        assert_eq!(ui.language, TechDocLanguage::TraditionalChinese);
        assert_eq!(ui.content(), TECH_DOC_ZH);
        assert_eq!(ui.language.window_title(), "技術文件 (Technical Document)");

        ui.open();
        assert!(ui.is_open());

        ui.set_language(TechDocLanguage::English);
        assert_eq!(ui.language, TechDocLanguage::English);
        assert_eq!(ui.content(), TECH_DOC_EN);
        assert_eq!(ui.language.window_title(), "Technical Document");

        ui.toggle();
        assert!(!ui.is_open());

        ui.toggle();
        assert!(ui.is_open());

        ui.close();
        assert!(!ui.is_open());
    }

    #[test]
    fn test_tech_doc_language_conversions() {
        let zh: TechDocLanguage = Language::TraditionalChinese.into();
        assert_eq!(zh, TechDocLanguage::TraditionalChinese);

        let en: TechDocLanguage = Language::English.into();
        assert_eq!(en, TechDocLanguage::English);

        let lang_zh: Language = TechDocLanguage::TraditionalChinese.into();
        assert_eq!(lang_zh, Language::TraditionalChinese);

        let lang_en: Language = TechDocLanguage::English.into();
        assert_eq!(lang_en, Language::English);
    }

    #[test]
    fn test_headless_ui_render_both_languages() {
        let ctx = egui::Context::default();

        // 1. Render closed - should be a no-op
        let mut ui = TechDocUi::new();
        let _ = ctx.run(Default::default(), |ctx| {
            ui.show(ctx);
        });
        assert!(!ui.is_open());

        // 2. Render open Traditional Chinese
        ui.open();
        let _ = ctx.run(Default::default(), |ctx| {
            ui.show(ctx);
        });
        assert!(ui.is_open());

        // 3. Render open English
        ui.set_language(TechDocLanguage::English);
        let _ = ctx.run(Default::default(), |ctx| {
            ui.show(ctx);
        });
        assert!(ui.is_open());
    }
}
