using AgainstRomeMapEditor.Modules.Persistence;
using AgainstRomeModifier.Scripting;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class ScenarioSavePreflightTests
{
    [Fact]
    public void Deleted_target_is_rejected_but_disabled_event_is_retained()
    {
        var item = new ScenarioEvent("Target") { Actions = [new(ScenarioActionKind.Message, "Ready")],
            Conditions = [new(ScenarioConditionKind.ObjectExists, Guid.NewGuid())] };
        var doc = new ScenarioDocument { Events = [item] };
        Assert.Throws<InvalidDataException>(() => ScenarioSavePreflight.Validate(doc, []));
        doc.Events[0] = item with { Enabled = false }; ScenarioSavePreflight.Validate(doc, []);
    }

    [Fact]
    public void New_prebuilt_target_does_not_require_binding_until_DATA_rebuild()
    {
        var spawn = new ScenarioSpawn("BUILD", 4000, 5000, 0, Prebuilt: true) { Id = Guid.NewGuid() };
        var doc = new ScenarioDocument { Spawns = [spawn], Events = [new("New building") {
            Actions = [new(ScenarioActionKind.Message, "Ready")], Conditions = [new(ScenarioConditionKind.ObjectExists, spawn.Id)] }] };
        ScenarioSavePreflight.Validate(doc, ["BUILD"]); Assert.Empty(doc.DataSlots);
        doc.Spawns.Add(spawn); Assert.Throws<InvalidDataException>(() => ScenarioSavePreflight.Validate(doc, ["BUILD"]));
    }

    [Theory]
    [InlineData(float.NaN, 0, 0)]
    [InlineData(16384, 0, 0)]
    [InlineData(10, 21, 0)]
    [InlineData(10, 10, 8)]
    public void Invalid_positions_and_silently_truncated_unit_counts_are_rejected(float x, int count, int team)
    {
        var doc = new ScenarioDocument { Spawns = [new("UNIT", x, 10, team, count) { Id = Guid.NewGuid() }] };
        Assert.Throws<InvalidDataException>(() => ScenarioSavePreflight.Validate(doc, ["UNIT"]));
    }
}
