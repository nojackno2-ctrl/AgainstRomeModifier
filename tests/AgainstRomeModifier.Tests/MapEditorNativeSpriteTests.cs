using System.Drawing;
using System.Numerics;
using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

/// <summary>原生 ALR/APT sprite 接到 3D 場景：頂點排序／錨點，以及真正 OpenGL 畫面上的出現、移動與關閉。</summary>
public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Sprite_quads_are_painted_far_to_near_with_ground_anchor_offsets()
    {
        var heights = new TerrainHeightField(257, 257, Enumerable.Repeat((byte)0, 257 * 257).ToArray(), tileWidth: 64, tileHeight: 64);
        var near = new MapSceneObject("Near", 256 * 10, 0, 256 * 10, 0, "a.sdl");
        var far = new MapSceneObject("Far", 256 * 50, 0, 256 * 50, 0, "a.sdl");
        var sprite = new NativeSprite(4, 10, Enumerable.Repeat(0xFF0000FFu, 40).ToArray(), 1, 8, "x.alr");
        var atlas = NativeSpriteAtlas.Pack([sprite]);
        float[] vertices = SceneObjectRenderer.BuildSpriteVertices([near, far, near], [sprite, sprite, null], atlas, heights, new Vector3(0, 10, 0));
        int stride = SceneObjectRenderer.FloatsPerSpriteVertex;
        Assert.Equal(2 * 6 * stride, vertices.Length); // the object without a sprite is skipped
        Assert.Equal(50f, vertices[0], 3); Assert.Equal(50f, vertices[2], 3); // far object first
        Assert.Equal(10f, vertices[6 * stride], 3);
        float s = SceneObjectRenderer.TilesPerSpritePixel;
        // First vertex is the bottom-left corner: 1px left of and 2px below the anchor.
        Assert.Equal(-1 * s, vertices[3], 6); Assert.Equal(-2 * s, vertices[4], 6);
        // Third vertex is the top-right corner: 3px right of and 8px above the anchor.
        Assert.Equal(3 * s, vertices[2 * stride + 3], 6); Assert.Equal(8 * s, vertices[2 * stride + 4], 6);
        Assert.True(atlas.TryGetUv(sprite, out NativeSpriteUv uv));
        Assert.Equal((uv.U0, uv.V1), (vertices[5], vertices[6]));
    }

    [Fact]
    public void Picking_hits_opaque_sprite_pixels_prefers_the_nearest_and_falls_back_to_markers()
    {
        var heights = new TerrainHeightField(257, 257, Enumerable.Repeat((byte)0, 257 * 257).ToArray(), tileWidth: 64, tileHeight: 64);
        var camera = new EditorCamera { Target = new Vector3(32, 0, 32) };
        camera.Zoom(.1f); // close-up: the sprite spans about 110 screen pixels
        Matrix4x4 view = camera.GetViewMatrix(), projection = camera.GetProjectionMatrix(1);
        var viewport = new Vector2(800, 800);
        // 200x200 sprite whose left half is transparent; ground anchor at bottom centre.
        uint[] pixels = Enumerable.Range(0, 200 * 200).Select(i => i % 200 < 100 ? 0u : 0xFFFFFFFFu).ToArray();
        var sprite = new NativeSprite(200, 200, pixels, 100, 200, "x.alr");
        var centre = new MapSceneObject("A", 32 * 256, 0, 32 * 256, 0, "a.sdl", 1);
        var nearer = centre with { Name = "B", ObjectIndex = 2, WorldX = centre.WorldX + 30, WorldZ = centre.WorldZ + 30 }; // toward the camera
        var marker = centre with { Name = "C", ObjectIndex = 3, WorldX = 20 * 256 };
        Vector2 Screen(MapSceneObject item, float upPixels)
        {
            Vector4 eye = Vector4.Transform(new Vector4(SceneObjectRenderer.GroundPoint(item, heights, item.Name == "C" ? .25f : 0), 1), view);
            Vector4 clip = Vector4.Transform(eye + new Vector4(0, upPixels * SceneObjectRenderer.TilesPerSpritePixel, 0, 0), projection);
            return new Vector2((clip.X / clip.W * .5f + .5f) * viewport.X, (.5f - clip.Y / clip.W * .5f) * viewport.Y);
        }
        int Pick(Vector2 point, params MapSceneObject[] objects) => SceneObjectRenderer.PickObject(objects,
            objects.Select(item => item.Name == "C" ? null : sprite).ToArray(), _ => true, heights, view, projection, viewport, point);
        Vector2 body = Screen(centre, 100) + new Vector2(3, 0); // just right of centre: opaque half
        Assert.Equal(0, Pick(body, centre));
        Assert.Equal(-1, Pick(body - new Vector2(8, 0), centre)); // transparent half
        Assert.Equal(-1, Pick(Screen(centre, 260), centre)); // above the quad
        Assert.Equal(1, Pick(body, centre, nearer)); // overlapping: the nearer (drawn last) wins
        Assert.Equal(0, Pick(body, nearer, centre)); // independent of list order
        Assert.Equal(0, Pick(Screen(marker, 0) + new Vector2(5, 0), marker)); // marker radius
        Assert.Equal(-1, Pick(Screen(marker, 0) + new Vector2(12, 0), marker));
        Assert.Equal(-1, SceneObjectRenderer.PickObject([centre], [sprite], _ => false, heights, view, projection, viewport, body)); // not in the atlas => marker only
    }

    [Fact]
    public void Real_opengl_click_selects_visible_sprite_and_drag_moves_it()
    {
        string map = CreateFixture();
        RunInSta(() =>
        {
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.ThrowException, threadScope: true);
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "SpritePick", "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            form.Show(); Application.DoEvents();
            Invoke(form, "SetActiveView", true); Application.DoEvents();
            var view = GetField<Map3DViewControl>(form, "_view3d");
            if (!view.IsReady) { Assert.NotEqual("1", Environment.GetEnvironmentVariable("ARM_OPENGL_REQUIRED")); return; }
            using var catalog = NativeSpriteCatalog.FromText(
                string.Join(",", Enumerable.Range(0, 60).Select(i => i switch { 0 => "42", 5 => "0", 8 => "-1", 14 => "-1", 17 => "0", 52 => "BauRomHau00_Haupthaus", _ => "   0" })),
                "0000,big.alr", "", name => name == "big.alr" ? SolidAlr(200, 240) : null, _ => null);
            view.SpriteCatalog = catalog;
            Type mode = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
            Invoke(form, "SetEditMode", Enum.Parse(mode, "SceneMove"));
            var sceneList = GetField<ListView>(form, "_sceneList");
            sceneList.SelectedItems.Clear(); Application.DoEvents();
            Assert.True(view.ScenePickEnabled);
            view.FocusTile(10000 / 256f, 6000 / 256f);
            ((EditorCamera)typeof(Map3DViewControl).GetField("_camera", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(view)!).Zoom(.05f);
            Size size = view.ClientSize;
            using Bitmap frame = view.CaptureFrame(size.Width, size.Height)!;
            List<Point> red = Red(frame);
            Assert.True(red.Count > 500, $"sprite 只畫出 {red.Count} 個像素。");
            var target = Point.Round(Centroid(red));
            Assert.Equal(0, view.PickSceneObject(target)); // the drawn pixels are what picking hits
            void Mouse(string method, Point at) => typeof(Control).GetMethod(method, BindingFlags.Instance | BindingFlags.NonPublic)!
                .Invoke(view, [new MouseEventArgs(MouseButtons.Left, 1, at.X, at.Y, 0)]);
            Mouse("OnMouseDown", target); Mouse("OnMouseUp", target);
            Assert.Single(sceneList.SelectedItems.Cast<ListViewItem>());
            var selected = (MapSceneObject)sceneList.SelectedItems[0].Tag!;
            Assert.Equal("BauRomHau00_Haupthaus", selected.Name);
            Assert.Equal((10000f, 6000f), (selected.WorldX, selected.WorldZ)); // a plain click does not move the object
            Assert.True(view.SceneMoveEnabled);

            var away = new Point(target.X + 120, target.Y + 60);
            Mouse("OnMouseDown", target); Mouse("OnMouseMove", away); Mouse("OnMouseUp", away);
            var moved = (MapSceneObject)sceneList.SelectedItems[0].Tag!;
            Assert.NotEqual((10000f, 6000f), (moved.WorldX, moved.WorldZ));
            Assert.True(moved.WorldX > 10000, $"往畫面右下拖曳應增加 X：{moved.WorldX},{moved.WorldZ}");
        }, TimeSpan.FromMinutes(2));
    }

    [Fact]
    public void Real_opengl_frame_draws_native_sprites_that_follow_object_moves()
    {
        string output = Environment.GetEnvironmentVariable("ARM_OPENGL_OUTPUT")
            ?? Path.Combine(Path.GetTempPath(), "ArmOpenGl_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(output);
        string map = CreateFixture();
        RunInSta(() =>
        {
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.ThrowException, threadScope: true);
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Sprites", "Test"));
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
            // Fixture house at world (10000, 6000): tile (39.06, 23.44).
            var house = new MapSceneObject("BauRomHau00_Haupthaus", 10000, 320, 6000, 3, "Endlos_Rom_Siedlung1.sdl", 0);
            view.UpdateSceneObjects([house]);
            view.FocusTile(10000 / 256f, 6000 / 256f);
            var camera = (EditorCamera)typeof(Map3DViewControl).GetField("_camera", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(view)!;
            camera.Zoom(.05f);
            using Bitmap markers = view.CaptureFrame(640, 480)!;
            Assert.True(Red(markers).Count < 50, "未設定素材庫時不應出現 sprite。");

            using var catalog = NativeSpriteCatalog.FromText(
                string.Join(",", Enumerable.Range(0, 60).Select(i => i switch { 0 => "42", 5 => "0", 14 => "-1", 17 => "0", 52 => "BauRomHau00_Haupthaus", _ => "   0" })),
                "0000,big.alr", "", name => name == "big.alr" ? SolidAlr(200, 240) : null, _ => null);
            view.SpriteCatalog = catalog;
            Assert.Equal(1, view.SpriteObjectCount);
            using Bitmap drawn = view.CaptureFrame(640, 480)!;
            drawn.Save(Path.Combine(output, "sprite-1-drawn.png"));
            List<Point> red = Red(drawn);
            Assert.True(red.Count > 2000, $"sprite 只畫出 {red.Count} 個像素。");
            PointF center = Centroid(red);
            Assert.InRange(center.X, 260, 380); // focused object is centred horizontally
            Assert.True(center.Y < 240, $"sprite 應畫在地面錨點上方，重心 {center}。");

            view.UpdateSceneObjects([house with { WorldX = house.WorldX + 256 }]); // one tile along +X
            using Bitmap moved = view.CaptureFrame(640, 480)!;
            moved.Save(Path.Combine(output, "sprite-2-moved.png"));
            PointF movedCenter = Centroid(Red(moved));
            // Game-aligned camera: map +X runs right and down the screen.
            Assert.True(movedCenter.X > center.X + 10 && movedCenter.Y > center.Y + 5, $"移動前 {center}，移動後 {movedCenter}。");

            view.SpriteCatalog = null;
            using Bitmap cleared = view.CaptureFrame(640, 480)!;
            Assert.True(Red(cleared).Count < 50, "移除素材庫後 sprite 應消失並退回標記點。");
            Assert.Equal(0, view.SpriteObjectCount);
        }, TimeSpan.FromMinutes(2));
    }

    /// <summary>
    /// 真實素材視覺驗收（需 ARM_NATIVE_ASSETS 指向唯讀素材副本：alr.dat、apt.dat、objdef.txt、cl_alr.txt、cl_apt.txt）。
    /// 只寫 ARM_OPENGL_OUTPUT／TEMP 截圖；不讀寫安裝目錄。
    /// </summary>
    [Fact]
    public void Real_assets_render_buildings_units_and_trees_in_the_3d_scene()
    {
        string? assets = Environment.GetEnvironmentVariable("ARM_NATIVE_ASSETS");
        if (string.IsNullOrWhiteSpace(assets) || !File.Exists(Path.Combine(assets, "alr.dat"))) return;
        string output = Environment.GetEnvironmentVariable("ARM_OPENGL_OUTPUT")
            ?? Path.Combine(Path.GetTempPath(), "ArmOpenGl_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(output);
        string map = CreateFixture();
        RunInSta(() =>
        {
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.ThrowException, threadScope: true);
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "RealSprites", "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            form.Show(); Application.DoEvents();
            Invoke(form, "SetActiveView", true); Application.DoEvents();
            var view = GetField<Map3DViewControl>(form, "_view3d");
            if (!view.IsReady) { Assert.NotEqual("1", Environment.GetEnvironmentVariable("ARM_OPENGL_REQUIRED")); return; }
            using var alr = System.IO.Compression.ZipFile.OpenRead(Path.Combine(assets, "alr.dat"));
            using var apt = System.IO.Compression.ZipFile.OpenRead(Path.Combine(assets, "apt.dat"));
            static byte[]? Read(System.IO.Compression.ZipArchive zip, string path)
            {
                var entry = zip.GetEntry(path);
                if (entry is null) return null;
                using var stream = entry.Open(); using var buffer = new MemoryStream(); stream.CopyTo(buffer); return buffer.ToArray();
            }
            string Text(string file) => MapTextEncoding.Game.GetString(File.ReadAllBytes(Path.Combine(assets, file)));
            using var catalog = NativeSpriteCatalog.FromText(Text("objdef.txt"), Text("cl_alr.txt"), Text("cl_apt.txt"),
                name => Read(alr, "SYSTEM/DATA/ALR/" + name), name => Read(apt, "SYSTEM/DATA/APT/" + name));
            float cx = 32 * 256f, cz = 32 * 256f;
            var scene = new List<MapSceneObject> { new("BauGerHau02_Haupthaus", cx, 0, cz, 1, "a.sdl") };
            for (int i = 0; i < 6; i++) scene.Add(new("FigGerSch01_Axt_Schild", cx + 700 + i * 90, 0, cz + 500, i % 3 + 1, "a.sdl"));
            for (int i = 0; i < 12; i++) scene.Add(new($"LanGerNad{i:00}_Tanne_{(i < 6 ? "gross" : "klein")}", cx - 1200 + (i % 4) * 260, 0, cz - 900 + (i / 4) * 260, 8, "a.sdl"));
            view.SpriteCatalog = catalog;
            view.UpdateSceneObjects(scene);
            Assert.Equal(scene.Count, view.SpriteObjectCount);
            view.FocusTile(32, 32);
            var camera = (EditorCamera)typeof(Map3DViewControl).GetField("_camera", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(view)!;
            camera.Zoom(.25f);
            using Bitmap frame = view.CaptureFrame(1280, 800)!;
            frame.Save(Path.Combine(output, "real-assets.png"));
            view.SpriteCatalog = null;
        }, TimeSpan.FromMinutes(2));
    }

    /// <summary>Minimal v6 8-bit ALR: one opaque frame of palette index 1 (pure red in the native 0x00BBGGRR order).</summary>
    private static byte[] SolidAlr(int width, int height)
    {
        using var stream = new MemoryStream();
        using var writer = new BinaryWriter(stream);
        int canvas = Math.Max(width, height) * 2;
        foreach (uint word in new uint[] { 0x41524C41, 6, 72, 1, 8, 0, 1, 1, 1, 0, 0, 0, 0, 0, 0, (uint)canvas, (uint)canvas, 0, 0, 0, 0, 0, 0 })
            writer.Write(word);
        int pixels = width * height, padded = (pixels + 3) & ~3;
        writer.Write(-1);
        writer.Write((uint)(2 * 4 + padded));
        writer.Write((uint)((canvas / 2 - width / 2) | (canvas / 2 - height) << 16)); // feet on the canvas centre
        writer.Write((uint)(width | height << 11 | 2 << 22));
        writer.Write((uint)pixels);
        writer.Write(0u); writer.Write(0x000000FFu);
        writer.Write(Enumerable.Repeat((byte)1, pixels).ToArray()); writer.Write(new byte[padded - pixels]);
        for (int y = 0; y <= height; y++) writer.Write((uint)(y * width));
        return stream.ToArray();
    }

    private static List<Point> Red(Bitmap frame)
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

    private static PointF Centroid(List<Point> points)
        => points.Count == 0 ? new PointF(float.NaN, float.NaN) : new PointF((float)points.Average(p => p.X), (float)points.Average(p => p.Y));
}
