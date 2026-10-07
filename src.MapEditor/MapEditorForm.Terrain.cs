
namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private void PopulateTerrainToolOptions()
    {
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        int operation = Math.Max(0, _terrainOperation.SelectedIndex), strength = _terrainStrength.SelectedIndex < 0 ? 1 : _terrainStrength.SelectedIndex;
        _terrainOperation.Items.Clear();
        if (_editMode == EditMode.Height)
            _terrainOperation.Items.AddRange(isEn ? new object[] { "Raise", "Lower", "Smooth", "Flatten", "Roughen", "Water (carve)" } : new object[] { "升高", "降低", "平滑", "整平", "粗糙化", "水域（挖到水面下）" });
        else
            _terrainOperation.Items.AddRange(isEn ? new object[] { "Block", "Passable" } : new object[] { "阻擋", "可通行" });
        _terrainOperation.SelectedIndex = Math.Min(operation, _terrainOperation.Items.Count - 1);
        _terrainStrength.Items.Clear();
        _terrainStrength.Items.AddRange(isEn ? new object[] { "Gentle", "Medium", "Strong" } : new object[] { "輕", "中", "強" });
        _terrainStrength.SelectedIndex = strength;
        _terrainOperation.Visible = TerrainLayerMode;
        _terrainStrength.Visible = _editMode == EditMode.Height;
    }

    private const int WaterOperationIndex = (int)TerrainHeightOperation.Roughen + 1;
    /// <summary>水域筆刷挖到水面下的深度（高度圖單位）。</summary>
    internal const int WaterBedDepth = 16; // 原版湖底約在水面下 18 單位；6 單位時遊戲中水太淺、近乎透明

    /// <summary>水面上方至少要能挖出的深度（高度圖單位），太淺遊戲幾乎看不到水。</summary>
    internal const int MinimumWaterDepth = 4;

    /// <summary>
    /// 水域筆刷的目標高度：水面下最多 <see cref="WaterBedDepth"/>（不低於 0）。水面太低挖不出水時，自動把水面設為
    /// 「全圖最低點再低 1」——保證不會淹沒任何既有地形——並以 <paramref name="notice"/> 告知玩家；仍不可行時回傳 -1。
    /// </summary>
    private int WaterBedHeight(out string? notice)
    {
        notice = null;
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        if (_heightMapStep <= 0 || _terrainLayers is null) return -1;
        int surface = (int)MathF.Floor((float)_waterLevel.Value / _heightMapStep);
        if (surface < MinimumWaterDepth)
        {
            int raised = _terrainLayers.Heights.Min() - 1;
            decimal level = Math.Min(_waterLevel.Maximum, (decimal)(raised * _heightMapStep));
            if (raised < MinimumWaterDepth || level <= _waterLevel.Value)
            {
                notice = isEn ? "The terrain is too low everywhere to add water automatically; raise the terrain first." : "整張地圖地勢太低，無法自動設定水面；請先把地形升高一些。";
                return -1;
            }
            _waterLevel.Value = level;
            surface = raised;
            notice = isEn ? $"Water level set to {level} automatically (below all existing terrain, nothing is flooded)."
                          : $"已自動把水面設為 {level}（低於所有既有地形，不會淹沒任何區域）。";
        }
        return surface - Math.Min(WaterBedDepth, surface);
    }

    private int TerrainStrength => _terrainStrength.SelectedIndex switch { 0 => 2, 2 => 14, _ => 6 };

    private void InitializeTerrainLayers(string map)
    {
        _terrainLayers = null; _bodenLayer = _embossLayer = _collisionLayer = null; _flattenTarget = -1;
        if (_texturesDocument is null) return;
        int dimension = _texturesDocument.Dimension;
        TerrainLayer? boden = TerrainLayerFiles.Read(Path.Combine(map, "boden.bmp"));
        if (boden is null || boden.Width != boden.Height || boden.Width < 2 || (boden.Width - 1) % dimension != 0) return;
        TerrainLayer? emboss = TerrainLayerFiles.Read(Path.Combine(map, "emboss.bmp"));
        if (emboss is not null && (emboss.Width != boden.Width || emboss.Height != boden.Height)) emboss = null;
        TerrainLayer? collision = TerrainLayerFiles.Read(Path.Combine(map, "collision.bmp"));
        if (collision is not null && (collision.Width != collision.Height || collision.Width % dimension != 0)) collision = null;
        _bodenLayer = boden; _embossLayer = emboss; _collisionLayer = collision;
        _terrainLayers = new TerrainHeightEditSession(boden.Width, boden.Green, emboss?.Green, collision?.Width ?? 0, collision?.Green);
    }

    private void UpdateCollisionOverlay()
    {
        int size = _terrainLayers?.CollisionSize ?? 0;
        IReadOnlyList<byte>? collision = _editMode == EditMode.Collision ? _terrainLayers?.Collision : null;
        _canvas.SetCollisionOverlay(size, collision);
        _view3d?.SetCollisionOverlay(size, collision);
    }

    private void PaintTerrainLayer(TexturePaintEventArgs e)
    {
        if (_selected?.IsCustom != true || _terrainLayers is null || _texturesDocument is null) return;
        // 視圖只在滑鼠移動時回報 tile；快速拖曳會跳格，因此在同一筆畫內補齊上一點到目前點之間的 tile。
        IEnumerable<(int X, int Y)> tiles = _lastTerrainTile is { } last
            ? TerrainStrokePath.Between(last.X, last.Y, e.X, e.Y)
            : new[] { (e.X, e.Y) };
        _lastTerrainTile = (e.X, e.Y);
        bool heightsChanged = false, collisionChanged = false;
        foreach ((int x, int y) in tiles)
        {
            if (!_terrainStrokeTiles.Add(y * _texturesDocument.Dimension + x)) continue; // 同一筆畫不重複套用同一格。
            (bool height, bool collision) = PaintTerrainTile(x, y);
            heightsChanged |= height; collisionChanged |= collision;
        }
        if (heightsChanged) ApplyHeightsToViews();
        if (collisionChanged) UpdateCollisionOverlay();
        UpdateEditorState();
    }

    private (bool Heights, bool Collision) PaintTerrainTile(int tileX, int tileY)
    {
        int dimension = _texturesDocument!.Dimension;
        float radiusTiles = _canvas.BrushSize / 2f + .26f;
        if (_editMode == EditMode.Height)
        {
            float step = (_terrainLayers!.VertexSize - 1) / (float)dimension;
            float centerX = (tileX + .5f) * step, centerY = (tileY + .5f) * step;
            var operation = (TerrainHeightOperation)Math.Clamp(_terrainOperation.SelectedIndex, 0, (int)TerrainHeightOperation.Roughen);
            if (_terrainOperation.SelectedIndex == WaterOperationIndex)
            {
                // 水域：整平到水面下固定深度；水面高度由 boden.ini 的 Waterlevel 與 Heightmapstep 換算為高度圖數值。
                operation = TerrainHeightOperation.Flatten;
                _flattenTarget = WaterBedHeight(out string? waterNotice);
                // 提示放在狀態列的持續通知（直接寫 _status 會被隨後的 UpdateEditorState 覆蓋）；自動調整水面的說明保留到下一筆。
                if (waterNotice is not null || _flattenTarget < 0) _terrainBlendNotice = waterNotice;
                if (_flattenTarget < 0) return (false, false);
            }
            if (operation == TerrainHeightOperation.Flatten && _flattenTarget < 0)
                _flattenTarget = _terrainLayers.Heights[Math.Clamp((int)MathF.Round(centerY), 0, _terrainLayers.VertexSize - 1) * _terrainLayers.VertexSize + Math.Clamp((int)MathF.Round(centerX), 0, _terrainLayers.VertexSize - 1)];
            return (_terrainLayers.PaintHeight(centerX, centerY, radiusTiles * step + 1, operation, TerrainStrength, _flattenTarget, _roughnessSeed).Count > 0, false);
        }
        if (_terrainLayers!.HasCollision)
        {
            float step = _terrainLayers.CollisionSize / (float)dimension;
            var operation = _terrainOperation.SelectedIndex == 1 ? TerrainCollisionOperation.Clear : TerrainCollisionOperation.Block;
            return (false, _terrainLayers.PaintCollision((tileX + .5f) * step, (tileY + .5f) * step, radiusTiles * step, operation).Count > 0);
        }
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        _terrainBlendNotice = isEn ? "This map has no collision.bmp to edit." : "此地圖沒有可編輯的 collision.bmp。";
        return (false, false);
    }

    /// <summary>整平至水面上方、鋪基礎材質、清除阻擋與可移除地景；SDL 聚落保留，儲存前不寫檔。</summary>
    internal void ApplyBlankTerrain(bool confirm)
    {
        if (_selected?.IsCustom != true || _terrainLayers is null || _texturesDocument is null) return;
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        if (confirm && MessageBox.Show(this,
                isEn ? "Flatten the whole terrain?\nHeights, ground material, blocked areas, vertex colors, smoothing and lighting are reset when you Save. Removable scenery is cleared; settlements and scripts are kept. You can still use \"Reset Terrain\" before saving."
                     : "要整平整張地形嗎？\n高度、地表材質、阻擋區、頂點色、平滑遮罩與光照會在儲存時重設，可移除地景會清除；聚落與腳本保留。儲存前仍可用「還原地表」放棄。",
                isEn ? "Reset Flat Terrain" : "重設平坦地形", MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes) return;
        CommitStroke();
        float water = _heightMapStep > 0 ? (float)_waterLevel.Value / _heightMapStep : 0;
        _terrainLayers.ApplyBlankTerrain((byte)Math.Clamp((int)MathF.Round(water) + 20, 0, 255));
        _terrainLayers.CommitStroke();
        string? material = BlankBaseMaterial();
        if (material is not null && _terrainBlendSession is not null)
        {
            int dimension = _texturesDocument.Dimension;
            TerrainBlendPaintResult paint = _terrainBlendSession.PaintCircle(dimension / 2f, dimension / 2f, dimension, material);
            foreach (TerrainTextureChange change in paint.TextureChanges) ApplyTexture(change.X, change.Y, change.After);
            _terrainBlendSession.CommitStroke();
        }
        _resetAuxiliaryLayers = true;
        // 空白地形同時清除範本留下的地景物件（樹、草、灌木…）；腳本標記、特效與連結物件保留。
        if (_natureSession.Remove(_levelObjects.Where(IsRemovableNature).Select(item => item.Slot), _natureSession.Additions)) CommitStroke();
        RefreshSceneMarkers();
        ApplyHeightsToViews();
        UpdateCollisionOverlay();
        UpdateEditorState();
        _status.Text = isEn ? "Flat terrain prepared; settlements and scripts are kept. Shape the terrain, then Save." : "已準備平坦地形，聚落與腳本保留；完成地形編輯後按「儲存」。";
    }

    /// <summary>空白地形的基礎材質：目前地圖最常見的基礎材質（通常是草地），找不到時用材質庫第一項。</summary>
    private string? BlankBaseMaterial()
    {
        if (_floorMaterials is null || _floorMaterials.Materials.Count == 0 || _texturesDocument is null) return null;
        return _texturesDocument.Textures.Select(texture => _floorMaterials.FindByTexture(texture)?.Id).OfType<string>()
            .GroupBy(id => id, StringComparer.OrdinalIgnoreCase).OrderByDescending(group => group.Count()).Select(group => group.Key).FirstOrDefault()
            ?? _floorMaterials.Materials[0].Id;
    }

    /// <summary>將高度更新至 2D／3D 預覽。</summary>
    private void ApplyHeightsToViews()
    {
        if (_terrainLayers is null) return;
        if (!TryParseGameColor(_waterColor.Text, out Color waterColor)) waterColor = Color.SteelBlue;
        _canvas.SetHeightSamples(_terrainLayers.VertexSize, _terrainLayers.HeightsDirty ? _terrainLayers.Heights : null, (float)_waterLevel.Value, _heightMapStep, waterColor);
        _view3d?.SetHeightSamples(_terrainLayers.Heights);
    }

    private void ApplyTerrainLayerStroke(TerrainLayerStroke stroke)
    {
        if (stroke.Heights.Count > 0) ApplyHeightsToViews();
        if (stroke.Collision.Count > 0) UpdateCollisionOverlay();
    }

    private void ResetTerrain()
    {
        if (_texturesDocument is null || _savedTextures.Length != _texturesDocument.Textures.Count) return;
        bool isEn = AgainstRomeModifier.Loc.CurrentLanguage == AgainstRomeModifier.Language.English;
        string msg = isEn
            ? "Do you want to discard unsaved terrain changes (textures, heights, passability)?\nMap metadata, environment settings and scene objects will not be affected."
            : "要放棄這次尚未儲存的地表變更（材質、高度、通行區域）嗎？\n地圖名稱、環境設定與場景物件不會受影響。";
        string title = isEn ? "Reset Terrain" : "還原地表";
        if (MessageBox.Show(this, msg, title, MessageBoxButtons.YesNo, MessageBoxIcon.Question) != DialogResult.Yes) return;
        CommitStroke();
        if (_terrainLayers is not null)
        {
            _terrainLayers.ResetToBaseline();
            ApplyHeightsToViews();
            UpdateCollisionOverlay();
        }
        _resetAuxiliaryLayers = false;
        _natureSession.Clear(); RefreshSceneMarkers();
        if (_terrainBlendSession is null) { UpdateEditorState(); return; }
        IReadOnlyList<TerrainTextureChange> changes = _terrainBlendSession.ResetToBaseline();
        _texturesDocument.SetTextures(_terrainBlendSession.CurrentTextures); // 批次寫回，避免逐格重新解析整份 boden.txt。
        foreach (TerrainTextureChange change in changes)
        {
            _canvas.SetTexture(change.X, change.Y, change.After); _view3d?.SetTexture(change.X, change.Y, change.After);
        }
        _terrainBlendNotice = null;
        UpdateEditorState();
    }

}
