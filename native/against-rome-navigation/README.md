# Against Rome navigation diagnostics

Pure Rust port of the editor's `src.MapEditor.Modules/Pathfinding` diagnostic core. No game files, Windows APIs, C# runtime, or third-party dependencies are required.

Implemented: owned passability layer snapshots, collision/water queries and different texture-grid footprints; four-neighbor connected components, interest prioritization and bounded shortest obstacle bridge; Manhattan clearance, shortest routes and formation choke diagnostics; road recognition, style/direction checks, water exclusion, collision-clear prescriptions and bridge prescriptions.

Formation API (`formation` module, re-exported at the crate root): `required_clearance_signed`, `topological_chokes_signed`, `evaluate_corridors_signed` (signed troop counts like C#), `diagnose_route` / `diagnose_routes` (per-trip status: reachable / endpoint unusable with out-of-bounds, collision or water cause / disconnected, plus path length, narrowest cell, requirement and fit), and `analyze_formation` (corridor chokes with cross-trip dedup + the `MapDiagnostics` topological fallback).

Repair actions (`repair` module): `road_heal_actions` and `bridge_action` mirror `RoadPathHealer.CreateRepairActions` / `CreateBridgeAction` including stable ids (`road-heal-{n}-{x}-{z}`, `bridge-{component}-{x}-{z}`), `IssueKind`, focus coordinate and zh/en titles and descriptions.

Run `cargo test --manifest-path native/against-rome-navigation/Cargo.toml`.

## Differential tests

`tests/formation_differential.rs` replays `tests/fixtures/formation_golden.txt`, which `tools/csharp-golden` generates by compiling the real C# `NavMeshPassabilityGrid` / `FormationWidthValidator` (private `FindPath` via reflection) over 248 deterministic grids (random collision/height/water, invalid optional layers, out-of-bounds trips, negative troop counts, canyon lanes of width 1-7). Floats are compared bit-for-bit. Regenerate after touching the C# sources:

`dotnet run --project native/against-rome-navigation/tools/csharp-golden -- <abs path>/native/against-rome-navigation/tests/fixtures/formation_golden.txt`

`tests/connectivity_roads_differential.rs` does the same for `NavMeshConnectivityAnalyzer`, `RoadGapDetector` and `RoadPathHealer` (194 cases: random collision/height/water layers, texture grids of a different dimension than the passability grid, 1-4 tile road breaks, isolated land behind walls of thickness 1-14) using `tests/fixtures/connectivity_roads_golden.txt` from `tools/csharp-golden-roads`; region order, bridge points, gap tiles, styles, action ids and texts are compared exactly. Regenerate with `dotnet run --project native/against-rome-navigation/tools/csharp-golden-roads -- <abs path>/native/against-rome-navigation/tests/fixtures/connectivity_roads_golden.txt`.

## Compatibility boundaries

- Preserves the C# diagnostics' heuristic contracts. Water uses the existing nearest vertex selection at cell centers, not bilinear sampling. Formation width thresholds and maximum-unit estimates are copied as separate lookup tables.
- Clearance is distance to an actual obstacle; an entirely open grid retains `-1` values. Boundaries are not obstacles. This behavior is retained for compatibility, including corridor evaluation.
- Diagonal road gaps retain the existing single L-corner suggestion. Bridge prescriptions retain the existing single midpoint edit, even for a multi-cell obstacle span. These are suggestions for designer review, not proof that applying them connects the regions.
- Road endpoints are enumerated in row-major order for deterministic results rather than relying on hash-set enumeration.
- `FormationTrip` troop counts are `i32`: `<= 0` needs no clearance, `<= 1` is never a corridor warning. Obstacle-free grids keep the `-1` sentinel and are still flagged by C#; `RouteDiagnostic::no_obstacles` lets callers suppress that.
- Zero/overflow grid sizes and non-finite/non-positive world dimensions return errors. Malformed optional collision/height layers are ignored, as in the original loader.
- Repair prescriptions are pure data. Actual terrain transactions, undo/redo, UI integration and replacing the C# callers are still outstanding. There is no C ABI yet.

The integration tests cover collision ownership, layer validation, coordinate mapping, mixed-resolution footprints, water exclusion, four-neighbor islands, real bridge midpoint/distance and depth limit, clearance distances, shortest routes, choke deduplication, formation thresholds, texture direction/style and repair contents. They do not claim native game pathfinding equivalence or in-game acceptance.

## Contract gap report (NavMeshRepairContracts)

Covered by `connectivity_roads_differential`: `NavMeshCoordinate` (tile/world values, bit-exact), `IsolatedRegion`, `RoadGapCandidate`, `NavMeshTileChange`, and `NavMeshRepairAction` produced by `RoadPathHealer`/`RoadGapDetector`.

Not ported, deliberately (no production change made):
- `NavMeshIssueKind.FormationBottleneck` and `SubmergedPassage` are declared in C# but no C# source constructs a repair action with them, so there is no behavior to diff; Rust `IssueKind` has only `IsolatedLand` and `RoadGap`. Add them when a C# producer exists.
- `NavMeshCoordinate.FromWorld` (clamping world-to-tile) has no direct golden case; Rust only exercises it indirectly through bridge/heal paths. A dedicated golden line in `tools/csharp-golden-roads` would be needed.
- `NavMeshAnalysisResult` assembly (`analyze_navmesh`) is not diffed against `MapDiagnostics.AnalyzeNavMesh` here; that needs editor session types stubbed beyond `Stubs.cs`.
