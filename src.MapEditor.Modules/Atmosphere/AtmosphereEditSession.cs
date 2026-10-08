using System.Numerics;

namespace AgainstRomeMapEditor.Modules.Atmosphere;

/// <summary>
/// 大氣與天候編輯工作階段（實作 <see cref="IEditorModule{TSnapshot}"/> 統一架構介面）。
/// 負責追蹤髒狀態（IsDirty）、Undo/Redo 基準快照、預設套用與手動微調。
/// 不依賴 WinForms 控制項或原生檔案存取，可受單元測試完整涵蓋。
/// </summary>
public sealed class AtmosphereEditSession : IEditorModule<AtmosphereProfile>
{
    private AtmosphereProfile _current;
    private AtmosphereProfile _baseline;

    public AtmosphereEditSession()
    {
        _current = WeatherPresets.CreateClearSky();
        _baseline = _current.Clone();
    }

    public AtmosphereEditSession(AtmosphereProfile initial)
    {
        _current = initial?.Clone() ?? WeatherPresets.CreateClearSky();
        _baseline = _current.Clone();
    }

    /// <summary>模組識別字串。</summary>
    public string ModuleId => "atmosphere";

    /// <summary>當前編輯內容相較於基準快照是否有變更。</summary>
    public bool IsDirty => !_current.ValueEquals(_baseline);

    /// <summary>取得當前大氣設定檔唯讀參考。</summary>
    public AtmosphereProfile Current => _current;

    /// <summary>載入新大氣快照並將其設為乾淨基準。</summary>
    public void Load(AtmosphereProfile snapshot)
    {
        ArgumentNullException.ThrowIfNull(snapshot);
        _current = snapshot.Clone();
        AcceptChanges();
    }

    /// <summary>擷取當前大氣快照（深層複本）。</summary>
    public AtmosphereProfile Capture() => _current.Clone();

    /// <summary>確認並提交當前變更，更新基準狀態使 IsDirty 重設為 false。</summary>
    public void AcceptChanges()
    {
        _baseline = _current.Clone();
    }

    /// <summary>放棄未儲存之變更，還原為前次基準狀態。</summary>
    public void Reset()
    {
        _current = _baseline.Clone();
    }

    // ==========================================
    // 編輯操作 API (Authoring Operations)
    // ==========================================

    /// <summary>套用指定預設集。</summary>
    public void ApplyPreset(string presetId)
    {
        if (WeatherPresets.TryGetPreset(presetId, out var preset))
        {
            // 套用時保留原水體基準高度與高度圖步進
            float origWaterlevel = _current.Boden.Waterlevel;
            float origHeightStep = _current.Boden.Heightmapstep;

            _current = preset.Clone();
            _current.Boden.Waterlevel = origWaterlevel;
            _current.Boden.Heightmapstep = origHeightStep;
        }
    }

    /// <summary>更新水面波紋動態。</summary>
    public void SetWaterBump(int amplitude, int frequency, int warpShift)
    {
        _current.Boden.WaterBumpAmplitude = Math.Clamp(amplitude, 0, 1024);
        _current.Boden.WaterBumpFrequency = Math.Clamp(frequency, 1, 16);
        _current.Boden.WaterWarpShift = Math.Clamp(warpShift, 8, 20);
    }

    /// <summary>更新水體色彩 Hex。</summary>
    public void SetWaterColor(string hexColor)
    {
        if (!string.IsNullOrWhiteSpace(hexColor))
        {
            _current.Boden.WaterColor = hexColor.Trim();
        }
    }

    /// <summary>設定閃電每秒機率。</summary>
    public void SetFlashProbability(int flashProbability)
    {
        _current.Boden.FlashPropability = Math.Clamp(flashProbability, 0, 1000);
    }

    /// <summary>設定晝夜起訖時間。</summary>
    public void SetDayNightTimes(float dayStartTime, float dayEndTime)
    {
        _current.Boden.DayStartTime = Math.Clamp(dayStartTime, 0f, 24f);
        _current.Boden.DayEndTime = Math.Clamp(dayEndTime, 0f, 24f);
    }

    /// <summary>設定大氣迷霧參數。</summary>
    public void SetFog(bool enabled, float startDistance, float endDistance, float density, Vector3 color)
    {
        _current.FogEnabled = enabled;
        _current.FogStartDistance = MathF.Max(0f, startDistance);
        _current.FogEndDistance = MathF.Max(startDistance + 0.1f, endDistance);
        _current.FogDensity = Math.Clamp(density, 0f, 1f);
        _current.FogColor = Vector3.Clamp(color, Vector3.Zero, Vector3.One);
    }

    /// <summary>設定光照調色與曝光。</summary>
    public void SetLightingModulation(Vector3 ambientTint, float exposure)
    {
        _current.AmbientTint = Vector3.Clamp(ambientTint, Vector3.Zero, Vector3.One * 2f);
        _current.Exposure = Math.Clamp(exposure, 0.2f, 3.0f);
    }

    /// <summary>設定降水天候。</summary>
    public void SetPrecipitation(PrecipitationType type, float intensity)
    {
        _current.Precipitation = type;
        _current.PrecipitationIntensity = Math.Clamp(intensity, 0f, 1f);
        _current.Boden.RainDropsOnWater = type == PrecipitationType.Rain && intensity > 0.1f;
        _current.Boden.SnowAlrIndex = type == PrecipitationType.Snow ? 1 : -1;
    }
}
