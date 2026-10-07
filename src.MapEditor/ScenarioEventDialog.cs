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
    private readonly List<ScenarioAction> _actions;
    internal ScenarioEvent? Result { get; private set; }

    internal ScenarioEventDialog(ScenarioEvent item, IReadOnlyCollection<string> aliases, bool en, Func<string, string>? unitName = null)
    {
        Text = en ? "Edit event" : "編輯事件"; StartPosition = FormStartPosition.CenterParent;
        Size = new Size(650, 480); MinimumSize = new Size(560, 420);
        _name.Text = item.Name; _delay.Value = Math.Clamp(item.DelaySeconds, 0, 86400);
        _repeat.Text = en ? "Repeat" : "重複執行"; _repeat.Checked = item.Repeat;
        _enabled.Text = en ? "Enabled" : "啟用"; _enabled.Checked = item.Enabled;
        _actions = item.Actions.ToList();
        var fields = new TableLayoutPanel { Dock = DockStyle.Top, AutoSize = true, ColumnCount = 2, Padding = new Padding(10) };
        fields.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 110)); fields.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        Field(fields, en ? "Name" : "名稱", _name); Field(fields, en ? "Timer (seconds)" : "計時（秒）", _delay);
        Field(fields, "", _repeat); Field(fields, "", _enabled);
        var buttons = new FlowLayoutPanel { Dock = DockStyle.Bottom, AutoSize = true, Padding = new Padding(8) };
        void Edit(bool add)
        {
            if (!add && _list.SelectedIndex < 0) return;
            int index = _list.SelectedIndex;
            using var dialog = new ScenarioActionDialog(add ? new ScenarioAction(ScenarioActionKind.Message, "Welcome!") : _actions[index], aliases, en, unitName);
            if (dialog.ShowDialog(this) != DialogResult.OK) return;
            if (add) _actions.Add(dialog.Result!); else _actions[index] = dialog.Result!;
            RefreshList(en);
        }
        Button AddButton(string label, Action click) { var button = new Button { Text = label, AutoSize = true }; button.Click += (_, _) => click(); buttons.Controls.Add(button); return button; }
        AddButton(en ? "Add action" : "新增動作", () => Edit(true));
        AddButton(en ? "Edit action" : "編輯動作", () => Edit(false));
        AddButton(en ? "Delete action" : "刪除動作", () => { if (_list.SelectedIndex < 0) return; _actions.RemoveAt(_list.SelectedIndex); RefreshList(en); });
        var save = AddButton(en ? "OK" : "確定", () =>
        {
            try
            {
                var result = new ScenarioEvent(_name.Text.Trim(), (int)_delay.Value, _repeat.Checked, _enabled.Checked) { Actions = _actions.ToList() };
                ScenarioEventValidator.Validate([result], aliases); Result = result; DialogResult = DialogResult.OK;
            }
            catch (Exception ex) { MessageBox.Show(this, ex.Message, Text, MessageBoxButtons.OK, MessageBoxIcon.Warning); }
        });
        var cancel = AddButton(en ? "Cancel" : "取消", () => DialogResult = DialogResult.Cancel);
        AcceptButton = save; CancelButton = cancel;
        _list.DoubleClick += (_, _) => Edit(false);
        Controls.Add(_list); Controls.Add(fields); Controls.Add(buttons); RefreshList(en); WinFormsTheme.Apply(this);
    }

    private void RefreshList(bool en)
    {
        _list.Items.Clear();
        foreach (ScenarioAction action in _actions)
            _list.Items.Add(action.Kind switch
            {
                ScenarioActionKind.Message => $"{(en ? "Message" : "訊息")}: {action.Text}",
                ScenarioActionKind.Diplomacy => $"{(en ? "Diplomacy" : "外交")}: {action.Team} → {action.OtherTeam} {(action.Hostile ? (en ? "hostile" : "敵對") : (en ? "peace" : "和平"))}",
                _ => $"{(en ? "Spawn" : "生成部隊")}: {action.Alias} × {action.Count}, {(en ? "team" : "隊伍")} {action.Team} ({action.X}, {action.Z})"
            });
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
        kind.Items.AddRange(en ? ["Message", "Diplomacy", "Spawn units"] : ["顯示訊息", "改變外交", "生成部隊"]);
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
        ScenarioEventDialog.Field(fields, "", new Label { AutoSize = true, MaximumSize = new Size(350, 0), Text = en ? "Messages use the game's CP1251 encoding. Unit positions are world coordinates." : "訊息需使用遊戲支援的 CP1251 文字。部隊位置使用世界座標。" });
        void UpdateFields() { text.Enabled = kind.SelectedIndex == 0; team.Enabled = kind.SelectedIndex != 0; other.Enabled = hostile.Enabled = kind.SelectedIndex == 1; alias.Enabled = x.Enabled = z.Enabled = count.Enabled = kind.SelectedIndex == 2; }
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
