using AgainstRomeModifier;

namespace AgainstRomeMapEditor;

internal sealed class PlacementBatchEditDialog : Form
{
    private readonly CheckBox _applyTeam = new() { AutoSize = true };
    private readonly CheckBox _applyAngle = new() { AutoSize = true };
    private readonly NumericUpDown _team = new() { Dock = DockStyle.Fill, Enabled = false };
    private readonly NumericUpDown _angle = new() { Dock = DockStyle.Fill, Minimum = 0, Maximum = 359, Increment = 45, Enabled = false };

    internal int? Team
    {
        get => _applyTeam.Checked ? (int)_team.Value : null;
        set { _applyTeam.Checked = value.HasValue; if (value.HasValue) _team.Value = value.Value; }
    }
    internal float? Angle
    {
        get => _applyAngle.Checked ? (float)_angle.Value : null;
        set { _applyAngle.Checked = value.HasValue; if (value.HasValue) _angle.Value = (decimal)value.Value; }
    }

    public PlacementBatchEditDialog(int count, bool includesUnits)
    {
        bool en = Loc.CurrentLanguage == Language.English;
        Text = en ? "Set selected teams / directions" : "批次設定隊伍／方向";
        AutoScaleMode = AutoScaleMode.Dpi; AutoScaleDimensions = new SizeF(96, 96);
        StartPosition = FormStartPosition.CenterParent; FormBorderStyle = FormBorderStyle.FixedDialog;
        MinimizeBox = MaximizeBox = false; ClientSize = new Size(420, 220);
        _team.Minimum = includesUnits ? 0 : -1; _team.Maximum = includesUnits ? 7 : 15;
        _applyTeam.Text = en ? "Change team" : "設定隊伍"; _applyAngle.Text = en ? "Change direction" : "設定方向";
        var grid = new TableLayoutPanel { Dock = DockStyle.Fill, Padding = new Padding(12), ColumnCount = 2, RowCount = 4 };
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 55)); grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 45));
        var hint = new Label { Dock = DockStyle.Fill, AutoSize = true, Text = en
            ? $"{count} selected objects. Only checked settings change; position, count and event identity are preserved."
            : $"已選取 {count} 個物件。只修改勾選的設定；位置、人數及事件目標身份保留。" };
        grid.Controls.Add(hint, 0, 0); grid.SetColumnSpan(hint, 2);
        grid.RowStyles.Add(new RowStyle(SizeType.Percent, 50));
        grid.RowStyles.Add(new RowStyle(SizeType.AutoSize)); grid.RowStyles.Add(new RowStyle(SizeType.AutoSize)); grid.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        grid.Controls.Add(_applyTeam, 0, 1); grid.Controls.Add(_team, 1, 1);
        grid.Controls.Add(_applyAngle, 0, 2); grid.Controls.Add(_angle, 1, 2);
        var buttons = new FlowLayoutPanel { Dock = DockStyle.Fill, FlowDirection = FlowDirection.RightToLeft, AutoSize = true };
        var apply = new Button { Text = en ? "Apply" : "套用", DialogResult = DialogResult.OK, AutoSize = true, Enabled = false };
        var cancel = new Button { Text = en ? "Cancel" : "取消", DialogResult = DialogResult.Cancel, AutoSize = true };
        buttons.Controls.AddRange([apply, cancel]); grid.Controls.Add(buttons, 0, 3); grid.SetColumnSpan(buttons, 2);
        _applyTeam.CheckedChanged += (_, _) => { _team.Enabled = _applyTeam.Checked; apply.Enabled = _applyTeam.Checked || _applyAngle.Checked; };
        _applyAngle.CheckedChanged += (_, _) => { _angle.Enabled = _applyAngle.Checked; apply.Enabled = _applyTeam.Checked || _applyAngle.Checked; };
        Controls.Add(grid); AcceptButton = apply; CancelButton = cancel; WinFormsTheme.Apply(this);
    }
}
