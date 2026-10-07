using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class TerrainBlendModuleTests
{
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
