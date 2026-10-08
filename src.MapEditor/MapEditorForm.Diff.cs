using AgainstRomeMapEditor.Modules.Diff;
using AgainstRomeModifier.Maps;
using AgainstRomeModifier.Scripting;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    /// <summary>比較目前編輯內容與磁碟上已儲存版本（地形、材質、通行、放置物件、事件），回傳摘要文字。</summary>
    internal string BuildSavedDiffReport()
    {
        bool en = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        if (_selected is null || _texturesDocument is null || _terrainLayers is not { } layers)
            return en ? "Open a map first." : "請先開啟地圖。";
        ScenarioDocument saved;
        try { saved = ScenarioDocument.Load(_selected.DirectoryPath); }
        catch (System.Text.Json.JsonException) { saved = new ScenarioDocument(); }

        var baselineObjects = saved.Spawns.Select(spawn => new DiffObjectItem(spawn.Id, -1, 0, spawn.Alias, -1, spawn.Team,
            spawn.X, spawn.Y, spawn.Z, spawn.Angle, spawn.Count, spawn.Prebuilt)).ToArray();
        var currentObjects = _placedObjects.Select(item => new DiffObjectItem(item.ScenarioId, -1, 0, AliasOf(item.Type), item.Type.Definition,
            item.Team, item.WorldX, item.WorldY, item.WorldZ, item.Angle, Math.Max(0, item.UnitCount),
            item.Type.Category == SdlObjectCategory.Building && item.Team is >= 0 and <= 8)).ToArray();

        var baseline = new MapVersionSnapshot(en ? "Saved" : "已儲存版本", _texturesDocument.Dimension, layers.VertexSize, layers.BaselineHeights,
            _heightMapStep, _savedTextures, layers.CollisionSize, layers.BaselineCollision, baselineObjects, saved.Events);
        var current = new MapVersionSnapshot(en ? "Editing" : "目前編輯", _texturesDocument.Dimension, layers.VertexSize, layers.Heights,
            _heightMapStep, _texturesDocument.Textures.ToArray(), layers.CollisionSize, layers.Collision, currentObjects, EventSession.Capture());
        var report = MapDiffEngine.Compare(baseline, current);
        return report.HasDifferences ? report.GenerateSummaryText() : (en ? "No differences from the saved version." : "與已儲存版本沒有差異。");
    }
}
