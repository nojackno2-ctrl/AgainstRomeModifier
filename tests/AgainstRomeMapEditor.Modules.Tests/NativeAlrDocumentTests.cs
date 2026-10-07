using AgainstRomeMapEditor.NativeAssets;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class NativeAlrDocumentTests
{
    [Theory]
    [InlineData(4u)]
    [InlineData(5u)]
    [InlineData(6u)]
    public void Raw_container_preserves_shared_frames_palette_variants_offsets_and_pixels(uint version)
    {
        byte[] source = Fixture(version);
        var document = NativeAlrDocument.Parse(source);
        Assert.Equal(version, document.Version);
        Assert.Equal(2u, document.LayoutColumns); Assert.Equal(2u, document.LayoutRows);
        Assert.Equal(12u, document.AnchorWidth); Assert.Equal(18u, document.AnchorHeight);
        Assert.Equal(2, document.PaletteVariantCount);
        Assert.Equal(4, document.Frames.Count);
        Assert.Equal(new NativeAlrFrameInfo(7, 11, 5, 1, 2, Size(5, 1, 2)), document.Frames[0]);
        Assert.Same(document.Frames[0], document.Frames[1]);
        Assert.Same(document.Frames[0], document.Frames[3]);
        Assert.Throws<NotSupportedException>(() => ((IList<NativeAlrFrameInfo>)document.Frames)[0] = document.Frames[2]);
        Array.Fill(source, (byte)0); // Parsed records own their bytes.
        Assert.Equal(new uint[] { 0, 0xFF102030, 0, 0xFF102030, 0 }, document.DecodeFrame(0).ArgbPixels);
        Assert.Equal(document.DecodeFrame(0).ArgbPixels, document.DecodeFrame(1).ArgbPixels);
        Assert.Equal(document.DecodeFrame(0).ArgbPixels, document.DecodeFrame(3).ArgbPixels);
        Assert.Equal(new uint[] { 0, 0xFF90A0B0, 0, 0xFF90A0B0, 0 }, document.DecodeFrame(0, 1).ArgbPixels);
        // Frame 2 contains a deliberately different palette; native helper still selects frame 0's palette.
        Assert.Equal(new uint[] { 0xFF102030, 0xFF405060 }, document.DecodeFrame(2).ArgbPixels);
        Assert.Equal(new uint[] { 0xFF90A0B0, 0xFFC0D0E0 }, document.DecodeFrame(2, 1).ArgbPixels);
        Assert.Throws<ArgumentOutOfRangeException>(() => document.DecodeFrame(-1));
        Assert.Throws<ArgumentOutOfRangeException>(() => document.DecodeFrame(4));
        Assert.Throws<ArgumentOutOfRangeException>(() => document.DecodeFrame(0, -1));
        Assert.Throws<ArgumentOutOfRangeException>(() => document.DecodeFrame(0, 2));
    }

    [Fact]
    public void Every_truncated_prefix_of_a_complete_frame_table_is_rejected()
    {
        byte[] source = Fixture(6);
        for (int length = 0; length < source.Length; length++)
            Assert.Throws<InvalidDataException>(() => NativeAlrDocument.Parse(source.AsSpan(0, length)));
        Assert.Equal(4, NativeAlrDocument.Parse(source).Frames.Count);
    }

    [Theory]
    [InlineData(0, 0u)] // signature
    [InlineData(12, 0u)] // frame count
    [InlineData(12, uint.MaxValue)] // frame count cannot be allocated from remaining bytes
    [InlineData(32, 0u)] // palette variants
    [InlineData(32, uint.MaxValue)] // variant overflow
    [InlineData(92, 0u)] // first frame references itself
    [InlineData(96, uint.MaxValue)] // invalid row table position
    [InlineData(104, 0u)] // empty frame extent
    [InlineData(108, uint.MaxValue)] // byte count overflow/truncation
    [InlineData(140, 3u)] // forward shared reference
    public void Invalid_header_or_frame_reference_is_rejected(int byteOffset, uint value)
    {
        byte[] source = Fixture(6);
        WriteWord(source, byteOffset, value);
        Assert.Throws<InvalidDataException>(() => NativeAlrDocument.Parse(source));
    }

    [Fact]
    public void Unsupported_layouts_are_explicit_and_frame_decode_rejects_corrupt_row_data()
    {
        byte[] source = Fixture(6);
        WriteWord(source, 4, 3);
        Assert.Throws<NotSupportedException>(() => NativeAlrDocument.Parse(source));
        WriteWord(source, 4, 7);
        Assert.Throws<NotSupportedException>(() => NativeAlrDocument.Parse(source));
        WriteWord(source, 4, 6); WriteWord(source, 16, 24);
        Assert.Throws<NotSupportedException>(() => NativeAlrDocument.Parse(source));
        source = Fixture(6); source[128] = 2; // Palette index outside the first frame palette.
        Assert.Throws<InvalidDataException>(() => NativeAlrDocument.Parse(source).DecodeFrame(0));
        source = Fixture(6); WriteWord(source, 136, 15); // Backwards row end (starts at payload offset 16).
        Assert.Throws<InvalidDataException>(() => NativeAlrDocument.Parse(source).DecodeFrame(0));
    }

    [Fact]
    public void Frame_without_local_palette_uses_first_record_and_trailing_metadata_is_not_interpreted()
    {
        byte[] source = Fixture(6, secondFramePalette: false);
        source = [.. source, 0x49, 0x46, 0x4F, 0x4D, 0, 0, 0, 0]; // opaque trailing IFOM block
        var document = NativeAlrDocument.Parse(source);
        Assert.Equal(0, document.Frames[2].PaletteColorCount);
        Assert.Equal(new uint[] { 0xFF102030, 0xFF405060 }, document.DecodeFrame(2).ArgbPixels);
        Assert.Equal(new uint[] { 0xFF90A0B0, 0xFFC0D0E0 }, document.DecodeFrame(2, 1).ArgbPixels);
    }

    private static byte[] Fixture(uint version, bool secondFramePalette = true)
    {
        using var stream = new MemoryStream();
        using var writer = new BinaryWriter(stream);
        // Exact 23-word v4..v6 native header read order. Unknown words are distinct sentinels.
        foreach (uint word in new uint[] { 0x41524C41, version, 0x100, 4, 8, 0x200, 2, 2, 2, 0x300,
            0x401, 0x402, 0x403, 0x404, 0x405, 12, 18, 0x501, 0x502, 0x503, 0x504, 0x601, 0x602 }) writer.Write(word);
        Record(writer, 7, 11, 5, [0x405060, 0x102030, 0xC0D0E0, 0x90A0B0],
            [1, 0, 1], [(1u << 21) | 16, 19]);
        writer.Write(0); // shared first frame
        uint secondOffset = secondFramePalette ? 16u : 0;
        Record(writer, 2, 3, 2, secondFramePalette ? [0, 0xFFFFFF, 0, 0xEEEEEE] : [],
            [1, 0], [0x80000000 | secondOffset, secondOffset + 2]);
        writer.Write(1); // transitive shared frame
        return stream.ToArray();
    }

    private static void Record(BinaryWriter writer, int offsetX, int offsetY, int width,
        uint[] palette, byte[] pixels, uint[] rows)
    {
        writer.Write(-1);
        writer.Write((uint)(palette.Length * 4 + ((pixels.Length + 3) & ~3)));
        writer.Write((uint)(offsetX | offsetY << 16));
        writer.Write(Size(width, 1, palette.Length / 2));
        writer.Write((uint)pixels.Length);
        foreach (uint color in palette) writer.Write(color);
        writer.Write(pixels);
        for (int i = pixels.Length; (i & 3) != 0; i++) writer.Write((byte)0xAA);
        foreach (uint row in rows) writer.Write(row);
    }

    private static uint Size(int width, int height, int colors) => (uint)(width | height << 11 | colors << 22);
    private static void WriteWord(byte[] bytes, int offset, uint value)
        => System.Buffers.Binary.BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(offset), value);
}
