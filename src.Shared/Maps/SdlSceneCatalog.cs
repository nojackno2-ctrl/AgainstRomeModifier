using System.Globalization;
using System.Text;
using System.Text.RegularExpressions;

namespace AgainstRomeModifier.Maps;

public sealed record MapSceneObject(
    string Name,
    float WorldX,
    float WorldY,
    float WorldZ,
    int Team,
    string SourceFile,
    int ObjectIndex = -1,
    float LocalX = 0,
    float LocalY = 0,
    float LocalZ = 0)
{
    public string Kind => Name.StartsWith("Bau", StringComparison.OrdinalIgnoreCase) ? "建築"
        : Name.StartsWith("Fig", StringComparison.OrdinalIgnoreCase) ? "單位" : "物件";
}

public static class SdlSceneCatalog
{
    public const float WorldUnitsPerMapPixel = 64f;
    public const float MapPixelSize = 256f;
    private static readonly Regex Section = new(@"(?ms)^\[(?<name>[^\]]+)\]\s*\r?\n(?<body>.*?)(?=^\[|\z)", RegexOptions.Compiled);

    public static IReadOnlyList<MapSceneObject> LoadDirectory(string mapDirectory)
    {
        var objects = new List<MapSceneObject>();
        foreach (string path in Directory.GetFiles(mapDirectory, "*.sdl", SearchOption.TopDirectoryOnly))
        {
            try { objects.AddRange(Load(path)); }
            catch (InvalidDataException) { }
        }
        return objects;
    }

    public static IReadOnlyList<MapSceneObject> Load(string path)
    {
        byte[] bytes = File.ReadAllBytes(path);
        if (bytes.Length >= 4 && bytes.AsSpan(0, 4).SequenceEqual("PFIL"u8)) bytes = GameLZSS.DecompressPfil(bytes);
        Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
        string text = Encoding.GetEncoding(1251).GetString(bytes);
        Match settlement = Section.Matches(text).FirstOrDefault(match => match.Groups["name"].Value.Equals("settlement", StringComparison.OrdinalIgnoreCase))
            ?? throw new InvalidDataException("SDL 缺少 settlement 區段。");
        float[] reference = Vector(Value(settlement.Groups["body"].Value, "refpos"));
        var result = new List<MapSceneObject>();
        foreach (Match section in Section.Matches(text))
        {
            Match objectName = Regex.Match(section.Groups["name"].Value, @"^object(?<index>\d+)$", RegexOptions.IgnoreCase);
            if (!objectName.Success || !int.TryParse(objectName.Groups["index"].Value, NumberStyles.None, CultureInfo.InvariantCulture, out int objectIndex)) continue;
            string body = section.Groups["body"].Value;
            float[] position = Vector(Value(body, "pos"));
            string name = Value(body, "namedef")?.Trim() ?? "未命名物件";
            int.TryParse(Value(body, "team"), NumberStyles.Integer, CultureInfo.InvariantCulture, out int team);
            float x = reference[0] + position[0], y = reference[1] + position[1], z = reference[2] + position[2];
            if (x is >= 0 and <= WorldUnitsPerMapPixel * MapPixelSize && z is >= 0 and <= WorldUnitsPerMapPixel * MapPixelSize)
                result.Add(new MapSceneObject(name, x, y, z, team, Path.GetFileName(path), objectIndex, position[0], position[1], position[2]));
        }
        return result;
    }

    private static string? Value(string body, string key)
    {
        Match match = Regex.Match(body, $@"(?im)^[ \t]*{Regex.Escape(key)}[ \t]*=[ \t]*(?<value>[^\r\n]*)");
        return match.Success ? match.Groups["value"].Value : null;
    }

    private static float[] Vector(string? value)
    {
        string[] parts = (value ?? "0,0,0").Split(',');
        var result = new float[3];
        for (int i = 0; i < Math.Min(3, parts.Length); i++) float.TryParse(parts[i], NumberStyles.Float, CultureInfo.InvariantCulture, out result[i]);
        return result;
    }
}
