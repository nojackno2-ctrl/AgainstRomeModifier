using System.Drawing;
using System.IO.Compression;
using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Display_retry_recreates_released_gl_resources_and_preserves_the_dirty_scene()
    {
        string map = CreateFixture();
        RunInSta(() =>
        {
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.ThrowException, threadScope: true);
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Context retry", "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            form.Show(); Application.DoEvents();
            var view = GetField<Map3DViewControl>(form, "_view3d");
            if (!view.IsReady) { Assert.NotEqual("1", Environment.GetEnvironmentVariable("ARM_OPENGL_REQUIRED")); return; }
            Type mode = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
            Invoke(form, "SetEditMode", Enum.Parse(mode, "Height"));
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(32, 32, "", ""));
            Invoke(form, "CommitStroke");
            GetField<TextBox>(form, "_title").Text = "Pending context retry";
            Invoke(form, "SetEditMode", Enum.Parse(mode, "Collision"));
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(35, 35, "", ""));
            Invoke(form, "CommitStroke");
            var layers = GetField<TerrainHeightEditSession>(form, "_terrainLayers");
            byte[] edited = layers.Heights.ToArray();
            byte[] collision = layers.Collision!.ToArray();
            Dictionary<string, byte[]> disk = SnapshotDirectory(map);
            using Bitmap before = view.CaptureFrame(640, 480)!;
            Assert.NotNull(before);
            typeof(Map3DViewControl).GetMethod("ReleaseGlResources", BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(view, []);
            Assert.False(view.IsReady);
            Invoke(form, "Disable3DView", "Test released OpenGL resources", null!);
            GetField<ToolStripButton>(form, "_retryDisplayButton").PerformClick();
            Application.DoEvents();
            Assert.True(view.IsReady);
            Assert.True(GetField<ToolStripButton>(form, "_view3dButton").Enabled);
            Assert.Same(layers, GetField<TerrainHeightEditSession>(form, "_terrainLayers"));
            Assert.Equal(edited, layers.Heights);
            Assert.Equal("Pending context retry", GetField<TextBox>(form, "_title").Text);
            Assert.True(GetProperty<bool>(form, "IsDirty"));
            Invoke(form, "SetActiveView", true); Application.DoEvents();
            using Bitmap after = view.CaptureFrame(640, 480)!;
            Assert.NotNull(after);
            Assert.Equal(0, Changed(before, after));
            Invoke(form, "Undo"); Invoke(form, "Redo"); Assert.Equal(edited, layers.Heights); Assert.Equal(collision, layers.Collision);
            AssertFilesEqual(disk, SnapshotDirectory(map));
        });
    }

    [Fact]
    public void Display_retry_loads_restored_archive_without_reloading_map_or_discarding_edits()
    {
        string map = CreateFixture();
        string archive = Path.Combine(_root, "floortex.dat"), backup = archive + ".fixture-backup";
        File.Move(archive, backup);
        RunInSta(() =>
        {
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.ThrowException, threadScope: true);
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Retry", "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            form.Show(); Application.DoEvents();
            var view = GetField<Map3DViewControl>(form, "_view3d");
            if (!view.IsReady) { Assert.NotEqual("1", Environment.GetEnvironmentVariable("ARM_OPENGL_REQUIRED")); return; }
            var layers = GetField<TerrainHeightEditSession>(form, "_terrainLayers");
            byte[] original = layers.Heights.ToArray();
            Type mode = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
            Invoke(form, "SetEditMode", Enum.Parse(mode, "Height"));
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(32, 32, "", ""));
            Invoke(form, "CommitStroke");
            byte[] edited = layers.Heights.ToArray();
            GetField<TextBox>(form, "_title").Text = "Unsaved title";
            Dictionary<string, byte[]> disk = SnapshotDirectory(map);
            File.Move(backup, archive);
            GetField<ToolStripButton>(form, "_retryDisplayButton").PerformClick();
            Assert.Same(layers, GetField<TerrainHeightEditSession>(form, "_terrainLayers"));
            Assert.Equal(edited, layers.Heights);
            Assert.Equal("Unsaved title", GetField<TextBox>(form, "_title").Text);
            Assert.True(GetProperty<bool>(form, "IsDirty"));
            Assert.True(GetField<ToolStripButton>(form, "_view3dButton").Enabled);
            Assert.False(GetField<ToolStripButton>(form, "_3dDiagnosticsButton").Visible);
            Assert.NotNull(GetField<TerrainBlendEditSession>(form, "_terrainBlendSession"));
            Assert.NotEmpty(GetField<ListBox>(form, "_palette").Items.Cast<object>());
            AssertFilesEqual(disk, SnapshotDirectory(map));
            Invoke(form, "Undo"); Assert.Equal(original, layers.Heights);
            Invoke(form, "Redo"); Assert.Equal(edited, layers.Heights);
            Invoke(form, "SetActiveView", true);
            using var frame = view.CaptureFrame(64, 64); Assert.NotNull(frame);
            Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
            Invoke(form, "LoadSelectedMap");
            Assert.Equal(edited, GetField<TerrainHeightEditSession>(form, "_terrainLayers").Heights);
            Assert.Equal("Unsaved title", GetField<TextBox>(form, "_title").Text);
            Assert.False(GetProperty<bool>(form, "IsDirty"));
        });
    }

    [Fact]
    public void Archive_retry_rejects_corruption_keeps_history_then_accepts_replacement_and_new_tiles()
    {
        string map = CreateFixture();
        string archive = Path.Combine(_root, "floortex.dat");
        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Retry", "Test"));
            _ = form.Handle; Invoke(form, "LoadSelectedMap");
            GetField<CheckBox>(form, "_stampMode").Checked = true;
            Invoke(form, "SelectSampledTexture", "4BB___52");
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(5, 6, "", ""));
            Invoke(form, "CommitStroke");
            var session = GetField<TerrainBlendEditSession>(form, "_terrainBlendSession");
            var library = GetField<FloorTextureLibrary>(form, "_floorTextures");
            string[] edited = session.CurrentTextures.ToArray();
            Dictionary<string, byte[]> disk = SnapshotDirectory(map);
            string oldArchive = archive + ".fixture-old";
            File.Move(archive, oldArchive);
            File.WriteAllText(archive, "Not a ZIP");
            object[] args = [null!];
            Assert.False((bool)Invoke(form, "TryReloadDisplayResources", args)!);
            Assert.IsAssignableFrom<Exception>(args[0]);
            Assert.Same(session, GetField<TerrainBlendEditSession>(form, "_terrainBlendSession"));
            Assert.Same(library, GetField<FloorTextureLibrary>(form, "_floorTextures"));
            Assert.Equal(edited, session.CurrentTextures);
            Assert.NotNull(library.Get("4BB___51"));
            Invoke(form, "Undo"); Invoke(form, "Redo"); Assert.Equal(edited, session.CurrentTextures);
            File.Move(archive, archive + ".fixture-bad");
            File.Copy(oldArchive, archive);
            using (var zip = ZipFile.Open(archive, ZipArchiveMode.Update))
            using (Stream stream = zip.CreateEntry("SYSTEM/DATA/FLOORTEXTURE/broken.bmp").Open())
                stream.Write(new byte[] { 1, 2, 3 });
            args = [null!];
            Assert.False((bool)Invoke(form, "TryReloadDisplayResources", args)!);
            Assert.IsAssignableFrom<Exception>(args[0]);
            Assert.Equal(edited, session.CurrentTextures);
            Assert.NotNull(library.Get("4BB___51"));
            File.Move(archive, archive + ".fixture-bad-bitmap");
            File.Copy(oldArchive, archive);
            using (var zip = ZipFile.Open(archive, ZipArchiveMode.Update))
            using (Stream stream = zip.CreateEntry("SYSTEM/DATA/FLOORTEXTURE/weg1.bmp").Open())
                stream.Write(Encode(128, Enumerable.Repeat((byte)60, 128 * 128).ToArray()));
            args = [null!];
            Assert.True((bool)Invoke(form, "TryReloadDisplayResources", args)!, args[0]?.ToString());
            Assert.Null(args[0]);
            Assert.Same(session, GetField<TerrainBlendEditSession>(form, "_terrainBlendSession"));
            Assert.Same(library, GetField<FloorTextureLibrary>(form, "_floorTextures"));
            Assert.Contains("weg1", library.Names);
            Assert.Equal(edited, session.CurrentTextures);
            AssertFilesEqual(disk, SnapshotDirectory(map));
            Invoke(form, "SelectSampledTexture", "weg1");
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(30, 30, "", ""));
            Invoke(form, "CommitStroke");
            Assert.Equal("weg1", GetField<BodenTexturesDocument>(form, "_texturesDocument").GetTexture(30, 30));
            Invoke(form, "Undo"); Assert.Equal(edited, session.CurrentTextures);
            Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
        });
    }

    private static void AssertFilesEqual(Dictionary<string, byte[]> expected, Dictionary<string, byte[]> actual)
    {
        Assert.Equal(expected.Keys.OrderBy(p => p), actual.Keys.OrderBy(p => p));
        foreach (string key in expected.Keys) Assert.Equal(expected[key], actual[key]);
    }
}
