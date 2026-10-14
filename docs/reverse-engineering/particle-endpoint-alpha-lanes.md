# Endpoint alpha DWORD lanes and unresolved buffer alignment

2026-10-14 Codex. Installed EXE SHA256 remains
6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf.
REA40186A complete body40186A..40197B,274 bytes:
`ev_c3ffd29bbad8112b5a6c20facea84ae489d46b30ac0ef1f343b6f360c31c7b4c`.
Masks615FA0..615FC7,40 bytes:
`ev_6cc3b292bd72d311161d7b63fb5effae32e0266e26f289e38e9c32805b7a6833`.
Each qword repeats its DWORD mask:3FC00000,7F800000,3FC00000,
00FFFFFF,FF000000. PAND/PSLLD/PADDD computes each independent DWORD as
`(source & 00FFFFFF) + (((source<<22 & 3FC00000) +
(source<<15 & 7F800000) + (source<<6 & 3FC00000)) & FF000000)`
using wrapping32-bit arithmetic. Old alpha is discarded; RGB retained.

This equals floor((low+2*middle+high)/4) in the high byte, but that DWORD
formula is not the full native buffer operation. Source pointer TEST7 chooses
an alignment prefix;4018C5 increments EDI before4018CE writes the prefix result.
Paired and tail processing use different pointer updates. In-place buffer
effects, source alignment admission and actual caller allocations remain
unverified. Do not infer a general safe sequential native buffer transform.

Rust explicit alpha-lane primitive and additive atomic ABI/managed bridge
operate only independent DWORDs; their names/documentation exclude alignment
pointer effects. Actual DLL/asset30 tests passed: all256 grayscale values,
old alpha removal, fractional sum boundary, immutable input/read-only output.
Release build0 errors/44 warnings; native deployment and publish succeeded.
Logs `particle-endpoint-alpha-*`. Existing Rust/Host application-control limits
were not bypassed. No game execution or live pointer dispatch was observed.

Export `particle-endpoint-alpha-20261014-evidence.json`,25 records, SHA256
9da02dc658cc483ac3db293291551a7966fd26dd32d355bc61c4cc86e1bf772b;
matching snapshot saved, session closed. Full endpoint/live mode adoption and
GL/joint sprite ordering remain unfinished.
