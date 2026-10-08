// 2:1 等角投影渲染分層 (Render Layer)，確保宏觀前後圖層覆蓋順序絕對正確。
using System.Runtime.CompilerServices;

namespace AgainstRomeMapEditor.Rendering;

/// <summary>
/// 2:1 等角投影渲染分層 (Render Layer)，確保宏觀前後圖層覆蓋順序絕對正確。
/// </summary>
public enum IsometricRenderLayer : byte
{
    /// <summary>地面印章、道路痕跡、燒痕（貼地底層）</summary>
    TerrainDecal = 0,

    /// <summary>物件地表投影陰影（緊貼地形表面，被任何實體物件遮擋）</summary>
    GroundShadow = 1,

    /// <summary>地表貼附結構（地基石、矮柵欄、踏步、水溝）</summary>
    TerrainAttachment = 2,

    /// <summary>標準地圖實體（步兵、騎兵、長屋、高塔、樹幹等實體本體）</summary>
    StandardObject = 3,

    /// <summary>高空懸掛與頂層結構（樹冠遮蔽層、挑高屋頂、瞭望台頂部）</summary>
    OverheadCanopy = 4,

    /// <summary>空中特效與飛行單位（鳥類、投石機彈道、煙霧羽流、雲霧）</summary>
    AirborneEffect = 5,
}

/// <summary>
/// 物件在世界坐標系下的佔地尺寸與物理高度。
/// </summary>
public readonly record struct IsometricFootprint(
    float WidthX,  // 世界 X 方向佔地寬度 (World Units)
    float DepthZ,  // 世界 Z 方向佔地深度 (World Units)
    float HeightY  // 物件垂直物理高度 (World Units)
)
{
    /// <summary>點狀物件（單位、小旗幟、標記）的零足跡預設值。</summary>
    public static readonly IsometricFootprint Point = new(0f, 0f, 0f);

    /// <summary>標準單兵佔地（約 1/4 地圖格）。</summary>
    public static readonly IsometricFootprint Unit = new(0.5f, 0.5f, 1.8f);

    /// <summary>標準日耳曼長屋佔地（例如 4x3 格，高度 6 單位）。</summary>
    public static readonly IsometricFootprint Longhouse = new(4.0f, 3.0f, 6.0f);

    /// <summary>標準羅馬高塔佔地（例如 2x2 格，高度 14 單位）。</summary>
    public static readonly IsometricFootprint RomanTower = new(2.0f, 2.0f, 14.0f);

    /// <summary>計算沿視線方向的最南端地面偏移量 (Southmost Footprint Offset)。</summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public float GetSouthmostGroundOffset() => (WidthX + DepthZ) * 0.5f;
}

/// <summary>
/// 封裝待排序物件的完整幾何與層級元數據。
/// </summary>
public readonly struct IsometricSortItem
{
    public int ItemId { get; }
    public float WorldX { get; }
    public float WorldY { get; }
    public float WorldZ { get; }
    public IsometricFootprint Footprint { get; }
    public IsometricRenderLayer Layer { get; }
    public int SubPriority { get; } // 次要微調偏移量 (-128..127)

    public IsometricSortItem(
        int itemId,
        float worldX,
        float worldY,
        float worldZ,
        IsometricFootprint footprint,
        IsometricRenderLayer layer = IsometricRenderLayer.StandardObject,
        int subPriority = 0)
    {
        ItemId = itemId;
        WorldX = worldX;
        WorldY = worldY;
        WorldZ = worldZ;
        Footprint = footprint;
        Layer = layer;
        SubPriority = Math.Clamp(subPriority, -128, 127);
    }
}

/// <summary>
/// 緊湊 64 位元深度定點數排序鍵 (Deterministic 64-bit Depth Key)。
/// 結構劃分：
/// [63..56] (8 bits) : Render Layer (宏觀層級 0..5)
/// [55..24] (32 bits): Ground Depth (地面等角深軸投影定點數，16位整數 + 16位小數)
/// [23..12] (12 bits): Elevation Tier (世界高度補正定點數)
/// [11..0]  (12 bits): Deterministic Tie-Breaker (物件 ID 與偏置雜湊，杜絕同位閃爍)
/// </summary>
public readonly struct IsometricDepthKey : IComparable<IsometricDepthKey>, IEquatable<IsometricDepthKey>
{
    public ulong Value { get; }

    public IsometricDepthKey(ulong value) => Value = value;

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public int CompareTo(IsometricDepthKey other) => Value.CompareTo(other.Value);

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public bool Equals(IsometricDepthKey other) => Value == other.Value;

    public override bool Equals(object? obj) => obj is IsometricDepthKey other && Equals(other);
    public override int GetHashCode() => Value.GetHashCode();
    public override string ToString() => $"DepthKey(0x{Value:X16})";

    public static bool operator ==(IsometricDepthKey a, IsometricDepthKey b) => a.Value == b.Value;
    public static bool operator !=(IsometricDepthKey a, IsometricDepthKey b) => a.Value != b.Value;
    public static bool operator <(IsometricDepthKey a, IsometricDepthKey b) => a.Value < b.Value;
    public static bool operator >(IsometricDepthKey a, IsometricDepthKey b) => a.Value > b.Value;
    public static bool operator <=(IsometricDepthKey a, IsometricDepthKey b) => a.Value <= b.Value;
    public static bool operator >=(IsometricDepthKey a, IsometricDepthKey b) => a.Value >= b.Value;
}

/// <summary>
/// 等角投影 2D/3D 像素級深度排序器 (IsometricDepthSorter)。
/// 解決 2:1 等角投影下多層大型建築、高低差地形部隊、樹冠與微小物件的正確前後遮擋，
/// 提供 64 位元無 GC 定點數鍵值排序與局部重疊拓撲消解。
/// </summary>
public static class IsometricDepthSorter
{
    /// <summary>
    /// 2:1 等角投影標準斜率常數。
    /// ScreenX = (WorldX - WorldZ) * IsometricKx
    /// ScreenY = (WorldX + WorldZ) * IsometricKy - WorldY * HeightScale
    /// </summary>
    public const float IsometricKx = 0.5f;
    public const float IsometricKy = 0.25f;

    /// <summary>
    /// 高度對地面深軸投影的微弱解耦係數。
    /// 在 2.5D 畫家演算法中，地面前後 (X+Z) 具絕對主導權，
    /// 避免高處背景建築因為相機 3D View-Z 靠前而錯誤覆蓋前景矮處物體。
    /// </summary>
    public const float ElevationCouplingFactor = 0.001f;

    /// <summary>
    /// 計算單一物件的緊湊 64 位元深度鍵。
    /// </summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static IsometricDepthKey ComputeKey(in IsometricSortItem item)
    {
        // 1. Layer 分量 (高 8 位)
        ulong layerBits = (ulong)item.Layer << 56;

        // 2. 地面深軸投影分量 (X + Z + 南向足跡擴展)
        // 考量物件佔地幾何，以物件最南端 (最靠近畫面下緣/觀察者) 的接觸點作為主深軸參考
        float southOffset = item.Footprint.GetSouthmostGroundOffset();
        float groundDepth = item.WorldX + item.WorldZ + southOffset;

        // 轉為 16.16 定點數 (支援 0..65535 世界單位，精度 1/65536)
        long fixedGround = (long)(Math.Clamp(groundDepth, 0f, 65535f) * 65536.0f);
        ulong groundBits = ((ulong)fixedGround & 0xFFFFFFFFUL) << 24;

        // 3. 高度階層分量 (Elevation Tier) (12 位，8 位整數 + 4 位小數)
        // 當兩物件地面足跡接近時，高架結構 (如橋樑、二樓平台) 正確覆蓋地面物件
        float elevation = Math.Clamp(item.WorldY, 0f, 255f);
        uint fixedElev = (uint)(elevation * 16.0f) & 0xFFF;
        ulong elevBits = (ulong)fixedElev << 12;

        // 4. Tie-Breaker (低 12 位)：SubPriority (-128..127 映射為 0..255) 加上 ItemId 雜湊
        uint tie = (uint)(((item.SubPriority + 128) & 0xFF) << 4) | ((uint)(item.ItemId * 73856093) & 0x0F);
        ulong tieBits = tie & 0xFFFUL;

        return new IsometricDepthKey(layerBits | groundBits | elevBits | tieBits);
    }

    /// <summary>
    /// 計算浮點連續深度值 (用於 OpenGL/DirectX 深度緩衝或連續著色器傳參)。
    /// </summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static float ComputeContinuousDepth(in IsometricSortItem item)
    {
        float groundComponent = item.WorldX + item.WorldZ + item.Footprint.GetSouthmostGroundOffset();
        float layerComponent = (int)item.Layer * 10000.0f;
        float heightComponent = item.WorldY * ElevationCouplingFactor;
        float priorityComponent = item.SubPriority * 0.0001f;
        return layerComponent + groundComponent + heightComponent + priorityComponent;
    }

    /// <summary>
    /// 判定物件 A 是否在物件 B 的後方（即 A 應先繪製，B 應後繪製而可能遮擋 A）。
    /// 依據三維地面包圍盒與高度進行精確幾何偏序推導。
    /// </summary>
    public static bool ShouldDrawBefore(in IsometricSortItem a, in IsometricSortItem b)
    {
        // 1. 若分屬不同圖層，圖層號小者先繪製
        if (a.Layer != b.Layer)
        {
            return a.Layer < b.Layer;
        }

        // 2. 地面包圍盒判定：計算 A 與 B 的世界空間地面包圍盒
        float aMinX = a.WorldX, aMaxX = a.WorldX + a.Footprint.WidthX;
        float aMinZ = a.WorldZ, aMaxZ = a.WorldZ + a.Footprint.DepthZ;

        float bMinX = b.WorldX, bMaxX = b.WorldX + b.Footprint.WidthX;
        float bMinZ = b.WorldZ, bMaxZ = b.WorldZ + b.Footprint.DepthZ;

        // 若 A 嚴格位於 B 的北方 (A 的最大坐標 <= B 的最小坐標)，A 必先繪製
        if (aMaxX <= bMinX && aMaxZ <= bMinZ) return true;
        if (bMaxX <= aMinX && bMaxZ <= aMinZ) return false;

        // 3. 2:1 等角投影下的對角軸分離測試：
        // 視線方向向量沿著 (+1, +1)
        float aRear = aMinX + aMinZ;
        float aFront = aMaxX + aMaxZ;
        float bRear = bMinX + bMinZ;
        float bFront = bMaxX + bMaxZ;

        if (aFront <= bRear) return true;  // A 完全在 B 後方
        if (bFront <= aRear) return false; // B 完全在 A 後方

        // 4. 重疊交錯情況：比對 64 位元深度鍵
        return ComputeKey(a) < ComputeKey(b);
    }

    /// <summary>
    /// 就地排序待渲染物件陣列（由遠到近，升序排列，即畫家演算法標準順序）。
    /// 零 GC、高效能。
    /// </summary>
    public static void Sort(Span<IsometricSortItem> items)
    {
        if (items.Length <= 1) return;

        // 配對建立鍵值與索引
        Span<SortEntry> entries = items.Length <= 1024
            ? stackalloc SortEntry[items.Length]
            : new SortEntry[items.Length];

        for (int i = 0; i < items.Length; i++)
        {
            entries[i] = new SortEntry(ComputeKey(items[i]), i);
        }

        // 執行內省排序 (Introsort / QuickSort)
        entries.Sort(static (x, y) => x.Key.CompareTo(y.Key));

        // 依排序結果重新排列原 items
        Span<IsometricSortItem> copy = items.Length <= 1024
            ? stackalloc IsometricSortItem[items.Length]
            : new IsometricSortItem[items.Length];

        items.CopyTo(copy);
        for (int i = 0; i < items.Length; i++)
        {
            items[i] = copy[entries[i].OriginalIndex];
        }
    }

    /// <summary>
    /// 輸出排序後的索引陣列，避免移動大型原始資料結構。
    /// </summary>
    public static void SortIndices(ReadOnlySpan<IsometricSortItem> items, Span<int> sortedIndices)
    {
        ArgumentOutOfRangeException.ThrowIfLessThan(sortedIndices.Length, items.Length);
        if (items.Length == 0) return;

        Span<SortEntry> entries = items.Length <= 2048
            ? stackalloc SortEntry[items.Length]
            : new SortEntry[items.Length];

        for (int i = 0; i < items.Length; i++)
        {
            entries[i] = new SortEntry(ComputeKey(items[i]), i);
        }

        entries.Sort(static (x, y) => x.Key.CompareTo(y.Key));

        for (int i = 0; i < items.Length; i++)
        {
            sortedIndices[i] = entries[i].OriginalIndex;
        }
    }

    private readonly struct SortEntry : IComparable<SortEntry>
    {
        public readonly IsometricDepthKey Key;
        public readonly int OriginalIndex;

        public SortEntry(IsometricDepthKey key, int originalIndex)
        {
            Key = key;
            OriginalIndex = originalIndex;
        }

        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public int CompareTo(SortEntry other) => Key.CompareTo(other.Key);
    }
}
