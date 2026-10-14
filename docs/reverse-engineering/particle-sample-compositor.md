# Packed indexed sample compositor

2026-10-14 Codex. Rust `indexed_sample_pixels` composes packed ALR samples over
caller-supplied raw destination words using explicit kernels0 replacement,
1 half-average, or2 palette-alpha/256. These are pixel ABI kernel IDs, not native
mode IDs. Palette preparation and dispatch remain the caller's responsibility.

Absent spans and index0 in a skipping segment retain destination exactly;
opaque-zero writes palette0. Invalid packed flags and indices reject the whole
result. `ar_particle_sample_pixels` stages full output, supports aliasing inputs,
writes exactly count words, preserves tail and leaves output unchanged on error.
Error202 means invalid sample encoding;201 index failure;200 unsupported kernel.
Lengths are element counts; pointer validity/alignment belong to the caller.

Managed `ComposeSamples` binds the additive export through the existing decoder
library/gate and returns detached raw words. No ARGB swap/alpha conversion occurs.
Actual DLL asset test composes its decoded samples/raw palette, verifies exact
raw replacement, unchanged destination and unsupported-kernel rejection.

Rust26 tests and managed asset25 tests passed; Release build0 errors/44 warnings;
DLL deployment and editor publish succeeded. Logs `particle-sample-*`.
Host13 tests could not load AgainstRomeMapEditor.dll because Windows application
control rejected it with0x800711C7, so none establish GL/host behavior this run.
No policy changes or bypass attempts were made. Native palette dispatch and
new GL blend modes remain unfinished.

Clipped blit follow-up: `blit_indexed_samples`/`ar_particle_sample_blit` add
unscaled integer source placement into a sized destination, including signed
negative offsets and clipping. Source/destination lengths must exactly match
dimensions. The full source is validated even when fully clipped, then output
is staged; error203 means incompatible raster dimensions. No screen projection,
scaling, native palette preparation or mode selection is inferred.

Managed `BlitSamples` uses the same library/gate. Actual DLL asset25 tests passed
with negative-X clipping, exact half-average word00718192, alpha0 preserving
destination, fully offscreen placement, offscreen malformed-sample rejection
and dimension mismatch. Destination inputs remain unchanged. Release build0
errors/15 warnings, deployment/publish succeeded. Logs `particle-blit-*`.
Rust test binaries compiled, but alr_scanlines runner was never executed because
application control rejected it with4551; new Rust blit tests are not claimed
passed. Host/GL0x800711C7 from the preceding run remains unresolved and was not
retried. No security policy changes or bypass attempts were made.
