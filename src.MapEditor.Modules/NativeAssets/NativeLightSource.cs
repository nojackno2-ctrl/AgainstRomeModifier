using System.Numerics;

namespace AgainstRomeMapEditor.NativeAssets;

/// <summary>
/// 光源預覽資料模型；原生定義初始化與合併由共用 NativeLightDefinitions 處理。
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
    /// 建立預覽用 fallback 光源；原生初始 definition 為 inactive，不使用此 fallback 建立資料池。
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
        => FromDefinitions(AgainstRomeModifier.Maps.NativeLightDefinitions.Parse(content));

    public static NativeLightCatalog OpenGame(string gamePath)
        => FromDefinitions(AgainstRomeModifier.Maps.NativeLightDefinitions.Load(gamePath));

    private static NativeLightCatalog FromDefinitions(IReadOnlyDictionary<int, AgainstRomeModifier.Maps.NativeLightTypeDefinition> definitions)
        => new(definitions.Values.Select(value => new NativeLightDefinition
        {
            Index = value.Index, IsActive = value.Active != 0, Color = value.Color, Radius = value.Radius,
            FlickerType = value.FlickerType, FlickerParam = value.FlickerParameter,
            SpecialFx = unchecked((int)value.SpecialFx), Name = value.Name
        }));

}

/// <summary>
/// 執行期單一局部光源實例（對應 0x7F9EF0 陣列中步長 0x44 的物件）。
/// 兩種來源：（1）舊預覽：由 <see cref="NativeLightDefinition"/> 合成，位置為 editor 預覽座標；
/// （2）已保存快照（<see cref="CreateSaved"/>）：instance 自帶原生 XYZ／RGB／effective radius，不依 definition，
/// 建立後不可變，RGB 不夾值，也不從 definition 重建。
/// </summary>
public sealed class NativeLightInstance
{
    private Vector3 _worldPosition;
    private NativeLightDefinition _definition;
    private bool _isActive;
    private readonly Vector3 _savedColor;
    private readonly float _savedEffectiveRadius;

    /// <summary>已保存快照沒有可用 definition 時的占位；不代表任何原生 definition。</summary>
    public static NativeLightDefinition SavedInstanceOnlyDefinition { get; } = new()
    {
        Index = -1, IsActive = false, Color = Vector3.Zero, Radius = 0f, Name = "(saved instance, no definition)"
    };

    /// <summary>true＝原生保存快照（XYZ 為原生世界座標）；false＝舊 definition 合成的預覽光源。</summary>
    public bool IsSaved { get; }
    /// <summary>保存快照的 slot；舊預覽光源為 -1。僅供識別，不作去重鍵。</summary>
    public int SavedSlot { get; } = -1;
    /// <summary>保存的 baseline radius（column8）；僅資訊，渲染使用 effective radius。舊預覽光源回傳 definition 半徑。</summary>
    public float BaselineRadius { get; }
    public bool HasDefinition => !ReferenceEquals(_definition, SavedInstanceOnlyDefinition);

    public Vector3 WorldPosition
    {
        get => _worldPosition;
        set { ThrowIfSaved(); _worldPosition = value; }
    }

    public NativeLightDefinition Definition
    {
        get => _definition;
        set { ThrowIfSaved(); _definition = value ?? throw new ArgumentNullException(nameof(value)); }
    }

    public bool IsActive
    {
        get => _isActive;
        set { ThrowIfSaved(); _isActive = value; }
    }

    public float Radius => IsSaved ? _savedEffectiveRadius : Definition.Radius;
    public Vector3 Color => IsSaved ? _savedColor : Definition.Color;
    public float RadiusSquared => Radius * Radius;
    public float InvRadiusSquared => Radius > 0 ? 1.0f / (Radius * Radius) : 0f;

    /// <summary>保存光源可繪製：active、XYZ／RGB／radius 皆 finite 且 effective radius &gt; 0。</summary>
    public bool IsRenderable => IsActive && float.IsFinite(_worldPosition.X) && float.IsFinite(_worldPosition.Y) && float.IsFinite(_worldPosition.Z)
        && float.IsFinite(Color.X) && float.IsFinite(Color.Y) && float.IsFinite(Color.Z) && float.IsFinite(Radius) && Radius > 0;

    public NativeLightInstance(Vector3 worldPosition, NativeLightDefinition definition, bool isActive = true)
    {
        _worldPosition = worldPosition;
        _definition = definition ?? throw new ArgumentNullException(nameof(definition));
        _isActive = isActive;
        BaselineRadius = definition.Radius;
    }

    private NativeLightInstance(int slot, Vector3 nativePosition, Vector3 rgb, float effectiveRadius, float baselineRadius, bool isActive, NativeLightDefinition? definition)
    {
        IsSaved = true; SavedSlot = slot;
        _worldPosition = nativePosition; _savedColor = rgb; _savedEffectiveRadius = effectiveRadius; BaselineRadius = baselineRadius;
        _definition = definition ?? SavedInstanceOnlyDefinition; _isActive = isActive;
    }

    /// <summary>
    /// 建立已保存的原生光源：位置是原生 XYZ（不可再乘 tile／relief），RGB 與 effective radius 為保存值。
    /// 不驗證也不夾值（nonfinite／radius &lt;= 0 以 <see cref="IsRenderable"/> 在繪製時略過）；definition 只作 metadata。
    /// </summary>
    public static NativeLightInstance CreateSaved(int slot, Vector3 nativePosition, Vector3 rgb, float effectiveRadius,
        float baselineRadius = float.NaN, bool isActive = true, NativeLightDefinition? definition = null)
        => new(slot, nativePosition, rgb, effectiveRadius, baselineRadius, isActive, definition);

    private void ThrowIfSaved()
    {
        if (IsSaved) throw new InvalidOperationException("已保存光源快照不可變。");
    }
}

/// <summary>
/// 明確的物件識別鍵（來源檔＋該檔內 ObjectIndex），由呼叫端指定哪些場景物件已有保存光源而不再由 APT／lidef 合成。
/// 不從名稱或 family 推斷；只有 ObjectIndex == -1 沒有鍵。其餘包含負索引的鍵皆由 caller 明確提供。
/// </summary>
public readonly struct SceneLightSuppressionKey : IEquatable<SceneLightSuppressionKey>
{
    public string SourceFile { get; }
    public int ObjectIndex { get; }

    public SceneLightSuppressionKey(string sourceFile, int objectIndex)
    {
        ArgumentNullException.ThrowIfNull(sourceFile);
        ArgumentOutOfRangeException.ThrowIfEqual(objectIndex, -1);
        SourceFile = sourceFile.Replace('\\', '/').ToUpperInvariant(); ObjectIndex = objectIndex;
    }

    public static bool TryFor(AgainstRomeModifier.Maps.MapSceneObject item, out SceneLightSuppressionKey key)
    {
        key = default;
        if (item.ObjectIndex == -1) return false;
        key = new SceneLightSuppressionKey(item.SourceFile ?? string.Empty, item.ObjectIndex);
        return true;
    }

    public bool Equals(SceneLightSuppressionKey other) => ObjectIndex == other.ObjectIndex && string.Equals(SourceFile, other.SourceFile, StringComparison.Ordinal);
    public override bool Equals(object? obj) => obj is SceneLightSuppressionKey other && Equals(other);
    public override int GetHashCode() => HashCode.Combine(SourceFile, ObjectIndex);
    public static bool operator ==(SceneLightSuppressionKey left, SceneLightSuppressionKey right) => left.Equals(right);
    public static bool operator !=(SceneLightSuppressionKey left, SceneLightSuppressionKey right) => !left.Equals(right);
}

/// <summary>
/// 不可變的已保存光源快照輸入：instance 清單（同座標的不同 instance 全部保留，不去重）＋明確抑制鍵。
/// 由呼叫端（Form／Rust projection）建立；view 不讀檔、不載入 native DLL。
/// </summary>
public sealed class SavedSceneLightSet
{
    public static SavedSceneLightSet Empty { get; } = new([], null);

    public System.Collections.Immutable.ImmutableArray<NativeLightInstance> Instances { get; }
    public System.Collections.Immutable.ImmutableHashSet<SceneLightSuppressionKey> SuppressedObjects { get; }

    public SavedSceneLightSet(IEnumerable<NativeLightInstance> instances, IEnumerable<SceneLightSuppressionKey>? suppressedObjects)
    {
        ArgumentNullException.ThrowIfNull(instances);
        Instances = System.Collections.Immutable.ImmutableArray.CreateRange(instances);
        foreach (NativeLightInstance light in Instances)
            if (light is null || !light.IsSaved) throw new ArgumentException("僅接受 NativeLightInstance.CreateSaved 建立的保存光源。", nameof(instances));
        SuppressedObjects = System.Collections.Immutable.ImmutableHashSet.CreateRange(suppressedObjects ?? Array.Empty<SceneLightSuppressionKey>());
    }

    public bool HasData => Instances.Length > 0 || SuppressedObjects.Count > 0;
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
