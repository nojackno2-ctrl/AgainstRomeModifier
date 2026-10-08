namespace AgainstRomeMapEditor.Modules.Packaging;

/// <summary>小地圖與發布縮圖中繪製的出生點與戰略標記。</summary>
public sealed record MapThumbnailSpawnPoint(
    float WorldX,
    float WorldZ,
    int Team,
    string Label = "",
    bool IsPlayer = true);

/// <summary>縮圖渲染器組態設定。</summary>
public sealed record ThumbnailRenderOptions
{
    public int Width { get; init; } = 512;
    public int Height { get; init; } = 512;
    public float SunAzimuthDegrees { get; init; } = 315f;   // 西北偏北光源
    public float SunAltitudeDegrees { get; init; } = 45f;    // 45 度俯角
    public float ReliefStrength { get; init; } = 1.35f;      // 浮雕光影對比強度
    public bool ShowWater { get; init; } = true;
    public bool ShowShorelines { get; init; } = true;
    public bool ShowSpawnBanners { get; init; } = true;
    public bool ShowRoads { get; init; } = true;
    public float WaterLevel { get; init; }
    public float HeightStep { get; init; } = 4.0f;
}

/// <summary>
/// 自動繪製高質感發布縮圖渲染器 (MinimapThumbnailRenderer)。
/// 結合地形高度立體浮雕 (Horn's Hillshading)、水體深淺層次、海岸浪花、道路網與玩家軍旗標記。
/// 純受控 C# 實作，零 GPU 依賴，支援無周邊 (headless) CLI 與自動化打包管線。
/// </summary>
public static class MinimapThumbnailRenderer
{
    // 各隊伍的代表色盤 (Team 0..7)
    private static readonly (byte R, byte G, byte B)[] TeamPalette =
    [
        (30, 110, 235),  // 0: 玩家/藍
        (225, 45, 45),   // 1: 鮮紅
        (35, 175, 60),   // 2: 翡翠綠
        (240, 195, 25),  // 3: 琥珀金
        (165, 55, 215),  // 4: 紫羅蘭
        (25, 205, 215),  // 5: 青藍
        (240, 125, 25),  // 6: 焰橘
        (215, 220, 230)  // 7: 白銀
    ];

    /// <summary>
    /// 渲染並輸出標準 24-bit 未壓縮 Windows BMP 影像位元組（影像自底向上存放，附 54-byte BMP 標頭）。
    /// </summary>
    public static byte[] RenderToBmp(
        IReadOnlyList<byte>? heightGrid,
        int heightSize,
        ThumbnailRenderOptions options,
        IReadOnlyList<byte>? collisionGrid = null,
        int collisionSize = 256,
        IReadOnlyList<MapThumbnailSpawnPoint>? spawnPoints = null,
        IReadOnlyList<(float X, float Z)>? roadWaypoints = null)
    {
        int width = options.Width;
        int height = options.Height;
        byte[] rgba = RenderToRgba(heightGrid, heightSize, options, collisionGrid, collisionSize, spawnPoints, roadWaypoints);

        int stride = (width * 3 + 3) & ~3;
        int imageSize = stride * height;
        int fileSize = 54 + imageSize;
        byte[] bmp = new byte[fileSize];

        // BITMAPFILEHEADER
        bmp[0] = (byte)'B';
        bmp[1] = (byte)'M';
        BitConverter.TryWriteBytes(bmp.AsSpan(2), fileSize);
        BitConverter.TryWriteBytes(bmp.AsSpan(10), 54);

        // BITMAPINFOHEADER
        BitConverter.TryWriteBytes(bmp.AsSpan(14), 40);
        BitConverter.TryWriteBytes(bmp.AsSpan(18), width);
        BitConverter.TryWriteBytes(bmp.AsSpan(22), height);
        BitConverter.TryWriteBytes(bmp.AsSpan(26), (short)1);
        BitConverter.TryWriteBytes(bmp.AsSpan(28), (short)24);
        BitConverter.TryWriteBytes(bmp.AsSpan(34), imageSize);

        // 像素資料：BMP 為由下而上、BGR 順序
        for (int y = 0; y < height; y++)
        {
            int bmpRowOffset = 54 + (height - 1 - y) * stride;
            int rgbaRowOffset = y * width * 4;

            for (int x = 0; x < width; x++)
            {
                int src = rgbaRowOffset + x * 4;
                int dst = bmpRowOffset + x * 3;

                bmp[dst] = rgba[src + 2];     // Blue
                bmp[dst + 1] = rgba[src + 1]; // Green
                bmp[dst + 2] = rgba[src];     // Red
            }
        }

        return bmp;
    }

    /// <summary>
    /// 渲染為 RGBA32 格式像素緩衝區（行優先，每像素 4 位元組：R, G, B, A）。
    /// </summary>
    public static byte[] RenderToRgba(
        IReadOnlyList<byte>? heightGrid,
        int heightSize,
        ThumbnailRenderOptions options,
        IReadOnlyList<byte>? collisionGrid = null,
        int collisionSize = 256,
        IReadOnlyList<MapThumbnailSpawnPoint>? spawnPoints = null,
        IReadOnlyList<(float X, float Z)>? roadWaypoints = null)
    {
        int width = options.Width;
        int height = options.Height;
        byte[] pixels = new byte[width * height * 4];

        float sunAzRad = options.SunAzimuthDegrees * MathF.PI / 180f;
        float sunAltRad = options.SunAltitudeDegrees * MathF.PI / 180f;

        // 計算太陽光照向量 (L)
        float lx = MathF.Sin(sunAzRad) * MathF.Cos(sunAltRad);
        float ly = -MathF.Cos(sunAzRad) * MathF.Cos(sunAltRad);
        float lz = MathF.Sin(sunAltRad);

        bool hasHeights = heightGrid != null && heightSize > 1 && heightGrid.Count >= heightSize * heightSize;
        float waterWorld = options.WaterLevel;
        float heightStep = options.HeightStep;

        // 第一階段：地貌浮雕、水體與海岸渲染
        for (int py = 0; py < height; py++)
        {
            float normY = (py + 0.5f) / height;
            for (int px = 0; px < width; px++)
            {
                float normX = (px + 0.5f) / width;
                int pixelIndex = (py * width + px) * 4;

                float elevation = 0f;
                float shadeFactor = 1.0f;
                bool isSubmerged = false;
                float waterDepth = 0f;

                if (hasHeights)
                {
                    float gx = normX * (heightSize - 1);
                    float gy = normY * (heightSize - 1);

                    int x0 = Math.Clamp((int)gx, 0, heightSize - 1);
                    int y0 = Math.Clamp((int)gy, 0, heightSize - 1);
                    int x1 = Math.Min(x0 + 1, heightSize - 1);
                    int y1 = Math.Min(y0 + 1, heightSize - 1);

                    float fx = gx - x0;
                    float fy = gy - y0;

                    float h00 = heightGrid![y0 * heightSize + x0];
                    float h10 = heightGrid[y0 * heightSize + x1];
                    float h01 = heightGrid[y1 * heightSize + x0];
                    float h11 = heightGrid[y1 * heightSize + x1];

                    // 雙線性內插取樣高度
                    float rawHeight = (1 - fx) * (1 - fy) * h00 + fx * (1 - fy) * h10 + (1 - fx) * fy * h01 + fx * fy * h11;
                    elevation = rawHeight * heightStep;

                    // 中心差分梯度計算
                    int xm = Math.Max(0, x0 - 1);
                    int xp = Math.Min(heightSize - 1, x1 + 1);
                    int ym = Math.Max(0, y0 - 1);
                    int yp = Math.Min(heightSize - 1, y1 + 1);

                    float dzdx = ((heightGrid[y0 * heightSize + xp] - heightGrid[y0 * heightSize + xm]) * heightStep) / (xp - xm);
                    float dzdy = ((heightGrid[yp * heightSize + x0] - heightGrid[ym * heightSize + x0]) * heightStep) / (yp - ym);

                    // 法向量 N = (-dzdx * relief, -dzdy * relief, 1.0)
                    float nx = -dzdx * options.ReliefStrength * 0.1f;
                    float ny = -dzdy * options.ReliefStrength * 0.1f;
                    float nz = 1.0f;
                    float invLen = 1.0f / MathF.Sqrt(nx * nx + ny * ny + nz * nz);
                    nx *= invLen; ny *= invLen; nz *= invLen;

                    // 蘭伯特光照反射模型 (Lambertian Shading)
                    float dot = nx * lx + ny * ly + nz * lz;
                    shadeFactor = Math.Clamp(0.35f + 0.65f * MathF.Max(0f, dot), 0.25f, 1.75f);

                    if (options.ShowWater && elevation < waterWorld)
                    {
                        isSubmerged = true;
                        waterDepth = waterWorld - elevation;
                    }
                }

                // 基礎著色
                byte r, g, b;
                if (isSubmerged)
                {
                    if (options.ShowShorelines && waterDepth <= 3.0f)
                    {
                        // 海岸沙灘淺灘浪花邊界
                        float t = waterDepth / 3.0f;
                        r = (byte)Math.Clamp((1 - t) * 220 + t * 65, 0, 255);
                        g = (byte)Math.Clamp((1 - t) * 205 + t * 155, 0, 255);
                        b = (byte)Math.Clamp((1 - t) * 150 + t * 195, 0, 255);
                    }
                    else
                    {
                        // 深淺水體漸變
                        float depthRatio = Math.Clamp(waterDepth / 50.0f, 0f, 1f);
                        r = (byte)Math.Clamp((1 - depthRatio) * 45 + depthRatio * 15, 0, 255);
                        g = (byte)Math.Clamp((1 - depthRatio) * 140 + depthRatio * 45, 0, 255);
                        b = (byte)Math.Clamp((1 - depthRatio) * 185 + depthRatio * 95, 0, 255);
                    }
                }
                else
                {
                    // 陸地隨高度與浮雕光照之漸變色
                    if (elevation > 550f)
                    {
                        // 高山白雪
                        r = 220; g = 225; b = 230;
                    }
                    else if (elevation > 350f)
                    {
                        // 岩壁石山
                        r = 130; g = 125; b = 120;
                    }
                    else if (elevation > 180f)
                    {
                        // 高地丘陵
                        r = 95; g = 135; b = 70;
                    }
                    else
                    {
                        // 平原草地
                        r = 65; g = 118; b = 52;
                    }

                    // 乘上地形浮雕陰影
                    r = (byte)Math.Clamp(r * shadeFactor, 0, 255);
                    g = (byte)Math.Clamp(g * shadeFactor, 0, 255);
                    b = (byte)Math.Clamp(b * shadeFactor, 0, 255);
                }

                pixels[pixelIndex] = r;
                pixels[pixelIndex + 1] = g;
                pixels[pixelIndex + 2] = b;
                pixels[pixelIndex + 3] = 255;
            }
        }

        // 第二階段：道路網絡覆蓋層
        if (options.ShowRoads && roadWaypoints != null && roadWaypoints.Count > 1)
        {
            RenderRoadNetwork(pixels, width, height, roadWaypoints);
        }

        // 第三階段：玩家出發點軍旗與標記
        if (options.ShowSpawnBanners && spawnPoints != null && spawnPoints.Count > 0)
        {
            RenderSpawnBanners(pixels, width, height, spawnPoints);
        }

        return pixels;
    }

    private static void RenderRoadNetwork(byte[] pixels, int width, int height, IReadOnlyList<(float X, float Z)> waypoints)
    {
        for (int i = 0; i < waypoints.Count - 1; i++)
        {
            var p1 = waypoints[i];
            var p2 = waypoints[i + 1];

            int x0 = (int)(p1.X / 16384f * width);
            int y0 = (int)(p1.Z / 16384f * height);
            int x1 = (int)(p2.X / 16384f * width);
            int y1 = (int)(p2.Z / 16384f * height);

            DrawThickLine(pixels, width, height, x0, y0, x1, y1, 3, (180, 168, 142));
        }
    }

    private static void DrawThickLine(byte[] pixels, int width, int height, int x0, int y0, int x1, int y1, int thickness, (byte R, byte G, byte B) color)
    {
        int dx = Math.Abs(x1 - x0), sx = x0 < x1 ? 1 : -1;
        int dy = -Math.Abs(y1 - y0), sy = y0 < y1 ? 1 : -1;
        int err = dx + dy;

        while (true)
        {
            for (int oy = -thickness / 2; oy <= thickness / 2; oy++)
            for (int ox = -thickness / 2; ox <= thickness / 2; ox++)
            {
                int px = x0 + ox;
                int py = y0 + oy;
                if (px >= 0 && px < width && py >= 0 && py < height)
                {
                    int idx = (py * width + px) * 4;
                    pixels[idx] = color.R;
                    pixels[idx + 1] = color.G;
                    pixels[idx + 2] = color.B;
                }
            }

            if (x0 == x1 && y0 == y1) break;
            int e2 = 2 * err;
            if (e2 >= dy) { err += dy; x0 += sx; }
            if (e2 <= dx) { err += dx; y0 += sy; }
        }
    }

    private static void RenderSpawnBanners(byte[] pixels, int width, int height, IReadOnlyList<MapThumbnailSpawnPoint> spawns)
    {
        int bannerRadius = Math.Clamp(width / 36, 6, 16);

        foreach (var spawn in spawns)
        {
            int cx = (int)(spawn.WorldX / 16384f * width);
            int cy = (int)(spawn.WorldZ / 16384f * height);

            if (cx < bannerRadius || cx >= width - bannerRadius || cy < bannerRadius || cy >= height - bannerRadius)
                continue;

            int teamIndex = Math.Clamp(spawn.Team, 0, TeamPalette.Length - 1);
            var (tR, tG, tB) = TeamPalette[teamIndex];

            // 繪製陰影、圓形外框與軍旗本體
            for (int dy = -bannerRadius - 2; dy <= bannerRadius + 2; dy++)
            for (int dx = -bannerRadius - 2; dx <= bannerRadius + 2; dx++)
            {
                int px = cx + dx;
                int py = cy + dy;
                if (px < 0 || px >= width || py < 0 || py >= height) continue;

                float distSq = dx * dx + dy * dy;
                float rOuterSq = (bannerRadius + 1.5f) * (bannerRadius + 1.5f);
                float rInnerSq = (bannerRadius - 1.5f) * (bannerRadius - 1.5f);
                float rCenterSq = (bannerRadius - 4f) * (bannerRadius - 4f);

                int idx = (py * width + px) * 4;

                if (distSq <= rCenterSq)
                {
                    // 中心高亮標記（白色十字或亮心）
                    if (Math.Abs(dx) <= 1 || Math.Abs(dy) <= 1)
                    {
                        pixels[idx] = 255;
                        pixels[idx + 1] = 255;
                        pixels[idx + 2] = 255;
                    }
                    else
                    {
                        pixels[idx] = tR;
                        pixels[idx + 1] = tG;
                        pixels[idx + 2] = tB;
                    }
                }
                else if (distSq <= rInnerSq)
                {
                    // 隊伍色圓形盾牌填充
                    pixels[idx] = tR;
                    pixels[idx + 1] = tG;
                    pixels[idx + 2] = tB;
                }
                else if (distSq <= rOuterSq)
                {
                    // 金色或純黑高對比輪廓線
                    pixels[idx] = 20;
                    pixels[idx + 1] = 20;
                    pixels[idx + 2] = 20;
                }
                else if (distSq <= (bannerRadius + 3.5f) * (bannerRadius + 3.5f) && dx >= 0 && dy >= 0)
                {
                    // 柔和陰影
                    pixels[idx] = (byte)(pixels[idx] * 0.55f);
                    pixels[idx + 1] = (byte)(pixels[idx + 1] * 0.55f);
                    pixels[idx + 2] = (byte)(pixels[idx + 2] * 0.55f);
                }
            }
        }
    }
}
