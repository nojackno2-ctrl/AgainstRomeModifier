using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class TerrainWaterPassabilityTests
{
    [Theory]
    [InlineData(193, 192)]
    [InlineData(257, 256)]
    [InlineData(9, 4)]
    public void Water_blocks_wet_pixels_and_preserves_dry_shore_and_existing_blocks(int vertices, int pixels)
    {
        byte[] heights = Enumerable.Repeat((byte)80, vertices * vertices).ToArray();
        byte[] collision = new byte[pixels * pixels];
        int middle = pixels / 2;
        collision[middle * pixels + middle] = 128;
        var session = new TerrainHeightEditSession(vertices, heights, null, pixels, collision);
        float center = (vertices - 1) / 2f;
        TerrainLayerStroke result = session.PaintWater(center, center, (vertices - 1) / 2f, 64, 10, 50);
        Assert.NotEmpty(result.Heights);
        Assert.NotEmpty(result.Collision);
        Assert.Equal(128, session.Collision![middle * pixels + middle]);
        Assert.Equal(0, session.Collision[0]);
        Assert.All(result.Collision, change => { Assert.Equal(0, change.Before); Assert.Equal(255, change.After); });
        Assert.True(session.CommitStroke());
        byte[] wetHeight = session.Heights.ToArray(), wetCollision = session.Collision.ToArray();
        session.Undo();
        Assert.Equal(heights, session.Heights);
        Assert.Equal(collision, session.Collision);
        session.Redo();
        Assert.Equal(wetHeight, session.Heights);
        Assert.Equal(wetCollision, session.Collision);
        session.ResetToBaseline();
        Assert.False(session.IsDirty);
    }

    [Fact]
    public void Already_carved_water_repairs_passability_without_height_changes_and_cancels_together()
    {
        var session = new TerrainHeightEditSession(5, new byte[25], null, 4, new byte[16]);
        TerrainLayerStroke result = session.PaintWater(2, 2, 1, 14, 0, 30);
        Assert.Empty(result.Heights);
        Assert.Equal(4, result.Collision.Count);
        Assert.True(session.CollisionDirty);
        session.CancelStroke();
        Assert.False(session.IsDirty);
        Assert.False(session.CanUndo);
    }

    [Fact]
    public void Water_threshold_uses_pixel_center_interpolation_and_leaves_surface_equal_pixels_passable()
    {
        var session = new TerrainHeightEditSession(2, new byte[] { 0, 40, 0, 40 }, null, 1, new byte[1]);
        Assert.Empty(session.PaintWater(.5f, .5f, .1f, 1, 0, 20).Collision);
        Assert.Single(session.PaintWater(.5f, .5f, .1f, 1, 0, 20.1f).Collision);
        Assert.Equal(255, session.Collision![0]);
    }

    [Fact]
    public void Missing_collision_does_not_invent_a_layer()
    {
        var session = new TerrainHeightEditSession(5, Enumerable.Repeat((byte)80, 25).ToArray(), null, 0, null);
        TerrainLayerStroke result = session.PaintWater(2, 2, 2, 64, 10, 30);
        Assert.NotEmpty(result.Heights);
        Assert.Empty(result.Collision);
        Assert.False(session.HasCollision);
    }

    [Theory]
    [InlineData(float.NaN)]
    [InlineData(float.PositiveInfinity)]
    [InlineData(0)]
    [InlineData(10)]
    public void Invalid_surface_rejects_before_mutation(float surface)
    {
        var session = new TerrainHeightEditSession(5, Enumerable.Repeat((byte)80, 25).ToArray(), null, 4, new byte[16]);
        Assert.Throws<ArgumentOutOfRangeException>(() => session.PaintWater(2, 2, 2, 64, 10, surface));
        Assert.False(session.IsDirty);
        Assert.False(session.CanUndo);
    }
}
