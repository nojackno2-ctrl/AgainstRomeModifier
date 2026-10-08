namespace AgainstRomeMapEditor.Modules.Diff;

/// <summary>
/// 視覺遮罩標記類型。
/// </summary>
public enum DiffMarkerType
{
    /// <summary>新增物件 (綠色)。</summary>
    Added,

    /// <summary>刪除物件 (紅色幽靈標記)。</summary>
    Deleted,

    /// <summary>修改屬性 (黃色)。</summary>
    Modified,

    /// <summary>移動物件 (黃色箭頭軌跡)。</summary>
    Moved
}

/// <summary>
/// 向量視覺標記（供 2D 畫布與 3D 視圖疊加繪製）。
/// </summary>
public sealed record DiffOverlayMarker(
    DiffMarkerType Type,
    float WorldX,
    float WorldZ,
    float TargetWorldX,
    float TargetWorldZ,
    uint ColorArgb,
    string Label,
    string SubLabel
);

/// <summary>
/// 視覺遮罩輸出選項。
/// </summary>
public sealed record DiffOverlayOptions(
    bool ShowHeightHeatmap = true,
    bool ShowTextureDiff = true,
    bool ShowCollisionDiff = false,
    bool ShowObjects = true,
    float GlobalOpacity = 0.7f,
    float HeightSensitivity = 1.0f,
    bool ShowMovementVectors = true
);

/// <summary>
/// 點陣熱力遮罩緩衝區（32 位元 ARGB 像素陣列）。
/// </summary>
public sealed class HeatmapMaskBuffer
{
    public int Width { get; }
    public int Height { get; }
    public uint[] Pixels { get; }

    public HeatmapMaskBuffer(int width, int height)
    {
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(width);
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(height);
        Width = width;
        Height = height;
        Pixels = new uint[width * height];
    }

    public void SetPixel(int x, int y, uint argb)
    {
        if (x >= 0 && x < Width && y >= 0 && y < Height)
        {
            Pixels[y * Width + x] = argb;
        }
    }

    public uint GetPixel(int x, int y)
    {
        if (x >= 0 && x < Width && y >= 0 && y < Height)
        {
            return Pixels[y * Width + x];
        }
        return 0;
    }
}

/// <summary>
/// 地圖歷史版本差異視覺化檢視遮罩引擎 (MapVisualDiffOverlay)。
/// 負責產出 2D 畫布與小地圖的紅（刪除）、綠（新增）、黃（修改）熱力遮罩與向量軌跡。
/// </summary>
public static class MapVisualDiffOverlay
{
    // 標準配色常數 (AARRGGBB)
    public const uint ColorAdded = 0xA000E676;       // 亮綠色半透明
    public const uint ColorDeleted = 0xA0FF1744;     // 亮紅色半透明
    public const uint ColorModified = 0xB0FFD600;    // 琥珀黃半透明
    public const uint ColorMoved = 0xC0FFA000;       // 亮橘黃色半透明
    public const uint ColorBlocked = 0xA0D50000;     // 碰撞阻擋 (深紅)
    public const uint ColorCleared = 0xA000B0FF;     // 碰撞清除 (天藍)

    /// <summary>
    /// 產出 2D 畫布與視圖疊加用之點陣熱力遮罩緩衝區。
    /// </summary>
    public static HeatmapMaskBuffer GenerateHeatmapMask(
        MapDiffReport diff,
        int outputWidth,
        int outputHeight,
        DiffOverlayOptions? options = null)
    {
        ArgumentNullException.ThrowIfNull(diff);
        options ??= new DiffOverlayOptions();

        var buffer = new HeatmapMaskBuffer(outputWidth, outputHeight);
        float opacity = Math.Clamp(options.GlobalOpacity, 0.0f, 1.0f);
        if (opacity <= 0.001f) return buffer;

        // 1. 疊加地形高度差異熱力
        if (options.ShowHeightHeatmap && diff.Height.Dimension > 0 && diff.Height.DeltaGrid.Length > 0)
        {
            int hDim = diff.Height.Dimension;
            float[] deltas = diff.Height.DeltaGrid;
            float maxDelta = Math.Max(diff.Height.MaxRaise, diff.Height.MaxLower);
            if (maxDelta <= 0.001f) maxDelta = 1.0f;

            for (int py = 0; py < outputHeight; py++)
            {
                for (int px = 0; px < outputWidth; px++)
                {
                    // 雙線性取樣高度差
                    float gx = (float)px / (outputWidth - 1) * (hDim - 1);
                    float gy = (float)py / (outputHeight - 1) * (hDim - 1);

                    int x0 = Math.Clamp((int)MathF.Floor(gx), 0, hDim - 1);
                    int y0 = Math.Clamp((int)MathF.Floor(gy), 0, hDim - 1);
                    int x1 = Math.Clamp(x0 + 1, 0, hDim - 1);
                    int y1 = Math.Clamp(y0 + 1, 0, hDim - 1);

                    float fx = gx - x0;
                    float fy = gy - y0;

                    float d00 = deltas[y0 * hDim + x0];
                    float d10 = deltas[y0 * hDim + x1];
                    float d01 = deltas[y1 * hDim + x0];
                    float d11 = deltas[y1 * hDim + x1];

                    float delta = (1 - fx) * (1 - fy) * d00 + fx * (1 - fy) * d10 + (1 - fx) * fy * d01 + fx * fy * d11;

                    if (MathF.Abs(delta) > 0.1f)
                    {
                        float norm = Math.Clamp(MathF.Abs(delta) / maxDelta * options.HeightSensitivity, 0.0f, 1.0f);
                        byte alpha = (byte)Math.Clamp((int)(norm * 180 * opacity), 0, 255);

                        uint color;
                        if (delta > 0)
                        {
                            // 抬升：綠色漸層 (0, 230, 118)
                            color = ((uint)alpha << 24) | 0x0000E676;
                        }
                        else
                        {
                            // 下陷：紅色漸層 (255, 23, 68)
                            color = ((uint)alpha << 24) | 0x00FF1744;
                        }

                        buffer.SetPixel(px, py, color);
                    }
                }
            }
        }

        // 2. 疊加地表材質變更網格
        if (options.ShowTextureDiff && diff.Textures.Dimension > 0 && diff.Textures.ChangedTiles.Count > 0)
        {
            int tDim = diff.Textures.Dimension;
            float tileW = (float)outputWidth / tDim;
            float tileH = (float)outputHeight / tDim;

            byte texAlpha = (byte)Math.Clamp((int)(160 * opacity), 0, 255);
            uint texColor = ((uint)texAlpha << 24) | (ColorModified & 0x00FFFFFF);

            foreach (var change in diff.Textures.ChangedTiles)
            {
                int startX = (int)MathF.Floor(change.TileX * tileW);
                int endX = (int)MathF.Ceiling((change.TileX + 1) * tileW);
                int startY = (int)MathF.Floor(change.TileY * tileH);
                int endY = (int)MathF.Ceiling((change.TileY + 1) * tileH);

                for (int y = startY; y < endY; y++)
                {
                    for (int x = startX; x < endX; x++)
                    {
                        // 混合至緩衝區
                        buffer.SetPixel(x, y, BlendArgb(buffer.GetPixel(x, y), texColor));
                    }
                }
            }
        }

        // 3. 疊加通行碰撞變更
        if (options.ShowCollisionDiff && diff.Collision.Changes.Count > 0 && diff.Collision.Dimension > 0)
        {
            int cDim = diff.Collision.Dimension;
            float cW = (float)outputWidth / cDim;
            float cH = (float)outputHeight / cDim;

            foreach (var col in diff.Collision.Changes)
            {
                uint colColor = col.OldValue == 0 ? ColorBlocked : ColorCleared;
                byte a = (byte)Math.Clamp((int)(((colColor >> 24) & 0xFF) * opacity), 0, 255);
                colColor = ((uint)a << 24) | (colColor & 0x00FFFFFF);

                int startX = (int)MathF.Floor(col.TileX * cW);
                int endX = (int)MathF.Ceiling((col.TileX + 1) * cW);
                int startY = (int)MathF.Floor(col.TileY * cH);
                int endY = (int)MathF.Ceiling((col.TileY + 1) * cH);

                for (int y = startY; y < endY; y++)
                {
                    for (int x = startX; x < endX; x++)
                    {
                        buffer.SetPixel(x, y, BlendArgb(buffer.GetPixel(x, y), colColor));
                    }
                }
            }
        }

        return buffer;
    }

    /// <summary>
    /// 產出 256×256 小地圖專用疊加遮罩。
    /// </summary>
    public static HeatmapMaskBuffer GenerateMinimapOverlay(
        MapDiffReport diff,
        int minimapWidth = 256,
        int minimapHeight = 256,
        DiffOverlayOptions? options = null)
    {
        options ??= new DiffOverlayOptions();
        var buffer = GenerateHeatmapMask(diff, minimapWidth, minimapHeight, options);

        // 在小地圖上標繪顯著物件異動點
        if (options.ShowObjects && diff.Objects.TotalChanges > 0)
        {
            float scaleX = (float)minimapWidth / 16384f;
            float scaleZ = (float)minimapHeight / 16384f;

            // 新增物件 (綠色 2x2 方塊)
            foreach (var obj in diff.Objects.Added)
            {
                int mx = (int)MathF.Round(obj.X * scaleX);
                int my = (int)MathF.Round(obj.Z * scaleZ);
                DrawBlock(buffer, mx, my, 2, ColorAdded);
            }

            // 刪除物件 (紅色 2x2 方塊)
            foreach (var obj in diff.Objects.Deleted)
            {
                int mx = (int)MathF.Round(obj.X * scaleX);
                int my = (int)MathF.Round(obj.Z * scaleZ);
                DrawBlock(buffer, mx, my, 2, ColorDeleted);
            }

            // 修改/移動物件 (黃色 2x2 方塊)
            foreach (var mod in diff.Objects.Modified)
            {
                int mx = (int)MathF.Round(mod.Current.X * scaleX);
                int my = (int)MathF.Round(mod.Current.Z * scaleZ);
                DrawBlock(buffer, mx, my, 2, ColorModified);
            }
        }

        return buffer;
    }

    /// <summary>
    /// 產出 2D 畫布與 3D 視圖之向量標記清單（包含移動軌跡向量、邊框與文字說明）。
    /// </summary>
    public static IReadOnlyList<DiffOverlayMarker> GenerateVectorMarkers(
        MapDiffReport diff,
        DiffOverlayOptions? options = null)
    {
        ArgumentNullException.ThrowIfNull(diff);
        options ??= new DiffOverlayOptions();

        var markers = new List<DiffOverlayMarker>();
        if (!options.ShowObjects) return markers;

        // 新增物件 (綠色)
        foreach (var obj in diff.Objects.Added)
        {
            string label = $"+ {FormatObjectTitle(obj)}";
            string subLabel = $"坐標: ({obj.X:F0}, {obj.Z:F0}) 隊伍: {obj.Team}";
            markers.Add(new DiffOverlayMarker(
                DiffMarkerType.Added,
                obj.X,
                obj.Z,
                obj.X,
                obj.Z,
                ColorAdded,
                label,
                subLabel
            ));
        }

        // 刪除物件 (紅色幽靈框)
        foreach (var obj in diff.Objects.Deleted)
        {
            string label = $"- {FormatObjectTitle(obj)} [已刪除]";
            string subLabel = $"原坐標: ({obj.X:F0}, {obj.Z:F0})";
            markers.Add(new DiffOverlayMarker(
                DiffMarkerType.Deleted,
                obj.X,
                obj.Z,
                obj.X,
                obj.Z,
                ColorDeleted,
                label,
                subLabel
            ));
        }

        // 修改與移動物件 (黃色/橘色)
        foreach (var mod in diff.Objects.Modified)
        {
            if (mod.DistanceMoved > 0.5f && options.ShowMovementVectors)
            {
                string label = $"-> {FormatObjectTitle(mod.Current)} [移動 {mod.DistanceMoved:F1}m]";
                string subLabel = string.Join("; ", mod.ChangedAttributes);
                markers.Add(new DiffOverlayMarker(
                    DiffMarkerType.Moved,
                    mod.Baseline.X,
                    mod.Baseline.Z,
                    mod.Current.X,
                    mod.Current.Z,
                    ColorMoved,
                    label,
                    subLabel
                ));
            }
            else
            {
                string label = $"* {FormatObjectTitle(mod.Current)} [修改]";
                string subLabel = string.Join("; ", mod.ChangedAttributes);
                markers.Add(new DiffOverlayMarker(
                    DiffMarkerType.Modified,
                    mod.Current.X,
                    mod.Current.Z,
                    mod.Current.X,
                    mod.Current.Z,
                    ColorModified,
                    label,
                    subLabel
                ));
            }
        }

        return markers;
    }

    private static string FormatObjectTitle(DiffObjectItem item)
    {
        string alias = string.IsNullOrEmpty(item.Alias) ? $"Type {item.TypeId}" : item.Alias;
        return item.UnitCount > 0 ? $"{alias} ({item.UnitCount}人)" : alias;
    }

    private static void DrawBlock(HeatmapMaskBuffer buffer, int cx, int cy, int size, uint color)
    {
        int half = size / 2;
        for (int dy = -half; dy <= half; dy++)
        {
            for (int dx = -half; dx <= half; dx++)
            {
                buffer.SetPixel(cx + dx, cy + dy, color);
            }
        }
    }

    private static uint BlendArgb(uint baseColor, uint overColor)
    {
        byte aOver = (byte)((overColor >> 24) & 0xFF);
        if (aOver == 0) return baseColor;
        if (aOver == 255) return overColor;

        byte aBase = (byte)((baseColor >> 24) & 0xFF);
        float alphaNorm = aOver / 255.0f;
        float invAlpha = 1.0f - alphaNorm;

        byte rOver = (byte)((overColor >> 16) & 0xFF);
        byte gOver = (byte)((overColor >> 8) & 0xFF);
        byte bOver = (byte)(overColor & 0xFF);

        byte rBase = (byte)((baseColor >> 16) & 0xFF);
        byte gBase = (byte)((baseColor >> 8) & 0xFF);
        byte bBase = (byte)(baseColor & 0xFF);

        byte rOut = (byte)Math.Clamp((int)(rOver * alphaNorm + rBase * invAlpha), 0, 255);
        byte gOut = (byte)Math.Clamp((int)(gOver * alphaNorm + gBase * invAlpha), 0, 255);
        byte bOut = (byte)Math.Clamp((int)(bOver * alphaNorm + bBase * invAlpha), 0, 255);
        byte aOut = (byte)Math.Clamp((int)(aOver + aBase * (1.0f - alphaNorm)), 0, 255);

        return ((uint)aOut << 24) | ((uint)rOut << 16) | ((uint)gOut << 8) | bOut;
    }
}
