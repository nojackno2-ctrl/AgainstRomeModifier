using System.Numerics;
using AgainstRomeMapEditor.Modules.Atmosphere;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class AtmosphereTests
{
    [Fact]
    public void BodenIniData_DefaultValues_PassValidation()
    {
        var data = new BodenIniData();
        bool valid = BodenIniSerializer.Validate(data, out var errors);
        Assert.True(valid, string.Join("; ", errors));
        Assert.Empty(errors);
    }

    [Fact]
    public void BodenIniData_WaterRgbConversion_RoundTripsAccurately()
    {
        var data = new BodenIniData();
        // 0xffdfbf -> B=255, G=223, R=191
        data.WaterColor = "0xffdfbf";
        var rgb = data.GetWaterRgb();
        Assert.InRange(rgb.X, 190f / 255f, 192f / 255f);
        Assert.InRange(rgb.Y, 222f / 255f, 224f / 255f);
        Assert.InRange(rgb.Z, 254f / 255f, 1.0f);

        data.SetWaterRgb(0.5f, 0.25f, 0.75f);
        Assert.Equal("0xbf4080", data.WaterColor.ToLowerInvariant());
        var convertedBack = data.GetWaterRgb();
        Assert.InRange(MathF.Abs(convertedBack.X - 0.5f), 0f, 0.01f);
        Assert.InRange(MathF.Abs(convertedBack.Y - 0.25f), 0f, 0.01f);
        Assert.InRange(MathF.Abs(convertedBack.Z - 0.75f), 0f, 0.01f);
    }

    [Fact]
    public void BodenIniSerializer_ParseText_ParsesKnownAndPreservesUnknownSections()
    {
        string sample = """
            [Waterlevel] ;海平面
            150.5
            [Heightmapstep]
            4
            [WaterBumpAmplitude] ;0..1024
            512
            [WaterBumpFrequency]
            8
            [WaterWarpShift]
            12
            [RainDropsOnWater]
            1
            [WaterColor]
            0x4080c0
            [FlashPropability]
            25
            [DayStartTime]
            5.5
            [DayEndTime]
            21.0
            [CustomWeatherExtra] ;保留自訂參數
            SpecialVal=999
            """;

        var data = BodenIniSerializer.ParseText(sample);
        Assert.Equal(150.5f, data.Waterlevel);
        Assert.Equal(4f, data.Heightmapstep);
        Assert.Equal(512, data.WaterBumpAmplitude);
        Assert.Equal(8, data.WaterBumpFrequency);
        Assert.Equal(12, data.WaterWarpShift);
        Assert.True(data.RainDropsOnWater);
        Assert.Equal("0x4080c0", data.WaterColor);
        Assert.Equal(25, data.FlashPropability);
        Assert.Equal(5.5f, data.DayStartTime);
        Assert.Equal(21.0f, data.DayEndTime);

        // 未知區段保留
        Assert.True(data.PreservedSections.ContainsKey("CustomWeatherExtra"));
        Assert.Equal("SpecialVal=999", data.PreservedSections["CustomWeatherExtra"]);

        // 重新生成時亦包含保留之自訂區段
        string generated = BodenIniSerializer.GenerateDefaultIni(data);
        Assert.Contains("[CustomWeatherExtra]", generated);
        Assert.Contains("SpecialVal=999", generated);
        Assert.Contains("[WaterBumpAmplitude]", generated);
        Assert.Contains("512", generated);
    }

    [Theory]
    [InlineData(-1, 4, 14, 0, "0xffdfbf", 6f, 20f, "WaterBumpAmplitude")]
    [InlineData(1050, 4, 14, 0, "0xffdfbf", 6f, 20f, "WaterBumpAmplitude")]
    [InlineData(256, 0, 14, 0, "0xffdfbf", 6f, 20f, "WaterBumpFrequency")]
    [InlineData(256, 17, 14, 0, "0xffdfbf", 6f, 20f, "WaterBumpFrequency")]
    [InlineData(256, 4, 7, 0, "0xffdfbf", 6f, 20f, "WaterWarpShift")]
    [InlineData(256, 4, 14, -5, "0xffdfbf", 6f, 20f, "FlashPropability")]
    [InlineData(256, 4, 14, 1500, "0xffdfbf", 6f, 20f, "FlashPropability")]
    [InlineData(256, 4, 14, 0, "bad_color", 6f, 20f, "WaterColor")]
    [InlineData(256, 4, 14, 0, "0xffdfbf", -1f, 20f, "DayStartTime")]
    [InlineData(256, 4, 14, 0, "0xffdfbf", 6f, 25f, "DayEndTime")]
    public void BodenIniSerializer_Validate_CatchesInvalidValues(
        int amp, int freq, int warp, int flash, string color, float dayStart, float dayEnd, string expectedKeyError)
    {
        var data = new BodenIniData
        {
            WaterBumpAmplitude = amp,
            WaterBumpFrequency = freq,
            WaterWarpShift = warp,
            FlashPropability = flash,
            WaterColor = color,
            DayStartTime = dayStart,
            DayEndTime = dayEnd
        };

        bool valid = BodenIniSerializer.Validate(data, out var errors);
        Assert.False(valid);
        Assert.Contains(errors, err => err.Contains(expectedKeyError));
    }

    [Fact]
    public void WeatherPresets_AllFivePresets_AreDistinctAndValid()
    {
        var presets = WeatherPresets.All;
        Assert.Equal(5, presets.Count);

        var ids = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var preset in presets)
        {
            Assert.True(ids.Add(preset.PresetId), $"重複的預設識別碼: {preset.PresetId}");
            Assert.False(string.IsNullOrWhiteSpace(preset.Name));
            Assert.False(string.IsNullOrWhiteSpace(preset.EnglishName));
            Assert.False(string.IsNullOrWhiteSpace(preset.Description));
            Assert.True(BodenIniSerializer.Validate(preset.Boden, out var errors), string.Join("; ", errors));

            if (preset.FogEnabled)
            {
                Assert.True(preset.FogEndDistance > preset.FogStartDistance);
                Assert.InRange(preset.FogDensity, 0.05f, 1.0f);
            }
        }
    }

    [Fact]
    public void WeatherPresets_TryGetPreset_FindsCaseInsensitive()
    {
        Assert.True(WeatherPresets.TryGetPreset("STORM", out var storm));
        Assert.Equal("storm", storm.PresetId);
        Assert.Equal(PrecipitationType.Rain, storm.Precipitation);
        Assert.True(storm.Boden.RainDropsOnWater);
        Assert.True(storm.Boden.FlashPropability > 0);

        Assert.True(WeatherPresets.TryGetPreset("mountain_snow", out var snow));
        Assert.Equal(PrecipitationType.Snow, snow.Precipitation);
        Assert.Equal(837, snow.Boden.SnowAlrIndex);

        Assert.False(WeatherPresets.TryGetPreset("non_existent", out var fallback));
        Assert.Equal(WeatherPresets.ClearSkyId, fallback.PresetId);
    }

    [Fact]
    public void AtmosphereLightingBridge_ComputeUniforms_CalculatesEffectiveAmbientAndFog()
    {
        var bridge = new AtmosphereLightingBridge();
        var profile = WeatherPresets.CreateSunsetDusk();
        profile.AmbientTint = new Vector3(1.1f, 0.9f, 0.7f);
        profile.Exposure = 1.2f;

        Vector3 baseAmbient = new Vector3(0.8f, 0.8f, 0.8f);
        var uniforms = bridge.ComputeUniforms(profile, baseAmbient);

        // EffectiveAmbient = baseAmbient * AmbientTint * Exposure
        Vector3 expected = baseAmbient * profile.AmbientTint * 1.2f;
        Assert.InRange(MathF.Abs(uniforms.EffectiveAmbient.X - expected.X), 0f, 0.001f);
        Assert.InRange(MathF.Abs(uniforms.EffectiveAmbient.Y - expected.Y), 0f, 0.001f);
        Assert.InRange(MathF.Abs(uniforms.EffectiveAmbient.Z - expected.Z), 0f, 0.001f);

        Assert.Equal(profile.FogColor, uniforms.FogColor);
        Assert.Equal(profile.FogStartDistance, uniforms.FogParams.X);
        Assert.Equal(profile.FogEndDistance, uniforms.FogParams.Y);
        Assert.Equal(profile.FogDensity, uniforms.FogParams.Z);
        Assert.Equal(1.0f, uniforms.FogParams.W);
    }

    [Fact]
    public void AtmosphereLightingBridge_ManualFlashAndDecay_WorksExpectedly()
    {
        var bridge = new AtmosphereLightingBridge();
        var profile = WeatherPresets.CreateClearSky();

        Assert.Equal(0f, bridge.FlashIntensity);
        bridge.TriggerManualFlash(0.9f);
        Assert.Equal(0.9f, bridge.FlashIntensity);

        var uniformsDuringFlash = bridge.ComputeUniforms(profile, Vector3.One);
        Assert.True(uniformsDuringFlash.EffectiveAmbient.X > 1.0f, "雷電爆閃應暫時提升環境光強度");

        // 影格更新 0.2 秒後應完全消退
        bridge.Update(0.2f, profile);
        Assert.Equal(0f, bridge.FlashIntensity);
        Assert.Equal(0.2f, bridge.SimulationTime);
    }

    [Fact]
    public void AtmosphereEditSession_IEditorModule_TracksDirtyBaselineAndReset()
    {
        var session = new AtmosphereEditSession();
        Assert.Equal("atmosphere", session.ModuleId);
        Assert.False(session.IsDirty);

        // 修改水波
        session.SetWaterBump(600, 10, 12);
        Assert.True(session.IsDirty);
        Assert.Equal(600, session.Current.Boden.WaterBumpAmplitude);

        // 提交變更
        session.AcceptChanges();
        Assert.False(session.IsDirty);

        // 套用暴風雨預設
        session.ApplyPreset(WeatherPresets.StormId);
        Assert.True(session.IsDirty);
        Assert.Equal("storm", session.Current.PresetId);
        Assert.Equal(8, session.Current.Boden.FlashPropability);

        // 放棄變更，重設回基準
        session.Reset();
        Assert.False(session.IsDirty);
        Assert.Equal(600, session.Current.Boden.WaterBumpAmplitude);
    }

    [Fact]
    public void AtmosphereEditSession_CaptureAndLoad_PerformsDeepCloning()
    {
        var session = new AtmosphereEditSession(WeatherPresets.CreateDenseFog());
        var captured = session.Capture();

        // 變更 captured 不應影響 session 內部
        captured.Boden.WaterBumpAmplitude = 999;
        Assert.NotEqual(999, session.Current.Boden.WaterBumpAmplitude);

        // 載入 captured
        session.Load(captured);
        Assert.Equal(999, session.Current.Boden.WaterBumpAmplitude);
        Assert.False(session.IsDirty);
    }
}
