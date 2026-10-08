using AgainstRomeMapEditor.Modules.Nature;
using AgainstRomeModifier.Maps;

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
        const float tileWorld = SdlSceneCatalog.WorldUnitsPerMapPixel * 4f; // 貼圖格（64×64）= 4 個碰撞像素
        var layers = _terrainLayers;
        var context = new FloraScatterContext
        {
            Dimension = _texturesDocument.Dimension,
            TileWorldSize = tileWorld,
            Heights = layers?.Heights,
            VertexSize = layers?.VertexSize ?? 257,
            HeightMapStep = _heightMapStep,
            WaterLevel = (byte)Math.Clamp((int)Math.Round((float)_waterLevel.Value / _heightMapStep), 0, 255), // boden.ini 的 Waterlevel 是世界高度；引擎用高度位元組
            // 碰撞圖是 256×256，與貼圖格索引不同；避障改由既有物件座標處理。
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
            int species = context.Profile.ResolveTemplates(context.Templates, context.ObjDefNames).TotalSpeciesCount;
            _status.Text = species == 0
                ? (en ? "No flora species of this biome exist in the game catalog." : "遊戲目錄中找不到此生態圈的植被物種。")
                : (en ? $"No flora could be placed in the selected rectangle ({species} species available; terrain may be water or too steep)." : $"選取矩形內無法散播植被（可用物種 {species} 種；可能是水域或坡度太陡）。");
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
