using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class TerrainRegionTests
{
    [Fact]
    public void Rectangle_reverses_corners_and_connected_fill_excludes_diagonal_and_other_materials()
    {
        Assert.Equal(new[] { (1, 1), (2, 1), (1, 2), (2, 2) }, TerrainRegionPlanner.Rectangle(3, (2, 2), (1, 1)));
        string[] materials = ["grass", "rock", "grass", "GRASS", "rock", "grass", "rock", "grass", "grass"];
        Assert.Equal(new[] { (0, 0), (0, 1) }, TerrainRegionPlanner.Connected(3, materials, (0, 0)));
        Assert.Throws<ArgumentOutOfRangeException>(() => TerrainRegionPlanner.Rectangle(3, (0, 0), (3, 2)));
    }

    [Fact]
    public void Polyline_has_connected_diagonal_steps_turns_and_clipped_width()
    {
        var cells = TerrainRegionPlanner.Polyline(8, [(0, 0), (4, 4), (4, 7)], 3);
        Assert.Contains((0, 0), cells); Assert.Contains((4, 7), cells);
        Assert.Equal(cells.Count, cells.Distinct().Count());
        Assert.All(cells, c => { Assert.InRange(c.X, 0, 7); Assert.InRange(c.Y, 0, 7); });
        string[] mask = Enumerable.Range(0, 64).Select(i => cells.Contains((i % 8, i / 8)) ? "road" : "grass").ToArray();
        Assert.Equal(cells.Count, TerrainRegionPlanner.Connected(8, mask, (0, 0)).Count);
    }

    [Fact]
    public void Region_bakes_as_one_stroke_and_failed_region_preserves_redo_and_pending_edits()
    {
        var resolver = new Resolver();
        string[] original = Enumerable.Repeat("grass/grass/grass/grass", 16).ToArray();
        var session = new TerrainBlendEditSession(TerrainBlendAuthoringMap.Import(4, original, resolver, "grass"), original, resolver);
        Assert.True(session.PaintTiles(TerrainRegionPlanner.Rectangle(4, (1, 1), (2, 2)), "road").Succeeded);
        session.CommitStroke(); string[] painted = session.CurrentTextures.ToArray();
        Assert.Equal("road/road/road/road", painted[5]);
        session.Undo(); Assert.Equal(original, session.CurrentTextures); Assert.False(session.CanUndo);
        Assert.False(session.PaintTiles([(0, 0)], "unsupported").Succeeded);
        Assert.Equal(original, session.CurrentTextures); Assert.True(session.CanRedo);
        session.Redo(); Assert.Equal(painted, session.CurrentTextures);
        Assert.True(session.PaintCircle(0, 0, 0, "sand").Succeeded);
        string[] pending = session.CurrentTextures.ToArray();
        Assert.False(session.PaintTiles([(0, 0), (3, 3)], "unsupported").Succeeded);
        Assert.Equal(pending, session.CurrentTextures);
        Assert.Throws<ArgumentOutOfRangeException>(() => session.PaintTiles([(0, 0), (4, 0)], "road"));
        Assert.Equal(pending, session.CurrentTextures);
    }

    private sealed class Resolver : INativeTerrainMaterialResolver
    {
        public bool TryResolveNativeCorners(string texture, out IReadOnlyList<string> corners) { corners = texture.Split('/'); return corners.Count == 4; }
        public string? ResolveNativeTile(IReadOnlyList<string> corners, int x, int y) => corners.Contains("unsupported") ? null : string.Join('/', corners);
    }
}
