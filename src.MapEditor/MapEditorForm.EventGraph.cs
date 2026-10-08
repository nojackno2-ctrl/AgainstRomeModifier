using AgainstRomeMapEditor.Modules.Persistence;
using AgainstRomeMapEditor.Events;
using AgainstRomeMapEditor.Modules.Events.Graph;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private readonly EventGraphCanvasControl _eventGraphCanvas = new() { Dock = DockStyle.Fill, Zoom = .45f };
    private readonly TabControl _eventViews = new() { Dock = DockStyle.Fill };
    private readonly Button _graphApply = new() { AutoSize = true };
    private readonly Button _graphReload = new() { AutoSize = true };
    private readonly Button _graphValidate = new() { AutoSize = true };
    private readonly Button _graphDisconnect = new() { AutoSize = true };
    private readonly Label _graphStatus = new() { Dock = DockStyle.Bottom, Height = 70 };
    private bool _eventGraphDirty;

    private Control BuildEventGraphTabs(Control listPanel)
    {
        _eventViews.TabPages.Add(new TabPage());
        _eventViews.TabPages[0].Controls.Add(listPanel);
        var graphPanel = new Panel { Dock = DockStyle.Fill };
        var commands = new FlowLayoutPanel { Dock = DockStyle.Top, AutoSize = true };
        commands.Controls.AddRange([_graphApply, _graphReload, _graphValidate, _graphDisconnect]);
        graphPanel.Controls.Add(_eventGraphCanvas);
        graphPanel.Controls.Add(_graphStatus);
        graphPanel.Controls.Add(commands);
        _eventViews.TabPages.Add(new TabPage());
        _eventViews.TabPages[1].Controls.Add(graphPanel);
        _eventGraphCanvas.GraphModified += (_, _) => MarkEventGraphChanged();
        _eventGraphCanvas.SelectionChanged += (_, _) => UpdateEventGraphButtons();
        _graphReload.Click += (_, _) => { ReloadEventGraph(); UpdateEditorState(); };
        _graphValidate.Click += (_, _) => ValidateEventGraph();
        _graphApply.Click += (_, _) =>
        {
            try { ApplyEventGraph(); UpdateEditorState(); }
            catch (Exception ex) { _graphStatus.Text = ex.Message; }
        };
        _graphDisconnect.Click += (_, _) =>
        {
            if (_selected?.IsCustom != true || _eventGraphCanvas.Graph is not { } graph) return;
            foreach (var id in _eventGraphCanvas.SelectedNodes)
                foreach (var edge in graph.GetNodeEdges(id)) graph.Disconnect(edge.Id);
            MarkEventGraphChanged();
        };
        return _eventViews;
    }

    private void ReloadEventGraph()
    {
        _eventGraphCanvas.Graph = EventGraphConverter.FromScenarioEvents(EventSession.Capture());
        _eventGraphDirty = false;
        _graphStatus.Text = "";
        UpdateEventGraphButtons();
    }

    internal void MarkEventGraphChanged()
    {
        if (_selected?.IsCustom != true) return;
        _eventGraphDirty = true;
        ValidateEventGraph();
        UpdateEditorState();
    }

    private IReadOnlyList<GraphDiagnostic> ValidateEventGraph()
    {
        if (_eventGraphCanvas.Graph is not { } graph) return [];
        var diagnostics = EventGraphValidator.Validate(graph);
        _eventGraphCanvas.SetDiagnostics(diagnostics);
        _graphStatus.Text = string.Join(Environment.NewLine, diagnostics.Select(d => $"{d.Code}: {d.Message}"));
        return diagnostics;
    }

    internal void ApplyEventGraph()
    {
        if (!_eventGraphDirty || _selected?.IsCustom != true || _eventGraphCanvas.Graph is not { } graph) return;
        var diagnostics = ValidateEventGraph();
        // The converter cannot represent delays, execution links into triggers, or disconnected nodes.
        // Reject these drafts rather than silently truncate actions on save.
        if (diagnostics.Any(d => d.Severity == GraphDiagnosticSeverity.Error ||
                d.Code is "ORPHAN_ACTION" or "DANGLING_CONDITION" or "ORPHAN_DELAY") ||
            graph.Nodes.Any(n => n is DelayNode) ||
            graph.Nodes.OfType<EventTriggerNode>().Any(n => graph.GetIncomingEdges(n.ExecIn.Id).Count > 0))
            throw new InvalidDataException(AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English
                ? "Fix graph errors and disconnected nodes before applying. Delay nodes and chained triggers are unsupported."
                : "請先修正事件圖錯誤與未連接節點。尚不支援延遲節點與串接觸發器。");
        var events = EventGraphConverter.ToScenarioEvents(graph);
        var previous = ScenarioDocument.Load(_selected.DirectoryPath);
        var scenario = new ScenarioDocument
        {
            Events = events,
            Spawns = PlacedDirty() ? _placedObjects.Select(item => new ScenarioSpawn(AliasOf(item.Type),
                item.WorldX, item.WorldZ, item.Team) { Id = item.ScenarioId }).ToList() : previous.Spawns.ToList()
        };
        ScenarioSavePreflight.Validate(scenario, _objectCatalog.Select(AliasOf).ToArray());
        // Replace through the session, retaining its save baseline.
        EventSession.ReplaceAll(events);
        _eventGraphDirty = false;
        RefreshEventList();
    }

    private void UpdateEventGraphButtons()
    {
        bool editable = _selected?.IsCustom == true;
        _eventGraphCanvas.Enabled = editable;
        _graphApply.Enabled = editable && _eventGraphDirty;
        _graphReload.Enabled = _eventGraphDirty;
        _graphValidate.Enabled = _selected is not null;
        _graphDisconnect.Enabled = editable && _eventGraphCanvas.SelectedNodes.Count > 0;
    }

    private void LocalizeEventGraph(bool en)
    {
        if (_eventViews.TabPages.Count < 2) return;
        _eventViews.TabPages[0].Text = en ? "Event list" : "事件清單";
        _eventViews.TabPages[1].Text = en ? "Visual graph" : "視覺事件圖";
        _graphApply.Text = en ? "Apply graph" : "套用事件圖";
        _graphReload.Text = en ? "Discard graph edits" : "放棄圖編輯";
        _graphValidate.Text = en ? "Validate" : "驗證";
        _graphDisconnect.Text = en ? "Disconnect selected" : "中斷選取連線";
    }
}
