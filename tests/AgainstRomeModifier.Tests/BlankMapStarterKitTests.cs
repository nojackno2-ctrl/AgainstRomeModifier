using System.Buffers.Binary;
using System.Security.Cryptography;
using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;
using Xunit.Abstractions;

namespace AgainstRomeModifier.Tests;

public sealed class BlankMapStarterKitTests
{
    private readonly ITestOutputHelper _output;
    public BlankMapStarterKitTests(ITestOutputHelper output) => _output = output;

    [Fact]
    public void Create_samples_each_height_and_produces_five_prebuilt_individuals_with_safe_internal_spacing()
    {
        using var fixture = new StarterKitSyntheticFixture();
        var sampled = new List<(float X, float Z)>();
        var kit = BlankMapStarterKit.Create(fixture.Root, fixture.Map, 6, 8000, 8000,
            (x, z) => { sampled.Add((x, z)); return x / 100 + z / 200; });
        Assert.Equal(5, kit.Spawns.Count);
        Assert.Equal(kit.Spawns.Select(spawn => (spawn.X, spawn.Z)), sampled);
        Assert.Equal(5, kit.Spawns.Select(spawn => spawn.Id).Distinct().Count());
        Assert.All(kit.Spawns, spawn =>
        {
            Assert.NotEqual(Guid.Empty, spawn.Id); Assert.True(spawn.Prebuilt);
            Assert.Equal(6, spawn.Team); Assert.Equal(0, spawn.Count);
            Assert.Equal(spawn.X / 100 + spawn.Z / 200, spawn.Y);
        });
        var house = Assert.Single(kit.Spawns, spawn => spawn.Alias == BlankMapStarterKit.MainHouseAlias);
        var units = kit.Spawns.Where(spawn => spawn.Alias == BlankMapStarterKit.UnitAlias).ToArray();
        Assert.Equal(4, units.Length);
        Assert.All(units, unit => Assert.True(Distance(house, unit) >= 800));
        for (int i = 0; i < units.Length; i++)
        for (int j = i + 1; j < units.Length; j++) Assert.True(Distance(units[i], units[j]) >= 320);
    }

    [Fact]
    public void ApplyToCandidate_then_save_reload_preserves_native_type_ids_and_chosen_team_without_script_spawns()
    {
        using var fixture = new StarterKitSyntheticFixture();
        var kit = BlankMapStarterKit.Create(fixture.Root, fixture.Map, 7, 8000, 8000);
        var store = LevelObjectStore.Load(fixture.Map); store.ResetForBlankMap();
        var previous = new ScenarioDocument();
        var before = StarterKitFixture.Snapshot(fixture.Map);
        var result = kit.ApplyToCandidate(store, previous);
        Assert.Empty(store.Objects()); Assert.Empty(previous.Spawns); Assert.Empty(previous.DataSlots);
        StarterKitFixture.AssertUnchanged(fixture.Map, before);
        Assert.Empty(result.Skipped); Assert.Empty(result.Scenario.ScriptSpawns);
        using (var rollback = new FileRollbackScope())
        { result.Store.Save(fixture.Map, rollback); result.Scenario.Save(fixture.Map, rollback); rollback.Commit(); }
        AssertNativeKit(kit, LevelObjectStore.Load(fixture.Map), ScenarioDocument.Load(fixture.Map), 7);
    }

    [Fact]
    public void Real_TEMP_DATA_pools_with_synthetic_safe_templates_Apply_save_reload_preserves_verified_type_ids_and_source()
    {
        using var fixture = new StarterKitFixture();
        using var synthetic = new StarterKitSyntheticFixture();
        Assert.Equal("BauGerHau00_Haupthaus", fixture.Names[232]);
        Assert.Equal("FigGerInf02_Zweihandaxt", fixture.Names[218]);
        // TEMP has real native pools and definitions but no safe axeman template.
        // Positive allocation uses explicitly synthetic templates; the separate real-source
        // test below enforces fail-closed behavior instead of pretending these are original records.
        var kit = BlankMapStarterKit.Create(synthetic.Root, synthetic.Map, 3, 8192, 8192);
        _output.WriteLine($"Read-only DATA source={fixture.Source}; ENDL_000; synthetic templates GER_HAU00 TypeId=232 / GER_INF02 TypeId=218; team=3");
        Assert.Equal(232, kit.Templates[BlankMapStarterKit.MainHouseAlias].TypeId);
        Assert.Equal(218, kit.Templates[BlankMapStarterKit.UnitAlias].TypeId);
        byte[] idle = File.ReadAllBytes(Path.Combine(fixture.Map, "SCRIPT", LevelScriptInjector.ScriptFile));
        var candidate = kit.ApplyToCandidate(LevelObjectStore.Load(fixture.Map), ScenarioDocument.Load(fixture.Map));
        using (var rollback = new FileRollbackScope())
        { candidate.Store.Save(fixture.Map, rollback); candidate.Scenario.Save(fixture.Map, rollback); rollback.Commit(); }
        AssertNativeKit(kit, LevelObjectStore.Load(fixture.Map), ScenarioDocument.Load(fixture.Map), 3);
        Assert.Equal(idle, File.ReadAllBytes(Path.Combine(fixture.Map, "SCRIPT", LevelScriptInjector.ScriptFile)));
        fixture.AssertSourceUnchanged();
    }

    [Fact]
    public void Real_TEMP_source_missing_safe_house_and_unit_templates_fails_closed_without_writes_or_script_fallback()
    {
        using var fixture = new StarterKitFixture();
        var before = StarterKitFixture.Snapshot(fixture.Root);
        Assert.DoesNotContain(218, fixture.Templates.Keys);
        var error = Assert.Throws<InvalidDataException>(() => BlankMapStarterKit.Create(fixture.Root, fixture.OfficialMap, 3, 8192, 8192));
        Assert.Contains("GER_INF02", error.Message);
        _output.WriteLine($"Source={fixture.Source}; ENDL_000; requested real IDs 232 / 218; missing safe DATA templates rejected: {error.Message}");
        StarterKitFixture.AssertUnchanged(fixture.Root, before);
        fixture.AssertSourceUnchanged();
    }

    [Fact]
    public void Object_budget_accepts_14000_and_rejects_14001_including_script_formation_banner()
    {
        using var fixture = new StarterKitSyntheticFixture();
        var single = new ScenarioSpawn("Existing", 100, 100, 0);
        Assert.Equal(14000, BlankMapStarterKit.MaximumObjects);
        Assert.Equal(5, BlankMapStarterKit.Create(fixture.Root, fixture.Map, 0, 8000, 8000,
            existingPlacements: Enumerable.Repeat(single, 13995)).Spawns.Count);
        Assert.Throws<InvalidDataException>(() => BlankMapStarterKit.Create(fixture.Root, fixture.Map, 0, 8000, 8000,
            existingPlacements: Enumerable.Repeat(single, 13996)));
        var formations = Enumerable.Repeat(single with { Count = 20 }, 666).Concat(Enumerable.Repeat(single, 9));
        Assert.Equal(5, BlankMapStarterKit.Create(fixture.Root, fixture.Map, 0, 8000, 8000, existingPlacements: formations).Spawns.Count);
        Assert.Throws<InvalidDataException>(() => BlankMapStarterKit.Create(fixture.Root, fixture.Map, 0, 8000, 8000,
            existingPlacements: formations.Append(single)));
    }

    [Theory]
    [InlineData(true)]
    [InlineData(false)]
    public void Missing_required_template_fails_closed_without_any_disk_change_or_script_fallback(bool house)
    {
        using var fixture = new StarterKitSyntheticFixture(includeHouse: !house, includeUnits: house);
        var before = StarterKitFixture.Snapshot(fixture.Root);
        var error = Assert.Throws<InvalidDataException>(() => BlankMapStarterKit.Create(fixture.Root, fixture.Map, 0, 8000, 8000));
        Assert.Contains(house ? "GER_HAU00" : "GER_INF02", error.Message);
        StarterKitFixture.AssertUnchanged(fixture.Root, before);
    }

    [Theory]
    [InlineData(-1, 8000, 8000)]
    [InlineData(8, 8000, 8000)]
    [InlineData(0, 100, 8000)]
    [InlineData(0, 8000, 16000)]
    [InlineData(0, float.NaN, 8000)]
    public void Invalid_team_or_out_of_bounds_kit_fails_before_mutation(int team, float x, float z)
    {
        using var fixture = new StarterKitSyntheticFixture();
        var before = StarterKitFixture.Snapshot(fixture.Root);
        Assert.Throws<ArgumentOutOfRangeException>(() => BlankMapStarterKit.Create(fixture.Root, fixture.Map, team, x, z));
        StarterKitFixture.AssertUnchanged(fixture.Root, before);
    }

    [Fact]
    public void Nonfinite_height_and_insufficient_DATA_capacity_fail_closed()
    {
        using var fixture = new StarterKitSyntheticFixture(capacity: 5);
        Assert.Throws<ArgumentOutOfRangeException>(() => BlankMapStarterKit.Create(fixture.Root, fixture.Map, 0, 8000, 8000, (_, _) => float.NaN));
        var kit = BlankMapStarterKit.Create(fixture.Root, fixture.Map, 0, 8000, 8000);
        var store = LevelObjectStore.Load(fixture.Map); // Two source objects leave only three free slots.
        var original = store.Objects().ToArray(); var previous = new ScenarioDocument();
        Assert.Throws<InvalidDataException>(() => kit.ApplyToCandidate(store, previous));
        Assert.Equal(original, store.Objects()); Assert.Empty(previous.Spawns); Assert.Empty(previous.DataSlots);
    }

    [Theory]
    [InlineData(14000, true)]
    [InlineData(14001, false)]
    public void Native_template_capacity_accepts_14000_but_rejects_14001(int capacity, bool allowed)
    {
        using var fixture = new StarterKitSyntheticFixture(capacity);
        if (allowed) Assert.Equal(5, BlankMapStarterKit.Create(fixture.Root, fixture.Map, 0, 8000, 8000).Spawns.Count);
        else Assert.Throws<InvalidDataException>(() => BlankMapStarterKit.Create(fixture.Root, fixture.Map, 0, 8000, 8000));
    }

    [Fact]
    public void Apply_with_unresolvable_existing_prebuilt_rejects_skipped_template_without_mutating_inputs()
    {
        using var fixture = new StarterKitSyntheticFixture();
        var kit = BlankMapStarterKit.Create(fixture.Root, fixture.Map, 0, 8000, 8000);
        var store = LevelObjectStore.Load(fixture.Map); store.ResetForBlankMap();
        var previous = new ScenarioDocument
        { Spawns = [new ScenarioSpawn("MISSING_EXISTING_HOUSE", 1000, 1000, 0, Prebuilt: true) { Id = Guid.NewGuid() }] };
        var original = previous.Spawns.ToArray(); var before = StarterKitFixture.Snapshot(fixture.Map);
        Assert.Throws<InvalidDataException>(() => kit.ApplyToCandidate(store, previous));
        Assert.Empty(store.Objects()); Assert.Equal(original, previous.Spawns); Assert.Empty(previous.DataSlots);
        StarterKitFixture.AssertUnchanged(fixture.Map, before);
    }

    internal static void AssertNativeKit(BlankMapStarterKit kit, LevelObjectStore store, ScenarioDocument scenario, int team)
    {
        Assert.Equal(5, store.Objects().Count); Assert.Equal(5, scenario.Spawns.Count);
        Assert.Equal(5, scenario.DataSlots.Count); Assert.Empty(scenario.ScriptSpawns);
        foreach (var spawn in kit.Spawns)
        {
            var binding = Assert.Single(scenario.DataSlots, item => item.SpawnId == spawn.Id);
            var obj = Assert.Single(store.Objects(), item => item.Slot == binding.Slot);
            Assert.Equal(binding.Uid, obj.Uid); Assert.NotEqual(0u, obj.Uid);
            Assert.Equal(kit.TemplateFor(spawn)!.TypeId, obj.TypeId); Assert.Equal(team, obj.Team);
            Assert.Equal(spawn.X, obj.X); Assert.Equal(spawn.Y, obj.Y); Assert.Equal(spawn.Z, obj.Z);
            Assert.Equal(spawn.Id, Assert.Single(scenario.Spawns, item => item.Id == spawn.Id).Id);
        }
    }

    private static double Distance(ScenarioSpawn a, ScenarioSpawn b) => Math.Sqrt(Math.Pow(a.X - b.X, 2) + Math.Pow(a.Z - b.Z, 2));

    [Fact]
    public void Starter_placements_are_one_batch_undo_redo_preserving_IDS_team_and_native_rotation()
    {
        using var fixture = new StarterKitSyntheticFixture();
        var kit = BlankMapStarterKit.Create(fixture.Root, fixture.Map, 4, 8000, 8000);
        var session = new PlacementEditSession();
        var placements = kit.Spawns.Select(spawn => new SdlPlacedObject(
            new SdlObjectType(spawn.Alias == BlankMapStarterKit.MainHouseAlias ? BlankMapStarterKit.MainHouseName : BlankMapStarterKit.UnitName,
                kit.TemplateFor(spawn)!.TypeId, spawn.Alias == BlankMapStarterKit.MainHouseAlias ? SdlObjectCategory.Building : SdlObjectCategory.Figure,
                "Ger", 1, new Dictionary<string, string> { ["alias"] = spawn.Alias }),
            spawn.X, spawn.Y, spawn.Z, spawn.Team) { ScenarioId = spawn.Id, RawRotation = 0 }).ToArray();
        Assert.Equal(Enumerable.Range(0, 5), session.AddMany(placements));
        Assert.Equal(5, session.Count); Assert.True(session.IsDirty);
        Assert.True(session.Undo()); Assert.Empty(session.Capture()); Assert.False(session.CanUndo); Assert.False(session.IsDirty);
        Assert.True(session.Redo()); Assert.False(session.CanRedo);
        Assert.Equal(placements.Select(item => item.ScenarioId), session.Capture().Select(item => item.ScenarioId));
        Assert.All(session.Capture(), item => { Assert.Equal(4, item.Team); Assert.Equal(0f, item.RawRotation); });
    }
}

internal sealed class StarterKitSyntheticFixture : IDisposable
{
    internal string Root { get; } = Path.Combine(Path.GetTempPath(), "ArmStarterSynthetic_" + Guid.NewGuid().ToString("N"));
    internal string Map => Path.Combine(Root, "MAPS", "ENDL_000");
    internal StarterKitSyntheticFixture(int capacity = 16, bool includeHouse = true, bool includeUnits = true)
    {
        Directory.CreateDirectory(Path.Combine(Map, "DATA"));
        Directory.CreateDirectory(Path.Combine(Root, "SYSTEM", "CLAK"));
        Directory.CreateDirectory(Path.Combine(Root, "SYSTEM", "DATA_MP", "DEFAULTS"));
        byte[] header = new byte[64]; "PFIL"u8.CopyTo(header);
        void Text(string path, string text) => File.WriteAllBytes(path, GameLZSS.CompressPfil(MapTextEncoding.Game.GetBytes(text), header));
        Text(Path.Combine(Root, "SYSTEM", "CLAK", "cl_scint.ini"),
            "[ObjDefName]\r\nGER_HAU00=BauGerHau00_Haupthaus\r\nGER_INF02=FigGerInf02_Zweihandaxt\r\n");
        string Definition(int id, string name)
        { var columns = Enumerable.Repeat("0", 53).ToArray(); columns[0] = id.ToString(); columns[1] = "1"; columns[52] = name; return string.Join(',', columns); }
        Text(Path.Combine(Root, "SYSTEM", "DATA_MP", "DEFAULTS", "objdef.dau"),
            "[ObjectDefaults]\r\n" + Definition(232, BlankMapStarterKit.MainHouseName) + "\r\n" + Definition(218, BlankMapStarterKit.UnitName) + "\r\n");
        byte[] objects = new byte[16 + capacity * (LevelObjectStore.RecordSize + LevelObjectStore.ColumnWidths.Sum())];
        byte[] data = new byte[8 + capacity * LevelObjectStore.ObjDataWidths.Sum()];
        byte[] positions = new byte[8 + capacity * 2 * LevelObjectStore.PositionSize];
        void I32(byte[] bytes, int offset, int value) => BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(offset), value);
        I32(objects, 0, 1); I32(objects, 4, capacity); I32(objects, 8, 30); I32(objects, 12, 30);
        I32(data, 0, 1); I32(data, 4, capacity); I32(positions, 0, 1); I32(positions, 4, capacity * 2);
        for (int slot = 0; slot < capacity; slot++)
        {
            int record = 16 + slot * LevelObjectStore.RecordSize;
            bool active = slot == 0 && includeHouse || slot == 1 && includeUnits;
            objects[record] = active ? (byte)1 : (byte)0;
            BinaryPrimitives.WriteUInt32LittleEndian(objects.AsSpan(record + 3), (uint)(slot + 1));
            foreach (int offset in new[] { 71, 73, 77 }) BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(record + offset), 0xFFFF);
            BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(record + 67), (ushort)(slot * 2));
            BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(record + 69), (ushort)(slot * 2 + 1));
            BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(record + 75), (ushort)(slot == 0 ? 232 : 218));
            int columnStart = 16 + capacity * LevelObjectStore.RecordSize;
            for (int column = 0; column < LevelObjectStore.ColumnWidths.Length; column++)
            {
                uint value = column == 1 ? (uint)slot : column is 2 or 3 or 4 or 5 or 8 or 10 or 11 or 12 or 13 ? 0xFFFFu : 0;
                int offset = columnStart + slot * LevelObjectStore.ColumnWidths[column];
                if (LevelObjectStore.ColumnWidths[column] == 2) BinaryPrimitives.WriteUInt16LittleEndian(objects.AsSpan(offset), (ushort)value);
                else BinaryPrimitives.WriteUInt32LittleEndian(objects.AsSpan(offset), value);
                columnStart += capacity * LevelObjectStore.ColumnWidths[column];
            }
            data[8 + slot * LevelObjectStore.ObjDataWidths[0]] = active ? (byte)1 : (byte)0;
            if (active) { positions[8 + slot * 2 * LevelObjectStore.PositionSize] = 1; positions[8 + (slot * 2 + 1) * LevelObjectStore.PositionSize] = 1; }
        }
        File.WriteAllBytes(Path.Combine(Map, "DATA", "objects.dat"), objects);
        File.WriteAllBytes(Path.Combine(Map, "DATA", "objdata.dat"), data);
        File.WriteAllBytes(Path.Combine(Map, "DATA", "position.dat"), positions);
    }
    public void Dispose() { if (Directory.Exists(Root)) Directory.Delete(Root, true); }
}

// All writes stay in a unique disposable copy.
internal sealed class StarterKitFixture : IDisposable
{
    internal const string CompareSource = @"C:\Users\nojac\AppData\Local\Temp\ArmGameCompare_20261007";
    internal string Root { get; } = Path.Combine(Path.GetTempPath(), "ArmStarterKit_" + Guid.NewGuid().ToString("N"));
    internal string Map => Path.Combine(Root, "MAPS", "ENDL_005");
    internal string OfficialMap => Path.Combine(Root, "MAPS", "ENDL_000");
    internal string Source { get; }
    private readonly Dictionary<string, string> _sourceHashes;

    internal StarterKitFixture()
    {
        Source = Directory.Exists(CompareSource) ? CompareSource : @"C:\Program Files (x86)\Against Rome";
        string sourceMap = Path.Combine(Source, Source == CompareSource ? "ENDL_000" : @"MAPS\ENDL_000");
        Assert.True(Directory.Exists(sourceMap), $"Real starter-kit fixture is required: {sourceMap}");
        // Do not copy images, archives or TEXTUREN assets that the persistence tests do not consume.
        var files = Directory.EnumerateFiles(sourceMap, "*", SearchOption.AllDirectories)
            .Where(path => Path.GetRelativePath(sourceMap, path).StartsWith("DATA" + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase)
                || Path.GetRelativePath(sourceMap, path).StartsWith("SCRIPT" + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase)
                || Path.GetExtension(path).Equals(".sdl", StringComparison.OrdinalIgnoreCase))
            .Concat(new[] { @"CLAK\cl_scint.ini", @"DATA_MP\DEFAULTS\objdef.dau", @"DATA_MP\DEFAULTS\lightdef.dau" }
                .Select(relative => Path.Combine(Source, "SYSTEM", relative)).Where(File.Exists)).ToArray();
        _sourceHashes = files.ToDictionary(path => path, Hash);
        foreach (string file in files)
        {
            string destination = file.StartsWith(sourceMap + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase)
                ? Path.Combine(OfficialMap, Path.GetRelativePath(sourceMap, file))
                : Path.Combine(Root, "SYSTEM", Path.GetRelativePath(Path.Combine(Source, "SYSTEM"), file));
            CopyFile(file, destination);
        }
        foreach (string file in Directory.EnumerateFiles(OfficialMap, "*", SearchOption.AllDirectories))
            CopyFile(file, Path.Combine(Map, Path.GetRelativePath(OfficialMap, file)));
        var store = LevelObjectStore.Load(Map);
        store.ResetForBlankMap();
        using var rollback = new FileRollbackScope();
        store.Save(Map, rollback);
        new ScenarioDocument().Save(Map, rollback);
        rollback.Commit();
        Directory.CreateDirectory(Path.Combine(Map, "SCRIPT"));
        byte[] idle = BciImage.CreateIdleLevel().Serialize();
        File.WriteAllBytes(Path.Combine(Map, "SCRIPT", LevelScriptInjector.ScriptFile), idle);
        File.WriteAllBytes(Path.Combine(Map, "SCRIPT", LevelScriptInjector.OriginalBackupFile), idle);
        File.WriteAllText(Path.Combine(Map, CustomMapManifest.MarkerFileName), "{}");
        Assert.Empty(LevelObjectStore.Load(Map).Objects());
    }

    internal IReadOnlyDictionary<int, string> Names => ObjDefNames.Load(Root);
    internal IReadOnlyDictionary<int, LevelObjectTemplate> Templates => LevelObjectStore.LoadOfficialTemplates(Root, _ => true);

    internal void AssertSourceUnchanged()
    {
        foreach (var (file, hash) in _sourceHashes) Assert.Equal(hash, Hash(file));
    }

    internal static Dictionary<string, byte[]> Snapshot(string directory) => Directory.EnumerateFiles(directory, "*", SearchOption.AllDirectories)
        .ToDictionary(path => Path.GetRelativePath(directory, path), File.ReadAllBytes);

    internal static void AssertUnchanged(string directory, Dictionary<string, byte[]> before)
    {
        var after = Snapshot(directory);
        Assert.Equal(before.Keys.Order(), after.Keys.Order());
        foreach (var (file, bytes) in before) Assert.Equal(bytes, after[file]);
    }

    private static string Hash(string file) => Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(file)));
    private static void CopyFile(string source, string destination)
    { Directory.CreateDirectory(Path.GetDirectoryName(destination)!); File.Copy(source, destination); }
    public void Dispose() { if (Directory.Exists(Root)) Directory.Delete(Root, true); }
}
