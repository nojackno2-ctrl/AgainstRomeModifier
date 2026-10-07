using AgainstRomeModifier;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor;

internal sealed class ScenarioEventDialog : Form
{
    private readonly TextBox _name = new() { Dock = DockStyle.Fill };
    private readonly NumericUpDown _delay = new() { Dock = DockStyle.Fill, Maximum = 86400 };
    private readonly CheckBox _repeat = new() { AutoSize = true };
    private readonly CheckBox _enabled = new() { AutoSize = true };
    private readonly ListBox _list = new() { Dock = DockStyle.Fill, IntegralHeight = false };
    private readonly Button _moveUp = new() { AutoSize = true };
    private readonly Button _moveDown = new() { AutoSize = true };
    private Button _addAction = null!;
    private readonly List<ScenarioAction> _actions;
    private readonly List<ScenarioCondition> _conditions;
    private readonly ListBox _conditionList = new() { Dock = DockStyle.Fill, IntegralHeight = false };
    internal ScenarioEvent? Result { get; private set; }

    internal ScenarioEventDialog(ScenarioEvent item, IReadOnlyCollection<string> aliases, bool en, Func<string, string>? unitName = null,
        IReadOnlyList<ScenarioSpawn>? targets = null)
    {
        Text = en ? "Edit event" : "編輯事件"; StartPosition = FormStartPosition.CenterParent;
        Size = new Size(650, 480); MinimumSize = new Size(560, 420);
        _name.Text = item.Name; _delay.Value = Math.Clamp(item.DelaySeconds, 0, 86400);
        _repeat.Text = en ? "Repeat" : "重複執行"; _repeat.Checked = item.Repeat;
        _enabled.Text = en ? "Enabled" : "啟用"; _enabled.Checked = item.Enabled;
        _actions = item.Actions.ToList();
        _conditions = item.Conditions.ToList(); targets ??= Array.Empty<ScenarioSpawn>();
        var fields = new TableLayoutPanel { Dock = DockStyle.Top, AutoSize = true, ColumnCount = 2, Padding = new Padding(10) };
        fields.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 110)); fields.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        Field(fields, en ? "Name" : "名稱", _name); Field(fields, en ? "Earliest / interval (s)" : "最早／間隔（秒）", _delay);
        Field(fields, "", _repeat); Field(fields, "", _enabled);
        var buttons = new FlowLayoutPanel { Dock = DockStyle.Bottom, AutoSize = true, Padding = new Padding(8) };
        void Edit(bool add)
        {
            if (!add && _list.SelectedIndex < 0) return;
            int index = _list.SelectedIndex;
            using var dialog = new ScenarioActionDialog(add ? new ScenarioAction(ScenarioActionKind.Message, "Welcome!") : _actions[index], aliases, en, unitName);
            if (dialog.ShowDialog(this) != DialogResult.OK) return;
            if (add) _actions.Add(dialog.Result!); else _actions[index] = dialog.Result!;
            RefreshList(en, add ? _actions.Count - 1 : index);
        }
        Button AddButton(string label, Action click) { var button = new Button { Text = label, AutoSize = true }; button.Click += (_, _) => click(); buttons.Controls.Add(button); return button; }
        _addAction = AddButton(en ? "Add action" : "新增動作", () => { if (_actions.Count < 32) Edit(true); });
        AddButton(en ? "Edit action" : "編輯動作", () => Edit(false));
        AddButton(en ? "Delete action" : "刪除動作", () => { int index = _list.SelectedIndex; if (index < 0) return; _actions.RemoveAt(index); RefreshList(en, Math.Min(index, _actions.Count - 1)); });
        _moveUp.Text = en ? "Move up" : "上移"; _moveDown.Text = en ? "Move down" : "下移";
        void Move(int offset)
        {
            int index = _list.SelectedIndex, target = index + offset;
            if (index < 0 || target < 0 || target >= _actions.Count) return;
            (_actions[index], _actions[target]) = (_actions[target], _actions[index]);
            RefreshList(en, target);
        }
        _moveUp.Click += (_, _) => Move(-1); _moveDown.Click += (_, _) => Move(1);
        buttons.Controls.AddRange([_moveUp, _moveDown]);
        var save = AddButton(en ? "OK" : "確定", () =>
        {
            try
            {
                var result = new ScenarioEvent(_name.Text.Trim(), (int)_delay.Value, _repeat.Checked, _enabled.Checked)
                    { Actions = _actions.ToList(), Conditions = _conditions.ToList() };
                ScenarioEventValidator.Validate([result], aliases); Result = result; DialogResult = DialogResult.OK;
            }
            catch (Exception ex) { MessageBox.Show(this, ex.Message, Text, MessageBoxButtons.OK, MessageBoxIcon.Warning); }
        });
        var cancel = AddButton(en ? "Cancel" : "取消", () => DialogResult = DialogResult.Cancel);
        AcceptButton = save; CancelButton = cancel;
        _list.DoubleClick += (_, _) => Edit(false);
        _list.SelectedIndexChanged += (_, _) => UpdateActionButtons();
        var tabs = new TabControl { Dock = DockStyle.Fill };
        var actionsTab = new TabPage(en ? "Actions" : "動作"); actionsTab.Controls.Add(_list);
        var conditionsTab = new TabPage(en ? "Conditions (all)" : "條件（全部成立）");
        var conditionCommands = new FlowLayoutPanel { Dock = DockStyle.Bottom, AutoSize = true };
        Button ConditionButton(string label, Action action) { var button = new Button { Text = label, AutoSize = true }; button.Click += (_, _) => action(); conditionCommands.Controls.Add(button); return button; }
        Button? addCondition = null;
        void RefreshConditions(int selected = -1)
        {
            _conditionList.Items.Clear();
            foreach (ScenarioCondition condition in _conditions)
            {
                ScenarioSpawn? target = targets.FirstOrDefault(spawn => spawn.Id == condition.TargetId);
                string label = target is null ? (en ? "Missing target" : "目標已刪除") : $"{unitName?.Invoke(target.Alias) ?? target.Alias} ({target.X:0}, {target.Z:0})";
                string kindLabel = condition.Kind switch
                {
                    ScenarioConditionKind.ObjectExists => en ? "Exists" : "存在",
                    ScenarioConditionKind.ObjectDeadOrRemoved => en ? "Dead/removed" : "死亡／移除",
                    _ => $"{(en ? "Inside area" : "位於區域")} [{condition.MinX}, {condition.MinZ}]–[{condition.MaxX}, {condition.MaxZ}]"
                };
                _conditionList.Items.Add($"{kindLabel}: {label}");
            }
            if (selected >= 0 && selected < _conditions.Count) _conditionList.SelectedIndex = selected;
            if (addCondition is not null) addCondition.Enabled = _conditions.Count < 32 && targets.Count > 0;
        }
        void EditCondition(bool add)
        {
            int index = _conditionList.SelectedIndex;
            if (add ? _conditions.Count >= 32 : index < 0) return;
            using var dialog = new ScenarioConditionDialog(add ? null : _conditions[index], targets, en, unitName);
            if (dialog.ShowDialog(this) != DialogResult.OK) return;
            if (add) _conditions.Add(dialog.Result!); else _conditions[index] = dialog.Result!;
            RefreshConditions(add ? _conditions.Count - 1 : index);
        }
        addCondition = ConditionButton(en ? "Add condition" : "新增條件", () => EditCondition(true));
        ConditionButton(en ? "Edit condition" : "編輯條件", () => EditCondition(false));
        ConditionButton(en ? "Delete condition" : "刪除條件", () => { int index = _conditionList.SelectedIndex; if (index < 0) return; _conditions.RemoveAt(index); RefreshConditions(Math.Min(index, _conditions.Count - 1)); });
        _conditionList.DoubleClick += (_, _) => EditCondition(false);
        conditionsTab.Controls.Add(_conditionList); conditionsTab.Controls.Add(new Label { Dock = DockStyle.Top, Height = 46, Text = en
            ? "All conditions must hold when the timer is due. Empty conditions use only the timer. Dead/removed targets are tracked before the timer is due."
            : "計時到期且所有條件成立才執行。沒有條件時只依計時。死亡／移除目標在計時到期前也持續追蹤。" });
        conditionsTab.Controls.Add(conditionCommands); tabs.TabPages.AddRange([actionsTab, conditionsTab]);
        Controls.Add(tabs); Controls.Add(fields); Controls.Add(buttons); RefreshList(en); RefreshConditions(); WinFormsTheme.Apply(this);
    }

    private void RefreshList(bool en, int selected = -1)
    {
        _list.BeginUpdate();
        _list.Items.Clear();
        foreach (ScenarioAction action in _actions)
            _list.Items.Add(action.Kind switch
            {
                ScenarioActionKind.Message => $"{(en ? "Message" : "訊息")}: {action.Text}",
                ScenarioActionKind.Diplomacy => $"{(en ? "Diplomacy" : "外交")}: {action.Team} → {action.OtherTeam} {(action.Hostile ? (en ? "hostile" : "敵對") : (en ? "peace" : "和平"))}",
                ScenarioActionKind.Victory => en ? "Victory (end mission)" : "勝利（結束任務）",
                ScenarioActionKind.Defeat => en ? "Defeat (end mission)" : "失敗（結束任務）",
                _ => $"{(en ? "Spawn" : "生成部隊")}: {action.Alias} × {action.Count}, {(en ? "team" : "隊伍")} {action.Team} ({action.X}, {action.Z})"
            });
        if (selected >= 0 && selected < _list.Items.Count) _list.SelectedIndex = selected;
        _list.EndUpdate(); UpdateActionButtons();
    }

    private void UpdateActionButtons()
    {
        _addAction.Enabled = _actions.Count < 32;
        _moveUp.Enabled = _list.SelectedIndex > 0;
        _moveDown.Enabled = _list.SelectedIndex >= 0 && _list.SelectedIndex < _actions.Count - 1;
    }

    internal static void Field(TableLayoutPanel table, string title, Control control)
    {
        int row = table.RowCount++;
        table.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        table.Controls.Add(new Label { Text = title, AutoSize = true, Padding = new Padding(0, 6, 0, 6) }, 0, row);
        table.Controls.Add(control, 1, row);
    }
}

internal sealed class ScenarioActionDialog : Form
{
    private sealed record UnitChoice(string Alias, string Label) { public override string ToString() => Label; }
    internal ScenarioAction? Result { get; private set; }

    internal ScenarioActionDialog(ScenarioAction item, IReadOnlyCollection<string> aliases, bool en, Func<string, string>? unitName = null)
    {
        Text = en ? "Edit action" : "編輯動作"; StartPosition = FormStartPosition.CenterParent;
        Size = new Size(590, 500); MinimumSize = new Size(520, 470);
        var fields = new TableLayoutPanel { Dock = DockStyle.Fill, AutoScroll = true, ColumnCount = 2, Padding = new Padding(12) };
        fields.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 110)); fields.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        var kind = new ComboBox { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDownList };
        kind.Items.AddRange(en ? ["Message", "Diplomacy", "Spawn units", "Victory (end mission)", "Defeat (end mission)"] : ["顯示訊息", "改變外交", "生成部隊", "勝利（結束任務）", "失敗（結束任務）"]);
        kind.SelectedIndex = (int)item.Kind;
        var text = new TextBox { Dock = DockStyle.Fill, Multiline = true, Height = 70, ScrollBars = ScrollBars.Vertical, Text = item.Text };
        NumericUpDown Number(decimal maximum, decimal value, decimal minimum = 0) => new() { Dock = DockStyle.Fill, Minimum = minimum, Maximum = maximum, Value = Math.Clamp(value, minimum, maximum) };
        var team = Number(7, item.Team); var other = Number(7, item.OtherTeam);
        var hostile = new CheckBox { Text = en ? "Hostile (unchecked: peace)" : "敵對（取消勾選為和平）", Checked = item.Hostile, AutoSize = true };
        var alias = new ComboBox { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDownList };
        alias.Items.AddRange(aliases.Select(name => new UnitChoice(name, unitName?.Invoke(name) ?? name)).Cast<object>().ToArray());
        alias.SelectedItem = alias.Items.Cast<UnitChoice>().FirstOrDefault(choice => choice.Alias == item.Alias);
        if (alias.SelectedIndex < 0 && alias.Items.Count > 0) alias.SelectedIndex = 0;
        var x = Number(16383, float.IsFinite(item.X) ? (decimal)item.X : 0); var z = Number(16383, float.IsFinite(item.Z) ? (decimal)item.Z : 0);
        var count = Number(20, item.Count, 1);
        ScenarioEventDialog.Field(fields, en ? "Action" : "動作", kind);
        ScenarioEventDialog.Field(fields, en ? "Message" : "訊息", text);
        ScenarioEventDialog.Field(fields, en ? "Team (0: player)" : "隊伍（0：玩家）", team);
        ScenarioEventDialog.Field(fields, en ? "Other team" : "另一隊伍", other);
        ScenarioEventDialog.Field(fields, "", hostile);
        ScenarioEventDialog.Field(fields, en ? "Unit type" : "部隊種類", alias);
        ScenarioEventDialog.Field(fields, "X", x); ScenarioEventDialog.Field(fields, "Z", z);
        ScenarioEventDialog.Field(fields, en ? "Members" : "人數", count);
        ScenarioEventDialog.Field(fields, "", new Label { AutoSize = true, MaximumSize = new Size(350, 0), Text = en ? "Messages use the game's CP1251 encoding. Unit positions are world coordinates. Victory/defeat must be the last action of a nonrepeating event." : "訊息需使用遊戲支援的 CP1251 文字。部隊位置使用世界座標。勝敗必須是單次事件的最後一個動作。" });
        void UpdateFields() { text.Enabled = kind.SelectedIndex == 0; team.Enabled = kind.SelectedIndex is 1 or 2; other.Enabled = hostile.Enabled = kind.SelectedIndex == 1; alias.Enabled = x.Enabled = z.Enabled = count.Enabled = kind.SelectedIndex == 2; }
        kind.SelectedIndexChanged += (_, _) => UpdateFields(); UpdateFields();
        var commands = new FlowLayoutPanel { Dock = DockStyle.Bottom, AutoSize = true, Padding = new Padding(8) };
        var save = new Button { Text = en ? "OK" : "確定", AutoSize = true };
        var cancel = new Button { Text = en ? "Cancel" : "取消", AutoSize = true, DialogResult = DialogResult.Cancel };
        save.Click += (_, _) =>
        {
            try
            {
                var result = new ScenarioAction((ScenarioActionKind)kind.SelectedIndex, text.Text, (int)team.Value, (int)other.Value, hostile.Checked,
                    (alias.SelectedItem as UnitChoice)?.Alias ?? "", (float)x.Value, (float)z.Value, (int)count.Value);
                ScenarioEventValidator.Validate([new ScenarioEvent("action") { Actions = [result] }], aliases);
                Result = result; DialogResult = DialogResult.OK;
            }
            catch (Exception ex) { MessageBox.Show(this, ex.Message, Text, MessageBoxButtons.OK, MessageBoxIcon.Warning); }
        };
        commands.Controls.AddRange([save, cancel]); Controls.Add(fields); Controls.Add(commands); AcceptButton = save; CancelButton = cancel; WinFormsTheme.Apply(this);
    }
}
