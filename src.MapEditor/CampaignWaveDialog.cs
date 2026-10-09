using AgainstRomeMapEditor.Modules.AI;
using AgainstRomeModifier;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor;

internal sealed class CampaignWaveDialog : Form
{
    private readonly ComboBox _archetype = new() { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDownList };
    private readonly NumericUpDown _waves = Number(1, 256, 3);
    private readonly NumericUpDown _firstDelay = Number(0, 86400, 60);
    private readonly NumericUpDown _interval = Number(1, 86400, 60);
    private readonly NumericUpDown _count = Number(1, 20, 10);
    private readonly NumericUpDown _team = Number(0, 7, 1);
    private readonly NumericUpDown _x = Number(0, 16383, 8000);
    private readonly NumericUpDown _z = Number(0, 16383, 8000);
    private readonly TextBox _preview = new() { Dock = DockStyle.Fill, Multiline = true, ReadOnly = true, ScrollBars = ScrollBars.Vertical };
    private readonly Button _apply = new() { AutoSize = true };
    private readonly string[] _aliases;
    private readonly ScenarioDocument _scenario;
    private readonly bool _en;

    internal string ArchetypeId
    {
        get => ((AiArchetypeProfile)_archetype.SelectedItem!).Id;
        set => _archetype.SelectedItem = _archetype.Items.Cast<AiArchetypeProfile>().Single(p => p.Id == value);
    }
    internal int WaveCount { get => (int)_waves.Value; set => _waves.Value = value; }
    internal int FirstDelaySeconds { get => (int)_firstDelay.Value; set => _firstDelay.Value = value; }
    internal int IntervalSeconds { get => (int)_interval.Value; set => _interval.Value = value; }
    internal int SpawnX { get => (int)_x.Value; set => _x.Value = value; }
    internal int SpawnZ { get => (int)_z.Value; set => _z.Value = value; }
    internal int SquadCount { get => (int)_count.Value; set => _count.Value = value; }
    internal CampaignCompilationResult PreviewResult { get; private set; } = new(false, [], []);
    internal string PreviewText => _preview.Text;
    internal bool CanApply => _apply.Enabled;

    // Archetypes supply unit composition only; do not pass unsupported faction AI settings to the compiler.
    internal CampaignMissionPlan Plan => new("Campaign waves", "", [],
        Enumerable.Range(1, WaveCount).Select(i => new WaveAttackDefinition(i,
            FirstDelaySeconds + (i - 1) * IntervalSeconds, "", (float)_x.Value, (float)_z.Value,
            ((AiArchetypeProfile)_archetype.SelectedItem!).UnitPreferences.Select(p =>
                new WaveSquadDefinition(p.Alias, SquadCount, (int)_team.Value)).ToArray())).ToArray(), [], []);

    public CampaignWaveDialog(IReadOnlyCollection<string> aliases, ScenarioDocument scenario)
    {
        _aliases = aliases.ToArray();
        _scenario = scenario;
        _en = Loc.CurrentLanguage == Language.English;
        Text = T("AI 戰役波次企劃", "AI Campaign Wave Planner");
        AutoScaleDimensions = new SizeF(96, 96);
        AutoScaleMode = AutoScaleMode.Dpi;
        StartPosition = FormStartPosition.CenterParent;
        MinimizeBox = MaximizeBox = false;
        ClientSize = new Size(640, 640);
        MinimumSize = new Size(480, 560);
        var grid = new TableLayoutPanel { Dock = DockStyle.Fill, Padding = new Padding(12), ColumnCount = 2, RowCount = 11 };
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 40));
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 60));
        var hint = new Label { AutoSize = true, Dock = DockStyle.Fill, Text = T(
            "原型僅用於選擇兵種。只生成定時部隊；巡邏、指定攻擊目標、戰術、經濟與擴張尚不支援。時間為開局後的秒數，人數為每兵種每波的人數。套用後可復原，儲存才寫入；遊戲內行為尚未驗證。",
            "Archetypes select unit types only. Timed spawning is supported; patrol, attack targets, tactics, economy and expansion are unsupported. Times are seconds after mission start; count is per unit type per wave. Undo is available; Save writes the changes. In-game behavior is unverified.") };
        hint.MaximumSize = new Size(LogicalToDeviceUnits(600), 0); // wrap at a bounded width so the auto-sized row grows with the text
        grid.Controls.Add(hint, 0, 0); grid.SetColumnSpan(hint, 2);
        _archetype.Items.AddRange(AiArchetypeCatalog.All.Cast<object>().ToArray());
        _archetype.FormattingEnabled = true;
        _archetype.Format += (_, e) => { if (e.ListItem is AiArchetypeProfile p) e.Value = _en ? p.Id : p.DisplayName; };
        _archetype.SelectedIndex = 0;
        AddRow(1, T("原型", "Archetype"), _archetype);
        AddRow(2, T("波次數", "Waves"), _waves);
        AddRow(3, T("首波時間（秒）", "First wave (seconds)"), _firstDelay);
        AddRow(4, T("波次間隔（秒）", "Interval (seconds)"), _interval);
        AddRow(5, T("每兵種人數", "Count per unit type"), _count);
        AddRow(6, T("隊伍（0–7）", "Team (0–7)"), _team);
        AddRow(7, T("生成點 X", "Spawn X"), _x);
        AddRow(8, T("生成點 Z", "Spawn Z"), _z);
        grid.Controls.Add(new Label { Text = T("預覽與驗證", "Preview and validation"), AutoSize = true }, 0, 9);
        grid.SetColumnSpan(grid.GetControlFromPosition(0, 9)!, 2);
        grid.Controls.Add(_preview, 0, 10); grid.SetColumnSpan(_preview, 2);
        for (int i = 0; i < 10; i++) grid.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        grid.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        var buttons = new FlowLayoutPanel { Dock = DockStyle.Bottom, AutoSize = true, FlowDirection = FlowDirection.RightToLeft, Padding = new Padding(8) };
        var cancel = new Button { Text = T("取消", "Cancel"), AutoSize = true, DialogResult = DialogResult.Cancel };
        _apply.Text = T("合併事件", "Merge events");
        _apply.Click += (_, _) => { RefreshPreview(); if (CanApply) DialogResult = DialogResult.OK; };
        buttons.Controls.AddRange([cancel, _apply]);
        Controls.Add(grid); Controls.Add(buttons);
        AcceptButton = _apply; CancelButton = cancel;
        _archetype.SelectedIndexChanged += (_, _) => RefreshPreview();
        foreach (var number in new[] { _waves, _firstDelay, _interval, _count, _team, _x, _z }) number.ValueChanged += (_, _) => RefreshPreview();
        WinFormsTheme.Apply(this);
        RefreshPreview();

        void AddRow(int row, string label, Control input)
        {
            grid.Controls.Add(new Label { Text = label, AutoSize = true, Anchor = AnchorStyles.Left }, 0, row);
            grid.Controls.Add(input, 1, row);
        }
    }

    internal void RefreshPreview()
    {
        PreviewResult = CompileForMerge(Plan, _aliases, _scenario);
        _apply.Enabled = PreviewResult.Success;
        _preview.Text = string.Join(Environment.NewLine,
            PreviewResult.Diagnostics.Concat(PreviewResult.Success ? PreviewResult.CompiledEvents.Select(e =>
                $"{e.Name}: {e.DelaySeconds}s — " + string.Join(", ", e.Actions.Select(a => $"{a.Alias} × {a.Count} (team {a.Team}, {a.X}, {a.Z})"))) : []));
    }

    internal static CampaignCompilationResult CompileForMerge(CampaignMissionPlan plan, IReadOnlyCollection<string> aliases, ScenarioDocument scenario)
    {
        var result = CampaignMissionCompiler.Compile(plan, null, aliases, scenario);
        if (!result.Success) return result;
        try
        {
            var merged = scenario.Events.Concat(result.CompiledEvents).ToArray();
            ScenarioEventValidator.Validate(merged, aliases);
            ScenarioEventValidator.ValidateConditions(merged, scenario);
            return result;
        }
        catch (InvalidDataException ex)
        {
            return new(false, [], result.Diagnostics.Concat(new[] { "[Error] " + ex.Message }).ToArray());
        }
    }
    private string T(string zh, string en) => _en ? en : zh;
    private static NumericUpDown Number(int min, int max, int value) => new() { Minimum = min, Maximum = max, Value = value, Dock = DockStyle.Fill };
}
