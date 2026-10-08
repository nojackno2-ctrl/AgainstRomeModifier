using System.Text;
using AgainstRomeModifier;

namespace AgainstRomeMapEditor;

/// <summary>Produces a reviewable plan; only the explicit Apply command changes the host.</summary>
internal sealed class AiMapPlanningDialog : Form
{
    private readonly Func<CancellationToken, Task<IReadOnlyList<string>>> _listModels;
    private readonly Func<IReadOnlyList<MultiAiMapRoleRequest>, string, CancellationToken, IProgress<AiMapRoleProgress>, Task<MultiAiMapPlanResult>> _generate;
    private readonly Func<AiMapPlan, AiMapApplyResult> _apply;
    private readonly Func<AiMapPlan, AiMapPlanPreview>? _preview;
    private readonly CancellationTokenSource _lifetime = new();
    private CancellationTokenSource? _operation;
    private AiMapPlan? _plan;
    private bool _closing;
    private bool _loading;
    private bool _generationStarted;
    private int _inputVersion;
    private readonly bool _isEn;
    internal ComboBox[] ModelBoxes { get; } = Enumerable.Range(0, 3).Select(_ => new ComboBox { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDown, Text = "laguna-xs-2.1:latest" }).ToArray();
    internal CheckBox[] RoleChecks { get; } = Enumerable.Range(0, 3).Select(_ => new CheckBox { Checked = true, AutoSize = true, Anchor = AnchorStyles.Left }).ToArray();
    internal TextBox BoundsBox { get; } = new() { Dock = DockStyle.Fill, Text = "0,0,64,64" };
    internal TextBox LocksBox { get; } = new() { Dock = DockStyle.Fill, Multiline = true, ScrollBars = ScrollBars.Vertical };
    internal CheckBox PassabilityCheck { get; } = new() { AutoSize = true };
    internal TextBox DescriptionBox { get; } = new() { Dock = DockStyle.Fill, Multiline = true, ScrollBars = ScrollBars.Vertical, AcceptsReturn = true };
    internal TextBox PreviewBox { get; } = new() { Dock = DockStyle.Fill, Multiline = true, ReadOnly = true, ScrollBars = ScrollBars.Both, WordWrap = false };
    internal PictureBox PreviewImage { get; } = new() { Dock = DockStyle.Fill, SizeMode = PictureBoxSizeMode.Zoom, BackColor = Color.FromArgb(30, 30, 30) };
    internal Label StatusLabel { get; } = new() { Dock = DockStyle.Fill, AutoSize = false };
    internal ProgressBar GenerationProgress { get; } = new() { Dock = DockStyle.Bottom, Height = 12, Maximum = 3 };
    internal Button GenerateButton { get; } = new() { AutoSize = true };
    internal Button ApplyButton { get; } = new() { AutoSize = true, Enabled = false };
    internal Button CancelGenerationButton { get; } = new() { AutoSize = true, Enabled = false };
    private bool CanUpdate => !_closing && !IsDisposed && !Disposing;
    private string T(string zh, string en) => _isEn ? en : zh;

    internal AiMapPlanningDialog(Func<CancellationToken, Task<IReadOnlyList<string>>> listModels,
        Func<IReadOnlyList<MultiAiMapRoleRequest>, string, CancellationToken, Task<MultiAiMapPlanResult>> generate,
        Func<AiMapPlan, AiMapApplyResult> apply, Func<AiMapPlan, AiMapPlanPreview>? preview = null)
        : this(listModels, (requests, description, token, _) => generate(requests, description, token), apply, preview) { }

    internal AiMapPlanningDialog(Func<CancellationToken, Task<IReadOnlyList<string>>> listModels,
        Func<IReadOnlyList<MultiAiMapRoleRequest>, string, CancellationToken, IProgress<AiMapRoleProgress>, Task<MultiAiMapPlanResult>> generate,
        Func<AiMapPlan, AiMapApplyResult> apply, Func<AiMapPlan, AiMapPlanPreview>? preview = null)
    {
        _listModels = listModels; _generate = generate; _apply = apply; _preview = preview;
        _isEn = Loc.CurrentLanguage == Language.English;
        Text = T("多 AI 製圖（本機 Ollama）", "Multi-AI Map Maker (local Ollama)");
        Size = new Size(940, 850); MinimumSize = new Size(740, 750); StartPosition = FormStartPosition.CenterParent;
        MinimizeBox = false; MaximizeBox = false; ShowInTaskbar = false;
        var layout = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 2, Padding = new Padding(12), RowCount = 7 };
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 110)); layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        for (int i = 0; i < 3; i++)
        {
            layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 36));
            string role = _isEn ? ((AiMapDesignRole)i).ToString() : MultiAiMapPlanResult.RoleName((AiMapDesignRole)i);
            RoleChecks[i].Text = role;
            layout.Controls.Add(RoleChecks[i], 0, i);
            layout.Controls.Add(ModelBoxes[i], 1, i);
        }
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 190));
        layout.RowStyles.Add(new RowStyle(SizeType.Percent, 72));
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 70));
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 45));
        DescriptionBox.Text = T("一條從西北流向東南的河谷，東北方是山脈，西南有小湖；河岸是沙地，保留寬廣平坦的草地讓村莊發展。", "A river valley from north-west to south-east, mountains in the north-east, a small lake in the south-west, sandy riverbanks and wide flat dry meadows for villages.");
        var input = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 3, RowCount = 3 };
        input.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 185)); input.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        input.ColumnStyles.Add(new ColumnStyle(SizeType.AutoSize));
        input.RowStyles.Add(new RowStyle(SizeType.Absolute, 30)); input.RowStyles.Add(new RowStyle(SizeType.Absolute, 60)); input.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        input.Controls.Add(new Label { Text = T("範圍 X,Y,寬,高", "Area X,Y,width,height"), AutoSize = true }, 0, 0); input.Controls.Add(BoundsBox, 1, 0);
        PassabilityCheck.Text = T("允許修改通行", "Edit passability"); input.Controls.Add(PassabilityCheck, 2, 0);
        input.Controls.Add(new Label { Text = T("AI 鎖定區（每行一區）", "AI locks (one area per line)"), AutoSize = true }, 0, 1); input.Controls.Add(LocksBox, 1, 1);
        input.SetColumnSpan(LocksBox, 2);
        input.Controls.Add(new Label { Text = T("地圖描述", "Description"), AutoSize = true }, 0, 2); input.Controls.Add(DescriptionBox, 1, 2);
        input.SetColumnSpan(DescriptionBox, 2);
        layout.Controls.Add(new Label { Text = T("局部重做", "Partial redo"), AutoSize = true }, 0, 3); layout.Controls.Add(input, 1, 3);
        var reviewTabs = new TabControl { Dock = DockStyle.Fill };
        if (_preview is not null)
        {
            var mapTab = new TabPage(T("地圖預覽", "Map preview")) { BackColor = Color.FromArgb(30, 30, 30), UseVisualStyleBackColor = false };
            var legend = new Label { Dock = DockStyle.Bottom, AutoSize = true, Text = T("示意圖：藍＝水域、紅＝阻擋、橙點＝材質變更、紫點＝材質拒絕；灰暗＝範圍外／鎖定。非遊戲渲染。", "Schematic: blue = water, red = blocked; orange = material changes, purple = rejected; dimmed = outside area / locked. Not game rendering.") };
            mapTab.SizeChanged += (_, _) => legend.MaximumSize = new Size(Math.Max(1, mapTab.ClientSize.Width), 0);
            mapTab.Controls.Add(PreviewImage); mapTab.Controls.Add(legend); reviewTabs.TabPages.Add(mapTab);
        }
        var detailsTab = new TabPage(T("方案與診斷", "Plan and diagnostics")); detailsTab.Controls.Add(PreviewBox); reviewTabs.TabPages.Add(detailsTab);
        layout.Controls.Add(new Label { Text = T("方案檢視", "Plan review"), AutoSize = true }, 0, 4); layout.Controls.Add(reviewTabs, 1, 4);
        StatusLabel.Text = T("生成後先檢視方案，再按「套用方案」。可復原；按「儲存」前不會寫入檔案。", "Generate, review, then Apply. Changes are undoable and no files are written until Save.");
        var statusHost = new Panel { Dock = DockStyle.Fill }; statusHost.Controls.Add(StatusLabel); statusHost.Controls.Add(GenerationProgress);
        layout.Controls.Add(statusHost, 0, 5); layout.SetColumnSpan(statusHost, 2);
        var buttons = new FlowLayoutPanel { Dock = DockStyle.Fill, FlowDirection = FlowDirection.RightToLeft };
        var close = new Button { Text = T("關閉", "Close"), AutoSize = true, DialogResult = DialogResult.Cancel };
        GenerateButton.Text = T("生成／重試", "Generate / Retry"); ApplyButton.Text = T("套用方案", "Apply plan"); CancelGenerationButton.Text = T("取消生成", "Cancel generation");
        buttons.Controls.AddRange([close, ApplyButton, CancelGenerationButton, GenerateButton]);
        layout.Controls.Add(buttons, 0, 6); layout.SetColumnSpan(buttons, 2); Controls.Add(layout); CancelButton = close;
        WinFormsTheme.Apply(this); WinFormsTheme.StylePrimaryButton(ApplyButton);
        DescriptionBox.TextChanged += (_, _) => InvalidatePlan();
        foreach (var box in ModelBoxes) box.TextChanged += (_, _) => InvalidatePlan();
        foreach (var check in RoleChecks) check.CheckedChanged += (_, _) => InvalidatePlan();
        BoundsBox.TextChanged += (_, _) => InvalidatePlan(); LocksBox.TextChanged += (_, _) => InvalidatePlan();
        PassabilityCheck.CheckedChanged += (_, _) => InvalidatePlan();
        Shown += async (_, _) => await LoadModelsAsync();
        GenerateButton.Click += async (_, _) => await GenerateAsync();
        ApplyButton.Click += (_, _) => ApplyPlan();
        CancelGenerationButton.Click += (_, _) => _operation?.Cancel();
        FormClosing += (_, _) => { _closing = true; _lifetime.Cancel(); _operation?.Cancel(); };
    }

    internal async Task LoadModelsAsync()
    {
        if (_loading || !CanUpdate) return;
        _loading = true;
        try
        {
            var models = await _listModels(_lifetime.Token);
            if (!CanUpdate || _generationStarted) return;
            foreach (ComboBox box in ModelBoxes)
            {
                string previous = box.Text;
                box.Items.Clear(); box.Items.AddRange(models.Cast<object>().ToArray());
                box.Text = previous.Length > 0 ? previous : (models.Count > 0 ? models[0] : "");
            }
            if (models.Count == 0) StatusLabel.Text = T("Ollama 沒有模型；請先安裝模型，或輸入已知模型名稱。", "No Ollama models found. Install one or enter a known model name.");
        }
        catch (OperationCanceledException) { }
        catch (Exception ex) { if (CanUpdate && !_generationStarted) StatusLabel.Text = T("讀取模型失敗，可輸入模型名稱後重試：", "Cannot list models; enter model names and retry: ") + ex.Message; }
        finally { _loading = false; }
    }

    internal async Task GenerateAsync()
    {
        if (_operation is not null || !CanUpdate) return;
        _generationStarted = true; _plan = null; ApplyButton.Enabled = false; PreviewBox.Clear(); ClearPreviewImage();
        using var operation = CancellationTokenSource.CreateLinkedTokenSource(_lifetime.Token);
        _operation = operation; SetBusy(true);
        GenerationProgress.Value = 0;
        StatusLabel.Text = T("地形、水系、材質依序規劃，一次只執行一個本機模型；可能需要數分鐘。", "Terrain, water and materials plan in order, with one local inference at a time; this may take several minutes.");
        try
        {
            var requests = ModelBoxes.Select((box, i) => new MultiAiMapRoleRequest((AiMapDesignRole)i, box.Text.Trim())).Where(request => RoleChecks[(int)request.Role].Checked).ToArray();
            if (requests.Length == 0) throw new ArgumentException(T("請至少選擇一個角色。", "Select at least one specialist."));
            AiMapEditScope scope = AiMapEditScope.Parse(BoundsBox.Text, LocksBox.Text, PassabilityCheck.Checked && RoleChecks[0].Checked);
            GenerationProgress.Maximum = requests.Length;
            int inputVersion = _inputVersion;
            var progress = new Progress<AiMapRoleProgress>(update =>
            {
                if (!CanUpdate || _operation != operation || operation.IsCancellationRequested || inputVersion != _inputVersion) return;
                GenerationProgress.Value = Math.Clamp(update.CompletedRoles, 0, requests.Length);
                string role = _isEn ? update.Role.ToString() : MultiAiMapPlanResult.RoleName(update.Role);
                StatusLabel.Text = update.Finished
                    ? T($"已完成 {update.CompletedRoles}/{requests.Length}：{role} / {update.Model}（{update.Status}）", $"Completed {update.CompletedRoles}/{requests.Length}: {role} / {update.Model} ({update.Status})")
                    : T($"正在規劃 {role} / {update.Model}；已完成 {update.CompletedRoles}/{requests.Length}。", $"Planning {role} / {update.Model}; completed {update.CompletedRoles}/{requests.Length}.");
            });
            string description = DescriptionBox.Text + $"\nUse global tile coordinates inside X={scope.Bounds.X}..{scope.Bounds.Right - 1}, Y={scope.Bounds.Y}..{scope.Bounds.Bottom - 1}. Protected rectangles (X,Y,width,height): "
                + string.Join("; ", scope.Locked.Select(r => $"{r.X},{r.Y},{r.Width},{r.Height}"));
            if (!scope.EditPassability) description += "\nDo not create blocked or passable features; passability is protected.";
            MultiAiMapPlanResult result = await _generate(requests, description, operation.Token, progress);
            if (!CanUpdate) return;
            if (inputVersion != _inputVersion) { ShowInvalidatedPlan(); return; }
            // A provider can finish after Cancel; its result must still never become applicable.
            if (operation.IsCancellationRequested || result.IsCancelled)
            {
                StatusLabel.Text = T("已取消生成；可重試。", "Generation cancelled; you can retry."); return;
            }
            if (result.Plan is { } scopedPlan) scopedPlan.EditScope = scope;
            PreviewBox.Text = FormatReview(result);
            PreviewBox.AppendText(T($"\r\n重做範圍：{BoundsBox.Text}；鎖定區 {scope.Locked.Count} 個。共用邊界頂點保留，鎖定僅保護本次 AI 套用。\r\n", $"\r\nEdit area: {BoundsBox.Text}; {scope.Locked.Count} locks. Shared boundary vertices are preserved; locks protect this AI operation only.\r\n"));
            if (!scope.EditPassability) PreviewBox.AppendText(T("通行層保護中；方案內 blocked/passable 不會套用。\r\n", "Passability is protected; blocked/passable features will not be applied.\r\n"));
            if (result.Plan is { } plan && _preview is not null)
            {
                AiMapPlanPreview preview = _preview(plan);
                PreviewImage.Image = preview.Image;
                var changes = preview.Changes;
                PreviewBox.AppendText(T($"\r\n預計變更：高度 {changes.HeightSamplesChanged} 點、通行 {changes.CollisionPixelsChanged} 點；材質接受 {changes.MaterialStrokes - changes.RejectedMaterialStrokes}/{changes.MaterialStrokes} 區。\r\n",
                    $"\r\nExpected changes: {changes.HeightSamplesChanged} height samples, {changes.CollisionPixelsChanged} passability pixels; {changes.MaterialStrokes - changes.RejectedMaterialStrokes}/{changes.MaterialStrokes} material areas accepted.\r\n"));
                if (changes.RejectedMaterialStrokes > 0) PreviewBox.AppendText(T("部分材質區域無法表示，套用時會略過。", "Some material areas cannot be represented and will be skipped on Apply."));
                PreviewBox.AppendText(T($"\r\n生成後地圖檢查：{preview.Issues.Count} 項（含既有問題）。\r\n", $"\r\nPost-generation map check: {preview.Issues.Count} issues (including existing issues).\r\n"));
                foreach (var issue in preview.Issues) PreviewBox.AppendText($"{issue.Severity}: {(_isEn ? issue.English : issue.Chinese)}\r\n");
            }
            _plan = result.Plan;
            GenerationProgress.Value = Math.Clamp(result.Roles.Count, 0, requests.Length);
            StatusLabel.Text = result.Plan is null ? T("全部角色失敗；請檢視診斷後重試。", "All specialists failed. Review diagnostics and retry.")
                : result.HasFailures ? T("部分角色失敗：這是未完整的方案。請檢視診斷，再決定套用或重試。", "Some specialists failed: this is a partial plan. Review diagnostics before applying or retrying.")
                : T("方案已生成，尚未套用。請先檢視，再按「套用方案」。", "Plan generated and awaiting review. Click Apply to change the map.");
        }
        catch (OperationCanceledException) { if (CanUpdate) StatusLabel.Text = T("已取消生成；可重試。", "Generation cancelled; you can retry."); }
        catch (Exception ex) { _plan = null; if (CanUpdate) { ClearPreviewImage(); StatusLabel.Text = T("生成或預覽失敗；可重試：", "Generation or preview failed; you can retry: ") + ex.Message; } }
        finally { _operation = null; if (CanUpdate) SetBusy(false); }
    }

    private void InvalidatePlan()
    {
        _inputVersion++;
        if (_plan is null || !CanUpdate) return;
        _plan = null; ApplyButton.Enabled = false;
        ShowInvalidatedPlan();
    }

    private void ShowInvalidatedPlan()
    {
        PreviewBox.Clear();
        ClearPreviewImage();
        StatusLabel.Text = T("角色、範圍、鎖定區或描述已變更，舊方案已失效。請重新生成。", "Roles, area, locks or description changed; the old plan is invalid. Generate again.");
    }
    private void ClearPreviewImage()
    {
        Image? old = PreviewImage.Image; PreviewImage.Image = null; old?.Dispose();
    }
    private void SetBusy(bool busy)
    {
        GenerateButton.Enabled = !busy; DescriptionBox.Enabled = !busy;
        foreach (var box in ModelBoxes) box.Enabled = !busy;
        foreach (var check in RoleChecks) check.Enabled = !busy;
        BoundsBox.Enabled = !busy; LocksBox.Enabled = !busy;
        PassabilityCheck.Enabled = !busy;
        CancelGenerationButton.Enabled = busy; ApplyButton.Enabled = !busy && _plan is not null;
    }

    internal void ApplyPlan()
    {
        if (_plan is null || _operation is not null || !CanUpdate) return;
        AiMapPlan plan = _plan; _plan = null; ApplyButton.Enabled = false;
        try
        {
            AiMapApplyResult result = _apply(plan);
            StatusLabel.Text = T($"已套用：高度 {result.HeightSamplesChanged} 點、材質 {result.MaterialStrokes - result.RejectedMaterialStrokes}/{result.MaterialStrokes} 區、通行 {result.CollisionPixelsChanged} 點。請檢視地圖後儲存。", $"Applied: {result.HeightSamplesChanged} height samples, {result.MaterialStrokes - result.RejectedMaterialStrokes}/{result.MaterialStrokes} material areas, {result.CollisionPixelsChanged} passability pixels. Review the map, then Save.");
            if (result.RejectedMaterialStrokes > 0) StatusLabel.Text += T(" 部分材質無法以原版 transition tile 表達，已略過。", " Some materials could not be represented with transition tiles and were skipped.");
        }
        catch (Exception ex) { StatusLabel.Text = T("套用失敗，請檢查地圖後重新生成：", "Apply failed; inspect the map before generating again: ") + ex.Message; }
    }

    private string FormatReview(MultiAiMapPlanResult result)
    {
        var text = new StringBuilder();
        foreach (var role in result.Roles)
        {
            text.AppendLine($"{(_isEn ? role.Role.ToString() : MultiAiMapPlanResult.RoleName(role.Role))} / {role.Model}: {role.Status} ({role.AcceptedFeatures})");
            if (role.Error is not null) text.AppendLine(role.Error);
            foreach (string warning in role.Warnings) text.AppendLine(warning);
        }
        if (result.Plan is not { } plan) return text.ToString();
        text.AppendLine().AppendLine(plan.Summary);
        if (plan.BaseHeight is not null) text.AppendLine(T("可編輯區基礎高度：", "Editable-area base height: ") + plan.BaseHeight);
        if (plan.BaseMaterial is not null) text.AppendLine(T("可編輯區基礎材質：", "Editable-area base material: ") + plan.BaseMaterial);
        foreach (var feature in plan.Features)
            text.AppendLine($"{feature.Type}: {feature.Location ?? $"({feature.X},{feature.Y})"} → {feature.ToLocation ?? (feature.X2 is null ? "—" : $"({feature.X2},{feature.Y2})")}; radius={feature.Radius}; amount={feature.Amount}; material={feature.Material}");
        return text.ToString();
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing && !_closing) { _closing = true; _lifetime.Cancel(); _operation?.Cancel(); }
        if (disposing) { ClearPreviewImage(); _lifetime.Dispose(); }
        base.Dispose(disposing);
    }
}
