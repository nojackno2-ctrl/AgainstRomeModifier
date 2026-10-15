# Against Rome navigation diagnostics

Pure Rust port of the editor's `src.MapEditor.Modules/Pathfinding` diagnostic core. No game files, Windows APIs, C# runtime, or third-party dependencies are required.

Implemented: owned passability layer snapshots, collision/water queries and different texture-grid footprints; four-neighbor connected components, interest prioritization and bounded shortest obstacle bridge; Manhattan clearance, shortest routes and formation choke diagnostics; road recognition, style/direction checks, water exclusion, collision-clear prescriptions and bridge prescriptions.

Run `cargo test --manifest-path native/against-rome-navigation/Cargo.toml`.

## Compatibility boundaries

- Preserves the C# diagnostics' heuristic contracts. Water uses the existing nearest vertex selection at cell centers, not bilinear sampling. Formation width thresholds and maximum-unit estimates are copied as separate lookup tables.
- Clearance is distance to an actual obstacle; an entirely open grid retains `-1` values. Boundaries are not obstacles. This behavior is retained for compatibility, including corridor evaluation.
- Diagonal road gaps retain the existing single L-corner suggestion. Bridge prescriptions retain the existing single midpoint edit, even for a multi-cell obstacle span. These are suggestions for designer review, not proof that applying them connects the regions.
- Road endpoints are enumerated in row-major order for deterministic results rather than relying on hash-set enumeration.
- Zero/overflow grid sizes and non-finite/non-positive world dimensions return errors. Malformed optional collision/height layers are ignored, as in the original loader.
- Repair prescriptions are pure data. Actual terrain transactions, undo/redo, UI integration and replacing the C# callers are still outstanding. There is no C ABI yet.

The integration tests cover collision ownership, layer validation, coordinate mapping, mixed-resolution footprints, water exclusion, four-neighbor islands, real bridge midpoint/distance and depth limit, clearance distances, shortest routes, choke deduplication, formation thresholds, texture direction/style and repair contents. They do not claim native game pathfinding equivalence or in-game acceptance.
