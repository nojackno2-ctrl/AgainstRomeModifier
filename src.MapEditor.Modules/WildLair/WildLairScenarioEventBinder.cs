using AgainstRomeMapEditor.Modules.Events;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor.Modules.WildLair;

/// <summary>Adapts explicitly authored timers to the existing ScenarioEvent compiler.
/// Does not generate bytecode, native animal AI, resource regeneration, loot or diplomacy.</summary>
public static class WildLairScenarioEventBinder
{
    public const string LairEventPrefix = "LAIR_";
    private static string Prefix(Guid id) => $"{LairEventPrefix}{id:N}_";

    public static IReadOnlyList<ScenarioEvent> GenerateEventsForLair(
        PlacedNeutralLair lair, NeutralLairDefinition definition, int playerTeam = 0,
        float tileWorldSize = 256f, IReadOnlyList<ScriptObjectAlias>? aliases = null)
    {
        ArgumentNullException.ThrowIfNull(lair);
        NeutralLairCatalog.ValidateDefinition(definition);
        if (lair.InstanceId == Guid.Empty || lair.DefinitionId != definition.Id)
            throw new ArgumentException("A persistent instance ID and matching definition are required.");
        if (definition.Category == LairCategory.WildAnimal)
            throw new NotSupportedException("FigTie animals require single-object creation and neutral team 8. ScenarioEvent SpawnUnit only provides troop creation for teams 0-7.");
        if (aliases is null) throw new ArgumentNullException(nameof(aliases), "Load ScriptObjectAliases from the actual game data before binding.");
        if (!aliases.Any(a => a.NameDef.Equals(definition.NativeBuildingOrLandscapeType, StringComparison.OrdinalIgnoreCase)))
            throw new NotSupportedException("The blueprint's core object is not in the loaded alias catalog.");
        if (lair.Team is < 0 or > 7) throw new NotSupportedException("ScenarioEvent SpawnUnit cannot express neutral team 8.");
        if (!float.IsFinite(tileWorldSize) || tileWorldSize <= 0 ||
            !float.IsFinite(lair.WorldX) || !float.IsFinite(lair.WorldZ) || !float.IsFinite(lair.RotationDeg) ||
            lair.WorldX is < 0 or > 16383 || lair.WorldZ is < 0 or > 16383)
            throw new ArgumentOutOfRangeException(nameof(lair), "Use finite world coordinates and tile scale.");
        if (definition.DefaultGuards.Count != 0 || lair.GuardSpawnIds?.Count > 0)
            throw new NotSupportedException("Guard placement, patrol and death-triggered respawn are not supported by this timer adapter.");
        if (definition.Loot.Wood != 0 || definition.Loot.Food != 0 || definition.Loot.Gold != 0 || definition.Loot.HonorPoints != 0)
            throw new NotSupportedException("The verified event compiler has no resource or honor reward action.");
        if (!string.IsNullOrEmpty(definition.Loot.CompletionMessage) && lair.CoreStructureSpawnId == Guid.Empty)
            throw new NotSupportedException("A completion message requires a tracked ScenarioSpawn core ID.");

        var events = new List<ScenarioEvent>();
        for (int i = 0; i < definition.WaveRules.Count; i++)
        {
            LairWaveSpawnRule wave = definition.WaveRules[i];
            if (wave.InitialDelaySeconds != wave.IntervalSeconds || wave.MaxActiveWaves != 0 ||
                !string.Equals(wave.AggroBehavior, "None", StringComparison.OrdinalIgnoreCase))
                throw new NotSupportedException("A ScenarioEvent has one timer period; separate initial delays, active-wave caps and aggression orders cannot be expressed. Use MaxActiveWaves=0 (uncapped) and AggroBehavior=None.");
            // SpawnUnit supplies an infantry unit container plus a faction figure member alias.
            // Animal, civilian, building and landscape aliases are not interchangeable with members.
            ScriptObjectAlias? alias = aliases.SingleOrDefault(a => a.Alias.Equals(wave.UnitAlias, StringComparison.OrdinalIgnoreCase));
            if (alias is null || !IsTroopMember(alias.NameDef))
                throw new NotSupportedException($"Alias {wave.UnitAlias} is not an observed faction infantry member for SpawnUnit.");
            if (!float.IsFinite(wave.SpawnRadiusTiles) || wave.SpawnRadiusTiles < 0)
                throw new ArgumentOutOfRangeException(nameof(definition), "Spawn radius must be finite and nonnegative.");
            float angle = i * MathF.PI / 2 + lair.RotationDeg * MathF.PI / 180;
            float x = lair.WorldX + MathF.Cos(angle) * wave.SpawnRadiusTiles * tileWorldSize;
            float z = lair.WorldZ + MathF.Sin(angle) * wave.SpawnRadiusTiles * tileWorldSize;
            var item = new ScenarioEvent($"{Prefix(lair.InstanceId)}WAVE_{i}", wave.IntervalSeconds, true, lair.IsActive)
            {
                Actions = [new(ScenarioActionKind.SpawnUnit, Team: lair.Team, Alias: alias.Alias,
                    X: x, Z: z, Count: wave.SpawnCount)]
            };
            if (lair.CoreStructureSpawnId != Guid.Empty)
                item.Conditions.Add(new(ScenarioConditionKind.ObjectExists, lair.CoreStructureSpawnId));
            events.Add(item);
        }
        if (!string.IsNullOrEmpty(definition.Loot.CompletionMessage))
            events.Add(new ScenarioEvent($"{Prefix(lair.InstanceId)}CLEARED", 1, false, lair.IsActive)
            {
                Conditions = [new(ScenarioConditionKind.ObjectDeadOrRemoved, lair.CoreStructureSpawnId)],
                Actions = [new(ScenarioActionKind.Message, Text: definition.Loot.CompletionMessage)]
            });
        ScenarioEventValidator.Validate(events, aliases.Select(a => a.Alias).ToArray());
        return events;
    }

    private static bool IsTroopMember(string name) =>
        new[] { "FigGerInf", "FigHunInf", "FigKelInf", "FigRomInf" }
            .Any(prefix => name.StartsWith(prefix, StringComparison.Ordinal));

    public static IReadOnlyList<ScenarioEvent> GenerateEventsForResource(TimedResourceReplacement request)
    {
        ArgumentNullException.ThrowIfNull(request);
        new ResourceRegenerationPlanner().PlanReplacement(request.NameDef, request.WorldX, request.WorldZ, request.DelaySeconds, request.Team);
        throw new NotSupportedException("ScenarioEvent has no landscape/single-object spawn or depletion/replenishment action. Tree, stone, field, mine and goldsmith timers are gated; no bytecode is generated.");
    }

    public static void SyncLairEvents(ScenarioEventSession session, PlacedNeutralLair lair,
        NeutralLairDefinition definition, int playerTeam = 0, IReadOnlyList<ScriptObjectAlias>? aliases = null,
        float tileWorldSize = 256f)
    {
        ArgumentNullException.ThrowIfNull(session);
        var replacement = GenerateEventsForLair(lair, definition, playerTeam, tileWorldSize, aliases);
        var current = session.Capture();
        int retained = current.Count(e => !e.Name.StartsWith(Prefix(lair.InstanceId), StringComparison.Ordinal));
        if (retained + replacement.Count > 256)
            throw new InvalidOperationException("Event limit exceeded; existing session was preserved.");
        RemoveLairEvents(session, lair.InstanceId);
        foreach (var item in replacement) session.Add(item);
    }

    public static int RemoveLairEvents(ScenarioEventSession session, Guid instanceId)
    {
        ArgumentNullException.ThrowIfNull(session);
        var events = session.Capture();
        int removed = 0;
        for (int i = events.Count - 1; i >= 0; i--)
            if (events[i].Name.StartsWith(Prefix(instanceId), StringComparison.Ordinal))
            { session.RemoveAt(i); removed++; }
        return removed;
    }

    public static IReadOnlyList<string> ValidateLairBindings(IReadOnlyList<ScenarioEvent> events,
        IReadOnlyList<PlacedNeutralLair> activeLairs)
    {
        ArgumentNullException.ThrowIfNull(events);
        ArgumentNullException.ThrowIfNull(activeLairs);
        var ids = activeLairs.Select(l => l.InstanceId).ToHashSet();
        return events.Where(e => e.Name.StartsWith(LairEventPrefix, StringComparison.Ordinal))
            .Where(e => e.Name.Length < LairEventPrefix.Length + 33 ||
                e.Name[LairEventPrefix.Length + 32] != '_' ||
                !Guid.TryParseExact(e.Name.Substring(LairEventPrefix.Length, 32), "N", out Guid id) || !ids.Contains(id))
            .Select(e => $"Event {e.Name} has an unknown or legacy lair instance reference.").ToArray();
    }
}
