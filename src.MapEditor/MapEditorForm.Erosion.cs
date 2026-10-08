namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    /// <summary>在圖格矩形內執行水力＋熱力侵蝕；只改矩形內高度，單次 Undo 還原。</summary>
    internal int ApplyErosionTool(Rectangle tiles)
    {
        bool en = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        if (_selected?.IsCustom != true || _texturesDocument is null || _terrainLayers is not { } layers)
        {
            _status.Text = en ? "Terrain heights are not available." : "地形高度尚不可用。";
            return 0;
        }
        int step = Math.Max(1, (layers.VertexSize - 1) / _texturesDocument.Dimension);
        int minV = tiles.Left * step, maxV = tiles.Right * step, minZ = tiles.Top * step, maxZ = tiles.Bottom * step;
        float cx = (minV + maxV) / 2f, cz = (minZ + maxZ) / 2f, radius = MathF.Max(maxV - minV, maxZ - minZ) / 2f + 1f;
        CommitStroke();
        var buffer = new TerrainHeightWorkBuffer(layers.VertexSize, layers.Heights);
        HydraulicErosionSimulator.Simulate(buffer, new HydraulicErosionParams { DropletCount = 8000 }, cx, cz, radius);
        ThermalErosionSimulator.Simulate(buffer, null, cx, cz, radius);
        int size = layers.VertexSize;
        var inside = buffer.ExtractChanges(layers.Heights)
            .Where(change => change.Index % size is var x && change.Index / size is var z && x >= minV && x <= maxV && z >= minZ && z <= maxZ).ToArray();
        if (inside.Length == 0 || layers.ApplySampleChanges(inside).Count == 0)
        {
            _status.Text = en ? "Erosion made no visible change." : "侵蝕沒有造成可見變化。";
            return 0;
        }
        layers.CommitStroke();
        _lastActionWasHeightTool = true;
        _lastActionWasHeightToolUndone = false;
        ApplyHeightsToViews();
        UpdateEditorState();
        _status.Text = en ? $"Erosion applied: {inside.Length} height samples." : $"已套用侵蝕：{inside.Length} 個高度點。";
        return inside.Length;
    }
}
