# Rust editor domain

Independent Rust library for the native editor migration. There is no .NET/FFI
dependency in the library. `TerrainSession` owns height and optional collision
layers and groups pending edits into undoable strokes. Heights use the game's
8-bit green-channel encoding, not world-space heights.

From this directory:

```powershell
cargo test --offline --locked
cargo test --offline --locked --test differential -- --ignored --nocapture
cargo clippy --offline --locked --all-targets -- -D warnings
```

`.cargo/config.toml` directs builds to `re_workspace/rust-editor-target`.
When invoking Cargo from the repository root with `--manifest-path`, also pass
`--target-dir re_workspace/rust-editor-target` because Cargo discovers config
relative to the working directory.

The opt-in differential test runs the actual C# Modules project through
`tests/oracle/Oracle.csproj`, using its existing `AlrScanlineDifferential` friend
assembly permission. It requires .NET 8. The oracle writes layer snapshots to
`re_workspace/terrain-csharp-oracle.bin`; Rust independently repeats the operation
recipe and compares every layer byte. The recipe covers 193 and 257 vertex maps,
all six ramp falloffs with flat/round caps, sequential Raise/Lower/Smooth/Flatten/
Roughen strokes, water, undo/redo, cancel and baseline commit. It separately
compares roughness noise at 289 coordinates including negative coordinates.

Verified on 2026-10-15: 24 ramps, 22 session snapshots and 3,502,254 layer bytes
matched exactly; all 289 noise samples matched exactly. This validates this
specific recipe, not every floating-point input or all editor behavior. The
small unit tests also cover .NET-compatible ties-to-even quantization, sampled
ramp endpoints, zero-length ramps, invalid inputs, and reverted pending edits.

The Rust API rejects nonfinite ramp parameters rather than allowing undefined
results. Collision is designer-authored data: height edits do not infer global
navigation. The water tool preserves existing nonzero collision values and
marks only initially clear underwater pixels inside its brush.

Still outside this crate's current port: emboss lighting reconstruction,
erosion/sculpt filters, material editing, persistence and desktop UI. Existing
C# implementations remain authoritative for those features until migrated.
