using System.Reflection;
using System.Security.Cryptography;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;
using Xunit.Abstractions;

namespace AgainstRomeModifier.Tests;

internal sealed class RequiresInstalledStarterKitFactAttribute : FactAttribute
{
    public RequiresInstalledStarterKitFactAttribute()
    {
        if (Environment.GetEnvironmentVariable("ARM_REQUIRE_INSTALLED_FIXTURES") == "1") return;
        if (!InstalledStarterKitFixture.HasRequiredFixtures(out string reason))
            Skip = reason;
    }
}

internal sealed class RequiresInstalledStarterKitTheoryAttribute : TheoryAttribute
{
    public RequiresInstalledStarterKitTheoryAttribute()
    {
        if (Environment.GetEnvironmentVariable("ARM_REQUIRE_INSTALLED_FIXTURES") == "1") return;
        if (!InstalledStarterKitFixture.HasRequiredFixtures(out string reason))
            Skip = reason;
    }
}

public sealed class StarterKitFamilyTests(ITestOutputHelper output)
{
    [RequiresInstalledStarterKitFact]
    public void Official_TUTOR_01_family_places_saves_reloads_all_members_pools_offsets_and_root_identity()
    {
        using var fixture = new InstalledStarterKitFixture();
        var source = LevelObjectStore.Load(fixture.HouseMap);
        var sourceRoot = Assert.Single(source.Objects(), item => item.Slot == 5504);
        var sourceFamily = source.FamilyAt(sourceRoot.Slot, sourceRoot.Uid)!;
        Assert.Equal(26484u, sourceRoot.Uid);
        Assert.Equal(2, sourceFamily.ObjectSlots.Count);
        var kit = BlankMapStarterKit.Create(fixture.Root, fixture.Map, 5, 8000, 8000, (_, _) => 120);
        Assert.Equal(6, kit.NativeObjectCount);
        Assert.Equal(2, kit.HouseFamilyObjectCount);
        Assert.Equal(2, kit.HouseFamilyPlacements.Count);
        Assert.Equal(Path.GetFullPath(fixture.HouseMap), Path.GetFullPath(kit.HouseSourceMapPath!));
        Assert.Single(kit.Templates);
        Assert.Null(kit.TemplateFor(kit.Spawns.Single(s => s.Alias == BlankMapStarterKit.MainHouseAlias)));
        Assert.Equal(218, kit.Templates[BlankMapStarterKit.UnitAlias].TypeId);
        var store = LevelObjectStore.Load(fixture.Map);
        var previous = new ScenarioDocument();
        var diskBefore = StarterKitFixture.Snapshot(fixture.Map);
        var candidate = kit.ApplyToCandidate(store, previous);
        Assert.Empty(store.Objects()); Assert.Empty(previous.Spawns); Assert.Empty(previous.DataSlots);
        StarterKitFixture.AssertUnchanged(fixture.Map, diskBefore);
        fixture.Save(candidate);
        var saved = LevelObjectStore.Load(fixture.Map);
        var scenario = ScenarioDocument.Load(fixture.Map);
        AssertFamilyKit(kit, saved, scenario, 5);
        var house = kit.Spawns.Single(s => s.Alias == BlankMapStarterKit.MainHouseAlias);
        var binding = scenario.DataSlots.Single(b => b.SpawnId == house.Id);
        var family = saved.FamilyAt(binding.Slot, binding.Uid)!;
        var actual = family.ObjectSlots.Select(slot => saved.Objects().Single(o => o.Slot == slot)).ToArray();
        var original = sourceFamily.ObjectSlots.Select(slot => source.Objects().Single(o => o.Slot == slot)).ToArray();
        Assert.Equal(original.Select(o => o.TypeId).Order(), actual.Select(o => o.TypeId).Order());
        foreach (var member in original)
        {
            var translated = Assert.Single(actual, o => o.TypeId == member.TypeId);
            Assert.Equal(member.X - sourceRoot.X + house.X, translated.X, 3);
            Assert.Equal(member.Y - sourceRoot.Y + house.Y, translated.Y, 3);
            Assert.Equal(member.Z - sourceRoot.Z + house.Z, translated.Z, 3);
            Assert.Equal(member.Rotation, translated.Rotation);
        }
        var oldLights = NativeLightPool.Load(Path.Combine(fixture.HouseMap, "DATA", "light.dat")).ActiveLights()
            .Where(l => sourceFamily.LightSlots.Contains(l.Slot)).ToArray();
        var newLights = NativeLightPool.Load(Path.Combine(fixture.Map, "DATA", "light.dat")).ActiveLights()
            .Where(l => family.LightSlots.Contains(l.Slot)).ToArray();
        Assert.Equal(2, newLights.Length);
        foreach (var light in oldLights)
            Assert.Contains(newLights, l => Math.Abs(l.Position.X - (light.Position.X - sourceRoot.X + house.X)) < .01f
                && Math.Abs(l.Position.Y - (light.Position.Y - sourceRoot.Y + house.Y)) < .01f
                && Math.Abs(l.Position.Z - (light.Position.Z - sourceRoot.Z + house.Z)) < .01f);
        output.WriteLine($"Official source={InstalledStarterKitFixture.Installed}; TUTOR_01 root5504 UID26484; cloned family: 6 DATA objects / 5 root bindings / 2 lights / 1 lager / 1 biglager; hashes unchanged.");
        fixture.AssertSourceUnchanged();
    }

    [RequiresInstalledStarterKitTheory]
    [InlineData("identity")]
    [InlineData("uid")]
    [InlineData("capacity")]
    [InlineData("light.dat")]
    [InlineData("biglager.dat")]
    [InlineData("position")]
    public void Family_candidate_failure_preserves_entire_input_store_scenario_and_disk(string failure)
    {
        using var fixture = new InstalledStarterKitFixture();
        var kit = BlankMapStarterKit.Create(fixture.Root, fixture.Map, 3, 8000, 8000);
        if (failure.EndsWith(".dat", StringComparison.Ordinal)) File.Delete(Path.Combine(fixture.Map, "DATA", failure));
        var store = LevelObjectStore.Load(fixture.Map);
        var previous = new ScenarioDocument();
        if (failure == "identity") previous.Spawns.Add(kit.Spawns[0]);
        if (failure == "uid") Field("_nextUid").SetValue(store, 0x00FFFFFEu); // family fits; later unit allocation must fail.
        if (failure == "capacity")
        {
            var objects = (byte[])Field("_objects").GetValue(store)!;
            for (int i = 0; i < store.Capacity - 5; i++) objects[16 + i * LevelObjectStore.RecordSize] = 1;
        }
        if (failure == "position")
        {
            var positions = (byte[])Field("_positions").GetValue(store)!;
            for (int i = 8; i < positions.Length; i += LevelObjectStore.PositionSize) positions[i] = 1;
        }
        var memoryBefore = fixture.SnapshotStore(store, "before");
        var scenarioBefore = System.Text.Json.JsonSerializer.Serialize(previous);
        var diskBefore = StarterKitFixture.Snapshot(fixture.Map);
        Assert.ThrowsAny<InvalidDataException>(() => kit.ApplyToCandidate(store, previous));
        var memoryAfter = fixture.SnapshotStore(store, "after");
        Assert.Equal(memoryBefore.Keys.Order(), memoryAfter.Keys.Order());
        foreach (var (file, bytes) in memoryBefore) Assert.Equal(bytes, memoryAfter[file]);
        Assert.Equal(scenarioBefore, System.Text.Json.JsonSerializer.Serialize(previous));
        StarterKitFixture.AssertUnchanged(fixture.Map, diskBefore);
        fixture.AssertSourceUnchanged();
    }

    [RequiresInstalledStarterKitFact]
    public void Family_budget_counts_hidden_child_at_14000_boundary_including_formation_banner()
    {
        using var fixture = new InstalledStarterKitFixture();
        var spawn = new ScenarioSpawn("Existing", 0, 0, 0);
        var existing = Enumerable.Repeat(spawn with { Count = 20 }, 666).Concat(Enumerable.Repeat(spawn, 8));
        Assert.Equal(6, BlankMapStarterKit.Create(fixture.Root, fixture.Map, 0, 8000, 8000, existingPlacements: existing).NativeObjectCount);
        Assert.Throws<InvalidDataException>(() => BlankMapStarterKit.Create(fixture.Root, fixture.Map, 0, 8000, 8000,
            existingPlacements: existing.Append(spawn)));
        fixture.AssertSourceUnchanged();
    }

    internal static void AssertFamilyKit(BlankMapStarterKit kit, LevelObjectStore store, ScenarioDocument scenario, int team)
    {
        Assert.Equal(6, store.Objects().Count); Assert.Equal(5, scenario.Spawns.Count);
        Assert.Equal(5, scenario.DataSlots.Count); Assert.Empty(scenario.ScriptSpawns);
        Assert.Equal(6, store.Objects().Select(o => o.Uid).Distinct().Count());
        Assert.All(store.Objects(), o => { Assert.Equal(team, o.Team); Assert.InRange(o.Uid, 1u, 0x00FFFFFFu); });
        foreach (var spawn in kit.Spawns)
        {
            var binding = Assert.Single(scenario.DataSlots, b => b.SpawnId == spawn.Id);
            var obj = Assert.Single(store.Objects(), o => o.Slot == binding.Slot && o.Uid == binding.Uid);
            Assert.Equal(spawn.Alias == BlankMapStarterKit.MainHouseAlias ? 232 : 218, obj.TypeId);
            Assert.Equal(spawn.X, obj.X); Assert.Equal(spawn.Y, obj.Y); Assert.Equal(spawn.Z, obj.Z);
            if (obj.TypeId == 232)
            {
                var family = store.FamilyAt(obj.Slot, obj.Uid);
                Assert.NotNull(family); Assert.Equal(2, family.ObjectSlots.Count);
                Assert.Equal(2, family.LightSlots.Count); Assert.Single(family.LagerSlots); Assert.Single(family.BigLagerSlots);
                Assert.DoesNotContain(scenario.DataSlots, b => b.Slot != obj.Slot && family.ObjectSlots.Contains(b.Slot));
            }
        }
    }

    private static FieldInfo Field(string name) => typeof(LevelObjectStore).GetField(name, BindingFlags.Instance | BindingFlags.NonPublic)!;
}

internal sealed class InstalledStarterKitFixture : IDisposable
{
    internal const string DefaultInstalled = @"C:\Program Files (x86)\Against Rome";
    internal static string Installed => ResolveInstalledPath();
    internal static string ResolveInstalledPath()
    {
        string? custom = Environment.GetEnvironmentVariable("ARM_TEST_INSTALLED_PATH")
            ?? Environment.GetEnvironmentVariable("ARM_GAME_PATH");
        return !string.IsNullOrWhiteSpace(custom) ? custom : DefaultInstalled;
    }

    internal static bool HasRequiredFixtures(out string missingReason)
    {
        string root = Installed;
        foreach (string id in new[] { "TUTOR_01", "KAMP_021_i" })
        {
            string source = Path.Combine(root, "MAPS", id, "DATA");
            if (!Directory.Exists(source))
            {
                missingReason = $"Requires installed official DATA at '{source}'; proprietary game files are not in Git.";
                return false;
            }
        }
        foreach (string relative in new[] { @"CLAK\cl_scint.ini", @"DATA_MP\DEFAULTS\objdef.dau", @"DATA_MP\DEFAULTS\lightdef.dau" })
        {
            string file = Path.Combine(root, "SYSTEM", relative);
            if (!File.Exists(file))
            {
                missingReason = $"Requires installed system file at '{file}'; proprietary game files are not in Git.";
                return false;
            }
        }
        missingReason = string.Empty;
        return true;
    }

    internal string Root { get; } = Path.Combine(Path.GetTempPath(), "ArmStarterOfficial_" + Guid.NewGuid().ToString("N"));
    internal string HouseMap => Path.Combine(Root, "MAPS", "TUTOR_01");
    internal string Map => Path.Combine(Root, "MAPS", "ENDL_005");
    private readonly Dictionary<string, string> _hashes = new();

    internal InstalledStarterKitFixture()
    {
        string root = Installed;
        foreach (string id in new[] { "TUTOR_01", "KAMP_021_i" })
        {
            string source = Path.Combine(root, "MAPS", id, "DATA");
            Assert.True(Directory.Exists(source), $"Installed official DATA required: {source}");
            foreach (string file in Directory.EnumerateFiles(source)) CopyOfficial(file, Path.Combine(Root, "MAPS", id, "DATA", Path.GetFileName(file)));
        }
        foreach (string relative in new[] { @"CLAK\cl_scint.ini", @"DATA_MP\DEFAULTS\objdef.dau", @"DATA_MP\DEFAULTS\lightdef.dau" })
            CopyOfficial(Path.Combine(root, "SYSTEM", relative), Path.Combine(Root, "SYSTEM", relative));
        foreach (string file in Directory.EnumerateFiles(Path.Combine(HouseMap, "DATA")))
        { Directory.CreateDirectory(Path.Combine(Map, "DATA")); File.Copy(file, Path.Combine(Map, "DATA", Path.GetFileName(file))); }
        var blank = LevelObjectStore.Load(Map); blank.ResetForBlankMap();
        using var rollback = new FileRollbackScope(); blank.Save(Map, rollback); new ScenarioDocument().Save(Map, rollback); rollback.Commit();
        File.WriteAllText(Path.Combine(Map, CustomMapManifest.MarkerFileName), "{}");
    }

    internal void Save(ScenarioCandidateResult candidate)
    { using var rollback = new FileRollbackScope(); candidate.Store.Save(Map, rollback); candidate.Scenario.Save(Map, rollback); rollback.Commit(); }
    internal Dictionary<string, byte[]> SnapshotStore(LevelObjectStore store, string label)
    {
        string path = Path.Combine(Root, "Snapshot", label); Directory.CreateDirectory(path);
        using var rollback = new FileRollbackScope(); store.Save(path, rollback); rollback.Commit();
        return StarterKitFixture.Snapshot(path);
    }
    private void CopyOfficial(string source, string destination)
    {
        _hashes[source] = Hash(source); Directory.CreateDirectory(Path.GetDirectoryName(destination)!); File.Copy(source, destination);
    }
    internal void AssertSourceUnchanged() { foreach (var (file, hash) in _hashes) Assert.Equal(hash, Hash(file)); }
    private static string Hash(string path) => Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(path)));
    public void Dispose() { if (Directory.Exists(Root)) Directory.Delete(Root, true); }
}
