using System.Drawing;
using System.Numerics;
using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

// 已保存光源 → 3D view 接線的真 GL framebuffer 測試。直接提供 saved instance，不讀 light.dat、不載 native DLL、不 mock GL。
// 無 GL 時略過；ARM_OPENGL_REQUIRED=1 時必須有 GL。夾具高度 raw=50、Heightmapstep=4 → native terrain Y=200。
// 這些像素測試只驗證編輯器預覽（0x49FFE0 公式＋原生 XYZ），不是原遊戲像素等價。
public sealed partial class MapEditorSaveTransactionTests
{
    private const float SavedLightRaw = 50f, SavedLightNativeGroundY = 200f, SavedLightTile = 256f;

    [Fact]
    public void Saved_native_instance_carries_its_own_rgb_and_effective_radius_without_a_definition()
    {
        NativeLightInstance saved = NativeLightInstance.CreateSaved(7, new Vector3(1000, 200, 3000), new Vector3(2.5f, 0.25f, 0), 320f, 90f);
        Assert.True(saved.IsSaved); Assert.False(saved.HasDefinition); Assert.Equal(7, saved.SavedSlot);
        Assert.Equal(new Vector3(1000, 200, 3000), saved.WorldPosition);
        Assert.Equal(new Vector3(2.5f, 0.25f, 0), saved.Color); // RGB>1 不夾值
        Assert.Equal(320f, saved.Radius); Assert.Equal(90f, saved.BaselineRadius);
        Assert.Equal(320f * 320f, saved.RadiusSquared); Assert.True(saved.IsRenderable);
        Assert.Throws<InvalidOperationException>(() => saved.WorldPosition = Vector3.Zero);
        Assert.Throws<InvalidOperationException>(() => saved.Definition = NativeLightDefinition.CreateDefault());
        Assert.Throws<InvalidOperationException>(() => saved.IsActive = false);

        Assert.False(NativeLightInstance.CreateSaved(0, Vector3.Zero, Vector3.One, 0f).IsRenderable);
        Assert.False(NativeLightInstance.CreateSaved(0, new Vector3(float.NaN, 0, 0), Vector3.One, 10f).IsRenderable);
        Assert.False(NativeLightInstance.CreateSaved(0, Vector3.Zero, new Vector3(1, float.PositiveInfinity, 1), 10f).IsRenderable);
        Assert.False(NativeLightInstance.CreateSaved(0, Vector3.Zero, Vector3.One, 10f, isActive: false).IsRenderable);

        // 舊 definition fallback 不變。
        var definition = new NativeLightDefinition { Index = 3, IsActive = true, Color = new Vector3(.5f, .6f, .7f), Radius = 400 };
        var legacy = new NativeLightInstance(new Vector3(1, 2, 3), definition);
        Assert.False(legacy.IsSaved); Assert.True(legacy.HasDefinition);
        Assert.Equal(definition.Color, legacy.Color); Assert.Equal(400f, legacy.Radius); Assert.Equal(400f, legacy.BaselineRadius);
        legacy.WorldPosition = new Vector3(4, 5, 6); Assert.Equal(new Vector3(4, 5, 6), legacy.WorldPosition);

        // definition 只是 metadata：RGB／半徑仍來自 instance。
        NativeLightInstance withDefinition = NativeLightInstance.CreateSaved(1, Vector3.Zero, Vector3.UnitX, 50f, 60f, definition: definition);
        Assert.True(withDefinition.HasDefinition); Assert.Equal(Vector3.UnitX, withDefinition.Color); Assert.Equal(50f, withDefinition.Radius);

        Assert.Throws<ArgumentException>(() => new SavedSceneLightSet([legacy], null));
    }

    [Fact]
    public void Suppression_key_is_explicit_and_normalised_and_skips_unindexed_objects()
    {
        var item = new MapSceneObject("Torch", 0, 0, 0, 0, "Maps\\A.sdl", ObjectIndex: 4);
        Assert.True(SceneLightSuppressionKey.TryFor(item, out SceneLightSuppressionKey key));
        Assert.Equal(new SceneLightSuppressionKey("maps/a.SDL", 4), key);
        Assert.NotEqual(new SceneLightSuppressionKey("maps/a.sdl", 5), key);
        Assert.False(SceneLightSuppressionKey.TryFor(item with { ObjectIndex = -1 }, out _));
        Assert.Throws<ArgumentOutOfRangeException>(() => new SceneLightSuppressionKey("a", -1));
        foreach (int index in new[] { -100000, -100007, -5000, -5002 })
        {
            Assert.True(SceneLightSuppressionKey.TryFor(item with { ObjectIndex = index }, out var negative));
            Assert.Equal(new SceneLightSuppressionKey(item.SourceFile, index), negative);
        }
    }

    [Fact]
    public void Native_query_height_ignores_relief_and_unknown_step_is_never_substituted()
    {
        var samples = new byte[] { 0, 100, 200, 30, 130, 230, 60, 160, 255 };
        var heights = new TerrainHeightField(3, 3, samples, 6f, 2, 2, 4f);
        Assert.True(heights.HasNativeQuery);
        Assert.True(heights.TryNativeQueryHeight(1f, 1f, out float y)); Assert.Equal(130f * 4f, y);
        Assert.True(heights.TryNativeQueryHeight(.5f, 0f, out y)); Assert.Equal(50f * 4f, y);
        float[] texels = heights.BuildNativeHeightTexels()!;
        Assert.Equal(samples.Select(sample => sample * 4f), texels);
        heights.HeightScale = 0f; // relief 0
        Assert.Equal(0f, heights.SampleHeight(1f, 1f));
        Assert.True(heights.TryNativeQueryHeight(1f, 1f, out y)); Assert.Equal(130f * 4f, y);
        Assert.Equal(texels, heights.BuildNativeHeightTexels());

        var unknown = new TerrainHeightField(3, 3, samples, 6f, 2, 2, float.NaN);
        Assert.False(unknown.HasNativeQuery); Assert.False(unknown.TryNativeQueryHeight(1f, 1f, out _)); Assert.Null(unknown.BuildNativeHeightTexels());

        var legacy = new MapSceneObject("Tree", 256, 77, 256, 0, "a.sdl");
        Assert.Equal(130f * 4f, SceneObjectRenderer.NativeQueryY(legacy, heights));
        Assert.Equal(SceneObjectRenderer.UnknownNativeQueryY, SceneObjectRenderer.NativeQueryY(legacy, unknown));
        var saved = legacy with { HasNativeSavedPosition = true, WorldY = 321f };
        Assert.Equal(321f, SceneObjectRenderer.NativeQueryY(saved, heights));
        Assert.Equal(321f, SceneObjectRenderer.NativeQueryY(saved, unknown));
        Assert.Equal(SceneObjectRenderer.UnknownNativeQueryY, SceneObjectRenderer.NativeQueryY(saved with { WorldY = float.NaN }, heights));

        var sprite = new NativeSprite(4, 4, Enumerable.Repeat(0xFFFFFFFFu, 16).ToArray(), 2, 2, "t");
        var atlas = NativeSpriteAtlas.Pack([sprite]);
        float[] vertices = SceneObjectRenderer.BuildSpriteVertices([saved, legacy], [sprite, sprite], atlas, heights, Matrix4x4.CreateLookAt(new Vector3(8, 8, 8), Vector3.Zero, Vector3.UnitY));
        Assert.Equal(2 * 6 * SceneObjectRenderer.FloatsPerSpriteVertex, vertices.Length);
        for (int vertex = 0; vertex < 12; vertex++)
        {
            float nativeY = vertices[vertex * SceneObjectRenderer.FloatsPerSpriteVertex + 7];
            Assert.True(nativeY == 321f || nativeY == 130f * 4f, $"vertex {vertex} nativeY={nativeY}");
        }
        Assert.All(vertices, value => Assert.True(float.IsFinite(value)));
    }

    [Fact]
    public void Real_opengl_saved_instance_rgb_and_effective_radius_override_baseline_and_do_not_clamp()
    {
        SavedLightRun(.08f, (view, camera) =>
        {
            view.GameLightingEnabled = true; view.GameHour = 0;
            Color Sample(float tileX, params NativeLightInstance[] lights)
            {
                view.UpdateSavedSceneLights(lights.Length == 0 ? null : new SavedSceneLightSet(lights, null));
                using Bitmap frame = view.CaptureFrame(640, 480)!;
                return SavedLightPixel(frame, camera, tileX, 32, view.ReliefScale);
            }
            NativeLightInstance At(Vector3 rgb, float effective, float baseline)
                => NativeLightInstance.CreateSaved(0, new Vector3(32 * SavedLightTile, SavedLightNativeGroundY, 32 * SavedLightTile), rgb, effective, baseline);
            Color none = Sample(33);
            // effective 400／baseline 100：1 tile(256) 內受光；顛倒後（effective 100／baseline 400）不受光。
            Color wide = Sample(33, At(Vector3.One, 400, 100)), narrow = Sample(33, At(Vector3.One, 100, 400));
            Assert.True(wide.R > none.R * 2 && wide.G > none.G * 2, $"effective>baseline 應受光：{none}/{wide}");
            Assert.True(Math.Abs(narrow.R - none.R) <= 3 && Math.Abs(narrow.G - none.G) <= 3, $"effective<baseline 不應受光：{none}/{narrow}");
            // RGB 為保存值且不夾：2.0*0.59 → 通道飽和 1.0，比 1.0*0.59 亮；G、B 仍是環境光。
            Color one = Sample(33, At(new Vector3(1, 0, 0), 400, 400)), over = Sample(33, At(new Vector3(2, 0, 0), 400, 400));
            Assert.True(over.R > one.R * 1.4, $"RGB>1 應比 RGB=1 亮：{one}/{over}");
            Assert.True(Math.Abs(over.G - none.G) <= 3 && Math.Abs(one.B - none.B) <= 3, "未點亮的通道應維持環境光。");
            Assert.Equal(new Vector3(32 * SavedLightTile, SavedLightNativeGroundY, 32 * SavedLightTile), At(Vector3.One, 1, 1).WorldPosition);
        });
    }

    [Fact]
    public void Real_opengl_saved_light_distance_uses_native_Y_not_tile_scaled_Y()
    {
        SavedLightRun(.08f, (view, camera) =>
        {
            view.GameLightingEnabled = true; view.GameHour = 0;
            Assert.Equal(1f, view.ReliefScale); // 預覽 Y＝1.18 tile＝301 world；native Y＝200
            Color Sample(float lightY, float radius)
            {
                view.UpdateSavedSceneLights(new SavedSceneLightSet([NativeLightInstance.CreateSaved(0, new Vector3(32 * SavedLightTile, lightY, 32 * SavedLightTile), Vector3.One, radius)], null));
                using Bitmap frame = view.CaptureFrame(640, 480)!;
                return SavedLightPixel(frame, camera, 32, 32, view.ReliefScale);
            }
            Color full = Sample(SavedLightNativeGroundY, 400), above300 = Sample(SavedLightNativeGroundY + 300, 400), above500 = Sample(SavedLightNativeGroundY + 500, 400);
            Assert.True(full.R > above300.R * 1.5, $"Y 距離 0 vs 300：{full}/{above300}");
            Assert.True(above300.R > above500.R * 2, $"Y 距離 300（半徑內）vs 500（半徑外）：{above300}/{above500}");
            // 原生距離 0：半徑 150 與 1000 同樣全亮；若誤與 relief 縮放的 Y（301）混算，半徑 150 會明顯變暗。
            Color tight = Sample(SavedLightNativeGroundY, 150), loose = Sample(SavedLightNativeGroundY, 1000);
            Assert.True(Math.Abs(tight.R - loose.R) <= 4 && tight.R > 60, $"半徑 150／1000 應一致：{tight}/{loose}");
        });
    }

    [Fact]
    public void Real_opengl_saved_terrain_lighting_is_identical_for_relief_zero_and_nonzero()
    {
        SavedLightRun(.08f, (view, camera) =>
        {
            view.GameLightingEnabled = true; view.GameHour = 0;
            view.UpdateSavedSceneLights(new SavedSceneLightSet([NativeLightInstance.CreateSaved(0,
                new Vector3(32 * SavedLightTile, SavedLightNativeGroundY, 32 * SavedLightTile), Vector3.One, 300f)], null));
            var reference = new List<Color>();
            foreach (float relief in new[] { 1f, 0f, .5f, 2f })
            {
                view.SetReliefScale(relief);
                Assert.Equal(relief, view.ReliefScale);
                using Bitmap frame = view.CaptureFrame(640, 480)!;
                // Sample tile interiors: relief moves rasterized edges, where
                // this fixture has two different gray materials (110 / 120).
                Color[] samples = [SavedLightPixel(frame, camera, 32.2f, 32.2f, relief), SavedLightPixel(frame, camera, 32.9f, 32.2f, relief), SavedLightPixel(frame, camera, 32.2f, 33.5f, relief)];
                Assert.True(samples[0].R > 60 && samples[0].R > samples[1].R && samples[1].R > samples[2].R, $"relief={relief} 光暈中心／邊緣／外側 {samples[0]}/{samples[1]}/{samples[2]}");
                if (reference.Count == 0) reference.AddRange(samples);
                else for (int i = 0; i < samples.Length; i++)
                    Assert.True(Math.Abs(samples[i].R - reference[i].R) <= 4 && Math.Abs(samples[i].G - reference[i].G) <= 4 && Math.Abs(samples[i].B - reference[i].B) <= 4,
                        $"relief={relief} sample {i}: {samples[i]} vs relief=1 {reference[i]}");
            }
            Assert.Equal(1, view.LightDiagnostics.SavedRenderable);
        });
    }

    [Fact]
    public void Real_opengl_saved_sprite_lighting_queries_native_Y_for_relief_zero_and_nonzero()
    {
        SavedLightRun(.05f, (view, camera) =>
        {
            using var sprites = NativeSpriteCatalog.FromText(
                string.Join(',', Enumerable.Range(0, 60).Select(i => i switch { 0 => "42", 5 => "0", 8 => "-1", 14 => "-1", 52 => "House", _ => "0" })),
                "0000,red.alr", "", _ => SolidAlr(100, 160), _ => null);
            view.SpriteCatalog = sprites; view.ShowObjects = true; view.AnimationsEnabled = false;
            var legacy = new MapSceneObject("House", 32 * SavedLightTile, 0, 32 * SavedLightTile, 0, "test.sdl");
            var saved = legacy with { WorldY = 320f, HasNativeSavedPosition = true };
            // (物件, 光源 Y, 半徑, 預期 R 範圍)：legacy 查 raw*step＝200；saved 物件查自己的 WorldY＝320（而非地形 200）。
            (MapSceneObject Item, float LightY, float Radius, int Min, int Max)[] cases =
            [
                (legacy, 200f, 300f, 230, 255), (legacy, 700f, 300f, 25, 40),
                (saved, 520f, 350f, 140, 200), (saved, 700f, 300f, 25, 40)
            ];
            foreach (float relief in new[] { 0f, 1f })
            {
                view.SetReliefScale(relief);
                foreach (var (item, lightY, radius, min, max) in cases)
                {
                    view.UpdateSceneObjects([item]); Assert.Equal(1, view.SpriteObjectCount);
                    view.GameLightingEnabled = false; view.UpdateSavedSceneLights(null);
                    using Bitmap neutral = view.CaptureFrame(640, 480)!;
                    view.GameLightingEnabled = true; view.GameHour = 0;
                    view.UpdateSavedSceneLights(new SavedSceneLightSet([NativeLightInstance.CreateSaved(0,
                        new Vector3(32 * SavedLightTile, lightY, 32 * SavedLightTile), Vector3.One, radius)], null));
                    using Bitmap lit = view.CaptureFrame(640, 480)!;
                    int count = 0; long sum = 0;
                    for (int y = 0; y < neutral.Height; y++) for (int x = 0; x < neutral.Width; x++)
                    {
                        Color a = neutral.GetPixel(x, y);
                        if (a.R > 200 && a.G < 5 && a.B < 5) { count++; sum += lit.GetPixel(x, y).R; }
                    }
                    Assert.True(count > 500, $"relief={relief} sprite 像素 {count}");
                    double mean = (double)sum / count;
                    Assert.True(mean >= min && mean <= max, $"relief={relief} saved={item.HasNativeSavedPosition} lightY={lightY}: sprite R 平均 {mean:F1} 不在 {min}..{max}");
                }
            }
        });
    }

    [Fact]
    public void Real_opengl_same_coordinate_saved_instances_are_both_kept_and_combine_per_channel()
    {
        SavedLightRun(.08f, (view, camera) =>
        {
            view.GameLightingEnabled = true; view.GameHour = 0;
            Vector3 position = new(32 * SavedLightTile, SavedLightNativeGroundY, 32 * SavedLightTile);
            NativeLightInstance red = NativeLightInstance.CreateSaved(10, position, Vector3.UnitX, 400f, 400f);
            NativeLightInstance green = NativeLightInstance.CreateSaved(11, position, Vector3.UnitY, 400f, 400f);
            NativeLightInstance twin = NativeLightInstance.CreateSaved(12, position, Vector3.UnitX, 400f, 400f);
            view.UpdateSavedSceneLights(new SavedSceneLightSet([red, green], null));
            Assert.Equal(2, view.SceneLightCount); Assert.Equal(2, view.LightDiagnostics.SavedInstances);
            using (Bitmap frame = view.CaptureFrame(640, 480)!)
            {
                Assert.Equal(2, (int)typeof(Map3DViewControl).GetField("_selectedLightCount", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(view)!);
                Color centre = SavedLightPixel(frame, camera, 32, 32, view.ReliefScale);
                Assert.True(centre.R > centre.B * 2 && centre.G > centre.B * 2, $"同座標紅／綠光應同時存在（逐色道 max）：{centre}");
            }
            // 完全相同座標與 RGB 的兩個 instance 同樣不去重。
            view.UpdateSavedSceneLights(new SavedSceneLightSet([red, twin], null));
            Assert.Equal(2, view.SceneLightCount);
            Assert.Equal(position, red.WorldPosition); Assert.Equal(position, green.WorldPosition); // 原生 XYZ 不變
            view.SetReliefScale(0f); view.SetReliefScale(1.5f);
            Assert.Equal(position, red.WorldPosition); Assert.Equal(2, view.SceneLightCount);
        });
    }

    [Fact]
    public void Real_opengl_explicit_suppression_replaces_only_named_objects_and_keeps_legacy_synthesis()
    {
        SavedLightRun(.08f, (view, camera) =>
        {
            string objdef = LightingObjdef("Torch");
            using var context = new SceneLightingContext(null, NativeLightCatalog.Parse("[LightDefault]\n0,1,1,1,1,512"), NativeObjectLightingCatalog.Parse(objdef), null, null);
            view.LightingContext = context;
            var torch = new MapSceneObject("Torch", 10 * SavedLightTile, 512, 10 * SavedLightTile, 0, "Maps/A.sdl", ObjectIndex: 0);
            MapSceneObject[] scene = [torch, torch with { ObjectIndex = 1, WorldX = 12 * SavedLightTile }, torch with { ObjectIndex = -1, WorldX = 14 * SavedLightTile }];
            view.UpdateSceneObjects(scene);
            Assert.Equal(3, view.SceneLightCount);
            NativeLightInstance saved = NativeLightInstance.CreateSaved(1, new Vector3(5000, 100, 5000), new Vector3(1, .5f, .25f), 300f);

            // 只抑制 (A.SDL, 0)：鍵不分大小寫／分隔符；其餘（index 1 與無 index 的 legacy 物件）仍合成。
            view.UpdateSavedSceneLights(new SavedSceneLightSet([saved], [new SceneLightSuppressionKey("maps\\a.sdl", 0)]));
            Assert.Equal(3, view.SceneLightCount); // 2 合成 + 1 saved
            SceneLightDiagnostics diagnostics = view.LightDiagnostics;
            Assert.Equal((1, 1, 1, 2), (diagnostics.SavedInstances, diagnostics.SavedRenderable, diagnostics.SuppressedObjects, diagnostics.SynthesizedLights));
            Assert.Contains("不代表原遊戲像素等價", diagnostics.Boundary);

            // 無 saved instance、只有抑制鍵：仍視為有資料，被抑制物件不再合成。
            view.UpdateSavedSceneLights(new SavedSceneLightSet([], [new SceneLightSuppressionKey("MAPS/A.SDL", 0), new SceneLightSuppressionKey("Maps/A.sdl", 1)]));
            Assert.Equal(1, view.SceneLightCount); Assert.Equal(1, view.LightDiagnostics.SynthesizedLights); Assert.Equal(2, view.LightDiagnostics.SuppressedObjects);
            // 不同來源檔的同 index 不誤抑制。
            view.UpdateSavedSceneLights(new SavedSceneLightSet([], [new SceneLightSuppressionKey("Maps/B.sdl", 0)]));
            Assert.Equal(3, view.SceneLightCount); Assert.Equal(0, view.LightDiagnostics.SuppressedObjects);
            // 清除後恢復舊行為。
            view.UpdateSavedSceneLights(null);
            Assert.Equal(3, view.SceneLightCount); Assert.Equal(0, view.LightDiagnostics.SavedInstances);
            view.UpdateSavedSceneLights(SavedSceneLightSet.Empty);
            Assert.Equal(3, view.SceneLightCount);
        });
    }

    [Fact]
    public void Real_opengl_unknown_native_height_step_makes_saved_lights_inert_instead_of_using_terrain_value()
    {
        SavedLightRun(.08f, (view, camera) =>
        {
            view.GameLightingEnabled = true; view.GameHour = 0;
            var flat = Enumerable.Repeat((byte)SavedLightRaw, 257 * 257).ToArray();
            using (Bitmap none = view.CaptureFrame(640, 480)!)
            {
                Color ambient = SavedLightPixel(none, camera, 32, 32, view.ReliefScale);
                view.UpdateSavedSceneLights(new SavedSceneLightSet([NativeLightInstance.CreateSaved(0,
                    new Vector3(32 * SavedLightTile, SavedLightNativeGroundY, 32 * SavedLightTile), Vector3.One, 400f)], null));
                Assert.True(view.LightDiagnostics.NativeHeightKnown);
                using Bitmap known = view.CaptureFrame(640, 480)!;
                Assert.True(SavedLightPixel(known, camera, 32, 32, view.ReliefScale).R > ambient.R * 2);
                // 換成 step 未知的高度場（SetHeightSamples 沿用其 step）：terrain 的 native Y 不可由預覽高度冒充。
                FieldInfo field = typeof(Map3DViewControl).GetField("_heights", BindingFlags.Instance | BindingFlags.NonPublic)!;
                var old = (TerrainHeightField)field.GetValue(view)!;
                field.SetValue(view, new TerrainHeightField(old.Width, old.Height, flat, old.HeightScale, old.TileWidth, old.TileHeight, float.NaN));
                view.SetHeightSamples(flat);
                Assert.False(view.LightDiagnostics.NativeHeightKnown);
                using Bitmap unknown = view.CaptureFrame(640, 480)!;
                Color after = SavedLightPixel(unknown, camera, 32, 32, view.ReliefScale);
                Assert.True(Math.Abs(after.R - ambient.R) <= 3 && Math.Abs(after.G - ambient.G) <= 3, $"step 未知時 saved 光源應不作用：{ambient}/{after}");
            }
        });
    }

    private void SavedLightRun(float zoom, Action<Map3DViewControl, EditorCamera> body)
    {
        string map = CreateFixture(); WriteLightingDaynight(Path.Combine(map, "daynight.bmp"));
        RunInSta(() =>
        {
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.ThrowException, threadScope: true);
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Saved lights", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            form.Show(); Application.DoEvents(); Invoke(form, "SetActiveView", true); Application.DoEvents();
            var view = GetField<Map3DViewControl>(form, "_view3d");
            if (!view.IsReady) form.TryReloadDisplayResources(out _);
            if (!view.IsReady)
            {
                Assert.True(Environment.GetEnvironmentVariable("ARM_OPENGL_REQUIRED") != "1",
                    $"{view.LastFailureReason}; {view.ContextDescription}; " +
                    $"{typeof(MapEditorForm).GetField("_last3DDiagnostic", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(form)}; " +
                    GetField<ToolStripStatusLabel>(form, "_status").Text);
                return;
            }
            // 夜間環境光＋沒有 lightdef／objdef catalog：場景光源只來自測試提供的 saved instance。
            using var context = new SceneLightingContext(MapLightingDayNight.Parse(File.ReadAllBytes(Path.Combine(map, "daynight.bmp"))), null, null, null, null);
            view.LightingContext = context;
            view.ShowGrid = view.ShowObjects = false;
            view.SetHeightSamples(Enumerable.Repeat((byte)SavedLightRaw, 257 * 257).ToArray()); view.UpdateWater(-100, Color.Black);
            view.UpdateSceneObjects([]);
            view.FocusTile(32, 32);
            var camera = (EditorCamera)typeof(Map3DViewControl).GetField("_camera", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(view)!;
            camera.Zoom(zoom);
            try { body(view, camera); }
            finally { view.LightingContext = null; }
        }, TimeSpan.FromMinutes(3));
    }

    private static Color SavedLightPixel(Bitmap frame, EditorCamera camera, float tileX, float tileZ, float relief)
    {
        Point point = LightingScreenPoint(camera, new Vector3(tileX, 6f * relief * SavedLightRaw / 255f, tileZ), frame.Width, frame.Height);
        return frame.GetPixel(Math.Clamp(point.X, 0, frame.Width - 1), Math.Clamp(point.Y, 0, frame.Height - 1));
    }
}
