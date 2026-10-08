using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private readonly ToolStripButton _regionToolsButton = new("區域工具…") { Enabled = false };
    private readonly ToolStripButton _boxSelectButton = new("框選") { Enabled = false };
    private (int X, int Y)? _boxStart, _boxEnd;
    private bool _ignoreCancelledBoxStroke;

    internal void BeginBoxSelection()
    {
        CommitStroke(); SetEditMode(EditMode.Texture); _boxStart = _boxEnd = null; _boxSelectButton.Checked = true;
        SetBrushToken(TerrainToolBrushToken); _canvas.ContinuousPaint = true;
        if (_view3d is not null) _view3d.ContinuousPaint = true;
        _status.Text = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English ? "Drag to select placed objects; Escape cancels." : "拖曳選取放置物件；Escape 取消。";
    }

    private void ShowBoxSelection(Rectangle? rectangle)
    {
        _canvas.SelectionTiles = rectangle; _canvas.Invalidate();
        if (_view3d is not null) { _view3d.SelectionTiles = rectangle; _view3d.Invalidate(); }
    }

    protected override bool ProcessCmdKey(ref Message msg, Keys keyData)
    {
        if (keyData == Keys.Escape && _boxSelectButton.Checked)
        { SetEditMode(EditMode.Texture); _ignoreCancelledBoxStroke = true; return true; }
        if (keyData == Keys.Escape && _autoRoad.Checked && _roadStrokePath.Count > 0)
        { CancelRoadStroke(); return true; }
        return base.ProcessCmdKey(ref msg, keyData);
    }

    internal Func<TerrainRegionDialog, DialogResult> RegionDialogRunner { get; set; } = dialog => dialog.ShowDialog();

    internal void RunRegionTool()
    {
        if (_selected?.IsCustom != true || _texturesDocument is null || _terrainBlendSession is null) return;
        using var dialog = new TerrainRegionDialog(_texturesDocument.Dimension);
        if (_canvas.SelectionTiles is { } sel)
        {
            dialog.VerticesText = $"{sel.Left},{sel.Top}\r\n{sel.Right - 1},{sel.Bottom - 1}";
        }
        if (RegionDialogRunner(dialog) != DialogResult.OK) return;
        try { ApplyRegionTool(dialog.Operation, dialog.ReadVertices(), dialog.RoadWidth); }
        catch (ArgumentException ex) { _status.Text = ex.Message; }
    }

    internal void ApplyRegionTool(TerrainRegionOperation operation, IReadOnlyList<(int X, int Y)> vertices, int width)
    {
        if (_selected?.IsCustom != true || _texturesDocument is null || _terrainBlendSession is null) return;
        int dimension = _texturesDocument.Dimension;
        int count = operation == TerrainRegionOperation.Connected ? 1 : 2;
        if (vertices.Count < count || (operation != TerrainRegionOperation.Road && operation != TerrainRegionOperation.River && operation != TerrainRegionOperation.Wall && vertices.Count != count)) throw new ArgumentException("Invalid vertex count.");

        if (operation == TerrainRegionOperation.River)
        {
            ApplyRiverTool(vertices);
            return;
        }

        if (operation == TerrainRegionOperation.Wall)
        {
            ApplyWallTool(vertices);
            return;
        }

        if (operation == TerrainRegionOperation.Erosion)
        {
            ApplyErosionTool(Rectangle.FromLTRB(Math.Min(vertices[0].X, vertices[1].X), Math.Min(vertices[0].Y, vertices[1].Y),
                Math.Max(vertices[0].X, vertices[1].X) + 1, Math.Max(vertices[0].Y, vertices[1].Y) + 1));
            return;
        }

        if (operation == TerrainRegionOperation.Flora)
        {
            ApplyFloraScatter(Rectangle.FromLTRB(Math.Min(vertices[0].X, vertices[1].X), Math.Min(vertices[0].Y, vertices[1].Y),
                Math.Max(vertices[0].X, vertices[1].X) + 1, Math.Max(vertices[0].Y, vertices[1].Y) + 1));
            return;
        }

        if (operation == TerrainRegionOperation.Cliff)
        {
            int minX = Math.Min(vertices[0].X, vertices[1].X);
            int minY = Math.Min(vertices[0].Y, vertices[1].Y);
            int maxX = Math.Max(vertices[0].X, vertices[1].X);
            int maxY = Math.Max(vertices[0].Y, vertices[1].Y);
            ApplyCliffTool(Rectangle.FromLTRB(minX, minY, maxX + 1, maxY + 1));
            return;
        }

        var cells = operation switch
        {
            TerrainRegionOperation.Road => TerrainRegionPlanner.Polyline(dimension, vertices, width),
            TerrainRegionOperation.Connected => TerrainRegionPlanner.Connected(dimension,
                _terrainBlendSession.CurrentTextures.Select(texture => _floorMaterials?.FindByTexture(texture)?.Id ?? texture).ToArray(), vertices[0]),
            _ => TerrainRegionPlanner.Rectangle(dimension, vertices[0], vertices[1])
        };
        if (operation == TerrainRegionOperation.SelectObjects)
        {
            int left = cells.Min(c => c.X), top = cells.Min(c => c.Y), right = cells.Max(c => c.X) + 1, bottom = cells.Max(c => c.Y) + 1;
            RefreshPlacedList();
            foreach (ListViewItem row in _placedList.Items)
            {
                var item = _placementSession[(int)row.Tag!];
                float x = item.WorldX / 256f, y = item.WorldZ / 256f;
                row.Selected = x >= left && x < right && y >= top && y < bottom;
            }
            _inspectorTabs.SelectedIndex = 3; SetEditMode(EditMode.PlaceObject);
            return;
        }
        string? material = operation == TerrainRegionOperation.Road ? FloorMaterialCatalog.PathMaterialId : _activeMaterial?.Id;
        if (material is null || _floorMaterials?.Materials.Any(m => m.Id == material) != true)
            throw new ArgumentException(AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English ? "Select an available material first; dirt roads require PFAD assets." : "請先選擇可用材質；土路需要 PFAD 素材。");
        CommitStroke();
        var result = _terrainBlendSession.PaintTiles(cells, material, _autoBridge.Checked);
        if (result.TextureChanges.Count > 0) _texturesDocument.SetTextures(_terrainBlendSession.CurrentTextures);
        foreach (var change in result.TextureChanges)
        { _canvas.SetTexture(change.X, change.Y, change.After); _view3d?.SetTexture(change.X, change.Y, change.After); }
        if (result.Succeeded) _terrainBlendSession.CommitStroke();
        bool en = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        _terrainBlendNotice = result.Succeeded ? null : en ? $"Unsupported native transition at {result.Issues[0].TileX},{result.Issues[0].TileY}; region unchanged."
            : $"圖格 {result.Issues[0].TileX},{result.Issues[0].TileY} 缺少原版接縫；區域未修改。";
        SetEditMode(EditMode.Texture); UpdateEditorState();
    }

    internal void ApplyRiverTool(IReadOnlyList<(int X, int Y)> path, RiverPlannerOptions? options = null)
    {
        if (_selected?.IsCustom != true || _texturesDocument is null || _terrainBlendSession is null) return;
        bool en = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        options ??= new RiverPlannerOptions { AutoDetectDownhillFlow = true };
        options = options with { WaterLevel = (float)_waterLevel.Value, HeightmapStep = _heightMapStep };

        var available = _floorTextures?.Names ?? _texturesDocument.Textures;
        var catalog = RiverTileCatalog.BuildAvailable(available);

        CommitStroke();
        var result = _terrainBlendSession.PaintRiverPath(path, catalog, options, _terrainLayers);

        if (result.TextureChanges.Count > 0)
        {
            _texturesDocument.SetTextures(_terrainBlendSession.CurrentTextures);
            foreach (var change in result.TextureChanges)
            {
                _canvas.SetTexture(change.X, change.Y, change.After);
                _view3d?.SetTexture(change.X, change.Y, change.After);
            }
        }

        if (result.HeightChanges.Count > 0)
        {
            ApplyHeightsToViews(useCurrentSamples: true);
            _canvas.Invalidate();
            _view3d?.Invalidate();
        }

        if (result.Succeeded)
        {
            _terrainBlendSession.CommitStroke();
            if (_terrainLayers?.CommitStroke() == true) { }
            _lastActionWasRiverOrCliff = true;
            _lastActionWasRiverOrCliffUndone = false;
        }

        SetEditMode(EditMode.Texture);
        UpdateEditorState();

        if (result.Succeeded)
        {
            _status.Text = en
                ? $"River applied: {result.TextureChanges.Count} tiles updated."
                : $"河流已套用：更新了 {result.TextureChanges.Count} 個圖塊。";
        }
        else
        {
            _status.Text = en ? "River planning failed; check that the water level allows at least 6 height bytes of depth." : "河流規劃失敗；請確認水位至少能容納 6 個高度單位的深度。";
        }
    }

    internal void ApplyRiverOnSelectedRectangle(RiverPlannerOptions? options = null)
    {
        if (_canvas.SelectionTiles is { } sel)
        {
            ApplyRiverTool([(sel.Left, sel.Top), (sel.Right - 1, sel.Bottom - 1)], options);
        }
    }

    internal void ApplyCliffOnSelectedRectangle(CliffPlannerOptions? options = null)
    {
        if (_canvas.SelectionTiles is { } sel)
        {
            ApplyCliffTool(sel, options);
        }
    }

    internal void ApplyCliffTool(Rectangle rectangle, CliffPlannerOptions? options = null)
    {
        if (_selected?.IsCustom != true || _texturesDocument is null || _terrainBlendSession is null) return;
        bool en = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        int dimension = _texturesDocument.Dimension;
        options ??= new CliffPlannerOptions();
        if (options.GenerateScree && _floorMaterials?.Materials.Any(m => m.Id == options.ScreeMaterialId) != true)
        {
            options = options with { GenerateScree = false };
        }
        if (options.GenerateCrestTransition && _floorMaterials?.Materials.Any(m => m.Id == options.CrestMaterialId) != true)
        {
            options = options with { GenerateCrestTransition = false };
        }

        if (_terrainLayers is null || _terrainLayers.Heights.Count != _terrainLayers.VertexSize * _terrainLayers.VertexSize)
        {
            _status.Text = en ? "No height data available for cliff detection." : "無高度資料可用於懸崖偵測。";
            return;
        }

        var detection = CliffEdgeDetector.DetectFromVertexHeights(
            vertexSize: _terrainLayers.VertexSize,
            vertexHeights: _terrainLayers.Heights,
            tileDimension: dimension,
            heightmapStep: _heightMapStep > 0 ? (int)MathF.Round(_heightMapStep) : 4);

        var cellsInRect = detection.CliffCells
            .Where(c => c.X >= rectangle.Left && c.X < rectangle.Right && c.Y >= rectangle.Top && c.Y < rectangle.Bottom)
            .ToList();

        if (cellsInRect.Count == 0)
        {
            _status.Text = en ? "No steep slopes detected in selected rectangle." : "選取矩形內未偵測到符合條件的陡坡。";
            return;
        }

        var filteredDetection = new CliffDetectionResult(dimension, cellsInRect, []);
        var catalog = CliffTileCatalog.CreateDefault();
        var plan = CliffFacePlanner.Plan(dimension, filteredDetection, catalog, options);

        CommitStroke();
        var applyResult = CliffFacePlanner.ApplyPlan(_terrainBlendSession, _terrainLayers, plan);

        if (_texturesDocument is not null)
        {
            _texturesDocument.SetTextures(_terrainBlendSession.CurrentTextures);
            _canvas.Invalidate();
            _view3d?.Invalidate();
        }

        if (applyResult.Succeeded)
        {
            _lastActionWasRiverOrCliff = true;
            _lastActionWasRiverOrCliffUndone = false;
        }

        SetEditMode(EditMode.Texture);
        UpdateEditorState();

        if (applyResult.Succeeded)
        {
            _status.Text = en
                ? $"Cliff applied: {applyResult.StampedCliffTiles} cliff tiles, {applyResult.PaintedScreeTiles} scree tiles, {applyResult.BlockedCollisionPixels} collision pixels."
                : $"懸崖已套用：{applyResult.StampedCliffTiles} 個岩壁圖塊、{applyResult.PaintedScreeTiles} 個碎石圖塊、{applyResult.BlockedCollisionPixels} 個阻擋像素。";
        }
        else
        {
            _status.Text = en ? "Cliff planning encountered issues." : "懸崖套用時發生問題。";
        }
    }

    internal void EditSelectedPlacedObjectsBatch()
    {
        if (_selected?.IsCustom != true) return;
        int[] indices = _placedList.SelectedItems.Cast<ListViewItem>().Select(row => (int)row.Tag!).Order().ToArray();
        if (indices.Length == 0) return;
        using var dialog = new PlacementBatchEditDialog(indices.Length, indices.Any(index => _placementSession[index].Type.Category == SdlObjectCategory.Figure));
        if (BatchEditDialogRunner(dialog) != DialogResult.OK) return;
        try { _placementSession.EditMany(indices, dialog.Team, dialog.Angle); }
        catch (ArgumentException ex)
        {
            _status.Text = (AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English ? "Cannot change selected objects: " : "無法修改選取物件：") + ex.Message;
            return;
        }
        RefreshPlacedList(); RefreshSceneMarkers(); UpdateEditorState();
        foreach (int index in indices) _placedList.Items[index].Selected = true;
    }
}
