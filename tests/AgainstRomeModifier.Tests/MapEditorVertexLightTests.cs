using System.Drawing;
using System.Drawing.Imaging;
using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Real_opengl_Vertex_light_multiplies_terrain_and_survives_context_recreation()
    {
        string map = CreateFixture();
        using (var vertex = new Bitmap(257, 257, PixelFormat.Format24bppRgb))
        {
            using (Graphics graphics = Graphics.FromImage(vertex))
            {
                graphics.Clear(Color.White);
                graphics.FillRectangle(Brushes.Red, 0, 0, 128, 257);
            }
            vertex.Save(Path.Combine(map, "vertex.bmp"), ImageFormat.Bmp);
        }

        RunInSta(() =>
        {
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.ThrowException, threadScope: true);
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Vertex light", "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            form.Show(); Application.DoEvents();
            Invoke(form, "SetActiveView", true); Application.DoEvents();
            var view = GetField<Map3DViewControl>(form, "_view3d");
            if (!view.IsReady)
            {
                Assert.False(string.IsNullOrWhiteSpace(view.LastFailureReason));
                Assert.NotEqual("1", Environment.GetEnvironmentVariable("ARM_OPENGL_REQUIRED"));
                return;
            }

            view.ShowGrid = false; view.ShowObjects = false;
            using Bitmap frame = view.CaptureFrame(640, 480) ?? throw new InvalidOperationException("CaptureFrame failed.");
            int red = 0, neutral = 0;
            for (int y = 0; y < frame.Height; y++) for (int x = 0; x < frame.Width; x++)
            {
                Color pixel = frame.GetPixel(x, y);
                if (pixel.R > 40 && pixel.G < pixel.R / 4 && pixel.B < pixel.R / 4) red++;
                if (pixel.R > 40 && Math.Abs(pixel.R - pixel.G) < 5 && Math.Abs(pixel.R - pixel.B) < 5) neutral++;
            }
            Assert.True(red > 2000, $"Expected red-lit terrain; found {red} pixels.");
            Assert.True(neutral > 2000, $"Expected neutral terrain; found {neutral} pixels.");

            typeof(Control).GetMethod("RecreateHandle", BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(view, null);
            Application.DoEvents();
            Assert.True(view.IsReady, view.LastFailureReason);
            using Bitmap recreated = view.CaptureFrame(640, 480) ?? throw new InvalidOperationException("CaptureFrame failed after context recreation.");
            Assert.True(Changed(frame, recreated) < 200, "Vertex lighting changed after context recreation.");
            form.Close(); Application.DoEvents();
            Assert.False(view.IsReady);
        }, TimeSpan.FromMinutes(2));
    }
}
