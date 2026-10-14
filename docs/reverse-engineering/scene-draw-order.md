# Shared editor painter order

2026-10-14 Codex. Rust `scene_draw_order` returns stable far-to-near input
indices for a supplied view-depth list. Equal numeric depths retain input order,
including signed zero. Nonfinite values reject the whole plan. This is editor
policy, not evidence of native game tie handling or visibility.

`ar_scene_draw_order` stages indices before copying, supports overlapping input/
output, writes count indices and leaves tail untouched. Empty count accepts null;
error207 means nonfinite depth or unsupported count. Caller owns aligned valid
storage. Managed bridge shares the existing native library/gate; particle batch
now adopts this order. A heterogeneous object/particle list can use the same API,
but the current GL passes are not yet merged.

Actual DLL/asset49 tests passed: interleaving depths, ties, signed zero, empty
list, immutable output/input and NaN/Infinity rejection. Forced actualGL/host13
passed with exact particle pixels, cache/submission, move/copy and reload behavior
unchanged. This newly built Host DLL loaded successfully; the prior DLL load
block was not reproduced on this revision. No security-policy changes were made.
Release build0 errors/44 warnings; native deployment/publish succeeded.
Logs `scene-draw-order-*`. Full joint GL ordering, other particle modes and native
map palette arguments remain unfinished; older Rust runner restrictions were not
retested.

Combined plan follow-up: `SceneCombinedSpritePlan` interleaves object quads
and supported mode0 particle quads through the shared Rust ordering API. Draw
records retain shader/atlas kind, source index and vertex range; vertex buffers
are referenced without copying. All anchor depths use the supplied current view,
so an older particle batch's cached depth is not blindly reused. Unsupported
particle modes remain outside this replacement plan. Invalid quad sizes and
nonfinite depth reject the plan; empty plans avoid native binding.
Host/forcedGL regression13 passed; final focused1 passed after the empty-plan
guard. Tests cover object/particle interleaving and original buffer/range identity,
NaN/malformed rejection and empty input. Release build0 errors/9 warnings;
publish succeeded. Logs `scene-combined-plan-*`. The GL consumer still needs
to adopt this plan and switch texture/program state in the resulting order.

GL adoption follow-up: object and mode0 particle quads now execute through
the combined painter plan, switching their original shader/texture and blend
state. Separate resident VBOs are uploaded once per pass; contiguous ranges
of the same kind are coalesced into submissions without changing order.
The standalone particle-only batch remains one submission. Objects/placement
preview restore their original program/texture; the trailing particle pass skips
already submitted particles. The particle VBO is released with context resources.

ForcedGL/host15 tests passed: a red object behind a particle leaves its exact
palette pixels visible, moving it in front removes them; duplicate particles
coalesce into one submission with unchanged pixels. Existing mode0, palette
reload, copy/move, native height framebuffer and picking regressions pass.
Release build0 errors/10 warnings; publish succeeded. Logs
`scene-combined-gl-*`. This establishes editor painter behavior for mode0;
other native blend modes, palette runtime arguments and game equivalence remain
unfinished.
