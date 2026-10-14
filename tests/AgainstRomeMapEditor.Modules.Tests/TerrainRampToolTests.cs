using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class TerrainRampToolTests
{
    [Fact]
    public void Interpolation_linear_heights_along_ramp_centerline()
    {
        // 16x16 flat heightmap, initial height = 100
        int size = 16;
        byte[] heights = Enumerable.Repeat((byte)100, size * size).ToArray();

        // Horizontal ramp from (2, 5) to (10, 5)
        // startHeight = 20, endHeight = 60
        var parameters = new TerrainRampParameters
        {
            Width = 1f,
            Falloff = 0f,
            StartHeight = 20f,
            EndHeight = 60f,
            RoundCaps = false
        };

        var changes = TerrainRampTool.CalculateRamp(heights, size, 2f, 5f, 10f, 5f, parameters);
        var changeMap = changes.ToDictionary(c => c.Index, c => c.After);

        // At start (x=2, y=5): target = 20
        int idxStart = 5 * size + 2;
        Assert.True(changeMap.ContainsKey(idxStart));
        Assert.Equal(20, changeMap[idxStart]);

        // At end (x=10, y=5): target = 60
        int idxEnd = 5 * size + 10;
        Assert.True(changeMap.ContainsKey(idxEnd));
        Assert.Equal(60, changeMap[idxEnd]);

        // At midpoint (x=6, y=5): t = 0.5 -> target = 20 + 0.5 * (60 - 20) = 40
        int idxMid = 5 * size + 6;
        Assert.True(changeMap.ContainsKey(idxMid));
        Assert.Equal(40, changeMap[idxMid]);

        // At quarter point (x=4, y=5): t = 0.25 -> target = 30
        int idxQuarter = 5 * size + 4;
        Assert.True(changeMap.ContainsKey(idxQuarter));
        Assert.Equal(30, changeMap[idxQuarter]);
    }

    [Fact]
    public void Interpolation_diagonal_samples_bilinear_when_endpoint_heights_omitted()
    {
        int size = 10;
        byte[] heights = new byte[size * size];

        // Set start point (1, 1) = 30, end point (7, 7) = 90
        heights[1 * size + 1] = 30;
        heights[7 * size + 7] = 90;

        var parameters = new TerrainRampParameters
        {
            Width = 1f,
            Falloff = 0f,
            RoundCaps = false
        };

        var changes = TerrainRampTool.CalculateRamp(heights, size, 1f, 1f, 7f, 7f, parameters);
        var changeMap = changes.ToDictionary(c => c.Index, c => c.After);

        // Midpoint (4, 4) should be halfway between 30 and 90 -> 60
        int idxMid = 4 * size + 4;
        Assert.True(changeMap.ContainsKey(idxMid));
        Assert.Equal(60, changeMap[idxMid]);
    }

    [Fact]
    public void Width_and_falloff_smoothly_blends_into_existing_terrain()
    {
        int size = 15;
        // Background height = 10
        byte[] heights = Enumerable.Repeat((byte)10, size * size).ToArray();

        // Horizontal ramp along y = 7 from x = 2 to x = 12, height = 100 constant
        // Core width = 2 (half-width = 1), falloff = 2
        var parameters = new TerrainRampParameters
        {
            Width = 2f,
            Falloff = 2f,
            StartHeight = 100f,
            EndHeight = 100f,
            FalloffType = TerrainFalloffType.Smoothstep,
            RoundCaps = false
        };

        var changes = TerrainRampTool.CalculateRamp(heights, size, 2f, 7f, 12f, 7f, parameters);
        var changeMap = changes.ToDictionary(c => c.Index, c => c.After);

        // Core line at y = 7: dist = 0 <= 1 -> weight = 1.0 -> height = 100
        Assert.Equal(100, changeMap[7 * size + 5]);

        // Adjacent line at y = 8: dist = 1.0 <= 1 -> still in core width -> height = 100
        Assert.Equal(100, changeMap[8 * size + 5]);

        // Line at y = 9: dist = 2.0. normFalloff = (2 - 1) / 2 = 0.5.
        // Smoothstep(0.5): falloff = 1 - 0.25 * (3 - 1) = 0.5.
        // Blended: 0.5 * 100 + 0.5 * 10 = 55.
        int idxFalloff = 9 * size + 5;
        Assert.True(changeMap.ContainsKey(idxFalloff));
        Assert.Equal(55, changeMap[idxFalloff]);

        // Beyond falloff at y = 11: dist = 4 > 3 -> should NOT be in changes (unchanged)
        int idxOutside = 11 * size + 5;
        Assert.False(changeMap.ContainsKey(idxOutside));
    }

    [Fact]
    public void Bounds_clipping_handles_out_of_bounds_and_real_193x193_dimensions()
    {
        // 193x193 map (real Against Rome TUTOR_01 size)
        int size = 193;
        byte[] heights = new byte[size * size];

        // Ramp starting outside bounds (-10, 50) and ending well inside (50, 50)
        var parameters = new TerrainRampParameters
        {
            Width = 4f,
            Falloff = 2f,
            StartHeight = 20f,
            EndHeight = 80f
        };

        var changes = TerrainRampTool.CalculateRamp(heights, size, -10f, 50f, 50f, 50f, parameters);

        Assert.NotEmpty(changes);
        // All modified vertex indices must strictly lie within [0, size * size - 1]
        foreach (var change in changes)
        {
            Assert.InRange(change.Index, 0, size * size - 1);
            int x = change.Index % size;
            int y = change.Index / size;
            Assert.InRange(x, 0, size - 1);
            Assert.InRange(y, 0, size - 1);
        }

        // Near right-bottom boundary (185, 185) to (210, 210)
        var edgeChanges = TerrainRampTool.CalculateRamp(heights, size, 185f, 185f, 210f, 210f, parameters);
        Assert.NotEmpty(edgeChanges);
        foreach (var change in edgeChanges)
        {
            Assert.InRange(change.Index, 0, size * size - 1);
        }
    }

    [Fact]
    public void Degenerate_endpoints_returns_empty_without_error()
    {
        byte[] heights = new byte[64];
        var changes = TerrainRampTool.CalculateRamp(heights, 8, 4f, 4f, 4f, 4f);
        Assert.Empty(changes);
    }

    [Fact]
    public void Undo_redo_applies_and_reverts_ramp_in_single_stroke()
    {
        int size = 16;
        byte[] heights = Enumerable.Repeat((byte)20, size * size).ToArray();
        var session = new TerrainHeightEditSession(size, heights, null, size, null);

        Assert.False(session.CanUndo);
        Assert.False(session.HeightsDirty);

        // Apply ramp from (2, 8) to (12, 8) with heights 50 to 100
        var parameters = new TerrainRampParameters
        {
            Width = 2f,
            Falloff = 1f,
            StartHeight = 50f,
            EndHeight = 100f
        };

        var changes = TerrainRampTool.ApplyRamp(session, 2f, 8f, 12f, 8f, parameters);

        Assert.NotEmpty(changes);
        Assert.True(session.HeightsDirty);
        Assert.True(session.CanUndo);
        Assert.False(session.CanRedo);

        // Verify center heights changed
        Assert.Equal(50, session.Heights[8 * size + 2]);
        Assert.Equal(100, session.Heights[8 * size + 12]);

        // Undo: should revert all changes in ONE step
        var undone = session.Undo();
        Assert.NotNull(undone);
        Assert.False(session.HeightsDirty);
        Assert.True(session.CanRedo);

        // Verify heights reverted to initial 20
        Assert.Equal(20, session.Heights[8 * size + 2]);
        Assert.Equal(20, session.Heights[8 * size + 12]);

        // Redo: should re-apply the ramp in ONE step
        var redone = session.Redo();
        Assert.NotNull(redone);
        Assert.True(session.HeightsDirty);
        Assert.Equal(50, session.Heights[8 * size + 2]);
        Assert.Equal(100, session.Heights[8 * size + 12]);
    }

    [Fact]
    public void TileToVertex_scales_correctly_for_different_map_dimensions()
    {
        // 193x193 heightmap on 64x64 tiles: step = 192 / 64 = 3.0
        (float vx, float vy) = TerrainRampTool.TileToVertex(10, 20, 64, 193);
        Assert.Equal(10.5f * 3f, vx);
        Assert.Equal(20.5f * 3f, vy);

        // 257x257 heightmap on 256x256 tiles: step = 256 / 256 = 1.0
        (float vx2, float vy2) = TerrainRampTool.TileToVertex(10, 20, 256, 257);
        Assert.Equal(10.5f, vx2);
        Assert.Equal(20.5f, vy2);
    }
}
