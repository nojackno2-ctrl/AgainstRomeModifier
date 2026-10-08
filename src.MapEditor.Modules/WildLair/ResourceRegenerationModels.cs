namespace AgainstRomeMapEditor.Modules.WildLair;

/// <summary>
/// 森林微網格細胞生態狀態。
/// </summary>
public enum ForestCellState : byte
{
    /// <summary>開闊生長空地（土壤可用，等待鄰近林地散播種子）。</summary>
    Barren = 0,
    /// <summary>採伐殘樁（工人砍伐後留下的樹樁，具腐朽週期與養分釋放）。</summary>
    Stump = 1,
    /// <summary>樹苗幼株（剛發芽的新生幼木，脆弱易損）。</summary>
    Sapling = 2,
    /// <summary>成長期小樹（生長中喬木，開始阻擋視野與通行）。</summary>
    YoungTree = 3,
    /// <summary>成熟喬木（具完整林冠，可供伐木工採伐，能向外播種）。</summary>
    MatureTree = 4,
    /// <summary>原始巨木老林（林相核心，種子擴散機率與生態穩定度最高）。</summary>
    AncientCanopy = 5,
    /// <summary>水體地貌（湖泊、河流、濕地沼澤，阻擋樹木生長）。</summary>
    Water = 6,
    /// <summary>人造建築、岩石峭壁或不可生長障礙。</summary>
    Obstacle = 7
}

/// <summary>
/// 森林網格單一細胞結構。
/// </summary>
public sealed record ForestGridCell(
    int X,
    int Y,
    ForestCellState State,
    float Fertility = 0.5f,
    float Moisture = 0.5f,
    int AgeTicks = 0,
    string TreeSpecies = "LanGerTanne01");

/// <summary>
/// 砍伐殘樁追蹤標記紀錄。
/// </summary>
public sealed record StumpHarvestRecord(
    int TileX,
    int TileZ,
    float WorldX,
    float WorldZ,
    string OriginalSpecies,
    int NativeSlotIndex = -1,
    int RemainingDecayTicks = 6);

/// <summary>
/// Cellular Automata 森林動態再生模擬器參數。
/// </summary>
public sealed class RegenerationParameters
{
    /// <summary>成熟林木向 Moore 八鄰域播種的基準擴散率（0.01 ~ 0.50，預設 0.12）。</summary>
    public float SeedDispersalProbability { get; init; } = 0.12f;

    /// <summary>原始老林（AncientCanopy）種子傳播加成倍率（預設 1.75）。</summary>
    public float AncientSeedDispersalBonus { get; init; } = 1.75f;

    /// <summary>幼苗成長為小樹所需迭代時步（預設 3 步）。</summary>
    public int SaplingToYoungTicks { get; init; } = 3;

    /// <summary>小樹成長為成樹所需迭代時步（預設 5 步）。</summary>
    public int YoungToMatureTicks { get; init; } = 5;

    /// <summary>成樹晉升為原始巨木老林所需迭代時步（預設 12 步）。</summary>
    public int MatureToAncientTicks { get; init; } = 12;

    /// <summary>砍伐殘樁風化腐朽為肥沃土地所需時步（預設 6 步）。</summary>
    public int StumpDecayTicks { get; init; } = 6;

    /// <summary>森林自疏（Self-Thinning）擁擠致死門檻（當周圍 8 鄰域成熟樹 >= 7 株時枯萎機率，預設 0.08）。</summary>
    public float CrowdingMortalityRate { get; init; } = 0.08f;

    /// <summary>水體鄰近（濕潤度）對發芽與成長之速度加成（預設 1.35）。</summary>
    public float MoistureGrowthBonus { get; init; } = 1.35f;

    /// <summary>最大可生長地表坡度傾角（超過則視為陡坡岩壁，不可生長樹木）。</summary>
    public float MaxGrowableSlope { get; init; } = 0.35f;

    /// <summary>隨機種子碼。</summary>
    public int Seed { get; init; } = 1337;
}

/// <summary>
/// 森林生態網格時序統計摘要。
/// </summary>
public sealed record ForestEcologyStatistics(
    int TotalCells,
    int BarrenCount,
    int StumpCount,
    int SaplingCount,
    int YoungTreeCount,
    int MatureTreeCount,
    int AncientCanopyCount,
    int WaterOrObstacleCount,
    float ForestCanopyCoveragePercentage);
