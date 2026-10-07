using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class TerrainRoughenTests
{
    private const int Size = 65;

    private static TerrainHeightEditSession Flat(byte level = 120) =>
        new(Size, Enumerable.Repeat(level, Size * Size).ToArray(), null, 0, null);

    [Fact]
    public void Roughen_raises_and_lowers_within_the_brush_and_is_undoable()
    {
        var session = Flat();
        var changes = session.PaintHeight(32, 32, 12, TerrainHeightOperation.Roughen, 14, roughnessSeed: 7);
        Assert.Contains(changes, change => change.After > change.Before);
        Assert.Contains(changes, change => change.After < change.Before);
        Assert.All(changes, change =>
        {
            int x = change.Index % Size, y = change.Index / Size;
            Assert.True((x - 32) * (x - 32) + (y - 32) * (y - 32) <= 12 * 12, $"({x},{y}) 在筆刷外");
            Assert.InRange(change.After - change.Before, -14, 14);
        });
        session.CommitStroke();
        Assert.NotNull(session.Undo());
        Assert.All(session.Heights, height => Assert.Equal(120, height));
    }

    [Fact]
    public void Roughen_is_deterministic_per_seed_and_varies_between_seeds()
    {
        byte[] Paint(int seed)
        {
            var session = Flat();
            session.PaintHeight(32, 32, 16, TerrainHeightOperation.Roughen, 14, roughnessSeed: seed);
            return session.Heights.ToArray();
        }
        Assert.Equal(Paint(3), Paint(3));
        Assert.NotEqual(Paint(3), Paint(4));
    }

    [Fact]
    public void Roughness_noise_is_bounded_and_smooth_between_neighbouring_vertices()
    {
        for (int y = 0; y < 64; y++)
        for (int x = 0; x < 64; x++)
        {
            float value = TerrainHeightEditSession.RoughnessNoise(x, y, 11);
            Assert.InRange(value, -1f, 1f);
            // 相鄰頂點差距受限：起伏是數頂點寬的丘陵，不是逐點尖刺。
            Assert.True(MathF.Abs(value - TerrainHeightEditSession.RoughnessNoise(x + 1, y, 11)) <= .75f);
            Assert.True(MathF.Abs(value - TerrainHeightEditSession.RoughnessNoise(x, y + 1, 11)) <= .75f);
        }
    }
}
