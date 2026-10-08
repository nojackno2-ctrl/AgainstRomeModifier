using AgainstRomeMapEditor.Modules.Diagnostics;
using AgainstRomeModifier.Scripting;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class MapDiagnosticsTests
{
    private static MapCheckObject Unit(float x = 1000, float z = 1000) => new(new("UNIT", x, z, 0, 10) { Id = Guid.NewGuid() }, false, false);
    private static MapCheckObject House(float x, float z, bool template = true) => new(new("HOUSE", x, z, 0, Prebuilt: true) { Id = Guid.NewGuid() }, true, template);
    private static MapCheckSnapshot Map(IReadOnlyList<MapCheckObject> objects, byte[]? collision = null, IReadOnlyList<ScenarioEvent>? events = null) =>
        new(objects, events ?? [], ["UNIT", "HOUSE"], collision is null ? 0 : 8, collision);

    [Fact]
    public void Invalid_targets_and_positions_are_errors_and_disabled_targets_do_not_block()
    {
        var item = new ScenarioEvent("Deleted") { Conditions = [new(ScenarioConditionKind.ObjectExists, Guid.NewGuid())], Actions = [new(ScenarioActionKind.Victory)] };
        var issues = MapDiagnostics.Check(Map([Unit(float.NaN)], events: [item]));
        Assert.Contains(issues, issue => issue.Code == "coordinates" && issue.Severity == MapIssueSeverity.Error);
        Assert.Contains(issues, issue => issue.Code == "event-target" && issue.EventIndex == 0);
        Assert.DoesNotContain(MapDiagnostics.Check(Map([Unit()], events: [item with { Enabled = false }])), issue => issue.Code == "event-target");
    }

    [Fact]
    public void Collision_wall_disconnects_house_and_event_area_without_turning_warnings_into_errors()
    {
        var unit = Unit(); var house = House(14000, 1000);
        byte[] collision = new byte[64]; for (int y = 0; y < 8; y++) collision[y * 8 + 4] = 255;
        var area = new ScenarioEvent("Cross wall") { Conditions = [new(ScenarioConditionKind.ObjectInArea, unit.Spawn.Id, 13000, 0, 15000, 2000)], Actions = [new(ScenarioActionKind.Victory)] };
        var issues = MapDiagnostics.Check(Map([unit, house], collision, [area]));
        Assert.Contains(issues, issue => issue.Code == "isolated-building" && issue.ObjectId == house.Spawn.Id);
        Assert.Contains(issues, issue => issue.Code == "isolated-area" && issue.EventIndex == 0 && issue.WorldX == 14000);
        Assert.DoesNotContain(issues, issue => issue.Severity == MapIssueSeverity.Error);
        collision[4] = 0;
        Assert.DoesNotContain(MapDiagnostics.Check(Map([unit, house], collision, [area])), issue => issue.Code.StartsWith("isolated-", StringComparison.Ordinal));
    }

    [Fact]
    public void Submerged_start_and_missing_templates_are_warnings_and_input_is_unchanged()
    {
        var unit = Unit(); byte[] collision = new byte[64], heights = Enumerable.Repeat((byte)10, 81).ToArray();
        var map = Map([unit, House(14000, 14000, false)], collision) with { HeightSize = 9, Heights = heights, HeightStep = 4, WaterLevel = 120 };
        var issues = MapDiagnostics.Check(map);
        Assert.Contains(issues, issue => issue.Code == "blocked-start");
        Assert.Contains(issues, issue => issue.Code == "construction-site");
        Assert.All(issues, issue => Assert.Equal(MapIssueSeverity.Warning, issue.Severity));
        Assert.All(collision, value => Assert.Equal(0, value)); Assert.All(heights, value => Assert.Equal(10, value));
    }

    [Fact]
    public void Missing_terrain_and_start_overlap_are_reported_without_guessing_script_spawns()
    {
        var a = House(1000, 1000); var b = House(1010, 1010);
        var issues = MapDiagnostics.Check(Map([a, b]));
        Assert.Contains(issues, issue => issue.Code == "no-start");
        Assert.Contains(issues, issue => issue.Code == "no-collision");
        Assert.Equal(2, issues.Count(issue => issue.Code == "overlap"));
    }

    [Fact]
    public void Separate_starting_regions_both_count_as_reachable_and_diagonal_contact_is_not_a_path()
    {
        byte[] collision = Enumerable.Repeat((byte)255, 64).ToArray(); collision[0] = collision[9] = 0;
        var unit = Unit(); var house = House(3000, 3000);
        Assert.Contains(MapDiagnostics.Check(Map([unit, house], collision)), issue => issue.Code == "isolated-building");
        Assert.DoesNotContain(MapDiagnostics.Check(Map([unit, Unit(3000, 3000), house], collision)), issue => issue.Code == "isolated-building");
    }
}
