//! Portable `.arm-layout.json` layouts (C# `MapLayoutPresets` / `MapEditorForm.Layouts.cs`).
//!
//! The serializer mirrors `System.Text.Json` with `WriteIndented`, camelCase names, a string enum
//! for `kind`, `UnmappedMemberHandling.Disallow` and shortest round-trip float text, so files are
//! interchangeable with the C# editor. Everything here is pure (no egui, no globals) so it is
//! unit-testable; the file pickers are injectable like the other panels.

use crate::design_apply::PlacementRequest;
use crate::placement_ui::TileRect;
pub use against_rome_environment::settlement::{MapLayoutEntry, MapLayoutKind, MapLayoutPreset, MapLayoutPresets};
use against_rome_placement::WorldObject;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// File extension used by the C# editor (`DefaultExt`).
pub const LAYOUT_EXTENSION: &str = "arm-layout.json";
/// C# `MapLayoutPresets.Deserialize` / import size cap.
pub const MAX_LAYOUT_BYTES: usize = 2_000_000;
/// One texture tile in world units.
pub const TILE_WORLD: f32 = 256.0;
/// .NET `Environment.NewLine` on Windows, where the C# editor writes its files.
const NEWLINE: &str = "\r\n";

// ------------------------------------------------------------------ file pickers

/// What the picker is being asked to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayoutDialogKind {
    Save { suggested_name: String },
    Open,
}

/// Injectable for tests; the desktop default opens the native `rfd` dialogs.
pub type LayoutFilePicker = Box<dyn Fn(&LayoutDialogKind) -> Option<PathBuf> + Send + Sync>;

pub fn native_layout_picker() -> LayoutFilePicker {
    Box::new(|kind| match kind {
        LayoutDialogKind::Save { suggested_name } => rfd::FileDialog::new()
            .set_title("Save map layout / 保存配置")
            .add_filter("Map layout (*.arm-layout.json)", &["json"])
            .set_file_name(suggested_name.as_str())
            .save_file(),
        LayoutDialogKind::Open => rfd::FileDialog::new()
            .set_title("Load map layout / 載入配置")
            .add_filter("Map layout / JSON", &["json"])
            .pick_file(),
    })
}

/// Default file name per layout kind (C# `SaveLayoutFile`).
pub fn suggested_file_name(kind: MapLayoutKind) -> String {
    match kind {
        MapLayoutKind::Nature => format!("forest.{LAYOUT_EXTENSION}"),
        MapLayoutKind::Placement => format!("settlement.{LAYOUT_EXTENSION}"),
    }
}

/// Mirrors `SaveFileDialog.AddExtension`: a name without any extension gets the default one.
pub fn with_default_extension(path: &Path) -> PathBuf {
    if path.extension().is_some() {
        path.to_path_buf()
    } else {
        let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
        name.push(format!(".{LAYOUT_EXTENSION}"));
        path.with_file_name(name)
    }
}

/// Validates and writes the layout (temp file + rename so a failed write never truncates).
pub fn write_layout_file(path: &Path, preset: &MapLayoutPreset) -> Result<PathBuf, String> {
    let json = serialize(preset)?;
    let target = with_default_extension(path);
    let mut temp_name = target.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    temp_name.push(".tmp");
    let temp = target.with_file_name(temp_name);
    std::fs::write(&temp, json.as_bytes()).map_err(|e| format!("{}: {e}", temp.display()))?;
    std::fs::rename(&temp, &target).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        format!("{}: {e}", target.display())
    })?;
    Ok(target)
}

/// Reads and validates a layout file (rejects > 2 MB before reading it).
pub fn read_layout_file(path: &Path) -> Result<MapLayoutPreset, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.len() > MAX_LAYOUT_BYTES as u64 {
        return Err("Layout is too large.".into());
    }
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = String::from_utf8(bytes).map_err(|_| "Layout is not valid UTF-8.".to_string())?;
    deserialize(&text)
}

// ------------------------------------------------------------------ JSON (System.Text.Json compatible)

/// .NET `float` text as written by `Utf8JsonWriter`: shortest round-trip digits, no trailing
/// `.0`, scientific (`1E-05`, `1.5E+20`) once the exponent leaves `[-5, 15)`.
pub fn format_float(value: f32) -> String {
    if value == 0.0 {
        return if value.is_sign_negative() { "-0".into() } else { "0".into() };
    }
    let sci = format!("{:e}", value.abs()); // e.g. "1.2345e3"
    let (mantissa, exp) = sci.split_once('e').expect("LowerExp always has an exponent");
    let exp: i32 = exp.parse().expect("valid exponent");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let sign = if value < 0.0 { "-" } else { "" };
    if !(-4..15).contains(&exp) {
        let head = &digits[..1];
        let tail = &digits[1..];
        let frac = if tail.is_empty() { String::new() } else { format!(".{tail}") };
        let exp_sign = if exp < 0 { '-' } else { '+' };
        return format!("{sign}{head}{frac}E{exp_sign}{:02}", exp.abs());
    }
    let body = if exp < 0 {
        format!("0.{}{}", "0".repeat((-exp - 1) as usize), digits)
    } else if digits.len() as i32 <= exp + 1 {
        format!("{digits}{}", "0".repeat((exp + 1) as usize - digits.len()))
    } else {
        let split = (exp + 1) as usize;
        format!("{}.{}", &digits[..split], &digits[split..])
    };
    format!("{sign}{body}")
}

fn escape_json_string(text: &str) -> String {
    // STJ's default encoder is HTML-safe: it escapes <, >, &, ', + and all non-ASCII as \uXXXX.
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\u0022"),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (' '..='~').contains(&c) && !matches!(c, '<' | '>' | '&' | '\'' | '+' | '`') => out.push(c),
            c => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    out.push_str(&format!("\\u{unit:04X}"));
                }
            }
        }
    }
    out.push('"');
    out
}

pub fn kind_name(kind: MapLayoutKind) -> &'static str {
    match kind {
        MapLayoutKind::Placement => "Placement",
        MapLayoutKind::Nature => "Nature",
    }
}

/// `MapLayoutPresets.Serialize`: validates, then writes indented camelCase JSON.
pub fn serialize(preset: &MapLayoutPreset) -> Result<String, String> {
    MapLayoutPresets::validate(preset)?;
    let mut out = String::new();
    out.push('{');
    out.push_str(NEWLINE);
    out.push_str(&format!("  \"version\": {},{NEWLINE}", preset.version));
    out.push_str(&format!("  \"kind\": \"{}\",{NEWLINE}", kind_name(preset.kind)));
    out.push_str(&format!("  \"entries\": [{NEWLINE}"));
    for (index, e) in preset.entries.iter().enumerate() {
        out.push_str(&format!("    {{{NEWLINE}"));
        out.push_str(&format!("      \"type\": {},{NEWLINE}", escape_json_string(&e.type_name)));
        out.push_str(&format!("      \"x\": {},{NEWLINE}", format_float(e.x)));
        out.push_str(&format!("      \"z\": {},{NEWLINE}", format_float(e.z)));
        out.push_str(&format!("      \"heightOffset\": {},{NEWLINE}", format_float(e.height_offset)));
        out.push_str(&format!("      \"angle\": {},{NEWLINE}", format_float(e.angle)));
        out.push_str(&format!("      \"team\": {},{NEWLINE}", e.team));
        out.push_str(&format!("      \"count\": {}{NEWLINE}", e.count));
        out.push_str(if index + 1 == preset.entries.len() { "    }" } else { "    }," });
        out.push_str(NEWLINE);
    }
    out.push_str("  ]");
    out.push_str(NEWLINE);
    out.push('}');
    Ok(out)
}

fn read_f32(object: &serde_json::Map<String, Value>, key: &str) -> Result<f32, String> {
    match object.get(key) {
        None => Ok(0.0),
        Some(Value::Number(n)) => n
            .as_f64()
            .map(|v| v as f32)
            .ok_or_else(|| format!("Invalid number for '{key}'.")),
        Some(_) => Err(format!("'{key}' must be a number.")),
    }
}

fn read_i32(object: &serde_json::Map<String, Value>, key: &str) -> Result<i32, String> {
    match object.get(key) {
        None => Ok(0),
        Some(Value::Number(n)) => n
            .as_i64()
            .and_then(|v| i32::try_from(v).ok())
            .ok_or_else(|| format!("'{key}' must be an integer.")),
        Some(_) => Err(format!("'{key}' must be an integer.")),
    }
}

fn parse_kind(value: Option<&Value>) -> Result<MapLayoutKind, String> {
    match value {
        Some(Value::String(s)) if s.eq_ignore_ascii_case("placement") => Ok(MapLayoutKind::Placement),
        Some(Value::String(s)) if s.eq_ignore_ascii_case("nature") => Ok(MapLayoutKind::Nature),
        Some(Value::Number(n)) if n.as_i64() == Some(0) => Ok(MapLayoutKind::Placement),
        Some(Value::Number(n)) if n.as_i64() == Some(1) => Ok(MapLayoutKind::Nature),
        None => Ok(MapLayoutKind::Placement), // C# enum default (0); Validate still checks the rest
        _ => Err("Unsupported layout version, kind or object count.".into()),
    }
}

/// `MapLayoutPresets.Deserialize`: strict property names (unmapped members rejected), then validate.
pub fn deserialize(json: &str) -> Result<MapLayoutPreset, String> {
    if json.chars().count() > MAX_LAYOUT_BYTES {
        return Err("Layout is too large.".into());
    }
    let json = json.strip_prefix('\u{feff}').unwrap_or(json);
    let root: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let root = match root {
        Value::Object(map) => map,
        Value::Null => return Err("Empty layout.".into()),
        _ => return Err("Layout must be a JSON object.".into()),
    };
    if let Some(unknown) = root.keys().find(|k| !matches!(k.as_str(), "version" | "kind" | "entries")) {
        return Err(format!("Unmapped layout member '{unknown}'."));
    }
    let version = read_i32(&root, "version")?;
    let kind = parse_kind(root.get("kind"))?;
    let entries = match root.get("entries") {
        Some(Value::Array(items)) => items,
        _ => return Err("Unsupported layout version, kind or object count.".into()),
    };
    let mut parsed = Vec::with_capacity(entries.len());
    for item in entries {
        let Value::Object(object) = item else {
            return Err("Invalid layout object.".into());
        };
        if let Some(unknown) = object.keys().find(|k| {
            !matches!(k.as_str(), "type" | "x" | "z" | "heightOffset" | "angle" | "team" | "count")
        }) {
            return Err(format!("Unmapped layout member '{unknown}'."));
        }
        let type_name = match object.get("type") {
            Some(Value::String(s)) => s.clone(),
            _ => String::new(), // null / missing: rejected by Validate like C#
        };
        parsed.push(MapLayoutEntry::with_team_and_count(
            type_name,
            read_f32(object, "x")?,
            read_f32(object, "z")?,
            read_f32(object, "heightOffset")?,
            read_f32(object, "angle")?,
            read_i32(object, "team")?,
            read_i32(object, "count")?,
        ));
    }
    let preset = MapLayoutPreset { version, kind, entries: parsed };
    MapLayoutPresets::validate(&preset)?;
    Ok(preset)
}

// ------------------------------------------------------------------ capture

fn normalize_angle(angle: f32) -> f32 {
    (angle % 360.0 + 360.0) % 360.0
}

/// Saved-name lookup: `type_id -> objdef/alias name`.
pub type NameOf<'a> = &'a dyn Fn(u16) -> Option<String>;
/// Terrain height at a world position.
pub type GroundHeight<'a> = &'a dyn Fn(f32, f32) -> f32;

/// Editor-provided lookups for capture/apply: objdef names and terrain height.
pub struct LayoutContext<'a> {
    /// `(type_id, objdef name)` pairs.
    pub names: &'a [(i32, String)],
    pub ground: GroundHeight<'a>,
}

impl LayoutContext<'_> {
    pub fn name_of(&self, type_id: u16) -> Option<String> {
        self.names.iter().find(|(id, _)| *id == i32::from(type_id)).map(|(_, n)| n.clone())
    }

    pub fn resolve(&self, name: &str) -> Option<i32> {
        self.names.iter().find(|(_, n)| n.eq_ignore_ascii_case(name)).map(|(id, _)| *id)
    }
}

/// `CapturePlacementLayout`: relative to the min corner of the selected objects.
/// Unit counts are not stored in DATA, so `count` is written as 0.
pub fn capture_placement(
    objects: &[WorldObject],
    keys: &[(usize, u32)],
    name_of: NameOf<'_>,
    ground: GroundHeight<'_>,
) -> Result<MapLayoutPreset, String> {
    let mut picked: Vec<&WorldObject> = Vec::new();
    for key in keys {
        if let Some(o) = objects.iter().find(|o| (o.slot, o.uid) == *key) {
            if !picked.iter().any(|p| (p.slot, p.uid) == *key) {
                picked.push(o);
            }
        }
    }
    if picked.is_empty() {
        return Err("Select placed objects first.".into());
    }
    let min_x = picked.iter().map(|o| o.position[0]).fold(f32::INFINITY, f32::min);
    let min_z = picked.iter().map(|o| o.position[2]).fold(f32::INFINITY, f32::min);
    let mut entries = Vec::with_capacity(picked.len());
    for o in picked {
        let name = name_of(o.type_id).ok_or_else(|| format!("Unknown object name for type {}.", o.type_id))?;
        let [x, y, z] = o.position;
        entries.push(MapLayoutEntry::with_team_and_count(
            name,
            x - min_x,
            z - min_z,
            y - ground(x, z),
            o.rotation_degrees,
            i32::from(o.team),
            0,
        ));
    }
    let preset = MapLayoutPreset::new(MapLayoutKind::Placement, entries);
    MapLayoutPresets::validate(&preset)?;
    Ok(preset)
}

/// Landscape objects use the `Lan…` prefix (same rule as the Nature workflow).
pub fn is_landscape_name(name: &str) -> bool {
    name.len() >= 3 && name.is_char_boundary(3) && name[..3].eq_ignore_ascii_case("Lan")
}

/// `CaptureNatureLayout`: landscape objects inside the half-open tile rectangle, relative to its
/// top-left corner.
pub fn capture_nature(
    objects: &[WorldObject],
    tiles: TileRect,
    name_of: NameOf<'_>,
    ground: GroundHeight<'_>,
) -> Result<MapLayoutPreset, String> {
    if tiles.left >= tiles.right || tiles.top >= tiles.bottom || tiles.left < 0 || tiles.top < 0 {
        return Err("Invalid tile rectangle.".into());
    }
    let (ox, oz) = (tiles.left as f32 * TILE_WORLD, tiles.top as f32 * TILE_WORLD);
    let (rx, rz) = (tiles.right as f32 * TILE_WORLD, tiles.bottom as f32 * TILE_WORLD);
    let mut entries = Vec::new();
    for o in objects.iter().filter(|o| o.parent.is_none()) {
        let [x, y, z] = o.position;
        if !(x >= ox && z >= oz && x < rx && z < rz) {
            continue;
        }
        let Some(name) = name_of(o.type_id).filter(|n| is_landscape_name(n)) else {
            continue;
        };
        entries.push(MapLayoutEntry::new(name, x - ox, z - oz, y - ground(x, z), o.rotation_degrees));
    }
    if entries.is_empty() {
        return Err("No landscape objects in the region.".into());
    }
    let preset = MapLayoutPreset::new(MapLayoutKind::Nature, entries);
    MapLayoutPresets::validate(&preset)?;
    Ok(preset)
}

// ------------------------------------------------------------------ apply

/// Apply-dialog values (`LayoutApplyDialog`): anchor in tile units (0..63.99), rotation 0..359
/// degrees about the saved origin, optional team override (placement layouts only).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutApplyOptions {
    pub anchor_tile_x: f32,
    pub anchor_tile_z: f32,
    pub rotation: f32,
    pub team: Option<i32>,
}

impl Default for LayoutApplyOptions {
    fn default() -> Self {
        Self { anchor_tile_x: 20.0, anchor_tile_z: 20.0, rotation: 0.0, team: None }
    }
}

impl LayoutApplyOptions {
    pub fn anchor_world(&self) -> (f32, f32) {
        (self.anchor_tile_x * TILE_WORLD, self.anchor_tile_z * TILE_WORLD)
    }

    /// Dialog ranges: tile 0..=63.99, rotation 0..=359, team -1..=15.
    pub fn validate(&self) -> Result<(), String> {
        if !self.anchor_tile_x.is_finite() || !(0.0..=63.99).contains(&self.anchor_tile_x) {
            return Err("Anchor X must be between 0 and 63.99 tiles.".into());
        }
        if !self.anchor_tile_z.is_finite() || !(0.0..=63.99).contains(&self.anchor_tile_z) {
            return Err("Anchor Z must be between 0 and 63.99 tiles.".into());
        }
        if !self.rotation.is_finite() || !(0.0..=359.0).contains(&self.rotation) {
            return Err("Rotation must be between 0 and 359 degrees.".into());
        }
        if let Some(team) = self.team {
            if !(-1..=15).contains(&team) {
                return Err("Team must be between -1 and 15.".into());
            }
        }
        Ok(())
    }
}

/// C# `Transform`: rotate the saved offset about the origin, add the anchor, reject outside map.
fn transform(entry: &MapLayoutEntry, anchor_x: f32, anchor_z: f32, rotation: f32) -> Result<(f32, f32), String> {
    if !(anchor_x.is_finite() && anchor_z.is_finite() && rotation.is_finite()) {
        return Err("Invalid anchor.".into());
    }
    let radians = normalize_angle(rotation) * std::f32::consts::PI / 180.0;
    let (mut cos, mut sin) = (radians.cos(), radians.sin());
    if cos.abs() < 1e-6 {
        cos = 0.0;
    }
    if sin.abs() < 1e-6 {
        sin = 0.0;
    }
    let x = anchor_x + entry.x * cos - entry.z * sin;
    let z = anchor_z + entry.x * sin + entry.z * cos;
    if !x.is_finite() || !z.is_finite() || !(0.0..=16383.0).contains(&x) || !(0.0..=16383.0).contains(&z) {
        return Err("Layout extends outside the map.".into());
    }
    Ok((x, z))
}

/// `PlanPlacements` / `PlanNature` as DATA-session requests. The whole layout is rejected when any
/// type is unknown or any position leaves the map (C# contract).
pub fn plan_requests(
    preset: &MapLayoutPreset,
    resolve: &dyn Fn(&str) -> Option<i32>,
    options: &LayoutApplyOptions,
    ground: GroundHeight<'_>,
) -> Result<Vec<PlacementRequest>, String> {
    MapLayoutPresets::validate(preset)?;
    options.validate()?;
    let (ax, az) = options.anchor_world();
    let placement = preset.kind == MapLayoutKind::Placement;
    let mut out = Vec::with_capacity(preset.entries.len());
    for entry in &preset.entries {
        let type_id = resolve(&entry.type_name).ok_or_else(|| {
            format!(
                "Missing {} type: {}",
                if placement { "placement" } else { "nature" },
                entry.type_name
            )
        })?;
        let (x, z) = transform(entry, ax, az, options.rotation)?;
        let y = ground(x, z) + entry.height_offset;
        if !y.is_finite() {
            return Err("Invalid ground height.".into());
        }
        let team = if placement {
            let team = options.team.unwrap_or(entry.team);
            u16::try_from(team).ok()
        } else {
            None
        };
        out.push(PlacementRequest {
            type_id,
            name: entry.type_name.clone(),
            x,
            y,
            z,
            rotation_deg: normalize_angle(entry.angle + options.rotation),
            team,
        });
    }
    Ok(out)
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;

    /// Shape produced by C# `MapLayoutPresets.Serialize` (CRLF on Windows).
    const CSHARP_SAMPLE: &str = "{\r\n  \"version\": 1,\r\n  \"kind\": \"Placement\",\r\n  \"entries\": [\r\n    {\r\n      \"type\": \"RomHaus01\",\r\n      \"x\": 0,\r\n      \"z\": 0,\r\n      \"heightOffset\": 0,\r\n      \"angle\": 90,\r\n      \"team\": 1,\r\n      \"count\": 0\r\n    },\r\n    {\r\n      \"type\": \"RomSol01\",\r\n      \"x\": 256.5,\r\n      \"z\": 128.25,\r\n      \"heightOffset\": -1.5,\r\n      \"angle\": 0.1,\r\n      \"team\": 1,\r\n      \"count\": 12\r\n    }\r\n  ]\r\n}";

    fn sample() -> MapLayoutPreset {
        MapLayoutPreset::new(
            MapLayoutKind::Placement,
            vec![
                MapLayoutEntry::with_team_and_count("RomHaus01", 0.0, 0.0, 0.0, 90.0, 1, 0),
                MapLayoutEntry::with_team_and_count("RomSol01", 256.5, 128.25, -1.5, 0.1, 1, 12),
            ],
        )
    }

    #[test]
    fn serializes_byte_identical_to_csharp_shape() {
        assert_eq!(serialize(&sample()).unwrap(), CSHARP_SAMPLE);
    }

    #[test]
    fn deserializes_csharp_sample_and_roundtrips() {
        let parsed = deserialize(CSHARP_SAMPLE).unwrap();
        assert_eq!(parsed, sample());
        assert_eq!(serialize(&parsed).unwrap(), CSHARP_SAMPLE);
        // LF-only and BOM-prefixed text (hand edited / other tools) is accepted too.
        let lf = CSHARP_SAMPLE.replace("\r\n", "\n");
        assert_eq!(deserialize(&format!("\u{feff}{lf}")).unwrap(), sample());
    }

    #[test]
    fn nature_kind_and_float_edge_cases() {
        let preset = MapLayoutPreset::new(
            MapLayoutKind::Nature,
            vec![MapLayoutEntry::new("LanGerNad00_Tanne", 1e-5, 12345.678, -0.0, 359.5)],
        );
        let json = serialize(&preset).unwrap();
        assert!(json.contains("\"kind\": \"Nature\""));
        assert!(json.contains("\"x\": 1E-05"), "{json}");
        assert!(json.contains("\"z\": 12345.678"), "{json}");
        assert!(json.contains("\"heightOffset\": -0"), "{json}");
        assert_eq!(deserialize(&json).unwrap().entries[0].angle, 359.5);
    }

    #[test]
    fn float_text_matches_dotnet() {
        for (value, text) in [
            (0.0, "0"),
            (1.0, "1"),
            (-2.5, "-2.5"),
            (100.0, "100"),
            (16383.0, "16383"),
            (0.0001, "0.0001"),
            (0.5, "0.5"),
            (123456.0, "123456"),
            (1e15, "1E+15"),
            (1.5e20, "1.5E+20"),
            (0.1, "0.1"),
        ] {
            assert_eq!(format_float(value), text, "{value}");
        }
    }

    #[test]
    fn rejects_what_csharp_rejects() {
        for bad in [
            "null",
            "[]",
            r#"{"version":2,"kind":"Placement","entries":[{"type":"a"}]}"#,
            r#"{"version":1,"kind":"Placement","entries":[]}"#,
            r#"{"version":1,"kind":"Bogus","entries":[{"type":"a"}]}"#,
            r#"{"version":1,"kind":"Placement","entries":[{"type":"a","extra":1}]}"#,
            r#"{"version":1,"kind":"Placement","extra":1,"entries":[{"type":"a"}]}"#,
            r#"{"version":1,"kind":"Placement","entries":[{"type":"a","team":16}]}"#,
            r#"{"version":1,"kind":"Placement","entries":[{"type":"a","count":21}]}"#,
            r#"{"version":1,"kind":"Placement","entries":[{"type":"a","x":16384}]}"#,
            r#"{"version":1,"kind":"Placement","entries":[{"type":"  "}]}"#,
            r#"{"version":1,"kind":"Placement","entries":[{"x":1}]}"#,
            r#"{"version":1,"kind":"Nature","entries":[{"type":"a","team":1}]}"#,
            r#"{"Version":1,"kind":"Placement","entries":[{"type":"a"}]}"#,
        ] {
            assert!(deserialize(bad).is_err(), "should reject {bad}");
        }
        assert!(deserialize(r#"{"version":1,"kind":"placement","entries":[{"type":"a"}]}"#).is_ok());
    }

    #[test]
    fn non_ascii_names_use_stj_escapes() {
        let preset = MapLayoutPreset::new(
            MapLayoutKind::Placement,
            vec![MapLayoutEntry::new("羅馬<a>", 0.0, 0.0, 0.0, 0.0)],
        );
        let json = serialize(&preset).unwrap();
        assert!(json.contains(r#""type": "\u7F85\u99AC\u003Ca\u003E""#), "{json}");
        assert_eq!(deserialize(&json).unwrap(), preset);
    }

    fn object(slot: usize, uid: u32, type_id: u16, team: u16, x: f32, y: f32, z: f32, rot: f32) -> WorldObject {
        WorldObject { slot, type_id, team, uid, position: [x, y, z], rotation_degrees: rot, parent: None, vitals: None }
    }

    fn names(id: u16) -> Option<String> {
        match id {
            1 => Some("RomHaus01".into()),
            2 => Some("LanGerGra01_Gras".into()),
            _ => None,
        }
    }

    #[test]
    fn captures_relative_to_min_corner_with_ground_offset() {
        let objects = vec![object(0, 10, 1, 2, 1000.0, 55.0, 2000.0, 45.0), object(1, 11, 1, 2, 1256.0, 60.0, 2100.0, 90.0)];
        let preset = capture_placement(&objects, &[(1, 11), (0, 10), (0, 10)], &names, &|_, _| 50.0).unwrap();
        assert_eq!(preset.kind, MapLayoutKind::Placement);
        assert_eq!(preset.entries.len(), 2, "duplicate keys collapse");
        let first = &preset.entries[1];
        assert_eq!((first.x, first.z, first.height_offset, first.angle, first.team), (0.0, 0.0, 5.0, 45.0, 2));
        assert_eq!(preset.entries[0].x, 256.0);
        assert!(capture_placement(&objects, &[(9, 9)], &names, &|_, _| 0.0).is_err());
    }

    #[test]
    fn nature_capture_is_half_open_and_landscape_only() {
        let objects = vec![
            object(0, 1, 2, 0, 512.0, 10.0, 512.0, 0.0),  // inside, tile (2,2)
            object(1, 2, 2, 0, 1024.0, 10.0, 512.0, 0.0), // x == right: out
            object(2, 3, 1, 0, 600.0, 10.0, 600.0, 0.0),  // building: skipped
        ];
        let rect = TileRect { left: 2, top: 2, right: 4, bottom: 4 };
        let preset = capture_nature(&objects, rect, &names, &|_, _| 0.0).unwrap();
        assert_eq!(preset.kind, MapLayoutKind::Nature);
        assert_eq!(preset.entries.len(), 1);
        assert_eq!((preset.entries[0].x, preset.entries[0].z), (0.0, 0.0));
        assert!(capture_nature(&objects, TileRect { left: 10, top: 10, right: 11, bottom: 11 }, &names, &|_, _| 0.0).is_err());
    }

    #[test]
    fn plan_rotates_translates_and_overrides_team() {
        let preset = MapLayoutPreset::new(
            MapLayoutKind::Placement,
            vec![MapLayoutEntry::with_team_and_count("RomHaus01", 256.0, 0.0, 2.0, 10.0, 3, 0)],
        );
        let resolve = |n: &str| (n == "RomHaus01").then_some(7);
        let opts = LayoutApplyOptions { anchor_tile_x: 10.0, anchor_tile_z: 10.0, rotation: 90.0, team: Some(5) };
        let plan = plan_requests(&preset, &resolve, &opts, &|_, _| 100.0).unwrap();
        let r = &plan[0];
        assert_eq!(r.type_id, 7);
        assert!((r.x - 2560.0).abs() < 1e-3 && (r.z - 2816.0).abs() < 1e-3, "{} {}", r.x, r.z);
        assert_eq!(r.y, 102.0);
        assert_eq!(r.rotation_deg, 100.0);
        assert_eq!(r.team, Some(5));
        let keep = plan_requests(&preset, &resolve, &LayoutApplyOptions { team: None, ..opts }, &|_, _| 0.0).unwrap();
        assert_eq!(keep[0].team, Some(3));
    }

    #[test]
    fn plan_rejects_entire_layout_on_unknown_type_or_out_of_bounds() {
        let preset = MapLayoutPreset::new(
            MapLayoutKind::Placement,
            vec![MapLayoutEntry::new("A", 0.0, 0.0, 0.0, 0.0), MapLayoutEntry::new("B", 0.0, 0.0, 0.0, 0.0)],
        );
        let only_a = |n: &str| (n == "A").then_some(1);
        assert!(plan_requests(&preset, &only_a, &LayoutApplyOptions::default(), &|_, _| 0.0)
            .unwrap_err()
            .contains("Missing placement type: B"));
        let both = |_: &str| Some(1);
        let far = LayoutApplyOptions { anchor_tile_x: 63.99, anchor_tile_z: 63.99, ..Default::default() };
        let wide = MapLayoutPreset::new(MapLayoutKind::Placement, vec![MapLayoutEntry::new("A", 100.0, 0.0, 0.0, 0.0)]);
        assert!(plan_requests(&wide, &both, &far, &|_, _| 0.0).unwrap_err().contains("outside"));
        let bad = LayoutApplyOptions { rotation: 400.0, ..Default::default() };
        assert!(plan_requests(&preset, &both, &bad, &|_, _| 0.0).is_err());
    }

    #[test]
    fn nature_plan_has_no_team_and_file_roundtrip_adds_extension() {
        let preset = MapLayoutPreset::new(MapLayoutKind::Nature, vec![MapLayoutEntry::new("LanGerGra01_Gras", 1.0, 2.0, 0.0, 10.0)]);
        let plan = plan_requests(&preset, &|_| Some(4), &LayoutApplyOptions::default(), &|_, _| 0.0).unwrap();
        assert_eq!(plan[0].team, None);

        let dir = std::env::temp_dir().join(format!("arm_layout_io_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let written = write_layout_file(&dir.join("forest"), &preset).unwrap();
        assert!(written.to_string_lossy().ends_with("forest.arm-layout.json"));
        assert_eq!(read_layout_file(&written).unwrap(), preset);
        std::fs::write(dir.join("big.json"), vec![b' '; MAX_LAYOUT_BYTES + 1]).unwrap();
        assert!(read_layout_file(&dir.join("big.json")).unwrap_err().contains("too large"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
