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
    private bool _painting;
    private bool _panning;
    private Point _panStart;
    private PointF _pan = PointF.Empty;
    private float _zoom = 1f;
    private readonly HashSet<int> _paintedInDrag = new();

    public string? BrushTexture { get; set; }
    public int BrushSize { get; set; } = 1;
    public bool EditingEnabled { get; set; }
    public bool ShowGrid { get; set; } = true;
    public bool ShowObjects { get; set; } = true;
    public int SceneObjectCount => _sceneObjects.Count;
    public event EventHandler<TexturePaintEventArgs>? TexturePainted;
    public event EventHandler<TileHoverEventArgs>? TileHovered;
    public event EventHandler<TextureSampleEventArgs>? TextureSampled;

    public MapCanvasControl()
    {
        DoubleBuffered = true;
        BackColor = Color.FromArgb(24, 28, 36);
        Dock = DockStyle.Fill;
        Cursor = Cursors.Cross;
        SetStyle(ControlStyles.Selectable, true);
    }

    public bool LoadTextures(int dimension, IReadOnlyList<string> textures, IReadOnlyList<string> baselineTextures, string mapDirectory, string floorTextureArchivePath, IReadOnlyList<MapSceneObject> sceneObjects, float waterLevel, float heightMapStep, Color waterColor)
    {
        _bitmap?.Dispose(); _bitmap = null; _terrainScene?.Dispose(); _terrainScene = null; _emboss?.Dispose(); _emboss = null; _smooth?.Dispose(); _smooth = null; _heightShade?.Dispose(); _heightShade = null; _waterOverlay?.Dispose(); _waterOverlay = null; _floorTextures?.Dispose(); _floorTextures = null;
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
        _zoom = 1f; _pan = PointF.Empty;
        _floorTextures = new FloorTextureLibrary(floorTextureArchivePath);
        _sceneObjects = sceneObjects;
        BuildTexturePreviewColors(); RenderTerrainScene(); Invalidate();
        return _floorTextures.IsAvailable;
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
        DrawFrame(graphics, bounds);
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
        _painting = true; _paintedInDrag.Clear(); TryPaint(e.Location);
    }

    protected override void OnMouseMove(MouseEventArgs e)
    {
        base.OnMouseMove(e);
        if (_panning && e.Button == MouseButtons.Middle)
        {
            _pan = new PointF(_pan.X + e.X - _panStart.X, _pan.Y + e.Y - _panStart.Y); _panStart = e.Location; Invalidate(); return;
        }
        if (TryGetTile(e.Location, out int x, out int y)) TileHovered?.Invoke(this, new TileHoverEventArgs(x, y, _textures?[y * _dimension + x]));
        if (_painting && e.Button == MouseButtons.Left) TryPaint(e.Location);
    }

    protected override void OnMouseUp(MouseEventArgs e)
    {
        base.OnMouseUp(e); _painting = false; _panning = false; Cursor = Cursors.Cross; _paintedInDrag.Clear();
    }

    protected override void OnMouseWheel(MouseEventArgs e)
    {
        base.OnMouseWheel(e);
        float previous = _zoom;
        _zoom = Math.Clamp(_zoom * (e.Delta > 0 ? 1.2f : 1f / 1.2f), .75f, 6f);
        if (Math.Abs(previous - _zoom) < .001f) return;
        _pan = new PointF(_pan.X * (_zoom / previous), _pan.Y * (_zoom / previous)); Invalidate();
    }

    private void TryPaint(Point location)
    {
        if (!EditingEnabled || string.IsNullOrWhiteSpace(BrushTexture) || _textures is null) return;
        if (!TryGetTile(location, out int x, out int y)) return;
        int radius = Math.Max(0, BrushSize / 2);
        for (int paintY = Math.Max(0, y - radius); paintY <= Math.Min(_dimension - 1, y + radius); paintY++)
        for (int paintX = Math.Max(0, x - radius); paintX <= Math.Min(_dimension - 1, x + radius); paintX++)
        {
            int index = paintY * _dimension + paintX;
            if (!_paintedInDrag.Add(index)) continue;
            string previous = _textures[index];
            if (StringComparer.OrdinalIgnoreCase.Equals(previous, BrushTexture)) continue;
            _textures[index] = BrushTexture;
            RenderTerrainCell(paintX, paintY);
            TexturePainted?.Invoke(this, new TexturePaintEventArgs(paintX, paintY, previous, BrushTexture));
        }
        Invalidate();
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
        int width = Math.Max(1, heightMap.Width - 1), height = Math.Max(1, heightMap.Height - 1);
        var shade = new Bitmap(width, height, PixelFormat.Format32bppArgb);
        for (int y = 0; y < height; y++) for (int x = 0; x < width; x++)
        {
            int left = heightMap.GetPixel(Math.Max(0, x - 1), y).R;
            int right = heightMap.GetPixel(Math.Min(heightMap.Width - 1, x + 1), y).R;
            int top = heightMap.GetPixel(x, Math.Max(0, y - 1)).R;
            int bottom = heightMap.GetPixel(x, Math.Min(heightMap.Height - 1, y + 1)).R;
            int light = Math.Clamp(128 + (left - right) * 2 + (top - bottom) * 2, 0, 255);
            int delta = Math.Abs(light - 128);
            shade.SetPixel(x, y, light >= 128 ? Color.FromArgb(Math.Min(115, delta), 255, 248, 220) : Color.FromArgb(Math.Min(135, delta), 0, 0, 0));
        }
        return shade;
    }

    private static Bitmap BuildWaterOverlay(Bitmap heightMap, float threshold, Color waterColor)
    {
        int width = Math.Max(1, heightMap.Width - 1), height = Math.Max(1, heightMap.Height - 1);
        var water = new Bitmap(width, height, PixelFormat.Format32bppArgb);
        for (int y = 0; y < height; y++) for (int x = 0; x < width; x++)
        {
            int elevation = heightMap.GetPixel(x, y).R;
            if (elevation > threshold + 2) continue;
            int alpha = elevation <= threshold ? 118 : (int)((threshold + 2 - elevation) / 2f * 118);
            water.SetPixel(x, y, Color.FromArgb(Math.Clamp(alpha, 0, 118), waterColor));
        }
        return water;
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
        var sums = new Dictionary<string, (long R, long G, long B, long Count)>(StringComparer.OrdinalIgnoreCase);
        for (int y = 0; y < _dimension; y++) for (int x = 0; x < _dimension; x++)
        {
            string texture = _textures[y * _dimension + x];
            int left = x * _bitmap.Width / _dimension, right = (x + 1) * _bitmap.Width / _dimension;
            int top = y * _bitmap.Height / _dimension, bottom = (y + 1) * _bitmap.Height / _dimension;
            sums.TryGetValue(texture, out var sum);
            for (int py = top; py < bottom; py++) for (int px = left; px < right; px++) { Color pixel = _bitmap.GetPixel(px, py); sum.R += pixel.R; sum.G += pixel.G; sum.B += pixel.B; sum.Count++; }
            sums[texture] = sum;
        }
        foreach (var pair in sums) if (pair.Value.Count > 0 && !_texturePreviewColors.ContainsKey(pair.Key)) _texturePreviewColors[pair.Key] = Color.FromArgb((int)(pair.Value.R / pair.Value.Count), (int)(pair.Value.G / pair.Value.Count), (int)(pair.Value.B / pair.Value.Count));
    }

    private static Color FallbackTextureColor(string texture)
    {
        int hash = StringComparer.OrdinalIgnoreCase.GetHashCode(texture);
        return Color.FromArgb(90 + (hash & 0x4f), 90 + ((hash >> 8) & 0x4f), 90 + ((hash >> 16) & 0x4f));
    }

    protected override void Dispose(bool disposing) { if (disposing) { _bitmap?.Dispose(); _terrainScene?.Dispose(); _emboss?.Dispose(); _smooth?.Dispose(); _heightShade?.Dispose(); _waterOverlay?.Dispose(); _floorTextures?.Dispose(); } base.Dispose(disposing); }
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
