using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class TerrainAutoBridgeTests
{
    // 原版限制的縮影：rock 與 grass 之間沒有過渡 tile，但兩者都能與 mud 過渡；tile 只能含兩種材質。
    private sealed class ChainResolver(params string[] pairs) : INativeTerrainMaterialResolver
    {
        private readonly HashSet<string> _pairs = pairs.SelectMany(pair => new[] { pair, string.Join('|', Enumerable.Reverse(pair.Split('|'))) }).ToHashSet();

        public IReadOnlyList<string> MaterialIds { get; } = ["grass", "mud", "rock", "water"];

        public bool TryResolveNativeCorners(string texture, out IReadOnlyList<string> corners)
        {
            corners = texture.Split('/');
            return corners.Count == 4;
        }

        public string? ResolveNativeTile(IReadOnlyList<string> corners, int x, int y)
        {
            string[] distinct = corners.Distinct().ToArray();
            if (distinct.Length == 1) return string.Join('/', corners);
            return distinct.Length == 2 && _pairs.Contains(distinct[0] + "|" + distinct[1]) ? string.Join('/', corners) : null;
        }
    }

    private static TerrainBlendEditSession Uniform(INativeTerrainMaterialResolver resolver, int size, string material)
    {
        string[] source = Enumerable.Repeat($"{material}/{material}/{material}/{material}", size * size).ToArray();
        return new TerrainBlendEditSession(TerrainBlendAuthoringMap.Import(size, source, resolver, material), source, resolver);
    }

    [Fact]
    public void Without_bridge_a_missing_transition_is_rejected_and_rolled_back()
    {
        var resolver = new ChainResolver("grass|mud", "mud|rock");
        var session = Uniform(resolver, 12, "grass");
        var result = session.PaintCircle(6, 6, 2, "rock");
        Assert.False(result.Succeeded);
        Assert.NotEmpty(result.Issues);
        Assert.All(session.CurrentTextures, texture => Assert.Equal("grass/grass/grass/grass", texture));
    }

    [Fact]
    public void Auto_bridge_inserts_a_mud_ring_between_rock_and_grass_using_only_supported_tiles()
    {
        var resolver = new ChainResolver("grass|mud", "mud|rock");
        var session = Uniform(resolver, 12, "grass");
        var result = session.PaintCircle(6, 6, 2, "rock", autoBridge: true);
        Assert.True(result.Succeeded, string.Join(" ", result.Issues.Select(issue => string.Join(",", issue.CornerMaterialIds))));
        Assert.All(session.CurrentTextures, texture =>
        {
            string[] corners = texture.Split('/');
            Assert.NotNull(resolver.ResolveNativeTile(corners, 0, 0));
            Assert.False(corners.Contains("rock") && corners.Contains("grass"), "rock 不可直接接 grass");
        });
        Assert.Contains(session.CurrentTextures, texture => texture.Contains("mud"));
        Assert.Contains(session.CurrentTextures, texture => texture == "rock/rock/rock/rock");
        session.CommitStroke();
        session.Undo();
        Assert.All(session.CurrentTextures, texture => Assert.Equal("grass/grass/grass/grass", texture));
        Assert.False(session.IsDirty);
    }

    [Fact]
    public void Auto_bridge_can_chain_two_rings_and_still_rejects_when_no_bridge_exists()
    {
        var chained = Uniform(new ChainResolver("grass|mud", "mud|water", "water|rock"), 16, "grass");
        Assert.True(chained.PaintCircle(8, 8, 2, "rock", autoBridge: true).Succeeded); // rock→water→mud→grass

        var isolated = Uniform(new ChainResolver("grass|mud"), 12, "grass");
        var result = isolated.PaintCircle(6, 6, 2, "rock", autoBridge: true);
        Assert.False(result.Succeeded);
        Assert.All(isolated.CurrentTextures, texture => Assert.Equal("grass/grass/grass/grass", texture));
    }
}

public sealed class TerrainStampTests
{
    private sealed class Resolver : INativeTerrainMaterialResolver
    {
        public bool TryResolveNativeCorners(string texture, out IReadOnlyList<string> corners)
        {
            corners = texture.Split('/');
            return corners.Count == 4;
        }

        public string? ResolveNativeTile(IReadOnlyList<string> corners, int x, int y) => string.Join('/', corners);
    }

    [Fact]
    public void Stamp_sets_one_exact_tile_and_is_undoable_and_redoable()
    {
        var resolver = new Resolver();
        string[] source = Enumerable.Repeat("grass/grass/grass/grass", 9).ToArray();
        var session = new TerrainBlendEditSession(TerrainBlendAuthoringMap.Import(3, source, resolver, "grass"), source, resolver);
        Assert.Null(session.StampTexture(5, 0, "weg1"));
        var change = session.StampTexture(1, 1, "weg1");
        Assert.NotNull(change);
        Assert.Equal("weg1", session.CurrentTextures[4]);
        Assert.Null(session.StampTexture(1, 1, "weg1"));
        Assert.True(session.CommitStroke());
        session.Undo();
        Assert.Equal("grass/grass/grass/grass", session.CurrentTextures[4]);
        Assert.False(session.IsDirty);
        session.Redo();
        Assert.Equal("weg1", session.CurrentTextures[4]);
        // 之後在同一格塗材質會依角點重新烘焙並覆蓋印章。
        Assert.True(session.PaintCircle(1, 1, 0, "sand").Succeeded);
        Assert.NotEqual("weg1", session.CurrentTextures[4]);
    }
}
