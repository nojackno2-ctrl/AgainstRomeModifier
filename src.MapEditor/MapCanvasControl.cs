using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor;

internal sealed class MapCanvasControl : Control
{
    private Bitmap? _bitmap;
    private Bitmap? _terrainScene;
    private Bitmap? _emboss;
    private Bitmap? _smooth;
    private Bitmap? _heightShade;
    private Bitmap? _waterOverlay;
    private FloorTextureLibrary? _floorTextures;
    private string[]? _textures;
    private string[]? _baselineTextures;
    private IReadOnlyList<MapSceneObject> _sceneObjects = Array.Empty<MapSceneObject>();
    private readonly Dictionary<string, Color> _texturePreviewColors = new(StringComparer.OrdinalIgnoreCase);
    private int _dimension;
    private int _hoverX = -1, _hoverY = -1;
    private bool _painting;
    private bool _movingSceneObject;
    private bool _panning;
    private Point _panStart;
    private PointF _pan = PointF.Empty;
    private float _zoom = 1f;
    private readonly HashSet<int> _paintedInDrag = new();

    public string? BrushTexture { get; set; }
    public int BrushSize { get; set; } = 1;
    public bool EditingEnabled { get; set; }
    public bool SceneMoveEnabled { get; set; }
    public bool ShowGrid { get; set; } = true;
    public bool ShowObjects { get; set; } = true;
    public int SceneObjectCount => _sceneObjects.Count;
    public event EventHandler<TexturePaintEventArgs>? TexturePainted;
    public event EventHandler<TileHoverEventArgs>? TileHovered;
    public event EventHandler<TextureSampleEventArgs>? TextureSampled;
    public event EventHandler? StrokeEnded;
    public event EventHandler<SceneObjectMoveEventArgs>? SceneObjectMoved;

    public MapCanvasControl()
    {
        DoubleBuffered = true;
        BackColor = Color.FromArgb(24, 28, 36);
        Dock = DockStyle.Fill;
        Cursor = Cursors.Cross;
        SetStyle(ControlStyles.Selectable, true);
    }

    public bool LoadTextures(int dimension, IReadOnlyList<string> textures, IReadOnlyList<string> baselineTextures, string mapDirectory, FloorTextureLibrary floorTextures, IReadOnlyList<MapSceneObject> sceneObjects, float waterLevel, float heightMapStep, Color waterColor, bool preserveView = false)
    {
        _bitmap?.Dispose(); _bitmap = null; _terrainScene?.Dispose(); _terrainScene = null; _emboss?.Dispose(); _emboss = null; _smooth?.Dispose(); _smooth = null; _heightShade?.Dispose(); _heightShade = null; _waterOverlay?.Dispose(); _waterOverlay = null;
        string minimapPath = Path.Combine(mapDirectory, "minimap.bmp");
        if (File.Exists(minimapPath)) using (var source = new Bitmap(minimapPath)) _bitmap = new Bitmap(source);
        _emboss = LoadBitmap(Path.Combine(mapDirectory, "emboss.bmp"));
        _smooth = LoadBitmap(Path.Combine(mapDirectory, "smooth.bmp"));
        using (Bitmap? heightMap = LoadBitmap(Path.Combine(mapDirectory, "boden.bmp")))
        {
            _heightShade = heightMap is null ? null : BuildHeightShade(heightMap);
            _waterOverlay = heightMap is null || heightMapStep <= 0 ? null : BuildWaterOverlay(heightMap, waterLevel / heightMapStep, waterColor);
        }
        _dimension = dimension; _textures = textures.ToArray(); _baselineTextures = baselineTextures.ToArray();
        if (!preserveView) { _zoom = 1f; _pan = PointF.Empty; }
        _floorTextures = floorTextures; // 由 MapEditorForm 擁有生命週期，此處僅借用，不負責釋放。
        _sceneObjects = sceneObjects;
        BuildTexturePreviewColors(); RenderTerrainScene(); Invalidate();
        return _floorTextures.IsAvailable;
    }

    /// <summary>只重建水面遮罩（改水面高度／顏色時的即時預覽），不重載整個場景。</summary>
    public void UpdateWaterOverlay(string mapDirectory, float waterLevel, float heightMapStep, Color waterColor)
    {
        _waterOverlay?.Dispose(); _waterOverlay = null;
        using Bitmap? heightMap = LoadBitmap(Path.Combine(mapDirectory, "boden.bmp"));
        _waterOverlay = heightMap is null || heightMapStep <= 0 ? null : BuildWaterOverlay(heightMap, waterLevel / heightMapStep, waterColor);
        Invalidate();
    }

    /// <summary>目前每格材質的平均色（供 minimap 重生成使用）；找不到場景時回傳空陣列。</summary>
    public IReadOnlyList<string>? CurrentTextures => _textures;

    /// <summary>
    /// 依目前材質重繪遊戲用的 minimap.bmp（256×256、24bpp，1 tile = 4×4 px），讓遊戲內小地圖與編輯後的地表一致。
    /// 僅在真實地表貼圖可用時輸出，否則回傳 null（避免用簡化色塊覆蓋原始小地圖）。
    /// </summary>
    public byte[]? RenderMinimapBmp()
    {
        if (_terrainScene is null || _floorTextures is null || !_floorTextures.IsAvailable) return null;
        using var minimap = new Bitmap(256, 256, PixelFormat.Format24bppRgb);
        using (Graphics graphics = Graphics.FromImage(minimap))
        {
            graphics.InterpolationMode = InterpolationMode.HighQualityBicubic;
            graphics.PixelOffsetMode = PixelOffsetMode.HighQuality;
            graphics.DrawImage(_terrainScene, new Rectangle(0, 0, 256, 256));
            if (_heightShade is not null) graphics.DrawImage(_heightShade, new Rectangle(0, 0, 256, 256));
        }
        using var stream = new MemoryStream();
        minimap.Save(stream, ImageFormat.Bmp);
        return stream.ToArray();
    }

    public void SetTexture(int x, int y, string texture)
    {
        if (_textures is null || _dimension <= 0) return;
        int index = y * _dimension + x;
        if (index < 0 || index >= _textures.Length) return;
        _textures[index] = texture;
        RenderTerrainCell(x, y);
        Invalidate();
    }

    protected override void OnPaint(PaintEventArgs e)
    {
        base.OnPaint(e);
        e.Graphics.SmoothingMode = SmoothingMode.HighQuality;
        if (_textures is not null && _dimension > 0)
        {
            DrawTextureEditor(e.Graphics);
            return;
        }

        TextRenderer.DrawText(e.Graphics, "請從左側選擇地圖", Font, ClientRectangle, Color.SlateGray,
            TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter);
    }

    private void DrawTextureEditor(Graphics graphics)
    {
        string[] textures = _textures!;
        Rectangle bounds = SceneBounds();
        float cellWidth = bounds.Width / (float)_dimension, cellHeight = bounds.Height / (float)_dimension;
        if (_terrainScene is not null)
        {
            graphics.InterpolationMode = InterpolationMode.HighQualityBicubic;
            graphics.DrawImage(_terrainScene, bounds);
            if (_heightShade is not null) graphics.DrawImage(_heightShade, bounds);
            if (_waterOverlay is not null) graphics.DrawImage(_waterOverlay, bounds);
            DrawMapOverlay(graphics, _smooth, bounds, .12f);
            DrawMapOverlay(graphics, _emboss, bounds, .22f);
        }
        else
        {
            using var background = new SolidBrush(Color.FromArgb(55, 60, 68)); graphics.FillRectangle(background, bounds);
        }

        if (_baselineTextures is not null && _baselineTextures.Length == textures.Length)
        {
            using var changedPen = new Pen(Color.FromArgb(235, 255, 220, 80), 1.5f);
            for (int index = 0; index < textures.Length; index++)
            {
                if (StringComparer.OrdinalIgnoreCase.Equals(textures[index], _baselineTextures[index])) continue;
                int x = index % _dimension, y = index / _dimension;
                graphics.DrawRectangle(changedPen, bounds.X + x * cellWidth, bounds.Y + y * cellHeight, cellWidth, cellHeight);
            }
        }

        if (ShowGrid && cellWidth >= 6)
        {
            using var shadow = new Pen(Color.FromArgb(70, 0, 0, 0), 1);
            using var light = new Pen(Color.FromArgb(55, 255, 255, 255), 1);
            for (int i = 0; i <= _dimension; i++)
            {
                float px = bounds.X + i * cellWidth, py = bounds.Y + i * cellHeight;
                graphics.DrawLine(shadow, px + 1, bounds.Top, px + 1, bounds.Bottom); graphics.DrawLine(light, px, bounds.Top, px, bounds.Bottom);
                graphics.DrawLine(shadow, bounds.Left, py + 1, bounds.Right, py + 1); graphics.DrawLine(light, bounds.Left, py, bounds.Right, py);
            }
        }
        if (ShowObjects) DrawSceneObjects(graphics, bounds);
        DrawBrushCursor(graphics, bounds, cellWidth, cellHeight);
        DrawFrame(graphics, bounds);
    }

    private void DrawBrushCursor(Graphics graphics, Rectangle bounds, float cellWidth, float cellHeight)
    {
        if (!EditingEnabled || _hoverX < 0 || _hoverY < 0 || string.IsNullOrWhiteSpace(BrushTexture)) return;
        int radius = Math.Max(0, BrushSize / 2);
        int x0 = Math.Max(0, _hoverX - radius), y0 = Math.Max(0, _hoverY - radius);
        int x1 = Math.Min(_dimension - 1, _hoverX + radius), y1 = Math.Min(_dimension - 1, _hoverY + radius);
        var rect = new RectangleF(bounds.X + x0 * cellWidth, bounds.Y + y0 * cellHeight, (x1 - x0 + 1) * cellWidth, (y1 - y0 + 1) * cellHeight);
        using var fill = new SolidBrush(Color.FromArgb(55, 255, 255, 255));
        using var pen = new Pen(Color.FromArgb(235, 255, 240, 120), 2);
        graphics.FillRectangle(fill, rect);
        graphics.DrawRectangle(pen, rect.X, rect.Y, rect.Width, rect.Height);
    }

    protected override void OnMouseDown(MouseEventArgs e)
    {
        base.OnMouseDown(e);
        Focus();
        if (e.Button == MouseButtons.Middle)
        {
            _panning = true; _panStart = e.Location; Cursor = Cursors.SizeAll; return;
        }
        if (e.Button == MouseButtons.Right && TryGetTile(e.Location, out int sampleX, out int sampleY) && _textures is not null)
        {
            TextureSampled?.Invoke(this, new TextureSampleEventArgs(sampleX, sampleY, _textures[sampleY * _dimension + sampleX]));
            return;
        }
        if (e.Button != MouseButtons.Left) return;
        if (SceneMoveEnabled)
        {
            _movingSceneObject = TryMoveSceneObject(e.Location, completed: false);
            return;
        }
        _painting = true; _paintedInDrag.Clear(); TryPaint(e.Location);
    }

    protected override void OnMouseMove(MouseEventArgs e)
    {
        base.OnMouseMove(e);
        if (_panning && e.Button == MouseButtons.Middle)
        {
            _pan = new PointF(_pan.X + e.X - _panStart.X, _pan.Y + e.Y - _panStart.Y); _panStart = e.Location; Invalidate(); return;
        }
        if (TryGetTile(e.Location, out int x, out int y))
        {
            if (x != _hoverX || y != _hoverY) { _hoverX = x; _hoverY = y; if (EditingEnabled) Invalidate(); }
            TileHovered?.Invoke(this, new TileHoverEventArgs(x, y, _textures?[y * _dimension + x]));
        }
        else if (_hoverX != -1) { _hoverX = _hoverY = -1; if (EditingEnabled) Invalidate(); }
        if (_movingSceneObject && e.Button == MouseButtons.Left) TryMoveSceneObject(e.Location, completed: false);
        else if (_painting && e.Button == MouseButtons.Left) TryPaint(e.Location);
    }

    protected override void OnMouseLeave(EventArgs e)
    {
        base.OnMouseLeave(e);
        if (_hoverX == -1) return;
        _hoverX = _hoverY = -1; if (EditingEnabled) Invalidate();
    }

    protected override void OnMouseUp(MouseEventArgs e)
    {
        base.OnMouseUp(e);
        bool wasPainting = _painting;
        if (_movingSceneObject && e.Button == MouseButtons.Left) TryMoveSceneObject(e.Location, completed: true);
        _painting = false; _movingSceneObject = false; _panning = false; Cursor = Cursors.Cross; _paintedInDrag.Clear();
        if (wasPainting) StrokeEnded?.Invoke(this, EventArgs.Empty);
    }

    /// <summary>把目前材質設為「已儲存基準」，儲存後清掉變更格高亮，不需重載場景。</summary>
    public void CommitBaseline()
    {
        if (_textures is null) return;
        _baselineTextures = (string[])_textures.Clone();
        Invalidate();
    }

    /// <summary>將指定 tile 置中於畫面（維持目前縮放），供場景物件清單跳轉使用。</summary>
    public void FocusTile(float tileX, float tileY)
    {
        if (_textures is null || _dimension <= 0) return;
        Rectangle fitted = Fit(new Size(_dimension, _dimension), ClientRectangle);
        int width = Math.Max(1, (int)(fitted.Width * _zoom)), height = Math.Max(1, (int)(fitted.Height * _zoom));
        float centerX = ClientSize.Width / 2f, centerY = ClientSize.Height / 2f;
        _pan = new PointF(
            centerX - fitted.X - (fitted.Width - width) / 2f - (tileX + .5f) / _dimension * width,
            centerY - fitted.Y - (fitted.Height - height) / 2f - (tileY + .5f) / _dimension * height);
        Invalidate();
    }

    public void UpdateSceneObjects(IReadOnlyList<MapSceneObject> sceneObjects)
    {
        _sceneObjects = sceneObjects;
        Invalidate();
    }

    protected override void OnMouseWheel(MouseEventArgs e)
    {
        base.OnMouseWheel(e);
        if (_textures is null || _dimension <= 0) return;
        float previous = _zoom;
        float next = Math.Clamp(_zoom * (e.Delta > 0 ? 1.2f : 1f / 1.2f), .75f, 6f);
        if (Math.Abs(previous - next) < .001f) return;
        // 以游標為錨：縮放後讓游標下的世界座標維持在原位。
        Rectangle before = SceneBounds();
        float fractionX = before.Width > 0 ? (e.X - before.X) / (float)before.Width : .5f;
        float fractionY = before.Height > 0 ? (e.Y - before.Y) / (float)before.Height : .5f;
        _zoom = next;
        Rectangle fitted = Fit(new Size(_dimension, _dimension), ClientRectangle);
        int width = Math.Max(1, (int)(fitted.Width * _zoom)), height = Math.Max(1, (int)(fitted.Height * _zoom));
        _pan = new PointF(
            e.X - fractionX * width - fitted.X - (fitted.Width - width) / 2f,
            e.Y - fractionY * height - fitted.Y - (fitted.Height - height) / 2f);
        Invalidate();
    }

    private void TryPaint(Point location)
    {
        if (!EditingEnabled || string.IsNullOrWhiteSpace(BrushTexture) || _textures is null) return;
        if (!TryGetTile(location, out int x, out int y)) return;
        int index = y * _dimension + x;
        if (!_paintedInDrag.Add(index)) return;
        TexturePainted?.Invoke(this, new TexturePaintEventArgs(x, y, _textures[index], BrushTexture));
        Invalidate();
    }

    private bool TryMoveSceneObject(Point location, bool completed)
    {
        if (!EditingEnabled || !SceneMoveEnabled || !TryGetTile(location, out int x, out int y)) return false;
        float worldUnitsPerTile = SdlSceneCatalog.WorldUnitsPerMapPixel * (SdlSceneCatalog.MapPixelSize / (float)_dimension);
        SceneObjectMoved?.Invoke(this, new SceneObjectMoveEventArgs((x + .5f) * worldUnitsPerTile, (y + .5f) * worldUnitsPerTile, completed));
        return true;
    }

    private bool TryGetTile(Point location, out int x, out int y)
    {
        x = y = -1;
        if (_textures is null || _dimension <= 0) return false;
        Rectangle bounds = SceneBounds();
        if (!bounds.Contains(location)) return false;
        x = Math.Clamp((int)((location.X - bounds.X) * _dimension / (float)bounds.Width), 0, _dimension - 1);
        y = Math.Clamp((int)((location.Y - bounds.Y) * _dimension / (float)bounds.Height), 0, _dimension - 1);
        return true;
    }

    private static Rectangle Fit(Size source, Rectangle target)
    {
        Rectangle inner = Rectangle.Inflate(target, -24, -24);
        float scale = Math.Min(inner.Width / (float)source.Width, inner.Height / (float)source.Height);
        int width = Math.Max(1, (int)(source.Width * scale)), height = Math.Max(1, (int)(source.Height * scale));
        return new Rectangle(inner.X + (inner.Width - width) / 2, inner.Y + (inner.Height - height) / 2, width, height);
    }

    private static void DrawFrame(Graphics graphics, Rectangle bounds)
    {
        using var pen = new Pen(Color.FromArgb(100, 155, 180, 205), 1);
        graphics.DrawRectangle(pen, bounds);
    }

    internal Color GetTexturePreviewColor(string texture)
        => _texturePreviewColors.TryGetValue(texture, out Color color) ? color : FallbackTextureColor(texture);

    private void RenderTerrainScene()
    {
        if (_textures is null || _dimension <= 0) return;
        _terrainScene = new Bitmap(_dimension * 32, _dimension * 32);
        using Graphics graphics = Graphics.FromImage(_terrainScene);
        graphics.InterpolationMode = InterpolationMode.HighQualityBicubic;
        for (int y = 0; y < _dimension; y++) for (int x = 0; x < _dimension; x++) RenderTerrainCell(graphics, x, y);
    }

    private void DrawSceneObjects(Graphics graphics, Rectangle bounds)
    {
        foreach (MapSceneObject item in _sceneObjects)
        {
            float x = bounds.Left + item.WorldX / (SdlSceneCatalog.WorldUnitsPerMapPixel * SdlSceneCatalog.MapPixelSize) * bounds.Width;
            float y = bounds.Top + item.WorldZ / (SdlSceneCatalog.WorldUnitsPerMapPixel * SdlSceneCatalog.MapPixelSize) * bounds.Height;
            float size = Math.Clamp(bounds.Width / 180f, 3f, 11f);
            Color color = TeamColor(item.Team);
            using var fill = new SolidBrush(Color.FromArgb(215, color));
            using var outline = new Pen(Color.FromArgb(235, 20, 20, 20), Math.Max(1, size / 5));
            if (item.Kind == "建築") { graphics.FillRectangle(fill, x - size, y - size, size * 2, size * 2); graphics.DrawRectangle(outline, x - size, y - size, size * 2, size * 2); }
            else if (item.Kind == "單位") { graphics.FillEllipse(fill, x - size / 2, y - size / 2, size, size); graphics.DrawEllipse(outline, x - size / 2, y - size / 2, size, size); }
            else { PointF[] points = { new(x, y - size), new(x + size, y), new(x, y + size), new(x - size, y) }; graphics.FillPolygon(fill, points); graphics.DrawPolygon(outline, points); }
        }
    }

    private static Color TeamColor(int team) => team switch
    {
        1 => Color.RoyalBlue, 2 => Color.Crimson, 3 => Color.Goldenrod, 4 => Color.MediumSeaGreen,
        5 => Color.MediumOrchid, 6 => Color.DarkOrange, 7 => Color.Cyan, 8 => Color.White, _ => Color.Silver
    };

    private Rectangle SceneBounds()
    {
        Rectangle fitted = Fit(new Size(_dimension, _dimension), ClientRectangle);
        int width = Math.Max(1, (int)(fitted.Width * _zoom)), height = Math.Max(1, (int)(fitted.Height * _zoom));
        return new Rectangle(fitted.X + (fitted.Width - width) / 2 + (int)_pan.X, fitted.Y + (fitted.Height - height) / 2 + (int)_pan.Y, width, height);
    }

    private static Bitmap? LoadBitmap(string path)
    {
        if (!File.Exists(path)) return null;
        using var source = new Bitmap(path); return new Bitmap(source);
    }

    private static void DrawMapOverlay(Graphics graphics, Bitmap? overlay, Rectangle target, float opacity)
    {
        if (overlay is null) return;
        using var attributes = new ImageAttributes();
        var matrix = new ColorMatrix { Matrix00 = 1, Matrix11 = 1, Matrix22 = 1, Matrix33 = opacity, Matrix44 = 1 };
        attributes.SetColorMatrix(matrix);
        graphics.DrawImage(overlay, target, 0, 0, overlay.Width, overlay.Height, GraphicsUnit.Pixel, attributes);
    }

    private static Bitmap BuildHeightShade(Bitmap heightMap)
    {
        int sourceWidth = heightMap.Width, sourceHeight = heightMap.Height;
        int[] source = BitmapPixels.Read(heightMap);
        int Red(int x, int y) => (source[y * sourceWidth + x] >> 16) & 0xff;
        int width = Math.Max(1, sourceWidth - 1), height = Math.Max(1, sourceHeight - 1);
        var shade = new int[width * height];
        for (int y = 0; y < height; y++) for (int x = 0; x < width; x++)
        {
            int left = Red(Math.Max(0, x - 1), y);
            int right = Red(Math.Min(sourceWidth - 1, x + 1), y);
            int top = Red(x, Math.Max(0, y - 1));
            int bottom = Red(x, Math.Min(sourceHeight - 1, y + 1));
            int light = Math.Clamp(128 + (left - right) * 2 + (top - bottom) * 2, 0, 255);
            int delta = Math.Abs(light - 128);
            shade[y * width + x] = light >= 128
                ? (Math.Min(115, delta) << 24) | (255 << 16) | (248 << 8) | 220
                : Math.Min(135, delta) << 24;
        }
        return BitmapPixels.Write(width, height, shade);
    }

    private static Bitmap BuildWaterOverlay(Bitmap heightMap, float threshold, Color waterColor)
    {
        int sourceWidth = heightMap.Width, sourceHeight = heightMap.Height;
        int[] source = BitmapPixels.Read(heightMap);
        int width = Math.Max(1, sourceWidth - 1), height = Math.Max(1, sourceHeight - 1);
        var water = new int[width * height];
        int colorBits = (waterColor.R << 16) | (waterColor.G << 8) | waterColor.B;
        for (int y = 0; y < height; y++) for (int x = 0; x < width; x++)
        {
            int elevation = (source[y * sourceWidth + x] >> 16) & 0xff;
            if (elevation > threshold + 2) continue;
            int alpha = elevation <= threshold ? 118 : (int)((threshold + 2 - elevation) / 2f * 118);
            water[y * width + x] = (Math.Clamp(alpha, 0, 118) << 24) | colorBits;
        }
        return BitmapPixels.Write(width, height, water);
    }

    private void RenderTerrainCell(int x, int y)
    {
        if (_terrainScene is null || _textures is null || _dimension <= 0) return;
        using Graphics graphics = Graphics.FromImage(_terrainScene);
        RenderTerrainCell(graphics, x, y);
    }

    private void RenderTerrainCell(Graphics graphics, int x, int y)
    {
        if (_terrainScene is null || _textures is null || _dimension <= 0) return;
        string name = _textures[y * _dimension + x];
        Rectangle target = new(x * 32, y * 32, 32, 32);
        Bitmap? texture = _floorTextures?.Get(name);
        if (texture is not null)
        {
            graphics.InterpolationMode = InterpolationMode.HighQualityBicubic;
            graphics.DrawImage(texture, target);
        }
        else
        {
            using var fill = new SolidBrush(GetTexturePreviewColor(name));
            graphics.FillRectangle(fill, target);
        }
    }

    private void BuildTexturePreviewColors()
    {
        _texturePreviewColors.Clear();
        if (_textures is null || _dimension <= 0) return;
        foreach (string textureName in _textures.Distinct(StringComparer.OrdinalIgnoreCase))
        {
            Bitmap? texture = _floorTextures?.Get(textureName);
            if (texture is null) continue;
            long r = 0, g = 0, b = 0, count = 0;
            int stepX = Math.Max(1, texture.Width / 16), stepY = Math.Max(1, texture.Height / 16);
            for (int y = 0; y < texture.Height; y += stepY) for (int x = 0; x < texture.Width; x += stepX)
            {
                Color pixel = texture.GetPixel(x, y); r += pixel.R; g += pixel.G; b += pixel.B; count++;
            }
            if (count > 0) _texturePreviewColors[textureName] = Color.FromArgb((int)(r / count), (int)(g / count), (int)(b / count));
        }
        if (_bitmap is null) return;
        int bitmapWidth = _bitmap.Width, bitmapHeight = _bitmap.Height;
        int[] minimap = BitmapPixels.Read(_bitmap);
        var sums = new Dictionary<string, (long R, long G, long B, long Count)>(StringComparer.OrdinalIgnoreCase);
        for (int y = 0; y < _dimension; y++) for (int x = 0; x < _dimension; x++)
        {
            string texture = _textures[y * _dimension + x];
            int left = x * bitmapWidth / _dimension, right = (x + 1) * bitmapWidth / _dimension;
            int top = y * bitmapHeight / _dimension, bottom = (y + 1) * bitmapHeight / _dimension;
            sums.TryGetValue(texture, out var sum);
            for (int py = top; py < bottom; py++) for (int px = left; px < right; px++) { int pixel = minimap[py * bitmapWidth + px]; sum.R += (pixel >> 16) & 0xff; sum.G += (pixel >> 8) & 0xff; sum.B += pixel & 0xff; sum.Count++; }
            sums[texture] = sum;
        }
        foreach (var pair in sums) if (pair.Value.Count > 0 && !_texturePreviewColors.ContainsKey(pair.Key)) _texturePreviewColors[pair.Key] = Color.FromArgb((int)(pair.Value.R / pair.Value.Count), (int)(pair.Value.G / pair.Value.Count), (int)(pair.Value.B / pair.Value.Count));
    }

    private static Color FallbackTextureColor(string texture)
    {
        int hash = StringComparer.OrdinalIgnoreCase.GetHashCode(texture);
        return Color.FromArgb(90 + (hash & 0x4f), 90 + ((hash >> 8) & 0x4f), 90 + ((hash >> 16) & 0x4f));
    }

    protected override void Dispose(bool disposing) { if (disposing) { _bitmap?.Dispose(); _terrainScene?.Dispose(); _emboss?.Dispose(); _smooth?.Dispose(); _heightShade?.Dispose(); _waterOverlay?.Dispose(); /* _floorTextures 由 MapEditorForm 擁有，不在此釋放 */ } base.Dispose(disposing); }
}

internal sealed class TexturePaintEventArgs : EventArgs
{
    public TexturePaintEventArgs(int x, int y, string previousTexture, string texture) { X = x; Y = y; PreviousTexture = previousTexture; Texture = texture; }
    public int X { get; }
    public int Y { get; }
    public string PreviousTexture { get; }
    public string Texture { get; }
}

internal sealed class TileHoverEventArgs : EventArgs
{
    public TileHoverEventArgs(int x, int y, string? texture) { X = x; Y = y; Texture = texture; }
    public int X { get; }
    public int Y { get; }
    public string? Texture { get; }
}

internal sealed class TextureSampleEventArgs : EventArgs
{
    public TextureSampleEventArgs(int x, int y, string texture) { X = x; Y = y; Texture = texture; }
    public int X { get; }
    public int Y { get; }
    public string Texture { get; }
}

internal sealed class SceneObjectMoveEventArgs : EventArgs
{
    public SceneObjectMoveEventArgs(float worldX, float worldZ, bool completed) { WorldX = worldX; WorldZ = worldZ; Completed = completed; }
    public float WorldX { get; }
    public float WorldZ { get; }
    public bool Completed { get; }
}
