namespace AgainstRomeMapEditor.Modules.Objectives;

using System.Text.Json;
using AgainstRomeModifier.Scripting;

/// <summary>
/// BCI 目標編譯結果容器。
/// </summary>
public sealed record BciObjectiveCompilationResult(
    bool Success,
    IReadOnlyList<ScenarioEvent> CompiledEvents,
    IReadOnlyList<ObjectiveDiagnostic> Diagnostics,
    string Summary);

/// <summary>
/// BCI 目標規則編譯器（BciObjectiveCompiler）：
/// 將高階戰役目標依賴圖（ObjectiveDependencyGraph）無損編譯為相容 Against Rome 引擎規格的
/// 原生 ScenarioEvent 序列與 BCI 腳本條件。
/// </summary>
public static class BciObjectiveCompiler
{
    private static readonly JsonSerializerOptions JsonOptions = new() { WriteIndented = true };

    /// <summary>
    /// 將目標依賴圖編譯為符合 Against Rome 規格的 ScenarioEvent 事件序列。
    /// </summary>
    public static BciObjectiveCompilationResult Compile(
        ObjectiveDependencyGraph graph,
        ScenarioDocument? scenario = null)
    {
        ArgumentNullException.ThrowIfNull(graph);

        var diagnostics = graph.Validate().ToList();
        if (diagnostics.Any(d => d.Severity == ObjectiveDiagnosticSeverity.Error))
        {
            return new BciObjectiveCompilationResult(
                Success: false,
                CompiledEvents: Array.Empty<ScenarioEvent>(),
                Diagnostics: diagnostics,
                Summary: "目標依賴圖驗證失敗，存在嚴重錯誤。");
        }

        var events = new List<ScenarioEvent>();
        var topologicalOrder = graph.GetTopologicalOrder();

        // 1. 開局戰役任務通報事件（Briefing Event，t = 2s）
        var initialPrimaries = topologicalOrder
            .Where(o => o.Category == ObjectiveCategory.Primary && o.InitialState == ObjectiveState.Active)
            .ToList();

        if (initialPrimaries.Count > 0)
        {
            var briefingEvent = new ScenarioEvent("OBJ_SYS_Briefing", DelaySeconds: 2, Repeat: false, Enabled: true);
            string primaryTitles = string.Join("、", initialPrimaries.Select(p => $"【{p.Title}】"));
            briefingEvent.Actions.Add(new ScenarioAction(
                ScenarioActionKind.Message,
                Text: $"戰役開始！主要任務：{primaryTitles}"));
            events.Add(briefingEvent);
        }

        // 2. 針對個別目標編譯專屬監控與達成事件
        var primaryCompletionEvents = new List<ScenarioEvent>();

        foreach (var obj in topologicalOrder)
        {
            var objEvents = CompileObjectiveEvents(obj, graph, scenario);
            foreach (var evt in objEvents)
            {
                events.Add(evt);
                if (obj.Category == ObjectiveCategory.Primary && evt.Name.StartsWith($"OBJ_WIN_{obj.Id:N}", StringComparison.Ordinal))
                {
                    primaryCompletionEvents.Add(evt);
                }
            }
        }

        // 3. 戰役終局判定事件整合
        // 若只有單一主線目標，直接在該目標的完成動作後附加 Victory；
        // 若有多個主線目標且無單一最終匯聚點，建立聯合終局判定事件。
        EnsureTerminalVictoryAction(events, graph, primaryCompletionEvents);

        // 4. 事件數量與合規性驗證
        if (events.Count > 256)
        {
            diagnostics.Add(new ObjectiveDiagnostic(
                ObjectiveDiagnosticSeverity.Error,
                "OBJ_EVENT_LIMIT",
                $"編譯產生的事件數量（{events.Count}）超過原生引擎上限 256 個。"));
            return new BciObjectiveCompilationResult(
                Success: false,
                CompiledEvents: Array.Empty<ScenarioEvent>(),
                Diagnostics: diagnostics,
                Summary: "編譯後事件總數超出遊戲上限。");
        }

        string summary = $"成功編譯 {graph.Objectives.Count} 個戰役目標為 {events.Count} 個原生 ScenarioEvent。";
        return new BciObjectiveCompilationResult(
            Success: true,
            CompiledEvents: events,
            Diagnostics: diagnostics,
            Summary: summary);
    }

    private static List<ScenarioEvent> CompileObjectiveEvents(
        ObjectiveDefinition obj,
        ObjectiveDependencyGraph graph,
        ScenarioDocument? scenario)
    {
        var result = new List<ScenarioEvent>();
        string idShort = obj.Id.ToString("N")[..8];

        switch (obj.Kind)
        {
            case ObjectiveKind.AssassinateTarget:
            case ObjectiveKind.DestroyBuilding:
            {
                // 目標死亡判定事件
                var targetGuid = obj.Parameters.TargetGuids.Count > 0 ? obj.Parameters.TargetGuids[0] : Guid.Empty;
                if (targetGuid != Guid.Empty)
                {
                    var evt = new ScenarioEvent($"OBJ_WIN_{obj.Id:N}_{obj.Kind}", DelaySeconds: 1, Repeat: false, Enabled: true);
                    evt.Conditions.Add(new ScenarioCondition(ScenarioConditionKind.ObjectDeadOrRemoved, targetGuid));
                    evt.Actions.Add(new ScenarioAction(ScenarioActionKind.Message, Text: $"目標達成：{obj.Title}！"));
                    AppendRewardsAndUnlocks(evt, obj, graph);
                    result.Add(evt);
                }
                break;
            }

            case ObjectiveKind.Survival:
            {
                var defendTargetGuid = obj.Parameters.TargetGuids.Count > 0 ? obj.Parameters.TargetGuids[0] : Guid.Empty;
                if (obj.Category == ObjectiveCategory.FailureCriterion)
                {
                    // 純失敗判據：守護目標陣亡立即失敗
                    if (defendTargetGuid != Guid.Empty)
                    {
                        var failEvt = new ScenarioEvent($"OBJ_FAIL_{obj.Id:N}_DefendLost", DelaySeconds: 1, Repeat: false, Enabled: true);
                        failEvt.Conditions.Add(new ScenarioCondition(ScenarioConditionKind.ObjectDeadOrRemoved, defendTargetGuid));
                        failEvt.Actions.Add(new ScenarioAction(ScenarioActionKind.Message, Text: $"防守失敗：{obj.Title} 遭摧毀！"));
                        failEvt.Actions.Add(new ScenarioAction(ScenarioActionKind.Defeat));
                        result.Add(failEvt);
                    }
                }
                else
                {
                    // 堅守計時成功事件（時間到達且目標仍然存在）
                    int holdSec = Math.Max(1, obj.Parameters.HoldDurationSeconds);
                    var winEvt = new ScenarioEvent($"OBJ_WIN_{obj.Id:N}_SurvivalSuccess", DelaySeconds: holdSec, Repeat: false, Enabled: true);
                    if (defendTargetGuid != Guid.Empty)
                    {
                        winEvt.Conditions.Add(new ScenarioCondition(ScenarioConditionKind.ObjectExists, defendTargetGuid));
                    }
                    winEvt.Actions.Add(new ScenarioAction(ScenarioActionKind.Message, Text: $"成功堅守陣地：{obj.Title}（維持 {holdSec} 秒）！"));
                    AppendRewardsAndUnlocks(winEvt, obj, graph);
                    result.Add(winEvt);

                    // 堅守失敗伴隨事件
                    if (defendTargetGuid != Guid.Empty)
                    {
                        var failEvt = new ScenarioEvent($"OBJ_FAIL_{obj.Id:N}_DefendLost", DelaySeconds: 1, Repeat: false, Enabled: true);
                        failEvt.Conditions.Add(new ScenarioCondition(ScenarioConditionKind.ObjectDeadOrRemoved, defendTargetGuid));
                        failEvt.Actions.Add(new ScenarioAction(ScenarioActionKind.Message, Text: $"守護目標陣亡，堅守任務失敗！"));
                        if (obj.Category == ObjectiveCategory.Primary)
                        {
                            failEvt.Actions.Add(new ScenarioAction(ScenarioActionKind.Defeat));
                        }
                        result.Add(failEvt);
                    }
                }
                break;
            }

            case ObjectiveKind.EscortUnit:
            {
                var vipGuid = obj.Parameters.TargetGuids.Count > 0 ? obj.Parameters.TargetGuids[0] : Guid.Empty;
                var area = obj.Parameters.Area ?? new ObjectiveAreaBounds();

                // 成功抵達事件
                if (vipGuid != Guid.Empty)
                {
                    var winEvt = new ScenarioEvent($"OBJ_WIN_{obj.Id:N}_EscortArrived", DelaySeconds: 1, Repeat: false, Enabled: true);
                    winEvt.Conditions.Add(new ScenarioCondition(
                        ScenarioConditionKind.ObjectInArea,
                        vipGuid,
                        MinX: area.MinX,
                        MinZ: area.MinZ,
                        MaxX: area.MaxX,
                        MaxZ: area.MaxZ));
                    winEvt.Actions.Add(new ScenarioAction(ScenarioActionKind.Message, Text: $"護送成功：{obj.Title} 已安全抵達！"));
                    AppendRewardsAndUnlocks(winEvt, obj, graph);
                    result.Add(winEvt);

                    // VIP 陣亡失敗事件
                    var failEvt = new ScenarioEvent($"OBJ_FAIL_{obj.Id:N}_VipDead", DelaySeconds: 1, Repeat: false, Enabled: true);
                    failEvt.Conditions.Add(new ScenarioCondition(ScenarioConditionKind.ObjectDeadOrRemoved, vipGuid));
                    failEvt.Actions.Add(new ScenarioAction(ScenarioActionKind.Message, Text: $"護送失敗：關鍵人員於途中陣亡！"));
                    if (obj.Category == ObjectiveCategory.Primary)
                    {
                        failEvt.Actions.Add(new ScenarioAction(ScenarioActionKind.Defeat));
                    }
                    result.Add(failEvt);
                }
                break;
            }

            case ObjectiveKind.CaptureArea:
            case ObjectiveKind.KingOfTheHill:
            {
                var area = obj.Parameters.Area ?? new ObjectiveAreaBounds();
                int holdSec = Math.Max(1, obj.Parameters.HoldDurationSeconds);

                // 若有指定領頭部隊或先鋒部隊
                var vanguardGuid = obj.Parameters.TargetGuids.Count > 0 ? obj.Parameters.TargetGuids[0] : Guid.Empty;
                var winEvt = new ScenarioEvent($"OBJ_WIN_{obj.Id:N}_AreaCaptured", DelaySeconds: holdSec, Repeat: false, Enabled: true);

                if (vanguardGuid != Guid.Empty)
                {
                    winEvt.Conditions.Add(new ScenarioCondition(
                        ScenarioConditionKind.ObjectInArea,
                        vanguardGuid,
                        MinX: area.MinX,
                        MinZ: area.MinZ,
                        MaxX: area.MaxX,
                        MaxZ: area.MaxZ));
                }

                winEvt.Actions.Add(new ScenarioAction(ScenarioActionKind.Message, Text: $"戰略據點佔領達成：{obj.Title}！"));
                AppendRewardsAndUnlocks(winEvt, obj, graph);
                result.Add(winEvt);
                break;
            }

            case ObjectiveKind.EliminateAllEnemies:
            {
                var winEvt = new ScenarioEvent($"OBJ_WIN_{obj.Id:N}_EnemiesWiped", DelaySeconds: 2, Repeat: false, Enabled: true);
                if (obj.Parameters.TargetGuids.Count > 0)
                {
                    foreach (var guid in obj.Parameters.TargetGuids.Take(30))
                    {
                        winEvt.Conditions.Add(new ScenarioCondition(ScenarioConditionKind.ObjectDeadOrRemoved, guid));
                    }
                }
                winEvt.Actions.Add(new ScenarioAction(ScenarioActionKind.Message, Text: $"全殲敵軍：{obj.Title}！"));
                AppendRewardsAndUnlocks(winEvt, obj, graph);
                result.Add(winEvt);
                break;
            }

            case ObjectiveKind.CustomScripted:
            {
                var winEvt = new ScenarioEvent($"OBJ_WIN_{obj.Id:N}_Custom", DelaySeconds: Math.Max(1, obj.Parameters.HoldDurationSeconds), Repeat: false, Enabled: true);
                foreach (var cond in obj.Parameters.CustomConditions.Take(30))
                {
                    winEvt.Conditions.Add(cond);
                }
                winEvt.Actions.Add(new ScenarioAction(ScenarioActionKind.Message, Text: $"目標達成：{obj.Title}！"));
                AppendRewardsAndUnlocks(winEvt, obj, graph);
                result.Add(winEvt);
                break;
            }
        }

        return result;
    }

    private static void AppendRewardsAndUnlocks(ScenarioEvent evt, ObjectiveDefinition obj, ObjectiveDependencyGraph graph)
    {
        // 1. 獎勵提示與額外動作
        if (obj.Reward is not null)
        {
            if (!string.IsNullOrWhiteSpace(obj.Reward.CompletionMessage))
            {
                evt.Actions.Add(new ScenarioAction(ScenarioActionKind.Message, Text: obj.Reward.CompletionMessage));
            }
            foreach (var action in obj.Reward.Actions)
            {
                evt.Actions.Add(action);
            }
        }

        // 2. 解鎖後續目標提示
        var dependents = graph.GetDependents(obj.Id);
        if (dependents.Count > 0)
        {
            string unlockedTitles = string.Join("、", dependents.Select(d => $"【{d.Title}】"));
            evt.Actions.Add(new ScenarioAction(ScenarioActionKind.Message, Text: $"新目標已解鎖：{unlockedTitles}！"));
        }
    }

    private static void EnsureTerminalVictoryAction(
        List<ScenarioEvent> events,
        ObjectiveDependencyGraph graph,
        List<ScenarioEvent> primaryWinEvents)
    {
        var primaryObjectives = graph.Objectives
            .Where(o => o.Category == ObjectiveCategory.Primary)
            .ToList();

        if (primaryObjectives.Count == 0) return;

        // 若只有單一主線目標，在其完成事件最尾端加入 Victory
        if (primaryObjectives.Count == 1 && primaryWinEvents.Count == 1)
        {
            var targetEvent = primaryWinEvents[0];
            if (!targetEvent.Actions.Any(a => a.Kind is ScenarioActionKind.Victory or ScenarioActionKind.Defeat))
            {
                targetEvent.Actions.Add(new ScenarioAction(ScenarioActionKind.Victory));
            }
            return;
        }

        // 若有多個主線目標，尋找拓撲排序中最後一個主線目標
        var topologicalOrder = graph.GetTopologicalOrder();
        var lastPrimary = topologicalOrder.LastOrDefault(o => o.Category == ObjectiveCategory.Primary);
        if (lastPrimary is not null)
        {
            var lastWinEvent = primaryWinEvents.FirstOrDefault(e => e.Name.StartsWith($"OBJ_WIN_{lastPrimary.Id:N}", StringComparison.Ordinal));
            if (lastWinEvent is not null && !lastWinEvent.Actions.Any(a => a.Kind is ScenarioActionKind.Victory or ScenarioActionKind.Defeat))
            {
                lastWinEvent.Actions.Add(new ScenarioAction(ScenarioActionKind.Victory));
                return;
            }
        }

        // 獨立勝利宣告事件作為終點收尾
        var victoryFinalEvent = new ScenarioEvent("OBJ_SYS_CampaignVictory", DelaySeconds: 3, Repeat: false, Enabled: true);
        victoryFinalEvent.Actions.Add(new ScenarioAction(ScenarioActionKind.Message, Text: "戰役全勝！所有戰略主線任務已全部順利達成！"));
        victoryFinalEvent.Actions.Add(new ScenarioAction(ScenarioActionKind.Victory));
        events.Add(victoryFinalEvent);
    }

    #region Serialization & Lossless Metadata

    /// <summary>
    /// 將目標依賴圖序列化為 JSON 字串，以利儲存於 arm_scenario.json 或獨立設定檔。
    /// </summary>
    public static string SerializeGraph(ObjectiveDependencyGraph graph)
    {
        ArgumentNullException.ThrowIfNull(graph);
        var dto = new ObjectiveGraphDto(
            Objectives: graph.Objectives.ToList(),
            Dependencies: graph.Dependencies.ToList());
        return JsonSerializer.Serialize(dto, JsonOptions);
    }

    /// <summary>
    /// 從 JSON 字串反序列化並重建 ObjectiveDependencyGraph。
    /// </summary>
    public static ObjectiveDependencyGraph DeserializeGraph(string json)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(json);
        var dto = JsonSerializer.Deserialize<ObjectiveGraphDto>(json, JsonOptions)
            ?? throw new InvalidDataException("無法解析目標依賴圖資料。");

        var graph = new ObjectiveDependencyGraph();
        foreach (var obj in dto.Objectives)
        {
            graph.AddObjective(obj);
        }
        foreach (var dep in dto.Dependencies)
        {
            graph.AddDependency(dep.SourceObjectiveId, dep.TargetObjectiveId, dep.Relation);
        }
        return graph;
    }

    private sealed record ObjectiveGraphDto(
        List<ObjectiveDefinition> Objectives,
        List<ObjectiveDependency> Dependencies);

    #endregion
}
