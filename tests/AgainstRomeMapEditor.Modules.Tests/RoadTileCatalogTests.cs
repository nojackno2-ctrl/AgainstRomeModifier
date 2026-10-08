using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class RoadTileCatalogTests
{
    [Fact]
    public void BuildAvailable_with_standard_road_tiles_plans_straights_and_turns()
    {
        string[] textures = ["H_WEG1", "H_WEG2", "V_WEG1", "weg1"];
        var available = RoadTileCatalog.BuildAvailable(textures);

        Assert.NotEmpty(available);
        Assert.Contains(available, t => t.Texture == "H_WEG1" && t.Connections == (RoadConnections.East | RoadConnections.West));
        Assert.Contains(available, t => t.Texture == "V_WEG1" && t.Connections == (RoadConnections.North | RoadConnections.South));
        Assert.Contains(available, t => t.Texture == "weg1" && t.Connections == (RoadConnections.North | RoadConnections.East));

        string[] blank = Enumerable.Repeat("grass", 36).ToArray();
        // L-turn from (1, 1) to (4, 1) to (4, 4)
        var plan = RoadStrokePlanner.Plan(6, blank, [(1, 1), (4, 1), (4, 4)], available, "H_WEG1");
        Assert.True(plan.Succeeded);
        Assert.Equal(7, plan.Tiles.Count);

        // Horizontal segment should use H_WEG
        Assert.Contains(plan.Tiles, t => t.X == 2 && t.Y == 1 && t.Texture.StartsWith("H_WEG", StringComparison.OrdinalIgnoreCase));
        Assert.Contains(plan.Tiles, t => t.X == 3 && t.Y == 1 && t.Texture.StartsWith("H_WEG", StringComparison.OrdinalIgnoreCase));
        // Corner (4, 1) connects West & South -> should use weg1
        Assert.Contains(plan.Tiles, t => t.X == 4 && t.Y == 1 && t.Texture == "weg1");
        // Vertical segment should use V_WEG
        Assert.Contains(plan.Tiles, t => t.X == 4 && t.Y == 2 && t.Texture.StartsWith("V_WEG", StringComparison.OrdinalIgnoreCase));
        Assert.Contains(plan.Tiles, t => t.X == 4 && t.Y == 3 && t.Texture.StartsWith("V_WEG", StringComparison.OrdinalIgnoreCase));
    }

    [Fact]
    public void BuildAvailable_prefers_roman_pieces_when_roman_selected()
    {
        string[] textures = ["H_WEG1", "V_WEG1", "weg1", "WEG_H1ROM", "WEG_V1ROM", "Pflaster_braun1"];
        var available = RoadTileCatalog.BuildAvailable(textures, preferredTexture: "WEG_H1ROM");

        string[] blank = Enumerable.Repeat("grass", 36).ToArray();
        var plan = RoadStrokePlanner.Plan(6, blank, [(1, 1), (3, 1), (3, 3)], available, "WEG_H1ROM");
        Assert.True(plan.Succeeded);

        Assert.Contains(plan.Tiles, t => t.X == 2 && t.Y == 1 && t.Texture == "WEG_H1ROM");
        Assert.Contains(plan.Tiles, t => t.X == 3 && t.Y == 2 && t.Texture == "WEG_V1ROM");
        Assert.Contains(plan.Tiles, t => t.X == 3 && t.Y == 1 && t.Texture == "Pflaster_braun1");
    }

    [Fact]
    public void BuildAvailable_falls_back_to_weg_when_directional_straights_missing()
    {
        // When floortex library only has weg1 (e.g. test fixture), weg1 serves as fallback for all masks
        string[] textures = ["weg1"];
        var available = RoadTileCatalog.BuildAvailable(textures);

        string[] blank = Enumerable.Repeat("grass", 36).ToArray();
        var plan = RoadStrokePlanner.Plan(6, blank, [(1, 1), (3, 1), (3, 3)], available, "weg1");
        Assert.True(plan.Succeeded);
        Assert.All(plan.Tiles, t => Assert.Equal("weg1", t.Texture));
    }
}
