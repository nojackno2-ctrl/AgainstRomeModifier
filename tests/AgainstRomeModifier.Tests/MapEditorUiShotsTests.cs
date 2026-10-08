using System.Drawing;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    /// <summary>手動取得 UI 截圖：設 ARM_UI_SHOTS=輸出目錄；未設定時直接返回。</summary>
    [Fact]
    public void Ui_shots_for_new_tools_when_requested()
    {
        string? output = Environment.GetEnvironmentVariable("ARM_UI_SHOTS");
        if (string.IsNullOrWhiteSpace(output)) return;
        Directory.CreateDirectory(output);
        string map = CreateFixture("ENDL_005");

        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "UiShots", "Test"));
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            typeof(MapEditorForm).GetField("_allowClose", System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = System.Windows.Forms.FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            form.Size = new Size(1300, 800); form.Show(); System.Windows.Forms.Application.DoEvents(); Invoke(form, "SetActiveView", false);
            var tabs = GetField<System.Windows.Forms.TabControl>(form, "_inspectorTabs");
            foreach (var (name, field) in new[] { ("mapcheck", "_mapCheckTab"), ("console", "_consoleTab") })
            {
                tabs.SelectedTab = GetField<System.Windows.Forms.TabPage>(form, field); form.PerformLayout(); System.Windows.Forms.Application.DoEvents();
                using var image = new Bitmap(form.Width, form.Height); form.DrawToBitmap(image, new Rectangle(Point.Empty, form.Size)); image.Save(Path.Combine(output, $"editor-{name}.png"));
            }
            using var dialog = new TerrainRegionDialog(64);
            foreach (var op in new[] { TerrainRegionOperation.Flora, TerrainRegionOperation.Erosion, TerrainRegionOperation.Wall })
            {
                dialog.Operation = op; dialog.StartPosition = System.Windows.Forms.FormStartPosition.Manual; dialog.Location = new Point(-30000, -30000); dialog.Show(); System.Windows.Forms.Application.DoEvents();
                using var shot = new Bitmap(dialog.Width, dialog.Height); dialog.DrawToBitmap(shot, new Rectangle(Point.Empty, dialog.Size));
                shot.Save(Path.Combine(output, $"region-dialog-{op}.png"));
            }
        });
    }
}
