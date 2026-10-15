# Native placement saved-state core

Ports the persisted layouts and supported editing transactions from
`src.Shared/Maps/LevelObjectStore.cs` and `.Families.cs`. This is a real DATA
reader/writer, not a replacement simulation or native object-creation claim.

`ObjectStore::load(map)` strictly validates `objects.dat` version/30-byte name
widths/79-byte records/18 parallel columns, the 36 `objdata.dat` segments,
17-byte active/XYZ/degrees position records, and loaded auxiliary/effect pools.
`objects()` exposes slot, full UID, type, team, raw float XYZ, degrees and parent.
`family(slot, uid)` proves the parent closure is independent before editing.

Implemented operations:

- Fixed 16.16 family move, including both primary positions, optional third
  position, saved descendants and direct/attached lights; float64 addition to
  float32 matches the managed translation contract.
- Absolute and relative family rotation (`set_family_rotation`, `rotate_family`, `set_rotation`),
  normalizing angles into [0, 360) degrees and rejecting nonfinite inputs.
- Team ownership edits (`set_family_team`, `set_team`), validating bounds (0..15 or 65535 unassigned)
  and propagating across family closures.
- Vitals editing and inspection (`set_family_vitals`, `set_vitals`, `set_hp_morale`, `vitals`),
  updating and reading HP, morale, and their max ratios across `objdata` segments 0 and 1.
- Family copy allocates free objects/positions/objdata/anim/gfxtype/action/lights,
  remaps references, preserves record/segment/tail bytes and allocates unique
  24-bit UIDs; updates the persisted engine counter when present.
- Family delete releases those pools using the C# inactive-template/reset policy.
- Detached full-state undo/redo; errors leave pending edits intact.
- Save checks **all loaded input files** against their original encoded bytes,
  runs readonly preflights, preserves permissions, takes an exclusive DATA lock,
  stages and syncs every candidate, renames originals to exclusive backups,
  publishes candidates and rolls back ordinary errors while protecting unexpected external targets.
  Success retains `.ar-placement-stage-*/<pool>.old` backups. Failed rollback
  retains recovery files and reports their path. This is not crash-atomic across
  files and the cooperating-editor lock cannot prevent arbitrary third-party
  writes. Saving under Program Files is rejected; use a working copy.

Unknown decoded bytes survive untouched. Unchanged encoded files are never
rewritten. Changed PFIL payloads retain all 64 header bytes except the declared
size; the encoder emits valid literal LZSS, so compressed bytes are intentionally
different and may be larger.

Not yet migrated: garrison/lager/biglager or particle families, cross-map family
import, definition-based initial creation, batch selection, undo persistence.
Those family relationships are rejected (including reverse ownership references);
they are not silently dropped.

```powershell
cargo test --manifest-path native/against-rome-placement/Cargo.toml
# Installed source is read only; this explicit test edits a unique TEMP copy.
cargo test --manifest-path native/against-rome-placement/Cargo.toml retail_temp_roundtrip -- --ignored --nocapture
cargo run --manifest-path native/against-rome-placement/Cargo.toml -- inspect 'C:\Program Files (x86)\Against Rome\MAPS\TUTOR_01' 0
cargo run --manifest-path native/against-rome-placement/Cargo.toml -- move 'C:\path\to\working-map' 0 13136 65536 0 0
cargo run --manifest-path native/against-rome-placement/Cargo.toml -- rotate 'C:\path\to\working-map' 0 13136 180.0
cargo run --manifest-path native/against-rome-placement/Cargo.toml -- set-team 'C:\path\to\working-map' 0 13136 3
cargo run --manifest-path native/against-rome-placement/Cargo.toml -- set-vitals 'C:\path\to\working-map' 0 13136 500.0 80.0
```

`inspect MAP [SLOT]` lists the readonly schema, and optionally validates the
selected family. `move` deltas are signed 16.16 integers: `65536` = one world unit.

Verified on 2026-10-15: 10 regular tests pass (byte preservation, stale UID/shared
position rejection, nonfinite atomicity, PFIL/header preservation, copy/delete/
undo, parent+light move/copy/release, mixed/external light guards, complete-input
conflict/exclusive lock, injected second-publication rollback/retry and failed
rollback recovery evidence retention).
Explicit TUTOR_01 test passes: slot 0 UID 13136, TEMP move/save/reload/undo/save,
copy/save/reload/delete/save, all installed source DATA files exactly byte-equal
before/after. CLI inspect and TEMP move/save/reload also executed successfully.

Two explicit differential tests run an independent C# oracle project referencing
the real `src.Shared` implementation. They invoke actual family move/copy/delete
methods and compare complete decoded payloads after each step:

- TUTOR_01 root 0/13136 → copy 5507/26932: **all 11 pools, 7,490,210 bytes per
  stage are bitwise identical** after move, clone, and delete. Installed DATA
  files remain byte-for-byte unchanged.
- Synthetic parent-child family, direct and attached lights, patterned unknown
  animation/gfxtype records and both animation tails, action records and an
  engine counter 100: **all 8 pools are bitwise identical** after every stage;
  clone allocates root 2/100 independently.

Run all explicit tests with `cargo test --manifest-path
native/against-rome-placement/Cargo.toml -- --ignored --nocapture --test-threads=1`.
