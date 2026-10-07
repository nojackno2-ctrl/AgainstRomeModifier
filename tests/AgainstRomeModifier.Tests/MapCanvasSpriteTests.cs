using System.Drawing;
using System.Runtime.ExceptionServices;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

// WinForms rendering must not run in parallel with the editor form tests (high-DPI emulation is process-wide).
[Collection(MapEditorSaveTransactionTests.WinFormsCollection)]
public sealed class MapCanvasSpriteTests : IDisposable
{
    private readonly string _tempDir = Path.Combine(Path.GetTempPath(), "ArmCanvasTest_" + Guid.NewGuid().ToString("N"));

    public MapCanvasSpriteTests()
    {
        Directory.CreateDirectory(_tempDir);
    }

    public void Dispose()
    {
        try { Directory.Delete(_tempDir, true); } catch { }
    }

    [Fact]
    public void Canvas_renders_native_sprite_colour_only_when_catalog_is_assigned()
    {
        RunInSta(() =>
        {
            using var canvas = new MapCanvasControl();
            canvas.Size = new Size(640, 480);

            // 10x10 map with baseline textures
            int dimension = 10;
            var textures = Enumerable.Repeat("BA", dimension * dimension).ToArray();
            var floorTextures = new FloorTextureLibrary(_tempDir);

            // Object placed near the center of the map:
            // Map dimensions in world units: dimension * SdlSceneCatalog.WorldUnitsPerMapPixel * (SdlSceneCatalog.MapPixelSize / dimension)
            // SdlSceneCatalog.MapPixelSize is 256. WorldUnitsPerMapPixel is 256.
            // Map coordinates span 0 .. 256 * 256 = 65536.
            // Center is 32768, 32768.
            float centerX = (SdlSceneCatalog.WorldUnitsPerMapPixel * SdlSceneCatalog.MapPixelSize) / 2f;
            float centerZ = (SdlSceneCatalog.WorldUnitsPerMapPixel * SdlSceneCatalog.MapPixelSize) / 2f;
            var sceneObject = new MapSceneObject("TestHouse", centerX, 0, centerZ, 0, "test.sdl", 1);

            canvas.LoadTextures(
                dimension,
                textures,
                textures,
                _tempDir,
                floorTextures,
                new[] { sceneObject },
                waterLevel: 0,
                heightMapStep: 1,
                waterColor: Color.SteelBlue);

            // Without catalog: render to bitmap and assert pure red pixels do NOT appear
            using var bitmapWithoutCatalog = new Bitmap(canvas.Width, canvas.Height);
            canvas.DrawToBitmap(bitmapWithoutCatalog, new Rectangle(0, 0, canvas.Width, canvas.Height));

            List<Point> redWithout = FindRedPixels(bitmapWithoutCatalog);
            Assert.Empty(redWithout);

            // Build catalog with a SolidAlr sprite that has pure red pixels
            using var catalog = NativeSpriteCatalog.FromText(
                string.Join(",", Enumerable.Range(0, 60).Select(i => i switch { 0 => "42", 5 => "0", 8 => "-1", 14 => "-1", 17 => "0", 52 => "TestHouse", _ => "   0" })),
                "0000,testhouse.alr",
                "",
                name => name == "testhouse.alr" ? CreateSolidAlr(64, 64) : null,
                _ => null);

            canvas.SpriteCatalog = catalog;

            // With catalog: render to bitmap and assert pure red sprite pixels appear
            using var bitmapWithCatalog = new Bitmap(canvas.Width, canvas.Height);
            canvas.DrawToBitmap(bitmapWithCatalog, new Rectangle(0, 0, canvas.Width, canvas.Height));

            List<Point> redWith = FindRedPixels(bitmapWithCatalog);
            Assert.NotEmpty(redWith);

            // Removing catalog restores previous state
            canvas.SpriteCatalog = null;
            using var bitmapCleared = new Bitmap(canvas.Width, canvas.Height);
            canvas.DrawToBitmap(bitmapCleared, new Rectangle(0, 0, canvas.Width, canvas.Height));
            Assert.Empty(FindRedPixels(bitmapCleared));
        });
    }

    private static void RunInSta(Action action, TimeSpan? timeout = null)
    {
        Exception? failure = null;
        var thread = new Thread(() =>
        {
            try { action(); }
            catch (Exception ex) { failure = ex; }
        });
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        Assert.True(thread.Join(timeout ?? TimeSpan.FromSeconds(60)), "STA 測試執行逾時。");
        if (failure is not null)
        {
            ExceptionDispatchInfo.Capture(failure).Throw();
        }
    }

    /// <summary>Minimal v6 8-bit ALR: one opaque frame of palette index 1 (pure red in the native 0x00BBGGRR order).</summary>
    private static byte[] CreateSolidAlr(int width, int height)
    {
        using var stream = new MemoryStream();
        using var writer = new BinaryWriter(stream);
        int canvas = Math.Max(width, height) * 2;
        foreach (uint word in new uint[] { 0x41524C41, 6, 72, 1, 8, 0, 1, 1, 1, 0, 0, 0, 0, 0, 0, (uint)canvas, (uint)canvas, 0, 0, 0, 0, 0, 0 })
            writer.Write(word);
        int pixels = width * height, padded = (pixels + 3) & ~3;
        writer.Write(-1);
        writer.Write((uint)(2 * 4 + padded));
        writer.Write((uint)((canvas / 2 - width / 2) | (canvas / 2 - height) << 16));
        writer.Write((uint)(width | height << 11 | 2 << 22));
        writer.Write((uint)pixels);
        writer.Write(0u); writer.Write(0x000000FFu);
        writer.Write(Enumerable.Repeat((byte)1, pixels).ToArray()); writer.Write(new byte[padded - pixels]);
        for (int y = 0; y <= height; y++) writer.Write((uint)(y * width));
        return stream.ToArray();
    }

    private static List<Point> FindRedPixels(Bitmap frame)
    {
        var points = new List<Point>();
        for (int y = 0; y < frame.Height; y++)
            for (int x = 0; x < frame.Width; x++)
            {
                Color color = frame.GetPixel(x, y);
                if (color.R > 200 && color.G < 60 && color.B < 60) points.Add(new Point(x, y));
            }
        return points;
    }
}
