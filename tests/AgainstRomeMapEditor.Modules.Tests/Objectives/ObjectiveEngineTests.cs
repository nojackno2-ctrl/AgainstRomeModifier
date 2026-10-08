namespace AgainstRomeMapEditor.Modules.Tests.Objectives;

using AgainstRomeMapEditor.Modules.Objectives;
using AgainstRomeModifier.Scripting;
using Xunit;

public sealed class ObjectiveEngineTests
{
    #region ObjectiveRuleCatalog Tests

    [Fact]
    public void Catalog_AllStandardTemplates_PassValidation()
    {
        var heroId = Guid.NewGuid();
        var buildingId = Guid.NewGuid();
        var vipId = Guid.NewGuid();
        var defendId = Guid.NewGuid();
        var area = ObjectiveAreaBounds.FromRectangle(1000, 1000, 3000, 3000);

        var templates = new List<ObjectiveDefinition>
        {
            ObjectiveRuleCatalog.CreateEliminateEnemies(),
            ObjectiveRuleCatalog.CreateSurvival("堅守防線", "抵擋羅馬軍團進攻", defendId, 120),
            ObjectiveRuleCatalog.CreateCaptureArea("佔領渡口", "搶佔萊茵河渡口", area, 20),
            ObjectiveRuleCatalog.CreateAssassinateTarget("刺殺百夫長", "斬殺前線指揮官", heroId),
            ObjectiveRuleCatalog.CreateDestroyBuilding("摧毀兵營", "拔除敵方前哨基地", buildingId),
            ObjectiveRuleCatalog.CreateEscortUnit("護送物資", "護送糧草車隊至營地", vipId, area),
            ObjectiveRuleCatalog.CreateDefendFailureCriterion("保護部落長老", "長老若陣亡則戰役失敗", defendId)
        };

        foreach (var def in templates)
        {
            var result = ObjectiveRuleCatalog.Validate(def);
            Assert.True(result.IsValid, $"範本「{def.Title}」驗證失敗: {string.Join(", ", result.Errors)}");
        }
    }

    [Fact]
    public void Catalog_InvalidParameters_ReportsExplicitErrors()
    {
        var invalidSurvival = new ObjectiveDefinition
        {
            Title = "",
            Kind = ObjectiveKind.Survival,
            Parameters = new ObjectiveRuleParameters
            {
                TargetGuids = [], // 缺少目標 GUID
                HoldDurationSeconds = -10, // 負數秒數
                PlayerTeam = 9 // 越界隊伍
            }
        };

        var result = ObjectiveRuleCatalog.Validate(invalidSurvival);
        Assert.False(result.IsValid);
        Assert.Contains(result.Errors, e => e.Contains("標題不可為空"));
        Assert.Contains(result.Errors, e => e.Contains("GUID"));
        Assert.Contains(result.Errors, e => e.Contains("隊伍"));
    }

    [Fact]
    public void ObjectiveAreaBounds_PointInBounds_EvaluatesCorrectly()
    {
        var rect = ObjectiveAreaBounds.FromRectangle(2000, 2000, 4000, 4000);
        Assert.True(rect.Contains(3000, 3000));
        Assert.True(rect.Contains(2000, 2000));
        Assert.False(rect.Contains(1999, 3000));
        Assert.False(rect.Contains(5000, 5000));

        var circle = ObjectiveAreaBounds.FromCircle(5000, 5000, 100);
        Assert.True(circle.Contains(5000, 5000));
        Assert.True(circle.Contains(5050, 5050)); // r = 70.7 <= 100
        Assert.False(circle.Contains(5100, 5100)); // r = 141.4 > 100
    }

    #endregion

    #region ObjectiveDependencyGraph Tests

    [Fact]
    public void Graph_CycleDetection_FindsCycles()
    {
        var graph = new ObjectiveDependencyGraph();
        var a = ObjectiveRuleCatalog.CreateEliminateEnemies("A");
        var b = ObjectiveRuleCatalog.CreateEliminateEnemies("B");
        var c = ObjectiveRuleCatalog.CreateEliminateEnemies("C");

        graph.AddObjective(a);
        graph.AddObjective(b);
        graph.AddObjective(c);

        graph.AddDependency(a.Id, b.Id, DependencyRelation.Prerequisite);
        graph.AddDependency(b.Id, c.Id, DependencyRelation.Prerequisite);
        graph.AddDependency(c.Id, a.Id, DependencyRelation.Prerequisite);

        var cycles = graph.DetectCycles();
        Assert.NotEmpty(cycles);
        Assert.Throws<InvalidOperationException>(() => graph.GetTopologicalOrder());
    }

    [Fact]
    public void Graph_TopologicalSort_PreservesOrder()
    {
        var graph = new ObjectiveDependencyGraph();
        var a = ObjectiveRuleCatalog.CreateEliminateEnemies("A");
        var b = ObjectiveRuleCatalog.CreateEliminateEnemies("B");
        var c = ObjectiveRuleCatalog.CreateEliminateEnemies("C");

        graph.AddObjective(a);
        graph.AddObjective(b);
        graph.AddObjective(c);

        graph.AddDependency(a.Id, b.Id, DependencyRelation.Prerequisite);
        graph.AddDependency(b.Id, c.Id, DependencyRelation.Prerequisite);

        var order = graph.GetTopologicalOrder();
        Assert.Equal(3, order.Count);
        Assert.Equal(a.Id, order[0].Id);
        Assert.Equal(b.Id, order[1].Id);
        Assert.Equal(c.Id, order[2].Id);
    }

    [Fact]
    public void Graph_PropagateStates_HandlesPrerequisitesAndBranching()
    {
        var graph = new ObjectiveDependencyGraph();
        var primary1 = ObjectiveRuleCatalog.CreateAssassinateTarget("刺殺將領", "刺殺", Guid.NewGuid());
        var primary2 = ObjectiveRuleCatalog.CreateEliminateEnemies("清剿敵軍");
        primary2 = primary2 with { InitialState = ObjectiveState.Inactive };

        graph.AddObjective(primary1);
        graph.AddObjective(primary2);
        graph.AddDependency(primary1.Id, primary2.Id, DependencyRelation.Prerequisite);

        // 初始狀態
        var initial = new Dictionary<Guid, ObjectiveState>
        {
            [primary1.Id] = ObjectiveState.Active,
            [primary2.Id] = ObjectiveState.Inactive
        };

        var propagated1 = graph.PropagateStates(initial);
        Assert.Equal(ObjectiveState.Active, propagated1[primary1.Id]);
        Assert.Equal(ObjectiveState.Inactive, propagated1[primary2.Id]);

        // 完成 primary1
        var next = new Dictionary<Guid, ObjectiveState>(propagated1)
        {
            [primary1.Id] = ObjectiveState.Completed
        };

        var propagated2 = graph.PropagateStates(next);
        Assert.Equal(ObjectiveState.Completed, propagated2[primary1.Id]);
        Assert.Equal(ObjectiveState.Active, propagated2[primary2.Id]); // 已被解鎖
    }

    [Fact]
    public void Graph_MutuallyExclusive_AbandonsRivalRoute()
    {
        var graph = new ObjectiveDependencyGraph();
        var routeGaul = ObjectiveRuleCatalog.CreateEliminateEnemies("結盟高盧");
        var routeRome = ObjectiveRuleCatalog.CreateEliminateEnemies("結盟羅馬");

        graph.AddObjective(routeGaul);
        graph.AddObjective(routeRome);
        graph.AddDependency(routeGaul.Id, routeRome.Id, DependencyRelation.MutuallyExclusive);

        var states = new Dictionary<Guid, ObjectiveState>
        {
            [routeGaul.Id] = ObjectiveState.Completed,
            [routeRome.Id] = ObjectiveState.Active
        };

        var updated = graph.PropagateStates(states);
        Assert.Equal(ObjectiveState.Completed, updated[routeGaul.Id]);
        Assert.Equal(ObjectiveState.Abandoned, updated[routeRome.Id]);
    }

    [Fact]
    public void Graph_EvaluateCampaignResult_VictoryAndDefeatConditions()
    {
        var graph = new ObjectiveDependencyGraph();
        var primary = ObjectiveRuleCatalog.CreateEliminateEnemies("主線");
        var bonus = ObjectiveRuleCatalog.CreateEliminateEnemies("支線", category: ObjectiveCategory.Bonus);
        var failure = ObjectiveRuleCatalog.CreateDefendFailureCriterion("保護主屋", "主屋被毀即失敗", Guid.NewGuid());

        graph.AddObjective(primary);
        graph.AddObjective(bonus);
        graph.AddObjective(failure);

        // 1. 進行中
        var states = new Dictionary<Guid, ObjectiveState>
        {
            [primary.Id] = ObjectiveState.Active,
            [bonus.Id] = ObjectiveState.Active,
            [failure.Id] = ObjectiveState.Active
        };
        Assert.Equal(CampaignResult.InProgress, graph.EvaluateCampaignResult(states));

        // 2. 失敗條件觸發
        states[failure.Id] = ObjectiveState.Failed;
        Assert.Equal(CampaignResult.Defeat, graph.EvaluateCampaignResult(states));

        // 3. 恢復失敗條件，主線達成
        states[failure.Id] = ObjectiveState.Active;
        states[primary.Id] = ObjectiveState.Completed;
        // 即使 bonus 尚未完成，主線全達成仍應算戰役勝利
        Assert.Equal(CampaignResult.Victory, graph.EvaluateCampaignResult(states));
    }

    #endregion

    #region BciObjectiveCompiler Tests

    [Fact]
    public void Compiler_CompilesDependencyGraphToValidScenarioEvents()
    {
        var graph = new ObjectiveDependencyGraph();
        var targetHero = Guid.NewGuid();
        var defendGuid = Guid.NewGuid();

        var assassinate = ObjectiveRuleCatalog.CreateAssassinateTarget("刺殺百夫長", "斬殺首領", targetHero,
            reward: new ObjectiveReward("獲得日耳曼勇士增援！", [
                new ScenarioAction(ScenarioActionKind.SpawnUnit, Alias: "GER_INF01", Count: 10)
            ]));

        var survival = ObjectiveRuleCatalog.CreateSurvival("堅守大本營", "堅守防線", defendGuid, 30);
        survival = survival with { InitialState = ObjectiveState.Inactive };

        graph.AddObjective(assassinate);
        graph.AddObjective(survival);
        graph.AddDependency(assassinate.Id, survival.Id, DependencyRelation.Prerequisite);

        var result = BciObjectiveCompiler.Compile(graph);
        Assert.True(result.Success, result.Summary);
        Assert.NotEmpty(result.CompiledEvents);

        // 檢查包含 Briefing 事件
        Assert.Contains(result.CompiledEvents, e => e.Name == "OBJ_SYS_Briefing");

        // 檢查包含暗殺事件
        var assassinateEvent = result.CompiledEvents.First(e => e.Name.StartsWith($"OBJ_WIN_{assassinate.Id:N}"));
        Assert.Contains(assassinateEvent.Conditions, c => c.Kind == ScenarioConditionKind.ObjectDeadOrRemoved && c.TargetId == targetHero);
        Assert.Contains(assassinateEvent.Actions, a => a.Kind == ScenarioActionKind.SpawnUnit);

        // 檢查最後具備 Victory 終結動作
        Assert.Contains(result.CompiledEvents, e => e.Actions.Any(a => a.Kind == ScenarioActionKind.Victory));
    }

    [Fact]
    public void Compiler_RoundTripSerialization_IsLossless()
    {
        var graph = new ObjectiveDependencyGraph();
        var heroId = Guid.NewGuid();
        var obj1 = ObjectiveRuleCatalog.CreateAssassinateTarget("目標1", "說明1", heroId);
        var obj2 = ObjectiveRuleCatalog.CreateEliminateEnemies("目標2", "說明2");

        graph.AddObjective(obj1);
        graph.AddObjective(obj2);
        graph.AddDependency(obj1.Id, obj2.Id, DependencyRelation.Prerequisite);

        string json = BciObjectiveCompiler.SerializeGraph(graph);
        Assert.False(string.IsNullOrWhiteSpace(json));

        var restored = BciObjectiveCompiler.DeserializeGraph(json);
        Assert.Equal(2, restored.Objectives.Count);
        Assert.Single(restored.Dependencies);

        var restoredObj1 = restored.FindObjective(obj1.Id);
        Assert.NotNull(restoredObj1);
        Assert.Equal(obj1.Title, restoredObj1.Title);
        Assert.Equal(obj1.Parameters.TargetGuids, restoredObj1.Parameters.TargetGuids);
    }

    #endregion

    #region ObjectiveSandbox Tests

    [Fact]
    public void Sandbox_SurvivalSimulation_ReachesVictory()
    {
        var graph = new ObjectiveDependencyGraph();
        var defendGuid = Guid.NewGuid();
        var survival = ObjectiveRuleCatalog.CreateSurvival("守衛神殿", "抵擋 10 秒", defendGuid, 10);
        graph.AddObjective(survival);

        var sandbox = new ObjectiveSandboxSession();
        sandbox.Initialize(graph);

        Assert.Equal(CampaignResult.InProgress, sandbox.CampaignStatus);

        // 推進 5 秒
        sandbox.Tick(5f);
        Assert.Equal(CampaignResult.InProgress, sandbox.CampaignStatus);
        var progress = sandbox.GetProgress(survival.Id);
        Assert.NotNull(progress);
        Assert.Equal(50f, progress.ProgressPercentage, precision: 1);

        // 再推進 6 秒（累計 11 秒）
        sandbox.Tick(6f);
        Assert.Equal(CampaignResult.Victory, sandbox.CampaignStatus);
        progress = sandbox.GetProgress(survival.Id);
        Assert.NotNull(progress);
        Assert.Equal(ObjectiveState.Completed, progress.State);
    }

    [Fact]
    public void Sandbox_DefendFailureCriterion_TriggersDefeat()
    {
        var graph = new ObjectiveDependencyGraph();
        var heroGuid = Guid.NewGuid();
        var baseGuid = Guid.NewGuid();

        var primary = ObjectiveRuleCatalog.CreateAssassinateTarget("刺殺敵將", "擊殺敵將", heroGuid);
        var failure = ObjectiveRuleCatalog.CreateDefendFailureCriterion("保護糧倉", "糧倉被毀即敗", baseGuid);

        graph.AddObjective(primary);
        graph.AddObjective(failure);

        var sandbox = new ObjectiveSandboxSession();
        sandbox.Initialize(graph);

        Assert.Equal(CampaignResult.InProgress, sandbox.CampaignStatus);

        // 模擬糧倉被毀
        sandbox.SimulateObjectDeath(baseGuid);
        Assert.Equal(CampaignResult.Defeat, sandbox.CampaignStatus);
        var progress = sandbox.GetProgress(failure.Id);
        Assert.NotNull(progress);
        Assert.Equal(ObjectiveState.Failed, progress.State);
    }

    [Fact]
    public void Sandbox_EscortCaravan_MovesToAreaAndSucceeds()
    {
        var graph = new ObjectiveDependencyGraph();
        var vipGuid = Guid.NewGuid();
        var area = ObjectiveAreaBounds.FromRectangle(5000, 5000, 6000, 6000);
        var escort = ObjectiveRuleCatalog.CreateEscortUnit("護送特使", "前往東方營地", vipGuid, area);
        graph.AddObjective(escort);

        var sandbox = new ObjectiveSandboxSession();
        sandbox.Initialize(graph);

        // 移動到區域外 (4000, 4000)
        sandbox.SimulateObjectMove(vipGuid, 4000, 4000);
        sandbox.Tick(1f);
        Assert.Equal(CampaignResult.InProgress, sandbox.CampaignStatus);

        // 移動到區域內 (5500, 5500)
        sandbox.SimulateObjectMove(vipGuid, 5500, 5500);
        Assert.Equal(CampaignResult.Victory, sandbox.CampaignStatus);
        var progress = sandbox.GetProgress(escort.Id);
        Assert.NotNull(progress);
        Assert.Equal(ObjectiveState.Completed, progress.State);
    }

    [Fact]
    public void Sandbox_CaptureArea_HoldDurationProgression()
    {
        var graph = new ObjectiveDependencyGraph();
        var vanguardGuid = Guid.NewGuid();
        var area = ObjectiveAreaBounds.FromRectangle(2000, 2000, 3000, 3000);
        var capture = ObjectiveRuleCatalog.CreateCaptureArea("奪取橋頭堡", "維持佔領 8 秒", area, holdDurationSeconds: 8);
        capture = capture with { Parameters = capture.Parameters with { TargetGuids = [vanguardGuid] } };
        graph.AddObjective(capture);

        var sandbox = new ObjectiveSandboxSession();
        sandbox.Initialize(graph);

        // 先鋒部隊進駐
        sandbox.SimulateObjectMove(vanguardGuid, 2500, 2500);

        // 推進 4 秒
        sandbox.Tick(4f);
        Assert.Equal(CampaignResult.InProgress, sandbox.CampaignStatus);
        var progress = sandbox.GetProgress(capture.Id);
        Assert.NotNull(progress);
        Assert.Equal(50f, progress.ProgressPercentage, precision: 1);

        // 推進 5 秒（累計 9 秒）
        sandbox.Tick(5f);
        Assert.Equal(CampaignResult.Victory, sandbox.CampaignStatus);
        progress = sandbox.GetProgress(capture.Id);
        Assert.NotNull(progress);
        Assert.Equal(ObjectiveState.Completed, progress.State);
    }

    #endregion
}
