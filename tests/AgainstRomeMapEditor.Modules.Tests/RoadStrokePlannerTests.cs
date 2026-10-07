using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class RoadStrokePlannerTests
{
    private static RoadTile[] CompleteTiles() => Enumerable.Range(1, 15).Select(mask => new RoadTile("road" + mask, (RoadConnections)mask)).ToArray();
    private static string[] Blank(int dimension = 6) => Enumerable.Repeat("grass", dimension * dimension).ToArray();

    [Fact]
    public void Sparse_pointer_path_chooses_straights_turn_and_endpoints()
    {
        var result = RoadStrokePlanner.Plan(6, Blank(), [(1, 1), (4, 1), (4, 4)], CompleteTiles(), "road10");
        Assert.True(result.Succeeded);
        Assert.Equal(7, result.Tiles.Count);
        Assert.Contains(new RoadTilePlacement(2, 1, "road10"), result.Tiles);
        Assert.Contains(new RoadTilePlacement(4, 1, "road12"), result.Tiles);
        Assert.Contains(new RoadTilePlacement(4, 2, "road5"), result.Tiles);
        Assert.Contains(new RoadTilePlacement(1, 1, "road2"), result.Tiles);
        Assert.Contains(new RoadTilePlacement(4, 4, "road1"), result.Tiles);
    }

    [Fact]
    public void Crossing_existing_road_preserves_old_links_and_chooses_junction()
    {
        string[] textures = Blank();
        for (int x = 0; x < 6; x++) textures[2 * 6 + x] = "road10";
        var result = RoadStrokePlanner.Plan(6, textures, [(3, 0), (3, 5)], CompleteTiles(), "road5");
        Assert.True(result.Succeeded);
        Assert.Contains(new RoadTilePlacement(3, 2, "road15"), result.Tiles);
        Assert.Equal("road10", textures[2 * 6 + 3]); // planner never mutates input
    }

    [Fact]
    public void Missing_turn_rejects_entire_plan_instead_of_writing_partial_road()
    {
        RoadTile[] straights = [new("horizontal", RoadConnections.East | RoadConnections.West), new("vertical", RoadConnections.North | RoadConnections.South)];
        var result = RoadStrokePlanner.Plan(6, Blank(), [(1, 1), (4, 1), (4, 4)], straights, "horizontal");
        Assert.False(result.Succeeded);
        Assert.Empty(result.Tiles);
        Assert.Equal(new[] { (4, 1) }, result.Unsupported);
        Assert.True(RoadStrokePlanner.Plan(6, Blank(), [(1, 1), (4, 1)], straights, "horizontal").Succeeded);
    }

    [Theory]
    [InlineData(1, 1, 4, 4)]
    [InlineData(4, 4, 1, 1)]
    [InlineData(1, 4, 4, 1)]
    [InlineData(4, 1, 1, 4)]
    public void Diagonal_path_is_connected_by_edges(int x1, int y1, int x2, int y2)
    {
        var path = RoadStrokePlanner.OrthogonalBetween((x1, y1), (x2, y2)).Prepend((x1, y1)).ToArray();
        Assert.Equal((x2, y2), path[^1]);
        for (int i = 1; i < path.Length; i++) Assert.Equal(1, Math.Abs(path[i].Item1 - path[i - 1].Item1) + Math.Abs(path[i].Item2 - path[i - 1].Item2));
        Assert.True(RoadStrokePlanner.Plan(6, Blank(), [(x1, y1), (x2, y2)], CompleteTiles(), "road10").Succeeded);
    }

    [Fact]
    public void Revisited_tiles_combine_links_without_duplicate_placements()
    {
        var result = RoadStrokePlanner.Plan(6, Blank(), [(1, 2), (4, 2), (2, 2), (2, 4)], CompleteTiles(), "road10");
        Assert.True(result.Succeeded);
        Assert.Contains(new RoadTilePlacement(2, 2, "road14"), result.Tiles);
        Assert.Equal(result.Tiles.Count, result.Tiles.Select(t => (t.X, t.Y)).Distinct().Count());
    }

    [Fact]
    public void Out_of_bounds_path_rejects_before_planning_and_empty_path_is_no_op()
    {
        var result = RoadStrokePlanner.Plan(6, Blank(), [(1, 1), (6, 1)], CompleteTiles(), "road10");
        Assert.False(result.Succeeded);
        Assert.Empty(result.Tiles);
        Assert.Equal(new[] { (6, 1) }, result.Unsupported);
        Assert.Empty(RoadStrokePlanner.Plan(6, Blank(), [], CompleteTiles(), "road10").Tiles);
    }

    [Fact]
    public void Road_session_rejection_restores_pending_stroke_and_keeps_committed_history()
    {
        var source = Blank();
        var resolver = new Resolver();
        var session = new TerrainBlendEditSession(TerrainBlendAuthoringMap.Import(6, source, resolver, "grass"), source, resolver);
        RoadTile[] straights = [new("horizontal", RoadConnections.East | RoadConnections.West), new("vertical", RoadConnections.North | RoadConnections.South)];
        Assert.True(session.PaintRoadPath([(0, 0), (3, 0)], straights, "horizontal").Succeeded);
        Assert.True(session.CommitStroke());
        string[] baseline = session.CurrentTextures.ToArray();
        Assert.True(session.PaintRoadPath([(1, 3), (4, 3)], straights, "horizontal").Succeeded);
        var rejected = session.PaintRoadPath([(1, 3), (4, 3), (4, 5)], straights, "horizontal");
        Assert.False(rejected.Succeeded);
        Assert.Equal(4, rejected.TextureChanges.Count);
        Assert.Equal(baseline, session.CurrentTextures);
        Assert.False(session.CommitStroke());
        session.Undo();
        Assert.Equal(source, session.CurrentTextures);
        session.Redo();
        Assert.Equal(baseline, session.CurrentTextures);
        Assert.True(session.PaintRoadPath([(1, 3), (4, 3), (4, 5)], CompleteTiles(), "road10").Succeeded);
        Assert.True(session.CommitStroke());
        string[] after = session.CurrentTextures.ToArray();
        session.Undo(); Assert.Equal(baseline, session.CurrentTextures);
        session.Redo(); Assert.Equal(after, session.CurrentTextures);
        session.ResetToBaseline(); Assert.False(session.IsDirty); Assert.False(session.CanUndo);
    }

    private sealed class Resolver : INativeTerrainMaterialResolver
    {
        public bool TryResolveNativeCorners(string texture, out IReadOnlyList<string> corners)
        {
            corners = Enumerable.Repeat("grass", 4).ToArray();
            return texture == "grass";
        }
        public string? ResolveNativeTile(IReadOnlyList<string> cornerMaterialIds, int x, int y) => "grass";
    }
}
