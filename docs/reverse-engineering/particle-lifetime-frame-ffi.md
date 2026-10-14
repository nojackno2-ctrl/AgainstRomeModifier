# Particle lifetime frame bridge

2026-10-14 Codex. Adds `ar_particle_definition_lifetime_frame` to the existing
owned definition catalog. The Rust query resolves the owner/parameter/geometry/grafik
chain, requires the supplied ALR ID to match the grafik object, and uses that
object's divisor with the existing verified signed32 lifetime kernel.

The caller must supply verified ALR layout and payload count. Matching an ID does
not prove residency or metadata correctness. Velocity and multi-direction branches
return explicit unavailable codes. Errors leave the native output untouched;
the managed bridge returns a nullable frame and diagnostic, with no frame-zero
fallback. Its lock covers queries and disposal. The host wrapper also applies
the existing map and stale-source guards. No particle renderer calls it yet.

## Verification

- Release solution build: 0 errors, 42 warnings, candidate DLL explicitly selected.
- Native regression: 89 passed, 0 failed across the existing selected core suites
  and the new lifetime-frame FFI suite. Policy-blocked metadata tests excluded.
- New FFI tests: grafik divisor differs from owner divisor; error output atomicity;
  missing chain, ALR mismatch, invalid layout, bounds, velocity/direction,
  zero divisor, unsupported route and null handle.
- Installed-game managed catalog tests: 3 passed; actual owner830 returns no frame
  for its unverified velocity branch, and a mismatched ALR is rejected.
- Host definition lifecycle/frame tests: 4 passed, 0 skipped, 0 failed.
- Full Modules suite: 1073 passed, 2 existing skips, 0 failed.

Logs are local ignored files `re_workspace/frame-rust-regression.log`,
`frame-module-test.log`, `frame-host-test.log`, `frame-modules-regression.log`
and `particle-frame-ffi-build-20261014.log`. Original game files were not written.
Actual game rendering, velocity/direction rounding and final particle compositing
remain unverified. The overall map editor objective remains active.
