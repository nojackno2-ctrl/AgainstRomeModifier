using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed class EndlessAiAdditionalMapTests : IDisposable
{
    private const string ScriptPattern = "MAPS/ENDL_*/SCRIPT/ak_level.bci";
    private const string SdlPattern = "MAPS/ENDL_*/Endlos_*_Siedlung*.sdl";
    private readonly string _root = Path.Combine(Path.GetTempPath(), "ArmExtraEndl_" + Guid.NewGuid().ToString("N"));

    [Theory]
    [InlineData(ScriptPattern)]
    [InlineData(SdlPattern)]
    public void Resolution_covers_additional_slots_and_excludes_nonmaps_staging_and_nested_copies(string pattern)
    {
        foreach (string id in new[] { "ENDL_000", "ENDL_005", "ENDL_999", "ENDL_ABC", "ENDL_005.tmp_arm", "ENDL_006.deleting_arm", "Other/ENDL_001", "ENDL_005/Extra" })
            AddMap(id, [1, 2, 3, 4]);
        string[] expected = new[] { "ENDL_000", "ENDL_005", "ENDL_999" }
            .Select(id => Path.Combine(_root, "MAPS", id, pattern == ScriptPattern ? "SCRIPT/ak_level.bci" : "Endlos_Rom_Siedlung1.sdl").Replace('/', Path.DirectorySeparatorChar)).ToArray();
        Assert.Equal(expected, EndlessAiOrchestrator.ResolvePaths(_root, pattern));
        Assert.Equal(3, EndlessAiOrchestrator.GetExpectedFileCount(_root, pattern));
    }

    [Fact]
    public void Real_P1_patch_detects_applies_saves_reloads_and_restores_all_seven_slots()
    {
        var orchestrator = new EndlessAiOrchestrator();
        var p1 = Assert.IsType<BciLiteralPatch>(orchestrator.M1.Patches.Single(patch => patch.Id == "P1"));
        var module = new EndlessAiModule("P1-coverage", "P1 coverage", [p1]);
        int[] words = p1.Signature.Select(word => word ?? 123456).ToArray();
        for (int i = 0; i < p1.ValueWordIndices.Length; i++) words[p1.ValueWordIndices[i]] = p1.OriginalValues[i];
        byte[] header = new byte[64];
        header[0] = (byte)'P'; header[1] = (byte)'F'; header[2] = (byte)'I'; header[3] = (byte)'L';
        byte[] original = GameLZSS.CompressPfil(words.SelectMany(BitConverter.GetBytes).ToArray(), header);
        foreach (int slot in new[] { 0, 1, 2, 3, 4, 5, 999 })
        {
            string map = AddMap($"ENDL_{slot:000}", original);
            if (slot >= 5) File.WriteAllText(Path.Combine(map, CustomMapManifest.MarkerFileName), "{}");
        }
        string[] paths = EndlessAiOrchestrator.ResolvePaths(_root, ScriptPattern).ToArray();
        Assert.Equal(7, EndlessAiOrchestrator.GetExpectedFileCount(_root, ScriptPattern));
        Assert.Equal(PatchState.Original, orchestrator.DetectModule(_root, module));
        Assert.True(orchestrator.ApplyModule(_root, module, true));
        Assert.All(paths, path => Assert.Equal(original, File.ReadAllBytes(path))); // Buffered only.
        using (var rollback = new FileRollbackScope()) { orchestrator.SaveAll(_root, rollback); rollback.Commit(); }
        var loaded = new EndlessAiOrchestrator();
        Assert.Equal(PatchState.Ultimate, loaded.DetectModule(_root, module));
        Assert.All(paths, path => Assert.Equal(PatchState.Ultimate, p1.Detect(GameLZSS.DecompressPfil(File.ReadAllBytes(path)))));
        Assert.True(loaded.ApplyModule(_root, module, false));
        using (var rollback = new FileRollbackScope()) { loaded.SaveAll(_root, rollback); rollback.Commit(); }
        Assert.Equal(PatchState.Original, new EndlessAiOrchestrator().DetectModule(_root, module));
        Assert.All(paths, path => Assert.Equal(GameLZSS.DecompressPfil(original), GameLZSS.DecompressPfil(File.ReadAllBytes(path))));
    }

    private string AddMap(string id, byte[] script)
    {
        string map = Path.Combine(_root, "MAPS", id.Replace('/', Path.DirectorySeparatorChar));
        Directory.CreateDirectory(Path.Combine(map, "SCRIPT"));
        File.WriteAllBytes(Path.Combine(map, "SCRIPT", "ak_level.bci"), script);
        File.WriteAllText(Path.Combine(map, "Endlos_Rom_Siedlung1.sdl"), "[object0000]\r\nteam=1\r\n");
        return map;
    }

    [Fact]
    public void Standalone_empty_level_does_not_make_real_P1_unknown_and_is_preserved_during_apply_and_restore()
    {
        var orchestrator = new EndlessAiOrchestrator();
        var p1 = Assert.IsType<BciLiteralPatch>(orchestrator.M1.Patches.Single(patch => patch.Id == "P1"));
        var module = new EndlessAiModule("P1-standalone", "Standalone scope", [p1]);
        int[] words = p1.Signature.Select(word => word ?? 123456).ToArray();
        for (int index = 0; index < p1.ValueWordIndices.Length; index++) words[p1.ValueWordIndices[index]] = p1.OriginalValues[index];
        byte[] header = new byte[64]; "PFIL"u8.CopyTo(header);
        byte[] original = GameLZSS.CompressPfil(words.SelectMany(BitConverter.GetBytes).ToArray(), header);
        AddMap("ENDL_000", original); string template = AddMap("ENDL_005", original);
        File.WriteAllText(Path.Combine(template, CustomMapManifest.MarkerFileName), "{}");
        byte[] idle = GameLZSS.CompressPfil(Scripting.BciImage.CreateIdleLevel().Serialize(), header);
        string empty = AddMap("ENDL_006", idle);
        File.WriteAllText(Path.Combine(empty, CustomMapManifest.MarkerFileName), System.Text.Json.JsonSerializer.Serialize(
            new CustomMapEntry(6, 0, DateTimeOffset.UtcNow, "Test") { StandaloneLevel = true }));
        var before = Directory.GetFiles(empty, "*", SearchOption.AllDirectories).ToDictionary(path => path, File.ReadAllBytes);
        Assert.True(CustomMapManifest.HasStandaloneLevel(empty)); Assert.False(CustomMapManifest.HasStandaloneLevel(template));
        Assert.Equal(2, EndlessAiOrchestrator.GetExpectedFileCount(_root, ScriptPattern));
        Assert.Equal(2, EndlessAiOrchestrator.GetExpectedFileCount(_root, SdlPattern));
        Assert.Equal(PatchState.Original, orchestrator.DetectModule(_root, module));
        Assert.True(orchestrator.ApplyModule(_root, module, true));
        using (var rollback = new FileRollbackScope()) { orchestrator.SaveAll(_root, rollback); rollback.Commit(); }
        var loaded = new EndlessAiOrchestrator(); Assert.Equal(PatchState.Ultimate, loaded.DetectModule(_root, module));
        Assert.True(loaded.ApplyModule(_root, module, false));
        using (var rollback = new FileRollbackScope()) { loaded.SaveAll(_root, rollback); rollback.Commit(); }
        foreach (var (path, bytes) in before) Assert.Equal(bytes, File.ReadAllBytes(path));
        // A misplaced marker must never hide an original slot from detection.
        string official = Path.Combine(_root, "MAPS", "ENDL_000");
        File.Copy(Path.Combine(empty, CustomMapManifest.MarkerFileName), Path.Combine(official, CustomMapManifest.MarkerFileName));
        Assert.False(CustomMapManifest.HasStandaloneLevel(official));
        Assert.Equal(2, EndlessAiOrchestrator.GetExpectedFileCount(_root, ScriptPattern));
    }
    public void Dispose() { if (Directory.Exists(_root)) Directory.Delete(_root, true); }
}
