using System.Numerics;

namespace AgainstRomeMapEditor.NativeAssets;

/// <summary>daynight.bmp 原始色表及已證實的部分公式；不是完整原生 renderer。</summary>
internal sealed class MapLightingDayNight
{
    private readonly MapLightingBitmap _bitmap;
    private MapLightingDayNight(MapLightingBitmap bitmap) => _bitmap = bitmap;
    public static MapLightingDayNight Parse(ReadOnlySpan<byte> bytes)
    {
        var bitmap = MapLightingBitmap.Parse(bytes);
        if (bitmap.Width != 24 || bitmap.Height != 6) throw new NotSupportedException("Expected a 24x6 daynight table.");
        return new MapLightingDayNight(bitmap);
    }
    public Vector3 RawColor(int hour, int row) => _bitmap.ColorAt(hour, row);

    /// <summary>0x49A350：16.16 權重各自 floor，色道 >>16 再除以 256；只讀 row0。</summary>
    public Vector3 AmbientAt(int hour, int minute)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(hour);
        ArgumentOutOfRangeException.ThrowIfGreaterThanOrEqual(hour, 24);
        ArgumentOutOfRangeException.ThrowIfNegative(minute);
        ArgumentOutOfRangeException.ThrowIfGreaterThanOrEqual(minute, 60);
        int nextWeight = minute * 65536 / 60, currentWeight = (60 - minute) * 65536 / 60;
        Vector3 current = RawColor(hour, 0), next = RawColor((hour + 1) % 24, 0);
        return new Vector3(
            ((int)current.X * currentWeight + (int)next.X * nextWeight) >> 16,
            ((int)current.Y * currentWeight + (int)next.Y * nextWeight) >> 16,
            ((int)current.Z * currentWeight + (int)next.Z * nextWeight) >> 16) / 256;
    }
}

internal static class MapLightingModel
{
    /// <summary>0x49FFE0：每色道 max(ambient, lightColor*(1-distance²/radius²))，最後上限 1。</summary>
    public static Vector3 ApplyLocalLight(Vector3 ambient, Vector3 lightColor, float distanceSquared, float radiusSquared)
    {
        if (!float.IsFinite(distanceSquared) || distanceSquared < 0) throw new ArgumentOutOfRangeException(nameof(distanceSquared));
        if (!float.IsFinite(radiusSquared) || radiusSquared <= 0) throw new ArgumentOutOfRangeException(nameof(radiusSquared));
        if (distanceSquared >= radiusSquared) return Vector3.Min(ambient, Vector3.One);
        return Vector3.Min(Vector3.Max(ambient, lightColor * (1 - distanceSquared / radiusSquared)), Vector3.One);
    }

    /// <summary>0x49A490 的末段；caller 提供 cloud*terrainShadow 及額外 byte 取樣值。
    /// 額外 byte 的產品語意未完全追查，不能自行當成陰影或 sky density。
    /// 回傳 normalized gain；native 回傳值是此值乘 256。
    /// </summary>
    public static Vector3 SpriteGain(Vector3 locallyLit, float cloudAndTerrainShadow, byte visibilitySample)
    {
        if (!float.IsFinite(cloudAndTerrainShadow) || cloudAndTerrainShadow is < 0 or > 1)
            throw new ArgumentOutOfRangeException(nameof(cloudAndTerrainShadow));
        const float nativeScale = 0.0039062025025486946f; // 0x3B7FFF34，native 略低於 1/256。
        float visibility = visibilitySample * nativeScale;
        return locallyLit * ((cloudAndTerrainShadow * 0.707f + 0.293f) * visibility * visibility);
    }
}
