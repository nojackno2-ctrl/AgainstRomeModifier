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
}
