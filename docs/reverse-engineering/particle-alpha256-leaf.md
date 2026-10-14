# Palette alpha/256 leaf

2026-10-14 Codex, reverse-engineer-anything/REA Ghidra12.1.4. Read-only installed
Against_Rome.exe SHA256 `6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`.

## Observations

- Full4CAF80 body1008 bytes: frame bounds, variant upper clamps, format8 gate,
  palette40E4D0/40E5C0 and row40EA00/40EA70 dispatch.
  Evidence `ev_117cebe91353d7fad737f47a7baba2d5038b8ede3d08cfca8fb5581988c57574`.
- Row wrappers40EA00/70 (86 bytes each) call401644/40173F and the cleanup pointer.
  Evidence `ev_bac27f33273799ef109d968a4f01014f7384521975aec1d02f5b59582292b928`,
  `ev_bfbc8294a8dac1d8206a640cb813d12f2d9a1bbcabb03e36e81f7ef638d26fde`.
- Full401644 (251 bytes) and40173F (121 bytes) use palette high byte as alpha,
  multiply four unpacked byte lanes, subtract destination product from destination
  shifted8, add source product, logical-shift8 and pack.40173F skips index0.
  Evidence `ev_ed8a41da18a30c4f28b8058e9d6fedc3e2f83c39dfc442bb34a4f37c613689db`,
  `ev_de0c769eaf9b9be814928cbc133c1077db59201257a96fdd6f8bb7acf867a3ee`.
- Exact2048-byte static table618320 contains256 entries, each four words equal
  alpha0..255, zero mismatches. Evidence
  `ev_9e582c2cd0361e10e74b905b09d06bcb915f348118642e1a5bb7cbbbcb563de3`.

Derived raw lane formula: `(S*A + D*(256-A)) >> 8`. Alpha applies to all four
bytes, including the high byte. White alpha255 over zero yieldsFEFEFEFE.
This is not standard /255 blending or editor ARGB conversion. Table mutation,
401644 odd-tail global zero-unpack invariant, final mode selection, viewport/depth,
palette preparation and actual game rendering remain separate obligations.

## Implementation and verification

Rust `palette_alpha_256` and explicit pixel-row mode2 preserve the raw formula;
caller selects the route and zero policy. Existing modes0/1 retain their behavior.
36864 boundary/random raw-word pairs match an actual CPU MMX instruction replay
using the zero-unpack sequence observed in40173F/401644 paired path. This executes
the instructions in a test, not the original game routine. FFI checks raw high-byte
output, skip-zero and untouched tail; invalid mode is now3 because2 is supported.

Selected Rust regression95 passed/0 failed; candidate DLL/export build and solution
build0 errors/20 warnings; full Modules1081 passed/2 existing skips/0 failed.
Metadata policy tests were excluded. Game bytes were not written.
REA session closed after export10 records and snapshot. Bundle
`re_workspace/particle-alpha-leaf-20261014-evidence.json` SHA256
`297d4563908dcb7ea8bbede2f4f45aab45a84ec8e6514e1058c6847de421aef2`.
Local logs are `particle-alpha-*` under re_workspace. Renderer integration and
full editor objective remain active.
