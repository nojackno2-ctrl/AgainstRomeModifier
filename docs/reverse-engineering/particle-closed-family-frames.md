# Closed-family particle frame candidates

2026-10-14 Codex. The host now captures baseline particle frame candidates only
after current-map, closed-family, membership, owner slot/UID/TypeId and complete
record/state checks. It uses the existing immutable Rust saved baseline and the
effective definition/asset-layout batch bridge. A failed family produces no partial
results. Unavailable frames remain explicit per-child statuses.

`SceneObjectsFor3D` refreshes this baseline candidate set at scene-update boundaries;
definition cleanup clears it. These are baseline coordinates only. No move/copy
translation, native visibility, palette/blend selection or particle drawing is claimed.

Verification: final solution build0 errors/5 warnings; focused host regression18
passed/0 skipped/0 failed, including3 new family cases and existing saved particle,
saved light and definition lifecycle tests. New cases verify closed linked root,
child ordinal5/frame3/X=-2, scene adoption, cleanup, disk unchanged, stale owner
UID/type, incorrect root UID, failed baseline load and a distinct closed snapshot's
changed record rejecting the complete family without writing it.

Initial build failed because templates do not expose UID; identity comparison now
uses store world objects. Initial2 tests captured directly but scene refresh failed:
`LevelWorldObject.Linked` means a pool link, not a parent link. Roots are now selected
by `FamilyAt`'s actual parent/UID checks. Final18-test regression passed after repair.

Logs: `re_workspace/family-particle-build.log`, `family-particle-tests.log`,
`family-particle-regression.log`. Tests use existing local family/pool fixtures;
their unrelated uncommitted helper sources are preserved. Original game files were
not written. Renderer and gameplay verification remain open; editor goal active.
