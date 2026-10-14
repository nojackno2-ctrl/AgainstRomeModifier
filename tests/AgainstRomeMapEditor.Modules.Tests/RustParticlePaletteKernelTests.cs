using AgainstRomeMapEditor.NativeAssets;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class RustParticlePaletteKernelTests
{
    [Fact]
    public void Explicit_endpoint_preserves_rgb_key_and_requires_verified_aligned_alpha_suffix()
    {
        uint[] source = [0xaa405060, 0x102030];
        Assert.Equal(new uint[] { 0xaa405060, 0x1f0f1f2f }, RustAlrFrameDecoder.PrepareEndpointDefault(
            source, 0xff405060, 255, 255, 255, 0, 1, 4));
        Assert.Equal(new uint[] { 0xaa405060, 0x474f57 }, RustAlrFrameDecoder.PrepareEndpointDefault(
            source, 0x405060, 255, 255, 255, 0x808080, 0, null));
        Assert.Equal(new uint[] { 0x4f3f4f5f, 0x1f0f1f2f }, RustAlrFrameDecoder.PrepareEndpointDefault(
            source, 0, 255, 255, 255, 0, uint.MaxValue, 0));
        Assert.Equal(new uint[] { 0xaa405060, 0x070f17 }, RustAlrFrameDecoder.PrepareEndpointDefault(
            source, 0x405060, 255, 255, 255, 0x01000000, 0, null));
        Assert.Equal(new uint[] { 0xaa405060, 0 }, RustAlrFrameDecoder.PrepareEndpointDefault(
            source, 0x405060, 256, 0, 0, 0, 0, null));
        foreach (uint? alignment in new uint?[] { null, 0, 2 })
            Assert.Throws<InvalidDataException>(() => RustAlrFrameDecoder.PrepareEndpointDefault(
                source, 0x405060, 255, 255, 255, 0, 1, alignment));
        Assert.Equal(new uint[] { 0xaa405060, 0x102030 }, source);
    }

    [Fact]
    public void Explicit_alpha_lanes_use_raw_masks_and_ignore_previous_alpha()
    {
        uint[] source = [0x00ffffff, 0xaa102030, 3, 0x00000103];
        var result = RustAlrFrameDecoder.PreparePaletteAlphaLanes(source);
        Assert.Equal(new uint[] { 0xffffffff, 0x20102030, 3, 0x01000103 }, result);
        Assert.Equal(new uint[] { 0x00ffffff, 0xaa102030, 3, 0x00000103 }, source);
        Assert.Throws<NotSupportedException>(() => ((IList<uint>)result)[0] = 0);
        var colors = Enumerable.Range(0, 256).Select(i => (uint)(i | i << 8 | i << 16)).ToArray();
        Assert.Equal(colors.Select((color, index) => color | (uint)index << 24), RustAlrFrameDecoder.PreparePaletteAlphaLanes(colors));
    }

    [Fact]
    public void Explicit_primary_default_uses_packed_byte_gains_and_truncates_native_products()
    {
        uint[] source = [0xffffffff, 0xaa102030];
        Assert.Equal(new uint[] { 0x00fefefe, 0x000f1f2f },
            RustAlrFrameDecoder.PreparePrimaryPaletteDefault(source, 0xffffffff));
        var asymmetric = RustAlrFrameDecoder.PreparePrimaryPaletteDefault(source, 0xff8000);
        Assert.Equal(new uint[] { 0x00fe7f00, 0x000f1000 }, asymmetric);
        Assert.Equal(new uint[] { 0xffffffff, 0xaa102030 }, source);
        Assert.Throws<NotSupportedException>(() => ((IList<uint>)asymmetric)[0] = 0);
        Assert.Throws<InvalidDataException>(() => RustAlrFrameDecoder.PreparePrimaryPaletteDefault(Array.Empty<uint>(), 0));
    }

    [Fact]
    public void Prepared_palette_alpha_flows_into_clipped_frame_without_argb_normalization()
    {
        var document = NativeAlrDocument.Parse(NativeAlrDocumentTests.Fixture(6));
        var frame = new NativeParticleAssetFrame(258, "particle.alr", 2, document.AnchorWidth,
            document.AnchorHeight, document.Frames[2], document.DecodeFrame(2))
            { RawPalette = document.ReadRawPalette(), IndexedSamples = document.ReadIndexedSamples(2) };
        var prepared = RustAlrFrameDecoder.PrepareRawPalette(frame.RawPalette, new uint[] { 0, 0x506070 },
            2, new(128, 256, 256, 256, 0, 1));
        Assert.Equal(new uint[] { 0x405060, 0x40304050 }, prepared);
        uint[] canvas = Enumerable.Repeat(0x11223344u, 8).ToArray();
        var result = RustAlrFrameDecoder.BlitSamples(frame, prepared, canvas, 4, 2, 1, 0, 2);
        Assert.Equal(new uint[] { 0x11223344, 0x1c253647, 0x11223344, 0x11223344,
            0x11223344, 0x11223344, 0x11223344, 0x11223344 }, result);
        Assert.Equal(canvas, RustAlrFrameDecoder.BlitSamples(frame, canvas, 4, 2, 1, 0, 2));
        Assert.Equal(new uint[] { 0x405060, 0x102030 }, frame.RawPalette);
        Assert.Equal(Enumerable.Repeat(0x11223344u, 8), canvas);
        Assert.Throws<InvalidDataException>(() => RustAlrFrameDecoder.BlitSamples(frame,
            Array.Empty<uint>(), canvas, 4, 2, int.MinValue, 0, 2));
    }

    [Fact]
    public void Actual_palette_kernel_retains_entry_zero_and_raw_interpolation_then_composes_samples()
    {
        uint[] a = [0xdeadbeef, 0x102030], b = [0, 0x506070];
        var palette = RustAlrFrameDecoder.PrepareRawPalette(a, b, 2, new(128, 256, 256, 256, 0, 0));
        Assert.Equal(new uint[] { 0xdeadbeef, 0x304050 }, palette);
        Assert.Equal(new uint[] { 0xdeadbeef, 0x102030 }, a);
        Assert.Equal(new uint[] { 0, 0x506070 }, b);
        Assert.Throws<NotSupportedException>(() => ((IList<uint>)palette)[1] = 0);
        Assert.Equal(new uint[] { 0xdeadbeef, 0x108060 }, RustAlrFrameDecoder.PrepareRawPalette(
            new uint[] { 0xdeadbeef, 0x4080c0 }, new uint[] { 0, 0 }, 2, new(0, 128, 256, 64, 0, 0)));
        Assert.Equal(new uint[] { 0xdeadbeef }, RustAlrFrameDecoder.PrepareRawPalette(a, b, 1,
            new(int.MinValue, uint.MaxValue, 0, 1, uint.MaxValue, uint.MaxValue)));
        uint[] destination = [0xabcdef01, 0x11223344, 0x44556677];
        Assert.Equal(new uint[] { palette[0], palette[1], destination[2] },
            RustAlrFrameDecoder.ComposeSamples(new ushort[] { 0x300, 0x101, 0 }, palette, destination, 0));
        Assert.Equal(new uint[] { 0xabcdef01, 0x11223344, 0x44556677 }, destination);
    }

    [Fact]
    public void Actual_palette_kernel_rejects_missing_buffers_and_unsupported_count()
    {
        Assert.Throws<InvalidDataException>(() => RustAlrFrameDecoder.PrepareRawPalette(new uint[] { 1 },
            new uint[] { 2 }, 2, new(0, 256, 256, 256, 0, 0)));
        Assert.Throws<ArgumentOutOfRangeException>(() => RustAlrFrameDecoder.PrepareRawPalette(
            Array.Empty<uint>(), Array.Empty<uint>(), 0, new(0, 256, 256, 256, 0, 0)));
    }
}
