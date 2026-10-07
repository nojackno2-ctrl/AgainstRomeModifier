using System.Globalization;
using System.Numerics;

namespace AgainstRomeMapEditor.NativeAssets;

/// <summary>
/// 光源定義資料模型（對應 lightdef.dau 中的 [LightDefault] 表與記憶體 0x4D2FE9 預設值）。
/// </summary>
public sealed class NativeLightDefinition
{
    public int Index { get; init; }
    public bool IsActive { get; init; }
    public Vector3 Color { get; init; }
    public float Radius { get; init; }
    public int FlickerType { get; init; }
    public float FlickerParam { get; init; }
    public int SpecialFx { get; init; }
    public string Name { get; init; } = string.Empty;

    public float RadiusSquared => Radius * Radius;
    public float InvRadiusSquared => Radius > 0 ? 1.0f / (Radius * Radius) : 0f;

    /// <summary>
    /// 建立原生引擎預設光源定義（對應 0x4D2FE9：預設半徑 500.0，RGB 1.0，恆亮）。
    /// </summary>
    public static NativeLightDefinition CreateDefault(int index = 0) => new()
    {
        Index = index,
        IsActive = true,
        Color = Vector3.One,
        Radius = 500.0f,
        FlickerType = 0,
        FlickerParam = 0.0f,
        SpecialFx = 0,
        Name = "DefaultLight"
    };
}

/// <summary>
/// 局部光源目錄（載入 lightdef.dau，提供 fast lookup 與 fallback 預設值）。
/// </summary>
public sealed class NativeLightCatalog
{
    private readonly Dictionary<int, NativeLightDefinition> _definitions;

    public NativeLightCatalog(IEnumerable<NativeLightDefinition>? definitions = null)
    {
        _definitions = new Dictionary<int, NativeLightDefinition>();
        if (definitions != null)
        {
            foreach (var def in definitions)
            {
                _definitions[def.Index] = def;
            }
        }
    }

    public int Count => _definitions.Count;

    public bool TryGetDefinition(int index, out NativeLightDefinition definition)
    {
        if (_definitions.TryGetValue(index, out var found))
        {
            definition = found;
            return true;
        }

        definition = NativeLightDefinition.CreateDefault(index);
        return false;
    }

    public NativeLightDefinition GetDefinition(int index)
    {
        if (_definitions.TryGetValue(index, out var def))
        {
            return def;
        }
        return NativeLightDefinition.CreateDefault(index);
    }

    /// <summary>
    /// 從檔案路徑載入 lightdef.dau（自動偵測並解壓縮 PFIL 容器，使用 GameLZSS）。
    /// </summary>
    public static NativeLightCatalog? Open(string filePath)
    {
        if (!File.Exists(filePath)) return null;
        byte[] bytes = File.ReadAllBytes(filePath);
        return Parse(bytes);
    }

    /// <summary>
    /// 解析位元組陣列中的 lightdef.dau（支援 PFIL 壓縮或純文字）。
    /// </summary>
    public static NativeLightCatalog Parse(ReadOnlySpan<byte> bytes)
    {
        byte[] raw = bytes.ToArray();
        if (raw.Length >= 4 && raw.AsSpan(0, 4).SequenceEqual("PFIL"u8))
        {
            raw = AgainstRomeModifier.GameLZSS.DecompressPfil(raw);
        }
        string text = AgainstRomeModifier.Maps.MapTextEncoding.Game.GetString(raw);
        return Parse(text);
    }

    /// <summary>
    /// 解析 lightdef.dau 或 [LightDefault] 設定文字。
    /// 格式：;idx ,activ,  red,  grn,  blu,      rad, type,     typep,spefx,-------------name-------------
    /// </summary>
    public static NativeLightCatalog Parse(string content)
    {
        ArgumentNullException.ThrowIfNull(content);
        var dict = new Dictionary<int, NativeLightDefinition>();

        using var reader = new StringReader(content);
        string? line;
        bool inLightSection = false;

        while ((line = reader.ReadLine()) != null)
        {
            line = line.Trim();
            if (line.Length == 0) continue;

            if (line.StartsWith('[') && line.EndsWith(']'))
            {
                inLightSection = line.Equals("[LightDefault]", StringComparison.OrdinalIgnoreCase);
                continue;
            }

            if (line.StartsWith(';') || line.StartsWith('#')) continue;

            // 若沒有明確 section 標頭但有 CSV 資料列，亦嘗試剖析
            var parts = line.Split(',', StringSplitOptions.TrimEntries);
            if (parts.Length < 6) continue;

            if (int.TryParse(parts[0], NumberStyles.Integer, CultureInfo.InvariantCulture, out int idx))
            {
                int activ = int.TryParse(parts[1], NumberStyles.Integer, CultureInfo.InvariantCulture, out int a) ? a : 1;
                float r = float.TryParse(parts[2], NumberStyles.Float, CultureInfo.InvariantCulture, out float red) ? red : 1.0f;
                float g = float.TryParse(parts[3], NumberStyles.Float, CultureInfo.InvariantCulture, out float grn) ? grn : 1.0f;
                float b = float.TryParse(parts[4], NumberStyles.Float, CultureInfo.InvariantCulture, out float blu) ? blu : 1.0f;
                float rad = float.TryParse(parts[5], NumberStyles.Float, CultureInfo.InvariantCulture, out float radius) ? radius : 500.0f;
                int type = (parts.Length > 6 && int.TryParse(parts[6], NumberStyles.Integer, CultureInfo.InvariantCulture, out int t)) ? t : 0;
                float typep = (parts.Length > 7 && float.TryParse(parts[7], NumberStyles.Float, CultureInfo.InvariantCulture, out float tp)) ? tp : 0.0f;
                int spefx = (parts.Length > 8 && int.TryParse(parts[8], NumberStyles.Integer, CultureInfo.InvariantCulture, out int fx)) ? fx : 0;
                string name = parts.Length > 9 ? parts[9] : $"Light_{idx}";

                dict[idx] = new NativeLightDefinition
                {
                    Index = idx,
                    IsActive = activ != 0,
                    Color = new Vector3(r, g, b),
                    Radius = rad,
                    FlickerType = type,
                    FlickerParam = typep,
                    SpecialFx = spefx,
                    Name = name
                };
            }
        }

        return new NativeLightCatalog(dict.Values);
    }
}

/// <summary>
/// 執行期單一局部光源實例（對應 0x7F9EF0 陣列中步長 0x44 的物件）。
/// </summary>
public sealed class NativeLightInstance
{
    public Vector3 WorldPosition { get; set; }
    public NativeLightDefinition Definition { get; set; }
    public bool IsActive { get; set; }

    public float Radius => Definition.Radius;
    public Vector3 Color => Definition.Color;
    public float RadiusSquared => Definition.RadiusSquared;
    public float InvRadiusSquared => Definition.InvRadiusSquared;

    public NativeLightInstance(Vector3 worldPosition, NativeLightDefinition definition, bool isActive = true)
    {
        WorldPosition = worldPosition;
        Definition = definition ?? throw new ArgumentNullException(nameof(definition));
        IsActive = isActive;
    }
}

/// <summary>
/// 原生光照計算模型（對應 0x49FFE0 二次衰減與每通道取最大值合成，及 APT 坐標映射）。
/// </summary>
public static class NativeLightCalculator
{
    /// <summary>
    /// 原生單一光源二次衰減公式（0x49FFE0）。
    /// 當 distanceSquared &lt; radiusSquared 時：candidate = lightColor * (1 - distanceSquared / radiusSquared)。
    /// 與 ambient 逐色道取最大值，最後 clamp 至 1.0。
    /// </summary>
    public static Vector3 ApplyLocalLight(Vector3 ambient, Vector3 lightColor, float distanceSquared, float radiusSquared)
    {
        return MapLightingModel.ApplyLocalLight(ambient, lightColor, distanceSquared, radiusSquared);
    }

    /// <summary>
    /// 依據世界坐標走訪動態光源清單，計算合成光照（0x49FFE0 / 0x499BD3 / 0x49A526）。
    /// </summary>
    public static Vector3 Evaluate(Vector3 ambient, Vector3 targetWorldPosition, ReadOnlySpan<NativeLightInstance> lights)
    {
        Vector3 lit = ambient;

        foreach (var light in lights)
        {
            if (!light.IsActive) continue;
            float r2 = light.RadiusSquared;
            if (r2 <= 0f) continue;

            float dx = light.WorldPosition.X - targetWorldPosition.X;
            float dy = light.WorldPosition.Y - targetWorldPosition.Y;
            float dz = light.WorldPosition.Z - targetWorldPosition.Z;
            float d2 = dx * dx + dy * dy + dz * dz;

            if (d2 < r2)
            {
                float falloff = 1.0f - (d2 * light.InvRadiusSquared);
                Vector3 candidate = light.Color * falloff;
                lit = Vector3.Max(lit, candidate);
            }
        }

        return Vector3.Min(lit, Vector3.One);
    }

    /// <summary>
    /// 將 APT 動畫相對於建築錨點的 2D 螢幕坐標偏移轉換為世界空間坐標位移。
    /// 遊戲採 2:1 等角投影：+1 WorldX = (+0.5, +0.25) px, +1 WorldZ = (-0.5, +0.25) px。
    /// 逆運算為：DeltaWorldX = DeltaScreenX + 2 * DeltaScreenY, DeltaWorldZ = 2 * DeltaScreenY - DeltaScreenX。
    /// </summary>
    public static Vector3 IsometricAptOffsetToWorldDelta(int deltaScreenX, int deltaScreenY, float heightOffset = 0f)
    {
        float deltaWorldX = deltaScreenX + 2.0f * deltaScreenY;
        float deltaWorldZ = 2.0f * deltaScreenY - deltaScreenX;
        return new Vector3(deltaWorldX, heightOffset, deltaWorldZ);
    }
}
