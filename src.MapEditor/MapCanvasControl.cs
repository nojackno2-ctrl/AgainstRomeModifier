using System.Drawing.Drawing2D;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor;

internal enum MapCanvasLayer { Textures, Minimap, Collision }

internal sealed class MapCanvasControl : Control
{
    private Bitmap? _bitmap;
    private string[]? _textures;
    private string[]? _baselineTextures;
    private readonly Dictionary<string, Color> _texturePreviewColors = new(StringComparer.OrdinalIgnoreCase);
    private int _dimension;
    private bool _painting;
    private readonly HashSet<int> _paintedInDrag = new();

    public MapCanvasLayer Layer { get; private set; } = MapCanvasLayer.Textures;
    public string? BrushTexture { get; set; }
    public bool EditingEnabled { get; set; }
    public bool ShowGrid { get; set; } = true;
    public event EventHandler<TexturePaintEventArgs>? TexturePainted;
    public event EventHandler<TileHoverEventArgs>? TileHovered;
    public event EventHandler<TextureSampleEventArgs>? TextureSampled;

    public MapCanvasControl()
    {
        DoubleBuffered = true;
        BackColor = Color.FromArgb(24, 28, 36);
        Dock = DockStyle.Fill;
        Cursor = Cursors.Cross;
    }

    public void LoadBitmapLayer(string mapDirectory, MapCanvasLayer layer)
    {
        _bitmap?.Dispose(); _bitmap = null; _textures = null; _baselineTextures = null; _texturePreviewColors.Clear(); _dimension = 0; Layer = layer;
        string name = layer == MapCanvasLayer.Minimap ? "minimap.bmp" : "collision.bmp";
        string path = Path.Combine(mapDirectory, name);
        if (File.Exists(path)) using (var source = new Bitmap(path)) _bitmap = new Bitmap(source);
        Invalidate();
    }

    public void LoadTextures(int dimension, IReadOnlyList<string> textures, IReadOnlyList<string> baselineTextures, string minimapPath)
    {
        _bitmap?.Dispose(); _bitmap = null; Layer = MapCanvasLayer.Textures;
        if (File.Exists(minimapPath)) using (var source = new Bitmap(minimapPath)) _bitmap = new Bitmap(source);
        _dimension = dimension; _textures = textures.ToArray(); _baselineTextures = baselineTextures.ToArray();
        BuildTexturePreviewColors(); Invalidate();
    }

    public void SetTexture(int x, int y, string texture)
    {
        if (_textures is null || _dimension <= 0) return;
        int index = y * _dimension + x;
        if (index < 0 || index >= _textures.Length) return;
        _textures[index] = texture;
        Invalidate();
    }

    protected override void OnPaint(PaintEventArgs e)
    {
        base.OnPaint(e);
        e.Graphics.SmoothingMode = SmoothingMode.HighQuality;
        if (Layer == MapCanvasLayer.Textures && _textures is not null && _dimension > 0)
        {
            DrawTextureEditor(e.Graphics);
            return;
        }

        if (_bitmap is not null)
        {
            Rectangle bounds = Fit(_bitmap.Size, ClientRectangle);
            e.Graphics.InterpolationMode = InterpolationMode.NearestNeighbor;
            e.Graphics.DrawImage(_bitmap, bounds);
            DrawFrame(e.Graphics, bounds);
            return;
        }

        TextRenderer.DrawText(e.Graphics, "請從左側選擇地圖", Font, ClientRectangle, Color.SlateGray,
            TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter);
    }

    private void DrawTextureEditor(Graphics graphics)
    {
        Rectangle bounds = Fit(new Size(_dimension, _dimension), ClientRectangle);
        if (_bitmap is not null)
        {
            graphics.InterpolationMode = InterpolationMode.HighQualityBicubic;
            graphics.DrawImage(_bitmap, bounds);
        }
        else
        {
            using var background = new SolidBrush(Color.FromArgb(55, 60, 68)); graphics.FillRectangle(background, bounds);
        }

        float cellWidth = bounds.Width / (float)_dimension, cellHeight = bounds.Height / (float)_dimension;
        if (_baselineTextures is not null && _baselineTextures.Length == _textures!.Length)
        {
            for (int index = 0; index < _textures.Length; index++)
            {
                if (StringComparer.OrdinalIgnoreCase.Equals(_textures[index], _baselineTextures[index])) continue;
                int x = index % _dimension, y = index / _dimension;
                Color preview = GetTexturePreviewColor(_textures[index]);
                using var overlay = new SolidBrush(Color.FromArgb(185, preview));
                RectangleF cell = new(bounds.X + x * cellWidth, bounds.Y + y * cellHeight, Math.Max(1, cellWidth), Math.Max(1, cellHeight));
                graphics.FillRectangle(overlay, cell);
                using var changedPen = new Pen(Color.FromArgb(230, 255, 220, 80), 1); graphics.DrawRectangle(changedPen, cell.X, cell.Y, cell.Width, cell.Height);
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
        DrawFrame(graphics, bounds);
    }

    protected override void OnMouseDown(MouseEventArgs e)
    {
        base.OnMouseDown(e);
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
        if (TryGetTile(e.Location, out int x, out int y)) TileHovered?.Invoke(this, new TileHoverEventArgs(x, y, _textures?[y * _dimension + x]));
        if (_painting && e.Button == MouseButtons.Left) TryPaint(e.Location);
    }

    protected override void OnMouseUp(MouseEventArgs e)
    {
        base.OnMouseUp(e); _painting = false; _paintedInDrag.Clear();
    }

    private void TryPaint(Point location)
    {
        if (!EditingEnabled || Layer != MapCanvasLayer.Textures || string.IsNullOrWhiteSpace(BrushTexture) || _textures is null) return;
        if (!TryGetTile(location, out int x, out int y)) return;
        int index = y * _dimension + x;
        if (!_paintedInDrag.Add(index)) return;
        string previous = _textures[index];
        if (StringComparer.OrdinalIgnoreCase.Equals(previous, BrushTexture)) return;
        _textures[index] = BrushTexture;
        TexturePainted?.Invoke(this, new TexturePaintEventArgs(x, y, previous, BrushTexture));
        Invalidate();
    }

    private bool TryGetTile(Point location, out int x, out int y)
    {
        x = y = -1;
        if (Layer != MapCanvasLayer.Textures || _textures is null || _dimension <= 0) return false;
        Rectangle bounds = Fit(new Size(_dimension, _dimension), ClientRectangle);
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

    private void BuildTexturePreviewColors()
    {
        _texturePreviewColors.Clear();
        if (_bitmap is null || _textures is null || _dimension <= 0) return;
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
        foreach (var pair in sums) if (pair.Value.Count > 0) _texturePreviewColors[pair.Key] = Color.FromArgb((int)(pair.Value.R / pair.Value.Count), (int)(pair.Value.G / pair.Value.Count), (int)(pair.Value.B / pair.Value.Count));
    }

    private static Color FallbackTextureColor(string texture)
    {
        int hash = StringComparer.OrdinalIgnoreCase.GetHashCode(texture);
        return Color.FromArgb(90 + (hash & 0x4f), 90 + ((hash >> 8) & 0x4f), 90 + ((hash >> 16) & 0x4f));
    }

    protected override void Dispose(bool disposing) { if (disposing) _bitmap?.Dispose(); base.Dispose(disposing); }
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
