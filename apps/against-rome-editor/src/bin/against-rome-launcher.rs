//! Standalone Rust Launcher Desktop Entry for Against Rome Pro.
//!
//! Port of the C# `src.Modifier/UI/LauncherForm.cs` and `TechDocForm.cs`.
//! Provides a unified desktop launcher that coordinates sibling applications
//! (`against-rome-editor.exe`, `against-rome-modifier-gui.exe`, `save-manager.exe`)
//! as independent processes, provides an embedded read-only technical document viewer,
//! and supports Traditional Chinese / English localization.

use eframe::egui::{self, Color32, RichText, ScrollArea, Stroke, Vec2};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Application version.
pub const VERSION: &str = match option_env!("CARGO_PKG_VERSION") {
    Some(v) => v,
    None => "0.1.0",
};

/// Embedded Traditional Chinese technical documentation from repository root.
pub const TECH_DOC_ZH: &str = include_str!("../../../../TechDoc.md");

/// Embedded English technical documentation from repository root.
pub const TECH_DOC_EN: &str = include_str!("../../../../TechDoc_EN.md");

/// UI display language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    TraditionalChinese,
    English,
}

impl Language {
    pub fn is_chinese(self) -> bool {
        matches!(self, Language::TraditionalChinese)
    }

    pub fn window_title(self) -> &'static str {
        match self {
            Language::TraditionalChinese => "Against Rome 啟動器",
            Language::English => "Against Rome Pro Launcher",
        }
    }

    pub fn main_title(self) -> &'static str {
        match self {
            Language::TraditionalChinese => "AGAINST ROME 啟動器",
            Language::English => "AGAINST ROME PRO LAUNCHER",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Language::TraditionalChinese => "經典羅馬帝國 RTS 專業逆向與地圖設計工具套件",
            Language::English => "Professional Reverse Engineering & Map Design Suite for Against Rome",
        }
    }

    pub fn game_path_label(self) -> &'static str {
        match self {
            Language::TraditionalChinese => "遊戲安裝路徑:",
            Language::English => "Game Install Path:",
        }
    }

    pub fn game_path_hint(self) -> &'static str {
        match self {
            Language::TraditionalChinese => "請輸入或確認 Against Rome 遊戲主目錄（含 Against_Rome.exe 或 MAPS）",
            Language::English => "Enter or verify Against Rome directory (containing Against_Rome.exe or MAPS)",
        }
    }

    pub fn game_detected(self) -> &'static str {
        match self {
            Language::TraditionalChinese => "已辨識遊戲目錄",
            Language::English => "Game Directory Verified",
        }
    }

    pub fn game_not_found(self) -> &'static str {
        match self {
            Language::TraditionalChinese => "未偵測到遊戲目錄（仍可手動啟動各工具）",
            Language::English => "Directory not detected (tools can still be launched)",
        }
    }

    pub fn modifier_title(self) -> &'static str {
        match self {
            Language::TraditionalChinese => "啟動修改器",
            Language::English => "Launch Modifier",
        }
    }

    pub fn modifier_desc(self) -> &'static str {
        match self {
            Language::TraditionalChinese => "調整兵種屬性（生命、傷害、視野、防護）、遊戲全域設定、金幣與科技資源修改。",
            Language::English => "Modify troop attributes (HP, damage, sight, armor), game settings, and resources.",
        }
    }

    pub fn map_editor_title(self) -> &'static str {
        match self {
            Language::TraditionalChinese => "地圖編輯器",
            Language::English => "Map Editor",
        }
    }

    pub fn map_editor_desc(self) -> &'static str {
        match self {
            Language::TraditionalChinese => "2D/3D 地景高程、水文浸蝕、植被生態、物件配置、動態光影與腳本事件編輯。",
            Language::English => "2D/3D terrain elevation, hydraulic erosion, flora, object placement, ambience, and events.",
        }
    }

    pub fn save_manager_title(self) -> &'static str {
        match self {
            Language::TraditionalChinese => "存檔管理器",
            Language::English => "Save Manager",
        }
    }

    pub fn save_manager_desc(self) -> &'static str {
        match self {
            Language::TraditionalChinese => "戰役與無盡模式存檔檢視、AI無響應卡頓修復、自動備份與歷史版本還原。",
            Language::English => "Inspect campaign & endless saves, repair unresponsive AI states, manage backups.",
        }
    }

    pub fn tech_doc_title(self) -> &'static str {
        match self {
            Language::TraditionalChinese => "修改技術文件",
            Language::English => "Technical Document",
        }
    }

    pub fn tech_doc_desc(self) -> &'static str {
        match self {
            Language::TraditionalChinese => "檢視完整逆向工程規格、檔案格式定義、記憶體配置與修改器架構說明文件。",
            Language::English => "View comprehensive reverse engineering specs, file formats, and architecture documentation.",
        }
    }

    pub fn btn_launch(self) -> &'static str {
        match self {
            Language::TraditionalChinese => "啟動應用程式 ➔",
            Language::English => "Launch App ➔",
        }
    }

    pub fn btn_open_doc(self) -> &'static str {
        match self {
            Language::TraditionalChinese => "開啟文件 ➔",
            Language::English => "Open Document ➔",
        }
    }

    pub fn close(self) -> &'static str {
        match self {
            Language::TraditionalChinese => "關閉",
            Language::English => "Close",
        }
    }

    pub fn dismiss(self) -> &'static str {
        match self {
            Language::TraditionalChinese => "確認並關閉提示",
            Language::English => "Dismiss Notice",
        }
    }
}

/// Sibling application types that the launcher can spawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildApp {
    Modifier,
    MapEditor,
    SaveManager,
}

impl ChildApp {
    /// Candidate file names for locating the child executable.
    pub fn executable_candidates(&self) -> &'static [&'static str] {
        match self {
            ChildApp::Modifier => &[
                "against-rome-modifier-gui.exe",
                "against-rome-modifier-gui",
                "against-rome-modifier.exe",
                "against-rome-modifier",
            ],
            ChildApp::MapEditor => &[
                "against-rome-editor.exe",
                "against-rome-editor",
            ],
            ChildApp::SaveManager => &[
                "save-manager.exe",
                "save-manager",
            ],
        }
    }

    /// Primary expected executable name reported to users in error messages.
    pub fn primary_executable_name(&self) -> &'static str {
        match self {
            ChildApp::Modifier => "against-rome-modifier-gui.exe",
            ChildApp::MapEditor => "against-rome-editor.exe",
            ChildApp::SaveManager => "save-manager.exe",
        }
    }

    /// User-friendly label for status messages.
    pub fn display_label(&self, lang: Language) -> &'static str {
        match (self, lang) {
            (ChildApp::Modifier, Language::TraditionalChinese) => "修改器 (against-rome-modifier-gui)",
            (ChildApp::Modifier, Language::English) => "Modifier (against-rome-modifier-gui)",
            (ChildApp::MapEditor, Language::TraditionalChinese) => "地圖編輯器 (against-rome-editor)",
            (ChildApp::MapEditor, Language::English) => "Map Editor (against-rome-editor)",
            (ChildApp::SaveManager, Language::TraditionalChinese) => "存檔管理器 (save-manager)",
            (ChildApp::SaveManager, Language::English) => "Save Manager (save-manager)",
        }
    }

    /// Builds the CLI argument vector for this child application.
    ///
    /// Per-child routing contract:
    /// - `MapEditor`:
    ///   - Accepts `--game <dir>` only when accompanied by `--map <id>`.
    ///   - When map is None (or empty), passes NO arguments so the editor opens its default map selection UI.
    ///   - Does NOT accept `--lang` (rejected by the editor CLI parser).
    /// - `Modifier`:
    ///   - Standalone GUI accepts `--state-dir`, `--smoke-test` and `--lang <zh|en>`.
    ///   - Does NOT accept `--game` or `--map`.
    /// - `SaveManager`:
    ///   - Accepts `--game <dir>` and `--lang <zh|en>`.
    ///   - Does NOT accept `--map`.
    pub fn build_arguments(
        &self,
        game_path: Option<&str>,
        map_id: Option<&str>,
        lang: Language,
    ) -> Vec<String> {
        let mut args = Vec::new();
        match self {
            ChildApp::MapEditor => {
                let trimmed_game = game_path.map(str::trim).filter(|s| !s.is_empty());
                let trimmed_map = map_id.map(str::trim).filter(|s| !s.is_empty());
                // Editor requires BOTH --game and --map to open a map directly.
                // If either is missing, pass neither so the editor opens its map-selection UI.
                if let (Some(game), Some(map)) = (trimmed_game, trimmed_map) {
                    args.push("--game".to_string());
                    args.push(game.to_string());
                    args.push("--map".to_string());
                    args.push(map.to_string());
                }
                // Never pass --lang to editor
            }
            ChildApp::Modifier => {
                // Modifier GUI does not accept --game or --map; it only takes --lang.
                args.push("--lang".to_string());
                args.push(
                    match lang {
                        Language::TraditionalChinese => "zh",
                        Language::English => "en",
                    }
                    .to_string(),
                );
            }
            ChildApp::SaveManager => {
                let trimmed_game = game_path.map(str::trim).filter(|s| !s.is_empty());
                if let Some(game) = trimmed_game {
                    args.push("--game".to_string());
                    args.push(game.to_string());
                }
                let lang_code = match lang {
                    Language::TraditionalChinese => "zh",
                    Language::English => "en",
                };
                args.push("--lang".to_string());
                args.push(lang_code.to_string());
            }
        }
        args
    }
}

/// Resolves the child executable path relative to the given base directory.
pub fn resolve_child_executable(
    base_dir: &Path,
    app: ChildApp,
    lang: Language,
) -> Result<PathBuf, String> {
    for candidate in app.executable_candidates() {
        let path = base_dir.join(candidate);
        if path.is_file() {
            return Ok(path);
        }
    }

    let primary = app.primary_executable_name();
    let expected_path = base_dir.join(primary);

    let error_msg = match lang {
        Language::TraditionalChinese => format!(
            "找不到子應用程式可執行檔「{primary}」。\n\
             搜尋目標目錄：{}\n\n\
             處置建議：\n\
             1. 請確認該組件是否已編譯（例如：cargo build --release）。\n\
             2. 請將編譯後的可執行檔放置於與啟動器相同的目錄中。\n\
             （注意：子程式視窗正由平行任務建置中）",
            expected_path.display()
        ),
        Language::English => format!(
            "Child application executable '{primary}' was not found.\n\
             Expected path: {}\n\n\
             Actionable advice:\n\
             1. Ensure the component has been built (e.g., cargo build --release).\n\
             2. Place the resulting executable in the same directory as this launcher.\n\
             (Note: Sibling app windows are currently being added by parallel tasks)",
            expected_path.display()
        ),
    };

    Err(error_msg)
}

/// Attempts to spawn a child application process with optional game path and map arguments.
pub fn launch_child_process_with_map(
    base_dir: &Path,
    app: ChildApp,
    game_path: Option<&str>,
    map_id: Option<&str>,
    lang: Language,
) -> Result<u32, String> {
    let exe_path = resolve_child_executable(base_dir, app, lang)?;

    let mut command = Command::new(&exe_path);
    for arg in app.build_arguments(game_path, map_id, lang) {
        command.arg(arg);
    }

    match command.spawn() {
        Ok(child) => Ok(child.id()),
        Err(err) => {
            let msg = match lang {
                Language::TraditionalChinese => format!(
                    "啟動「{}」處理程序時失敗: {}\n執行路徑: {}",
                    app.primary_executable_name(),
                    err,
                    exe_path.display()
                ),
                Language::English => format!(
                    "Failed to spawn '{}' process: {}\nExecution path: {}",
                    app.primary_executable_name(),
                    err,
                    exe_path.display()
                ),
            };
            Err(msg)
        }
    }
}

/// Attempts to spawn a child application process with optional game path argument.
pub fn launch_child_process(
    base_dir: &Path,
    app: ChildApp,
    game_path: Option<&str>,
    lang: Language,
) -> Result<u32, String> {
    launch_child_process_with_map(base_dir, app, game_path, None, lang)
}

/// Detects the Against Rome game directory using the C# `GameDirectoryLocator` chain:
/// portable install next to the launcher -> registry (HKCU, HKLM, HKLM WOW6432Node)
/// -> default install path. Returns `None` when every source misses.
pub fn detect_default_game_path() -> Option<PathBuf> {
    let found = against_rome_modifier::services::GameDirectoryLocator::resolve_initial_game_path(None);
    if found.as_os_str().is_empty() {
        None
    } else {
        Some(found)
    }
}

/// Pure fallback-chain resolver (injectable inputs) behind [`detect_default_game_path`].
pub fn resolve_game_path_chain(base: &Path, registry: &Path, default: &Path) -> Option<PathBuf> {
    let found = against_rome_modifier::services::GameDirectoryLocator::resolve_with(
        None, base, registry, default,
    );
    if found.as_os_str().is_empty() {
        None
    } else {
        Some(found)
    }
}

/// Checks whether a given directory looks like a valid Against Rome game folder.
pub fn verify_game_path(path_str: &str) -> bool {
    let p = Path::new(path_str.trim());
    if !p.is_dir() {
        return false;
    }
    p.join("Against_Rome.exe").is_file()
        || p.join("MAPS").is_dir()
        || p.join("DATA").is_dir()
        || p.join("SAVE").is_dir()
}

/// Command line options passed to launcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupOptions {
    pub game_path: Option<String>,
    /// Direct-open map ID (`--map`); always paired with `game_path` and pre-validated.
    pub map_id: Option<String>,
    pub language: Language,
    pub smoke_test: bool,
    pub show_help: bool,
    pub show_version: bool,
}

pub fn parse_cli_args<I, T>(args: I) -> Result<StartupOptions, String>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString>,
{
    let mut options = StartupOptions {
        game_path: None,
        map_id: None,
        language: Language::TraditionalChinese,
        smoke_test: false,
        show_help: false,
        show_version: false,
    };

    let mut iter = args.into_iter().map(|s| s.into());

    while let Some(arg_os) = iter.next() {
        let arg = arg_os.to_string_lossy();
        match arg.as_ref() {
            "--help" | "-h" => {
                options.show_help = true;
            }
            "--version" | "-v" => {
                options.show_version = true;
            }
            "--smoke-test" => {
                options.smoke_test = true;
            }
            "--game" | "-g" => {
                let next = iter
                    .next()
                    .ok_or_else(|| "--game requires a directory path".to_string())?;
                options.game_path = Some(next.to_string_lossy().to_string());
            }
            "--map" => {
                let next = iter
                    .next()
                    .ok_or_else(|| "--map 缺少地圖代號。".to_string())?;
                if options.map_id.is_some() {
                    return Err("--map specified more than once".to_string());
                }
                options.map_id = Some(validate_map_id(&next.to_string_lossy())?);
            }
            "--lang" | "-l" => {
                let next = iter
                    .next()
                    .ok_or_else(|| "--lang requires language code (zh or en)".to_string())?;
                let val = next.to_string_lossy();
                // Mirrors C# CliRunner: blank keeps the default, "en" is English, anything else zh.
                if val.trim().is_empty() {
                } else if val.eq_ignore_ascii_case("en") {
                    options.language = Language::English;
                } else {
                    options.language = Language::TraditionalChinese;
                }
            }
            other => {
                if !other.starts_with('-') && options.game_path.is_none() {
                    options.game_path = Some(other.to_string());
                } else {
                    return Err(format!("Unknown or unexpected command line argument: {other}"));
                }
            }
        }
    }

    if options.map_id.is_some()
        && options
            .game_path
            .as_deref()
            .map_or(true, |g| g.trim().is_empty())
    {
        return Err("使用 --map 直接開啟地圖時必須同時指定 --game <path>。".to_string());
    }

    Ok(options)
}

/// Syntactic `--map` validation (no filesystem access), matching C# `ParseStartupOptions` +
/// `ResolveDirectMap`: ID must be a plain `[A-Za-z0-9_]` token (no separators or `..`) and
/// must not be a campaign (`KAMP_`) map. Any other category (`ENDL_`, `HIST_`, `MP_`,
/// `TUTOR_`, ...) is accepted here; retail maps open read-only in the editor, whose own
/// `CustomMapAccess` policy keeps ENDL_005–999 the only writable maps.
pub fn validate_map_id(raw: &str) -> Result<String, String> {
    let id = raw.trim();
    if id.is_empty() {
        return Err("未指定 --map 地圖代號。".to_string());
    }
    if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("無效的地圖代號。".to_string());
    }
    if id.to_ascii_uppercase().starts_with("KAMP_") {
        return Err(format!("劇情戰役地圖不在地圖編輯器支援範圍：{id}"));
    }
    Ok(id.to_string())
}

/// Resolves a validated `--map` against the game folder: the map must exist in `MAPS`
/// (with `boden.txt` and `minimap.bmp`) and not be a campaign map (same as the editor).
pub fn resolve_direct_map(game_path: &str, map_id: &str) -> Result<(), String> {
    let id = validate_map_id(map_id)?;
    let map = against_rome_maps::catalog::GameMapCatalog::new()
        .require(Path::new(game_path.trim()), &id)?;
    if map.id.to_ascii_uppercase().starts_with("KAMP_") {
        return Err(format!("劇情戰役地圖不在地圖編輯器支援範圍：{}", map.id));
    }
    Ok(())
}

/// The main Launcher application state.
pub struct LauncherApp {
    pub language: Language,
    pub game_path_input: String,
    pub show_tech_doc: bool,
    pub tech_doc_lang: Language,
    pub status_error: Option<String>,
    pub status_success: Option<String>,
    pub base_dir: PathBuf,
}

impl LauncherApp {
    pub fn new(initial_game_path: Option<String>, language: Language) -> Self {
        let base_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));
        Self::with_base_dir(initial_game_path, language, base_dir)
    }

    pub fn with_base_dir(
        initial_game_path: Option<String>,
        language: Language,
        base_dir: PathBuf,
    ) -> Self {
        let game_path_input = initial_game_path
            .or_else(|| detect_default_game_path().map(|p| p.to_string_lossy().to_string()))
            .unwrap_or_default();

        Self {
            language,
            game_path_input,
            show_tech_doc: false,
            tech_doc_lang: language,
            status_error: None,
            status_success: None,
            base_dir,
        }
    }

    pub fn current_game_path(&self) -> Option<&str> {
        let trimmed = self.game_path_input.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }

    pub fn trigger_launch(&mut self, app: ChildApp) {
        self.status_error = None;
        self.status_success = None;

        let lang = self.language;
        let game_path = self.current_game_path();

        match launch_child_process(&self.base_dir, app, game_path, lang) {
            Ok(pid) => {
                let msg = match lang {
                    Language::TraditionalChinese => {
                        format!("已成功啟動 {}（PID: {pid}）", app.display_label(lang))
                    }
                    Language::English => {
                        format!("Successfully launched {} (PID: {pid})", app.display_label(lang))
                    }
                };
                self.status_success = Some(msg);
            }
            Err(err) => {
                self.status_error = Some(err);
            }
        }
    }

    pub fn update_ui(&mut self, ctx: &egui::Context) {
        let lang = self.language;

        // Top Header Panel
        egui::TopBottomPanel::top("launcher_header").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.heading(
                    RichText::new(lang.main_title())
                        .color(Color32::from_rgb(230, 165, 60))
                        .strong()
                        .size(20.0),
                );

                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!("v{VERSION}"))
                        .color(Color32::from_rgb(140, 150, 165))
                        .size(12.0),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let is_en = !lang.is_chinese();
                    let is_zh = lang.is_chinese();

                    if ui
                        .selectable_label(is_en, RichText::new("English").strong())
                        .clicked()
                    {
                        self.language = Language::English;
                        self.tech_doc_lang = Language::English;
                    }

                    if ui
                        .selectable_label(is_zh, RichText::new("繁體中文").strong())
                        .clicked()
                    {
                        self.language = Language::TraditionalChinese;
                        self.tech_doc_lang = Language::TraditionalChinese;
                    }
                });
            });

            ui.add_space(4.0);
            ui.label(
                RichText::new(lang.subtitle())
                    .color(Color32::from_rgb(160, 170, 185))
                    .size(12.0),
            );
            ui.add_space(6.0);
        });

        // Bottom Status/Error Panel
        if self.status_error.is_some() || self.status_success.is_some() {
            let mut clear_error = false;
            let mut clear_success = false;

            egui::TopBottomPanel::bottom("launcher_status_panel").show(ctx, |ui| {
                ui.add_space(6.0);
                if let Some(err) = &self.status_error {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("⚠")
                                    .color(Color32::from_rgb(235, 75, 75))
                                    .size(18.0)
                                    .strong(),
                            );
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(err)
                                        .color(Color32::from_rgb(245, 120, 120))
                                        .size(12.0),
                                );
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button(RichText::new(lang.dismiss()).size(12.0)).clicked() {
                                        clear_error = true;
                                    }
                                },
                            );
                        });
                    });
                } else if let Some(success) = &self.status_success {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("✓")
                                    .color(Color32::from_rgb(80, 200, 120))
                                    .size(18.0)
                                    .strong(),
                            );
                            ui.label(
                                RichText::new(success)
                                    .color(Color32::from_rgb(120, 225, 150))
                                    .size(12.0),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button(RichText::new(lang.dismiss()).size(12.0)).clicked() {
                                        clear_success = true;
                                    }
                                },
                            );
                        });
                    });
                }
                ui.add_space(6.0);
            });

            if clear_error {
                self.status_error = None;
            }
            if clear_success {
                self.status_success = None;
            }
        }

        // Central Panel
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(6.0);

            // Game Path Configuration Box
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(lang.game_path_label()).strong());
                    let text_edit = egui::TextEdit::singleline(&mut self.game_path_input)
                        .hint_text(lang.game_path_hint())
                        .desired_width(f32::INFINITY);
                    ui.add(text_edit);
                });

                let is_valid = verify_game_path(&self.game_path_input);
                ui.horizontal(|ui| {
                    if is_valid {
                        ui.label(
                            RichText::new(format!("✓ {}", lang.game_detected()))
                                .color(Color32::from_rgb(80, 200, 120))
                                .size(11.0),
                        );
                    } else {
                        ui.label(
                            RichText::new(format!("ℹ {}", lang.game_not_found()))
                                .color(Color32::from_rgb(160, 165, 175))
                                .size(11.0),
                        );
                    }
                });
            });

            ui.add_space(10.0);

            // Action Cards Grid
            let total_width = ui.available_width();
            let col_width = ((total_width - 16.0) / 2.0).max(280.0);

            ui.horizontal_top(|ui| {
                // Left Column: Modifier Hero Card
                ui.allocate_ui(Vec2::new(col_width, 360.0), |ui| {
                    egui::Frame::group(ui.style())
                        .fill(Color32::from_rgb(32, 36, 46))
                        .stroke(Stroke::new(1.2_f32, Color32::from_rgb(200, 145, 55)))
                        .rounding(8.0)
                        .inner_margin(16.0)
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new("⚔")
                                            .color(Color32::from_rgb(230, 165, 60))
                                            .size(24.0),
                                    );
                                    ui.heading(
                                        RichText::new(lang.modifier_title())
                                            .color(Color32::from_rgb(240, 240, 245))
                                            .size(18.0)
                                            .strong(),
                                    );
                                });

                                ui.add_space(10.0);
                                ui.label(
                                    RichText::new(lang.modifier_desc())
                                        .color(Color32::from_rgb(180, 185, 200))
                                        .size(13.0),
                                );

                                ui.add_space(16.0);
                                ui.label(
                                    RichText::new(format!(
                                        "Target: {}",
                                        ChildApp::Modifier.primary_executable_name()
                                    ))
                                    .color(Color32::from_rgb(130, 140, 155))
                                    .size(11.0),
                                );

                                ui.add_space(20.0);
                                let btn = egui::Button::new(
                                    RichText::new(lang.btn_launch())
                                        .size(15.0)
                                        .color(Color32::from_rgb(20, 22, 28))
                                        .strong(),
                                )
                                .fill(Color32::from_rgb(225, 160, 50))
                                .min_size(Vec2::new(ui.available_width(), 44.0));

                                if ui.add(btn).clicked() {
                                    self.trigger_launch(ChildApp::Modifier);
                                }
                            });
                        });
                });

                ui.add_space(12.0);

                // Right Column: Vertical Stack (Map Editor, Save Manager, Technical Document)
                ui.allocate_ui(Vec2::new(col_width, 360.0), |ui| {
                    ui.vertical(|ui| {
                        // Card 1: Map Editor
                        render_action_card(
                            ui,
                            "🗺",
                            lang.map_editor_title(),
                            lang.map_editor_desc(),
                            ChildApp::MapEditor.primary_executable_name(),
                            lang.btn_launch(),
                            Color32::from_rgb(60, 130, 210),
                            || self.trigger_launch(ChildApp::MapEditor),
                        );

                        ui.add_space(10.0);

                        // Card 2: Save Manager
                        render_action_card(
                            ui,
                            "💾",
                            lang.save_manager_title(),
                            lang.save_manager_desc(),
                            ChildApp::SaveManager.primary_executable_name(),
                            lang.btn_launch(),
                            Color32::from_rgb(60, 175, 120),
                            || self.trigger_launch(ChildApp::SaveManager),
                        );

                        ui.add_space(10.0);

                        // Card 3: Technical Document
                        render_action_card(
                            ui,
                            "📖",
                            lang.tech_doc_title(),
                            lang.tech_doc_desc(),
                            "TechDoc.md / TechDoc_EN.md",
                            lang.btn_open_doc(),
                            Color32::from_rgb(170, 100, 220),
                            || {
                                self.show_tech_doc = true;
                            },
                        );
                    });
                });
            });
        });

        // Technical Document Read-Only Modal Window
        if self.show_tech_doc {
            let mut open = self.show_tech_doc;
            let doc_lang = self.tech_doc_lang;

            let doc_title = match doc_lang {
                Language::TraditionalChinese => "技術文件 (Technical Document) — Read-Only",
                Language::English => "Technical Document — Read-Only",
            };

            egui::Window::new(RichText::new(doc_title).strong())
                .open(&mut open)
                .default_size(Vec2::new(760.0, 520.0))
                .min_size(Vec2::new(480.0, 320.0))
                .resizable(true)
                .show(ctx, |ui| {
                    // Window Top Control Bar
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(match doc_lang {
                            Language::TraditionalChinese => "語言切換:",
                            Language::English => "Language:",
                        }).strong());

                        let is_zh = doc_lang.is_chinese();
                        let is_en = !doc_lang.is_chinese();

                        if ui.selectable_label(is_zh, "繁體中文 (Traditional Chinese)").clicked() {
                            self.tech_doc_lang = Language::TraditionalChinese;
                        }
                        if ui.selectable_label(is_en, "English (英文)").clicked() {
                            self.tech_doc_lang = Language::English;
                        }

                        ui.separator();

                        let doc_content = match self.tech_doc_lang {
                            Language::TraditionalChinese => TECH_DOC_ZH,
                            Language::English => TECH_DOC_EN,
                        };

                        let lines = doc_content.lines().count();
                        let chars = doc_content.chars().count();
                        ui.weak(match doc_lang {
                            Language::TraditionalChinese => format!("{lines} 行 · {chars} 字元 (唯讀視圖)"),
                            Language::English => format!("{lines} lines · {chars} chars (read-only view)"),
                        });
                    });

                    ui.separator();

                    // Read-only Vertical Scrolling Text View
                    let doc_content = match self.tech_doc_lang {
                        Language::TraditionalChinese => TECH_DOC_ZH,
                        Language::English => TECH_DOC_EN,
                    };

                    ScrollArea::vertical()
                        .id_source("launcher_tech_doc_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(doc_content)
                                        .monospace()
                                        .size(12.0)
                                        .color(Color32::from_rgb(215, 220, 230)),
                                )
                                .wrap(true)
                                .selectable(true),
                            );
                        });
                });

            self.show_tech_doc = open;
        }
    }
}

/// Helper function to render a compact action card.
fn render_action_card<F: FnOnce()>(
    ui: &mut egui::Ui,
    icon: &str,
    title: &str,
    desc: &str,
    target: &str,
    btn_text: &str,
    accent: Color32,
    on_click: F,
) {
    egui::Frame::group(ui.style())
        .fill(Color32::from_rgb(28, 31, 40))
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(50, 56, 70)))
        .rounding(6.0)
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon).color(accent).size(18.0));
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).strong().size(14.0));
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(desc)
                            .color(Color32::from_rgb(165, 172, 185))
                            .size(11.0),
                    );
                    ui.weak(RichText::new(format!("Target: {target}")).size(10.0));
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let button = egui::Button::new(
                        RichText::new(btn_text)
                            .size(12.0)
                            .strong()
                            .color(Color32::from_rgb(240, 240, 245)),
                    )
                    .fill(Color32::from_rgb(45, 52, 68))
                    .min_size(Vec2::new(110.0, 32.0));

                    if ui.add(button).clicked() {
                        on_click();
                    }
                });
            });
        });
}

impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_ui(ctx);
    }
}

pub fn run() -> Result<(), String> {
    let options = parse_cli_args(std::env::args_os().skip(1))?;

    if options.show_help {
        println!(
            "against-rome-launcher [OPTIONS]\n\n\
             Standalone Rust Launcher for Against Rome Suite.\n\n\
             OPTIONS:\n\
               --game, -g <PATH>   Specify Against Rome game directory\n\
               --map <ID>          Open an existing non-campaign map (e.g. ENDL_/HIST_/MP_) in the editor (requires --game)\n\
               --lang, -l <zh|en>  Set UI language (zh = Traditional Chinese, en = English)\n\
               --smoke-test        Execute headless UI update cycles and exit without window\n\
               --version, -v       Print version information and exit\n\
               --help, -h          Print this help message and exit"
        );
        return Ok(());
    }

    if options.show_version {
        println!("against-rome-launcher {VERSION}");
        return Ok(());
    }

    if options.smoke_test {
        // Guarantee isolation: point base_dir to a clean temporary directory that contains no child binaries.
        // This ensures --smoke-test NEVER accidentally spawns sibling binaries in the build/install folder.
        let isolated_empty_dir = std::env::temp_dir().join(format!(
            "against_rome_launcher_smoke_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::create_dir_all(&isolated_empty_dir);

        let mut app = LauncherApp::with_base_dir(
            options.game_path,
            options.language,
            isolated_empty_dir.clone(),
        );
        let ctx = egui::Context::default();

        // Frame 1: Initial state rendering
        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ui(ctx);
        });

        // Frame 2: Open technical document modal & switch language
        app.show_tech_doc = true;
        app.language = Language::English;
        app.tech_doc_lang = Language::English;
        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ui(ctx);
        });
        assert!(app.show_tech_doc);

        // Frame 3: Attempt child launch on nonexistent binary in isolated base dir;
        // must produce actionable error without spawning any real process or panicking.
        app.trigger_launch(ChildApp::MapEditor);
        let _ = std::fs::remove_dir_all(&isolated_empty_dir);

        assert!(
            app.status_error.is_some(),
            "Nonexistent child executable must produce actionable error message"
        );
        let err_text = app.status_error.as_deref().unwrap_or_default();
        assert!(
            err_text.contains("against-rome-editor.exe"),
            "Actionable error must identify the expected executable name"
        );

        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ui(ctx);
        });

        println!("Native desktop launcher smoke test completed: three UI frames executed successfully.");
        return Ok(());
    }

    if let (Some(game), Some(map)) = (options.game_path.as_deref(), options.map_id.as_deref()) {
        resolve_direct_map(game, map)?;
        let base_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));
        let pid = launch_child_process_with_map(
            &base_dir,
            ChildApp::MapEditor,
            Some(game),
            Some(map),
            options.language,
        )?;
        println!("Launched map editor (pid {pid}) for {map}.");
        return Ok(());
    }

    let initial_lang = options.language;
    let title = initial_lang.window_title();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size([760.0, 540.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        title,
        native_options,
        Box::new(move |cc| {
            // Apply CJK system fonts on Windows for Traditional Chinese display
            #[cfg(windows)]
            if let Some(windows) = std::env::var_os("WINDIR") {
                for font_file in ["msjh.ttc", "msyh.ttc", "simsun.ttc"] {
                    let font_path = PathBuf::from(&windows).join("Fonts").join(font_file);
                    if let Ok(bytes) = std::fs::read(&font_path) {
                        let mut fonts = egui::FontDefinitions::default();
                        fonts
                            .font_data
                            .insert("system-cjk".into(), egui::FontData::from_owned(bytes));
                        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                            fonts.families.entry(family).or_default().push("system-cjk".into());
                        }
                        cc.egui_ctx.set_fonts(fonts);
                        break;
                    }
                }
            }

            // Dark theme visuals customization
            let mut visuals = egui::Visuals::dark();
            visuals.override_text_color = Some(Color32::from_rgb(235, 238, 245));
            visuals.panel_fill = Color32::from_rgb(24, 27, 34);
            visuals.window_fill = Color32::from_rgb(28, 31, 40);
            visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(32, 36, 46);
            visuals.widgets.inactive.bg_fill = Color32::from_rgb(38, 43, 55);
            visuals.widgets.hovered.bg_fill = Color32::from_rgb(52, 60, 78);
            visuals.widgets.active.bg_fill = Color32::from_rgb(65, 75, 98);
            cc.egui_ctx.set_visuals(visuals);

            Box::new(LauncherApp::new(options.game_path, options.language))
        }),
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_path_chain_prefers_portable_then_registry_then_default() {
        let tmp = std::env::temp_dir().join(format!("ar_launcher_chain_{}", std::process::id()));
        let base = tmp.join("base");
        let default = tmp.join("default");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::create_dir_all(&default).unwrap();
        let reg = tmp.join("registry");
        // Nothing portable: registry wins over default.
        assert_eq!(resolve_game_path_chain(&base, &reg, &default), Some(reg.clone()));
        // Empty registry: default install if it exists.
        assert_eq!(resolve_game_path_chain(&base, Path::new(""), &default), Some(default.clone()));
        // Portable exe beats registry.
        std::fs::write(base.join("Against_Rome.exe"), b"x").unwrap();
        assert_eq!(resolve_game_path_chain(&base, &reg, &default), Some(base.clone()));
        // Everything missing.
        assert_eq!(
            resolve_game_path_chain(&tmp.join("nope"), Path::new(""), &tmp.join("nope2")),
            None
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_parse_cli_args_help() {
        let opts = parse_cli_args(["--help"]).unwrap();
        assert!(opts.show_help);

        let opts_short = parse_cli_args(["-h"]).unwrap();
        assert!(opts_short.show_help);
    }

    #[test]
    fn test_parse_cli_args_version() {
        let opts = parse_cli_args(["--version"]).unwrap();
        assert!(opts.show_version);

        let opts_short = parse_cli_args(["-v"]).unwrap();
        assert!(opts_short.show_version);
    }

    #[test]
    fn test_parse_cli_args_game_path() {
        let opts = parse_cli_args(["--game", "C:\\Games\\Against Rome"]).unwrap();
        assert_eq!(opts.game_path.as_deref(), Some("C:\\Games\\Against Rome"));

        let opts_short = parse_cli_args(["-g", "D:\\AgainstRome"]).unwrap();
        assert_eq!(opts_short.game_path.as_deref(), Some("D:\\AgainstRome"));

        let opts_pos = parse_cli_args(["E:\\CustomRome"]).unwrap();
        assert_eq!(opts_pos.game_path.as_deref(), Some("E:\\CustomRome"));
    }

    #[test]
    fn test_parse_cli_args_language() {
        let opts_en = parse_cli_args(["--lang", "en"]).unwrap();
        assert_eq!(opts_en.language, Language::English);

        let opts_zh = parse_cli_args(["--lang", "zh"]).unwrap();
        assert_eq!(opts_zh.language, Language::TraditionalChinese);
    }

    #[test]
    fn test_parse_cli_args_smoke_test() {
        let opts = parse_cli_args(["--smoke-test"]).unwrap();
        assert!(opts.smoke_test);
    }

    #[test]
    fn test_parse_cli_args_map_routing() {
        let opts = parse_cli_args(["--game", "C:\\G", "--map", "endl_005"]).unwrap();
        assert_eq!(opts.map_id.as_deref(), Some("endl_005"));
        let opts = parse_cli_args(["--map", " ENDL_999 ", "--game", "C:\\G"]).unwrap();
        assert_eq!(opts.map_id.as_deref(), Some("ENDL_999"));
        assert!(parse_cli_args(["--game", "C:\\G"]).unwrap().map_id.is_none());
    }

    #[test]
    fn test_parse_cli_args_map_rejections() {
        assert!(parse_cli_args(["--game", "C:\\G", "--map"]).is_err());
        assert!(parse_cli_args(["--map", "ENDL_005"]).is_err(), "needs --game");
        assert!(parse_cli_args(["--game", "  ", "--map", "ENDL_005"]).is_err());
        assert!(parse_cli_args(["--game", "G", "--map", "ENDL_005", "--map", "ENDL_006"]).is_err());
        for bad in [
            "", "  ", "KAMP_001", "kamp_x", " Kamp_002 ", "ENDL_-05", "..", ".",
            "..\\ENDL_005", "../ENDL_005", "ENDL_005/x", "C:\\MAPS\\ENDL_005", "ENDL_005.",
            "HIST_001/..", "MP_01\\x", "MP 01",
        ] {
            assert!(
                parse_cli_args(["--game", "G", "--map", bad]).is_err(),
                "map {bad:?} must be rejected"
            );
        }
    }

    #[test]
    fn test_parse_cli_args_accepts_all_noncampaign_id_types() {
        // C# ParseStartupOptions/ResolveDirectMap only exclude KAMP_; retail and
        // non-ENDL ids route to the editor, which enforces read-only policy.
        for ok in [
            "ENDL_000", "ENDL_004", "endl_005", "ENDL_999", "ENDL_5", "HIST_001", "hist_x",
            "MP_Arena", "TUTOR_01", "Custom1",
        ] {
            let opts = parse_cli_args(["--game", "G", "--map", ok]).unwrap_or_else(|e| {
                panic!("map {ok:?} must be accepted: {e}")
            });
            assert_eq!(opts.map_id.as_deref(), Some(ok));
        }
    }

    #[test]
    fn test_resolve_direct_map_requires_existing_noncampaign_map() {
        let root = std::env::temp_dir().join(format!(
            "launcher_map_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let maps = root.join("MAPS");
        for id in ["ENDL_002", "ENDL_005", "HIST_001", "MP_Arena", "KAMP_001"] {
            std::fs::create_dir_all(maps.join(id)).unwrap();
        }
        // Complete maps (boden.txt + minimap.bmp) for all but ENDL_005 (incomplete).
        for id in ["ENDL_002", "HIST_001", "MP_Arena", "KAMP_001"] {
            std::fs::write(maps.join(id).join("boden.txt"), "").unwrap();
            std::fs::write(maps.join(id).join("minimap.bmp"), "").unwrap();
        }
        let g = root.to_string_lossy().to_string();
        // Existing retail / non-ENDL maps resolve (case-insensitively).
        assert!(resolve_direct_map(&g, "ENDL_002").is_ok());
        assert!(resolve_direct_map(&g, "hist_001").is_ok());
        assert!(resolve_direct_map(&g, "MP_Arena").is_ok());
        // Incomplete directory, missing map, campaign and traversal are refused.
        assert!(resolve_direct_map(&g, "ENDL_005").is_err());
        assert!(resolve_direct_map(&g, "ENDL_777").is_err());
        assert!(resolve_direct_map(&g, "KAMP_001").is_err());
        assert!(resolve_direct_map(&g, "..").is_err());
        assert!(resolve_direct_map(&g, r"..\MAPS\HIST_001").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_parse_cli_args_errors() {
        assert!(parse_cli_args(["--game"]).is_err());
        assert!(parse_cli_args(["--lang"]).is_err());
        assert!(parse_cli_args(["--unknown-flag"]).is_err());
    }

    #[test]
    fn test_resolve_child_executable_missing_returns_actionable_error() {
        let temp_dir = std::env::temp_dir().join(format!("launcher_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let err_zh = resolve_child_executable(&temp_dir, ChildApp::MapEditor, Language::TraditionalChinese)
            .unwrap_err();
        assert!(err_zh.contains("找不到子應用程式可執行檔"));
        assert!(err_zh.contains("against-rome-editor.exe"));

        let err_en = resolve_child_executable(&temp_dir, ChildApp::SaveManager, Language::English)
            .unwrap_err();
        assert!(err_en.contains("Child application executable 'save-manager.exe' was not found"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_resolve_child_executable_existing() {
        let temp_dir = std::env::temp_dir().join(format!("launcher_test_found_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let exe_file = temp_dir.join("against-rome-editor.exe");
        std::fs::write(&exe_file, b"test").unwrap();

        let resolved = resolve_child_executable(&temp_dir, ChildApp::MapEditor, Language::TraditionalChinese)
            .unwrap();
        assert_eq!(resolved, exe_file);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_embedded_tech_docs_integrity() {
        assert!(!TECH_DOC_ZH.is_empty(), "ZH TechDoc should not be empty");
        assert!(!TECH_DOC_EN.is_empty(), "EN TechDoc should not be empty");

        assert!(
            TECH_DOC_ZH.contains("Against Rome Modifier 技術文件"),
            "ZH TechDoc should contain expected title"
        );
        assert!(
            TECH_DOC_EN.contains("Against Rome Modifier Complete Technical Document"),
            "EN TechDoc should contain expected title"
        );

        assert!(TECH_DOC_ZH.lines().count() > 100);
        assert!(TECH_DOC_EN.lines().count() > 100);
    }

    #[test]
    fn test_language_strings() {
        assert_eq!(Language::TraditionalChinese.main_title(), "AGAINST ROME 啟動器");
        assert_eq!(Language::English.main_title(), "AGAINST ROME PRO LAUNCHER");

        assert_eq!(Language::TraditionalChinese.modifier_title(), "啟動修改器");
        assert_eq!(Language::English.modifier_title(), "Launch Modifier");

        assert_eq!(Language::TraditionalChinese.map_editor_title(), "地圖編輯器");
        assert_eq!(Language::English.map_editor_title(), "Map Editor");

        assert_eq!(Language::TraditionalChinese.save_manager_title(), "存檔管理器");
        assert_eq!(Language::English.save_manager_title(), "Save Manager");
    }

    #[test]
    fn test_launch_child_process_missing_binary() {
        let temp_dir = std::env::temp_dir().join(format!("launcher_spawn_missing_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let res = launch_child_process(
            &temp_dir,
            ChildApp::Modifier,
            Some("C:\\Games\\Against Rome"),
            Language::TraditionalChinese,
        );
        assert!(res.is_err(), "Spawning missing binary must return Err");
        let err_msg = res.unwrap_err();
        assert!(err_msg.contains("against-rome-modifier-gui.exe"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_verify_game_path_logic() {
        assert!(!verify_game_path(""));
        assert!(!verify_game_path("C:\\nonexistent_against_rome_directory_12345"));

        let temp_dir = std::env::temp_dir().join(format!("launcher_verify_game_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        // Empty directory is not a game folder
        assert!(!verify_game_path(temp_dir.to_str().unwrap()));

        // With MAPS folder, it is recognized
        let maps_dir = temp_dir.join("MAPS");
        let _ = std::fs::create_dir_all(&maps_dir);
        assert!(verify_game_path(temp_dir.to_str().unwrap()));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_headless_smoke_frames() {
        let isolated_dir = std::env::temp_dir().join(format!("launcher_test_smoke_frames_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&isolated_dir);
        let mut app = LauncherApp::with_base_dir(
            Some("C:\\Games\\Against Rome".into()),
            Language::TraditionalChinese,
            isolated_dir.clone(),
        );
        let ctx = egui::Context::default();

        // 1. Initial frame
        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ui(ctx);
        });
        assert!(!app.show_tech_doc);
        assert!(app.status_error.is_none());

        // 2. Open tech doc & toggle language
        app.show_tech_doc = true;
        app.language = Language::English;
        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ui(ctx);
        });
        assert!(app.show_tech_doc);

        // 3. Child app launch failure generates actionable error without panic
        app.trigger_launch(ChildApp::Modifier);
        let _ = std::fs::remove_dir_all(&isolated_dir);
        assert!(app.status_error.is_some());

        let _ = ctx.run(Default::default(), |ctx| {
            app.update_ui(ctx);
        });
    }

    #[test]
    fn test_child_app_arguments_contract_editor() {
        // Direct map launch requires both game and map
        let args_direct = ChildApp::MapEditor.build_arguments(
            Some("C:\\Games\\Against Rome"),
            Some("ENDL_001"),
            Language::TraditionalChinese,
        );
        assert_eq!(
            args_direct,
            vec!["--game", "C:\\Games\\Against Rome", "--map", "ENDL_001"]
        );

        // When map is None, do NOT pass orphan --game; pass no arguments for map selection UI
        let args_no_map = ChildApp::MapEditor.build_arguments(
            Some("C:\\Games\\Against Rome"),
            None,
            Language::TraditionalChinese,
        );
        assert!(
            args_no_map.is_empty(),
            "Editor without map must pass no arguments to open map-selector UI"
        );

        // When game is None, do NOT pass map alone
        let args_no_game = ChildApp::MapEditor.build_arguments(
            None,
            Some("ENDL_001"),
            Language::English,
        );
        assert!(args_no_game.is_empty());

        // Editor must NEVER receive --lang argument
        let args_en = ChildApp::MapEditor.build_arguments(
            Some("C:\\Games\\Against Rome"),
            Some("ENDL_002"),
            Language::English,
        );
        assert!(!args_en.contains(&"--lang".to_string()));
        assert!(!args_en.contains(&"en".to_string()));

        // Whitespace-only strings are treated as None
        let args_ws = ChildApp::MapEditor.build_arguments(
            Some("   "),
            Some("ENDL_001"),
            Language::TraditionalChinese,
        );
        assert!(args_ws.is_empty());
    }

    #[test]
    fn test_child_app_arguments_contract_modifier() {
        // Modifier GUI takes only --lang from the launcher; game path and map id are ignored.
        let args = ChildApp::Modifier.build_arguments(
            Some("C:\\Games\\Against Rome"),
            Some("ENDL_005"),
            Language::TraditionalChinese,
        );
        assert_eq!(args, vec!["--lang".to_string(), "zh".to_string()]);

        let args_en = ChildApp::Modifier.build_arguments(
            None,
            None,
            Language::English,
        );
        assert_eq!(args_en, vec!["--lang".to_string(), "en".to_string()]);
    }

    #[test]
    fn test_child_app_arguments_contract_save_manager() {
        // Save Manager accepts --game and --lang, but ignores map_id
        let args_zh = ChildApp::SaveManager.build_arguments(
            Some("C:\\Games\\Against Rome"),
            Some("IGNORED_MAP"),
            Language::TraditionalChinese,
        );
        assert_eq!(
            args_zh,
            vec!["--game", "C:\\Games\\Against Rome", "--lang", "zh"]
        );

        let args_en = ChildApp::SaveManager.build_arguments(
            Some("D:\\Rome"),
            None,
            Language::English,
        );
        assert_eq!(args_en, vec!["--game", "D:\\Rome", "--lang", "en"]);

        // When no game path is provided, pass only language
        let args_no_game = ChildApp::SaveManager.build_arguments(
            None,
            None,
            Language::TraditionalChinese,
        );
        assert_eq!(args_no_game, vec!["--lang", "zh"]);

        // Whitespace game path treated as None
        let args_ws = ChildApp::SaveManager.build_arguments(
            Some("   "),
            None,
            Language::English,
        );
        assert_eq!(args_ws, vec!["--lang", "en"]);
    }

    #[test]
    fn test_child_app_arguments_ignore_unsupported() {
        // Ensure unsupported flags are strictly absent across all child kinds
        let all_children = [ChildApp::MapEditor, ChildApp::Modifier, ChildApp::SaveManager];
        for app in all_children {
            let args = app.build_arguments(
                Some("C:\\Games\\Against Rome"),
                Some("ENDL_001"),
                Language::TraditionalChinese,
            );
            match app {
                ChildApp::MapEditor => {
                    assert!(!args.contains(&"--lang".to_string()));
                }
                ChildApp::Modifier => {
                    assert!(!args.contains(&"--game".to_string()));
                    assert!(!args.contains(&"--map".to_string()));
                    assert!(args.contains(&"--lang".to_string()));
                }
                ChildApp::SaveManager => {
                    assert!(!args.contains(&"--map".to_string()));
                }
            }
        }
    }

    #[test]
    fn test_smoke_test_does_not_execute_sibling_binaries() {
        // Create a mock directory with dummy executable files matching child candidates
        let mock_sibling_dir = std::env::temp_dir().join(format!(
            "launcher_mock_sibling_test_{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&mock_sibling_dir);

        let dummy_editor = mock_sibling_dir.join("against-rome-editor.exe");
        std::fs::write(&dummy_editor, b"mock-editor-binary").unwrap();
        let dummy_modifier = mock_sibling_dir.join("against-rome-modifier-gui.exe");
        std::fs::write(&dummy_modifier, b"mock-modifier-binary").unwrap();

        // An app using the mock sibling dir would find the binaries
        assert!(resolve_child_executable(&mock_sibling_dir, ChildApp::MapEditor, Language::English).is_ok());

        // But smoke-test requires an isolated clean base directory where no siblings exist
        let isolated_smoke_dir = std::env::temp_dir().join(format!(
            "launcher_isolated_smoke_test_{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&isolated_smoke_dir);

        let mut smoke_app = LauncherApp::with_base_dir(
            None,
            Language::TraditionalChinese,
            isolated_smoke_dir.clone(),
        );

        // Attempting launch in isolated dir must NOT execute the mock sibling
        smoke_app.trigger_launch(ChildApp::MapEditor);

        assert!(smoke_app.status_error.is_some(), "Must report error for missing binary");
        assert!(smoke_app.status_success.is_none(), "Must NOT report success");
        let err = smoke_app.status_error.as_ref().unwrap();
        assert!(err.contains("against-rome-editor.exe"));
        assert!(err.contains("處置建議"));

        // Clean up test directories
        let _ = std::fs::remove_dir_all(&mock_sibling_dir);
        let _ = std::fs::remove_dir_all(&isolated_smoke_dir);
    }

    #[test]
    fn test_actionable_error_on_missing_binaries_all_children() {
        let empty_dir = std::env::temp_dir().join(format!(
            "launcher_empty_actionable_err_{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&empty_dir);

        let children = [
            (ChildApp::MapEditor, "against-rome-editor.exe"),
            (ChildApp::Modifier, "against-rome-modifier-gui.exe"),
            (ChildApp::SaveManager, "save-manager.exe"),
        ];

        for (child, exe_name) in children {
            // Traditional Chinese actionable error
            let err_zh = resolve_child_executable(&empty_dir, child, Language::TraditionalChinese)
                .unwrap_err();
            assert!(err_zh.contains(exe_name), "ZH error must mention {exe_name}");
            assert!(err_zh.contains("找不到子應用程式可執行檔"));
            assert!(err_zh.contains("處置建議："));
            assert!(err_zh.contains("cargo build --release"));

            // English actionable error
            let err_en = resolve_child_executable(&empty_dir, child, Language::English)
                .unwrap_err();
            assert!(err_en.contains(exe_name), "EN error must mention {exe_name}");
            assert!(err_en.contains("Child application executable"));
            assert!(err_en.contains("Actionable advice:"));
            assert!(err_en.contains("cargo build --release"));
        }

        let _ = std::fs::remove_dir_all(&empty_dir);
    }
}
