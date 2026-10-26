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

/// Files changed or deliberately carried forward when a blank scene is initialized.
/// Paths are relative to the staging map root and use `/` separators for stable UI output.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BlankMapResetReport {
    pub reset_data_files: Vec<String>,
    pub preserved_data_files: Vec<String>,
    pub preserved_sdl_templates: Vec<String>,
    pub initialized_scenario_file: String,
    pub generated_script_files: Vec<String>,
}

impl BlankMapContent {
    /// Resets an uncommitted staging copy (*.tmp_arm) for a blank map.
    /// Resets runtime pools, native objects, lights, particles, camps, and scripts,
    /// while preserving SDL settlement templates.
    pub fn reset_staging_directory(directory: &Path) -> Result<(), String> {
        Self::reset_staging_directory_with_report(directory).map(|_| ())
    }

    /// Like `reset_staging_directory`, but also returns the exact DATA files reset and the
    /// template files left untouched. The report is emitted only after the transaction commits.
    pub fn reset_staging_directory_with_report(
        directory: &Path,
    ) -> Result<BlankMapResetReport, String> {
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

        let all_data_files = matching_files(&data_dir, None, true)?;
        let preserved_sdl_templates = matching_files(&full, Some("sdl"), true)?;

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

        let mut reset_data_files: Vec<String> = pending_writes
            .iter()
            .filter_map(|(path, _)| path.file_name().and_then(|name| name.to_str()))
            .map(|name| format!("DATA/{name}"))
            .collect();
        sort_paths(&mut reset_data_files);
        let reset_paths: std::collections::HashSet<String> = reset_data_files
            .iter()
            .map(|path| path.to_ascii_lowercase())
            .collect();
        let mut preserved_data_files: Vec<String> = all_data_files
            .into_iter()
            .map(|name| format!("DATA/{name}"))
            .filter(|path| !reset_paths.contains(&path.to_ascii_lowercase()))
            .collect();
        sort_paths(&mut preserved_data_files);

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
        let mut generated_script_files = reset_scripts_directory(&full, &mut rollback)?;
        sort_paths(&mut generated_script_files);

        rollback.commit();
        Ok(BlankMapResetReport {
            reset_data_files,
            preserved_data_files,
            preserved_sdl_templates,
            initialized_scenario_file: "arm_scenario.json".to_string(),
            generated_script_files,
        })
    }
}

fn matching_files(root: &Path, extension: Option<&str>, recursive: bool) -> Result<Vec<String>, String> {
    fn visit(root: &Path, current: &Path, extension: Option<&str>, recursive: bool, out: &mut Vec<String>) -> Result<(), String> {
        let entries = fs::read_dir(current)
            .map_err(|e| format!("讀取目錄 {} 失敗: {e}", current.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("讀取目錄項目 {} 失敗: {e}", current.display()))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|e| format!("讀取檔案類型 {} 失敗: {e}", path.display()))?;
            let extension_matches = extension.is_none_or(|expected| {
                path.extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.eq_ignore_ascii_case(expected))
            });
            if file_type.is_file() && extension_matches {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|e| format!("整理相對路徑失敗: {e}"))?
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push(relative);
            } else if recursive && file_type.is_dir() {
                visit(root, &path, extension, recursive, out)?;
            }
        }
        Ok(())
    }

    let mut files = Vec::new();
    visit(root, root, extension, recursive, &mut files)?;
    sort_paths(&mut files);
    Ok(files)
}

fn sort_paths(paths: &mut [String]) {
    paths.sort_by(|left, right| {
        left.to_ascii_lowercase()
            .cmp(&right.to_ascii_lowercase())
            .then_with(|| left.cmp(right))
    });
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
    if !(1..=33000).contains(&count) { return Err("objects.dat 槽位數不符。".into()); }
    // 現行零售 objects 使用 79 位元組紀錄與 18 個平行欄位，保留舊版布局相容性。
    let columns = crate::starter_kit::native::COLUMNS;
    if data.len() == 16 + count * (79 + columns.iter().sum::<usize>()) {
        if read_i32(&data, 8)? != 30 || read_i32(&data, 12)? != 30 {
            return Err("objects.dat 字串欄寬不符。".into());
        }
        data[16..].fill(0);
        for slot in 0..count {
            for offset in [67, 69, 71, 73, 77] {
                let start = 16 + slot * 79 + offset;
                data[start..start + 2].fill(255);
            }
            for (column, width) in columns.iter().enumerate() {
                if *width == 2 {
                    let start = 16 + count * 79 + count * columns[..column].iter().sum::<usize>() + slot * width;
                    data[start..start + 2].fill(255);
                }
            }
        }
        return pack_data(data, &original);
    }
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
    if !(1..=33000).contains(&count) { return Err("objdata.dat 槽位數不符。".into()); }
    if data.len() == 8 + count * crate::starter_kit::native::SEGMENTS.iter().sum::<usize>() {
        data[8..].fill(0);
        return pack_data(data, &original);
    }
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

    let count = read_i32(&data, 4)? as usize;
    if !(1..=33000).contains(&count) || data.len() != 8 + count * 17 {
        return Err("position.dat 槽位／長度不符。".into());
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
    if data.len() < 12 || read_i32(&data, 0)? != 1 || read_i32(&data, 4)? != CAPACITY as i32 {
        return Err("particle.dat 結構不符。".into());
    }

    let h8 = read_i32(&data, 8)?;
    let children = if data.len() == against_rome_data::particle::PAYLOAD_LEN && h8 == 64 {
        against_rome_data::particle::ParticlePool::from_payload(&data).map_err(|e| e.to_string())?;
        64
    } else if (h8 == 4 || h8 == 12) && data.len() == 12 + CAPACITY * 432 + 4 * CAPACITY * 4 {
        12
    } else {
        return Err(format!("particle.dat 結構或子項目數標頭 ({h8}) 不符。"));
    };

    let record_size = 12 + children * 35;
    let payload_len = 12 + CAPACITY * record_size + 4 * CAPACITY * 4;
    if data.len() != payload_len {
        return Err(format!("particle.dat 長度 {} 與預期 {} 不符。", data.len(), payload_len));
    }

    for slot in 0..CAPACITY {
        let record = 12 + slot * record_size;
        data[record..record + record_size].fill(0);
        data[record + 6..record + 8].copy_from_slice(&(-1i16).to_le_bytes());
        for child in 0..children {
            let child_offset = record + 12 + child * 35 + 33;
            data[child_offset..child_offset + 2].copy_from_slice(&(-1i16).to_le_bytes());
        }
        for column in 0..4 {
            let col_offset = 12 + CAPACITY * record_size + column * CAPACITY * 4 + slot * 4;
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

    // 零售版有 32 個營地，每個營地包含 800 個索引。
    if data.len() == 51244 && read_i32(&data, 0)? == 1 && read_i32(&data, 4)? == 32 && read_i32(&data, 8)? == 800 {
        for record in data[12..].chunks_exact_mut(1601) {
            record[0] = 0;
            record[1..].fill(255);
        }
        return pack_data(data, &original);
    }
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

fn reset_scripts_directory(map_dir: &Path, rollback: &mut FileRollbackScope) -> Result<Vec<String>, String> {
    let scripts_dir = find_script_dir(map_dir);
    if !scripts_dir.is_dir() {
        fs::create_dir_all(&scripts_dir)
            .map_err(|e| format!("建立 SCRIPT 目錄失敗 ({}): {e}", scripts_dir.display()))?;
    }
    let script_prefix = scripts_dir
        .strip_prefix(map_dir)
        .map_err(|e| format!("整理 SCRIPT 相對路徑失敗: {e}"))?
        .to_string_lossy()
        .replace('\\', "/");

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

    Ok(vec![
        format!("{script_prefix}/ak_level.arm_original"),
        format!("{script_prefix}/ak_level.bci"),
    ])
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

#[cfg(test)]
mod tests {
    use super::*;

    struct TempFile(PathBuf);
    impl TempFile {
        fn new(name: &str, bytes: &[u8]) -> Self {
            let path = std::env::temp_dir().join(format!("arm_test_blank_{}_{}_{name}", std::process::id(), uuid::Uuid::new_v4()));
            fs::write(&path, bytes).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    #[test]
    fn reset_particle_dat_supports_retail_layout() {
        let mut raw = vec![0u8; against_rome_data::particle::PAYLOAD_LEN];
        raw[0..4].copy_from_slice(&1i32.to_le_bytes());
        raw[4..8].copy_from_slice(&1024i32.to_le_bytes());
        raw[8..12].copy_from_slice(&64i32.to_le_bytes());
        raw[12..].fill(0xAA);
        let temp = TempFile::new("particle_retail.dat", &raw);

        let reset = reset_particle_dat(temp.path()).unwrap();
        assert_eq!(reset.len(), against_rome_data::particle::PAYLOAD_LEN);

        let pool = against_rome_data::particle::ParticlePool::from_payload(&reset).unwrap();
        assert_eq!(pool.active_slots().count(), 0);

        let snap0 = pool.snapshot_any(0).unwrap();
        assert_eq!(snap0.active_raw(), 0);
        assert_eq!(snap0.owner(), -1);
        assert_eq!(snap0.state_words(), [0, 0, 0, 30]);
        for c in 0..64 {
            let child = snap0.child(c).unwrap();
            assert_eq!(child.light_link(), -1);
            assert_eq!(child.active_raw(), 0);
        }

        let snap_last = pool.snapshot_any(1023).unwrap();
        assert_eq!(snap_last.active_raw(), 0);
        assert_eq!(snap_last.owner(), -1);
        assert_eq!(snap_last.state_words(), [0, 0, 0, 30]);
        for c in 0..64 {
            let child = snap_last.child(c).unwrap();
            assert_eq!(child.light_link(), -1);
        }
    }

    #[test]
    fn reset_particle_dat_supports_legacy_layout() {
        for &legacy_h8 in &[4i32, 12i32] {
            const LEGACY_LEN: usize = 458764;
            let mut raw = vec![0u8; LEGACY_LEN];
            raw[0..4].copy_from_slice(&1i32.to_le_bytes());
            raw[4..8].copy_from_slice(&1024i32.to_le_bytes());
            raw[8..12].copy_from_slice(&legacy_h8.to_le_bytes());
            raw[12..].fill(0xBB);
            let temp = TempFile::new("particle_legacy.dat", &raw);

            let reset = reset_particle_dat(temp.path()).unwrap();
            assert_eq!(reset.len(), LEGACY_LEN);
            assert_eq!(read_i32(&reset, 0).unwrap(), 1);
            assert_eq!(read_i32(&reset, 4).unwrap(), 1024);
            assert_eq!(read_i32(&reset, 8).unwrap(), legacy_h8);

            let slot0_owner = i16::from_le_bytes([reset[12 + 6], reset[12 + 7]]);
            assert_eq!(slot0_owner, -1);
            for c in 0..12 {
                let offset = 12 + 12 + c * 35 + 33;
                let link = i16::from_le_bytes([reset[offset], reset[offset + 1]]);
                assert_eq!(link, -1);
            }
            let col3_slot0 = 12 + 1024 * 432 + 3 * 1024 * 4;
            assert_eq!(read_i32(&reset, col3_slot0).unwrap(), 30);
            let col0_slot0 = 12 + 1024 * 432 + 0 * 1024 * 4;
            assert_eq!(read_i32(&reset, col0_slot0).unwrap(), 0);

            let slot_last_owner = i16::from_le_bytes([reset[12 + 1023 * 432 + 6], reset[12 + 1023 * 432 + 7]]);
            assert_eq!(slot_last_owner, -1);
            for c in 0..12 {
                let offset = 12 + 1023 * 432 + 12 + c * 35 + 33;
                let link = i16::from_le_bytes([reset[offset], reset[offset + 1]]);
                assert_eq!(link, -1);
            }
            let col3_last = 12 + 1024 * 432 + 3 * 1024 * 4 + 1023 * 4;
            assert_eq!(read_i32(&reset, col3_last).unwrap(), 30);
        }
    }

    #[test]
    fn reset_particle_dat_rejects_invalid_headers_and_counts() {
        let mut raw = vec![0u8; against_rome_data::particle::PAYLOAD_LEN];
        raw[0..4].copy_from_slice(&2i32.to_le_bytes());
        raw[4..8].copy_from_slice(&1024i32.to_le_bytes());
        raw[8..12].copy_from_slice(&64i32.to_le_bytes());
        let temp = TempFile::new("bad_ver.dat", &raw);
        assert!(reset_particle_dat(temp.path()).is_err());

        raw[0..4].copy_from_slice(&1i32.to_le_bytes());
        raw[4..8].copy_from_slice(&512i32.to_le_bytes());
        let temp = TempFile::new("bad_cap.dat", &raw);
        assert!(reset_particle_dat(temp.path()).is_err());

        raw[4..8].copy_from_slice(&1024i32.to_le_bytes());
        raw[8..12].copy_from_slice(&30i32.to_le_bytes());
        let temp = TempFile::new("bad_children.dat", &raw);
        assert!(reset_particle_dat(temp.path()).is_err());

        let temp = TempFile::new("truncated.dat", &raw[..100]);
        assert!(reset_particle_dat(temp.path()).is_err());
    }

    #[test]
    fn reset_light_dat_retail_pool() {
        let mut raw = vec![0u8; against_rome_data::pool::PAYLOAD_LEN];
        raw[0..4].copy_from_slice(&1i32.to_le_bytes());
        raw[4..8].copy_from_slice(&1024i32.to_le_bytes());
        raw[8..].fill(0x55);
        let temp = TempFile::new("light_retail.dat", &raw);

        let reset = reset_light_dat(temp.path()).unwrap();
        assert_eq!(reset.len(), against_rome_data::pool::PAYLOAD_LEN);

        let pool = against_rome_data::pool::LightPool::from_payload(&reset).unwrap();
        let snap = pool.snapshot_any(0).unwrap();
        assert_eq!(snap.active_raw(), 0);
        assert_eq!(snap.owner(), -1);
        assert_eq!(snap.parent(), -1);
        assert_eq!(snap.definition(), -1);

        let snap_last = pool.snapshot_any(1023).unwrap();
        assert_eq!(snap_last.active_raw(), 0);
        assert_eq!(snap_last.owner(), -1);
        assert_eq!(snap_last.parent(), -1);
        assert_eq!(snap_last.definition(), -1);
    }

    #[test]
    fn reset_biglager_dat_both_retail_and_legacy() {
        let mut raw_retail = vec![0u8; 51244];
        raw_retail[0..4].copy_from_slice(&1i32.to_le_bytes());
        raw_retail[4..8].copy_from_slice(&32i32.to_le_bytes());
        raw_retail[8..12].copy_from_slice(&800i32.to_le_bytes());
        raw_retail[12..].fill(0x77);
        let temp = TempFile::new("biglager_retail.dat", &raw_retail);
        let reset = reset_biglager_dat(temp.path()).unwrap();
        assert_eq!(reset.len(), 51244);
        for chunk in reset[12..].chunks_exact(1601) {
            assert_eq!(chunk[0], 0);
            assert!(chunk[1..].iter().all(|&b| b == 255));
        }

        const LEGACY_BIG: usize = 12 + 800 * 1601;
        let mut raw_legacy = vec![0u8; LEGACY_BIG];
        raw_legacy[0..4].copy_from_slice(&1i32.to_le_bytes());
        raw_legacy[4..8].copy_from_slice(&800i32.to_le_bytes());
        raw_legacy[8..].fill(0x88);
        let temp = TempFile::new("biglager_legacy.dat", &raw_legacy);
        let reset = reset_biglager_dat(temp.path()).unwrap();
        assert_eq!(reset.len(), LEGACY_BIG);
        for slot in 0..800 {
            let offset = 12 + slot * 1601;
            assert_eq!(reset[offset], 0);
            assert!(reset[offset + 1..offset + 1 + 1600].iter().all(|&b| b == 255));
        }
    }

    #[test]
    fn reset_lager_dat_retail_pool() {
        const LAGER_LEN: usize = 16 + 3200 * 45;
        let mut raw = vec![0u8; LAGER_LEN];
        raw[0..4].copy_from_slice(&1i32.to_le_bytes());
        raw[4..8].copy_from_slice(&3200i32.to_le_bytes());
        raw[8..12].copy_from_slice(&6i32.to_le_bytes());
        raw[12..16].copy_from_slice(&10i32.to_le_bytes());
        raw[16..].fill(0x33);
        let temp = TempFile::new("lager_retail.dat", &raw);
        let reset = reset_lager_dat(temp.path()).unwrap();
        assert_eq!(reset.len(), LAGER_LEN);
        let tail0 = 16 + 3200 * 43;
        assert_eq!(i16::from_le_bytes([reset[tail0], reset[tail0 + 1]]), -1);
        let tail_last = 16 + 3200 * 43 + 3199 * 2;
        assert_eq!(i16::from_le_bytes([reset[tail_last], reset[tail_last + 1]]), -1);
    }
    #[test]
    fn reset_pools_reject_unknown_lengths_and_counts() {
        let specs: &[(&str, usize, &[i32], fn(&Path) -> Result<Vec<u8>, String>)] = &[
            ("position", 8 + 3 * 17, &[1, 3], reset_position_dat),
            ("objects", 16 + 3 * 121, &[1, 3, 30, 30], reset_objects_dat),
            ("objdata", 8 + 3 * 123, &[1, 3], reset_objdata_dat),
            ("light", against_rome_data::pool::PAYLOAD_LEN, &[1, 1024], reset_light_dat),
            ("particle", against_rome_data::particle::PAYLOAD_LEN, &[1, 1024, 64], reset_particle_dat),
            ("lager", 144016, &[1, 3200, 6, 10], reset_lager_dat),
            ("biglager", 51244, &[1, 32, 800], reset_biglager_dat),
        ];
        for &(name, len, header, reset) in specs {
            let mut raw = vec![0; len];
            for (i, v) in header.iter().enumerate() { raw[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes()); }
            let temp = TempFile::new(name, &raw);
            assert!(reset(temp.path()).is_ok(), "{name}");
            for bytes in [raw[..len - 1].to_vec(), [raw.clone(), vec![0]].concat()] {
                fs::write(temp.path(), bytes).unwrap();
                assert!(reset(temp.path()).is_err(), "{name}");
            }
            raw[4..8].copy_from_slice(&(-1i32).to_le_bytes());
            fs::write(temp.path(), raw).unwrap();
            assert!(reset(temp.path()).is_err(), "{name}");
        }
    }

    #[test]
    fn reset_objects_rejects_unknown_name_width() {
        let mut raw = vec![0; 16 + 121];
        for (i, v) in [1i32, 1, 29, 30].iter().enumerate() {
            raw[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        let temp = TempFile::new("objects", &raw);
        assert!(reset_objects_dat(temp.path()).is_err());
    }
}
