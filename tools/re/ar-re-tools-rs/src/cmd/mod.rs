//! One module per ported script. Every module exposes `run(args) -> Result<i32>`.

pub mod all_pool_structures;
pub mod common;
pub mod defaults;
pub mod evidence;
pub mod engine_config;
pub mod gfxtype_links;
pub mod inspect_formatio_lager;
pub mod inventory;
pub mod lager_pool;
pub mod light_pool;
pub mod map_pools;
pub mod object_anim_links;
pub mod object_parents;
pub mod object_pool_links;
pub mod object_position_links;
pub mod particle_pool;
pub mod pe_interfaces;
pub mod pua;
pub mod saved_light_render;
pub mod scan_scene;
pub mod scene_projection;
pub mod type_position;
pub mod typus_creation;
pub mod verify_24_pools;
pub mod verify_bci;
pub mod visual_state;
pub mod wise_media;
pub mod zip_assets;

use crate::util::R;

/// Parsed command line: positionals plus `--name value` options (repeatable).
pub struct Parsed {
    pub pos: Vec<String>,
    pub opts: Vec<(String, String)>,
}

impl Parsed {
    pub fn parse(args: &[String], value_options: &[&str]) -> R<Parsed> {
        let (mut pos, mut opts) = (Vec::new(), Vec::new());
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            if let Some(rest) = arg.strip_prefix("--") {
                let (name, inline) = match rest.split_once('=') {
                    Some((n, v)) => (n, Some(v.to_string())),
                    None => (rest, None),
                };
                if !value_options.contains(&name) {
                    return Err(crate::util::usage(format!("unrecognized arguments: {arg}")));
                }
                let value = match inline {
                    Some(v) => v,
                    None => iter
                        .next()
                        .cloned()
                        .ok_or_else(|| crate::util::usage(format!("argument --{name}: expected one argument")))?,
                };
                opts.push((name.to_string(), value));
            } else {
                pos.push(arg.clone());
            }
        }
        Ok(Parsed { pos, opts })
    }
    pub fn opt(&self, name: &str) -> Option<&str> {
        self.opts.iter().rev().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
    }
    pub fn all(&self, name: &str) -> Vec<&str> {
        self.opts.iter().filter(|(n, _)| n == name).map(|(_, v)| v.as_str()).collect()
    }
    /// Exactly `count` positionals, else a usage error naming them.
    pub fn exact(&self, count: usize, names: &str) -> R<()> {
        if self.pos.len() == count {
            Ok(())
        } else {
            Err(crate::util::usage(names))
        }
    }
}

/// Dispatches `ar-re <subcommand> ...`; returns the process exit code.
pub fn dispatch(args: &[String]) -> R<i32> {
    let Some((name, rest)) = args.split_first() else {
        return Err(crate::util::usage(format!("ar-re <subcommand> [args]\n{}", subcommands_help())));
    };
    match name.as_str() {
        "inventory-game-directory" => inventory::run(rest),
        "probe-all-pool-structures" => all_pool_structures::run(rest),
        "probe-engine-config" => engine_config::run(rest),
        "probe-gfxtype-links" => gfxtype_links::run(rest),
        "probe-inspect-formatio-lager" => inspect_formatio_lager::run(rest),
        "probe-map-pools" => map_pools::run(rest),
        "probe-native-lager-pool" => lager_pool::run(rest),
        "probe-native-light-pool" => light_pool::run(rest),
        "probe-native-object-parents" => object_parents::run(rest),
        "probe-native-particle-pool" => particle_pool::run(rest),
        "probe-native-scene-projection" => scene_projection::run(rest),
        "probe-native-visual-state" => visual_state::run(rest),
        "probe-object-anim-links" => object_anim_links::run(rest),
        "probe-object-pool-links" => object_pool_links::run(rest),
        "probe-object-position-links" => object_position_links::run(rest),
        "probe-pe-interfaces" => pe_interfaces::run(rest),
        "probe-pua" => pua::run(rest),
        "probe-saved-light-render-contract" => saved_light_render::run(rest),
        "probe-type-position-contract" => type_position::run(rest),
        "probe-typus-creation-contract" => typus_creation::run(rest),
        "verify-all-24-pools" => verify_24_pools::run(rest),
        "probe-wise-media" => wise_media::run(rest),
        "probe-zip-assets" => zip_assets::run(rest),
        "scan-native-scene" => scan_scene::run(rest),
        "verify-objective-bci" => verify_bci::run(rest),
        "-h" | "--help" | "help" => {
            println!("{}", subcommands_help());
            Ok(0)
        }
        other => Err(crate::util::usage(format!("unknown subcommand {other}\n{}", subcommands_help()))),
    }
}

pub const SUBCOMMANDS: [(&str, &str); 25] = [
    ("inventory-game-directory", "GAME OUTPUT.json"),
    ("probe-all-pool-structures", "[GAME_ROOT]"),
    ("probe-engine-config", "EXE OUTPUT.json"),
    ("probe-gfxtype-links", "ROOT OUTPUT.json"),
    ("probe-inspect-formatio-lager", "[GAME_ROOT]"),
    ("probe-map-pools", "ROOT OUTPUT.json"),
    ("probe-native-lager-pool", "GAME_ROOT OUTPUT.json --exe EXE"),
    ("probe-native-light-pool", "GAME_ROOT OUTPUT.json --exe EXE"),
    ("probe-native-object-parents", "GAME_ROOT OUTPUT.json --exe EXE"),
    ("probe-native-particle-pool", "GAME_ROOT OUTPUT.json --exe EXE"),
    ("probe-native-scene-projection", "EXE OUTPUT.json [--game-root ROOT] [--evidence FILE]"),
    ("probe-native-visual-state", "EXE OUTPUT.json [--game-root ROOT] [--evidence FILE]"),
    ("probe-object-anim-links", "ROOT OUTPUT.json"),
    ("probe-object-pool-links", "ROOT OUTPUT.json"),
    ("probe-object-position-links", "ROOT OUTPUT.json [--exe EXE]"),
    ("probe-pe-interfaces", "SOURCE OUTPUT.json"),
    ("probe-pua", "PUA EXE OUTPUT.json"),
    ("probe-saved-light-render-contract", "EXE OUTPUT.json STRENGTHENED_REFERENCE.json"),
    ("probe-type-position-contract", "GAME_ROOT OUTPUT.json --exe EXE"),
    ("probe-typus-creation-contract", "GAME_ROOT OUTPUT.json --exe EXE"),
    ("verify-all-24-pools", "[MAPS_DIR] [--output FILE.json]"),
    ("probe-wise-media", "DISC1 DISC2 OUTPUT_DIR [--installed DIR] [--extract MAINDIR_REL]..."),
    ("probe-zip-assets", "ROOT OUTPUT.json"),
    ("scan-native-scene", "EXE [--range 0xA:0xB]... [--target 0xADDR]... [--output FILE]"),
    ("verify-objective-bci", "ORIGINAL INJECTED"),
];

pub fn subcommands_help() -> String {
    SUBCOMMANDS.iter().map(|(n, a)| format!("  ar-re {n} {a}")).collect::<Vec<_>>().join("\n")
}
