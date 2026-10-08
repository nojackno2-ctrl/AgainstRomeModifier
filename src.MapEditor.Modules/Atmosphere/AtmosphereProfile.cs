using System.Numerics;

namespace AgainstRomeMapEditor.Modules.Atmosphere;

/// <summary>
/// 降水天候類型。
/// </summary>
public enum PrecipitationType
{
    None = 0,
    Rain = 1,
    Snow = 2,
    Hail = 3
}

/// <summary>
/// 大氣與天候完整配置設定檔 (Atmosphere Profile)。
/// 結合原生遊戲引擎 boden.ini 參數與現代 3D 即時視圖光照/大氣散射/霧氣深度著色管線。
/// </summary>
public sealed class AtmosphereProfile
{
    // ==========================================
    // 識別與描述
    // ==========================================

    /// <summary>預設唯一識別碼 (例如 "clear_sky", "storm", "sunset_dusk", "dense_fog", "mountain_snow")。</summary>
    public string PresetId { get; set; } = "custom";

    /// <summary>繁體中文顯示名稱。</summary>
    public string Name { get; set; } = "自訂環境";

    /// <summary>英文顯示名稱。</summary>
    public string EnglishName { get; set; } = "Custom Atmosphere";

    /// <summary>詳細描述說明。</summary>
    public string Description { get; set; } = string.Empty;

    // ==========================================
    // 原生引擎 boden.ini 參數
    // ==========================================

    /// <summary>原生 boden.ini 設定資料。</summary>
    public BodenIniData Boden { get; set; } = new();

    // ==========================================
    // 3D 視圖大氣與霧氣 (Atmosphere & Fog)
    // ==========================================

    /// <summary>是否啟用大氣霧氣渲染。</summary>
    public bool FogEnabled { get; set; } = true;

    /// <summary>霧氣起始距離（以地圖格 tile 為單位，相機起算）。</summary>
    public float FogStartDistance { get; set; } = 40f;

    /// <summary>霧氣完全遮蔽終止距離（以地圖格 tile 為單位）。</summary>
    public float FogEndDistance { get; set; } = 120f;

    /// <summary>霧氣整體最大不透明密度 (0.0..1.0)。</summary>
    public float FogDensity { get; set; } = 0.5f;

    /// <summary>大氣霧氣色彩 (RGB 0.0..1.0)。</summary>
    public Vector3 FogColor { get; set; } = new(0.7f, 0.75f, 0.8f);

    /// <summary>天色大氣瑞利/米氏散射加權係數 (RGB)。</summary>
    public Vector3 SkyScatteringColor { get; set; } = Vector3.One;

    // ==========================================
    // 光照調節 (Lighting Modulation)
    // ==========================================

    /// <summary>全域時段環境色調修飾乘數 (乘上 daynight.bmp row0)。</summary>
    public Vector3 AmbientTint { get; set; } = Vector3.One;

    /// <summary>整體視圖曝光增益係數 (0.5..2.0，標準為 1.0)。</summary>
    public float Exposure { get; set; } = 1.0f;

    /// <summary>主要方向光/陽光色彩。</summary>
    public Vector3 SunLightColor { get; set; } = new(1.0f, 0.98f, 0.92f);

    /// <summary>主要方向光朝向 (標準化向量)。</summary>
    public Vector3 SunLightDirection { get; set; } = Vector3.Normalize(new(0.4f, 0.85f, 0.3f));

    // ==========================================
    // 天氣現象與動態因子 (Weather Dynamics)
    // ==========================================

    /// <summary>降水型態 (無 / 下雨 / 降雪 / 冰雹)。</summary>
    public PrecipitationType Precipitation { get; set; } = PrecipitationType.None;

    /// <summary>降水強度 (0.0..1.0)。</summary>
    public float PrecipitationIntensity { get; set; }

    /// <summary>風速強度 (單位/秒，影響水面波浪動態頻率與粒子飄移)。</summary>
    public float WindSpeed { get; set; } = 5f;

    /// <summary>風向角度 (0..360 度)。</summary>
    public float WindDirection { get; set; } = 45f;

    /// <summary>
    /// 建立此大氣設定檔之完整深層複本。
    /// </summary>
    public AtmosphereProfile Clone()
    {
        return new AtmosphereProfile
        {
            PresetId = PresetId,
            Name = Name,
            EnglishName = EnglishName,
            Description = Description,
            Boden = Boden.Clone(),
            FogEnabled = FogEnabled,
            FogStartDistance = FogStartDistance,
            FogEndDistance = FogEndDistance,
            FogDensity = FogDensity,
            FogColor = FogColor,
            SkyScatteringColor = SkyScatteringColor,
            AmbientTint = AmbientTint,
            Exposure = Exposure,
            SunLightColor = SunLightColor,
            SunLightDirection = SunLightDirection,
            Precipitation = Precipitation,
            PrecipitationIntensity = PrecipitationIntensity,
            WindSpeed = WindSpeed,
            WindDirection = WindDirection
        };
    }

    /// <summary>
    /// 檢查兩個設定檔內容是否等價（用於變更偵測與髒標記判斷）。
    /// </summary>
    public bool ValueEquals(AtmosphereProfile other)
    {
        if (other is null) return false;
        if (ReferenceEquals(this, other)) return true;

        return PresetId == other.PresetId
            && Name == other.Name
            && EnglishName == other.EnglishName
            && FogEnabled == other.FogEnabled
            && MathF.Abs(FogStartDistance - other.FogStartDistance) < 0.001f
            && MathF.Abs(FogEndDistance - other.FogEndDistance) < 0.001f
            && MathF.Abs(FogDensity - other.FogDensity) < 0.001f
            && FogColor == other.FogColor
            && SkyScatteringColor == other.SkyScatteringColor
            && AmbientTint == other.AmbientTint
            && MathF.Abs(Exposure - other.Exposure) < 0.001f
            && Precipitation == other.Precipitation
            && MathF.Abs(PrecipitationIntensity - other.PrecipitationIntensity) < 0.001f
            && MathF.Abs(WindSpeed - other.WindSpeed) < 0.001f
            && MathF.Abs(WindDirection - other.WindDirection) < 0.001f
            && BodenValuesEqual(Boden, other.Boden);
    }

    private static bool BodenValuesEqual(BodenIniData a, BodenIniData b)
    {
        return MathF.Abs(a.Waterlevel - b.Waterlevel) < 0.001f
            && MathF.Abs(a.Heightmapstep - b.Heightmapstep) < 0.001f
            && a.WaterBumpAmplitude == b.WaterBumpAmplitude
            && a.WaterBumpFrequency == b.WaterBumpFrequency
            && a.WaterWarpShift == b.WaterWarpShift
            && a.RainDropsOnWater == b.RainDropsOnWater
            && string.Equals(a.WaterColor, b.WaterColor, StringComparison.OrdinalIgnoreCase)
            && string.Equals(a.WasserTexturName, b.WasserTexturName, StringComparison.OrdinalIgnoreCase)
            && a.FlashPropability == b.FlashPropability
            && a.FlashLightDefaultIndex == b.FlashLightDefaultIndex
            && a.SnowAlrIndex == b.SnowAlrIndex
            && MathF.Abs(a.DayStartTime - b.DayStartTime) < 0.001f
            && MathF.Abs(a.DayEndTime - b.DayEndTime) < 0.001f
            && a.ShadowMeshMode == b.ShadowMeshMode;
    }
}
