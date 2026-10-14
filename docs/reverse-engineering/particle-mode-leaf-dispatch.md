# Native sprite mode dispatch

2026-10-14 Codex. Re-read the retained complete4B53C0 dossier, Evidence
`ev_4e9598580cc9863a719b1a43a7649f97da08ae5f0100413bb797f7120efa3c72`,
and its assembly branches/calls. Current installed EXE SHA256 matches the dossier:
`6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf`.
Bundle `re_workspace/particle-render-consumers-20261012-evidence.json` SHA256
`a2f395d19816bef29883b92d5f542c78a8c60427ce4e618817d99dbad54adcb1`.

| Native unsigned mode | Leaf |
|---|---|
|0|4CA010|
|1,5,6|4CA340|
|2,7,8|4CAB80|
|3|4CAF80|
|4|4CB380|
|9|4CBCC0|
|49|4CA740|
|Other|No leaf call|

Assembly comparisons include4B54B3/BC,4B58DF/E8,4B5980/89 and4B59A0;
mode3 call4B567B goes to4CAF80, joining the new alpha256 leaf evidence.
Native mode3 is distinct from pixel-row ABI mode2; the latter selects the raw
kernel only. Modes sharing a leaf may supply different palette/gain arguments.

Rust `native_sprite_leaf_address` preserves this static mapping and returns None
for unknown modes. Targeted particle frame tests6 passed/0 failed, including all
11 known values and unknown10/48/50/99/UINT32_MAX. This test validates the lookup;
the assembly/dossier is its independent behavioral evidence.

This does not establish resource/owner gates, mode99 caller substitution,
palette preparation, native visibility or final renderer behavior. No game files
were written and no Ghidra session was reopened for duplicate analysis.

Caller mode99 follow-up (2026-10-14): retained full4DCFC0 dossier, Evidence
`ev_af0d6b826e20f7c857c52868bf473845cfb3a5bbdcbbcef3fcde4a64787a6963`,
shows signed gfx mode99 substituting the primary grafik object's signed type mode
once. Current EXE hash still matches. Rust `resolve_particle_mode` preserves this
rule and keeps missing verified primary-object context unresolved. It does not use
the emitter owner's mode, default0 or recursive substitution. Seven targeted frame
tests pass, including all65535 non99 signed16 inputs, missing context, negative
override and override99. Log: `re_workspace/particle-mode99-tests.log`.
Host metadata extraction/identity binding and actual drawing remain unverified.

Saved mode source follow-up: retained48C460 reader Evidence
`ev_c26f4dde6bad999b2152e5cf5287d0ec727c9902081e375bbbfa1cec71cdbfed`
reads active byte plus7 words into runtime+0/+2..+e, then loads a separate
parallel word array at+10.4DCFC0 reads the signed high word atC2706A,
which isC2706C=baseC2705C+10. Thus mode is not serialized record+13.
Rust `saved_gfxtype_mode` accepts exact15-byte active record plus separate2-byte
mode word; inactive/wrong buffers remain unresolved. Eight frame tests pass,
including deliberately different record-tail3 and mode99, negative mode and
composition with the once-only mode99 resolver. Caller still must verify the
object->gfxtype link; no whole-pool parser, FFI or renderer integration is claimed.

Decoded payload lookup follow-up: `saved_gfxtype_mode_at` now admits exact
version1/count<=14000/length8+17*N and validates slot before locating record and
parallel mode word separately. PFIL decoding is an explicit caller step.
Ten targeted tests pass: malformed header/count/length, inactive/out-of-range
slots and distinct record-tail versus parallel mode. The live installed ENDL_005
smoke decoded238008 bytes, checked14000 slots and found6618 active modes; source
bytes remained identical. Log `re_workspace/particle-mode-pool-tests.log`.
This proves payload access, not object-link correctness or renderer behavior.

Object-link follow-up: Rust `saved_object_gfxtype_mode` accepts an exact active
79-byte objects record, retains UID+3 and follows its gfxtype link+73 into the
checked payload lookup. It never assumes gfx slot equals object slot. Sentinel,
out-of-range, inactive target and wrong-size inputs remain unresolved. Eleven
frame tests pass, including a linked slot1/mode99 distinct from slot0/mode3.
Caller still must establish object-slot/UID provenance and primary-type context.
Log: `re_workspace/particle-object-mode-tests.log`. Host/FFI/rendering remain open.

Mode FFI follow-up: appended `ar_particle_saved_object_mode` reports size24,
UID, linked slot, raw/resolved mode and leaf address (0 means no known leaf).
249=missing primary context,250=bad option,251=UID mismatch,252=unresolved saved
data; errors leave the entire output unchanged. Caller supplies verified primary
grafik type context, not emitter type. Twelve frame tests pass; actual candidate
DLL PInvoke verifies99->3->4CAF80 and atomic UID rejection. Solution build succeeds
with0 errors/43 warnings. Logs: `particle-mode-ffi-*` under re_workspace.
Host metadata adoption and rendering still remain open.

Managed bridge follow-up: the owned definition catalog binds size24 mode exports
lazily under its existing query/disposal gate. C# performs marshaling and nullable
leaf/diagnostic mapping only. Real DLL tests cover99->3, UID/context rejection,
unknown leaf, signed16 overflow, inactive target, unchanged buffers and disposal.
Full Modules1083 passed/2 existing skips/0 failed; final build0 errors/15 warnings.
Logs `particle-mode-managed-*` under re_workspace. Host adoption remains open.

Closed-family host follow-up: mode queries now use a detached decoded gfxtype
image from the adopted family store, captured owner record and loaded UID. The
host checks current-map/definition state, closed membership and emitter ownership;
Rust checks UID/link/mode. Primary-type mode remains explicit caller context.
No per-query filesystem reads. Focused7 tests and broader host26 tests passed,
zero skipped/failed; build0 errors/43 warnings. Cases cover real DLL99->3, missing
context, non-emitter member, stale UID and disk unchanged. Logs `particle-mode-host-*`.
Automatic primary-type extraction and renderer drawing still remain open.
