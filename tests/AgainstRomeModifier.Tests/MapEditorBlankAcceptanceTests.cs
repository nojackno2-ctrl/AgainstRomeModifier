using System.Buffers.Binary;
using System.Drawing;
using System.Reflection;
using System.Text.Json;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Blank_scene_real_copy_removes_source_content_before_publication_and_survives_editor_save_reload()
    {
        if (Environment.GetEnvironmentVariable("ARM_BLANK_ACCEPTANCE") != "1") return;
        string source = RequireDemoTempPath(Environment.GetEnvironmentVariable("ARM_COMPARE_GAME"));
        string output = RequireDemoTempPath(Environment.GetEnvironmentVariable("ARM_BLANK_OUTPUT"));
        Assert.False(Directory.Exists(output), "Use a new output directory.");
        Assert.False(output.StartsWith(source + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase));
        var hashes = DemoHashes(source);
        string game = Path.Combine(output, "game"), template = Path.Combine(game, "MAPS", "ENDL_000");
        CopyDemoDirectory(Path.Combine(source, "ENDL_000"), template);
        CopyDemoDirectory(Path.Combine(source, "SYSTEM"), Path.Combine(game, "SYSTEM"));
        foreach (string asset in new[] { "floortex.dat", "alr.dat", "apt.dat", "shad.dat" }) File.Copy(Path.Combine(source, asset), Path.Combine(game, asset));
        Assert.NotEmpty(LevelObjectStore.Load(template).Objects());
        Assert.True(Directory.GetFiles(template, "*.sdl").Sum(path => SdlDocument.Load(path).Objects.Count) > 0);
        var inherited = new ScenarioDocument { Spawns = [new("GER_INF01", 3000, 4000, 0, Count: 10) { Id = Guid.NewGuid() }],
            Events = [new("Old event", 2) { Actions = [new(ScenarioActionKind.Message, "Inherited")] }] };
        using (var rollback = new FileRollbackScope())
        {
            inherited.Save(template, rollback); LevelScriptInjector.Apply(template, inherited, ["GER_INF01"], rollback); rollback.Commit();
        }
        File.Copy(Path.Combine(template, "SCRIPT", LevelScriptInjector.ScriptFile), Path.Combine(template, "SCRIPT", "old_autospawn.bci"));
        foreach (var (file, header, record, stateWidth) in new[] { ("light.dat", 8, 4, 4), ("particle.dat", 12, 2252, 2),
            ("explos.dat", 8, 46, 2), ("hitex.dat", 8, 28, 2), ("flash.dat", 12, 201, 1) })
        {
            string path = Path.Combine(template, "DATA", file); byte[] packed = File.ReadAllBytes(path), bytes = GameLZSS.DecompressPfil(packed);
            bytes.AsSpan(header + 2 * record, stateWidth).Clear(); bytes[header + 2 * record] = 1;
            File.WriteAllBytes(path, GameLZSS.CompressPfil(bytes, packed[..64]));
        }
        var templateBefore = DemoHashes(template);
        EndlessMapInfo created = BlankMapBuilder.Create(game, "ENDL_000", 5, "ARM Empty Scene");
        string map = created.DirectoryPath;
        Assert.True(created.IsCustom); Assert.Equal("ENDL_005", created.Id);
        Assert.True(CustomMapManifest.HasStandaloneLevel(map));
        Assert.True(CustomMapManifest.Load(game).Entries.Single(entry => entry.Slot == 5).StandaloneLevel);
        Assert.Equal(1, EndlessAiOrchestrator.GetExpectedFileCount(game, "MAPS/ENDL_*/SCRIPT/ak_level.bci"));
        Assert.Equal(templateBefore, DemoHashes(template)); Assert.Empty(LevelObjectStore.Load(map).Objects());
        Assert.All(Directory.GetFiles(map, "*.sdl"), path => Assert.Empty(SdlDocument.Load(path).Objects));
        ScenarioDocument empty = ScenarioDocument.Load(map);
        Assert.Empty(empty.Spawns); Assert.Empty(empty.Events); Assert.Empty(empty.DataSlots);
        string script = Path.Combine(map, "SCRIPT", LevelScriptInjector.ScriptFile);
        byte[] bootstrap = File.ReadAllBytes(script);
        Assert.Equal(bootstrap, File.ReadAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.OriginalBackupFile)));
        Assert.Single(Directory.GetFiles(Path.Combine(map, "SCRIPT"), "*.bci"));
        BciImage image = BciImage.Parse(GameLZSS.DecompressPfil(bootstrap));
        Assert.Equal(BciImage.CreateIdleLevel().Serialize(), image.Serialize());
        byte[] positions = GameLZSS.DecompressPfil(File.ReadAllBytes(Path.Combine(map, "DATA", "position.dat")));
        Assert.True(positions.AsSpan(8).IndexOfAnyExcept((byte)0) < 0);
        var pools = new[] { ("anim.dat", 8, 21, 1), ("gfxtype.dat", 8, 15, 1), ("action.dat", 12, 25, 1), ("hirarchy.dat", 12, 103, 1),
            ("formatio.dat", 8, 15, 1), ("lager.dat", 16, 43, 1), ("biglager.dat", 12, 1601, 1), ("light.dat", 8, 4, 4),
            ("particle.dat", 12, 2252, 2), ("explos.dat", 8, 46, 2), ("hitex.dat", 8, 28, 2), ("flash.dat", 12, 201, 1) };
        foreach (var (file, header, record, stateWidth) in pools)
        {
            byte[] before = GameLZSS.DecompressPfil(File.ReadAllBytes(Path.Combine(template, "DATA", file)));
            byte[] after = GameLZSS.DecompressPfil(File.ReadAllBytes(Path.Combine(map, "DATA", file)));
            Assert.Equal(before.Length, after.Length); Assert.Equal(before.AsSpan(0, header).ToArray(), after.AsSpan(0, header).ToArray());
            int count = BinaryPrimitives.ReadInt32LittleEndian(after.AsSpan(4));
            for (int slot = 0; slot < count; slot++) Assert.True(after.AsSpan(header + slot * record, stateWidth).IndexOfAnyExcept((byte)0) < 0);
        }
        byte[] ways = GameLZSS.DecompressPfil(File.ReadAllBytes(Path.Combine(map, "DATA", "way.dat")));
        Assert.True(ways.AsSpan(12).IndexOfAnyExcept((byte)0) < 0);
        Assert.Single(BodenTexturesDocument.Load(Path.Combine(map, "boden.txt")).Textures.Distinct());
        foreach (string cache in TerrainLayerFiles.HeightDependentCaches) Assert.False(File.Exists(Path.Combine(map, cache)));
        foreach (var (file, value) in new[] { ("boden.bmp", (byte)50), ("vertex.bmp", (byte)255), ("smooth.bmp", (byte)0), ("emboss.bmp", (byte)0), ("collision.bmp", (byte)0) })
        {
            TerrainLayer layer = TerrainLayerFiles.Read(Path.Combine(map, file))!;
            Assert.All(layer.Green, actual => Assert.Equal(value, actual));
            Assert.All(layer.Argb, actual => Assert.Equal(Color.FromArgb(value, value, value).ToArgb(), actual));
        }
        RunInSta(() =>
        {
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using (var form = new MapEditorForm(game, new GameMapInfo(created.Id, map, true, created.DisplayName, "Test")))
            {
                typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
                form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000); form.Show(); Application.DoEvents();
                Assert.Empty(form.PlacementSession.Capture());
                Assert.False(GetField<bool>(form, "_propertyDirty"));
                Assert.Empty(GetField<IReadOnlyList<LevelWorldObject>>(form, "_levelObjects"));
                Assert.True(form.TrySaveMap(false, out var error), error?.ToString());
                Assert.Equal(bootstrap, File.ReadAllBytes(script));
            }
            using var reload = new MapEditorForm(game, new GameMapInfo(created.Id, map, true, created.DisplayName, "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(reload, true);
            reload.StartPosition = FormStartPosition.Manual; reload.Location = new Point(-30000, -30000); reload.Show(); Application.DoEvents();
            Assert.Empty(reload.PlacementSession.Capture()); Assert.Empty(LevelObjectStore.Load(map).Objects());
            var type = GetField<IReadOnlyList<SdlObjectType>>(reload, "_objectCatalog").First(item => item.Category == SdlObjectCategory.Figure);
            reload.PlacementSession.Add(new SdlPlacedObject(type, 8192, 200, 8192, 0, 90, 10) { ScenarioId = Guid.NewGuid() });
            GetField<List<ScenarioEvent>>(reload, "_events").Add(new("New event", 2) { Actions = [new(ScenarioActionKind.Message, "Empty scene ready")] });
            Assert.True(reload.TrySaveMap(false, out var spawnError), spawnError?.ToString());
            var authored = ScenarioDocument.Load(map);
            Assert.Equal(10, Assert.Single(authored.Spawns).Count); Assert.Single(authored.Events);
            BciImage injected = BciImage.Parse(GameLZSS.DecompressPfil(File.ReadAllBytes(script)));
            Assert.DoesNotContain(Enumerable.Range(0, injected.ConstOffsets.Count).Select(injected.Constant), constant => constant == "s_setVillageTemplate");
            Assert.Equal(bootstrap, File.ReadAllBytes(Path.Combine(map, "SCRIPT", LevelScriptInjector.OriginalBackupFile)));
            using (var fresh = new MapEditorForm(game, new GameMapInfo(created.Id, map, true, created.DisplayName, "Test")))
            {
                typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(fresh, true);
                fresh.StartPosition = FormStartPosition.Manual; fresh.Location = new Point(-30000, -30000); fresh.Show(); Application.DoEvents();
                Assert.Equal(10, Assert.Single(fresh.PlacementSession.Capture()).UnitCount);
                Assert.True(fresh.TrySaveMap(false, out var freshError), freshError?.ToString());
                Assert.Equal(injected.Serialize(), BciImage.Parse(GameLZSS.DecompressPfil(File.ReadAllBytes(script))).Serialize());
            }
            // Removing all authored content must restore the new empty bootstrap, not the inherited source main.
            reload.PlacementSession.RemoveAt(0); GetField<List<ScenarioEvent>>(reload, "_events").Clear();
            Assert.True(reload.TrySaveMap(false, out var clearError), clearError?.ToString());
            Assert.Equal(bootstrap, File.ReadAllBytes(script)); Assert.Empty(ScenarioDocument.Load(map).Spawns);
            foreach (var language in new[] { Language.TraditionalChinese, Language.English })
            {
                Loc.OverrideLanguageForTesting(language);
                using var selection = new MapSelectionForm(game) { Width = 900, Height = 600, StartPosition = FormStartPosition.Manual, Location = new Point(-30000, -30000) };
                selection.Show(); Application.DoEvents();
                using var shot = new Bitmap(selection.Width, selection.Height); selection.DrawToBitmap(shot, new Rectangle(Point.Empty, shot.Size));
                shot.Save(Path.Combine(output, $"selection-{language}.png"));
                using var dialog = MapSelectionForm.BuildNameDialog("Empty Scene", "Empty Scene", BlankMapBuilder.Describe("ENDL_000", language == Language.English),
                    language == Language.English, selection.BackColor, selection.ForeColor, out _);
                dialog.StartPosition = FormStartPosition.Manual; dialog.Location = new Point(-30000, -30000); dialog.Show(); Application.DoEvents();
                using var review = new Bitmap(dialog.Width, dialog.Height); dialog.DrawToBitmap(review, new Rectangle(Point.Empty, review.Size));
                review.Save(Path.Combine(output, $"review-{language}.png"));
            }
            Loc.OverrideLanguageForTesting(Language.TraditionalChinese);
        });
        // Failure after staging writes must remove only the new slot, restore its manifest, and preserve the completed blank map.
        var validMap = DemoHashes(map);
        Assert.Throws<InvalidDataException>(() => new EndlessMapCloner().ClonePrepared(game, "ENDL_000", 6, "Rejected", staging =>
        { File.WriteAllText(Path.Combine(staging, "partial.txt"), "partial"); throw new InvalidDataException("Injected preparation failure"); }));
        Assert.False(Directory.Exists(Path.Combine(game, "MAPS", "ENDL_006"))); Assert.False(Directory.Exists(Path.Combine(game, "MAPS", "ENDL_006.tmp_arm")));
        Assert.Equal(validMap, DemoHashes(map)); Assert.Equal(templateBefore, DemoHashes(template));
        byte[] runtime = File.ReadAllBytes(Path.Combine(template, "DATA", "anim.dat"));
        byte[] damaged = GameLZSS.DecompressPfil(runtime); BinaryPrimitives.WriteInt32LittleEndian(damaged, 2);
        File.WriteAllBytes(Path.Combine(template, "DATA", "anim.dat"), GameLZSS.CompressPfil(damaged, runtime.AsSpan(0, 64).ToArray()));
        Assert.Throws<InvalidDataException>(() => BlankMapBuilder.Create(game, "ENDL_000", 6, "Invalid layout"));
        Assert.False(Directory.Exists(Path.Combine(game, "MAPS", "ENDL_006"))); Assert.False(Directory.Exists(Path.Combine(game, "MAPS", "ENDL_006.tmp_arm")));
        Assert.Equal(validMap, DemoHashes(map));
        File.WriteAllBytes(Path.Combine(template, "DATA", "anim.dat"), runtime); Assert.Equal(templateBefore, DemoHashes(template));
        EndlessMapInfo copy = new EndlessMapCloner().Clone(game, created.Id, 7, "Empty Scene Copy");
        Assert.True(CustomMapManifest.HasStandaloneLevel(copy.DirectoryPath));
        Assert.True(CustomMapManifest.Load(game).Entries.Single(entry => entry.Slot == 7).StandaloneLevel);
        Assert.Equal(bootstrap, File.ReadAllBytes(Path.Combine(copy.DirectoryPath, "SCRIPT", LevelScriptInjector.ScriptFile)));
        Assert.Equal(1, EndlessAiOrchestrator.GetExpectedFileCount(game, "MAPS/ENDL_*/SCRIPT/ak_level.bci"));
        Assert.Equal(hashes, DemoHashes(source));
        File.WriteAllText(Path.Combine(output, "source-hashes.json"), JsonSerializer.Serialize(hashes, Core.Services.JsonDefaults.Indented));
        File.WriteAllText(Path.Combine(output, "result.json"), JsonSerializer.Serialize(new { Status = "editor-verified-game-startup-pending", created.Id,
            Objects = 0, SdlObjects = 0, Spawns = 0, Events = 0, NativePoolsReset = pools.Length, SourceUnchanged = true, StagingFailureVerified = true }, Core.Services.JsonDefaults.Indented));
    }
}
