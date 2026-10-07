using AgainstRomeMapEditor.Modules.Nature;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class NatureScatterTests
{
    private static float Distance((float X, float Y) a, (float X, float Y) b) => MathF.Sqrt((a.X - b.X) * (a.X - b.X) + (a.Y - b.Y) * (a.Y - b.Y));

    [Fact]
    public void Single_tile_brush_plants_one_object_inside_the_tile()
    {
        var points = NatureScatter.Plan(10, 20, 1, NatureDensity.Normal, 64, [], new Random(1));
        var point = Assert.Single(points);
        Assert.InRange(point.X, 10.2f, 10.8f);
        Assert.InRange(point.Y, 20.2f, 20.8f);
    }

    [Theory]
    [InlineData(NatureDensity.Sparse)]
    [InlineData(NatureDensity.Normal)]
    [InlineData(NatureDensity.Dense)]
    public void Large_brush_scatters_within_radius_and_map_and_keeps_spacing(NatureDensity density)
    {
        var points = NatureScatter.Plan(2, 30, 15, density, 64, [], new Random(7));
        Assert.NotEmpty(points);
        float radius = 15 / 2f + .26f + 1.5f; // 中心格到點的最遠距離（格內偏移）
        Assert.All(points, point =>
        {
            Assert.InRange(point.X, 0f, 64f); Assert.InRange(point.Y, 0f, 64f);
            Assert.True(Distance(point, (2.5f, 30.5f)) <= radius);
        });
        float spacing = NatureScatter.MinSpacingTiles(density);
        for (int i = 0; i < points.Count; i++)
            for (int j = i + 1; j < points.Count; j++)
                Assert.True(Distance(points[i], points[j]) >= spacing - 1e-4f);
    }

    [Fact]
    public void Density_orders_counts_and_repeated_dabs_do_not_stack()
    {
        int Count(NatureDensity density) => Enumerable.Range(0, 20).Sum(seed => NatureScatter.Plan(32, 32, 9, density, 64, [], new Random(seed)).Count);
        Assert.True(Count(NatureDensity.Sparse) < Count(NatureDensity.Normal));
        Assert.True(Count(NatureDensity.Normal) < Count(NatureDensity.Dense));

        var random = new Random(3);
        var first = NatureScatter.Plan(32, 32, 9, NatureDensity.Dense, 64, [], random);
        var second = NatureScatter.Plan(32, 32, 9, NatureDensity.Dense, 64, first, random);
        float spacing = NatureScatter.MinSpacingTiles(NatureDensity.Dense);
        Assert.All(second, point => Assert.All(first, other => Assert.True(Distance(point, other) >= spacing - 1e-4f)));
        Assert.True(second.Count < first.Count, "同一位置再塗一次應只補空隙");
    }
}
