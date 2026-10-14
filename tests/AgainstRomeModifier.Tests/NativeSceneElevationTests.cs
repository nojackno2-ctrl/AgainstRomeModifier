using System.Buffers.Binary;
using System.Drawing;
using System.Numerics;
using System.Reflection;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.Nature;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed class NativeSceneElevationTests
{
    private static TerrainHeightField Asymmetric(float step = 4, float scale = 6)
        => new(2, 2, new byte[] { 10, 70, 150, 250 }, scale, 2, 2, step);

    private static MapSceneObject Native(float y)
        => new("Native", 128.125f, y, 320.5f, 0, "DATA/objects.dat") { HasNativeSavedPosition = true };

    [Theory]
    [InlineData(510.25f)]
    [InlineData(0f)]
    [InlineData(-127.5f)]
    public void Native_Y_is_absolute_in_editor_water_units_without_terrain_double_add(float y)
    {
        var heights = Asymmetric();
        var item = Native(y);
        Vector3 anchor = SceneObjectRenderer.GroundPoint(item, heights);
        Assert.Equal(item.WorldX / 256f, anchor.X);
        Assert.Equal(item.WorldZ / 256f, anchor.Z);
        Assert.Equal(y / 4f / 255f * 6f, anchor.Y, 5);
        Assert.NotEqual(heights.SampleHeight(anchor.X, anchor.Z), anchor.Y);
        Assert.Null(item.NativeVisualState); // Missing anim never suppresses a valid native position.
        Assert.Equal(anchor.Y + .25f, SceneObjectRenderer.BuildMarkerPoints([item], heights)[0].Y);
    }

    [Fact]
    public void Relative_child_own_Y_and_fractional_negative_XYZ_are_preserved()
    {
        var heights = Asymmetric(.5f);
        var root = Native(-20.125f) with { WorldX = -64.125f, WorldZ = -128.5f };
        var child = root with { WorldX = root.WorldX + .125f, WorldY = root.WorldY + 3.125f, WorldZ = root.WorldZ - .5f };
        var a = SceneObjectRenderer.GroundPoint(root, heights);
        var b = SceneObjectRenderer.GroundPoint(child, heights);
        Assert.Equal(3.125f / .5f / 255f * 6, b.Y - a.Y, 5);
        Assert.Equal(.125f / 256, b.X - a.X);
        Assert.Equal(-.5f / 256, b.Z - a.Z);
        foreach (var point in new[] { a, b })
            Assert.True(float.IsFinite(point.X) && float.IsFinite(point.Y) && float.IsFinite(point.Z));
    }

    [Theory]
    [InlineData(float.NaN, 100f)]
    [InlineData(float.PositiveInfinity, 100f)]
    [InlineData(float.NegativeInfinity, 100f)]
    [InlineData(0f, 100f)]
    [InlineData(-4f, 100f)]
    [InlineData(4f, float.NaN)]
    [InlineData(4f, float.PositiveInfinity)]
    [InlineData(4f, float.NegativeInfinity)]
    [InlineData(float.Epsilon, float.MaxValue)]
    public void Invalid_or_unrepresentable_native_height_falls_back_to_terrain(float step, float y)
    {
        var heights = Asymmetric(step);
        Vector3 point = SceneObjectRenderer.GroundPoint(Native(y), heights);
        Assert.Equal(heights.SampleHeight(point.X, point.Z), point.Y);
        Assert.True(float.IsFinite(point.Y));
    }

    [Fact]
    public void Legacy_SDL_and_new_canonical_placements_ignore_Y_even_with_visual_state()
    {
        var heights = Asymmetric();
        foreach (string source in new[] { "legacy.sdl", SdlPlacedObjectsFile.FileName })
        {
            var item = Native(800) with { HasNativeSavedPosition = false, SourceFile = source,
                NativeVisualState = new NativeAnimationVisualState(0, 0, 0, 0, 0) };
            var point = SceneObjectRenderer.GroundPoint(item, heights);
            Assert.Equal(heights.SampleHeight(point.X, point.Z), point.Y);
            Assert.Equal(point, SceneObjectRenderer.GroundPoint(item with { WorldY = -800 }, heights));
        }
        Assert.False(new MapSceneObject("Legacy", 1, 2, 3, 0, "a.sdl").HasNativeSavedPosition);
    }

    [Fact]
    public void Actual_sprite_cache_rebuilds_for_step_identity_scale_and_height_snapshot_changes()
    {
        MapSceneObject[] objects = [Native(510)];
        var sprite = new NativeSprite(16, 16, Enumerable.Repeat(0xFFFFFFFFu, 256).ToArray(), 8, 8, "test");
        NativeSprite?[] sprites = [sprite];
        var atlas = NativeSpriteAtlas.Pack([sprite]);
        var cache = new SceneSpriteGeometryBuffer();
        var view = Matrix4x4.CreateLookAt(new Vector3(8, 8, 8), Vector3.Zero, Vector3.UnitY);
        var heights = Asymmetric(4);
        float[] first = cache.Build(objects, sprites, atlas, heights, view).ToArray();
        Assert.Equal(3f, first[1]);
        // Same objects, sprites, samples, scale and camera: ONLY step (immutable field identity) changes.
        var changedStep = Asymmetric(2);
        Assert.Equal(first[1] * 2, cache.Build(objects, sprites, atlas, changedStep, view)[1]);
        Assert.Equal(SceneObjectRenderer.BuildSpriteVertices(objects, sprites, atlas, changedStep, view),
            cache.Build(objects, sprites, atlas, changedStep, view));
        changedStep.HeightScale = 3;
        Assert.Equal(3f, cache.Build(objects, sprites, atlas, changedStep, view)[1]);
        var changedSamples = new TerrainHeightField(2, 2, new byte[] { 250, 150, 70, 10 }, 3, 2, 2, 2);
        Assert.Equal(3f, cache.Build(objects, sprites, atlas, changedSamples, view)[1]);
        Assert.Equal(3f, cache.Build(objects, sprites, atlas, Asymmetric(4), view)[1]); // Reload.
    }

    [Fact]
    public void Sprite_vertices_marker_centres_and_picker_use_the_same_saved_height()
    {
        var heights = Asymmetric(.5f);
        var item = Native(255);
        var camera = new EditorCamera { Target = SceneObjectRenderer.GroundPoint(item, heights) };
        camera.Zoom(.1f);
        var view = camera.GetViewMatrix();
        var projection = camera.GetProjectionMatrix(1);
        var viewport = new Vector2(800, 800);
        var sprite = new NativeSprite(64, 64, Enumerable.Repeat(0xFFFFFFFFu, 4096).ToArray(), 32, 32, "test");
        var atlas = NativeSpriteAtlas.Pack([sprite]);
        float[] vertices = SceneObjectRenderer.BuildSpriteVertices([item], [sprite], atlas, heights, view);
        var anchor = SceneObjectRenderer.GroundPoint(item, heights);
        Assert.Equal(anchor, new Vector3(vertices[0], vertices[1], vertices[2]));
        Vector2 spriteCentre = ProjectElevation(anchor, view, projection, viewport);
        Assert.Equal(0, SceneObjectRenderer.PickObject([item], [sprite], _ => true, heights, view, projection, viewport, spriteCentre));
        var legacy = item with { HasNativeSavedPosition = false };
        Assert.Equal(-1, SceneObjectRenderer.PickObject([legacy], [sprite], _ => true, heights, view, projection, viewport, spriteCentre));
        Vector2 markerCentre = ProjectElevation(SceneObjectRenderer.BuildMarkerPoints([item], heights)[0], view, projection, viewport);
        Assert.Equal(0, SceneObjectRenderer.PickObject([item], [null], _ => false, heights, view, projection, viewport, markerCentre));
        Assert.Equal(-1, SceneObjectRenderer.PickObject([legacy], [null], _ => false, heights, view, projection, viewport, markerCentre));
    }

    [Fact]
    public void Sprite_painter_order_uses_saved_Y_in_view_depth()
    {
        var heights = Asymmetric();
        var low = Native(0);
        var high = low with { WorldY = 510 };
        var sprite = new NativeSprite(1, 1, [0xFFFFFFFFu], 0, 1, "test");
        var atlas = NativeSpriteAtlas.Pack([sprite]);
        var view = Matrix4x4.CreateLookAt(new Vector3(8, 8, 8), Vector3.Zero, Vector3.UnitY);
        var vertices = SceneObjectRenderer.BuildSpriteVertices([high, low], [sprite, sprite], atlas, heights, view);
        Assert.Equal(0f, vertices[1]);
        Assert.Equal(3f, vertices[6 * SceneObjectRenderer.FloatsPerSpriteVertex + 1]);
        Assert.Equal(vertices, new SceneSpriteGeometryBuffer().Build(new[] { high, low }, new NativeSprite?[] { sprite, sprite }, atlas, heights, view));
    }

    internal static Vector2 ProjectElevation(Vector3 point, Matrix4x4 view, Matrix4x4 projection, Vector2 viewport,
        Vector2 eyeOffset = default)
    {
        Vector4 eye = Vector4.Transform(new Vector4(point, 1), view) + new Vector4(eyeOffset, 0, 0);
        Vector4 clip = Vector4.Transform(eye, projection);
        return new((clip.X / clip.W * .5f + .5f) * viewport.X, (.5f - clip.Y / clip.W * .5f) * viewport.Y);
    }
}

// Use the existing actual host fixtures, closed native family and framebuffer helpers.
public sealed partial class MapEditorSaveTransactionTests
{
    private static T ElevationViewField<T>(Map3DViewControl view, string name)
        => (T)typeof(Map3DViewControl).GetField(name, BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(view)!;

    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Host_original_DATA_and_nature_positions_remain_native_when_animation_is_missing(bool pfil)
    {
        string map = CreateFixture();
        var pools = new NativeVisualFixture(map, pfil);
        foreach (int slot in new[] { 0, 2, 4 })
            BinaryPrimitives.WriteUInt16LittleEndian(pools.Objects.AsSpan(16 + slot * LevelObjectStore.RecordSize + 71), ushort.MaxValue);
        pools.WriteObjects();
        var before = SnapshotDirectory(map);
        RunInSta(() =>
        {
            using var form = NativeVisualForm(map);
            Invoke(form, "LoadLevelObjects", map);
            var native = NativeVisualScene(form);
            Assert.Equal(3, native.Count);
            Assert.All(native, item => { Assert.True(item.HasNativeSavedPosition); Assert.Null(item.NativeVisualState); });
            var heights = new TerrainHeightField(2, 2, new byte[] { 150, 200, 70, 250 }, nativeHeightMapStep: 4);
            Assert.All(native, item => Assert.Equal(0f, SceneObjectRenderer.GroundPoint(item, heights).Y));
            var template = LevelObjectStore.Load(map).OwnedTemplate(4, 300)!;
            GetField<NatureEditSession>(form, "_natureSession").Plant(new NatureAddition(template, "LanGerNad18", 123, 600, 456, 1));
            Assert.False(Assert.Single(NativeVisualScene(form), item => item.ObjectIndex == -200000).HasNativeSavedPosition);
            Type editMode = typeof(MapEditorForm).GetNestedType("EditMode", BindingFlags.NonPublic)!;
            typeof(MapEditorForm).GetField("_editMode", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, Enum.Parse(editMode, "Nature"));
            var natureScene = (IReadOnlyList<MapSceneObject>)Invoke(form, "SceneObjectsFor3D", Invoke(form, "EffectiveSceneObjects")!)!;
            Assert.True(Assert.Single(natureScene, item => item.ObjectIndex == -100004).HasNativeSavedPosition);
            Assert.False(Assert.Single(natureScene, item => item.ObjectIndex == -200000).HasNativeSavedPosition);
            Assert.All(SdlSceneCatalog.LoadDirectory(map), item => Assert.False(item.HasNativeSavedPosition));
        });
        AssertNativeVisualSnapshot(map, before);
    }

    [Fact]
    public void Native_elevated_shadow_stays_on_asymmetric_ground()
    {
        var heights = new TerrainHeightField(2, 2, new byte[] { 10, 70, 150, 250 }, tileWidth: 64, tileHeight: 64, nativeHeightMapStep: 4);
        using var shadows = NativeShadowCatalog.FromText(ShadowObjdef, "[ShadowNames]\r\n0000,box.bmp\r\n", _ => ShadowBmp(4, 200));
        var shadow = shadows.GetShadow("BauRomHau00_Haupthaus")!;
        var mask = new NativeSprite(4, 4, new uint[16], 0, 0, "box.bmp");
        var atlas = NativeSpriteAtlas.Pack([mask]);
        var item = new MapSceneObject("BauRomHau00_Haupthaus", 2560.125f, 5100, 5120.5f, 0, "DATA/objects.dat") { HasNativeSavedPosition = true };
        float[] Build(MapSceneObject value) => SceneObjectRenderer.BuildShadowVertices([value], [shadow], _ => mask, atlas, heights);
        var elevated = Build(item);
        Assert.NotEmpty(elevated);
        Assert.Equal(Build(item with { WorldY = -5100 }), elevated);
        Assert.Equal(Build(item with { HasNativeSavedPosition = false }), elevated);
        for (int i = 0; i < elevated.Length; i += SceneObjectRenderer.FloatsPerShadowVertex)
            Assert.Equal(heights.SampleHeight(elevated[i], elevated[i + 2]) + .03f, elevated[i + 1]);
    }

    [Theory]
    [InlineData("2", 2f)]
    [InlineData("NaN", float.NaN)]
    [InlineData("0", float.NaN)]
    [InlineData("-4", float.NaN)]
    [InlineData("Infinity", float.NaN)]
    public void Host_height_step_relief_update_and_reload_keep_saved_height_context(string configuredStep, float expectedStep)
    {
        string map = CreateFixture("ENDL_012");
        SetupPreviewFamily(map, pfil: false, fractional: true);
        string ini = Path.Combine(map, "boden.ini");
        File.WriteAllText(ini, File.ReadAllText(ini).Replace("[Heightmapstep]\r\n4", "[Heightmapstep]\r\n" + configuredStep));
        RunInSta(() =>
        {
            using var form = FamilyPreviewForm(map);
            var view = GetField<Map3DViewControl>(form, "_view3d");
            var heights = ElevationViewField<TerrainHeightField>(view, "_heights");
            Assert.Equal(expectedStep, heights.NativeHeightMapStep);
            var root = Assert.Single(FamilyPreviewScene(form), item => item.ObjectIndex == -100000);
            Assert.True(root.HasNativeSavedPosition);
            view.SetReliefScale(.5f);
            Assert.Equal(3f, heights.HeightScale);
            view.SetHeightSamples(Enumerable.Repeat((byte)200, heights.Width * heights.Height).ToArray());
            var updated = ElevationViewField<TerrainHeightField>(view, "_heights");
            Assert.NotSame(heights, updated);
            Assert.Equal(expectedStep, updated.NativeHeightMapStep);
            Assert.Equal(3f, updated.HeightScale);
            var point = SceneObjectRenderer.GroundPoint(root, updated);
            Assert.Equal(float.IsNaN(expectedStep) ? 200f / 255f * 3 : root.WorldY / expectedStep / 255f * 3, point.Y, 5);
            // Change ONLY ini step. Host reload must not reuse the same samples' sprite-cache entry.
            File.WriteAllText(ini, File.ReadAllText(ini).Replace("[Heightmapstep]\r\n" + configuredStep, "[Heightmapstep]\r\n8"));
            Invoke(form, "LoadSelectedMap");
            var reloaded = ElevationViewField<TerrainHeightField>(view, "_heights");
            Assert.NotSame(updated, reloaded);
            Assert.Equal(8f, reloaded.NativeHeightMapStep);
            Assert.Equal(root.WorldY / 8f / 255f * reloaded.HeightScale, SceneObjectRenderer.GroundPoint(root, reloaded).Y, 5);
        });
    }

    [Fact]
    public void Real_host_native_family_height_framebuffer_and_child_pick_match_absolute_anchors()
    {
        string map = CreateFixture("ENDL_012");
        SetupPreviewFamily(map, pfil: false, fractional: true);
        string ini = Path.Combine(map, "boden.ini");
        File.WriteAllText(ini, File.ReadAllText(ini).Replace("[Heightmapstep]\r\n4", "[Heightmapstep]\r\n0.05")
            .Replace("[Waterlevel]\r\n120", "[Waterlevel]\r\n0"));
        var before = SnapshotDirectory(map);
        RunInSta(() =>
        {
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.ThrowException, threadScope: true);
            OpenTK.Windowing.Desktop.GLFWProvider.CheckForMainThread = false;
            using var form = FamilyPreviewForm(map);
            typeof(MapEditorForm).GetField("_allowClose", BindingFlags.Instance | BindingFlags.NonPublic)!.SetValue(form, true);
            form.StartPosition = FormStartPosition.Manual; form.Location = new Point(-30000, -30000);
            form.Show(); Application.DoEvents();
            Invoke(form, "SetActiveView", true); Application.DoEvents();
            var view = GetField<Map3DViewControl>(form, "_view3d");
            if (!view.IsReady) { Assert.NotEqual("1", Environment.GetEnvironmentVariable("ARM_OPENGL_REQUIRED")); return; }
            form.SelectNativeSceneObject(0); form.CopyActiveNativeFamily();
            var copy = form.PlacementSession[0];
            form.PlacementSession.Edit(0, copy with { WorldX = 8192.125f, WorldZ = 8192.5f });
            Invoke(form, "PushSceneObjects", Invoke(form, "EffectiveSceneObjects")!);
            var scene = ElevationViewField<IReadOnlyList<MapSceneObject>>(view, "_objects");
            var members = scene.Where(item => item.ObjectIndex == -5000).ToArray();
            Assert.Equal(2, members.Length);
            Assert.All(members, item => Assert.True(item.HasNativeSavedPosition));
            Assert.Equal(3.125f, members[1].WorldY - members[0].WorldY);
            // Eight identical red frames keep saved frames 3/7 valid and animation enabled during captures.
            byte[] singleFrame = SolidAlr(64, 40);
            const int headerLength = 23 * sizeof(uint);
            byte[] alr = new byte[headerLength + 8 * (singleFrame.Length - headerLength)];
            singleFrame.AsSpan(0, headerLength).CopyTo(alr);
            BinaryPrimitives.WriteUInt32LittleEndian(alr.AsSpan(3 * sizeof(uint)), 8);
            BinaryPrimitives.WriteUInt32LittleEndian(alr.AsSpan(6 * sizeof(uint)), 8);
            for (int frameIndex = 0; frameIndex < 8; frameIndex++)
                singleFrame.AsSpan(headerLength).CopyTo(alr.AsSpan(headerLength + frameIndex * (singleFrame.Length - headerLength)));
            string Definition(int id, string name) => string.Join(",", Enumerable.Range(0, 60).Select(i => i switch
                { 0 => id.ToString(), 2 => "1000", 5 => "0", 8 => "-1", 14 => "-1", 17 => "1", 52 => name, _ => "0" }));
            using var catalog = NativeSpriteCatalog.FromText(Definition(555, "BauGerTest") + "\n" + Definition(556, "BauGerChild"),
                "0000,big.alr", "", _ => alr, _ => null);
            view.SpriteCatalog = catalog; view.ShowGrid = false;
            Assert.True(view.AnimationsEnabled);
            Assert.True(view.AnimatedObjectCount >= 2);
            var heights = ElevationViewField<TerrainHeightField>(view, "_heights");
            Assert.Equal(.05f, heights.NativeHeightMapStep);
            var camera = ElevationViewField<EditorCamera>(view, "_camera");
            camera.Target = (SceneObjectRenderer.GroundPoint(members[0], heights) + SceneObjectRenderer.GroundPoint(members[1], heights)) / 2;
            camera.Zoom(.05f);
            var size = view.ClientSize;
            using Bitmap frame = view.CaptureFrame(size.Width, size.Height)!;
            Assert.True(Red(frame).Count > 100);
            var sprites = ElevationViewField<NativeSprite?[]>(view, "_objectSprites");
            foreach (var member in members)
            {
                int index = Enumerable.Range(0, scene.Count).Single(i => ReferenceEquals(scene[i], member));
                var sprite = sprites[index]!;
                var offset = new Vector2((sprite.Width / 2f - sprite.AnchorX) * SceneObjectRenderer.TilesPerSpritePixel,
                    (sprite.AnchorY - sprite.Height / 2f) * SceneObjectRenderer.TilesPerSpritePixel);
                Vector2 centre = NativeSceneElevationTests.ProjectElevation(SceneObjectRenderer.GroundPoint(member, heights),
                    camera.GetViewMatrix(), camera.GetProjectionMatrix(size.Width / (float)size.Height), new(size.Width, size.Height), offset);
                var pixel = new Point((int)MathF.Round(centre.X), (int)MathF.Round(centre.Y));
                Assert.InRange(pixel.X, 0, size.Width - 1); Assert.InRange(pixel.Y, 0, size.Height - 1);
                var color = frame.GetPixel(pixel.X, pixel.Y);
                Assert.True(color.R > 200 && color.G < 60 && color.B < 60, $"Native member centre missing at {pixel}: {color}");
                Assert.Equal(index, view.PickSceneObject(pixel));
                Invoke(form, "SelectPickedSceneObject", scene[view.PickSceneObject(pixel)]);
                Assert.True(form.PlacedList.Items[0].Selected); // Child and root share the copy's selection identity.
            }
            // Same host objects with legacy grounding must move the actual framebuffer away from these anchors.
            view.UpdateSceneObjects(scene.Select(item => item with { HasNativeSavedPosition = false }).ToArray());
            using Bitmap grounded = view.CaptureFrame(size.Width, size.Height)!;
            Assert.True(Red(grounded).Count < 50);
            view.UpdateSceneObjects(scene);
            view.SpriteCatalog = null;
        }, TimeSpan.FromMinutes(2));
        AssertNativeVisualSnapshot(map, before);
    }
}
