# Map editor modules and parallel development

The editor's module state is separated from WinForms, rendering and installed
game files. `src.MapEditor.Modules` targets plain net8.0 and references only
`src.Shared`; it can be built and tested while the editor host is being changed.

## Implemented boundary

`IEditorModule<TSnapshot>` defines `Load`, `Capture`, `IsDirty`, `AcceptChanges`
and `Reset`. A module owns state and returns isolated snapshots. Loading is
clean; `AcceptChanges` is called only after the host's save transaction succeeds.
Modules do not open files, show dialogs, call another module or change renderer
state. The host coordinates those operations.

`NativeAssets/NativeAlrIndexedFrame` decodes extracted native 8-bit ALR scanlines
and a selected palette into owned, read-only ARGB pixels. It does not open the
game archives, select animation frames or update the renderer. Its current
evidence is native x86 control flow plus synthetic pixel tests; actual asset
and scene fidelity remain unverified. See [native scene evidence](reverse-engineering/native-scene-rendering.md).

`NativeAssets/NativeAlrDocument` parses owned v4..v6 indexed container records
from bytes, preserves shared frames and selects palettes as the native helper
does. `DecodeFrame` supplies the scanline decoder with the entire frame payload,
including its palette prefix. Archive I/O, object definitions, animation and
direction mapping, trailing metadata and scene integration remain with the host
or future format work.

`Events/ScenarioEventSession` owns event commands, duplication limits, copied
action/condition lists and the saved baseline. `MapEditorForm.Events.cs` is its
UI adapter. The existing host's `_events` read interface and baseline setter
are retained temporarily so another AI can continue changing the host without
having to migrate that interface at the same time. This compatibility bridge
is private to the editor assembly; module consumers use snapshots and commands.
Dirty checks compare event contents, including nested actions and conditions.

Independent validation (no editor host or Windows UI required):

```powershell
$env:DOTNET_ROLL_FORWARD = 'Major'
dotnet test tests/AgainstRomeMapEditor.Modules.Tests -c Release
```

## File boundaries for simultaneous work

| Area | Files owned by that task | Shared boundary |
| --- | --- | --- |
| Events state | `src.MapEditor.Modules/Events/*`, matching module tests | Shared scenario DTOs and `IEditorModule` |
| Events UI | `MapEditorForm.Events.cs`, `ScenarioEventDialog.cs`, `ScenarioConditionDialog.cs` | Event session commands and snapshots |
| Terrain | terrain sessions, authoring/history classes and terrain adapter | Height/texture snapshots; renderer updates through host |
| Nature / placement | nature and placement adapters, object state/history | Persistent IDs and placement snapshots |
| Rendering | `MapCanvasControl`, `Map3DViewControl`, mesh/picker/renderer classes | Read-only scene and terrain input |
| AI planning | planner classes and AI adapter | Plans returned for preview/application by host |
| Persistence / integration | persistence adapter, project references, solution | One rollback transaction; accept baselines after commit |

These are ownership boundaries, not a claim that all rows are independent
assemblies already. Events, nature, placement, terrain height/history and blend
authoring/session state have been extracted. Rendering and persistence still
have host coupling. Check the current handoff for active file owners before
editing shared files.

Before parallel editing, record each task's owned files and required boundary
changes in `AI_HANDOFF.md`. A single integrator applies shared interface, project
reference and save-coordinator changes. Each task runs its independent checks;
the integrator then runs the solution build and all tests. A passing module
test does not establish that the whole editor builds or that a map runs in-game.

## Nature boundary and resumed work (2026-10-07)

Nature/NatureEditSession owns pending additions/removals and stroke undo/redo.
The host supplies templates, coordinates and removable slots; the session has
no disk, UI, renderer or game-directory access. Snapshots isolate collections.
The nature adapter provides read snapshots to rendering and persistence; only
session commands mutate state. After a successful save the host reloads DATA,
which clears pending changes and history. A failed transaction retains them.
Terrain height and blend sessions already own their editing/history behavior.
Feature adapters are split into Ai/Nature/Terrain/Placement/Scene/Persistence.
The host still coordinates rendering and one rollback transaction.

Historical parallel assignments (all finished; check the live handoff for new owners):
- AI task: AiMapPlanningDialog.cs, MapEditorForm.Ai.cs, matching AI UI tests.
- Events task: scenario event/condition DTO/compiler/dialog files and their tests.
- Integrator: host, persistence, module interfaces, project references and handoff.
Do not change another task's files or commit while parallel edits are running.
Report interface changes to the integrator before editing shared boundaries.

Resumed development (2026-10-07): blend authoring/session now lives in
`src.MapEditor.Modules/Terrain`. `INativeTerrainMaterialResolver` provides only
`TryResolveNativeCorners` and `ResolveNativeTile`; the host's FloorMaterialCatalog
implements it and retains bitmap and file I/O. Existing namespaces/internal APIs
remain compatible. Pure module tests exercise import, bake, history and rejected
stroke rollback without referencing the Windows host.

Placement batch commands validate all selected indices and duplicate coordinates
before mutation. Multi-selection duplicate/delete produces one undo step; a
duplicate shifted beyond X/Z 0–16383 rejects the whole selection. Mutations also
reject nonfinite coordinates and angles, preserving history on rejection.
Local Ollama roles run sequentially and share one inference gate; default model
is laguna-xs-2.1:latest.
