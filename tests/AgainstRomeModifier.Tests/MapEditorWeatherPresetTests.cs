using System.Globalization;
using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.Atmosphere;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Theory]
    [InlineData(WeatherPresets.ClearSkyId)]
    [InlineData(WeatherPresets.StormId)]
    [InlineData(WeatherPresets.SunsetDuskId)]
    [InlineData(WeatherPresets.DenseFogId)]
    [InlineData(WeatherPresets.MountainSnowId)]
    public void Weather_preset_menu_applies_fields_and_saves_to_boden_ini_keeping_water_level(string presetId)
    {
        string map = CreateFixture("ENDL_005");

        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "WeatherTest", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");

            Assert.True(form.ApplyWeatherPreset(presetId));
            WeatherPresets.TryGetPreset(presetId, out AtmosphereProfile preset);
            var data = preset.Boden;

            Assert.True(form.TrySaveMap(showSuccess: false, out Exception? error), error?.ToString());
            var ini = BodenIniDocument.Load(Path.Combine(map, "boden.ini"));
            Assert.Equal(data.WaterColor, ini.GetValue("WaterColor"));
            Assert.Equal(data.FlashPropability.ToString(CultureInfo.InvariantCulture), ini.GetValue("FlashPropability"));
            Assert.Equal(data.WaterBumpAmplitude.ToString(CultureInfo.InvariantCulture), ini.GetValue("WaterBumpAmplitude"));
            Assert.Equal(data.WaterBumpFrequency.ToString(CultureInfo.InvariantCulture), ini.GetValue("WaterBumpFrequency"));
            Assert.Equal(data.WaterWarpShift.ToString(CultureInfo.InvariantCulture), ini.GetValue("WaterWarpShift"));
            Assert.Equal(data.DayStartTime.ToString(CultureInfo.InvariantCulture), ini.GetValue("DayStartTime"));
            Assert.Equal(data.DayEndTime.ToString(CultureInfo.InvariantCulture), ini.GetValue("DayEndTime"));
            Assert.Equal(data.RainDropsOnWater ? "1" : "0", ini.GetValue("RainDropsOnWater"));
            Assert.Equal("4", ini.GetValue("Heightmapstep"));
            Assert.DoesNotContain("[FogDensity]", File.ReadAllText(Path.Combine(map, "boden.ini")));
            Assert.DoesNotContain("[Precipitation]", File.ReadAllText(Path.Combine(map, "boden.ini")));
            Assert.Equal("120", ini.GetValue("Waterlevel")); // 水位與地形綁定，不隨天候改變
            Assert.False(form.ApplyWeatherPreset("no_such_preset"));
        });
    }
}
