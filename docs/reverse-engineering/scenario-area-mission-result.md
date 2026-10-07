# Object rectangle conditions and mission results

2026-10-07, Codex. Read-only static analysis of repository-local
`re_workspace/Against_Rome.exe`, SHA256
`6AC85239EA3B87A4357ED8CE09A1818E68C09FE3C00E8D98831F473577B719BF`.
No installed game files were accessed. Disassembly used pefile 2024.8.26 and
Capstone 5.0.9 installed into TEMP/ArmScenarioResearchPackages.

## Position native

`s_getObjPos` registration is `i(iiii)`, handler `0x519690`.
The thunk loads arguments 1/2 unchanged, resolves arguments 3/4 through
`0x5b1680` (VM reference resolver), and calls `0x50f700` at `0x5196bb`.
Thus its ABI is `s_getObjPos(index, uid, &x, &z)`.

`0x50f700` resolves the first pair through the already documented `0x518d30`,
rejects negative or out-of-range indices, and forwards the two output pointers
with the resolved index to `0x50f750`. It returns exactly 1 on success and -1
on failure. The output helper calls `0x4bd320` at `0x50f7a3` for world XYZ
floats; that getter copies three consecutive coordinate components from
`0xb18840/44/48 + position_index * 20`. The helper converts first and third
components through `0x5c5af2` and `fistp` at `0x50f7ba/0x50f7c5`, writing
integer X/Z. The middle Y component is discarded. These are the same world
coordinates used by placement generation; they are not terrain grid indices.

Scenario v6 implements `ObjectInArea` for a persistent placement ID with
integer MinX/MinZ/MaxX/MaxZ, inclusive boundaries in 0–16383. The compiler
passes distinct output locals and gates on return exactly 1 before comparing
bounds. Missing objects, reused indices with mismatched UID, and position
failures cannot reuse stale output. DATA pairs are rebound on each compile;
script placements use their native-output keys. A troop uses its container
position, not an any-member or any-team search. The unit creation path
`0x5247fd → 0x5243a0` creates its container through `0x50ecb0` at
`0x52442e`, retains the resulting index in EBX, and requires index <14000
at `0x52443c`; its successful return at `0x52450d` supplies that object index
to the existing output-pair helper. This fits the position helper range. Corpses with a valid position
can qualify, as with the existing existence condition. This is a continuously
true region condition, not a latched entry-edge trigger.

The ten/eleven-argument team search APIs remain unexposed: their full filter
semantics have not been established. No guessed native call is generated.

## Mission result and termination

Do not use `s_lgcSetMissionResult` for scenario gameplay: its implementation
`0x54c2b0` writes `0x29e7688` plus a separate flag, whereas the gameplay
termination/debriefing path reads the string-key `GLOBAL_MISSION_RESULT`.

`s_setScriptVarL` handler `0x5220d0` resolves the name reference and calls
`0x520fb0`, which invokes `0x4285f0`; this is the same setter used by the game's
own `GLOBAL_MISSION_RESULT` initialization paths. `0x478970` reads that name
via `0x428630`, retains the result using `0x478940`, and clears the global.
The debriefing selector `0x479020` calls `0x478950` and indexes a four-entry
table at `0x479010`:

| Result | Branch | Text key address | Text key |
|---|---|---|---|
| 0 | `0x479034` | `0x5f0e56` | `debriefing_text_loss` |
| 1 | `0x479069` | `0x5f0e6b` | `debriefing_text_win` |
| 2 | `0x479070` | `0x5f0e7f` | `debriefing_text_equal` |
| 3 | `0x479077` | `0x5f0e95` | `debriefing_text_misc` |

`s_quitGame` handler `0x51cd20` jumps to `0x51c5e0`, which jumps to `0x468690`;
that routine sends command 4 through `0x426b00`, the same exit path used by
existing game UI code immediately after setting `GLOBAL_MISSION_RESULT`.
Victory/Defeat therefore set the global to 1/0 then call `s_quitGame`.

A terminal action must be last in a nonrepeating event. Compiler validation
rejects multiple/nonfinal terminal actions. Before ending, it sets
`ARM_MISSION_ENDED`; remaining events in the current poll and all future
polls are skipped, so a simultaneous event cannot overwrite the result.
The original frame and wait are restored as usual. Messages or other actions
before the terminal action retain their order.

## Verification scope

Synthetic VM tests exercise all rectangle edges, outside points, UID reuse,
lookup failure without output writes, repeats, BCI serialization, native
argument/output order, balanced stack, both results before termination,
and simultaneous-event suppression. JSON tests preserve area bounds and
terminal actions; STA form tests preserve area values and labels, validate
region ordering, and disable unrelated terminal-action fields.

Static evidence proves the ABI and result mapping; synthetic and editor
checks do not prove gameplay, multiplayer behavior, corpse timing, or game
save/load persistence. Those require a user-authorized game test outside this
repository-only session.
