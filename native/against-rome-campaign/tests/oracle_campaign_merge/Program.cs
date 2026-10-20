// Runs the real C# campaign wave merge path on fixed cases and dumps the resulting Event JSON.
// Usage: dotnet run -c Release -- <cases.json> <oracle_out.json>
//
// Path under test (src.MapEditor) and how each piece is exercised:
//   CampaignWaveDialog.CompileForMerge (internal static)   REAL call via reflection.
//   MapEditorForm.UniqueEventNames     (private instance)  REAL call via reflection on an uninitialized form whose
//                                                          _eventSession holds the existing events (the method only reads EventSession.Capture()).
//   MapEditorForm.ApplyCampaignWaves   unsupported-field guard   REPLICATED (inline in an instance method needing a live
//                                                          form + object catalog + disk); pinned to source text, see VerifySourcePins.
//   MapEditorForm.MergeScenarioEvents  256-event cap / empty short circuit   REPLICATED, pinned.
// Replicated logic is guarded by VerifySourcePins: the run exits 2 when a pinned C# fragment changes, and the SHA-256 of each
// normalized fragment is written to the output ("sourcePins") so the committed expected file records which source was mirrored.
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.RegularExpressions;
using AgainstRomeMapEditor.Modules.AI;
using AgainstRomeMapEditor.Modules.Events;
using AgainstRomeModifier.Scripting;

var input = JsonNode.Parse(File.ReadAllText(args[0]))!.AsObject();
var json = new JsonSerializerOptions { WriteIndented = true }; // same options as ScenarioDocument persistence

var editorAssembly = Assembly.Load("AgainstRomeMapEditor");
var dialog = editorAssembly.GetType("AgainstRomeMapEditor.CampaignWaveDialog", throwOnError: true)!;
var compileForMerge = dialog.GetMethod("CompileForMerge", BindingFlags.NonPublic | BindingFlags.Public | BindingFlags.Static)
    ?? throw new InvalidOperationException("CompileForMerge not found");
var formType = editorAssembly.GetType("AgainstRomeMapEditor.MapEditorForm", throwOnError: true)!;
var uniqueEventNames = formType.GetMethod("UniqueEventNames", BindingFlags.NonPublic | BindingFlags.Instance)
    ?? throw new InvalidOperationException("MapEditorForm.UniqueEventNames not found");
var eventSessionField = formType.GetField("_eventSession", BindingFlags.NonPublic | BindingFlags.Instance)
    ?? throw new InvalidOperationException("MapEditorForm._eventSession not found");

var pins = VerifySourcePins();
if (pins is null) return 2;

var results = new JsonArray();
foreach (var node in input["cases"]!.AsArray())
{
    var c = node!.AsObject();
    var aliases = c["aliases"]!.AsArray().Select(a => a!.GetValue<string>()).ToArray();
    var plan = BuildPlan(c["plan"]!.AsObject());

    var existing = new List<ScenarioEvent>();
    foreach (var name in c["existingEventNames"]?.AsArray().Select(n => n!.GetValue<string>()) ?? [])
        existing.Add(Placeholder(name));
    for (int i = 0; i < (c["padExistingEvents"]?.GetValue<int>() ?? 0); i++) existing.Add(Placeholder($"Pad_{i}"));
    var scenario = new ScenarioDocument
    {
        Events = existing,
        Spawns = (c["spawnIds"]?.AsArray() ?? []).Select(id => new ScenarioSpawn(c["spawnAlias"]?.GetValue<string>() ?? aliases[0], 100, 100, 0) { Id = Guid.Parse(id!.GetValue<string>()) }).ToList(),
    };

    bool success;
    IReadOnlyList<string> diagnostics = [];
    List<ScenarioEvent> merged = [];
    // REPLICA of the MapEditorForm.ApplyCampaignWaves guard (pinned: apply_campaign_waves_guard).
    bool guardRejected = plan.FactionProfiles.Count > 0 || plan.Waves.Any(w => w.AssignedPathId.HasValue || w.TargetObjectId.HasValue || w.Squads.Any(s => s.RelativeDelaySeconds != 0))
        || plan.Reinforcements.Any(r => r.AssignedPathId.HasValue || r.Squads.Any(s => s.RelativeDelaySeconds != 0));
    bool? helperSucceedsWithoutGuard = null;
    if (guardRejected)
    {
        success = false;
        // Evidence that the guard (not the compiler) is what rejects: what the real helper says when the guard is bypassed.
        try { helperSucceedsWithoutGuard = ((CampaignCompilationResult)compileForMerge.Invoke(null, [plan, aliases, scenario])!).Success; }
        catch (TargetInvocationException) { helperSucceedsWithoutGuard = null; }
    }
    else
    {
        var result = (CampaignCompilationResult)compileForMerge.Invoke(null, [plan, aliases, scenario])!;
        success = result.Success;
        diagnostics = result.Diagnostics;
        if (success && result.CompiledEvents.Count > 0)
        {
            // REPLICA of the MergeScenarioEvents cap (pinned: merge_scenario_events_cap).
            if (existing.Count + result.CompiledEvents.Count > 256) success = false;
            else merged = RealUniqueEventNames(existing, result.CompiledEvents);
        }
    }

    var row = new JsonObject
    {
        ["name"] = c["name"]!.GetValue<string>(),
        ["success"] = success,
        ["diagnostics"] = JsonSerializer.SerializeToNode(diagnostics, json),
        ["events"] = JsonSerializer.SerializeToNode(success ? merged : [], json),
        ["guardRejected"] = guardRejected,
    };
    if (guardRejected) row["helperSucceedsWithoutGuard"] = helperSucceedsWithoutGuard;
    results.Add(row);
}
var output = new JsonObject { ["sourcePins"] = pins, ["results"] = results };
File.WriteAllText(args[1], output.ToJsonString(json).Replace("\r\n", "\n") + "\n", new UTF8Encoding(false));
Console.WriteLine($"cases={results.Count} pins={pins.Count}");
return 0;

static ScenarioEvent Placeholder(string name) =>
    new(name) { Actions = [new ScenarioAction(ScenarioActionKind.Message, Text: "existing")] };

// Real MapEditorForm.UniqueEventNames. GetUninitializedObject skips the WinForms constructor/field initializers; the
// method only reads EventSession (=> _eventSession), which is set to the existing events here.
List<ScenarioEvent> RealUniqueEventNames(IReadOnlyList<ScenarioEvent> existingEvents, IReadOnlyList<ScenarioEvent> events)
{
    var form = RuntimeHelpers.GetUninitializedObject(formType);
    var session = new ScenarioEventSession();
    session.AddRange(existingEvents);
    eventSessionField.SetValue(form, session);
    return (List<ScenarioEvent>)(uniqueEventNames.Invoke(form, [events]) ?? throw new InvalidOperationException("UniqueEventNames returned null"));
}

// Each fragment is C# text mirrored (or whose contract is relied on) by this oracle / the Rust port. Whitespace is
// collapsed before comparing, so reformatting is fine; any token change fails the run.
static JsonArray? VerifySourcePins()
{
    var dir = new DirectoryInfo(AppContext.BaseDirectory);
    while (dir is not null && !File.Exists(Path.Combine(dir.FullName, "src.MapEditor", "MapEditorForm.Campaign.cs"))) dir = dir.Parent;
    if (dir is null) { Console.Error.WriteLine("SourcePins: cannot locate repo root / src.MapEditor"); return null; }
    static string Norm(string t) => Regex.Replace(t, @"\s+", " ").Trim();
    var sources = new Dictionary<string, string>
    {
        ["MapEditorForm.Campaign.cs"] = Norm(File.ReadAllText(Path.Combine(dir.FullName, "src.MapEditor", "MapEditorForm.Campaign.cs"))),
        ["CampaignWaveDialog.cs"] = Norm(File.ReadAllText(Path.Combine(dir.FullName, "src.MapEditor", "CampaignWaveDialog.cs"))),
    };
    var pinList = new (string Name, string File, string Fragment)[]
    {
        ("apply_campaign_waves_preconditions", "MapEditorForm.Campaign.cs",
            "if (_selected?.IsCustom != true || _eventGraphDirty) return new(false, [],"),
        ("apply_campaign_waves_guard", "MapEditorForm.Campaign.cs",
            "if (plan.FactionProfiles.Count > 0 || plan.Waves.Any(w => w.AssignedPathId.HasValue || w.TargetObjectId.HasValue || w.Squads.Any(s => s.RelativeDelaySeconds != 0)) || plan.Reinforcements.Any(r => r.AssignedPathId.HasValue || r.Squads.Any(s => s.RelativeDelaySeconds != 0))) return new(false, [],"),
        ("apply_campaign_waves_flow", "MapEditorForm.Campaign.cs",
            "var result = CampaignWaveDialog.CompileForMerge(plan, CampaignAliases(), CampaignScenario()); if (!result.Success || result.CompiledEvents.Count == 0) return result; return MergeScenarioEvents(result.CompiledEvents) ? result : new(false, [],"),
        ("merge_scenario_events_cap", "MapEditorForm.Campaign.cs",
            "if (!CanMergeScenarioEvents() || events.Count == 0 || EventSession.Count + events.Count > 256) return false; int index = EventSession.Count; EventSession.AddRange(UniqueEventNames(events));"),
        ("can_merge_scenario_events", "MapEditorForm.Campaign.cs",
            "private bool CanMergeScenarioEvents() => _selected?.IsCustom == true && !_eventGraphDirty && EventSession.Count < 256;"),
        ("unique_event_names_reads_session_only", "MapEditorForm.Campaign.cs",
            "private List<ScenarioEvent> UniqueEventNames(IReadOnlyList<ScenarioEvent> events) { var used = new HashSet<string>(EventSession.Capture().Select(e => e.Name), StringComparer.OrdinalIgnoreCase);"),
        ("compile_for_merge_body", "CampaignWaveDialog.cs",
            "internal static CampaignCompilationResult CompileForMerge(CampaignMissionPlan plan, IReadOnlyCollection<string> aliases, ScenarioDocument scenario) { var result = CampaignMissionCompiler.Compile(plan, null, aliases, scenario); if (!result.Success) return result; try { var merged = scenario.Events.Concat(result.CompiledEvents).ToArray(); ScenarioEventValidator.Validate(merged, aliases); ScenarioEventValidator.ValidateConditions(merged, scenario); return result; } catch (InvalidDataException ex)"),
    };
    var pinsOut = new JsonArray();
    bool ok = true;
    foreach (var (name, file, fragment) in pinList)
    {
        var normalized = Norm(fragment);
        if (!sources[file].Contains(normalized, StringComparison.Ordinal))
        {
            Console.Error.WriteLine($"SourcePins: '{name}' no longer matches src.MapEditor/{file}; re-audit the Rust port and this oracle's replica.");
            ok = false;
        }
        pinsOut.Add(new JsonObject
        {
            ["name"] = name,
            ["file"] = "src.MapEditor/" + file,
            ["sha256"] = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(normalized))).ToLowerInvariant(),
        });
    }
    return ok ? pinsOut : null;
}

static Guid? OptGuid(JsonObject o, string key) => o[key] is JsonNode n ? Guid.Parse(n.GetValue<string>()) : null;

static WaveSquadDefinition[] Squads(JsonObject o) => o["squads"]!.AsArray().Select(s =>
{
    var q = s!.AsObject();
    return new WaveSquadDefinition(q["alias"]!.GetValue<string>(), q["count"]!.GetValue<int>(), q["team"]!.GetValue<int>(),
        q["relativeDelay"]?.GetValue<int>() ?? 0);
}).ToArray();

static CampaignMissionPlan BuildPlan(JsonObject p)
{
    var objectives = (p["objectives"]?.AsArray() ?? []).Select(n =>
    {
        var o = n!.AsObject();
        return new CampaignObjective(Guid.Parse(o["id"]!.GetValue<string>()), Enum.Parse<CampaignObjectiveType>(o["type"]!.GetValue<string>()),
            o["title"]!.GetValue<string>(), o["description"]?.GetValue<string>() ?? "", OptGuid(o, "targetId"),
            o["minX"]?.GetValue<int>() ?? 0, o["minZ"]?.GetValue<int>() ?? 0, o["maxX"]?.GetValue<int>() ?? 16383, o["maxZ"]?.GetValue<int>() ?? 16383,
            o["requiredSeconds"]?.GetValue<int>() ?? 0);
    }).ToArray();
    var waves = (p["waves"]?.AsArray() ?? []).Select(n =>
    {
        var w = n!.AsObject();
        var d = w["moveDestination"]?.AsObject();
        return new WaveAttackDefinition(w["index"]!.GetValue<int>(), w["delay"]!.GetValue<int>(), w["announcement"]?.GetValue<string>() ?? "",
            w["x"]!.GetValue<float>(), w["z"]!.GetValue<float>(), Squads(w), OptGuid(w, "pathId"), OptGuid(w, "targetObjectId"),
            d is null ? null : new ScenarioWorldPosition(d["x"]!.GetValue<int>(), d["z"]!.GetValue<int>()));
    }).ToArray();
    var reinforcements = (p["reinforcements"]?.AsArray() ?? []).Select(n =>
    {
        var r = n!.AsObject();
        return new ReinforcementDefinition(Guid.Parse(r["id"]!.GetValue<string>()), r["name"]!.GetValue<string>(), r["delay"]!.GetValue<int>(),
            r["team"]!.GetValue<int>(), r["x"]!.GetValue<float>(), r["z"]!.GetValue<float>(), Squads(r),
            r["notification"]?.GetValue<string>() ?? "", OptGuid(r, "pathId"));
    }).ToArray();
    var factions = (p["factionProfiles"]?.AsArray() ?? []).Select(n =>
    {
        var f = n!.AsObject();
        return new FactionAiProfile(f["team"]!.GetValue<int>(), f["archetypeId"]!.GetValue<string>(), null, null, f["enabled"]?.GetValue<bool>() ?? true);
    }).ToArray();
    return new CampaignMissionPlan(p["title"]!.GetValue<string>(), p["briefing"]?.GetValue<string>() ?? "", objectives, waves, reinforcements, factions);
}
