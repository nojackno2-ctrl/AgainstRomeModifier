//! Faithful port of `NativePoolDiagnostics` and `NativePoolDiagnosticReport`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use crate::native_objdef::NativeObjectDefinition;
use crate::text_encoding::decompress_pfil_if_needed;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NativePoolIssueSeverity {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativePoolIssue {
    pub severity: NativePoolIssueSeverity,
    pub code: String,
    pub message: String,
    pub object_slot: i32,
    pub target_slot: i32,
    pub target_pool: Option<String>,
}

impl NativePoolIssue {
    pub fn new(
        severity: NativePoolIssueSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
        object_slot: i32,
        target_slot: i32,
        target_pool: Option<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            object_slot,
            target_slot,
            target_pool,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativePoolDiagnosticReport {
    pub is_healthy: bool,
    pub issues: Vec<NativePoolIssue>,
    pub active_objects: usize,
    pub active_anim_slots: usize,
    pub active_gfxtype_slots: usize,
    pub active_action_slots: usize,
    pub active_groups: usize,
    pub total_object_slots: usize,
    pub engine_uid_counter: Option<u32>,
    pub active_lights: usize,
    pub active_emitters: usize,
}

pub struct NativePoolDiagnostics;

impl NativePoolDiagnostics {
    pub const COLUMN_WIDTHS: [usize; 18] = [2, 2, 2, 2, 2, 2, 2, 4, 2, 2, 2, 2, 2, 2, 4, 2, 2, 4];
    pub const OBJ_DATA_WIDTHS: [usize; 36] = [
        9, 8, 4, 4, 2, 16, 4, 4, 2, 2, 2, 2, 4, 2, 2, 2, 4, 4, 4, 4, 2, 4, 2, 2, 2, 2, 2, 2, 2,
        2, 2, 2, 4, 4, 2, 2,
    ];

    pub fn diagnose_directory(
        data_directory: &Path,
        definitions: Option<&HashMap<i32, NativeObjectDefinition>>,
    ) -> NativePoolDiagnosticReport {
        let mut issues = Vec::new();
        let path_of = |file: &str| data_directory.join(file);

        let col_widths_sum: usize = Self::COLUMN_WIDTHS.iter().sum();
        let obj_data_widths_sum: usize = Self::OBJ_DATA_WIDTHS.iter().sum();

        // 讀取 objects.dat
        let objects = match read_payload(&path_of("objects.dat")) {
            Ok(bytes) => bytes,
            Err(err) => {
                issues.push(NativePoolIssue::new(
                    NativePoolIssueSeverity::Error,
                    "missing-objects",
                    format!("無法讀取 objects.dat: {err}"),
                    -1,
                    -1,
                    None,
                ));
                return empty_report(issues);
            }
        };

        if objects.len() < 16 || i32::from_le_bytes(objects[0..4].try_into().unwrap()) != 1 {
            issues.push(NativePoolIssue::new(
                NativePoolIssueSeverity::Error,
                "invalid-objects-header",
                "objects.dat 版本或標頭無效。",
                -1,
                -1,
                None,
            ));
            return empty_report(issues);
        }

        let object_count = i32::from_le_bytes(objects[4..8].try_into().unwrap()) as usize;
        let expected_obj_len = 16 + object_count * (79 + col_widths_sum);
        if !(1..=14000).contains(&object_count) || objects.len() != expected_obj_len {
            issues.push(NativePoolIssue::new(
                NativePoolIssueSeverity::Error,
                "objects-length-mismatch",
                format!(
                    "objects.dat 長度或槽位數不符（槽位={}，實際長度={}，預期={}）。",
                    object_count,
                    objects.len(),
                    expected_obj_len
                ),
                -1,
                -1,
                None,
            ));
            return empty_report(issues);
        }

        // 讀取 anim.dat
        let mut anim_count = 0;
        let mut active_anim = HashSet::new();
        let anim_bytes = read_payload(&path_of("anim.dat"));
        match anim_bytes {
            Ok(anim) => {
                if anim.len() >= 8 && i32::from_le_bytes(anim[0..4].try_into().unwrap()) == 1 {
                    anim_count = i32::from_le_bytes(anim[4..8].try_into().unwrap()) as usize;
                    let expected_anim_len = 8 + anim_count * (21 + 4);
                    if anim.len() == expected_anim_len {
                        for i in 0..anim_count {
                            if anim[8 + i * 21] != 0 {
                                active_anim.insert(i);
                            }
                        }
                    } else {
                        issues.push(NativePoolIssue::new(
                            NativePoolIssueSeverity::Error,
                            "anim-length-mismatch",
                            format!("anim.dat 長度不符（預期 {expected_anim_len}，實際 {}）。", anim.len()),
                            -1,
                            -1,
                            None,
                        ));
                    }
                } else {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "invalid-anim-header",
                        "anim.dat 標頭或版本無效。",
                        -1,
                        -1,
                        None,
                    ));
                }
            }
            Err(err) => {
                issues.push(NativePoolIssue::new(
                    NativePoolIssueSeverity::Warning,
                    "missing-anim",
                    format!("缺少 anim.dat: {err}"),
                    -1,
                    -1,
                    None,
                ));
            }
        }

        // 讀取 gfxtype.dat
        let mut gfx_count = 0;
        let mut active_gfx = HashSet::new();
        let gfx_bytes = read_payload(&path_of("gfxtype.dat"));
        match gfx_bytes {
            Ok(gfxtype) => {
                if gfxtype.len() >= 8 && i32::from_le_bytes(gfxtype[0..4].try_into().unwrap()) == 1 {
                    gfx_count = i32::from_le_bytes(gfxtype[4..8].try_into().unwrap()) as usize;
                    let expected_gfx_len = 8 + gfx_count * (15 + 2);
                    if gfxtype.len() == expected_gfx_len {
                        for i in 0..gfx_count {
                            if gfxtype[8 + i * 15] != 0 {
                                active_gfx.insert(i);
                            }
                        }
                    } else {
                        issues.push(NativePoolIssue::new(
                            NativePoolIssueSeverity::Error,
                            "gfxtype-length-mismatch",
                            format!("gfxtype.dat 長度不符（預期 {expected_gfx_len}，實際 {}）。", gfxtype.len()),
                            -1,
                            -1,
                            None,
                        ));
                    }
                } else {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "invalid-gfxtype-header",
                        "gfxtype.dat 標頭或版本無效。",
                        -1,
                        -1,
                        None,
                    ));
                }
            }
            Err(err) => {
                issues.push(NativePoolIssue::new(
                    NativePoolIssueSeverity::Warning,
                    "missing-gfxtype",
                    format!("缺少 gfxtype.dat: {err}"),
                    -1,
                    -1,
                    None,
                ));
            }
        }

        // 讀取 action.dat
        let mut act_count = 0;
        let mut active_action = HashSet::new();
        let action_bytes = read_payload(&path_of("action.dat"));
        match action_bytes {
            Ok(action) => {
                if action.len() >= 12 && i32::from_le_bytes(action[0..4].try_into().unwrap()) == 1 {
                    act_count = i32::from_le_bytes(action[4..8].try_into().unwrap()) as usize;
                    let words = i32::from_le_bytes(action[8..12].try_into().unwrap());
                    let expected_act_len = 12 + act_count * 25;
                    if words == 6 && action.len() == expected_act_len {
                        for i in 0..act_count {
                            if action[12 + i * 25] != 0 {
                                active_action.insert(i);
                            }
                        }
                    } else {
                        issues.push(NativePoolIssue::new(
                            NativePoolIssueSeverity::Error,
                            "action-length-mismatch",
                            format!(
                                "action.dat 長度或欄位數不符（預期長度 {expected_act_len}，實際 {}，words={words}）。",
                                action.len()
                            ),
                            -1,
                            -1,
                            None,
                        ));
                    }
                } else {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "invalid-action-header",
                        "action.dat 標頭或版本無效。",
                        -1,
                        -1,
                        None,
                    ));
                }
            }
            Err(err) => {
                issues.push(NativePoolIssue::new(
                    NativePoolIssueSeverity::Warning,
                    "missing-action",
                    format!("缺少 action.dat: {err}"),
                    -1,
                    -1,
                    None,
                ));
            }
        }

        // 讀取 hirarchy.dat
        let mut group_count = 0;
        let mut active_groups = HashSet::new();
        let mut group_members: HashMap<usize, Vec<i32>> = HashMap::new();
        let hirarchy_bytes = read_payload(&path_of("hirarchy.dat"));
        match hirarchy_bytes {
            Ok(hirarchy) => {
                if hirarchy.len() >= 12 && i32::from_le_bytes(hirarchy[0..4].try_into().unwrap()) == 1 {
                    group_count = i32::from_le_bytes(hirarchy[4..8].try_into().unwrap()) as usize;
                    let member_capacity = i32::from_le_bytes(hirarchy[8..12].try_into().unwrap());
                    let expected_hir_len = 12 + group_count * 105;
                    if member_capacity == 50 && hirarchy.len() == expected_hir_len {
                        for g in 0..group_count {
                            let g_off = 12 + g * 103;
                            if hirarchy[g_off] != 0 {
                                active_groups.insert(g);
                                let m_count = i16::from_le_bytes(hirarchy[g_off + 1..g_off + 3].try_into().unwrap()) as i32;
                                if !(0..=50).contains(&m_count) {
                                    issues.push(NativePoolIssue::new(
                                        NativePoolIssueSeverity::Error,
                                        "hirarchy-member-count-invalid",
                                        format!("hirarchy 群組 {g} 人數無效: {m_count}"),
                                        -1,
                                        g as i32,
                                        Some("hirarchy".into()),
                                    ));
                                    continue;
                                }
                                let mut members = Vec::with_capacity(m_count as usize);
                                for m in 0..m_count as usize {
                                    let m_slot = i16::from_le_bytes(
                                        hirarchy[g_off + 3 + m * 2..g_off + 5 + m * 2].try_into().unwrap(),
                                    ) as i32;
                                    members.push(m_slot);
                                }
                                group_members.insert(g, members);
                            }
                        }
                    } else {
                        issues.push(NativePoolIssue::new(
                            NativePoolIssueSeverity::Error,
                            "hirarchy-length-mismatch",
                            format!(
                                "hirarchy.dat 長度不符（預期 {expected_hir_len}，實際 {}，capacity={member_capacity}）。",
                                hirarchy.len()
                            ),
                            -1,
                            -1,
                            None,
                        ));
                    }
                } else {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "invalid-hirarchy-header",
                        "hirarchy.dat 標頭或版本無效。",
                        -1,
                        -1,
                        None,
                    ));
                }
            }
            Err(err) => {
                issues.push(NativePoolIssue::new(
                    NativePoolIssueSeverity::Warning,
                    "missing-hirarchy",
                    format!("缺少 hirarchy.dat: {err}"),
                    -1,
                    -1,
                    None,
                ));
            }
        }

        // 讀取 objdata.dat
        let mut active_obj_data = HashSet::new();
        let objdata_bytes = read_payload(&path_of("objdata.dat"));
        match objdata_bytes {
            Ok(objdata) => {
                if objdata.len() >= 8 && i32::from_le_bytes(objdata[0..4].try_into().unwrap()) == 1 {
                    let data_count = i32::from_le_bytes(objdata[4..8].try_into().unwrap()) as usize;
                    let expected_data_len = 8 + data_count * obj_data_widths_sum;
                    if data_count == object_count && objdata.len() == expected_data_len {
                        let seg0_offset = 8;
                        for i in 0..data_count {
                            if objdata[seg0_offset + i * Self::OBJ_DATA_WIDTHS[0]] != 0 {
                                active_obj_data.insert(i);
                            }
                        }
                    } else {
                        issues.push(NativePoolIssue::new(
                            NativePoolIssueSeverity::Error,
                            "objdata-length-mismatch",
                            format!("objdata.dat 長度或物件數不符（物件數={data_count}，預期={object_count}）。"),
                            -1,
                            -1,
                            None,
                        ));
                    }
                } else {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "invalid-objdata-header",
                        "objdata.dat 標頭或版本無效。",
                        -1,
                        -1,
                        None,
                    ));
                }
            }
            Err(err) => {
                issues.push(NativePoolIssue::new(
                    NativePoolIssueSeverity::Warning,
                    "missing-objdata",
                    format!("缺少 objdata.dat: {err}"),
                    -1,
                    -1,
                    None,
                ));
            }
        }

        // 讀取 position.dat
        let mut active_positions = HashSet::new();
        let position_bytes = read_payload(&path_of("position.dat"));
        match position_bytes {
            Ok(positions) => {
                if positions.len() >= 8 && i32::from_le_bytes(positions[0..4].try_into().unwrap()) == 1 {
                    let pos_count = i32::from_le_bytes(positions[4..8].try_into().unwrap()) as usize;
                    let expected_pos_len = 8 + pos_count * 17;
                    if !(1..=33000).contains(&pos_count) || positions.len() != expected_pos_len {
                        issues.push(NativePoolIssue::new(
                            NativePoolIssueSeverity::Error,
                            "position-length-mismatch",
                            format!("position.dat 長度不符（預期 {expected_pos_len}，實際 {}）。", positions.len()),
                            -1,
                            -1,
                            None,
                        ));
                    } else {
                        for i in 0..pos_count {
                            if positions[8 + i * 17] != 0 {
                                active_positions.insert(i);
                            }
                        }
                    }
                } else {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "invalid-position-header",
                        "position.dat 標頭或版本無效。",
                        -1,
                        -1,
                        None,
                    ));
                }
            }
            Err(err) => {
                issues.push(NativePoolIssue::new(
                    NativePoolIssueSeverity::Warning,
                    "missing-position",
                    format!("缺少 position.dat: {err}"),
                    -1,
                    -1,
                    None,
                ));
            }
        }

        // 讀取 engine.dat
        let mut engine_counter: Option<u32> = None;
        let engine_path = path_of("engine.dat");
        if engine_path.exists() {
            match read_payload(&engine_path) {
                Ok(engine) if engine.len() == 94 && i32::from_le_bytes(engine[0..4].try_into().unwrap()) == 1 => {
                    engine_counter = Some(u32::from_le_bytes(engine[34..38].try_into().unwrap()));
                }
                _ => {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "invalid-engine",
                        "engine.dat 版本或長度無效，version1 必須完整94bytes。",
                        -1,
                        -1,
                        None,
                    ));
                }
            }
        }

        // Light pool
        struct LightEntry {
            owner: i16,
            parent: i16,
        }
        let mut lights: Option<HashMap<i32, LightEntry>> = None;
        let light_path = path_of("light.dat");
        if light_path.exists() {
            match fs::read(&light_path) {
                Ok(raw) => {
                    match against_rome_data::pool::LightPool::from_file_bytes(&raw) {
                        Ok(pool) => {
                            let mut map = HashMap::new();
                            for snap in pool.active_snapshots() {
                                map.insert(
                                    snap.slot() as i32,
                                    LightEntry {
                                        owner: snap.owner(),
                                        parent: i16::from_le_bytes(snap.raw_fields()[52..54].try_into().unwrap()),
                                    },
                                );
                            }
                            lights = Some(map);
                        }
                        Err(e) => {
                            issues.push(NativePoolIssue::new(
                                NativePoolIssueSeverity::Error,
                                "invalid-light",
                                format!("light.dat 光源池無效：{e}"),
                                -1,
                                -1,
                                Some("light".into()),
                            ));
                        }
                    }
                }
                Err(e) => {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "invalid-light",
                        format!("light.dat 光源池無效：{e}"),
                        -1,
                        -1,
                        Some("light".into()),
                    ));
                }
            }
        }

        // Particle pool
        struct EmitterEntry {
            slot: i32,
            owner: i16,
            light_slots: Vec<i32>,
        }
        let mut emitters: Option<HashMap<i32, EmitterEntry>> = None;
        let particle_path = path_of("particle.dat");
        if particle_path.exists() {
            match fs::read(&particle_path) {
                Ok(raw) => {
                    match against_rome_data::particle::ParticlePool::from_file_bytes(&raw) {
                        Ok(pool) => {
                            let mut map = HashMap::new();
                            for slot in pool.active_slots() {
                                if let Some(snap) = pool.capture(slot) {
                                    let mut light_links = Vec::new();
                                    for ord in 0..against_rome_data::particle::CHILDREN as u32 {
                                        if let Ok(child) = snap.child(ord) {
                                            if child.active_raw() != 0 && child.light_link() != -1 {
                                                light_links.push(child.light_link() as i32);
                                            }
                                        }
                                    }
                                    map.insert(
                                        slot as i32,
                                        EmitterEntry {
                                            slot: slot as i32,
                                            owner: snap.owner(),
                                            light_slots: light_links,
                                        },
                                    );
                                }
                            }
                            emitters = Some(map);
                        }
                        Err(e) => {
                            issues.push(NativePoolIssue::new(
                                NativePoolIssueSeverity::Error,
                                "invalid-particle",
                                format!("particle.dat 粒子池無效：{e}"),
                                -1,
                                -1,
                                Some("particle".into()),
                            ));
                        }
                    }
                }
                Err(e) => {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "invalid-particle",
                        format!("particle.dat 粒子池無效：{e}"),
                        -1,
                        -1,
                        Some("particle".into()),
                    ));
                }
            }
        }

        // Object scanning
        let col1_offset = 16 + object_count * 79 + object_count * Self::COLUMN_WIDTHS[0];
        let col4_offset = 16 + object_count * 79 + object_count * Self::COLUMN_WIDTHS[..4].iter().sum::<usize>();
        let col10_offset = 16 + object_count * 79 + object_count * Self::COLUMN_WIDTHS[..10].iter().sum::<usize>();
        let col11_offset = 16 + object_count * (79 + Self::COLUMN_WIDTHS[..11].iter().sum::<usize>());
        let col12_offset = 16 + object_count * (79 + Self::COLUMN_WIDTHS[..12].iter().sum::<usize>());
        let parent_offset = 16 + object_count * 79 + object_count * Self::COLUMN_WIDTHS[..13].iter().sum::<usize>();

        let mut active_object_count = 0;
        let mut uids_seen = HashSet::new();

        for slot in 0..object_count {
            let r = 16 + slot * 79;
            if objects[r] == 0 {
                continue;
            }
            active_object_count += 1;

            // Particle target
            let particle_target = u16::from_le_bytes(objects[col11_offset + slot * 2..col11_offset + slot * 2 + 2].try_into().unwrap());
            if particle_target != 0xFFFF {
                let target = particle_target as i32;
                if particle_target as usize >= against_rome_data::particle::CAPACITY {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "particle-target-out-of-range",
                        format!("物件 {slot} 的粒子索引越界：{target}"),
                        slot as i32,
                        target,
                        Some("particle".into()),
                    ));
                } else if emitters.is_none() {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "particle-pool-unavailable",
                        format!("物件 {slot} 引用粒子 {target}，但粒子池無法讀取。"),
                        slot as i32,
                        target,
                        Some("particle".into()),
                    ));
                } else if let Some(map) = &emitters {
                    if let Some(emitter) = map.get(&target) {
                        if emitter.owner != slot as i16 {
                            issues.push(NativePoolIssue::new(
                                NativePoolIssueSeverity::Error,
                                "particle-owner-mismatch",
                                format!("物件 {slot} 的粒子 {target} owner={} 不相符", emitter.owner),
                                slot as i32,
                                target,
                                Some("particle".into()),
                            ));
                        }
                    } else {
                        issues.push(NativePoolIssue::new(
                            NativePoolIssueSeverity::Error,
                            "particle-target-inactive",
                            format!("物件 {slot} 指向未啟用粒子 {target}"),
                            slot as i32,
                            target,
                            Some("particle".into()),
                        ));
                    }
                }
            }

            // Light target
            if let Some(map) = &lights {
                let target = i16::from_le_bytes(objects[col12_offset + slot * 2..col12_offset + slot * 2 + 2].try_into().unwrap());
                if target != -1 {
                    if target < 0 || target as usize >= against_rome_data::pool::CAPACITY {
                        issues.push(NativePoolIssue::new(
                            NativePoolIssueSeverity::Error,
                            "light-target-out-of-range",
                            format!("物件 {slot} 的直接光源索引越界：{target}"),
                            slot as i32,
                            target as i32,
                            Some("light".into()),
                        ));
                    } else if let Some(light) = map.get(&(target as i32)) {
                        if light.owner != slot as i16 {
                            issues.push(NativePoolIssue::new(
                                NativePoolIssueSeverity::Error,
                                "light-owner-mismatch",
                                format!("物件 {slot} 的光源 {target} owner={} 不相符", light.owner),
                                slot as i32,
                                target as i32,
                                Some("light".into()),
                            ));
                        }
                    } else {
                        issues.push(NativePoolIssue::new(
                            NativePoolIssueSeverity::Error,
                            "light-target-inactive",
                            format!("物件 {slot} 指向未啟用光源 {target}"),
                            slot as i32,
                            target as i32,
                            Some("light".into()),
                        ));
                    }
                }
            }

            // UID
            let uid = u32::from_le_bytes(objects[r + 3..r + 7].try_into().unwrap());
            if !uids_seen.insert(uid) {
                issues.push(NativePoolIssue::new(
                    NativePoolIssueSeverity::Error,
                    "duplicate-uid",
                    format!("物件槽位 {slot} 的 UID 重複：{uid}"),
                    slot as i32,
                    -1,
                    None,
                ));
            }

            if let Some(counter) = engine_counter {
                if (uid & 0x00FFFFFF) > counter {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Warning,
                        "uid-exceeds-counter",
                        format!(
                            "物件槽位 {slot} 的 UID 低 24 位元 ({}) 超過 engine.dat 計數器 ({counter})",
                            uid & 0x00FFFFFF
                        ),
                        slot as i32,
                        -1,
                        None,
                    ));
                }
            }

            // Anim link
            let anim_target = i16::from_le_bytes(objects[r + 71..r + 73].try_into().unwrap());
            if anim_target != -1 {
                let t = anim_target as usize;
                if anim_target < 0 || t >= anim_count {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "anim-out-of-range",
                        format!("物件槽位 {slot} 的 anim link 越界: {anim_target}"),
                        slot as i32,
                        anim_target as i32,
                        Some("anim".into()),
                    ));
                } else if !active_anim.contains(&t) {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "anim-target-inactive",
                        format!("物件槽位 {slot} 的 anim link 指向未啟用的 anim 槽位: {anim_target}"),
                        slot as i32,
                        anim_target as i32,
                        Some("anim".into()),
                    ));
                }
            }

            // Gfxtype link
            let gfx_target = i16::from_le_bytes(objects[r + 73..r + 75].try_into().unwrap());
            if gfx_target != -1 {
                let t = gfx_target as usize;
                if gfx_target < 0 || t >= gfx_count {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "gfxtype-out-of-range",
                        format!("物件槽位 {slot} 的 gfxtype link 越界: {gfx_target}"),
                        slot as i32,
                        gfx_target as i32,
                        Some("gfxtype".into()),
                    ));
                } else if !active_gfx.contains(&t) {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "gfxtype-target-inactive",
                        format!("物件槽位 {slot} 的 gfxtype link 指向未啟用的 gfxtype 槽位: {gfx_target}"),
                        slot as i32,
                        gfx_target as i32,
                        Some("gfxtype".into()),
                    ));
                }
            }

            // Action link
            let act_target = i16::from_le_bytes(objects[r + 77..r + 79].try_into().unwrap());
            if act_target != -1 {
                let t = act_target as usize;
                if act_target < 0 || t >= act_count {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "action-out-of-range",
                        format!("物件槽位 {slot} 的 action link 越界: {act_target}"),
                        slot as i32,
                        act_target as i32,
                        Some("action".into()),
                    ));
                } else if !active_action.contains(&t) {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "action-target-inactive",
                        format!("物件槽位 {slot} 的 action link 指向未啟用的 action 槽位: {act_target}"),
                        slot as i32,
                        act_target as i32,
                        Some("action".into()),
                    ));
                }
            }

            // ObjData link
            let data_target = u16::from_le_bytes(objects[col1_offset + slot * 2..col1_offset + slot * 2 + 2].try_into().unwrap());
            if data_target != 0xFFFF {
                let t = data_target as usize;
                if t >= object_count {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "objdata-out-of-range",
                        format!("物件槽位 {slot} 的 objdata link 越界: {data_target}"),
                        slot as i32,
                        data_target as i32,
                        Some("objdata".into()),
                    ));
                } else if !active_obj_data.contains(&t) {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "objdata-target-inactive",
                        format!("物件槽位 {slot} 的 objdata link 指向未啟用的 objdata 槽位: {data_target}"),
                        slot as i32,
                        data_target as i32,
                        Some("objdata".into()),
                    ));
                }
            }

            // Parent link
            let parent = u16::from_le_bytes(objects[parent_offset + slot * 2..parent_offset + slot * 2 + 2].try_into().unwrap());
            if parent != 0xFFFF {
                let p = parent as usize;
                if p >= object_count {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "object-parent-out-of-range",
                        format!("附屬物件槽位 {slot} 的 parent 索引越界: {parent}"),
                        slot as i32,
                        parent as i32,
                        Some("objects".into()),
                    ));
                } else if p == slot {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "object-parent-self",
                        format!("附屬物件槽位 {slot} 的 parent 指向自身"),
                        slot as i32,
                        parent as i32,
                        Some("objects".into()),
                    ));
                } else if objects[16 + p * 79] == 0 {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "object-parent-inactive",
                        format!("附屬物件槽位 {slot} 的 parent 指向未啟用物件: {parent}"),
                        slot as i32,
                        parent as i32,
                        Some("objects".into()),
                    ));
                }
            }

            // Positions
            let p0 = u16::from_le_bytes(objects[r + 67..r + 69].try_into().unwrap()) as usize;
            let p1 = u16::from_le_bytes(objects[r + 69..r + 71].try_into().unwrap()) as usize;
            let p2 = u16::from_le_bytes(objects[col10_offset + slot * 2..col10_offset + slot * 2 + 2].try_into().unwrap());
            let obj_type = u16::from_le_bytes(objects[r + 75..r + 77].try_into().unwrap()) as i32;

            if let Some(defs) = definitions {
                if !defs.is_empty() {
                    if let Some(def) = defs.get(&obj_type) {
                        if !def.active {
                            issues.push(NativePoolIssue::new(
                                NativePoolIssueSeverity::Error,
                                "invalid-object-type",
                                format!("物件槽位 {slot} 的類型 {obj_type} 沒有有效 objdef 定義"),
                                slot as i32,
                                -1,
                                None,
                            ));
                        } else if def.requires_third_position() && p2 == 0xFFFF {
                            issues.push(NativePoolIssue::new(
                                NativePoolIssueSeverity::Error,
                                "position-2-required",
                                format!("物件槽位 {slot} 的類型 {obj_type}（afstr={}）缺少必要第三位置", def.afstr),
                                slot as i32,
                                p2 as i32,
                                Some("position".into()),
                            ));
                        } else if !def.requires_third_position() && p2 != 0xFFFF {
                            issues.push(NativePoolIssue::new(
                                NativePoolIssueSeverity::Warning,
                                "position-2-unexpected",
                                format!("物件槽位 {slot} 的類型 {obj_type}（afstr=0）帶有額外第三位置"),
                                slot as i32,
                                p2 as i32,
                                Some("position".into()),
                            ));
                        }
                    } else {
                        issues.push(NativePoolIssue::new(
                            NativePoolIssueSeverity::Error,
                            "invalid-object-type",
                            format!("物件槽位 {slot} 的類型 {obj_type} 沒有有效 objdef 定義"),
                            slot as i32,
                            -1,
                            None,
                        ));
                    }
                }
            }

            if !active_positions.contains(&p0) {
                issues.push(NativePoolIssue::new(
                    NativePoolIssueSeverity::Error,
                    "position-0-invalid",
                    format!("物件槽位 {slot} 的主要位置索引 {p0} 越界或無效"),
                    slot as i32,
                    p0 as i32,
                    Some("position".into()),
                ));
            }
            if !active_positions.contains(&p1) {
                issues.push(NativePoolIssue::new(
                    NativePoolIssueSeverity::Error,
                    "position-1-invalid",
                    format!("物件槽位 {slot} 的次要位置索引 {p1} 越界或無效"),
                    slot as i32,
                    p1 as i32,
                    Some("position".into()),
                ));
            }
            if p2 != 0xFFFF && !active_positions.contains(&(p2 as usize)) {
                issues.push(NativePoolIssue::new(
                    NativePoolIssueSeverity::Error,
                    "position-2-invalid",
                    format!("物件槽位 {slot} 的第三位置索引 {p2} 越界或無效"),
                    slot as i32,
                    p2 as i32,
                    Some("position".into()),
                ));
            }

            // Hirarchy backlink
            let g_link = i16::from_le_bytes(objects[col4_offset + slot * 2..col4_offset + slot * 2 + 2].try_into().unwrap());
            if g_link != -1 {
                let g = g_link as usize;
                if g_link < 0 || g >= group_count {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "hirarchy-backlink-out-of-range",
                        format!("物件槽位 {slot} 的群組反向連結越界: {g_link}"),
                        slot as i32,
                        g_link as i32,
                        Some("hirarchy".into()),
                    ));
                } else if !active_groups.contains(&g) {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "hirarchy-group-inactive",
                        format!("物件槽位 {slot} 指向未啟用的 hirarchy 群組: {g_link}"),
                        slot as i32,
                        g_link as i32,
                        Some("hirarchy".into()),
                    ));
                } else if !group_members.get(&g).map(|m| m.contains(&(slot as i32))).unwrap_or(false) {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "hirarchy-member-mismatch",
                        format!("物件槽位 {slot} 指向群組 {g_link}，但該群組成員清單不包含該物件"),
                        slot as i32,
                        g_link as i32,
                        Some("hirarchy".into()),
                    ));
                }
            }
        }

        // Particle emitters verification
        if let Some(map) = &emitters {
            let mut child_light_owners: HashMap<i32, i32> = HashMap::new();
            for emitter in map.values() {
                let owner = emitter.owner as i32;
                if owner != -1 {
                    if owner < 0 || owner as usize >= object_count {
                        issues.push(NativePoolIssue::new(
                            NativePoolIssueSeverity::Error,
                            "particle-owner-out-of-range",
                            format!("粒子 {} 的 owner 物件索引越界：{owner}", emitter.slot),
                            owner,
                            emitter.slot,
                            Some("particle".into()),
                        ));
                    } else if objects[16 + owner as usize * 79] == 0 {
                        issues.push(NativePoolIssue::new(
                            NativePoolIssueSeverity::Error,
                            "particle-owner-inactive",
                            format!("粒子 {} 的 owner 指向未啟用物件 {owner}", emitter.slot),
                            owner,
                            emitter.slot,
                            Some("particle".into()),
                        ));
                    } else {
                        let backlink = u16::from_le_bytes(
                            objects[col11_offset + owner as usize * 2..col11_offset + owner as usize * 2 + 2].try_into().unwrap(),
                        );
                        if backlink as i32 != emitter.slot {
                            issues.push(NativePoolIssue::new(
                                NativePoolIssueSeverity::Error,
                                "particle-backlink-mismatch",
                                format!("粒子 {} 的 owner 物件 {owner} 未回連此粒子", emitter.slot),
                                owner,
                                emitter.slot,
                                Some("particle".into()),
                            ));
                        }
                    }
                }

                let mut distinct_targets = emitter.light_slots.clone();
                distinct_targets.sort();
                distinct_targets.dedup();
                for target in distinct_targets {
                    if target < 0 || target as usize >= against_rome_data::pool::CAPACITY {
                        issues.push(NativePoolIssue::new(
                            NativePoolIssueSeverity::Error,
                            "particle-light-out-of-range",
                            format!("粒子 {} 的子光源索引越界：{target}", emitter.slot),
                            owner,
                            target,
                            Some("particle".into()),
                        ));
                    } else if lights.as_ref().map(|l| !l.contains_key(&target)).unwrap_or(true) {
                        issues.push(NativePoolIssue::new(
                            NativePoolIssueSeverity::Error,
                            "particle-light-unavailable",
                            format!("粒子 {} 指向未啟用或無法讀取的光源 {target}", emitter.slot),
                            owner,
                            target,
                            Some("particle".into()),
                        ));
                    }

                    if let Some(prev) = child_light_owners.get(&target) {
                        if *prev != emitter.slot {
                            issues.push(NativePoolIssue::new(
                                NativePoolIssueSeverity::Error,
                                "particle-light-shared",
                                format!("粒子 {prev} 與 {} 共用子光源 {target}", emitter.slot),
                                owner,
                                target,
                                Some("particle".into()),
                            ));
                        }
                    } else {
                        child_light_owners.insert(target, emitter.slot);
                    }
                }
            }
        }

        // Lights verification
        if let Some(map) = &lights {
            for (slot, light) in map {
                for (reference, field) in [(light.owner as i32, "owner"), (light.parent as i32, "parent")] {
                    if reference == -1 {
                        continue;
                    }
                    if reference < 0 || reference as usize >= object_count {
                        issues.push(NativePoolIssue::new(
                            NativePoolIssueSeverity::Error,
                            format!("light-{field}-out-of-range"),
                            format!("光源 {slot} 的 {field} 物件索引越界：{reference}"),
                            reference,
                            *slot,
                            Some("light".into()),
                        ));
                    } else if objects[16 + reference as usize * 79] == 0 {
                        issues.push(NativePoolIssue::new(
                            NativePoolIssueSeverity::Error,
                            format!("light-{field}-inactive"),
                            format!("光源 {slot} 的 {field} 指向未啟用物件 {reference}"),
                            reference,
                            *slot,
                            Some("light".into()),
                        ));
                    } else if field == "owner" {
                        let backlink = i16::from_le_bytes(
                            objects[col12_offset + reference as usize * 2..col12_offset + reference as usize * 2 + 2].try_into().unwrap(),
                        );
                        if backlink as i32 != *slot {
                            issues.push(NativePoolIssue::new(
                                NativePoolIssueSeverity::Error,
                                "light-backlink-mismatch",
                                format!("光源 {slot} 的 owner 物件 {reference} 未回連此光源"),
                                reference,
                                *slot,
                                Some("light".into()),
                            ));
                        }
                    }
                }
            }
        }

        // Hirarchy members verification
        for (g, members) in &group_members {
            let mut unique = members.clone();
            unique.sort();
            unique.dedup();
            if unique.len() != members.len() {
                issues.push(NativePoolIssue::new(
                    NativePoolIssueSeverity::Warning,
                    "duplicate-hirarchy-members",
                    format!("hirarchy 群組 {g} 包含重複成員槽位"),
                    -1,
                    *g as i32,
                    Some("hirarchy".into()),
                ));
            }

            for &m_slot in members {
                if m_slot < 0 || m_slot as usize >= object_count {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "member-out-of-range",
                        format!("hirarchy 群組 {g} 成員索引越界: {m_slot}"),
                        m_slot,
                        *g as i32,
                        Some("hirarchy".into()),
                    ));
                } else if objects[16 + m_slot as usize * 79] == 0 {
                    issues.push(NativePoolIssue::new(
                        NativePoolIssueSeverity::Error,
                        "member-inactive",
                        format!("hirarchy 群組 {g} 成員指向未啟用的物件: {m_slot}"),
                        m_slot,
                        *g as i32,
                        Some("hirarchy".into()),
                    ));
                } else {
                    let actual_group = i16::from_le_bytes(
                        objects[col4_offset + m_slot as usize * 2..col4_offset + m_slot as usize * 2 + 2].try_into().unwrap(),
                    );
                    if actual_group as usize != *g {
                        issues.push(NativePoolIssue::new(
                            NativePoolIssueSeverity::Error,
                            "member-backlink-mismatch",
                            format!(
                                "hirarchy 群組 {g} 成員 {m_slot} 的反向連結為 {actual_group}，不相符"
                            ),
                            m_slot,
                            *g as i32,
                            Some("hirarchy".into()),
                        ));
                    }
                }
            }
        }

        let is_healthy = !issues.iter().any(|i| i.severity == NativePoolIssueSeverity::Error);
        NativePoolDiagnosticReport {
            is_healthy,
            issues,
            active_objects: active_object_count,
            active_anim_slots: active_anim.len(),
            active_gfxtype_slots: active_gfx.len(),
            active_action_slots: active_action.len(),
            active_groups: active_groups.len(),
            total_object_slots: object_count,
            engine_uid_counter: engine_counter,
            active_lights: lights.as_ref().map(|l| l.len()).unwrap_or(0),
            active_emitters: emitters.as_ref().map(|e| e.len()).unwrap_or(0),
        }
    }
}

fn empty_report(issues: Vec<NativePoolIssue>) -> NativePoolDiagnosticReport {
    let is_healthy = !issues.iter().any(|i| i.severity == NativePoolIssueSeverity::Error);
    NativePoolDiagnosticReport {
        is_healthy,
        issues,
        active_objects: 0,
        active_anim_slots: 0,
        active_gfxtype_slots: 0,
        active_action_slots: 0,
        active_groups: 0,
        total_object_slots: 0,
        engine_uid_counter: None,
        active_lights: 0,
        active_emitters: 0,
    }
}

fn read_payload(path: &Path) -> Result<Vec<u8>, String> {
    if !path.exists() {
        return Err("檔案不存在".into());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    decompress_pfil_if_needed(&bytes)
}
