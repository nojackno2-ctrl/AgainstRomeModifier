using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor.Modules.Events.Graph;

#region Trigger Nodes

/// <summary>劇情事件觸發核心節點，對應單一 ScenarioEvent 的生命週期與計時控制。</summary>
public sealed class EventTriggerNode : GraphNode
{
    public override string Title => string.IsNullOrWhiteSpace(EventName) ? "Scenario Event" : $"Event: {EventName}";
    public override string Category => "Trigger";

    public string EventName { get; set; } = "New Event";
    public int DelaySeconds { get; set; } = 10;
    public bool Repeat { get; set; }
    public bool Enabled { get; set; } = true;

    public GraphPort ExecIn { get; }
    public GraphPort ConditionsIn { get; }
    public GraphPort ExecOut { get; }

    public EventTriggerNode()
    {
        ExecIn = AddPort("ExecIn", "In", PortType.Execution, PortDirection.Input);
        ConditionsIn = AddPort("ConditionsIn", "Conditions (AND)", PortType.Condition, PortDirection.Input, allowMultiple: true);
        ExecOut = AddPort("ExecOut", "Triggered", PortType.Execution, PortDirection.Output);
    }

    public override GraphNode Clone() => new EventTriggerNode
    {
        X = X,
        Y = Y,
        EventName = EventName,
        DelaySeconds = DelaySeconds,
        Repeat = Repeat,
        Enabled = Enabled
    };
}

#endregion

#region Condition Nodes

/// <summary>條件節點基底類別。</summary>
public abstract class ConditionNode : GraphNode
{
    public override string Category => "Condition";
    public abstract ScenarioConditionKind Kind { get; }
    public Guid TargetId { get; set; }
    public string TargetLabel { get; set; } = "";

    public GraphPort ConditionOut { get; }

    protected ConditionNode()
    {
        ConditionOut = AddPort("ConditionOut", "Condition", PortType.Condition, PortDirection.Output, allowMultiple: true);
    }

    public abstract ScenarioCondition ToScenarioCondition();
}

/// <summary>物件存在條件：地圖上指定目標尚在場上。</summary>
public sealed class ObjectExistsConditionNode : ConditionNode
{
    public override string Title => "Condition: Object Exists";
    public override ScenarioConditionKind Kind => ScenarioConditionKind.ObjectExists;

    public ObjectExistsConditionNode() { }
    public ObjectExistsConditionNode(Guid targetId, string targetLabel = "")
    {
        TargetId = targetId;
        TargetLabel = targetLabel;
    }

    public override ScenarioCondition ToScenarioCondition() =>
        new(ScenarioConditionKind.ObjectExists, TargetId);

    public override GraphNode Clone() => new ObjectExistsConditionNode(TargetId, TargetLabel) { X = X, Y = Y };
}

/// <summary>物件陣亡或移除條件：目標小隊被全滅、建築被摧毀或屍體被清理。</summary>
public sealed class ObjectDeadConditionNode : ConditionNode
{
    public override string Title => "Condition: Object Dead/Removed";
    public override ScenarioConditionKind Kind => ScenarioConditionKind.ObjectDeadOrRemoved;

    public ObjectDeadConditionNode() { }
    public ObjectDeadConditionNode(Guid targetId, string targetLabel = "")
    {
        TargetId = targetId;
        TargetLabel = targetLabel;
    }

    public override ScenarioCondition ToScenarioCondition() =>
        new(ScenarioConditionKind.ObjectDeadOrRemoved, TargetId);

    public override GraphNode Clone() => new ObjectDeadConditionNode(TargetId, TargetLabel) { X = X, Y = Y };
}

/// <summary>物件位於區域條件：目標在指定 X/Z 矩形邊界內。</summary>
public sealed class ObjectInAreaConditionNode : ConditionNode
{
    public override string Title => "Condition: Object In Area";
    public override ScenarioConditionKind Kind => ScenarioConditionKind.ObjectInArea;

    public int MinX { get; set; }
    public int MinZ { get; set; }
    public int MaxX { get; set; } = 16383;
    public int MaxZ { get; set; } = 16383;

    public ObjectInAreaConditionNode() { }
    public ObjectInAreaConditionNode(Guid targetId, int minX, int minZ, int maxX, int maxZ, string targetLabel = "")
    {
        TargetId = targetId;
        MinX = minX;
        MinZ = minZ;
        MaxX = maxX;
        MaxZ = maxZ;
        TargetLabel = targetLabel;
    }

    public override ScenarioCondition ToScenarioCondition() =>
        new(ScenarioConditionKind.ObjectInArea, TargetId, MinX, MinZ, MaxX, MaxZ);

    public override GraphNode Clone() => new ObjectInAreaConditionNode(TargetId, MinX, MinZ, MaxX, MaxZ, TargetLabel) { X = X, Y = Y };
}

#endregion

#region Action Nodes

/// <summary>動作節點抽象基底類別。</summary>
public abstract class ActionNode : GraphNode
{
    public override string Category => "Action";
    public abstract ScenarioActionKind Kind { get; }

    public GraphPort ExecIn { get; }
    public GraphPort? ExecOut { get; protected set; }

    protected ActionNode(bool hasExecOut = true)
    {
        ExecIn = AddPort("ExecIn", "In", PortType.Execution, PortDirection.Input);
        if (hasExecOut)
        {
            ExecOut = AddPort("ExecOut", "Out", PortType.Execution, PortDirection.Output);
        }
    }

    public abstract ScenarioAction ToScenarioAction();
}

/// <summary>顯示對話或任務提示訊息。</summary>
public sealed class MessageActionNode : ActionNode
{
    public override string Title => "Action: Show Message";
    public override ScenarioActionKind Kind => ScenarioActionKind.Message;
    public string MessageText { get; set; } = "";

    public MessageActionNode() : base(hasExecOut: true) { }
    public MessageActionNode(string text) : base(hasExecOut: true) => MessageText = text;

    public override ScenarioAction ToScenarioAction() =>
        new(ScenarioActionKind.Message, Text: MessageText);

    public override GraphNode Clone() => new MessageActionNode(MessageText) { X = X, Y = Y };
}

/// <summary>變更陣營外交關係（交戰/和平）。</summary>
public sealed class DiplomacyActionNode : ActionNode
{
    public override string Title => $"Action: Diplomacy (T{Team} {(Hostile ? "vs" : "with")} T{OtherTeam})";
    public override ScenarioActionKind Kind => ScenarioActionKind.Diplomacy;

    public int Team { get; set; }
    public int OtherTeam { get; set; } = 1;
    public bool Hostile { get; set; } = true;

    public DiplomacyActionNode() : base(hasExecOut: true) { }
    public DiplomacyActionNode(int team, int otherTeam, bool hostile = true) : base(hasExecOut: true)
    {
        Team = team;
        OtherTeam = otherTeam;
        Hostile = hostile;
    }

    public override ScenarioAction ToScenarioAction() =>
        new(ScenarioActionKind.Diplomacy, Team: Team, OtherTeam: OtherTeam, Hostile: Hostile);

    public override GraphNode Clone() => new DiplomacyActionNode(Team, OtherTeam, Hostile) { X = X, Y = Y };
}

/// <summary>生成新部隊（援軍或敵軍增援）。</summary>
public sealed class SpawnUnitActionNode : ActionNode
{
    public override string Title => $"Action: Spawn Unit ({Alias} x{Count})";
    public override ScenarioActionKind Kind => ScenarioActionKind.SpawnUnit;

    public string Alias { get; set; } = "GER_INF01";
    public int Team { get; set; }
    public float SpawnX { get; set; } = 8000;
    public float SpawnZ { get; set; } = 8000;
    public int Count { get; set; } = 10;

    public SpawnUnitActionNode() : base(hasExecOut: true) { }
    public SpawnUnitActionNode(string alias, int team, float x, float z, int count = 10) : base(hasExecOut: true)
    {
        Alias = alias;
        Team = team;
        SpawnX = x;
        SpawnZ = z;
        Count = count;
    }

    public override ScenarioAction ToScenarioAction() =>
        new(ScenarioActionKind.SpawnUnit, Alias: Alias, Team: Team, X: SpawnX, Z: SpawnZ, Count: Count);

    public override GraphNode Clone() => new SpawnUnitActionNode(Alias, Team, SpawnX, SpawnZ, Count) { X = X, Y = Y };
}

/// <summary>勝利判定終止動作。</summary>
public sealed class VictoryActionNode : ActionNode
{
    public override string Title => "Action: Victory";
    public override ScenarioActionKind Kind => ScenarioActionKind.Victory;

    public VictoryActionNode() : base(hasExecOut: false) { }

    public override ScenarioAction ToScenarioAction() => new(ScenarioActionKind.Victory);

    public override GraphNode Clone() => new VictoryActionNode { X = X, Y = Y };
}

/// <summary>失敗判定終止動作。</summary>
public sealed class DefeatActionNode : ActionNode
{
    public override string Title => "Action: Defeat";
    public override ScenarioActionKind Kind => ScenarioActionKind.Defeat;

    public DefeatActionNode() : base(hasExecOut: false) { }

    public override ScenarioAction ToScenarioAction() => new(ScenarioActionKind.Defeat);

    public override GraphNode Clone() => new DefeatActionNode { X = X, Y = Y };
}

/// <summary>計時器延遲節點（用於視覺化連鎖事件階段轉移）。</summary>
public sealed class DelayNode : GraphNode
{
    public override string Title => $"Delay: {Seconds}s";
    public override string Category => "Flow";

    public int Seconds { get; set; } = 5;

    public GraphPort ExecIn { get; }
    public GraphPort ExecOut { get; }

    public DelayNode()
    {
        ExecIn = AddPort("ExecIn", "In", PortType.Execution, PortDirection.Input);
        ExecOut = AddPort("ExecOut", "Out", PortType.Execution, PortDirection.Output);
    }

    public DelayNode(int seconds) : this() => Seconds = seconds;

    public override GraphNode Clone() => new DelayNode(Seconds) { X = X, Y = Y };
}

#endregion
