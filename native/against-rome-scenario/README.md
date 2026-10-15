# Rust scenario / IPR compiler

`against-rome-scenario` ports the real designer scenario pipeline: v1–6 PascalCase
`arm_scenario.json`, stable legacy GUID migration, spawn runtime identity binding,
DATA identity validation, all five event actions and three object conditions,
repeat deadlines, wave movement orders, victory/defeat, and camelCase numeric-enum
`arm_start_camera.json` with center-to-world or direct-engine position.

The compiler keeps the original code addresses and constant indices stationary.
It validates the main's reachable CFG, requires one cyclic wait hook, appends the
spawn/initialization/poll/camera shims, and changes only the original wait pair.
Errors leave caller images unchanged. `compile` returns raw BCI0;
`compile_script` preserves PFIL metadata/compression kind (literal LZSS) and returns
original bytes exactly when no content is enabled. Always pass the original
`ak_level.arm_original` as input when available; compilation is deterministic from
that input and cannot accumulate duplicate shims.

```
cargo test --manifest-path native/against-rome-scenario/Cargo.toml
cargo test --manifest-path native/against-rome-scenario/Cargo.toml -- --include-ignored --nocapture
cargo run --manifest-path native/against-rome-scenario/Cargo.toml -- inspect original.bci
cargo run --manifest-path native/against-rome-scenario/Cargo.toml -- compile original.bci arm_scenario.json new-output.bci camera.json
```

CLI compilation creates a new file and refuses overwriting or writing inside the
installed game directory. The library is pure; the integrating editor must write
scenario JSON, original backup, DATA bindings and script as one rollback scope.
Object creation requires a nonempty verified alias catalog; missing/failed catalogs
fail compilation. CLI accepts `[camera.json|-] [game-root|-] [source-map]` after
the output path. Prebuilt metadata requires `source-map`; the CLI reads native
`DATA/objects.dat`, verifies active slot/UID identity and watches its exact bytes.
Recompression is format-equivalent, not byte-identical
to the C# greedy compressor.

Verification: default tests plus explicit C# differential compare complete raw
BCI bytes for synthetic idle and retail ENDL_000 / TUTOR_01, both camera primitives,
spawns, runtime/prebuilt conditions, area/death tracking, all five actions,
repeat polling, orders, and terminal victory/defeat. Retail input SHA256 is checked unchanged.
The oracle links the existing C# implementations; it is a verification dependency
only, not a Rust runtime dependency. No original game file is written. Game VM
execution and in-game victory/pathfinding are not proven by these byte comparisons.

Remaining wider migration: event graph/objective/cutscene timeline editors and their
additional models, full-map multi-file save integration,
and real gameplay acceptance. The C# implementations remain until those callers
are ported.

The app's structured ScenarioUi and new-work-copy export controller are implemented;
local app compilation/tests remain unverified because Windows policy blocks serde's
normal build-script execution with error 4551. Native core tests and the oracle run
successfully. CI can run `csharp_synthetic_differential` without the retail game.

## Scenario review fixes (2026-10-15, Codex)

All six findings have regression coverage: empty compile validates BCI before
restoring bytes; referenced CONS strings require NUL; spawn/wave alias, position
and count errors are rejected instead of clamped; alias catalog failures fail
closed; prebuilt identity is checked against active source DATA and source changes;
`ScenarioDocument::remove_spawn` removes matching DataSlots atomically and refuses
objects still referenced by conditions. It removes editor metadata, not native DATA.

`compile`/`compile_script` remain pure bytecode APIs. User-facing callers should use
`DataBindingSource::compile_script` or `compile_script_verified` with a validated
active snapshot, and recheck watched source bytes immediately before publishing.
The app integration lives outside this crate and was not modified in this task.
Full-map transactional save and live game behavior remain unverified.

Validation uses process-local CARGO_HOME/RUSTUP_HOME as specified in AI_HANDOFF.md:

```powershell
cargo test --locked --manifest-path native/against-rome-scenario/Cargo.toml --target-dir re_workspace/rust-scenario-target
cargo test --locked --manifest-path native/against-rome-scenario/Cargo.toml --target-dir re_workspace/rust-scenario-target --test compiler csharp_synthetic_differential -- --ignored --exact --nocapture
```

General suite: 17 passed, 0 failed, 2 explicitly ignored gates. Synthetic gate:
1 passed, 0 failed, 2 exact complete BCI byte comparisons (both camera primitives).
Initial sandbox runs failed reading the user NuGet.Config; the same gate passed
with normal permissions. No error 4551 occurred, no security policy was changed.
No installed game file was written, no commits were made. Root AI_HANDOFF.md was
left unchanged to honor this task's exclusive `native/against-rome-scenario/**` scope.
