namespace AgainstRomeMapEditor.Modules.Nature;

/// <summary>
/// 自然生態圈專用多八度雜訊產生器。
/// 採用無分配（Allocation-Free）、確定性整數雜湊的 2D 梯度雜訊（Gradient / Improved Perlin Noise）。
/// 保證在相同種子碼與座標下產生完全一致的自然連續分佈，無週期邊界效應。
/// </summary>
public static class EcologyNoise
{
    /// <summary>
    /// 計算二維梯度雜訊，輸出範圍正規化至 [0.0, 1.0]。
    /// </summary>
    public static float Perlin2D(float x, float y, int seed)
    {
        int x0 = (int)MathF.Floor(x);
        int y0 = (int)MathF.Floor(y);
        int x1 = x0 + 1;
        int y1 = y0 + 1;

        float dx = x - x0;
        float dy = y - y0;

        // 五次平滑插值曲線（Quintic Fade Curve: 6t^5 - 15t^4 + 10t^3）
        float u = dx * dx * dx * (dx * (dx * 6f - 15f) + 10f);
        float v = dy * dy * dy * (dy * (dy * 6f - 15f) + 10f);

        // 四個網格角點的梯度點積
        float g00 = DotGrad(Hash(x0, y0, seed), dx, dy);
        float g10 = DotGrad(Hash(x1, y0, seed), dx - 1f, dy);
        float g01 = DotGrad(Hash(x0, y1, seed), dx, dy - 1f);
        float g11 = DotGrad(Hash(x1, y1, seed), dx - 1f, dy - 1f);

        // 雙線性平滑插值
        float nx0 = Lerp(g00, g10, u);
        float nx1 = Lerp(g01, g11, u);
        float value = Lerp(nx0, nx1, v);

        // 將 [-1.0, 1.0] 映射至 [0.0, 1.0]
        return Math.Clamp((value + 1f) * 0.5f, 0f, 1f);
    }

    /// <summary>
    /// 多八度分形雜訊（Fractal Brownian Motion / Octave Perlin）。
    /// 疊加不同頻率與振幅的雜訊層，模擬宏觀地景與微觀林冠的多尺度自然多樣性。
    /// </summary>
    /// <param name="x">X 軸取樣座標</param>
    /// <param name="y">Y 軸取樣座標</param>
    /// <param name="octaves">八度層數（通常 2~4 層）</param>
    /// <param name="persistence">振幅衰減係數（通常 0.5）</param>
    /// <param name="lacunarity">頻率倍增係數（通常 2.0）</param>
    /// <param name="seed">隨機種子</param>
    /// <returns>正規化至 [0.0, 1.0] 的多八度雜訊值</returns>
    public static float OctaveNoise2D(float x, float y, int octaves, float persistence, float lacunarity, int seed)
    {
        if (octaves <= 1) return Perlin2D(x, y, seed);

        float total = 0f;
        float frequency = 1f;
        float amplitude = 1f;
        float maxAmplitude = 0f;

        for (int i = 0; i < octaves; i++)
        {
            total += Perlin2D(x * frequency, y * frequency, seed + i * 1013) * amplitude;
            maxAmplitude += amplitude;
            amplitude *= persistence;
            frequency *= lacunarity;
        }

        return maxAmplitude > 0f ? Math.Clamp(total / maxAmplitude, 0f, 1f) : 0f;
    }

    private static float Lerp(float a, float b, float t) => a + t * (b - a);

    private static int Hash(int x, int y, int seed)
    {
        uint h = (uint)(seed ^ (x * 374761393) ^ (y * 668265263));
        h = (h ^ (h >> 13)) * 1274126177u;
        return (int)(h ^ (h >> 16));
    }

    private static float DotGrad(int hash, float x, float y)
    {
        return (hash & 7) switch
        {
            0 =>  x + y,
            1 => -x + y,
            2 =>  x - y,
            3 => -x - y,
            4 =>  x * 1.2f,
            5 => -x * 1.2f,
            6 =>  y * 1.2f,
            _ => -y * 1.2f,
        };
    }
}
