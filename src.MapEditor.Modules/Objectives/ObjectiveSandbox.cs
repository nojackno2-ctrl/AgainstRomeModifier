namespace AgainstRomeMapEditor.Modules.Objectives;

/// <summary>
/// 目標狀態變更事件引數。
/// </summary>
public sealed record ObjectiveStateChangedEventArgs(
    ObjectiveDefinition Objective,
    ObjectiveState PreviousState,
    ObjectiveState NewState,
    string Reason);

/// <summary>
/// 單一目標即時進度快照。
/// </summary>
public sealed record ObjectiveProgressSnapshot(
    Guid ObjectiveId,
    string Title,
    ObjectiveKind Kind,
    ObjectiveCategory Category,
    ObjectiveState State,
    float ProgressPercentage,
    float ElapsedSeconds,
    float RemainingTimeLimitSeconds,
    float CurrentHoldSeconds,
    float TargetHoldSeconds);

/// <summary>
/// 沙盒整體運行狀態快照。
/// </summary>
public sealed record ObjectiveSandboxSnapshot(
    CampaignResult CampaignStatus,
    float TotalSimulatedSeconds,
    IReadOnlyList<ObjectiveProgressSnapshot> ObjectiveSnapshots,
    IReadOnlyList<string> MessageLog);

/// <summary>
/// 編輯器目標進度即時測試沙盒合約（IObjectiveSandboxSession）：
/// 允許地圖作者在不啟動遊戲的情況下，於編輯器內部即時模擬時間流逝、
/// 物件陣亡、部隊移動與佔領狀態變遷，秒級驗證戰役目標鏈與勝敗判定。
/// </summary>
public interface IObjectiveSandboxSession
{
    /// <summary>當前整體戰役狀態。</summary>
    CampaignResult CampaignStatus { get; }

    /// <summary>累計模擬秒數。</summary>
    float TotalSimulatedSeconds { get; }

    /// <summary>初始化或重新載入目標依賴圖。</summary>
    void Initialize(ObjectiveDependencyGraph graph);

    /// <summary>向前推進模擬時間（秒）。</summary>
    void Tick(float deltaSeconds);

    /// <summary>模擬特定地圖物件被消滅/死亡（例如英雄陣亡、建築被毀）。</summary>
    void SimulateObjectDeath(Guid targetId);

    /// <summary>模擬特定物件移動至指定世界坐標（例如商隊抵達、先鋒部隊進入佔領區）。</summary>
    void SimulateObjectMove(Guid targetId, float worldX, float worldZ);

    /// <summary>模擬陣營存活部隊數量變更（用於全殲敵軍目標）。</summary>
    void SimulateTeamUnitCount(int team, int remainingCount);

    /// <summary>取得特定目標當前進度詳情。</summary>
    ObjectiveProgressSnapshot? GetProgress(Guid objectiveId);

    /// <summary>擷取當前沙盒整體運行快照。</summary>
    ObjectiveSandboxSnapshot CaptureSnapshot();

    /// <summary>重設沙盒回到開局初始狀態。</summary>
    void Reset();

    /// <summary>目標狀態變更事件。</summary>
    event EventHandler<ObjectiveStateChangedEventArgs>? StateChanged;

    /// <summary>戰役獲勝或失敗事件。</summary>
    event EventHandler<CampaignResult>? CampaignFinished;

    /// <summary>任務訊息廣播事件。</summary>
    event EventHandler<string>? MessageBroadcasted;
}

/// <summary>
/// 編輯器目標進度即時測試沙盒實作（ObjectiveSandboxSession）。
/// </summary>
public sealed class ObjectiveSandboxSession : IObjectiveSandboxSession
{
    private ObjectiveDependencyGraph? _graph;
    private readonly Dictionary<Guid, ObjectiveState> _states = new();
    private readonly Dictionary<Guid, float> _elapsedSeconds = new();
    private readonly Dictionary<Guid, float> _holdSeconds = new();
    private readonly Dictionary<Guid, bool> _deadObjects = new();
    private readonly Dictionary<Guid, (float X, float Z)> _objectPositions = new();
    private readonly Dictionary<int, int> _teamUnitCounts = new();
    private readonly List<string> _messageLog = new();

    public CampaignResult CampaignStatus { get; private set; } = CampaignResult.InProgress;
    public float TotalSimulatedSeconds { get; private set; } = 0f;

    public event EventHandler<ObjectiveStateChangedEventArgs>? StateChanged;
    public event EventHandler<CampaignResult>? CampaignFinished;
    public event EventHandler<string>? MessageBroadcasted;

    public void Initialize(ObjectiveDependencyGraph graph)
    {
        ArgumentNullException.ThrowIfNull(graph);
        _graph = graph;
        Reset();
    }

    public void Reset()
    {
        TotalSimulatedSeconds = 0f;
        CampaignStatus = CampaignResult.InProgress;
        _states.Clear();
        _elapsedSeconds.Clear();
        _holdSeconds.Clear();
        _deadObjects.Clear();
        _objectPositions.Clear();
        _teamUnitCounts.Clear();
        _messageLog.Clear();

        if (_graph is null) return;

        foreach (var obj in _graph.Objectives)
        {
            _states[obj.Id] = obj.InitialState;
            _elapsedSeconds[obj.Id] = 0f;
            _holdSeconds[obj.Id] = 0f;
        }

        // 開局狀態傳遞
        PropagateAndEvaluate();
    }

    public void Tick(float deltaSeconds)
    {
        if (_graph is null || CampaignStatus != CampaignResult.InProgress || deltaSeconds <= 0)
            return;

        TotalSimulatedSeconds += deltaSeconds;

        foreach (var obj in _graph.Objectives)
        {
            if (_states.GetValueOrDefault(obj.Id) != ObjectiveState.Active)
                continue;

            _elapsedSeconds[obj.Id] = _elapsedSeconds.GetValueOrDefault(obj.Id) + deltaSeconds;

            // 1. 時限超時判定（若有設定且非堅守目標）
            if (obj.Parameters.TimeLimitSeconds > 0 &&
                obj.Kind != ObjectiveKind.Survival &&
                _elapsedSeconds[obj.Id] >= obj.Parameters.TimeLimitSeconds)
            {
                TransitionState(obj, ObjectiveState.Failed, "目標逾時未完成。");
                continue;
            }

            // 2. 堅守目標進度（Survival）
            if (obj.Kind == ObjectiveKind.Survival && obj.Category != ObjectiveCategory.FailureCriterion)
            {
                var targetGuid = obj.Parameters.TargetGuids.FirstOrDefault();
                bool isTargetDead = targetGuid != Guid.Empty && _deadObjects.GetValueOrDefault(targetGuid);

                if (isTargetDead)
                {
                    TransitionState(obj, ObjectiveState.Failed, "守護目標已陣亡。");
                    continue;
                }

                _holdSeconds[obj.Id] = _holdSeconds.GetValueOrDefault(obj.Id) + deltaSeconds;
                if (_holdSeconds[obj.Id] >= obj.Parameters.HoldDurationSeconds)
                {
                    TransitionState(obj, ObjectiveState.Completed, $"成功堅守陣地 {obj.Parameters.HoldDurationSeconds} 秒！");
                    continue;
                }
            }

            // 3. 佔領區域維持進度（CaptureArea / KingOfTheHill）
            if (obj.Kind is ObjectiveKind.CaptureArea or ObjectiveKind.KingOfTheHill)
            {
                var targetGuid = obj.Parameters.TargetGuids.FirstOrDefault();
                bool inArea = false;

                if (targetGuid != Guid.Empty && _objectPositions.TryGetValue(targetGuid, out var pos))
                {
                    inArea = obj.Parameters.Area?.Contains(pos.X, pos.Z) ?? false;
                }

                if (inArea)
                {
                    _holdSeconds[obj.Id] = _holdSeconds.GetValueOrDefault(obj.Id) + deltaSeconds;
                    if (_holdSeconds[obj.Id] >= obj.Parameters.HoldDurationSeconds)
                    {
                        TransitionState(obj, ObjectiveState.Completed, $"成功佔領區域維持 {obj.Parameters.HoldDurationSeconds} 秒！");
                    }
                }
            }
        }

        PropagateAndEvaluate();
    }

    public void SimulateObjectDeath(Guid targetId)
    {
        if (_graph is null || targetId == Guid.Empty) return;
        _deadObjects[targetId] = true;

        foreach (var obj in _graph.Objectives)
        {
            if (_states.GetValueOrDefault(obj.Id) != ObjectiveState.Active)
                continue;

            // 1. 刺殺/破壞關鍵目標
            if (obj.Kind is ObjectiveKind.AssassinateTarget or ObjectiveKind.DestroyBuilding)
            {
                if (obj.Parameters.TargetGuids.Contains(targetId))
                {
                    TransitionState(obj, ObjectiveState.Completed, $"關鍵目標 {targetId} 已被殲滅！");
                }
            }

            // 2. 守護資產陣亡 -> 失敗判據觸發
            if (obj.Kind == ObjectiveKind.Survival || obj.Category == ObjectiveCategory.FailureCriterion)
            {
                if (obj.Parameters.TargetGuids.Contains(targetId))
                {
                    TransitionState(obj, ObjectiveState.Failed, $"重要守護目標 {targetId} 已遭摧毀！");
                }
            }

            // 3. VIP 陣亡 -> 護送失敗
            if (obj.Kind == ObjectiveKind.EscortUnit)
            {
                if (obj.Parameters.TargetGuids.Contains(targetId))
                {
                    TransitionState(obj, ObjectiveState.Failed, $"護送對象 {targetId} 途中遇害！");
                }
            }
        }

        PropagateAndEvaluate();
    }

    public void SimulateObjectMove(Guid targetId, float worldX, float worldZ)
    {
        if (_graph is null || targetId == Guid.Empty) return;
        _objectPositions[targetId] = (worldX, worldZ);

        foreach (var obj in _graph.Objectives)
        {
            if (_states.GetValueOrDefault(obj.Id) != ObjectiveState.Active)
                continue;

            // 護送商隊抵達終點區域
            if (obj.Kind == ObjectiveKind.EscortUnit && obj.Parameters.TargetGuids.Contains(targetId))
            {
                if (obj.Parameters.Area?.Contains(worldX, worldZ) == true)
                {
                    TransitionState(obj, ObjectiveState.Completed, $"商隊已平安抵達撤離區域 ({worldX:F0}, {worldZ:F0})！");
                }
            }
        }

        PropagateAndEvaluate();
    }

    public void SimulateTeamUnitCount(int team, int remainingCount)
    {
        if (_graph is null) return;
        _teamUnitCounts[team] = remainingCount;

        foreach (var obj in _graph.Objectives)
        {
            if (_states.GetValueOrDefault(obj.Id) != ObjectiveState.Active)
                continue;

            if (obj.Kind == ObjectiveKind.EliminateAllEnemies && obj.Parameters.TargetTeam == team)
            {
                if (remainingCount <= 0)
                {
                    TransitionState(obj, ObjectiveState.Completed, $"敵軍隊伍 {team} 的所有部隊已被全殲！");
                }
            }
        }

        PropagateAndEvaluate();
    }

    private void TransitionState(ObjectiveDefinition obj, ObjectiveState newState, string reason)
    {
        var oldState = _states.GetValueOrDefault(obj.Id);
        if (oldState == newState) return;

        _states[obj.Id] = newState;
        string logMsg = $"[{TotalSimulatedSeconds:F1}s] 目標「{obj.Title}」狀態：{oldState} -> {newState} ({reason})";
        _messageLog.Add(logMsg);

        StateChanged?.Invoke(this, new ObjectiveStateChangedEventArgs(obj, oldState, newState, reason));
        MessageBroadcasted?.Invoke(this, logMsg);

        // 發放獎勵提示
        if (newState == ObjectiveState.Completed && obj.Reward is not null && !string.IsNullOrWhiteSpace(obj.Reward.CompletionMessage))
        {
            string rewardMsg = $"[獎勵] {obj.Reward.CompletionMessage}";
            _messageLog.Add(rewardMsg);
            MessageBroadcasted?.Invoke(this, rewardMsg);
        }
    }

    private void PropagateAndEvaluate()
    {
        if (_graph is null) return;

        // 動態傳遞狀態
        var updated = _graph.PropagateStates(_states);
        foreach (var (id, state) in updated)
        {
            var oldState = _states.GetValueOrDefault(id);
            if (oldState != state)
            {
                _states[id] = state;
                var obj = _graph.FindObjective(id);
                if (obj is not null)
                {
                    StateChanged?.Invoke(this, new ObjectiveStateChangedEventArgs(obj, oldState, state, "依賴圖動態解鎖/狀態傳遞"));
                }
            }
        }

        // 評估戰役勝負
        var result = _graph.EvaluateCampaignResult(_states);
        if (result != CampaignStatus)
        {
            CampaignStatus = result;
            string finalMsg = result switch
            {
                CampaignResult.Victory => $"戰役全勝！所有主要戰役目標均已順利達成！(耗時 {TotalSimulatedSeconds:F1}s)",
                CampaignResult.Defeat => $"戰役失敗！未能保衛關鍵目標或主線任務失敗。(耗時 {TotalSimulatedSeconds:F1}s)",
                _ => ""
            };
            if (!string.IsNullOrEmpty(finalMsg))
            {
                _messageLog.Add(finalMsg);
                MessageBroadcasted?.Invoke(this, finalMsg);
            }
            CampaignFinished?.Invoke(this, result);
        }
    }

    public ObjectiveProgressSnapshot? GetProgress(Guid objectiveId)
    {
        if (_graph is null) return null;
        var obj = _graph.FindObjective(objectiveId);
        if (obj is null) return null;

        var state = _states.GetValueOrDefault(objectiveId, obj.InitialState);
        float elapsed = _elapsedSeconds.GetValueOrDefault(objectiveId, 0f);
        float hold = _holdSeconds.GetValueOrDefault(objectiveId, 0f);
        float targetHold = obj.Parameters.HoldDurationSeconds;
        float remainingLimit = obj.Parameters.TimeLimitSeconds > 0
            ? Math.Max(0, obj.Parameters.TimeLimitSeconds - elapsed)
            : 0f;

        float percentage = state switch
        {
            ObjectiveState.Completed => 100f,
            ObjectiveState.Failed => 0f,
            ObjectiveState.Active => targetHold > 0 ? Math.Clamp(hold / targetHold * 100f, 0f, 99f) : 50f,
            _ => 0f
        };

        return new ObjectiveProgressSnapshot(
            ObjectiveId: obj.Id,
            Title: obj.Title,
            Kind: obj.Kind,
            Category: obj.Category,
            State: state,
            ProgressPercentage: percentage,
            ElapsedSeconds: elapsed,
            RemainingTimeLimitSeconds: remainingLimit,
            CurrentHoldSeconds: hold,
            TargetHoldSeconds: targetHold);
    }

    public ObjectiveSandboxSnapshot CaptureSnapshot()
    {
        var progressList = _graph?.Objectives
            .Select(o => GetProgress(o.Id)!)
            .Where(p => p is not null)
            .ToList() ?? new List<ObjectiveProgressSnapshot>();

        return new ObjectiveSandboxSnapshot(
            CampaignStatus: CampaignStatus,
            TotalSimulatedSeconds: TotalSimulatedSeconds,
            ObjectiveSnapshots: progressList,
            MessageLog: _messageLog.ToList());
    }
}
