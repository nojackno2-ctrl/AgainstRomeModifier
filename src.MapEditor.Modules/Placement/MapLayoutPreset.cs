using System.Text.Json;
using System.Text.Json.Serialization;
using AgainstRomeModifier.Maps;
using AgainstRomeMapEditor.Modules.Nature;

namespace AgainstRomeMapEditor.Modules.Placement;

public enum MapLayoutKind { Placement, Nature }
public sealed record MapLayoutEntry(string Type, float X, float Z, float HeightOffset, float Angle, int Team = 0, int Count = 0);
public sealed record MapLayoutPreset(int Version, MapLayoutKind Kind, IReadOnlyList<MapLayoutEntry> Entries);

/// <summary>Portable layouts contain names and relative transforms, never native records or event identities.</summary>
public static class MapLayoutPresets
{
    private static readonly JsonSerializerOptions Json = new()
    {
        WriteIndented = true, PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        UnmappedMemberHandling = JsonUnmappedMemberHandling.Disallow,
        Converters = { new JsonStringEnumConverter() }
    };
    public static string Serialize(MapLayoutPreset preset) { Validate(preset); return JsonSerializer.Serialize(preset, Json); }
    public static MapLayoutPreset Deserialize(string json)
    {
        if (json.Length > 2_000_000) throw new InvalidDataException("Layout is too large.");
        var preset = JsonSerializer.Deserialize<MapLayoutPreset>(json, Json) ?? throw new InvalidDataException("Empty layout.");
        Validate(preset); return preset;
    }
    public static void Validate(MapLayoutPreset preset)
    {
        ArgumentNullException.ThrowIfNull(preset);
        if (preset.Version != 1 || !Enum.IsDefined(preset.Kind) || preset.Entries is null || preset.Entries.Count is < 1 or > 4096)
            throw new InvalidDataException("Unsupported layout version, kind or object count.");
        foreach (var entry in preset.Entries)
            if (entry is null || string.IsNullOrWhiteSpace(entry.Type) || entry.Type.Length > 256 ||
                !float.IsFinite(entry.X) || !float.IsFinite(entry.Z) || !float.IsFinite(entry.HeightOffset) || !float.IsFinite(entry.Angle) ||
                Math.Abs(entry.X) > 16383 || Math.Abs(entry.Z) > 16383 || Math.Abs(entry.HeightOffset) > 16383 ||
                entry.Team is < -1 or > 15 || entry.Count is < 0 or > 20 ||
                preset.Kind == MapLayoutKind.Nature && (entry.Team != 0 || entry.Count != 0))
                throw new InvalidDataException("Invalid layout object.");
    }
    public static IReadOnlyList<SdlPlacedObject> PlanPlacements(MapLayoutPreset preset, IReadOnlyList<SdlObjectType> catalog,
        float anchorX, float anchorZ, float rotation, Func<float, float, float> groundHeight, int? team = null)
    {
        Validate(preset);
        if (preset.Kind != MapLayoutKind.Placement) throw new InvalidDataException("Expected a placement layout.");
        var result = new List<SdlPlacedObject>();
        foreach (var entry in preset.Entries)
        {
            var type = catalog.FirstOrDefault(type => Alias(type).Equals(entry.Type, StringComparison.OrdinalIgnoreCase))
                ?? throw new InvalidDataException($"Missing placement type: {entry.Type}");
            var point = Transform(entry, anchorX, anchorZ, rotation);
            var item = new SdlPlacedObject(type, point.X, groundHeight(point.X, point.Z) + entry.HeightOffset, point.Z,
                team ?? entry.Team, Normalize(entry.Angle + rotation), entry.Count) { ScenarioId = Guid.NewGuid() };
            PlacementEditSession.ValidateBounds(item, strictUnitCount: true); result.Add(item);
        }
        return result;
    }
    public static IReadOnlyList<NatureAddition> PlanNature(MapLayoutPreset preset, IReadOnlyDictionary<string, LevelObjectTemplate> templates,
        float anchorX, float anchorZ, float rotation, Func<float, float, float> groundHeight)
    {
        Validate(preset);
        if (preset.Kind != MapLayoutKind.Nature) throw new InvalidDataException("Expected a nature layout.");
        var result = new List<NatureAddition>();
        foreach (var entry in preset.Entries)
        {
            if (!templates.TryGetValue(entry.Type, out var template)) throw new InvalidDataException($"Missing nature type: {entry.Type}");
            var point = Transform(entry, anchorX, anchorZ, rotation);
            float y = groundHeight(point.X, point.Z) + entry.HeightOffset;
            if (!float.IsFinite(y)) throw new InvalidDataException("Invalid ground height.");
            result.Add(new(template, entry.Type, point.X, y, point.Z, Normalize(entry.Angle + rotation) * MathF.PI / 180));
        }
        return result;
    }
    public static string Alias(SdlObjectType type) => type.TemplateFields.TryGetValue("alias", out string? alias) ? alias : type.NameDef;
    private static float Normalize(float angle) => (angle % 360 + 360) % 360;
    private static (float X, float Z) Transform(MapLayoutEntry entry, float anchorX, float anchorZ, float rotation)
    {
        if (!float.IsFinite(anchorX) || !float.IsFinite(anchorZ) || !float.IsFinite(rotation)) throw new ArgumentOutOfRangeException(nameof(anchorX));
        float radians = Normalize(rotation) * MathF.PI / 180, cos = MathF.Cos(radians), sin = MathF.Sin(radians);
        if (MathF.Abs(cos) < 1e-6f) cos = 0;
        if (MathF.Abs(sin) < 1e-6f) sin = 0;
        float x = anchorX + entry.X * cos - entry.Z * sin, z = anchorZ + entry.X * sin + entry.Z * cos;
        if (!float.IsFinite(x) || !float.IsFinite(z) || x is < 0 or > 16383 || z is < 0 or > 16383)
            throw new ArgumentOutOfRangeException(nameof(entry), "Layout extends outside the map.");
        return (x, z);
    }
}
