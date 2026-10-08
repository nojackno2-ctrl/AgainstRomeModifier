namespace AgainstRomeMapEditor.Modules.Profiling;

/// <summary>
/// Against Rome 原生遊戲引擎與編輯器之硬限制 (Hard Limits) 與安全預算警戒線 (Redlines)。
/// 依據逆向工程反組譯 (0x48BAE0, 0x49FFE0, 0x48AB70 等) 與 ENDL_000 基準量測推導。
/// </summary>
public static class MapBudgetLimits
{
    // =========================================================================
    // 1. 物件與資料池硬限制 (Entity & Pool Limits)
    // =========================================================================

    /// <summary>objects.dat 標頭固定容量：14,000 槽位 (0x48BAE0)。超過此數量遊戲無法配置物件。</summary>
    public const int MaxObjectsDatSlots = 14_000;

    /// <summary>物件插槽安全警戒線：10,000 (保留 4,000 給遊戲中動態生成、破壞碎片、掉落物與 NPC 擴張)。</summary>
    public const int RedlineObjectsDatSlots = 10_000;

    /// <summary>建議物件數量上限：8,000 (ENDL_000 官方地圖約 6,618 ~ 7,231 個場景物件)。</summary>
    public const int RecommendedObjectsDatSlots = 8_000;

    /// <summary>position.dat 標頭固定容量：33,000 筆記錄 (每個物件佔一對 position，共可支援 16,500 物件)。</summary>
    public const int MaxPositionDatRecords = 33_000;

    /// <summary>position 記錄安全警戒線：24,000 筆。</summary>
    public const int RedlinePositionDatRecords = 24_000;

    /// <summary>劇本 ScenarioSpawns 建議上限 (SDL 腳本開局初始化負載)。</summary>
    public const int RecommendedScenarioSpawns = 500;

    /// <summary>劇本 ScenarioSpawns 安全警戒線：1,000 (過多預放物件會拖慢 BCI 開局執行速度)。</summary>
    public const int RedlineScenarioSpawns = 1_000;

    /// <summary>lager.dat (倉庫/庫存) 槽位上限：3,200 (0x48D2D0)。</summary>
    public const int MaxLagerSlots = 3_200;

    /// <summary>biglager.dat (大型倉庫) 槽位上限：800 (0x48D590)。</summary>
    public const int MaxBigLagerSlots = 800;

    /// <summary>particle.dat 粒子發射器槽位上限：64 (0x48DD90)。</summary>
    public const int MaxParticleSlots = 64;

    /// <summary>explos.dat 爆炸特效槽位上限：46 (0x48E210)。</summary>
    public const int MaxExplosSlots = 46;

    /// <summary>flash.dat 閃電天候槽位上限：16 (0x48B910)。</summary>
    public const int MaxFlashSlots = 16;

    /// <summary>way.dat 尋路路徑點上限：256 (0x48DBA0)。</summary>
    public const int MaxWayPoints = 256;


    // =========================================================================
    // 2. 渲染與圖形管線限制 (Rendering & Draw Call Limits)
    // =========================================================================

    /// <summary>執行期全域動態光源陣列上限：1,024 (0x7F9EF0 步長 0x44)。</summary>
    public const int MaxGlobalRuntimeLights = 1_024;

    /// <summary>單一視圖/相機視錐 (Frustum) 可生效之最大光源數：64 (Shader uniform array / 固定管線硬限制)。</summary>
    public const int MaxActiveLightsInView = 64;

    /// <summary>單一局部區域聚集之重疊光源硬限制：64 (超過將被原生引擎截斷，導致閃爍瑕疵)。</summary>
    public const int HardLimitLightsPerCluster = 64;

    /// <summary>局部重疊光源警戒線：48 (接近上限，相機平移時極易觸發截斷)。</summary>
    public const int RedlineLightsPerCluster = 48;

    /// <summary>建議局部光源上限：32 (確保平滑衰減與無瑕疵照明)。</summary>
    public const int RecommendedLightsPerCluster = 32;

    /// <summary>16-bit 索引緩衝單一 Draw Call 最大 Quad 數：16,384 quads (65,536 頂點)。</summary>
    public const int MaxSpriteQuadsPerBatch = 16_384;

    /// <summary>非索引 Sprite 頂點緩衝建議上限 (10,922 quads * 6 = 65,532 頂點)。</summary>
    public const int SafeSpriteQuadsPerDrawCall = 10_922;

    /// <summary>4096×4096 Sprite Atlas 頁數建議上限 (超過將引發頻繁紋理切換)。</summary>
    public const int RecommendedAtlasPages = 2;

    /// <summary>Sprite Atlas 頁數警戒線：4 頁。</summary>
    public const int RedlineAtlasPages = 4;

    /// <summary>基準地圖陰影 Quad 數量參考 (ENDL_000 基準 2,050)。</summary>
    public const int BenchmarkShadowQuads = 2_050;

    /// <summary>陰影 Quad 數量警戒線：5,000。</summary>
    public const int RedlineShadowQuads = 5_000;


    // =========================================================================
    // 3. 地圖尺寸與尋路網格限制 (Map Geometry & Pathfinding Limits)
    // =========================================================================

    /// <summary>地圖網格尺寸：256×256 tiles (每個 tile 64 世界座標單位，全圖 16,384×16,384)。</summary>
    public const int MapGridDimension = 256;

    /// <summary>世界座標總跨度：16,384 世界座標單位。</summary>
    public const int MapWorldSize = 16_384;

    /// <summary>每個 tile 的世界座標寬度：64.0 單位。</summary>
    public const float WorldUnitsPerTile = 64.0f;

    /// <summary>地形高度頂點網格維度：257×257。</summary>
    public const int HeightGridDimension = 257;

    /// <summary>尋路網格總節點數：65,536 (256×256)。</summary>
    public const int TotalPathfindingNodes = 65_536;

    /// <summary>狹窄隘口寬度門檻：小於等於 2 格 (128 世界單位)，百人大軍易嚴重卡死引發 A* 尋路風暴。</summary>
    public const int ChokepointWidthThresholdCells = 2;

    /// <summary>地圖隘口總數警戒線 (超過此數量可能造成大量部隊同時堵塞)。</summary>
    public const int RedlineChokepoints = 30;

    /// <summary>死胡同深口袋門檻：深度大於等於 8 格之單向窄道。</summary>
    public const int DeadEndMinDepthCells = 8;

    /// <summary>死胡同數量警戒線：15 處。</summary>
    public const int RedlineDeadEnds = 15;

    /// <summary>可通行區域孤島數量警戒線 (獨立連通分量 > 1 即有孤島，> 10 為嚴重破碎)。</summary>
    public const int RedlineIsolatedRegions = 10;
}
