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
        return base.ProcessCmdKey(ref msg, keyData);
    }

    internal Func<TerrainRegionDialog, DialogResult> RegionDialogRunner { get; set; } = dialog => dialog.ShowDialog();

    internal void RunRegionTool()
    {
        if (_selected?.IsCustom != true || _texturesDocument is null || _terrainBlendSession is null) return;
        using var dialog = new TerrainRegionDialog(_texturesDocument.Dimension);
        if (RegionDialogRunner(dialog) != DialogResult.OK) return;
        try { ApplyRegionTool(dialog.Operation, dialog.ReadVertices(), dialog.RoadWidth); }
        catch (ArgumentException ex) { _status.Text = ex.Message; }
    }

    internal void ApplyRegionTool(TerrainRegionOperation operation, IReadOnlyList<(int X, int Y)> vertices, int width)
    {
        if (_selected?.IsCustom != true || _texturesDocument is null || _terrainBlendSession is null) return;
        int dimension = _texturesDocument.Dimension;
        int count = operation == TerrainRegionOperation.Connected ? 1 : 2;
        if (vertices.Count < count || (operation != TerrainRegionOperation.Road && vertices.Count != count)) throw new ArgumentException("Invalid vertex count.");
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
