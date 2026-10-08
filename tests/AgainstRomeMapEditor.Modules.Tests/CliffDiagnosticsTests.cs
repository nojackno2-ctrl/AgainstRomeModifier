using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.Diagnostics;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class CliffDiagnosticsTests
{
    [Fact]
    public void Unblocked_steep_slope_triggers_cliff_steep_unblocked_warning()
    {
        const int heightSize = 9;
        const int collisionSize = 8;
        var heights = new byte[heightSize * heightSize];

        // 建立陡坡（y=0 為 100，y=1 為 10，delta = 90 >= 15）
        for (int vy = 0; vy < heightSize; vy++)
        for (int vx = 0; vx < heightSize; vx++)
            heights[vy * heightSize + vx] = vy == 0 ? (byte)100 : (byte)10;

        // 通行層全為 0（未阻擋）
        var collision = new byte[collisionSize * collisionSize];

        var snapshot = new MapCheckSnapshot(
            Objects: [],
            Events: [],
            Aliases: [],
            CollisionSize: collisionSize,
            Collision: collision,
            HeightSize: heightSize,
            Heights: heights,
            HeightStep: 4f);

        var issues = CliffDiagnostics.Check(snapshot, tileDimension: 2);

        Assert.Contains(issues, i => i.Code == "cliff-steep-unblocked" && i.Severity == MapIssueSeverity.Warning);
    }

    [Fact]
    public void Blocked_steep_slope_clears_warning()
    {
        const int heightSize = 9;
        const int collisionSize = 8;
        var heights = new byte[heightSize * heightSize];
        for (int vy = 0; vy < heightSize; vy++)
        for (int vx = 0; vx < heightSize; vx++)
            heights[vy * heightSize + vx] = vy == 0 ? (byte)100 : (byte)10;

        // 通行層設為 255（已阻擋）
        var collision = new byte[collisionSize * collisionSize];
        Array.Fill(collision, (byte)255);

        var snapshot = new MapCheckSnapshot(
            Objects: [],
            Events: [],
            Aliases: [],
            CollisionSize: collisionSize,
            Collision: collision,
            HeightSize: heightSize,
            Heights: heights,
            HeightStep: 4f);

        var issues = CliffDiagnostics.Check(snapshot, tileDimension: 2);

        Assert.DoesNotContain(issues, i => i.Code == "cliff-steep-unblocked");
    }

    [Fact]
    public void Cliff_texture_without_collision_triggers_cliff_missing_collision()
    {
        const int tileDim = 2;
        const int collisionSize = 8;
        var collision = new byte[collisionSize * collisionSize]; // 全 0 通行

        var catalog = CliffTileCatalog.CreateDefault();
        string[] textures = ["FELS_N1", "4BB___50", "4BB___50", "4BB___50"];

        var snapshot = new MapCheckSnapshot(
            Objects: [],
            Events: [],
            Aliases: [],
            CollisionSize: collisionSize,
            Collision: collision);

        var issues = CliffDiagnostics.Check(snapshot, catalog, textures, tileDimension: tileDim);

        Assert.Contains(issues, i => i.Code == "cliff-missing-collision" && i.Severity == MapIssueSeverity.Warning);
    }
}
