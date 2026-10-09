using System.Numerics;

namespace AgainstRomeMapEditor.Modules.Atmosphere;

/// <summary>
/// 經典天候與大氣預設集 (Weather Presets Catalog)。
/// 提供原版引擎參數與現代 3D 著色器之五大核心情境（晴朗豔陽、雷鳴暴風雨、暮色日落、濃霧迷漫、高山霜雪）。
/// </summary>
// 原生值採 ENDL_000/005 附近的保守變化；霧/風/降水等 profile 值僅供編輯器效果，並非 boden.ini 鍵。
public static class WeatherPresets
{
    public const string ClearSkyId = "clear_sky";
    public const string StormId = "storm";
    public const string SunsetDuskId = "sunset_dusk";
    public const string DenseFogId = "dense_fog";
    public const string MountainSnowId = "mountain_snow";

    /// <summary>
    /// 取得所有預建之大氣預設集清單。
    /// </summary>
    public static IReadOnlyList<AtmosphereProfile> All => new[]
    {
        CreateClearSky(),
        CreateStorm(),
        CreateSunsetDusk(),
        CreateDenseFog(),
        CreateMountainSnow()
    };

    /// <summary>
    /// 依識別碼查詢大氣預設。
    /// </summary>
    public static bool TryGetPreset(string presetId, out AtmosphereProfile profile)
    {
        foreach (var item in All)
        {
            if (string.Equals(item.PresetId, presetId, StringComparison.OrdinalIgnoreCase))
            {
                profile = item;
                return true;
            }
        }
        profile = CreateClearSky();
        return false;
    }

    /// <summary>
    /// 晴朗豔陽 (Clear Sky)：蔚藍晴空，明亮溫暖的光照，水面微波蕩漾。
    /// </summary>
    public static AtmosphereProfile CreateClearSky()
    {
        return new AtmosphereProfile
        {
            PresetId = ClearSkyId,
            Name = "晴朗豔陽",
            EnglishName = "Clear Sky",
            Description = "晴空萬里，陽光明媚。水面呈現柔和微瀾，遠山呈現自然的空氣透視淡藍微霞。",
            Boden = new BodenIniData
            {
                WaterBumpAmplitude = 224,
                WaterBumpFrequency = 4,
                WaterWarpShift = 14,
                WaterColor = "0xffdfbf", // 蔚藍透亮水色
                RainDropsOnWater = false,
                FlashPropability = 0,
                DayStartTime = 6f,
                DayEndTime = 20f,
                ShadowMeshMode = 0
            },
            FogEnabled = true,
            FogStartDistance = 60f,
            FogEndDistance = 180f,
            FogDensity = 0.20f,
            FogColor = new Vector3(0.72f, 0.82f, 0.95f),
            SkyScatteringColor = new Vector3(1.05f, 1.02f, 0.98f),
            AmbientTint = new Vector3(1.0f, 1.0f, 1.0f),
            Exposure = 1.0f,
            SunLightColor = new Vector3(1.0f, 0.98f, 0.92f),
            SunLightDirection = Vector3.Normalize(new Vector3(0.4f, 0.85f, 0.3f)),
            Precipitation = PrecipitationType.None,
            PrecipitationIntensity = 0f,
            WindSpeed = 4f,
            WindDirection = 45f
        };
    }

    /// <summary>
    /// 雷鳴暴風雨 (Storm)：狂風驟雨，暗沉天色，水波洶湧與高頻雷電爆閃。
    /// </summary>
    public static AtmosphereProfile CreateStorm()
    {
        return new AtmosphereProfile
        {
            PresetId = StormId,
            Name = "雷鳴暴風雨",
            EnglishName = "Thunderstorm",
            Description = "烏雲蔽日，風雨交加。水面狂浪翻騰且泛起密集雨滴漣漪，天候伴隨頻繁雷鳴爆閃。",
            Boden = new BodenIniData
            {
                WaterBumpAmplitude = 320,
                WaterBumpFrequency = 5,
                WaterWarpShift = 12,
                WaterColor = "0xe0c8aa", // 原版 BGR 水色附近的較暗色彩
                RainDropsOnWater = true,
                FlashPropability = 8,  // 原版樣本值；此參數的引擎單位仍待確認
                FlashLightDefaultIndex = 6,
                DayStartTime = 6f,
                DayEndTime = 20f,
                ShadowMeshMode = 0
            },
            FogEnabled = true,
            FogStartDistance = 12f,
            FogEndDistance = 60f,
            FogDensity = 0.85f,
            FogColor = new Vector3(0.22f, 0.26f, 0.28f), // 陰暗暴雨鉛灰色
            SkyScatteringColor = new Vector3(0.50f, 0.55f, 0.60f),
            AmbientTint = new Vector3(0.65f, 0.70f, 0.68f),
            Exposure = 0.75f,
            SunLightColor = new Vector3(0.55f, 0.58f, 0.62f),
            SunLightDirection = Vector3.Normalize(new Vector3(0.2f, 0.95f, 0.2f)),
            Precipitation = PrecipitationType.Rain,
            PrecipitationIntensity = 0.95f,
            WindSpeed = 22f,
            WindDirection = 75f
        };
    }

    /// <summary>
    /// 暮色日落 (Sunset Dusk)：日暮黃昏，夕陽西下，大地染上暖金紅調與長影。
    /// </summary>
    public static AtmosphereProfile CreateSunsetDusk()
    {
        return new AtmosphereProfile
        {
            PresetId = SunsetDuskId,
            Name = "暮色日落",
            EnglishName = "Sunset Dusk",
            Description = "殘陽如血，暮靄四起。遠景瀰漫橘紅霞霧，水面反照深邃晚霞光斑，沉靜而蒼茫。",
            Boden = new BodenIniData
            {
                WaterBumpAmplitude = 256,
                WaterBumpFrequency = 4,
                WaterWarpShift = 13,
                WaterColor = "0xe0d0d0", // 原版 BGR 水色附近的偏暖色彩
                RainDropsOnWater = false,
                FlashPropability = 0,
                DayStartTime = 6f,
                DayEndTime = 20f,
                ShadowMeshMode = 0
            },
            FogEnabled = true,
            FogStartDistance = 30f,
            FogEndDistance = 110f,
            FogDensity = 0.50f,
            FogColor = new Vector3(0.85f, 0.45f, 0.30f), // 溫暖晚霞橘紅
            SkyScatteringColor = new Vector3(1.20f, 0.75f, 0.45f),
            AmbientTint = new Vector3(1.10f, 0.85f, 0.70f),
            Exposure = 0.95f,
            SunLightColor = new Vector3(1.25f, 0.70f, 0.35f),
            SunLightDirection = Vector3.Normalize(new Vector3(0.85f, 0.35f, 0.4f)),
            Precipitation = PrecipitationType.None,
            PrecipitationIntensity = 0f,
            WindSpeed = 5f,
            WindDirection = 200f
        };
    }

    /// <summary>
    /// 濃霧迷漫 (Dense Fog)：沼澤林地，視距極短，灰濛濛的大氣營造神秘與伏擊氛圍。
    /// </summary>
    public static AtmosphereProfile CreateDenseFog()
    {
        return new AtmosphereProfile
        {
            PresetId = DenseFogId,
            Name = "濃霧迷漫",
            EnglishName = "Dense Fog",
            Description = "晨霧如幔，視線受限。近距離即被迷離白霧遮蔽，水波平緩，適合營造神秘森林或埋伏戰術。",
            Boden = new BodenIniData
            {
                WaterBumpAmplitude = 224,
                WaterBumpFrequency = 4,
                WaterWarpShift = 14,
                WaterColor = "0xf0d8bf", // 原版 BGR 水色附近的淡灰藍
                RainDropsOnWater = false,
                FlashPropability = 0,
                DayStartTime = 6f,
                DayEndTime = 20f,
                ShadowMeshMode = 0
            },
            FogEnabled = true,
            FogStartDistance = 4f,
            FogEndDistance = 38f, // 極近距離遮蔽
            FogDensity = 0.96f,
            FogColor = new Vector3(0.68f, 0.73f, 0.76f), // 冷灰白迷霧
            SkyScatteringColor = new Vector3(0.75f, 0.78f, 0.80f),
            AmbientTint = new Vector3(0.78f, 0.82f, 0.84f),
            Exposure = 0.85f,
            SunLightColor = new Vector3(0.70f, 0.72f, 0.75f),
            SunLightDirection = Vector3.Normalize(new Vector3(0.3f, 0.9f, 0.3f)),
            Precipitation = PrecipitationType.None,
            PrecipitationIntensity = 0f,
            WindSpeed = 2f,
            WindDirection = 90f
        };
    }

    /// <summary>
    /// 高山霜雪 (Mountain Snow)：冷冽冰原，飄雪霏霏，水面浮冰晶藍，天地清澈肅殺。
    /// </summary>
    public static AtmosphereProfile CreateMountainSnow()
    {
        return new AtmosphereProfile
        {
            PresetId = MountainSnowId,
            Name = "高山霜雪",
            EnglishName = "Mountain Snow",
            Description = "冰天雪地，寒風刺骨。大氣瀰漫晶瑩冰晶，水體泛起冰川碧藍，地面與遠山覆蓋霜雪清輝。",
            Boden = new BodenIniData
            {
                WaterBumpAmplitude = 224,
                WaterBumpFrequency = 4,
                WaterWarpShift = 14,
                WaterColor = "0xffe8cc", // 冰川冰藍碧綠
                RainDropsOnWater = false,
                FlashPropability = 0,
                SnowAlrIndex = 837,        // 原版樣本的雪花素材索引；不是降雪開關
                SnowShadowIndex = 152,
                SnowShadowSize = 7,
                DayStartTime = 6f,
                DayEndTime = 20f,
                ShadowMeshMode = 0
            },
            FogEnabled = true,
            FogStartDistance = 25f,
            FogEndDistance = 95f,
            FogDensity = 0.48f,
            FogColor = new Vector3(0.82f, 0.88f, 0.98f), // 霜雪冷銀藍
            SkyScatteringColor = new Vector3(0.90f, 0.95f, 1.15f),
            AmbientTint = new Vector3(0.92f, 0.96f, 1.10f),
            Exposure = 1.12f,
            SunLightColor = new Vector3(0.95f, 0.98f, 1.05f),
            SunLightDirection = Vector3.Normalize(new Vector3(0.5f, 0.75f, 0.4f)),
            Precipitation = PrecipitationType.Snow,
            PrecipitationIntensity = 0.70f,
            WindSpeed = 12f,
            WindDirection = 315f
        };
    }
}
