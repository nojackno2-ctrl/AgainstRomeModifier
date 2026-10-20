# shad-probe-rs

Rust port of `../shad-probe`, using the existing
`against_rome_environment::shadow::ShadowDocument` decoder. It is registered in
the root Cargo workspace; the root `Cargo.lock` is authoritative.

```powershell
cargo run -p shad-probe -- `
  <explicit-readonly-ZIP-path> <NEW-directory-under-TEMP> `
  [decoded-objdef.txt] [decoded-cl_shado.txt]
cargo fmt -p shad-probe -- --check
cargo test -p shad-probe --locked --offline
# Also run the optional .NET 8 source oracle:
cargo test -p shad-probe --locked --offline -- --include-ignored
```

The input is a ZIP regardless of its extension. The probe hashes the entire
input, reads each entry to EOF (including CRC validation), and audits every
non-directory entry as BMP, even if its name has a different extension.
8/24-bit BI_RGB BMPs receive the C# structural/layout/palette audit; only entries
under `SYSTEM/DATA/SHADOWTEXTURE/` receive strict 8-bit shadow decoding. Palette
indices are strength values; palette RGB colors are ignored. Ordinary BMP
`biSizeImage` mismatches are reported, while shadow image-size mismatches fail.

`report.json` preserves C# field names, including `Width`, `Height`, and
`TrailingByteCount`. Archive order determines inventory and sample order. Five
named samples (`GerHau02_shadow.bmp`, `GerNadelbaum_shadow.bmp`,
`round_small_shadow.bmp`, `LakaDorn_shadow.bmp`, `error.bmp`) produce
`sample-N.png` and `sample-N-ground.png`. PNG compression bytes may differ from
.NET; dimensions and decoded RGBA pixels follow the same formulas.

The optional objdef is decoded comma-separated text with the `;idx` header,
five shadow columns, and object names in column 52. Only IDs 12, 42, 46 are
reported. The optional fourth argument supplies strict `[ShadowNames]` decoded
text (IDs 0..1999). UTF-8 and BOM-marked UTF-16/32 are supported. No PFIL decoder
is added: the C# probe also requires decoded text. Candidate names remain
explicitly unverified when mappings are absent; supplied missing mappings do
not fall back. Previews use frame 0, 720x400 flat ground, nearest sampling and
the original circular/square projection and crosses, with ties-to-even rounding.
The report retains this disclosure verbatim:

> Flat ground; assumed UV orientation. No game comparison or object sprite overlay.

The CLI returns 2 for argument/output validation, 1 for processing errors or a
report containing entry failures, and 0 for a report with no failures. Fatal
input/text errors are printed cleanly rather than emitting an unhandled stack
trace. Output is not transactional: failures can leave the newly created output
directory and earlier PNGs. No existing output is overwritten.

Safety is stricter than the C# implementation: explicit regular-file inputs
only; no installed-game lookup; output must be a new descendant of TEMP, reserved
with `create_dir`; files use `create_new`. Traversal/dot segments, empty ZIP path
components, Windows device/ADS/short-name aliases, trailing dots/spaces,
case-insensitive duplicate ZIP paths, symlinks/reparse-point path components,
and ZIP symlink/device entries are rejected. Shadow leaf collisions also fail.
ZIP paths are never extracted or used to construct output names. Unsafe path
failures therefore intentionally differ from the C# probe's permissive behavior.
Inputs remain read-only. As with ordinary filesystem programs, do not allow an
untrusted process to replace directories during a run.

The existing repository ZIP dependency (`zip` 0.6.6 with `deflate`) supports
stored/Deflate entries. Unsupported codecs and encrypted entries are dependency
errors recorded as failures, rather than silently skipped. No new ZIP codec or
encryption implementation is introduced. Synthetic differential tests also
compare decoded PNG pixels and JSON against the unchanged C# source.

Parent verification on read-only retail `shad.dat` matched the normalized C#
report exactly: 2,674 entries, 415 shadow documents/frames, 2,256 other BMPs,
three directories, and zero failures. All ten generated PNGs had matching
dimensions and decoded pixel SHA256. This validates the offline probe against
the installed archive; it does not establish in-game rendering behavior.
