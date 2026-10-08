namespace AgainstRomeMapEditor.Modules.Events.Graph;

/// <summary>
/// 劇情事件圖靜態分析器：
/// 1. 偵測 Execution 迴路與潛在死鎖。
/// 2. 偵測孤立動作與懸空條件。
/// 3. 驗證終止動作（勝利/失敗）合規性。
/// 4. 檢驗目標物件 GUID 是否有效或已遭刪除。
/// 5. 檢驗部隊別名與數值邊界合規性。
/// </summary>
public static class EventGraphValidator
{
    public static List<GraphDiagnostic> Validate(
        EventGraph graph,
        IReadOnlySet<Guid>? validTargetIds = null,
        IReadOnlyCollection<string>? validAliases = null)
    {
        ArgumentNullException.ThrowIfNull(graph);
        var diagnostics = new List<GraphDiagnostic>();

        // 1. 數量限制檢查
        var triggers = graph.Nodes.OfType<EventTriggerNode>().ToList();
        if (triggers.Count > 256)
        {
            diagnostics.Add(new GraphDiagnostic(
                GraphDiagnosticSeverity.Error,
                "EVENT_LIMIT_EXCEEDED",
                $"圖中事件觸發節點數量為 {triggers.Count}，超過遊戲上限 256 個。"));
        }

        // 2. 循環依賴與死鎖偵測 (Cycle Detection on Execution Flow)
        DetectExecutionCycles(graph, diagnostics);

        // 3. 逐節點深入驗證
        foreach (var node in graph.Nodes)
        {
            switch (node)
            {
                case EventTriggerNode trigger:
                    ValidateTriggerNode(graph, trigger, diagnostics);
                    break;

                case ConditionNode condition:
                    ValidateConditionNode(graph, condition, validTargetIds, diagnostics);
                    break;

                case ActionNode action:
                    ValidateActionNode(graph, action, validAliases, diagnostics);
                    break;

                case DelayNode delay:
                    ValidateDelayNode(graph, delay, diagnostics);
                    break;
            }
        }

        return diagnostics;
    }

    private static void ValidateTriggerNode(
        EventGraph graph,
        EventTriggerNode trigger,
        List<GraphDiagnostic> diagnostics)
    {
        if (string.IsNullOrWhiteSpace(trigger.EventName))
        {
            diagnostics.Add(new GraphDiagnostic(
                GraphDiagnosticSeverity.Error,
                "EMPTY_EVENT_NAME",
                "事件觸發節點名稱不可為空。",
                NodeId: trigger.Id));
        }

        if (trigger.DelaySeconds is < 0 or > 86400)
        {
            diagnostics.Add(new GraphDiagnostic(
                GraphDiagnosticSeverity.Error,
                "INVALID_DELAY",
                $"事件「{trigger.EventName}」延遲秒數必須介於 0 與 86400 秒。",
                NodeId: trigger.Id));
        }

        if (trigger.Repeat && trigger.DelaySeconds == 0)
        {
            diagnostics.Add(new GraphDiagnostic(
                GraphDiagnosticSeverity.Error,
                "REPEAT_ZERO_DELAY",
                $"重複執行的事件「{trigger.EventName}」間隔時間至少必須為 1 秒，否則將造成每一幀觸發之死鎖問題。",
                NodeId: trigger.Id));
        }

        // 檢查條件連線數量
        var incomingConditions = graph.GetIncomingEdges(trigger.ConditionsIn.Id);
        if (incomingConditions.Count > 32)
        {
            diagnostics.Add(new GraphDiagnostic(
                GraphDiagnosticSeverity.Error,
                "CONDITION_LIMIT_EXCEEDED",
                $"事件「{trigger.EventName}」連接了 {incomingConditions.Count} 個條件，超過上限 32 個。",
                NodeId: trigger.Id));
        }

        // 檢查動作序列
        var outgoingExec = graph.GetOutgoingEdges(trigger.ExecOut.Id);
        if (outgoingExec.Count == 0)
        {
            diagnostics.Add(new GraphDiagnostic(
                GraphDiagnosticSeverity.Warning,
                "EMPTY_TRIGGER_FLOW",
                $"事件觸發節點「{trigger.EventName}」未連接任何後續動作，觸發時將無實質效果。",
                NodeId: trigger.Id));
        }
        else
        {
            // 沿動作鏈統計數量與終端動作
            int actionCount = 0;
            bool hasTerminal = false;
            var currentPort = trigger.ExecOut;
            var visited = new HashSet<Guid>();

            while (currentPort is not null)
            {
                var nextEdges = graph.GetOutgoingEdges(currentPort.Id);
                if (nextEdges.Count == 0) break;

                var nextNode = graph.FindNode(nextEdges[0].TargetNodeId);
                if (nextNode is null || visited.Contains(nextNode.Id)) break;
                visited.Add(nextNode.Id);

                if (nextNode is ActionNode act)
                {
                    actionCount++;
                    if (act is VictoryActionNode or DefeatActionNode)
                    {
                        hasTerminal = true;
                        // 檢查終端動作是否在重複事件中
                        if (trigger.Repeat)
                        {
                            diagnostics.Add(new GraphDiagnostic(
                                GraphDiagnosticSeverity.Error,
                                "TERMINAL_IN_REPEAT_EVENT",
                                $"事件「{trigger.EventName}」設定為重複執行，但其動作鏈中包含勝利/失敗終止動作。",
                                NodeId: act.Id));
                        }

                        // 檢查終端動作後是否還有多餘動作
                        if (nextEdges.Count > 0 && act.ExecOut is not null && graph.GetOutgoingEdges(act.ExecOut.Id).Count > 0)
                        {
                            diagnostics.Add(new GraphDiagnostic(
                                GraphDiagnosticSeverity.Error,
                                "ACTION_AFTER_TERMINAL",
                                $"勝利/失敗終止動作後不可串接其他動作。",
                                NodeId: act.Id));
                        }
                    }
                    currentPort = act.ExecOut;
                }
                else if (nextNode is DelayNode delay)
                {
                    currentPort = delay.ExecOut;
                }
                else break;
            }

            if (actionCount > 32)
            {
                diagnostics.Add(new GraphDiagnostic(
                    GraphDiagnosticSeverity.Error,
                    "ACTION_LIMIT_EXCEEDED",
                    $"事件「{trigger.EventName}」動作數量為 {actionCount}，超過上限 32 個。",
                    NodeId: trigger.Id));
            }
        }
    }

    private static void ValidateConditionNode(
        EventGraph graph,
        ConditionNode condition,
        IReadOnlySet<Guid>? validTargetIds,
        List<GraphDiagnostic> diagnostics)
    {
        // 檢查目標 GUID 是否有效
        if (condition.TargetId == Guid.Empty)
        {
            diagnostics.Add(new GraphDiagnostic(
                GraphDiagnosticSeverity.Error,
                "EMPTY_TARGET_GUID",
                $"條件節點「{condition.Title}」尚未指定目標物件 GUID。",
                NodeId: condition.Id));
        }
        else if (validTargetIds is not null && !validTargetIds.Contains(condition.TargetId))
        {
            diagnostics.Add(new GraphDiagnostic(
                GraphDiagnosticSeverity.Error,
                "INVALID_TARGET_GUID",
                $"條件節點「{condition.Title}」引用的目標物件 GUID ({condition.TargetId}) 在地圖中不存在或已遭刪除。",
                NodeId: condition.Id));
        }

        // 檢查區域條件之座標有效性
        if (condition is ObjectInAreaConditionNode area)
        {
            if (area.MinX < 0 || area.MinZ < 0 || area.MaxX > 16383 || area.MaxZ > 16383 ||
                area.MinX > area.MaxX || area.MinZ > area.MaxZ)
            {
                diagnostics.Add(new GraphDiagnostic(
                    GraphDiagnosticSeverity.Error,
                    "INVALID_AREA_BOUNDS",
                    $"區域條件座標範圍無效：[{area.MinX}, {area.MinZ}] 至 [{area.MaxX}, {area.MaxZ}] 必須在 0–16383 範圍內且 Min <= Max。",
                    NodeId: condition.Id));
            }
        }

        // 檢查懸空條件
        var outgoingEdges = graph.GetOutgoingEdges(condition.ConditionOut.Id);
        if (outgoingEdges.Count == 0)
        {
            diagnostics.Add(new GraphDiagnostic(
                GraphDiagnosticSeverity.Warning,
                "DANGLING_CONDITION",
                $"條件節點「{condition.Title}」未連接至任何事件觸發節點，屬於懸空條件。",
                NodeId: condition.Id));
        }
    }

    private static void ValidateActionNode(
        EventGraph graph,
        ActionNode action,
        IReadOnlyCollection<string>? validAliases,
        List<GraphDiagnostic> diagnostics)
    {
        // 檢查是否為孤立動作（無輸入連線）
        var incomingEdges = graph.GetIncomingEdges(action.ExecIn.Id);
        if (incomingEdges.Count == 0)
        {
            diagnostics.Add(new GraphDiagnostic(
                GraphDiagnosticSeverity.Warning,
                "ORPHAN_ACTION",
                $"動作節點「{action.Title}」沒有前置執行連線，永遠不會被執行。",
                NodeId: action.Id));
        }

        switch (action)
        {
            case MessageActionNode msg:
                if (string.IsNullOrWhiteSpace(msg.MessageText))
                {
                    diagnostics.Add(new GraphDiagnostic(
                        GraphDiagnosticSeverity.Error,
                        "EMPTY_MESSAGE_TEXT",
                        "訊息動作文字不可為空。",
                        NodeId: action.Id));
                }
                break;

            case DiplomacyActionNode dip:
                if (dip.Team is < 0 or > 7 || dip.OtherTeam is < 0 or > 7 || dip.Team == dip.OtherTeam)
                {
                    diagnostics.Add(new GraphDiagnostic(
                        GraphDiagnosticSeverity.Error,
                        "INVALID_DIPLOMACY_TEAMS",
                        $"外交動作需要兩個不同隊伍（0–7），當前設定為隊伍 {dip.Team} 與隊伍 {dip.OtherTeam}。",
                        NodeId: action.Id));
                }
                break;

            case SpawnUnitActionNode spawn:
                if (spawn.Team is < 0 or > 7)
                {
                    diagnostics.Add(new GraphDiagnostic(
                        GraphDiagnosticSeverity.Error,
                        "INVALID_SPAWN_TEAM",
                        $"生成部隊隊伍必須介於 0 與 7，當前為 {spawn.Team}。",
                        NodeId: action.Id));
                }
                if (spawn.Count is < 1 or > 20)
                {
                    diagnostics.Add(new GraphDiagnostic(
                        GraphDiagnosticSeverity.Error,
                        "INVALID_SPAWN_COUNT",
                        $"生成部隊人數必須介於 1 與 20，當前為 {spawn.Count}。",
                        NodeId: action.Id));
                }
                if (!float.IsFinite(spawn.SpawnX) || !float.IsFinite(spawn.SpawnZ) ||
                    spawn.SpawnX is < 0 or > 16383 || spawn.SpawnZ is < 0 or > 16383)
                {
                    diagnostics.Add(new GraphDiagnostic(
                        GraphDiagnosticSeverity.Error,
                        "INVALID_SPAWN_COORDINATES",
                        $"生成部隊座標必須介於 0 與 16383 之間，當前為 ({spawn.SpawnX}, {spawn.SpawnZ})。",
                        NodeId: action.Id));
                }
                if (validAliases is not null && !validAliases.Contains(spawn.Alias, StringComparer.OrdinalIgnoreCase))
                {
                    diagnostics.Add(new GraphDiagnostic(
                        GraphDiagnosticSeverity.Error,
                        "INVALID_SPAWN_ALIAS",
                        $"未知的部隊別名：{spawn.Alias}。",
                        NodeId: action.Id));
                }
                break;
        }
    }

    private static void ValidateDelayNode(
        EventGraph graph,
        DelayNode delay,
        List<GraphDiagnostic> diagnostics)
    {
        var incoming = graph.GetIncomingEdges(delay.ExecIn.Id);
        if (incoming.Count == 0)
        {
            diagnostics.Add(new GraphDiagnostic(
                GraphDiagnosticSeverity.Warning,
                "ORPHAN_DELAY",
                $"延遲節點「{delay.Title}」沒有前置執行連線。",
                NodeId: delay.Id));
        }
        if (delay.Seconds < 0)
        {
            diagnostics.Add(new GraphDiagnostic(
                GraphDiagnosticSeverity.Error,
                "NEGATIVE_DELAY",
                "延遲時間不能為負數。",
                NodeId: delay.Id));
        }
    }

    private static void DetectExecutionCycles(
        EventGraph graph,
        List<GraphDiagnostic> diagnostics)
    {
        // 建立 Execution 連線鄰接表
        var adj = new Dictionary<Guid, List<Guid>>();
        foreach (var edge in graph.Edges)
        {
            var sourcePort = graph.FindPort(edge.SourcePortId);
            var targetPort = graph.FindPort(edge.TargetPortId);
            if (sourcePort?.Type == PortType.Execution && targetPort?.Type == PortType.Execution)
            {
                if (!adj.TryGetValue(edge.SourceNodeId, out var neighbors))
                {
                    neighbors = new List<Guid>();
                    adj[edge.SourceNodeId] = neighbors;
                }
                neighbors.Add(edge.TargetNodeId);
            }
        }

        // DFS 三色標記法偵測環路：0=White(未訪), 1=Gray(當前路徑), 2=Black(已完成)
        var color = new Dictionary<Guid, int>();
        foreach (var node in graph.Nodes)
        {
            color[node.Id] = 0;
        }

        foreach (var node in graph.Nodes)
        {
            if (color[node.Id] == 0)
            {
                var cycleNodes = new List<Guid>();
                if (DfsCycle(node.Id, adj, color, cycleNodes))
                {
                    diagnostics.Add(new GraphDiagnostic(
                        GraphDiagnosticSeverity.Error,
                        "EXECUTION_CYCLE_DETECTED",
                        $"偵測到執行流死循環（包含節點 {node.Title}），將導致遊戲腳本引擎陷入無窮迴圈或死鎖。",
                        NodeId: node.Id));
                }
            }
        }
    }

    private static bool DfsCycle(
        Guid u,
        Dictionary<Guid, List<Guid>> adj,
        Dictionary<Guid, int> color,
        List<Guid> path)
    {
        color[u] = 1; // Gray
        path.Add(u);

        if (adj.TryGetValue(u, out var neighbors))
        {
            foreach (var v in neighbors)
            {
                if (color.TryGetValue(v, out int c))
                {
                    if (c == 1) // 遇到 Gray 代表找到環！
                    {
                        return true;
                    }
                    if (c == 0 && DfsCycle(v, adj, color, path))
                    {
                        return true;
                    }
                }
            }
        }

        color[u] = 2; // Black
        path.RemoveAt(path.Count - 1);
        return false;
    }
}
