//! Unit icon loading, ported from `ModifierForm.Data.cs:LoadIcons`.
//!
//! `SYSTEM/CLMK/icon.ini` (PFIL-compressed, Windows-1251, taken from the in-memory backup) maps
//! `Fig*` unit keys to TGA file names; the TGA files live in `gui.dat` (a ZIP) under
//! `SYSTEM/CLMK/DLG/IGM0806/`, `US/` preferred and any other language folder as fallback.
//! Nothing here writes to disk. Missing inputs degrade to an empty icon set plus a log event
//! whose localization key matches the C# message (`LogGuiDatNotFound` / `LogIconIniNotFound` ...).
use crate::{lzss, patch_text, tga};
use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::Read,
    path::Path,
};

/// Backup key of the icon table.
pub const ICON_INI_KEY: &str = "SYSTEM/CLMK/icon.ini";
const ICON_DIR: &str = "SYSTEM/CLMK/DLG/IGM0806/";
const ICON_DIR_US: &str = "SYSTEM/CLMK/DLG/IGM0806/US/";
/// Defensive cap for one TGA entry read out of the archive.
const MAX_ICON_ENTRY_BYTES: u64 = 16 * 1024 * 1024;

/// Straight (non-premultiplied) RGBA8 image, `rgba.len() == width * height * 4`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitIcon {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// Mirrors the C# `Log(Loc.Get(...))` calls so the UI can localize them with `Loc::get`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IconLog {
    /// `LogGamePathNotSetIcon`
    GamePathNotSet,
    /// `LogGuiDatNotFound`
    GuiDatNotFound,
    /// `LogIconIniNotFound`
    IconIniNotFound,
    /// `LogIconsLoaded` with `{0}` = count
    Loaded(usize),
    /// `LogLoadIconFailed` followed by the error text
    Failed(String),
}

impl IconLog {
    /// Localization key in the `app_localization` tables.
    pub fn key(&self) -> &'static str {
        match self {
            IconLog::GamePathNotSet => "LogGamePathNotSetIcon",
            IconLog::GuiDatNotFound => "LogGuiDatNotFound",
            IconLog::IconIniNotFound => "LogIconIniNotFound",
            IconLog::Loaded(_) => "LogIconsLoaded",
            IconLog::Failed(_) => "LogLoadIconFailed",
        }
    }
    /// Message built from a localizer (`loc(key)` returns the template for that key).
    pub fn format(&self, loc: impl Fn(&str) -> String) -> String {
        let template = loc(self.key());
        match self {
            IconLog::Loaded(n) => template.replace("{0}", &n.to_string()),
            IconLog::Failed(e) => format!("{template}{e}"),
            _ => template,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnitIcons {
    /// Keyed by the unit key exactly as written in icon.ini.
    pub icons: BTreeMap<String, UnitIcon>,
    pub log: Vec<IconLog>,
}

impl UnitIcons {
    /// Case-insensitive lookup (unit keys are treated that way elsewhere in the editor).
    pub fn get(&self, key: &str) -> Option<&UnitIcon> {
        self.icons.get(key).or_else(|| {
            self.icons
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v)
        })
    }
    pub fn len(&self) -> usize {
        self.icons.len()
    }
    pub fn is_empty(&self) -> bool {
        self.icons.is_empty()
    }
}

/// Parses decompressed icon.ini text: lines starting with `Fig` that contain a comma give
/// `unit key -> tga file name` (trimmed; the last duplicate wins, like the C# dictionary).
pub fn parse_icon_ini(decompressed: &[u8]) -> HashMap<String, String> {
    let text = patch_text::decode_cp1251(decompressed);
    let mut map = HashMap::new();
    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() || !line.starts_with("Fig") || !line.contains(',') {
            continue;
        }
        let parts = patch_text::parse_csv_line(line);
        if parts.len() >= 2 {
            map.insert(parts[0].trim().to_owned(), parts[1].trim().to_owned());
        }
    }
    map
}

/// Picks the archive entry used for each icon file name (lower-cased): `US/` wins, otherwise the
/// first entry seen under any other language folder. Returns `lowercase file name -> entry index`.
fn index_icon_entries<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> HashMap<String, usize> {
    let mut index: HashMap<String, usize> = HashMap::new();
    for i in 0..archive.len() {
        let Ok(entry) = archive.by_index_raw(i) else {
            continue;
        };
        let full = entry.name().replace('\\', "/");
        let upper = full.to_ascii_uppercase();
        if entry.is_dir() || !upper.starts_with(ICON_DIR) {
            continue;
        }
        let name = full.rsplit('/').next().unwrap_or("");
        if name.is_empty() {
            continue;
        }
        let is_us = upper.starts_with(ICON_DIR_US);
        let key = name.to_ascii_lowercase();
        if is_us || !index.contains_key(&key) {
            index.insert(key, i);
        }
    }
    index
}

fn tga_to_icon(bytes: &[u8]) -> Option<UnitIcon> {
    let image = tga::decode(bytes)?;
    let mut rgba = image.bgra;
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    Some(UnitIcon {
        width: image.width,
        height: image.height,
        rgba,
    })
}

/// Core: PFIL icon.ini bytes + a `gui.dat` ZIP reader -> icons. Errors become `IconLog::Failed`.
pub fn load_unit_icons_from_parts<R: Read + std::io::Seek>(
    ini_pfil: &[u8],
    gui_dat: R,
) -> UnitIcons {
    let mut out = UnitIcons::default();
    let result = (|| -> Result<(), String> {
        let ini = lzss::decompress_pfil(ini_pfil).map_err(|e| e.to_string())?;
        let unit_to_tga = parse_icon_ini(&ini);
        let mut archive = zip::ZipArchive::new(gui_dat).map_err(|e| e.to_string())?;
        let entries = index_icon_entries(&mut archive);
        for (unit, file) in &unit_to_tga {
            let Some(&i) = entries.get(&file.to_ascii_lowercase()) else {
                continue;
            };
            let Ok(entry) = archive.by_index(i) else {
                continue;
            };
            if entry.size() > MAX_ICON_ENTRY_BYTES {
                continue;
            }
            let mut buf = Vec::new();
            if entry
                .take(MAX_ICON_ENTRY_BYTES)
                .read_to_end(&mut buf)
                .is_err()
            {
                continue;
            }
            if let Some(icon) = tga_to_icon(&buf) {
                out.icons.insert(unit.clone(), icon);
            }
        }
        Ok(())
    })();
    match result {
        Ok(()) => out.log.push(IconLog::Loaded(out.icons.len())),
        Err(e) => {
            out.icons.clear();
            out.log.push(IconLog::Failed(e));
        }
    }
    out
}

/// Full C# `LoadIcons`: `game` is the game directory (None / missing -> `GamePathNotSet`),
/// `ini_pfil` the backup's `SYSTEM/CLMK/icon.ini` bytes if present.
pub fn load_unit_icons(game: Option<&Path>, ini_pfil: Option<&[u8]>) -> UnitIcons {
    let mut out = UnitIcons::default();
    let Some(game) = game.filter(|g| g.is_dir()) else {
        out.log.push(IconLog::GamePathNotSet);
        return out;
    };
    let gui_dat = game.join("gui.dat");
    if !gui_dat.is_file() {
        out.log.push(IconLog::GuiDatNotFound);
        return out;
    }
    let Some(ini) = ini_pfil else {
        out.log.push(IconLog::IconIniNotFound);
        return out;
    };
    match File::open(&gui_dat) {
        Ok(f) => load_unit_icons_from_parts(ini, f),
        Err(e) => {
            out.log.push(IconLog::Failed(e.to_string()));
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};

    fn tga32(width: u16, height: u16, bgra: &[[u8; 4]]) -> Vec<u8> {
        let mut v = vec![0u8; 18];
        v[2] = 2;
        v[12..14].copy_from_slice(&width.to_le_bytes());
        v[14..16].copy_from_slice(&height.to_le_bytes());
        v[16] = 32;
        v[17] = 0x20 | 8; // top-to-bottom
        for px in bgra {
            v.extend_from_slice(px);
        }
        v
    }

    fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            w.start_file(*name, opts).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    fn pfil(text: &str) -> Vec<u8> {
        lzss::compress_pfil(text.as_bytes(), None).unwrap()
    }

    #[test]
    fn parses_fig_lines_only() {
        let m = parse_icon_ini(
            b"; comment\r\nFigA, a.tga \r\nOther,b.tga\r\nFigNoComma\r\n\r\nFigB,b.tga,extra\nFigA,a2.tga\n",
        );
        assert_eq!(m.len(), 2);
        assert_eq!(m["FigA"], "a2.tga");
        assert_eq!(m["FigB"], "b.tga");
    }

    #[test]
    fn loads_icons_with_us_priority_and_rgba_swizzle() {
        let us = tga32(2, 1, &[[1, 2, 3, 255], [10, 20, 30, 128]]);
        let other = tga32(1, 1, &[[9, 9, 9, 255]]);
        let zip = zip_of(&[
            ("SYSTEM/CLMK/DLG/IGM0806/DE/a.tga", &other),
            ("SYSTEM/CLMK/DLG/IGM0806/US/A.TGA", &us),
            ("SYSTEM/CLMK/DLG/IGM0806/FR/b.tga", &other),
            ("SYSTEM/CLMK/DLG/IGM0806/US/", b""),
            ("SYSTEM/CLMK/DLG/OTHER/c.tga", &other),
        ]);
        let ini = pfil("FigA,a.tga\r\nFigB,b.tga\r\nFigC,c.tga\r\nFigD,missing.tga\r\n");
        let icons = load_unit_icons_from_parts(&ini, Cursor::new(zip));
        assert_eq!(icons.log, vec![IconLog::Loaded(2)]);
        let a = icons.get("figa").expect("case-insensitive lookup");
        assert_eq!((a.width, a.height), (2, 1));
        assert_eq!(a.rgba, vec![3, 2, 1, 255, 30, 20, 10, 128]);
        assert_eq!(icons.get("FigB").unwrap().width, 1);
        assert!(icons.get("FigC").is_none() && icons.get("FigD").is_none());
    }

    #[test]
    fn bad_ini_reports_failure_and_no_icons() {
        let icons = load_unit_icons_from_parts(b"not pfil", Cursor::new(zip_of(&[])));
        assert!(icons.is_empty());
        assert!(matches!(icons.log.as_slice(), [IconLog::Failed(_)]));
        let icons =
            load_unit_icons_from_parts(&pfil("FigA,a.tga\n"), Cursor::new(b"nozip".to_vec()));
        assert!(matches!(icons.log.as_slice(), [IconLog::Failed(_)]));
    }

    #[test]
    fn missing_inputs_degrade_gracefully() {
        let none = load_unit_icons(None, Some(b"x"));
        assert_eq!(none.log, vec![IconLog::GamePathNotSet]);
        let tmp = crate::icons::temp_fixture::dir("ar_icons");
        std::fs::create_dir_all(&tmp).unwrap();
        let no_gui = load_unit_icons(Some(&tmp), Some(b"x"));
        assert_eq!(no_gui.log, vec![IconLog::GuiDatNotFound]);
        assert_eq!(no_gui.log[0].key(), "LogGuiDatNotFound");
        std::fs::write(tmp.join("gui.dat"), zip_of(&[])).unwrap();
        let no_ini = load_unit_icons(Some(&tmp), None);
        assert_eq!(no_ini.log, vec![IconLog::IconIniNotFound]);
        assert_eq!(no_ini.log[0].key(), "LogIconIniNotFound");
        let ok = load_unit_icons(Some(&tmp), Some(&pfil("FigA,a.tga\n")));
        assert_eq!(ok.log, vec![IconLog::Loaded(0)]);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn log_format_substitutes_count_and_appends_error() {
        let loc = |k: &str| match k {
            "LogIconsLoaded" => "Loaded {0} icons.".to_owned(),
            "LogLoadIconFailed" => "Failed: ".to_owned(),
            other => other.to_owned(),
        };
        assert_eq!(IconLog::Loaded(3).format(loc), "Loaded 3 icons.");
        assert_eq!(IconLog::Failed("boom".into()).format(loc), "Failed: boom");
    }
}

#[cfg(test)]
mod temp_fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/temp_unique.rs"));
}
