using AgainstRomeModifier;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{

    private void OpenAiMapDialog()
    {
        if (_selected?.IsCustom != true || _terrainLayers is null || _texturesDocument is null) return;
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        using var dialog = new Form
        {
            Text = isEn ? "AI Map Maker (local Ollama)" : "AI 製圖（本機 Ollama）",
            Width = 640, Height = 520, StartPosition = FormStartPosition.CenterParent, BackColor = BackColor, ForeColor = ForeColor,
            FormBorderStyle = FormBorderStyle.Sizable, MinimizeBox = false, MaximizeBox = false, ShowInTaskbar = false, MinimumSize = new Size(520, 420),
        };
        var layout = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 2, Padding = new Padding(12) };
        layout.ColumnStyles.Add(new ColumnStyle(SizeType.Absolute, 90)); layout.ColumnStyles.Add(new ColumnStyle(SizeType.Percent, 100));
        layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 36)); layout.RowStyles.Add(new RowStyle(SizeType.Percent, 100)); layout.RowStyles.Add(new RowStyle(SizeType.Absolute, 92));
        var modelBox = new ComboBox { Dock = DockStyle.Fill, DropDownStyle = ComboBoxStyle.DropDown };
        var prompt = new TextBox
        {
            Dock = DockStyle.Fill, Multiline = true, ScrollBars = ScrollBars.Vertical, AcceptsReturn = true,
            Text = isEn
                ? "A green river valley running from the north-west to the south-east, a high mountain range in the north-east, a small lake in the south-west, sandy ground near the river and several wide flat meadows for villages."
                : "一條從西北流向東南的河谷，東北方是高聳山脈，西南有一座小湖，河岸是沙地，另外保留幾片寬廣平坦的草地讓村莊發展。",
        };
        var status = new Label
        {
            Dock = DockStyle.Fill, ForeColor = WinFormsTheme.TextSecondary,
            Text = isEn
                ? "The plan is applied as one undoable step for heights (Terrain Height mode) and textures (Texture mode). Nothing is written until you click Save."
                : "結果會套用為可復原的步驟（高度在「地形高度」模式復原、材質在「材質筆刷」模式復原）；按「儲存」前不會寫入任何檔案。",
        };
        layout.Controls.Add(new Label { Text = isEn ? "Model" : "模型", AutoSize = true, Anchor = AnchorStyles.Left }, 0, 0); layout.Controls.Add(modelBox, 1, 0);
        layout.Controls.Add(new Label { Text = isEn ? "Description" : "地圖描述", AutoSize = true, Anchor = AnchorStyles.Left | AnchorStyles.Top }, 0, 1); layout.Controls.Add(prompt, 1, 1);
        layout.Controls.Add(status, 0, 2); layout.SetColumnSpan(status, 2);
        var buttons = new FlowLayoutPanel { Dock = DockStyle.Bottom, Height = 52, Padding = new Padding(12, 8, 12, 8), FlowDirection = FlowDirection.RightToLeft };
        var generate = new Button { Text = isEn ? "Generate && Apply" : "生成並套用", Width = 130, Height = 34 };
        var close = new Button { Text = isEn ? "Close" : "關閉", DialogResult = DialogResult.Cancel, Width = 90, Height = 34 };
        buttons.Controls.Add(generate); buttons.Controls.Add(close);
        dialog.Controls.Add(layout); dialog.Controls.Add(buttons); dialog.CancelButton = close;
        WinFormsTheme.Apply(dialog); WinFormsTheme.StylePrimaryButton(generate);

        using var planner = new OllamaMapPlanner();
        using var cancellation = new CancellationTokenSource();
        dialog.FormClosing += (_, _) => cancellation.Cancel();
        dialog.Shown += async (_, _) =>
        {
            try
            {
                IReadOnlyList<string> models = await planner.ListModelsAsync(cancellation.Token);
                modelBox.Items.AddRange(models.Cast<object>().ToArray());
                string? preferred = models.FirstOrDefault(name => name.StartsWith("gemma", StringComparison.OrdinalIgnoreCase) || name.StartsWith("qwen", StringComparison.OrdinalIgnoreCase)) ?? (models.Count > 0 ? models[0] : null);
                if (preferred is not null) modelBox.Text = preferred;
                if (models.Count == 0) status.Text = isEn ? "Ollama has no models. Install one first, e.g. `ollama pull gemma3:12b`." : "Ollama 尚未安裝任何模型，請先執行例如 `ollama pull gemma3:12b`。";
            }
            catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or System.Text.Json.JsonException)
            {
                status.Text = (isEn ? $"Cannot reach Ollama at {planner.Endpoint}. Start Ollama and reopen this dialog. " : $"無法連線到 {planner.Endpoint} 的 Ollama，請先啟動 Ollama 再開啟此視窗。") + ex.Message;
            }
        };
        generate.Click += async (_, _) =>
        {
            generate.Enabled = false; prompt.Enabled = false; modelBox.Enabled = false;
            status.Text = isEn ? $"Generating with {modelBox.Text}… (large local models can take a minute)" : $"正在以 {modelBox.Text} 規劃地圖…（大型本機模型可能需要一分鐘）";
            try
            {
                AiMaterialOption[] materials = _floorMaterials?.Materials.Select(material => new AiMaterialOption(material.Id, material.DisplayName + " / " + GetLocalizedMaterialName(material))).ToArray() ?? Array.Empty<AiMaterialOption>();
                float water = _heightMapStep > 0 ? (float)_waterLevel.Value / _heightMapStep : 0;
                (AiMapPlan plan, _) = await planner.GeneratePlanAsync(modelBox.Text.Trim(), prompt.Text, materials, water, cancellation.Token);
                AiMapApplyResult result = ApplyAiMapPlan(plan);
                status.Text = (plan.Summary is { Length: > 0 } summary ? summary + Environment.NewLine : "") + (isEn
                    ? $"Applied {plan.Features.Count} features: {result.HeightSamplesChanged} height samples, {result.MaterialStrokes - result.RejectedMaterialStrokes}/{result.MaterialStrokes} material areas, {result.CollisionPixelsChanged} passability pixels. Review, then Save."
                    : $"已套用 {plan.Features.Count} 個特徵：高度 {result.HeightSamplesChanged} 點、材質 {result.MaterialStrokes - result.RejectedMaterialStrokes}/{result.MaterialStrokes} 區、通行 {result.CollisionPixelsChanged} 點。請檢視後再按「儲存」。");
                if (result.RejectedMaterialStrokes > 0)
                    status.Text += isEn ? " Some material areas could not be expressed with the game's transition tiles and were skipped." : "部分材質區無法以原版 transition tile 表達，已略過。";
            }
            catch (OperationCanceledException) { }
            catch (Exception ex) when (ex is HttpRequestException or InvalidDataException or ArgumentException or System.Text.Json.JsonException or KeyNotFoundException)
            {
                status.Text = (isEn ? "AI map generation failed: " : "AI 製圖失敗：") + ex.Message;
            }
            finally
            {
                if (!dialog.IsDisposed) { generate.Enabled = true; prompt.Enabled = true; modelBox.Enabled = true; }
            }
        };
        dialog.ShowDialog(this);
    }

    /// <summary>把 AI 計畫套用到目前的高度／材質／通行狀態（每類各成一個可復原步驟），並更新 2D／3D 預覽。</summary>
    internal AiMapApplyResult ApplyAiMapPlan(AiMapPlan plan)
    {
        if (_terrainLayers is null || _texturesDocument is null) throw new InvalidOperationException("此地圖沒有可編輯的高度圖。");
        CommitStroke();
        float water = _heightMapStep > 0 ? (float)_waterLevel.Value / _heightMapStep : 0;
        AiMapApplyResult result = AiMapPlanApplier.Apply(plan, _terrainLayers, _texturesDocument.Dimension, water, (materialId, x, y, radius) =>
        {
            if (_terrainBlendSession is null) return false;
            TerrainBlendPaintResult paint = _terrainBlendSession.PaintCircle(x, y, radius, materialId);
            foreach (TerrainTextureChange change in paint.TextureChanges) ApplyTexture(change.X, change.Y, change.After);
            return paint.Succeeded;
        });
        _terrainLayers.CommitStroke();
        _terrainBlendSession?.CommitStroke();
        ApplyHeightsToViews();
        if (_editMode == EditMode.Collision) _canvas.SetCollisionOverlay(_terrainLayers.CollisionSize, _terrainLayers.Collision);
        UpdateEditorState();
        return result;
    }
}
