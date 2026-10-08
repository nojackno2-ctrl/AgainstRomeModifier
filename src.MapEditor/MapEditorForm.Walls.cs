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
        var catalog = new WallTileCatalog().Filtered(name => LayoutTypeResolver.ResolveAlias(_objectCatalog, name) is not null);
        FortificationStyle? style = new[] { FortificationStyle.GermanicPalisade, FortificationStyle.RomanStoneWall, FortificationStyle.CelticPalisade }
            .Cast<FortificationStyle?>().FirstOrDefault(candidate =>
            {
                var parts = catalog.GetComponents(candidate!.Value);
                return parts.Any(part => part.Kind == WallComponentKind.Straight) && parts.Any(part => part.Kind == WallComponentKind.Corner);
            });
        if (style is null)
        {
            _status.Text = en ? "This game has no wall or palisade pieces available." : "此遊戲目錄找不到可用的城牆／柵欄物件。";
            return 0;
        }
        var plan = WallStrokePlanner.Plan(_texturesDocument.Dimension, path.Select(point => (point.X, point.Y)).ToArray(),
            new WallStrokeOptions { Style = style.Value, Team = team, StampFoundations = false }, catalog);
        if (!plan.Succeeded || plan.Placements.Count == 0)
        {
            _status.Text = plan.FailureReason ?? (en ? "Wall planning failed." : "城牆規劃失敗。");
            return 0;
        }
        var preset = new MapLayoutPreset(1, MapLayoutKind.Placement, plan.Placements
            .Select(item => new MapLayoutEntry(item.NameDef, item.WorldX, item.WorldZ, 0, item.AngleDeg, team)).ToArray());
        preset = preset with { Entries = preset.Entries.Select(entry => entry with { Type = LayoutTypeResolver.ResolveAlias(_objectCatalog, entry.Type)! }).ToArray() };
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
