using System.Globalization;
using System.Numerics;

namespace AgainstRomeMapEditor.Modules.Atmosphere;

/// <summary>
/// 原生 boden.ini 之強型別資料模型。
/// 鍵名與預設值核對 TEMP 原版 ENDL_000/005；觀察範圍及原檔註解見 docs/reverse-engineering/boden-ini-values.md。
/// </summary>
public sealed class BodenIniData
{
    // ==========================================
    // 1. 水面與地形高度格網 (Water & Height Grid)
    // ==========================================

    /// <summary>全圖基準海平面高度 (世界單位，原版預設 120)。[Waterlevel]</summary>
    public float Waterlevel { get; set; } = 120f;

    /// <summary>高度圖步進乘數 (通常為 4，64 tile * 4 + 1 = 257 頂點)。[Heightmapstep]</summary>
    public float Heightmapstep { get; set; } = 4f;

    // ==========================================
    // 2. 水體動態與渲染 (Water Simulation)
    // ==========================================

    /// <summary>水面波紋凹凸幅度 (0..1024，原版預設 256)。[WaterBumpAmplitude]</summary>
    public int WaterBumpAmplitude { get; set; } = 256;

    /// <summary>水面波紋頻率 (1..16，原版預設 4)。[WaterBumpFrequency]</summary>
    public int WaterBumpFrequency { get; set; } = 4;

    /// <summary>水面折射扭曲移位 (0=關閉；10=高扭曲..18=低扭曲，原檔註解預設14、樣本12)。[WaterWarpShift]</summary>
    public int WaterWarpShift { get; set; } = 12;

    /// <summary>水面是否呈現雨滴漣漪效果 (0=否, 1=是)。[RainDropsOnWater]</summary>
    public bool RainDropsOnWater { get; set; } = true;

    /// <summary>水體著色 Hex 格式 (0xbbggrr，原版預設 0xffdfbf)。[WaterColor]</summary>
    public string WaterColor { get; set; } = "0xffdfbf";

    /// <summary>水面基底貼圖名稱前綴 (原版預設 wassAW，對應 wassAW00.bmp 等)。[WasserTexturName]</summary>
    public string WasserTexturName { get; set; } = "wassAW";

    /// <summary>水底焦散光貼圖名稱前綴 (原版預設 caustA)。[CausticTexturName]</summary>
    public string CausticTexturName { get; set; } = "caustA";

    /// <summary>天空穹頂貼圖前綴 (原版預設 sky)。[SkyTexturName]</summary>
    public string SkyTexturName { get; set; } = "sky";

    // ==========================================
    // 3. 天氣與閃電 (Weather & Lightning)
    // ==========================================

    /// <summary>閃電參數 (0..1000，原檔註解為最大降雨強度時每秒閃電數；實際單位待確認)。[FlashPropability]</summary>
    public int FlashPropability { get; set; } = 8;

    /// <summary>閃電世界物件預設索引（樣本803）。[FlashObjectDefaultIndex]</summary>
    public int FlashObjectDefaultIndex { get; set; } = 803;

    /// <summary>閃電世界物件預設索引（樣本1099）。[FlashObjectDefault2Index]</summary>
    public int FlashObjectDefault2Index { get; set; } = 1099;

    /// <summary>閃電局部光源預設定義索引 (對應 lightdef.dau)。[FlashLightDefaultIndex]</summary>
    public int FlashLightDefaultIndex { get; set; } = 6;

    // ==========================================
    // 4. 冰雪與粒子特效 (Snow & Particles)
    // ==========================================

    /// <summary>飄雪 ALR 動畫素材索引 (樣本837；-1 為模型未指定，不代表已確認的遊戲關閉值)。[SnowAlrIndex]</summary>
    public int SnowAlrIndex { get; set; } = 837;

    /// <summary>雪花陰影索引。[SnowShadowIndex]</summary>
    public int SnowShadowIndex { get; set; } = 152;

    /// <summary>雪花陰影尺寸。[SnowShadowSize]</summary>
    public int SnowShadowSize { get; set; } = 7;

    /// <summary>冰雹陰影索引。[HagelShadowIndex]</summary>
    public int HagelShadowIndex { get; set; } = 152;

    /// <summary>冰雹陰影尺寸。[HagelShadowSize]</summary>
    public int HagelShadowSize { get; set; } = 7;

    /// <summary>動態物件搖曳幅度 (如樹木隨風擺動幅度)。[MoveListAmplitude]</summary>
    public int MoveListAmplitude { get; set; } = 127;

    // ==========================================
    // 5. 晝夜時序與太陽光影 (Day/Night & Shadow)
    // ==========================================

    /// <summary>白天開始時刻 (時 0..24，原版預設 6)。[DayStartTime]</summary>
    public float DayStartTime { get; set; } = 6f;

    /// <summary>夜晚開始時刻 (時 0..24，原版預設 20)。[DayEndTime]</summary>
    public float DayEndTime { get; set; } = 20f;

    /// <summary>陰影網格模式 (0=太陽僅上下俯仰移動, 1=太陽東南西完整軌跡)。[ShadowMeshMode]</summary>
    public int ShadowMeshMode { get; set; }

    /// <summary>陰影網格精確度 (原版預設 6)。[ShadowMeshAccuracy]</summary>
    public int ShadowMeshAccuracy { get; set; } = 6;

    /// <summary>陰影網格 XZ 範圍 (原版預設 16000)。[ShadowMeshXZsize]</summary>
    public int ShadowMeshXZsize { get; set; } = 16000;

    /// <summary>陰影網格 Y 範圍 (原版預設 5000)。[ShadowMeshYsize]</summary>
    public int ShadowMeshYsize { get; set; } = 5000;

    // ==========================================
    // 6. 天空密度與快取旗標 (Sky Density & Caches)
    // ==========================================

    /// <summary>天空密度擴散參數。[Skydensspread]</summary>
    public float Skydensspread { get; set; } = 16f;

    /// <summary>天空密度精確度。[Skydensaccuracy]</summary>
    public int Skydensaccuracy { get; set; } = 2;

    /// <summary>是否處理天空密度快取圖 skydens.dat (0/1)。[HandleSkyDensMap]</summary>
    public int HandleSkyDensMap { get; set; } = 1;

    /// <summary>是否處理可見性快取圖 visible.dat (0/1)。[HandleVisibleMap]</summary>
    public int HandleVisibleMap { get; set; } = 1;

    /// <summary>是否處理裁切矩形快取圖 cliprect.dat (0/1)。[HandleClipRectMap]</summary>
    public int HandleClipRectMap { get; set; } = 1;

    /// <summary>是否處理地形陰影快取圖 shadows.dat (0/1)。[HandleShadowMeshes]</summary>
    public int HandleShadowMeshes { get; set; } = 1;

    /// <summary>是否顯示碰撞網格 (偵錯旗標)。[ShowCollisionMesh]</summary>
    public int ShowCollisionMesh { get; set; }

    // ==========================================
    // 7. 未知或擴充區段保留 (Preserved Sections)
    // ==========================================

    /// <summary>
    /// 未識別或自訂區段字典（保留所有行、註解與格式），確保與原版或擴充地圖 100% 雙向無損相容。
    /// </summary>
    public Dictionary<string, string> PreservedSections { get; } = new(StringComparer.OrdinalIgnoreCase);

    /// <summary>
    /// 建立此設定物件之深層複本。
    /// </summary>
    public BodenIniData Clone()
    {
        var clone = (BodenIniData)MemberwiseClone();
        clone.PreservedSections.Clear();
        foreach (var kvp in PreservedSections)
        {
            clone.PreservedSections.Add(kvp.Key, kvp.Value);
        }
        return clone;
    }

    /// <summary>
    /// 將十六進位水體色彩轉為 RGB 向量 (0..1 範圍)。
    /// </summary>
    public Vector3 GetWaterRgb()
    {
        string hex = WaterColor.Trim().Replace("0x", "", StringComparison.OrdinalIgnoreCase);
        if (hex.Length == 6 && int.TryParse(hex, NumberStyles.HexNumber, CultureInfo.InvariantCulture, out int bgr))
        {
            float r = (bgr & 0xFF) / 255f;
            float g = ((bgr >> 8) & 0xFF) / 255f;
            float b = ((bgr >> 16) & 0xFF) / 255f;
            return new Vector3(r, g, b);
        }
        return new Vector3(0.25f, 0.5f, 0.8f);
    }

    /// <summary>
    /// 設定 RGB 數值轉為 0xbbggrr 十六進位字串。
    /// </summary>
    public void SetWaterRgb(float r, float g, float b)
    {
        byte rByte = (byte)Math.Clamp((int)MathF.Round(r * 255f), 0, 255);
        byte gByte = (byte)Math.Clamp((int)MathF.Round(g * 255f), 0, 255);
        byte bByte = (byte)Math.Clamp((int)MathF.Round(b * 255f), 0, 255);
        WaterColor = $"0x{bByte:x2}{gByte:x2}{rByte:x2}";
    }
}
