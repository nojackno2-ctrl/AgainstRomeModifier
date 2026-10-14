namespace AgainstRomeMapEditor.NativeAssets;

/// <summary>
/// Owned ARGB pixels decoded from ALR's native 8-bit scanline representation.
/// Container parsing, animation selection and non-indexed formats are separate concerns.
/// </summary>
internal sealed class NativeAlrIndexedFrame
{
    private NativeAlrIndexedFrame(int width, int height, uint[] pixels)
    {
        Width = width; Height = height; ArgbPixels = Array.AsReadOnly(pixels);
    }

    public int Width { get; }
    public int Height { get; }
    public IReadOnlyList<uint> ArgbPixels { get; }

    /// <summary>
    /// Decode the row descriptors used by native functions 0x4E49C0 and 0x4E4A60.
    /// Offsets occupy bits 0..19, bit 20 introduces two runs, bits 21..30 specify
    /// leading transparent pixels, and bit 31 makes palette index zero opaque.
    /// The final descriptor supplies the end offset; data is the native payload
    /// excluding serialized palettes and including possible alignment padding.
    /// </summary>
    public static NativeAlrIndexedFrame Decode(int width, int height, ReadOnlySpan<uint> rows,
        ReadOnlySpan<byte> data, ReadOnlySpan<uint> palette)
    {
        if (width is < 0 or > 2047) throw new ArgumentOutOfRangeException(nameof(width));
        if (height is < 0 or > 2047) throw new ArgumentOutOfRangeException(nameof(height));
        if (rows.Length != height + 1) throw new ArgumentException("ALR requires one descriptor per row plus an end descriptor.", nameof(rows));
        if (palette.Length is < 1 or > 511) throw new ArgumentException("ALR palette size must fit its native 9-bit field.", nameof(palette));
        uint[] pixels = RustAlrFrameDecoder.Decode(width, height, rows, data, palette);
        return new NativeAlrIndexedFrame(width, height, pixels);
    }
}
