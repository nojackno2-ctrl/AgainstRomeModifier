//! Executable patch model and profile applicator, ported from ExePatchModel.cs.
use crate::PatchProfile;
use std::io;

fn error(s: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, s.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExePatchState {
    Unknown,
    Original,
    FocusPatched,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExeSpellAltarPatchState {
    Unknown,
    Original,
    Patched,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExeRomanEndlessPatchState {
    Unknown,
    Original,
    Patched,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExeCiviProduce20PatchState {
    Unknown,
    Original,
    Patched,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExeUnitRecruit20PatchState {
    Unknown,
    Original,
    Patched,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExeIdleSelect999PatchState {
    Unknown,
    Original,
    Patched,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExeCorpseRetentionPatchState {
    Unknown,
    Original,
    Patched,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExeDefaultSpecialArrowsPatchState {
    Unknown,
    Original,
    Patched,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExeNativeWidescreenPatchState {
    Unknown,
    Original,
    LegacyUnforced,
    LegacyForcedStaleUi,
    Patched,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExeCameraZoomOutPatchState {
    Unknown,
    Original,
    LegacyZoomHalf,
    Patched,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExeVillageSetterPatchState {
    Unknown,
    Original,
    Legacy2x,
    Legacy2Point5x,
    Legacy3x,
    Legacy5x,
    EntireMap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExeVillageRangePatchState {
    Unknown,
    Original,
    LegacyLogicOnly,
    Expanded,
}

// Constants from ExePatchModel
pub const FOCUS_OFFSET: usize = 0x161a88;
pub const FOCUS_ORIGINAL: &[u8] = &[0x89, 0x15, 0xC4, 0x7D, 0x9E, 0x02];
pub const FOCUS_PATCHED: &[u8] = &[0x90; 6];

pub const ROMAN_ENDLESS_OFFSET: usize = 0x5bd60;
pub const ROMAN_ENDLESS_ORIGINAL: &[u8] = &[
    0x53, 0x8B, 0x5C, 0x24, 0x08, 0x53, 0xE8, 0x25, 0x3B, 0xFE, 0xFF, 0x83, 0xC4, 0x04, 0x53, 0x89,
    0x1D, 0x78, 0x74, 0x73, 0x00,
];
pub const ROMAN_ENDLESS_PATCHED: &[u8] = &[
    0x53, 0x6A, 0x03, 0x5B, 0x90, 0x53, 0xE8, 0x25, 0x3B, 0xFE, 0xFF, 0x83, 0xC4, 0x04, 0x53, 0x89,
    0x1D, 0x78, 0x74, 0x73, 0x00,
];

pub const CIVI_PRODUCE_20_OFFSET: usize = 0x4fc00;
pub const CIVI_PRODUCE_20_ORIGINAL: &[u8] = &[
    0x83, 0x7C, 0x24, 0x18, 0x01, 0x0F, 0x95, 0xC0, 0x25, 0xFF, 0x00, 0x00, 0x00, 0x6A, 0x01, 0x40,
    0x50, 0x8B, 0x74, 0x24, 0x0C, 0x56, 0x8B, 0x7C, 0x24, 0x0C, 0x57, 0xE8, 0x90, 0x15, 0xFD, 0xFF,
];
pub const CIVI_PRODUCE_20_PATCHED: &[u8] = &[
    0x83, 0x7C, 0x24, 0x18, 0x01, 0x0F, 0x95, 0xC0, 0x25, 0xFF, 0x00, 0x00, 0x00, 0x6A, 0x14, 0x40,
    0x50, 0x8B, 0x74, 0x24, 0x0C, 0x56, 0x8B, 0x7C, 0x24, 0x0C, 0x57, 0xE8, 0x90, 0x15, 0xFD, 0xFF,
];

pub const UNIT_RECRUIT_20_OFFSET: usize = 0x4c7dd;
pub const UNIT_RECRUIT_20_ORIGINAL: &[u8] = &[0x29, 0xFD, 0x01, 0xFE];
pub const UNIT_RECRUIT_20_PATCHED: &[u8] = &[0x6A, 0x14, 0x5E, 0x90];

pub const IDLE_SELECT_999_OFFSET: usize = 0x51dc0;
pub const IDLE_SELECT_999_ORIGINAL: &[u8] = &[
    0x53, 0x56, 0x57, 0x81, 0xec, 0x40, 0x01, 0x00, 0x00, 0xba, 0xff, 0xff, 0xff, 0xff, 0x31, 0xdb,
    0x31, 0xf6, 0x83, 0xc3, 0x04, 0x89, 0x54, 0x1c, 0xfc, 0x81, 0xfb, 0xa0, 0x00, 0x00, 0x00, 0x75,
    0xf1, 0x6a, 0x03, 0x6a, 0x01, 0x6a, 0x01, 0x6a, 0x00, 0x6a, 0x01, 0xe8, 0x00, 0xb1, 0xff, 0xff,
    0x50, 0x6a, 0x28, 0x8d, 0x44, 0x24, 0x1c, 0x50, 0x8d, 0x84, 0x24, 0xc0, 0x00, 0x00, 0x00, 0x50,
    0x31, 0xdb, 0xe8, 0x19, 0xfa, 0xfc, 0xff, 0x83, 0xc4, 0x24, 0x8b, 0x0c, 0x1c, 0x51, 0x8b, 0xbc,
    0x1c, 0xa4, 0x00, 0x00, 0x00, 0x57, 0xe8, 0x15, 0xf3, 0xfc, 0xff, 0x83, 0xc4, 0x08, 0x83, 0xf8,
    0xff, 0x75, 0x19, 0x83, 0xc3, 0x04, 0x81, 0xfb, 0xa0, 0x00, 0x00, 0x00, 0x75, 0xdc, 0x85, 0xf6,
    0x75, 0x1a, 0x81, 0xc4, 0x40, 0x01, 0x00, 0x00, 0x5f, 0x5e, 0x5b, 0xc3, 0x50, 0xbe, 0x01, 0x00,
    0x00, 0x00, 0xe8, 0x59, 0xb7, 0xff, 0xff, 0x83, 0xc4, 0x04, 0xeb, 0xd7, 0x6a, 0x7c, 0xe8, 0xed,
    0xbc, 0xfe, 0xff, 0x83, 0xc4, 0x04, 0x81, 0xc4, 0x40, 0x01, 0x00, 0x00, 0x5f, 0x5e, 0x5b, 0xc3,
    0x8d, 0x80, 0x00, 0x00, 0x00, 0x00, 0x8d, 0x92, 0x00, 0x00, 0x00, 0x00, 0x8d, 0x44, 0x20, 0x00,
];
pub const IDLE_SELECT_999_PATCHED: &[u8] = &[
    0x53, 0x56, 0x57, 0x81, 0xec, 0x38, 0x1f, 0x00, 0x00, 0xba, 0xff, 0xff, 0xff, 0xff, 0x31, 0xf6,
    0xbb, 0x9c, 0x0f, 0x00, 0x00, 0x83, 0xeb, 0x04, 0x89, 0x14, 0x1c, 0x75, 0xf8, 0x6a, 0x03, 0x6a,
    0x01, 0x6a, 0x01, 0x6a, 0x00, 0x6a, 0x01, 0xe8, 0x04, 0xb1, 0xff, 0xff, 0x50, 0x68, 0xe7, 0x03,
    0x00, 0x00, 0x8d, 0x44, 0x24, 0x1c, 0x50, 0x8d, 0x84, 0x24, 0xbc, 0x0f, 0x00, 0x00, 0x50, 0x31,
    0xdb, 0xe8, 0x1a, 0xfa, 0xfc, 0xff, 0x83, 0xc4, 0x24, 0x8b, 0x0c, 0x1c, 0x51, 0x8b, 0xbc, 0x1c,
    0xa0, 0x0f, 0x00, 0x00, 0x57, 0xe8, 0x16, 0xf3, 0xfc, 0xff, 0x83, 0xc4, 0x08, 0x83, 0xf8, 0xff,
    0x75, 0x19, 0x83, 0xc3, 0x04, 0x81, 0xfb, 0x9c, 0x0f, 0x00, 0x00, 0x75, 0xdc, 0x85, 0xf6, 0x75,
    0x1a, 0x81, 0xc4, 0x38, 0x1f, 0x00, 0x00, 0x5f, 0x5e, 0x5b, 0xc3, 0x50, 0xbe, 0x01, 0x00, 0x00,
    0x00, 0xe8, 0x5a, 0xb7, 0xff, 0xff, 0x83, 0xc4, 0x04, 0xeb, 0xd7, 0x6a, 0x7c, 0xe8, 0xee, 0xbc,
    0xfe, 0xff, 0x83, 0xc4, 0x04, 0x81, 0xc4, 0x38, 0x1f, 0x00, 0x00, 0x5f, 0x5e, 0x5b, 0xc3, 0x90,
    0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90,
];

pub const CORPSE_RETENTION_OFFSET: usize = 0x110906;
pub const CORPSE_RETENTION_ORIGINAL: &[u8] = &[0xBB, 0xF4, 0x01, 0x00, 0x00];
pub const CORPSE_RETENTION_PATCHED: &[u8] = &[0xBB, 0x32, 0x00, 0x00, 0x00];

pub const SPELL_ALTAR_SITES: &[(usize, &[u8], &[u8])] = &[
    (
        0x4A112,
        &[0x83, 0xFE, 0x01, 0x0F, 0x8C, 0x54, 0xFF, 0xFF, 0xFF],
        &[0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x54, 0xFF, 0xFF, 0xFF],
    ),
    (
        0x4A136,
        &[0x83, 0xFE, 0x02, 0x0F, 0x8C, 0x58, 0xFF, 0xFF, 0xFF],
        &[0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x58, 0xFF, 0xFF, 0xFF],
    ),
    (
        0x4A15A,
        &[0x83, 0xFE, 0x03, 0x0F, 0x8C, 0x5C, 0xFF, 0xFF, 0xFF],
        &[0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x5C, 0xFF, 0xFF, 0xFF],
    ),
    (
        0x4A0E3,
        &[0x83, 0xFE, 0x04, 0x0F, 0x8D, 0x92, 0x00, 0x00, 0x00],
        &[0x83, 0xFE, 0x00, 0x0F, 0x8D, 0x92, 0x00, 0x00, 0x00],
    ),
    (
        0x4A1CC,
        &[0x83, 0xFE, 0x01, 0x0F, 0x8D, 0xA3, 0x00, 0x00, 0x00],
        &[0x83, 0xFE, 0x00, 0x0F, 0x8D, 0xA3, 0x00, 0x00, 0x00],
    ),
    (
        0x4A293,
        &[0x83, 0xFE, 0x02, 0x0F, 0x8C, 0x61, 0xFF, 0xFF, 0xFF],
        &[0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x61, 0xFF, 0xFF, 0xFF],
    ),
    (
        0x4A2B7,
        &[0x83, 0xFE, 0x03, 0x0F, 0x8C, 0x65, 0xFF, 0xFF, 0xFF],
        &[0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x65, 0xFF, 0xFF, 0xFF],
    ),
    (
        0x4A249,
        &[0x83, 0xFE, 0x04, 0x0F, 0x8D, 0x89, 0x00, 0x00, 0x00],
        &[0x83, 0xFE, 0x00, 0x0F, 0x8D, 0x89, 0x00, 0x00, 0x00],
    ),
    (
        0x4A329,
        &[0x83, 0xFE, 0x01, 0x0F, 0x8D, 0xA3, 0x00, 0x00, 0x00],
        &[0x83, 0xFE, 0x00, 0x0F, 0x8D, 0xA3, 0x00, 0x00, 0x00],
    ),
    (
        0x4A3F0,
        &[0x83, 0xFE, 0x02, 0x0F, 0x8C, 0x61, 0xFF, 0xFF, 0xFF],
        &[0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x61, 0xFF, 0xFF, 0xFF],
    ),
    (
        0x4A414,
        &[0x83, 0xFE, 0x03, 0x0F, 0x8C, 0x65, 0xFF, 0xFF, 0xFF],
        &[0x83, 0xFE, 0x00, 0x0F, 0x8C, 0x65, 0xFF, 0xFF, 0xFF],
    ),
    (
        0x4A3A6,
        &[0x83, 0xFE, 0x04, 0x0F, 0x8D, 0x89, 0x00, 0x00, 0x00],
        &[0x83, 0xFE, 0x00, 0x0F, 0x8D, 0x89, 0x00, 0x00, 0x00],
    ),
];

// Special arrows retired v1 restore sites
pub const ARROWS_BTN_OFFSET: usize = 0x413FC;
pub const ARROWS_BTN_ORIGINAL: &[u8] = &[0x00];
pub const ARROWS_BTN_PATCHED: &[u8] = &[0x03];

pub const ARROWS_GETTER_OFFSET: usize = 0x4E980;
pub const ARROWS_GETTER_ORIGINAL: &[u8] = &[0xA1, 0x58, 0x6A, 0x73, 0x00, 0xC3];
pub const ARROWS_GETTER_PATCHED: &[u8] = &[0xE9, 0xE5, 0xFF, 0xFF, 0xFF, 0xC3];

pub const ARROWS_CAVE_OFFSET: usize = 0x4E96A;
pub const ARROWS_CAVE_ORIGINAL: &[u8] = &[
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x8D, 0x80, 0x00, 0x00, 0x00, 0x00, 0x8D, 0x92, 0x00, 0x00,
    0x00, 0x00, 0x8D, 0x44, 0x20, 0x00,
];
pub const ARROWS_CAVE_PATCHED: &[u8] = &[
    0xA1, 0x58, 0x6A, 0x73, 0x00, 0x83, 0xF8, 0x03, 0x75, 0x03, 0x31, 0xC0, 0xC3, 0x85, 0xC0, 0x75,
    0x01, 0x40, 0xC3, 0x90, 0x90, 0x90,
];

// Native Widescreen
pub const WS_ID_OFFSET: usize = 0x246B3;
pub const WS_ID_ORIGINAL: &[u8] = &[
    0x81, 0x3E, 0x40, 0x06, 0x00, 0x00, 0x75, 0x12, 0x81, 0x3B, 0xB0, 0x04, 0x00, 0x00, 0x75, 0x0A,
    0x83, 0xFD, 0x20, 0x75, 0x05, 0xBF, 0x22, 0x00, 0x00, 0x00,
];
pub const WS_ID_PATCHED: &[u8] = &[
    0x81, 0x3E, 0x80, 0x07, 0x00, 0x00, 0x75, 0x12, 0x81, 0x3B, 0x38, 0x04, 0x00, 0x00, 0x75, 0x0A,
    0x83, 0xFD, 0x20, 0x75, 0x05, 0xBF, 0x22, 0x00, 0x00, 0x00,
];

pub const WS_CREATE_OFFSET: usize = 0x249AA;
pub const WS_CREATE_ORIGINAL: &[u8] = &[
    0x6A, 0x50, 0x6A, 0x20, 0x68, 0xB0, 0x04, 0x00, 0x00, 0x68, 0x40, 0x06, 0x00, 0x00, 0xE8, 0x83,
    0x96, 0x15, 0x00, 0x83, 0xC4, 0x10, 0x85, 0xC0, 0x0F, 0x85, 0x01, 0xFE, 0xFF, 0xFF, 0x68, 0xB0,
    0x04, 0x00, 0x00, 0x68, 0x40, 0x06, 0x00, 0x00, 0xE8, 0xD9, 0x96, 0x15, 0x00, 0x83, 0xC4, 0x08,
];
pub const WS_CREATE_PATCHED: &[u8] = &[
    0x6A, 0x50, 0x6A, 0x20, 0x68, 0x38, 0x04, 0x00, 0x00, 0x68, 0x80, 0x07, 0x00, 0x00, 0xE8, 0x83,
    0x96, 0x15, 0x00, 0x83, 0xC4, 0x10, 0x85, 0xC0, 0x0F, 0x85, 0x01, 0xFE, 0xFF, 0xFF, 0x68, 0x38,
    0x04, 0x00, 0x00, 0x68, 0x80, 0x07, 0x00, 0x00, 0xE8, 0xD9, 0x96, 0x15, 0x00, 0x83, 0xC4, 0x08,
];

pub const WS_TEXT_OFFSET: usize = 0x1DCC76;
pub const WS_TEXT_ORIGINAL: &[u8] = b"Modus 1600x1200 32bit\n\0";
pub const WS_TEXT_PATCHED: &[u8] = b"Modus 1920x1080 32bit\n\0";

pub const WS_FORCE_OFFSET: usize = 0x24BD0;
pub const WS_FORCE_ORIGINAL: &[u8] = &[0x8B, 0x15, 0xB0, 0x80, 0x65, 0x00, 0x52];
pub const WS_FORCE_PATCHED: &[u8] = &[0x6A, 0x22, 0x90, 0x90, 0x90, 0x90, 0x90];

pub const WS_ACTMODE_OFFSET: usize = 0x24B80;
pub const WS_ACTMODE_ORIGINAL: &[u8] = &[0xA1, 0xB0, 0x80, 0x65, 0x00, 0xC3];
pub const WS_ACTMODE_PATCHED: &[u8] = &[0xA1, 0xBC, 0x80, 0x65, 0x00, 0xC3];

pub const WS_IGM_OFFSET: usize = 0x41E25;
pub const WS_IGM_ORIGINAL: &[u8] = &[0x83, 0xF8, 0x21, 0x74, 0xD4];
pub const WS_IGM_PATCHED: &[u8] = &[0x83, 0xF8, 0x22, 0x74, 0xD4];

// Camera zoom
pub const ZOOM_INIT_OFFSET: usize = 0x81388;
pub const ZOOM_INIT_ORIGINAL: &[u8] = &[0xE8, 0xA3, 0x76, 0x01, 0x00];
pub const ZOOM_INIT_PATCHED: &[u8] = &[0xE8, 0x33, 0x12, 0x0E, 0x00];

pub const ZOOM_LOAD_OFFSET: usize = 0x8D880;
pub const ZOOM_LOAD_ORIGINAL: &[u8] = &[0xE8, 0xAB, 0xB1, 0x00, 0x00];
pub const ZOOM_LOAD_PATCHED: &[u8] = &[0xE8, 0x3B, 0x4D, 0x0D, 0x00];

pub const ZOOM_SCRIPT_OFFSET: usize = 0x14C1FA;
pub const ZOOM_SCRIPT_ORIGINAL: &[u8] = &[0xE8, 0x31, 0xC8, 0xF4, 0xFF];
pub const ZOOM_SCRIPT_PATCHED: &[u8] = &[0xE8, 0xC1, 0x63, 0x01, 0x00];

pub const ZOOM_CAVE_OFFSET: usize = 0x1625C0;
pub const ZOOM_CAVE_ORIGINAL: &[u8] = &[0; 28];
pub const ZOOM_CAVE_PATCHED: &[u8] = &[
    0x8B, 0x44, 0x24, 0x04, 0x85, 0xC0, 0x78, 0x07, 0x3D, 0x00, 0x00, 0x80, 0x3F, 0x73, 0x08, 0xC7,
    0x44, 0x24, 0x04, 0x00, 0x00, 0x80, 0x3F, 0xE9, 0x54, 0x64, 0xF3, 0xFF,
];
pub const ZOOM_CAVE_LEGACY: &[u8] = &[
    0x8B, 0x44, 0x24, 0x04, 0x85, 0xC0, 0x78, 0x07, 0x3D, 0x00, 0x00, 0x00, 0x3F, 0x73, 0x08, 0xC7,
    0x44, 0x24, 0x04, 0x00, 0x00, 0x00, 0x3F, 0xE9, 0x54, 0x64, 0xF3, 0xFF,
];

// Village setter
pub const VILLAGE_HOOK_OFFSET: usize = 0x1364c1;
pub const VILLAGE_HOOK_ORIGINAL: &[u8] = &[0x85, 0xF6, 0x7C, 0xA6, 0x85, 0xFF, 0x7C, 0xA2];
pub const VILLAGE_HOOK_PATCHED: &[u8] = &[0xE9, 0xC9, 0xC0, 0x02, 0x00, 0x90, 0x90, 0x90];

pub const VILLAGE_CAVE_OFFSET: usize = 0x16258f;
pub const VILLAGE_CAVE_ORIGINAL: &[u8] = &[0; 39];
pub const VILLAGE_CAVE_PATCHED: &[u8] = &[
    0x85, 0xF6, 0x0F, 0x8C, 0xD4, 0x3E, 0xFD, 0xFF, 0x85, 0xFF, 0x0F, 0x8C, 0xCC, 0x3E, 0xFD, 0xFF,
    0xBE, 0x30, 0x75, 0x00, 0x00, 0xBF, 0x30, 0x75, 0x00, 0x00, 0x57, 0x56, 0x50, 0xE8, 0x4F, 0xE3,
    0xF5, 0xFF, 0xE9, 0x1B, 0x3F, 0xFD, 0xFF,
];

// Exact 39-byte legacy caves from ExePatchModel, including the 2x zero padding.
pub const VILLAGE_CAVE_LEGACY_2X: &[u8] = &[
    0x85, 0xF6, 0x0F, 0x8C, 0xD4, 0x3E, 0xFD, 0xFF, 0x85, 0xFF, 0x0F, 0x8C, 0xCC, 0x3E, 0xFD, 0xFF,
    0xD1, 0xE6, 0xD1, 0xE7, 0x57, 0x56, 0x50, 0xE8, 0x55, 0xE3, 0xF5, 0xFF, 0xE9, 0x21, 0x3F, 0xFD,
    0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
pub const VILLAGE_CAVE_LEGACY_2_POINT_5X: &[u8] = &[
    0x85, 0xF6, 0x0F, 0x8C, 0xD4, 0x3E, 0xFD, 0xFF, 0x85, 0xFF, 0x0F, 0x8C, 0xCC, 0x3E, 0xFD, 0xFF,
    0x8D, 0x34, 0xB6, 0xD1, 0xEE, 0x8D, 0x3C, 0xBF, 0xD1, 0xEF, 0x57, 0x56, 0x50, 0xE8, 0x4F, 0xE3,
    0xF5, 0xFF, 0xE9, 0x1B, 0x3F, 0xFD, 0xFF,
];
pub const VILLAGE_CAVE_LEGACY_3X: &[u8] = &[
    0x85, 0xF6, 0x0F, 0x8C, 0xD4, 0x3E, 0xFD, 0xFF, 0x85, 0xFF, 0x0F, 0x8C, 0xCC, 0x3E, 0xFD, 0xFF,
    0x8D, 0x34, 0x76, 0x90, 0x90, 0x8D, 0x3C, 0x7F, 0x90, 0x90, 0x57, 0x56, 0x50, 0xE8, 0x4F, 0xE3,
    0xF5, 0xFF, 0xE9, 0x1B, 0x3F, 0xFD, 0xFF,
];
pub const VILLAGE_CAVE_LEGACY_5X: &[u8] = &[
    0x85, 0xF6, 0x0F, 0x8C, 0xD4, 0x3E, 0xFD, 0xFF, 0x85, 0xFF, 0x0F, 0x8C, 0xCC, 0x3E, 0xFD, 0xFF,
    0x8D, 0x34, 0xB6, 0x90, 0x90, 0x8D, 0x3C, 0xBF, 0x90, 0x90, 0x57, 0x56, 0x50, 0xE8, 0x4F, 0xE3,
    0xF5, 0xFF, 0xE9, 0x1B, 0x3F, 0xFD, 0xFF,
];

// Legacy village build range patches (ExePatchModel.VillageRange* / VillageFrame*)
pub const VILLAGE_RANGE_X_OFFSET: usize = 0x1366c4;
pub const VILLAGE_RANGE_Z_OFFSET: usize = 0x1366cd;
pub const VILLAGE_FRAME_X_OFFSET: usize = 0x0d722c;
pub const VILLAGE_FRAME_Z_OFFSET: usize = 0x0d723b;
pub const VILLAGE_RANGE_PATCH_REQUIRED_LENGTH: usize = 0x1366d0;
pub const VILLAGE_RANGE_X_ORIGINAL: &[u8] = &[0xC1, 0xE2, 0x06];
pub const VILLAGE_RANGE_Z_ORIGINAL: &[u8] = &[0xC1, 0xE1, 0x06];
pub const VILLAGE_RANGE_X_PATCHED: &[u8] = &[0xC1, 0xE2, 0x07];
pub const VILLAGE_RANGE_Z_PATCHED: &[u8] = &[0xC1, 0xE1, 0x07];
pub const VILLAGE_FRAME_X_ORIGINAL: &[u8] = &[0xC1, 0xE6, 0x06];
pub const VILLAGE_FRAME_Z_ORIGINAL: &[u8] = &[0xC1, 0xE7, 0x06];
pub const VILLAGE_FRAME_X_PATCHED: &[u8] = &[0xC1, 0xE6, 0x07];
pub const VILLAGE_FRAME_Z_PATCHED: &[u8] = &[0xC1, 0xE7, 0x07];

// Game speed constants
pub const GAME_SPEED_QPC_OFFSET: usize = 0x204214;
pub const GAME_SPEED_TGT_OFFSET: usize = 0x20424c;
pub const GAME_SPEED_QPC_BASE: f64 = 1_000_000_000.0;
pub const GAME_SPEED_TGT_BASE: f64 = 1_000_000.0;

fn slice<'a>(bytes: &'a [u8], offset: usize, len: usize) -> Option<&'a [u8]> {
    bytes.get(offset..offset.checked_add(len)?)
}

pub fn get_game_speed_multiplier(bytes: &[u8]) -> i32 {
    let Some(qpc_bytes) = slice(bytes, GAME_SPEED_QPC_OFFSET, 8) else {
        return 0;
    };
    let Some(tgt_bytes) = slice(bytes, GAME_SPEED_TGT_OFFSET, 8) else {
        return 0;
    };
    let qpc = f64::from_le_bytes(qpc_bytes.try_into().unwrap());
    let tgt = f64::from_le_bytes(tgt_bytes.try_into().unwrap());
    if qpc <= 0.0 || tgt <= 0.0 {
        return 0;
    }
    let s_qpc = qpc / GAME_SPEED_QPC_BASE;
    let s_tgt = tgt / GAME_SPEED_TGT_BASE;
    let rq = s_qpc.round() as i32;
    let rt = s_tgt.round() as i32;
    if rq != rt || rq < 1 {
        return 0;
    }
    if (s_qpc - rq as f64).abs() > 1e-6 || (s_tgt - rt as f64).abs() > 1e-6 {
        return 0;
    }
    rq
}

pub fn get_focus_patch_state(bytes: &[u8]) -> ExePatchState {
    match slice(bytes, FOCUS_OFFSET, FOCUS_ORIGINAL.len()) {
        Some(b) if b == FOCUS_ORIGINAL => ExePatchState::Original,
        Some(b) if b == FOCUS_PATCHED => ExePatchState::FocusPatched,
        _ => ExePatchState::Unknown,
    }
}

pub fn get_spell_altar_patch_state(bytes: &[u8]) -> ExeSpellAltarPatchState {
    let mut all_orig = true;
    let mut all_patched = true;
    for &(offset, orig, patched) in SPELL_ALTAR_SITES {
        match slice(bytes, offset, orig.len()) {
            Some(b) if b == orig => all_patched = false,
            Some(b) if b == patched => all_orig = false,
            _ => return ExeSpellAltarPatchState::Unknown,
        }
    }
    if all_orig {
        ExeSpellAltarPatchState::Original
    } else if all_patched {
        ExeSpellAltarPatchState::Patched
    } else {
        ExeSpellAltarPatchState::Unknown
    }
}

pub fn get_roman_endless_patch_state(bytes: &[u8]) -> ExeRomanEndlessPatchState {
    match slice(bytes, ROMAN_ENDLESS_OFFSET, ROMAN_ENDLESS_ORIGINAL.len()) {
        Some(b) if b == ROMAN_ENDLESS_ORIGINAL => ExeRomanEndlessPatchState::Original,
        Some(b) if b == ROMAN_ENDLESS_PATCHED => ExeRomanEndlessPatchState::Patched,
        _ => ExeRomanEndlessPatchState::Unknown,
    }
}

pub fn get_civi_produce_20_patch_state(bytes: &[u8]) -> ExeCiviProduce20PatchState {
    match slice(
        bytes,
        CIVI_PRODUCE_20_OFFSET,
        CIVI_PRODUCE_20_ORIGINAL.len(),
    ) {
        Some(b) if b == CIVI_PRODUCE_20_ORIGINAL => ExeCiviProduce20PatchState::Original,
        Some(b) if b == CIVI_PRODUCE_20_PATCHED => ExeCiviProduce20PatchState::Patched,
        _ => ExeCiviProduce20PatchState::Unknown,
    }
}

pub fn get_unit_recruit_20_patch_state(bytes: &[u8]) -> ExeUnitRecruit20PatchState {
    match slice(
        bytes,
        UNIT_RECRUIT_20_OFFSET,
        UNIT_RECRUIT_20_ORIGINAL.len(),
    ) {
        Some(b) if b == UNIT_RECRUIT_20_ORIGINAL => ExeUnitRecruit20PatchState::Original,
        Some(b) if b == UNIT_RECRUIT_20_PATCHED => ExeUnitRecruit20PatchState::Patched,
        _ => ExeUnitRecruit20PatchState::Unknown,
    }
}

pub fn get_idle_select_999_patch_state(bytes: &[u8]) -> ExeIdleSelect999PatchState {
    match slice(
        bytes,
        IDLE_SELECT_999_OFFSET,
        IDLE_SELECT_999_ORIGINAL.len(),
    ) {
        Some(b) if b == IDLE_SELECT_999_ORIGINAL => ExeIdleSelect999PatchState::Original,
        Some(b) if b == IDLE_SELECT_999_PATCHED => ExeIdleSelect999PatchState::Patched,
        _ => ExeIdleSelect999PatchState::Unknown,
    }
}

pub fn get_corpse_retention_patch_state(bytes: &[u8]) -> ExeCorpseRetentionPatchState {
    match slice(
        bytes,
        CORPSE_RETENTION_OFFSET,
        CORPSE_RETENTION_ORIGINAL.len(),
    ) {
        Some(b) if b == CORPSE_RETENTION_ORIGINAL => ExeCorpseRetentionPatchState::Original,
        Some(b) if b == CORPSE_RETENTION_PATCHED => ExeCorpseRetentionPatchState::Patched,
        _ => ExeCorpseRetentionPatchState::Unknown,
    }
}

pub fn get_native_widescreen_patch_state(bytes: &[u8]) -> ExeNativeWidescreenPatchState {
    let mode_orig = slice(bytes, WS_ID_OFFSET, WS_ID_ORIGINAL.len()) == Some(WS_ID_ORIGINAL)
        && slice(bytes, WS_CREATE_OFFSET, WS_CREATE_ORIGINAL.len()) == Some(WS_CREATE_ORIGINAL)
        && slice(bytes, WS_TEXT_OFFSET, WS_TEXT_ORIGINAL.len()) == Some(WS_TEXT_ORIGINAL);
    let mode_patch = slice(bytes, WS_ID_OFFSET, WS_ID_PATCHED.len()) == Some(WS_ID_PATCHED)
        && slice(bytes, WS_CREATE_OFFSET, WS_CREATE_PATCHED.len()) == Some(WS_CREATE_PATCHED)
        && slice(bytes, WS_TEXT_OFFSET, WS_TEXT_PATCHED.len()) == Some(WS_TEXT_PATCHED);
    let force_orig =
        slice(bytes, WS_FORCE_OFFSET, WS_FORCE_ORIGINAL.len()) == Some(WS_FORCE_ORIGINAL);
    let force_patch =
        slice(bytes, WS_FORCE_OFFSET, WS_FORCE_PATCHED.len()) == Some(WS_FORCE_PATCHED);
    let ui_orig = slice(bytes, WS_ACTMODE_OFFSET, WS_ACTMODE_ORIGINAL.len())
        == Some(WS_ACTMODE_ORIGINAL)
        && slice(bytes, WS_IGM_OFFSET, WS_IGM_ORIGINAL.len()) == Some(WS_IGM_ORIGINAL);
    let ui_patch = slice(bytes, WS_ACTMODE_OFFSET, WS_ACTMODE_PATCHED.len())
        == Some(WS_ACTMODE_PATCHED)
        && slice(bytes, WS_IGM_OFFSET, WS_IGM_PATCHED.len()) == Some(WS_IGM_PATCHED);

    if mode_orig && force_orig && ui_orig {
        ExeNativeWidescreenPatchState::Original
    } else if mode_patch && force_orig && ui_orig {
        ExeNativeWidescreenPatchState::LegacyUnforced
    } else if mode_patch && force_patch && ui_orig {
        ExeNativeWidescreenPatchState::LegacyForcedStaleUi
    } else if mode_patch && force_patch && ui_patch {
        ExeNativeWidescreenPatchState::Patched
    } else {
        ExeNativeWidescreenPatchState::Unknown
    }
}

pub fn get_camera_zoom_out_patch_state(bytes: &[u8]) -> ExeCameraZoomOutPatchState {
    let calls_orig = slice(bytes, ZOOM_INIT_OFFSET, ZOOM_INIT_ORIGINAL.len())
        == Some(ZOOM_INIT_ORIGINAL)
        && slice(bytes, ZOOM_LOAD_OFFSET, ZOOM_LOAD_ORIGINAL.len()) == Some(ZOOM_LOAD_ORIGINAL)
        && slice(bytes, ZOOM_SCRIPT_OFFSET, ZOOM_SCRIPT_ORIGINAL.len())
            == Some(ZOOM_SCRIPT_ORIGINAL);
    let calls_patch = slice(bytes, ZOOM_INIT_OFFSET, ZOOM_INIT_PATCHED.len())
        == Some(ZOOM_INIT_PATCHED)
        && slice(bytes, ZOOM_LOAD_OFFSET, ZOOM_LOAD_PATCHED.len()) == Some(ZOOM_LOAD_PATCHED)
        && slice(bytes, ZOOM_SCRIPT_OFFSET, ZOOM_SCRIPT_PATCHED.len()) == Some(ZOOM_SCRIPT_PATCHED);
    let cave = slice(bytes, ZOOM_CAVE_OFFSET, 28);
    if calls_orig && cave == Some(ZOOM_CAVE_ORIGINAL) {
        ExeCameraZoomOutPatchState::Original
    } else if calls_patch && cave == Some(ZOOM_CAVE_LEGACY) {
        ExeCameraZoomOutPatchState::LegacyZoomHalf
    } else if calls_patch && cave == Some(ZOOM_CAVE_PATCHED) {
        ExeCameraZoomOutPatchState::Patched
    } else {
        ExeCameraZoomOutPatchState::Unknown
    }
}

pub fn get_village_setter_patch_state(bytes: &[u8]) -> ExeVillageSetterPatchState {
    let hook = slice(bytes, VILLAGE_HOOK_OFFSET, VILLAGE_HOOK_ORIGINAL.len());
    let cave = slice(bytes, VILLAGE_CAVE_OFFSET, VILLAGE_CAVE_ORIGINAL.len());
    if hook == Some(VILLAGE_HOOK_ORIGINAL) && cave == Some(VILLAGE_CAVE_ORIGINAL) {
        ExeVillageSetterPatchState::Original
    } else if hook == Some(VILLAGE_HOOK_PATCHED) && cave == Some(VILLAGE_CAVE_PATCHED) {
        ExeVillageSetterPatchState::EntireMap
    } else if hook == Some(VILLAGE_HOOK_PATCHED) && cave == Some(VILLAGE_CAVE_LEGACY_2X) {
        ExeVillageSetterPatchState::Legacy2x
    } else if hook == Some(VILLAGE_HOOK_PATCHED) && cave == Some(VILLAGE_CAVE_LEGACY_2_POINT_5X) {
        ExeVillageSetterPatchState::Legacy2Point5x
    } else if hook == Some(VILLAGE_HOOK_PATCHED) && cave == Some(VILLAGE_CAVE_LEGACY_3X) {
        ExeVillageSetterPatchState::Legacy3x
    } else if hook == Some(VILLAGE_HOOK_PATCHED) && cave == Some(VILLAGE_CAVE_LEGACY_5X) {
        ExeVillageSetterPatchState::Legacy5x
    } else {
        ExeVillageSetterPatchState::Unknown
    }
}

/// Mirrors ExePatchModel.GetVillageBuildRangePatchState (legacy four-site range patch).
pub fn get_village_build_range_patch_state(bytes: &[u8]) -> ExeVillageRangePatchState {
    if bytes.len() < VILLAGE_RANGE_PATCH_REQUIRED_LENGTH {
        return ExeVillageRangePatchState::Unknown;
    }
    let x = slice(bytes, VILLAGE_RANGE_X_OFFSET, 3);
    let z = slice(bytes, VILLAGE_RANGE_Z_OFFSET, 3);
    let fx = slice(bytes, VILLAGE_FRAME_X_OFFSET, 3);
    let fz = slice(bytes, VILLAGE_FRAME_Z_OFFSET, 3);
    if x.is_none() || z.is_none() || fx.is_none() || fz.is_none() {
        return ExeVillageRangePatchState::Unknown;
    }
    let range_orig = x == Some(VILLAGE_RANGE_X_ORIGINAL) && z == Some(VILLAGE_RANGE_Z_ORIGINAL);
    let range_patched = x == Some(VILLAGE_RANGE_X_PATCHED) && z == Some(VILLAGE_RANGE_Z_PATCHED);
    let frame_orig = fx == Some(VILLAGE_FRAME_X_ORIGINAL) && fz == Some(VILLAGE_FRAME_Z_ORIGINAL);
    let frame_patched = fx == Some(VILLAGE_FRAME_X_PATCHED) && fz == Some(VILLAGE_FRAME_Z_PATCHED);
    if range_orig && frame_orig {
        ExeVillageRangePatchState::Original
    } else if range_patched && frame_orig {
        ExeVillageRangePatchState::LegacyLogicOnly
    } else if range_patched && frame_patched {
        ExeVillageRangePatchState::Expanded
    } else {
        ExeVillageRangePatchState::Unknown
    }
}

/// Mirrors ExePatchModel.PlanVillageRangeRestore: (offset, original bytes) writes; empty unless a legacy patch is present.
pub fn plan_village_range_restore(state: ExeVillageRangePatchState) -> Vec<(usize, &'static [u8])> {
    if state != ExeVillageRangePatchState::Expanded
        && state != ExeVillageRangePatchState::LegacyLogicOnly
    {
        return Vec::new();
    }
    vec![
        (VILLAGE_RANGE_X_OFFSET, VILLAGE_RANGE_X_ORIGINAL),
        (VILLAGE_RANGE_Z_OFFSET, VILLAGE_RANGE_Z_ORIGINAL),
        (VILLAGE_FRAME_X_OFFSET, VILLAGE_FRAME_X_ORIGINAL),
        (VILLAGE_FRAME_Z_OFFSET, VILLAGE_FRAME_Z_ORIGINAL),
    ]
}

pub fn is_exe_original(bytes: &[u8]) -> bool {
    get_focus_patch_state(bytes) == ExePatchState::Original
        && get_spell_altar_patch_state(bytes) == ExeSpellAltarPatchState::Original
        && get_roman_endless_patch_state(bytes) == ExeRomanEndlessPatchState::Original
        && get_civi_produce_20_patch_state(bytes) == ExeCiviProduce20PatchState::Original
        && get_unit_recruit_20_patch_state(bytes) == ExeUnitRecruit20PatchState::Original
        && get_idle_select_999_patch_state(bytes) == ExeIdleSelect999PatchState::Original
        && get_corpse_retention_patch_state(bytes) == ExeCorpseRetentionPatchState::Original
        && matches!(get_game_speed_multiplier(bytes), 1 | 0)
}

fn write_bytes(output: &mut [u8], offset: usize, bytes: &[u8]) -> io::Result<()> {
    if offset + bytes.len() > output.len() {
        return Err(error("patch range out of bounds"));
    }
    output[offset..offset + bytes.len()].copy_from_slice(bytes);
    Ok(())
}

/// Mirrors the C# patcher: an explicitly requested feature whose byte pattern is unknown is rejected.
fn require_known(requested: bool, unknown: bool, feature: &str) -> io::Result<()> {
    if requested && unknown {
        return Err(error(format!(
            "Against_Rome.exe byte pattern for {feature} is unknown; refusing to patch"
        )));
    }
    Ok(())
}

pub fn apply_profile(bytes: &[u8], profile: &PatchProfile) -> io::Result<Vec<u8>> {
    let mut out = bytes.to_vec();

    // 1. Focus
    let focus = profile.get_bool("FocusLoss");
    let state = get_focus_patch_state(&out);
    require_known(focus, state == ExePatchState::Unknown, "FocusLoss")?;
    if focus && state == ExePatchState::Original {
        write_bytes(&mut out, FOCUS_OFFSET, FOCUS_PATCHED)?;
    } else if !focus && state == ExePatchState::FocusPatched {
        write_bytes(&mut out, FOCUS_OFFSET, FOCUS_ORIGINAL)?;
    }

    // C# ApplyCompat restores the retired four-site range patch before the setter.
    // Unknown/mixed legacy sites are left untouched unless VillageBuildRange is requested.
    let legacy_state = get_village_build_range_patch_state(&out);
    for (offset, original) in plan_village_range_restore(legacy_state) {
        write_bytes(&mut out, offset, original)?;
    }
    require_known(
        profile.get_bool("VillageBuildRange"),
        get_village_build_range_patch_state(&out) != ExeVillageRangePatchState::Original,
        "VillageBuildRange legacy range",
    )?;

    // Current village setter follows retired range cleanup, as in C#.
    let village = profile.get_bool("VillageBuildRange");
    let village_state = get_village_setter_patch_state(&out);
    require_known(
        village,
        village_state == ExeVillageSetterPatchState::Unknown,
        "VillageBuildRange",
    )?;
    if village
        && village_state != ExeVillageSetterPatchState::EntireMap
        && village_state != ExeVillageSetterPatchState::Unknown
    {
        write_bytes(&mut out, VILLAGE_HOOK_OFFSET, VILLAGE_HOOK_PATCHED)?;
        write_bytes(&mut out, VILLAGE_CAVE_OFFSET, VILLAGE_CAVE_PATCHED)?;
    } else if !village
        && village_state != ExeVillageSetterPatchState::Original
        && village_state != ExeVillageSetterPatchState::Unknown
    {
        write_bytes(&mut out, VILLAGE_HOOK_OFFSET, VILLAGE_HOOK_ORIGINAL)?;
        write_bytes(&mut out, VILLAGE_CAVE_OFFSET, VILLAGE_CAVE_ORIGINAL)?;
    }

    // 2. Spell Altar
    let no_altar = profile.get_bool("NoSpellAltar");
    let altar_state = get_spell_altar_patch_state(&out);
    require_known(
        no_altar,
        altar_state == ExeSpellAltarPatchState::Unknown,
        "NoSpellAltar",
    )?;
    if no_altar && altar_state == ExeSpellAltarPatchState::Original {
        for &(offset, _, patched) in SPELL_ALTAR_SITES {
            write_bytes(&mut out, offset, patched)?;
        }
    } else if !no_altar && altar_state == ExeSpellAltarPatchState::Patched {
        for &(offset, orig, _) in SPELL_ALTAR_SITES {
            write_bytes(&mut out, offset, orig)?;
        }
    }

    // 3. Roman Endless
    let roman = profile.get_bool("RomanEndless");
    let roman_state = get_roman_endless_patch_state(&out);
    require_known(
        roman,
        roman_state == ExeRomanEndlessPatchState::Unknown,
        "RomanEndless",
    )?;
    if roman && roman_state == ExeRomanEndlessPatchState::Original {
        write_bytes(&mut out, ROMAN_ENDLESS_OFFSET, ROMAN_ENDLESS_PATCHED)?;
    } else if !roman && roman_state == ExeRomanEndlessPatchState::Patched {
        write_bytes(&mut out, ROMAN_ENDLESS_OFFSET, ROMAN_ENDLESS_ORIGINAL)?;
    }

    // 4. Civi produce 20
    let civi20 = profile.get_bool("CiviProduce20");
    let civi_state = get_civi_produce_20_patch_state(&out);
    require_known(
        civi20,
        civi_state == ExeCiviProduce20PatchState::Unknown,
        "CiviProduce20",
    )?;
    if civi20 && civi_state == ExeCiviProduce20PatchState::Original {
        write_bytes(&mut out, CIVI_PRODUCE_20_OFFSET, CIVI_PRODUCE_20_PATCHED)?;
    } else if !civi20 && civi_state == ExeCiviProduce20PatchState::Patched {
        write_bytes(&mut out, CIVI_PRODUCE_20_OFFSET, CIVI_PRODUCE_20_ORIGINAL)?;
    }

    // 5. Unit recruit 20
    let recruit20 = profile.get_bool("UnitRecruit20");
    let recruit_state = get_unit_recruit_20_patch_state(&out);
    require_known(
        recruit20,
        recruit_state == ExeUnitRecruit20PatchState::Unknown,
        "UnitRecruit20",
    )?;
    if recruit20 && recruit_state == ExeUnitRecruit20PatchState::Original {
        write_bytes(&mut out, UNIT_RECRUIT_20_OFFSET, UNIT_RECRUIT_20_PATCHED)?;
    } else if !recruit20 && recruit_state == ExeUnitRecruit20PatchState::Patched {
        write_bytes(&mut out, UNIT_RECRUIT_20_OFFSET, UNIT_RECRUIT_20_ORIGINAL)?;
    }

    // 6. Idle select 999
    let idle999 = profile.get_bool("IdleSelect999");
    let idle_state = get_idle_select_999_patch_state(&out);
    require_known(
        idle999,
        idle_state == ExeIdleSelect999PatchState::Unknown,
        "IdleSelect999",
    )?;
    if idle999 && idle_state == ExeIdleSelect999PatchState::Original {
        write_bytes(&mut out, IDLE_SELECT_999_OFFSET, IDLE_SELECT_999_PATCHED)?;
    } else if !idle999 && idle_state == ExeIdleSelect999PatchState::Patched {
        write_bytes(&mut out, IDLE_SELECT_999_OFFSET, IDLE_SELECT_999_ORIGINAL)?;
    }

    // 7. Corpse retention
    let corpse = profile.get_bool("CorpseRetention");
    let corpse_state = get_corpse_retention_patch_state(&out);
    require_known(
        corpse,
        corpse_state == ExeCorpseRetentionPatchState::Unknown,
        "CorpseRetention",
    )?;
    if corpse && corpse_state == ExeCorpseRetentionPatchState::Original {
        write_bytes(&mut out, CORPSE_RETENTION_OFFSET, CORPSE_RETENTION_PATCHED)?;
    } else if !corpse && corpse_state == ExeCorpseRetentionPatchState::Patched {
        write_bytes(&mut out, CORPSE_RETENTION_OFFSET, CORPSE_RETENTION_ORIGINAL)?;
    }

    // 8. Restore retired special arrows if present
    if slice(&out, ARROWS_BTN_OFFSET, 1) == Some(ARROWS_BTN_PATCHED) {
        write_bytes(&mut out, ARROWS_BTN_OFFSET, ARROWS_BTN_ORIGINAL)?;
    }
    if slice(&out, ARROWS_GETTER_OFFSET, ARROWS_GETTER_PATCHED.len()) == Some(ARROWS_GETTER_PATCHED)
    {
        write_bytes(&mut out, ARROWS_GETTER_OFFSET, ARROWS_GETTER_ORIGINAL)?;
    }
    if slice(&out, ARROWS_CAVE_OFFSET, ARROWS_CAVE_PATCHED.len()) == Some(ARROWS_CAVE_PATCHED) {
        write_bytes(&mut out, ARROWS_CAVE_OFFSET, ARROWS_CAVE_ORIGINAL)?;
    }

    // 9. Native widescreen
    let ws = profile.get_bool("NativeWidescreen1920x1080");
    let ws_state = get_native_widescreen_patch_state(&out);
    require_known(
        ws,
        ws_state == ExeNativeWidescreenPatchState::Unknown,
        "NativeWidescreen1920x1080",
    )?;
    if ws
        && ws_state != ExeNativeWidescreenPatchState::Patched
        && ws_state != ExeNativeWidescreenPatchState::Unknown
    {
        write_bytes(&mut out, WS_ID_OFFSET, WS_ID_PATCHED)?;
        write_bytes(&mut out, WS_CREATE_OFFSET, WS_CREATE_PATCHED)?;
        write_bytes(&mut out, WS_TEXT_OFFSET, WS_TEXT_PATCHED)?;
        write_bytes(&mut out, WS_FORCE_OFFSET, WS_FORCE_PATCHED)?;
        write_bytes(&mut out, WS_ACTMODE_OFFSET, WS_ACTMODE_PATCHED)?;
        write_bytes(&mut out, WS_IGM_OFFSET, WS_IGM_PATCHED)?;
    } else if !ws
        && ws_state != ExeNativeWidescreenPatchState::Original
        && ws_state != ExeNativeWidescreenPatchState::Unknown
    {
        write_bytes(&mut out, WS_ID_OFFSET, WS_ID_ORIGINAL)?;
        write_bytes(&mut out, WS_CREATE_OFFSET, WS_CREATE_ORIGINAL)?;
        write_bytes(&mut out, WS_TEXT_OFFSET, WS_TEXT_ORIGINAL)?;
        write_bytes(&mut out, WS_FORCE_OFFSET, WS_FORCE_ORIGINAL)?;
        write_bytes(&mut out, WS_ACTMODE_OFFSET, WS_ACTMODE_ORIGINAL)?;
        write_bytes(&mut out, WS_IGM_OFFSET, WS_IGM_ORIGINAL)?;
    }

    // 10. Camera zoom out
    let zoom = profile.get_bool("CameraZoomOut1");
    let zoom_state = get_camera_zoom_out_patch_state(&out);
    require_known(
        zoom,
        zoom_state == ExeCameraZoomOutPatchState::Unknown,
        "CameraZoomOut1",
    )?;
    if zoom
        && zoom_state != ExeCameraZoomOutPatchState::Patched
        && zoom_state != ExeCameraZoomOutPatchState::Unknown
    {
        write_bytes(&mut out, ZOOM_INIT_OFFSET, ZOOM_INIT_PATCHED)?;
        write_bytes(&mut out, ZOOM_LOAD_OFFSET, ZOOM_LOAD_PATCHED)?;
        write_bytes(&mut out, ZOOM_SCRIPT_OFFSET, ZOOM_SCRIPT_PATCHED)?;
        write_bytes(&mut out, ZOOM_CAVE_OFFSET, ZOOM_CAVE_PATCHED)?;
    } else if !zoom
        && zoom_state != ExeCameraZoomOutPatchState::Original
        && zoom_state != ExeCameraZoomOutPatchState::Unknown
    {
        write_bytes(&mut out, ZOOM_INIT_OFFSET, ZOOM_INIT_ORIGINAL)?;
        write_bytes(&mut out, ZOOM_LOAD_OFFSET, ZOOM_LOAD_ORIGINAL)?;
        write_bytes(&mut out, ZOOM_SCRIPT_OFFSET, ZOOM_SCRIPT_ORIGINAL)?;
        write_bytes(&mut out, ZOOM_CAVE_OFFSET, ZOOM_CAVE_ORIGINAL)?;
    }

    // 12. Game speed
    // C# PlanGameSpeed: desired < 1 is treated as 1 (restore); no upper clamp.
    let speed = profile.get_int("GameSpeed").max(1);
    let cur = get_game_speed_multiplier(&out);
    if cur > 0 && cur != speed {
        let qpc = (GAME_SPEED_QPC_BASE * speed as f64).to_le_bytes();
        let tgt = (GAME_SPEED_TGT_BASE * speed as f64).to_le_bytes();
        write_bytes(&mut out, GAME_SPEED_QPC_OFFSET, &qpc)?;
        write_bytes(&mut out, GAME_SPEED_TGT_OFFSET, &tgt)?;
    }

    Ok(out)
}

/// C# ExeFeaturePatcher.Apply (compat portion) + ApplyNativeWidescreen + ApplyCameraZoomOut.
/// Reverts Focus, Legacy range, Village setter, Spell altar, Native widescreen, Camera zoom out,
/// and resets GameSpeed to 1.
/// If `keep_roman_endless` is true, RomanEndless is preserved in its current state.
pub fn restore_compat_exe(bytes: &[u8], keep_roman_endless: bool) -> io::Result<Vec<u8>> {
    let mut out = bytes.to_vec();

    // 1. Focus
    let state = get_focus_patch_state(&out);
    if state == ExePatchState::FocusPatched {
        write_bytes(&mut out, FOCUS_OFFSET, FOCUS_ORIGINAL)?;
    }

    // 2. Legacy village range & current village setter
    let legacy_state = get_village_build_range_patch_state(&out);
    for (offset, original) in plan_village_range_restore(legacy_state) {
        write_bytes(&mut out, offset, original)?;
    }
    let village_state = get_village_setter_patch_state(&out);
    if village_state != ExeVillageSetterPatchState::Original
        && village_state != ExeVillageSetterPatchState::Unknown
    {
        write_bytes(&mut out, VILLAGE_HOOK_OFFSET, VILLAGE_HOOK_ORIGINAL)?;
        write_bytes(&mut out, VILLAGE_CAVE_OFFSET, VILLAGE_CAVE_ORIGINAL)?;
    }

    // 3. Spell Altar
    let altar_state = get_spell_altar_patch_state(&out);
    if altar_state == ExeSpellAltarPatchState::Patched {
        for &(offset, orig, _) in SPELL_ALTAR_SITES {
            write_bytes(&mut out, offset, orig)?;
        }
    }

    // 4. Roman Endless
    if !keep_roman_endless {
        let roman_state = get_roman_endless_patch_state(&out);
        if roman_state == ExeRomanEndlessPatchState::Patched {
            write_bytes(&mut out, ROMAN_ENDLESS_OFFSET, ROMAN_ENDLESS_ORIGINAL)?;
        }
    }

    // 5. Game speed
    let cur = get_game_speed_multiplier(&out);
    if cur > 1 {
        let qpc = (GAME_SPEED_QPC_BASE * 1.0).to_le_bytes();
        let tgt = (GAME_SPEED_TGT_BASE * 1.0).to_le_bytes();
        write_bytes(&mut out, GAME_SPEED_QPC_OFFSET, &qpc)?;
        write_bytes(&mut out, GAME_SPEED_TGT_OFFSET, &tgt)?;
    }

    // 6. Native widescreen
    let ws_state = get_native_widescreen_patch_state(&out);
    if ws_state != ExeNativeWidescreenPatchState::Original
        && ws_state != ExeNativeWidescreenPatchState::Unknown
    {
        write_bytes(&mut out, WS_ID_OFFSET, WS_ID_ORIGINAL)?;
        write_bytes(&mut out, WS_CREATE_OFFSET, WS_CREATE_ORIGINAL)?;
        write_bytes(&mut out, WS_TEXT_OFFSET, WS_TEXT_ORIGINAL)?;
        write_bytes(&mut out, WS_FORCE_OFFSET, WS_FORCE_ORIGINAL)?;
        write_bytes(&mut out, WS_ACTMODE_OFFSET, WS_ACTMODE_ORIGINAL)?;
        write_bytes(&mut out, WS_IGM_OFFSET, WS_IGM_ORIGINAL)?;
    }

    // 7. Camera zoom out
    let zoom_state = get_camera_zoom_out_patch_state(&out);
    if zoom_state != ExeCameraZoomOutPatchState::Original
        && zoom_state != ExeCameraZoomOutPatchState::Unknown
    {
        write_bytes(&mut out, ZOOM_INIT_OFFSET, ZOOM_INIT_ORIGINAL)?;
        write_bytes(&mut out, ZOOM_LOAD_OFFSET, ZOOM_LOAD_ORIGINAL)?;
        write_bytes(&mut out, ZOOM_SCRIPT_OFFSET, ZOOM_SCRIPT_ORIGINAL)?;
        write_bytes(&mut out, ZOOM_CAVE_OFFSET, ZOOM_CAVE_ORIGINAL)?;
    }

    Ok(out)
}

/// C# ExeFeaturePatcher.ApplyRomanEndless(false) + ApplyCiviProduce20(false) + ApplyUnitRecruit20(false)
/// + ApplyIdleSelect999(false) + ApplyCorpseRetention(false) + RestoreRetiredDefaultSpecialArrows.
pub fn restore_stats_exe(bytes: &[u8]) -> io::Result<Vec<u8>> {
    let mut out = bytes.to_vec();

    // 1. Roman Endless
    let roman_state = get_roman_endless_patch_state(&out);
    if roman_state == ExeRomanEndlessPatchState::Patched {
        write_bytes(&mut out, ROMAN_ENDLESS_OFFSET, ROMAN_ENDLESS_ORIGINAL)?;
    }

    // 2. Civi produce 20
    let civi_state = get_civi_produce_20_patch_state(&out);
    if civi_state == ExeCiviProduce20PatchState::Patched {
        write_bytes(&mut out, CIVI_PRODUCE_20_OFFSET, CIVI_PRODUCE_20_ORIGINAL)?;
    }

    // 3. Unit recruit 20
    let recruit_state = get_unit_recruit_20_patch_state(&out);
    if recruit_state == ExeUnitRecruit20PatchState::Patched {
        write_bytes(&mut out, UNIT_RECRUIT_20_OFFSET, UNIT_RECRUIT_20_ORIGINAL)?;
    }

    // 4. Idle select 999
    let idle_state = get_idle_select_999_patch_state(&out);
    if idle_state == ExeIdleSelect999PatchState::Patched {
        write_bytes(&mut out, IDLE_SELECT_999_OFFSET, IDLE_SELECT_999_ORIGINAL)?;
    }

    // 5. Corpse retention
    let corpse_state = get_corpse_retention_patch_state(&out);
    if corpse_state == ExeCorpseRetentionPatchState::Patched {
        write_bytes(&mut out, CORPSE_RETENTION_OFFSET, CORPSE_RETENTION_ORIGINAL)?;
    }

    // 6. Restore retired special arrows if present
    if slice(&out, ARROWS_BTN_OFFSET, 1) == Some(ARROWS_BTN_PATCHED) {
        write_bytes(&mut out, ARROWS_BTN_OFFSET, ARROWS_BTN_ORIGINAL)?;
    }
    if slice(&out, ARROWS_GETTER_OFFSET, ARROWS_GETTER_PATCHED.len()) == Some(ARROWS_GETTER_PATCHED)
    {
        write_bytes(&mut out, ARROWS_GETTER_OFFSET, ARROWS_GETTER_ORIGINAL)?;
    }
    if slice(&out, ARROWS_CAVE_OFFSET, ARROWS_CAVE_PATCHED.len()) == Some(ARROWS_CAVE_PATCHED) {
        write_bytes(&mut out, ARROWS_CAVE_OFFSET, ARROWS_CAVE_ORIGINAL)?;
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::FeatureValue;

    /// Synthetic exe where every known site holds its Original bytes (game speed 1x).
    fn synthetic_original() -> Vec<u8> {
        let mut b = vec![0u8; 0x210000];
        let sites: Vec<(usize, &[u8])> = vec![
            (FOCUS_OFFSET, FOCUS_ORIGINAL),
            (ROMAN_ENDLESS_OFFSET, ROMAN_ENDLESS_ORIGINAL),
            (CIVI_PRODUCE_20_OFFSET, CIVI_PRODUCE_20_ORIGINAL),
            (UNIT_RECRUIT_20_OFFSET, UNIT_RECRUIT_20_ORIGINAL),
            (IDLE_SELECT_999_OFFSET, IDLE_SELECT_999_ORIGINAL),
            (CORPSE_RETENTION_OFFSET, CORPSE_RETENTION_ORIGINAL),
            (VILLAGE_RANGE_X_OFFSET, VILLAGE_RANGE_X_ORIGINAL),
            (VILLAGE_RANGE_Z_OFFSET, VILLAGE_RANGE_Z_ORIGINAL),
            (VILLAGE_FRAME_X_OFFSET, VILLAGE_FRAME_X_ORIGINAL),
            (VILLAGE_FRAME_Z_OFFSET, VILLAGE_FRAME_Z_ORIGINAL),
            (VILLAGE_HOOK_OFFSET, VILLAGE_HOOK_ORIGINAL),
            (VILLAGE_CAVE_OFFSET, VILLAGE_CAVE_ORIGINAL),
        ];
        for (o, s) in sites {
            b[o..o + s.len()].copy_from_slice(s);
        }
        for &(o, orig, _) in SPELL_ALTAR_SITES {
            b[o..o + orig.len()].copy_from_slice(orig);
        }
        b[GAME_SPEED_QPC_OFFSET..GAME_SPEED_QPC_OFFSET + 8]
            .copy_from_slice(&GAME_SPEED_QPC_BASE.to_le_bytes());
        b[GAME_SPEED_TGT_OFFSET..GAME_SPEED_TGT_OFFSET + 8]
            .copy_from_slice(&GAME_SPEED_TGT_BASE.to_le_bytes());
        b
    }

    fn profile(id: &str) -> PatchProfile {
        let mut p = PatchProfile::new();
        p.set(id, FeatureValue::Bool(true));
        p
    }

    // Parse the C# authority independently so incorrect Rust cave bytes cannot
    // make both the classifier and its fixtures agree on the same mistake.
    fn csharp_village_cave(name: &str) -> Vec<u8> {
        let source = include_str!("../../../src.Core/Core/Patches/ExePatchModel.cs");
        let declaration = format!("VillageSetterCave{name}Bytes = {{");
        let body = source
            .split_once(&declaration)
            .unwrap()
            .1
            .split_once("};")
            .unwrap()
            .0;
        let bytes: Vec<_> = body
            .lines()
            .flat_map(|line| {
                line.split("//")
                    .next()
                    .unwrap()
                    .split(',')
                    .filter_map(|token| {
                        token
                            .trim()
                            .strip_prefix("0x")
                            .map(|hex| u8::from_str_radix(hex, 16).unwrap())
                    })
            })
            .collect();
        assert_eq!(bytes.len(), 39);
        bytes
    }

    fn assert_exact_legacy_setter(name: &str, cave: &[u8], state: ExeVillageSetterPatchState) {
        assert_eq!(cave, csharp_village_cave(name));
        for mask in [0, 3, 15] {
            let mut bytes = legacy_fixture(mask);
            write_bytes(&mut bytes, VILLAGE_HOOK_OFFSET, VILLAGE_HOOK_PATCHED).unwrap();
            write_bytes(&mut bytes, VILLAGE_CAVE_OFFSET, cave).unwrap();
            assert_eq!(get_village_setter_patch_state(&bytes), state);
            let mut expected = legacy_fixture(0);
            write_bytes(&mut expected, VILLAGE_HOOK_OFFSET, VILLAGE_HOOK_PATCHED).unwrap();
            write_bytes(&mut expected, VILLAGE_CAVE_OFFSET, VILLAGE_CAVE_PATCHED).unwrap();
            let enabled = apply_profile(&bytes, &profile("VillageBuildRange")).unwrap();
            assert_eq!(enabled, expected, "{name}: exact migration after cleanup");
            assert_eq!(
                get_village_setter_patch_state(&enabled),
                ExeVillageSetterPatchState::EntireMap
            );
            assert_eq!(
                apply_profile(&bytes, &PatchProfile::new()).unwrap(),
                legacy_fixture(0),
                "{name}: exact restoration after cleanup"
            );
        }
    }

    #[test]
    fn village_setter_exact_legacy_2x() {
        assert_exact_legacy_setter(
            "Legacy2x",
            VILLAGE_CAVE_LEGACY_2X,
            ExeVillageSetterPatchState::Legacy2x,
        );
    }

    #[test]
    fn village_setter_exact_legacy_2_point_5x() {
        assert_exact_legacy_setter(
            "Legacy2Point5x",
            VILLAGE_CAVE_LEGACY_2_POINT_5X,
            ExeVillageSetterPatchState::Legacy2Point5x,
        );
    }

    #[test]
    fn village_setter_exact_legacy_3x() {
        assert_exact_legacy_setter(
            "Legacy3x",
            VILLAGE_CAVE_LEGACY_3X,
            ExeVillageSetterPatchState::Legacy3x,
        );
    }

    #[test]
    fn village_setter_exact_legacy_5x() {
        assert_exact_legacy_setter(
            "Legacy5x",
            VILLAGE_CAVE_LEGACY_5X,
            ExeVillageSetterPatchState::Legacy5x,
        );
    }

    const KNOWN_SETTER_CAVES: [&[u8]; 5] = [
        VILLAGE_CAVE_LEGACY_2X,
        VILLAGE_CAVE_LEGACY_2_POINT_5X,
        VILLAGE_CAVE_LEGACY_3X,
        VILLAGE_CAVE_LEGACY_5X,
        VILLAGE_CAVE_PATCHED,
    ];

    fn assert_unknown_setter_safety(bytes: &[u8]) {
        assert_eq!(
            get_village_setter_patch_state(bytes),
            ExeVillageSetterPatchState::Unknown
        );
        let before = bytes.to_vec();
        let mut enabled = profile("VillageBuildRange");
        enabled.set("FocusLoss", FeatureValue::Bool(true));
        let err = apply_profile(bytes, &enabled).unwrap_err();
        assert!(err.to_string().contains("VillageBuildRange"), "{err}");
        assert_eq!(
            bytes, before,
            "error must preserve input, including retired range sites"
        );
        let mut expected = before;
        for (offset, original) in
            plan_village_range_restore(get_village_build_range_patch_state(bytes))
        {
            write_bytes(&mut expected, offset, original).unwrap();
        }
        let mut disabled = PatchProfile::new();
        disabled.set("VillageBuildRange", FeatureValue::Bool(false));
        for requested in [&disabled, &PatchProfile::new()] {
            let out = apply_profile(bytes, requested).unwrap();
            assert_eq!(
                out, expected,
                "only retired range cleanup may change unknown setter input"
            );
            assert_eq!(
                &out[VILLAGE_HOOK_OFFSET..VILLAGE_HOOK_OFFSET + VILLAGE_HOOK_ORIGINAL.len()],
                &bytes[VILLAGE_HOOK_OFFSET..VILLAGE_HOOK_OFFSET + VILLAGE_HOOK_ORIGINAL.len()]
            );
            assert_eq!(
                &out[VILLAGE_CAVE_OFFSET..],
                &bytes[VILLAGE_CAVE_OFFSET..],
                "preserve the entire unknown cave, including any trailing data"
            );
        }
    }

    #[test]
    fn village_setter_unknown_and_corrupt_caves_are_safe() {
        for mask in [0, 3, 15] {
            let mut bytes = legacy_fixture(mask);
            write_bytes(&mut bytes, VILLAGE_HOOK_OFFSET, VILLAGE_HOOK_PATCHED).unwrap();
            // A patched hook with an original/empty or arbitrary cave is unknown.
            assert_unknown_setter_safety(&bytes);
            bytes[VILLAGE_CAVE_OFFSET..VILLAGE_CAVE_OFFSET + 39].fill(0xCC);
            assert_unknown_setter_safety(&bytes);
            for cave in KNOWN_SETTER_CAVES {
                // Every cave byte matters: guards, operands, branch targets and padding.
                for i in 0..39 {
                    write_bytes(&mut bytes, VILLAGE_CAVE_OFFSET, cave).unwrap();
                    bytes[VILLAGE_CAVE_OFFSET + i] ^= 0xFF;
                    assert_unknown_setter_safety(&bytes);
                }
                // An exact cave still requires the exact patched hook.
                write_bytes(&mut bytes, VILLAGE_CAVE_OFFSET, cave).unwrap();
                write_bytes(&mut bytes, VILLAGE_HOOK_OFFSET, VILLAGE_HOOK_ORIGINAL).unwrap();
                assert_unknown_setter_safety(&bytes);
                for i in 0..VILLAGE_HOOK_PATCHED.len() {
                    write_bytes(&mut bytes, VILLAGE_HOOK_OFFSET, VILLAGE_HOOK_PATCHED).unwrap();
                    bytes[VILLAGE_HOOK_OFFSET + i] ^= 0xFF;
                    assert_unknown_setter_safety(&bytes);
                }
                write_bytes(&mut bytes, VILLAGE_HOOK_OFFSET, VILLAGE_HOOK_PATCHED).unwrap();
            }
        }
    }

    #[test]
    fn village_setter_mixed_caves_are_unknown() {
        let mut cases = 0;
        for left in KNOWN_SETTER_CAVES {
            for right in KNOWN_SETTER_CAVES {
                for split in 1..39 {
                    let mut cave = left.to_vec();
                    cave[split..].copy_from_slice(&right[split..]);
                    if KNOWN_SETTER_CAVES.contains(&cave.as_slice()) {
                        continue;
                    }
                    for mask in [0, 3, 15] {
                        let mut bytes = legacy_fixture(mask);
                        write_bytes(&mut bytes, VILLAGE_HOOK_OFFSET, VILLAGE_HOOK_PATCHED).unwrap();
                        write_bytes(&mut bytes, VILLAGE_CAVE_OFFSET, &cave).unwrap();
                        assert_unknown_setter_safety(&bytes);
                    }
                    cases += 1;
                }
            }
        }
        assert!(
            cases > 0,
            "exercise actual mixed versions, excluding exact recognized caves"
        );
    }

    #[test]
    fn village_setter_truncated_caves_are_unknown() {
        for cave in KNOWN_SETTER_CAVES {
            for mask in [0, 3, 15] {
                let mut bytes = legacy_fixture(mask);
                write_bytes(&mut bytes, VILLAGE_HOOK_OFFSET, VILLAGE_HOOK_PATCHED).unwrap();
                write_bytes(&mut bytes, VILLAGE_CAVE_OFFSET, cave).unwrap();
                for len in 0..39 {
                    assert_unknown_setter_safety(&bytes[..VILLAGE_CAVE_OFFSET + len]);
                }
            }
        }
    }

    // Literal sites from the C# authority, independent of the Rust restore plan.
    const LEGACY_SITES: [(usize, &[u8], &[u8]); 4] = [
        (0x1366c4, &[0xC1, 0xE2, 0x06], &[0xC1, 0xE2, 0x07]),
        (0x1366cd, &[0xC1, 0xE1, 0x06], &[0xC1, 0xE1, 0x07]),
        (0x0d722c, &[0xC1, 0xE6, 0x06], &[0xC1, 0xE6, 0x07]),
        (0x0d723b, &[0xC1, 0xE7, 0x06], &[0xC1, 0xE7, 0x07]),
    ];

    fn legacy_fixture(mask: u8) -> Vec<u8> {
        let mut bytes = synthetic_original();
        for (i, &(offset, original, patched)) in LEGACY_SITES.iter().enumerate() {
            write_bytes(
                &mut bytes,
                offset,
                if mask & (1 << i) != 0 {
                    patched
                } else {
                    original
                },
            )
            .unwrap();
        }
        bytes
    }

    #[test]
    fn village_range_original_is_unchanged() {
        let bytes = legacy_fixture(0);
        assert_eq!(
            get_village_build_range_patch_state(&bytes),
            ExeVillageRangePatchState::Original
        );
        assert!(plan_village_range_restore(ExeVillageRangePatchState::Original).is_empty());
        assert_eq!(apply_profile(&bytes, &PatchProfile::new()).unwrap(), bytes);
    }

    fn assert_legacy_restore(mask: u8, state: ExeVillageRangePatchState) {
        let bytes = legacy_fixture(mask);
        assert_eq!(get_village_build_range_patch_state(&bytes), state);
        let expected_plan: Vec<_> = LEGACY_SITES
            .iter()
            .map(|&(o, original, _)| (o, original))
            .collect();
        assert_eq!(
            plan_village_range_restore(state),
            expected_plan,
            "all four writes in C# order"
        );
        let out = apply_profile(&bytes, &PatchProfile::new()).unwrap();
        assert_eq!(
            get_village_build_range_patch_state(&out),
            ExeVillageRangePatchState::Original
        );
        assert_eq!(out, legacy_fixture(0));
        // Check every byte, including padding and the new setter hook/cave.
        let changed: Vec<_> = bytes
            .iter()
            .zip(&out)
            .enumerate()
            .filter_map(|(i, (before, after))| (before != after).then_some(i))
            .collect();
        let expected_changes: Vec<_> = LEGACY_SITES
            .iter()
            .enumerate()
            .filter_map(|(i, &(offset, _, _))| (mask & (1 << i) != 0).then_some(offset + 2))
            .collect();
        let mut expected_changes = expected_changes;
        expected_changes.sort_unstable();
        assert_eq!(
            changed, expected_changes,
            "only the legacy immediates may change"
        );
    }

    #[test]
    fn village_range_legacy_logic_only_restores_exact_four_site_plan() {
        assert_legacy_restore(3, ExeVillageRangePatchState::LegacyLogicOnly);
    }

    #[test]
    fn village_range_expanded_restores_exact_four_site_plan() {
        assert_legacy_restore(15, ExeVillageRangePatchState::Expanded);
    }

    fn assert_unknown_legacy_safety(bytes: &[u8]) {
        assert_eq!(
            get_village_build_range_patch_state(bytes),
            ExeVillageRangePatchState::Unknown
        );
        assert!(plan_village_range_restore(ExeVillageRangePatchState::Unknown).is_empty());
        assert_eq!(apply_profile(bytes, &PatchProfile::new()).unwrap(), bytes);
        let mut disabled = PatchProfile::new();
        disabled.set("VillageBuildRange", FeatureValue::Bool(false));
        assert_eq!(apply_profile(bytes, &disabled).unwrap(), bytes);
        let before = bytes.to_vec();
        let mut requested = profile("VillageBuildRange");
        if get_focus_patch_state(bytes) == ExePatchState::Original {
            requested.set("FocusLoss", FeatureValue::Bool(true));
        }
        let err = apply_profile(bytes, &requested).unwrap_err();
        assert!(err.to_string().contains("VillageBuildRange"), "{err}");
        assert_eq!(bytes, before, "failed application must preserve its input");
    }

    #[test]
    fn village_range_all_mixed_combinations_are_safe() {
        // All 13 unrecognized combinations, including original logic/patched frames.
        for mask in 0..16 {
            if ![0, 3, 15].contains(&mask) {
                assert_unknown_legacy_safety(&legacy_fixture(mask));
            }
        }
    }

    #[test]
    fn village_range_corrupt_and_truncated_sites_are_safe() {
        for mask in [0, 3, 15] {
            for &(offset, _, _) in &LEGACY_SITES {
                let mut bytes = legacy_fixture(mask);
                bytes[offset] ^= 0xFF;
                assert_unknown_legacy_safety(&bytes);
            }
        }
        for len in [
            0,
            VILLAGE_FRAME_X_OFFSET + 2,
            VILLAGE_RANGE_PATCH_REQUIRED_LENGTH - 1,
        ] {
            assert_unknown_legacy_safety(&legacy_fixture(0)[..len]);
        }
        let bytes = legacy_fixture(0);
        assert_eq!(
            get_village_build_range_patch_state(&bytes[..VILLAGE_RANGE_PATCH_REQUIRED_LENGTH]),
            ExeVillageRangePatchState::Original
        );
    }

    #[test]
    fn village_setter_applies_after_legacy_cleanup() {
        for mask in [0, 3, 15] {
            let bytes = legacy_fixture(mask);
            let out = apply_profile(&bytes, &profile("VillageBuildRange")).unwrap();
            let mut expected = legacy_fixture(0);
            write_bytes(&mut expected, VILLAGE_HOOK_OFFSET, VILLAGE_HOOK_PATCHED).unwrap();
            write_bytes(&mut expected, VILLAGE_CAVE_OFFSET, VILLAGE_CAVE_PATCHED).unwrap();
            assert_eq!(
                out, expected,
                "mask {mask}: cleanup plus only setter hook/cave changes"
            );
            assert_eq!(
                get_village_setter_patch_state(&out),
                ExeVillageSetterPatchState::EntireMap
            );
        }
    }

    #[test]
    fn village_range_repeated_application_is_idempotent() {
        for mask in [0, 3, 15] {
            for enabled in [false, true] {
                let mut requested = PatchProfile::new();
                requested.set("VillageBuildRange", FeatureValue::Bool(enabled));
                let once = apply_profile(&legacy_fixture(mask), &requested).unwrap();
                assert_eq!(apply_profile(&once, &requested).unwrap(), once);
                let disabled = apply_profile(&once, &PatchProfile::new()).unwrap();
                assert_eq!(disabled, legacy_fixture(0));
                assert_eq!(
                    apply_profile(&disabled, &PatchProfile::new()).unwrap(),
                    disabled
                );
            }
        }
    }

    #[test]
    fn village_cleanup_with_unknown_setter_only_fails_when_requested() {
        let mut bytes = legacy_fixture(15);
        bytes[VILLAGE_HOOK_OFFSET] ^= 0xFF;
        assert_eq!(
            get_village_setter_patch_state(&bytes),
            ExeVillageSetterPatchState::Unknown
        );
        let out = apply_profile(&bytes, &PatchProfile::new()).unwrap();
        let mut expected = legacy_fixture(0);
        expected[VILLAGE_HOOK_OFFSET] ^= 0xFF;
        assert_eq!(out, expected);
        let before = bytes.clone();
        let err = apply_profile(&bytes, &profile("VillageBuildRange")).unwrap_err();
        assert!(err.to_string().contains("VillageBuildRange"), "{err}");
        assert_eq!(
            bytes, before,
            "setter failure must not return partial legacy cleanup"
        );
    }

    #[test]
    fn village_cleanup_preserves_unknown_game_speed_skip() {
        let mut bytes = legacy_fixture(15);
        bytes[GAME_SPEED_QPC_OFFSET..GAME_SPEED_QPC_OFFSET + 8].fill(0xFF);
        assert_eq!(get_game_speed_multiplier(&bytes), 0);
        let mut requested = profile("VillageBuildRange");
        requested.set("GameSpeed", FeatureValue::Int(2));
        let out = apply_profile(&bytes, &requested).unwrap();
        assert_eq!(
            &out[GAME_SPEED_QPC_OFFSET..GAME_SPEED_QPC_OFFSET + 8],
            &bytes[GAME_SPEED_QPC_OFFSET..GAME_SPEED_QPC_OFFSET + 8]
        );
        assert_eq!(
            &out[GAME_SPEED_TGT_OFFSET..GAME_SPEED_TGT_OFFSET + 8],
            &bytes[GAME_SPEED_TGT_OFFSET..GAME_SPEED_TGT_OFFSET + 8]
        );
        assert_eq!(
            get_village_build_range_patch_state(&out),
            ExeVillageRangePatchState::Original
        );
        assert_eq!(
            get_village_setter_patch_state(&out),
            ExeVillageSetterPatchState::EntireMap
        );
    }

    #[test]
    fn requested_unknown_feature_errors_and_leaves_input_unchanged() {
        let mut bytes = synthetic_original();
        bytes[CORPSE_RETENTION_OFFSET] ^= 0xFF; // corpse retention -> Unknown
        assert_eq!(
            get_corpse_retention_patch_state(&bytes),
            ExeCorpseRetentionPatchState::Unknown
        );
        let before = bytes.clone();

        let mut p = profile("CorpseRetention");
        p.set("FocusLoss", FeatureValue::Bool(true)); // a valid request must not yield a partial result
        let err = apply_profile(&bytes, &p).expect_err("unknown requested feature must fail");
        assert!(err.to_string().contains("CorpseRetention"), "{err}");
        assert_eq!(bytes, before, "input buffer must not change");
    }

    #[test]
    fn unknown_feature_not_requested_is_ignored() {
        let mut bytes = synthetic_original();
        bytes[CORPSE_RETENTION_OFFSET] ^= 0xFF;
        let out =
            apply_profile(&bytes, &profile("FocusLoss")).expect("unrequested unknown is fine");
        assert_eq!(&out[FOCUS_OFFSET..FOCUS_OFFSET + 6], FOCUS_PATCHED);
    }

    #[test]
    fn known_original_feature_applies() {
        let bytes = synthetic_original();
        let out = apply_profile(&bytes, &profile("CorpseRetention")).expect("original applies");
        assert_eq!(
            &out[CORPSE_RETENTION_OFFSET..CORPSE_RETENTION_OFFSET + 5],
            CORPSE_RETENTION_PATCHED
        );
    }

    #[test]
    fn every_requested_unknown_exe_feature_fails_closed() {
        let cases = [
            ("FocusLoss", FOCUS_OFFSET),
            ("NoSpellAltar", SPELL_ALTAR_SITES[0].0),
            ("RomanEndless", ROMAN_ENDLESS_OFFSET),
            ("CiviProduce20", CIVI_PRODUCE_20_OFFSET),
            ("UnitRecruit20", UNIT_RECRUIT_20_OFFSET),
            ("IdleSelect999", IDLE_SELECT_999_OFFSET),
            ("CorpseRetention", CORPSE_RETENTION_OFFSET),
            ("NativeWidescreen1920x1080", WS_ID_OFFSET),
            ("CameraZoomOut1", ZOOM_INIT_OFFSET),
            ("VillageBuildRange", VILLAGE_HOOK_OFFSET),
        ];

        for (id, offset) in cases {
            let mut bytes = synthetic_original();
            bytes[offset] ^= 0xFF;
            let before = bytes.clone();
            let mut requested = PatchProfile::new();
            requested.set(id, FeatureValue::Bool(true));

            let err = match apply_profile(&bytes, &requested) {
                Err(err) => err,
                Ok(_) => panic!("{id} must reject an unknown byte pattern"),
            };
            assert!(err.to_string().contains(id), "{id}: {err}");
            assert_eq!(bytes, before, "{id} failure must preserve input bytes");
        }
    }

    fn set_speed(bytes: &mut [u8], m: f64) {
        bytes[GAME_SPEED_QPC_OFFSET..GAME_SPEED_QPC_OFFSET + 8]
            .copy_from_slice(&(GAME_SPEED_QPC_BASE * m).to_le_bytes());
        bytes[GAME_SPEED_TGT_OFFSET..GAME_SPEED_TGT_OFFSET + 8]
            .copy_from_slice(&(GAME_SPEED_TGT_BASE * m).to_le_bytes());
    }

    #[test]
    fn game_speed_detection_matches_csharp_bounds() {
        for (m, want) in [
            (1.0, 1),
            (10.0, 10),
            (11.0, 11),
            (0.5, 0),
            (0.0, 0),
            (2.00001, 0),
            (2.0000001, 2),
        ] {
            let mut b = synthetic_original();
            set_speed(&mut b, m);
            assert_eq!(get_game_speed_multiplier(&b), want, "multiplier {m}");
        }
        let mut b = synthetic_original();
        b[GAME_SPEED_TGT_OFFSET..GAME_SPEED_TGT_OFFSET + 8]
            .copy_from_slice(&(GAME_SPEED_TGT_BASE * 3.0).to_le_bytes());
        assert_eq!(
            get_game_speed_multiplier(&b),
            0,
            "qpc/tgt mismatch is Unknown"
        );
    }

    #[test]
    fn game_speed_above_ten_is_restored_and_below_one_means_restore() {
        let mut b = synthetic_original();
        set_speed(&mut b, 11.0);
        let out = apply_profile(&b, &PatchProfile::new()).unwrap();
        assert_eq!(get_game_speed_multiplier(&out), 1);
        let mut p = PatchProfile::new();
        p.set("GameSpeed", FeatureValue::Int(0));
        let mut b = synthetic_original();
        set_speed(&mut b, 4.0);
        assert_eq!(
            get_game_speed_multiplier(&apply_profile(&b, &p).unwrap()),
            1
        );
        let restored = restore_compat_exe(
            &{
                let mut b = synthetic_original();
                set_speed(&mut b, 11.0);
                b
            },
            false,
        )
        .unwrap();
        assert_eq!(get_game_speed_multiplier(&restored), 1);
    }
}
