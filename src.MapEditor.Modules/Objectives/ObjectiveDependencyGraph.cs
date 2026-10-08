namespace AgainstRomeMapEditor.Modules.Objectives;

/// <summary>
/// 目標依賴圖診斷嚴重性。
/// </summary>
public enum ObjectiveDiagnosticSeverity
{
    Info,
    Warning,
    Error
}

/// <summary>
/// 目標依賴圖診斷訊息。
/// </summary>
public sealed record ObjectiveDiagnostic(
    ObjectiveDiagnosticSeverity Severity,
    string Code,
    string Message,
    Guid? ObjectiveId = null);

/// <summary>
/// 階層式戰役目標依賴圖（ObjectiveDependencyGraph）：
/// 管理主線目標（Primary）、次要目標（Bonus）、失敗判據（FailureCriterion）之拓撲圖結構，
/// 支援前置依賴解鎖、複合邏輯判定、互斥分支路線與狀態機動態傳遞。
/// </summary>
public sealed class ObjectiveDependencyGraph
{
    private readonly Dictionary<Guid, ObjectiveDefinition> _objectives = new();
    private readonly List<ObjectiveDependency> _dependencies = new();

    public IReadOnlyCollection<ObjectiveDefinition> Objectives => _objectives.Values;
    public IReadOnlyList<ObjectiveDependency> Dependencies => _dependencies;

    /// <summary>新增目標節點。</summary>
    public void AddObjective(ObjectiveDefinition objective)
    {
        ArgumentNullException.ThrowIfNull(objective);
        if (_objectives.ContainsKey(objective.Id))
            throw new InvalidOperationException($"目標 ID {objective.Id} 已存在於圖中。");
        _objectives[objective.Id] = objective;
    }

    /// <summary>移除目標節點並同步清理所有相關依賴連線。</summary>
    public bool RemoveObjective(Guid objectiveId)
    {
        if (!_objectives.Remove(objectiveId)) return false;
        _dependencies.RemoveAll(d => d.SourceObjectiveId == objectiveId || d.TargetObjectiveId == objectiveId);
        return true;
    }

    /// <summary>尋找目標定義。</summary>
    public ObjectiveDefinition? FindObjective(Guid objectiveId) =>
        _objectives.GetValueOrDefault(objectiveId);

    /// <summary>建立兩個目標之間的依賴關係。</summary>
    public bool AddDependency(Guid sourceId, Guid targetId, DependencyRelation relation = DependencyRelation.Prerequisite)
    {
        if (!_objectives.ContainsKey(sourceId) || !_objectives.ContainsKey(targetId))
            return false;
        if (sourceId == targetId)
            return false; // 不允許自身循環

        // 避免重複依賴
        if (_dependencies.Any(d => d.SourceObjectiveId == sourceId && d.TargetObjectiveId == targetId && d.Relation == relation))
            return true;

        _dependencies.Add(new ObjectiveDependency(sourceId, targetId, relation));
        return true;
    }

    /// <summary>移除指定依賴關係。</summary>
    public bool RemoveDependency(Guid sourceId, Guid targetId)
    {
        int count = _dependencies.RemoveAll(d => d.SourceObjectiveId == sourceId && d.TargetObjectiveId == targetId);
        return count > 0;
    }

    /// <summary>取得指定目標的所有直接前置依賴（Prerequisites）。</summary>
    public IReadOnlyList<ObjectiveDefinition> GetPrerequisites(Guid targetId) =>
        _dependencies
            .Where(d => d.TargetObjectiveId == targetId && d.Relation == DependencyRelation.Prerequisite)
            .Select(d => _objectives.GetValueOrDefault(d.SourceObjectiveId))
            .Where(o => o is not null)
            .Cast<ObjectiveDefinition>()
            .ToList();

    /// <summary>取得直接依賴此目標的後續節點（Dependents）。</summary>
    public IReadOnlyList<ObjectiveDefinition> GetDependents(Guid sourceId) =>
        _dependencies
            .Where(d => d.SourceObjectiveId == sourceId && d.Relation == DependencyRelation.Prerequisite)
            .Select(d => _objectives.GetValueOrDefault(d.TargetObjectiveId))
            .Where(o => o is not null)
            .Cast<ObjectiveDefinition>()
            .ToList();

    /// <summary>取得複合節點的所有子節點。</summary>
    public IReadOnlyList<ObjectiveDefinition> GetCompositeChildren(Guid parentId, DependencyRelation relation) =>
        _dependencies
            .Where(d => d.TargetObjectiveId == parentId && d.Relation == relation)
            .Select(d => _objectives.GetValueOrDefault(d.SourceObjectiveId))
            .Where(o => o is not null)
            .Cast<ObjectiveDefinition>()
            .ToList();

    /// <summary>取得與此目標互斥的所有競爭目標。</summary>
    public IReadOnlyList<ObjectiveDefinition> GetMutuallyExclusiveObjectives(Guid objectiveId) =>
        _dependencies
            .Where(d => d.Relation == DependencyRelation.MutuallyExclusive &&
                        (d.SourceObjectiveId == objectiveId || d.TargetObjectiveId == objectiveId))
            .Select(d => d.SourceObjectiveId == objectiveId ? d.TargetObjectiveId : d.SourceObjectiveId)
            .Distinct()
            .Select(id => _objectives.GetValueOrDefault(id))
            .Where(o => o is not null)
            .Cast<ObjectiveDefinition>()
            .ToList();

    #region Graph Algorithms

    /// <summary>
    /// 偵測圖中是否存在循環依賴（Cycle Detection）。
    /// 回傳構成循環之目標 ID 集合，若無循環則回傳空清單。
    /// </summary>
    public IReadOnlyList<IReadOnlyList<Guid>> DetectCycles()
    {
        var cycles = new List<IReadOnlyList<Guid>>();
        var visited = new Dictionary<Guid, int>(); // 0: unvisited, 1: visiting, 2: visited
        var path = new List<Guid>();

        foreach (var id in _objectives.Keys)
        {
            if (visited.GetValueOrDefault(id) == 0)
            {
                Dfs(id);
            }
        }

        void Dfs(Guid current)
        {
            visited[current] = 1;
            path.Add(current);

            var neighbors = _dependencies
                .Where(d => d.SourceObjectiveId == current && d.Relation is DependencyRelation.Prerequisite or DependencyRelation.CompositeAnd or DependencyRelation.CompositeOr)
                .Select(d => d.TargetObjectiveId);

            foreach (var neighbor in neighbors)
            {
                if (visited.GetValueOrDefault(neighbor) == 1)
                {
                    // 發現環路
                    int startIndex = path.IndexOf(neighbor);
                    if (startIndex >= 0)
                    {
                        var cycle = path.Skip(startIndex).ToList();
                        cycle.Add(neighbor);
                        cycles.Add(cycle);
                    }
                }
                else if (visited.GetValueOrDefault(neighbor) == 0)
                {
                    Dfs(neighbor);
                }
            }

            path.RemoveAt(path.Count - 1);
            visited[current] = 2;
        }

        return cycles;
    }

    /// <summary>
    /// 計算目標拓撲排序序列（Topological Order）。
    /// 若存在環路則丟出 InvalidOperationException。
    /// </summary>
    public IReadOnlyList<ObjectiveDefinition> GetTopologicalOrder()
    {
        var cycles = DetectCycles();
        if (cycles.Count > 0)
            throw new InvalidOperationException("目標依賴圖存在循環依賴，無法執行拓撲排序。");

        var inDegree = _objectives.Keys.ToDictionary(k => k, _ => 0);
        foreach (var dep in _dependencies.Where(d => d.Relation is DependencyRelation.Prerequisite or DependencyRelation.CompositeAnd or DependencyRelation.CompositeOr))
        {
            if (inDegree.TryGetValue(dep.TargetObjectiveId, out int currentDegree))
            {
                inDegree[dep.TargetObjectiveId] = currentDegree + 1;
            }
        }

        var queue = new Queue<Guid>(_objectives.Keys.Where(k => inDegree[k] == 0));
        var order = new List<ObjectiveDefinition>();

        while (queue.TryDequeue(out var id))
        {
            if (_objectives.TryGetValue(id, out var obj))
                order.Add(obj);

            foreach (var dep in _dependencies.Where(d => d.SourceObjectiveId == id && d.Relation is DependencyRelation.Prerequisite or DependencyRelation.CompositeAnd or DependencyRelation.CompositeOr))
            {
                if (--inDegree[dep.TargetObjectiveId] == 0)
                {
                    queue.Enqueue(dep.TargetObjectiveId);
                }
            }
        }

        return order;
    }

    /// <summary>
    /// 全面驗證目標圖的健康度與完整性。
    /// </summary>
    public IReadOnlyList<ObjectiveDiagnostic> Validate()
    {
        var diagnostics = new List<ObjectiveDiagnostic>();

        if (_objectives.Count == 0)
        {
            diagnostics.Add(new ObjectiveDiagnostic(ObjectiveDiagnosticSeverity.Warning, "OBJ_EMPTY", "目前戰役目標圖未包含任何目標。"));
            return diagnostics;
        }

        // 1. 個別目標參數驗證
        foreach (var obj in _objectives.Values)
        {
            var result = ObjectiveRuleCatalog.Validate(obj);
            if (!result.IsValid)
            {
                foreach (var err in result.Errors)
                {
                    diagnostics.Add(new ObjectiveDiagnostic(ObjectiveDiagnosticSeverity.Error, "OBJ_PARAM_INVALID", $"[{obj.Title}] {err}", obj.Id));
                }
            }
        }

        // 2. 循環依賴檢查
        var cycles = DetectCycles();
        if (cycles.Count > 0)
        {
            foreach (var cycle in cycles)
            {
                string pathStr = string.Join(" -> ", cycle.Select(id => _objectives.TryGetValue(id, out var o) ? o.Title : id.ToString()));
                diagnostics.Add(new ObjectiveDiagnostic(ObjectiveDiagnosticSeverity.Error, "OBJ_CYCLE_DETECTED", $"偵測到循環依賴路徑：{pathStr}"));
            }
        }

        // 3. 主線目標存在性驗證
        int primaryCount = _objectives.Values.Count(o => o.Category == ObjectiveCategory.Primary);
        if (primaryCount == 0)
        {
            diagnostics.Add(new ObjectiveDiagnostic(ObjectiveDiagnosticSeverity.Error, "OBJ_NO_PRIMARY", "戰役目標圖必須至少包含一個主要主線目標（Primary Objective）。"));
        }

        // 4. 懸空依賴或未引用檢查
        foreach (var dep in _dependencies)
        {
            if (!_objectives.ContainsKey(dep.SourceObjectiveId))
                diagnostics.Add(new ObjectiveDiagnostic(ObjectiveDiagnosticSeverity.Error, "OBJ_DANGLING_DEP", $"依賴來源 ID {dep.SourceObjectiveId} 不存在。"));
            if (!_objectives.ContainsKey(dep.TargetObjectiveId))
                diagnostics.Add(new ObjectiveDiagnostic(ObjectiveDiagnosticSeverity.Error, "OBJ_DANGLING_DEP", $"依賴目標 ID {dep.TargetObjectiveId} 不存在。"));
        }

        return diagnostics;
    }

    #endregion

    #region State Progression Engine

    /// <summary>
    /// 動態狀態轉移計算：
    /// 根據當前各目標狀態，推進解鎖後續 Prerequisite、計算 Composite 節點、並處理互斥分支。
    /// </summary>
    public Dictionary<Guid, ObjectiveState> PropagateStates(IReadOnlyDictionary<Guid, ObjectiveState> currentStates)
    {
        var states = new Dictionary<Guid, ObjectiveState>(currentStates);

        // 確保所有目標皆有初始狀態
        foreach (var obj in _objectives.Values)
        {
            if (!states.ContainsKey(obj.Id))
                states[obj.Id] = obj.InitialState;
        }

        bool changed = true;
        int maxPasses = _objectives.Count * 2 + 5;
        int pass = 0;

        while (changed && pass++ < maxPasses)
        {
            changed = false;

            foreach (var obj in _objectives.Values)
            {
                var curState = states[obj.Id];

                // 1. 處理互斥路線（若任一互斥目標已完成，此目標自動標記為 Abandoned）
                if (curState is ObjectiveState.Active or ObjectiveState.Inactive or ObjectiveState.Hidden)
                {
                    var exclusiveNodes = GetMutuallyExclusiveObjectives(obj.Id);
                    if (exclusiveNodes.Any(other => states.GetValueOrDefault(other.Id) == ObjectiveState.Completed))
                    {
                        states[obj.Id] = ObjectiveState.Abandoned;
                        changed = true;
                        continue;
                    }
                }

                // 2. 處理前置解鎖（Prerequisites -> Inactive / Hidden 轉為 Active）
                if (curState is ObjectiveState.Inactive or ObjectiveState.Hidden)
                {
                    var prereqs = GetPrerequisites(obj.Id);
                    if (prereqs.Count > 0 && prereqs.All(p => states.GetValueOrDefault(p.Id) == ObjectiveState.Completed))
                    {
                        states[obj.Id] = ObjectiveState.Active;
                        changed = true;
                        continue;
                    }
                }

                // 3. 處理複合與（Composite AND 父節點自動達成）
                if (curState == ObjectiveState.Active)
                {
                    var andChildren = GetCompositeChildren(obj.Id, DependencyRelation.CompositeAnd);
                    if (andChildren.Count > 0 && andChildren.All(c => states.GetValueOrDefault(c.Id) == ObjectiveState.Completed))
                    {
                        states[obj.Id] = ObjectiveState.Completed;
                        changed = true;
                        continue;
                    }

                    // 4. 處理複合或（Composite OR 父節點任一達成）
                    var orChildren = GetCompositeChildren(obj.Id, DependencyRelation.CompositeOr);
                    if (orChildren.Count > 0 && orChildren.Any(c => states.GetValueOrDefault(c.Id) == ObjectiveState.Completed))
                    {
                        states[obj.Id] = ObjectiveState.Completed;
                        changed = true;
                        continue;
                    }
                }
            }
        }

        return states;
    }

    /// <summary>
    /// 評估戰役總體勝負狀態。
    /// </summary>
    public CampaignResult EvaluateCampaignResult(IReadOnlyDictionary<Guid, ObjectiveState> states)
    {
        // 1. 檢查關鍵失敗條件（FailureCriterion）
        foreach (var obj in _objectives.Values.Where(o => o.Category == ObjectiveCategory.FailureCriterion))
        {
            if (states.GetValueOrDefault(obj.Id) == ObjectiveState.Failed)
                return CampaignResult.Defeat;
        }

        // 2. 檢查主線目標（Primary）
        var primaryObjectives = _objectives.Values
            .Where(o => o.Category == ObjectiveCategory.Primary && states.GetValueOrDefault(o.Id) != ObjectiveState.Abandoned)
            .ToList();

        if (primaryObjectives.Count == 0)
            return CampaignResult.InProgress;

        // 若任一未被捨棄的主線目標已判定失敗，且無替代路徑，則戰役失敗
        if (primaryObjectives.Any(o => states.GetValueOrDefault(o.Id) == ObjectiveState.Failed))
            return CampaignResult.Defeat;

        // 若所有主線目標皆已 Completed，則戰役勝利！
        if (primaryObjectives.All(o => states.GetValueOrDefault(o.Id) == ObjectiveState.Completed))
            return CampaignResult.Victory;

        return CampaignResult.InProgress;
    }

    #endregion

    public void Clear()
    {
        _dependencies.Clear();
        _objectives.Clear();
    }
}
