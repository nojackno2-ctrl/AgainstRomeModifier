//! formation-probe: Against Rome 隊形線段取樣、ALR 士兵影格萃取與 PFIL 唯讀傾印工具。
//!
//! 完全相容 C# 原版 `tools/re/formation-probe` 及 Python `probe.py` 之唯讀分析功能：
//! 1. `--layout <formdef.dau>`：解壓縮並讀取 PFIL formdef，回報 SHA256、線段數、matchesEmbedded 及 counts 1..20 之世界座標。
//! 2. `<offline alr.dat> <NEW output dir> [asset name]`：安全讀取 ALR 封裝條目，解碼至多 24 個影格，寫入 RGBA 與 frames.json 並回報 "Exported frames"。
//! 3. `dump <pfil> [--dump START:END]`：唯讀 PFIL / LZSS 傾印，行格式完全比照 probe.py。
//!
//! 安全邊界：
//! - 嚴格僅使用明確指定之路徑，絕不隱式探測或搜尋任何安裝遊戲目錄。
//! - 輸出目標強制驗證為新目錄，拒絕覆寫任何既有檔案或目錄。
//! - 全面拒絕空路徑、空字元與路徑遍歷越界 (..)。
//! - 不包含 EXE 反組譯（無內建反組譯器依賴；EXE/PE 反組譯明確劃歸 Capstone/REA 等外部工具）。

pub mod archive;
pub mod formation_layout;
pub mod pfil;
pub mod safety;

pub use archive::{argb_to_rgba, export_soldier_frames, FrameExportInfo};
pub use formation_layout::{
    CountLayout, FormationLayout, FormationLayoutDefinition, FormationLayoutSegment, LayoutReport,
    OffsetPoint,
};
pub use pfil::{decompress_pfil, format_dump, parse_dump_range, sha256_hex};
pub use safety::SafetyError;

use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::Path;

/// 執行 `--layout` 命令：讀取並解壓縮指定 formdef 檔案，計算隊形佈局，回傳報告結構。
pub fn run_layout(path: &Path) -> Result<LayoutReport, String> {
    let clean =
        safety::validate_explicit_path(path).map_err(|e| format!("無效的隊形檔案路徑：{e}"))?;
    let input =
        fs::read(&clean).map_err(|e| format!("無法讀取隊形定義檔案 '{}': {e}", clean.display()))?;

    let sha256 = pfil::sha256_hex(&input);
    let decompressed = pfil::decompress_pfil(&input)?;
    // 以 ISO-8859-1 (Latin1) 解碼文字，對齊 C# Encoding.Latin1
    let text: String = decompressed.iter().map(|&b| b as char).collect();

    let definition = FormationLayoutDefinition::from_form_def_text(&text, 0)?;
    let matches_embedded = definition.matches_embedded();
    let segment_count = definition.segments().len();

    let mut layouts = Vec::with_capacity(20);
    for count in 1..=20 {
        let offsets =
            FormationLayout::create(count, 0.0, &definition, FormationLayout::INITIAL_SPACING)?;
        layouts.push(CountLayout { count, offsets });
    }

    Ok(LayoutReport {
        sha256,
        segment_count,
        matches_embedded,
        layouts,
    })
}

/// 執行 Archive 模式：從唯讀 ZIP 提取並解碼士兵影格，寫入 RGBA 與 frames.json。
pub fn run_archive(
    archive_path: &Path,
    output_dir: &Path,
    asset_name: Option<&str>,
) -> Result<Vec<FrameExportInfo>, String> {
    archive::export_soldier_frames(archive_path, output_dir, asset_name)
}

/// 執行 Dump 模式：傾印 PFIL 二進位內容，格式對齊 probe.py。
pub fn run_dump(path: &Path, range: (usize, usize)) -> Result<String, String> {
    let clean = safety::validate_explicit_path(path).map_err(|e| format!("無效的輸入路徑：{e}"))?;
    let data =
        fs::read(&clean).map_err(|e| format!("無法讀取輸入檔案 '{}': {e}", clean.display()))?;
    pfil::format_dump(&data, range.0, range.1)
}

const USAGE: &str = "\
Usage:
  formation-probe-rs --layout <formdef.dau>
  formation-probe-rs <offline alr.dat> <NEW output directory> [ALR asset name]
  formation-probe-rs dump <path> [--dump <START:END>]
  formation-probe-rs <path> --dump <START:END>

Notes:
  - Output must be a new directory; existing files/directories are never overwritten.
  - Paths must be explicit; no game directories are scanned implicitly.
  - EXE disassembly is not supported by this crate (no disassembler dependency).
";

/// CLI 進入點驅動程式。回傳行程退出碼 (0: 成功, 1: 執行失敗, 2: 參數或安全衝突)。
pub fn cli(args: &[OsString], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let arg_strings: Vec<String> = args
        .iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();

    if arg_strings.is_empty()
        || arg_strings
            .iter()
            .any(|s| s == "-h" || s == "--help" || s == "/?")
    {
        let _ = write!(stderr, "{USAGE}");
        return 2;
    }

    // 檢查反組譯邊界：若使用者指定 --range 則明確告知邊界限制
    if arg_strings
        .iter()
        .any(|s| s == "--range" || s.starts_with("--range="))
    {
        let _ = writeln!(
            stderr,
            "formation-probe: 本工具不支援 EXE 反組譯 (未依賴反組譯引擎)。\n\
             PE/EXE 二進位反組譯明確劃歸 Capstone 或 REA 等專用反向工程工具。"
        );
        return 2;
    }

    // 1. --layout 模式
    if arg_strings.len() == 2 && arg_strings[0] == "--layout" {
        match run_layout(Path::new(&arg_strings[1])) {
            Ok(report) => match serde_json::to_string(&report) {
                Ok(json) => {
                    let _ = writeln!(stdout, "{json}");
                    0
                }
                Err(e) => {
                    let _ = writeln!(stderr, "序列化報告 JSON 失敗: {e}");
                    1
                }
            },
            Err(e) => {
                let _ = writeln!(stderr, "formation-probe: {e}");
                1
            }
        }
    }
    // 2. dump 模式
    else if arg_strings[0] == "dump"
        || arg_strings
            .iter()
            .any(|s| s == "--dump" || s.starts_with("--dump="))
    {
        let mut path_str: Option<&str> = None;
        let mut range_str = "0:256";

        let mut i = 0;
        while i < arg_strings.len() {
            let s = &arg_strings[i];
            if s == "dump" {
                i += 1;
                continue;
            }
            if s == "--dump" {
                if i + 1 < arg_strings.len() {
                    range_str = &arg_strings[i + 1];
                    i += 2;
                    continue;
                }
            } else if let Some(stripped) = s.strip_prefix("--dump=") {
                range_str = stripped;
                i += 1;
                continue;
            } else if path_str.is_none() {
                path_str = Some(s);
            }
            i += 1;
        }

        let Some(path) = path_str else {
            let _ = writeln!(stderr, "缺少 dump 輸入檔案路徑。\n{USAGE}");
            return 2;
        };

        let range = match parse_dump_range(range_str) {
            Ok(r) => r,
            Err(e) => {
                let _ = writeln!(stderr, "傾印區間錯誤: {e}");
                return 2;
            }
        };

        match run_dump(Path::new(path), range) {
            Ok(out) => {
                let _ = write!(stdout, "{out}");
                0
            }
            Err(e) => {
                let _ = writeln!(stderr, "formation-probe dump: {e}");
                1
            }
        }
    }
    // 3. archive 模式: <alr.dat> <output_dir> [asset_name]
    else if arg_strings.len() >= 2 && arg_strings.len() <= 3 && !arg_strings[0].starts_with('-') {
        let archive_path = Path::new(&arg_strings[0]);
        let output_dir = Path::new(&arg_strings[1]);
        let asset_name = arg_strings.get(2).map(|s| s.as_str());

        match run_archive(archive_path, output_dir, asset_name) {
            Ok(_) => {
                let _ = writeln!(stdout, "Exported frames");
                0
            }
            Err(e) => {
                let _ = writeln!(stderr, "formation-probe: {e}");
                if e.contains("已存在") || e.contains("無效") || e.contains("不安全") {
                    2
                } else {
                    1
                }
            }
        }
    }
    // 4. 其他無效參數
    else {
        let _ = writeln!(
            stderr,
            "參數錯誤。Usage: <offline alr.dat> <NEW output directory> [ALR asset name] 或 --layout <formdef.dau>"
        );
        2
    }
}
