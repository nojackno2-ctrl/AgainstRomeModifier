namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private void OpenAiMapDialog()
    {
        if (_selected?.IsCustom != true || _terrainLayers is null || _texturesDocument is null) return;
        using var planner = new OllamaMapPlanner();
        var team = new MultiAiMapPlanner(planner);
        AiMaterialOption[] materials = _floorMaterials?.Materials.Select(material => new AiMaterialOption(material.Id, material.DisplayName + " / " + GetLocalizedMaterialName(material))).ToArray() ?? [];
        float water = _heightMapStep > 0 ? (float)_waterLevel.Value / _heightMapStep : 0;
        using var dialog = new AiMapPlanningDialog(planner.ListModelsAsync,
            (requests, description, token) => team.GeneratePlanAsync(requests, description, materials, water, token),
            ApplyAiMapPlan);
        dialog.ShowDialog(this);
    }

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
