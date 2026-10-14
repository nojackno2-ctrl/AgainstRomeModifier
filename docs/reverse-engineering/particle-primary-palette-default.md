# Explicit primary palette disk-default helper

2026-10-14 Codex. Current installed EXE SHA256 remains
6ac85239ea3b87a4357ed8ce09a1818e68c09fe3c00e8d98831f473577b719bf.
REA reused the matching particle-direction-palette snapshot and analyzed the
full contiguous40FD50 body,157 bytes ending40FDEC. Evidence
`ev_3237c1377ccbe3d59a6451136af30aa2a57dafaf2d09ba1673909b59748ffead`.
Export/snapshot `re_workspace/particle-endpoint-20261014-*` saved; session closed.
Export SHA25646a5b48ae49005cd7ebf8b1832963464b20f25f2bdcb4d42a9f2f01399f1d22c.

Frozen numeric evidence binds disk pointer618B54 to40FD50; runtime replacement
is unproven. The full helper extracts three unsigned byte gains from the packed
word, multiplies independently and truncates using native shifts/masks. Source
high byte is ignored; result high byte is zero. White times gain255 yields
00FEFEFE. This differs from the existing40E5C0 interpolation kernel and must not
be selected implicitly from a native mode alone.

Retained40E4D0 Evidence
`ev_ce940eb7d9db0b5987b75af7ea5bb5235d8b8c459aed2c81b61fe1a16840151c`
shows RGB24-only key comparison deciding whether entry0 is copied untouched.
It forms the packed gain by shift/add with low32 native semantics and calls
618B54; later tint and optional40186A alpha processing remain separate steps.
The new API models only the explicit40FD50 helper, not this entire wrapper.

Rust/atomic ABI and managed binding added under explicit default names. Actual
DLL/asset29 tests passed:255 truncation, distinct packed gains, ignored source
alpha, immutable buffers, read-only output and unsupported count. Release
build0 errors/44 warnings; native deployment and editor publish succeeded.
Logs `particle-primary-default-*`. Earlier Rust runner4551/Host0x800711C7 limits
remain; no runtime pointer or original game draw was observed.
