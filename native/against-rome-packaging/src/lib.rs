pub mod catalog;
pub mod contract;
pub mod diff;
pub mod exporter;
pub mod manifest;
pub mod preflight;
pub mod profiler;
pub mod thumbnail;

pub use catalog::{list_endless_maps, require_endless_map, EndlessMapInfo};
pub use contract::{is_payload_path, reject_links};
pub use diff::*;
pub use exporter::{
    ModBundleExporter, ModExportOptions, ModExportResult, ModInstallPolicy, ModInstallResult,
    SlotCollisionStrategy, MAX_CUSTOM_SLOT, MIN_CUSTOM_SLOT, MAP_SUBDIRECTORY_IN_ZIP,
};
pub use manifest::{
    CustomMapEntry, MapCompatibilityPolicy, MapFileCategory, MapPackageDimensions,
    MapPackageFileEntry, MapPackageLocalization, MapPackageManifest, MapRecommendedPlayers,
    CURRENT_SCHEMA_VERSION, CUSTOM_MAPS_REGISTRY_FILE, CUSTOM_MAP_MARKER_FILE,
    MANIFEST_FILE_NAME,
};
pub use preflight::{
    MapExportPreflightChecker, MapPreflightIssue, MapPreflightReport, PreflightCategory,
    PreflightSeverity,
};
pub use profiler::*;
pub use thumbnail::{MapThumbnailSpawnPoint, MinimapThumbnailRenderer, ThumbnailRenderOptions};
