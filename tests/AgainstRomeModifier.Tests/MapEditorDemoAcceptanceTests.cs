using System.Drawing;
using System.Reflection;
using System.Security.Cryptography;
using System.Text.Json;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.Nature;
using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    /// <summary>Opt-in acceptance using an existing TEMP asset copy; retains a new isolated demo and evidence.</summary>
    [Fact]
    public void Demo_real_copy_edits_save_and_fresh_reload_preserve_all_layers_and_event_targets()
    {
        if (Environment.GetEnvironmentVariable("ARM_DEMO_ACCEPTANCE") != "1") return;
        string source = RequireDemoTempPath(Environment.GetEnvironmentVariable("ARM_COMPARE_GAME"));
        string output = RequireDemoTempPath(Environment.GetEnvironmentVariable("ARM_DEMO_OUTPUT"));
        Assert.False(Directory.Exists(output), "Use a new output directory; existing work must not be overwritten.");
        Assert.False(output.StartsWith(source + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase));
        string originalMap = Path.Combine(source, "ENDL_005");
        var sourceHashes = DemoHashes(originalMap);
        string game = Path.Combine(output, "game"), map = Path.Combine(game, "MAPS", "ENDL_005");
        CopyDemoDirectory(originalMap, map);
        CopyDemoDirectory(Path.Combine(source, "ENDL_000"), Path.Combine(game, "MAPS", "ENDL_000"));
        CopyDemoDirectory(Path.Combine(source, "SYSTEM"), Path.Combine(game, "SYSTEM"));
        foreach (string asset in new[] { "floortex.dat", "alr.dat", "apt.dat", "shad.dat" })
        {
            Assert.False(File.GetAttributes(Path.Combine(source, asset)).HasFlag(FileAttributes.ReparsePoint));
            File.Copy(Path.Combine(source, asset), Path.Combine(game, asset));
        }

        RunInSta(() =>
        {
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(game, new GameMapInfo("ENDL_005", map, true, "Demo", "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            form.Show(); Application.DoEvents();
            var before = SnapshotDirectory(map);
            var layers = GetField<TerrainHeightEditSession>(form, "_terrainLayers");
            byte[] originalHeights = layers.Heights.ToArray();
            string[] originalTextures = GetField<BodenTexturesDocument>(form, "_texturesDocument").Textures.ToArray();
            var brush = GetField<ComboBox>(form, "_brushSize");
            brush.SelectedIndex = 2;
            SetWorkflowMode(form, "Texture");
            var palette = GetField<ListBox>(form, "_palette");
            palette.SelectedItem = palette.Items.Cast<object>().Single(item => item.ToString()!.Contains("土路") || item.ToString()!.Contains("Dirt Path"));
            form.RegionDialogRunner = dialog =>
            {
                dialog.Operation = TerrainRegionOperation.Road;
                dialog.VerticesText = "34,33\r\n44,33\r\n44,37"; dialog.RoadWidth = 3;
                return DialogResult.OK;
            };
            form.RunRegionTool();
            var textures = GetField<BodenTexturesDocument>(form, "_texturesDocument");
            Assert.Contains(textures.Textures, name => name.StartsWith("PFAD", StringComparison.OrdinalIgnoreCase));
            string[] nextTextures = textures.Textures.ToArray();
            Invoke(form, "Undo"); Assert.Equal(originalTextures, textures.Textures);
            Invoke(form, "Redo"); Assert.Equal(nextTextures, textures.Textures);

            SetWorkflowMode(form, "Height");
            GetField<ToolStripComboBox>(form, "_terrainOperation").SelectedIndex = (int)TerrainHeightOperation.Raise;
            for (int pass = 0; pass < 4; pass++) Invoke(form, "PaintTexture", new TexturePaintEventArgs(46, 23, "", ""));
            Invoke(form, "CommitStroke");
            Assert.False(originalHeights.SequenceEqual(layers.Heights));
            GetField<ToolStripComboBox>(form, "_terrainOperation").SelectedIndex = (int)TerrainHeightOperation.Roughen + 1;
            for (int pass = 0; pass < 12; pass++) Invoke(form, "PaintTexture", new TexturePaintEventArgs(30, 28, "", ""));
            Invoke(form, "CommitStroke");
            int lakeX = (int)(30.5f * (layers.VertexSize - 1) / 64), lakeY = (int)(28.5f * (layers.VertexSize - 1) / 64);
            decimal water = GetField<NumericUpDown>(form, "_waterLevel").Value;
            Assert.True(layers.Heights[lakeY * layers.VertexSize + lakeX] * 4 < water, "Lake center must be below water level (Heightmapstep=4).");

            SetWorkflowMode(form, "Nature");
            var natureTask = GetField<Task>(form, "_natureCatalogTask");
            var types = GetField<ListBox>(form, "_natureTypes");
            DateTime deadline = DateTime.UtcNow.AddSeconds(45);
            while ((!natureTask.IsCompleted || types.Items.Count == 0 || types.Items[0] is string) && DateTime.UtcNow < deadline)
            { Application.DoEvents(); Thread.Sleep(20); }
            Assert.True(natureTask.IsCompletedSuccessfully);
            var category = GetField<ComboBox>(form, "_natureCategory");
            category.SelectedIndex = Enumerable.Range(0, category.Items.Count).Single(index =>
                category.Items[index]!.GetType().GetProperty("Value")!.GetValue(category.Items[index]) as string == "tree");
            Assert.True(types.Items.Count > 0); types.SelectedIndex = 0;
            // Clear this demonstration patch first so random scattering cannot reject every point near existing trees.
            GetField<ComboBox>(form, "_natureOperation").SelectedIndex = 1;
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(28, 43, "", ""));
            Invoke(form, "CommitStroke");
            GetField<ComboBox>(form, "_natureOperation").SelectedIndex = 0;
            GetField<ComboBox>(form, "_natureDensity").SelectedIndex = 2;
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(28, 43, "", ""));
            Invoke(form, "CommitStroke");
            var nature = GetField<NatureEditSession>(form, "_natureSession");
            int originalPlanted = nature.Additions.Count; Assert.True(originalPlanted > 0);
            var forestLayout = form.CaptureNatureLayout(new Rectangle(26, 41, 5, 5));
            string forestFile = Path.Combine(output, "forest.arm-layout.json");
            File.WriteAllText(forestFile, MapLayoutPresets.Serialize(forestLayout));
            // Import must resolve official species even when no Nature catalog is already loaded.
            typeof(MapEditorForm).GetField("_layoutNativeTemplates", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, new Dictionary<int, LevelObjectTemplate>());
            typeof(MapEditorForm).GetField("_natureTemplates", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, new Dictionary<int, LevelObjectTemplate>());
            typeof(MapEditorForm).GetField("_natureCatalogTask", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, null);
            form.ApplyLayout(MapLayoutPresets.Deserialize(File.ReadAllText(forestFile)), 13 * 256, 12 * 256, 90);
            typeof(MapEditorForm).GetField("_layoutNativeTemplates", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, null);
            Assert.Equal(originalPlanted + forestLayout.Entries.Count, nature.Additions.Count);
            Invoke(form, "Undo"); Assert.Equal(originalPlanted, nature.Additions.Count);
            Invoke(form, "Redo"); Assert.Equal(originalPlanted + forestLayout.Entries.Count, nature.Additions.Count);
            int planted = nature.Additions.Count, removed = nature.RemovedSlots.Count;
            int worldBefore = LevelObjectStore.Load(map).Objects().Count;

            var placed = form.PlacementSession.Capture();
            int buildingIndex = Enumerable.Range(0, placed.Count).First(index => placed[index].Type.Category == SdlObjectCategory.Building && placed[index].Team == 0);
            var settlementLayout = form.CapturePlacementLayout([buildingIndex]);
            string settlementFile = Path.Combine(output, "settlement.arm-layout.json");
            File.WriteAllText(settlementFile, MapLayoutPresets.Serialize(settlementLayout));
            form.ApplyLayout(MapLayoutPresets.Deserialize(File.ReadAllText(settlementFile)), 8 * 256, 8 * 256, 90, 0);
            Assert.Equal(placed.Count + 1, form.PlacementSession.Count);
            Invoke(form, "Undo"); Assert.Equal(placed.Count, form.PlacementSession.Count);
            Invoke(form, "Redo"); Assert.Equal(placed.Count + 1, form.PlacementSession.Count);
            placed = form.PlacementSession.Capture();
            Guid copiedBuildingId = placed[^1].ScenarioId;
            int unitIndex = Enumerable.Range(0, placed.Count).First(index => placed[index].Team == 0 && placed[index].Type.Category == SdlObjectCategory.Figure);
            var unit = placed[unitIndex];
            form.PlacementSession.Edit(unitIndex, 0, unit.WorldX, unit.WorldY, unit.WorldZ, 90, 10);
            Assert.Contains(placed, item => item.Team == 0 && item.Type.Category == SdlObjectCategory.Building);
            var events = GetField<List<ScenarioEvent>>(form, "_events");
            events.Clear();
            events.Add(new("Demo welcome", 3) { Actions = [new(ScenarioActionKind.Message, "Follow the dirt path east. Move the starting troop into the goal area to win.")] });
            events.Add(new("Demo goal", 1) {
                Conditions = [new(ScenarioConditionKind.ObjectInArea, unit.ScenarioId, 11000, 8600, 12000, 9700)],
                Actions = [new(ScenarioActionKind.Message, "Goal reached."), new(ScenarioActionKind.Victory)] });
            GetField<TextBox>(form, "_title").Text = "ARM Demo - Path to Victory";
            Invoke(form, "RefreshEventList", 0); Invoke(form, "UpdateEditorState");
            AssertSnapshotUnchanged(map, before);
            byte[] nextHeights = layers.Heights.ToArray(), nextCollision = layers.Collision!.ToArray();
            NatureAddition[] expectedNature = nature.Additions.ToArray();
            Assert.NotEmpty(expectedNature);
            Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
            Assert.False(GetProperty<bool>(form, "IsDirty"));
            var saved = ScenarioDocument.Load(map);
            Assert.Equal(2, saved.Events.Count);
            Assert.Equal(unit.ScenarioId, saved.Events[1].Conditions[0].TargetId);
            Assert.Equal(10, Assert.Single(saved.Spawns, item => item.Id == unit.ScenarioId).Count);
            Assert.Equal(4, saved.DataSlots.Count); // original three buildings plus one reusable settlement copy
            var savedWorld = LevelObjectStore.Load(map).Objects();
            Assert.Equal(worldBefore - removed + planted + 1, savedWorld.Count);
            foreach (var addition in expectedNature)
                Assert.Contains(savedWorld, item => item.TypeId == addition.Template.TypeId && item.X == addition.X && item.Y == addition.Y && item.Z == addition.Z && item.Rotation == addition.Rotation);
            var copiedBinding = Assert.Single(saved.DataSlots, binding => binding.SpawnId == copiedBuildingId);
            var copiedBuilding = Assert.Single(savedWorld, item => item.Uid == copiedBinding.Uid && item.Slot == copiedBinding.Slot);
            Assert.Equal(8 * 256, copiedBuilding.X); Assert.Equal(8 * 256, copiedBuilding.Z); Assert.Equal(0, copiedBuilding.Team);
            Assert.Equal(MathF.PI / 2, copiedBuilding.Rotation, 5);
            var savedBytes = SnapshotDirectory(map);
            Assert.True(form.TrySaveMap(false, out error), error?.ToString()); AssertSnapshotUnchanged(map, savedBytes);

            using var reopened = new MapEditorForm(game, new GameMapInfo("ENDL_005", map, true, "Demo", "Test"));
            _ = reopened.Handle; Invoke(reopened, "LoadSelectedMap");
            var fresh = GetField<TerrainHeightEditSession>(reopened, "_terrainLayers");
            Assert.Equal(nextHeights, fresh.Heights); Assert.Equal(nextCollision, fresh.Collision);
            Assert.Equal(nextTextures, GetField<BodenTexturesDocument>(reopened, "_texturesDocument").Textures);
            Assert.Equal(placed.Select(item => item.ScenarioId), reopened.PlacementSession.Capture().Select(item => item.ScenarioId));
            Assert.Equal(unit.ScenarioId, GetField<List<ScenarioEvent>>(reopened, "_events")[1].Conditions[0].TargetId);
            Assert.Equal(savedWorld, GetField<IReadOnlyList<LevelWorldObject>>(reopened, "_levelObjects"));
            Assert.False(GetProperty<bool>(reopened, "IsDirty")); Assert.False(fresh.CanUndo);
            Assert.True(reopened.TrySaveMap(false, out error), error?.ToString()); AssertSnapshotUnchanged(map, savedBytes);
            Invoke(form, "SetActiveView", true); Application.DoEvents();
            var view = GetField<Map3DViewControl>(form, "_view3d");
            Assert.True(view.IsReady, "Demo acceptance requires a real OpenGL context.");
            view.FocusTile(39, 33);
            using var image = view.CaptureFrame(1280, 800)!; image.Save(Path.Combine(output, "demo-3d.png"));
            File.WriteAllText(Path.Combine(output, "result.json"), JsonSerializer.Serialize(new {
                Map = map, Planted = planted, Spawns = saved.Spawns.Count, Buildings = saved.DataSlots.Count,
                LakeHeight = nextHeights[lakeY * layers.VertexSize + lakeX], WaterLevel = water,
                Verified = "native path undo/redo, hill, lake, forest/settlement JSON layouts with batch undo/redo, troop, event target, save, fresh reload, repeated save bytes, OpenGL capture",
                Pending = "gameplay, resource usability, pathfinding, building completion in game, victory execution"
            }));
        }, TimeSpan.FromMinutes(3));
        Assert.Equal(sourceHashes, DemoHashes(originalMap));
        File.WriteAllText(Path.Combine(output, "source-hashes.json"), JsonSerializer.Serialize(sourceHashes));
    }

    private static string RequireDemoTempPath(string? path)
    {
        Assert.False(string.IsNullOrWhiteSpace(path));
        string full = Path.GetFullPath(path!);
        Assert.True(full.StartsWith(Path.GetFullPath(Path.GetTempPath()).TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar,
            StringComparison.OrdinalIgnoreCase), "Demo acceptance only accesses TEMP copies.");
        for (string? parent = full; parent is not null; parent = Path.GetDirectoryName(parent))
            if (Directory.Exists(parent)) Assert.False(File.GetAttributes(parent).HasFlag(FileAttributes.ReparsePoint));
        return full;
    }

    private static Dictionary<string, string> DemoHashes(string map)
    {
        var hashes = new Dictionary<string, string>();
        void Visit(string directory)
        {
            Assert.False(File.GetAttributes(directory).HasFlag(FileAttributes.ReparsePoint));
            foreach (string file in Directory.GetFiles(directory).Order(StringComparer.Ordinal))
            {
                Assert.False(File.GetAttributes(file).HasFlag(FileAttributes.ReparsePoint));
                hashes.Add(Path.GetRelativePath(map, file), Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(file))));
            }
            foreach (string child in Directory.GetDirectories(directory).Order(StringComparer.Ordinal)) Visit(child);
        }
        Visit(map);
        return hashes;
    }

    private static void CopyDemoDirectory(string source, string target)
    {
        Assert.False(File.GetAttributes(source).HasFlag(FileAttributes.ReparsePoint));
        Directory.CreateDirectory(target);
        foreach (string file in Directory.GetFiles(source))
        {
            Assert.False(File.GetAttributes(file).HasFlag(FileAttributes.ReparsePoint));
            File.Copy(file, Path.Combine(target, Path.GetFileName(file)));
        }
        foreach (string directory in Directory.GetDirectories(source))
            CopyDemoDirectory(directory, Path.Combine(target, Path.GetFileName(directory)));
    }
}
