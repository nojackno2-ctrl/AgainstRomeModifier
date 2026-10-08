using AgainstRomeMapEditor;
using AgainstRomeMapEditor.Modules.Atmosphere;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Fact]
    public void Weather_preset_menu_applies_fields_and_saves_to_boden_ini_keeping_water_level()
    {
        string map = CreateFixture("ENDL_005");

        RunInSta(() =>
        {
            using var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "WeatherTest", "Test"));
            _ = form.Handle;
            Invoke(form, "LoadSelectedMap");

            Assert.True(form.ApplyWeatherPreset(WeatherPresets.StormId));
            WeatherPresets.TryGetPreset(WeatherPresets.StormId, out AtmosphereProfile storm);

            Assert.True(form.TrySaveMap(showSuccess: false, out Exception? error), error?.ToString());
            var ini = BodenIniDocument.Load(Path.Combine(map, "boden.ini"));
            Assert.Equal(storm.Boden.WaterColor, ini.GetValue("WaterColor"));
            Assert.Equal(storm.Boden.FlashPropability.ToString(), ini.GetValue("FlashPropability"));
            Assert.Equal("120", ini.GetValue("Waterlevel")); // 水位與地形綁定，不隨天候改變
            Assert.False(form.ApplyWeatherPreset("no_such_preset"));
        });
    }
}
