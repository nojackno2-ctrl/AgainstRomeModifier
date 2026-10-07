using AgainstRomeMapEditor.NativeAssets;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class NativeAlrIndexedFrameTests
{
    [Fact]
    public void Native_row_flags_preserve_asymmetric_runs_gaps_and_index_zero_opacity()
    {
        // Row 0: leading gap 1, opaque [0, 1], gap 2, transparent [0, 2].
        // Row 1: leading gap 2, transparent [2, 0, 1]. End descriptor has no row flags.
        uint[] rows = [0x80000000 | (1u << 21) | 0x100000, (2u << 21) | 6, 9];
        byte[] bytes = [2, 2, 0, 1, 0, 2, 2, 0, 1, 0xAA]; // final byte is alignment padding
        uint[] palette = [0x123456, 0xCC1122, 0x3344EE];
        NativeAlrIndexedFrame frame = NativeAlrIndexedFrame.Decode(8, 2, rows, bytes, palette);
        Assert.Equal(8, frame.Width); Assert.Equal(2, frame.Height);
        Assert.Equal(new uint[] { 0, 0xFF123456, 0xFFCC1122, 0, 0, 0, 0xFF3344EE, 0,
            0, 0, 0xFF3344EE, 0, 0xFFCC1122, 0, 0, 0 }, frame.ArgbPixels);
        Array.Fill(rows, 0u); Array.Fill(bytes, (byte)0); Array.Fill(palette, 0u);
        Assert.Equal(0xFFCC1122u, frame.ArgbPixels[2]);
        Assert.Throws<NotSupportedException>(() => ((IList<uint>)frame.ArgbPixels)[2] = 0);
    }

    [Fact]
    public void Each_run_has_its_own_opacity_flag_and_palette_high_byte_does_not_set_alpha()
    {
        uint[] rows = [0x100000, 5];
        byte[] bytes = [1, 0x81, 0, 0, 1];
        var frame = NativeAlrIndexedFrame.Decode(5, 1, rows, bytes, [0xAA000000, 0x00FFFFFF]);
        Assert.Equal(new uint[] { 0, 0, 0xFF000000, 0xFFFFFFFF, 0 }, frame.ArgbPixels);
    }

    [Theory]
    [InlineData(0x100000u, 1u, new byte[] { 1 })] // missing two-run prefix
    [InlineData(0x100000u, 3u, new byte[] { 2, 0, 1 })] // first run longer than payload
    [InlineData(2u, 1u, new byte[] { 1, 1 })] // backwards offsets
    [InlineData(0u, 2u, new byte[] { 1 })] // truncated pixels
    [InlineData(0u, 1u, new byte[] { 2 })] // missing palette entry
    [InlineData(3u << 21, 1u, new byte[] { 1 })] // leading gap beyond width
    [InlineData(0x100000u, 4u, new byte[] { 1, 2, 1, 1 })] // second run beyond width
    public void Invalid_native_scanline_is_rejected(uint row, uint end, byte[] data)
        => Assert.Throws<InvalidDataException>(() => NativeAlrIndexedFrame.Decode(2, 1, [row, end], data, [0, 0x112233]));

    [Fact]
    public void Empty_rows_leave_transparent_pixels_and_invalid_dimensions_are_rejected()
    {
        var frame = NativeAlrIndexedFrame.Decode(2, 2, [0, 0, 0], [], [0]);
        Assert.All(frame.ArgbPixels, pixel => Assert.Equal(0u, pixel));
        Assert.Throws<ArgumentOutOfRangeException>(() => NativeAlrIndexedFrame.Decode(2048, 1, [0, 0], [], [0]));
        Assert.Throws<ArgumentOutOfRangeException>(() => NativeAlrIndexedFrame.Decode(1, 0, [0], [], [0]));
        Assert.Throws<ArgumentException>(() => NativeAlrIndexedFrame.Decode(1, 1, [0], [], [0]));
        Assert.Throws<ArgumentException>(() => NativeAlrIndexedFrame.Decode(1, 1, [0, 0], [], []));
    }
}
