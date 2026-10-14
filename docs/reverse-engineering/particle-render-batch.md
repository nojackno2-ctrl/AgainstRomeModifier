# Particle renderer input batch

2026-10-14 Codex. `CaptureSavedParticleRenderBatch` refreshes saved-family image
candidates using an explicit caller palette, then prepares a separate atlas and
six-vertex billboard for each accepted child. The Rust rectangle determines
view-space offsets; saved SAR16 child XYZ determines world anchor. Native Y
conversion must be available; terrain-ground substitution is not used.

Vertices match the existing eight-float editor sprite layout (anchor XYZ,
offset XY, UV, native Y). Each draw retains its full candidate and resolved
native mode for the eventual blend consumer. Depth ordering is stable from far
to near. Behind-camera children are excluded. Missing image, rectangle, known
leaf or height conversion produces diagnostics; atlas omissions are explicit.
Repeated frames share one atlas sprite within the batch.

Verification: solution Release build 0 errors/8 warnings; host regression12
passed, no skips/failures. Includes three baseline/moved/copied quads with exact
X and rectangle offsets, behind-camera exclusion, unavailable native height,
unknown-mode diagnostics, definition reload and unchanged saved source bytes.
Editor publish succeeded. Logs: `re_workspace/particle-render-batch-*`.

This is renderer input preparation, not a GL draw. It does not establish native
visibility, palette preparation, mode-specific blending or game pixel equality.
The existing object sprite shader is not used to approximate particle modes.
