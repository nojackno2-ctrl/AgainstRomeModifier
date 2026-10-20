using System.Text.Json;
using AgainstRomeModifier.Scripting;
using AgainstRomeMapEditor.Modules.Events;
using AgainstRomeMapEditor.Modules.Events.Graph;

namespace AgainstRome.Scenario.EventGraphOracle;

internal static class Program
{
    public static int Main(string[] args)
    {
        if (args.Length < 2)
        {
            Console.Error.WriteLine("Usage: EventGraphOracle <request.json> <response.json>");
            return 1;
        }

        string requestPath = args[0];
        string responsePath = args[1];

        var options = new JsonSerializerOptions
        {
            PropertyNameCaseInsensitive = true,
            WriteIndented = true
        };

        var requestJson = File.ReadAllText(requestPath);
        var request = JsonSerializer.Deserialize<OracleRequest>(requestJson, options)
            ?? throw new InvalidOperationException("Failed to deserialize OracleRequest");

        var response = new OracleResponse();

        // 1. FromScenarioEvents
        foreach (var tc in request.FromScenarioEventsCases)
        {
            var events = tc.Events.Select(ToScenarioEvent).ToList();
            var graph = EventGraphConverter.FromScenarioEvents(events);

            var res = new FromScenarioEventsResult { Name = tc.Name };
            foreach (var node in graph.Nodes)
            {
                var obsNode = new ObservedNode
                {
                    Id = node.Id,
                    Category = node.Category,
                    Title = node.Title,
                    X = node.X,
                    Y = node.Y
                };
                foreach (var port in node.Ports)
                {
                    obsNode.Ports.Add(new ObservedPort
                    {
                        Id = port.Id,
                        Name = port.Name,
                        DisplayName = port.DisplayName,
                        PortType = (int)port.Type,
                        Direction = (int)port.Direction,
                        AllowMultiple = port.AllowMultiple
                    });
                }
                res.Nodes.Add(obsNode);
            }

            foreach (var edge in graph.Edges)
            {
                res.Edges.Add(new ObservedEdge
                {
                    SourceNodeId = edge.SourceNodeId,
                    SourcePortName = graph.FindPort(edge.SourcePortId)?.Name ?? "",
                    TargetNodeId = edge.TargetNodeId,
                    TargetPortName = graph.FindPort(edge.TargetPortId)?.Name ?? ""
                });
            }

            response.FromScenarioEventsResults.Add(res);
        }

        // 2. ToScenarioEvents
        foreach (var tc in request.ToScenarioEventsCases)
        {
            var graph = BuildGraph(tc);
            var events = EventGraphConverter.ToScenarioEvents(graph);
            var res = new ToScenarioEventsResult
            {
                Name = tc.Name,
                Events = events.Select(FromScenarioEvent).ToList()
            };
            response.ToScenarioEventsResults.Add(res);
        }

        // 3. Validator
        foreach (var tc in request.ValidatorCases)
        {
            var graph = BuildGraph(new GraphTestCase { Name = tc.Name, Nodes = tc.Nodes, Edges = tc.Edges });
            var validTargets = tc.ValidTargetIds != null ? tc.ValidTargetIds.ToHashSet() : null;
            var diagnostics = EventGraphValidator.Validate(graph, validTargets, tc.ValidAliases);

            var res = new ValidatorResult { Name = tc.Name };
            foreach (var d in diagnostics)
            {
                res.Diagnostics.Add(new ObservedDiagnostic
                {
                    Severity = (int)d.Severity,
                    Code = d.Code,
                    Message = d.Message,
                    NodeId = d.NodeId
                });
            }
            response.ValidatorResults.Add(res);
        }

        // 4. Session
        foreach (var tc in request.SessionCases)
        {
            var session = new ScenarioEventSession();
            if (tc.InitialEvents != null && tc.InitialEvents.Count > 0)
            {
                var init = tc.InitialEvents.Select(ToScenarioEvent).ToList();
                session.Load(init);
            }

            var res = new SessionResult { Name = tc.Name };
            foreach (var op in tc.Operations)
            {
                var step = new ObservedSessionStep { Op = op.Op };
                try
                {
                    switch (op.Op)
                    {
                        case "Add":
                            session.Add(ToScenarioEvent(op.Event!));
                            step.Success = true;
                            break;
                        case "AddRange":
                            session.AddRange(op.Events!.Select(ToScenarioEvent).ToList());
                            step.Success = true;
                            break;
                        case "Replace":
                            session.Replace(op.Index, ToScenarioEvent(op.Event!));
                            step.Success = true;
                            break;
                        case "ReplaceAll":
                            session.ReplaceAll(op.Events!.Select(ToScenarioEvent).ToList());
                            step.Success = true;
                            break;
                        case "RemoveAt":
                            session.RemoveAt(op.Index);
                            step.Success = true;
                            break;
                        case "Duplicate":
                            session.Duplicate(op.Index, op.Suffix);
                            step.Success = true;
                            break;
                        case "Undo":
                            step.Success = session.Undo();
                            break;
                        case "Redo":
                            step.Success = session.Redo();
                            break;
                        case "Reset":
                            session.Reset();
                            step.Success = true;
                            break;
                        case "AcceptChanges":
                            session.AcceptChanges();
                            step.Success = true;
                            break;
                        case "AcceptBaseline":
                            session.AcceptBaseline(op.Events!.Select(ToScenarioEvent).ToList());
                            step.Success = true;
                            break;
                        case "Load":
                            session.Load(op.Events!.Select(ToScenarioEvent).ToList());
                            step.Success = true;
                            break;
                        default:
                            throw new InvalidOperationException($"Unknown session op: {op.Op}");
                    }
                }
                catch (Exception ex)
                {
                    step.Success = false;
                    step.Error = ex.GetType().Name + ": " + ex.Message;
                }

                step.Count = session.Count;
                step.IsDirty = session.IsDirty;
                step.CanUndo = session.CanUndo;
                step.CanRedo = session.CanRedo;
                step.EventNames = session.Capture().Select(e => e.Name).ToList();
                step.BaselineNames = session.Baseline.Select(e => e.Name).ToList();
                res.Steps.Add(step);
            }

            response.SessionResults.Add(res);
        }

        var responseJson = JsonSerializer.Serialize(response, options);
        File.WriteAllText(responsePath, responseJson);
        return 0;
    }

    private static EventGraph BuildGraph(GraphTestCase tc)
    {
        var graph = new EventGraph();
        foreach (var nodeDto in tc.Nodes)
        {
            var node = CreateNode(nodeDto);
            graph.AddNode(node);
        }

        foreach (var edgeDto in tc.Edges)
        {
            var sourceNode = graph.FindNode(edgeDto.SourceNodeId);
            var targetNode = graph.FindNode(edgeDto.TargetNodeId);
            if (sourceNode != null && targetNode != null)
            {
                var sp = sourceNode.FindPort(edgeDto.SourcePortName);
                var tp = targetNode.FindPort(edgeDto.TargetPortName);
                if (sp != null && tp != null)
                {
                    graph.Connect(sp, tp, out _, out _);
                }
            }
        }

        return graph;
    }

    private static GraphNode CreateNode(DtoNode dto)
    {
        GraphNode node = dto.Kind switch
        {
            "Trigger" => new EventTriggerNode
            {
                Id = dto.Id,
                EventName = dto.EventName,
                DelaySeconds = dto.DelaySeconds,
                Repeat = dto.Repeat,
                Enabled = dto.Enabled
            },
            "ExistsCondition" => new ObjectExistsConditionNode(dto.TargetId, dto.TargetLabel)
            {
                Id = dto.Id
            },
            "DeadCondition" => new ObjectDeadConditionNode(dto.TargetId, dto.TargetLabel)
            {
                Id = dto.Id
            },
            "AreaCondition" => new ObjectInAreaConditionNode(dto.TargetId, dto.MinX, dto.MinZ, dto.MaxX, dto.MaxZ, dto.TargetLabel)
            {
                Id = dto.Id
            },
            "MessageAction" => new MessageActionNode(dto.MessageText)
            {
                Id = dto.Id
            },
            "DiplomacyAction" => new DiplomacyActionNode(dto.Team, dto.OtherTeam, dto.Hostile)
            {
                Id = dto.Id
            },
            "SpawnUnitAction" => new SpawnUnitActionNode(dto.Alias, dto.Team, dto.SpawnX, dto.SpawnZ, dto.Count)
            {
                Id = dto.Id
            },
            "VictoryAction" => new VictoryActionNode
            {
                Id = dto.Id
            },
            "DefeatAction" => new DefeatActionNode
            {
                Id = dto.Id
            },
            "Delay" => new DelayNode(dto.Seconds)
            {
                Id = dto.Id
            },
            _ => throw new InvalidOperationException($"Unknown node kind: {dto.Kind}")
        };

        node.X = dto.X;
        node.Y = dto.Y;
        foreach (var port in node.Ports)
        {
            port.NodeId = node.Id;
        }
        return node;
    }

    private static ScenarioEvent ToScenarioEvent(DtoEvent dto)
    {
        var ev = new ScenarioEvent(dto.Name, dto.DelaySeconds, dto.Repeat, dto.Enabled);
        if (dto.Conditions != null)
        {
            foreach (var c in dto.Conditions)
            {
                ev.Conditions.Add(new ScenarioCondition(
                    (ScenarioConditionKind)c.Kind,
                    c.TargetId,
                    c.MinX,
                    c.MinZ,
                    c.MaxX,
                    c.MaxZ));
            }
        }
        if (dto.Actions != null)
        {
            foreach (var a in dto.Actions)
            {
                ev.Actions.Add(new ScenarioAction(
                    (ScenarioActionKind)a.Kind,
                    Text: a.Text,
                    Team: a.Team,
                    OtherTeam: a.OtherTeam,
                    Hostile: a.Hostile,
                    Alias: a.Alias,
                    X: a.X,
                    Z: a.Z,
                    Count: a.Count));
            }
        }
        return ev;
    }

    private static DtoEvent FromScenarioEvent(ScenarioEvent ev)
    {
        return new DtoEvent
        {
            Name = ev.Name,
            DelaySeconds = ev.DelaySeconds,
            Repeat = ev.Repeat,
            Enabled = ev.Enabled,
            Conditions = ev.Conditions.Select(c => new DtoCondition
            {
                Kind = (int)c.Kind,
                TargetId = c.TargetId,
                MinX = c.MinX,
                MinZ = c.MinZ,
                MaxX = c.MaxX,
                MaxZ = c.MaxZ
            }).ToList(),
            Actions = ev.Actions.Select(a => new DtoAction
            {
                Kind = (int)a.Kind,
                Text = a.Text,
                Team = a.Team,
                OtherTeam = a.OtherTeam,
                Hostile = a.Hostile,
                Alias = a.Alias,
                X = a.X,
                Z = a.Z,
                Count = a.Count
            }).ToList()
        };
    }
}

public class OracleRequest
{
    public List<FromScenarioEventsTestCase> FromScenarioEventsCases { get; set; } = new();
    public List<GraphTestCase> ToScenarioEventsCases { get; set; } = new();
    public List<ValidatorTestCase> ValidatorCases { get; set; } = new();
    public List<SessionTestCase> SessionCases { get; set; } = new();
}

public class FromScenarioEventsTestCase
{
    public string Name { get; set; } = "";
    public List<DtoEvent> Events { get; set; } = new();
}

public class GraphTestCase
{
    public string Name { get; set; } = "";
    public List<DtoNode> Nodes { get; set; } = new();
    public List<DtoEdge> Edges { get; set; } = new();
}

public class ValidatorTestCase
{
    public string Name { get; set; } = "";
    public List<DtoNode> Nodes { get; set; } = new();
    public List<DtoEdge> Edges { get; set; } = new();
    public List<Guid>? ValidTargetIds { get; set; }
    public List<string>? ValidAliases { get; set; }
}

public class SessionTestCase
{
    public string Name { get; set; } = "";
    public List<DtoEvent> InitialEvents { get; set; } = new();
    public List<DtoSessionOp> Operations { get; set; } = new();
}

public class DtoSessionOp
{
    public string Op { get; set; } = "";
    public DtoEvent? Event { get; set; }
    public List<DtoEvent>? Events { get; set; }
    public int Index { get; set; }
    public string Suffix { get; set; } = "";
}

public class DtoEvent
{
    public string Name { get; set; } = "";
    public int DelaySeconds { get; set; }
    public bool Repeat { get; set; }
    public bool Enabled { get; set; }
    public List<DtoCondition> Conditions { get; set; } = new();
    public List<DtoAction> Actions { get; set; } = new();
}

public class DtoCondition
{
    public int Kind { get; set; }
    public Guid TargetId { get; set; }
    public int MinX { get; set; }
    public int MinZ { get; set; }
    public int MaxX { get; set; } = 16383;
    public int MaxZ { get; set; } = 16383;
}

public class DtoAction
{
    public int Kind { get; set; }
    public string Text { get; set; } = "";
    public int Team { get; set; }
    public int OtherTeam { get; set; } = 1;
    public bool Hostile { get; set; } = true;
    public string Alias { get; set; } = "";
    public float X { get; set; }
    public float Z { get; set; }
    public int Count { get; set; } = 1;
}

public class DtoNode
{
    public Guid Id { get; set; }
    public string Kind { get; set; } = "";
    public float X { get; set; }
    public float Y { get; set; }
    public string EventName { get; set; } = "";
    public int DelaySeconds { get; set; }
    public bool Repeat { get; set; }
    public bool Enabled { get; set; } = true;
    public Guid TargetId { get; set; }
    public string TargetLabel { get; set; } = "";
    public int MinX { get; set; }
    public int MinZ { get; set; }
    public int MaxX { get; set; } = 16383;
    public int MaxZ { get; set; } = 16383;
    public string MessageText { get; set; } = "";
    public int Team { get; set; }
    public int OtherTeam { get; set; } = 1;
    public bool Hostile { get; set; } = true;
    public string Alias { get; set; } = "";
    public float SpawnX { get; set; }
    public float SpawnZ { get; set; }
    public int Count { get; set; } = 1;
    public int Seconds { get; set; }
}

public class DtoEdge
{
    public Guid SourceNodeId { get; set; }
    public string SourcePortName { get; set; } = "";
    public Guid TargetNodeId { get; set; }
    public string TargetPortName { get; set; } = "";
}

public class OracleResponse
{
    public List<FromScenarioEventsResult> FromScenarioEventsResults { get; set; } = new();
    public List<ToScenarioEventsResult> ToScenarioEventsResults { get; set; } = new();
    public List<ValidatorResult> ValidatorResults { get; set; } = new();
    public List<SessionResult> SessionResults { get; set; } = new();
}

public class FromScenarioEventsResult
{
    public string Name { get; set; } = "";
    public List<ObservedNode> Nodes { get; set; } = new();
    public List<ObservedEdge> Edges { get; set; } = new();
}

public class ObservedNode
{
    public Guid Id { get; set; }
    public string Category { get; set; } = "";
    public string Title { get; set; } = "";
    public float X { get; set; }
    public float Y { get; set; }
    public List<ObservedPort> Ports { get; set; } = new();
}

public class ObservedPort
{
    public Guid Id { get; set; }
    public string Name { get; set; } = "";
    public string DisplayName { get; set; } = "";
    public int PortType { get; set; }
    public int Direction { get; set; }
    public bool AllowMultiple { get; set; }
}

public class ObservedEdge
{
    public Guid SourceNodeId { get; set; }
    public string SourcePortName { get; set; } = "";
    public Guid TargetNodeId { get; set; }
    public string TargetPortName { get; set; } = "";
}

public class ToScenarioEventsResult
{
    public string Name { get; set; } = "";
    public List<DtoEvent> Events { get; set; } = new();
}

public class ValidatorResult
{
    public string Name { get; set; } = "";
    public List<ObservedDiagnostic> Diagnostics { get; set; } = new();
}

public class ObservedDiagnostic
{
    public int Severity { get; set; }
    public string Code { get; set; } = "";
    public string Message { get; set; } = "";
    public Guid? NodeId { get; set; }
}

public class SessionResult
{
    public string Name { get; set; } = "";
    public List<ObservedSessionStep> Steps { get; set; } = new();
}

public class ObservedSessionStep
{
    public string Op { get; set; } = "";
    public bool Success { get; set; }
    public string? Error { get; set; }
    public int Count { get; set; }
    public bool IsDirty { get; set; }
    public bool CanUndo { get; set; }
    public bool CanRedo { get; set; }
    public List<string> EventNames { get; set; } = new();
    public List<string> BaselineNames { get; set; } = new();
}
