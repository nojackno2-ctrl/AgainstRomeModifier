using System.Numerics;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed class SceneGeometryBufferTests
{
    [Fact]
    public void Reusable_geometry_matches_original_order_anchors_uv_and_refreshes_animation_rotation_height_and_removal()
    {
        var first = new NativeSprite(4, 10, new uint[40], 1, 8, "first");
        var second = new NativeSprite(8, 12, new uint[96], 2, 9, "second");
        var atlas = NativeSpriteAtlas.Pack([first, second]);
        MapSceneObject[] objects = [new("Near", 2560, 0, 2560, 0, "a"), new("Far", 12800, 0, 12800, 0, "a")];
        NativeSprite?[] sprites = [first, second];
        var heights = new TerrainHeightField(65, 65, Enumerable.Repeat((byte)70, 65 * 65).ToArray(), tileWidth: 64, tileHeight: 64);
        var buffer = new SceneSpriteGeometryBuffer();
        Matrix4x4 view = Matrix4x4.CreateLookAt(new(0, 10, 0), new(32, 0, 32), Vector3.UnitY);
        void Check() => Assert.Equal(SceneObjectRenderer.BuildSpriteVertices(objects, sprites, atlas, heights, view), buffer.Build(objects, sprites, atlas, heights, view));
        Check(); var allocated = buffer.Build(objects, sprites, atlas, heights, view);
        Matrix4x4 pan = view; pan.M41 += 5; pan.M42 -= 3; pan.M43 += 7;
        Assert.Same(allocated, buffer.Build(objects, sprites, atlas, heights, pan));
        Assert.Equal(SceneObjectRenderer.BuildSpriteVertices(objects, sprites, atlas, heights, pan), buffer.Build(objects, sprites, atlas, heights, pan));
        Matrix4x4 noisyPan = pan; noisyPan.M13 += 0.0000001f;
        Assert.Equal(SceneObjectRenderer.BuildSpriteVertices(objects, sprites, atlas, heights, noisyPan), buffer.Build(objects, sprites, atlas, heights, noisyPan));
        sprites[0] = second; Check(); // same array, changed animation frame
        view = Matrix4x4.CreateLookAt(new(64, 20, 64), new(32, 0, 32), Vector3.UnitY); Check();
        heights.HeightScale = 12; Check();
        heights = new TerrainHeightField(65, 65, new byte[65 * 65], tileWidth: 64, tileHeight: 64); Check();
        objects = [objects[0] with { WorldX = 5120 }]; sprites = [first]; Check();
        sprites[0] = null; Check(); Assert.Empty(buffer.Build(objects, sprites, atlas, heights, view));
    }

    [Fact]
    public void Warm_reusable_geometry_allocates_nothing_even_when_all_7000_animation_frames_change_and_camera_rotates()
    {
        var first = new NativeSprite(4, 4, new uint[16], 2, 4, "first"); var second = first with { AssetName = "second" };
        var atlas = NativeSpriteAtlas.Pack([first, second]);
        var objects = Enumerable.Range(0, 7000).Select(i => new MapSceneObject("Unit", (i % 64) * 256, 0, (i / 64) * 128, 0, "a")).ToArray();
        NativeSprite?[] sprites = Enumerable.Repeat<NativeSprite?>(first, objects.Length).ToArray();
        var heights = new TerrainHeightField(65, 65, new byte[65 * 65], tileWidth: 64, tileHeight: 64);
        var buffer = new SceneSpriteGeometryBuffer();
        var views = new[] { Matrix4x4.CreateLookAt(new(0, 10, 0), new(32, 0, 32), Vector3.UnitY), Matrix4x4.CreateLookAt(new(64, 10, 64), new(32, 0, 32), Vector3.UnitY) };
        for (int i = 0; i < 10; i++) { Array.Fill(sprites, i % 2 == 0 ? first : second); buffer.Build(objects, sprites, atlas, heights, views[i % 2]); }
        long before = GC.GetAllocatedBytesForCurrentThread();
        for (int i = 0; i < 30; i++) { Array.Fill(sprites, i % 2 == 0 ? first : second); buffer.Build(objects, sprites, atlas, heights, views[i % 2]); }
        Assert.Equal(0, GC.GetAllocatedBytesForCurrentThread() - before);
    }
}

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Shadow_geometry_cache_refreshes_for_object_movement_terrain_and_relief_changes()
    {
        var heights = new TerrainHeightField(65, 65, Enumerable.Repeat((byte)80, 65 * 65).ToArray(), tileWidth: 64, tileHeight: 64);
        using var catalog = NativeShadowCatalog.FromText(ShadowObjdef, "[ShadowNames]\r\n0000,box.bmp\r\n", _ => ShadowBmp(4, 200));
        var shadow = catalog.GetShadow("BauRomHau00_Haupthaus")!;
        var mask = new NativeSprite(4, 4, new uint[16], 0, 0, "box.bmp"); var atlas = NativeSpriteAtlas.Pack([mask]);
        MapSceneObject[] objects = [new("House", 8192, 0, 8192, 0, "a")]; NativeObjectShadow?[] shadows = [shadow];
        var cache = new SceneShadowGeometryCache(); NativeSprite? Mask(NativeObjectShadow _) => mask;
        void Check() => Assert.Equal(SceneObjectRenderer.BuildShadowVertices(objects, shadows, Mask, atlas, heights), cache.Build(objects, shadows, Mask, atlas, heights));
        Check(); var original = cache.Build(objects, shadows, Mask, atlas, heights);
        Assert.Same(original, cache.Build(objects, shadows, Mask, atlas, heights));
        heights.HeightScale = 12; Check(); Assert.NotSame(original, cache.Build(objects, shadows, Mask, atlas, heights));
        objects = [objects[0] with { WorldX = 4096 }]; Check();
        heights = new TerrainHeightField(65, 65, new byte[65 * 65], tileWidth: 64, tileHeight: 64); Check();
        shadows = [null]; Check(); Assert.Empty(cache.Build(objects, shadows, Mask, atlas, heights));
    }
}
