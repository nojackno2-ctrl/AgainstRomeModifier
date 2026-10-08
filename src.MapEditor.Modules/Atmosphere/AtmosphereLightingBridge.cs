using System.Numerics;
using AgainstRomeMapEditor.NativeAssets;

namespace AgainstRomeMapEditor.Modules.Atmosphere;

/// <summary>
/// 著色器即時大氣 Uniform 數值封裝。
/// </summary>
public sealed class AtmosphereShaderUniforms
{
    /// <summary>綜合環境光色彩（結合時段 daynight.bmp、AmbientTint、Exposure 與雷電爆閃）。</summary>
    public Vector3 EffectiveAmbient { get; init; } = Vector3.One;

    /// <summary>霧氣基礎色彩。</summary>
    public Vector3 FogColor { get; init; } = new(0.7f, 0.75f, 0.8f);

    /// <summary>
    /// 霧氣控制參數向量：
    /// X: 起始距離 (tile), Y: 終止距離 (tile), Z: 霧氣濃度 (0..1), W: 是否啟用 (1.0=啟用, 0.0=停用)。
    /// </summary>
    public Vector4 FogParams { get; init; } = new(40f, 120f, 0.5f, 1f);

    /// <summary>天色散射增益。</summary>
    public Vector3 SkyScattering { get; init; } = Vector3.One;

    /// <summary>主要方向光色彩。</summary>
    public Vector3 SunColor { get; init; } = Vector3.One;

    /// <summary>主要方向光向量。</summary>
    public Vector3 SunDirection { get; init; } = new(0.4f, 0.85f, 0.3f);

    /// <summary>
    /// 水體動態參數向量：
    /// X: 凹凸幅度 (0..1024), Y: 凹凸頻率 (1..16), Z: 扭曲位移 (0=關閉, 10..18), W: 雨滴漣漪開關 (0.0/1.0)。
    /// </summary>
    public Vector4 WaterDynamics { get; init; } = new(256f, 4f, 14f, 0f);

    /// <summary>當前雷鳴瞬間爆閃強度 (0.0..1.0，0 代表無雷電)。</summary>
    public float FlashIntensity { get; init; }

    /// <summary>動態模擬累積時間（秒），供著色器計算正弦波紋與漣漪動畫。</summary>
    public float SimulationTime { get; init; }
}

/// <summary>
/// 3D 視圖環境色與光照管線連動橋接器 (Atmosphere Lighting Bridge)。
/// 負責將時段 (GameHour)、天候配置 (AtmosphereProfile) 及動態天候特效（水波紋、雷電爆閃、大氣迷霧）
/// 即時傳入 Map3DViewControl 的著色器與渲染管線中。
/// </summary>
public sealed class AtmosphereLightingBridge
{
    private float _simulationTime;
    private float _flashIntensity;
    private float _nextFlashTimer;
    private readonly Random _random = new(42);

    /// <summary>當前累積模擬時間（秒）。</summary>
    public float SimulationTime => _simulationTime;

    /// <summary>當前雷電爆閃亮度（0.0..1.0）。</summary>
    public float FlashIntensity => _flashIntensity;

    /// <summary>
    /// 每幀推進模擬時鐘並計算隨機閃電與動態水波週期。
    /// </summary>
    /// <param name="deltaTimeSeconds">影格間隔時間（秒）。</param>
    /// <param name="profile">當前大氣設定檔。</param>
    public void Update(float deltaTimeSeconds, AtmosphereProfile profile)
    {
        if (deltaTimeSeconds <= 0f) return;
        _simulationTime += deltaTimeSeconds;

        // 衰減現有閃電強度 (約 0.15 秒內迅速消退，模擬真實電光猝滅)
        if (_flashIntensity > 0f)
        {
            _flashIntensity = MathF.Max(0f, _flashIntensity - deltaTimeSeconds * 6.5f);
        }

        // 模擬雷鳴閃電機率
        int flashProb = profile?.Boden?.FlashPropability ?? 0;
        if (flashProb > 0)
        {
            _nextFlashTimer -= deltaTimeSeconds;
            if (_nextFlashTimer <= 0f)
            {
                // 編輯器預覽的啟發式間隔；不是原生引擎 FlashPropability 的已驗證單位。
                // 原檔註解稱最大降雨時每秒閃電數，與此預覽算法不同，仍待引擎追蹤確認。
                float averageInterval = MathF.Max(0.5f, 1000f / flashProb);
                // 泊松隨機間隔偏移 (0.5 ~ 1.5 倍平均間隔)
                _nextFlashTimer = averageInterval * (0.5f + (float)_random.NextDouble());

                // 觸發閃電爆閃，隨機峰值 0.8..1.0
                _flashIntensity = 0.8f + (float)_random.NextDouble() * 0.2f;
            }
        }
        else
        {
            _flashIntensity = 0f;
            _nextFlashTimer = 5f;
        }
    }

    /// <summary>
    /// 手動強行觸發一次雷電爆閃（供編輯器預覽按鈕使用）。
    /// </summary>
    public void TriggerManualFlash(float intensity = 1.0f)
    {
        _flashIntensity = Math.Clamp(intensity, 0f, 1.0f);
    }

    /// <summary>
    /// 計算用於 3D 著色器之全套即時 Uniform 數值。
    /// </summary>
    /// <param name="profile">大氣配置檔。</param>
    /// <param name="gameHour">當前地圖時鐘小時 (0..24)。</param>
    /// <summary>
    /// 計算包含自訂基礎環境光的主著色器 Uniforms（供測試與自訂光照使用）。
    /// </summary>
    internal AtmosphereShaderUniforms ComputeUniforms(
        AtmosphereProfile profile,
        Vector3 baseAmbient)
    {
        ArgumentNullException.ThrowIfNull(profile);
        Vector3 litAmbient = baseAmbient * profile.AmbientTint * profile.Exposure;
        if (_flashIntensity > 0.001f)
        {
            litAmbient = Vector3.Min(new Vector3(1.6f, 1.6f, 1.6f), litAmbient + Vector3.One * (_flashIntensity * 0.75f));
        }
        var boden = profile.Boden;
        var waterDynamics = new Vector4(
            boden.WaterBumpAmplitude,
            boden.WaterBumpFrequency,
            boden.WaterWarpShift,
            boden.RainDropsOnWater ? 1f : 0f);
        var fogParams = new Vector4(
            profile.FogStartDistance,
            profile.FogEndDistance,
            profile.FogDensity,
            profile.FogEnabled ? 1f : 0f);
        return new AtmosphereShaderUniforms
        {
            EffectiveAmbient = litAmbient,
            FogColor = profile.FogColor,
            FogParams = fogParams,
            SkyScattering = profile.SkyScatteringColor,
            SunColor = profile.SunLightColor,
            SunDirection = profile.SunLightDirection,
            WaterDynamics = waterDynamics,
            FlashIntensity = _flashIntensity,
            SimulationTime = _simulationTime
        };
    }

    /// <param name="lightingContext">場景原生光照上下文（載入 daynight.bmp 與 APT 光源）。</param>
    internal AtmosphereShaderUniforms ComputeUniforms(
        AtmosphereProfile profile,
        float gameHour,
        SceneLightingContext? lightingContext = null)
    {
        ArgumentNullException.ThrowIfNull(profile);

        // 1. 取得日夜基礎環境色 (0x49A350 原生插值)
        Vector3 baseAmbient = lightingContext?.GetAmbientColor(gameHour) ?? Vector3.One;

        // 2. 乘上大氣調色盤與曝光增益
        Vector3 litAmbient = baseAmbient * profile.AmbientTint * profile.Exposure;

        // 3. 疊加雷電全域爆閃 (白光增益)
        if (_flashIntensity > 0.001f)
        {
            litAmbient = Vector3.Min(new Vector3(1.6f, 1.6f, 1.6f), litAmbient + Vector3.One * (_flashIntensity * 0.75f));
        }

        // 4. 水體參數
        var boden = profile.Boden;
        var waterDynamics = new Vector4(
            boden.WaterBumpAmplitude,
            boden.WaterBumpFrequency,
            boden.WaterWarpShift,
            boden.RainDropsOnWater ? 1f : 0f);

        // 5. 霧氣參數 (X=start, Y=end, Z=density, W=enabled)
        var fogParams = new Vector4(
            profile.FogStartDistance,
            profile.FogEndDistance,
            profile.FogDensity,
            profile.FogEnabled ? 1f : 0f);

        return new AtmosphereShaderUniforms
        {
            EffectiveAmbient = litAmbient,
            FogColor = profile.FogColor,
            FogParams = fogParams,
            SkyScattering = profile.SkyScatteringColor,
            SunColor = profile.SunLightColor,
            SunDirection = profile.SunLightDirection,
            WaterDynamics = waterDynamics,
            FlashIntensity = _flashIntensity,
            SimulationTime = _simulationTime
        };
    }

    // ==========================================
    // 著色器擴充程式碼片段產生器 (GLSL Extensions)
    // ==========================================

    /// <summary>
    /// 提供予 Map3DViewControl 著色器之大氣與迷霧通用 GLSL 標頭代碼。
    /// </summary>
    public const string GlslAtmosphereHeader = """
        // 大氣與天氣系統動態 Uniforms (由 AtmosphereLightingBridge 提供)
        uniform int uAtmosphereFogEnabled;
        uniform vec4 uAtmosphereFogParams; // x: startDistance, y: endDistance, z: density, w: unused
        uniform vec3 uAtmosphereFogColor;
        uniform vec3 uAtmosphereSkyScattering;
        uniform float uAtmosphereFlashIntensity;
        uniform vec4 uAtmosphereWaterDynamics; // x: bumpAmp, y: bumpFreq, z: warpShift, w: rainDrops
        uniform float uAtmosphereTime;

        // 空氣透視與距離迷霧混合函式
        vec3 applyAtmosphereFog(vec3 litColor, float viewDist) {
            if (uAtmosphereFogEnabled == 0) return litColor;
            float factor = clamp((viewDist - uAtmosphereFogParams.x) / max(0.001, uAtmosphereFogParams.y - uAtmosphereFogParams.x), 0.0, 1.0);
            factor = factor * uAtmosphereFogParams.z;
            return mix(litColor, uAtmosphereFogColor, factor);
        }

        // 閃電瞬時強光增益
        vec3 applyAtmosphereFlash(vec3 litColor) {
            return litColor + vec3(uAtmosphereFlashIntensity * 0.7);
        }
        """;

    /// <summary>
    /// 提供予水面 Shader 之動態波紋正弦/柏林擾動計算片段。
    /// </summary>
    public const string GlslWaterDynamics = """
        vec2 computeWaterRippleOffset(vec2 worldPos, float time, float amplitude, float frequency) {
            float phase = time * (frequency * 0.4);
            float wave1 = sin(worldPos.x * 0.15 + phase) * cos(worldPos.y * 0.15 + phase * 0.8);
            float wave2 = sin(worldPos.x * 0.3 - phase * 1.2) * cos(worldPos.y * 0.25 + phase * 1.1);
            float normAmp = amplitude / 1024.0 * 0.08;
            return vec2(wave1 + wave2 * 0.5, wave2 + wave1 * 0.5) * normAmp;
        }
        """;
}
