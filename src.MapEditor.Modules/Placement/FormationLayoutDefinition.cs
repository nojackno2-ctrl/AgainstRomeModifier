using System.Globalization;
using System.Numerics;

namespace AgainstRomeMapEditor.Modules.Placement;

/// <summary>formdef.dau 的一組 (startX,startZ,endX,endZ)，座標尚未乘上 spacing。</summary>
public readonly record struct FormationLayoutSegment(Vector2 Start, Vector2 End);

/// <summary>不可變的原生線段定義；不含檔案存取。</summary>
public sealed class FormationLayoutDefinition
{
    /// <summary>
    /// 原版 id 0 All_Haufen 的九條有效線段（2026-10-07 離線 formdef.dau）。
    /// 缺少外部表格時使用此幾何摘要；有表格時宿主優先解析它。不是截圖反推的座標。
    /// </summary>
    public static FormationLayoutDefinition NativeDefault { get; } = new([
        new(new(.05f, -.05f), new(.20f, .05f)),
        new(new(-.20f, .15f), new(-.05f, .05f)),
        new(new(.20f, -.20f), new(.05f, -.15f)),
        new(new(-.10f, -.05f), new(-.20f, -.20f)),
        new(new(.30f, -.15f), new(.40f, .10f)),
        new(new(-.25f, .25f), new(-.40f, .05f)),
        new(new(.05f, .30f), new(.30f, .20f)),
        new(new(.34f, -.25f), new(.10f, -.34f)),
        new(new(-.20f, -.34f), new(-.40f, -.15f)),
    ]);

    public FormationLayoutDefinition(IEnumerable<FormationLayoutSegment> segments)
    {
        ArgumentNullException.ThrowIfNull(segments);
        var owned = segments.ToArray();
        if (owned.Length is < 1 or > 20) throw new ArgumentException("隊形線段數必須介於 1 與 20。", nameof(segments));
        if (owned.Any(s => !Finite(s.Start) || !Finite(s.End)))
            throw new ArgumentException("隊形座標必須為有限數值。", nameof(segments));
        Segments = Array.AsReadOnly(owned);
    }

    public IReadOnlyList<FormationLayoutSegment> Segments { get; }
    private static bool Finite(Vector2 p) => float.IsFinite(p.X) && float.IsFinite(p.Y);

    /// <summary>
    /// 解析已解壓的 [FormationDefault] CSV；預設選 id=0（編輯器傳入值）。
    /// 只解析幾何欄位 0..82，不賦予其他 runtime 欄位語意。
    /// </summary>
    public static FormationLayoutDefinition FromFormDefText(string text, int id = 0)
    {
        ArgumentNullException.ThrowIfNull(text);
        if (id is < 0 or >= 100) throw new ArgumentOutOfRangeException(nameof(id));
        bool inSection = false;
        FormationLayoutDefinition? result = null;
        foreach (string raw in text.Split('\n'))
        {
            string line = raw.Split(';')[0].Trim();
            if (line.StartsWith('[')) { inSection = line.Equals("[FormationDefault]", StringComparison.OrdinalIgnoreCase); continue; }
            if (!inSection || line.Length == 0) continue;
            string[] fields = line.Split(',');
            if (!int.TryParse(fields[0], NumberStyles.Integer, CultureInfo.InvariantCulture, out int rowId) || rowId != id) continue;
            if (result is not null) throw new InvalidDataException("隊形 ID 重複。");
            if (fields.Length < 83 || !int.TryParse(fields[1], NumberStyles.Integer, CultureInfo.InvariantCulture, out int active)
                || active is < 1 or > 255 || !int.TryParse(fields[2], NumberStyles.Integer, CultureInfo.InvariantCulture, out int count)
                || count is < 1 or > 20) throw new InvalidDataException("不完整或無效的隊形幾何欄位。");
            var segments = new FormationLayoutSegment[count];
            float Value(int index) => float.TryParse(fields[index], NumberStyles.Float, CultureInfo.InvariantCulture, out float value)
                && float.IsFinite(value) ? value : throw new InvalidDataException("無效的隊形座標。");
            for (int i = 0; i < count; i++)
            {
                int offset = 3 + i * 4;
                segments[i] = new(new(Value(offset), Value(offset + 1)), new(Value(offset + 2), Value(offset + 3)));
            }
            result = new(segments);
        }
        return result ?? throw new InvalidDataException($"找不到隊形 ID {id}。");
    }
}
