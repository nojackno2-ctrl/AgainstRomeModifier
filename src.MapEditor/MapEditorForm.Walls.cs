using AgainstRomeMapEditor.Modules.Fortification;
using AgainstRomeMapEditor.Modules.Placement;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private const float WallTileWorld = SdlSceneCatalog.WorldUnitsPerMapPixel * 4f;

    /// <summary>沿圖格折線放置城牆／柵欄（轉角、城門自動選片）；單次 Undo 還原。回傳放置數量。</summary>
    internal int ApplyWallTool(IReadOnlyList<(int X, int Y)> path, int team = 0)
    {
        bool en = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        if (_selected?.IsCustom != true || _texturesDocument is null || _objectCatalog.Count == 0)
        {
            _status.Text = en ? "Object catalog is not available yet." : "物件目錄尚未可用。";
            return 0;
        }
        // 原版聚落實測：柵欄間距 64、角度 0；日耳曼南北向 Pal00／東西向 Pal01，羅馬相反；轉角 Pal02。
        (string AlongZ, string AlongX, string Corner)? variant = new[]
        {
            ("BauGerPal00_Palisade", "BauGerPal01_Palisade", "BauGerPal02_Palisadenecke"),
            ("BauRomPal01_Palisade", "BauRomPal00_Palisade", "BauRomPal02_Palisadenecke"),
        }.Cast<(string, string, string)?>().FirstOrDefault(set => LayoutTypeResolver.ResolveAlias(_objectCatalog, set!.Value.Item1) is not null
            && LayoutTypeResolver.ResolveAlias(_objectCatalog, set.Value.Item2) is not null && LayoutTypeResolver.ResolveAlias(_objectCatalog, set.Value.Item3) is not null);
        if (variant is null)
        {
            _status.Text = en ? "This game has no wall or palisade pieces available." : "此遊戲目錄找不到可用的城牆／柵欄物件。";
            return 0;
        }
        var pieces = PalisadeRunPlanner.Plan(path.Select(point => (point.X * WallTileWorld, point.Y * WallTileWorld)).ToArray(),
            variant.Value.AlongZ, variant.Value.AlongX, variant.Value.Corner);
        if (pieces.Count == 0)
        {
            _status.Text = en ? "Wall planning failed." : "城牆規劃失敗。";
            return 0;
        }
        var preset = new MapLayoutPreset(1, MapLayoutKind.Placement, pieces
            .Select(item => new MapLayoutEntry(LayoutTypeResolver.ResolveAlias(_objectCatalog, item.Name)!, item.X, item.Z, 0, 0, team)).ToArray());
        var planned = MapLayoutPresets.PlanPlacements(preset, _objectCatalog, 0, 0, 0, LayoutGroundHeight);
        CommitStroke();
        _placementSession.AddMany(planned);
        _lastActionWasPlacementTool = true;
        _lastActionWasPlacementToolUndone = false;
        SetEditMode(EditMode.PlaceObject);
        RefreshPlacedList(); RefreshSceneMarkers(); UpdateEditorState();
        _status.Text = en ? $"Wall placed: {planned.Count} pieces." : $"已放置城牆：{planned.Count} 件。";
        return planned.Count;
    }
}
