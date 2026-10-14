using AgainstRomeModifier;

namespace AgainstRomeMapEditor;

internal sealed partial class MapEditorForm
{
    private readonly ToolStripButton _rampTool = new("斜坡") { CheckOnClick = true };
    private (int X, int Y)? _rampStartTile;
    private (int X, int Y)? _rampMouseDownTile;
    private (int X, int Y)? _lastRampHoverTile;
    private float _rampWidthTiles = 4f;
    private float _rampFalloffTiles = 2f;
    private bool _view3dHooked;

    /// <summary>
    /// 斜坡帶狀核心寬度（圖塊單位，供測試或外部設定）。
    /// </summary>
    internal float RampWidthTiles
    {
        get => _rampWidthTiles;
        set => _rampWidthTiles = Math.Max(0.5f, value);
    }

    /// <summary>
    /// 斜坡邊緣柔和衰減寬度（圖塊單位，供測試或外部設定）。
    /// </summary>
    internal float RampFalloffTiles
    {
        get => _rampFalloffTiles;
        set => _rampFalloffTiles = Math.Max(0f, value);
    }

    /// <summary>
    /// 目前已選取之斜坡起點（圖塊座標）。
    /// </summary>
    internal (int X, int Y)? CurrentRampStartTile => _rampStartTile;

    private void InitializeRampTool(ToolStrip tools)
    {
        int heightIdx = tools.Items.IndexOf(_heightTool);
        if (heightIdx >= 0)
            tools.Items.Insert(heightIdx + 1, _rampTool);
        else
            tools.Items.Add(_rampTool);

        LocalizeRampTool(Loc.CurrentLanguage == Language.English);

        _rampTool.Click += (_, _) => ToggleRampTool();

        _canvas.MouseDown += OnCanvasMouseDownForRamp;
        _canvas.MouseUp += OnCanvasMouseUpForRamp;
        _canvas.TileHovered += (_, e) => _lastRampHoverTile = (e.X, e.Y);

        HookView3D();
        if (_canvasHost is not null)
        {
            _canvasHost.ControlAdded += (_, e) =>
            {
                if (e.Control is Map3DViewControl) HookView3D();
            };
        }

        _btnLangZH.Click += (_, _) => LocalizeRampTool(false);
        _btnLangEN.Click += (_, _) => LocalizeRampTool(true);

        KeyDown += (_, e) =>
        {
            if (e.KeyCode == Keys.Escape && _rampTool.Checked) CancelRampSelection();
        };
        _canvas.KeyDown += (_, e) =>
        {
            if (e.KeyCode == Keys.Escape && _rampTool.Checked) CancelRampSelection();
        };

        // 與其他繪製／編輯模式互斥
        _textureTool.Click += (_, _) => DeactivateRampTool();
        _heightTool.Click += (_, _) => DeactivateRampTool();
        _collisionTool.Click += (_, _) => DeactivateRampTool();
        _placeTool.Click += (_, _) => DeactivateRampTool();
        _natureTool.Click += (_, _) => DeactivateRampTool();
        _sceneMoveTool.Click += (_, _) => DeactivateRampTool();
    }

    private void LocalizeRampTool(bool isEn)
    {
        _rampTool.Text = isEn ? "Ramp" : "斜坡";
        _rampTool.ToolTipText = isEn
            ? "Terrain Ramp Tool: Click start and end points on the map to interpolate heights along a strip (right-click or Escape cancels)."
            : "地形斜坡工具：點選地圖上的起點與終點，沿著帶狀路徑平滑內插地勢（右鍵或 Escape 取消）。";
    }

    private void HookView3D()
    {
        if (_view3d is null || _view3dHooked) return;
        _view3dHooked = true;
        _view3d.MouseDown += OnView3DMouseDownForRamp;
        _view3d.MouseUp += OnView3DMouseUpForRamp;
        _view3d.TileHovered += (_, e) => _lastRampHoverTile = (e.X, e.Y);
        _view3d.KeyDown += (_, e) =>
        {
            if (e.KeyCode == Keys.Escape && _rampTool.Checked) CancelRampSelection();
        };
    }

    private void ToggleRampTool()
    {
        if (_rampTool.Checked)
        {
            ActivateRampTool();
        }
        else
        {
            DeactivateRampTool();
        }
    }

    internal void ActivateRampTool()
    {
        CommitStroke();
        SetEditMode(EditMode.Height);
        SetBrushToken(null); // 關閉筆刷送出，避免一般高度畫筆在點擊端點時重複塗抹
        _heightTool.Checked = false;
        _rampTool.Checked = true;
        _rampStartTile = null;
        _rampMouseDownTile = null;
        _canvas.Cursor = Cursors.Cross;

        bool isEn = Loc.CurrentLanguage == Language.English;
        _status.Text = isEn
            ? "Ramp Tool: Click start point on the map (or drag to end point). Right-click or Escape cancels."
            : "斜坡工具：請在地圖上點選起點（或拖曳至終點）。右鍵或 Escape 可取消。";
    }

    internal void DeactivateRampTool()
    {
        if (!_rampTool.Checked && _rampStartTile is null) return;
        _rampTool.Checked = false;
        _rampStartTile = null;
        _rampMouseDownTile = null;
        if (_editMode == EditMode.Height)
        {
            SetBrushToken(TerrainToolBrushToken);
        }
    }

    internal void CancelRampSelection()
    {
        if (_rampStartTile is null) return;
        _rampStartTile = null;
        _rampMouseDownTile = null;
        bool isEn = Loc.CurrentLanguage == Language.English;
        _status.Text = isEn ? "Ramp start point cancelled." : "已取消斜坡起點選取。";
    }

    private void OnCanvasMouseDownForRamp(object? sender, MouseEventArgs e)
    {
        if (!_rampTool.Checked) return;
        if (e.Button == MouseButtons.Right)
        {
            CancelRampSelection();
            return;
        }
        if (e.Button != MouseButtons.Left) return;
        if (_terrainLayers is null || _texturesDocument is null) return;

        int tileX = -1, tileY = -1;
        Rectangle bounds = _canvas.SceneBoundsRectangle;
        if (bounds.Contains(e.Location) && _canvas.MapDimension > 0)
        {
            tileX = Math.Clamp((int)((e.Location.X - bounds.X) * _canvas.MapDimension / (float)bounds.Width), 0, _canvas.MapDimension - 1);
            tileY = Math.Clamp((int)((e.Location.Y - bounds.Y) * _canvas.MapDimension / (float)bounds.Height), 0, _canvas.MapDimension - 1);
        }
        else if (_lastRampHoverTile is { } hover)
        {
            tileX = hover.X; tileY = hover.Y;
        }

        if (tileX < 0 || tileY < 0) return;
        HandleRampPointerDown(tileX, tileY);
    }

    private void OnCanvasMouseUpForRamp(object? sender, MouseEventArgs e)
    {
        if (!_rampTool.Checked || e.Button != MouseButtons.Left) return;
        if (_terrainLayers is null || _texturesDocument is null) return;

        int tileX = -1, tileY = -1;
        Rectangle bounds = _canvas.SceneBoundsRectangle;
        if (bounds.Contains(e.Location) && _canvas.MapDimension > 0)
        {
            tileX = Math.Clamp((int)((e.Location.X - bounds.X) * _canvas.MapDimension / (float)bounds.Width), 0, _canvas.MapDimension - 1);
            tileY = Math.Clamp((int)((e.Location.Y - bounds.Y) * _canvas.MapDimension / (float)bounds.Height), 0, _canvas.MapDimension - 1);
        }
        else if (_lastRampHoverTile is { } hover)
        {
            tileX = hover.X; tileY = hover.Y;
        }

        if (tileX < 0 || tileY < 0) return;
        HandleRampPointerUp(tileX, tileY);
    }

    private void OnView3DMouseDownForRamp(object? sender, MouseEventArgs e)
    {
        if (!_rampTool.Checked) return;
        if (e.Button == MouseButtons.Right)
        {
            CancelRampSelection();
            return;
        }
        if (e.Button != MouseButtons.Left) return;
        if (_lastRampHoverTile is not { } hover) return;
        HandleRampPointerDown(hover.X, hover.Y);
    }

    private void OnView3DMouseUpForRamp(object? sender, MouseEventArgs e)
    {
        if (!_rampTool.Checked || e.Button != MouseButtons.Left) return;
        if (_lastRampHoverTile is not { } hover) return;
        HandleRampPointerUp(hover.X, hover.Y);
    }

    private void HandleRampPointerDown(int tileX, int tileY)
    {
        if (_terrainLayers is null || _texturesDocument is null) return;
        bool isEn = Loc.CurrentLanguage == Language.English;

        if (_rampStartTile is null)
        {
            _rampStartTile = (tileX, tileY);
            _rampMouseDownTile = (tileX, tileY);
            int dim = _texturesDocument.Dimension;
            int vSize = _terrainLayers.VertexSize;
            (float vx, float vy) = TerrainRampTool.TileToVertex(tileX, tileY, dim, vSize);
            float h = TerrainRampTool.SampleBilinear(_terrainLayers.Heights, vSize, vx, vy);
            _status.Text = isEn
                ? $"Ramp: Start point set at ({tileX}, {tileY}), height {h:F0}. Click end point (or drag; Escape cancels)."
                : $"斜坡工具：已選取起點 ({tileX}, {tileY})，高度 {h:F0}。請點選或拖曳至終點（按 Escape 取消）。";
        }
        else
        {
            _rampMouseDownTile = (tileX, tileY);
            // 第二次點擊直接執行斜坡
            var start = _rampStartTile.Value;
            if (tileX != start.X || tileY != start.Y)
            {
                ExecuteRamp(start.X, start.Y, tileX, tileY);
                _rampStartTile = null;
                _rampMouseDownTile = null;
            }
        }
    }

    private void HandleRampPointerUp(int tileX, int tileY)
    {
        if (_terrainLayers is null || _texturesDocument is null) return;
        if (_rampStartTile is null || _rampMouseDownTile is null) return;

        var start = _rampStartTile.Value;
        // 若按下與放開不同格，判定為拖曳操作，立即完成斜坡
        if (tileX != start.X || tileY != start.Y)
        {
            ExecuteRamp(start.X, start.Y, tileX, tileY);
            _rampStartTile = null;
            _rampMouseDownTile = null;
        }
    }

    /// <summary>
    /// 在指定起點與終點之間建立斜坡，並套用至地圖的高度編輯 Session。
    /// 一個完整的斜坡為單一 Undo/Redo 步驟。
    /// </summary>
    internal IReadOnlyList<TerrainSampleChange> ExecuteRamp(
        int startTileX,
        int startTileY,
        int endTileX,
        int endTileY,
        TerrainRampParameters? parameters = null)
    {
        if (_terrainLayers is null || _texturesDocument is null) return Array.Empty<TerrainSampleChange>();
        int dim = _texturesDocument.Dimension;
        int vSize = _terrainLayers.VertexSize;
        float step = (vSize - 1) / (float)dim;

        (float sx, float sy) = TerrainRampTool.TileToVertex(startTileX, startTileY, dim, vSize);
        (float ex, float ey) = TerrainRampTool.TileToVertex(endTileX, endTileY, dim, vSize);

        parameters ??= new TerrainRampParameters
        {
            Width = Math.Max(1f, _rampWidthTiles * step),
            Falloff = Math.Max(0.5f, _rampFalloffTiles * step)
        };

        float h0 = parameters.StartHeight ?? TerrainRampTool.SampleBilinear(_terrainLayers.Heights, vSize, sx, sy);
        float h1 = parameters.EndHeight ?? TerrainRampTool.SampleBilinear(_terrainLayers.Heights, vSize, ex, ey);

        var changes = TerrainRampTool.ApplyRamp(_terrainLayers, sx, sy, ex, ey, parameters);

        if (changes.Count > 0)
        {
            ApplyHeightsToViews();
            UpdateEditorState();
            bool isEn = Loc.CurrentLanguage == Language.English;
            _status.Text = isEn
                ? $"Ramp created: ({startTileX}, {startTileY}) h={h0:F0} -> ({endTileX}, {endTileY}) h={h1:F0}, {changes.Count} vertices modified. Use Ctrl+Z to undo."
                : $"已建立斜坡：起點 ({startTileX}, {startTileY}) 高度 {h0:F0} → 終點 ({endTileX}, {endTileY}) 高度 {h1:F0}，修改 {changes.Count} 個頂點。可使用 Ctrl+Z 復原。";
        }

        return changes;
    }
}
