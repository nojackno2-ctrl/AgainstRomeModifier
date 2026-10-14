using System.Buffers.Binary;
using AgainstRomeMapEditor.NativeAssets;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class RustParticleSavedFrameTests
{
    [Fact]
    public void Installed_catalog_batch_uses_real_ABI_and_returns_coordinates_with_unavailable_frames()
    {
        string? root = Environment.GetEnvironmentVariable("ARM_GAME_PATH");
        if (root is null) { Assert.NotEqual("1", Environment.GetEnvironmentVariable("ARM_REQUIRE_NATIVE_DATA")); return; }
        using var definitions = RustParticleBaseDefinitions.LoadGameSources(root);
        byte[] record = new byte[2252], state = Enumerable.Repeat((byte)0xab, 16).ToArray();
        record[0] = 1; BinaryPrimitives.WriteInt32LittleEndian(record.AsSpan(2), 3);
        foreach (int ordinal in new[] { 0, 2, 3 })
        {
            int child = 12 + ordinal * 35; record[child] = 1;
            BinaryPrimitives.WriteInt32LittleEndian(record.AsSpan(child + 1), -1);
            BinaryPrimitives.WriteInt32LittleEndian(record.AsSpan(child + 5), 65537);
            BinaryPrimitives.WriteInt32LittleEndian(record.AsSpan(child + 9), int.MinValue);
            BinaryPrimitives.WriteInt32LittleEndian(record.AsSpan(child + 13), -65537);
            BinaryPrimitives.WriteInt32LittleEndian(record.AsSpan(child + 25), 14001);
        }
        byte[] before = (byte[])record.Clone();
        Assert.True(definitions.TryResolveSavedFrames(830, 7, record, state, 258, 19, 32, 608,
            out var children, out var diagnostic));
        Assert.Null(diagnostic); Assert.Equal(new uint[] { 0, 2 }, children.Select(c => c.Ordinal));
        Assert.All(children, child =>
        {
            Assert.Equal((-1, 1, -32768), (child.X, child.Y, child.Z));
            Assert.Equal((-2, 0, 0), (child.VelocityX, child.VelocityY, child.VelocityZ));
            Assert.Equal(14001, child.LifetimeRaw); Assert.Null(child.Frame); Assert.Equal(241, child.Status);
        });
        Assert.Equal(before, record); Assert.All(state, b => Assert.Equal(0xab, b));
        Assert.True(definitions.TryResolveSavedFrames(830, 7, record, state, 258, 19, 32, 608,
            out var translated, out diagnostic, 65537, -65537, 0));
        Assert.Null(diagnostic);
        Assert.All(translated, child =>
        {
            Assert.Equal((1, 0, -32768), (child.X, child.Y, child.Z));
            Assert.Null(child.Frame); Assert.Equal(241, child.Status);
            Assert.Equal(14001, child.LifetimeRaw);
        });
        Assert.Equal(before, record);
        BinaryPrimitives.WriteInt32LittleEndian(record.AsSpan(2), 65);
        Assert.False(definitions.TryResolveSavedFrames(830, 7, record, state, 258, 19, 32, 608,
            out children, out diagnostic));
        Assert.Empty(children); Assert.Contains("114", diagnostic);
        definitions.Dispose();
        Assert.Throws<ObjectDisposedException>(() => definitions.TryResolveSavedFrames(830, 7, record, state,
            258, 19, 32, 608, out _, out _));
    }
}
