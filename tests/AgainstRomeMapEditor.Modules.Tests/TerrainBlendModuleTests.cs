using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class TerrainBlendModuleTests
{
    [Fact]
    public void Protected_circle_preserves_adjacent_tiles_and_corners_through_bake_and_undo()
    {
        var resolver = new Resolver();
        string[] source = Enumerable.Repeat("grass/grass/grass/grass", 64).ToArray();
        var import = TerrainBlendAuthoringMap.Import(8, source, resolver, "grass");
        var session = new TerrainBlendEditSession(import, source, resolver);
        bool Allowed(int x, int y) => x >= 2 && y >= 2 && x < 7 && y < 7 && !(x == 4 && y == 4);
        var result = session.PaintCircle(4, 4, 8, "sand", allowsTile: Allowed);
        Assert.True(result.Succeeded); Assert.NotEmpty(result.TextureChanges);
        Assert.All(result.TextureChanges, change => Assert.True(Allowed(change.X, change.Y)));
        for (int y = 0; y < 8; y++) for (int x = 0; x < 8; x++)
            if (!Allowed(x, y)) Assert.Equal(source[y * 8 + x], session.CurrentTextures[y * 8 + x]);
        foreach (var corner in new[] { (4, 4), (5, 4), (4, 5), (5, 5), (2, 3), (7, 3) })
            Assert.Equal("grass", import.Map.GetCorner(corner.Item1, corner.Item2));
        var edited = session.CurrentTextures.ToArray(); session.CommitStroke(); session.Undo(); Assert.Equal(source, session.CurrentTextures);
        session.Redo(); Assert.Equal(edited, session.CurrentTextures);
        Assert.Throws<ArgumentException>(() => session.PaintCircle(4, 4, 8, "sand", autoBridge: true, allowsTile: Allowed));
    }
    [Fact]
    public void Rebinding_resource_resolver_keeps_saved_baseline_history_and_uses_new_resolver_for_future_strokes()
    {
        var resolver = new Resolver();
        string[] source = ["grass/grass/grass/grass"];
        var session = new TerrainBlendEditSession(TerrainBlendAuthoringMap.Import(1, source, resolver, "grass"), source, resolver);
        Assert.True(session.PaintCircle(0, 0, 0, "sand").Succeeded);
        Assert.True(session.CommitStroke());
        string[] first = session.CurrentTextures.ToArray();
        session.Undo();
        session.RebindMaterialResolver(new ReloadedResolver());
        Assert.False(session.IsDirty); Assert.True(session.CanRedo);
        session.Redo(); Assert.Equal(first, session.CurrentTextures);
        Assert.True(session.PaintCircle(1, 1, 0, "water").Succeeded);
        Assert.True(session.CommitStroke());
        string[] second = session.CurrentTextures.ToArray();
        Assert.Equal("new:sand/grass/water/grass", second[0]);
        session.Undo(); Assert.Equal(first, session.CurrentTextures);
        session.Undo(); Assert.Equal(source, session.CurrentTextures); Assert.False(session.IsDirty);
        session.Redo(); session.Redo(); Assert.Equal(second, session.CurrentTextures);
        session.ResetToBaseline(); Assert.Equal(source, session.CurrentTextures);
    }

    private sealed class ReloadedResolver : INativeTerrainMaterialResolver
    {
        public bool TryResolveNativeCorners(string texture, out IReadOnlyList<string> corners)
        {
            corners = texture.Replace("new:", "", StringComparison.Ordinal).Split('/');
            return corners.Count == 4;
        }
        public string? ResolveNativeTile(IReadOnlyList<string> cornerMaterialIds, int x, int y) => "new:" + string.Join('/', cornerMaterialIds);
    }

    [Fact]
    public void Rejected_independent_area_preserves_earlier_areas_in_one_undo_step()
    {
        var resolver = new Resolver();
        string[] source = ["grass/grass/grass/grass"];
        var import = TerrainBlendAuthoringMap.Import(1, source, resolver, "grass");
        var session = new TerrainBlendEditSession(import, source, resolver);
        Assert.True(session.PaintCircle(0, 0, 0, "sand", rollbackStrokeOnFailure: false).Succeeded);
        var rejected = session.PaintCircle(0.5f, 0.5f, 2, "unsupported", rollbackStrokeOnFailure: false);
        Assert.False(rejected.Succeeded);
        Assert.Empty(rejected.TextureChanges);
        Assert.Equal("sand/grass/grass/grass", session.CurrentTextures[0]);
        Assert.True(session.PaintCircle(1, 1, 0, "water", rollbackStrokeOnFailure: false).Succeeded);
        Assert.True(session.CommitStroke());
        Assert.Equal("sand/grass/water/grass", session.CurrentTextures[0]);
        session.Undo();
        Assert.False(session.IsDirty);
        Assert.False(session.CanUndo);
        session.Redo();
        Assert.Equal("sand/grass/water/grass", session.CurrentTextures[0]);
    }

    private sealed class Resolver : INativeTerrainMaterialResolver
    {
        public bool TryResolveNativeCorners(string texture, out IReadOnlyList<string> corners)
        {
            corners = texture.Split('/');
            return corners.Count == 4;
        }

        public string? ResolveNativeTile(IReadOnlyList<string> cornerMaterialIds, int x, int y) =>
            cornerMaterialIds.Contains("unsupported") ? null : string.Join('/', cornerMaterialIds);
    }

    [Fact]
    public void Blend_module_import_paint_undo_redo_and_baseline_need_no_host()
    {
        var resolver = new Resolver();
        string[] source = ["grass/grass/grass/grass"];
        var import = TerrainBlendAuthoringMap.Import(1, source, resolver, "grass");
        var session = new TerrainBlendEditSession(import, source, resolver);
        Assert.False(session.IsDirty);
        var result = session.PaintCircle(0, 0, 0, "sand");
        Assert.True(result.Succeeded);
        Assert.Equal("sand/grass/grass/grass", session.CurrentTextures[0]);
        Assert.True(session.CommitStroke());
        Assert.Single(session.Undo()!);
        Assert.False(session.IsDirty);
        Assert.Equal("grass", import.Map.GetCorner(0, 0));
        Assert.Single(session.Redo()!);
        Assert.Equal("sand", import.Map.GetCorner(0, 0));
        session.CommitBaseline();
        Assert.False(session.IsDirty);
        session.PaintCircle(1, 1, 0, "water");
        session.ResetToBaseline();
        Assert.Equal("sand/grass/grass/grass", session.CurrentTextures[0]);
        Assert.False(session.IsDirty);
        Assert.False(session.CanUndo);
    }

    [Fact]
    public void Unsupported_junction_rolls_back_entire_pending_stroke_without_losing_committed_history()
    {
        var resolver = new Resolver();
        string[] source = ["grass/grass/grass/grass"];
        var import = TerrainBlendAuthoringMap.Import(1, source, resolver, "grass");
        var session = new TerrainBlendEditSession(import, source, resolver);
        session.PaintCircle(0, 0, 0, "sand");
        session.CommitStroke();
        session.PaintCircle(1, 0, 0, "water");
        var rejected = session.PaintCircle(1, 1, 0, "unsupported");
        Assert.False(rejected.Succeeded);
        Assert.Single(rejected.Issues);
        Assert.Equal("sand/grass/grass/grass", session.CurrentTextures[0]);
        Assert.Equal(new[] { "sand", "grass", "grass", "grass" }, import.Map.CornerMaterials);
        Assert.True(session.CanUndo);
        session.Undo();
        Assert.False(session.IsDirty);
    }

    [Fact]
    public void Import_reports_unresolved_tiles_and_bake_reports_unsupported_corners()
    {
        var resolver = new Resolver();
        var import = TerrainBlendAuthoringMap.Import(1, ["unknown"], resolver, "grass");
        Assert.Equal(new[] { 0 }, import.UnresolvedTileIndices);
        import.Map.SetCorner(1, 1, "unsupported");
        var baked = import.Map.Bake(resolver);
        Assert.False(baked.IsComplete);
        Assert.Single(baked.Issues);
        Assert.Null(baked.Textures[0]);
    }
}
