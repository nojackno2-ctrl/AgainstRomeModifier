# Primary grafik type mode

2026-10-14 Codex. Objdef209-entry callback table7 points to4B1621. New REA
read56 bytes and format5F5BDF bytes `%ld\0`; evidence
`ev_9d92315eb010bbcd95973c39928b61a8da925bf8435935914f54d6e0e05bcb10`
and `ev_e21463b14ccc5a13c44c9062a84f649b00ceb5940161274b36fc594407fff6d1`.
Ghidra does not classify the callback as an independent procedure. The existing
source-bound Rust CLI decoded14 complete instructions over this explicit window;
4B1649 stores AX to[C648D0+type*2A4], matching the renderer's signed type-mode word.
This is byte-window evidence, not a full-function or runtime execution claim.

The installed header calls column7 `disps`. Initially using `disp` left the mode
unknown and caused1 live test failure (6 passed); an explicit header read located
the error. After correction, all2160 installed rows expose mode; primary227 raw3.
Source files remain byte-identical.

Rust object definitions now retain optional raw signed32 mode and native low16
interpretation. Effective layers update it only when column7 is present; omitted
values preserve the adopted value, corrupt rows reject atomically. Unverified
initialization stays None. Existing six-field numeric arrays and C ABI44 remain
unchanged. Legacy selected-field fixtures without a verified disps header keep
unknown mode instead of guessing0.

Verification: final selected Rust105 passed/0 failed; live chain7/source5 tests
passed; candidate DLL/export and solution build0 errors/20 warnings; full Modules
1083 passed/2 existing skips/0 failed. Metadata policy tests were excluded.
REA exported2 records and closed with snapshot. Bundle SHA256
`64363745ebe0bebdc8803c00e1c146dc8e342a13d26b167ea1ebcdd868e03b70`;
callback JSON SHA256 `e8f75a6b888e459ba4b7c594a1ba4e40c8a2b19149943dc86533e1de71724a5b`.
Artifacts/logs use `particle-primary-mode-*` under re_workspace.
Automatic mode FFI/host context and actual renderer integration remain open.

Automatic-context follow-up: appended `ar_particle_definition_primary_mode`
queries the validated owned chain's primary grafik object, returning signed mode
or224/225/249 without changing output. C# binds under the existing disposal gate.
The host automatic overload obtains this context from the same adopted catalog;
unknown mode is passed as absent so non99 saved modes still resolve directly.
Known99 uses the explicit effective disps value, never default0.
Native FFI7 tests pass, Modules1083 passed/2 existing skips/0 failed and host26
passed/0 skipped/failed. Tests verify installed owner830 mode3, disposal, unknown
output atomicity and host overlay/reload3->7 plus stale UID rejection.
Final solution build0 errors/8 warnings. Logs `particle-auto-mode-*` in re_workspace.
Particle drawing and full renderer acceptance remain open.
