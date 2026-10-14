using System.Buffers.Binary;
using AgainstRomeMapEditor.NativeAssets;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class NativeParticleAssetFrameTests
{
    [Fact]
    public void Disposed_particle_queries_do_not_read_assets_or_invoke_resolvers()
    {
        using var catalog = NativeSpriteCatalog.FromText("", "258,p.alr\n", "",
            _ => throw new InvalidOperationException("Reader must not run after disposal."), _ => null);
        catalog.Dispose();
        bool Resolve(int id, int columns, int directions, uint count,
            out IReadOnlyList<NativeParticleFrameCandidate> candidates, out string? reason)
            => throw new InvalidOperationException("Resolver must not run after disposal.");
        Assert.False(catalog.TryResolveParticleSavedFrames(258, Resolve, out var frames, out var diagnostic));
        Assert.Empty(frames); Assert.Contains("disposed", diagnostic);
        Assert.False(catalog.TryDecodeParticleFrameIndex(258, 0, 0, out var image, out diagnostic));
        Assert.Null(image); Assert.Contains("disposed", diagnostic);
    }
    [Fact]
    public void Explicit_particle_frame_decodes_exact_index_and_rejects_invalid_frame_and_palette()
    {
        using var catalog = Catalog(NativeAlrDocumentTests.Fixture(6));
        Assert.True(catalog.TryDecodeParticleFrameIndex(258, 2, 1, out var frame, out var reason));
        Assert.Null(reason); Assert.Equal(2u, frame!.FrameIndex);
        Assert.Equal(1, frame.PaletteVariant);
        Assert.Equal(new ushort[] { 0x301, 0x300 }, frame.IndexedSamples);
        Assert.Throws<NotSupportedException>(() => ((IList<ushort>)frame.IndexedSamples)[0] = 0);
        uint[] destination = [0x12345678, 0xabcdef01];
        uint[] canvas = Enumerable.Repeat(0x11223344u, 6).ToArray();
        Assert.Equal(new uint[] { 0x11223344, 0x11223344, 0x11223344, 0xC0D0E0, 0x11223344, 0x11223344 },
            RustAlrFrameDecoder.BlitSamples(frame, canvas, 3, 2, -1, 1, 0));
        Assert.Equal(canvas, RustAlrFrameDecoder.BlitSamples(frame, canvas, 3, 2, int.MinValue, 0, 1));
        Assert.Equal(0x00718192u, RustAlrFrameDecoder.BlitSamples(frame, canvas, 3, 2, -1, 1, 1)[3]);
        Assert.Equal(canvas, RustAlrFrameDecoder.BlitSamples(frame, canvas, 3, 2, -1, 1, 2));
        Assert.Throws<InvalidDataException>(() => RustAlrFrameDecoder.BlitSamples(
            frame with { IndexedSamples = Array.AsReadOnly(new ushort[] { 0x301, 0x400 }) },
            canvas, 3, 2, int.MinValue, 0, 0));
        Assert.Equal(Enumerable.Repeat(0x11223344u, 6), canvas);
        Assert.Throws<InvalidDataException>(() => RustAlrFrameDecoder.BlitSamples(frame, canvas, 2, 2, 0, 0, 0));
        Assert.Equal(new uint[] { 0x90A0B0, 0xC0D0E0 },
            RustAlrFrameDecoder.ComposeSamples(frame.IndexedSamples, frame.RawPalette, destination, 0));
        Assert.Equal(new uint[] { 0x12345678, 0xabcdef01 }, destination);
        Assert.Throws<InvalidDataException>(() => RustAlrFrameDecoder.ComposeSamples(
            frame.IndexedSamples, frame.RawPalette, destination, 99));
        Assert.Equal(new uint[] { 0xC0D0E0, 0x90A0B0 }, frame.RawPalette);
        Assert.Throws<NotSupportedException>(() => ((IList<uint>)frame.RawPalette)[0] = 0);
        Assert.Equal(new uint[] { 0xFFB0A090, 0xFFE0D0C0 }, frame.Image.ArgbPixels);
        Assert.True(catalog.TryDecodeParticleFrameIndex(258, 2, 1, out var cached, out _));
        Assert.Same(frame, cached);
        Assert.True(catalog.TryDecodeParticleFrameIndex(258, 2, 0, out var other, out _));
        Assert.NotSame(frame, other);
        var cache = (System.Collections.IDictionary)typeof(NativeSpriteCatalog).GetField("_particleAssetFrames",
            System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Instance)!.GetValue(catalog)!;
        Assert.Equal(2, cache.Count);
        Assert.False(catalog.TryDecodeParticleFrameIndex(258, uint.MaxValue, 0, out frame, out reason));
        Assert.Null(frame); Assert.Contains("valid frame", reason);
        Assert.False(catalog.TryDecodeParticleFrameIndex(258, 0, 2, out frame, out reason));
        Assert.Null(frame); Assert.Contains("palette", reason);
        catalog.Dispose(); Assert.Empty(cache.Keys.Cast<object>());
        Assert.Equal(new uint[] { 0xFFB0A090, 0xFFE0D0C0 }, cached!.Image.ArgbPixels);
        Assert.False(catalog.TryDecodeParticleFrameIndex(258, 2, 1, out frame, out reason));
        Assert.Null(frame); Assert.Contains("disposed", reason);
    }
    [Fact]
    public void Installed_particle_asset_layout_reaches_Rust_without_guessing_velocity_frame()
    {
        string? root = Environment.GetEnvironmentVariable("ARM_GAME_PATH");
        if (root is null) { Assert.NotEqual("1", Environment.GetEnvironmentVariable("ARM_REQUIRE_NATIVE_DATA")); return; }
        using var catalog = NativeSpriteCatalog.Open(root);
        Assert.NotNull(catalog);
        using var definitions = RustParticleBaseDefinitions.LoadGameSources(root);
        Assert.True(definitions.TryResolve(830, out var definition, out _));
        bool called = false;
        bool Resolve(int id, int columns, int directions, uint count, out uint? frame, out string? reason)
        {
            called = true;
            Assert.Equal(258, id); Assert.Equal(19, columns); Assert.Equal(32, directions); Assert.Equal(608u, count);
            return definitions.TryResolveLifetimeFrame(830, 14001, id, columns, directions, count, out frame, out reason);
        }
        Assert.False(catalog.TryDecodeParticleLifetimeFrame(definition!.AlrId, 0, Resolve, out var image, out var diagnostic));
        Assert.True(called); Assert.Null(image); Assert.Contains("velocity", diagnostic);
    }

    private static NativeSpriteCatalog Catalog(byte[]? bytes)
        => NativeSpriteCatalog.FromText("", "258,particle.alr\n", "", _ => bytes, _ => null);

    [Fact]
    public void Particle_asset_passes_its_own_layout_and_decodes_exact_frame_palette_and_offsets()
    {
        using var catalog = Catalog(NativeAlrDocumentTests.Fixture(6));
        bool Resolve(int id, int columns, int directions, uint count, out uint? frame, out string? reason)
        {
            Assert.Equal(258, id); Assert.Equal(2, columns); Assert.Equal(2, directions); Assert.Equal(4u, count);
            frame = 2; reason = null; return true;
        }
        Assert.True(catalog.TryDecodeParticleLifetimeFrame(258, 1, Resolve, out var frame, out var reason));
        Assert.Null(reason); Assert.NotNull(frame); Assert.Equal(2u, frame.FrameIndex);
        Assert.Equal(12u, frame.AnchorWidth); Assert.Equal(18u, frame.AnchorHeight);
        Assert.Equal(new uint[] { 0xFFB0A090, 0xFFE0D0C0 }, frame.Image.ArgbPixels);
        Assert.Equal(NativeAlrDocument.Parse(NativeAlrDocumentTests.Fixture(6)).Frames[2], frame.Info);
    }

    [Theory]
    [InlineData(null)]
    [InlineData(99u)]
    public void Resolver_success_without_valid_index_never_decodes_frame_zero(uint? invalid)
    {
        using var catalog = Catalog(NativeAlrDocumentTests.Fixture(6));
        bool Resolve(int id, int columns, int directions, uint count, out uint? frame, out string? reason)
        { frame = invalid; reason = null; return true; }
        Assert.False(catalog.TryDecodeParticleLifetimeFrame(258, 0, Resolve, out var frame, out var reason));
        Assert.Null(frame); Assert.Contains("valid frame", reason);
    }

    [Fact]
    public void Unsupported_asset_layout_palette_and_corrupt_decode_return_no_image()
    {
        bool Never(int id, int columns, int directions, uint count, out uint? frame, out string? reason)
            => throw new InvalidOperationException("Resolver must not run before asset validation.");
        using var missing = Catalog(null);
        Assert.False(missing.TryDecodeParticleLifetimeFrame(258, 0, Never, out _, out _));
        Assert.False(missing.TryDecodeParticleLifetimeFrame(999, 0, Never, out _, out _));
        byte[] bytes = NativeAlrDocumentTests.Fixture(6);
        BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(24), 0);
        using var layout = Catalog(bytes);
        Assert.False(layout.TryDecodeParticleLifetimeFrame(258, 0, Never, out _, out _));
        using var valid = Catalog(NativeAlrDocumentTests.Fixture(6));
        Assert.False(valid.TryDecodeParticleLifetimeFrame(258, -1, Never, out _, out _));
        Assert.False(valid.TryDecodeParticleLifetimeFrame(258, 2, Never, out _, out _));
        bytes = NativeAlrDocumentTests.Fixture(6); bytes[128] = 2;
        using var corrupt = Catalog(bytes);
        bool Resolve(int id, int columns, int directions, uint count, out uint? frame, out string? reason)
        { frame = 0; reason = null; return true; }
        Assert.False(corrupt.TryDecodeParticleLifetimeFrame(258, 0, Resolve, out var image, out var diagnostic));
        Assert.Null(image); Assert.Contains("decode failed", diagnostic);
    }

    [Fact]
    public void Resolver_unavailability_is_preserved_without_decoding()
    {
        using var catalog = Catalog(NativeAlrDocumentTests.Fixture(6));
        bool Resolve(int id, int columns, int directions, uint count, out uint? frame, out string? reason)
        { frame = null; reason = "direction unavailable"; return false; }
        Assert.False(catalog.TryDecodeParticleLifetimeFrame(258, 0, Resolve, out var image, out var diagnostic));
        Assert.Null(image); Assert.Equal("direction unavailable", diagnostic);
    }
}
