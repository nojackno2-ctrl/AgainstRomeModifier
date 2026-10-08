using AgainstRomeMapEditor.Modules.Objectives;
using AgainstRomeModifier;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor;

internal sealed class ObjectiveStudioDialog : Form
{
    private sealed record SpawnChoice(Guid Id, string Label)
    {
        public override string ToString() => Label;
    }

    private sealed record KindChoice(ObjectiveKind Kind, string DisplayName)
    {
        public override string ToString() => DisplayName;
    }

    private sealed record CategoryChoice(ObjectiveCategory Category, string DisplayName)
    {
        public override string ToString() => DisplayName;
    }

    private readonly ComboBox _kindCombo = new() { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDownList };
    private readonly ComboBox _categoryCombo = new() { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDownList };
    private readonly TextBox _titleBox = new() { Dock = DockStyle.Fill };
    private readonly TextBox _descriptionBox = new() { Dock = DockStyle.Fill, Multiline = true, Height = 50, ScrollBars = ScrollBars.Vertical };
    private readonly ComboBox _targetCombo = new() { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDownList };
    private readonly NumericUpDown _holdDuration = Number(0, 86400, 0);
    private readonly NumericUpDown _minX = Coordinate(0);
    private readonly NumericUpDown _minZ = Coordinate(0);
    private readonly NumericUpDown _maxX = Coordinate(16383);
    private readonly NumericUpDown _maxZ = Coordinate(16383);
    private readonly TextBox _rewardMessageBox = new() { Dock = DockStyle.Fill };
    private readonly TextBox _preview = new() { Dock = DockStyle.Fill, Multiline = true, ReadOnly = true, ScrollBars = ScrollBars.Vertical };
    private readonly Button _apply = new() { AutoSize = true };

    private readonly ScenarioDocument _scenario;
    private readonly string[] _aliases;
    private readonly bool _en;

    // Public / internal properties exposed for callers & tests
    internal IReadOnlyList<ScenarioEvent> CompiledEvents { get; private set; } = [];
    internal IReadOnlyList<string> ValidationErrors { get; private set; } = [];
    internal bool CanApply => _apply.Enabled;
    internal string PreviewText => _preview.Text;

    internal ObjectiveKind SelectedKind
    {
        get => ((KindChoice)_kindCombo.SelectedItem!).Kind;
        set
        {
            var item = _kindCombo.Items.Cast<KindChoice>().FirstOrDefault(k => k.Kind == value);
            if (item != null) _kindCombo.SelectedItem = item;
        }
    }

    internal ObjectiveCategory SelectedCategory
    {
        get => ((CategoryChoice)_categoryCombo.SelectedItem!).Category;
        set
        {
            var item = _categoryCombo.Items.Cast<CategoryChoice>().FirstOrDefault(c => c.Category == value);
            if (item != null) _categoryCombo.SelectedItem = item;
        }
    }

    internal string ObjectiveTitle { get => _titleBox.Text; set => _titleBox.Text = value; }
    internal string ObjectiveDescription { get => _descriptionBox.Text; set => _descriptionBox.Text = value; }
    internal Guid SelectedTargetGuid
    {
        get => (_targetCombo.SelectedItem as SpawnChoice)?.Id ?? Guid.Empty;
        set
        {
            var item = _targetCombo.Items.Cast<SpawnChoice>().FirstOrDefault(s => s.Id == value);
            if (item != null) _targetCombo.SelectedItem = item;
        }
    }
    internal int HoldDurationSeconds { get => (int)_holdDuration.Value; set => _holdDuration.Value = value; }
    internal int MinX { get => (int)_minX.Value; set => _minX.Value = value; }
    internal int MinZ { get => (int)_minZ.Value; set => _minZ.Value = value; }
    internal int MaxX { get => (int)_maxX.Value; set => _maxX.Value = value; }
    internal int MaxZ { get => (int)_maxZ.Value; set => _maxZ.Value = value; }
    internal string RewardCompletionMessage { get => _rewardMessageBox.Text; set => _rewardMessageBox.Text = value; }

    public ObjectiveStudioDialog(ScenarioDocument scenario, IReadOnlyCollection<string>? knownAliases = null, Func<string, string>? objectName = null)
    {
        _scenario = scenario ?? new ScenarioDocument();
        _aliases = (knownAliases ?? []).ToArray();
        _en = Loc.CurrentLanguage == Language.English;

        Text = T("戰役目標工作室", "Objective Studio");
        AutoScaleDimensions = new SizeF(96, 96);
        AutoScaleMode = AutoScaleMode.Dpi;
        StartPosition = FormStartPosition.CenterParent;
        MinimizeBox = MaximizeBox = false;
        ClientSize = new Size(680, 680);
        MinimumSize = new Size(520, 580);

        var grid = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            Padding = new Padding(12),
            ColumnCount = 2,
            RowCount = 13
        };
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 35));
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 65));

        var hint = new Label
        {
            AutoSize = true,
            Dock = DockStyle.Fill,
            Text = T(
                "設計戰役目標並編譯為 ScenarioEvent 原語。僅支援已驗證的目標種類（刺殺、破壞建築、存活、佔領、護送、自訂）；全殲與山丘之王等未支援種類將被拒絕。套用後寫入事件。",
                "Author campaign objectives and compile to verified ScenarioEvent primitives. Only verified kinds (Assassinate, DestroyBuilding, Survival, CaptureArea, EscortUnit, CustomScripted) are supported; unsupported kinds are rejected.")
        };
        grid.Controls.Add(hint, 0, 0);
        grid.SetColumnSpan(hint, 2);

        // Populate Kind ComboBox: includes all ObjectiveKind values so unsupported ones can be selected and rejected with diagnostic errors
        var allKinds = Enum.GetValues<ObjectiveKind>();
        foreach (var k in allKinds)
        {
            string label = k switch
            {
                ObjectiveKind.EliminateAllEnemies => T("全殲敵軍 (Eliminate Enemies) [不支援匯出]", "Eliminate All Enemies [Unsupported for export]"),
                ObjectiveKind.Survival => T("堅守陣地倒數 (Survival)", "Survival Countdown"),
                ObjectiveKind.CaptureArea => T("佔領矩形區域 (Capture Area)", "Capture Rectangular Area"),
                ObjectiveKind.KingOfTheHill => T("山丘之王 (King of the Hill) [不支援匯出]", "King of the Hill [Unsupported for export]"),
                ObjectiveKind.AssassinateTarget => T("刺殺敵方英雄 (Assassinate Hero)", "Assassinate Target"),
                ObjectiveKind.DestroyBuilding => T("破壞關鍵建築 (Destroy Building)", "Destroy Building"),
                ObjectiveKind.EscortUnit => T("護送商隊/VIP (Escort Unit)", "Escort Unit"),
                ObjectiveKind.CustomScripted => T("自訂腳本條件 (Custom Scripted)", "Custom Scripted"),
                _ => k.ToString()
            };
            _kindCombo.Items.Add(new KindChoice(k, label));
        }
        _kindCombo.SelectedIndex = 1; // Default to Survival (supported)

        // Category combo
        _categoryCombo.Items.Add(new CategoryChoice(ObjectiveCategory.Primary, T("主要主線目標 (Primary)", "Primary Objective")));
        _categoryCombo.Items.Add(new CategoryChoice(ObjectiveCategory.Bonus, T("次要/獎勵目標 (Bonus)", "Bonus Objective")));
        _categoryCombo.Items.Add(new CategoryChoice(ObjectiveCategory.FailureCriterion, T("關鍵失敗判據 (Failure Criterion)", "Failure Criterion")));
        _categoryCombo.SelectedIndex = 0;

        _titleBox.Text = "Survive the onslaught";

        // Targets combo from scenario spawns
        var availableSpawns = _scenario.Spawns.Where(s => s != null && s.Id != Guid.Empty).ToList();
        if (availableSpawns.Count > 0)
        {
            foreach (var spawn in availableSpawns)
            {
                string name = objectName?.Invoke(spawn.Alias) ?? spawn.Alias;
                string label = $"{name} — {( _en ? "Team" : "隊伍" )} {spawn.Team} ({spawn.X:0}, {spawn.Z:0}) [{spawn.Id.ToString("N")[..8]}]";
                _targetCombo.Items.Add(new SpawnChoice(spawn.Id, label));
            }
            _targetCombo.SelectedIndex = 0;
        }
        else
        {
            _targetCombo.Items.Add(new SpawnChoice(Guid.Empty, T("(場景中無可用物件)", "(No placed objects in scenario)")));
            _targetCombo.SelectedIndex = 0;
        }

        // Layout rows
        AddRow(1, T("目標種類", "Objective Kind"), _kindCombo);
        AddRow(2, T("類別屬性", "Category"), _categoryCombo);
        AddRow(3, T("目標標題", "Title"), _titleBox);
        AddRow(4, T("目標描述", "Description"), _descriptionBox);
        AddRow(5, T("目標物件", "Target Object"), _targetCombo);
        AddRow(6, T("堅守/維持秒數", "Hold Duration (sec)"), _holdDuration);

        var areaPanel = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 4, AutoSize = true, Margin = new Padding(0) };
        areaPanel.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 25));
        areaPanel.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 25));
        areaPanel.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 25));
        areaPanel.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 25));
        areaPanel.Controls.Add(_minX, 0, 0);
        areaPanel.Controls.Add(_minZ, 1, 0);
        areaPanel.Controls.Add(_maxX, 2, 0);
        areaPanel.Controls.Add(_maxZ, 3, 0);
        AddRow(7, T("區域 (MinX, MinZ, MaxX, MaxZ)", "Area (MinX, MinZ, MaxX, MaxZ)"), areaPanel);

        AddRow(8, T("完成獎勵訊息", "Reward Message"), _rewardMessageBox);

        grid.Controls.Add(new Label { Text = T("編譯預覽與驗證診斷", "Compilation Preview & Diagnostics"), AutoSize = true }, 0, 9);
        grid.SetColumnSpan(grid.GetControlFromPosition(0, 9)!, 2);
        grid.Controls.Add(_preview, 0, 10);
        grid.SetColumnSpan(_preview, 2);

        for (int i = 0; i < 10; i++) grid.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        grid.RowStyles.Add(new RowStyle(SizeType.Percent, 100));

        var buttons = new FlowLayoutPanel
        {
            Dock = DockStyle.Bottom,
            AutoSize = true,
            FlowDirection = FlowDirection.RightToLeft,
            Padding = new Padding(8)
        };
        var cancel = new Button { Text = T("取消", "Cancel"), AutoSize = true, DialogResult = DialogResult.Cancel };
        _apply.Text = T("編譯並套用", "Compile & Apply");
        _apply.Click += (_, _) =>
        {
            RefreshPreview();
            if (CanApply) DialogResult = DialogResult.OK;
        };
        buttons.Controls.AddRange([cancel, _apply]);

        Controls.Add(grid);
        Controls.Add(buttons);
        AcceptButton = _apply;
        CancelButton = cancel;

        // Event hooks
        _kindCombo.SelectedIndexChanged += (_, _) => { UpdateFieldAvailability(); RefreshPreview(); };
        _categoryCombo.SelectedIndexChanged += (_, _) => RefreshPreview();
        _titleBox.TextChanged += (_, _) => RefreshPreview();
        _descriptionBox.TextChanged += (_, _) => RefreshPreview();
        _targetCombo.SelectedIndexChanged += (_, _) => RefreshPreview();
        _holdDuration.ValueChanged += (_, _) => RefreshPreview();
        _minX.ValueChanged += (_, _) => RefreshPreview();
        _minZ.ValueChanged += (_, _) => RefreshPreview();
        _maxX.ValueChanged += (_, _) => RefreshPreview();
        _maxZ.ValueChanged += (_, _) => RefreshPreview();
        _rewardMessageBox.TextChanged += (_, _) => RefreshPreview();

        WinFormsTheme.Apply(this);
        UpdateFieldAvailability();
        RefreshPreview();

        void AddRow(int row, string label, Control input)
        {
            grid.Controls.Add(new Label { Text = label, AutoSize = true, Anchor = AnchorStyles.Left }, 0, row);
            grid.Controls.Add(input, 1, row);
        }
    }

    private void UpdateFieldAvailability()
    {
        var kind = SelectedKind;
        bool needsTarget = kind is ObjectiveKind.Survival or ObjectiveKind.AssassinateTarget or ObjectiveKind.DestroyBuilding or ObjectiveKind.EscortUnit or ObjectiveKind.CaptureArea;
        _targetCombo.Enabled = needsTarget;

        bool needsHold = kind is ObjectiveKind.Survival;
        _holdDuration.Enabled = needsHold;

        bool needsArea = kind is ObjectiveKind.CaptureArea or ObjectiveKind.EscortUnit;
        _minX.Enabled = _minZ.Enabled = _maxX.Enabled = _maxZ.Enabled = needsArea;
    }

    internal ObjectiveDefinition BuildDefinition()
    {
        var kind = SelectedKind;
        var category = SelectedCategory;
        var targetGuid = SelectedTargetGuid;

        var targetGuids = targetGuid != Guid.Empty && kind is not ObjectiveKind.EliminateAllEnemies && kind is not ObjectiveKind.KingOfTheHill && kind is not ObjectiveKind.CustomScripted
            ? new[] { targetGuid }
            : Array.Empty<Guid>();

        ObjectiveAreaBounds? area = null;
        if (kind is ObjectiveKind.CaptureArea or ObjectiveKind.EscortUnit)
        {
            area = ObjectiveAreaBounds.FromRectangle(MinX, MinZ, MaxX, MaxZ);
        }

        var customConditions = kind == ObjectiveKind.CustomScripted && targetGuid != Guid.Empty
            ? new[] { new ScenarioCondition(ScenarioConditionKind.ObjectExists, targetGuid) }
            : Array.Empty<ScenarioCondition>();

        var rewardMessage = RewardCompletionMessage.Trim();
        ObjectiveReward? reward = !string.IsNullOrEmpty(rewardMessage)
            ? new ObjectiveReward(rewardMessage)
            : null;

        return new ObjectiveDefinition
        {
            Id = Guid.NewGuid(),
            Title = ObjectiveTitle,
            Description = ObjectiveDescription,
            Kind = kind,
            Category = category,
            Parameters = new ObjectiveRuleParameters
            {
                TargetGuids = targetGuids,
                HoldDurationSeconds = kind == ObjectiveKind.Survival ? HoldDurationSeconds : 0,
                Area = area,
                CustomConditions = customConditions,
                PlayerTeam = 0,
                TargetTeam = 1
            },
            Reward = reward
        };
    }

    internal void RefreshPreview()
    {
        var definition = BuildDefinition();
        var graph = new ObjectiveDependencyGraph();
        graph.AddObjective(definition);

        var result = BciObjectiveCompiler.Compile(graph, _scenario, _aliases);

        var errors = new List<string>();
        foreach (var diag in result.Diagnostics.Where(d => d.Severity == ObjectiveDiagnosticSeverity.Error))
        {
            errors.Add($"[{diag.Code}] {diag.Message}");
        }

        if (result.Success)
        {
            CompiledEvents = result.CompiledEvents;
            ValidationErrors = errors;
            _apply.Enabled = true;

            var lines = new List<string>
            {
                T($"編譯成功！產生 {CompiledEvents.Count} 個 ScenarioEvent：",
                  $"Compilation Succeeded! Generated {CompiledEvents.Count} ScenarioEvents:")
            };

            foreach (var evt in CompiledEvents)
            {
                lines.Add($"• 事件: \"{evt.Name}\" (延遲: {evt.DelaySeconds}s)");
                foreach (var cond in evt.Conditions)
                {
                    lines.Add($"   [條件] {cond.Kind} (目標: {cond.TargetId:N})");
                }
                foreach (var act in evt.Actions)
                {
                    lines.Add($"   [動作] {act.Kind}: {act.Text ?? act.Alias ?? ""}");
                }
            }

            foreach (var diag in result.Diagnostics.Where(d => d.Severity != ObjectiveDiagnosticSeverity.Error))
            {
                lines.Add($"[{diag.Severity}] {diag.Message}");
            }

            _preview.Text = string.Join(Environment.NewLine, lines);
        }
        else
        {
            CompiledEvents = [];
            ValidationErrors = errors.Count > 0 ? errors : [T("編譯失敗。", "Compilation failed.")];
            _apply.Enabled = false;

            var lines = new List<string>
            {
                T("編譯失敗，無法套用：", "Compilation Failed:")
            };
            lines.AddRange(ValidationErrors);
            _preview.Text = string.Join(Environment.NewLine, lines);
        }
    }

    private string T(string zh, string en) => _en ? en : zh;
    private static NumericUpDown Number(int min, int max, int value) => new() { Minimum = min, Maximum = max, Value = value, Dock = DockStyle.Fill };
    private static NumericUpDown Coordinate(int value) => new() { Minimum = 0, Maximum = 16383, Value = Math.Clamp(value, 0, 16383), Dock = DockStyle.Fill };
}
