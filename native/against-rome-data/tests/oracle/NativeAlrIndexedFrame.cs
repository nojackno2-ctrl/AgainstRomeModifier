namespace AgainstRomeMapEditor.NativeAssets;

/// <summary>
/// Owned ARGB pixels decoded from ALR's native 8-bit scanline representation.
/// Cleanroom C# implementation linking original NativeAlrDocument.
/// </summary>
internal sealed class NativeAlrIndexedFrame
{
    internal NativeAlrIndexedFrame(int width, int height, uint[] pixels)
    {
        Width = width;
        Height = height;
        ArgbPixels = Array.AsReadOnly(pixels);
    }

    public int Width { get; }
    public int Height { get; }
    public IReadOnlyList<uint> ArgbPixels { get; }

    /// <summary>
    /// Decodes the row descriptors used by native functions 0x4E49C0 and 0x4E4A60.
    /// Pure C# scanline decoder.
    /// </summary>
    public static NativeAlrIndexedFrame Decode(int width, int height, ReadOnlySpan<uint> rows,
        ReadOnlySpan<byte> data, ReadOnlySpan<uint> palette)
    {
        if (width is < 0 or > 2047) throw new ArgumentOutOfRangeException(nameof(width));
        if (height is < 0 or > 2047) throw new ArgumentOutOfRangeException(nameof(height));
        if (rows.Length != height + 1) throw new ArgumentException("ALR requires one descriptor per row plus an end descriptor.", nameof(rows));
        if (palette.Length is < 1 or > 511) throw new ArgumentException("ALR palette size must fit its native 9-bit field.", nameof(palette));
        var pixels = new uint[width * height];
        for (int y = 0; y < height; y++)
        {
            uint row = rows[y];
            int start = (int)(row & 0xFFFFF), end = (int)(rows[y + 1] & 0xFFFFF);
            if (end < start || end > data.Length) throw new InvalidDataException($"ALR row {y} has invalid data offsets.");
            int x = (int)((row >> 21) & 0x3FF);
            bool firstOpaque = (row & 0x80000000) != 0;
            ReadOnlySpan<byte> payload = data[start..end];
            if ((row & 0x100000) == 0)
            {
                WriteRun(pixels.AsSpan(y * width, width), x, payload, palette, firstOpaque);
                continue;
            }
            if (payload.Length < 2) throw new InvalidDataException($"ALR row {y} is missing its two-run prefix.");
            int firstLength = payload[0], gap = payload[1] & 0x7F;
            bool secondOpaque = (payload[1] & 0x80) != 0;
            if (firstLength > payload.Length - 2) throw new InvalidDataException($"ALR row {y} has an invalid first run length.");
            Span<uint> destination = pixels.AsSpan(y * width, width);
            WriteRun(destination, x, payload.Slice(2, firstLength), palette, firstOpaque);
            WriteRun(destination, x + firstLength + gap, payload[(2 + firstLength)..], palette, secondOpaque);
        }
        return new NativeAlrIndexedFrame(width, height, pixels);
    }

    private static void WriteRun(Span<uint> destination, int x, ReadOnlySpan<byte> indices,
        ReadOnlySpan<uint> palette, bool zeroOpaque)
    {
        if (x > destination.Length || indices.Length > destination.Length - x)
            throw new InvalidDataException("ALR scanline extends beyond its frame width.");
        for (int i = 0; i < indices.Length; i++)
        {
            int index = indices[i];
            if (index >= palette.Length) throw new InvalidDataException("ALR pixel references a missing palette color.");
            if (index != 0 || zeroOpaque) destination[x + i] = NativePaletteColor.ToArgb(palette[index]);
        }
    }
}

internal static partial class RustAlrFrameDecoder
{
    internal static IReadOnlyList<ushort> DecodeIndexed(int width, int height, uint[] rows,
        ReadOnlySpan<byte> data, int paletteLength)
    {
        if (width is < 0 or > 2047) throw new ArgumentOutOfRangeException(nameof(width));
        if (height is < 0 or > 2047) throw new ArgumentOutOfRangeException(nameof(height));
        if (rows.Length != height + 1) throw new ArgumentException("ALR requires one descriptor per row plus an end descriptor.", nameof(rows));
        if (paletteLength is < 1 or > 511) throw new ArgumentException("ALR palette size must fit its native 9-bit field.", nameof(paletteLength));

        var output = new ushort[width * height];
        for (int y = 0; y < height; y++)
        {
            uint row = rows[y];
            int start = (int)(row & 0xFFFFF), end = (int)(rows[y + 1] & 0xFFFFF);
            if (end < start || end > data.Length) throw new InvalidDataException($"ALR row {y} has invalid data offsets.");
            int x = (int)((row >> 21) & 0x3FF);
            bool firstOpaque = (row & 0x80000000) != 0;
            ReadOnlySpan<byte> payload = data[start..end];
            if ((row & 0x100000) == 0)
            {
                WriteIndexedRun(output.AsSpan(y * width, width), x, payload, paletteLength, firstOpaque);
                continue;
            }
            if (payload.Length < 2) throw new InvalidDataException($"ALR row {y} is missing its two-run prefix.");
            int firstLength = payload[0], gap = payload[1] & 0x7F;
            bool secondOpaque = (payload[1] & 0x80) != 0;
            if (firstLength > payload.Length - 2) throw new InvalidDataException($"ALR row {y} has an invalid first run length.");
            Span<ushort> destination = output.AsSpan(y * width, width);
            WriteIndexedRun(destination, x, payload.Slice(2, firstLength), paletteLength, firstOpaque);
            WriteIndexedRun(destination, x + firstLength + gap, payload[(2 + firstLength)..], paletteLength, secondOpaque);
        }
        return Array.AsReadOnly(output);
    }

    private static void WriteIndexedRun(Span<ushort> destination, int x, ReadOnlySpan<byte> indices,
        int paletteLength, bool opaque)
    {
        if (x > destination.Length || indices.Length > destination.Length - x)
            throw new InvalidDataException("ALR scanline extends beyond its frame width.");
        for (int i = 0; i < indices.Length; i++)
        {
            int index = indices[i];
            if (index >= paletteLength) throw new InvalidDataException("ALR pixel references a missing palette color.");
            destination[x + i] = (ushort)(0x100 | (opaque ? 0x200 : 0) | index);
        }
    }
}
