//! Pure byte transformations and orchestration for Endless AI patches.
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatchState {
    Original,
    Ultimate,
    Legacy,
    Unknown,
}

fn error(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub fn read_word(data: &[u8], offset: usize) -> Option<i32> {
    Some(i32::from_le_bytes(
        data.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}

pub fn write_word(data: &mut [u8], offset: usize, value: i32) -> io::Result<bool> {
    let old = read_word(data, offset).ok_or_else(|| error("BCI word outside buffer"))?;
    if old == value {
        return Ok(false);
    }
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    Ok(true)
}

pub fn words_bytes(words: &[i32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

pub fn find_sites(data: &[u8], signature: &[Option<i32>]) -> Vec<usize> {
    if signature.is_empty() || signature.len() > data.len() / 4 {
        return vec![];
    }
    (0..=data.len() - signature.len() * 4)
        .step_by(4)
        .filter(|&offset| {
            signature
                .iter()
                .enumerate()
                .all(|(i, w)| w.is_none() || read_word(data, offset + i * 4) == *w)
        })
        .collect()
}

pub fn exact_sites(data: &[u8], words: &[i32]) -> Vec<usize> {
    find_sites(data, &words.iter().copied().map(Some).collect::<Vec<_>>())
}

fn has_words(data: &[u8], offset: usize, words: &[i32]) -> bool {
    words
        .iter()
        .enumerate()
        .all(|(i, w)| read_word(data, offset + i * 4) == Some(*w))
}

fn aggregate(states: impl IntoIterator<Item = PatchState>) -> PatchState {
    let (mut original, mut ultimate) = (true, true);
    for state in states {
        match state {
            PatchState::Unknown => return PatchState::Unknown,
            PatchState::Original => ultimate = false,
            PatchState::Ultimate => original = false,
            PatchState::Legacy => {
                original = false;
                ultimate = false;
            }
        }
    }
    if original {
        PatchState::Original
    } else if ultimate {
        PatchState::Ultimate
    } else {
        PatchState::Legacy
    }
}

pub trait EndlessPatch: Send + Sync {
    fn id(&self) -> &str;
    fn target_pattern(&self) -> &str;
    fn detect(&self, data: &[u8]) -> PatchState;
    fn apply(&self, data: &mut Vec<u8>, enabled: bool) -> io::Result<bool>;
}

/// C# CustomMapManifest.HasStandaloneLevel: an editable (marker-backed ENDL_005..999) map whose marker says StandaloneLevel.
pub fn has_standalone_level(dir: &Path) -> bool {
    let Some(name) = dir.file_name().map(|n| n.to_string_lossy().into_owned()) else {
        return false;
    };
    let editable = name.is_ascii()
        && name.len() == 8
        && name[..5].eq_ignore_ascii_case("ENDL_")
        && name[5..].bytes().all(|c| c.is_ascii_digit())
        && name[5..].parse::<u32>().is_ok_and(|n| n >= 5)
        && dir
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|p| p.to_string_lossy().eq_ignore_ascii_case("MAPS"));
    if !editable {
        return false;
    }
    let Ok(text) = fs::read_to_string(dir.join(".arm_custom_map")) else {
        return false;
    };
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| {
            v.get("StandaloneLevel")
                .and_then(serde_json::Value::as_bool)
        })
        .unwrap_or(false)
}

pub const LEVEL_PATTERN: &str = "MAPS/ENDL_*/SCRIPT/ak_level.bci";

#[derive(Clone, Debug)]
pub struct BciLiteralPatch {
    pub id: String,
    pub target_pattern: String,
    pub signature: Vec<Option<i32>>,
    pub value_word_indices: Vec<usize>,
    pub original_values: Vec<i32>,
    pub ultimate_values: Vec<i32>,
    pub expected_site_count: usize,
}

impl BciLiteralPatch {
    fn valid(&self) -> bool {
        !self.signature.is_empty()
            && self.value_word_indices.len() == self.original_values.len()
            && self.original_values.len() == self.ultimate_values.len()
            && self
                .value_word_indices
                .iter()
                .all(|&i| i < self.signature.len())
    }
}

impl EndlessPatch for BciLiteralPatch {
    fn id(&self) -> &str {
        &self.id
    }
    fn target_pattern(&self) -> &str {
        &self.target_pattern
    }
    fn detect(&self, data: &[u8]) -> PatchState {
        if !self.valid() {
            return PatchState::Unknown;
        }
        let sites = find_sites(data, &self.signature);
        if sites.len() != self.expected_site_count {
            return PatchState::Unknown;
        }
        aggregate(sites.iter().flat_map(|site| {
            self.value_word_indices
                .iter()
                .enumerate()
                .map(move |(j, i)| {
                    let v = read_word(data, site + i * 4);
                    if v == Some(self.ultimate_values[j]) {
                        PatchState::Ultimate
                    } else if v == Some(self.original_values[j]) {
                        PatchState::Original
                    } else {
                        PatchState::Legacy
                    }
                })
        }))
    }
    fn apply(&self, data: &mut Vec<u8>, enabled: bool) -> io::Result<bool> {
        if self.detect(data) == PatchState::Unknown {
            return if enabled {
                Err(error("literal patch signature mismatch"))
            } else {
                Ok(false)
            };
        }
        let mut changed = false;
        for site in find_sites(data, &self.signature) {
            for (j, i) in self.value_word_indices.iter().enumerate() {
                changed |= write_word(
                    data,
                    site + i * 4,
                    if enabled {
                        self.ultimate_values[j]
                    } else {
                        self.original_values[j]
                    },
                )?;
            }
        }
        Ok(changed)
    }
}

#[derive(Default)]
pub struct P4RetreatDeadlinePatch;
impl P4RetreatDeadlinePatch {
    fn sites(data: &[u8]) -> Vec<usize> {
        exact_sites(data, &[81, 61, 90, -3, 128, 83, 86, 66])
            .into_iter()
            .filter(|&s| {
                read_word(data, s + 52).is_some()
                    && has_words(data, s + 36, &[32, 44, 164])
                    && !has_words(data, s + 48, &[66, 34])
            })
            .map(|s| s + 32)
            .collect()
    }
}
impl EndlessPatch for P4RetreatDeadlinePatch {
    fn id(&self) -> &str {
        "P4"
    }
    fn target_pattern(&self) -> &str {
        LEVEL_PATTERN
    }
    fn detect(&self, data: &[u8]) -> PatchState {
        let sites = Self::sites(data);
        if sites.len() != 6 {
            return PatchState::Unknown;
        }
        let mut original = 0;
        let mut accelerated = 0;
        let mut protected = 0;
        for (i, s) in sites.iter().enumerate() {
            match read_word(data, *s) {
                Some(600000) => {
                    original += 1;
                    if i == 0 || i == 4 {
                        protected += 1;
                    }
                }
                Some(60000) => {
                    if i != 0 && i != 4 {
                        accelerated += 1;
                    }
                }
                _ => return PatchState::Unknown,
            }
        }
        if original == 6 {
            PatchState::Original
        } else if accelerated == 4 && protected == 2 {
            PatchState::Ultimate
        } else {
            PatchState::Legacy
        }
    }
    fn apply(&self, data: &mut Vec<u8>, enabled: bool) -> io::Result<bool> {
        let sites = Self::sites(data);
        if sites.len() != 6 {
            return Err(error("P4 signature count mismatch"));
        }
        let mut changed = false;
        for (i, s) in sites.iter().enumerate() {
            changed |= write_word(
                data,
                *s,
                if !enabled || i == 0 || i == 4 {
                    600000
                } else {
                    60000
                },
            )?;
        }
        Ok(changed)
    }
}

pub const ORIGINAL_SCHEDULER: &[i32] = &[
    66, 0, 128, 72, 86, 102, 117, 48, 128, 83, 86, 81, 16, 96, 101, 117, 12, 71, 66, 1, 117, 172,
    66, 0, 91, 19, 81, 71, 102, 117, 52, 66, 120000, 66, 60000, 128, 16, 73, -2, 86, 91, 19, 112,
    44, 66, 240000, 66, 120000, 128, 16, 73, -2, 86, 91, 19, 128, 83, 86, 90, 19, 32, 82, 16, 120,
    -9748,
];
pub fn ultimate_scheduler() -> Vec<i32> {
    let mut w = vec![
        66, 0, 128, 72, 86, 102, 117, 228, 128, 83, 86, 81, 16, 96, 101, 118, 56, 81, 16, 128, 83,
        86, 66, 60000, 32, 96, 100, 118, 8, 112, 136, 128, 83, 86, 66, 30000, 32, 82, 16,
    ];
    while w.len() < 63 {
        w.extend([112, 0]);
    }
    w.extend([120, -9748]);
    w
}
const LOOP_RANGES: [(i32, i32); 4] = [
    (960000, 480000),
    (960000, 480000),
    (360000, 240000),
    (120000, 60000),
];

#[derive(Default)]
pub struct P6LoopDelayPatch;
impl P6LoopDelayPatch {
    fn scheduler(data: &[u8]) -> Option<(usize, bool)> {
        let mut p: Vec<_> = ORIGINAL_SCHEDULER.iter().copied().map(Some).collect();
        for i in [32, 34, 45, 47] {
            p[i] = None;
        }
        let old = find_sites(data, &p);
        let repaired = exact_sites(data, &ultimate_scheduler());
        if old.len() + repaired.len() != 1 {
            None
        } else if repaired.len() == 1 {
            Some((repaired[0], true))
        } else {
            Some((old[0], false))
        }
    }
    fn loops(data: &[u8], scheduler: usize) -> Vec<usize> {
        let mut sites = vec![];
        for s in find_sites(data, &[Some(66), None, Some(66), None, Some(128), Some(16)]) {
            if sites.len() == 4 {
                break;
            }
            if s >= scheduler && s < scheduler + ORIGINAL_SCHEDULER.len() * 4 {
                continue;
            }
            let pair = (
                read_word(data, s + 4).unwrap(),
                read_word(data, s + 12).unwrap(),
            );
            if pair == LOOP_RANGES[sites.len()] || pair == (30000, 30000) || pair == (2000, 1000) {
                sites.push(s);
            }
        }
        sites
    }
}
impl EndlessPatch for P6LoopDelayPatch {
    fn id(&self) -> &str {
        "P6"
    }
    fn target_pattern(&self) -> &str {
        LEVEL_PATTERN
    }
    fn detect(&self, data: &[u8]) -> PatchState {
        let Some((s, repaired)) = Self::scheduler(data) else {
            return PatchState::Unknown;
        };
        let sites = Self::loops(data, s);
        if sites.len() != 4 {
            return PatchState::Unknown;
        }
        let loops = aggregate(sites.iter().enumerate().map(|(i, &s)| {
            let pair = (
                read_word(data, s + 4).unwrap(),
                read_word(data, s + 12).unwrap(),
            );
            if pair == LOOP_RANGES[i] {
                PatchState::Original
            } else if pair == (30000, 30000) {
                PatchState::Ultimate
            } else {
                PatchState::Legacy
            }
        }));
        let scheduler = if repaired {
            PatchState::Ultimate
        } else if [(32, 120000), (34, 60000), (45, 240000), (47, 120000)]
            .iter()
            .all(|(i, w)| read_word(data, s + i * 4) == Some(*w))
        {
            PatchState::Original
        } else {
            PatchState::Legacy
        };
        aggregate([loops, scheduler])
    }
    fn apply(&self, data: &mut Vec<u8>, enabled: bool) -> io::Result<bool> {
        let state = self.detect(data);
        if state == PatchState::Unknown {
            return if enabled {
                Err(error("P6 unsupported scheduler"))
            } else {
                Ok(false)
            };
        }
        if (enabled && state == PatchState::Ultimate) || (!enabled && state == PatchState::Original)
        {
            return Ok(false);
        }
        let (s, _) = Self::scheduler(data).unwrap();
        let sites = Self::loops(data, s);
        let mut changed = false;
        for (i, site) in sites.iter().enumerate() {
            let pair = if enabled {
                (30000, 30000)
            } else {
                LOOP_RANGES[i]
            };
            changed |= write_word(data, site + 4, pair.0)?;
            changed |= write_word(data, site + 12, pair.1)?;
        }
        let target = if enabled {
            ultimate_scheduler()
        } else {
            ORIGINAL_SCHEDULER.to_vec()
        };
        for (i, w) in target.iter().enumerate() {
            changed |= write_word(data, s + i * 4, *w)?;
        }
        Ok(changed)
    }
}

// Custom patches: 7, 8, 13, 14, 15
#[derive(Clone, Copy)]
pub struct CustomPatch(pub u8);

const GATE: &[i32] = &[66, 0, 66, 0, 66, 0, 66, 0, 66, 0, 90, 6, 102, 117, 32];
const GATE_LEGACY: &[&[i32]] = &[
    &[112, 272, 66, 0, 66, 0, 66, 0, 66, 0, 90, 6, 102, 117, 32],
    &[
        90, 11, 117, 252, 90, 11, 117, 236, 90, 11, 117, 220, 112, 224, 32,
    ],
    &[
        90, 11, 117, 252, 90, 11, 117, 236, 90, 11, 117, 220, 112, 232, 32,
    ],
];

fn p7_site(data: &[u8]) -> Option<usize> {
    find_sites(
        data,
        &[
            Some(66),
            None,
            Some(91),
            Some(2),
            Some(66),
            None,
            Some(91),
            Some(2),
            Some(66),
            Some(0),
            Some(91),
            Some(3),
            Some(90),
            Some(0),
            Some(91),
            Some(3),
        ],
    )
    .into_iter()
    .next()
    .filter(|s| read_word(data, s + 252).is_some())
}

fn p8_site(data: &[u8]) -> Option<usize> {
    let mut p = vec![
        Some(90),
        Some(0),
        Some(66),
        None,
        Some(96),
        Some(98),
        Some(91),
        Some(11),
    ];
    p.resize(23, None);
    find_sites(data, &p).into_iter().next()
}

fn p15_sites(data: &[u8]) -> Vec<usize> {
    find_sites(
        data,
        &[
            Some(71),
            Some(66),
            Some(0),
            Some(117),
            Some(16),
            Some(66),
            None,
            Some(91),
            None,
            Some(112),
            None,
            Some(66),
            Some(256),
            Some(90),
            Some(14),
            Some(96),
            Some(118),
        ],
    )
    .into_iter()
    .filter(|s| {
        matches!(read_word(data, s + 32), Some(6 | 7))
            && matches!(read_word(data, s + 24), Some(256 | 257))
    })
    .map(|s| s + 24)
    .collect()
}

fn p14_sites(data: &[u8]) -> Vec<(usize, i32, i32)> {
    let a = find_sites(
        data,
        &[
            Some(128),
            Some(43),
            Some(73),
            Some(-2),
            Some(86),
            Some(66),
            None,
            Some(96),
            Some(99),
            Some(117),
            Some(476),
        ],
    );
    let b = find_sites(
        data,
        &[
            Some(128),
            Some(69),
            Some(73),
            Some(-2),
            Some(86),
            None,
            Some(56),
            Some(66),
            Some(1),
            Some(82),
            Some(46),
        ],
    );
    let mut result = vec![];
    if let Some(s) = a.first() {
        result.push((s + 24, 0, 20));
    }
    if let Some(s) = b.first() {
        result.push((s + 20, 117, 112));
    }
    result
}

fn sdl_lines(data: &[u8]) -> (Vec<String>, Vec<usize>) {
    let text: String = data.iter().map(|b| char::from(*b)).collect();
    let lines: Vec<_> = text.split('\n').map(str::to_string).collect();
    let (mut active, mut sites) = (false, vec![]);
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if t.starts_with('[') {
            active = false;
        } else if t.starts_with("namedef") && t.contains("_Haupt") {
            active = true;
        } else if active && t.starts_with("resv") {
            active = false;
            sites.push(i);
        }
    }
    (lines, sites)
}

impl EndlessPatch for CustomPatch {
    fn id(&self) -> &str {
        match self.0 {
            7 => "P7",
            8 => "P8",
            13 => "P13",
            14 => "P14",
            15 => "P15",
            _ => "Unknown",
        }
    }
    fn target_pattern(&self) -> &str {
        match self.0 {
            13 => "MAPS/ENDL_*/Endlos_*_Siedlung*.sdl",
            14 => "SYSTEM/CLAK/SCRIPT/ak_npc.bci|SYSTEM/CLAK/SCRIPT/ak_produktion.bci",
            _ => LEVEL_PATTERN,
        }
    }
    fn detect(&self, data: &[u8]) -> PatchState {
        match self.0 {
            7 => {
                let Some(s) = p7_site(data) else {
                    return PatchState::Unknown;
                };
                let vals: Vec<_> = [4, 20, 96, 148, 200, 252]
                    .iter()
                    .map(|o| read_word(data, s + o).unwrap())
                    .collect();
                if vals == [0, 0, 80, 60, 40, 20] {
                    PatchState::Original
                } else if vals == [101; 6] {
                    PatchState::Ultimate
                } else {
                    PatchState::Legacy
                }
            }
            8 => {
                let Some(s) = p8_site(data) else {
                    return PatchState::Unknown;
                };
                let value = read_word(data, s + 12).unwrap();
                if has_words(data, s + 32, GATE) {
                    match value {
                        4 => PatchState::Original,
                        70 => PatchState::Ultimate,
                        8 | 30 | 40 => PatchState::Legacy,
                        _ => PatchState::Unknown,
                    }
                } else if [4, 8, 30, 40, 70].contains(&value)
                    && GATE_LEGACY.iter().any(|g| has_words(data, s + 32, g))
                {
                    PatchState::Legacy
                } else {
                    PatchState::Unknown
                }
            }
            13 => {
                let (lines, sites) = sdl_lines(data);
                if sites.is_empty() {
                    return PatchState::Unknown;
                }
                let mut values = vec![];
                for i in sites {
                    let Some((_, value)) = lines[i].split_once('=') else {
                        return PatchState::Unknown;
                    };
                    values.push(value.trim().to_string());
                }
                if values.iter().any(|v| v != &values[0]) {
                    PatchState::Unknown
                } else {
                    match values[0].as_str() {
                        "0,0,0,0,0,0" => PatchState::Original,
                        "614,300,372,250,460,288" => PatchState::Ultimate,
                        _ => PatchState::Unknown,
                    }
                }
            }
            14 => {
                let sites = p14_sites(data);
                if let Some((s, old, new)) = sites.first() {
                    match read_word(data, *s) {
                        Some(v) if v == *old => PatchState::Original,
                        Some(v) if v == *new => PatchState::Legacy,
                        _ => PatchState::Unknown,
                    }
                } else {
                    PatchState::Unknown
                }
            }
            15 => {
                let sites = p15_sites(data);
                if sites.len() != 2 {
                    PatchState::Unknown
                } else if sites.iter().all(|s| read_word(data, *s) == Some(256)) {
                    PatchState::Original
                } else {
                    PatchState::Legacy
                }
            }
            _ => PatchState::Unknown,
        }
    }
    fn apply(&self, data: &mut Vec<u8>, enabled: bool) -> io::Result<bool> {
        let mut changed = false;
        match self.0 {
            7 => {
                let Some(s) = p7_site(data) else {
                    return if enabled {
                        Err(error("P7 signature not found"))
                    } else {
                        Ok(false)
                    };
                };
                for (o, v) in [4, 20, 96, 148, 200, 252].into_iter().zip(if enabled {
                    [101; 6]
                } else {
                    [0, 0, 80, 60, 40, 20]
                }) {
                    changed |= write_word(data, s + o, v)?;
                }
            }
            8 => {
                if self.detect(data) == PatchState::Unknown {
                    return if enabled {
                        Err(error("P8 unknown control flow"))
                    } else {
                        Ok(false)
                    };
                }
                let s = p8_site(data).unwrap();
                changed |= write_word(data, s + 12, if enabled { 70 } else { 4 })?;
                for (i, w) in GATE.iter().enumerate() {
                    changed |= write_word(data, s + 32 + i * 4, *w)?;
                }
            }
            13 => {
                let (mut lines, sites) = sdl_lines(data);
                if sites.is_empty() {
                    return Err(error("P13 main building resv not found"));
                }
                for i in sites {
                    let Some((left, value)) = lines[i].split_once('=') else {
                        return Err(error("invalid SDL resv line"));
                    };
                    if !["0,0,0,0,0,0", "614,300,372,250,460,288"].contains(&value.trim()) {
                        return Err(error("unknown SDL resv value"));
                    }
                    let target = if enabled {
                        "614,300,372,250,460,288"
                    } else {
                        "0,0,0,0,0,0"
                    };
                    if value.trim() != target {
                        lines[i] = format!(
                            "{left}={target}{}",
                            if lines[i].ends_with('\r') { "\r" } else { "" }
                        );
                        changed = true;
                    }
                }
                if changed {
                    *data = lines.join("\n").chars().map(|c| c as u8).collect();
                }
            }
            14 => {
                for (s, old, new) in p14_sites(data) {
                    if ![Some(old), Some(new)].contains(&read_word(data, s)) {
                        return Err(error("P14 unsupported bytes"));
                    }
                    changed |= write_word(data, s, old)?;
                }
            }
            15 => {
                let sites = p15_sites(data);
                if sites.len() != 2 {
                    return Err(error("P15 signature count mismatch"));
                }
                for s in sites {
                    changed |= write_word(data, s, 256)?;
                }
            }
            _ => return Err(error("unknown custom patch")),
        }
        Ok(changed)
    }
}

// ==========================================
// P9: 撤退配額歸零
// ==========================================
#[derive(Default)]
pub struct P9RetreatQuotaPatch;

const RETREAT_QUOTA_SIG: &[Option<i32>] = &[
    Some(81),
    Some(56),
    Some(90),
    Some(-3),
    None,
    None,
    Some(164),
];
const DONATION_FILTER_SIG: &[Option<i32>] = &[
    Some(128),
    Some(214),
    Some(73),
    Some(-2),
    Some(86),
    Some(66),
    Some(1),
    Some(96),
    Some(102),
    None,
    None,
];
const DONATION_ACTION_SIG: &[Option<i32>] = &[
    Some(90),
    Some(42),
    Some(117),
    Some(56),
    Some(90),
    Some(41),
    Some(90),
    Some(4),
    Some(90),
    Some(3),
    None,
    None,
    None,
    None,
    Some(86),
    Some(71),
    Some(112),
    Some(116),
];
const RELEASE_HELPER_WORDS: &[i32] = &[
    74, 94, 73, 3, 66, 0, 90, -4, 90, -3, 128, 86, 73, -3, 86, 71, 90, -5, 90, -4, 90, -3, 128,
    141, 73, -3, 86, 71, 66, 1, 87, 112, 0, 95, 75, 121,
];

fn bci_code_end(data: &[u8]) -> Option<usize> {
    if data.len() < 44 || &data[..4] != b"BCI0" {
        return None;
    }
    let code_size = read_word(data, 8)?;
    if code_size <= 0 {
        return None;
    }
    let end = 0x24 + code_size as usize;
    if end + 8 <= data.len() && &data[end..end + 8] == b"SYMBCONS" {
        Some(end)
    } else {
        None
    }
}

impl EndlessPatch for P9RetreatQuotaPatch {
    fn id(&self) -> &str {
        "P9"
    }
    fn target_pattern(&self) -> &str {
        LEVEL_PATTERN
    }
    fn detect(&self, data: &[u8]) -> PatchState {
        let sites = find_sites(data, RETREAT_QUOTA_SIG);
        if sites.len() != 10 {
            return PatchState::Unknown;
        }

        let s8 = sites[8];
        let op8 = read_word(data, s8 + 16);
        let val8 = read_word(data, s8 + 20);
        let s8_vanilla = op8 == Some(90) && val8 == Some(6);
        let s8_legacy = op8 == Some(66) && val8 == Some(0);
        if !s8_vanilla && !s8_legacy {
            return PatchState::Unknown;
        }

        let s9 = sites[9];
        let op9 = read_word(data, s9 + 16);
        let val9 = read_word(data, s9 + 20);
        let s9_ultimate = op9 == Some(66) && val9 == Some(0);
        let s9_original = op9 == Some(90) && val9 == Some(15);
        if !s9_ultimate && !s9_original {
            return PatchState::Unknown;
        }

        let filter_sites = find_sites(data, DONATION_FILTER_SIG);
        if filter_sites.len() != 1 {
            return PatchState::Unknown;
        }
        let f = filter_sites[0];
        let jz_op = read_word(data, f + 36);
        let jz_val = read_word(data, f + 40);
        let f_orig = jz_op == Some(117) && jz_val == Some(92);
        let f_ult = jz_op == Some(117) && jz_val == Some(0);
        let f_legacy = jz_op == Some(118) && jz_val == Some(92);
        if !f_orig && !f_ult && !f_legacy {
            return PatchState::Unknown;
        }

        let act_sites = find_sites(data, DONATION_ACTION_SIG);
        if act_sites.len() != 1 {
            return PatchState::Unknown;
        }
        let a = act_sites[0];
        let call_op = read_word(data, a + 40);
        let call_val = read_word(data, a + 44);

        let code_end = bci_code_end(data).unwrap_or(data.len());
        let helper_len = RELEASE_HELPER_WORDS.len() * 4;
        let has_helper =
            code_end >= helper_len && has_words(data, code_end - helper_len, RELEASE_HELPER_WORDS);

        let direct_remark = call_op == Some(128) && call_val == Some(141);
        let release_and_remark = has_helper && call_op == Some(120);

        if s8_vanilla && s9_ultimate && f_ult && release_and_remark {
            PatchState::Ultimate
        } else if s8_vanilla && s9_original && f_orig && direct_remark && !has_helper {
            PatchState::Original
        } else {
            PatchState::Legacy
        }
    }

    fn apply(&self, data: &mut Vec<u8>, enabled: bool) -> io::Result<bool> {
        if self.detect(data) == PatchState::Unknown {
            return if enabled {
                Err(error("P9 unknown state"))
            } else {
                Ok(false)
            };
        }
        let sites = find_sites(data, RETREAT_QUOTA_SIG);
        if sites.len() != 10 {
            return Err(error("P9 signature count mismatch"));
        }

        let mut changed = false;
        // Site 8: always keep vanilla [90, 6]
        changed |= write_word(data, sites[8] + 16, 90)?;
        changed |= write_word(data, sites[8] + 20, 6)?;

        // Site 9: 66, 0 (enabled) or 90, 15 (disabled)
        changed |= write_word(data, sites[9] + 16, if enabled { 66 } else { 90 })?;
        changed |= write_word(data, sites[9] + 20, if enabled { 0 } else { 15 })?;

        // Filter
        let filter_sites = find_sites(data, DONATION_FILTER_SIG);
        if filter_sites.len() == 1 {
            let f = filter_sites[0];
            changed |= write_word(data, f + 36, 117)?;
            changed |= write_word(data, f + 40, if enabled { 0 } else { 92 })?;
        }

        // Action & helper
        let act_sites = find_sites(data, DONATION_ACTION_SIG);
        if act_sites.len() != 1 {
            return Err(error("P9 action signature not found"));
        }
        let a = act_sites[0];
        let call_offset = a + 40;

        let code_end = bci_code_end(data).unwrap_or(data.len());
        let helper_bytes = words_bytes(RELEASE_HELPER_WORDS);
        let has_helper = code_end >= helper_bytes.len()
            && has_words(data, code_end - helper_bytes.len(), RELEASE_HELPER_WORDS);

        if enabled {
            let helper_offset = if has_helper {
                code_end - helper_bytes.len()
            } else {
                let insert_at = code_end;
                data.splice(insert_at..insert_at, helper_bytes);
                if let Some(mut cs) = read_word(data, 8) {
                    cs += RELEASE_HELPER_WORDS.len() as i32 * 4;
                    let _ = write_word(data, 8, cs);
                }
                changed = true;
                insert_at
            };
            let operand = helper_offset as i32 - call_offset as i32 - 8;
            changed |= write_word(data, call_offset, 120)?;
            changed |= write_word(data, call_offset + 4, operand)?;
            changed |= write_word(data, call_offset + 8, 112)?;
            changed |= write_word(data, call_offset + 12, 4)?;
        } else if has_helper {
            let remove_at = code_end - helper_bytes.len();
            data.drain(remove_at..code_end);
            if let Some(mut cs) = read_word(data, 8) {
                cs -= RELEASE_HELPER_WORDS.len() as i32 * 4;
                let _ = write_word(data, 8, cs);
            }
            write_word(data, call_offset, 128)?;
            write_word(data, call_offset + 4, 141)?;
            write_word(data, call_offset + 8, 73)?;
            write_word(data, call_offset + 12, -3)?;
            changed = true;
        }

        Ok(changed)
    }
}

// ==========================================
// P20: Village Garrison Quota Multiplier
// ==========================================
pub struct P20VillageGarrisonQuotaPatch {
    pub multiplier: i32,
}

impl Default for P20VillageGarrisonQuotaPatch {
    fn default() -> Self {
        Self { multiplier: 3 }
    }
}

const P20_CALL_SIGNATURES: [&[Option<i32>]; 4] = [
    &[
        Some(66),
        Some(1),
        Some(81),
        Some(8),
        None,
        None,
        Some(73),
        Some(-2),
        Some(86),
        Some(91),
        Some(59),
    ],
    &[
        Some(66),
        Some(2),
        Some(81),
        Some(10),
        None,
        None,
        Some(73),
        Some(-2),
        Some(86),
        Some(91),
        Some(61),
    ],
    &[
        Some(66),
        Some(3),
        Some(81),
        Some(12),
        None,
        None,
        Some(73),
        Some(-2),
        Some(86),
        Some(91),
        Some(63),
    ],
    &[
        Some(66),
        Some(4),
        Some(81),
        Some(14),
        None,
        None,
        Some(73),
        Some(-2),
        Some(86),
        Some(91),
        Some(65),
    ],
];

const P20_HELPER_TEMPLATE: &[i32] = &[
    74, 94, 73, 2, 90, -4, 90, -3, 128, 151, 73, -2, 86, 66, 3, 34, 87, 112, 0, 95, 75, 121,
];

impl P20VillageGarrisonQuotaPatch {
    pub fn try_read_installed_multiplier(data: &[u8]) -> Option<i32> {
        let code_end = bci_code_end(data).unwrap_or(data.len());
        let hlen = P20_HELPER_TEMPLATE.len() * 4;
        if code_end < hlen {
            return None;
        }
        let h_offset = code_end - hlen;
        let mut match_template = true;
        for (i, &w) in P20_HELPER_TEMPLATE.iter().enumerate() {
            if i == 14 {
                continue;
            }
            if read_word(data, h_offset + i * 4) != Some(w) {
                match_template = false;
                break;
            }
        }
        if match_template {
            read_word(data, h_offset + 14 * 4)
        } else {
            None
        }
    }
}

impl EndlessPatch for P20VillageGarrisonQuotaPatch {
    fn id(&self) -> &str {
        "P20"
    }
    fn target_pattern(&self) -> &str {
        "SYSTEM/CLAK/SCRIPT/Dorfverteidigung.bci"
    }
    fn detect(&self, data: &[u8]) -> PatchState {
        let sites: Vec<_> = P20_CALL_SIGNATURES
            .iter()
            .filter_map(|sig| find_sites(data, sig).first().copied())
            .collect();
        if sites.len() != 4 {
            return PatchState::Unknown;
        }

        let installed = Self::try_read_installed_multiplier(data);
        let code_end = bci_code_end(data).unwrap_or(data.len());
        let h_offset = code_end - P20_HELPER_TEMPLATE.len() * 4;

        let mut all_orig = true;
        let mut all_redir = installed.is_some();

        for s in sites {
            let op = read_word(data, s + 16);
            let val = read_word(data, s + 20);
            let orig = op == Some(128) && val == Some(151);
            let redir = op == Some(120) && val == Some(h_offset as i32 - (s + 16) as i32 - 8);
            all_orig &= orig;
            all_redir &= redir;
        }

        if all_orig && installed.is_none() {
            PatchState::Original
        } else if all_redir {
            if installed == Some(self.multiplier) {
                PatchState::Ultimate
            } else {
                PatchState::Legacy
            }
        } else {
            PatchState::Unknown
        }
    }

    fn apply(&self, data: &mut Vec<u8>, enabled: bool) -> io::Result<bool> {
        let state = self.detect(data);
        if state == PatchState::Unknown {
            return Err(error("P20 unknown state"));
        }
        if (enabled && state == PatchState::Ultimate) || (!enabled && state == PatchState::Original)
        {
            return Ok(false);
        }

        let sites: Vec<_> = P20_CALL_SIGNATURES
            .iter()
            .filter_map(|sig| find_sites(data, sig).first().copied())
            .collect();
        if sites.len() != 4 {
            return Err(error("P20 sites count mismatch"));
        }

        let code_end = bci_code_end(data).unwrap_or(data.len());
        let hlen = P20_HELPER_TEMPLATE.len() * 4;
        let mut changed = false;

        if enabled {
            if state == PatchState::Legacy {
                // In-place update of multiplier literal
                let h_offset = code_end - hlen;
                changed |= write_word(data, h_offset + 14 * 4, self.multiplier)?;
            } else {
                let mut words = P20_HELPER_TEMPLATE.to_vec();
                words[14] = self.multiplier;
                let helper_bytes = words_bytes(&words);
                let insert_at = code_end;
                data.splice(insert_at..insert_at, helper_bytes);
                if let Some(mut cs) = read_word(data, 8) {
                    cs += hlen as i32;
                    let _ = write_word(data, 8, cs);
                }
                for s in sites {
                    let operand = insert_at as i32 - (s + 16) as i32 - 8;
                    write_word(data, s + 16, 120)?;
                    write_word(data, s + 20, operand)?;
                }
                changed = true;
            }
        } else if let Some(_) = Self::try_read_installed_multiplier(data) {
            let remove_at = code_end - hlen;
            data.drain(remove_at..code_end);
            if let Some(mut cs) = read_word(data, 8) {
                cs -= hlen as i32;
                let _ = write_word(data, 8, cs);
            }
            for s in sites {
                write_word(data, s + 16, 128)?;
                write_word(data, s + 20, 151)?;
            }
            changed = true;
        }

        Ok(changed)
    }
}

pub fn literal_patches() -> Vec<BciLiteralPatch> {
    vec![
        BciLiteralPatch {
            id: "P1".into(),
            target_pattern: LEVEL_PATTERN.into(),
            signature: vec![
                Some(0x42),
                None,
                Some(0x42),
                Some(1),
                Some(0x42),
                None,
                Some(0x42),
                None,
                Some(0x42),
                Some(0),
                Some(0x42),
                Some(0),
                Some(0x42),
                Some(8),
                Some(0x42),
                Some(3),
                Some(0x5A),
                Some(7),
                Some(0x80),
                Some(0xD4),
                Some(0x49),
                Some(-9),
                Some(0x56),
            ],
            value_word_indices: vec![5, 7],
            original_values: vec![4, 4],
            ultimate_values: vec![20, 20],
            expected_site_count: 1,
        },
        BciLiteralPatch {
            id: "P2".into(),
            target_pattern: LEVEL_PATTERN.into(),
            signature: vec![
                Some(0x42),
                None,
                Some(0x42),
                Some(1),
                Some(0x42),
                None,
                Some(0x42),
                None,
                Some(0x42),
                Some(0),
                Some(0x42),
                Some(0),
                Some(0x42),
                Some(8),
                Some(0x42),
                Some(3),
                Some(0x5A),
                Some(7),
                Some(0x80),
                Some(0xD4),
                Some(0x49),
                Some(-9),
                Some(0x56),
            ],
            value_word_indices: vec![1],
            original_values: vec![0],
            ultimate_values: vec![1],
            expected_site_count: 1,
        },
        BciLiteralPatch {
            id: "P3".into(),
            target_pattern: LEVEL_PATTERN.into(),
            signature: vec![
                Some(0x80),
                Some(83),
                Some(0x56),
                Some(66),
                None,
                Some(32),
                Some(44),
                Some(164),
                Some(0x42),
                Some(34),
                Some(0x5B),
                Some(5),
            ],
            value_word_indices: vec![4],
            original_values: vec![180000],
            ultimate_values: vec![30000],
            expected_site_count: 1,
        },
        BciLiteralPatch {
            id: "P5".into(),
            target_pattern: LEVEL_PATTERN.into(),
            signature: vec![
                Some(0x5A),
                Some(24),
                Some(0x42),
                None,
                Some(96),
                Some(101),
                Some(117),
                Some(16),
                Some(0x42),
                Some(1),
                Some(0x5B),
                Some(17),
            ],
            value_word_indices: vec![3],
            original_values: vec![20],
            ultimate_values: vec![3],
            expected_site_count: 1,
        },
        BciLiteralPatch {
            id: "P10".into(),
            target_pattern: "SYSTEM/CLAK/SCRIPT/ak_haupthaus.bci".into(),
            signature: vec![
                None,
                None,
                Some(81),
                Some(11),
                Some(81),
                Some(10),
                Some(81),
                Some(98),
                Some(128),
                Some(81),
                Some(73),
                Some(-4),
                Some(86),
            ],
            value_word_indices: vec![0, 1],
            original_values: vec![81, 59],
            ultimate_values: vec![66, 20],
            expected_site_count: 1,
        },
        BciLiteralPatch {
            id: "P11".into(),
            target_pattern: "SYSTEM/CLAK/SCRIPT/ak_haupthaus.bci".into(),
            signature: vec![
                Some(66),
                None,
                Some(66),
                Some(25),
                Some(66),
                Some(-25),
                Some(128),
                Some(34),
                Some(73),
                Some(-2),
                Some(86),
                Some(32),
                Some(82),
                Some(14),
                Some(81),
                Some(14),
                Some(82),
                Some(15),
            ],
            value_word_indices: vec![1],
            original_values: vec![1500],
            ultimate_values: vec![100],
            expected_site_count: 1,
        },
        BciLiteralPatch {
            id: "P12".into(),
            target_pattern: "SYSTEM/CLAK/SCRIPT/Dorfverteidigung.bci".into(),
            signature: vec![
                Some(66),
                Some(0),
                Some(66),
                Some(1),
                Some(66),
                None,
                Some(66),
                None,
                Some(66),
                Some(0),
                Some(66),
                Some(0),
                Some(66),
                None,
                Some(66),
                Some(1),
                Some(90),
                Some(8),
                Some(128),
                Some(157),
                Some(73),
                Some(-9),
                Some(86),
            ],
            value_word_indices: vec![5, 7],
            original_values: vec![6, 6],
            ultimate_values: vec![20, 20],
            expected_site_count: 4,
        },
        BciLiteralPatch {
            id: "P17".into(),
            target_pattern: LEVEL_PATTERN.into(),
            signature: vec![
                Some(66),
                None,
                Some(66),
                Some(100),
                Some(66),
                Some(1),
                Some(128),
                Some(16),
                Some(73),
                Some(-2),
                Some(86),
                Some(96),
                Some(101),
                Some(117),
            ],
            value_word_indices: vec![1],
            original_values: vec![60],
            ultimate_values: vec![100],
            expected_site_count: 1,
        },
        BciLiteralPatch {
            id: "P18".into(),
            target_pattern: LEVEL_PATTERN.into(),
            signature: vec![
                Some(120),
                Some(-636),
                Some(73),
                Some(-3),
                Some(86),
                Some(66),
                None,
                Some(96),
                Some(101),
                Some(117),
                Some(20),
                Some(66),
                Some(0),
                Some(87),
            ],
            value_word_indices: vec![6],
            original_values: vec![0],
            ultimate_values: vec![1],
            expected_site_count: 1,
        },
        BciLiteralPatch {
            id: "P19".into(),
            target_pattern: LEVEL_PATTERN.into(),
            signature: vec![
                Some(66),
                None,
                Some(90),
                Some(1),
                Some(90),
                Some(0),
                Some(120),
                Some(-36800),
                Some(73),
                Some(-3),
                Some(86),
            ],
            value_word_indices: vec![1],
            original_values: vec![2500],
            ultimate_values: vec![800],
            expected_site_count: 1,
        },
    ]
}

pub struct EndlessAiModule {
    pub id: String,
    pub name: String,
    pub patches: Vec<Box<dyn EndlessPatch>>,
}

impl EndlessAiModule {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        patches: Vec<Box<dyn EndlessPatch>>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            patches,
        }
    }
}

pub struct EndlessAiOrchestrator {
    pub m1: EndlessAiModule,
    pub respawn_core: EndlessAiModule,
    pub m2: EndlessAiModule,
    pub m3: EndlessAiModule,
    pub m4: EndlessAiModule,
    pub m5: EndlessAiModule,
    pub m6: EndlessAiModule,
    pub village_garrison_quota_3x: EndlessAiModule,
    pub r0: EndlessAiModule,
    pub user_modules: Vec<String>,
    pub file_cache: HashMap<PathBuf, (Vec<u8>, bool)>,
    /// Raw bytes (>= 64) each cached file was loaded with; its PFIL header is reused on save, like C# BciScriptFile.RawBytes.
    pub raw_headers: HashMap<PathBuf, Vec<u8>>,
    pub village_quota_multiplier: i32,
    pub exclude_custom_maps: bool,
}

impl Default for EndlessAiOrchestrator {
    fn default() -> Self {
        Self::new(false)
    }
}

impl EndlessAiOrchestrator {
    pub fn new(exclude_custom_maps: bool) -> Self {
        let lps = literal_patches();
        let lp = |id: &str| {
            Box::new(lps.iter().find(|p| p.id == id).unwrap().clone()) as Box<dyn EndlessPatch>
        };

        let m1 = EndlessAiModule::new("M1", "增援規模", vec![lp("P1"), lp("P10"), lp("P12")]);
        let m2 = EndlessAiModule::new(
            "M2",
            "增援節奏",
            vec![lp("P3"), Box::new(P6LoopDelayPatch), lp("P2")],
        );
        let m3 = EndlessAiModule::new(
            "M3",
            "敗亡快速回收",
            vec![Box::new(P4RetreatDeadlinePatch), lp("P5"), lp("P11")],
        );
        let m4 = EndlessAiModule::new(
            "M4",
            "強制部落生成",
            vec![Box::new(CustomPatch(7)), lp("P17"), lp("P18"), lp("P19")],
        );
        let m5 = EndlessAiModule::new("M5", "開局資源", vec![Box::new(CustomPatch(13))]);
        let m6 = EndlessAiModule::new(
            "M6",
            "羅馬增援士兵留守",
            vec![Box::new(CustomPatch(8)), Box::new(P9RetreatQuotaPatch)],
        );
        let village = EndlessAiModule::new(
            "VillageGarrisonQuota3x",
            "村莊 AI 駐軍配額倍率",
            vec![Box::new(P20VillageGarrisonQuotaPatch::default())],
        );
        let r0 = EndlessAiModule::new(
            "R0",
            "常駐修復",
            vec![Box::new(CustomPatch(14)), Box::new(CustomPatch(15))],
        );

        let mut core_patches: Vec<Box<dyn EndlessPatch>> = vec![];
        core_patches.push(lp("P3"));
        core_patches.push(Box::new(P6LoopDelayPatch));
        core_patches.push(lp("P2"));
        core_patches.push(Box::new(P4RetreatDeadlinePatch));
        core_patches.push(lp("P5"));
        core_patches.push(lp("P11"));
        core_patches.push(Box::new(CustomPatch(7)));
        core_patches.push(lp("P17"));
        core_patches.push(lp("P18"));
        core_patches.push(lp("P19"));
        let respawn_core = EndlessAiModule::new("Core", "無盡重生核心", core_patches);

        let user_modules = vec![
            "M1".into(),
            "Core".into(),
            "M5".into(),
            "M6".into(),
            "VillageGarrisonQuota3x".into(),
        ];

        Self {
            m1,
            respawn_core,
            m2,
            m3,
            m4,
            m5,
            m6,
            village_garrison_quota_3x: village,
            r0,
            user_modules,
            file_cache: HashMap::new(),
            raw_headers: HashMap::new(),
            village_quota_multiplier: 3,
            exclude_custom_maps,
        }
    }

    pub fn set_village_garrison_quota_multiplier(&mut self, multiplier: i32) {
        if multiplier > 1 {
            self.village_quota_multiplier = multiplier;
        }
    }

    fn module_mut(&mut self, id: &str) -> Option<&mut EndlessAiModule> {
        match id {
            "M1" => Some(&mut self.m1),
            "Core" => Some(&mut self.respawn_core),
            "M2" => Some(&mut self.m2),
            "M3" => Some(&mut self.m3),
            "M4" => Some(&mut self.m4),
            "M5" => Some(&mut self.m5),
            "M6" => Some(&mut self.m6),
            "VillageGarrisonQuota3x" => Some(&mut self.village_garrison_quota_3x),
            "R0" => Some(&mut self.r0),
            _ => None,
        }
    }

    /// Detects a module by id (module is temporarily moved out so the cache can be borrowed mutably).
    pub fn detect_module_by_id(&mut self, game_path: &Path, id: &str) -> Option<PatchState> {
        let module = std::mem::replace(self.module_mut(id)?, EndlessAiModule::new("", "", vec![]));
        let state = self.detect_module(game_path, &module);
        *self.module_mut(id).unwrap() = module;
        Some(state)
    }

    pub fn apply_module_by_id(
        &mut self,
        game_path: &Path,
        id: &str,
        enabled: bool,
    ) -> Option<io::Result<bool>> {
        let module = std::mem::replace(self.module_mut(id)?, EndlessAiModule::new("", "", vec![]));
        let result = self.apply_module(game_path, &module, enabled);
        *self.module_mut(id).unwrap() = module;
        Some(result)
    }

    pub fn get_module(&self, id: &str) -> Option<&EndlessAiModule> {
        match id {
            "M1" => Some(&self.m1),
            "Core" => Some(&self.respawn_core),
            "M2" => Some(&self.m2),
            "M3" => Some(&self.m3),
            "M4" => Some(&self.m4),
            "M5" => Some(&self.m5),
            "M6" => Some(&self.m6),
            "VillageGarrisonQuota3x" => Some(&self.village_garrison_quota_3x),
            "R0" => Some(&self.r0),
            _ => None,
        }
    }

    pub fn clear_cache(&mut self) {
        self.file_cache.clear();
        self.raw_headers.clear();
    }

    /// C# BciScriptFile.ReplaceRawBytes: swap in raw (possibly PFIL) bytes, adopting their header, and mark modified.
    pub fn replace_raw_bytes(&mut self, path: &Path, raw: &[u8]) -> io::Result<()> {
        let decompressed = crate::decompress_if_pfil(raw)?;
        self.raw_headers.insert(path.to_path_buf(), raw.to_vec());
        self.file_cache
            .insert(path.to_path_buf(), (decompressed, true));
        Ok(())
    }

    pub fn get_or_load_file(&mut self, path: &Path) -> io::Result<&mut (Vec<u8>, bool)> {
        if !self.file_cache.contains_key(path) {
            let raw = fs::read(path)?;
            let decompressed = crate::decompress_if_pfil(&raw)?;
            self.raw_headers.insert(path.to_path_buf(), raw);
            self.file_cache
                .insert(path.to_path_buf(), (decompressed, false));
        }
        Ok(self.file_cache.get_mut(path).unwrap())
    }

    pub fn detect_village_garrison_quota_multiplier(&mut self, game_path: &Path) -> i32 {
        let paths = Self::resolve_paths(game_path, "SYSTEM/CLAK/SCRIPT/Dorfverteidigung.bci");
        if paths.len() != 1 {
            return 1;
        }
        let Ok(file) = self.get_or_load_file(&paths[0]) else {
            return 1;
        };
        P20VillageGarrisonQuotaPatch::try_read_installed_multiplier(&file.0).unwrap_or(1)
    }

    pub fn resolve_paths(game_path: &Path, pattern: &str) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let maps = game_path.join("MAPS");
        let system = game_path.join("SYSTEM");

        if pattern == LEVEL_PATTERN {
            if maps.is_dir() {
                if let Ok(entries) = fs::read_dir(&maps) {
                    let mut dirs = Vec::new();
                    for e in entries.flatten() {
                        let name = e.file_name().to_string_lossy().to_string();
                        if name.len() == 8
                            && name.to_ascii_uppercase().starts_with("ENDL_")
                            && name[5..].chars().all(|c| c.is_ascii_digit())
                        {
                            if !has_standalone_level(&e.path()) {
                                dirs.push(e.path());
                            }
                        }
                    }
                    dirs.sort();
                    for d in dirs {
                        let f = d.join("SCRIPT/ak_level.bci");
                        if f.is_file() {
                            paths.push(f);
                        }
                    }
                }
            }
        } else if pattern == "SYSTEM/CLAK/SCRIPT/ak_haupthaus.bci" {
            let f = system.join("CLAK/SCRIPT/ak_haupthaus.bci");
            if f.is_file() {
                paths.push(f);
            }
        } else if pattern == "SYSTEM/CLAK/SCRIPT/Dorfverteidigung.bci" {
            let f = system.join("CLAK/SCRIPT/Dorfverteidigung.bci");
            if f.is_file() {
                paths.push(f);
            }
        } else if pattern == "MAPS/ENDL_*/Endlos_*_Siedlung*.sdl" {
            if maps.is_dir() {
                if let Ok(entries) = fs::read_dir(&maps) {
                    let mut dirs = Vec::new();
                    for e in entries.flatten() {
                        let name = e.file_name().to_string_lossy().to_string();
                        if name.len() == 8
                            && name.to_ascii_uppercase().starts_with("ENDL_")
                            && name[5..].chars().all(|c| c.is_ascii_digit())
                        {
                            if !has_standalone_level(&e.path()) {
                                dirs.push(e.path());
                            }
                        }
                    }
                    dirs.sort();
                    for d in dirs {
                        if let Ok(sub) = fs::read_dir(&d) {
                            let mut sdls = Vec::new();
                            for se in sub.flatten() {
                                let sname = se.file_name().to_string_lossy().to_string();
                                if sname.starts_with("Endlos_")
                                    && sname.ends_with(".sdl")
                                    && sname.contains("_Siedlung")
                                {
                                    sdls.push(se.path());
                                }
                            }
                            sdls.sort();
                            paths.extend(sdls);
                        }
                    }
                }
            }
        } else if pattern == "SYSTEM/CLAK/SCRIPT/ak_npc.bci|SYSTEM/CLAK/SCRIPT/ak_produktion.bci" {
            let f1 = system.join("CLAK/SCRIPT/ak_npc.bci");
            let f2 = system.join("CLAK/SCRIPT/ak_produktion.bci");
            if f1.is_file() {
                paths.push(f1);
            }
            if f2.is_file() {
                paths.push(f2);
            }
        }
        paths
    }

    /// C# ResolveOperationPaths: optionally drops files owned by marker-backed custom maps.
    fn resolve_operation_paths(&self, game_path: &Path, pattern: &str) -> Vec<PathBuf> {
        let mut paths = Self::resolve_paths(game_path, pattern);
        if self.exclude_custom_maps {
            let maps = game_path.join("MAPS");
            paths.retain(|p| !crate::services::is_custom_map_file(&maps, p));
        }
        paths
    }

    /// C# GetExpectedFileCount(gamePath, pattern): MAPS patterns expect whatever is installed.
    pub fn get_expected_file_count_for(game_path: &Path, pattern: &str) -> usize {
        if pattern == LEVEL_PATTERN || pattern == "MAPS/ENDL_*/Endlos_*_Siedlung*.sdl" {
            return Self::resolve_paths(game_path, pattern).len();
        }
        Self::get_expected_file_count(pattern)
    }

    pub fn get_expected_file_count(pattern: &str) -> usize {
        match pattern {
            LEVEL_PATTERN => 5,
            "SYSTEM/CLAK/SCRIPT/ak_haupthaus.bci" => 1,
            "SYSTEM/CLAK/SCRIPT/Dorfverteidigung.bci" => 1,
            "MAPS/ENDL_*/Endlos_*_Siedlung*.sdl" => 42,
            "SYSTEM/CLAK/SCRIPT/ak_npc.bci|SYSTEM/CLAK/SCRIPT/ak_produktion.bci" => 2,
            _ => 0,
        }
    }

    pub fn detect_module(&mut self, game_path: &Path, module: &EndlessAiModule) -> PatchState {
        let mut any_file = false;
        let mut all_orig = true;
        let mut all_ult = true;

        for patch in &module.patches {
            let paths = self.resolve_operation_paths(game_path, patch.target_pattern());
            if !paths.is_empty() {
                any_file = true;
            }
            let expected =
                if self.exclude_custom_maps && patch.target_pattern().starts_with("MAPS/") {
                    paths.len()
                } else {
                    Self::get_expected_file_count_for(game_path, patch.target_pattern())
                };
            if paths.len() != expected {
                if paths.is_empty() {
                    all_ult = false;
                    continue;
                }
                all_orig = false;
                all_ult = false;
            }
            for p in &paths {
                let Ok(file) = self.get_or_load_file(p) else {
                    return PatchState::Unknown;
                };
                let state = patch.detect(&file.0);
                match state {
                    PatchState::Unknown => return PatchState::Unknown,
                    PatchState::Original => all_ult = false,
                    PatchState::Ultimate => all_orig = false,
                    PatchState::Legacy => {
                        all_orig = false;
                        all_ult = false;
                    }
                }
            }
        }

        if !any_file {
            return PatchState::Unknown;
        }
        if all_orig {
            PatchState::Original
        } else if all_ult {
            PatchState::Ultimate
        } else {
            PatchState::Legacy
        }
    }

    pub fn apply_module(
        &mut self,
        game_path: &Path,
        module: &EndlessAiModule,
        enabled: bool,
    ) -> io::Result<bool> {
        let state = self.detect_module(game_path, module);
        if state == PatchState::Unknown && enabled {
            return Err(error("Endless AI module in unknown state"));
        }
        if !enabled && (state == PatchState::Original || state == PatchState::Unknown) {
            return Ok(false);
        }

        let mut changed = false;
        for patch in &module.patches {
            let paths = self.resolve_operation_paths(game_path, patch.target_pattern());
            for p in paths {
                let file = self.get_or_load_file(&p)?;
                if patch.apply(&mut file.0, enabled)? {
                    file.1 = true;
                    changed = true;
                }
            }
        }
        Ok(changed)
    }

    pub fn apply_mandatory_repair(&mut self, game_path: &Path) -> io::Result<bool> {
        // C# ApplyMandatoryRepair: always restore R0 sites to original, regardless of detected state.
        let r0 = std::mem::replace(&mut self.r0, EndlessAiModule::new("", "", vec![]));
        let result = (|| -> io::Result<bool> {
            let mut changed = false;
            for patch in &r0.patches {
                for path in self.resolve_operation_paths(game_path, patch.target_pattern()) {
                    let file = self.get_or_load_file(&path)?;
                    if patch.apply(&mut file.0, false)? {
                        file.1 = true;
                        changed = true;
                    }
                }
            }
            Ok(changed)
        })();
        self.r0 = r0;
        result
    }

    pub fn save_all(&mut self) -> io::Result<Vec<(PathBuf, Vec<u8>)>> {
        let mut results = Vec::new();
        let mut paths: Vec<_> = self.file_cache.keys().cloned().collect();
        paths.sort();
        for path in paths {
            let (bytes, modified) = &self.file_cache[&path];
            if *modified {
                let header = self
                    .raw_headers
                    .get(&path)
                    .map(Vec::as_slice)
                    .filter(|h| h.len() >= crate::lzss::PFIL_HEADER_LEN)
                    .ok_or_else(|| error("PFIL header must contain at least 64 bytes."))?;
                let compressed = crate::compress_pfil(bytes, Some(header))?;
                results.push((path, compressed));
            }
        }
        // A later serialization failure must not discard earlier dirty entries on retry.
        for (path, _) in &results {
            self.file_cache.get_mut(path).unwrap().1 = false;
        }
        Ok(results)
    }
}
