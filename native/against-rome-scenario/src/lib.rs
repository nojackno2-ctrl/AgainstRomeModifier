//! Native scenario compiler. Original BCI code and constant indices remain stationary;
//! shims append code and only the validated main-loop wait pair is patched.
pub mod bci;
mod bindings;
pub use bindings::DataBindingSource;
pub mod compiler;
pub mod model;
pub use bci::BciImage;
pub use compiler::*;
pub use model::*;
pub type Result<T> = std::result::Result<T, String>;
/// Creates a persistent editor identity without exposing a UUID runtime dependency to UI callers.
pub fn new_spawn_id() -> uuid::Uuid {
    uuid::Uuid::new_v4()
}
/// Reads the native script object aliases used by s_createObj/s_createUnitAndMems.
/// Installed files are read only. Missing catalogs are reported, never guessed.
pub fn load_aliases(game_root: &std::path::Path) -> Result<Vec<String>> {
    let path = game_root.join("SYSTEM/CLAK/cl_scint.ini");
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let raw = if bytes.starts_with(b"PFIL") {
        against_rome_data::lzss::decompress_pfil(&bytes, None).map_err(|e| e.to_string())?
    } else {
        bytes
    };
    let (text, bad) = encoding_rs::WINDOWS_1251.decode_without_bom_handling(&raw);
    if bad {
        return Err("invalid Windows-1251 alias catalog".into());
    }
    Ok(parse_aliases(&text))
}
pub fn parse_aliases(text: &str) -> Vec<String> {
    let mut result = vec![];
    let mut inside = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            inside = line.eq_ignore_ascii_case("[ObjDefName]");
            continue;
        }
        if !inside || line.is_empty() || line.starts_with(';') {
            continue;
        }
        if let Some((alias, name)) = line.split_once('=') {
            if !alias.trim().is_empty() && !name.trim().is_empty() {
                result.push(alias.trim().to_owned())
            }
        }
    }
    result
}
