# Saved particle frame candidates

2026-10-14 Codex. Adds `ar_particle_definition_saved_frames` and size40 candidate
ABI. It copies exact2252-byte record and16-byte state into a detached snapshot,
resolves the effective definition chain and matches its grafik ALR ID. The
existing Rust candidate kernel validates active emitter/count0..64, skips ordinal0
when swarm is positive, ignores inactive/out-of-count children, performs signed
SAR16 position/velocity conversions and computes lifetime frames.

Each candidate has ordinal, position, velocity, lifetime, frame and status.
Unavailable branches use frameUINT32_MAX and an explicit per-child status; these
are successful candidate queries. Whole-query errors (chain/asset mismatch,
invalid buffers/slot/count, insufficient capacity) leave output and count untouched.
The C header documents disjoint buffers and synchronized handle lifetime.

The C# bridge checks native/managed size40, holds the definition lock and returns
immutable nullable-frame records. It does no child parsing or numeric calculations.
The host wrapper obtains actual ALR layout through the asset catalog and preserves
current-map/source guards. Supplied emitter owner relationships remain the caller's
responsibility; this does not establish closed-family membership or native visibility.

## Verification

- Candidate DLL release build/export checks and solution build: 0 errors/42 warnings.
- Selected Rust regression: 91 passed/0 failed; subsequently added skip-first/buffer
  case and final FFI suite5 passed/0 failed. Metadata policy tests excluded.
- Full Modules1080 passed/2 existing skips/0 failed. Actual installed definition
  catalog and DLL return ordinals0/2, exact negative SAR16 coordinates, explicit
  velocity-unavailable frames, preserve input bytes and reject count65/disposed query.
- Host5 passed/0 skipped/failed. Asset layout4x1/count4 reaches real Rust batch;
  selected child returns frame3 and X=-1; failed source reload blocks batch results.
- Native tests also verify positive skip-first, output capacity atomicity, ID mismatch,
  bad buffers/slot, inactive emitter and preservation of output tail/input bytes.

Ignored local logs: `particle-batch-native-tests.log`, `particle-batch-rust-regression.log`,
`particle-batch-modules.log`, `particle-batch-host.log`, `particle-batch-build.log`
under `re_workspace`. No game files were written.

Closed-family adoption, per-child asset decode/scene rendering, visibility, blend
dispatch and velocity/direction rounding remain open. The overall editor goal is active.
