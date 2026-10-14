using System.Globalization;

namespace AgainstRomeModifier.Maps;

/// <summary>Verified objdef fields used by native create; not a reconstruction of all type initialization.</summary>
public sealed record NativeObjectDefinition(int TypeId, bool Active, short Afstr, int? InitialHp = null, int? InitialMorale = null, float? InitialMoves = null, int? InitialMaxRuhm = null, short? Typus = null, ushort? InitialField1c = null)
{
    public string? Name { get; init; }
    public short DirectLightDefinition { get; init; } = -1;
    public short DirectLightHeight { get; init; }
    public float DirectLightDistance { get; init; }
    public float DirectLightAngle { get; init; }

    /// <summary>Owner-light position after native 49f810 update; not create's initial XYZ or bit-exact x87 emulation.</summary>
    public System.Numerics.Vector3 DirectLightPosition(System.Numerics.Vector3 position, float rotationDegrees)
    {
        if (!float.IsFinite(position.X) || !float.IsFinite(position.Y) || !float.IsFinite(position.Z)
            || !float.IsFinite(rotationDegrees) || !float.IsFinite(DirectLightDistance) || !float.IsFinite(DirectLightAngle))
            throw new ArgumentOutOfRangeException(nameof(position), "光源座標與方向必須是有限數值。");
        // Preserve the shipped float constants and extended intermediate intent, then store float XYZ.
        double angle = ((double)rotationDegrees - 90d - DirectLightAngle) * (double)(1f / 360f) * 2d * (double)(float)Math.PI;
        var result = new System.Numerics.Vector3((float)(position.X + Math.Sin(angle) * DirectLightDistance),
            (float)((double)position.Y + DirectLightHeight), (float)(position.Z + Math.Cos(angle) * DirectLightDistance));
        if (!float.IsFinite(result.X) || !float.IsFinite(result.Y) || !float.IsFinite(result.Z))
            throw new ArgumentOutOfRangeException(nameof(position), "光源計算結果超出 float 座標範圍。");
        return result;
    }
    public bool RequiresThirdPosition => Active && Afstr != 0;
}

public static class NativeObjectDefinitions
{
    public static IReadOnlyDictionary<int, NativeObjectDefinition> Load(string gamePath)
    {
        string prefix = DefaultPrefix(gamePath);
        var result = new Dictionary<int, NativeObjectDefinition>();
        foreach (string file in new[] { "objdef.dau" }.Concat(Enumerable.Range(0, 10).Select(i => $"objdef.d{i:00}")))
        {
            string path = prefix + file;
            if (!File.Exists(path)) continue;
            ApplyLayer(ReadText(path), result);
        }
        var typus = new Dictionary<int, (bool Active, uint Flags)>();
        foreach (string file in new[] { "typus.dau" }.Concat(Enumerable.Range(0, 10).Select(i => $"typus.d{i:00}")))
        {
            string path = prefix + file;
            if (File.Exists(path)) ApplyTypusLayer(ReadText(path), typus);
        }
        foreach (int id in result.Keys.ToArray())
        {
            var definition = result[id];
            // 4a9a77 tests byte table+5 mask04: low 32-bit flags bit10, not bit2.
            bool enabled = definition.Active && definition.Typus is >= 0 and < 2500
                && typus.TryGetValue(definition.Typus.Value, out var entry) && entry.Active && (entry.Flags & 0x400) != 0;
            result[id] = definition with { InitialField1c = (ushort)(enabled ? 100 : 0) };
        }
        return result;
    }

    private static string ReadText(string path)
        => MapTextEncoding.Game.GetString(GameLZSS.DecompressPfil(File.ReadAllBytes(path)));

    internal static string DefaultPrefix(string gamePath)
    {
        string path = Path.Combine(gamePath, "SYSTEM", "clengine.ini");
        if (!File.Exists(path)) return Path.Combine(gamePath, "SYSTEM", "DATA_MP", "DEFAULTS") + Path.DirectorySeparatorChar;
        bool inSection = false;
        foreach (string line in ReadText(path).Split('\n'))
        {
            string row = line.Split(';')[0].Trim();
            if (row.StartsWith('[')) { inSection = row.Equals("[DefaultPath]", StringComparison.OrdinalIgnoreCase); continue; }
            if (!inSection || row.Length == 0) continue;
            string relative = row.Replace('\\', Path.DirectorySeparatorChar).Replace('/', Path.DirectorySeparatorChar);
            if (Path.IsPathRooted(relative)) throw new InvalidDataException("clengine.ini DefaultPath 必須是遊戲目錄內的相對路徑。");
            string root = Path.GetFullPath(gamePath);
            string directory = Path.GetFullPath(Path.Combine(root, relative));
            if (!directory.StartsWith(root.TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase))
                throw new InvalidDataException("clengine.ini DefaultPath 超出遊戲目錄，無法驗證物件定義來源。");
            return directory;
        }
        throw new InvalidDataException("clengine.ini 缺少有效 DefaultPath，無法驗證物件定義來源。");
    }

    // The native loader initializes the table once and applies callbacks only for present fields.
    private static void ApplyLayer(string text, Dictionary<int, NativeObjectDefinition> definitions)
    {
        bool inSection = false;
        foreach (string line in text.Split('\n'))
        {
            string row = line.Split(';')[0].Trim();
            if (row.StartsWith('['))
            {
                if (inSection) break;
                inSection = row.Equals("[ObjectDefaults]", StringComparison.OrdinalIgnoreCase);
                continue;
            }
            if (!inSection || row.Length == 0) continue;
            // 0x567d50 skips comma runs; whitespace-only tokens still reach the callback.
            string[] columns = row.Split(',', StringSplitOptions.RemoveEmptyEntries);
            if (columns.Length == 0) continue;
            if (!int.TryParse(columns[0].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int id) || id is < 0 or >= 2500)
                throw new InvalidDataException("objdef.dau 物件編號無效。");
            var previous = definitions.GetValueOrDefault(id) ?? new(id, false, 0, 0, 0, 0f, 0, -1);
            bool active = previous.Active; short afstr = previous.Afstr;
            if (columns.Length > 1)
            {
                if (!byte.TryParse(columns[1].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out byte flag))
                    throw new InvalidDataException($"objdef.dau 物件 {id} 的 activ 欄位無效。");
                active = flag != 0;
            }
            if (columns.Length > 60 && !short.TryParse(columns[60].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out afstr))
                throw new InvalidDataException($"objdef.dau 物件 {id} 的 afstr 欄位無效。");
            int? hp = previous.InitialHp, morale = previous.InitialMorale;
            if (columns.Length > 19) hp = ReadVital(columns[19], id, "lpmax");
            if (columns.Length > 21) morale = ReadVital(columns[21], id, "mpmax");
            float? moves = previous.InitialMoves;
            int? maxRuhm = previous.InitialMaxRuhm;
            if (columns.Length > 4)
            {
                if (!float.TryParse(columns[4].Trim(), NumberStyles.Float, CultureInfo.InvariantCulture, out float value) || !float.IsFinite(value))
                    throw new InvalidDataException($"objdef 物件 {id} 的 moves float 欄位無效。");
                moves = value;
            }
            if (columns.Length > 152) maxRuhm = ReadVital(columns[152], id, "maxruhm");
            short? typus = previous.Typus;
            if (columns.Length > 54)
            {
                if (!short.TryParse(columns[54].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out short value))
                    throw new InvalidDataException($"objdef 物件 {id} 的 typus signed word 欄位無效。");
                typus = value;
            }
            short directLight = previous.DirectLightDefinition;
            if (columns.Length > 66 && !short.TryParse(columns[66].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out directLight))
                throw new InvalidDataException($"objdef 物件 {id} 的 lidef signed16 欄位無效。");
            short lightHeight = columns.Length > 67 ? unchecked((short)ReadVital(columns[67], id, "lihei")) : previous.DirectLightHeight;
            float lightDistance = columns.Length > 70 ? ReadFiniteFloat(columns[70], id, "lightrad") : previous.DirectLightDistance;
            float lightAngle = columns.Length > 71 ? ReadFiniteFloat(columns[71], id, "lightang") : previous.DirectLightAngle;
            definitions[id] = new(id, active, afstr, hp, morale, moves, maxRuhm, typus)
            { Name = columns.Length > 52 ? columns[52].Trim() : previous.Name, DirectLightDefinition = directLight,
                DirectLightHeight = lightHeight, DirectLightDistance = lightDistance, DirectLightAngle = lightAngle };
        }
    }

    private static void ApplyTypusLayer(string text, Dictionary<int, (bool Active, uint Flags)> definitions)
    {
        bool inSection = false;
        foreach (string line in text.Split('\n'))
        {
            string row = line.Split(';')[0].Trim();
            if (row.StartsWith('['))
            {
                if (inSection) break;
                inSection = row.Equals("[TypusDefaults]", StringComparison.OrdinalIgnoreCase);
                continue;
            }
            if (!inSection || row.Length == 0) continue;
            string[] columns = row.Split(',', StringSplitOptions.RemoveEmptyEntries);
            if (columns.Length == 0) continue;
            if (!int.TryParse(columns[0].Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int id) || id is < 0 or >= 2500)
                throw new InvalidDataException("typus 物件編號無效。");
            var previous = definitions.GetValueOrDefault(id);
            if (columns.Length > 1)
                previous.Active = ReadVital(columns[1], id, "typus activ") != 0;
            if (columns.Length > 3)
            {
                string value = columns[3].Trim();
                if (value.StartsWith("0x", StringComparison.OrdinalIgnoreCase)) value = value[2..];
                if (!uint.TryParse(value, NumberStyles.AllowHexSpecifier, CultureInfo.InvariantCulture, out uint flags))
                    throw new InvalidDataException($"typus {id} 的低 flags hex 欄位無效。");
                previous.Flags = flags;
            }
            definitions[id] = previous;
        }
    }

    private static float ReadFiniteFloat(string value, int id, string field)
    {
        if (!float.TryParse(value.Trim(), NumberStyles.Float, CultureInfo.InvariantCulture, out float result) || !float.IsFinite(result))
            throw new InvalidDataException($"objdef 物件 {id} 的 {field} float 欄位無效。");
        return result;
    }

    private static int ReadVital(string value, int id, string field)
    {
        if (!int.TryParse(value.Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int result))
            throw new InvalidDataException($"objdef 物件 {id} 的 {field} signed integer 欄位無效。");
        return result;
    }

    internal static IReadOnlyDictionary<int, NativeObjectDefinition> Parse(string text)
    {
        var result = new Dictionary<int, NativeObjectDefinition>();
        ApplyLayer(text, result);
        return result;
    }
}
