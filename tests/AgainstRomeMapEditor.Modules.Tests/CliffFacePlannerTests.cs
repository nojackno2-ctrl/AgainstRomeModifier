using AgainstRomeMapEditor;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class CliffFacePlannerTests
{
    [Fact]
    public void Plan_matches_cliff_stamps_and_generates_scree_on_downhill_side()
    {
        const int dimension = 8;
        var heights = new float[dimension * dimension];

        // y <= 2 為高台 (100)，y >= 3 為低地 (20)
        // y = 2 是峭壁脊線，Facing South，下坡方向為 y = 3
        for (int y = 0; y < dimension; y++)
        {
            for (int x = 0; x < dimension; x++)
            {
                heights[y * dimension + x] = y <= 2 ? 100f : 20f;
            }
        }

        var catalog = CliffTileCatalog.CreateDefault();
        CliffDetectionResult detection = CliffEdgeDetector.DetectFromTileHeights(dimension, heights);

        var options = new CliffPlannerOptions(
            PreferredFamily: "FELS",
            GenerateScree: true,
            ScreeMaterialId: "BK",
            ScreeRadius: 1,
            AutoMarkCollision: true);

        CliffPlanResult plan = CliffFacePlanner.Plan(dimension, detection, catalog, options);

        Assert.True(plan.Succeeded);
        Assert.NotEmpty(plan.CliffTiles);

        // 懸崖脊線上的單元應放置 FELS_S*
        var cliffRidge = plan.CliffTiles.Where(p => p.Y == 2).ToArray();
        Assert.Equal(dimension, cliffRidge.Length);
        Assert.All(cliffRidge, p => Assert.StartsWith("FELS_S", p.Texture, StringComparison.OrdinalIgnoreCase));

        // 碎石坡腳應位於下坡側 (y = 3 或 y = 4)，且絕不覆蓋懸崖本體
        Assert.NotEmpty(plan.ScreePlacements);
        Assert.All(plan.ScreePlacements, scree =>
        {
            Assert.False(detection.IsCliff(scree.X, scree.Y), "碎石帶不應與懸崖本體重疊。");
            Assert.Equal("BK", scree.MaterialId);
        });

        // 每個懸崖圖塊應生成 4x4 (16) 個碰撞阻擋像素
        int expectedCollisionPixels = plan.CliffTiles.Count * 16;
        Assert.Equal(expectedCollisionPixels, plan.BlockedCollisionPixels.Count);
    }

    [Fact]
    public void Plan_without_scree_or_collision_omits_optional_layers()
    {
        const int dimension = 6;
        var heights = new float[dimension * dimension];
        for (int y = 0; y < dimension; y++)
        for (int x = 0; x < dimension; x++)
            heights[y * dimension + x] = x < 3 ? 100f : 20f;

        var catalog = CliffTileCatalog.CreateDefault();
        CliffDetectionResult detection = CliffEdgeDetector.DetectFromTileHeights(dimension, heights);

        var options = new CliffPlannerOptions(
            GenerateScree: false,
            AutoMarkCollision: false);

        CliffPlanResult plan = CliffFacePlanner.Plan(dimension, detection, catalog, options);

        Assert.True(plan.Succeeded);
        Assert.NotEmpty(plan.CliffTiles);
        Assert.Empty(plan.ScreePlacements);
        Assert.Empty(plan.BlockedCollisionPixels);
    }

    [Fact]
    public void Plan_scree_radius_expands_apron()
    {
        const int dimension = 8;
        var heights = new float[dimension * dimension];
        for (int y = 0; y < dimension; y++)
        for (int x = 0; x < dimension; x++)
            heights[y * dimension + x] = y <= 2 ? 100f : 20f;

        var catalog = CliffTileCatalog.CreateDefault();
        CliffDetectionResult detection = CliffEdgeDetector.DetectFromTileHeights(dimension, heights);

        var options1 = new CliffPlannerOptions(ScreeRadius: 1);
        var options2 = new CliffPlannerOptions(ScreeRadius: 2);

        CliffPlanResult plan1 = CliffFacePlanner.Plan(dimension, detection, catalog, options1);
        CliffPlanResult plan2 = CliffFacePlanner.Plan(dimension, detection, catalog, options2);

        Assert.True(plan2.ScreePlacements.Count >= plan1.ScreePlacements.Count);
    }
}
