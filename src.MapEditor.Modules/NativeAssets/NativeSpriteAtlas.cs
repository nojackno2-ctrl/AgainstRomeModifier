namespace AgainstRomeMapEditor.NativeAssets;

/// <summary>Normalized texture rectangle of a sprite inside an atlas.</summary>
internal readonly record struct NativeSpriteUv(float U0, float V0, float U1, float V1);

/// <summary>
/// Shelf-packs still sprites into one ARGB texture so a scene draws in a single call.
/// Each sprite keeps a one-pixel transparent gutter against filtering bleed. Sprites that
/// do not fit are left out (callers fall back to markers) instead of failing the scene.
/// </summary>
internal sealed class NativeSpriteAtlas
{
    private const int Gutter = 1;
    private readonly Dictionary<NativeSprite, NativeSpriteUv> _uv;

    private NativeSpriteAtlas(int width, int height, uint[] pixels, Dictionary<NativeSprite, NativeSpriteUv> uv)
    {
        Width = width; Height = height; ArgbPixels = pixels; _uv = uv;
    }

    public int Width { get; }
    public int Height { get; }
    public uint[] ArgbPixels { get; }
    public int Count => _uv.Count;

    public bool TryGetUv(NativeSprite sprite, out NativeSpriteUv uv) => _uv.TryGetValue(sprite, out uv);

    public static NativeSpriteAtlas Pack(IEnumerable<NativeSprite> sprites, int maxSize = 4096)
    {
        ArgumentOutOfRangeException.ThrowIfLessThan(maxSize, 1);
        NativeSprite[] ordered = sprites.Distinct(ReferenceEqualityComparer.Instance).Cast<NativeSprite>()
            .OrderByDescending(sprite => sprite.Height).ThenByDescending(sprite => sprite.Width).ToArray();
        var placed = new List<(NativeSprite Sprite, int X, int Y)>();
        int x = 0, y = 0, shelf = 0, usedWidth = 1, usedHeight = 1;
        foreach (NativeSprite sprite in ordered)
        {
            int w = sprite.Width + Gutter * 2, h = sprite.Height + Gutter * 2;
            if (w > maxSize || h > maxSize) continue;
            int nextX = x, nextY = y, nextShelf = shelf;
            if (nextX + w > maxSize) { nextX = 0; nextY += nextShelf; nextShelf = 0; }
            if (nextY + h > maxSize) continue; // the layout is unchanged, so a later, narrower sprite may still fit
            placed.Add((sprite, nextX + Gutter, nextY + Gutter));
            x = nextX + w; y = nextY; shelf = Math.Max(nextShelf, h);
            usedWidth = Math.Max(usedWidth, x); usedHeight = Math.Max(usedHeight, y + shelf);
        }
        int width = NextPowerOfTwo(usedWidth), height = NextPowerOfTwo(usedHeight);
        var pixels = new uint[width * height];
        var uv = new Dictionary<NativeSprite, NativeSpriteUv>(ReferenceEqualityComparer.Instance);
        foreach ((NativeSprite sprite, int left, int top) in placed)
        {
            for (int row = 0; row < sprite.Height; row++)
                Array.Copy(sprite.ArgbPixels, row * sprite.Width, pixels, (top + row) * width + left, sprite.Width);
            uv[sprite] = new NativeSpriteUv(left / (float)width, top / (float)height,
                (left + sprite.Width) / (float)width, (top + sprite.Height) / (float)height);
        }
        return new NativeSpriteAtlas(width, height, pixels, uv);
    }

    private static int NextPowerOfTwo(int value)
    {
        int result = 1;
        while (result < value) result <<= 1;
        return result;
    }
}
