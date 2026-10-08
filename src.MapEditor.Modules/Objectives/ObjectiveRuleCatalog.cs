namespace AgainstRomeMapEditor.Modules.Objectives;


/// <summary>
/// 規則驗證結果。
/// </summary>
public sealed record ObjectiveValidationResult(bool IsValid, IReadOnlyList<string> Errors)
{
    public static ObjectiveValidationResult Success => new(true, Array.Empty<string>());
    public static ObjectiveValidationResult Failure(params string[] errors) => new(false, errors);
    public static ObjectiveValidationResult Failure(IEnumerable<string> errors) => new(false, errors.ToArray());
}

/// <summary>
/// 目標規則類型描述元。
/// </summary>
public sealed record ObjectiveRuleDescriptor(
    ObjectiveKind Kind,
    string DisplayName,
    string Description,
    bool RequiresTargetGuid,
    bool RequiresArea,
    bool SupportsHoldDuration,
    bool SupportsTimeLimit,
    ObjectiveCategory DefaultCategory);

/// <summary>
/// 戰役目標規則型錄（ObjectiveRuleCatalog）：
/// 定義各種勝利與失敗條件模組之規格、參數驗證邏輯與標準範本工廠。
/// </summary>
public static class ObjectiveRuleCatalog
{
    private static readonly Dictionary<ObjectiveKind, ObjectiveRuleDescriptor> Descriptors = new()
    {
        [ObjectiveKind.EliminateAllEnemies] = new(
            ObjectiveKind.EliminateAllEnemies,
            "全殲敵軍 (Eliminate Enemies)",
            "消滅指定敵方勢力的所有部隊或摧毀其主要軍事指揮中心。",
            RequiresTargetGuid: false,
            RequiresArea: false,
            SupportsHoldDuration: false,
            SupportsTimeLimit: true,
            DefaultCategory: ObjectiveCategory.Primary),

        [ObjectiveKind.Survival] = new(
            ObjectiveKind.Survival,
            "堅守陣地倒數計時 (Survival)",
            "守護關鍵基地或指揮官，在敵軍猛烈圍攻下堅守至計時歸零。",
            RequiresTargetGuid: true,
            RequiresArea: false,
            SupportsHoldDuration: true,
            SupportsTimeLimit: false,
            DefaultCategory: ObjectiveCategory.Primary),

        [ObjectiveKind.CaptureArea] = new(
            ObjectiveKind.CaptureArea,
            "佔領特定區域 (Capture Area)",
            "匯出僅支援指定物件位於矩形區域（維持秒數為 0）；不檢查敵軍或隊伍佔領。",
            RequiresTargetGuid: false,
            RequiresArea: true,
            SupportsHoldDuration: true,
            SupportsTimeLimit: true,
            DefaultCategory: ObjectiveCategory.Primary),

        [ObjectiveKind.KingOfTheHill] = new(
            ObjectiveKind.KingOfTheHill,
            "山丘之王 (King of the Hill)",
            "爭奪地圖中央樞紐據點，在限定時間內累計佔領時長率先達成者獲勝。",
            RequiresTargetGuid: false,
            RequiresArea: true,
            SupportsHoldDuration: true,
            SupportsTimeLimit: true,
            DefaultCategory: ObjectiveCategory.Primary),

        [ObjectiveKind.AssassinateTarget] = new(
            ObjectiveKind.AssassinateTarget,
            "刺殺敵方英雄 (Assassinate Hero)",
            "擊殺敵軍特定將領、羅馬百夫長、祭司或酋長單位。",
            RequiresTargetGuid: true,
            RequiresArea: false,
            SupportsHoldDuration: false,
            SupportsTimeLimit: true,
            DefaultCategory: ObjectiveCategory.Primary),

        [ObjectiveKind.DestroyBuilding] = new(
            ObjectiveKind.DestroyBuilding,
            "破壞關鍵建築 (Destroy Building)",
            "摧毀敵軍關鍵軍事建築、神廟要塞或補給倉庫。",
            RequiresTargetGuid: true,
            RequiresArea: false,
            SupportsHoldDuration: false,
            SupportsTimeLimit: true,
            DefaultCategory: ObjectiveCategory.Primary),

        [ObjectiveKind.EscortUnit] = new(
            ObjectiveKind.EscortUnit,
            "護送商隊 (Escort Caravan)",
            "護送商隊或重要盟軍 VIP 單位安全抵達撤離區域，期間目標不可陣亡。",
            RequiresTargetGuid: true,
            RequiresArea: true,
            SupportsHoldDuration: false,
            SupportsTimeLimit: true,
            DefaultCategory: ObjectiveCategory.Primary),

        [ObjectiveKind.CustomScripted] = new(
            ObjectiveKind.CustomScripted,
            "自訂複合腳本 (Custom Scripted)",
            "自由組合原生物件存在、陣亡或區域條件之高階目標。",
            RequiresTargetGuid: false,
            RequiresArea: false,
            SupportsHoldDuration: true,
            SupportsTimeLimit: true,
            DefaultCategory: ObjectiveCategory.Bonus)
    };

    /// <summary>所有設計／模擬規則；匯出須另通過 ValidateForCompilation。</summary>
    public static IReadOnlyCollection<ObjectiveRuleDescriptor> AllDescriptors => Descriptors.Values;

    /// <summary>依目標種類取得描述元。</summary>
    public static ObjectiveRuleDescriptor GetDescriptor(ObjectiveKind kind)
    {
        if (Descriptors.TryGetValue(kind, out var descriptor)) return descriptor;
        throw new NotSupportedException($"未知的目標規則種類：{kind}");
    }

    /// <summary>驗證目標定義之參數完整性與邏輯有效性。</summary>
    public static ObjectiveValidationResult Validate(ObjectiveDefinition definition)
    {
        ArgumentNullException.ThrowIfNull(definition);
        var errors = new List<string>();

        if (string.IsNullOrWhiteSpace(definition.Title))
            errors.Add("目標標題不可為空。");

        if (definition.Title.Length > 120)
            errors.Add("目標標題長度上限為 120 字元。");

        if (!Descriptors.TryGetValue(definition.Kind, out var descriptor))
            return ObjectiveValidationResult.Failure("Unknown objective kind.");
        var param = definition.Parameters;

        if (param.PlayerTeam is < 0 or > 7)
            errors.Add("玩家隊伍編號必須介於 0–7 之間。");

        if (param.TargetTeam is < 0 or > 7)
            errors.Add("目標隊伍編號必須介於 0–7 之間。");

        if (descriptor.RequiresTargetGuid)
        {
            if (param.TargetGuids.Count == 0 || param.TargetGuids.Any(g => g == Guid.Empty))
                errors.Add($"規則「{descriptor.DisplayName}」需要至少一個有效的目標物件 GUID。");
        }

        if (descriptor.RequiresArea)
        {
            if (param.Area is null)
            {
                errors.Add($"規則「{descriptor.DisplayName}」必須設定有效的目標區域範圍。");
            }
            else
            {
                if (param.Area.MinX < 0 || param.Area.MinZ < 0 || param.Area.MaxX > 16383 || param.Area.MaxZ > 16383 ||
                    param.Area.MinX > param.Area.MaxX || param.Area.MinZ > param.Area.MaxZ)
                {
                    errors.Add("區域邊界必須介於世界座標 0–16383 且 Min 必須不大於 Max。");
                }
            }
        }

        if (descriptor.SupportsHoldDuration && param.HoldDurationSeconds < 0)
            errors.Add("維持秒數不可為負數。");

        if (descriptor.SupportsTimeLimit && param.TimeLimitSeconds < 0)
            errors.Add("時限秒數不可為負數。");

        if (definition.Kind == ObjectiveKind.Survival && definition.Category != ObjectiveCategory.FailureCriterion && param.HoldDurationSeconds <= 0)
            errors.Add("堅守陣地目標（Survival）必須指定大於 0 的堅守秒數。");

        if (definition.Kind == ObjectiveKind.CaptureArea && param.HoldDurationSeconds < 0)
            errors.Add("佔領目標維持秒數不可為負數。");

        return errors.Count == 0 ? ObjectiveValidationResult.Success : ObjectiveValidationResult.Failure(errors);
    }

    /// <summary>Export eligibility, separate from the offline graph/sandbox parameter validation.</summary>
    public static ObjectiveValidationResult ValidateForCompilation(ObjectiveDefinition definition)
    {
        ArgumentNullException.ThrowIfNull(definition);
        var errors = new List<string>();
        var p = definition.Parameters;
        if (!Enum.IsDefined(definition.Kind) || !Enum.IsDefined(definition.Category))
            errors.Add("Unknown objective kind or category.");
        if (definition.Id == Guid.Empty || definition.InitialState != ObjectiveState.Active)
            errors.Add("Export requires a persistent ID and an initially active objective.");
        if (definition.Kind is ObjectiveKind.EliminateAllEnemies or ObjectiveKind.KingOfTheHill)
            errors.Add("Team-wide elimination and accumulated area control have no verified ScenarioEvent mapping.");
        if (p.TimeLimitSeconds != 0 || p.RequiredCount != 1 || !string.IsNullOrEmpty(p.CustomData))
            errors.Add("Timeouts, kill/resource counts and custom data are simulation-only and cannot be silently ignored.");
        if (p.Area?.IsCircle == true)
            errors.Add("Only rectangular areas are supported; circles cannot be approximated by their bounding box.");
        if (definition.Category == ObjectiveCategory.FailureCriterion && definition.Kind != ObjectiveKind.Survival)
            errors.Add("Only protected-object loss is supported as a failure criterion.");
        if (definition.Kind is ObjectiveKind.Survival or ObjectiveKind.EscortUnit && definition.Category == ObjectiveCategory.Bonus)
            errors.Add("Bonus survival/escort needs a failed-state latch to suppress later rewards; unsupported.");
        if (definition.Kind is ObjectiveKind.Survival or ObjectiveKind.EscortUnit or ObjectiveKind.CaptureArea)
        {
            if (p.TargetGuids.Count != 1 || p.TargetGuids.Any(id => id == Guid.Empty))
                errors.Add("This rule requires exactly one explicit target GUID.");
        }
        if (p.TargetGuids.Count > 32 || p.TargetGuids.Any(id => id == Guid.Empty) || p.TargetGuids.Distinct().Count() != p.TargetGuids.Count)
            errors.Add("Target lists must contain at most 32 distinct, nonempty GUIDs; no truncation is allowed.");
        if (definition.Kind != ObjectiveKind.Survival && p.HoldDurationSeconds != 0)
            errors.Add("Area dwell/accumulation and custom hold timers are unsupported; DelaySeconds is a level-start deadline, not time spent satisfying a condition.");
        if (p.HoldDurationSeconds > 86400)
            errors.Add("Survival duration cannot exceed the existing compiler limit of 86400 seconds.");
        if (definition.Category == ObjectiveCategory.FailureCriterion && (p.HoldDurationSeconds != 0 || definition.Reward is not null))
            errors.Add("A protected-object failure criterion cannot have a completion timer or reward.");
        if (definition.Kind == ObjectiveKind.CustomScripted)
        {
            if (p.CustomConditions.Count is < 1 or > 32 || p.TargetGuids.Count != 0 || p.Area is not null)
                errors.Add("Custom objectives require 1-32 explicit ScenarioConditions and no unused targets/area.");
        }
        else if (p.CustomConditions.Count != 0)
            errors.Add("Custom conditions are only supported by CustomScripted objectives.");
        if (p.Area is not null && definition.Kind is not (ObjectiveKind.EscortUnit or ObjectiveKind.CaptureArea))
            errors.Add("This objective kind does not use an area.");
        if (definition.Reward?.Actions.Any(a => a is null || a.Kind is AgainstRomeModifier.Scripting.ScenarioActionKind.Victory or AgainstRomeModifier.Scripting.ScenarioActionKind.Defeat) == true)
            errors.Add("Reward actions cannot terminate the mission; terminal results are owned by the objective compiler.");
        return errors.Count == 0 ? ObjectiveValidationResult.Success : ObjectiveValidationResult.Failure(errors);
    }

    #region Standard Template Factories

    /// <summary>建立「全殲敵軍」標準目標。</summary>
    public static ObjectiveDefinition CreateEliminateEnemies(
        string title = "消滅敵方殘存勢力",
        string description = "全殲地圖上的所有敵軍作戰部隊。",
        int targetTeam = 1,
        int playerTeam = 0,
        int timeLimitSeconds = 0,
        ObjectiveCategory category = ObjectiveCategory.Primary,
        ObjectiveReward? reward = null) =>
        new()
        {
            Title = title,
            Description = description,
            Kind = ObjectiveKind.EliminateAllEnemies,
            Category = category,
            Reward = reward,
            Parameters = new()
            {
                PlayerTeam = playerTeam,
                TargetTeam = targetTeam,
                TimeLimitSeconds = timeLimitSeconds
            }
        };

    /// <summary>建立「堅守陣地倒數計時 (Survival)」標準目標。</summary>
    public static ObjectiveDefinition CreateSurvival(
        string title,
        string description,
        Guid defendTargetGuid,
        int holdDurationSeconds,
        int playerTeam = 0,
        int targetTeam = 1,
        ObjectiveCategory category = ObjectiveCategory.Primary,
        ObjectiveReward? reward = null) =>
        new()
        {
            Title = title,
            Description = description,
            Kind = ObjectiveKind.Survival,
            Category = category,
            Reward = reward,
            Parameters = new()
            {
                TargetGuids = [defendTargetGuid],
                HoldDurationSeconds = holdDurationSeconds,
                PlayerTeam = playerTeam,
                TargetTeam = targetTeam
            }
        };

    /// <summary>建立「佔領特定區域 (Capture Area)」標準目標。</summary>
    public static ObjectiveDefinition CreateCaptureArea(
        string title,
        string description,
        ObjectiveAreaBounds area,
        int holdDurationSeconds = 0,
        int playerTeam = 0,
        int timeLimitSeconds = 0,
        ObjectiveCategory category = ObjectiveCategory.Primary,
        ObjectiveReward? reward = null,
        Guid? targetGuid = null) =>
        new()
        {
            Title = title,
            Description = description,
            Kind = ObjectiveKind.CaptureArea,
            Category = category,
            Reward = reward,
            Parameters = new()
            {
                TargetGuids = targetGuid.HasValue ? [targetGuid.Value] : [],
                Area = area,
                HoldDurationSeconds = holdDurationSeconds,
                PlayerTeam = playerTeam,
                TimeLimitSeconds = timeLimitSeconds
            }
        };

    /// <summary>建立「刺殺敵方英雄」標準目標。</summary>
    public static ObjectiveDefinition CreateAssassinateTarget(
        string title,
        string description,
        Guid targetHeroGuid,
        int targetTeam = 1,
        int playerTeam = 0,
        int timeLimitSeconds = 0,
        ObjectiveCategory category = ObjectiveCategory.Primary,
        ObjectiveReward? reward = null) =>
        new()
        {
            Title = title,
            Description = description,
            Kind = ObjectiveKind.AssassinateTarget,
            Category = category,
            Reward = reward,
            Parameters = new()
            {
                TargetGuids = [targetHeroGuid],
                TargetTeam = targetTeam,
                PlayerTeam = playerTeam,
                TimeLimitSeconds = timeLimitSeconds
            }
        };

    /// <summary>建立「破壞關鍵建築」標準目標。</summary>
    public static ObjectiveDefinition CreateDestroyBuilding(
        string title,
        string description,
        Guid buildingGuid,
        int targetTeam = 1,
        int playerTeam = 0,
        int timeLimitSeconds = 0,
        ObjectiveCategory category = ObjectiveCategory.Primary,
        ObjectiveReward? reward = null) =>
        new()
        {
            Title = title,
            Description = description,
            Kind = ObjectiveKind.DestroyBuilding,
            Category = category,
            Reward = reward,
            Parameters = new()
            {
                TargetGuids = [buildingGuid],
                TargetTeam = targetTeam,
                PlayerTeam = playerTeam,
                TimeLimitSeconds = timeLimitSeconds
            }
        };

    /// <summary>建立「護送商隊/VIP」標準目標。</summary>
    public static ObjectiveDefinition CreateEscortUnit(
        string title,
        string description,
        Guid vipGuid,
        ObjectiveAreaBounds destArea,
        int timeLimitSeconds = 0,
        int playerTeam = 0,
        ObjectiveCategory category = ObjectiveCategory.Primary,
        ObjectiveReward? reward = null) =>
        new()
        {
            Title = title,
            Description = description,
            Kind = ObjectiveKind.EscortUnit,
            Category = category,
            Reward = reward,
            Parameters = new()
            {
                TargetGuids = [vipGuid],
                Area = destArea,
                TimeLimitSeconds = timeLimitSeconds,
                PlayerTeam = playerTeam
            }
        };

    /// <summary>建立「重要關鍵資產防守失敗判據 (Failure Criterion)」目標。</summary>
    public static ObjectiveDefinition CreateDefendFailureCriterion(
        string title,
        string description,
        Guid criticalAssetGuid,
        int playerTeam = 0) =>
        new()
        {
            Title = title,
            Description = description,
            Kind = ObjectiveKind.Survival,
            Category = ObjectiveCategory.FailureCriterion,
            Parameters = new()
            {
                TargetGuids = [criticalAssetGuid],
                PlayerTeam = playerTeam
            }
        };

    #endregion
}
