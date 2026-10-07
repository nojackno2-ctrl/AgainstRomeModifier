using System.Buffers.Binary;
using System.Numerics;

namespace AgainstRomeMapEditor.NativeAssets;

/// <summary>純 24-bit BI_RGB 解码，top-down RGB；不依賴 System.Drawing 或安裝路徑。</summary>
internal sealed class MapLightingBitmap
{
    private readonly byte[] _rgb;
    private MapLightingBitmap(int width, int height, byte[] rgb) { Width = width; Height = height; _rgb = rgb; }
    public int Width { get; }
    public int Height { get; }

    public static MapLightingBitmap Parse(ReadOnlySpan<byte> source)
    {
        if (source.Length < 54 || BinaryPrimitives.ReadUInt16LittleEndian(source) != 0x4D42)
            throw new InvalidDataException("Missing or truncated lighting BMP header.");
        uint declaredSize = Word(source, 2), offset = Word(source, 10);
        if (declaredSize != source.Length || Word(source, 6) != 0)
            throw new InvalidDataException("Invalid BMP size or reserved fields.");
        if (Word(source, 14) != 40 || BinaryPrimitives.ReadUInt16LittleEndian(source[28..]) != 24 || Word(source, 30) != 0)
            throw new NotSupportedException("Only 24-bit BITMAPINFOHEADER BI_RGB lighting BMPs are supported.");
        int width = unchecked((int)Word(source, 18)), signedHeight = unchecked((int)Word(source, 22));
        if (width <= 0 || signedHeight is 0 or int.MinValue || BinaryPrimitives.ReadUInt16LittleEndian(source[26..]) != 1)
            throw new InvalidDataException("Invalid BMP dimensions or plane count.");
        int height = Math.Abs(signedHeight);
        long stride = ((long)width * 3 + 3) & ~3L, extent = stride * height;
        uint imageSize = Word(source, 34);
        if (offset < 54 || offset > source.Length || extent > source.Length - (long)offset ||
            (imageSize != 0 && imageSize != extent) || (long)width * height * 3 > int.MaxValue)
            throw new InvalidDataException("Invalid BMP pixel extent.");
        var rgb = new byte[width * height * 3];
        for (int y = 0; y < height; y++)
        {
            int storedY = signedHeight < 0 ? y : height - 1 - y;
            var row = source.Slice((int)(offset + storedY * stride), width * 3);
            for (int x = 0; x < width; x++)
            {
                int dest = (y * width + x) * 3;
                rgb[dest] = row[x * 3 + 2]; rgb[dest + 1] = row[x * 3 + 1]; rgb[dest + 2] = row[x * 3];
            }
        }
        return new MapLightingBitmap(width, height, rgb);
    }

    /// <summary>回傳未正規化的 RGB bytes（0..255）。</summary>
    public Vector3 ColorAt(int x, int y)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(x);
        ArgumentOutOfRangeException.ThrowIfGreaterThanOrEqual(x, Width);
        ArgumentOutOfRangeException.ThrowIfNegative(y);
        ArgumentOutOfRangeException.ThrowIfGreaterThanOrEqual(y, Height);
        int i = (y * Width + x) * 3;
        return new Vector3(_rgb[i], _rgb[i + 1], _rgb[i + 2]);
    }
    private static uint Word(ReadOnlySpan<byte> bytes, int offset) => BinaryPrimitives.ReadUInt32LittleEndian(bytes[offset..]);
}
