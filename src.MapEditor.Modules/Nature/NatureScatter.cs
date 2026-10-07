namespace AgainstRomeMapEditor.Modules.Nature;

/// <summary>自然物件筆刷的散佈密度。</summary>
public enum NatureDensity { Sparse, Normal, Dense }

/// <summary>
/// 自然物件散佈的純計算：在圓形筆刷內逐格依密度機率取點、格內隨機偏移，並與既有物件保持最小間距，
/// 讓大筆刷種出疏密自然的樹林而不是整齊格點；同一位置反覆塗抹不會疊在一起。
/// </summary>
public static class NatureScatter
{
    /// <summary>密度對應每格種植機率；單格筆刷一律種一株（與舊行為相同）。</summary>
    public static double Probability(NatureDensity density) => density switch
    {
        NatureDensity.Sparse => .15,
        NatureDensity.Dense => .7,
        _ => .35,
    };

    /// <summary>最小間距（tile），密度越高越近。</summary>
    public static float MinSpacingTiles(NatureDensity density) => density switch
    {
        NatureDensity.Sparse => .9f,
        NatureDensity.Dense => .45f,
        _ => .65f,
    };

    /// <summary>
    /// 回傳新的種植位置（tile 座標，可含小數）。<paramref name="existing"/> 為已存在物件的 tile 座標；
    /// 大於一格的筆刷結果彼此之間與既有物件之間都至少相距 <see cref="MinSpacingTiles"/>；全部落在地圖 [0, dimension) 內。
    /// </summary>
    public static IReadOnlyList<(float X, float Y)> Plan(int centerTileX, int centerTileY, int brushSize, NatureDensity density,
        int dimension, IEnumerable<(float X, float Y)> existing, Random random)
    {
        ArgumentNullException.ThrowIfNull(existing);
        ArgumentNullException.ThrowIfNull(random);
        float spacing = MinSpacingTiles(density);
        var occupied = existing.ToList();
        var planted = new List<(float X, float Y)>();
        if (brushSize <= 1)
        {
            // 單格是使用者逐格指定：每格一定種一株（與舊行為相同，不做間距檢查），格內 [0.2, 0.8) 偏移。
            var point = (centerTileX + .2f + (float)random.NextDouble() * .6f, centerTileY + .2f + (float)random.NextDouble() * .6f);
            if (centerTileX >= 0 && centerTileY >= 0 && centerTileX < dimension && centerTileY < dimension) planted.Add(point);
            return planted;
        }
        float radius = brushSize / 2f + .26f;
        double probability = Probability(density);
        int reach = (int)MathF.Ceiling(radius);
        for (int y = centerTileY - reach; y <= centerTileY + reach; y++)
        for (int x = centerTileX - reach; x <= centerTileX + reach; x++)
        {
            if (x < 0 || y < 0 || x >= dimension || y >= dimension) continue;
            if ((x - centerTileX) * (x - centerTileX) + (y - centerTileY) * (y - centerTileY) > radius * radius) continue;
            if (random.NextDouble() >= probability) continue;
            var point = (x + (float)random.NextDouble(), y + (float)random.NextDouble());
            point = (Math.Min(point.Item1, dimension - .01f), Math.Min(point.Item2, dimension - .01f));
            if (!Free(point, occupied, spacing)) continue;
            planted.Add(point);
            occupied.Add(point);
        }
        return planted;
    }

    private static bool Free((float X, float Y) point, List<(float X, float Y)> occupied, float spacing)
        => occupied.All(other => (other.X - point.X) * (other.X - point.X) + (other.Y - point.Y) * (other.Y - point.Y) >= spacing * spacing);
}
