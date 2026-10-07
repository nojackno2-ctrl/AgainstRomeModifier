using System.Drawing;
using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Real_opengl_passability_overlay_tracks_edit_history_mode_and_saved_map()
    {
        string map = CreateFixture();
        string output = Path.Combine(Path.GetTempPath(), "ArmCollisionGl_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(output);
        RunInSta(() =>
        {
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.ThrowException, threadScope: true);
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Collision", "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            form.Show(); Application.DoEvents();
            Invoke(form, "SetActiveView", true); Application.DoEvents();
            var view = GetField<Map3DViewControl>(form, "_view3d");
            if (!view.IsReady)
            {
                Assert.NotEqual("1", Environment.GetEnvironmentVariable("ARM_OPENGL_REQUIRED"));
                Assert.False(string.IsNullOrWhiteSpace(view.LastFailureReason));
                return;
            }
            Type mode = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
            Invoke(form, "SetEditMode", Enum.Parse(mode, "Collision"));
            GetField<CheckBox>(form, "_showGrid").Checked = false;
            GetField<CheckBox>(form, "_showObjects").Checked = false;
            using Bitmap baseline = Capture(view, output, "1-clear");
            // 非對稱遮罩驗證 GL 取樣方向，避免中央對稱筆畫掩蓋 X/Z 或 Y 翻轉。
            byte[] mask = new byte[256 * 256];
            for (int y = 0; y < 128; y++) for (int x = 0; x < 128; x++) mask[y * 256 + x] = 1;
            view.SetCollisionOverlay(256, mask);
            Array.Clear(mask); // 控制項須擁有隔離快照。
            using Bitmap quadrant = Capture(view, output, "0-orientation");
            MethodInfo pick = typeof(Map3DViewControl).GetMethod("TryGetTile", BindingFlags.Instance | BindingFlags.NonPublic)!;
            int blockedChecks = 0, clearChecks = 0;
            Size oldSize = view.ClientSize; view.ClientSize = new Size(640, 480);
            for (int y = 24; y < 456; y += 12) for (int x = 24; x < 616; x += 12)
            {
                object[] args = [new Point(x, y), 0, 0];
                if (!(bool)pick.Invoke(view, args)!) continue;
                int tileX = (int)args[1], tileY = (int)args[2];
                if (tileX < 2 || tileY < 2 || tileX > 61 || tileY > 61) continue;
                if (tileX is >= 30 and <= 33 || tileY is >= 30 and <= 33) continue;
                int distance = Distance(baseline.GetPixel(x, y), quadrant.GetPixel(x, y));
                if (tileX < 30 && tileY < 30) { Assert.True(distance > 12, $"Pixel {x},{y} tile {tileX},{tileY} distance {distance}, viewport {view.ClientSize}"); blockedChecks++; }
                else { Assert.Equal(0, distance); clearChecks++; }
            }
            Assert.True(blockedChecks > 10 && clearChecks > 10);
            view.ClientSize = oldSize;
            view.SetCollisionOverlay(256, new byte[1]);
            using Bitmap invalidCleared = Capture(view, output, "0-invalid-cleared");
            Assert.Equal(0, Changed(baseline, invalidCleared));
            Invoke(form, "SetEditMode", Enum.Parse(mode, "Collision"));
            var size = GetField<ComboBox>(form, "_brushSize"); size.SelectedIndex = size.Items.Count - 1;
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(32, 32, "", ""));
            Invoke(form, "CommitStroke");
            using Bitmap blocked = Capture(view, output, "2-blocked");
            int delta = Changed(baseline, blocked);
            Assert.True(delta > 500, $"3D blocked overlay changed only {delta} pixels.");
            Assert.True(Average(blocked, c => c.R - c.G) > Average(baseline, c => c.R - c.G) + 1);
            Invoke(form, "Undo");
            using Bitmap undone = Capture(view, output, "3-undo");
            Assert.Equal(0, Changed(baseline, undone));
            Invoke(form, "Redo");
            using Bitmap redone = Capture(view, output, "4-redo");
            Assert.Equal(0, Changed(blocked, redone));
            Invoke(form, "SetEditMode", Enum.Parse(mode, "Texture"));
            using Bitmap hidden = Capture(view, output, "5-hidden");
            Assert.Equal(0, Changed(baseline, hidden));
            Invoke(form, "SetEditMode", Enum.Parse(mode, "Collision"));
            Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
            Invoke(form, "LoadSelectedMap");
            using Bitmap reopened = Capture(view, output, "6-reopened");
            Assert.Equal(0, Changed(blocked, reopened));
            GetField<ToolStripComboBox>(form, "_terrainOperation").SelectedIndex = 1;
            Invoke(form, "PaintTexture", new TexturePaintEventArgs(32, 32, "", ""));
            Invoke(form, "CommitStroke");
            using Bitmap cleared = Capture(view, output, "7-cleared");
            Assert.Equal(0, Changed(baseline, cleared));
            Assert.True(form.TrySaveMap(false, out error), error?.ToString());
            File.WriteAllText(Path.Combine(output, "verified.txt"), view.ContextDescription);
            form.Close(); Application.DoEvents();
            Assert.False(view.IsReady);
            Assert.Equal(0, typeof(Map3DViewControl).GetField("_collisionTexture", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(view));
        });
    }
}
