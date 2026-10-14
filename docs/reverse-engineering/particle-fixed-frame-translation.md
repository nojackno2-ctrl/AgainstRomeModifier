# Detached particle frame translation

2026-10-14 Codex. `ParticleSnapshot.translated_fixed` clones a snapshot and performs
checked signed32 addition on XYZ of every active child in an active emitter.
Inactive child bytes and all state/opaque words remain unchanged. This follows the
existing editor storage translation contract, including active out-of-count and
skip-first children; it does not claim native tick simulation.

The appended `ar_particle_definition_saved_frames_translated` export translates
before the verified count-bounded frame kernel performs SAR16. Overflow returns13
without writing output/count. The original saved_frames export forwards zero delta
and retains its ABI. The managed size40 bridge and closed-family adapter forward
integer deltas without calculating child coordinates. Default deltas are zero.

Verification: Rust selected regression93 passed/0 failed; full Modules1080 passed/
2 existing skips/0 failed; focused host18 passed/0 skipped/failed; candidate solution
build0 errors/44 warnings. Native negative-boundary test shows -65537 +1 becomes
-65536 then SAR16=-1, rather than adding separately quantized coordinates. Active
ordinal63 overflow rejects a count1 batch atomically; its inactive form remains
byte-identical. Managed and host tests verify translated coordinates, unchanged
lifetime/frame/status and unchanged input buffers/private pool baseline.

Logs under `re_workspace`: `particle-translation-native.log`,
`particle-translation-rust-regression.log`, `particle-translation-modules.log`,
`particle-translation-host.log`, `particle-translation-build.log`.
Original game files were read only. Scene move/copy adoption, rendering and blend
dispatch remain open; the overall map editor goal remains active.
