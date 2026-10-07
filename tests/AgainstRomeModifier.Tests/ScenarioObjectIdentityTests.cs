using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed class ScenarioObjectIdentityTests : IDisposable
{
    private readonly string _map = Path.Combine(Path.GetTempPath(), "ArmIdentity_" + Guid.NewGuid().ToString("N"));

    [Fact]
    public void Legacy_loads_get_stable_distinct_ids_and_save_preserves_binding_and_moves()
    {
        Directory.CreateDirectory(_map);
        File.WriteAllText(Path.Combine(_map, ScenarioDocument.FileName),
            "{\"Version\":2,\"Spawns\":[{\"Alias\":\"HOUSE\",\"Prebuilt\":true},{\"Alias\":\"UNIT\"}],\"DataSlots\":[{\"Slot\":42,\"Uid\":123}]}");
        ScenarioDocument first = ScenarioDocument.Load(_map), second = ScenarioDocument.Load(_map);
        Assert.Equal(first.Spawns, second.Spawns);
        Assert.NotEqual(first.Spawns[0].Id, first.Spawns[1].Id);
        Guid id = first.Spawns[0].Id;
        Assert.Equal(42, ScenarioObjectIdentity.DataBinding(first, id)!.Slot);
        first.Spawns[0] = first.Spawns[0] with { X = 1234, Z = 4567 };
        using (var rollback = new FileRollbackScope()) { first.Save(_map, rollback); rollback.Commit(); }
        ScenarioDocument saved = ScenarioDocument.Load(_map);
        Assert.Equal(6, saved.Version); Assert.Equal(id, saved.Spawns[0].Id);
        Assert.Equal(1234, saved.Spawns[0].X);
        Assert.Equal(first.DataSlots, saved.DataSlots);
        Assert.Null(ScenarioObjectIdentity.DataBinding(saved, Guid.NewGuid()));
    }

    [Fact]
    public void Incomplete_legacy_ownership_is_preserved_without_guessing_target_binding()
    {
        var document = new ScenarioDocument { Spawns = [new("A", 0, 0, 0, Prebuilt: true), new("B", 0, 0, 0, Prebuilt: true)], DataSlots = [new(42, 123)] };
        ScenarioObjectIdentity.Prepare(document, legacy: true);
        Assert.Equal(Guid.Empty, Assert.Single(document.DataSlots).SpawnId);
        Assert.Null(ScenarioObjectIdentity.DataBinding(document, document.Spawns[0].Id));
    }

    [Fact]
    public void Current_format_does_not_guess_unbound_ownership_even_with_matching_counts()
    {
        var document = new ScenarioDocument { Spawns = [new("A", 0, 0, 0, Prebuilt: true)], DataSlots = [new(42, 123)] };
        ScenarioObjectIdentity.Prepare(document);
        Assert.Equal(Guid.Empty, Assert.Single(document.DataSlots).SpawnId);
        Assert.Null(ScenarioObjectIdentity.DataBinding(document, document.Spawns[0].Id));
    }

    [Fact]
    public void Duplicate_ids_and_invalid_bindings_fail_before_file_write()
    {
        Directory.CreateDirectory(_map);
        string path = Path.Combine(_map, ScenarioDocument.FileName); File.WriteAllText(path, "baseline");
        Guid id = Guid.NewGuid();
        var document = new ScenarioDocument { Spawns = [new("A", 0, 0, 0) { Id = id }, new("B", 0, 0, 0) { Id = id }] };
        using var rollback = new FileRollbackScope();
        Assert.Throws<InvalidDataException>(() => document.Save(_map, rollback));
        Assert.Equal("baseline", File.ReadAllText(path));
        Guid other = Guid.NewGuid();
        document.Spawns = [new("A", 0, 0, 0, Prebuilt: true) { Id = id }, new("B", 0, 0, 0, Prebuilt: true) { Id = other }];
        document.DataSlots = [new(42, 123) { SpawnId = id }, new(42, 123) { SpawnId = other }];
        Assert.Throws<InvalidDataException>(() => document.Save(_map, rollback));
        Assert.Equal("baseline", File.ReadAllText(path));
        document.Spawns = [new("A", 0, 0, 0, Prebuilt: true) { Id = id }];
        document.DataSlots = [new(42, 123) { SpawnId = Guid.NewGuid() }];
        Assert.Throws<InvalidDataException>(() => document.Save(_map, rollback));
        Assert.Equal("baseline", File.ReadAllText(path));
    }

    [Fact]
    public void Version_four_does_not_silently_replace_missing_identity()
    {
        Directory.CreateDirectory(_map);
        File.WriteAllText(Path.Combine(_map, ScenarioDocument.FileName), "{\"Version\":4,\"Spawns\":[{\"Alias\":\"A\"}]}");
        Assert.Throws<InvalidDataException>(() => ScenarioDocument.Load(_map));
    }

    public void Dispose() { if (Directory.Exists(_map)) Directory.Delete(_map, true); }
}
