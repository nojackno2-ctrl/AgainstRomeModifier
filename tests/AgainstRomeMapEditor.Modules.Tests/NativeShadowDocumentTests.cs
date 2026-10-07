using System.Buffers.Binary;
using AgainstRomeMapEditor.NativeAssets;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class NativeShadowDocumentTests
{
    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public void Padded_rows_are_top_down_owned_index_strengths_independent_of_palette(bool topDown)
    {
        byte[] source = Fixture(topDown);
        var document = NativeShadowDocument.Parse(source);
        Assert.Equal(3, document.Width); Assert.Equal(2, document.Height);
        Assert.Equal(4, document.StoredStride); Assert.Equal(topDown, document.IsTopDown);
        Assert.Equal(256, document.PaletteColorCount); Assert.Equal(1, document.FrameCount);
        Assert.Equal(0, document.TrailingByteCount);
        Array.Fill(source, (byte)0);
        var frame = document.DecodeFrame();
        Assert.Equal(new byte[] { 0, 1, 128, 255, 64, 192 }, frame.AlphaMask.ToArray());
        Assert.Equal(new uint[] { 0, 0x01000000, 0x80000000, 0xFE000000, 0x40000000, 0xBF000000 },
            frame.ArgbPixels.ToArray());
        Assert.Equal(frame.AlphaMask.ToArray(), document.DecodeFrame().AlphaMask.ToArray());
        Assert.Throws<ArgumentOutOfRangeException>(() => document.DecodeFrame(-1));
        Assert.Throws<ArgumentOutOfRangeException>(() => document.DecodeFrame(1));
    }

    [Fact]
    public void Every_prefix_is_rejected_even_when_declared_size_is_adjusted_to_the_prefix()
    {
        byte[] source = Fixture();
        for (int length = 0; length < source.Length; length++)
        {
            byte[] prefix = source[..length];
            Assert.Throws<InvalidDataException>(() => NativeShadowDocument.Parse(prefix));
            if (length >= 6)
            {
                Write(prefix, 2, (uint)length);
                Assert.Throws<InvalidDataException>(() => NativeShadowDocument.Parse(prefix));
            }
        }
    }

    [Theory]
    [InlineData(2, 0u)]
    [InlineData(2, uint.MaxValue)]
    [InlineData(6, 1u)]
    [InlineData(10, 0u)]
    [InlineData(10, 1077u)]
    [InlineData(10, uint.MaxValue)]
    [InlineData(18, 0u)]
    [InlineData(18, uint.MaxValue)]
    [InlineData(18, int.MaxValue)]
    [InlineData(22, 0u)]
    [InlineData(22, 0x80000000u)]
    [InlineData(22, int.MaxValue)]
    [InlineData(34, 7u)]
    [InlineData(34, uint.MaxValue)]
    [InlineData(46, 257u)]
    [InlineData(50, 257u)]
    public void Bad_header_offsets_dimensions_and_extents_are_rejected(int offset, uint value)
    {
        byte[] source = Fixture(); Write(source, offset, value);
        Assert.Throws<InvalidDataException>(() => NativeShadowDocument.Parse(source));
    }

    [Fact]
    public void Bad_signature_planes_palette_and_extra_bytes_are_rejected()
    {
        byte[] source = Fixture(); source[0] = 0;
        Assert.Throws<InvalidDataException>(() => NativeShadowDocument.Parse(source));
        source = Fixture(); source[26] = 2;
        Assert.Throws<InvalidDataException>(() => NativeShadowDocument.Parse(source));
        source = Fixture(); Write(source, 46, 2); // Index 128 cannot reference a two-entry palette.
        Assert.Throws<InvalidDataException>(() => NativeShadowDocument.Parse(source));
        Assert.Throws<InvalidDataException>(() => NativeShadowDocument.Parse([.. Fixture(), 0]));
    }

    [Theory]
    [InlineData(14, 12u)]
    [InlineData(14, 108u)]
    [InlineData(30, 1u)]
    [InlineData(30, 2u)]
    public void Unsupported_dib_or_compression_is_explicit(int offset, uint value)
    {
        byte[] source = Fixture(); Write(source, offset, value);
        Assert.Throws<NotSupportedException>(() => NativeShadowDocument.Parse(source));
    }

    [Theory]
    [InlineData(1)]
    [InlineData(4)]
    [InlineData(24)]
    [InlineData(32)]
    public void Unsupported_depth_is_explicit(ushort depth)
    {
        byte[] source = Fixture(); BinaryPrimitives.WriteUInt16LittleEndian(source.AsSpan(28), depth);
        Assert.Throws<NotSupportedException>(() => NativeShadowDocument.Parse(source));
    }

    [Fact]
    public void Zero_colors_and_image_size_and_declared_two_byte_trailer_match_real_bmps()
    {
        byte[] source = [.. Fixture(), 0xAA, 0xBB];
        Write(source, 2, (uint)source.Length); Write(source, 34, 0); Write(source, 46, 0);
        var document = NativeShadowDocument.Parse(source);
        Assert.Equal(256, document.PaletteColorCount); Assert.Equal(2, document.TrailingByteCount);
        Assert.Equal(new byte[] { 0, 1, 128, 255, 64, 192 }, document.DecodeFrame().AlphaMask.ToArray());
    }

    [Fact]
    public void Native_darkening_uses_256_denominator_and_preserves_destination_alpha()
    {
        Assert.Equal(0x7F804020u, NativeShadowFrame.DarkenArgb(0x7F804020, 0));
        Assert.Equal(0x7F402010u, NativeShadowFrame.DarkenArgb(0x7F804020, 128));
        Assert.Equal(0x7F000000u, NativeShadowFrame.DarkenArgb(0x7FFFFFFF, 255));
        Assert.Equal(0x7FFEFEFEu, NativeShadowFrame.DarkenArgb(0x7FFFFFFF, 1));
        for (int strength = 0; strength < 256; strength++)
            Assert.Equal((uint)(255 * (256 - strength) / 256),
                NativeShadowFrame.DarkenArgb(0xFFFFFFFF, (byte)strength) & 255);
    }

    private static byte[] Fixture(bool topDown = false)
    {
        var bytes = new byte[1086];
        BinaryPrimitives.WriteUInt16LittleEndian(bytes, 0x4D42);
        Write(bytes, 2, (uint)bytes.Length); Write(bytes, 10, 1078); Write(bytes, 14, 40);
        Write(bytes, 18, 3); Write(bytes, 22, unchecked((uint)(topDown ? -2 : 2)));
        BinaryPrimitives.WriteUInt16LittleEndian(bytes.AsSpan(26), 1);
        BinaryPrimitives.WriteUInt16LittleEndian(bytes.AsSpan(28), 8);
        Write(bytes, 34, 8); Write(bytes, 46, 256);
        // Deliberately constant colored palette: index, not RGB or palette alpha, is the mask.
        for (int i = 0; i < 256; i++) Write(bytes, 54 + i * 4, 0xEE204080);
        byte[] pixels = topDown ? [0, 1, 128, 0xAA, 255, 64, 192, 0xBB] :
            [255, 64, 192, 0xBB, 0, 1, 128, 0xAA];
        pixels.CopyTo(bytes, 1078);
        return bytes;
    }

    private static void Write(byte[] bytes, int offset, uint value)
        => BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(offset), value);
}