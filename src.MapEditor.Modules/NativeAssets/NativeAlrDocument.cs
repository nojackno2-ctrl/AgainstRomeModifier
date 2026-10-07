using System.Buffers.Binary;

namespace AgainstRomeMapEditor.NativeAssets;

internal sealed record NativeAlrFrameInfo(int OffsetX, int OffsetY, int Width, int Height,
    int PaletteColorCount, uint PackedSize);

/// <summary>
/// Owned ALRA frame records following native loader 0x4E3A20. Supports the
/// common header layout of versions 4..6 and 8-bit indexed frames only.
/// Optional trailing metadata and animation/direction semantics are not interpreted.
/// </summary>
internal sealed class NativeAlrDocument
{
    private sealed record Frame(NativeAlrFrameInfo Info, byte[] Payload, uint[] Rows);
    private readonly Frame[] _frames;

    private NativeAlrDocument(uint version, uint columns, uint rows, int variants,
        uint anchorWidth, uint anchorHeight, Frame[] frames)
    {
        Version = version; LayoutColumns = columns; LayoutRows = rows; PaletteVariantCount = variants;
        AnchorWidth = anchorWidth; AnchorHeight = anchorHeight; _frames = frames;
        Frames = Array.AsReadOnly(frames.Select(frame => frame.Info).ToArray());
    }

    public uint Version { get; }
    public uint LayoutColumns { get; }
    public uint LayoutRows { get; }
    public int PaletteVariantCount { get; }
    public uint AnchorWidth { get; }
    public uint AnchorHeight { get; }
    public IReadOnlyList<NativeAlrFrameInfo> Frames { get; }

    public static NativeAlrDocument Parse(ReadOnlySpan<byte> source)
    {
        var reader = new Reader(source);
        if (reader.UInt32() != 0x41524C41) throw new InvalidDataException("Missing ALRA signature.");
        uint version = reader.UInt32();
        if (version is < 4 or > 6) throw new NotSupportedException($"ALR version {version} is not supported; expected 4..6.");
        _ = reader.UInt32(); // Native local +0x78; not a semantic animation field.
        uint count = reader.UInt32();
        if (reader.UInt32() != 8) throw new NotSupportedException("Only 8-bit indexed ALR frames are supported.");
        _ = reader.UInt32(); // Runtime +0x0C.
        uint columns = reader.UInt32(), rows = reader.UInt32(), variants = reader.UInt32();
        if (variants is 0 or > int.MaxValue) throw new InvalidDataException("Invalid ALR palette variant count.");
        _ = reader.UInt32(); // Runtime +0x3C.
        for (int i = 0; i < 5; i++) _ = reader.UInt32(); // Runtime +0x18..+0x28.
        uint anchorWidth = reader.UInt32(), anchorHeight = reader.UInt32(); // Runtime +0x2C/+0x30.
        for (int i = 0; i < 6; i++) _ = reader.UInt32(); // Runtime +0x48..+0x54 and two pre-frame words.
        if (count == 0 || count > int.MaxValue || (long)count * 4 > reader.Remaining)
            throw new InvalidDataException("Invalid or truncated ALR frame count.");
        var frames = new Frame[(int)count];
        for (int index = 0; index < frames.Length; index++)
        {
            int reference = unchecked((int)reader.UInt32());
            if (reference >= 0)
            {
                if (reference >= index) throw new InvalidDataException("ALR shared frame must reference an earlier record.");
                frames[index] = frames[reference];
                continue;
            }
            uint tableOffset = reader.UInt32(), position = reader.UInt32(), size = reader.UInt32();
            uint storedPixelBytes = reader.UInt32();
            int width = (int)(size & 0x7FF), height = (int)((size >> 11) & 0x7FF);
            int colors = (int)((size >> 22) & 0x1FF);
            long paletteBytes = (long)variants * colors * 4;
            long payloadBytes = paletteBytes + (((long)storedPixelBytes + 3) & ~3L);
            long tableBytes = (height + 1L) * 4;
            if (tableOffset != payloadBytes || payloadBytes > int.MaxValue || payloadBytes + tableBytes > reader.Remaining)
                throw new InvalidDataException("ALR frame has an invalid row table offset or truncated payload.");
            // Serialized row offsets address pixel bytes after the local palettes.
            // Runtime frame pointer layout must not be confused with serialized offsets.
            byte[] payload = reader.Bytes((int)payloadBytes).ToArray();
            var descriptors = new uint[height + 1];
            for (int y = 0; y < descriptors.Length; y++) descriptors[y] = reader.UInt32();
            frames[index] = new Frame(new NativeAlrFrameInfo((int)(position & 0xFFFF),
                (int)(position >> 16), width, height, colors, size), payload, descriptors);
        }
        return new NativeAlrDocument(version, columns, rows, (int)variants, anchorWidth, anchorHeight, frames);
    }

    public NativeAlrIndexedFrame DecodeFrame(int frameIndex, int paletteVariant = 0)
    {
        if ((uint)frameIndex >= (uint)_frames.Length) throw new ArgumentOutOfRangeException(nameof(frameIndex));
        if ((uint)paletteVariant >= (uint)PaletteVariantCount) throw new ArgumentOutOfRangeException(nameof(paletteVariant));
        // Native helper 0x4E48F0 obtains all frame palettes from the first frame record.
        Frame first = _frames[0], selected = _frames[frameIndex];
        int colors = first.Info.PaletteColorCount;
        if (colors == 0) throw new NotSupportedException("Indexed ALR has no palette in its first frame.");
        var palette = new uint[colors];
        ReadOnlySpan<byte> paletteBytes = first.Payload.AsSpan(paletteVariant * colors * 4, colors * 4);
        for (int i = 0; i < colors; i++) palette[i] = BinaryPrimitives.ReadUInt32LittleEndian(paletteBytes[(i * 4)..]);
        return NativeAlrIndexedFrame.Decode(selected.Info.Width, selected.Info.Height,
            selected.Rows, selected.Payload.AsSpan(PaletteVariantCount * selected.Info.PaletteColorCount * 4), palette);
    }

    private ref struct Reader(ReadOnlySpan<byte> source)
    {
        private readonly ReadOnlySpan<byte> _source = source;
        private int _position;
        public readonly int Remaining => _source.Length - _position;
        public uint UInt32() => BinaryPrimitives.ReadUInt32LittleEndian(Bytes(4));
        public ReadOnlySpan<byte> Bytes(int count)
        {
            if (count < 0 || count > Remaining) throw new InvalidDataException($"Truncated ALR at offset 0x{_position:X}.");
            ReadOnlySpan<byte> bytes = _source.Slice(_position, count);
            _position += count;
            return bytes;
        }
    }
}
