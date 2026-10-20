//! Initialization of new blank map content (`BlankMapContent`).
//! Operates only on uncommitted staging directory copies (`*.tmp_arm`).

use std::fs;
use std::path::{Path, PathBuf};
use against_rome_scenario::bci::BciImage;
use against_rome_scenario::model::ScenarioDocument;
use crate::rollback::{FileRollbackScope, SafeFileWriter};
use crate::text_encoding::{compress_pfil, decompress_pfil_if_needed, is_pfil};

struct RuntimeSlotsLayout {
    name: &'static str,
    header: usize,
    record: usize,
    columns: &'static [usize],
    extra_header: &'static [i32],
    state_width: usize,
}

const RUNTIME_LAYOUTS: &[RuntimeSlotsLayout] = &[
    RuntimeSlotsLayout {
        name: "anim.dat",
        header: 8,
        record: 21,
        columns: &[2, 2],
        extra_header: &[],
        state_width: 1,
    },
    RuntimeSlotsLayout {
        name: "gfxtype.dat",
        header: 8,
        record: 15,
        columns: &[2],
        extra_header: &[],
        state_width: 1,
    },
    RuntimeSlotsLayout {
        name: "action.dat",
        header: 12,
        record: 25,
        columns: &[],
        extra_header: &[6],
        state_width: 1,
    },
    RuntimeSlotsLayout {
        name: "hirarchy.dat",
        header: 12,
        record: 103,
        columns: &[2],
        extra_header: &[50],
        state_width: 1,
    },
    RuntimeSlotsLayout {
        name: "formatio.dat",
        header: 8,
        record: 15,
        columns: &[4, 2, 4],
        extra_header: &[],
        state_width: 1,
    },
    RuntimeSlotsLayout {
        name: "explos.dat",
        header: 8,
        record: 46,
        columns: &[4, 4],
        extra_header: &[],
        state_width: 2,
    },
    RuntimeSlotsLayout {
        name: "hitex.dat",
        header: 8,
        record: 28,
        columns: &[4, 4, 4],
        extra_header: &[],
        state_width: 2,
    },
    RuntimeSlotsLayout {
        name: "flash.dat",
        header: 12,
        record: 201,
        columns: &[32],
        extra_header: &[16],
        state_width: 1,
    },
];

pub struct BlankMapContent;

impl BlankMapContent {
    /// Resets an uncommitted staging copy (*.tmp_arm) for a blank map.
    /// Resets runtime pools, native objects, lights, particles, camps, and scripts,
    /// while preserving SDL settlement templates.
    pub fn reset_staging_directory(directory: &Path) -> Result<(), String> {
        let full = directory.canonicalize().unwrap_or_else(|_| directory.to_path_buf());
        let dir_name = full.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !dir_name.to_ascii_lowercase().ends_with(".tmp_arm") {
            return Err("空白初始化只能作用於尚未登錄的地圖暫存副本。".into());
        }

        let parent_name = full
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if !parent_name.eq_ignore_ascii_case("MAPS") {
            return Err("空白初始化只能作用於尚未登錄的地圖暫存副本。".into());
        }

        // Check for symlinks / reparse points
        check_reparse_points(&full)?;

        let data_dir = full.join("DATA");
        if !data_dir.is_dir() {
            return Err(format!("空白初始化缺少必要目錄 DATA ({}).", data_dir.display()));
        }

        // Required dedicated pool files
        for name in &["light.dat", "particle.dat", "lager.dat", "biglager.dat"] {
            let p = data_dir.join(name);
            if !p.is_file() {
                return Err(format!("空白初始化缺少必要資料池 {name}。"));
            }
        }

        // Prepare each runtime binary in memory first
        let mut pending_writes: Vec<(PathBuf, Vec<u8>)> = Vec::new();
        for layout in RUNTIME_LAYOUTS {
            let path = data_dir.join(layout.name);
            if !path.is_file() {
                return Err(format!("空白初始化缺少必要資料池 {}。", layout.name));
            }
            let bytes = empty_slots(&path, layout)?;
            pending_writes.push((path, bytes));
        }

        let way_path = data_dir.join("way.dat");
        if !way_path.is_file() {
            return Err("空白初始化缺少必要資料池 way.dat。".into());
        }
        let way_bytes = empty_ways(&way_path)?;
        pending_writes.push((way_path, way_bytes));

        let objects_path = data_dir.join("objects.dat");
        if !objects_path.is_file() {
            return Err("空白初始化缺少必要資料池 objects.dat。".into());
        }
        let objects_bytes = reset_objects_dat(&objects_path)?;
        pending_writes.push((objects_path, objects_bytes));

        let objdata_path = data_dir.join("objdata.dat");
        if !objdata_path.is_file() {
            return Err("空白初始化缺少必要資料池 objdata.dat。".into());
        }
        let objdata_bytes = reset_objdata_dat(&objdata_path)?;
        pending_writes.push((objdata_path, objdata_bytes));

        let position_path = data_dir.join("position.dat");
        if !position_path.is_file() {
            return Err("空白初始化缺少必要資料池 position.dat。".into());
        }
        let position_bytes = reset_position_dat(&position_path)?;
        pending_writes.push((position_path, position_bytes));

        let light_path = data_dir.join("light.dat");
        let light_bytes = reset_light_dat(&light_path)?;
        pending_writes.push((light_path, light_bytes));

        let particle_path = data_dir.join("particle.dat");
        let particle_bytes = reset_particle_dat(&particle_path)?;
        pending_writes.push((particle_path, particle_bytes));

        let lager_path = data_dir.join("lager.dat");
        let lager_bytes = reset_lager_dat(&lager_path)?;
        pending_writes.push((lager_path, lager_bytes));

        let biglager_path = data_dir.join("biglager.dat");
        let biglager_bytes = reset_biglager_dat(&biglager_path)?;
        pending_writes.push((biglager_path, biglager_bytes));

        // Start transaction for atomic writes
        let mut rollback = FileRollbackScope::new()?;

        for (path, bytes) in pending_writes {
            SafeFileWriter::write_all_bytes(&path, &bytes, Some(&mut rollback))?;
        }

        // Reset scenario document (arm_scenario.json)
        let scenario = ScenarioDocument {
            version: 6,
            spawns: Vec::new(),
            data_slots: Vec::new(),
            events: Vec::new(),
        };
        let scenario_json = serde_json::to_string_pretty(&scenario)
            .map_err(|e| format!("序列化場景失敗: {e}"))?;
        SafeFileWriter::write_all_bytes(
            &full.join("arm_scenario.json"),
            scenario_json.as_bytes(),
            Some(&mut rollback),
        )?;

        // Reset scripts
        reset_scripts_directory(&full, &mut rollback)?;

        rollback.commit();
        Ok(())
    }
}

fn check_reparse_points(dir: &Path) -> Result<(), String> {
    if is_symlink_or_reparse(dir) {
        return Err("空白初始化不接受 reparse points。".into());
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if is_symlink_or_reparse(&p) {
                return Err("空白初始化不接受 reparse points。".into());
            }
            if p.is_dir() {
                check_reparse_points(&p)?;
            }
        }
    }
    Ok(())
}

fn is_symlink_or_reparse(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

fn empty_slots(path: &Path, layout: &RuntimeSlotsLayout) -> Result<Vec<u8>, String> {
    let original = fs::read(path).map_err(|e| format!("讀取 {} 失敗: {e}", path.display()))?;
    let mut data = decompress_pfil_if_needed(&original)?;

    if data.len() < layout.header || read_i32(&data, 0)? != 1 {
        return Err(format!("{} 版本或標頭不符。", layout.name));
    }

    let count = read_i32(&data, 4)? as usize;
    let size = layout.record + layout.columns.iter().sum::<usize>();
    if count < 1 || count > 33000 || layout.header + count * size != data.len() {
        return Err(format!("{} 槽位／長度不符。", layout.name));
    }

    for (index, &expected) in layout.extra_header.iter().enumerate() {
        if read_i32(&data, 8 + index * 4)? != expected {
            return Err(format!("{} 陣列欄寬不符。", layout.name));
        }
    }

    // Find first empty slot
    let mut empty_slot = None;
    for slot in 0..count {
        let record_start = layout.header + slot * layout.record;
        let is_empty = data[record_start..record_start + layout.state_width]
            .iter()
            .all(|&b| b == 0);
        if is_empty {
            empty_slot = Some(slot);
            break;
        }
    }

    let empty = empty_slot.ok_or_else(|| format!("{} 無空槽範本。", layout.name))?;
    let record_start = layout.header + empty * layout.record;
    let record_bytes = data[record_start..record_start + layout.record].to_vec();

    for slot in 0..count {
        let dst = layout.header + slot * layout.record;
        data[dst..dst + layout.record].copy_from_slice(&record_bytes);
    }

    let mut start = layout.header + count * layout.record;
    for &width in layout.columns {
        let col_start = start + empty * width;
        let col_bytes = data[col_start..col_start + width].to_vec();
        for slot in 0..count {
            let dst = start + slot * width;
            data[dst..dst + width].copy_from_slice(&col_bytes);
        }
        start += count * width;
    }

    pack_data(data, &original)
}

fn empty_ways(path: &Path) -> Result<Vec<u8>, String> {
    let original = fs::read(path).map_err(|e| format!("讀取 way.dat 失敗: {e}"))?;
    let mut data = decompress_pfil_if_needed(&original)?;

    if data.len() < 12 || read_i32(&data, 0)? != 1 || read_i32(&data, 8)? != 256 {
        return Err("way.dat 版本或路徑容量不符。".into());
    }

    let count = read_i32(&data, 4)? as usize;
    if count < 1 || count > 33000 || 12 + count * 1030 != data.len() {
        return Err("way.dat 槽位／長度不符。".into());
    }

    data[12..].fill(0);
    pack_data(data, &original)
}

fn reset_objects_dat(path: &Path) -> Result<Vec<u8>, String> {
    let original = fs::read(path).map_err(|e| format!("讀取 objects.dat 失敗: {e}"))?;
    let mut data = decompress_pfil_if_needed(&original)?;

    if data.len() < 8 || read_i32(&data, 0)? != 1 {
        return Err("objects.dat 版本或標頭不符。".into());
    }
    let count = read_i32(&data, 4)? as usize;
    if data.len() != 8 + count * 56 {
        return Err("objects.dat 長度或槽位數不符。".into());
    }

    // Find empty record template
    let mut empty_slot = None;
    for slot in 0..count {
        let start = 8 + slot * 30;
        if data[start] == 0 {
            empty_slot = Some(slot);
            break;
        }
    }

    let empty_record = if let Some(slot) = empty_slot {
        data[8 + slot * 30..8 + slot * 30 + 30].to_vec()
    } else {
        vec![0u8; 30]
    };

    // 13 columns of width 2
    let col_base = 8 + count * 30;
    let mut empty_cols = Vec::new();
    for col in 0..13 {
        let col_val = if let Some(slot) = empty_slot {
            let offset = col_base + col * count * 2 + slot * 2;
            [data[offset], data[offset + 1]]
        } else {
            [0u8, 0u8]
        };
        empty_cols.push(col_val);
    }

    for slot in 0..count {
        data[8 + slot * 30..8 + slot * 30 + 30].copy_from_slice(&empty_record);
        for col in 0..13 {
            let offset = col_base + col * count * 2 + slot * 2;
            let val = if col == 12 {
                [0xFF, 0xFF] // DirectLightColumn = -1 (0xFFFF)
            } else {
                empty_cols[col]
            };
            data[offset..offset + 2].copy_from_slice(&val);
        }
    }

    pack_data(data, &original)
}

fn reset_objdata_dat(path: &Path) -> Result<Vec<u8>, String> {
    let original = fs::read(path).map_err(|e| format!("讀取 objdata.dat 失敗: {e}"))?;
    let mut data = decompress_pfil_if_needed(&original)?;

    if data.len() < 8 || read_i32(&data, 0)? != 1 {
        return Err("objdata.dat 版本或標頭不符。".into());
    }
    let count = read_i32(&data, 4)? as usize;
    if data.len() != 8 + count * 9 {
        return Err("objdata.dat 長度或槽位數不符。".into());
    }

    let mut empty_slot = None;
    for slot in 0..count {
        if data[8 + slot] == 0 {
            empty_slot = Some(slot);
            break;
        }
    }

    for segment in 0..9 {
        let empty_val = if let Some(slot) = empty_slot {
            data[8 + segment * count + slot]
        } else {
            0
        };
        let seg_start = 8 + segment * count;
        data[seg_start..seg_start + count].fill(empty_val);
    }

    pack_data(data, &original)
}

fn reset_position_dat(path: &Path) -> Result<Vec<u8>, String> {
    let original = fs::read(path).map_err(|e| format!("讀取 position.dat 失敗: {e}"))?;
    let mut data = decompress_pfil_if_needed(&original)?;

    if data.len() < 8 || read_i32(&data, 0)? != 1 {
        return Err("position.dat 版本或標頭不符。".into());
    }

    data[8..].fill(0);
    pack_data(data, &original)
}

fn reset_light_dat(path: &Path) -> Result<Vec<u8>, String> {
    let original = fs::read(path).map_err(|e| format!("讀取 light.dat 失敗: {e}"))?;
    let mut data = decompress_pfil_if_needed(&original)?;

    const CAPACITY: usize = 1024;
    const PAYLOAD_LEN: usize = 8 + CAPACITY * 56;
    if data.len() != PAYLOAD_LEN || read_i32(&data, 0)? != 1 || read_i32(&data, 4)? != CAPACITY as i32 {
        return Err("light.dat 必須是完整的 version1／1024 槽平行欄位布局。".into());
    }

    let widths = [4, 4, 4, 4, 4, 4, 4, 4, 4, 2, 4, 4, 4, 2, 2, 2];
    let mut offsets = [0usize; 16];
    let mut offset = 8;
    for (i, &w) in widths.iter().enumerate() {
        offsets[i] = offset;
        offset += CAPACITY * w;
    }

    for slot in 0..CAPACITY {
        for (i, &w) in widths.iter().enumerate() {
            let o = offsets[i] + slot * w;
            data[o..o + w].fill(0);
        }
        for i in 13..16 {
            let o = offsets[i] + slot * widths[i];
            data[o..o + 2].copy_from_slice(&(-1i16).to_le_bytes());
        }
    }

    pack_data(data, &original)
}

fn reset_particle_dat(path: &Path) -> Result<Vec<u8>, String> {
    let original = fs::read(path).map_err(|e| format!("讀取 particle.dat 失敗: {e}"))?;
    let mut data = decompress_pfil_if_needed(&original)?;

    const CAPACITY: usize = 1024;
    const RECORD_SIZE: usize = 432;
    const PAYLOAD_LEN: usize = 12 + CAPACITY * RECORD_SIZE + 4 * CAPACITY * 4;
    if data.len() != PAYLOAD_LEN || read_i32(&data, 0)? != 1 || read_i32(&data, 4)? != CAPACITY as i32 {
        return Err("particle.dat 結構不符。".into());
    }

    for slot in 0..CAPACITY {
        let record = 12 + slot * RECORD_SIZE;
        data[record..record + RECORD_SIZE].fill(0);
        data[record + 6..record + 8].copy_from_slice(&(-1i16).to_le_bytes());
        for child in 0..12 {
            let child_offset = record + 12 + child * 35 + 33;
            data[child_offset..child_offset + 2].copy_from_slice(&(-1i16).to_le_bytes());
        }
        for column in 0..4 {
            let col_offset = 12 + CAPACITY * RECORD_SIZE + column * CAPACITY * 4 + slot * 4;
            let val = if column == 3 { 30i32 } else { 0i32 };
            data[col_offset..col_offset + 4].copy_from_slice(&val.to_le_bytes());
        }
    }

    pack_data(data, &original)
}

fn reset_lager_dat(path: &Path) -> Result<Vec<u8>, String> {
    let original = fs::read(path).map_err(|e| format!("讀取 lager.dat 失敗: {e}"))?;
    let mut data = decompress_pfil_if_needed(&original)?;

    const CAPACITY: usize = 3200;
    const RECORD_SIZE: usize = 43;
    const PAYLOAD_LEN: usize = 16 + CAPACITY * (RECORD_SIZE + 2);
    if data.len() != PAYLOAD_LEN
        || read_i32(&data, 0)? != 1
        || read_i32(&data, 4)? != CAPACITY as i32
        || read_i32(&data, 8)? != 6
        || read_i32(&data, 12)? != 10
    {
        return Err("lager.dat 必須是 version1／3200 槽／6 輔助欄／10 成員的完整布局。".into());
    }

    for slot in 0..CAPACITY {
        let record = 16 + slot * RECORD_SIZE;
        data[record..record + RECORD_SIZE].fill(0);
        data[record + 23..record + 43].fill(255);
        let tail = 16 + CAPACITY * RECORD_SIZE + slot * 2;
        data[tail..tail + 2].copy_from_slice(&(-1i16).to_le_bytes());
    }

    pack_data(data, &original)
}

fn reset_biglager_dat(path: &Path) -> Result<Vec<u8>, String> {
    let original = fs::read(path).map_err(|e| format!("讀取 biglager.dat 失敗: {e}"))?;
    let mut data = decompress_pfil_if_needed(&original)?;

    const CAPACITY: usize = 800;
    const RECORD_SIZE: usize = 1601;
    const PAYLOAD_LEN: usize = 12 + CAPACITY * RECORD_SIZE;
    if data.len() != PAYLOAD_LEN || read_i32(&data, 0)? != 1 || read_i32(&data, 4)? != CAPACITY as i32 {
        return Err("biglager.dat 結構不符。".into());
    }

    for slot in 0..CAPACITY {
        let record = 12 + slot * RECORD_SIZE;
        data[record] = 0;
        data[record + 1..record + 1 + 800 * 2].fill(255);
    }

    pack_data(data, &original)
}

fn reset_scripts_directory(map_dir: &Path, rollback: &mut FileRollbackScope) -> Result<(), String> {
    let scripts_dir = find_script_dir(map_dir);
    if !scripts_dir.is_dir() {
        fs::create_dir_all(&scripts_dir)
            .map_err(|e| format!("建立 SCRIPT 目錄失敗 ({}): {e}", scripts_dir.display()))?;
    }

    let level_script = scripts_dir.join("ak_level.bci");
    let original_bci = if level_script.is_file() {
        fs::read(&level_script).unwrap_or_default()
    } else {
        Vec::new()
    };

    let idle_bci = BciImage::idle().serialize();
    let bootstrap = if is_pfil(&original_bci) && original_bci.len() >= 64 {
        compress_pfil(&idle_bci, &original_bci[..64])?
    } else {
        idle_bci
    };

    // Remove any extraneous .bci files or existing backup files in SCRIPT/
    if let Ok(entries) = fs::read_dir(&scripts_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                if ext.eq_ignore_ascii_case("bci") || name.eq_ignore_ascii_case("ak_level.arm_original") {
                    SafeFileWriter::delete_file(&path, Some(rollback))?;
                }
            }
        }
    }

    // Write new idle bootstrap and backup
    SafeFileWriter::write_all_bytes(&level_script, &bootstrap, Some(rollback))?;
    SafeFileWriter::write_all_bytes(
        &scripts_dir.join("ak_level.arm_original"),
        &bootstrap,
        Some(rollback),
    )?;

    Ok(())
}

fn find_script_dir(map_dir: &Path) -> PathBuf {
    if let Ok(entries) = fs::read_dir(map_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                    if name.eq_ignore_ascii_case("script") {
                        return p;
                    }
                }
            }
        }
    }
    map_dir.join("SCRIPT")
}

fn read_i32(data: &[u8], offset: usize) -> Result<i32, String> {
    if offset + 4 > data.len() {
        return Err("資料過短，無法讀取 int32。".into());
    }
    Ok(i32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]))
}

fn pack_data(data: Vec<u8>, original: &[u8]) -> Result<Vec<u8>, String> {
    if is_pfil(original) && original.len() >= 64 {
        compress_pfil(&data, &original[..64])
    } else {
        Ok(data)
    }
}
