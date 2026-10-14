using System.Numerics;
using System.Security.Cryptography;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed class NativeLightDefinitionsTests
{
    [Fact]
    public void Omitted_fields_use_inactive_native_initial_values_and_other_sections_are_ignored()
    {
        var values = NativeLightDefinitions.Parse("[Other]\n4,1\n[LightDefault]\n2\n[Other]\n3,1");
        Assert.Single(values);
        Assert.Equal(NativeLightTypeDefinition.Initial(2), values[2]);
        Assert.Throws<InvalidDataException>(() => values[2].Create(Vector3.Zero));
    }

    [Fact]
    public void Partial_overlay_preserves_fields_and_native_integer_widths_and_name_token()
    {
        var values = NativeLightDefinitions.Parse("[LightDefault]\n1,-2,1.5,.2,.3,400,65535,.1,-1,abcdefghijklmnopqrstuvwxyz123456789 suffix\n1,1,.9\n2,,1");
        var value = values[1];
        Assert.Equal(new Vector3(.9f, .2f, .3f), value.Color);
        Assert.Equal(400, value.Radius); Assert.Equal((short)-1, value.FlickerType);
        Assert.Equal(uint.MaxValue, value.SpecialFx);
        Assert.Equal("abcdefghijklmnopqrstuvwxyz1234", value.Name);
        Assert.Equal(1, values[2].Active); // comma run skipped, not an empty field
        var pool = NativeLightPool.ParsePayload(NativeLightPoolTests.EmptyPool());
        Assert.Equal(new[] { 0 }, pool.AddBatch(new[] { value.Create(new(20, 30, 40), 7, 8) }));
        Assert.Equal(new NativeSavedLight(0, 1, new(20, 30, 40), 7, 8, 1), Assert.Single(pool.ActiveLights()));
    }

    [Theory]
    [InlineData("256,1")]
    [InlineData("-1,1")]
    [InlineData("1,2147483648")]
    [InlineData("1,1,NaN")]
    [InlineData("1,1,Infinity")]
    [InlineData("1,1,1e99")]
    [InlineData("1,1, ,1")]
    [InlineData("1,1,1,1,1,500,0,0,0, ,extra")]
    public void Invalid_present_field_is_rejected(string row)
        => Assert.Throws<InvalidDataException>(() => NativeLightDefinitions.Parse("[LightDefault]\n" + row));

    [Fact]
    public void Configured_literal_prefix_layers_apply_through_d09_and_do_not_read_d10()
    {
        string root = Path.Combine(Path.GetTempPath(), "ArmLightDefinitions_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(Path.Combine(root, "SYSTEM"));
        try
        {
            File.WriteAllText(Path.Combine(root, "SYSTEM", "clengine.ini"), "[DefaultPath]\nCUSTOM");
            File.WriteAllText(Path.Combine(root, "CUSTOMlightdef.dau"), "[LightDefault]\n1,1,.2,.4,.6,350,2,.15,7,Torch");
            File.WriteAllText(Path.Combine(root, "CUSTOMlightdef.d00"), "[LightDefault]\n1,0");
            File.WriteAllText(Path.Combine(root, "CUSTOMlightdef.d09"), "[LightDefault]\n1,-1,.8");
            File.WriteAllText(Path.Combine(root, "CUSTOMlightdef.d10"), "[LightDefault]\n1,invalid");
            var value = NativeLightDefinitions.Load(root)[1];
            Assert.Equal(-1, value.Active); Assert.Equal(new Vector3(.8f, .4f, .6f), value.Color);
            Assert.Equal("Torch", value.Name); Assert.Equal(350, value.Radius);
        }
        finally { Directory.Delete(root, true); }
    }

    [RequiresMarkerGameFact]
    public void Installed_definitions_cover_saved_lights_and_sources_remain_unchanged()
    {
        string root = Environment.GetEnvironmentVariable("ARM_GAME_PATH")!;
        string source = Path.Combine(root, "SYSTEM", "DATA_MP", "DEFAULTS", "lightdef.dau");
        byte[] hash = SHA256.HashData(File.ReadAllBytes(source));
        var values = NativeLightDefinitions.Load(root);
        int lights = 0;
        foreach (string path in Directory.GetFiles(Path.Combine(root, "MAPS"), "light.dat", SearchOption.AllDirectories))
            foreach (var light in NativeLightPool.Load(path).ActiveLights())
            {
                Assert.True(values.TryGetValue(light.Definition, out var value));
                Assert.NotEqual(0, value.Active); value.Create(light.Position); lights++;
            }
        Assert.Equal(1415, lights);
        Assert.Equal(hash, SHA256.HashData(File.ReadAllBytes(source)));
    }
}
