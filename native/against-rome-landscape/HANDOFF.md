# WP-06 Landscape handoff

2026-10-15 Codex. WP-06 implemented and verified at library/planner level. Ownership: only this crate; no git mutations and no installed-game writes.

- Read AGENTS.md, AI_HANDOFF.md, inventory section 5 WP-06 and C# planners. Existing dirty app/other crates preserved.
- Disjoint helpers: cliff.rs + CLIFF_HANDOFF.md; river.rs + RIVER_HANDOFF.md; fortification.rs + FORTIFICATION_HANDOFF.md. Parent: road.rs, lib.rs, manifest, tests, this file.
- Road/river use Bresenham plus X-first diagonal bridge; walls deliberately use X-then-Z Manhattan segments, matching the different C# implementations.
- Navigation dependency supplies immutable PassabilityGrid snapshots. Texture catalogs accept available texture names without coupling pure planning to texture file I/O.
- Milestone: all four modules implemented. Initial Rust suite: 35 pass / 2 fail; failures were fixture expectations (standard road catalog also includes Roman junctions; i32 extreme hash selects N2). Fixed after checking sources/hash.
- Differential attempt resolved: .NET restore denied reading user NuGet.Config, even with --configfile. Framework-only local NuGet.Config plus process-local APPDATA under tests/oracle/cache restores successfully; no system security changes. No app-control 4551 encountered by this crate.

## Delivered APIs

- `cliff`: tile/vertex height edge detection, contour chains, legacy and verified-real-name catalogs, weighted deterministic selection, cliff/scree/crest/collision planning, session application adapter traits.
- `road`: native standard/Roman catalog and atomic orthogonal stroke plans, preserving existing reciprocal links and all revisited links.
- `river`: native catalog, topology/flow/banks, downhill reversal and validation, descending/trench carving, water-level-aware local bed/bank profiles.
- `fortification`: four-style catalogs, wall/gate/tower/endcap plans, snapped palisades, ordered reversible collision changes, gate toggles, boundary flood fill, navigation `PassabilityGrid` snapshot adapter.
- Placement identities use 128-bit GUIDs: Windows `CoCreateGuid` with checked HRESULT; non-Windows `/dev/urandom` UUID4 (not executed here). GUID failure rejects the complete plan.

## Final verification

Process-local environment and exact command, run from repository root:

```powershell
$env:CARGO_HOME='D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\cargo'
$env:RUSTUP_HOME='D:\Github\AgainstRomeModifier\re_workspace\rust-runtime\rustup'
$env:PATH="$env:CARGO_HOME\bin;$env:PATH"
cargo test --offline --locked --manifest-path native/against-rome-landscape/Cargo.toml --target-dir re_workspace/target-against-rome-landscape -- --include-ignored --nocapture --test-threads=1
```

- Final result: **43 passed / 0 failed / 0 ignored**: 40 unit tests plus 3 executed .NET differential tests; 0 doc tests. `cargo fmt` completed.
- Linked original C# sources (not a second reimplementation) matched 1,296 road placement sequences; 768 river water/bank/height/warning sequences; 64 cliff cell/facing/catalog/contour sequences; 64 fortification placement/collision byte/change sequences. Random GUID values excluded from equality and tested separately for distinctness/failure atomicity. Total 2,192 differential cases.
- Oracle requires .NET 10 SDK. Framework-only restore has no package feeds; build products/cache ignored within tests/oracle. Default Rust-only suite skips the 3 opt-in oracles; use the exact command above to execute them.

## Integration boundaries / next owner

- No app or other crate files modified. App wiring, terrain save/load transactions and live-game acceptance remain outside this package. No claim of GUI or installed-game validation.
- Consume texture names from the existing textures crate for catalog filtering; pure planners intentionally do not perform texture file I/O. Implement CliffBlendSession/CliffHeightSession adapters in the app/session owner. River and road plans expose change lists for that owner to apply transactionally.
- `PassabilityGrid` is immutable: rebuild it from synchronized collision bytes after edits/undo/redo, preserving optional height metadata. See fortification::passability_grid.
- Preserved source quirks and deliberate invalid-input robustness differences are documented in CLIFF_HANDOFF.md, RIVER_HANDOFF.md and FORTIFICATION_HANDOFF.md. Do not assume topology helpers implement the native game's pathfinder.
