using AgainstRomeMapEditor.NativeAssets;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class NativeParticlePalettePreparationTests
{
    [Fact]
    public void Serialized256_color_variants_support_exact_endpoint_preparation()
    {
        using var stream = new MemoryStream();
        using (var writer = new BinaryWriter(stream, System.Text.Encoding.ASCII, leaveOpen: true))
        {
            foreach (uint word in new uint[] { 0x41524c41, 6, 0, 1, 8, 0, 1, 1, 2, 0,
                0, 0, 0, 0, 0, 12, 18, 0, 0, 0, 0, 0, 0 }) writer.Write(word);
            writer.Write(-1); writer.Write(2052u); writer.Write(0u);
            writer.Write(2u | 1u << 11 | 256u << 22); writer.Write(2u);
            for (int variant = 0; variant < 2; variant++)
                for (int color = 0; color < 256; color++)
                    writer.Write(color == 0 ? (variant == 0 ? 0xaa405060u : 0xbb405060u)
                        : (variant == 0 ? 0x102030u : 0x506070u));
            writer.Write(new byte[] { 1, 0, 0, 0 }); writer.Write(0x80000000u); writer.Write(2u);
        }
        var document = NativeAlrDocument.Parse(stream.ToArray());
        var prepared = NativeParticlePalettePreparation.Prepare4Ca340Default(document,
            0, 1, 256, 0xff405060, 255, 255, 255, 0, 256);
        Assert.Equal(256, prepared.Words.Count); Assert.Equal(1, prepared.Selection.FirstVariant);
        Assert.Equal(0xbb405060u, prepared.Words[0]); Assert.Equal(0x4f5f6fu, prepared.Words[255]);
        Assert.Equal(0x506070u, document.ReadRawPalette(1)[255]);
    }

    [Fact]
    public void Alr_document_reads_native_first_frame_palette_variants_without_padding_endpoint()
    {
        byte[] source = NativeAlrDocumentTests.Fixture(6);
        var document = NativeAlrDocument.Parse(source); Array.Fill(source, (byte)0);
        var prepared = NativeParticlePalettePreparation.Prepare4Ca340Default(document,
            0, 1, 128, 0, 256, 256, 256, 0, 2);
        Assert.Equal(new uint[] { 0x405060, 0x506070 }, prepared.Words);
        Assert.Throws<InvalidDataException>(() => NativeParticlePalettePreparation.Prepare4Ca340Default(document,
            0, -1, 0, 0, 255, 255, 255, 0, 2));
    }

    [Fact]
    public void Catalog_prepares_actual_parsed_variants_and_blocks_reader_after_disposal()
    {
        byte[] source = NativeAlrDocumentTests.Fixture(6); int reads = 0;
        using var catalog = NativeSpriteCatalog.FromText("", "258,particle.alr\n", "",
            _ => { reads++; return source; }, _ => null);
        Assert.True(catalog.TryPrepareParticlePalette4Ca340Default(258, 0, 1, 128,
            0, 256, 256, 256, 0, 2, out var prepared, out var reason));
        Assert.Null(reason); Assert.Equal(new uint[] { 0x405060, 0x506070 }, prepared!.Words);
        Assert.True(catalog.TryDecodeParticleFrameIndex(258, 2, 0, out var frame, out reason));
        Assert.Null(reason);
        uint[] canvas = [0x11223344, 0x11223344];
        Assert.Equal(new uint[] { 0x506070, 0x405060 },
            RustAlrFrameDecoder.BlitPreparedSamples(frame!, prepared, canvas, 2, 1, 0, 0, 0));
        using var other = NativeSpriteCatalog.FromText("", "258,particle.alr\n", "", _ => source, _ => null);
        Assert.True(other.TryDecodeParticleFrameIndex(258, 2, 0, out var unrelated, out _));
        Assert.Throws<InvalidDataException>(() => RustAlrFrameDecoder.BlitPreparedSamples(
            unrelated!, prepared, canvas, 2, 1, 0, 0, 0));
        Assert.Throws<InvalidDataException>(() => RustAlrFrameDecoder.BlitPreparedSamples(
            frame!, prepared with { SourceAlrId = 999 }, canvas, 2, 1, 0, 0, 0));
        Assert.False(catalog.TryPrepareParticlePalette4Ca340Default(258, 0, -1, 0,
            0, 255, 255, 255, 0, 2, out var unavailable, out reason));
        Assert.Null(unavailable); Assert.Contains("256", reason);
        catalog.Dispose(); int before = reads;
        Assert.False(catalog.TryPrepareParticlePalette4Ca340Default(258, 0, 1, 128,
            0, 256, 256, 256, 0, 2, out unavailable, out reason));
        Assert.Null(unavailable); Assert.Contains("disposed", reason); Assert.Equal(before, reads);
        Assert.Equal(new uint[] { 0x405060, 0x506070 }, prepared.Words);
        Assert.Equal(new uint[] { 0x506070, 0x405060 },
            RustAlrFrameDecoder.BlitPreparedSamples(frame!, prepared, canvas, 2, 1, 0, 0, 0));
    }

    private static IReadOnlyList<IReadOnlyList<uint>> Variants()
    {
        var a = Enumerable.Repeat(0x102030u, 257).ToArray(); a[0] = 0xaa405060;
        var b = Enumerable.Repeat(0x506070u, 257).ToArray(); b[0] = 0xbb405060;
        return new IReadOnlyList<uint>[] { Array.AsReadOnly(a), Array.AsReadOnly(b) };
    }

    [Fact]
    public void Endpoint_full_weight_chooses_secondary_and_exact_native256_words()
    {
        var variants = Variants();
        var prepared = NativeParticlePalettePreparation.Prepare4Ca340Default(variants,
            0, 1, 999, 0xff405060, 255, 255, 255, 0, 2);
        Assert.True(prepared.Selection.IsEndpoint); Assert.Equal(1, prepared.Selection.FirstVariant);
        Assert.Equal(256, prepared.Words.Count);
        Assert.Equal(0xbb405060u, prepared.Words[0]); Assert.Equal(0x4f5f6fu, prepared.Words[1]);
        Assert.Equal(0x506070u, variants[1][1]);
        Assert.Equal(new uint[] { 0x4f5f6f, 0x11223344, 0xbb405060 },
            RustAlrFrameDecoder.ComposeSamples(new ushort[] { 0x101, 0, 0x300 }, prepared.Words,
                new uint[] { 0x11223344, 0x11223344, 0x11223344 }, 0));
        Assert.Throws<NotSupportedException>(() => ((IList<uint>)prepared.Words)[1] = 0);
    }

    [Fact]
    public void Interpolation_uses_declared_count_and_preserves_first_entry()
    {
        var prepared = NativeParticlePalettePreparation.Prepare4Ca340Default(Variants(),
            0, 1, 128, 0, 256, 256, 256, 0, 2);
        Assert.False(prepared.Selection.IsEndpoint);
        Assert.Equal(new uint[] { 0xaa405060, 0x304050 }, prepared.Words);
        var negative = NativeParticlePalettePreparation.Prepare4Ca340Default(Variants(),
            0, 1, -1, 0, 256, 256, 256, 0, 2);
        Assert.Equal(-1, negative.Selection.Weight);
        Assert.Equal(new uint[] { 0xaa405060, 0x0f1f2f }, negative.Words);
        Assert.Throws<InvalidDataException>(() => NativeParticlePalettePreparation.Prepare4Ca340Default(
            new IReadOnlyList<uint>[] { new uint[] { 1, 2 } }, 0, -1, 0, 0, 255, 255, 255, 0, 2));
    }
}
