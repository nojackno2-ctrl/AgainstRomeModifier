# Explicit raw working-palette kernel ABI

2026-10-14 Codex. `ar_particle_palette_kernel` exposes the existing source-bound
Rust40e5c0 kernel over explicit A/B buffers. Count1..256 is the product bound;
weight/gains/tint/alpha-flag retain raw native bit semantics. Output is staged,
aliasing allowed, success writes count words and leaves tail untouched. Error204
means unsupported count or insufficient input words; null/capacity/panic use
existing1/2/6 codes. Pointer validity/alignment remain caller obligations.

Managed `PrepareRawPalette` shares the decoder's process-lifetime library/gate
and returns detached read-only raw words. It does not swap channels, normalize
alpha, clamp raw arguments or select native endpoint/primary palette routes.

Actual DLL/asset27 tests passed: raw entry0 preservation, midpoint interpolation,
independent gains, count1 with extreme raw arguments, source ownership/read-only,
invalid counts/input lengths and composition into explicit indexed samples.
Release build0 errors/15 warnings; DLL deployment/editor publish succeeded.
Logs `particle-palette-kernel-*`. Rust runner4551 and Host DLL0x800711C7 from
earlier runs remain unresolved; neither was retried or bypassed this turn.
No new GL/native mode is claimed complete. Native endpoint dispatch, actual
palette arguments, visibility and joint object/particle ordering remain open.

Prepared-palette blit follow-up: the managed clipped framebuffer bridge now
accepts an explicit prepared palette, preserving the source frame's raw palette.
Its existing overload still chooses the stored source palette. Actual DLL/asset28
tests passed: kernel-generated raw40304050 reaches alpha256 clipped blit and
produces target1c253647, while other cells/input framebuffer/source palette stay
unchanged. The raw source palette with alpha0 preserves destination; missing
prepared colors reject even when offscreen. This closes the supplied-buffer
palette-to-blit path, not native endpoint selection or GL mode adoption.
Release build0 errors/44 warnings; publish succeeded. Logs
`particle-prepared-blit-*`. Earlier Rust/Host application-control limits remain.
