using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private readonly List<ScenarioEvent> _events = new();
    private IReadOnlyList<ScenarioEvent> _eventsBaseline = Array.Empty<ScenarioEvent>();
    private readonly ListBox _eventList = new() { Dock = DockStyle.Fill, IntegralHeight = false };
    private readonly Button _eventAdd = new() { AutoSize = true };
    private readonly Button _eventEdit = new() { AutoSize = true };
    private readonly Button _eventDelete = new() { AutoSize = true };
    private readonly Button _eventCopy = new() { AutoSize = true };
    private readonly Label _eventHint = new() { Dock = DockStyle.Top, Height = 100, Padding = new Padding(8) };

    private bool EventsDirty() => !_events.SequenceEqual(_eventsBaseline);

    private Control BuildEventsPanel()
    {
        var panel = new Panel { Dock = DockStyle.Fill, Padding = new Padding(8) };
        var commands = new FlowLayoutPanel { Dock = DockStyle.Bottom, AutoSize = true };
        commands.Controls.AddRange([_eventAdd, _eventEdit, _eventCopy, _eventDelete]);
        panel.Controls.Add(_eventList); panel.Controls.Add(_eventHint); panel.Controls.Add(commands);
        _eventList.SelectedIndexChanged += (_, _) => UpdateEventButtons();
        _eventList.DoubleClick += (_, _) => EditEvent(false);
        _eventAdd.Click += (_, _) => EditEvent(true);
        _eventEdit.Click += (_, _) => EditEvent(false);
        _eventCopy.Click += (_, _) => DuplicateEvent();
        _eventDelete.Click += (_, _) =>
        {
            if (_selected?.IsCustom != true || _eventList.SelectedIndex < 0) return;
            int index = _eventList.SelectedIndex;
            _events.RemoveAt(index); RefreshEventList(Math.Min(index, _events.Count - 1)); UpdateEditorState();
        };
        return panel;
    }

    private void LoadEvents(string map)
    {
        _events.Clear(); _events.AddRange(ScenarioDocument.Load(map).Events);
        _eventsBaseline = _events.ToArray(); RefreshEventList();
    }

    private void LocalizeEvents(bool en)
    {
        if (_inspectorTabs.TabPages.Count > 5) _inspectorTabs.TabPages[5].Text = en ? "Events" : "事件";
        _eventAdd.Text = en ? "Add" : "新增"; _eventEdit.Text = en ? "Edit" : "編輯"; _eventDelete.Text = en ? "Delete" : "刪除";
        _eventCopy.Text = en ? "Duplicate" : "複製";
        _eventHint.Text = en
            ? "Run actions when the timer is due and all object conditions hold. Conditions can check existence or death/removal of placed objects. Repeat uses the selected interval. Save to apply; in-game behavior still needs validation."
            : "計時到期且所有物件條件成立時執行動作。條件可檢查放置物件存在、死亡或移除；可單次或依間隔重複。按「儲存」套用；遊戲內效果仍待驗證。";
        RefreshEventList(_eventList.SelectedIndex);
    }

    private void RefreshEventList(int selected = -1)
    {
        bool en = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        _eventList.BeginUpdate(); _eventList.Items.Clear();
        foreach (ScenarioEvent item in _events)
            _eventList.Items.Add($"{(item.Enabled ? "●" : "○")} {item.Name} — {item.DelaySeconds}s {(item.Repeat ? (en ? "repeat" : "重複") : (en ? "once" : "單次"))}, {item.Conditions.Count} {(en ? "conditions" : "條件")}");
        if (selected >= 0 && selected < _eventList.Items.Count) _eventList.SelectedIndex = selected;
        _eventList.EndUpdate(); UpdateEventButtons();
    }

    private void UpdateEventButtons()
    {
        bool custom = _selected?.IsCustom == true, selected = _eventList.SelectedIndex >= 0;
        _eventAdd.Enabled = custom && _events.Count < 256;
        _eventCopy.Enabled = _eventAdd.Enabled && selected;
        _eventEdit.Enabled = _eventDelete.Enabled = custom && selected;
    }

    private void DuplicateEvent()
    {
        if (_selected?.IsCustom != true || _eventList.SelectedIndex < 0 || _events.Count >= 256) return;
        int index = _eventList.SelectedIndex;
        ScenarioEvent source = _events[index];
        bool en = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        string suffix = en ? " (copy)" : "（副本）";
        string name = source.Name[..Math.Min(source.Name.Length, 100 - suffix.Length)] + suffix;
        _events.Insert(index + 1, source with { Name = name, Actions = source.Actions.ToList(), Conditions = source.Conditions.ToList() });
        RefreshEventList(index + 1); UpdateEditorState();
    }

    private void EditEvent(bool add)
    {
        if (_selected?.IsCustom != true || add && _events.Count >= 256 || !add && _eventList.SelectedIndex < 0) return;
        int index = _eventList.SelectedIndex;
        bool en = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        var seed = add ? new ScenarioEvent(en ? $"Event {_events.Count + 1}" : $"事件 {_events.Count + 1}")
            { Actions = [new ScenarioAction(ScenarioActionKind.Message, "Welcome!")] } : _events[index];
        var unitTypes = _objectCatalog.Where(item => item.Category == AgainstRomeModifier.Maps.SdlObjectCategory.Figure).ToArray();
        using var dialog = new ScenarioEventDialog(seed, unitTypes.Select(AliasOf).ToArray(), en,
            alias => _objectCatalog.FirstOrDefault(item => AliasOf(item) == alias) is { } type ? ObjectDisplayName(type, en) : alias,
            _placedObjects.Select(item => new ScenarioSpawn(AliasOf(item.Type), item.WorldX, item.WorldZ, item.Team)
                { Id = item.ScenarioId }).ToArray());
        if (dialog.ShowDialog(this) != DialogResult.OK) return;
        if (add) { _events.Add(dialog.Result!); index = _events.Count - 1; } else _events[index] = dialog.Result!;
        RefreshEventList(index); UpdateEditorState();
    }
}
