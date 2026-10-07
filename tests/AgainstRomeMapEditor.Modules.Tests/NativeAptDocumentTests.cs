using System.Buffers.Binary;
using AgainstRomeMapEditor.NativeAssets;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class NativeAptDocumentTests
{
    [Theory]
    [InlineData(2u)]
    [InlineData(3u)]
    public void Raw_and_compressed_diamonds_compose_without_erasing_transparent_pixels(uint version)
    {
        byte[] bytes = Fixture(version);
        var document = NativeAptDocument.Parse(bytes);
        Array.Fill(bytes, (byte)0);
        Assert.Equal(version, document.Version);
        Assert.Equal(30, document.AnchorX); Assert.Equal(15, document.AnchorY);
        Assert.Equal(new uint[] { 1, 2, 1, 1 }, document.Layout);
        Assert.Equal(new NativeAptFrameRange(0, 1), document.Frames[0]);
        Assert.Equal(new NativeAptFrameRange(0, 2), document.Frames[1]);
        Assert.Equal(30, document.Tiles[0].X); Assert.Equal(0, document.Tiles[0].Y);
        var raw = document.DecodeTile(0);
        Assert.Equal(64, raw.Width); Assert.Equal(31, raw.Height);
        Assert.Equal(new uint[] { 0xFF332211, 0, 0xFF332211, 0 }, raw.ArgbPixels.Skip(30).Take(4));
        Assert.All(raw.ArgbPixels.Skip(64), color => Assert.Equal(0u, color));
        var composed = document.DecodeFrame(1);
        Assert.Equal(new uint[] { 0xFF776655, 0xFF332211, 0xFF332211, 0 }, composed.ArgbPixels.Skip(30).Take(4));
        Assert.Equal(new uint[] { 0xFF998877, 0xFFEFCDAB, 0xFFEFCDAB, 0 }, document.DecodeFrame(1, 1).ArgbPixels.Skip(30).Take(4));
        Assert.Throws<NotSupportedException>(() => ((IList<uint>)composed.ArgbPixels)[0] = 1);
        Assert.Throws<NotSupportedException>(() => ((IList<NativeAptFrameRange>)document.Frames)[0] = new(0, 0));
        Assert.Throws<ArgumentOutOfRangeException>(() => document.DecodeTile(-1));
        Assert.Throws<ArgumentOutOfRangeException>(() => document.DecodeFrame(2));
        Assert.Throws<ArgumentOutOfRangeException>(() => document.DecodeFrame(0, 2));
    }

    [Fact]
    public void Compressed_row_gap_is_measured_in_four_pixels_and_uses_full_diamond_center()
    {
        byte[] bytes = Fixture(3);
        int blob = BlobOffset(bytes);
        // Move the compressed row to a wider row (width 8), leaving a 4-pixel gap.
        BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(blob + 1032), 0xFFFFFFFD);
        BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(blob + 1036), 2);
        BinaryPrimitives.WriteUInt16LittleEndian(bytes.AsSpan(blob + 1032 + 10), 0x800 | 64);
        var tile = NativeAptDocument.Parse(bytes).DecodeTile(1);
        Assert.Equal(0u, tile.ArgbPixels[64 + 31]);
        Assert.Equal(0xFF776655u, tile.ArgbPixels[64 + 32]);
        Assert.Equal(0xFF332211u, tile.ArgbPixels[64 + 33]);
    }

    [Theory]
    [InlineData(0, 0u)]
    [InlineData(8, uint.MaxValue)]
    [InlineData(12, 108u)]
    [InlineData(80, 0u)]
    [InlineData(84, uint.MaxValue)]
    [InlineData(92, 2049u)]
    [InlineData(236, 65u)]
    public void Malformed_counts_and_tables_are_rejected(int offset, uint value)
    {
        byte[] bytes = Fixture(3); Put(bytes, offset, value);
        Assert.Throws<InvalidDataException>(() => NativeAptDocument.Parse(bytes));
    }

    [Fact]
    public void Truncation_bad_references_and_bad_pixel_rows_are_rejected()
    {
        byte[] bytes = Fixture(3);
        for (int length = 0; length < bytes.Length; length++)
            Assert.Throws<InvalidDataException>(() => NativeAptDocument.Parse(bytes.AsSpan(0, length)));
        int blob = BlobOffset(bytes), tiles = blob - 28, frames = tiles - 32;
        byte[] broken = bytes.ToArray(); Put(broken, frames + 4, 3);
        Assert.Throws<InvalidDataException>(() => NativeAptDocument.Parse(broken));
        broken = bytes.ToArray(); Put(broken, tiles, uint.MaxValue);
        Assert.Throws<InvalidDataException>(() => NativeAptDocument.Parse(broken));
        broken = bytes.ToArray(); broken[blob + 8] = 2;
        Assert.Throws<InvalidDataException>(() => NativeAptDocument.Parse(broken).DecodeTile(0));
        broken = bytes.ToArray(); BinaryPrimitives.WriteUInt16LittleEndian(broken.AsSpan(blob + 1032 + 8), 63);
        Assert.Throws<InvalidDataException>(() => NativeAptDocument.Parse(broken).DecodeTile(1));
        broken = bytes.ToArray(); Put(broken, tiles + 4, 2047);
        Assert.Throws<InvalidDataException>(() => NativeAptDocument.Parse(broken).DecodeFrame(0));
        Assert.Equal(bytes, Fixture(3)); // No reader/decode mutation.
    }

    [Theory]
    [InlineData(4, 1u)]
    [InlineData(4, 4u)]
    [InlineData(24, 32u)]
    [InlineData(28, 15u)]
    [InlineData(52, 16u)]
    [InlineData(36, 0u)]
    [InlineData(36, 4u)]
    public void Unsupported_layouts_are_explicit(int offset, uint value)
    {
        byte[] bytes = Fixture(3); Put(bytes, offset, value);
        Assert.Throws<NotSupportedException>(() => NativeAptDocument.Parse(bytes));
    }

    internal static byte[] Fixture(uint version)
    {
        using var stream = new MemoryStream(); using var writer = new BinaryWriter(stream);
        uint[] header = new uint[28];
        header[0] = 0x54415041; header[1] = version; header[2] = 2; header[3] = 112; header[4] = 2;
        header[6] = 64; header[7] = 31; header[8] = 1; header[9] = 2; header[10] = 1; header[11] = 1;
        header[13] = 8; header[20] = 2; header[21] = 2; header[23] = 64; header[24] = 31; header[27] = 1108;
        foreach (uint word in header) writer.Write(word);
        int position = 0;
        for (int y = 0; y < 31; y++) { writer.Write(position); position += RowWidth(y); }
        for (int y = 0; y < 31; y++) writer.Write(RowWidth(y));
        foreach (uint word in new uint[] { 32, 20, 30, 15, 1, 1, 1 }) writer.Write(word);
        writer.Write(-1000000); writer.Write(-1000000); writer.Write(1); writer.Write(1);
        writer.Write(1); if (version == 3) writer.Write(1); // v2's list is stored once.
        writer.Write(1); writer.Write(1);
        writer.Write(new byte[16]); // Two extra pairs.
        writer.Write(new byte[version == 3 ? 60 : 52]); // Opaque group lists.
        foreach (uint word in new uint[] { 0, 1, 0, 2, 0xAA556677, 0xBB112233, 0xCC778899, 0xDDABCDEF }) writer.Write(word);
        foreach (uint word in new uint[] { 0, 30, 0, 1032, 30, 0 }) writer.Write(word);
        writer.Write(0x54414450u);
        writer.Write(0x7FFFFFFEu); writer.Write(0u);
        writer.Write(new byte[] { 1, 0, 1, 0 }); writer.Write(new byte[1020]);
        writer.Write(0xFFFFFFFEu); writer.Write(1u);
        for (int y = 0; y < 32; y++) writer.Write((ushort)(y == 0 ? 64 : 66));
        writer.Write(new byte[] { 0, 1, 0, 0 }); // 4-byte alignment padding.
        return stream.ToArray();
    }

    private static int RowWidth(int y) => y <= 15 ? (y + 1) * 4 : (31 - y) * 4;
    private static int BlobOffset(byte[] bytes) => bytes.Length - 1108;
    private static void Put(byte[] bytes, int offset, uint value) => BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(offset), value);
}
