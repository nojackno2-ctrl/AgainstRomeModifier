# Particle asset frame bridge

2026-10-14 Codex. `NativeSpriteCatalog.ParticleFrames` resolves an ALR ID directly
through the existing read-only archive/catalog, loads an owned ALR document,
and supplies its columns, directions and frame count to the Rust frame resolver.
It decodes exactly the returned frame with an explicit palette variant and keeps
the uncropped pixels, stored offsets and anchor canvas. There is no name-based
object lookup, direction fallback, overlay or guessed palette selection.

The host `TryDecodeParticleLifetimeFrame` applies current-map/source guards and
uses the effective Rust definition's grafik ALR ID. Missing assets, unsupported
layouts, invalid palettes, unavailable frame branches, invalid resolver results
and corrupt scanlines return no image with a diagnostic. Zero or oversized raw
layout fields are rejected; runtime normalization is not inferred.

## Verified

- Final solution build: 0 errors, 19 warnings (incremental).
- Full Modules: 1079 passed, 2 existing skips, 0 failed.
- Six new asset cases include installed ALR258: columns19, directions32, 608
  frames reach the actual Rust DLL, which rejects its unverified velocity branch.
- Host lifecycle/frame suite: 5 passed, 0 skipped/failed. New fixture passes the
  actual catalog layout4x1/count4 to Rust, obtains frame3 using the grafik divisor,
  checks decoded pixels and blocks stale-source decoding after corrupt reload.
- Initial host run: 4 passed/1 failed due to an incorrect test expectation for
  transparent palette index0; corrected expectation, implementation unchanged.

Local logs: `re_workspace/particle-asset-build.log`,
`particle-asset-modules-all.log`, `particle-asset-host-tests.log`.
The game directory was read only; fixtures were written under TEMP.

This is decoded asset evidence, not native game residency/visibility or final
particle drawing evidence. Scene placement, native blend dispatch and
velocity/direction branches remain open; the map editor goal remains active.
