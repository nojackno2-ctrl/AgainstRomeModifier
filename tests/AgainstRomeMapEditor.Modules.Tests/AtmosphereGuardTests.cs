using System.Numerics;
using AgainstRomeMapEditor.Modules.Atmosphere;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class AtmosphereGuardTests
{
    [Fact]
    public void AtmosphereEditSession_Constructor_NullInitial_DefaultsToClearSky()
    {
        var session = new AtmosphereEditSession(null!);
        Assert.NotNull(session.Current);
        Assert.Equal(WeatherPresets.ClearSkyId, session.Current.PresetId);
        Assert.False(session.IsDirty);
    }

    [Fact]
    public void AtmosphereEditSession_Load_ThrowsOnNullSnapshot()
    {
        var session = new AtmosphereEditSession();
        Assert.Throws<ArgumentNullException>(() => session.Load(null!));
    }

    [Fact]
    public void AtmosphereEditSession_ApplyPreset_UnknownPresetLeavesCurrentUnchanged()
    {
        var session = new AtmosphereEditSession();
        string initialPresetId = session.Current.PresetId;

        session.ApplyPreset("unknown_and_invalid_preset_id");
        Assert.Equal(initialPresetId, session.Current.PresetId);
        Assert.False(session.IsDirty);
    }

    [Fact]
    public void AtmosphereEditSession_SetWaterBump_ClampsAllParameters()
    {
        var session = new AtmosphereEditSession();

        // 負數測試
        session.SetWaterBump(amplitude: -50, frequency: -10, warpShift: 2);
        Assert.Equal(0, session.Current.Boden.WaterBumpAmplitude);
        Assert.Equal(1, session.Current.Boden.WaterBumpFrequency);
        Assert.Equal(8, session.Current.Boden.WaterWarpShift);

        // 超大數值測試
        session.SetWaterBump(amplitude: 2000, frequency: 50, warpShift: 99);
        Assert.Equal(1024, session.Current.Boden.WaterBumpAmplitude);
        Assert.Equal(16, session.Current.Boden.WaterBumpFrequency);
        Assert.Equal(20, session.Current.Boden.WaterWarpShift);
    }

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("   ")]
    public void AtmosphereEditSession_SetWaterColor_NullOrWhitespaceIsIgnored(string? invalidColor)
    {
        var session = new AtmosphereEditSession();
        session.SetWaterColor("0x112233");
        Assert.Equal("0x112233", session.Current.Boden.WaterColor);

        session.SetWaterColor(invalidColor!);
        Assert.Equal("0x112233", session.Current.Boden.WaterColor);
    }

    [Fact]
    public void AtmosphereEditSession_SetFlashProbability_ClampsRange()
    {
        var session = new AtmosphereEditSession();
        session.SetFlashProbability(-10);
        Assert.Equal(0, session.Current.Boden.FlashPropability);

        session.SetFlashProbability(2500);
        Assert.Equal(1000, session.Current.Boden.FlashPropability);
    }

    [Fact]
    public void AtmosphereEditSession_SetDayNightTimes_ClampsRange()
    {
        var session = new AtmosphereEditSession();
        session.SetDayNightTimes(-5f, 30f);
        Assert.Equal(0f, session.Current.Boden.DayStartTime);
        Assert.Equal(24f, session.Current.Boden.DayEndTime);
    }

    [Fact]
    public void AtmosphereEditSession_SetFog_ClampsAndAdjustsEndDistance()
    {
        var session = new AtmosphereEditSession();

        // 當 endDistance 小於等於 startDistance 時，應自動推進為 startDistance + 0.1f
        session.SetFog(true, startDistance: -10f, endDistance: 20f, density: 1.5f, color: new Vector3(2f, -1f, 0.5f));
        Assert.Equal(0f, session.Current.FogStartDistance);
        Assert.Equal(20f, session.Current.FogEndDistance);
        Assert.Equal(1.0f, session.Current.FogDensity);
        Assert.Equal(new Vector3(1f, 0f, 0.5f), session.Current.FogColor);

        session.SetFog(true, startDistance: 50f, endDistance: 30f, density: 0.5f, color: Vector3.One);
        Assert.Equal(50f, session.Current.FogStartDistance);
        Assert.Equal(50.1f, session.Current.FogEndDistance, 1);
    }

    [Fact]
    public void AtmosphereEditSession_SetLightingModulation_ClampsRange()
    {
        var session = new AtmosphereEditSession();
        session.SetLightingModulation(new Vector3(3f, -1f, 1f), exposure: 5.0f);
        Assert.Equal(new Vector3(2f, 0f, 1f), session.Current.AmbientTint);
        Assert.Equal(3.0f, session.Current.Exposure);

        session.SetLightingModulation(Vector3.One, exposure: 0.05f);
        Assert.Equal(0.2f, session.Current.Exposure);
    }

    [Fact]
    public void AtmosphereEditSession_SetPrecipitation_UpdatesBodenInterlockingFlags()
    {
        var session = new AtmosphereEditSession();

        // 雨天強降雨 -> RainDropsOnWater = true
        session.SetPrecipitation(PrecipitationType.Rain, 0.8f);
        Assert.True(session.Current.Boden.RainDropsOnWater);
        Assert.Equal(-1, session.Current.Boden.SnowAlrIndex);

        // 雨天微雨 (<= 0.1f) -> RainDropsOnWater = false
        session.SetPrecipitation(PrecipitationType.Rain, 0.05f);
        Assert.False(session.Current.Boden.RainDropsOnWater);

        // 雪天 -> SnowAlrIndex = 1
        session.SetPrecipitation(PrecipitationType.Snow, 0.5f);
        Assert.Equal(1, session.Current.Boden.SnowAlrIndex);
        Assert.False(session.Current.Boden.RainDropsOnWater);

        // 無降雨 -> SnowAlrIndex = -1
        session.SetPrecipitation(PrecipitationType.None, 0f);
        Assert.Equal(-1, session.Current.Boden.SnowAlrIndex);
        Assert.False(session.Current.Boden.RainDropsOnWater);
    }

    [Fact]
    public void BodenIniSerializer_GuardsAgainstNullArguments()
    {
        Assert.Throws<ArgumentNullException>(() => BodenIniSerializer.ParseText(null!));
        Assert.Throws<ArgumentNullException>(() => BodenIniSerializer.GenerateDefaultIni(null!));
        Assert.Throws<ArgumentNullException>(() => BodenIniSerializer.Validate(null!, out _));
    }

    [Theory]
    [InlineData(float.NaN, 4f, 6f, 20f, "Waterlevel")]
    [InlineData(100f, float.NaN, 6f, 20f, "Heightmapstep")]
    [InlineData(100f, -1f, 6f, 20f, "Heightmapstep")]
    [InlineData(100f, 4f, 25f, 20f, "DayStartTime")]
    [InlineData(100f, 4f, 6f, -1f, "DayEndTime")]
    public void BodenIniSerializer_Validate_CatchesInvalidFloatingPointAndTemporalRanges(
        float waterlevel, float heightStep, float dayStart, float dayEnd, string expectedKey)
    {
        var data = new BodenIniData
        {
            Waterlevel = waterlevel,
            Heightmapstep = heightStep,
            DayStartTime = dayStart,
            DayEndTime = dayEnd
        };

        bool valid = BodenIniSerializer.Validate(data, out var errors);
        Assert.False(valid);
        Assert.Contains(errors, e => e.Contains(expectedKey));
    }

    [Fact]
    public void AtmosphereProfile_ValueEquals_NullAndIdentity()
    {
        var profile = WeatherPresets.CreateClearSky();
        Assert.False(profile.ValueEquals(null!));
        Assert.True(profile.ValueEquals(profile));

        var clone = profile.Clone();
        Assert.True(profile.ValueEquals(clone));

        clone.FogDensity += 0.1f;
        Assert.False(profile.ValueEquals(clone));
    }
}
