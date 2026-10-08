using AgainstRomeMapEditor.Modules.Nature;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    /// <summary>以生態圈引擎在圖格矩形內散播多層次植被；單次交易，Undo 一次還原。</summary>
    internal int ApplyFloraScatter(Rectangle tiles, int seed = 1337)
    {
        bool en = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        if (_selected?.IsCustom != true || _texturesDocument is null || !_natureStoreAvailable || _natureTemplates.Count == 0)
        {
            _status.Text = en ? "Nature catalog is not available yet." : "自然物件目錄尚未可用。";
            return 0;
        }
        float tileWorld = 64f;
        var layers = _terrainLayers;
        var context = new FloraScatterContext
        {
            Dimension = _texturesDocument.Dimension,
            TileWorldSize = tileWorld,
            Heights = layers?.Heights,
            VertexSize = layers?.VertexSize ?? 257,
            HeightMapStep = _heightMapStep,
            WaterLevel = (byte)Math.Clamp((int)_waterLevel.Value, 0, 255),
            Collision = layers?.Collision,
            ExistingAdditions = _natureAdditions.Select(item => (item.X, item.Z))
                .Concat(_levelObjects.Where(item => IsRemovableNature(item) && !_natureRemovals.Contains(item.Slot)).Select(item => (item.X, item.Z))).ToArray(),
            Profile = BiomeEcologyProfile.GermanicForest,
            Templates = _natureTemplates,
            ObjDefNames = _objdefNames
        };
        var result = FloraScatterEngine.Generate(context, new FloraScatterParameters
        {
            Seed = seed, MinTileX = tiles.Left, MinTileY = tiles.Top, MaxTileX = tiles.Right - 1, MaxTileY = tiles.Bottom - 1
        });
        CommitStroke();
        if (result.Additions.Count == 0 || !_natureSession.PlantMany(result.Additions))
        {
            _status.Text = en ? "No flora could be placed in the selected rectangle." : "選取矩形內無法散播植被。";
            return 0;
        }
        _lastActionWasFlora = true;
        _lastActionWasFloraUndone = false;
        RefreshSceneMarkers();
        UpdateEditorState();
        _status.Text = en ? $"Flora scattered: {result.Report.TotalPlanted} plants." : $"已散播植被：{result.Report.TotalPlanted} 株。";
        return result.Additions.Count;
    }
}
