namespace AgainstRomeMapEditor;

internal sealed class TerrainHeightField
{
    private readonly byte[] _samples;

    public TerrainHeightField(int width, int height, ReadOnlySpan<byte> samples, float heightScale = 6f, float? tileWidth = null, float? tileHeight = null)
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
        _samples = samples.ToArray();
    }

    public int Width { get; }
    public int Height { get; }
    public float HeightScale { get; set; }
    public float TileWidth { get; }
    public float TileHeight { get; }

    public float SampleHeight(float tileX, float tileY)
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
        return (topValue + (bottomValue - topValue) * ty) / 255f * HeightScale;
    }

    private byte Sample(int x, int y) => _samples[y * Width + x];
}
