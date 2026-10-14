# Particle image reuse

2026-10-14 Codex. The asset catalog caches owned particle frames by ALR ID,
linear frame index and explicit palette variant under its existing gate. Frame
resolution and validation still run before cache lookup; unavailable or invalid
requests never create cache entries. Lifetime and explicit-index paths share this
cache. Dispose clears references; already-returned owned pixel data remains valid.

Solution build succeeded. Seven focused asset tests passed, zero skipped/failed;
added assertions verify identical request reuse, distinct palette results, cleared
cache and preserved returned pixels after cleanup. Local logs are
`re_workspace/particle-image-cache-build.log` and `particle-image-cache-tests.log`.
No game files were written. This does not verify renderer performance, native
blend/palette selection or gameplay. The editor objective remains active.

Disposal follow-up: particle decode and saved-frame queries now check closed state
under the catalog gate before reading assets or invoking a resolver. Eight focused
tests pass, including throwing reader/resolver sentinels that must never run after
Dispose. Returned owned pixels remain valid. Closed queries return explicit
diagnostics and no image/candidates instead of reopening the disposed archive.
