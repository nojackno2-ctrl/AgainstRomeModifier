# Explicit endpoint default palette profile

2026-10-14 Codex. `primary_endpoint_default` composes the source-bound40E4D0
steps with the explicit disk-default40FD50 helper: RGB24 key comparison preserves
entry0 only on a match; gain components are packed through wrapping32-bit
shift/add; nonzero raw tint halves each operand independently; nonzero raw alpha
applies the verified40186A DWORD formula to the transformed suffix.

This profile admits alpha only when caller-supplied working-buffer base mod8,
plus the optional four-byte preserved entry, produces an aligned8 suffix.
Unknown or unsupported layout returns205 and leaves output untouched. This
context describes the native working buffer before entry0 handling, not the
managed source array's address. Independent input/output copies are used.
Misaligned native pointer behavior and live618B54 replacement are not guessed.

Atomic additive ABI and managed readonly binding are available under explicit
default-profile names. Actual DLL/asset31 tests passed: matched/unmatched key,
raw source alpha preservation, tint including high-byte-only nonzero gate,
gain packing carry, raw nonzero alpha and known/unknown alignment contexts.
Release build0 errors/15 warnings; deployment and publish succeeded. Logs
`particle-endpoint-profile-*`. The implementation remains caller-selected;
map runtime arguments, live dispatch, nonaligned40186A semantics, GL blend modes
and joint sprite ordering are incomplete. Earlier execution-policy restrictions
were not bypassed or retried in this turn.
