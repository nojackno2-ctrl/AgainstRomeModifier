using System.Drawing;
using System.Drawing.Imaging;
using System.Numerics;
using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Real_opengl_game_lighting_darkens_terrain_and_draws_a_local_spot_in_world_units()
    {
        string map = CreateFixture();
        WriteLightingDaynight(Path.Combine(map, "daynight.bmp"));
        string defaults = Path.Combine(_root, "SYSTEM", "DATA_MP", "DEFAULTS"); Directory.CreateDirectory(defaults);
        File.WriteAllBytes(Path.Combine(defaults, "objdef.dau"), LightingPfil(LightingObjdef("Torch")));
        File.WriteAllBytes(Path.Combine(defaults, "lightdef.dau"), LightingPfil("[LightDefault]\n0,1,1,1,1,512"));
        RunInSta(() =>
        {
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.ThrowException, threadScope: true);
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Lighting", "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            form.Show(); Application.DoEvents(); Invoke(form, "SetActiveView", true); Application.DoEvents();
            var view = GetField<Map3DViewControl>(form, "_view3d");
            if (!view.IsReady) { Assert.NotEqual("1", Environment.GetEnvironmentVariable("ARM_OPENGL_REQUIRED")); return; }
            Assert.NotNull(view.LightingContext); Assert.False(view.GameLightingEnabled);
            view.ShowGrid = view.ShowObjects = false;
            // 非零地面高度 2 tiles = 512 world，亦驗證 shader Y 不是混用 tile／world。
            view.SetHeightSamples(Enumerable.Repeat((byte)85, 257 * 257).ToArray()); view.UpdateWater(-100, Color.Black);
            view.FocusTile(32, 32);
            var camera = (EditorCamera)typeof(Map3DViewControl).GetField("_camera", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(view)!;
            camera.Zoom(.08f);
            using Bitmap neutral = view.CaptureFrame(640, 480)!;
            var enabled = GetField<CheckBox>(form, "_gameLighting"); var hour = GetField<NumericUpDown>(form, "_gameHour");
            enabled.Checked = true; hour.Value = 0;
            Assert.True(view.GameLightingEnabled); Assert.True(hour.Enabled); Assert.Equal(0, view.GameHour);
            using Bitmap night = view.CaptureFrame(640, 480)!;
            int darkened = 0;
            for (int y = 0; y < 480; y += 2) for (int x = 0; x < 640; x += 2)
            {
                Color a = neutral.GetPixel(x, y), b = night.GetPixel(x, y);
                if (a.R > 50 && b.R < a.R / 3 && b.G < a.G / 3) darkened++;
            }
            Assert.True(darkened > 10000, $"夜間只有 {darkened} 個地表取樣變暗。");
            // 超過 64 個光源，近處最後加入，必須依相機目標選取而非取物件前 64 筆。
            var torch = new MapSceneObject("Torch", 32 * 256, 512, 32 * 256, 0, "test.sdl");
            MapSceneObject[] lights = Enumerable.Range(0, 70).Select(i => torch with { WorldX = i, WorldZ = 0 }).Append(torch).ToArray();
            view.UpdateSceneObjects(lights); Assert.Equal(71, view.SceneLightCount);
            using Bitmap lit = view.CaptureFrame(640, 480)!;
            Assert.Equal(Map3DViewControl.MaximumGameLights, (int)typeof(Map3DViewControl).GetField("_selectedLightCount", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(view)!);
            Point At(float x, float z) => LightingScreenPoint(camera, new Vector3(x, 2, z), 640, 480);
            Color centre = lit.GetPixel(At(32, 32).X, At(32, 32).Y);
            Color edge = lit.GetPixel(At(32 + 1.5f, 32).X, At(32 + 1.5f, 32).Y);
            Color outside = lit.GetPixel(At(32 + 3, 32).X, At(32 + 3, 32).Y);
            Assert.True(centre.R > 60 && edge.R > outside.R * 2 && centre.R > edge.R, $"光暈中心／中間／外側 R={centre.R}/{edge.R}/{outside.R}");
            // 半徑是 world=512=2 tiles；搬走光源後原位置恢復夜色。
            view.UpdateSceneObjects([torch with { WorldX = 40 * 256 }]);
            using Bitmap moved = view.CaptureFrame(640, 480)!;
            Color movedCentre = moved.GetPixel(At(32, 32).X, At(32, 32).Y);
            Assert.True(movedCentre.R < centre.R / 3);
            var sceneLights = (List<NativeLightInstance>)typeof(Map3DViewControl).GetField("_sceneLights", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(view)!;
            NativeLightInstance cached = sceneLights[0];
            view.FocusTile(40, 32); using (var atMovedLight = view.CaptureFrame(640, 480)!)
                Assert.True(atMovedLight.GetPixel(320, 240).R > 60, "光源選取應跟隨相機目標。");
            Assert.Same(cached, sceneLights[0]); // render／相機移動不重建場景光源。
            view.FocusTile(32, 32);
            // 重建 GL context 仍保留光照設定及 uniforms。
            typeof(Control).GetMethod("RecreateHandle", BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(view, null); Application.DoEvents();
            Assert.True(view.IsReady, view.LastFailureReason);
            using Bitmap recreated = view.CaptureFrame(640, 480)!;
            Assert.True(Changed(moved, recreated) < 200);
            enabled.Checked = false; Assert.False(hour.Enabled);
            using Bitmap restored = view.CaptureFrame(640, 480)!;
            Assert.True(Changed(neutral, restored) < 200, "關閉光照後應恢复中性編輯畫面。");
        }, TimeSpan.FromMinutes(2));
    }

    [Fact]
    public void Real_opengl_game_lighting_multiplies_sprites_and_place_preview_at_ground_anchors()
    {
        string map = CreateFixture(); WriteLightingDaynight(Path.Combine(map, "daynight.bmp"));
        RunInSta(() =>
        {
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.ThrowException, threadScope: true);
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Sprite lighting", "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            form.Show(); Application.DoEvents(); Invoke(form, "SetActiveView", true); Application.DoEvents();
            var view = GetField<Map3DViewControl>(form, "_view3d");
            if (!view.IsReady) { Assert.NotEqual("1", Environment.GetEnvironmentVariable("ARM_OPENGL_REQUIRED")); return; }
            view.ShowGrid = false; view.AnimationsEnabled = false;
            view.SetHeightSamples(new byte[257 * 257]); view.UpdateWater(-100, Color.Black);
            using var sprites = NativeSpriteCatalog.FromText(
                string.Join(',', Enumerable.Range(0, 60).Select(i => i switch { 0 => "42", 5 => "0", 8 => "-1", 14 => "-1", 52 => "House", _ => "0" })),
                "0000,red.alr", "", _ => SolidAlr(100, 160), _ => null);
            view.SpriteCatalog = sprites;
            view.UpdateSceneObjects([new("House", 32 * 256, 0, 32 * 256, 0, "test.sdl")]);
            view.FocusTile(32, 32);
            var camera = (EditorCamera)typeof(Map3DViewControl).GetField("_camera", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(view)!; camera.Zoom(.05f);
            view.EditingEnabled = true; view.SetPlacementPreview("House", 0, 0);
            typeof(Map3DViewControl).GetField("_hoverX", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(view, 30);
            typeof(Map3DViewControl).GetField("_hoverY", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(view, 32);
            using Bitmap neutral = view.CaptureFrame(640, 480)!;
            view.GameLightingEnabled = true; view.GameHour = 0;
            using Bitmap night = view.CaptureFrame(640, 480)!;
            int sprite = 0, ghost = 0;
            for (int y = 0; y < 480; y++) for (int x = 0; x < 640; x++)
            {
                Color a = neutral.GetPixel(x, y), b = night.GetPixel(x, y);
                if (a.R > 200 && a.G < 5 && a.B < 5) { sprite++; Assert.InRange(b.R, 25, 40); }
                else if (a.R > 140 && a.R > a.G * 3 && a.G > 5) { ghost++; Assert.True(b.R < a.R / 3); }
            }
            Assert.True(sprite > 500 && ghost > 500, $"sprite／preview 像素數={sprite}/{ghost}");
            using var lights = new SceneLightingContext(MapLightingDayNight.Parse(File.ReadAllBytes(Path.Combine(map, "daynight.bmp"))),
                NativeLightCatalog.Parse("[LightDefault]\n0,1,1,1,1,512"), NativeObjectLightingCatalog.Parse(LightingObjdef("House")), null, null);
            view.LightingContext = lights;
            using Bitmap bright = view.CaptureFrame(640, 480)!;
            int brightSprite = 0, brightGhost = 0;
            for (int y = 0; y < 480; y++) for (int x = 0; x < 640; x++)
            {
                Color a = neutral.GetPixel(x, y), b = bright.GetPixel(x, y), n = night.GetPixel(x, y);
                if (a.R > 200 && a.G < 5 && a.B < 5 && b.R > 200) brightSprite++;
                else if (a.R > 140 && a.R > a.G * 3 && a.G > 5 && b.R > n.R * 1.5f) brightGhost++;
            }
            Assert.True(brightSprite > 500 && brightGhost > 500, $"局部光源應照亮 sprite／preview：{brightSprite}/{brightGhost}");
        }, TimeSpan.FromMinutes(2));
    }

    [Fact]
    public void Real_game_copy_renders_daytime_lighting_beside_game_house()
    {
        string? game = Environment.GetEnvironmentVariable("ARM_COMPARE_GAME");
        if (string.IsNullOrWhiteSpace(game) || !Directory.Exists(Path.Combine(game, "ENDL_005"))) return;
        string output = Environment.GetEnvironmentVariable("ARM_OPENGL_OUTPUT") ?? game; Directory.CreateDirectory(output);
        RunInSta(() =>
        {
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(game, new GameMapInfo("ENDL_005", Path.Combine(game, "ENDL_005"), false, "Lighting compare", "Test"));
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            form.Show(); Application.DoEvents(); Invoke(form, "SetActiveView", true); Application.DoEvents();
            var view = GetField<Map3DViewControl>(form, "_view3d");
            if (!view.IsReady) { Assert.NotEqual("1", Environment.GetEnvironmentVariable("ARM_OPENGL_REQUIRED")); return; }
            Assert.NotNull(view.LightingContext); Assert.True(view.LightingContext.IsLightCatalogAvailable);
            Assert.True(view.SceneLightCount > 0); view.AnimationTimeMs = 0;
            view.FocusTile(10624 / 256f, 10112 / 256f);
            var camera = (EditorCamera)typeof(Map3DViewControl).GetField("_camera", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(view)!; camera.ZoomToGameScale(610);
            view.GameLightingEnabled = true; view.GameHour = 12;
            using Bitmap rendered = view.CaptureFrame(1024, 610)!; rendered.Save(Path.Combine(output, "editor-house-lighting-12.png"));
            using var original = new Bitmap(Path.Combine(game, "game-house.png"));
            using var pair = new Bitmap(2048, 660);
            using (Graphics graphics = Graphics.FromImage(pair))
            {
                graphics.Clear(Color.FromArgb(24, 28, 36));
                graphics.DrawString("Editor - Game lighting 12:00", SystemFonts.DefaultFont, Brushes.White, 16, 10);
                graphics.DrawString("Game screenshot - capture time unknown", SystemFonts.DefaultFont, Brushes.White, 1040, 10);
                graphics.DrawImageUnscaled(rendered, 0, 40);
                graphics.DrawImage(original, new Rectangle(1024, 40, 1024, 610), new Rectangle(0, 0, 1024, 610), GraphicsUnit.Pixel);
            }
            pair.Save(Path.Combine(output, "house-lighting-side-by-side.png"));
        }, TimeSpan.FromMinutes(2));
    }

    private static string LightingObjdef(string name)
        => string.Join(',', Enumerable.Range(0, 164).Select(i => i switch { 0 => "42", 14 => "-1", 52 => name, 66 => "0", 162 => "-1", _ => "0" }));

    private static byte[] LightingPfil(string text)
    {
        byte[] header = new byte[64]; "PFIL"u8.CopyTo(header);
        return GameLZSS.CompressPfil(MapTextEncoding.Game.GetBytes(text), header);
    }

    private static Point LightingScreenPoint(EditorCamera camera, Vector3 position, int width, int height)
    {
        Vector4 clip = Vector4.Transform(new Vector4(position, 1), camera.GetViewMatrix() * camera.GetProjectionMatrix(width / (float)height));
        return new Point((int)((clip.X / clip.W * .5f + .5f) * width), (int)((.5f - clip.Y / clip.W * .5f) * height));
    }

    private static void WriteLightingDaynight(string path)
    {
        using var bitmap = new Bitmap(24, 6, PixelFormat.Format24bppRgb);
        for (int row = 0; row < 6; row++) for (int hour = 0; hour < 24; hour++)
            bitmap.SetPixel(hour, row, row == 0 ? (hour >= 6 && hour <= 18 ? Color.FromArgb(240, 224, 208) : Color.FromArgb(32, 48, 64)) : Color.White);
        bitmap.Save(path, ImageFormat.Bmp);
    }
}
