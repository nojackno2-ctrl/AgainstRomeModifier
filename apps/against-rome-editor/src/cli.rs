use against_rome_maps::catalog::{GameMapCatalog, GameMapInfo};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Startup options parsed from CLI arguments.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct StartupOptions {
    pub game_path: Option<PathBuf>,
    pub map_arg: Option<String>,
    pub validate_map: bool,
    pub smoke_test: bool,
    pub show_help: bool,
    pub show_version: bool,
    /// `--lang zh`: Traditional Chinese console output (default English).
    pub chinese: Option<bool>,
}

/// Checks whether map ID is not a campaign (KAMP_) map.
/// Campaign maps are not editable in the map editor.
pub fn is_selectable_map_id(id: &str) -> bool {
    !id.to_ascii_uppercase().starts_with("KAMP_")
}

/// Checks whether map is selectable in the editor (parity with C# `IsSelectableMap`).
pub fn is_selectable_map(map: &GameMapInfo) -> bool {
    is_selectable_map_id(&map.id)
}

/// Resolves a direct map from game path and map ID (parity with C# `ResolveDirectMap`).
pub fn resolve_direct_map(game_path: &Path, map_id: &str) -> Result<GameMapInfo, String> {
    if map_id.trim().is_empty() {
        return Err("未指定 --map 地圖代號。".into());
    }
    if game_path.as_os_str().is_empty() {
        return Err("使用 --map 直接開啟地圖時必須同時指定 --game <path>。".into());
    }
    let catalog = GameMapCatalog::new();
    let map = catalog.require(game_path, map_id)?;
    if !is_selectable_map(&map) {
        return Err(format!("劇情戰役地圖不在地圖編輯器支援範圍：{}", map.id));
    }
    Ok(map)
}

/// Parses CLI arguments case-insensitively.
pub fn parse_startup_options<I, T>(args: I) -> Result<StartupOptions, String>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let mut options = StartupOptions::default();
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        let os_str: OsString = arg.into();
        let arg_str = os_str.to_string_lossy();

        if arg_str.eq_ignore_ascii_case("--help") || arg_str.eq_ignore_ascii_case("-h") {
            options.show_help = true;
            return Ok(options);
        } else if arg_str.eq_ignore_ascii_case("--version") {
            options.show_version = true;
            return Ok(options);
        } else if arg_str.eq_ignore_ascii_case("--game") {
            if options.game_path.is_some() {
                return Err(format!("unknown or duplicate argument: {}", arg_str));
            }
            let next_val = iter
                .next()
                .ok_or_else(|| "--game 缺少路徑參數。".to_string())?;
            options.game_path = Some(PathBuf::from(next_val.into()));
        } else if arg_str.eq_ignore_ascii_case("--map") {
            if options.map_arg.is_some() {
                return Err(format!("unknown or duplicate argument: {}", arg_str));
            }
            let next_val = iter
                .next()
                .ok_or_else(|| "--map 缺少地圖代號。".to_string())?;
            let os_val: OsString = next_val.into();
            options.map_arg = Some(os_val.to_string_lossy().into_owned());
        } else if arg_str.eq_ignore_ascii_case("--validate-map") {
            if options.validate_map {
                return Err(format!("unknown or duplicate argument: {}", arg_str));
            }
            options.validate_map = true;
        } else if arg_str.eq_ignore_ascii_case("--smoke-test") {
            if options.smoke_test {
                return Err(format!("unknown or duplicate argument: {}", arg_str));
            }
            options.smoke_test = true;
        } else if arg_str.eq_ignore_ascii_case("--lang") {
            if options.chinese.is_some() {
                return Err(format!("unknown or duplicate argument: {}", arg_str));
            }
            let os_val: OsString = iter
                .next()
                .ok_or_else(|| "--lang 缺少語言參數（zh 或 en）。".to_string())?
                .into();
            let value = os_val.to_string_lossy().to_ascii_lowercase();
            options.chinese = Some(match value.as_str() {
                "zh" | "zh-tw" | "zh-hant" => true,
                "en" => false,
                _ => return Err(format!("--lang 只接受 zh 或 en：{value}")),
            });
        } else {
            return Err(format!("unknown or duplicate argument: {}", arg_str));
        }
    }

    Ok(options)
}

/// Resolves the startup map directory (either via `--game <dir> --map <ID>` or direct `--map <dir>`).
pub fn resolve_startup_map(options: &StartupOptions) -> Result<Option<PathBuf>, String> {
    if options.validate_map && (options.map_arg.is_none() || options.smoke_test) {
        return Err("--validate-map requires --map and cannot combine with --smoke-test".into());
    }

    match (&options.game_path, &options.map_arg) {
        (Some(game_path), Some(map_id)) => {
            if map_id.trim().is_empty() {
                return Err("未指定 --map 地圖代號。".into());
            }
            let map_info = resolve_direct_map(game_path, map_id)?;
            Ok(Some(map_info.directory_path))
        }
        (Some(_), None) => Err("未指定 --map 地圖代號。".into()),
        (None, Some(map_arg)) => {
            if map_arg.trim().is_empty() {
                return Err("未指定 --map 地圖代號。".into());
            }
            let path = PathBuf::from(map_arg);
            if path.is_dir()
                || map_arg.contains('/')
                || map_arg.contains('\\')
                || map_arg.starts_with('.')
            {
                Ok(Some(path))
            } else {
                Err("使用 --map 直接開啟地圖時必須同時指定 --game <path>。".into())
            }
        }
        (None, None) => Ok(None),
    }
}
