//! 戰役匯出閉環：模型 → 編譯 → 寫入地圖 `SCRIPT/ak_level.bci` → 回讀驗證。

use crate::cinematics::MapStartCameraInjector;
use crate::pipeline::{CampaignCompiler, CampaignPipelineOptions, CampaignPipelineResult};
use against_rome_scenario::{BciImage, ScenarioDocument, StartCamera};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// 匯出後回讀驗證摘要。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExportVerification {
    pub is_pfil: bool,
    /// 原始程式碼區被修補的字數（僅允許主迴圈等待指令對，最多 2）。
    pub patched_original_words: usize,
    pub appended_code_words: usize,
    pub appended_constants: usize,
    pub messages_found: usize,
    pub aliases_found: usize,
    pub native_calls_found: usize,
}

/// 戰役匯出成果。
#[derive(Debug, Clone)]
pub struct CampaignExportResult {
    pub success: bool,
    pub diagnostics: Vec<String>,
    pub script_path: Option<PathBuf>,
    pub backup_path: Option<PathBuf>,
    pub backup_created: bool,
    pub bytes_written: usize,
    pub verification: Option<ExportVerification>,
    pub pipeline: Option<CampaignPipelineResult>,
}

impl CampaignExportResult {
    fn failure(diagnostics: Vec<String>, pipeline: Option<CampaignPipelineResult>) -> Self {
        Self {
            success: false,
            diagnostics,
            script_path: None,
            backup_path: None,
            backup_created: false,
            bytes_written: 0,
            verification: None,
            pipeline,
        }
    }
}

/// 失敗時自動刪除暫存檔的守衛。
struct TempGuard {
    path: PathBuf,
    armed: bool,
}

impl Drop for TempGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn decode_script(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.starts_with(b"PFIL") {
        against_rome_data::lzss::decompress_pfil(bytes, None).map_err(|e| e.to_string())
    } else {
        Ok(bytes.to_vec())
    }
}

fn constant_set(image: &BciImage) -> Result<HashSet<Vec<u8>>, String> {
    (0..image.constant_offsets.len())
        .map(|i| image.constant(i).map(<[u8]>::to_vec))
        .collect()
}

/// 依「來源位元碼」與情境文件，驗證輸出位元碼確實只在來源後方追加內容：
/// 原始程式碼僅允許修補主迴圈等待指令對、常數/符號/變數表為來源前綴、
/// 入口指向追加區，且預期的訊息文字、別名與原生呼叫名稱皆存在。
pub fn verify_exported_script(
    source: &[u8],
    output: &[u8],
    document: &ScenarioDocument,
    camera: Option<&StartCamera>,
) -> Result<ExportVerification, String> {
    let is_pfil = output.starts_with(b"PFIL");
    if is_pfil != source.starts_with(b"PFIL") {
        return Err("輸出與來源的 PFIL 容器型態不一致".into());
    }
    if is_pfil {
        match (source.get(..64), output.get(..64)) {
            (Some(a), Some(b)) if a[..16] == b[..16] && a[20..] == b[20..] => {}
            _ => return Err("PFIL 標頭中，長度欄位以外的內容被改動".into()),
        }
    }
    let old = BciImage::parse(&decode_script(source)?).map_err(|e| format!("來源 BCI 無效: {e}"))?;
    let new = BciImage::parse(&decode_script(output)?).map_err(|e| format!("輸出 BCI 回讀失敗: {e}"))?;

    let has_events = document.events.iter().any(|e| e.enabled);
    let has_spawns = document.spawns.iter().any(|s| !s.prebuilt);
    let has_camera = camera.is_some_and(|c| c.enabled);
    if !has_events && !has_spawns && !has_camera {
        if old != new {
            return Err("情境無腳本內容，輸出卻與來源不同".into());
        }
        return Ok(ExportVerification {
            is_pfil,
            ..Default::default()
        });
    }
    if new == old {
        return Err("情境含啟用內容，輸出卻與來源位元碼完全相同".into());
    }

    if new.code.len() <= old.code.len() {
        return Err("輸出程式碼區未追加任何指令".into());
    }
    let patched = old.code.iter().zip(&new.code).filter(|(a, b)| a != b).count();
    if patched > 2 {
        return Err(format!(
            "原始程式碼區被修補 {patched} 個字，超出允許的 2 字（主迴圈等待指令對）"
        ));
    }
    if new.dtor != old.dtor {
        return Err("解構入口被改動".into());
    }
    if new.main != old.main && (new.main < old.code.len() as i32 * 4 || new.main % 4 != 0) {
        return Err("主入口未指向追加的程式碼區".into());
    }
    let prefix_ok = new.symbols.starts_with(&old.symbols)
        && new.constants.starts_with(&old.constants)
        && new.constant_offsets.starts_with(&old.constant_offsets)
        && new.variables.starts_with(&old.variables)
        && new.variable_offsets.starts_with(&old.variable_offsets);
    if !prefix_ok {
        return Err("常數/符號/變數表不是來源的前綴；原有索引已位移".into());
    }

    let constants = constant_set(&new)?;
    let mut messages_found = 0;
    let mut aliases_found = 0;
    for event in document.events.iter().filter(|e| e.enabled) {
        for action in &event.actions {
            match action.kind {
                0 => {
                    let (bytes, _, bad) = encoding_rs::WINDOWS_1251.encode(&action.text);
                    if bad || !constants.contains(bytes.as_ref()) {
                        return Err(format!("事件「{}」的訊息未出現在輸出常數表", event.name));
                    }
                    messages_found += 1;
                }
                2 => {
                    if !constants.contains(action.alias.as_bytes()) {
                        return Err(format!(
                            "事件「{}」的單位別名「{}」未出現在輸出常數表",
                            event.name, action.alias
                        ));
                    }
                    aliases_found += 1;
                }
                _ => {}
            }
        }
    }
    for spawn in document.spawns.iter().filter(|s| !s.prebuilt) {
        if !constants.contains(spawn.alias.as_bytes()) {
            return Err(format!("起始物件別名「{}」未出現在輸出常數表", spawn.alias));
        }
        aliases_found += 1;
    }
    let mut native_calls_found = 0;
    if let Some(c) = camera.filter(|c| c.enabled) {
        let natives: &[&str] = if c.primitive == 0 {
            &["s_centerToPos", "s_lgcSetEngineZoom"]
        } else {
            &["s_lgcSetEnginePos", "s_lgcSetEngineZoom"]
        };
        for name in natives {
            if !constants.contains(name.as_bytes()) {
                return Err(format!("開局相機原生呼叫「{name}」未出現在輸出常數表"));
            }
            native_calls_found += 1;
        }
    }

    Ok(ExportVerification {
        is_pfil,
        patched_original_words: patched,
        appended_code_words: new.code.len() - old.code.len(),
        appended_constants: new.constant_offsets.len() - old.constant_offsets.len(),
        messages_found,
        aliases_found,
        native_calls_found,
    })
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    file.write_all(bytes)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    file.sync_all().map_err(|e| format!("{}: {e}", path.display()))
}

impl CampaignCompiler {
    /// 單一閉環：模型 → 編譯 → 寫入 `<map>/SCRIPT/ak_level.bci` → 回讀驗證。
    ///
    /// 來源位元碼優先取 `ak_level.arm_original`（重複匯出不會疊加注入），否則取現有
    /// `ak_level.bci` 並於成功編譯後建立備份。輸出先寫入同目錄暫存檔並回讀驗證，通過後
    /// 才以改名取代；任何一步失敗皆保持原檔位元不變並清除暫存檔。
    pub fn export_to_map(
        options: CampaignPipelineOptions,
        map_directory: &Path,
    ) -> CampaignExportResult {
        let mut diagnostics = vec![];
        let script_dir = MapStartCameraInjector::script_directory(map_directory);
        let script_path = script_dir.join(MapStartCameraInjector::SCRIPT_FILE);
        let backup_path = script_dir.join(MapStartCameraInjector::ORIGINAL_BACKUP_FILE);
        let temp_path = script_dir.join(format!("{}.arm_tmp", MapStartCameraInjector::SCRIPT_FILE));
        let backup_exists = backup_path.exists();

        // 現有腳本（可能不存在）用於失敗還原，且無備份時作為來源。
        let current = match fs::read(&script_path) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                diagnostics.push(format!("[Error] 讀取 {} 失敗: {e}", script_path.display()));
                return CampaignExportResult::failure(diagnostics, None);
            }
        };
        let source = if backup_exists {
            match fs::read(&backup_path) {
                Ok(b) => {
                    diagnostics.push("[Info] 以 ak_level.arm_original 作為編譯來源。".into());
                    b
                }
                Err(e) => {
                    diagnostics.push(format!(
                        "[Error] 讀取備份 {} 失敗: {e}",
                        backup_path.display()
                    ));
                    return CampaignExportResult::failure(diagnostics, None);
                }
            }
        } else if let Some(bytes) = current.clone() {
            bytes
        } else {
            diagnostics.push(format!(
                "[Error] 地圖缺少 ak_level.bci 且無備份: {}",
                script_path.display()
            ));
            return CampaignExportResult::failure(diagnostics, None);
        };

        let pipeline = Self::compile_to_bci(options, &source);
        diagnostics.extend(pipeline.diagnostics.iter().cloned());
        let output = match (&pipeline.success, &pipeline.bci_bytes) {
            (true, Some(b)) => b.clone(),
            _ => {
                diagnostics.push("[Error] 編譯失敗，未寫入任何檔案。".into());
                return CampaignExportResult::failure(diagnostics, Some(pipeline));
            }
        };

        let camera: Option<StartCamera> = pipeline.resolved_camera.as_ref().map(Into::into);
        let verification = match verify_exported_script(
            &source,
            &output,
            &pipeline.scenario_document,
            camera.as_ref(),
        ) {
            Ok(v) => v,
            Err(e) => {
                diagnostics.push(format!("[Error] 記憶體內回讀驗證失敗: {e}"));
                return CampaignExportResult::failure(diagnostics, Some(pipeline));
            }
        };

        // 寫入暫存檔 → 磁碟回讀驗證。
        let mut guard = TempGuard {
            path: temp_path.clone(),
            armed: true,
        };
        if let Err(e) = write_synced(&temp_path, &output) {
            diagnostics.push(format!("[Error] 寫入暫存檔失敗: {e}"));
            return CampaignExportResult::failure(diagnostics, Some(pipeline));
        }
        let verify_disk = |path: &Path| -> Result<(), String> {
            let on_disk = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
            if on_disk != output {
                return Err(format!("{} 回讀位元組與預期不符", path.display()));
            }
            verify_exported_script(&source, &on_disk, &pipeline.scenario_document, camera.as_ref())
                .map(|_| ())
        };
        if let Err(e) = verify_disk(&temp_path) {
            diagnostics.push(format!("[Error] 暫存檔回讀驗證失敗: {e}"));
            return CampaignExportResult::failure(diagnostics, Some(pipeline));
        }

        // 無備份時，先備份原始腳本並回讀確認，才允許取代。
        let mut backup_created = false;
        if !backup_exists {
            let backup_temp = script_dir.join(format!(
                "{}.arm_tmp",
                MapStartCameraInjector::ORIGINAL_BACKUP_FILE
            ));
            let mut backup_guard = TempGuard {
                path: backup_temp.clone(),
                armed: true,
            };
            let result = write_synced(&backup_temp, &source)
                .and_then(|_| fs::rename(&backup_temp, &backup_path).map_err(|e| e.to_string()));
            if let Err(e) = result {
                diagnostics.push(format!("[Error] 建立 ak_level.arm_original 失敗: {e}"));
                return CampaignExportResult::failure(diagnostics, Some(pipeline));
            }
            backup_guard.armed = false;
            backup_created = true;
            if fs::read(&backup_path).ok().as_deref() != Some(source.as_slice()) {
                let _ = fs::remove_file(&backup_path);
                diagnostics.push("[Error] 備份回讀位元組不符，已移除備份且未取代腳本。".into());
                return CampaignExportResult::failure(diagnostics, Some(pipeline));
            }
        }

        // 取代並再次回讀；失敗則還原先前內容。
        let replaced = fs::rename(&temp_path, &script_path).map_err(|e| format!("取代腳本失敗: {e}"));
        guard.armed = replaced.is_err();
        if let Err(e) = replaced.and_then(|_| verify_disk(&script_path)) {
            diagnostics.push(format!("[Error] {e}"));
            let restore = match &current {
                Some(bytes) => {
                    let tmp = script_dir.join(format!(
                        "{}.arm_restore",
                        MapStartCameraInjector::SCRIPT_FILE
                    ));
                    let r = write_synced(&tmp, bytes)
                        .and_then(|_| fs::rename(&tmp, &script_path).map_err(|e| e.to_string()));
                    if r.is_err() {
                        let _ = fs::remove_file(&tmp);
                    }
                    r
                }
                None => fs::remove_file(&script_path).map_err(|e| e.to_string()),
            };
            match restore {
                Ok(()) => diagnostics.push("[Info] 已還原先前的 ak_level.bci。".into()),
                Err(re) => diagnostics.push(format!(
                    "[Error] 還原先前腳本失敗，請以 ak_level.arm_original 手動復原: {re}"
                )),
            }
            return CampaignExportResult::failure(diagnostics, Some(pipeline));
        }

        diagnostics.push(format!(
            "[Info] 已寫入並回讀驗證 {} ({} bytes, 追加 {} 個程式碼字)。",
            script_path.display(),
            output.len(),
            verification.appended_code_words
        ));
        CampaignExportResult {
            success: true,
            diagnostics,
            script_path: Some(script_path),
            backup_path: Some(backup_path),
            backup_created,
            bytes_written: output.len(),
            verification: Some(verification),
            pipeline: Some(pipeline),
        }
    }
}
