use super::camera::MapStartCamera;
use super::compiler::CinematicBciCompiler;
use against_rome_scenario::BciImage;
use std::fs;
use std::path::{Path, PathBuf};

/// 將開局相機設定編譯為 IPR 位元碼並注入地圖的 ak_level.bci 腳本。
pub struct MapStartCameraInjector;

impl MapStartCameraInjector {
    pub const OP_JUMP: i32 = 112;
    pub const SCRIPT_FILE: &'static str = "ak_level.bci";
    pub const ORIGINAL_BACKUP_FILE: &'static str = "ak_level.arm_original";

    /// 在 BciImage 中注入開局相機 shim，並將 main 指向 shim。
    /// shim 依序呼叫開局相機 API 後，無條件跳躍回原 main。
    pub fn inject(image: &mut BciImage, camera: &MapStartCamera) -> Result<(), String> {
        if !camera.enabled {
            return Ok(());
        }

        let camera_calls = CinematicBciCompiler::compile_start_camera_calls(image, camera)?;
        let original_main = image.main;
        let start = image.code.len() as i32 * 4;
        let jump_at = start + camera_calls.len() as i32 * 4;
        let jump_operand = original_main - (jump_at + 8);

        let mut shim = camera_calls;
        shim.push(Self::OP_JUMP);
        shim.push(jump_operand);

        image.append(&shim);
        image.main = start;
        Ok(())
    }

    /// 尋找地圖的腳本目錄 (通常為 mapDirectory/SCRIPT)。
    pub fn script_directory(map_directory: &Path) -> PathBuf {
        if let Ok(entries) = fs::read_dir(map_directory) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.eq_ignore_ascii_case("SCRIPT") {
                                return entry.path();
                            }
                        }
                    }
                }
            }
        }
        map_directory.join("SCRIPT")
    }

    /// 將開局相機注入至地圖的 ak_level.bci 檔案，並儲存 arm_start_camera.json。
    /// 若原版腳本備份 ak_level.arm_original 不存在，則先行備份。
    pub fn apply(map_directory: &Path, camera: &MapStartCamera) -> Result<(), String> {
        let script_dir = Self::script_directory(map_directory);
        let script_path = script_dir.join(Self::SCRIPT_FILE);
        let backup_path = script_dir.join(Self::ORIGINAL_BACKUP_FILE);

        if !backup_path.exists() {
            if !camera.enabled {
                camera.save(map_directory)?;
                return Ok(());
            }
            if !script_path.exists() {
                return Err(format!(
                    "地圖缺少 ak_level.bci，無法注入開局相機: {}",
                    script_path.display()
                ));
            }
            let original_bytes =
                fs::read(&script_path).map_err(|e| format!("讀取原始腳本失敗: {e}"))?;
            fs::write(&backup_path, &original_bytes).map_err(|e| format!("建立備份失敗: {e}"))?;
        }

        let script_bytes = fs::read(&script_path).map_err(|e| format!("讀取腳本失敗: {e}"))?;
        let is_pfil = script_bytes.len() >= 64 && script_bytes.starts_with(b"PFIL");
        let raw = if is_pfil {
            against_rome_data::lzss::decompress_pfil(&script_bytes, None)
                .map_err(|e| e.to_string())?
        } else {
            script_bytes.clone()
        };

        let mut image = BciImage::parse(&raw)?;

        if camera.enabled {
            Self::inject(&mut image, camera)?;
            let serialized = image.serialize();
            let output = if is_pfil {
                Self::pack_pfil(&serialized, &script_bytes)?
            } else {
                serialized
            };
            fs::write(&script_path, output).map_err(|e| format!("寫入腳本失敗: {e}"))?;
        }

        camera.save(map_directory)?;
        Ok(())
    }

    /// 以 Literal-only LZSS 將已序列化的 BCI 位元碼打包回 PFIL 格式。
    fn pack_pfil(raw: &[u8], template_pfil: &[u8]) -> Result<Vec<u8>, String> {
        let mut out = template_pfil
            .get(..64)
            .ok_or_else(|| "截斷的 PFIL 標頭".to_string())?
            .to_vec();
        out[16..20].copy_from_slice(&(raw.len() as u32).to_le_bytes());
        for chunk in raw.chunks(8) {
            out.push(((1u16 << chunk.len()) - 1) as u8);
            out.extend_from_slice(chunk);
        }
        Ok(out)
    }
}
