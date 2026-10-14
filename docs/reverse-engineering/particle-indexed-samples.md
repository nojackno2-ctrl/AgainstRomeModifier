# Rust ALR indexed samples

2026-10-14 Codex. `decode_indexed` retains palette indices and segment-specific
zero policy before color conversion. `None` means no serialized span at that
pixel; `Some(IndexedSample { index: 0, write_zero: false })` means a sample the
segment skips; index0 with write_zero true must be written. Nonzero indices
write regardless of that flag. Two-run rows retain independent zero policies.

The existing ARGB decoder uses this same bounded scanline parser and then
applies its established preview conversion. Dimensions, descriptor counts,
palette lengths, row offsets, prefixes, run bounds and indices keep their
validation. Invalid inputs do not produce a partial raster. Palette values
are not needed to retain indices, but the caller must supply their verified
palette length for bounds checks.

Verification: Rust scanline5/pixel-FFI5/pixel13 passed; actual managed ALR and
particle asset25 passed; forcedGL/host13 passed with existing exact framebuffer
behavior unchanged. Release build0 errors/44 warnings; native DLL deployment
and editor publish succeeded, DLL hashes match. Logs `particle-indexed-*`.

This is a Rust API; no indexed-raster C ABI or managed sample adoption has been
added. Native endpoint palette preparation, nonzero modes and joint sprite
ordering remain unfinished.

ABI/managed follow-up (2026-10-14): `ar_alr_decode_indexed` writes packed u16
samples, absent=0, bit8=present, bit9=write-zero, low8=palette index. Element
counts and pointer alignment are caller obligations. Full raster is staged before
writing; success writes exactly width*height words, tail unchanged, and malformed,
null or insufficient-capacity requests leave output untouched. The existing ABI
version remains1; this is an additive export.

The managed bridge uses the same process-lifetime library reference and binding
gate as the ARGB decoder. Documents expose detached read-only sample words and
cached particle asset results retain them alongside raw palette/ARGB pixels.
Rust6, managed asset25 and forcedGL/host13 tests passed, no skips/failures;
actual DLL flags and read-only mutation rejection are covered. Release build0
errors/44 warnings; deployment and publish succeeded. Logs
`particle-indexed-ffi-*`. New mixed-mode palette preparation/rendering remains open.
