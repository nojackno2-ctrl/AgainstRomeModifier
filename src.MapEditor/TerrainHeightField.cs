namespace AgainstRomeMapEditor;

internal sealed class TerrainHeightField
{
    private readonly byte[] _samples;

    public TerrainHeightField(int width, int height, ReadOnlySpan<byte> samples, float heightScale = 6f, float? tileWidth = null, float? tileHeight = null,
        float nativeHeightMapStep = float.NaN)
    {
        if (width < 2 || height < 2 || samples.Length != width * height)
            throw new ArgumentException("Height samples must describe a grid of at least 2 by 2 pixels.", nameof(samples));
        if (tileWidth is <= 0 || tileHeight is <= 0)
            throw new ArgumentOutOfRangeException(nameof(tileWidth), "Terrain tile dimensions must be positive.");
        Width = width;
        Height = height;
        TileWidth = tileWidth ?? width - 1;
        TileHeight = tileHeight ?? height - 1;
        HeightScale = heightScale;
        NativeHeightMapStep = float.IsFinite(nativeHeightMapStep) && nativeHeightMapStep > 0 ? nativeHeightMapStep : float.NaN;
        _samples = samples.ToArray();
    }

    public int Width { get; }
    public int Height { get; }
    public float HeightScale { get; set; }
    // Immutable: SceneSpriteGeometryBuffer keys on height-field identity and HeightScale.
    // A changed step must create a new field, even when the samples are unchanged.
    public float NativeHeightMapStep { get; }
    public float TileWidth { get; }
    public float TileHeight { get; }

    /// <summary>Editor terrain/water preview convention, not native FPU or pixel equivalence.</summary>
    public bool TryNativeHeight(float worldY, out float height)
    {
        height = 0;
        if (!float.IsFinite(worldY) || !float.IsFinite(NativeHeightMapStep) || !float.IsFinite(HeightScale)) return false;
        double scaled = (double)worldY / NativeHeightMapStep / 255d * HeightScale;
        if (!double.IsFinite(scaled) || scaled < -float.MaxValue || scaled > float.MaxValue) return false;
        height = (float)scaled;
        return true;
    }

    public float SampleHeight(float tileX, float tileY) => SampleRaw(tileX, tileY) / 255f * HeightScale;

    /// <summary>Bilinear raw byte value (0..255 scale) at a tile position; independent of <see cref="HeightScale"/> (relief).</summary>
    public float SampleRaw(float tileX, float tileY)
    {
        float x = Math.Clamp(tileX, 0, TileWidth) * (Width - 1) / TileWidth;
        float y = Math.Clamp(tileY, 0, TileHeight) * (Height - 1) / TileHeight;
        int left = Math.Min((int)MathF.Floor(x), Width - 2);
        int top = Math.Min((int)MathF.Floor(y), Height - 2);
        float tx = x - left;
        float ty = y - top;
        float a = Sample(left, top);
        float b = Sample(left + 1, top);
        float c = Sample(left, top + 1);
        float d = Sample(left + 1, top + 1);
        float topValue = a + (b - a) * tx;
        float bottomValue = c + (d - c) * tx;
        return topValue + (bottomValue - topValue) * ty;
    }

    /// <summary>The native step is finite and every raw sample * step fits a float.</summary>
    public bool HasNativeQuery => float.IsFinite(NativeHeightMapStep) && Math.Abs(255d * NativeHeightMapStep) <= float.MaxValue;

    public float MinimumNativeHeight => HasNativeQuery
        ? (float)((double)_samples.Min() * NativeHeightMapStep) : float.NaN;

    /// <summary>
    /// Native-unit terrain Y (raw sample * <see cref="NativeHeightMapStep"/>) at a tile position, independent of relief.
    /// False when the step is unknown: callers must not substitute the relief-scaled preview height.
    /// </summary>
    public bool TryNativeQueryHeight(float tileX, float tileY, out float nativeY)
    {
        nativeY = 0;
        if (!HasNativeQuery) return false;
        nativeY = (float)((double)SampleRaw(tileX, tileY) * NativeHeightMapStep);
        return true;
    }

    /// <summary>Raw samples * step as float texels (row-major, Width x Height) for the GPU native-height texture; null when the step is unknown.</summary>
    public float[]? BuildNativeHeightTexels()
    {
        if (!HasNativeQuery) return null;
        var texels = new float[_samples.Length];
        for (int i = 0; i < texels.Length; i++) texels[i] = (float)((double)_samples[i] * NativeHeightMapStep);
        return texels;
    }

    private byte Sample(int x, int y) => _samples[y * Width + x];
}
