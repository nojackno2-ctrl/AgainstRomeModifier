using AgainstRomeMapEditor.Modules.WildLair;
using AgainstRomeModifier;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor;

internal sealed class WildLairDialog : Form
{
    private static readonly Dictionary<string, string> KnownAliasesMap = new(StringComparer.OrdinalIgnoreCase)
    {
        ["ALL_WOL00"] = "FigTieWol00_Wilder_Wolf",
        ["ALL_BAE00"] = "FigTieBae00_Baer",
        ["ALL_EBE00"] = "FigTieEbe00_Wildschwein",
        ["ALL_RAU00"] = "FigTieRau00_Raubkatze",
        ["ALL_PACKPF00"] = "FigTiePac00_Packpferd",
        ["ALL_ZIVMAN00"] = "FigZivMan00_Zivilist",
        ["ALL_ZIVWEI00"] = "FigZivWei00_Zivilistin",
        ["GER_INF00"] = "FigGerInf00_Keule",
        ["GER_INF01"] = "FigGerInf01_Schwert",
        ["GER_INF02"] = "FigGerInf02_Axt",
        ["GER_INF03"] = "FigGerInf03_Schwert_Schild",
        ["GER_SCH00"] = "FigGerSch00_Schleuderer",
        ["GER_KAVINF00"] = "FigGerKavInf00_Kavallerie",
        ["ROM_INF00"] = "FigRomInf00_Lanze",
        ["ROM_INF01"] = "FigRomInf01_Schwert_Schild",
        ["ROM_SCH00"] = "FigRomSch00_Wurfspiesse",
        ["ROM_KAVINF00"] = "FigRomKavInf00_Kavallerie",
        ["HUN_INF00"] = "FigHunInf00_Keule",
        ["HUN_INF01"] = "FigHunInf01_Schwert",
        ["HUN_SCH00"] = "FigHunSch00_Bogenschuetze",
        ["HUN_KAVINF00"] = "FigHunKavInf00_Kavallerie",
        ["HUN_KAVSCH00"] = "FigHunKavSch00_Kavallerie",
        ["KEL_INF00"] = "FigKelInf00_Keule",
        ["KEL_INF01"] = "FigKelInf01_Schwert",
        ["KEL_INF02"] = "FigKelInf02_Lanze",
        ["KEL_SCH00"] = "FigKelSch00_Schleuderer",
        ["GER_HAU00"] = "BauGerHau00_Haupthaus",
        ["ROM_HAU00"] = "BauRomHau00_Hauptzelt"
    };

    private readonly ComboBox _lairCombo = new() { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDownList };
    private readonly NumericUpDown _waveCount = Number(1, 16, 2);
    private readonly ComboBox _unitAlias = new() { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDown };
    private readonly NumericUpDown _spawnCount = Number(1, 20, 3);
    private readonly NumericUpDown _interval = Number(1, 86400, 60);
    private readonly NumericUpDown _spawnRadius = NumberDecimal(0, 50, 2.5m);
    private readonly NumericUpDown _team = Number(0, 8, 1);
    private readonly NumericUpDown _worldX = Number(0, 16383, 4000);
    private readonly NumericUpDown _worldZ = Number(0, 16383, 6000);
    private readonly NumericUpDown _rotation = Number(0, 360, 0);
    private readonly NumericUpDown _rewardGold = Number(0, 100000, 0);
    private readonly TextBox _completionMessage = new() { Dock = DockStyle.Fill };
    private readonly TextBox _previewBox = new() { Dock = DockStyle.Fill, Multiline = true, ReadOnly = true, ScrollBars = ScrollBars.Vertical };
    private readonly Button _applyButton = new() { AutoSize = true };
    private readonly Button _cancelButton = new() { AutoSize = true, DialogResult = DialogResult.Cancel };

    private readonly IReadOnlyList<ScriptObjectAlias> _aliases;
    private readonly NeutralLairCatalog _catalog;
    private readonly ScenarioDocument? _scenario;
    private readonly bool _en;

    private int _rewardWood;
    private int _rewardFood;
    private int _rewardHonor;
    private int? _initialDelaySeconds;
    private int _maxActiveWaves;
    private string _aggroBehavior = "None";
    private Guid _lairInstanceId = Guid.NewGuid();
    private Guid _coreStructureSpawnId = Guid.Empty;

    internal NeutralLairDefinition? SelectedDefinition
    {
        get => _lairCombo.SelectedItem as NeutralLairDefinition;
        set
        {
            if (value is not null && !_lairCombo.Items.Contains(value))
                _lairCombo.Items.Add(value);
            _lairCombo.SelectedItem = value;
            RefreshPreview();
        }
    }

    internal string? SelectedLairId
    {
        get => SelectedDefinition?.Id;
        set
        {
            if (value is null) return;
            var match = _lairCombo.Items.Cast<NeutralLairDefinition>().FirstOrDefault(d => string.Equals(d.Id, value, StringComparison.OrdinalIgnoreCase));
            if (match is not null)
            {
                _lairCombo.SelectedItem = match;
                RefreshPreview();
            }
        }
    }

    internal int WaveCount { get => (int)_waveCount.Value; set { _waveCount.Value = Math.Clamp(value, _waveCount.Minimum, _waveCount.Maximum); RefreshPreview(); } }
    internal string UnitAlias { get => _unitAlias.Text.Trim(); set { _unitAlias.Text = value; RefreshPreview(); } }
    internal int SpawnCount { get => (int)_spawnCount.Value; set { _spawnCount.Value = Math.Clamp(value, _spawnCount.Minimum, _spawnCount.Maximum); RefreshPreview(); } }
    internal int IntervalSeconds { get => (int)_interval.Value; set { _interval.Value = Math.Clamp(value, _interval.Minimum, _interval.Maximum); RefreshPreview(); } }
    internal float SpawnRadiusTiles { get => (float)_spawnRadius.Value; set { _spawnRadius.Value = (decimal)Math.Clamp(value, (float)_spawnRadius.Minimum, (float)_spawnRadius.Maximum); RefreshPreview(); } }
    internal int Team { get => (int)_team.Value; set { _team.Value = Math.Clamp(value, _team.Minimum, _team.Maximum); RefreshPreview(); } }
    internal float WorldX { get => (float)_worldX.Value; set { _worldX.Value = (decimal)Math.Clamp(value, (float)_worldX.Minimum, (float)_worldX.Maximum); RefreshPreview(); } }
    internal float WorldZ { get => (float)_worldZ.Value; set { _worldZ.Value = (decimal)Math.Clamp(value, (float)_worldZ.Minimum, (float)_worldZ.Maximum); RefreshPreview(); } }
    internal float RotationDeg { get => (float)_rotation.Value; set { _rotation.Value = (decimal)Math.Clamp(value, (float)_rotation.Minimum, (float)_rotation.Maximum); RefreshPreview(); } }
    internal int RewardGold { get => (int)_rewardGold.Value; set { _rewardGold.Value = Math.Clamp(value, _rewardGold.Minimum, _rewardGold.Maximum); RefreshPreview(); } }
    internal int RewardWood { get => _rewardWood; set { _rewardWood = value; RefreshPreview(); } }
    internal int RewardFood { get => _rewardFood; set { _rewardFood = value; RefreshPreview(); } }
    internal int RewardHonor { get => _rewardHonor; set { _rewardHonor = value; RefreshPreview(); } }
    internal string CompletionMessage { get => _completionMessage.Text.Trim(); set { _completionMessage.Text = value; RefreshPreview(); } }

    internal int InitialDelaySeconds
    {
        get => _initialDelaySeconds ?? (int)_interval.Value;
        set { _initialDelaySeconds = value; RefreshPreview(); }
    }

    internal int MaxActiveWaves
    {
        get => _maxActiveWaves;
        set { _maxActiveWaves = value; RefreshPreview(); }
    }

    internal string AggroBehavior
    {
        get => _aggroBehavior;
        set { _aggroBehavior = value; RefreshPreview(); }
    }

    internal Guid LairInstanceId
    {
        get => _lairInstanceId;
        set { _lairInstanceId = value; RefreshPreview(); }
    }

    internal Guid CoreStructureSpawnId
    {
        get => _coreStructureSpawnId;
        set { _coreStructureSpawnId = value; RefreshPreview(); }
    }

    internal IReadOnlyList<ScenarioEvent> ResultingEvents { get; private set; } = [];
    internal IReadOnlyList<ScenarioEvent> ScenarioEvents => ResultingEvents;
    internal IReadOnlyList<string> ValidationErrors { get; private set; } = [];
    internal string PreviewText => _previewBox.Text;
    internal bool CanApply => _applyButton.Enabled;

    internal NeutralLairDefinition EffectiveDefinition
    {
        get
        {
            var selected = SelectedDefinition ?? new NeutralLairDefinition(
                "UNKNOWN", "未知巢穴", "Unknown Lair",
                LairCategory.BarbarianCamp, LairDifficultyTier.Unrated,
                "BauGerHau00_Haupthaus", 2.5f, [], [], new());

            var waves = Enumerable.Range(0, Math.Max(0, WaveCount))
                .Select(i => new LairWaveSpawnRule(
                    $"wave_{i}",
                    UnitAlias,
                    SpawnCount,
                    IntervalSeconds,
                    InitialDelaySeconds: InitialDelaySeconds,
                    MaxActiveWaves: MaxActiveWaves,
                    SpawnRadiusTiles: SpawnRadiusTiles,
                    AggroBehavior: AggroBehavior))
                .ToArray();

            var loot = new LairLootReward(
                Wood: RewardWood,
                Food: RewardFood,
                Gold: RewardGold,
                HonorPoints: RewardHonor,
                CompletionMessage: CompletionMessage);

            return selected with
            {
                WaveRules = waves,
                Loot = loot
            };
        }
    }

    internal PlacedNeutralLair PlacedLair => new(
        InstanceId: LairInstanceId,
        DefinitionId: SelectedDefinition?.Id ?? string.Empty,
        WorldX: WorldX,
        WorldY: 0f,
        WorldZ: WorldZ,
        RotationDeg: RotationDeg,
        Team: Team,
        CoreStructureSpawnId: CoreStructureSpawnId,
        IsActive: true);

    public WildLairDialog() : this((IReadOnlyList<ScriptObjectAlias>?)null, null, null)
    {
    }

    public WildLairDialog(IReadOnlyCollection<string> aliases, ScenarioDocument? scenario = null, NeutralLairCatalog? catalog = null)
        : this(NormalizeAliases(null, aliases), catalog, scenario)
    {
    }

    public WildLairDialog(
        IReadOnlyList<ScriptObjectAlias>? aliases = null,
        NeutralLairCatalog? catalog = null,
        ScenarioDocument? scenario = null)
    {
        _aliases = NormalizeAliases(aliases, null);
        _catalog = catalog ?? NeutralLairCatalog.Default;
        _scenario = scenario;
        _en = Loc.CurrentLanguage == Language.English;

        Text = T("野外巢穴與守衛波次設定", "Neutral Lair & Guard Waves Planner");
        AutoScaleDimensions = new SizeF(96, 96);
        AutoScaleMode = AutoScaleMode.Dpi;
        StartPosition = FormStartPosition.CenterParent;
        MinimizeBox = MaximizeBox = false;
        ClientSize = new Size(680, 720);
        MinimumSize = new Size(480, 560);

        var grid = new TableLayoutPanel
        {
            Dock = DockStyle.Fill,
            Padding = new Padding(12),
            ColumnCount = 2,
            RowCount = 15
        };
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 35));
        grid.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 65));

        var hint = new Label
        {
            AutoSize = true,
            Dock = DockStyle.Fill,
            Text = T(
                "選擇中立巢穴原型並設定守衛定時波次。野生動物、隊伍 8 與資源獎勵不支援且已封鎖。支援部隊定時生成；巡邏與戰利品不支援。",
                "Select a neutral lair definition and configure timed guard waves. Wild animals, team 8 and resource rewards are unsupported and blocked. Faction infantry timed spawns are supported; patrol and loot are unsupported.")
        };
        grid.Controls.Add(hint, 0, 0);
        grid.SetColumnSpan(hint, 2);

        _lairCombo.Items.AddRange(_catalog.AllDefinitions.Cast<object>().ToArray());
        _lairCombo.FormattingEnabled = true;
        _lairCombo.Format += (_, e) =>
        {
            if (e.ListItem is NeutralLairDefinition def)
                e.Value = _en ? $"{def.DisplayNameEn} ({def.Id})" : $"{def.DisplayNameZh} ({def.Id})";
        };
        if (_lairCombo.Items.Count > 0)
            _lairCombo.SelectedIndex = 0;

        var troopAliases = _aliases
            .Where(a => new[] { "FigGerInf", "FigHunInf", "FigKelInf", "FigRomInf" }.Any(p => a.NameDef.StartsWith(p, StringComparison.Ordinal)))
            .Select(a => a.Alias)
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .ToArray();

        if (troopAliases.Length > 0)
        {
            _unitAlias.Items.AddRange(troopAliases.Cast<object>().ToArray());
            _unitAlias.SelectedIndex = 0;
        }
        else
        {
            _unitAlias.Text = "GER_INF01";
        }

        AddRow(1, T("中立巢穴原型", "Neutral lair definition"), _lairCombo);
        AddRow(2, T("波次數量", "Wave count"), _waveCount);
        AddRow(3, T("生成兵種別名", "Unit alias"), _unitAlias);
        AddRow(4, T("每波人數", "Spawn count"), _spawnCount);
        AddRow(5, T("波次間隔（秒）", "Wave interval (seconds)"), _interval);
        AddRow(6, T("生成半徑（圖格）", "Spawn radius (tiles)"), _spawnRadius);
        AddRow(7, T("所屬隊伍（0–8）", "Team (0–8)"), _team);
        AddRow(8, T("巢穴坐標 X", "Lair X"), _worldX);
        AddRow(9, T("巢穴坐標 Z", "Lair Z"), _worldZ);
        AddRow(10, T("旋轉角度（度）", "Rotation (deg)"), _rotation);
        AddRow(11, T("黃金獎勵（不支援）", "Gold reward (unsupported)"), _rewardGold);
        AddRow(12, T("通關完成訊息", "Completion message"), _completionMessage);

        var previewLabel = new Label { Text = T("事件預覽與驗證結果", "Preview and validation"), AutoSize = true };
        grid.Controls.Add(previewLabel, 0, 13);
        grid.SetColumnSpan(previewLabel, 2);

        grid.Controls.Add(_previewBox, 0, 14);
        grid.SetColumnSpan(_previewBox, 2);

        for (int i = 0; i < 14; i++) grid.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        grid.RowStyles.Add(new RowStyle(SizeType.Percent, 100));

        var buttons = new FlowLayoutPanel
        {
            Dock = DockStyle.Bottom,
            AutoSize = true,
            FlowDirection = FlowDirection.RightToLeft,
            Padding = new Padding(8)
        };
        _cancelButton.Text = T("取消", "Cancel");
        _applyButton.Text = T("套用巢穴事件", "Apply lair events");
        _applyButton.Click += (_, _) =>
        {
            RefreshPreview();
            if (CanApply) DialogResult = DialogResult.OK;
        };
        buttons.Controls.AddRange([_cancelButton, _applyButton]);

        Controls.Add(grid);
        Controls.Add(buttons);
        AcceptButton = _applyButton;
        CancelButton = _cancelButton;

        _lairCombo.SelectedIndexChanged += (_, _) =>
        {
            if (SelectedDefinition is { } def)
            {
                if (def.WaveRules.Count > 0)
                {
                    var first = def.WaveRules[0];
                    _waveCount.Value = Math.Clamp(def.WaveRules.Count, _waveCount.Minimum, _waveCount.Maximum);
                    _spawnCount.Value = Math.Clamp(first.SpawnCount, _spawnCount.Minimum, _spawnCount.Maximum);
                    _interval.Value = Math.Clamp(first.IntervalSeconds, _interval.Minimum, _interval.Maximum);
                    if (!string.IsNullOrEmpty(first.UnitAlias))
                        _unitAlias.Text = first.UnitAlias;
                    _spawnRadius.Value = (decimal)Math.Clamp(first.SpawnRadiusTiles, (float)_spawnRadius.Minimum, (float)_spawnRadius.Maximum);
                }
                if (def.Loot is { } loot)
                {
                    _rewardWood = loot.Wood;
                    _rewardFood = loot.Food;
                    _rewardGold.Value = Math.Clamp(loot.Gold, _rewardGold.Minimum, _rewardGold.Maximum);
                    _rewardHonor = loot.HonorPoints;
                    _completionMessage.Text = loot.CompletionMessage;
                }
            }
            RefreshPreview();
        };

        foreach (var num in new[] { _waveCount, _spawnCount, _interval, _spawnRadius, _team, _worldX, _worldZ, _rotation, _rewardGold })
            num.ValueChanged += (_, _) => RefreshPreview();
        _unitAlias.TextChanged += (_, _) => RefreshPreview();
        _completionMessage.TextChanged += (_, _) => RefreshPreview();

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
        var errors = new List<string>();
        var selected = SelectedDefinition;

        if (selected is null)
        {
            errors.Add(T("未選擇中立巢穴原型。", "No neutral lair definition selected."));
        }
        else if (selected.Category == LairCategory.WildAnimal || selected.Id.StartsWith("ALL_", StringComparison.OrdinalIgnoreCase))
        {
            errors.Add(T(
                "不支援野生動物生成（FigTie 動物需單一物件創建且歸屬隊伍 8，SpawnUnit 僅提供隊伍 0–7 部隊生成）。",
                "FigTie animals require single-object creation and neutral team 8. ScenarioEvent SpawnUnit only provides troop creation for teams 0-7."));
        }

        if (Team == 8)
        {
            errors.Add(T(
                "不支援隊伍 8（隊伍 8 為中立 DATA 歸屬，SpawnUnit 僅支援隊伍 0–7）。",
                "ScenarioEvent SpawnUnit cannot express neutral team 8."));
        }
        else if (Team is < 0 or > 7)
        {
            errors.Add(T("隊伍編號必須介於 0 至 7 之間。", "Team must be between 0 and 7."));
        }

        if (RewardGold > 0 || RewardWood > 0 || RewardFood > 0 || RewardHonor > 0)
        {
            errors.Add(T(
                "不支援戰利品資源與榮譽獎勵（已驗證的事件編譯器無資源或榮譽獎勵動作）。",
                "The verified event compiler has no resource or honor reward action."));
        }

        if (selected is not null && selected.DefaultGuards.Count > 0)
        {
            errors.Add(T(
                "不支援常駐守衛巡邏或死亡重生設定。",
                "Guard placement, patrol and death-triggered respawn are not supported by this timer adapter."));
        }

        if (_aliases.Count == 0)
        {
            errors.Add(T(
                "未載入遊戲物件別名（Aliases 清單為空）。",
                "Load ScriptObjectAliases from the actual game data before binding."));
        }

        if (InitialDelaySeconds != IntervalSeconds)
        {
            errors.Add(T(
                "ScenarioEvent 單一事件僅支援單一週期，不支援獨立首波延遲。",
                "A ScenarioEvent has one timer period; separate initial delays cannot be expressed."));
        }

        if (MaxActiveWaves != 0)
        {
            errors.Add(T(
                "不支援活躍波次上限（必須為 0 無上限）。",
                "Active-wave caps are unsupported (use MaxActiveWaves=0)."));
        }

        if (!string.Equals(AggroBehavior, "None", StringComparison.OrdinalIgnoreCase))
        {
            errors.Add(T(
                "不支援侵略/仇恨行為命令（必須為 None）。",
                "Aggression orders cannot be expressed (use AggroBehavior=None)."));
        }

        if (IntervalSeconds < 10)
        {
            errors.Add(T("波次間隔必須大於或等於 10 秒。", "Interval must be at least 10 seconds."));
        }

        if (SpawnCount is < 1 or > 20)
        {
            errors.Add(T("每波人數必須介於 1 至 20 人之間。", "Spawn count must be between 1 and 20."));
        }

        if (WaveCount < 1)
        {
            errors.Add(T("波次數量必須大於 0。", "Wave count must be greater than 0."));
        }

        if (WorldX is < 0 or > 16383 || WorldZ is < 0 or > 16383)
        {
            errors.Add(T("世界坐標超出範圍 (0–16383)。", "World coordinates must be within [0, 16383]."));
        }

        if (!string.IsNullOrEmpty(CompletionMessage) && CoreStructureSpawnId == Guid.Empty)
        {
            errors.Add(T(
                "通關完成訊息需要指定已追蹤的核心物件 ID。",
                "A completion message requires a tracked ScenarioSpawn core ID."));
        }

        if (errors.Count > 0)
        {
            ValidationErrors = errors;
            ResultingEvents = [];
            _applyButton.Enabled = false;
            _previewBox.Text = string.Join(Environment.NewLine, errors.Select(err => $"[Error] {err}"));
            return;
        }

        try
        {
            var events = WildLairScenarioEventBinder.GenerateEventsForLair(
                PlacedLair, EffectiveDefinition, playerTeam: 0, tileWorldSize: 256f, aliases: _aliases);

            if (_scenario is not null)
            {
                var merged = _scenario.Events.Concat(events).ToArray();
                if (merged.Length > 256)
                    throw new InvalidDataException(T(
                        $"合併後事件總數（{merged.Length}）超過上限 256。",
                        $"Merged event count ({merged.Length}) exceeds maximum 256."));
                ScenarioEventValidator.Validate(merged, _aliases.Select(a => a.Alias).ToArray());
                ScenarioEventValidator.ValidateConditions(merged, _scenario);
            }

            ValidationErrors = [];
            ResultingEvents = events;
            _applyButton.Enabled = true;
            _previewBox.Text = string.Join(Environment.NewLine, events.Select(e =>
                $"{e.Name}: {e.DelaySeconds}s (repeat={e.Repeat}) — " +
                string.Join(", ", e.Actions.Select(a => $"{a.Alias} × {a.Count} (team {a.Team}, x={a.X:F0}, z={a.Z:F0})")) +
                (e.Conditions.Count > 0 ? " [Conditions: " + string.Join(", ", e.Conditions.Select(c => $"{c.Kind}:{c.TargetId}")) + "]" : "")));
        }
        catch (Exception ex)
        {
            ValidationErrors = [ex.Message];
            ResultingEvents = [];
            _applyButton.Enabled = false;
            _previewBox.Text = $"[Error] {ex.Message}";
        }
    }

    internal static NeutralLairDefinition CreateAuthoredBlueprint(
        string id = "AUTHORED_OUTPOST",
        string nameZh = "蠻族前哨營地",
        string nameEn = "Barbarian Outpost",
        LairCategory category = LairCategory.BarbarianCamp,
        string nativeBuilding = "BauGerHau00_Haupthaus",
        float footprintRadius = 2.5f,
        IReadOnlyList<LairWaveSpawnRule>? waves = null,
        LairLootReward? loot = null) =>
        new(id, nameZh, nameEn, category, LairDifficultyTier.Tier1Scout,
            nativeBuilding, footprintRadius, [], waves ?? [], loot ?? new());

    private static IReadOnlyList<ScriptObjectAlias> NormalizeAliases(
        IReadOnlyList<ScriptObjectAlias>? aliases,
        IReadOnlyCollection<string>? stringAliases)
    {
        if (aliases is not null && aliases.Count > 0) return aliases;
        if (stringAliases is not null && stringAliases.Count > 0)
        {
            return stringAliases.Select(ResolveAlias).ToList();
        }
        return [];
    }

    private static ScriptObjectAlias ResolveAlias(string alias)
    {
        if (KnownAliasesMap.TryGetValue(alias, out var nameDef))
            return new ScriptObjectAlias(alias, nameDef);
        return new ScriptObjectAlias(alias, alias);
    }

    private string T(string zh, string en) => _en ? en : zh;
    private static NumericUpDown Number(int min, int max, int value) => new() { Minimum = min, Maximum = max, Value = value, Dock = DockStyle.Fill };
    private static NumericUpDown NumberDecimal(decimal min, decimal max, decimal value) => new() { Minimum = min, Maximum = max, Value = value, DecimalPlaces = 1, Increment = 0.5m, Dock = DockStyle.Fill };
}
