using AgainstRomeMapEditor.NativeAssets;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class RustParticleSpriteGeometryTests
{
    [Fact]
    public void Actual_geometry_bridge_preserves_offsets_rejects_metadata_and_guards_disposal()
    {
        string root = Path.Combine(Path.GetTempPath(), "ArmGeometry_" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(root);
        Directory.CreateDirectory(Path.Combine(root, "SYSTEM"));
        File.WriteAllText(Path.Combine(root, "SYSTEM", "clengine.ini"), "[DefaultPath]\nSelected/\n");
        Directory.CreateDirectory(Path.Combine(root, "Selected"));
        File.WriteAllText(Path.Combine(root, "Selected", "partpar.dau"), "[ParticleParameterDefault]\n22,1,Arrow,64,15000\n");
        try
        {
            using var definitions = RustParticleBaseDefinitions.LoadGameSources(root);
            var document = NativeAlrDocument.Parse(NativeAlrDocumentTests.Fixture(6));
            var frame = new NativeParticleAssetFrame(258, "particle.alr", 0, document.AnchorWidth,
                document.AnchorHeight, document.Frames[0], document.DecodeFrame(0, 0));
            Assert.True(definitions.TryResolveSpriteRect(frame, out var rect, out var reason));
            Assert.Null(reason); Assert.Equal(new NativeParticleSpriteRect(1, 2, 6, 3), rect);
            Assert.False(definitions.TryResolveSpriteRect(frame with { Info = frame.Info with { Width = 4 } }, out rect, out reason));
            Assert.Null(rect); Assert.Contains("dimensions", reason);
            Assert.False(definitions.TryResolveSpriteRect(frame with { Info = frame.Info with { OffsetX = int.MinValue } }, out rect, out reason));
            Assert.Null(rect); Assert.Contains("bounds", reason);
            definitions.Dispose();
            Assert.Throws<ObjectDisposedException>(() => definitions.TryResolveSpriteRect(frame, out _, out _));
            Assert.Equal(5, frame.Image.Width);
        }
        finally
        {
            File.Delete(Path.Combine(root, "SYSTEM", "clengine.ini"));
            Directory.Delete(Path.Combine(root, "SYSTEM"));
            File.Delete(Path.Combine(root, "Selected", "partpar.dau"));
            Directory.Delete(Path.Combine(root, "Selected"));
            Directory.Delete(root);
        }
    }
}
