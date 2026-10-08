using System.Numerics;
using System.Runtime.CompilerServices;

namespace AgainstRomeMapEditor.Rendering;

/// <summary>
/// 2D 螢幕空間軸對齊包圍盒 (Screen-Space AABB)。
/// </summary>
public readonly record struct ScreenAabb(float MinX, float MinY, float MaxX, float MaxY)
{
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public bool Intersects(float width, float height) =>
        MaxX >= 0f && MinX <= width && MaxY >= 0f && MinY <= height;

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public bool Intersects(in ScreenAabb other) =>
        MaxX >= other.MinX && MinX <= other.MaxX && MaxY >= other.MinY && MinY <= other.MaxY;
}

/// <summary>
/// 三維空間軸對齊包圍盒 (World-Space 3D AABB)。
/// </summary>
public readonly record struct WorldAabb(
    float MinX, float MinY, float MinZ,
    float MaxX, float MaxY, float MaxZ)
{
    /// <summary>
    /// 從物體地面坐標與足跡構造 3D 包圍盒。
    /// </summary>
    public static WorldAabb FromSortItem(in IsometricSortItem item)
    {
        return new WorldAabb(
            item.WorldX,
            item.WorldY,
            item.WorldZ,
            item.WorldX + Math.Max(item.Footprint.WidthX, 0.5f),
            item.WorldY + Math.Max(item.Footprint.HeightY, 1.0f),
            item.WorldZ + Math.Max(item.Footprint.DepthZ, 0.5f));
    }
}

/// <summary>
/// 空間分割區塊 (Chunk)，用於對場景物件進行階層式粗粒度視錐剔除。
/// 預設以 32x32 世界單位劃分一個 Chunk。
/// </summary>
public sealed class SpatialChunk
{
    public const float DefaultChunkSize = 32.0f;

    public int ChunkX { get; }
    public int ChunkZ { get; }
    public List<int> ObjectIndices { get; } = [];

    public WorldAabb BoundingBox { get; private set; }

    public SpatialChunk(int chunkX, int chunkZ)
    {
        ChunkX = chunkX;
        ChunkZ = chunkZ;
        BoundingBox = new WorldAabb(
            chunkX * DefaultChunkSize, 0f, chunkZ * DefaultChunkSize,
            (chunkX + 1) * DefaultChunkSize, 0f, (chunkZ + 1) * DefaultChunkSize);
    }

    /// <summary>
    /// 擴展區塊包圍盒以包含新加入的物件。
    /// </summary>
    public void ExpandBounds(in WorldAabb itemBounds)
    {
        BoundingBox = new WorldAabb(
            Math.Min(BoundingBox.MinX, itemBounds.MinX),
            Math.Min(BoundingBox.MinY, itemBounds.MinY),
            Math.Min(BoundingBox.MinZ, itemBounds.MinZ),
            Math.Max(BoundingBox.MaxX, itemBounds.MaxX),
            Math.Max(BoundingBox.MaxY, itemBounds.MaxY),
            Math.Max(BoundingBox.MaxZ, itemBounds.MaxZ));
    }
}

/// <summary>
/// 等角投影視錐剔除與視錐裁剪優化器 (IsometricFrustumCuller)。
/// 解決大型多層建築（如羅馬高塔頂部進入螢幕但地面錨點在螢幕外）的錯誤剔除與視野穿透，
/// 提供 3D AABB 螢幕投影裁剪與零 GC 批次剔除合約。
/// </summary>
public static class IsometricFrustumCuller
{
    /// <summary>
    /// 將世界空間 3D AABB 投射至 2D 螢幕空間，取得外接螢幕矩形 (Screen AABB)。
    /// </summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static ScreenAabb ProjectAabbToScreen(
        in WorldAabb aabb,
        in Matrix4x4 viewProjection,
        Vector2 viewport)
    {
        // 評估 3D AABB 的 8 個角點
        Span<Vector3> corners = stackalloc Vector3[8]
        {
            new(aabb.MinX, aabb.MinY, aabb.MinZ),
            new(aabb.MaxX, aabb.MinY, aabb.MinZ),
            new(aabb.MinX, aabb.MinY, aabb.MaxZ),
            new(aabb.MaxX, aabb.MinY, aabb.MaxZ),
            new(aabb.MinX, aabb.MaxY, aabb.MinZ),
            new(aabb.MaxX, aabb.MaxY, aabb.MinZ),
            new(aabb.MinX, aabb.MaxY, aabb.MaxZ),
            new(aabb.MaxX, aabb.MaxY, aabb.MaxZ),
        };

        float minX = float.MaxValue, minY = float.MaxValue;
        float maxX = float.MinValue, maxY = float.MinValue;

        for (int i = 0; i < 8; i++)
        {
            Vector4 clip = Vector4.Transform(new Vector4(corners[i], 1.0f), viewProjection);
            if (Math.Abs(clip.W) < 1e-6f) clip.W = 1.0f;

            float ndcX = clip.X / clip.W;
            float ndcY = clip.Y / clip.W;

            // NDC (-1..1) 映射至螢幕坐標 (0..viewport)
            float sx = (ndcX * 0.5f + 0.5f) * viewport.X;
            float sy = (0.5f - ndcY * 0.5f) * viewport.Y;

            minX = Math.Min(minX, sx);
            minY = Math.Min(minY, sy);
            maxX = Math.Max(maxX, sx);
            maxY = Math.Max(maxY, sy);
        }

        return new ScreenAabb(minX, minY, maxX, maxY);
    }

    /// <summary>
    /// 檢驗單一物件是否落在視窗可見區域內（考慮物件高度與三維足跡）。
    /// </summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static bool IsVisible(
        in IsometricSortItem item,
        in Matrix4x4 viewProjection,
        Vector2 viewport,
        float screenPadding = 32.0f)
    {
        WorldAabb bounds = WorldAabb.FromSortItem(item);
        ScreenAabb screenAabb = ProjectAabbToScreen(bounds, viewProjection, viewport);

        // 考慮安全邊界 (Padding) 避免邊緣閃爍
        return screenAabb.MaxX >= -screenPadding &&
               screenAabb.MinX <= viewport.X + screenPadding &&
               screenAabb.MaxY >= -screenPadding &&
               screenAabb.MinY <= viewport.Y + screenPadding;
    }

    /// <summary>
    /// 批次視錐剔除（零 GC 堆疊/記憶體配置合約）。
    /// 將通過剔除的可見物件索引寫入 visibleIndicesBuffer，回傳可見物件總數。
    /// </summary>
    public static int CullItems(
        ReadOnlySpan<IsometricSortItem> items,
        in Matrix4x4 viewProjection,
        Vector2 viewport,
        Span<int> visibleIndicesBuffer,
        float screenPadding = 32.0f)
    {
        ArgumentOutOfRangeException.ThrowIfLessThan(visibleIndicesBuffer.Length, items.Length);

        int visibleCount = 0;
        for (int i = 0; i < items.Length; i++)
        {
            if (IsVisible(items[i], viewProjection, viewport, screenPadding))
            {
                visibleIndicesBuffer[visibleCount++] = i;
            }
        }

        return visibleCount;
    }

    /// <summary>
    /// 構建場景物件的粗篩空間區塊網格 (Spatial Chunk Grid)。
    /// </summary>
    public static Dictionary<(int, int), SpatialChunk> BuildChunkGrid(ReadOnlySpan<IsometricSortItem> items)
    {
        var chunks = new Dictionary<(int, int), SpatialChunk>();

        for (int i = 0; i < items.Length; i++)
        {
            int cx = (int)MathF.Floor(items[i].WorldX / SpatialChunk.DefaultChunkSize);
            int cz = (int)MathF.Floor(items[i].WorldZ / SpatialChunk.DefaultChunkSize);

            if (!chunks.TryGetValue((cx, cz), out var chunk))
            {
                chunk = new SpatialChunk(cx, cz);
                chunks.Add((cx, cz), chunk);
            }

            chunk.ObjectIndices.Add(i);
            chunk.ExpandBounds(WorldAabb.FromSortItem(items[i]));
        }

        return chunks;
    }

    /// <summary>
    /// 利用空間區塊網格進行階層式剔除加速：
    /// 先粗篩淘汰整個區塊，再細篩區塊內之物件。
    /// </summary>
    public static int CullWithChunks(
        ReadOnlySpan<IsometricSortItem> items,
        IReadOnlyCollection<SpatialChunk> chunks,
        in Matrix4x4 viewProjection,
        Vector2 viewport,
        Span<int> visibleIndicesBuffer,
        float screenPadding = 32.0f)
    {
        int visibleCount = 0;

        foreach (var chunk in chunks)
        {
            // 1. 區塊粗篩：檢驗 Chunk 的整體 3D AABB 是否與螢幕相交
            ScreenAabb chunkScreen = ProjectAabbToScreen(chunk.BoundingBox, viewProjection, viewport);
            if (chunkScreen.MaxX < -screenPadding || chunkScreen.MinX > viewport.X + screenPadding ||
                chunkScreen.MaxY < -screenPadding || chunkScreen.MinY > viewport.Y + screenPadding)
            {
                continue; // 整個 Chunk 都在螢幕外，直接略過內部所有物件！
            }

            // 2. 區塊內細篩
            foreach (int index in chunk.ObjectIndices)
            {
                if (IsVisible(items[index], viewProjection, viewport, screenPadding))
                {
                    visibleIndicesBuffer[visibleCount++] = index;
                }
            }
        }

        return visibleCount;
    }
}
