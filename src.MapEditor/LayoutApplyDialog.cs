using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeModifier;

namespace AgainstRomeMapEditor;

internal sealed class LayoutApplyDialog : Form
{
    private readonly NumericUpDown _x = new() { Minimum = 0, Maximum = 63.99m, DecimalPlaces = 2, Value = 20, Dock = DockStyle.Fill };
    private readonly NumericUpDown _z = new() { Minimum = 0, Maximum = 63.99m, DecimalPlaces = 2, Value = 20, Dock = DockStyle.Fill };
    private readonly NumericUpDown _rotation = new() { Minimum = 0, Maximum = 359, Increment = 45, Dock = DockStyle.Fill };
    private readonly CheckBox _overrideTeam = new() { AutoSize = true };
    private readonly NumericUpDown _team = new() { Minimum = -1, Maximum = 15, Enabled = false, Dock = DockStyle.Fill };
    internal float AnchorX => (float)_x.Value * 256;
    internal float AnchorZ => (float)_z.Value * 256;
    internal float Rotation => (float)_rotation.Value;
    internal int? Team => _overrideTeam.Checked ? (int)_team.Value : null;
    internal void SetTransform(float worldX, float worldZ, float rotation, int? team = null)
    {
        _x.Value = (decimal)worldX / 256; _z.Value = (decimal)worldZ / 256; _rotation.Value = (decimal)rotation;
        _overrideTeam.Checked = team.HasValue; if (team.HasValue) _team.Value = team.Value;
    }
    public LayoutApplyDialog(MapLayoutPreset preset)
    {
        bool en = Loc.CurrentLanguage == Language.English;
        Text = en ? "Apply layout" : "套用配置"; ClientSize = new Size(440, 285);
        AutoScaleMode = AutoScaleMode.Dpi; AutoScaleDimensions = new SizeF(96, 96);
        FormBorderStyle = FormBorderStyle.FixedDialog; MinimizeBox = MaximizeBox = false; StartPosition = FormStartPosition.CenterParent;
        var grid = new TableLayoutPanel { Dock = DockStyle.Fill, Padding = new Padding(12), ColumnCount = 2, RowCount = 6 };
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50)); grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 50));
        grid.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        var hint = new Label { Dock = DockStyle.Fill, Text = en ? $"{preset.Kind}: {preset.Entries.Count} objects. Anchor uses tile coordinates. Rotates around the saved origin; heights follow the terrain. Entire layout is rejected if any type or position is invalid."
            : $"配置共 {preset.Entries.Count} 個物件。錨點使用圖格座標；繞保存的原點旋轉，高度貼合地形。類型缺失或位置越界時整批拒絕。" };
        grid.Controls.Add(hint, 0, 0); grid.SetColumnSpan(hint, 2);
        string[] labels = en ? ["Anchor X", "Anchor Z", "Rotation (degrees)"] : ["錨點 X", "錨點 Z", "旋轉（度）"];
        Control[] controls = [_x, _z, _rotation];
        for (int i = 0; i < controls.Length; i++) { grid.RowStyles.Add(new RowStyle(SizeType.AutoSize)); grid.Controls.Add(new Label { Text = labels[i], AutoSize = true }, 0, i + 1); grid.Controls.Add(controls[i], 1, i + 1); }
        _overrideTeam.Text = en ? "Override team" : "指定隊伍"; _overrideTeam.Enabled = preset.Kind == MapLayoutKind.Placement;
        _overrideTeam.CheckedChanged += (_, _) => _team.Enabled = _overrideTeam.Checked;
        grid.RowStyles.Add(new RowStyle(SizeType.AutoSize)); grid.Controls.Add(_overrideTeam, 0, 4); grid.Controls.Add(_team, 1, 4);
        var buttons = new FlowLayoutPanel { AutoSize = true, Dock = DockStyle.Fill, FlowDirection = FlowDirection.RightToLeft };
        var apply = new Button { AutoSize = true, Text = en ? "Apply" : "套用", DialogResult = DialogResult.OK };
        var cancel = new Button { AutoSize = true, Text = en ? "Cancel" : "取消", DialogResult = DialogResult.Cancel };
        buttons.Controls.AddRange([apply, cancel]); grid.RowStyles.Add(new RowStyle(SizeType.AutoSize)); grid.Controls.Add(buttons, 0, 5); grid.SetColumnSpan(buttons, 2);
        Controls.Add(grid); AcceptButton = apply; CancelButton = cancel; WinFormsTheme.Apply(this);
    }
}
