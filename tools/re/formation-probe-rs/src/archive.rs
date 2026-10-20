use crate::safety;
use against_rome_data::AlrDocument;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameExportInfo {
    pub frame: usize,
    #[serde(rename = "Width")]
    pub width: usize,
    #[serde(rename = "Height")]
    pub height: usize,
    #[serde(rename = "AnchorX")]
    pub anchor_x: i32,
    #[serde(rename = "AnchorY")]
    pub anchor_y: i32,
}

/// 將 ARGB u32 (0xAARRGGBB) 陣列轉換為 RGBA 位元組陣列 (R, G, B, A)。
pub fn argb_to_rgba(pixels: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(pixels.len() * 4);
    for &p in pixels {
        out.push((p >> 16) as u8);
        out.push((p >> 8) as u8);
        out.push(p as u8);
        out.push((p >> 24) as u8);
    }
    out
}

/// 從唯讀 ALR zip archive 中安全取出資產、解碼至多 24 個士兵影格、輸出 RGBA 與 frames.json。
pub fn export_soldier_frames(
    archive_path: &Path,
    output_dir: &Path,
    asset_name: Option<&str>,
) -> Result<Vec<FrameExportInfo>, String> {
    let clean_archive =
        safety::validate_explicit_path(archive_path).map_err(|e| format!("無效的封裝路徑：{e}"))?;
    safety::validate_new_output_dir(output_dir).map_err(|e| format!("輸出目錄無效：{e}"))?;

    let raw_name = asset_name.unwrap_or("gerinf01.alr");
    safety::validate_asset_name(raw_name).map_err(|e| format!("資產名稱不安全：{e}"))?;

    let file = File::open(&clean_archive)
        .map_err(|e| format!("無法開啟封裝檔案 '{}': {e}", clean_archive.display()))?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| format!("無法解析 ZIP 封裝 '{}': {e}", clean_archive.display()))?;

    let target_subpath = if raw_name.starts_with("SYSTEM/DATA/ALR/")
        || raw_name.starts_with("SYSTEM\\DATA\\ALR\\")
    {
        raw_name.replace('\\', "/")
    } else {
        format!("SYSTEM/DATA/ALR/{raw_name}")
    };

    let mut target_index = None;
    for i in 0..zip.len() {
        if let Ok(entry) = zip.by_index_raw(i) {
            // ZipArchive.GetEntry in the C# probe uses the exact stored entry name.
            if entry.name() == target_subpath {
                target_index = Some(i);
                break;
            }
        }
    }

    let Some(idx) = target_index else {
        return Err(format!(
            "在 ZIP 封裝中找不到指定的 ALR 資產 '{target_subpath}'。"
        ));
    };

    let mut entry_data = Vec::new();
    let mut entry = zip
        .by_index(idx)
        .map_err(|e| format!("讀取 ZIP 項目失敗: {e}"))?;
    entry
        .read_to_end(&mut entry_data)
        .map_err(|e| format!("讀取 ZIP 項目資料失敗: {e}"))?;

    let doc = AlrDocument::parse(&entry_data).map_err(|e| format!("解析 ALR 資料失敗: {e}"))?;

    create_output_directory(output_dir)?;

    let max_frames = std::cmp::min(24, doc.layout_columns as usize);
    let mut info = Vec::with_capacity(max_frames);

    for frame in 0..max_frames {
        let index = if doc.layout_rows > 14 {
            14 * doc.layout_columns as usize + frame
        } else {
            frame
        };

        let pixels = doc
            .decode_frame_argb(index, 0)
            .map_err(|e| format!("解碼第 {index} 幀 (frame={frame}) 失敗: {e}"))?;
        let meta = doc
            .frames
            .get(index)
            .ok_or_else(|| format!("找不到影格中繼資料 (index={index})"))?
            .info;

        let rgba_bytes = argb_to_rgba(&pixels);
        let rgba_path = output_dir.join(format!("soldier-{frame}.rgba"));
        write_new_file(&rgba_path, &rgba_bytes)
            .map_err(|e| format!("無法寫入 RGBA 檔案 '{}': {e}", rgba_path.display()))?;

        info.push(FrameExportInfo {
            frame,
            width: meta.width,
            height: meta.height,
            anchor_x: (doc.anchor_width as i32) / 2 - meta.offset_x,
            anchor_y: (doc.anchor_height as i32) / 2 - meta.offset_y,
        });
    }

    let json_text =
        serde_json::to_string(&info).map_err(|e| format!("序列化 frames.json 失敗: {e}"))?;
    let json_path = output_dir.join("frames.json");
    write_new_file(&json_path, json_text.as_bytes())
        .map_err(|e| format!("無法寫入 frames.json 到 '{}': {e}", json_path.display()))?;

    Ok(info)
}

fn create_output_directory(path: &Path) -> Result<(), String> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
    if let Some(parent) = parent {
        fs::create_dir_all(parent)
            .map_err(|e| format!("無法建立輸出父目錄 '{}': {e}", parent.display()))?;
    }
    // `create_dir` is atomic for the leaf: if another process creates the
    // target after validation, fail instead of writing into its directory.
    fs::create_dir(path).map_err(|e| format!("無法建立輸出目錄 '{}': {e}", path.display()))
}

fn write_new_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)
}
