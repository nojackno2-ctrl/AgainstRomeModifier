# Scenario event conditions: object identity and query evidence

2026-10-07, Codex. Read-only analysis of the repository's ignored
`re_workspace/Against_Rome.exe` with pefile/Capstone, and original level/priest
bytecode with `tools/bcitool.py`. Installed-script reads were explicitly
authorized by the user. No installed file was changed. These findings are
static evidence; condition gameplay and save/load behavior remain unverified.

## Object existence and death

The native registrations in `script-natives.md` expose:

| Native | Signature | Handler | Implementation |
|---|---|---|---|
| `s_objExists` | `i(ii)` | `0x5194f0` | `0x50f270` |
| `s_objDead` | `i(ii)` | `0x519540` | `0x50f3a0` |

Both handler thunks forward the two integer arguments in their existing order
and store the implementation's return value in the native result register.

`0x50f270` decrements argument 1 before calling `0x50f2a0`. That routine checks
the zero-based index through `0x50f2d0` and compares argument 2 with the value
returned by `0x518d60`. The world-object branch of `0x518d60` calls `0x4bda10`,
which reads the runtime record's identifier at `0xa14c00 + index * 76` after
checking its active state. Consequently, the script API needs a one-based
object index and the matching UID; an index alone cannot distinguish reuse of
the same slot. The DATA file's 79-byte serialized records and runtime's
76-byte records are different layouts, not interchangeable memory images.

Original `SYSTEM/CLAK/SCRIPT/ak_priester.bci` corroborates the call convention:
at code offsets `0xa9e8` and `0xa9f0`, the script pushes variables 94 and 103,
then invokes `s_objExists` at `0xa9f8`, drops two argument words and pushes the
return register. The pair comes from `s_readFromObjArray` immediately before
this sequence. Native argument 1 is the last-pushed object index, and argument
2 is the earlier-pushed identifier.

`0x50f3a0` resolves the same pair through `0x518d30`. The resolver returns
index-minus-one only if `s_objExists` succeeds; otherwise it returns -1.
Death then queries state flags 3 and 4 through `0x50f3d0` / `0x518cf0` and
returns 1 if either matches. A missing or mismatched object returns **0**.
Thus `!s_objDead(pair)` is not a valid alive test. Alive requires a successful
existence check as well; a destroyed/removed condition must distinguish an
object that previously existed from one that never spawned successfully.
The exact lifecycle of state flags 3/4, corpse removal and save/load still
requires game observation.

## Why raw slot/UID references are insufficient in the editor

`ScenarioLevelObjects.Apply` removes previously owned DATA slots with
`RemoveIfUid`, then rebuilds prebuilt objects from templates. `LevelObjectStore.Add`
chooses a free slot and assigns the currently largest active UID plus one.
Adding nature objects, changing placement order or deleting a placed building
can therefore change the resulting pair. Event-only saves preserve DATA slots,
but that does not make the pair stable across placement changes.

Persistent editor identities and DATA ownership bindings are now implemented
in scenario format v4. `ScenarioSpawn.Id` survives editor load, move and save;
new placements receive new IDs. `ScenarioDataSlot.SpawnId` associates each
prebuilt object with its freshly assigned slot/UID during the existing save
transaction. Legacy v1–v3 scenarios receive deterministic map-local identities;
only complete legacy ordered ownership lists are paired automatically.
Incomplete lists remain unbound, and v4 unbound lists are never guessed from
order or proximity. Duplicate identities, invalid bindings and multiple IDs
bound to one physical slot are rejected before writing the scenario.

Tests verify repeated legacy loads, a real form move/save/reload, unchanged IDs
after DATA reorder despite changed slot/UID, and invalid-file rejection. This
is an editor-side binding, not yet verified against the game's runtime loader.
Script-created placements now retain native creation outputs as described
below. Before exposing object conditions, reject deleted or unresolved targets
with a clear editor error and give copied placements new identities. Never
retarget to an object occupying an old slot.

Required regression coverage: unchanged target after reorder/move/save;
separate identities after copying; failed spawn never counts as destruction;
UID mismatch does not refer to a replacement object; corpse removal after a
confirmed existence; event deadlines and identity/armed state survive game
save/load. The last item requires real game evidence, not a synthetic VM alone.

## Script-created placement bindings

`s_createObj` handler `0x5192e0` resolves its first two arguments as output
pointers and calls `0x50eaf0`. `s_createUnitAndMems` handler `0x52a020` similarly
calls `0x524530`. Both implementations pass those output pointers and the
created zero-based runtime index to `0x518db0`. That helper writes index+1
to output 1 at `0x518df1`, and the UID returned by `0x518d60` to output 2 at
`0x518df8`. An invalid index writes 0 and -1 to the outputs. The wrapper
returns 1 on successful conversion and -1 on failure; it can also fail before
writing either output. Unit output identifies the troop container, not each
individual member.

For placements with nonempty persistent IDs, the spawn shim initializes
`ARM_OBJECT_<Guid N>_INDEX` to 0 and `_UID` to -1, resets its two output locals,
then invokes the existing creation API. Only return value 1 publishes the
native UID and index to those string-key ScriptVarL variables. No wait is
introduced between the two stores. Other results retain the invalid sentinel
pair, even if a native wrote partial outputs. Event-spawn actions have no
placement ID and do not overwrite these bindings. Future object conditions
must still validate existence with both values and separately arm any
destroyed/removed condition; successful creation alone does not prove lifecycle
or save/load behavior.

Synthetic VM tests cover both APIs, unit-first order and the original building
wait, failure without outputs and with partial outputs, stale global values,
exact return-value gating, bytecode serialization, duplicate-ID rejection,
balanced frames and coexistence with event-spawn actions. Full Release tests
pass (405 passed, 21 skipped). This is static/VM validation, not live gameplay.

## Area-search investigation

`s_searchTeamUnits` registers as `i(iiiiiiiiii)` at `0x53c460`, forwarding all
ten integers to `0x5384e0`. Original ENDL_000 uses it at `0x934`, cleaning ten
words and using the native result. The nearby sequence first creates an IPR
array via `s_createIPRArray`, stores its handle in local 0, and passes that
handle as the last-pushed (first native) argument.

`s_searchTeamFigures` registers as `i(iiiiiiiiiii)` at `0x53c300`, forwarding
eleven arguments to `0x538120`; ENDL_000 calls it at `0x3ba8`. These registrations
and call sites do not by themselves prove radius, coordinate, filtering or
array-capacity semantics. Do not expose a guessed rectangular/circular query.
Follow the implementations and position-based variants and corroborate them
against original scripts before generating area-condition bytecode.

An independent AGY read-only research job (`7d065654ae11`) reached its 180-second
print timeout without a usable report. The server labelled the job succeeded
with exit code 0, but the actual result was only a timeout notice; it is not
evidence that this investigation was completed.
