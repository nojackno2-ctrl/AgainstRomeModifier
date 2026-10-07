using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class TerrainHeightEditSessionTests
{
    [Fact]
    public void Height_brush_modifies_vertices_and_supports_undo_redo_and_baseline_reset()
    {
        byte[] heights = new byte[16]; // 4x4
        var session = new TerrainHeightEditSession(4, heights, null, 2, null);

        Assert.False(session.IsDirty);
        Assert.False(session.HeightsDirty);
        Assert.False(session.CanUndo);

        // Raise center
        IReadOnlyList<TerrainSampleChange> changes = session.PaintHeight(1.5f, 1.5f, 1.0f, TerrainHeightOperation.Raise, 20);
        Assert.NotEmpty(changes);
        Assert.True(session.HeightsDirty);
        Assert.True(session.IsDirty);
        Assert.True(session.CanUndo);

        session.CommitStroke();
        Assert.True(session.CanUndo);
        Assert.False(session.CanRedo);

        // Undo
        TerrainLayerStroke? undoStroke = session.Undo();
        Assert.NotNull(undoStroke);
        Assert.False(session.HeightsDirty);
        Assert.False(session.IsDirty);
        Assert.True(session.CanRedo);

        // Redo
        TerrainLayerStroke? redoStroke = session.Redo();
        Assert.NotNull(redoStroke);
        Assert.True(session.HeightsDirty);
        Assert.True(session.IsDirty);

        // ResetToBaseline
        session.ResetToBaseline();
        Assert.False(session.HeightsDirty);
        Assert.False(session.IsDirty);
        Assert.False(session.CanUndo);
        Assert.False(session.CanRedo);
    }

    [Fact]
    public void Collision_brush_tracks_dirty_and_undo_redo()
    {
        byte[] heights = new byte[16];
        byte[] collision = new byte[4]; // 2x2
        var session = new TerrainHeightEditSession(4, heights, null, 2, collision);

        Assert.True(session.HasCollision);
        Assert.False(session.CollisionDirty);

        IReadOnlyList<TerrainSampleChange> changes = session.PaintCollision(0.5f, 0.5f, 1.0f, TerrainCollisionOperation.Block);
        Assert.NotEmpty(changes);
        Assert.True(session.CollisionDirty);
        Assert.True(session.IsDirty);

        session.CommitStroke();
        session.Undo();
        Assert.False(session.CollisionDirty);
        Assert.Equal(0, session.Collision![0]);

        session.Redo();
        Assert.True(session.CollisionDirty);
        Assert.Equal(255, session.Collision![0]);

        session.CommitBaseline(null);
        Assert.False(session.CollisionDirty);
        Assert.False(session.IsDirty);
    }

    [Fact]
    public void Emboss_light_fit_computes_slopes_and_blank_terrain_triggers_relight()
    {
        int size = 8;
        byte[] heights = new byte[size * size];
        byte[] emboss = new byte[size * size];
        for (int y = 0; y < size; y++)
        {
            for (int x = 0; x < size; x++)
            {
                heights[y * size + x] = (byte)(x * 10);
                emboss[y * size + x] = (byte)(128 + x * 5);
            }
        }

        var session = new TerrainHeightEditSession(size, heights, emboss, 4, null);
        Assert.True(session.HasEmboss);
        Assert.False(session.EmbossRelightPending);

        session.ApplyBlankTerrain(50);
        Assert.True(session.EmbossRelightPending);
        Assert.True(session.IsDirty);

        byte[]? relit = session.BuildEmboss();
        Assert.NotNull(relit);
        Assert.Equal(size * size, relit.Length);

        session.CommitBaseline(relit);
        Assert.False(session.EmbossRelightPending);
        Assert.False(session.IsDirty);
    }

    [Fact]
    public void TerrainStrokePath_generates_correct_bresenham_line()
    {
        var points = TerrainStrokePath.Between(0, 0, 3, 3).ToArray();
        Assert.Equal(3, points.Length);
        Assert.Equal((1, 1), points[0]);
        Assert.Equal((2, 2), points[1]);
        Assert.Equal((3, 3), points[2]);
    }

    [Fact]
    public void TerrainEditHistory_tracks_textures_undo_redo_and_dirty()
    {
        string[] initial = ["4BA___50", "4BA___50", "4BA___50", "4BA___50"];
        var history = new TerrainEditHistory(2, initial);

        Assert.False(history.IsDirty);
        Assert.False(history.CanUndo);

        TerrainTextureChange? change = history.Paint(0, 0, "4BB___50");
        Assert.NotNull(change);
        Assert.Equal("4BA___50", change.Before);
        Assert.Equal("4BB___50", change.After);
        Assert.True(history.IsDirty);
        Assert.True(history.CanUndo);

        history.CommitStroke();
        IReadOnlyList<TerrainTextureChange>? undone = history.Undo();
        Assert.NotNull(undone);
        Assert.False(history.IsDirty);
        Assert.Equal("4BA___50", history.Current[0]);

        IReadOnlyList<TerrainTextureChange>? redone = history.Redo();
        Assert.NotNull(redone);
        Assert.True(history.IsDirty);
        Assert.Equal("4BB___50", history.Current[0]);

        history.CommitBaseline();
        Assert.False(history.IsDirty);
    }
}
