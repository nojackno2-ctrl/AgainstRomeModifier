using AgainstRomeMapEditor.NativeAssets;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class NativeSpriteAtlasTests
{
    [Fact]
    public void Packs_distinct_sprites_with_gutters_and_exact_uv_rectangles()
    {
        NativeSprite tall = Sprite(3, 5, 0xFF000001), wide = Sprite(6, 2, 0xFF000002), same = Sprite(3, 5, 0xFF000001);
        var atlas = NativeSpriteAtlas.Pack([wide, tall, tall, same], maxSize: 16);
        Assert.Equal(3, atlas.Count); // reference identity: value-equal records stay separate entries
        Assert.Equal((16, 16), (atlas.Width, atlas.Height)); // 7px shelf of the 3x5 pair, then the 6x2 sprite
        foreach (NativeSprite sprite in new[] { tall, wide, same })
        {
            Assert.True(atlas.TryGetUv(sprite, out NativeSpriteUv uv));
            int left = (int)Math.Round(uv.U0 * atlas.Width), top = (int)Math.Round(uv.V0 * atlas.Height);
            Assert.Equal(sprite.Width, (int)Math.Round(uv.U1 * atlas.Width) - left);
            Assert.Equal(sprite.Height, (int)Math.Round(uv.V1 * atlas.Height) - top);
            for (int y = 0; y < sprite.Height; y++)
                for (int x = 0; x < sprite.Width; x++)
                    Assert.Equal(sprite.ArgbPixels[y * sprite.Width + x], atlas.ArgbPixels[(top + y) * atlas.Width + left + x]);
            Assert.Equal(0u, atlas.ArgbPixels[(top - 1) * atlas.Width + left]); // transparent gutter
        }
    }

    [Fact]
    public void Oversized_and_overflowing_sprites_are_omitted_instead_of_failing()
    {
        NativeSprite huge = Sprite(20, 2, 1), narrow = Sprite(2, 2, 9);
        NativeSprite[] blocks = Enumerable.Range(0, 5).Select(i => Sprite(6, 6, (uint)i + 2)).ToArray();
        var atlas = NativeSpriteAtlas.Pack([huge, .. blocks, narrow], maxSize: 16);
        Assert.False(atlas.TryGetUv(huge, out _));
        Assert.Equal(4, blocks.Count(sprite => atlas.TryGetUv(sprite, out _))); // two 8px cells per shelf, two shelves
        Assert.False(atlas.TryGetUv(narrow, out _));
        Assert.Equal((16, 16), (atlas.Width, atlas.Height));
        Assert.Throws<ArgumentOutOfRangeException>(() => NativeSpriteAtlas.Pack([], 0));
        Assert.Equal(0, NativeSpriteAtlas.Pack([]).Count);
    }

    private static NativeSprite Sprite(int width, int height, uint color)
        => new(width, height, Enumerable.Range(0, width * height).Select(i => color + (uint)i * 0x100).ToArray(), 0, 0, "test");
}
