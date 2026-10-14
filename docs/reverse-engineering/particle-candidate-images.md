# Particle candidate images

2026-10-14 Codex. The host image interface refreshes scene candidates, resolves
their current effective owner definition, and decodes the exact Rust-selected
linear ALR index. Each result retains the candidate's target GUID, position,
status and uncropped asset metadata (offsets/anchor). The caller supplies an
explicit palette variant; team mapping and native blend dispatch are not inferred.

Unavailable frame branches produce a diagnostic and no image. Invalid frame,
palette or asset data also produce no image. The asset bridge validates bounds
and uses the existing Rust-backed indexed scanline decoder, with no frame0 fallback.

Verification: final solution build0 errors/5 warnings; full Modules1081 passed/
2 existing skips/0 failed; focused host25 passed/0 skipped/0 failed. Cases verify
explicit frame2/palette1 pixels, invalid indices/palettes, actual host frame3 pixels,
three independently positioned baseline/copy images, unchanged saved pool/disk,
and an effective definition reload selecting velocity branch241 with no image.
Existing family save/rollback/reload, linked lights and definition lifecycle tests
are included in the host regression.

Logs under `re_workspace`: `particle-images-build.log`, `particle-images-modules.log`,
`particle-images-host.log`. Original game files were read only. This provides owned
candidate images for renderer integration; it does not yet draw particle sprites
or verify native visibility/blending. Overall editor goal remains active.

Mode follow-up: candidate image results now retain automatic saved-object mode
and an independent mode diagnostic. Resolution is shared once per source root/
owner within a decode invocation, including baseline and multiple copied targets;
no cross-reload cache is introduced. Unknown leaf keeps its mode and explicit
diagnostic. Pixel inspection remains available; this does not declare draw readiness.
Host regression26 passed/0 skipped/failed, including effective mode3->7 reload,
unknown99 leaf, independent moved images and shared immutable mode identity.
Final solution build0 errors/5 warnings. Logs `particle-image-mode-*` under
re_workspace. Palette preparation, native visibility and renderer drawing remain open.

Definition batch follow-up (2026-10-14): exact-frame image decoding shares
definition resolution by owner type within one invocation, including failures.
Unavailable child frames do not trigger this lookup. Each new invocation checks
the current adopted map/catalog again. Focused FamilyParticle/ParticleDefinitionHost
regression: 12 passed, no skips/failures; Release solution build: 0 errors,
23 warnings; editor publish succeeded. No speed benchmark or renderer claim.

Raw palette follow-up: particle asset results retain detached read-only raw
first-frame palette words and the caller's explicit variant. High byte and raw
channel order remain unchanged; ARGB inspection decoding still uses the same
palette source. These words are not the prepared native endpoint/tint palette.
Focused ALR/particle asset25 tests passed, including high-byte ownership,
read-only mutation rejection, variants and shared-frame palette sourcing.
ForcedGL/host13 passed with existing exact mode0 framebuffer behavior unchanged.
Release build0 errors/15 warnings; publish succeeded. Logs
`particle-raw-palette-*`. Initial test expected the wrong fixture variant words;
corrected from the serialized fixture before final verification.
