# Particle placement frame candidates

2026-10-14 Codex. The scene update uses a wrapper around the existing family
preview transaction. It retains unsuppressed baseline children, discovers only
accepted family members, validates their copy/binding request, and queries Rust
translated candidates from the original saved family. Each result carries the
target persistent scenario GUID; baseline results carry Guid.Empty.

Bound moves replace baseline candidates once; native copies retain the original
and have separate target identities. Deleted suppressed families have no candidate.
Undo/Redo recomputes from the immutable baseline rather than the previous preview.
No particle pixels are rendered yet; coordinates/statuses are prepared scene data.

Verification: solution build0 errors/5 warnings. Focused host regression24 passed,
0 skipped/failed, including5 family-particle tests, existing pool/light/definition
tests and4 family copy/move/save/rollback/reload cases. New2 cases cover native
copy, duplicate, independent move, Undo/Redo, unchanged saved pool and disk, and
bound move/delete/Undo without duplicate baseline candidates.

Initial build CS1628 was repaired by using a local alias for an out parameter;
test build CS7036 was repaired by calling the existing RemoveAt API.
Logs: `re_workspace/particle-placement-build.log`, `particle-placement-tests.log`,
`particle-placement-regression.log`. Existing unrelated dirty files/helpers remain
preserved. Original game files were not written. Particle asset drawing, native
visibility, velocity/direction and blend dispatch remain open; editor goal active.

Owner-index follow-up (2026-10-14): candidate batches now share owner dictionaries
within one invocation, built lazily. Direct queries still build fresh indexes;
no state is cached across reloads. Identity and byte comparisons remain in place.
Host regression25 passed, followed by6 family cases after the lazy-build change;
build succeeded. No performance benchmark or renderer acceptance is claimed.
Logs: `particle-owner-index-*` under re_workspace.
