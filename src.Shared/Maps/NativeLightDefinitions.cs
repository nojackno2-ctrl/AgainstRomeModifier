using System.Globalization;
using System.Numerics;

namespace AgainstRomeModifier.Maps;

public sealed record NativeLightTypeDefinition(int Index, int Active, Vector3 Color, float Radius,
    short FlickerType, float FlickerParameter, uint SpecialFx, string Name)
{
    public static NativeLightTypeDefinition Initial(int index)
        => new(index, 0, Vector3.One, 500, 0, 0, 0, "LD_NoNameEntered");

    public NativeLightCreation Create(Vector3 position, int ownerObject = -1, int parentObject = -1)
    {
        if (Index is < 0 or >= 256 || Active == 0)
            throw new InvalidDataException("光源 definition 未啟用或超出原生索引範圍。");
        return new(position, Color, Radius, FlickerType, FlickerParameter, SpecialFx,
            checked((short)Index), ownerObject, parentObject);
    }
}

/// <summary>Verified loose-file DefaultPath lightdef.dau/d00..d09 overlay loader.</summary>
public static class NativeLightDefinitions
{
    public static IReadOnlyDictionary<int, NativeLightTypeDefinition> Load(string gamePath)
    {
        string prefix = NativeObjectDefinitions.DefaultPrefix(gamePath);
        var result = new Dictionary<int, NativeLightTypeDefinition>();
        foreach (string file in new[] { "lightdef.dau" }.Concat(Enumerable.Range(0, 10).Select(i => $"lightdef.d{i:00}")))
        {
            string path = prefix + file;
            if (File.Exists(path)) ApplyLayer(MapTextEncoding.Game.GetString(GameLZSS.DecompressPfil(File.ReadAllBytes(path))), result);
        }
        return result;
    }

    public static IReadOnlyDictionary<int, NativeLightTypeDefinition> Parse(string text)
    {
        var result = new Dictionary<int, NativeLightTypeDefinition>();
        ApplyLayer(text, result);
        return result;
    }

    private static void ApplyLayer(string text, Dictionary<int, NativeLightTypeDefinition> result)
    {
        ArgumentNullException.ThrowIfNull(text);
        bool inSection = false;
        foreach (string line in text.Split('\n'))
        {
            string row = line.Split(';')[0].Trim();
            if (row.StartsWith('['))
            {
                if (inSection) break;
                inSection = row.Equals("[LightDefault]", StringComparison.OrdinalIgnoreCase);
                continue;
            }
            if (!inSection || row.Length == 0 || row.StartsWith('#')) continue;
            // Shared native tokenizer 567d50 skips comma runs, not whitespace tokens.
            string[] columns = row.Split(',', StringSplitOptions.RemoveEmptyEntries);
            int id = Integer(columns[0]);
            if (id is < 0 or >= 256) throw new InvalidDataException("lightdef 索引超出0..255。");
            var value = result.GetValueOrDefault(id) ?? NativeLightTypeDefinition.Initial(id);
            if (columns.Length > 1) value = value with { Active = Integer(columns[1]) };
            Vector3 color = value.Color;
            if (columns.Length > 2) color.X = Float(columns[2]);
            if (columns.Length > 3) color.Y = Float(columns[3]);
            if (columns.Length > 4) color.Z = Float(columns[4]);
            value = value with { Color = color };
            if (columns.Length > 5) value = value with { Radius = Float(columns[5]) };
            if (columns.Length > 6) value = value with { FlickerType = unchecked((short)Integer(columns[6])) };
            if (columns.Length > 7) value = value with { FlickerParameter = Float(columns[7]) };
            if (columns.Length > 8) value = value with { SpecialFx = unchecked((uint)Integer(columns[8])) };
            if (columns.Length > 9)
            {
                string name = columns[9].Trim().Split([' ', '\t', '\r'], StringSplitOptions.RemoveEmptyEntries).FirstOrDefault() ?? "";
                if (name.Length == 0) throw new InvalidDataException("lightdef name 必須含非空白token。");
                byte[] bytes = MapTextEncoding.Game.GetBytes(name);
                value = value with { Name = MapTextEncoding.Game.GetString(bytes.AsSpan(0, Math.Min(30, bytes.Length))) };
            }
            result[id] = value;
        }
    }

    private static int Integer(string text)
        => int.TryParse(text.Trim(), NumberStyles.Integer, CultureInfo.InvariantCulture, out int value)
            ? value : throw new InvalidDataException("lightdef signed32 欄位無效。");

    private static float Float(string text)
        => float.TryParse(text.Trim(), NumberStyles.Float, CultureInfo.InvariantCulture, out float value) && float.IsFinite(value)
            ? value : throw new InvalidDataException("lightdef float32 欄位無效或非有限值。");
}
