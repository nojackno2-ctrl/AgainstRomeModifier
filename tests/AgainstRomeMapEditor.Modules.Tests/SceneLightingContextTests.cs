using System.Buffers.Binary;
using System.Numerics;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier;
using AgainstRomeModifier.Maps;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class SceneLightingContextTests
{
    [Fact]
    public void Scene_lights_use_objdef_indices_APT_offsets_and_world_heights_without_flicker()
    {
        var objects = NativeObjectLightingCatalog.Parse(Objdef(42, "House", 3, 7, 15, 1, 30));
        Assert.True(objects.TryGetDefinition(42, out var definition));
        Assert.Equal((3, 7, 15, 1, 30), (definition.AptIndex, definition.LightDefIndex,
            definition.LightHeightOffset, definition.AptLightDefIndex, definition.AptLightHeightOffset));
        var lights = NativeLightCatalog.Parse("[LightDefault]\n1,1,1.48,1.22,0,350,1,.05,0,Coal\n7,1,1.33,.61,.33,300,1,.15,0,Fire");
        int reads = 0;
        using var context = new SceneLightingContext(null, lights, objects, new Dictionary<int, string> { [3] = "house.apt" },
            _ => { reads++; return Apt((96, 48), (-128, 64)); });
        var house = new MapSceneObject("house", 10624, 75, 10112, 0, "house.sdl");
        float Ground(float x, float z) => (x - 10624) / 2 + (z - 10112) / 4;
        var scene = context.CollectSceneLights([house, house with { Name = "Unknown" }], Ground);
        Assert.Equal(3, scene.Count);
        Assert.Equal(new Vector3(10816, 126, 10112), scene[0].WorldPosition);
        Assert.Equal(new Vector3(10624, 94, 10368), scene[1].WorldPosition);
        Assert.Equal(new Vector3(10624, 90, 10112), scene[2].WorldPosition); // position.Y + lihei
        Assert.Equal(350, scene[0].Radius);
        Assert.Equal(new Vector3(1.48f, 1.22f, 0), scene[0].Color);
        Assert.Equal(300, scene[2].Radius);
        Assert.Equal(1, reads);
        // 移動物件只重算位置；同 APT 不重新載入，不套用 FlickerType 的即時擾動。
        var moved = context.CollectSceneLights([house with { WorldX = 11000 }], Ground);
        Assert.Equal(11192, moved[0].WorldPosition.X);
        Assert.Equal(scene[0].Color, moved[0].Color);
        Assert.Equal(1, reads);
    }

    [Fact]
    public void Scene_lights_skip_inactive_missing_or_invalid_definitions_and_cache_bad_APT()
    {
        var objects = NativeObjectLightingCatalog.Parse(string.Join('\n',
            Objdef(1, "BrokenApt", 3, 0, 0, 0, 0), Objdef(2, "Inactive", -1, 1, 0, -1, 0),
            Objdef(3, "Missing", -1, 2, 0, -1, 0), Objdef(4, "ZeroRadius", -1, 3, 0, -1, 0),
            Objdef(5, "NotFinite", -1, 4, 0, -1, 0)));
        var lights = new NativeLightCatalog([
            NativeLightDefinition.CreateDefault(), new() { Index = 1, IsActive = false, Radius = 500, Color = Vector3.One },
            new() { Index = 3, IsActive = true, Radius = 0, Color = Vector3.One },
            new() { Index = 4, IsActive = true, Radius = float.NaN, Color = Vector3.One }]);
        int reads = 0;
        using var context = new SceneLightingContext(null, lights, objects, new Dictionary<int, string> { [3] = "bad.apt" },
            _ => { reads++; throw new InvalidDataException("broken archive"); });
        MapSceneObject[] scene = new[] { "BrokenApt", "Inactive", "Missing", "ZeroRadius", "NotFinite" }
            .Select(name => new MapSceneObject(name, 0, 0, 0, 0, "test.sdl")).ToArray();
        Assert.Single(context.CollectSceneLights(scene, (_, _) => 0));
        Assert.Single(context.CollectSceneLights(scene, (_, _) => 0));
        Assert.Equal(1, reads);
    }

    [Fact]
    public void Ambient_uses_only_row_zero_native_floor_weights_and_wraps_at_24()
    {
        var dayNight = MapLightingDayNight.Parse(Daynight());
        using var context = new SceneLightingContext(dayNight, null, null, null, null);
        Assert.Equal(new Vector3(20, 102, 198) / 256, context.GetAmbientColor(2));
        Assert.Equal(new Vector3(25, 102, 197) / 256, context.GetAmbientColor(2.5f));
        // minute=1 的權重獨立 floor，總和 65535，不能直接浮點 lerp。
        Assert.Equal(new Vector3(20, 102, 197) / 256, context.GetAmbientColor(2.02f));
        Assert.Equal(new Vector3(115, 111, 188) / 256, context.GetAmbientColor(23.5f));
        Assert.Equal(context.GetAmbientColor(0), context.GetAmbientColor(24));
        Assert.Equal(context.GetAmbientColor(0), context.GetAmbientColor(-1));
        Assert.Equal(context.GetAmbientColor(12), context.GetAmbientColor(float.NaN));
    }

    [Fact]
    public void Missing_sources_keep_neutral_ambient_and_no_local_lights()
    {
        using var context = new SceneLightingContext(null, null, null, null, null);
        Assert.Equal(Vector3.One, context.GetAmbientColor(0));
        Assert.Empty(context.CollectSceneLights([new("Unknown", 0, 0, 0, 0, "test.sdl")], (_, _) => 0));
    }

    [Fact]
    public void Loading_is_read_only_fail_soft_and_supports_PFIL_and_case_insensitive_archives()
    {
        string root = Path.Combine(Path.GetTempPath(), "ArmLighting_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(Path.Combine(root, "SYSTEM", "DATA_MP", "DEFAULTS"));
        try
        {
            string defaults = Path.Combine(root, "SYSTEM", "DATA_MP", "DEFAULTS");
            byte[] objdef = Pfil(Objdef(1, "House", 3, -1, 0, 1, 30));
            File.WriteAllBytes(Path.Combine(defaults, "objdef.dau"), objdef);
            File.WriteAllBytes(Path.Combine(defaults, "lightdef.dau"), Pfil("[LightDefault]\n1,1,1.48,1.22,0,350"));
            File.WriteAllBytes(Path.Combine(root, "SYSTEM", "cl_apt.ini"), Pfil("0003,house.apt"));
            File.WriteAllBytes(Path.Combine(root, "daynight.bmp"), Daynight());
            using (var zip = System.IO.Compression.ZipFile.Open(Path.Combine(root, "apt.dat"), System.IO.Compression.ZipArchiveMode.Create))
                using (var stream = zip.CreateEntry("system/data/apt/HOUSE.APT").Open()) stream.Write(Apt((96, 48)));
            var before = Directory.GetFiles(root, "*", SearchOption.AllDirectories).ToDictionary(path => path, File.ReadAllBytes);
            using (var context = SceneLightingContext.TryCreate(root, root))
            {
                Assert.NotNull(context);
                Assert.True(context.IsDayNightAvailable); Assert.True(context.IsLightCatalogAvailable);
                Assert.Single(context.CollectSceneLights([new("House", 100, 0, 200, 0, "test.sdl")], (_, _) => 0));
            }
            foreach (var file in before) Assert.Equal(file.Value, File.ReadAllBytes(file.Key));
            File.WriteAllText(Path.Combine(root, "daynight.bmp"), "broken");
            File.WriteAllText(Path.Combine(root, "apt.dat"), "broken");
            using var partial = SceneLightingContext.TryCreate(root, root);
            Assert.NotNull(partial); Assert.False(partial.IsDayNightAvailable);
            Assert.Equal(Vector3.One, partial.GetAmbientColor(0));
            Assert.Empty(partial.CollectSceneLights([new("House", 100, 0, 200, 0, "test.sdl")], (_, _) => 0));
            Assert.Null(SceneLightingContext.TryCreate(Path.Combine(root, "absent"), Path.Combine(root, "absent")));
        }
        finally { Directory.Delete(root, true); }
    }

    private static string Objdef(int id, string name, int apt, int light, int height, int aptLight, int aptHeight)
        => string.Join(',', Enumerable.Range(0, 164).Select(i => i switch
        { 0 => id.ToString(), 14 => apt.ToString(), 52 => name, 66 => light.ToString(), 67 => height.ToString(), 162 => aptLight.ToString(), 163 => aptHeight.ToString(), _ => "0" }));

    private static byte[] Pfil(string text)
    {
        byte[] header = new byte[64]; "PFIL"u8.CopyTo(header);
        return GameLZSS.CompressPfil(MapTextEncoding.Game.GetBytes(text), header);
    }

    private static byte[] Apt(params (int X, int Y)[] deltas)
    {
        using var buffer = new MemoryStream();
        using var writer = new BinaryWriter(buffer);
        uint[] header = new uint[28]; header[0] = 0x54415041; header[1] = 3; header[3] = 112; header[9] = 1;
        foreach (uint value in header) writer.Write(value);
        for (int i = 0; i < 62; i++) writer.Write(0);
        writer.Write(0); writer.Write(0); writer.Write(569); writer.Write(405);
        writer.Write(deltas.Length); writer.Write(0); writer.Write(0);
        foreach (var point in deltas) { writer.Write(point.X + 569); writer.Write(point.Y + 405); }
        return buffer.ToArray();
    }

    private static byte[] Daynight()
    {
        byte[] bytes = new byte[54 + 72 * 6]; bytes[0] = (byte)'B'; bytes[1] = (byte)'M';
        foreach (var (offset, value) in new[] { (2, bytes.Length), (10, 54), (14, 40), (18, 24), (22, -6), (34, 72 * 6) })
            BinaryPrimitives.WriteInt32LittleEndian(bytes.AsSpan(offset), value);
        BinaryPrimitives.WriteUInt16LittleEndian(bytes.AsSpan(26), 1); BinaryPrimitives.WriteUInt16LittleEndian(bytes.AsSpan(28), 24);
        for (int y = 0; y < 6; y++) for (int x = 0; x < 24; x++)
        {
            int i = 54 + y * 72 + x * 3;
            bytes[i] = y == 0 ? (byte)(200 - x) : (byte)255;
            bytes[i + 1] = y == 0 ? (byte)(100 + x) : (byte)255;
            bytes[i + 2] = y == 0 ? (byte)(10 * x) : (byte)255;
        }
        return bytes;
    }
}
