using AgainstRomeMapEditor.Modules.Settlement;
using AgainstRomeModifier;

namespace AgainstRomeMapEditor;

internal sealed class SettlementGeneratorDialog : Form
{
    private readonly NumericUpDown _playerCount = new() { Minimum = 2, Maximum = 8, Value = 2, Dock = DockStyle.Fill };
    private readonly ComboBox _tribe = new() { DropDownStyle = ComboBoxStyle.DropDownList, Dock = DockStyle.Fill };
    private readonly NumericUpDown _seed = new() { Minimum = 0, Maximum = 999999, Value = 42, Dock = DockStyle.Fill };
    private readonly CheckBox _includeNature = new() { AutoSize = true, Checked = true };
    private readonly Button _randomSeedButton = new() { AutoSize = true };

    internal int PlayerCount
    {
        get => (int)_playerCount.Value;
        set => _playerCount.Value = Math.Clamp(value, 2, 8);
    }

    internal SettlementTribe SelectedTribe
    {
        get => (SettlementTribe)Math.Clamp(_tribe.SelectedIndex, 0, 3);
        set => _tribe.SelectedIndex = (int)value;
    }

    internal int Seed
    {
        get => (int)_seed.Value;
        set => _seed.Value = Math.Clamp(value, 0, 999999);
    }

    internal bool IncludeNature
    {
        get => _includeNature.Checked;
        set => _includeNature.Checked = value;
    }

    public SettlementGeneratorDialog()
    {
        bool en = Loc.CurrentLanguage == Language.English;
        Text = en ? "One-Click Settlement / Base Generator" : "一鍵生成對戰基地";
        AutoScaleMode = AutoScaleMode.Dpi;
        AutoScaleDimensions = new SizeF(96, 96);
        StartPosition = FormStartPosition.CenterParent;
        FormBorderStyle = FormBorderStyle.FixedDialog;
        MinimizeBox = MaximizeBox = false;
        ClientSize = new Size(460, 270);

        _tribe.Items.AddRange(en
            ? new object[] { "Germanic (日耳曼)", "Roman (羅馬)", "Celtic (塞爾特)", "Hun (匈人)" }
            : new object[] { "日耳曼 (Germanic)", "羅馬 (Roman)", "塞爾特 (Celtic)", "匈人 (Hun)" });
        _tribe.SelectedIndex = 0;

        _includeNature.Text = en ? "Generate Forests & Quarries (Nature Objects)" : "生成周邊森林與採石場（自然地景物件）";
        _randomSeedButton.Text = en ? "Randomize" : "隨機種子";
        _randomSeedButton.Click += (_, _) => _seed.Value = Random.Shared.Next(1, 100000);

        var grid = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            Padding = new Padding(12),
            ColumnCount = 3,
            RowCount = 5
        };
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 35));
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 45));
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 20));

        var hint = new Label
        {
            Dock = DockStyle.Fill,
            AutoSize = true,
            Text = en
                ? "Evaluates terrain flatness and water clearance, generating symmetric bases with main houses and resources. Single Undo supported."
                : "自動評估地形平坦度與水體距離，為各勢力生成對稱平衡的基地與資源（支援單步復原）。"
        };
        grid.Controls.Add(hint, 0, 0);
        grid.SetColumnSpan(hint, 3);

        grid.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        grid.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        grid.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        grid.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        grid.RowStyles.Add(new RowStyle(SizeType.AutoSize));

        var lblPlayer = new Label { Text = en ? "Player Count:" : "玩家人數：", Dock = DockStyle.Fill, TextAlign = ContentAlignment.MiddleLeft };
        grid.Controls.Add(lblPlayer, 0, 1);
        grid.Controls.Add(_playerCount, 1, 1);
        grid.SetColumnSpan(_playerCount, 2);

        var lblTribe = new Label { Text = en ? "Tribe / Style:" : "部族風格：", Dock = DockStyle.Fill, TextAlign = ContentAlignment.MiddleLeft };
        grid.Controls.Add(lblTribe, 0, 2);
        grid.Controls.Add(_tribe, 1, 2);
        grid.SetColumnSpan(_tribe, 2);

        var lblSeed = new Label { Text = en ? "Seed:" : "隨機種子：", Dock = DockStyle.Fill, TextAlign = ContentAlignment.MiddleLeft };
        grid.Controls.Add(lblSeed, 0, 3);
        grid.Controls.Add(_seed, 1, 3);
        grid.Controls.Add(_randomSeedButton, 2, 3);

        grid.Controls.Add(_includeNature, 0, 4);
        grid.SetColumnSpan(_includeNature, 3);

        var buttons = new FlowLayoutPanel { Dock = DockStyle.Bottom, Height = 40, FlowDirection = FlowDirection.RightToLeft, Padding = new Padding(8, 4, 8, 4) };
        var cancel = new Button { Text = en ? "Cancel" : "取消", DialogResult = DialogResult.Cancel, AutoSize = true };
        var apply = new Button { Text = en ? "Generate" : "一鍵生成", DialogResult = DialogResult.OK, AutoSize = true };
        buttons.Controls.AddRange([cancel, apply]);

        Controls.Add(grid);
        Controls.Add(buttons);
        AcceptButton = apply;
        CancelButton = cancel;
        WinFormsTheme.Apply(this);
    }
}
