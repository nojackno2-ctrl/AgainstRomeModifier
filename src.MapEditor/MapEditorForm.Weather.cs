using System.Globalization;
using AgainstRomeMapEditor.Modules.Atmosphere;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private readonly ToolStripDropDownButton _weatherMenu = new("天候");

    private void InitializeWeatherMenu(ToolStrip tools)
    {
        foreach (AtmosphereProfile preset in WeatherPresets.All)
        {
            var item = new ToolStripMenuItem(preset.Name) { Tag = preset.PresetId };
            item.Click += (_, _) => ApplyWeatherPreset(preset.PresetId);
            _weatherMenu.DropDownItems.Add(item);
        }
        tools.Items.Insert(tools.Items.IndexOf(_layoutMenu) + 1, _weatherMenu);
    }

    private void LocalizeWeatherMenu(bool en)
    {
        _weatherMenu.Text = en ? "Weather" : "天候";
        foreach (ToolStripMenuItem item in _weatherMenu.DropDownItems.OfType<ToolStripMenuItem>())
            if (item.Tag is string id && WeatherPresets.TryGetPreset(id, out AtmosphereProfile preset))
                item.Text = en ? preset.EnglishName : preset.Name;
    }

    /// <summary>
    /// 套用天候預設到水面／閃電／晝夜／雨滴欄位；保留此圖的水位（與地形高度綁定），儲存時沿用既有 boden.ini 交易。
    /// </summary>
    internal bool ApplyWeatherPreset(string presetId)
    {
        bool en = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        if (_selected?.IsCustom != true || !WeatherPresets.TryGetPreset(presetId, out AtmosphereProfile preset)) return false;
        if (!BodenIniSerializer.Validate(preset.Boden, out var errors))
        {
            _status.Text = string.Join(" ", errors);
            return false;
        }
        var data = preset.Boden;
        _waterColor.Text = data.WaterColor;
        _waterWarpShift.Value = Math.Clamp(data.WaterWarpShift, (int)_waterWarpShift.Minimum, (int)_waterWarpShift.Maximum);
        _waterBumpAmplitude.Value = Math.Clamp(data.WaterBumpAmplitude, (int)_waterBumpAmplitude.Minimum, (int)_waterBumpAmplitude.Maximum);
        _waterBumpFrequency.Value = Math.Clamp(data.WaterBumpFrequency, (int)_waterBumpFrequency.Minimum, (int)_waterBumpFrequency.Maximum);
        _flashProbability.Value = Math.Clamp(data.FlashPropability, (int)_flashProbability.Minimum, (int)_flashProbability.Maximum);
        _dayStart.Value = Math.Clamp((decimal)data.DayStartTime, _dayStart.Minimum, _dayStart.Maximum);
        _dayEnd.Value = Math.Clamp((decimal)data.DayEndTime, _dayEnd.Minimum, _dayEnd.Maximum);
        _rain.Checked = data.RainDropsOnWater;
        MarkDirty();
        _status.Text = en ? $"Weather preset applied: {preset.EnglishName}. Save to keep it." : $"已套用天候：{preset.Name}。儲存後生效。";
        return true;
    }
}
