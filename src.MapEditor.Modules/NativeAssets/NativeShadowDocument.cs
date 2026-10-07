using System.Buffers.Binary;

namespace AgainstRomeMapEditor.NativeAssets;

/// <summary>
/// 自有 bytes 的單張 8-bit BI_RGB shadow BMP；不是 ALRA 動畫容器。
/// 遮罩取原始 index，忽略 RGBQUAD 顏色：native 0x40FE80 / 0x410380。
/// </summary>
internal sealed class NativeShadowDocument
{
    private readonly byte[] _mask;

    private NativeShadowDocument(int width, int height, int stride, bool topDown,
        int colors, int trailingBytes, byte[] mask)
    {
        Width = width; Height = height; StoredStride = stride; IsTopDown = topDown;
        PaletteColorCount = colors; TrailingByteCount = trailingBytes; _mask = mask;
    }

    public int Width { get; }
    public int Height { get; }
    public int StoredStride { get; }
    public bool IsTopDown { get; }
    public int PaletteColorCount { get; }
    public int TrailingByteCount { get; }
    public int FrameCount => 1;

    public static NativeShadowDocument Parse(ReadOnlySpan<byte> source)
    {
        if (source.Length < 54) throw new InvalidDataException("Truncated shadow BMP header.");
        if (BinaryPrimitives.ReadUInt16LittleEndian(source) != 0x4D42)
            throw new InvalidDataException("Missing BM signature.");
        if (Word(source, 2) != source.Length || Word(source, 6) != 0)
            throw new InvalidDataException("Invalid shadow BMP file size or reserved fields.");
        if (Word(source, 14) != 40)
            throw new NotSupportedException("Only BITMAPINFOHEADER (40 bytes) shadow BMPs are supported.");
        int width = unchecked((int)Word(source, 18)), signedHeight = unchecked((int)Word(source, 22));
        if (width <= 0 || signedHeight is 0 or int.MinValue)
            throw new InvalidDataException("Invalid shadow BMP dimensions.");
        if (BinaryPrimitives.ReadUInt16LittleEndian(source[26..]) != 1)
            throw new InvalidDataException("Invalid shadow BMP plane count.");
        if (BinaryPrimitives.ReadUInt16LittleEndian(source[28..]) != 8 || Word(source, 30) != 0)
            throw new NotSupportedException("Only uncompressed 8-bit BI_RGB shadow BMPs are supported.");
        int height = Math.Abs(signedHeight);
        long stride = ((long)width + 3) & ~3L, pixelBytes = stride * height;
        uint colors = Word(source, 46);
        if (colors == 0) colors = 256;
        uint important = Word(source, 50), offset = Word(source, 10), imageSize = Word(source, 34);
        if (colors > 256 || important > colors || offset < 54L + colors * 4L ||
            offset > source.Length || pixelBytes > source.Length - (long)offset ||
            (imageSize != 0 && imageSize != pixelBytes) || (long)width * height > int.MaxValue)
            throw new InvalidDataException("Invalid shadow BMP palette, pixel offset, or payload extent.");
        // 不讀 RGBQUAD：LakaDorn 的 palette 全為 (1,1,1)，仍須保留不同 index 強度。
        var mask = new byte[width * height];
        for (int y = 0; y < height; y++)
        {
            int storedY = signedHeight < 0 ? y : height - 1 - y;
            ReadOnlySpan<byte> row = source.Slice((int)(offset + storedY * stride), width);
            for (int x = 0; x < width; x++)
            {
                if (row[x] >= colors) throw new InvalidDataException("Shadow BMP index exceeds its palette table.");
                mask[y * width + x] = row[x];
            }
        }
        return new NativeShadowDocument(width, height, (int)stride, signedHeight < 0,
            (int)colors, (int)(source.Length - offset - pixelBytes), mask);
    }

    public NativeShadowFrame DecodeFrame(int frameIndex = 0)
    {
        ArgumentOutOfRangeException.ThrowIfNotEqual(frameIndex, 0);
        return new NativeShadowFrame(Width, Height, (byte[])_mask.Clone());
    }

    private static uint Word(ReadOnlySpan<byte> source, int offset)
        => BinaryPrimitives.ReadUInt32LittleEndian(source[offset..]);
}

/// <summary>Top-down 強度遮罩及預覽用 black ARGB；native 強度的分母是 256。</summary>
internal sealed class NativeShadowFrame
{
    private readonly byte[] _mask;
    private readonly uint[] _argb;

    internal NativeShadowFrame(int width, int height, byte[] ownedMask)
    {
        Width = width; Height = height; _mask = ownedMask;
        _argb = new uint[ownedMask.Length];
        for (int i = 0; i < _argb.Length; i++)
            _argb[i] = (uint)((ownedMask[i] * 255 + 128) >> 8) << 24;
    }

    public int Width { get; }
    public int Height { get; }
    /// <summary>原生強度 0..255；0 保留地面色，255 保留地面色的 1/256。</summary>
    public ReadOnlySpan<byte> AlphaMask => _mask;
    /// <summary>分母 256 的強度轉為標準 alpha/255（四捨五入）；僅作預覽。</summary>
    public ReadOnlySpan<uint> ArgbPixels => _argb;

    /// <summary>重現 0x410380 的逐色道 floor(channel * (256 - strength) / 256)。</summary>
    public static uint DarkenArgb(uint destination, byte strength)
    {
        uint factor = 256u - strength;
        uint r = ((destination >> 16) & 255) * factor >> 8;
        uint g = ((destination >> 8) & 255) * factor >> 8;
        uint b = (destination & 255) * factor >> 8;
        return (destination & 0xFF000000) | r << 16 | g << 8 | b;
    }
}