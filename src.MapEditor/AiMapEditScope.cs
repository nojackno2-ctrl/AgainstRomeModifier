namespace AgainstRomeMapEditor;

/// <summary>Immutable tile protection contract shared by preview and Apply.</summary>
internal sealed class AiMapEditScope
{
    private readonly Rectangle[] _locked;
    internal Rectangle Bounds { get; }
    internal bool EditPassability { get; }
    internal IReadOnlyList<Rectangle> Locked => Array.AsReadOnly(_locked);
    internal AiMapEditScope(Rectangle bounds, IEnumerable<Rectangle> locked, bool editPassability = true)
    {
        static bool Valid(Rectangle r) => r.X >= 0 && r.Y >= 0 && r.X < 64 && r.Y < 64 && r.Width > 0 && r.Height > 0 && r.Width <= 64 && r.Height <= 64 && r.Right <= 64 && r.Bottom <= 64;
        _locked = locked.ToArray();
        if (!Valid(bounds) || _locked.Any(r => !Valid(r))) throw new ArgumentException("範圍須為 X,Y,寬,高，位於 64×64 地圖內。");
        Bounds = bounds;
        EditPassability = editPassability;
    }
    internal bool AllowsTile(int x, int y) => Bounds.Contains(x, y) && !_locked.Any(r => r.Contains(x, y));
    // A vertex/corner belongs to up to four tiles; protect all neighbouring tile surfaces.
    internal bool AllowsVertex(float x, float y, int dimension)
    {
        int left = (int)MathF.Ceiling(x) - 1, right = (int)MathF.Floor(x);
        int top = (int)MathF.Ceiling(y) - 1, bottom = (int)MathF.Floor(y);
        for (int ty = top; ty <= bottom; ty++)
            for (int tx = left; tx <= right; tx++)
                if (tx >= 0 && ty >= 0 && tx < dimension && ty < dimension && !AllowsTile(tx, ty)) return false;
        return true;
    }
    internal static AiMapEditScope Parse(string bounds, string locked, bool editPassability = true)
    {
        static Rectangle Read(string text)
        {
            string[] parts = text.Split(',');
            if (parts.Length != 4 || parts.Any(p => !int.TryParse(p.Trim(), out _))) throw new ArgumentException("請輸入 X,Y,寬,高，例如 8,8,24,24。");
            int[] values = parts.Select(p => int.Parse(p.Trim(), System.Globalization.CultureInfo.InvariantCulture)).ToArray();
            return new(values[0], values[1], values[2], values[3]);
        }
        return new(Read(bounds), locked.Split(['\r', '\n'], StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries).Select(Read), editPassability);
    }
}
