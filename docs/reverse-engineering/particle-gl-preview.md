# Particle mode0 GL preview

2026-10-14 Codex. Host scene load/update supplies explicit palette0 particle
images to a separate GL pass. Only resolved native mode0 is drawn. The shader
copies decoded texels, discards transparent texels and uses nearest sampling;
blending is disabled during replacement and restored afterwards. Depth/cull
enable state is also restored. Particle texture/program are released with the
view's GL context resources and created lazily after context recreation.

Other native modes are reported without drawing. The pass uses the existing
saved-child/Rust rectangle batch and keeps its far-to-near particle ordering.
It currently runs after ordinary object sprites: combined object/particle
ordering, native visibility gates, team palette preparation, scaling fidelity,
other modes and native game pixel equivalence remain unfinished.

Verification: forced actual GL context test (`ARM_OPENGL_REQUIRED=1`) and host
particle/definition regression13 passed, no skips/failures. The GL case captures
a frame, confirms one mode0 draw with no GL error, reloads through saved gfx99
primary mode3 and confirms zero draws plus an unsupported-mode diagnostic;
empty update clears counters/diagnostics. This verifies draw execution, not
framebuffer pixel equivalence. Initial fixture used saved gfx0, so changing
primary mode alone did not affect it; changed fixture to gfx99 before rerunning.
Release build0 errors/6 warnings. Logs `re_workspace/particle-gl-*`.

Framebuffer follow-up (2026-10-14): forced actual GL regression13 passed with
freshly built binaries. The camera targets the saved child anchor, ordinary
objects/grid are removed, and 1024x768 baseline/particle/cleared captures are
compared. Mode0 increases the count of exact decoded palette RGB `#605040`;
clearing restores that count to baseline. Depth/cull/blend are enabled after
the pass, and GL reports no error. This proves visible texel output and removal
for the synthetic fixture, not native game image equality. Build0 errors/6
warnings; publish succeeded. Logs `particle-gl-pixels-*`.

Two earlier build attempts failed with CS1503 because a form-only reflection
helper was used with the view; subsequent no-build runs executed old tests and
are not framebuffer evidence. The final run uses the existing view helper and
starts tests only after a successful build.

Cache follow-up: unchanged image snapshots, heightfield identity, relief and
camera view share the prepared atlas/quad batch. Unchanged draws do not upload
the texture again. Image update invalidates the batch; GL resource release marks
the texture dirty. Forced actualGL plus host13 tests passed, including repeated
framebuffer RGB equality with unchanged build/upload counters and image refresh
with increased counters but unchanged pixels. Release build0 errors/6 warnings;
publish succeeded. Logs `particle-gl-cache-*`. No timing benchmark or context
recreation test was performed in this follow-up.

Submission follow-up: sorted mode0 quads now share a cached concatenated vertex
array, one buffer upload and one DrawArrays submission. Logical particle count
is tracked separately from GL submission count. Forced GL plus host13 tests
passed: two overlapping candidates produce two logical draws/one submission
and preserve exact palette pixel count; clear produces zero submissions.
Release build0 errors/9 warnings; publish succeeded. Logs
`particle-gl-submit-*`. This reduces submission count; no timing benchmark was
performed, and other native modes/combined object ordering remain unfinished.
