using AgainstRomeMapEditor.Modules.Events.Graph;
using AgainstRomeModifier.Scripting;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class EventGraphTests
{
    private static readonly string[] ValidAliases = ["GER_INF01", "ROM_LEG01", "CEL_INF01"];

    [Fact]
    public void EventGraph_NodeAndPortOperations_WorkCorrectly()
    {
        var graph = new EventGraph();
        var trigger = new EventTriggerNode { EventName = "TestTrigger" };
        var msg = new MessageActionNode("Hello World");
        var cond = new ObjectExistsConditionNode(Guid.NewGuid());

        graph.AddNode(trigger);
        graph.AddNode(msg);
        graph.AddNode(cond);

        Assert.Equal(3, graph.Nodes.Count);

        // 測試連接 Condition -> ConditionsIn
        bool condConnected = graph.Connect(cond.ConditionOut, trigger.ConditionsIn, out var condEdge, out string? condErr);
        Assert.True(condConnected);
        Assert.NotNull(condEdge);
        Assert.Null(condErr);

        // 測試連接 Trigger.ExecOut -> Msg.ExecIn
        bool execConnected = graph.Connect(trigger.ExecOut, msg.ExecIn, out var execEdge, out string? execErr);
        Assert.True(execConnected);
        Assert.NotNull(execEdge);
        Assert.Null(execErr);

        // 測試不可連接同節點端口
        bool selfConnect = graph.Connect(trigger.ExecIn, trigger.ExecOut, out _, out string? selfErr);
        Assert.False(selfConnect);
        Assert.NotNull(selfErr);

        // 測試類型不相容不可連接（Condition -> Execution）
        bool invalidTypeConnect = graph.Connect(cond.ConditionOut, msg.ExecIn, out _, out string? typeErr);
        Assert.False(invalidTypeConnect);
        Assert.NotNull(typeErr);

        // 測試移除節點會同步清理關聯連線
        graph.RemoveNode(msg.Id);
        Assert.Equal(2, graph.Nodes.Count);
        Assert.Single(graph.Edges); // 只剩 cond -> trigger
    }

    [Fact]
    public void EventGraphConverter_RoundTrip_IsLossless()
    {
        Guid target1 = Guid.NewGuid();
        Guid target2 = Guid.NewGuid();

        var originalEvents = new List<ScenarioEvent>
        {
            new ScenarioEvent("Stage1_KillEnemies", DelaySeconds: 5, Repeat: false, Enabled: true)
            {
                Conditions =
                [
                    new ScenarioCondition(ScenarioConditionKind.ObjectDeadOrRemoved, target1),
                    new ScenarioCondition(ScenarioConditionKind.ObjectInArea, target2, MinX: 100, MinZ: 200, MaxX: 500, MaxZ: 600)
                ],
                Actions =
                [
                    new ScenarioAction(ScenarioActionKind.Message, Text: "Enemies eliminated!"),
                    new ScenarioAction(ScenarioActionKind.Diplomacy, Team: 0, OtherTeam: 1, Hostile: false),
                    new ScenarioAction(ScenarioActionKind.SpawnUnit, Alias: "GER_INF01", Team: 0, X: 4500, Z: 5500, Count: 8)
                ]
            },
            new ScenarioEvent("Stage2_VictoryCheck", DelaySeconds: 15, Repeat: false, Enabled: true)
            {
                Conditions =
                [
                    new ScenarioCondition(ScenarioConditionKind.ObjectExists, target2)
                ],
                Actions =
                [
                    new ScenarioAction(ScenarioActionKind.Message, Text: "Victory achieved!"),
                    new ScenarioAction(ScenarioActionKind.Victory)
                ]
            }
        };

        // 轉換為圖
        var graph = EventGraphConverter.FromScenarioEvents(originalEvents);
        Assert.Equal(2, graph.Nodes.OfType<EventTriggerNode>().Count());
        Assert.Equal(3, graph.Nodes.OfType<ConditionNode>().Count());
        Assert.Equal(5, graph.Nodes.OfType<ActionNode>().Count());

        // 編譯回 ScenarioEvent 清單
        var compiledEvents = EventGraphConverter.ToScenarioEvents(graph);

        // 斷言結構與內容 100% 一致
        Assert.Equal(originalEvents.Count, compiledEvents.Count);
        for (int i = 0; i < originalEvents.Count; i++)
        {
            var orig = originalEvents[i];
            var comp = compiledEvents[i];

            Assert.Equal(orig.Name, comp.Name);
            Assert.Equal(orig.DelaySeconds, comp.DelaySeconds);
            Assert.Equal(orig.Repeat, comp.Repeat);
            Assert.Equal(orig.Enabled, comp.Enabled);

            Assert.Equal(orig.Conditions.Count, comp.Conditions.Count);
            for (int c = 0; c < orig.Conditions.Count; c++)
            {
                Assert.Equal(orig.Conditions[c].Kind, comp.Conditions[c].Kind);
                Assert.Equal(orig.Conditions[c].TargetId, comp.Conditions[c].TargetId);
                Assert.Equal(orig.Conditions[c].MinX, comp.Conditions[c].MinX);
                Assert.Equal(orig.Conditions[c].MinZ, comp.Conditions[c].MinZ);
                Assert.Equal(orig.Conditions[c].MaxX, comp.Conditions[c].MaxX);
                Assert.Equal(orig.Conditions[c].MaxZ, comp.Conditions[c].MaxZ);
            }

            Assert.Equal(orig.Actions.Count, comp.Actions.Count);
            for (int a = 0; a < orig.Actions.Count; a++)
            {
                Assert.Equal(orig.Actions[a].Kind, comp.Actions[a].Kind);
                Assert.Equal(orig.Actions[a].Text, comp.Actions[a].Text);
                Assert.Equal(orig.Actions[a].Team, comp.Actions[a].Team);
                Assert.Equal(orig.Actions[a].OtherTeam, comp.Actions[a].OtherTeam);
                Assert.Equal(orig.Actions[a].Hostile, comp.Actions[a].Hostile);
                Assert.Equal(orig.Actions[a].Alias, comp.Actions[a].Alias);
                Assert.Equal(orig.Actions[a].X, comp.Actions[a].X);
                Assert.Equal(orig.Actions[a].Z, comp.Actions[a].Z);
                Assert.Equal(orig.Actions[a].Count, comp.Actions[a].Count);
            }
        }
    }

    [Fact]
    public void EventGraph_ComplexCampaignChain_ValidatesAgainstNativeValidator()
    {
        // 戰役任務情境：
        // 擊敗指定敵軍部隊 -> 觸發對話 -> 計時器啟動(5s) -> 聚落轉移與援軍抵達 -> 勝利判定
        Guid enemySquadId = Guid.NewGuid();
        Guid villageChiefId = Guid.NewGuid();

        var events = new List<ScenarioEvent>
        {
            new ScenarioEvent("Campaign_DefeatEnemyAndWin", DelaySeconds: 5, Repeat: false, Enabled: true)
            {
                Conditions =
                [
                    new ScenarioCondition(ScenarioConditionKind.ObjectDeadOrRemoved, enemySquadId),
                    new ScenarioCondition(ScenarioConditionKind.ObjectExists, villageChiefId)
                ],
                Actions =
                [
                    new ScenarioAction(ScenarioActionKind.Message, Text: "敵方先遣隊已全滅！日耳曼聚落決定加入我方盟約。"),
                    new ScenarioAction(ScenarioActionKind.Diplomacy, Team: 0, OtherTeam: 2, Hostile: false),
                    new ScenarioAction(ScenarioActionKind.SpawnUnit, Alias: "GER_INF01", Team: 0, X: 4200, Z: 5100, Count: 10),
                    new ScenarioAction(ScenarioActionKind.Victory)
                ]
            }
        };

        // 轉為圖並進行靜態分析
        var graph = EventGraphConverter.FromScenarioEvents(events);
        var validTargets = new HashSet<Guid> { enemySquadId, villageChiefId };
        var diagnostics = EventGraphValidator.Validate(graph, validTargets, ValidAliases);

        // 無任何錯誤診斷
        Assert.DoesNotContain(diagnostics, d => d.Severity == GraphDiagnosticSeverity.Error);

        // 編譯回原生 ScenarioEvent 並通過原生 ScenarioEventValidator 檢驗
        var compiled = EventGraphConverter.ToScenarioEvents(graph);
        ScenarioEventValidator.Validate(compiled, ValidAliases);
        ScenarioEventValidator.ValidateTerminalActions(compiled);
    }

    [Fact]
    public void EventGraphValidator_DetectsCycles()
    {
        var graph = new EventGraph();
        var trigger = new EventTriggerNode { EventName = "CycleTrigger" };
        var msg1 = new MessageActionNode("Step 1");
        var msg2 = new MessageActionNode("Step 2");

        graph.AddNode(trigger);
        graph.AddNode(msg1);
        graph.AddNode(msg2);

        graph.Connect(trigger.ExecOut, msg1.ExecIn, out _, out _);
        graph.Connect(msg1.ExecOut!, msg2.ExecIn, out _, out _);
        // 刻意建立環路：msg2 -> msg1
        graph.Connect(msg2.ExecOut!, msg1.ExecIn, out _, out _);

        var diagnostics = EventGraphValidator.Validate(graph);
        Assert.Contains(diagnostics, d => d.Code == "EXECUTION_CYCLE_DETECTED" && d.Severity == GraphDiagnosticSeverity.Error);
    }

    [Fact]
    public void EventGraphValidator_DetectsOrphanActionsAndDanglingConditions()
    {
        var graph = new EventGraph();
        var trigger = new EventTriggerNode { EventName = "EmptyTrigger" }; // 沒有後續動作
        var orphanAction = new MessageActionNode("Never Called"); // 沒有 ExecIn 連線
        var danglingCond = new ObjectExistsConditionNode(Guid.NewGuid()); // 沒有連至 Trigger

        graph.AddNode(trigger);
        graph.AddNode(orphanAction);
        graph.AddNode(danglingCond);

        var diagnostics = EventGraphValidator.Validate(graph);

        Assert.Contains(diagnostics, d => d.Code == "EMPTY_TRIGGER_FLOW" && d.Severity == GraphDiagnosticSeverity.Warning);
        Assert.Contains(diagnostics, d => d.Code == "ORPHAN_ACTION" && d.Severity == GraphDiagnosticSeverity.Warning);
        Assert.Contains(diagnostics, d => d.Code == "DANGLING_CONDITION" && d.Severity == GraphDiagnosticSeverity.Warning);
    }

    [Fact]
    public void EventGraphValidator_DetectsInvalidTargetGuidsAndAliases()
    {
        var graph = new EventGraph();
        var trigger = new EventTriggerNode { EventName = "Trigger" };
        var emptyGuidCond = new ObjectExistsConditionNode(Guid.Empty);
        var nonexistentGuidCond = new ObjectExistsConditionNode(Guid.NewGuid());
        var invalidSpawnAction = new SpawnUnitActionNode("NON_EXISTENT_UNIT", team: 99, x: 20000, z: -50, count: 50);

        graph.AddNode(trigger);
        graph.AddNode(emptyGuidCond);
        graph.AddNode(nonexistentGuidCond);
        graph.AddNode(invalidSpawnAction);

        graph.Connect(emptyGuidCond.ConditionOut, trigger.ConditionsIn, out _, out _);
        graph.Connect(nonexistentGuidCond.ConditionOut, trigger.ConditionsIn, out _, out _);
        graph.Connect(trigger.ExecOut, invalidSpawnAction.ExecIn, out _, out _);

        var validTargets = new HashSet<Guid> { Guid.NewGuid() }; // 不包含 nonexistentGuidCond
        var diagnostics = EventGraphValidator.Validate(graph, validTargets, ValidAliases);

        Assert.Contains(diagnostics, d => d.Code == "EMPTY_TARGET_GUID");
        Assert.Contains(diagnostics, d => d.Code == "INVALID_TARGET_GUID");
        Assert.Contains(diagnostics, d => d.Code == "INVALID_SPAWN_ALIAS");
        Assert.Contains(diagnostics, d => d.Code == "INVALID_SPAWN_TEAM");
        Assert.Contains(diagnostics, d => d.Code == "INVALID_SPAWN_COUNT");
        Assert.Contains(diagnostics, d => d.Code == "INVALID_SPAWN_COORDINATES");
    }

    [Fact]
    public void EventGraphValidator_ValidatesTerminalActions()
    {
        // 測試 Repeat 事件中包含 Victory 動作
        var graph = new EventGraph();
        var trigger = new EventTriggerNode { EventName = "RepeatVictory", Repeat = true, DelaySeconds = 5 };
        var victory = new VictoryActionNode();

        graph.AddNode(trigger);
        graph.AddNode(victory);
        graph.Connect(trigger.ExecOut, victory.ExecIn, out _, out _);

        var diagnostics = EventGraphValidator.Validate(graph);
        Assert.Contains(diagnostics, d => d.Code == "TERMINAL_IN_REPEAT_EVENT" && d.Severity == GraphDiagnosticSeverity.Error);
    }
}
