//! 工作區介面字面值檢查：解析 Rust 語法，涵蓋多行呼叫、格式字串與 RichText。
use std::{fs, path::Path};
use syn::{
    spanned::Spanned,
    visit::{self, Visit},
    Expr, Lit,
};

// 僅允許無須翻譯的技術名稱、座標、快捷鍵與格式占位符。
const ALLOWED: &[&str] = &[
    "X",
    "Y",
    "Z",
    "XYZ",
    "F",
    "I",
    "F5",
    "PageUp / PageDn",
    "Home",
    "Ctrl+Z / Ctrl+Y",
    "English",
    "T{}",
    "{}",
    "{} {}",
    "{}{}",
    "{} {index}:",
    "{}: {:?}",
    "  - {}",
    // 語言代碼、版本前綴、原生相機 API 名稱與識別碼單位。
    "EN",
    "v{VERSION}",
    "CenterToPos",
    "DirectEnginePos",
    "ID {}",
    // 編碼、檔案別名、UID 與尺寸分隔符；周圍說明仍須翻譯。
    "{} {} | {} CP1251 (PFIL)",
    "{} {} (UID {}): {} {}, {} {}, {} {}, {} {}",
    "{} {}x{} {}",
    "{} ENDL_{:03}?",
    "{}: ENDL_{:03}",
];

#[derive(Default)]
struct UiLiterals {
    found: Vec<(String, proc_macro2::Span)>,
}

impl UiLiterals {
    fn text(&mut self, e: &Expr) {
        match e {
            Expr::Lit(v) => {
                if let Lit::Str(s) = &v.lit {
                    let text = s.value();
                    // 忽略中文、純符號，以及格式參數中的 Rust 識別字。
                    let mut depth = 0;
                    let prose: String = text
                        .chars()
                        .filter(|&c| {
                            if c == '{' {
                                depth += 1;
                            }
                            let keep = depth == 0;
                            if c == '}' {
                                depth -= 1;
                            }
                            keep
                        })
                        .collect();
                    if prose.chars().any(|c| c.is_ascii_alphabetic())
                        && !text.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c))
                        && !ALLOWED.contains(&text.as_str())
                    {
                        self.found.push((text, e.span()));
                    }
                }
            }
            Expr::Macro(m) if m.mac.path.is_ident("format") => {
                use syn::parse::Parser;
                if let Ok(args) =
                    syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
                        .parse2(m.mac.tokens.clone())
                {
                    let before = self.found.len();
                    if let Some(first) = args.first() {
                        self.text(first);
                    }
                    for item in &mut self.found[before..] {
                        item.1 = e.span();
                    }
                }
            }
            Expr::Call(c) if !is_translation(e) => {
                // RichText::new 等顯示包裝器。
                if let Expr::Path(p) = &*c.func {
                    if p.path.segments.iter().any(|s| s.ident == "RichText") {
                        for arg in &c.args {
                            self.text(arg);
                        }
                    }
                }
            }
            Expr::MethodCall(c) => self.text(&c.receiver),
            Expr::Reference(r) => self.text(&r.expr),
            Expr::Paren(p) => self.text(&p.expr),
            Expr::If(i) if !language_condition(&i.cond) || !has_translation(e) => {
                for s in &i.then_branch.stmts {
                    if let syn::Stmt::Expr(e, _) = s {
                        self.text(e);
                    }
                }
                if let Some((_, e)) = &i.else_branch {
                    self.text(e);
                }
            }
            Expr::Block(b) => {
                for s in &b.block.stmts {
                    if let syn::Stmt::Expr(e, _) = s {
                        self.text(e);
                    }
                }
            }
            Expr::Match(m) if !language_condition(&m.expr) || !has_translation(e) => {
                for arm in &m.arms {
                    self.text(&arm.body);
                }
            }
            // 語言分支與動態資料由各面板的渲染／語意測試負責。
            _ => {}
        }
    }
}

fn language_condition(e: &Expr) -> bool {
    struct Language(bool);
    impl<'a> Visit<'a> for Language {
        fn visit_ident(&mut self, ident: &'a syn::Ident) {
            self.0 |= matches!(
                ident.to_string().as_str(),
                "english"
                    | "en"
                    | "is_en"
                    | "lang"
                    | "language"
                    | "doc_lang"
                    | "is_chinese"
                    | "app_language"
            );
        }
    }
    let mut language = Language(false);
    language.visit_expr(e);
    language.0
}

fn has_translation(e: &Expr) -> bool {
    struct Translation(bool);
    impl<'a> Visit<'a> for Translation {
        fn visit_lit_str(&mut self, s: &'a syn::LitStr) {
            self.0 |= s
                .value()
                .chars()
                .any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
        }
        fn visit_expr_call(&mut self, c: &'a syn::ExprCall) {
            if let Expr::Path(p) = &*c.func {
                self.0 |= p
                    .path
                    .segments
                    .last()
                    .is_some_and(|s| matches!(s.ident.to_string().as_str(), "tr" | "tl" | "t"));
            }
            visit::visit_expr_call(self, c);
        }
        fn visit_macro(&mut self, m: &'a syn::Macro) {
            fn chinese(tokens: proc_macro2::TokenStream) -> bool {
                tokens.into_iter().any(|t| match t {
                    proc_macro2::TokenTree::Group(g) => chinese(g.stream()),
                    proc_macro2::TokenTree::Literal(l) => l
                        .to_string()
                        .chars()
                        .any(|c| ('\u{3400}'..='\u{9fff}').contains(&c)),
                    _ => false,
                })
            }
            self.0 |= chinese(m.tokens.clone());
        }
    }
    let mut translation = Translation(false);
    translation.visit_expr(e);
    translation.0
}

fn is_translation(e: &Expr) -> bool {
    if let Expr::Call(c) = e {
        if let Expr::Path(p) = &*c.func {
            return p
                .path
                .segments
                .last()
                .is_some_and(|s| matches!(s.ident.to_string().as_str(), "tr" | "tl" | "t"));
        }
    }
    false
}

impl<'ast> Visit<'ast> for UiLiterals {
    fn visit_item_mod(&mut self, m: &'ast syn::ItemMod) {
        if m.attrs.iter().any(|a| {
            a.path().is_ident("cfg")
                && matches!(&a.meta, syn::Meta::List(l) if l.tokens.to_string() == "test")
        }) {
            return;
        }
        visit::visit_item_mod(self, m);
    }
    fn visit_expr_method_call(&mut self, c: &'ast syn::ExprMethodCall) {
        let index = match c.method.to_string().as_str() {
            "label" | "heading" | "button" | "small" | "on_hover_text" => Some(0),
            "checkbox" | "selectable_label" | "colored_label" | "toggle_value" => Some(1),
            "selectable_value" => Some(2),
            _ => None,
        };
        if let Some(e) = index.and_then(|i| c.args.iter().nth(i)) {
            self.text(e);
        }
        visit::visit_expr_method_call(self, c);
    }
    fn visit_expr_call(&mut self, c: &'ast syn::ExprCall) {
        if let Expr::Path(p) = &*c.func {
            let names: Vec<_> = p
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect();
            if names.last().is_some_and(|s| s == "new")
                && names.iter().any(|s| s == "Button" || s == "Window")
            {
                if let Some(e) = c.args.first() {
                    self.text(e);
                }
            }
        }
        visit::visit_expr_call(self, c);
    }
}

fn scan(path: &Path, failures: &mut Vec<String>) {
    for entry in fs::read_dir(path).expect("讀取原始碼目錄") {
        let path = entry.expect("讀取原始碼項目").path();
        if path.is_dir() {
            scan(&path, failures);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let source = fs::read_to_string(&path).expect("讀取 Rust 原始碼");
            let syntax = syn::parse_file(&source).expect("解析 Rust 原始碼");
            let mut visitor = UiLiterals::default();
            visitor.visit_file(&syntax);
            for (literal, span) in visitor.found {
                failures.push(format!(
                    "{}:{}: {literal:?}",
                    path.display(),
                    span.start().line
                ));
            }
        }
    }
}

#[test]
fn workspace_ui_literals_are_localized() {
    // 同一檢查也驗證掃描器能抓出多行、格式字串與包裝器中的遺漏。
    let fixture = syn::parse_file(
        r#"fn ui() {
        ui.checkbox(&mut enabled, "English checkbox");
        ui.label(format!("Objects: {}", count));
        ui.label(egui::RichText::new("English heading").strong());
        egui::Window::new("English window");
        ui.label(tl(english, "中文", "English"));
        ui.label("XYZ");
        ui.label(if dirty { "English dirty" } else { "English clean" });
        ui.label(match state { Some(_) => "English state", None => "中文" });
        ui.label(if english { "English branch" } else { "中文" });
        ui.label(format!("Forgotten English {}", tl(english, "中文", "English")));
        ui.label(if english { "Missing translation A" } else { "Missing translation B" });
    }"#,
    )
    .expect("解析掃描器測試資料");
    let mut visitor = UiLiterals::default();
    visitor.visit_file(&fixture);
    assert_eq!(visitor.found.len(), 10);
    let mut failures = Vec::new();
    scan(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut failures,
    );
    assert!(
        failures.is_empty(),
        "尚有 {} 個英文介面字面值：\n{}",
        failures.len(),
        failures.join("\n")
    );
}
