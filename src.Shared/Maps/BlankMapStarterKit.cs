using System.Collections.ObjectModel;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Maps;

/// <summary>Detached main-house family and four safe individual units. Creation reads sources only;
/// the caller validates terrain/clearance and accepts both returned store and scenario.</summary>
public sealed class BlankMapStarterKit
{
    public const int MaximumObjects = 14000;
    public const string MainHouseAlias = "GER_HAU00";
    public const string MainHouseName = "BauGerHau00_Haupthaus";
    public const string UnitAlias = "GER_INF02";
    public const string UnitName = "FigGerInf02_Zweihandaxt";

    private readonly IReadOnlyDictionary<string, LevelObjectTemplate> _templates;
    private readonly LevelObjectStore? _houseSource;
    private readonly LevelWorldObject? _houseRoot;

    private BlankMapStarterKit(ScenarioSpawn[] spawns, Dictionary<string, LevelObjectTemplate> templates,
        LevelObjectStore? houseSource, LevelWorldObject? houseRoot, string? sourceMap)
    {
        Spawns = Array.AsReadOnly(spawns);
        _templates = new ReadOnlyDictionary<string, LevelObjectTemplate>(templates);
        _houseSource = houseSource?.CloneForEditing();
        _houseRoot = houseRoot;
        HouseSourceMapPath = sourceMap;
        if (_houseSource is not null && houseRoot is not null)
        {
            var family = _houseSource.FamilyAt(houseRoot.Slot, houseRoot.Uid)
                ?? throw new InvalidDataException("Starter house family is no longer valid.");
            HouseFamilyObjectCount = family.ObjectSlots.Count;
            var members = family.ObjectSlots.ToHashSet();
            var spawn = spawns[0];
            double dx = FixedDelta(houseRoot.X, spawn.X) / 65536d, dy = FixedDelta(houseRoot.Y, spawn.Y) / 65536d,
                dz = FixedDelta(houseRoot.Z, spawn.Z) / 65536d;
            HouseFamilyPlacements = Array.AsReadOnly(_houseSource.Objects().Where(obj => members.Contains(obj.Slot))
                .Select(obj => obj with { X = (float)(obj.X + dx), Y = (float)(obj.Y + dy),
                    Z = (float)(obj.Z + dz), Team = spawn.Team }).ToArray());
            foreach (var obj in HouseFamilyPlacements) CheckPosition(obj.X, obj.Y, obj.Z);
        }
        else
        {
            HouseFamilyObjectCount = 1;
            HouseFamilyPlacements = Array.Empty<LevelWorldObject>();
        }
    }

    public IReadOnlyList<ScenarioSpawn> Spawns { get; }
    /// <summary>Safe singleton templates only. A linked main house is deliberately absent.</summary>
    public IReadOnlyDictionary<string, LevelObjectTemplate> Templates => _templates;
    public int HouseFamilyObjectCount { get; }
    public int NativeObjectCount => HouseFamilyObjectCount + 4;
    public string? HouseSourceMapPath { get; }
    /// <summary>Member extent at the intended destination; slots/UIDs identify source members, not target bindings.</summary>
    public IReadOnlyList<LevelWorldObject> HouseFamilyPlacements { get; }

    public static BlankMapStarterKit Create(string gamePath, string templateMapPath,
        int team, float x, float z, Func<float, float, float>? height = null,
        IEnumerable<ScenarioSpawn>? existingPlacements = null)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(gamePath);
        ArgumentException.ThrowIfNullOrWhiteSpace(templateMapPath);
        if (team is < 0 or > 7) throw new ArgumentOutOfRangeException(nameof(team), "Starter kit team must be 0–7.");
        CheckPosition(x, 0, z);
        var aliases = ScriptObjectAliases.Load(gamePath);
        var names = ObjDefNames.Load(gamePath);
        int Resolve(string alias, string name)
        {
            var matches = aliases.Where(a => a.Alias.Equals(alias, StringComparison.OrdinalIgnoreCase)).ToArray();
            var ids = names.Where(p => p.Value.Equals(name, StringComparison.OrdinalIgnoreCase)).Select(p => p.Key).ToArray();
            if (matches.Length != 1 || !matches[0].NameDef.Equals(name, StringComparison.OrdinalIgnoreCase) || ids.Length != 1)
                throw new InvalidDataException($"Starter kit requires verified alias {alias} → {name} and a unique objdef TypeId.");
            return ids[0];
        }
        int houseId = Resolve(MainHouseAlias, MainHouseName), unitId = Resolve(UnitAlias, UnitName);
        var source = LevelObjectStore.Load(templateMapPath);
        if (source.Capacity is < 5 or > MaximumObjects)
            throw new InvalidDataException("Template map capacity is outside the native 14000-object limit.");
        var templates = source.Templates().Where(t => t.TypeId == houseId || t.TypeId == unitId).ToDictionary(t => t.TypeId);
        if (!templates.ContainsKey(houseId) || !templates.ContainsKey(unitId))
            foreach (var pair in LevelObjectStore.LoadOfficialTemplates(gamePath, id => id == houseId || id == unitId))
                templates.TryAdd(pair.Key, pair.Value);

        LevelObjectStore? familySource = null;
        LevelWorldObject? root = null;
        string? sourceMap = null;
        // Prefer the known complete official tutorial family. Custom maps never supply linked houses.
        string maps = Path.Combine(gamePath, "MAPS");
        if (Directory.Exists(maps))
            foreach (string map in Directory.GetDirectories(maps)
                .OrderBy(path => Path.GetFileName(path).Equals("TUTOR_01", StringComparison.OrdinalIgnoreCase) ? 0 : 1)
                .ThenBy(path => path, StringComparer.OrdinalIgnoreCase))
            {
                if (CustomMapManifest.IsCustomMapDirectory(map) || !File.Exists(Path.Combine(map, "DATA", "objects.dat"))) continue;
                try
                {
                    var official = LevelObjectStore.Load(map);
                    var house = official.Objects().Where(obj => obj.TypeId == houseId)
                        .FirstOrDefault(obj => official.FamilyAt(obj.Slot, obj.Uid) is { ObjectSlots.Count: > 1 });
                    if (house is null || !float.IsFinite(house.Rotation)) continue;
                    familySource = official; root = house; sourceMap = Path.GetFullPath(map); break;
                }
                catch (Exception ex) when (ex is IOException or InvalidDataException) { }
            }
        bool hasHouse = templates.TryGetValue(houseId, out var houseTemplate);
        bool hasUnit = templates.TryGetValue(unitId, out var unitTemplate);
        if ((!hasHouse && root is null) || !hasUnit)
            throw new InvalidDataException("Starter kit missing safe DATA templates or complete official family: "
                + (!hasHouse && root is null ? MainHouseAlias + " " : "")
                + (!hasUnit ? UnitAlias : ""));

        (string Alias, float Dx, float Dz)[] layout =
        [ (MainHouseAlias, 0, 0), (UnitAlias, -480, 800), (UnitAlias, -160, 800),
            (UnitAlias, 160, 800), (UnitAlias, 480, 800) ];
        var spawns = layout.Select(item =>
        {
            float px = x + item.Dx, pz = z + item.Dz;
            CheckPosition(px, 0, pz);
            float py = height?.Invoke(px, pz) ?? 0;
            CheckPosition(px, py, pz);
            return new ScenarioSpawn(item.Alias, px, pz, team, Y: py, Prebuilt: true) { Id = Guid.NewGuid() };
        }).ToArray();
        if (root is not null)
            spawns[0] = spawns[0] with { RawRotation = root.Rotation, Angle = ScenarioLevelObjects.RadiansToDisplayAngle(root.Rotation) };
        var resolved = new Dictionary<string, LevelObjectTemplate>(StringComparer.OrdinalIgnoreCase) { [UnitAlias] = unitTemplate! };
        if (root is null) resolved[MainHouseAlias] = houseTemplate!;
        var result = new BlankMapStarterKit(spawns, resolved, familySource, root, sourceMap);
        CheckBudget(existingPlacements ?? Array.Empty<ScenarioSpawn>(), result.NativeObjectCount);
        var blank = source.CloneForEditing();
        blank.ResetForBlankMap();
        result.ApplyToCandidate(blank, new ScenarioDocument());
        return result;
    }

    public LevelObjectTemplate? TemplateFor(ScenarioSpawn spawn)
    {
        ArgumentNullException.ThrowIfNull(spawn);
        return _templates.GetValueOrDefault(spawn.Alias);
    }

    /// <summary>Imports then binds the complete house root to the existing native family save path.
    /// Neither input is mutated, including when a later unit allocation fails.</summary>
    public ScenarioCandidateResult ApplyToCandidate(LevelObjectStore store, ScenarioDocument previous,
        Func<ScenarioSpawn, LevelObjectTemplate?>? existingTemplateFor = null)
    {
        ArgumentNullException.ThrowIfNull(store);
        ArgumentNullException.ThrowIfNull(previous);
        CheckBudget(previous.Spawns, NativeObjectCount);
        // Bound families have more DATA objects than designer placements. Count the live store once,
        // then add only unbound/script placement budgets, avoiding double counting existing bindings.
        long budget = store.Objects().Count + NativeObjectCount;
        foreach (var spawn in previous.Spawns)
            if (!spawn.Prebuilt || !previous.DataSlots.Any(binding => binding.SpawnId == spawn.Id && store.UidAt(binding.Slot) == binding.Uid))
                budget += spawn.Count > 0 ? spawn.Count + 1 : 1;
        if (store.Capacity > MaximumObjects || budget > Math.Min(store.Capacity, MaximumObjects))
            throw new InvalidDataException("Starter kit exceeds the native DATA object capacity.");
        if (previous.Spawns.Any(spawn => Spawns.Any(kit => kit.Id == spawn.Id)))
            throw new InvalidDataException("Starter kit has already been applied to this scenario.");
        var seededPrevious = new ScenarioDocument
        {
            Version = previous.Version, Spawns = previous.Spawns.ToList(),
            DataSlots = previous.DataSlots.ToList(), Events = previous.Events.ToList()
        };
        var working = store;
        if (_houseSource is not null && _houseRoot is { } root)
        {
            var house = Spawns[0];
            var imported = store.CreateFamilyImportCandidate(_houseSource, root.Slot, root.Uid,
                FixedDelta(root.X, house.X), FixedDelta(root.Y, house.Y), FixedDelta(root.Z, house.Z), house.Team)
                ?? throw new InvalidDataException("Starter house family could not be allocated in the target pools.");
            working = imported.Store;
            seededPrevious.Spawns.Add(house);
            seededPrevious.DataSlots.Add(new ScenarioDataSlot(imported.RootSlot, imported.RootUid) { SpawnId = house.Id });
        }
        var next = new ScenarioDocument
        {
            Version = previous.Version, Spawns = previous.Spawns.Concat(Spawns).ToList(),
            DataSlots = seededPrevious.DataSlots.ToList(), Events = previous.Events.ToList()
        };
        var candidate = ScenarioLevelObjects.ApplyToCandidate(working, seededPrevious, next,
            spawn => TemplateFor(spawn) ?? existingTemplateFor?.Invoke(spawn));
        if (candidate.Skipped.Count != 0)
            throw new InvalidDataException("Starter kit DATA transaction skipped required templates: "
                + string.Join(", ", candidate.Skipped.Select(spawn => spawn.Alias)));
        if (candidate.Scenario.ScriptSpawns.Any(spawn => Spawns.Any(kit => kit.Id == spawn.Id))
            || Spawns.Any(spawn => candidate.Scenario.DataSlots.Count(binding => binding.SpawnId == spawn.Id) != 1))
            throw new InvalidDataException("Starter kit was not completely bound to DATA.");
        return candidate;
    }

    private static int FixedDelta(float source, float target)
    {
        double value = ((double)target - source) * 65536d;
        if (!double.IsFinite(value) || value < int.MinValue || value > int.MaxValue || value != Math.Truncate(value))
            throw new InvalidDataException("Starter family displacement exceeds signed16.16 precision or range.");
        return (int)value;
    }

    private static void CheckBudget(IEnumerable<ScenarioSpawn> existing, int added)
    {
        long count = added;
        foreach (var spawn in existing)
        {
            if (spawn is null || spawn.Count is < 0 or > 20)
                throw new InvalidDataException("Existing placements contain an invalid troop count.");
            count += spawn.Count > 0 ? spawn.Count + 1 : 1;
            if (count > MaximumObjects) throw new InvalidDataException("Starter kit exceeds 14000 objects.");
        }
    }

    private static void CheckPosition(float x, float y, float z)
    {
        if (!float.IsFinite(x) || !float.IsFinite(y) || !float.IsFinite(z) || x is < 0 or > 16383 || z is < 0 or > 16383)
            throw new ArgumentOutOfRangeException(nameof(x), "Starter kit positions must be finite with X/Z between 0 and 16383.");
    }
}
