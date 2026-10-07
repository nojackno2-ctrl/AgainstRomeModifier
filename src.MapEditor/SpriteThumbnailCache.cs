using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using AgainstRomeMapEditor.NativeAssets;

namespace AgainstRomeMapEditor;

/// <summary>依 sprite 參考快取透明縮圖；DPI 尺寸改變時釋放舊圖。</summary>
internal sealed class SpriteThumbnailCache : IDisposable
{
    private readonly Dictionary<NativeSprite, Bitmap> _images = new(ReferenceEqualityComparer.Instance);
    private int _size;

    public Bitmap Get(NativeSprite sprite, int size)
    {
        if (_size != size) { Dispose(); _size = size; }
        if (_images.TryGetValue(sprite, out Bitmap? cached)) return cached;
        using var source = new Bitmap(sprite.Width, sprite.Height, PixelFormat.Format32bppArgb);
        BitmapData data = source.LockBits(new Rectangle(0, 0, source.Width, source.Height),
            ImageLockMode.WriteOnly, PixelFormat.Format32bppArgb);
        try
        {
            int[] pixels = MemoryMarshal.Cast<uint, int>(sprite.ArgbPixels.AsSpan()).ToArray();
            for (int y = 0; y < sprite.Height; y++)
                Marshal.Copy(pixels, y * sprite.Width, IntPtr.Add(data.Scan0, y * data.Stride), sprite.Width);
        }
        finally { source.UnlockBits(data); }
        var thumbnail = new Bitmap(size, size, PixelFormat.Format32bppArgb);
        try
        {
            using var graphics = Graphics.FromImage(thumbnail);
            graphics.Clear(Color.Transparent);
            graphics.InterpolationMode = InterpolationMode.HighQualityBicubic;
            graphics.PixelOffsetMode = PixelOffsetMode.HighQuality;
            graphics.CompositingQuality = CompositingQuality.HighQuality;
            float scale = Math.Min((float)size / sprite.Width, (float)size / sprite.Height);
            float width = sprite.Width * scale, height = sprite.Height * scale;
            using var attributes = new ImageAttributes();
            attributes.SetWrapMode(WrapMode.TileFlipXY);
            graphics.DrawImage(source, Rectangle.Round(new RectangleF((size - width) / 2, (size - height) / 2, width, height)),
                0, 0, sprite.Width, sprite.Height, GraphicsUnit.Pixel, attributes);
            _images.Add(sprite, thumbnail);
            return thumbnail;
        }
        catch { thumbnail.Dispose(); throw; }
    }

    public void Dispose()
    {
        foreach (Bitmap image in _images.Values) image.Dispose();
        _images.Clear();
    }
}
