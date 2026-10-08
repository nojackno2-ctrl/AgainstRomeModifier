namespace AgainstRomeMapEditor.Modules.Objectives;

using AgainstRomeModifier.Scripting;

/// <summary>
/// 戰役目標種類，涵蓋《反抗羅馬》常見的戰術與劇情戰役玩法。
/// </summary>
public enum ObjectiveKind
{
    /// <summary>全殲敵軍：消滅指定敵對陣營的所有部隊或主要軍事實體。</summary>
    EliminateAllEnemies,

    /// <summary>堅守陣地倒數計時：守護關鍵物件（基地、陣地、橋樑），在指定秒數內存活。</summary>
    Survival,

    /// <summary>佔領特定區域：派遣部隊進駐目標矩形或圓形區域，並維持佔領指定秒數。</summary>
    CaptureArea,

    /// <summary>山丘之王：爭奪戰略高地或樞紐，累計佔領維持時長率先達標者勝。</summary>
    KingOfTheHill,

    /// <summary>刺殺敵方英雄：擊殺敵對特定關鍵指揮官或英雄單位。</summary>
    AssassinateTarget,

    /// <summary>破壞關鍵建築：摧毀指定要塞、神廟、奇觀或軍事哨所。</summary>
    DestroyBuilding,

    /// <summary>護送商隊/VIP：引導或保護重要人物/商隊安全抵達目的區域，全程不可陣亡。</summary>
    EscortUnit,

    /// <summary>自訂腳本條件：由多重原生物件狀態與位置條件所構成的複合目標。</summary>
    CustomScripted
}

/// <summary>
/// 目標類別屬性：區分主線、獎勵與失敗判據。
/// </summary>
public enum ObjectiveCategory
{
    /// <summary>主要目標：戰役獲勝所必須達成的核心主線目標。</summary>
    Primary,

    /// <summary>次要/獎勵目標（Bonus）：可選任務，達成可獲得援軍、資源或戰術加成，失敗不影響主線勝利。</summary>
    Bonus,

    /// <summary>關鍵失敗判據：若此條件滿足（例如守護對象死亡、老家被平），立即判定戰役失敗。</summary>
    FailureCriterion
}

/// <summary>
/// 戰役目標的即時運作狀態機。
/// </summary>
public enum ObjectiveState
{
    /// <summary>隱藏未揭露：玩家在任務日誌中看不見（或標記為「???」），等待前置劇情觸發揭曉。</summary>
    Hidden,

    /// <summary>未解鎖/等待中：已列在日誌中但尚未激活，等待前置目標達成後解鎖。</summary>
    Inactive,

    /// <summary>進行中：目標已激活，計時器與條件監控正即時運行中。</summary>
    Active,

    /// <summary>已達成：玩家已順利完成該目標。</summary>
    Completed,

    /// <summary>已失敗：該目標未達成（例如被護送對象死亡或逾時）。</summary>
    Failed,

    /// <summary>已捨棄：因互斥分支路線（Mutually Exclusive）被選擇而失效的旁支目標。</summary>
    Abandoned
}

/// <summary>
/// 目標依賴關係類型。
/// </summary>
public enum DependencyRelation
{
    /// <summary>前置解鎖依賴：前置目標達成後，目標從 Inactive 解鎖為 Active。</summary>
    Prerequisite,

    /// <summary>複合與（Composite AND）：所有子目標皆達成後，父目標自動達成。</summary>
    CompositeAnd,

    /// <summary>複合或（Composite OR）：任一子目標達成後，父目標即達成。</summary>
    CompositeOr,

    /// <summary>互斥分支（Mutually Exclusive）：來源目標一旦達成，目標將被標記為 Abandoned。</summary>
    MutuallyExclusive
}

/// <summary>
/// 整體戰役狀態評估結果。
/// </summary>
public enum CampaignResult
{
    /// <summary>戰役持續進行中。</summary>
    InProgress,

    /// <summary>主線目標全數達成，戰役勝利！</summary>
    Victory,

    /// <summary>觸發失敗條件或關鍵主線失敗，戰役失敗！</summary>
    Defeat
}

/// <summary>
/// 區域邊界定義：支援矩形世界坐標與圓形範圍定義。
/// </summary>
public sealed record ObjectiveAreaBounds(
    int MinX = 0,
    int MinZ = 0,
    int MaxX = 16383,
    int MaxZ = 16383,
    float CenterX = 8192,
    float CenterZ = 8192,
    float Radius = 0,
    bool IsCircle = false)
{
    /// <summary>建立標準矩形區域。</summary>
    public static ObjectiveAreaBounds FromRectangle(int minX, int minZ, int maxX, int maxZ) =>
        new(Math.Min(minX, maxX), Math.Min(minZ, maxZ), Math.Max(minX, maxX), Math.Max(minZ, maxZ),
            (minX + maxX) * 0.5f, (minZ + maxZ) * 0.5f, 0, false);

    /// <summary>建立圓形半徑區域（內部自動計算對應的外接矩形供原生引擎比對）。</summary>
    public static ObjectiveAreaBounds FromCircle(float centerX, float centerZ, float radius)
    {
        int minX = (int)Math.Clamp(centerX - radius, 0, 16383);
        int maxX = (int)Math.Clamp(centerX + radius, 0, 16383);
        int minZ = (int)Math.Clamp(centerZ - radius, 0, 16383);
        int maxZ = (int)Math.Clamp(centerZ + radius, 0, 16383);
        return new(minX, minZ, maxX, maxZ, centerX, centerZ, radius, true);
    }

    /// <summary>檢查指定坐標是否位於區域內。</summary>
    public bool Contains(float x, float z)
    {
        if (x < MinX || x > MaxX || z < MinZ || z > MaxZ) return false;
        if (!IsCircle || Radius <= 0) return true;
        float dx = x - CenterX;
        float dz = z - CenterZ;
        return (dx * dx + dz * dz) <= (Radius * Radius);
    }
}

/// <summary>
/// 目標完成時發放的自訂獎勵或動作序列。
/// </summary>
public sealed record ObjectiveReward(
    string CompletionMessage = "",
    IReadOnlyList<ScenarioAction>? ExtraActions = null)
{
    public IReadOnlyList<ScenarioAction> Actions => ExtraActions ?? Array.Empty<ScenarioAction>();
}

/// <summary>
/// 戰役目標規則參數模型。
/// </summary>
public sealed record ObjectiveRuleParameters
{
    /// <summary>目標物件 GUID 清單（英雄、建築、VIP、守護標的）。</summary>
    public IReadOnlyList<Guid> TargetGuids { get; init; } = Array.Empty<Guid>();

    /// <summary>防守或維持佔領秒數（例如 Survival 需撐過 180 秒，或佔領需維持 30 秒）。</summary>
    public int HoldDurationSeconds { get; init; } = 0;

    /// <summary>目的區域或佔領區域範圍。</summary>
    public ObjectiveAreaBounds? Area { get; init; }

    /// <summary>玩家所屬隊伍編號（0–7，預設 0）。</summary>
    public int PlayerTeam { get; init; } = 0;

    /// <summary>敵方或目標所屬隊伍編號（0–7，預設 1）。</summary>
    public int TargetTeam { get; init; } = 1;

    /// <summary>時限秒數（0 代表無限制，逾時將導致目標判定失敗）。</summary>
    public int TimeLimitSeconds { get; init; } = 0;

    /// <summary>需求數量（例如需消滅之小隊數、或佔領點數）。</summary>
    public int RequiredCount { get; init; } = 1;

    /// <summary>自訂擴充原生條件組。</summary>
    public IReadOnlyList<ScenarioCondition> CustomConditions { get; init; } = Array.Empty<ScenarioCondition>();

    /// <summary>擴充自訂資料（如自訂標籤或腳本變數名稱）。</summary>
    public string CustomData { get; init; } = "";
}

/// <summary>
/// 目標依賴連線定義。
/// </summary>
public sealed record ObjectiveDependency(
    Guid SourceObjectiveId,
    Guid TargetObjectiveId,
    DependencyRelation Relation = DependencyRelation.Prerequisite);

/// <summary>
/// 戰役目標核心定義模型。
/// </summary>
public sealed record ObjectiveDefinition
{
    public Guid Id { get; init; } = Guid.NewGuid();
    public string Title { get; init; } = "New Objective";
    public string Description { get; init; } = "";
    public ObjectiveKind Kind { get; init; } = ObjectiveKind.EliminateAllEnemies;
    public ObjectiveCategory Category { get; init; } = ObjectiveCategory.Primary;
    public ObjectiveRuleParameters Parameters { get; init; } = new();
    public ObjectiveReward? Reward { get; init; }
    public ObjectiveState InitialState { get; init; } = ObjectiveState.Active;
    public float UiOrder { get; init; } = 0f;
}
