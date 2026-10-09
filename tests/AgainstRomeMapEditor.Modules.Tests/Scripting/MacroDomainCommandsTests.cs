using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeMapEditor.Modules.Scripting.Execution;
using AgainstRomeMapEditor.Modules.Scripting.Models;
using AgainstRomeModifier.Maps;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests.Scripting;

public sealed class MacroDomainCommandsTests
{
    private static SdlObjectType CreateTestObjectType(string nameDef, SdlObjectCategory category = SdlObjectCategory.Building) =>
        new(nameDef, 1, category, "Rom", 1, new Dictionary<string, string> { ["alias"] = nameDef });

    [Fact]
    public void ElevateAndFlatten_RectAndCircle_ModifiesHeightsCorrectly()
    {
        byte[] heights = new byte[16 * 16];
        Array.Fill(heights, (byte)40);
        var heightSession = new TerrainHeightEditSession(16, heights, null, 16, null);

        var context = new CommandExecutionContext
        {
            HeightSession = heightSession,
            MapTileDimension = 16
        };

        var registry = CommandRegistry.CreateDefault();
        var runner = new MacroScriptRunner(registry);

        // 1. Elevate rect
        var res1 = runner.ExecuteLine("/elevate rect 2 2 6 6 20", context, recordSingleActionAsCompound: true);
        Assert.True(res1.Success);
        Assert.Equal(60, heightSession.Heights[4 * 16 + 4]);

        // 2. Flatten circle
        var res2 = runner.ExecuteLine("/flatten circle 4 4 2 95", context, recordSingleActionAsCompound: true);
        Assert.True(res2.Success);
        Assert.Equal(95, heightSession.Heights[4 * 16 + 4]);

        // 3. Undo
        runner.Undo(); // undo flatten
        Assert.Equal(60, heightSession.Heights[4 * 16 + 4]);

        runner.Undo(); // undo elevate
        Assert.Equal(40, heightSession.Heights[4 * 16 + 4]);
    }

    [Fact]
    public void SpawnRing_PlacesObjectsInCircle_WithCorrectCoordinatesAndAngles()
    {
        var placementSession = new PlacementEditSession();
        var towerType = CreateTestObjectType("BauRomTurm01");

        var context = new CommandExecutionContext
        {
            PlacementSession = placementSession,
            AvailableObjectTypes = new[] { towerType },
            WorldDimension = 16384f
        };

        var registry = CommandRegistry.CreateDefault();
        var runner = new MacroScriptRunner(registry);

        var res = runner.ExecuteLine("/spawn-ring BauRomTurm01 5000 5000 400 4 --team 1 --angle outward", context, recordSingleActionAsCompound: true);

        Assert.True(res.Success);
        Assert.Equal(4, res.AffectedCount);
        Assert.Equal(4, placementSession.Count);

        // 驗證 4 個正交點半徑均約為 400
        for (int i = 0; i < 4; i++)
        {
            var item = placementSession[i];
            Assert.Equal("BauRomTurm01", item.Type.NameDef);
            Assert.Equal(1, item.Team);

            float dx = item.WorldX - 5000f;
            float dz = item.WorldZ - 5000f;
            float dist = MathF.Sqrt(dx * dx + dz * dz);
            Assert.True(Math.Abs(dist - 400f) < 1.0f);
        }

        // 驗證單步 Undo
        Assert.True(runner.CanUndo);
        runner.Undo();
        Assert.Equal(0, placementSession.Count);
    }

    [Fact]
    public void AlignGrid_SnapsCoordinatesToSpecifiedStep()
    {
        var placementSession = new PlacementEditSession();
        var buildingType = CreateTestObjectType("BauGerHau01");

        // 放置在未對齊的坐標
        placementSession.Add(new SdlPlacedObject(buildingType, 105f, 0f, 215f, Team: 0) { ScenarioId = Guid.NewGuid() });
        placementSession.Add(new SdlPlacedObject(buildingType, 301f, 0f, 410f, Team: 0) { ScenarioId = Guid.NewGuid() });

        var context = new CommandExecutionContext
        {
            PlacementSession = placementSession,
            WorldDimension = 16384f
        };

        var registry = CommandRegistry.CreateDefault();
        var runner = new MacroScriptRunner(registry);

        var res = runner.ExecuteLine("/align-grid 32", context, recordSingleActionAsCompound: true);

        Assert.True(res.Success);
        Assert.Equal(2, res.AffectedCount);

        // 105 -> 96 (32 * 3), 215 -> 224 (32 * 7)
        Assert.Equal(96f, placementSession[0].WorldX);
        Assert.Equal(224f, placementSession[0].WorldZ);

        // 301 -> 288 (32 * 9), 410 -> 416 (32 * 13)
        Assert.Equal(288f, placementSession[1].WorldX);
        Assert.Equal(416f, placementSession[1].WorldZ);

        // Undo 測試
        runner.Undo();
        Assert.Equal(105f, placementSession[0].WorldX);
        Assert.Equal(215f, placementSession[0].WorldZ);
        Assert.Equal(301f, placementSession[1].WorldX);
        Assert.Equal(410f, placementSession[1].WorldZ);
        Assert.Equal(2, placementSession.Count);
        Assert.False(runner.CanUndo);

        Assert.True(runner.CanRedo);
        runner.Redo();
        Assert.Equal(96f, placementSession[0].WorldX);
        Assert.Equal(224f, placementSession[0].WorldZ);
        Assert.Equal(288f, placementSession[1].WorldX);
        Assert.Equal(416f, placementSession[1].WorldZ);
    }

    [Fact]
    public void SelectAndSetTeam_FiltersAndBatchModifiesTeamId()
    {
        var placementSession = new PlacementEditSession();
        var figureType = CreateTestObjectType("GER_INF01", SdlObjectCategory.Figure);
        var buildingType = CreateTestObjectType("BauRomTurm01", SdlObjectCategory.Building);

        placementSession.Add(new SdlPlacedObject(figureType, 100f, 0f, 100f, Team: 0, UnitCount: 10) { ScenarioId = Guid.NewGuid() });
        placementSession.Add(new SdlPlacedObject(figureType, 200f, 0f, 200f, Team: 1, UnitCount: 10) { ScenarioId = Guid.NewGuid() });
        placementSession.Add(new SdlPlacedObject(buildingType, 300f, 0f, 300f, Team: 1) { ScenarioId = Guid.NewGuid() });

        var context = new CommandExecutionContext
        {
            PlacementSession = placementSession,
            WorldDimension = 16384f
        };

        var registry = CommandRegistry.CreateDefault();
        var runner = new MacroScriptRunner(registry);

        // 1. Select team 1
        var selRes = runner.ExecuteLine("/select-team 1", context);
        Assert.True(selRes.Success);
        Assert.Equal(2, context.SelectedPlacementIndices.Count);

        // 2. Set team 3 on selected
        var setRes = runner.ExecuteLine("/set-team 3 --selected", context, recordSingleActionAsCompound: true);
        Assert.True(setRes.Success);
        Assert.Equal(2, setRes.AffectedCount);

        Assert.Equal(0, placementSession[0].Team);
        Assert.Equal(3, placementSession[1].Team);
        Assert.Equal(3, placementSession[2].Team);

        // 3. Undo
        runner.Undo();
        Assert.Equal(1, placementSession[1].Team);
        Assert.Equal(1, placementSession[2].Team);
    }

    [Fact]
    public void ReplaceTexture_ReplacesInBoundingRect_AndUndoRestores()
    {
        const int dim = 64;
        var textures = Enumerable.Repeat("Gras1", dim * dim).ToArray();
        var map = new TerrainBlendAuthoringMap(dim, "Gras1");
        var import = new NativeTerrainImportResult(map, [], []);
        var blendSession = new TerrainBlendEditSession(import, textures, new MacroTestResolver());

        var context = new CommandExecutionContext
        {
            BlendSession = blendSession,
            MapTileDimension = dim,
            WorldDimension = dim * 256f
        };

        var registry = CommandRegistry.CreateDefault();
        var runner = new MacroScriptRunner(registry);

        // Replace Gras1 with Sand1 in rect 10,10 to 12,12 (3x3 = 9 tiles)
        var res = runner.ExecuteLine("/replace-texture Gras1 Sand1 --rect 10,10,12,12", context, recordSingleActionAsCompound: true);
        Assert.True(res.Success);
        Assert.Equal(9, res.AffectedCount);

        // Check target tile is Sand1
        Assert.Equal("Sand1", blendSession.CurrentTextures[10 * dim + 10]);
        Assert.Equal("Sand1", blendSession.CurrentTextures[12 * dim + 12]);
        // Tile outside rect is still Gras1
        Assert.Equal("Gras1", blendSession.CurrentTextures[9 * dim + 10]);

        // Undo
        Assert.True(runner.CanUndo);
        runner.Undo();
        Assert.Equal("Gras1", blendSession.CurrentTextures[10 * dim + 10]);
        Assert.Equal("Gras1", blendSession.CurrentTextures[12 * dim + 12]);
    }

    [Fact]
    public void HealNavMesh_FindsAndHealsRoadGaps_On64TileGrid_AndUndoRestores()
    {
        const int dim = 64;
        var textures = Enumerable.Repeat("Gras1", dim * dim).ToArray();
        // Create road gap at tile (20, 15): endpoints at (19, 15) and (21, 15)
        textures[15 * dim + 18] = "H_WEG1";
        textures[15 * dim + 19] = "H_WEG1";
        // gap at (20, 15)
        textures[15 * dim + 21] = "H_WEG1";
        textures[15 * dim + 22] = "H_WEG1";

        var map = new TerrainBlendAuthoringMap(dim, "Gras1");
        var import = new NativeTerrainImportResult(map, [], []);
        var blendSession = new TerrainBlendEditSession(import, textures, new MacroTestResolver());

        var context = new CommandExecutionContext
        {
            BlendSession = blendSession,
            MapTileDimension = dim,
            WorldDimension = dim * 256f
        };

        var registry = CommandRegistry.CreateDefault();
        var runner = new MacroScriptRunner(registry);

        var res = runner.ExecuteLine("/heal-navmesh", context, recordSingleActionAsCompound: true);
        Assert.True(res.Success);
        Assert.Equal(1, res.AffectedCount);

        // The gap at (20, 15) should now be patched with H_WEG1
        Assert.Equal("H_WEG1", blendSession.CurrentTextures[15 * dim + 20]);

        // Undo
        Assert.True(runner.CanUndo);
        runner.Undo();
        Assert.Equal("Gras1", blendSession.CurrentTextures[15 * dim + 20]);
    }

    private sealed class MacroTestResolver : INativeTerrainMaterialResolver
    {
        public bool TryResolveNativeCorners(string texture, out IReadOnlyList<string> corners)
        {
            corners = ["grass", "grass", "grass", "grass"];
            return true;
        }

        public string? ResolveNativeTile(IReadOnlyList<string> corners, int tileX, int tileY) => corners[0];
        public bool HasEdgeBake(string inner, string outer) => true;
        public IReadOnlyList<string> IntermediateMaterials(string inner, string outer) => [];
    }
}
