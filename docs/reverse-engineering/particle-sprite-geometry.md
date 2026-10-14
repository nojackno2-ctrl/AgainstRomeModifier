# Particle preview sprite rectangle

2026-10-14 Codex. Rust converts uncropped ALR frame dimensions and offsets into
four signed pixel edges relative to the canvas centre. Centre uses integer
division by two, matching the existing editor ALR sprite anchor convention.
Negative offsets and frames extending beyond the canvas remain intact.

`ar_particle_sprite_rect` writes four i32 edges only on success; null output
returns 1, unsupported dimensions/count/overflow returns 253. The caller must
provide four aligned writable i32 values. Positive dimensions, signed32 canvas
size, exact width*height pixel count and every edge operation are checked.

The owned managed catalog binds this function under its lifetime gate and
checks decoded dimensions against frame metadata. Host particle image results
retain the rectangle and a separate geometry diagnostic alongside mode and
pixel diagnostics. Failed geometry does not erase the decoded inspection image.

Verification: Rust geometry4 and existing frame12 tests passed; actual managed
DLL test passed (offsets, metadata mismatch, overflow, disposal); host12 tests
passed including moved/copied candidate edges, definition reload and unchanged
saved baseline. Initial managed fixture attempts lacked required base headers
or effective source files and were rejected with 220/226; the final fixture uses
the established clengine.ini plus selected partial definition source contract.
Logs: `re_workspace/particle-geometry-*`.

This establishes editor billboard input geometry. Native visibility, palette
preparation, blend dispatch, GL drawing and game pixel equivalence remain open.
