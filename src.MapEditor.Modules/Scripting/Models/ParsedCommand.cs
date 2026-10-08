using System.Globalization;

namespace AgainstRomeMapEditor.Modules.Scripting.Models;

/// <summary>
/// 表示矩形區域座標 (Tile 或 World 空間)。
/// </summary>
public readonly record struct MapRegionRect(int MinX, int MinZ, int MaxX, int MaxZ)
{
    public int Width => Math.Max(0, MaxX - MinX + 1);
    public int Height => Math.Max(0, MaxZ - MinZ + 1);
    public int Area => Width * Height;

    public bool Contains(int x, int z) => x >= MinX && x <= MaxX && z >= MinZ && z <= MaxZ;
}

/// <summary>
/// 表示圓形區域座標。
/// </summary>
public readonly record struct MapRegionCircle(float CenterX, float CenterZ, float Radius)
{
    public bool Contains(float x, float z)
    {
        float dx = x - CenterX;
        float dz = z - CenterZ;
        return (dx * dx + dz * dz) <= (Radius * Radius);
    }
}

/// <summary>
/// 解析後的結構化指令物件。
/// </summary>
public sealed class ParsedCommand
{
    public string RawText { get; }
    public string CommandName { get; }
    public IReadOnlyList<string> PositionalArgs { get; }
    public IReadOnlyDictionary<string, string?> NamedFlags { get; }

    public ParsedCommand(
        string rawText,
        string commandName,
        IReadOnlyList<string> positionalArgs,
        IReadOnlyDictionary<string, string?> namedFlags)
    {
        RawText = rawText;
        CommandName = commandName;
        PositionalArgs = positionalArgs;
        NamedFlags = namedFlags;
    }

    public bool HasFlag(string flagName) =>
        NamedFlags.ContainsKey(flagName);

    public string? GetFlag(string flagName, string? defaultValue = null) =>
        NamedFlags.TryGetValue(flagName, out string? value) ? (value ?? defaultValue) : defaultValue;

    public bool TryGetFlagInt(string flagName, out int value)
    {
        if (NamedFlags.TryGetValue(flagName, out string? raw) &&
            int.TryParse(raw, NumberStyles.Integer, CultureInfo.InvariantCulture, out value))
        {
            return true;
        }
        value = 0;
        return false;
    }

    public bool TryGetFlagFloat(string flagName, out float value)
    {
        if (NamedFlags.TryGetValue(flagName, out string? raw) &&
            float.TryParse(raw, NumberStyles.Float, CultureInfo.InvariantCulture, out value))
        {
            return true;
        }
        value = 0f;
        return false;
    }

    public string? GetPositional(int index) =>
        index >= 0 && index < PositionalArgs.Count ? PositionalArgs[index] : null;

    public bool TryGetPositionalInt(int index, out int value)
    {
        string? raw = GetPositional(index);
        if (raw is not null && int.TryParse(raw, NumberStyles.Integer, CultureInfo.InvariantCulture, out value))
        {
            return true;
        }
        value = 0;
        return false;
    }

    public bool TryGetPositionalFloat(int index, out float value)
    {
        string? raw = GetPositional(index);
        if (raw is not null && float.TryParse(raw, NumberStyles.Float, CultureInfo.InvariantCulture, out value))
        {
            return true;
        }
        value = 0f;
        return false;
    }

    /// <summary>
    /// 解析矩形參數：支援 "x1,z1,x2,z2" 或連續 4 個數值引數。
    /// </summary>
    public static bool TryParseRect(string raw, out MapRegionRect rect)
    {
        rect = default;
        if (string.IsNullOrWhiteSpace(raw)) return false;

        string[] parts = raw.Split(new[] { ',', ' ', ':', ';' }, StringSplitOptions.RemoveEmptyEntries);
        if (parts.Length != 4) return false;

        if (int.TryParse(parts[0], NumberStyles.Integer, CultureInfo.InvariantCulture, out int x1) &&
            int.TryParse(parts[1], NumberStyles.Integer, CultureInfo.InvariantCulture, out int z1) &&
            int.TryParse(parts[2], NumberStyles.Integer, CultureInfo.InvariantCulture, out int x2) &&
            int.TryParse(parts[3], NumberStyles.Integer, CultureInfo.InvariantCulture, out int z2))
        {
            rect = new MapRegionRect(Math.Min(x1, x2), Math.Min(z1, z2), Math.Max(x1, x2), Math.Max(z1, z2));
            return true;
        }

        return false;
    }

    /// <summary>
    /// 解析圓形參數：支援 "cx,cz,radius" 或連續 3 個數值引數。
    /// </summary>
    public static bool TryParseCircle(string raw, out MapRegionCircle circle)
    {
        circle = default;
        if (string.IsNullOrWhiteSpace(raw)) return false;

        string[] parts = raw.Split(new[] { ',', ' ', ':', ';' }, StringSplitOptions.RemoveEmptyEntries);
        if (parts.Length != 3) return false;

        if (float.TryParse(parts[0], NumberStyles.Float, CultureInfo.InvariantCulture, out float cx) &&
            float.TryParse(parts[1], NumberStyles.Float, CultureInfo.InvariantCulture, out float cz) &&
            float.TryParse(parts[2], NumberStyles.Float, CultureInfo.InvariantCulture, out float r) &&
            r > 0)
        {
            circle = new MapRegionCircle(cx, cz, r);
            return true;
        }

        return false;
    }
}
