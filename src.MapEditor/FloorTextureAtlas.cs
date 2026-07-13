using System.Drawing.Imaging;
using System.Numerics;

namespace AgainstRomeMapEditor;

internal readonly record struct AtlasRect(int X, int Y, int Width, int Height)
{
    public Vector2[] ToUv(int atlasSize)
    {
        float inset = .5f / atlasSize;
        float left = X / (float)atlasSize + inset, top = Y / (float)atlasSize + inset;
        float right = (X + Width) / (float)atlasSize - inset, bottom = (Y + Height) / (float)atlasSize - inset;
        return new[] { new Vector2(left, top), new Vector2(right, top), new Vector2(right, bottom), new Vector2(left, bottom) };
    }
}

internal sealed class FloorTextureAtlas : IDisposable
{
    private readonly Dictionary<string, AtlasRect> _rects;
    private FloorTextureAtlas(Bitmap image, Dictionary<string, AtlasRect> rects) { Image = image; _rects = rects; }
    public Bitmap Image { get; }
    public int Size => Image.Width;

    public Vector2[] GetUv(string texture) => _rects.TryGetValue(texture, out AtlasRect rect) ? rect.ToUv(Size) : new AtlasRect(0, 0, Size, Size).ToUv(Size);

    public static FloorTextureAtlas Create(IEnumerable<string> names, FloorTextureLibrary library)
    {
        string[] unique = names.Distinct(StringComparer.OrdinalIgnoreCase).ToArray();
        const int cell = 128;
        int columns = 1;
        while (columns * columns < Math.Max(1, unique.Length)) columns *= 2;
        int size = columns * cell <= 2048 ? 2048 : 4096;
        if (columns * cell > size) throw new InvalidOperationException("This map uses too many distinct ground textures for one atlas.");
        var image = new Bitmap(size, size, PixelFormat.Format32bppArgb);
        using Graphics graphics = Graphics.FromImage(image);
        graphics.Clear(Color.FromArgb(90, 90, 90));
        var rects = new Dictionary<string, AtlasRect>(StringComparer.OrdinalIgnoreCase);
        for (int index = 0; index < unique.Length; index++)
        {
            int x = index % columns * cell, y = index / columns * cell;
            var rect = new AtlasRect(x, y, cell, cell);
            rects.Add(unique[index], rect);
            Bitmap? texture = library.Get(unique[index]);
            if (texture is not null) graphics.DrawImage(texture, new Rectangle(x, y, cell, cell));
            else using (var fill = new SolidBrush(FallbackColor(unique[index]))) graphics.FillRectangle(fill, x, y, cell, cell);
        }
        return new FloorTextureAtlas(image, rects);
    }

    public void Dispose() => Image.Dispose();

    internal static IReadOnlyDictionary<string, AtlasRect> Layout(IEnumerable<string> names, int atlasSize = 2048)
    {
        string[] unique = names.Distinct(StringComparer.OrdinalIgnoreCase).ToArray();
        int columns = 1; while (columns * columns < Math.Max(1, unique.Length)) columns *= 2;
        if (columns * 128 > atlasSize) throw new InvalidOperationException("Atlas capacity exceeded.");
        return unique.Select((name, index) => new { name, rect = new AtlasRect(index % columns * 128, index / columns * 128, 128, 128) })
            .ToDictionary(x => x.name, x => x.rect, StringComparer.OrdinalIgnoreCase);
    }

    private static Color FallbackColor(string texture)
    {
        int hash = StringComparer.OrdinalIgnoreCase.GetHashCode(texture);
        return Color.FromArgb(90 + (hash & 0x4f), 90 + ((hash >> 8) & 0x4f), 90 + ((hash >> 16) & 0x4f));
    }
}
