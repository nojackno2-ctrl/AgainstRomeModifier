using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor.Modules.Events.Graph;

/// <summary>
/// 無損雙向轉換器：
/// 1. 將 ScenarioEvent 清單轉換為 EventGraph 視覺化節點圖（含階層式自動排版）。
/// 2. 將 EventGraph 編譯回符合 Against Rome 規格的 ScenarioEvent 清單。
/// </summary>
public static class EventGraphConverter
{
    private const float LayoutStartY = 60f;
    private const float ConditionColumnX = 60f;
    private const float TriggerColumnX = 360f;
    private const float ActionColumnStartX = 660f;
    private const float ActionSpacingX = 260f;
    private const float RowSpacingY = 240f;
    private const float ConditionSpacingY = 90f;

    /// <summary>將現有 ScenarioEvent 清單轉換為 EventGraph。</summary>
    public static EventGraph FromScenarioEvents(
        IReadOnlyList<ScenarioEvent> events,
        Func<Guid, string>? targetLabelResolver = null)
    {
        var graph = new EventGraph();
        if (events is null || events.Count == 0) return graph;

        float currentY = LayoutStartY;

        foreach (var item in events)
        {
            float eventStartY = currentY;

            // 1. 建立觸發節點
            var triggerNode = new EventTriggerNode
            {
                X = TriggerColumnX,
                Y = eventStartY,
                EventName = item.Name,
                DelaySeconds = item.DelaySeconds,
                Repeat = item.Repeat,
                Enabled = item.Enabled
            };
            graph.AddNode(triggerNode);

            // 2. 建立條件節點並連線至觸發節點
            float condY = eventStartY;
            foreach (var cond in item.Conditions)
            {
                string targetLabel = targetLabelResolver?.Invoke(cond.TargetId) ?? cond.TargetId.ToString();
                ConditionNode condNode = cond.Kind switch
                {
                    ScenarioConditionKind.ObjectExists => new ObjectExistsConditionNode(cond.TargetId, targetLabel),
                    ScenarioConditionKind.ObjectDeadOrRemoved => new ObjectDeadConditionNode(cond.TargetId, targetLabel),
                    ScenarioConditionKind.ObjectInArea => new ObjectInAreaConditionNode(cond.TargetId, cond.MinX, cond.MinZ, cond.MaxX, cond.MaxZ, targetLabel),
                    _ => new ObjectExistsConditionNode(cond.TargetId, targetLabel)
                };

                condNode.X = ConditionColumnX;
                condNode.Y = condY;
                graph.AddNode(condNode);

                graph.Connect(condNode.ConditionOut, triggerNode.ConditionsIn, out _, out _);
                condY += ConditionSpacingY;
            }

            // 3. 建立動作節點並形成執行序列連線
            GraphPort? previousExecOut = triggerNode.ExecOut;
            float actionX = ActionColumnStartX;

            foreach (var action in item.Actions)
            {
                ActionNode actionNode = action.Kind switch
                {
                    ScenarioActionKind.Message => new MessageActionNode(action.Text),
                    ScenarioActionKind.Diplomacy => new DiplomacyActionNode(action.Team, action.OtherTeam, action.Hostile),
                    ScenarioActionKind.SpawnUnit => new SpawnUnitActionNode(action.Alias, action.Team, action.X, action.Z, action.Count),
                    ScenarioActionKind.Victory => new VictoryActionNode(),
                    ScenarioActionKind.Defeat => new DefeatActionNode(),
                    _ => throw new NotSupportedException($"不支援的動作類型：{action.Kind}")
                };

                actionNode.X = actionX;
                actionNode.Y = eventStartY;
                graph.AddNode(actionNode);

                if (previousExecOut is not null)
                {
                    graph.Connect(previousExecOut, actionNode.ExecIn, out _, out _);
                }

                previousExecOut = actionNode.ExecOut;
                actionX += ActionSpacingX;
            }

            // 計算下一個事件的 Y 起始位置
            float rowHeight = Math.Max(RowSpacingY, Math.Max(condY - eventStartY + 60f, 180f));
            currentY += rowHeight;
        }

        return graph;
    }

    /// <summary>將 EventGraph 編譯回標準 ScenarioEvent 清單。</summary>
    public static List<ScenarioEvent> ToScenarioEvents(EventGraph graph)
    {
        ArgumentNullException.ThrowIfNull(graph);

        var result = new List<ScenarioEvent>();
        var triggerNodes = graph.Nodes.OfType<EventTriggerNode>().OrderBy(n => n.Y).ThenBy(n => n.X).ToList();

        foreach (var trigger in triggerNodes)
        {
            // 1. 收集條件
            var conditions = new List<ScenarioCondition>();
            var incomingConditionEdges = graph.GetIncomingEdges(trigger.ConditionsIn.Id);
            foreach (var edge in incomingConditionEdges)
            {
                var condNode = graph.FindNode(edge.SourceNodeId) as ConditionNode;
                if (condNode is not null)
                {
                    conditions.Add(condNode.ToScenarioCondition());
                }
            }

            // 2. 收集動作序列（沿 ExecOut 遍歷）
            var actions = new List<ScenarioAction>();
            GraphPort? currentExecPort = trigger.ExecOut;

            var visitedNodes = new HashSet<Guid>();
            while (currentExecPort is not null)
            {
                var outgoingEdges = graph.GetOutgoingEdges(currentExecPort.Id);
                if (outgoingEdges.Count == 0) break;

                var edge = outgoingEdges[0];
                var nextNode = graph.FindNode(edge.TargetNodeId);
                if (nextNode is null || visitedNodes.Contains(nextNode.Id))
                {
                    break; // 避免循環
                }

                visitedNodes.Add(nextNode.Id);

                if (nextNode is ActionNode actionNode)
                {
                    actions.Add(actionNode.ToScenarioAction());
                    currentExecPort = actionNode.ExecOut;
                }
                else
                {
                    // 若串接其他控制流節點，此處中斷或延伸
                    break;
                }
            }

            var scenarioEvent = new ScenarioEvent(
                trigger.EventName,
                trigger.DelaySeconds,
                trigger.Repeat,
                trigger.Enabled)
            {
                Conditions = conditions,
                Actions = actions
            };

            result.Add(scenarioEvent);
        }

        return result;
    }
}
