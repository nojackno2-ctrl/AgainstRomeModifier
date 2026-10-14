# Source-bound4CA340 palette selection

2026-10-14 Codex. Retained complete4CA340 Evidence
`ev_5693cabb1da55696707699108b5bcae39fcbf85e11b5d2c13c42964602c1c375`
in `particle-pixel-composite-20261012-evidence.json` supplies these rules:
variant upper bounds clamp to count-1; weight clamps only above256;
secondary-1, zero/full weight or equal variants select endpoint. At full weight
with a present secondary, the secondary becomes the endpoint source.
Otherwise both variants and the signed weight reach40E5C0 interpolation.
4CA4F9 CMP/4CA4FB JLE confirms signed upper-only weight clamp;
4CA508/4CA50D/4CA5DB/4CA5E0/4CA5EC establish sentinel/full/equal branches.

Rust selector/additive atomic four-i32 ABI returns route0 endpoint or1
interpolation, selected first/second(-1 unused), and effective weight.
Invalid variant context returns206 without output writes. Negative primary
or secondary less than-1 are unsupported product inputs, not native rejection
claims. Count must be positive. This selector is specific to4CA340; other
sprite leaves are not automatically generalized.

Managed binding shares the decoder library/gate. Actual DLL/asset40 tests
passed: midpoint, zero, full, above256, missing secondary, negative weight,
equal/clamped variants and invalid contexts. Release build0 errors/44 warnings;
native deployment and editor publish succeeded. Logs `particle-palette-selection-*`.
No map caller arguments or live dispatch were observed; GL adoption and joint
ordering remain unfinished. Earlier execution-policy limits remain unresolved.

Preparation integration: `Prepare4Ca340Default` connects the Rust selector to
the explicit default endpoint or interpolation kernel, retaining selection
metadata and detached read-only words. Endpoint requires256 supplied words
and emits exactly256; interpolation uses the verified caller count. Both paths
use alpha-flag0 as observed in4CA340. Missing endpoint buffers are rejected;
no padding, frame0 fallback or source palette mutation is introduced.
Actual DLL/asset42 tests passed: selected secondary endpoint, ignored tail,
declared interpolation count, negative weight, insufficient endpoint source
and downstream sample composition. Release build0 errors/15 warnings; publish
succeeded. Logs `particle-palette-prepare-*`. The buffers/key/gains are caller-
supplied; live pointer, map parameters and GL adoption remain unverified.

ALR/catalog integration: a lazy document variant adapter reads only palettes
selected by Rust, always from the native first-frame palette source. The catalog
offers an ALR-ID preparation query under its existing ownership gate. Disposed,
missing/unsupported asset and short endpoint buffers produce diagnostics;
prepared words remain detached and usable after disposal. Interpolation count
is still explicit caller context, not inferred from serialized color count.
Actual DLL/asset45 tests passed: parsed256-color secondary endpoint, parsed
short-palette interpolation/rejection, source ownership, disposal reader guard
and retained prepared output. Release build0 errors/15 warnings; publish succeeded.
Logs `particle-alr-palette-*`. Native runtime arguments, live pointer and GL
adoption remain unfinished.

Asset provenance follow-up: catalog-prepared palettes and decoded particle
frames retain ALR ID/name plus a lightweight parsed-document identity token.
`BlitPreparedSamples` rejects mismatched IDs, names or catalog snapshots before
composition. Generic raw-buffer APIs retain their explicit caller contracts.
The token has no reference back to the document/payload. Detached matching
palette/frame pairs remain usable after catalog disposal.
Actual DLL/asset45 tests passed, including same-ID/same-name different catalog
snapshot rejection, wrong-ID rejection and matching composition before/after
disposal. Release build0 errors/44 warnings; publish succeeded. Logs
`particle-palette-provenance-*`. This guards asset provenance only; current
map definition/owner context and live GL adoption remain unfinished.
