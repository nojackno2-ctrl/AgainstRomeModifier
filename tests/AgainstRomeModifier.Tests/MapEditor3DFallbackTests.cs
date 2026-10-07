using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Restored_height_resource_reenables_3d_after_reload_without_losing_saved_edits()
    {
        string map = CreateFixture();
        string height = Path.Combine(map, "boden.bmp");
        string backup = height + ".fixture-backup";
        File.Move(height, backup);
        RunInSta(() =>
        {
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.ThrowException, threadScope: true);
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Restored height", "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new System.Drawing.Point(-30000, -30000);
            form.Show(); Application.DoEvents();
            var view = GetField<Map3DViewControl>(form, "_view3d");
            if (!view.IsReady) { Assert.NotEqual("1", Environment.GetEnvironmentVariable("ARM_OPENGL_REQUIRED")); return; }
            var button = GetField<ToolStripButton>(form, "_view3dButton");
            Assert.False(button.Enabled);
            Assert.True(GetField<ToolStripButton>(form, "_3dDiagnosticsButton").Visible);
            GetField<TextBox>(form, "_title").Text = "Saved in fallback";
            Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
            File.Move(backup, height);
            Invoke(form, "LoadSelectedMap");
            Assert.True(button.Enabled);
            Assert.False(GetField<ToolStripButton>(form, "_3dDiagnosticsButton").Visible);
            Assert.Null(GetField<string?>(form, "_last3DDiagnostic"));
            Invoke(form, "SetActiveView", true);
            Assert.True(button.Checked);
            Assert.True(view.Visible);
            Assert.False(GetField<MapCanvasControl>(form, "_canvas").Visible);
            using var frame = view.CaptureFrame(64, 64);
            Assert.NotNull(frame);
            Assert.Equal("Saved in fallback", GetField<TextBox>(form, "_title").Text);
            Assert.False(GetProperty<bool>(form, "IsDirty"));
        });
    }

    [Fact]
    public void Missing_floor_texture_library_disables_3d_with_diagnostics_and_keeps_2d_editing_and_save()
    {
        string map = CreateFixture();
        File.Delete(Path.Combine(_root, "floortex.dat"));
        RunInSta(() =>
        {
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.ThrowException, threadScope: true);
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false; // 測試在非主執行緒的 STA
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "No floortex", "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new System.Drawing.Point(-30000, -30000);
            form.Show(); Application.DoEvents(); // Shown 事件載入地圖

            var view3d = GetField<ToolStripButton>(form, "_view3dButton");
            var view2d = GetField<ToolStripButton>(form, "_view2dButton");
            var diagnostics = GetField<ToolStripButton>(form, "_3dDiagnosticsButton");
            Assert.False(view3d.Enabled);
            Assert.True(view2d.Checked);
            Assert.True(GetField<MapCanvasControl>(form, "_canvas").Visible);
            Assert.True(diagnostics.Visible);
            string report = GetField<string>(form, "_last3DDiagnostic");
            Assert.Contains("floortex.dat", report);
            Assert.Contains("缺少", report);
            Assert.Contains("floortex.dat", GetField<Label>(form, "_modeBanner").Text);

            Invoke(form, "SetActiveView", true); // 停用後要求 3D 仍維持 2D
            Assert.True(view2d.Checked);

            var layers = GetField<TerrainHeightEditSession>(form, "_terrainLayers");
            byte[] before = layers.Heights.ToArray();
            Type mode = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
            Invoke(form, "SetEditMode", Enum.Parse(mode, "Height"));
            for (int x = 20; x <= 30; x++) Invoke(form, "PaintTexture", new TexturePaintEventArgs(x, 20, "", ""));
            Invoke(form, "CommitStroke");
            Assert.NotEqual(before, layers.Heights);
            byte[] edited = layers.Heights.ToArray();
            Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());

            using var reopened = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Reopen", "Test"));
            _ = reopened.Handle; Invoke(reopened, "LoadSelectedMap");
            Assert.Equal(edited, GetField<TerrainHeightEditSession>(reopened, "_terrainLayers").Heights);
        });
    }
}
