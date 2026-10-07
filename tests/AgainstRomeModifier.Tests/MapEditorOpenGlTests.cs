using System.Drawing;
using System.Reflection;
using System.Text.Json;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

/// <summary>
/// 真正的 OpenGL 3.3 畫面：以 Map3DViewControl.CaptureFrame 讀回與螢幕相同繪製路徑的像素，
/// 確認地形有被畫出，且高度筆畫、undo、材質與水面更新會反映在 GPU 畫面。
/// 本機沒有 OpenGL 3.3 時只檢查失敗原因被記錄；設 ARM_OPENGL_REQUIRED=1 則視為失敗。
/// </summary>
public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Real_opengl_frame_reflects_height_undo_material_and_water_updates()
    {
        string output = Environment.GetEnvironmentVariable("ARM_OPENGL_OUTPUT")
            ?? Path.Combine(Path.GetTempPath(), "ArmOpenGl_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(output);
        string map = CreateFixture();
        void Trace(string step) => File.AppendAllText(Path.Combine(output, "trace.log"), $"{DateTime.Now:HH:mm:ss.fff} {step}{Environment.NewLine}");
        RunInSta(() =>
        {
            Trace("start");
            // 事件處理中的例外直接拋出，不要彈出 ThreadExceptionDialog 卡住測試。
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.ThrowException, threadScope: true);
            // 正式程式在主執行緒建立 GLControl；測試在獨立 STA 執行緒，需關閉 OpenTK 的主執行緒檢查。
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "OpenGL", "Test"));
            form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            // 測試結束釋放表單時不要跳出「是否儲存」對話框（會掩蓋真正的斷言失敗）。
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.Show(); Trace("shown"); Application.DoEvents(); // Shown 事件載入地圖
            Invoke(form, "SetActiveView", true); Application.DoEvents(); Trace("3d active");
            var view = GetField<Map3DViewControl>(form, "_view3d"); Trace("ready=" + view.IsReady + " " + view.ContextDescription + " " + view.LastFailureReason);
            if (!view.IsReady)
            {
                Assert.False(string.IsNullOrWhiteSpace(view.LastFailureReason), "3D 不可用時必須記錄原因。");
                Assert.NotEqual("1", Environment.GetEnvironmentVariable("ARM_OPENGL_REQUIRED"));
                File.WriteAllText(Path.Combine(output, "opengl.txt"), "unavailable: " + view.LastFailureReason);
                return;
            }

            using Bitmap original = Capture(view, output, "1-original"); Trace("captured");
            Assert.True(Changed(original, Background(original)) > original.Width * original.Height / 10, "地形未繪製到畫面。");

            var paintClock = System.Diagnostics.Stopwatch.StartNew();
            for (int frame = 0; frame < 30; frame++) view.Refresh(); // 同步 OnPaint：繪製＋SwapBuffers
            double paintMs = paintClock.Elapsed.TotalMilliseconds / 30;
            Assert.True(paintMs < 50, $"3D 重繪平均 {paintMs:0.0} ms。");

            Type mode = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
            Invoke(form, "SetEditMode", Enum.Parse(mode, "Height"));
            var strokeSamples = new List<double>();
            for (int pass = 0; pass < 6; pass++)
            {
                for (int y = 24; y <= 40; y++) for (int x = 24; x <= 40; x++)
                {
                    var tick = System.Diagnostics.Stopwatch.StartNew(); // 單一筆刷事件：高度 session＋2D/3D 更新
                    Invoke(form, "PaintTexture", new TexturePaintEventArgs(x, y, "", ""));
                    strokeSamples.Add(tick.Elapsed.TotalMilliseconds);
                }
                Invoke(form, "CommitStroke");
            }
            strokeSamples.Sort();
            double strokeP95 = strokeSamples[(int)(strokeSamples.Count * .95)];
            Assert.True(strokeP95 < 100, $"筆刷事件 p95 {strokeP95:0.0} ms。");
            using Bitmap raised = Capture(view, output, "2-height");
            int heightDelta = Changed(original, raised);
            Assert.True(heightDelta > 500, $"高度筆畫只改變 {heightDelta} 像素。");

            for (int pass = 0; pass < 6; pass++) Invoke(form, "Undo");
            using Bitmap undone = Capture(view, output, "3-undo");
            int undoDelta = Changed(original, undone);
            Assert.True(undoDelta < heightDelta / 20, $"undo 後仍有 {undoDelta} 像素與原畫面不同。");

            for (int y = 0; y < 64; y++) for (int x = 0; x < 64; x++) view.SetTexture(x, y, "4BB___52");
            using Bitmap material = Capture(view, output, "4-material");
            Assert.True(Brightness(material) > Brightness(undone), "材質更新未反映在畫面亮度。");

            view.UpdateWater(800, Color.FromArgb(20, 60, 220));
            using Bitmap water = Capture(view, output, "5-water");
            Assert.True(Blueness(water) > Blueness(material) + 5, "水面更新未反映在畫面。");

            // handle 重建（未釋放控制項）後 context 會隨舊 handle 消失，必須重新初始化並畫出相同畫面。
            typeof(Control).GetMethod("RecreateHandle", BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(view, null);
            Application.DoEvents();
            Assert.True(view.IsReady, view.LastFailureReason);
            using Bitmap recreated = Capture(view, output, "6-recreated");
            int recreatedDelta = Changed(water, recreated);
            Assert.True(recreatedDelta < 200, $"重建 handle 後畫面差異 {recreatedDelta} 像素。");

            File.WriteAllText(Path.Combine(output, "opengl.json"), JsonSerializer.Serialize(new
            {
                view.ContextDescription, Size = $"{original.Width}x{original.Height}", HeightChangedPixels = heightDelta, UndoResidualPixels = undoDelta,
                BrightnessBefore = Brightness(undone), BrightnessAfter = Brightness(material), BlueBefore = Blueness(material), BlueAfter = Blueness(water), RecreatedResidualPixels = recreatedDelta,
                AveragePaintMs = paintMs, BrushEventMedianMs = strokeSamples[strokeSamples.Count / 2], BrushEventP95Ms = strokeP95, BrushEvents = strokeSamples.Count
            }));
            form.Close(); // 關閉時父視窗先銷毀子 handle；GL 資源須在 context 消失前釋放，且不得觸發重新初始化或例外。
            Application.DoEvents();
            Assert.False(view.IsReady);
        }, TimeSpan.FromMinutes(2));
    }

    [Fact]
    public void Real_opengl_pick_returns_the_tile_drawn_under_the_pointer()
    {
        string output = Environment.GetEnvironmentVariable("ARM_OPENGL_OUTPUT")
            ?? Path.Combine(Path.GetTempPath(), "ArmOpenGl_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(output);
        string map = CreateFixture();
        RunInSta(() =>
        {
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.ThrowException, threadScope: true);
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Pick", "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            form.Show(); Application.DoEvents();
            Invoke(form, "SetActiveView", true); Application.DoEvents();
            var view = GetField<Map3DViewControl>(form, "_view3d");
            if (!view.IsReady)
            {
                Assert.NotEqual("1", Environment.GetEnvironmentVariable("ARM_OPENGL_REQUIRED"));
                return;
            }
            MethodInfo pick = typeof(Map3DViewControl).GetMethod("TryGetTile", BindingFlags.Instance | BindingFlags.NonPublic)!;
            for (int y = 0; y < 64; y++) for (int x = 0; x < 64; x++) view.SetTexture(x, y, "4BB___51");
            Size size = view.ClientSize;
            using Bitmap baseline = view.CaptureFrame(size.Width, size.Height)!;
            var results = new List<string>();
            foreach ((int tileX, int tileY) in new[] { (32, 32), (12, 50), (50, 12), (45, 45), (20, 20) })
            {
                view.SetTexture(tileX, tileY, "4BB___52");
                using Bitmap marked = view.CaptureFrame(size.Width, size.Height)!;
                view.SetTexture(tileX, tileY, "4BB___51");
                long sumX = 0, sumY = 0; int count = 0;
                for (int y = 0; y < size.Height; y++) for (int x = 0; x < size.Width; x++)
                    if (Distance(baseline.GetPixel(x, y), marked.GetPixel(x, y)) > 6) { sumX += x; sumY += y; count++; }
                Assert.True(count > 0, $"格子 ({tileX},{tileY}) 的材質變更沒有出現在畫面。");
                var center = new Point((int)(sumX / count), (int)(sumY / count));
                object?[] args = [center, 0, 0];
                Assert.True((bool)pick.Invoke(view, args)!, $"像素 {center} 沒有選到地形。");
                results.Add($"tile ({tileX},{tileY}) drawn at {center} ({count} px) -> picked ({args[1]},{args[2]})");
                Assert.Equal((tileX, tileY), ((int)args[1]!, (int)args[2]!));
            }
            File.WriteAllLines(Path.Combine(output, "pick.txt"), results.Prepend($"{view.ContextDescription} {size}"));
        }, TimeSpan.FromMinutes(2));
    }

    private static Bitmap Capture(Map3DViewControl view, string output, string name)
    {
        Application.DoEvents();
        Bitmap frame = view.CaptureFrame(640, 480) ?? throw new InvalidOperationException("CaptureFrame 失敗。");
        frame.Save(Path.Combine(output, name + ".png"));
        return frame;
    }

    private static Color Background(Bitmap frame) => frame.GetPixel(0, 0);

    private static int Changed(Bitmap a, Color background)
    {
        int count = 0;
        for (int y = 0; y < a.Height; y += 2) for (int x = 0; x < a.Width; x += 2) if (Distance(a.GetPixel(x, y), background) > 12) count += 4;
        return count;
    }

    private static int Changed(Bitmap a, Bitmap b)
    {
        int count = 0;
        for (int y = 0; y < a.Height; y += 2) for (int x = 0; x < a.Width; x += 2) if (Distance(a.GetPixel(x, y), b.GetPixel(x, y)) > 12) count += 4;
        return count;
    }

    private static int Distance(Color a, Color b) => Math.Abs(a.R - b.R) + Math.Abs(a.G - b.G) + Math.Abs(a.B - b.B);

    private static double Brightness(Bitmap frame) => Average(frame, color => (color.R + color.G + color.B) / 3.0);

    private static double Blueness(Bitmap frame) => Average(frame, color => color.B - (color.R + color.G) / 2.0);

    private static double Average(Bitmap frame, Func<Color, double> value)
    {
        double total = 0; int count = 0;
        for (int y = 0; y < frame.Height; y += 4) for (int x = 0; x < frame.Width; x += 4) { total += value(frame.GetPixel(x, y)); count++; }
        return total / count;
    }
}
