using System.Text;
using AgainstRomeModifier;

namespace AgainstRomeMapEditor;

/// <summary>Produces a reviewable plan; only the explicit Apply command changes the host.</summary>
internal sealed class AiMapPlanningDialog : Form
{
    private readonly Func<CancellationToken, Task<IReadOnlyList<string>>> _listModels;
    private readonly Func<IReadOnlyList<MultiAiMapRoleRequest>, string, CancellationToken, Task<MultiAiMapPlanResult>> _generate;
    private readonly Func<AiMapPlan, AiMapApplyResult> _apply;
    private readonly CancellationTokenSource _lifetime = new();
    private CancellationTokenSource? _operation;
    private AiMapPlan? _plan;
    private bool _closing;
    private bool _loading;
    private bool _generationStarted;
    private int _inputVersion;
    private readonly bool _isEn;
    internal ComboBox[] ModelBoxes { get; } = Enumerable.Range(0, 3).Select(_ => new ComboBox { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDown, Text = "laguna-xs-2.1:latest" }).ToArray();
    internal TextBox DescriptionBox { get; } = new() { Dock = DockStyle.Fill, Multiline = true, ScrollBars = ScrollBars.Vertical, AcceptsReturn = true };
    internal TextBox PreviewBox { get; } = new() { Dock = DockStyle.Fill, Multiline = true, ReadOnly = true, ScrollBars = ScrollBars.Both, WordWrap = false };
    internal Label StatusLabel { get; } = new() { Dock = DockStyle.Fill, AutoSize = false };
    internal Button GenerateButton { get; } = new() { AutoSize = true };
    internal Button ApplyButton { get; } = new() { AutoSize = true, Enabled = false };
    internal Button CancelGenerationButton { get; } = new() { AutoSize = true, Enabled = false };
    private bool CanUpdate => !_closing && !IsDisposed && !Disposing;
    private string T(string zh, string en) => _isEn ? en : zh;

    internal AiMapPlanningDialog(Func<CancellationToken, Task<IReadOnlyList<string>>> listModels,
        Func<IReadOnlyList<MultiAiMapRoleRequest>, string, CancellationToken, Task<MultiAiMapPlanResult>> generate,
        Func<AiMapPlan, AiMapApplyResult> apply)
    {
        _listModels = listModels; _generate = generate; _apply = apply;
        _isEn = Loc.CurrentLanguage == Language.English;
        Text = T("多 AI 製圖（本機 Ollama）", "Multi-AI Map Maker (local Ollama)");
        Size = new Size(880, 740); MinimumSize = new Size(640, 580); StartPosition = FormStartPosition.CenterParent;
        MinimizeBox = false; MaximizeBox = false; ShowInTaskbar = false;
        var layout = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 2, Padding = new Padding(12), RowCount = 7 };
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 110)); layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        for (int i = 0; i < 3; i++)
        {
            layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 36));
            string role = _isEn ? ((AiMapDesignRole)i).ToString() : MultiAiMapPlanResult.RoleName((AiMapDesignRole)i);
            layout.Controls.Add(new Label { Text = role + T("模型", " model"), AutoSize = true, Anchor = AnchorStyles.Left }, 0, i);
            layout.Controls.Add(ModelBoxes[i], 1, i);
        }
        layout.RowStyles.Add(new RowStyle(SizeType.Percent, 28));
        layout.RowStyles.Add(new RowStyle(SizeType.Percent, 72));
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 70));
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 45));
        DescriptionBox.Text = T("一條從西北流向東南的河谷，東北方是山脈，西南有小湖；河岸是沙地，保留寬廣平坦的草地讓村莊發展。", "A river valley from north-west to south-east, mountains in the north-east, a small lake in the south-west, sandy riverbanks and wide flat dry meadows for villages.");
        layout.Controls.Add(new Label { Text = T("地圖描述", "Description"), AutoSize = true }, 0, 3); layout.Controls.Add(DescriptionBox, 1, 3);
        layout.Controls.Add(new Label { Text = T("方案檢視", "Plan review"), AutoSize = true }, 0, 4); layout.Controls.Add(PreviewBox, 1, 4);
        StatusLabel.Text = T("生成後先檢視方案，再按「套用方案」。可復原；按「儲存」前不會寫入檔案。", "Generate, review, then Apply. Changes are undoable and no files are written until Save.");
        layout.Controls.Add(StatusLabel, 0, 5); layout.SetColumnSpan(StatusLabel, 2);
        var buttons = new FlowLayoutPanel { Dock = DockStyle.Fill, FlowDirection = FlowDirection.RightToLeft };
        var close = new Button { Text = T("關閉", "Close"), AutoSize = true, DialogResult = DialogResult.Cancel };
        GenerateButton.Text = T("生成／重試", "Generate / Retry"); ApplyButton.Text = T("套用方案", "Apply plan"); CancelGenerationButton.Text = T("取消生成", "Cancel generation");
        buttons.Controls.AddRange([close, ApplyButton, CancelGenerationButton, GenerateButton]);
        layout.Controls.Add(buttons, 0, 6); layout.SetColumnSpan(buttons, 2); Controls.Add(layout); CancelButton = close;
        WinFormsTheme.Apply(this); WinFormsTheme.StylePrimaryButton(ApplyButton);
        DescriptionBox.TextChanged += (_, _) => InvalidatePlan();
        foreach (var box in ModelBoxes) box.TextChanged += (_, _) => InvalidatePlan();
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
        _generationStarted = true; _plan = null; ApplyButton.Enabled = false; PreviewBox.Clear();
        using var operation = CancellationTokenSource.CreateLinkedTokenSource(_lifetime.Token);
        _operation = operation; SetBusy(true);
        StatusLabel.Text = T("地形、水系、材質依序規劃，一次只執行一個本機模型；可能需要數分鐘。", "Terrain, water and materials plan in order, with one local inference at a time; this may take several minutes.");
        try
        {
            var requests = ModelBoxes.Select((box, i) => new MultiAiMapRoleRequest((AiMapDesignRole)i, box.Text.Trim())).ToArray();
            int inputVersion = _inputVersion;
            MultiAiMapPlanResult result = await _generate(requests, DescriptionBox.Text, operation.Token);
            if (!CanUpdate) return;
            if (inputVersion != _inputVersion) { ShowInvalidatedPlan(); return; }
            // A provider can finish after Cancel; its result must still never become applicable.
            if (operation.IsCancellationRequested || result.IsCancelled)
            {
                StatusLabel.Text = T("已取消生成；可重試。", "Generation cancelled; you can retry."); return;
            }
            _plan = result.Plan;
            PreviewBox.Text = FormatReview(result);
            StatusLabel.Text = result.Plan is null ? T("全部角色失敗；請檢視診斷後重試。", "All specialists failed. Review diagnostics and retry.")
                : result.HasFailures ? T("部分角色失敗：這是未完整的方案。請檢視診斷，再決定套用或重試。", "Some specialists failed: this is a partial plan. Review diagnostics before applying or retrying.")
                : T("方案已生成，尚未套用。請先檢視，再按「套用方案」。", "Plan generated and awaiting review. Click Apply to change the map.");
        }
        catch (OperationCanceledException) { if (CanUpdate) StatusLabel.Text = T("已取消生成；可重試。", "Generation cancelled; you can retry."); }
        catch (Exception ex) { if (CanUpdate) StatusLabel.Text = T("生成失敗；可重試：", "Generation failed; you can retry: ") + ex.Message; }
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
        StatusLabel.Text = T("描述或角色模型已變更，舊方案已失效。請重新生成。", "The description or specialist model changed; the old plan is invalid. Generate again.");
    }
    private void SetBusy(bool busy)
    {
        GenerateButton.Enabled = !busy; DescriptionBox.Enabled = !busy;
        foreach (var box in ModelBoxes) box.Enabled = !busy;
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
        if (plan.BaseHeight is not null) text.AppendLine(T("全圖基礎高度：", "Whole-map base height: ") + plan.BaseHeight);
        if (plan.BaseMaterial is not null) text.AppendLine(T("全圖基礎材質：", "Whole-map base material: ") + plan.BaseMaterial);
        foreach (var feature in plan.Features)
            text.AppendLine($"{feature.Type}: {feature.Location ?? $"({feature.X},{feature.Y})"} → {feature.ToLocation ?? (feature.X2 is null ? "—" : $"({feature.X2},{feature.Y2})")}; radius={feature.Radius}; amount={feature.Amount}; material={feature.Material}");
        return text.ToString();
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing && !_closing) { _closing = true; _lifetime.Cancel(); _operation?.Cancel(); }
        if (disposing) _lifetime.Dispose();
        base.Dispose(disposing);
    }
}
