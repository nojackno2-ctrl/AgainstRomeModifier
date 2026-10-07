using AgainstRomeModifier;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor;

internal sealed class ScenarioConditionDialog : Form
{
    private sealed record Choice(Guid Id, string Label) { public override string ToString() => Label; }
    internal ScenarioCondition? Result { get; private set; }

    internal ScenarioConditionDialog(ScenarioCondition? item, IReadOnlyList<ScenarioSpawn> targets, bool en,
        Func<string, string>? objectName = null)
    {
        AutoScaleDimensions = new SizeF(96F, 96F); AutoScaleMode = AutoScaleMode.Dpi; // 以 96 DPI 設計，PerMonitorV2 下依實際 DPI 縮放固定像素版面
        Text = en ? "Object condition" : "物件條件"; Size = new Size(620, 440);
        StartPosition = FormStartPosition.CenterParent; MinimumSize = Size;
        var fields = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 2, Padding = new Padding(12) };
        fields.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 105)); fields.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        var kind = new ComboBox { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDownList };
        kind.Items.AddRange(en ? ["Object exists", "Object dead or removed (previously seen)", "Object inside rectangle"] : ["物件存在", "物件死亡或移除（曾確認存在）", "物件位於矩形區域"]);
        kind.SelectedIndex = item is null ? 0 : (int)item.Kind;
        var target = new ComboBox { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDownList };
        target.Items.AddRange(targets.Where(spawn => spawn.Id != Guid.Empty).Select(spawn => new Choice(spawn.Id,
            $"{objectName?.Invoke(spawn.Alias) ?? spawn.Alias} — {(en ? "team" : "隊伍")} {spawn.Team} ({spawn.X:0}, {spawn.Z:0}) [{spawn.Id.ToString("N")[..8]}]")).Cast<object>().ToArray());
        if (item is not null)
        {
            target.SelectedItem = target.Items.Cast<Choice>().FirstOrDefault(choice => choice.Id == item.TargetId);
            if (target.SelectedIndex < 0)
            {
                var missing = new Choice(item.TargetId, en ? "Missing target — select another object" : "目標已刪除—請重新選擇物件");
                target.Items.Add(missing); target.SelectedItem = missing;
            }
        }
        else if (target.Items.Count > 0) target.SelectedIndex = 0;
        ScenarioEventDialog.Field(fields, en ? "Condition" : "條件", kind);
        ScenarioEventDialog.Field(fields, en ? "Placed object" : "放置物件", target);
        NumericUpDown Coordinate(int value) => new() { Dock = DockStyle.Fill, Maximum = 16383, Value = Math.Clamp(value, 0, 16383) };
        var minX = Coordinate(item?.MinX ?? 0); var minZ = Coordinate(item?.MinZ ?? 0);
        var maxX = Coordinate(item?.MaxX ?? 16383); var maxZ = Coordinate(item?.MaxZ ?? 16383);
        ScenarioEventDialog.Field(fields, en ? "Minimum X" : "最小 X", minX);
        ScenarioEventDialog.Field(fields, en ? "Maximum X" : "最大 X", maxX);
        ScenarioEventDialog.Field(fields, en ? "Minimum Z" : "最小 Z", minZ);
        ScenarioEventDialog.Field(fields, en ? "Maximum Z" : "最大 Z", maxZ);
        void UpdateArea() => minX.Enabled = minZ.Enabled = maxX.Enabled = maxZ.Enabled = kind.SelectedIndex == 2;
        kind.SelectedIndexChanged += (_, _) => UpdateArea(); UpdateArea();
        ScenarioEventDialog.Field(fields, "", new Label { AutoSize = true, MaximumSize = new Size(430, 0), Text = en
            ? "Existence includes corpses still present. Dead/removed requires confirmed existence; a failed spawn does not qualify. Rectangle boundaries are inclusive world X/Z coordinates; troop position refers to its container, not each member."
            : "存在包含尚未移除的屍體。死亡／移除需要先確認存在；建立失敗不算死亡。矩形邊界包含端點，使用世界 X/Z 座標；部隊位置指部隊容器，並非每個成員。" });
        var commands = new FlowLayoutPanel { Dock = DockStyle.Bottom, AutoSize = true, Padding = new Padding(8) };
        var save = new Button { Text = en ? "OK" : "確定", AutoSize = true };
        var cancel = new Button { Text = en ? "Cancel" : "取消", AutoSize = true, DialogResult = DialogResult.Cancel };
        save.Click += (_, _) =>
        {
            if (kind.SelectedIndex < 0 || target.SelectedItem is not Choice choice || !targets.Any(spawn => spawn.Id == choice.Id))
            {
                MessageBox.Show(this, en ? "Select an existing placed object." : "請選擇仍存在的放置物件。", Text); return;
            }
            var result = new ScenarioCondition((ScenarioConditionKind)kind.SelectedIndex, choice.Id,
                (int)minX.Value, (int)minZ.Value, (int)maxX.Value, (int)maxZ.Value);
            try { ScenarioEventValidator.ValidateConditions([new("condition") { Conditions = [result] }]); }
            catch (InvalidDataException ex) { MessageBox.Show(this, ex.Message, Text); return; }
            Result = result; DialogResult = DialogResult.OK;
        };
        commands.Controls.AddRange([save, cancel]); Controls.Add(fields); Controls.Add(commands);
        AcceptButton = save; CancelButton = cancel; WinFormsTheme.Apply(this);
    }
}
