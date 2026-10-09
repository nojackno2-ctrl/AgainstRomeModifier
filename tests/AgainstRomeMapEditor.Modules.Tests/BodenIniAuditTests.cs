using System.Globalization;
using System.Text.RegularExpressions;
using AgainstRomeMapEditor.Modules.Atmosphere;
using AgainstRomeModifier.Maps;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class BodenIniAuditTests
{
    // Inline values only; no game assets.
    private const string Sample = """
        [Waterlevel]
        120
        [Heightmapstep]
        4
        [Skydensspread]
        16
        [Skydensaccuracy]
        2
        [ShadowMeshAccuracy]
        6
        [ShadowMeshXZsize]
        16000
        [ShadowMeshYsize]
        5000
        [ShadowMeshMode]
        0
        [HandleSkyDensMap]
        1
        [HandleVisibleMap]
        1
        [HandleClipRectMap]
        1
        [HandleShadowMeshes]
        1
        [WasserTexturName]
        wassAW
        [CausticTexturName]
        caustA
        [SkyTexturName]
        sky
        [RainDropsOnWater]
        1
        [WaterWarpShift]
        12
        [WaterBumpAmplitude]
        256
        [WaterBumpFrequency]
        4
        [FlashPropability]
        8
        [FlashObjectDefaultIndex]
        803
        [FlashObjectDefault2Index]
        1099
        [FlashLightDefaultIndex]
        6
        [SnowAlrIndex]
        837
        [SnowShadowIndex]
        152
        [SnowShadowSize]
        7
        [HagelShadowIndex]
        152
        [HagelShadowSize]
        7
        [MoveListAmplitude]
        127
        [WaterColor]
        0xffdfbf
        [ShowCollisionMesh]
        0
        [DayStartTime]
        6
        [DayEndTime]
        20
        """;

    [Fact]
    public void Observed_33_keys_match_defaults_and_both_parsers_including_lowercase()
    {
        var expected = Sections(Sample);
        Assert.Equal(33, expected.Count);
        AssertSectionsEqual(expected, Sections(BodenIniSerializer.GenerateDefaultIni(new BodenIniData())));
        foreach (string sample in new[] { Sample, Regex.Replace(Sample, @"\[[^\]]+\]", m => m.Value.ToLowerInvariant()) })
        {
            var parsed = BodenIniSerializer.ParseText(sample);
            Assert.True(BodenIniSerializer.Validate(parsed, out var errors), string.Join("; ", errors));
            Assert.Empty(parsed.PreservedSections);
            AssertSectionsEqual(expected, Sections(BodenIniSerializer.GenerateDefaultIni(parsed)));
            string path = Path.Combine(Path.GetTempPath(), $"ArmWeatherAudit_{Guid.NewGuid():N}.ini");
            try
            {
                File.WriteAllText(path, sample);
                var documentData = BodenIniSerializer.Parse(BodenIniDocument.Load(path));
                AssertSectionsEqual(expected, Sections(BodenIniSerializer.GenerateDefaultIni(documentData)));
            }
            finally { File.Delete(path); }
        }
    }

    [Fact]
    public void Lowercase_keys_parse_nondefault_values_instead_of_silently_using_defaults()
    {
        const string sample = """
            [waterlevel]
            150
            [heightmapstep]
            8
            [skydensspread]
            32
            [skydensaccuracy]
            1
            [flashobjectdefaultindex]
            804
            [flashobjectdefault2index]
            1100
            [flashlightdefaultindex]
            7
            [snowalrindex]
            838
            [snowshadowindex]
            153
            [snowshadowsize]
            8
            [hagelshadowindex]
            154
            [hagelshadowsize]
            9
            [movelistamplitude]
            64
            [handleskydensmap]
            0
            [handlevisiblemap]
            0
            [handlecliprectmap]
            0
            [handleshadowmeshes]
            0
            [showcollisionmesh]
            1
            """;
        var data = BodenIniSerializer.ParseText(sample);
        Assert.Equal(150f, data.Waterlevel);
        Assert.Equal(8f, data.Heightmapstep);
        Assert.Equal(32f, data.Skydensspread);
        Assert.Equal(1, data.Skydensaccuracy);
        Assert.Equal(804, data.FlashObjectDefaultIndex);
        Assert.Equal(1100, data.FlashObjectDefault2Index);
        Assert.Equal(7, data.FlashLightDefaultIndex);
        Assert.Equal(838, data.SnowAlrIndex);
        Assert.Equal(153, data.SnowShadowIndex);
        Assert.Equal(8, data.SnowShadowSize);
        Assert.Equal(154, data.HagelShadowIndex);
        Assert.Equal(9, data.HagelShadowSize);
        Assert.Equal(64, data.MoveListAmplitude);
        Assert.Equal(0, data.HandleSkyDensMap);
        Assert.Equal(0, data.HandleVisibleMap);
        Assert.Equal(0, data.HandleClipRectMap);
        Assert.Equal(0, data.HandleShadowMeshes);
        Assert.Equal(1, data.ShowCollisionMesh);
    }

    [Fact]
    public void Update_preserves_comments_unknown_sections_and_does_not_invent_keys()
    {
        string path = Path.Combine(Path.GetTempPath(), $"ArmWeatherAudit_{Guid.NewGuid():N}.ini");
        const string sample = "[WaterWarpShift] ;motion\r\n12 ;keep\r\n[WaterColor]\r\n0xffdfbf\r\n[CustomKey]\r\nretained\r\n";
        try
        {
            File.WriteAllText(path, sample);
            var doc = BodenIniDocument.Load(path);
            var data = BodenIniSerializer.Parse(doc);
            data.WaterWarpShift = 0;
            BodenIniSerializer.UpdateDocument(doc, data);
            doc.Save();
            Assert.Equal(sample.Replace("12 ;keep", "0 ;keep"), File.ReadAllText(path));
            Assert.Null(doc.GetValue("SnowAlrIndex"));
        }
        finally { File.Delete(path); }
    }

    [Fact]
    public void Every_preset_uses_observed_keys_and_nearby_native_values()
    {
        var keys = Sections(Sample).Keys.Order().ToArray();
        var baseline = BodenIniSerializer.ParseText(Sample);
        foreach (var profile in WeatherPresets.All)
        {
            var data = profile.Boden;
            Assert.Equal(keys, Sections(BodenIniSerializer.GenerateDefaultIni(data)).Keys.Order().ToArray());
            Assert.InRange(data.WaterBumpAmplitude, 224, 320); // sample256, -12.5%..+25%
            Assert.InRange(data.WaterBumpFrequency, 4, 5);
            Assert.InRange(data.WaterWarpShift, 12, 14);
            Assert.InRange(data.FlashPropability, 0, 8);
            var delta = System.Numerics.Vector3.Abs(data.GetWaterRgb() - baseline.GetWaterRgb());
            Assert.InRange(delta.X, 0, 32f / 255f);
            Assert.InRange(delta.Y, 0, 32f / 255f);
            Assert.InRange(delta.Z, 0, 32f / 255f);
            Assert.Equal(6f, data.DayStartTime);
            Assert.Equal(20f, data.DayEndTime);
            Assert.Equal(0, data.ShadowMeshMode);
            Assert.Equal(803, data.FlashObjectDefaultIndex);
            Assert.Equal(1099, data.FlashObjectDefault2Index);
            Assert.Equal(6, data.FlashLightDefaultIndex);
            Assert.Equal(837, data.SnowAlrIndex);
            Assert.Equal(152, data.SnowShadowIndex);
            Assert.Equal(7, data.SnowShadowSize);
            Assert.Equal(152, data.HagelShadowIndex);
            Assert.Equal(7, data.HagelShadowSize);
        }
    }

    [Theory]
    [InlineData(0, true)]
    [InlineData(10, true)]
    [InlineData(18, true)]
    [InlineData(-1, false)]
    [InlineData(1, false)]
    [InlineData(8, false)]
    [InlineData(9, false)]
    [InlineData(19, false)]
    [InlineData(20, false)]
    public void Warp_accepts_documented_off_value_and_motion_range(int shift, bool valid)
    {
        var data = new BodenIniData { WaterWarpShift = shift };
        Assert.Equal(valid, BodenIniSerializer.Validate(data, out _));
        var bridge = new AtmosphereLightingBridge();
        Assert.Equal((float)shift, bridge.ComputeUniforms(new AtmosphereProfile { Boden = data }, System.Numerics.Vector3.One).WaterDynamics.Z);
    }

    [Theory]
    [InlineData("Waterlevel", -1)]
    [InlineData("Waterlevel", double.NaN)]
    [InlineData("Waterlevel", double.PositiveInfinity)]
    [InlineData("Heightmapstep", 0.5)]
    [InlineData("Heightmapstep", double.NaN)]
    [InlineData("Heightmapstep", double.PositiveInfinity)]
    [InlineData("DayStartTime", double.NaN)]
    [InlineData("DayStartTime", double.NegativeInfinity)]
    [InlineData("DayEndTime", double.NaN)]
    [InlineData("DayEndTime", double.PositiveInfinity)]
    [InlineData("ShadowMeshAccuracy", -1)]
    [InlineData("ShadowMeshAccuracy", 8)]
    [InlineData("ShadowMeshXZsize", 8191)]
    [InlineData("ShadowMeshXZsize", 16385)]
    [InlineData("ShadowMeshYsize", 3999)]
    [InlineData("ShadowMeshYsize", 16001)]
    [InlineData("Skydensspread", 0)]
    [InlineData("Skydensspread", 12)]
    [InlineData("Skydensspread", double.NaN)]
    [InlineData("Skydensaccuracy", 3)]
    [InlineData("MoveListAmplitude", -1)]
    [InlineData("MoveListAmplitude", 128)]
    [InlineData("HandleSkyDensMap", 2)]
    [InlineData("HandleVisibleMap", -1)]
    [InlineData("HandleClipRectMap", 2)]
    [InlineData("HandleShadowMeshes", 2)]
    [InlineData("ShowCollisionMesh", 2)]
    public void Validation_rejects_nonfinite_or_undocumented_values(string key, double value)
    {
        var data = new BodenIniData();
        var property = typeof(BodenIniData).GetProperty(key)!;
        property.SetValue(data, Convert.ChangeType(value, property.PropertyType, CultureInfo.InvariantCulture));
        Assert.False(BodenIniSerializer.Validate(data, out var errors));
        Assert.Contains(errors, e => e.Contains(key, StringComparison.Ordinal));
    }

    [Fact]
    public void Comment_endpoints_remain_editable_beyond_observed_values()
    {
        foreach (bool upper in new[] { false, true })
        {
            var data = new BodenIniData
            {
                Waterlevel = 0, Heightmapstep = 1,
                WaterBumpAmplitude = upper ? 1024 : 0, WaterBumpFrequency = upper ? 16 : 1,
                FlashPropability = upper ? 1000 : 0, DayStartTime = upper ? 24 : 0, DayEndTime = upper ? 24 : 0,
                ShadowMeshAccuracy = upper ? 7 : 0, ShadowMeshXZsize = upper ? 16384 : 8192,
                ShadowMeshYsize = upper ? 16000 : 4000, Skydensspread = upper ? 1024 : 4,
                Skydensaccuracy = upper ? 2 : 0, MoveListAmplitude = upper ? 127 : 0,
                ShadowMeshMode = upper ? 1 : 0, HandleSkyDensMap = 0, HandleVisibleMap = 0,
                HandleClipRectMap = 0, HandleShadowMeshes = 0, ShowCollisionMesh = 1
            };
            Assert.True(BodenIniSerializer.Validate(data, out var errors), string.Join("; ", errors));
        }
    }

    private static Dictionary<string, string> Sections(string text) =>
        Regex.Matches(text, @"(?m)^\s*\[(?<key>[^\]\r\n]+)\][^\r\n]*\r?\n(?<value>[^\r\n]*)")
            .ToDictionary(m => m.Groups["key"].Value, m => m.Groups["value"].Value.Trim());

    private static void AssertSectionsEqual(Dictionary<string, string> expected, Dictionary<string, string> actual)
    {
        Assert.Equal(expected.Keys.Order(), actual.Keys.Order());
        foreach (var (key, value) in expected) Assert.Equal(value, actual[key]);
    }
}
