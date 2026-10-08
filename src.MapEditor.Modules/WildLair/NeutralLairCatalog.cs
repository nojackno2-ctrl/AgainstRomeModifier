namespace AgainstRomeMapEditor.Modules.WildLair;

/// <summary>Observed wild animals and explicitly authored scenario blueprints.
/// Default records do not claim native lair mechanics or threat ratings.</summary>
public sealed class NeutralLairCatalog
{
    private static readonly Lazy<NeutralLairCatalog> _defaultInstance = new(() => new NeutralLairCatalog(CreateCanonicalDefinitions()));
    public static NeutralLairCatalog Default => _defaultInstance.Value;

    private readonly Dictionary<string, NeutralLairDefinition> _definitions;

    public IReadOnlyList<NeutralLairDefinition> AllDefinitions => _definitions.Values.ToArray();

    public NeutralLairCatalog(IEnumerable<NeutralLairDefinition> definitions)
    {
        ArgumentNullException.ThrowIfNull(definitions);
        _definitions = new Dictionary<string, NeutralLairDefinition>(StringComparer.OrdinalIgnoreCase);
        foreach (var def in definitions)
        {
            ValidateDefinition(def);
            _definitions[def.Id] = def;
        }
    }

    public NeutralLairDefinition? GetById(string id)
    {
        if (string.IsNullOrWhiteSpace(id)) return null;
        return _definitions.GetValueOrDefault(id);
    }

    public IEnumerable<NeutralLairDefinition> GetByCategory(LairCategory category) =>
        _definitions.Values.Where(def => def.Category == category);

    public IEnumerable<NeutralLairDefinition> GetByTier(LairDifficultyTier tier) =>
        _definitions.Values.Where(def => def.Tier == tier);

    public IEnumerable<NeutralLairDefinition> Filter(
        LairCategory? category = null,
        LairDifficultyTier? tier = null,
        string? biome = null)
    {
        return _definitions.Values.Where(def =>
            (!category.HasValue || def.Category == category.Value) &&
            (!tier.HasValue || def.Tier == tier.Value) &&
            (string.IsNullOrWhiteSpace(biome) || string.Equals(def.BiomeAffinity, biome, StringComparison.OrdinalIgnoreCase)));
    }

    /// <summary>No native threat formula was recovered. Empty observations have no authored
    /// threat; populated blueprints cannot be scored from invented combat coefficients.</summary>
    public static float CalculateThreatRating(NeutralLairDefinition definition)
    {
        ArgumentNullException.ThrowIfNull(definition);
        if (definition.DefaultGuards.Count == 0 && definition.WaveRules.Count == 0) return 0;
        throw new NotSupportedException("No evidence-based native threat formula is available.");
    }

    public static void ValidateDefinition(NeutralLairDefinition def)
    {
        ArgumentNullException.ThrowIfNull(def);
        if (string.IsNullOrWhiteSpace(def.Id))
            throw new ArgumentException("巢穴識別碼不得為空。", nameof(def));
        if (string.IsNullOrWhiteSpace(def.DisplayNameZh))
            throw new ArgumentException("巢穴繁體中文名稱不得為空。", nameof(def));
        if (string.IsNullOrWhiteSpace(def.NativeBuildingOrLandscapeType))
            throw new ArgumentException("原生建築或地景代碼不得為空。", nameof(def));
        if (!float.IsFinite(def.FootprintRadiusTiles) || def.FootprintRadiusTiles <= 0f)
            throw new ArgumentOutOfRangeException(nameof(def), "佔地半徑必須大於 0。");
        if (def.DefaultGuards is null)
            throw new ArgumentNullException(nameof(def), "常駐守衛清單不得為 null。");
        if (def.WaveRules is null)
            throw new ArgumentNullException(nameof(def), "刷怪波次規則不得為 null。");
        if (def.Loot is null)
            throw new ArgumentNullException(nameof(def), "戰利品獎勵不得為 null。");

        foreach (var wave in def.WaveRules)
        {
            if (wave.IntervalSeconds < 10)
                throw new ArgumentOutOfRangeException(nameof(def), $"刷怪週期過短（{wave.IntervalSeconds} 秒），最低間隔須 >= 10 秒。");
            if (wave.SpawnCount is < 1 or > 20)
                throw new ArgumentOutOfRangeException(nameof(def), $"刷怪人數必須介於 1 至 20 人之間。");
        }
    }

    private static IReadOnlyList<NeutralLairDefinition> CreateCanonicalDefinitions() =>
    [
        Animal("ALL_WOL00", "FigTieWol00_Wilder_Wolf", "Wild wolf"),
        Animal("ALL_BAE00", "FigTieBae00_Baer", "Bear"),
        Animal("ALL_EBE00", "FigTieEbe00_Wildschwein", "Wild boar"),
        Animal("ALL_RAU00", "FigTieRau00_Raubkatze", "Wild cat")
    ];

    private static NeutralLairDefinition Animal(string alias, string name, string display) =>
        new(alias, display, display, LairCategory.WildAnimal, LairDifficultyTier.Unrated, name, 1,
            [], [], new LairLootReward(0, 0, 0, 0, ""), "",
            "Observed in objdef and cl_scint.ini; no native lair, patrol, reward or respawn rule established.")
        { ScriptAlias = alias };

    /// <summary>Require the loaded game's alias to resolve to exactly the observed definition.</summary>
    public NeutralLairCatalog FilteredBy(IReadOnlyList<AgainstRomeModifier.Scripting.ScriptObjectAlias> aliases)
    {
        ArgumentNullException.ThrowIfNull(aliases);
        return new NeutralLairCatalog(AllDefinitions.Where(d => aliases.Any(a =>
            string.Equals(a.Alias, d.ScriptAlias, StringComparison.OrdinalIgnoreCase) &&
            string.Equals(a.NameDef, d.NativeBuildingOrLandscapeType, StringComparison.OrdinalIgnoreCase))));
    }
}
