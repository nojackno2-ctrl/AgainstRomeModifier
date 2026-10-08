namespace AgainstRomeMapEditor.Modules.Objectives;

using System.Text.Json;
using AgainstRomeModifier;
using AgainstRomeModifier.Scripting;

/// <summary>Validated ScenarioEvents; BciBytes is populated only by CompileToBci.</summary>
public sealed record BciObjectiveCompilationResult(
    bool Success,
    IReadOnlyList<ScenarioEvent> CompiledEvents,
    IReadOnlyList<ObjectiveDiagnostic> Diagnostics,
    string Summary)
{
    public byte[]? BciBytes { get; init; }
}

/// <summary>Conservative lowering to the existing ScenarioEvent compiler. No objective-specific bytecode.</summary>
public static class BciObjectiveCompiler
{
    private static readonly JsonSerializerOptions JsonOptions = new() { WriteIndented = true };

    public static BciObjectiveCompilationResult Compile(ObjectiveDependencyGraph graph,
        ScenarioDocument? scenario = null, IReadOnlyCollection<string>? knownAliases = null)
    {
        ArgumentNullException.ThrowIfNull(graph);
        var diagnostics = graph.Validate().ToList();
        void Error(string code, string message, Guid? id = null) =>
            diagnostics.Add(new(ObjectiveDiagnosticSeverity.Error, code, message, id));
        if (graph.Objectives.Count(o => o.Category == ObjectiveCategory.Primary) != 1)
            Error("OBJ_PRIMARY_LIMIT", "Export requires exactly one primary objective; completion latches for multiple primaries are unavailable.");
        if (graph.Dependencies.Count != 0)
            Error("OBJ_DEPENDENCY_UNSUPPORTED", "Prerequisites, AND/OR parents and exclusive branches are simulation-only; no event completion primitive is exposed.");
        foreach (var obj in graph.Objectives)
        {
            foreach (string message in ObjectiveRuleCatalog.ValidateForCompilation(obj).Errors)
                Error("OBJ_EXPORT_UNSUPPORTED", message, obj.Id);
        }
        if (diagnostics.Any(d => d.Severity == ObjectiveDiagnosticSeverity.Error)) return Failure(diagnostics);

        var failures = new List<ScenarioEvent>();
        var completions = new List<ScenarioEvent>();
        foreach (var obj in graph.Objectives)
        {
            if (obj.Category == ObjectiveCategory.FailureCriterion || obj.Kind is ObjectiveKind.Survival or ObjectiveKind.EscortUnit)
            {
                // Defeat must precede any possible victory, including a corpse still inside the arrival area.
                var failed = new ScenarioEvent($"OBJ_FAIL_{obj.Id:N}", 0);
                failed.Conditions.Add(new(ScenarioConditionKind.ObjectDeadOrRemoved, obj.Parameters.TargetGuids.Single()));
                failed.Actions.Add(new(ScenarioActionKind.Message, Text: $"Objective failed: {obj.Title}"));
                failed.Actions.Add(new(ScenarioActionKind.Defeat));
                failures.Add(failed);
                if (obj.Category == ObjectiveCategory.FailureCriterion) continue;
            }
            var completed = new ScenarioEvent($"OBJ_WIN_{obj.Id:N}",
                obj.Kind == ObjectiveKind.Survival ? obj.Parameters.HoldDurationSeconds : 0);
            switch (obj.Kind)
            {
                case ObjectiveKind.DestroyBuilding:
                case ObjectiveKind.AssassinateTarget:
                    completed.Conditions.AddRange(obj.Parameters.TargetGuids.Select(id => new ScenarioCondition(ScenarioConditionKind.ObjectDeadOrRemoved, id)));
                    break;
                case ObjectiveKind.Survival:
                    completed.Conditions.Add(new(ScenarioConditionKind.ObjectExists, obj.Parameters.TargetGuids.Single()));
                    break;
                case ObjectiveKind.CaptureArea:
                case ObjectiveKind.EscortUnit:
                    var area = obj.Parameters.Area!;
                    completed.Conditions.Add(new(ScenarioConditionKind.ObjectInArea, obj.Parameters.TargetGuids.Single(),
                        area.MinX, area.MinZ, area.MaxX, area.MaxZ));
                    break;
                case ObjectiveKind.CustomScripted:
                    completed.Conditions.AddRange(obj.Parameters.CustomConditions);
                    break;
                default: throw new InvalidOperationException("Export validation must reject unsupported objective kinds.");
            }
            completed.Actions.Add(new(ScenarioActionKind.Message, Text: $"Objective completed: {obj.Title}"));
            if (obj.Reward is { } reward)
            {
                if (!string.IsNullOrWhiteSpace(reward.CompletionMessage))
                    completed.Actions.Add(new(ScenarioActionKind.Message, Text: reward.CompletionMessage));
                completed.Actions.AddRange(reward.Actions);
            }
            if (obj.Category == ObjectiveCategory.Primary) completed.Actions.Add(new(ScenarioActionKind.Victory));
            completions.Add(completed);
        }
        // Run bonus completions before the primary terminal event; failure always has priority.
        var events = failures.Concat(completions.OrderBy(e => e.Actions.Any(a => a.Kind == ScenarioActionKind.Victory))).ToList();
        try
        {
            ScenarioEventValidator.Validate(events, knownAliases ?? Array.Empty<string>());
            ScenarioEventValidator.ValidateConditions(events, scenario is null ? null : ValidatedScenarioCopy(scenario));
        }
        catch (Exception ex) when (ex is InvalidDataException or System.Text.EncoderFallbackException)
        {
            Error("OBJ_SCENARIO_INVALID", ex.Message);
            return Failure(diagnostics);
        }
        if (scenario is null)
            diagnostics.Add(new(ObjectiveDiagnosticSeverity.Warning, "OBJ_BINDING_UNCHECKED", "Event structure validated; target bindings require a ScenarioDocument before BCI generation."));
        diagnostics.Add(new(ObjectiveDiagnosticSeverity.Info, "OBJ_GAMEPLAY_UNVERIFIED", "Uses existing ScenarioEvent primitives; this objective composition has not been tested in game."));
        return new(true, events, diagnostics, $"Compiled {graph.Objectives.Count} objectives to {events.Count} validated ScenarioEvents.");
    }

    /// <summary>Compile a pristine raw BCI0 or PFIL original in memory. Caller must not supply an already injected script.
    /// Existing events are preserved; source bytes and scenario remain unchanged. File saving uses LevelScriptInjector.Apply.</summary>
    public static BciObjectiveCompilationResult CompileToBci(ObjectiveDependencyGraph graph, byte[] original,
        ScenarioDocument scenario, IReadOnlyCollection<string> knownAliases)
    {
        ArgumentNullException.ThrowIfNull(original);
        ArgumentNullException.ThrowIfNull(scenario);
        ArgumentNullException.ThrowIfNull(knownAliases);
        var result = Compile(graph, scenario, knownAliases);
        if (!result.Success) return result;
        try
        {
            scenario = ValidatedScenarioCopy(scenario);
            var events = scenario.Events.Concat(result.CompiledEvents).ToList();
            ScenarioEventValidator.Validate(events, knownAliases);
            ScenarioEventValidator.ValidateConditions(events, scenario);
            foreach (var spawn in scenario.ScriptSpawns)
                if (!knownAliases.Contains(spawn.Alias, StringComparer.OrdinalIgnoreCase))
                    throw new InvalidDataException($"Unknown spawn alias: {spawn.Alias}");
            bool pfil = original.AsSpan().StartsWith("PFIL"u8);
            var image = BciImage.Parse(pfil ? GameLZSS.DecompressPfil(original) : original);
            int originalMain = image.MainAddress;
            if (scenario.ScriptSpawns.Count > 0) LevelScriptInjector.Inject(image, scenario.ScriptSpawns);
            ScenarioEventCompiler.Inject(image, events, originalMain, scenario);
            byte[] raw = image.Serialize();
            if (!raw.SequenceEqual(BciImage.Parse(raw).Serialize()))
                throw new InvalidDataException("BCI round trip failed.");
            return result with { BciBytes = pfil ? GameLZSS.CompressPfil(raw, original.AsSpan(0, 64).ToArray()) : raw };
        }
        catch (Exception ex) when (ex is InvalidDataException or ArgumentException or OverflowException)
        {
            return Failure(result.Diagnostics.Append(new ObjectiveDiagnostic(ObjectiveDiagnosticSeverity.Error,
                "OBJ_BCI_INVALID", ex.Message)).ToList());
        }
    }

    private static ScenarioDocument ValidatedScenarioCopy(ScenarioDocument scenario)
    {
        if (scenario.Spawns.Any(spawn => spawn is null || spawn.Id == Guid.Empty))
            throw new InvalidDataException("Scenario placements require persistent identities before objective compilation.");
        var copy = new ScenarioDocument
        {
            Version = scenario.Version, Spawns = scenario.Spawns.ToList(),
            DataSlots = scenario.DataSlots.ToList(), Events = scenario.Events.ToList()
        };
        ScenarioObjectIdentity.Prepare(copy); // Detect duplicate identities, owners and physical slots without changing caller data.
        return copy;
    }

    private static BciObjectiveCompilationResult Failure(IReadOnlyList<ObjectiveDiagnostic> diagnostics) =>
        new(false, Array.Empty<ScenarioEvent>(), diagnostics, "Objective export rejected; no events or BCI emitted.");

    #region Serialization & Lossless Metadata

    /// <summary>
    /// 將目標依賴圖序列化為 JSON 字串，以利儲存於 arm_scenario.json 或獨立設定檔。
    /// </summary>
    public static string SerializeGraph(ObjectiveDependencyGraph graph)
    {
        ArgumentNullException.ThrowIfNull(graph);
        var dto = new ObjectiveGraphDto(
            Objectives: graph.Objectives.ToList(),
            Dependencies: graph.Dependencies.ToList());
        return JsonSerializer.Serialize(dto, JsonOptions);
    }

    /// <summary>
    /// 從 JSON 字串反序列化並重建 ObjectiveDependencyGraph。
    /// </summary>
    public static ObjectiveDependencyGraph DeserializeGraph(string json)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(json);
        var dto = JsonSerializer.Deserialize<ObjectiveGraphDto>(json, JsonOptions)
            ?? throw new InvalidDataException("無法解析目標依賴圖資料。");

        var graph = new ObjectiveDependencyGraph();
        foreach (var obj in dto.Objectives)
        {
            graph.AddObjective(obj);
        }
        foreach (var dep in dto.Dependencies)
        {
            graph.AddDependency(dep.SourceObjectiveId, dep.TargetObjectiveId, dep.Relation);
        }
        return graph;
    }

    private sealed record ObjectiveGraphDto(
        List<ObjectiveDefinition> Objectives,
        List<ObjectiveDependency> Dependencies);

    #endregion
}
