using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private readonly TextBox _placeSearch = new() { Dock = DockStyle.Fill };
    private readonly Button _placedSnapButton = new() { Dock = DockStyle.Fill, AutoSize = true, MinimumSize = new Size(0, 32) };
    private Label _lblPlaceSearch = null!;

    private void ConfigurePlacementWorkflow(TableLayoutPanel options)
    {
        _lblPlaceSearch = AddSceneField(options, 5, "搜尋", _placeSearch);
        _placeSearch.TextChanged += (_, _) => RefreshPlacementTypes();
        _placedSnapButton.Click += (_, _) => MoveSelectedPlacements(0, 0, snap: true);
        _placedList.SelectedIndexChanged += (_, _) =>
            _placedSnapButton.Enabled = _selected?.IsCustom == true && _placedList.SelectedItems.Count > 0;
        _placedSnapButton.Enabled = false;
    }

    private void LocalizePlacementWorkflow(bool isEn)
    {
        _lblPlaceSearch.Text = isEn ? "Search" : "搜尋";
        _placeSearch.PlaceholderText = isEn ? "Name or alias (space = AND)" : "名稱／別名（空白分隔多條件）";
        _placedSnapButton.Text = isEn ? "Align selection to tile center" : "整批對齊格心";
        _placeHint.Text += isEn
            ? "\nList: Ctrl+A selects all; Ctrl+arrows moves by one tile, Shift adds fine 64-unit steps. Ctrl+D duplicates; Delete removes. Align uses the first selected anchor and preserves spacing."
            : "\n清單：Ctrl+A 全選；Ctrl+方向鍵整批移動一格，加 Shift 改為 64 單位微調；Ctrl+D 複製、Delete 刪除。對齊以第一個選取錨點為準，保留間距。";
    }

    internal static bool MatchesPlacementSearch(SdlObjectType type, string query)
    {
        string text = string.Join(" ", type.NameDef, AliasOf(type), ObjectDisplayName(type, false),
            ObjectDisplayName(type, true), CategoryText(type.Category, false), CategoryText(type.Category, true),
            type.Tribe, TribeText(type.Tribe, false), TribeText(type.Tribe, true));
        return query.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries)
            .All(term => text.Contains(term, StringComparison.OrdinalIgnoreCase));
    }

    internal void HandlePlacementWorkflowKey(KeyEventArgs e)
    {
        if (e.Alt) return;
        if (e.Control && e.KeyCode == Keys.A)
        {
            foreach (ListViewItem row in _placedList.Items) row.Selected = true;
        }
        else if (_selected?.IsCustom != true) return;
        else if (e.Control && e.KeyCode == Keys.D) DuplicateSelectedPlacedObjects();
        else if (!e.Control && !e.Shift && e.KeyCode == Keys.Delete) DeleteSelectedPlacedObjects();
        else if (e.Control && e.KeyCode is Keys.Left or Keys.Right or Keys.Up or Keys.Down)
        {
            float step = e.Shift ? 64 : 256;
            MoveSelectedPlacements(e.KeyCode == Keys.Left ? -step : e.KeyCode == Keys.Right ? step : 0,
                e.KeyCode == Keys.Up ? -step : e.KeyCode == Keys.Down ? step : 0);
        }
        else return;
        e.SuppressKeyPress = true;
    }

    internal void MoveSelectedPlacements(float x, float z, bool snap = false)
    {
        if (_selected?.IsCustom != true) return;
        int[] indices = _placedList.SelectedItems.Cast<ListViewItem>().Select(row => (int)row.Tag!).ToArray();
        if (indices.Length == 0) return;
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        try
        {
            float HeightAt(float worldX, float worldZ)
            {
                if (_terrainLayers is null) return 0f;
                if (_texturesDocument is not null) return LayoutGroundHeight(worldX, worldZ);
                float step = (_terrainLayers.VertexSize - 1) / (64f * 256f);
                int vx = Math.Clamp((int)MathF.Round(worldX * step), 0, _terrainLayers.VertexSize - 1);
                int vz = Math.Clamp((int)MathF.Round(worldZ * step), 0, _terrainLayers.VertexSize - 1);
                return _terrainLayers.Heights[vz * _terrainLayers.VertexSize + vx] * _heightMapStep;
            }
            Func<float, float, float>? height = _terrainLayers is null ? null : HeightAt;
            if (snap) _placementSession.SnapManyToGrid(indices, height);
            else _placementSession.MoveMany(indices, x, z, height);
        }
        catch (ArgumentOutOfRangeException)
        {
            _status.Text = isEn ? "Cannot move: selection would leave the map. No objects changed."
                : "無法移動：選取物件會超出地圖；所有物件均未變更。";
            return;
        }
        RefreshPlacedList();
        foreach (int index in indices) _placedList.Items[index].Selected = true;
        PushSceneObjects(EffectiveSceneObjects());
        UpdateEditorState();
        _status.Text = isEn ? $"Moved {indices.Length} objects; Save to write." : $"已移動 {indices.Length} 個物件；按「儲存」才會寫入。";
        const float clearance = 384;
        if (indices.Any(index => _placementSession[index].Type.Category == SdlObjectCategory.Building &&
            NatureDisplayObjects().Any(item => MathF.Abs(item.WorldX - _placementSession[index].WorldX) < clearance &&
                MathF.Abs(item.WorldZ - _placementSession[index].WorldZ) < clearance)))
            _status.Text += isEn ? " Warning: landscape objects are close; clear space with Nature tools."
                : " 注意：附近有地景物件，建議用自然物件工具清出空地。";
    }
}
