using System.Globalization;
using System.Windows.Forms;
using AgainstRomeMapEditor;
using AgainstRomeModifier.Maps;

namespace AgainstRomeModifier.Tests;

public sealed partial class MapEditorSaveTransactionTests
{
    [Theory]
    [InlineData("de-DE")]
    [InlineData("fr-FR")]
    [InlineData("en-US")]
    public void Environment_values_load_save_and_reopen_independently_of_windows_number_format(string cultureName)
    {
        string map = CreateFixture();
        (string Field, string Key, decimal Initial, decimal Edited)[] values =
        [
            ("_waterLevel", "Waterlevel", 12.5m, 18.25m),
            ("_waterWarpShift", "WaterWarpShift", 0.25m, 0.75m),
            ("_waterBumpAmplitude", "WaterBumpAmplitude", 0.5m, 1.25m),
            ("_waterBumpFrequency", "WaterBumpFrequency", 1.5m, 2.5m),
            ("_flashProbability", "FlashPropability", 0.15m, 0.25m),
            ("_dayStart", "DayStartTime", 6.5m, 7.25m),
            ("_dayEnd", "DayEndTime", 18.5m, 20.25m)
        ];
        string iniPath = Path.Combine(map, "boden.ini");
        var ini = BodenIniDocument.Load(iniPath);
        foreach (var value in values) ini.SetValue(value.Key, value.Initial.ToString(CultureInfo.InvariantCulture));
        ini.SetValue("Heightmapstep", "4.5");
        ini.Save();
        File.AppendAllText(iniPath, "[UnknownFutureField]\r\n  preserve-me ; comment\r\n");

        RunInSta(() =>
        {
            CultureInfo previous = CultureInfo.CurrentCulture;
            try
            {
                CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo(cultureName);
                using (var form = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Environment", "Endless")))
                {
                    _ = form.Handle;
                    Invoke(form, "LoadSelectedMap");
                    Assert.Equal(4.5f, GetField<float>(form, "_heightMapStep"));
                    foreach (var value in values)
                    {
                        var control = GetField<NumericUpDown>(form, value.Field);
                        Assert.Equal(value.Initial, control.Value);
                        control.Value = value.Edited;
                    }
                    GetField<CheckBox>(form, "_rain").Checked = true;
                    Assert.True(GetProperty<bool>(form, "IsDirty"));
                    Assert.True(form.TrySaveMap(false, out Exception? error), error?.ToString());
                    Assert.Null(error);
                    Assert.False(GetProperty<bool>(form, "IsDirty"));
                }
                var saved = BodenIniDocument.Load(iniPath);
                foreach (var value in values)
                    Assert.Equal(value.Edited.ToString(CultureInfo.InvariantCulture), saved.GetValue(value.Key));
                Assert.Equal("1", saved.GetValue("RainDropsOnWater"));
                Assert.Contains("[UnknownFutureField]\r\n  preserve-me ; comment\r\n", File.ReadAllText(iniPath));
                var afterSave = SnapshotDirectory(map);
                using var reopened = new MapEditorForm(_root, new GameMapInfo("ENDL_005", map, true, "Environment", "Endless"));
                _ = reopened.Handle;
                Invoke(reopened, "LoadSelectedMap");
                Assert.Equal(4.5f, GetField<float>(reopened, "_heightMapStep"));
                foreach (var value in values) Assert.Equal(value.Edited, GetField<NumericUpDown>(reopened, value.Field).Value);
                Assert.True(GetField<CheckBox>(reopened, "_rain").Checked);
                Assert.False(GetProperty<bool>(reopened, "IsDirty"));
                Assert.True(reopened.TrySaveMap(false, out Exception? retryError), retryError?.ToString());
                Assert.Null(retryError);
                var afterRetry = SnapshotDirectory(map);
                Assert.Equal(afterSave.Keys.Order(), afterRetry.Keys.Order());
                foreach (var file in afterSave) Assert.Equal(file.Value, afterRetry[file.Key]);
            }
            finally { CultureInfo.CurrentCulture = previous; }
        });
    }
}
