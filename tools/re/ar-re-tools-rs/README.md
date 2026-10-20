# ar-re-tools (binary `ar-re`)

Rust port of the reusable, read-only `tools/re/*.py` validation scripts. One crate, one subcommand per
script: `ar-re <subcommand> ...` (run without arguments for the list). Python files are untouched.

Safety rules kept from the originals: inputs are only read; reports use exclusive create (never
overwrite); outputs inside the input / installed-game directories are refused; EXE probes are gated on the
SHA-256 of the supported `Against_Rome.exe`. Deviations: `verify-all-24-pools` writes a report only with
`--output` (the Python script silently overwrote `re_workspace/all-24-pools-validation-report.json`);
`scan-native-scene --output` refuses existing files; `probe-inspect-formatio-lager` prints JSON, not dict reprs.

| Python script | Subcommand | Notes |
|---|---|---|
| inventory_game_directory | `inventory-game-directory GAME OUT.json` | output identical to Python |
| probe_all_pool_structures | `probe-all-pool-structures [GAME]` | |
| probe_engine_config | `probe-engine-config EXE OUT.json` | identical |
| probe_gfxtype_links | `probe-gfxtype-links ROOT OUT.json` | identical |
| probe_inspect_formatio_lager | `probe-inspect-formatio-lager [GAME]` | |
| probe_map_pools | `probe-map-pools ROOT OUT.json` | identical |
| probe_native_lager_pool | `probe-native-lager-pool GAME OUT.json --exe EXE` | |
| probe_native_light_pool | `probe-native-light-pool ...` | |
| probe_native_object_parents | `probe-native-object-parents ...` | |
| probe_native_particle_pool | `probe-native-particle-pool ...` | |
| probe_native_scene_projection | `probe-native-scene-projection EXE OUT [--game-root R] [--evidence F]` | semantically identical |
| probe_native_visual_state | `probe-native-visual-state EXE OUT [--game-root R] [--evidence F]` | semantically identical |
| probe_object_anim_links | `probe-object-anim-links ROOT OUT.json` | identical |
| probe_object_pool_links | `probe-object-pool-links ROOT OUT.json` | identical |
| probe_object_position_links | `probe-object-position-links ROOT OUT.json [--exe EXE]` | |
| probe_pe_interfaces | `probe-pe-interfaces SRC OUT.json` | identical |
| probe_pua | `probe-pua PUA EXE OUT.json` | identical |
| probe_saved_light_render_contract | `probe-saved-light-render-contract EXE OUT.json REF.json` | delegates to `tools/re/saved-light-contract-rs` |
| probe_type_position_contract | `probe-type-position-contract GAME OUT.json --exe EXE` | |
| probe_typus_creation_contract | `probe-typus-creation-contract GAME OUT.json --exe EXE` | |
| probe_verify_all_24_pools | `verify-all-24-pools [MAPS_DIR] [--output F]` | |
| probe_wise_media | `probe-wise-media DISC1 DISC2 OUTDIR [--installed D] [--extract REL]...` | needs the original ISOs (not testable here; components unit-tested) |
| probe_zip_assets | `probe-zip-assets ROOT OUT.json` | identical |
| scan_native_scene | `scan-native-scene EXE [--range A:B] [--target A] [--output F]` | iced-x86 instead of capstone/pefile |
| verify_objective_bci | `verify-objective-bci ORIGINAL INJECTED` | |

"identical" = byte-identical to the Python report on the installed game apart from CRLF line endings.
Capstone is replaced by iced-x86 (`src/dis.rs`); EXE instruction assertions are compared after spelling-only
normalization (radix, whitespace, 32-bit immediates), and the `instructions` dumps may differ from Capstone
in x87 operand spelling (`fmul st, st(1)` vs `fmul st(1)`) and `push -1` vs `push 0xffffffff`.

Reused crates: `against-rome-data` (PFIL/LZSS for BCI), `saved-light-contract` (iced canonicalization and the
saved-light proof). `cargo test -p ar-re-tools --offline` runs the synthetic tests;
`cargo test -p ar-re-tools --offline -- --ignored` also runs read-only checks on the installed game.
