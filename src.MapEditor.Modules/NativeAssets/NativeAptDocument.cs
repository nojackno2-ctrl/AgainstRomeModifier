using System.Buffers.Binary;

namespace AgainstRomeMapEditor.NativeAssets;

internal sealed record NativeAptFrameRange(int FirstTile, int TileCount);
internal sealed record NativeAptTileInfo(int DataOffset, uint PackedPosition, uint AuxiliaryBytes)
{
    public int X => (int)(PackedPosition & 0x7FF);
    public int Y => (int)((PackedPosition >> 11) & 0x7FF);
}

internal sealed class NativeAptIndexedImage(int width, int height, uint[] pixels)
{
    public int Width { get; } = width;
    public int Height { get; } = height;
    public IReadOnlyList<uint> ArgbPixels { get; } = Array.AsReadOnly(pixels);
}

/// <summary>
/// APAT v2/v3 indexed diamond patches following loader 0x4E5710 and row helper
/// 0x4E6CD0. These are raster patches, not assumed triangle meshes. Parses owned
/// bytes only; unknown group metadata and IFOM semantics are not interpreted.
/// </summary>
internal sealed class NativeAptDocument
{
    private readonly uint[] _rowOffsets, _rowWidths, _palettes;
    private readonly byte[] _data;

    private NativeAptDocument(uint version, int width, int height, int anchorX, int anchorY,
        uint[] layout, uint[] rowOffsets, uint[] rowWidths, uint[] palettes, int colors,
        int variants, NativeAptFrameRange[] frames, NativeAptTileInfo[] tiles, byte[] data)
    {
        Version = version; Width = width; Height = height; AnchorX = anchorX; AnchorY = anchorY;
        Layout = Array.AsReadOnly(layout); _rowOffsets = rowOffsets; _rowWidths = rowWidths;
        _palettes = palettes; PaletteColorCount = colors; PaletteVariantCount = variants;
        Frames = Array.AsReadOnly(frames); Tiles = Array.AsReadOnly(tiles); _data = data;
    }

    public uint Version { get; }
    public int Width { get; }
    public int Height { get; }
    public int AnchorX { get; }
    public int AnchorY { get; }
    public int PaletteColorCount { get; }
    public int PaletteVariantCount { get; }
    // Native linear index: (((a * Layout[1]) + b) * Layout[2] + c) * Layout[3] + d.
    public IReadOnlyList<uint> Layout { get; }
    public IReadOnlyList<NativeAptFrameRange> Frames { get; }
    public IReadOnlyList<NativeAptTileInfo> Tiles { get; }

    public static NativeAptDocument Parse(ReadOnlySpan<byte> source)
    {
        var reader = new Reader(source);
        uint[] header = reader.Words(28);
        if (header[0] != 0x54415041) throw new InvalidDataException("Missing APAT signature.");
        uint version = header[1];
        if (version is < 2 or > 3) throw new NotSupportedException($"APT version {version} is not supported; expected 2..3.");
        if (header[6] != 64 || header[7] != 31 || header[13] != 8)
            throw new NotSupportedException("Only 8-bit APT 64x31 diamond patches are supported.");
        int frameCount = Count(header[2]), tileCount = Count(header[4]);
        int groupVariants = Count(header[9]);
        if (groupVariants is < 1 or > 3) throw new NotSupportedException("APT group variants exceed the native three slots.");
        int width = Count(header[23]), height = Count(header[24]);
        if (width is < 1 or > 2048 || height is < 1 or > 2048)
            throw new InvalidDataException("Invalid APT canvas dimensions.");
        int colors = Count(header[20]), variants = Count(header[21]);
        if (colors is < 1 or > 256 || variants < 1) throw new InvalidDataException("Invalid APT palette dimensions.");
        if (header[3] < 112) throw new InvalidDataException("APT header overlaps its tables.");
        reader.Skip(header[3] - 112);
        uint[] rowOffsets = reader.Words(31), rowWidths = reader.Words(31);
        for (int y = 0; y < 31; y++)
            if (rowWidths[y] > 64) throw new InvalidDataException("APT row width exceeds its diamond.");
        _ = reader.Words(2); // First anchor pair has different native consumers.
        int anchorX = unchecked((int)reader.Word()), anchorY = unchecked((int)reader.Word());
        uint extraA = reader.Word(), extraB = reader.Word();
        int groups = Count(reader.Word());
        reader.Require((long)groups * (version == 3 ? 6 + groupVariants : 7) * 4);
        var groupCounts = new uint[groups][];
        for (int i = 0; i < groups; i++)
        {
            _ = reader.Words(2); // Group bounds/position pair, not mesh vertices.
            uint first = reader.Word(), second = reader.Word();
            uint[] perVariant = reader.Words(version == 3 ? groupVariants : 1);
            // v2 stores this list once and the loader replicates it into variant slots.
            groupCounts[i] = [first, second, .. perVariant, reader.Word(), reader.Word()];
        }
        reader.Skip(((long)extraA + extraB) * 8);
        foreach (uint[] counts in groupCounts)
        {
            long bytes = (long)counts[0] * 12 + (long)counts[1] * 8;
            for (int i = 2; i < counts.Length - 2; i++) bytes += (long)counts[i] * 8;
            bytes += (long)counts[^2] * 8 + (long)counts[^1] * 16;
            reader.Skip(bytes);
        }
        reader.Require((long)frameCount * 8);
        var frames = new NativeAptFrameRange[frameCount];
        for (int i = 0; i < frames.Length; i++)
        {
            int first = Count(reader.Word()), count = Count(reader.Word());
            if ((long)first + count > tileCount) throw new InvalidDataException("APT frame references tiles outside its table.");
            frames[i] = new NativeAptFrameRange(first, count);
        }
        long paletteWords = (long)colors * variants;
        if (paletteWords > int.MaxValue) throw new InvalidDataException("APT palettes are too large.");
        uint[] palettes = reader.Words((int)paletteWords);
        reader.Require((long)tileCount * 12);
        var tiles = new NativeAptTileInfo[tileCount];
        for (int i = 0; i < tiles.Length; i++)
            tiles[i] = new NativeAptTileInfo(Count(reader.Word()), reader.Word(), reader.Word());
        if (reader.Word() != 0x54414450) throw new InvalidDataException("Missing APT PDAT block.");
        byte[] data = reader.Bytes(Count(header[27])).ToArray();
        foreach (var tile in tiles)
            if ((long)tile.DataOffset + 8 > data.Length) throw new InvalidDataException("APT tile points outside its pixel block.");
        return new NativeAptDocument(version, width, height, anchorX, anchorY, header[8..12],
            rowOffsets, rowWidths, palettes, colors, variants, frames, tiles, data);
    }

    public NativeAptIndexedImage DecodeTile(int tileIndex, int paletteVariant = 0)
    {
        ValidatePalette(paletteVariant);
        if ((uint)tileIndex >= (uint)Tiles.Count) throw new ArgumentOutOfRangeException(nameof(tileIndex));
        var pixels = new uint[64 * 31];
        WriteTile(tileIndex, paletteVariant, pixels, 64, 31, 30, 0);
        return new NativeAptIndexedImage(64, 31, pixels);
    }

    public NativeAptIndexedImage DecodeFrame(int frameIndex, int paletteVariant = 0)
    {
        ValidatePalette(paletteVariant);
        if ((uint)frameIndex >= (uint)Frames.Count) throw new ArgumentOutOfRangeException(nameof(frameIndex));
        var pixels = new uint[Width * Height];
        NativeAptFrameRange frame = Frames[frameIndex];
        for (int index = frame.FirstTile; index < frame.FirstTile + frame.TileCount; index++)
            WriteTile(index, paletteVariant, pixels, Width, Height, Tiles[index].X, Tiles[index].Y);
        return new NativeAptIndexedImage(Width, Height, pixels);
    }

    private void WriteTile(int tileIndex, int variant, uint[] pixels, int width, int height, int centerX, int topY)
    {
        ReadOnlySpan<byte> data = _data.AsSpan(Tiles[tileIndex].DataOffset);
        uint flags = BinaryPrimitives.ReadUInt32LittleEndian(data), opaque = BinaryPrimitives.ReadUInt32LittleEndian(data[4..]);
        bool compressed = (flags & 0x80000000) != 0;
        if (compressed && data.Length < 72) throw new InvalidDataException("APT compressed tile is missing its row table.");
        for (int y = 0; y < 31; y++)
        {
            if ((flags & (1u << y)) != 0) continue; // Native consumer skips this row.
            int start = Count(_rowOffsets[y]), length = (int)_rowWidths[y], gap = 0;
            if (compressed)
            {
                ushort row = BinaryPrimitives.ReadUInt16LittleEndian(data[(8 + y * 2)..]);
                ushort next = BinaryPrimitives.ReadUInt16LittleEndian(data[(10 + y * 2)..]);
                start = row & 0x7FF; length = (next & 0x7FF) - start; gap = (row >> 9) & 0x3C;
                if (start < 64) throw new InvalidDataException("APT pixels overlap the compressed row table.");
            }
            if (length < 0 || gap + length > _rowWidths[y] || (long)start + length + 8 > data.Length)
                throw new InvalidDataException("APT row has invalid pixel offsets or width.");
            int x = centerX + gap + 2 - (int)_rowWidths[y] / 2;
            bool zeroOpaque = (opaque & (1u << y)) != 0;
            for (int index = 0; index < length; index++)
            {
                int color = data[8 + start + index];
                if (color >= PaletteColorCount) throw new InvalidDataException("APT pixel references a missing palette color.");
                if (color == 0 && !zeroOpaque) continue;
                int dx = x + index, dy = topY + y;
                if ((uint)dx >= (uint)width || (uint)dy >= (uint)height)
                    throw new InvalidDataException("APT visible pixel extends outside its canvas.");
                pixels[dy * width + dx] = NativePaletteColor.ToArgb(_palettes[variant * PaletteColorCount + color]);
            }
        }
    }

    private void ValidatePalette(int variant)
    {
        if ((uint)variant >= (uint)PaletteVariantCount) throw new ArgumentOutOfRangeException(nameof(variant));
    }

    private static int Count(uint count) => count <= int.MaxValue ? (int)count : throw new InvalidDataException("APT count exceeds supported bounds.");

    private ref struct Reader(ReadOnlySpan<byte> source)
    {
        private readonly ReadOnlySpan<byte> _source = source;
        private int _position;
        public void Require(long count)
        {
            if (count < 0 || count > _source.Length - _position) throw new InvalidDataException("Truncated APT data.");
        }
        public uint Word()
        {
            Require(4); uint value = BinaryPrimitives.ReadUInt32LittleEndian(_source[_position..]); _position += 4; return value;
        }
        public uint[] Words(int count)
        {
            Require((long)count * 4); var words = new uint[count];
            for (int i = 0; i < count; i++) words[i] = Word();
            return words;
        }
        public void Skip(long count) { Require(count); _position += (int)count; }
        public ReadOnlySpan<byte> Bytes(int count)
        {
            Require(count); var bytes = _source.Slice(_position, count); _position += count; return bytes;
        }
    }
}
