using AgainstRomeMapEditor.NativeAssets;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class RustSavedObjectModeTests
{
    [Fact]
    public void Real_mode_bridge_preserves_context_unknowns_inputs_and_catalog_lifetime()
    {
        string? root = Environment.GetEnvironmentVariable("ARM_GAME_PATH");
        if (root is null) { Assert.NotEqual("1", Environment.GetEnvironmentVariable("ARM_REQUIRE_NATIVE_DATA")); return; }
        using var definitions = RustParticleBaseDefinitions.LoadGameSources(root);
        Assert.True(definitions.TryResolvePrimaryMode(830, out var primary, out var primaryReason));
        Assert.Null(primaryReason); Assert.Equal(3, primary);
        byte[] record = new byte[79], gfx = new byte[25];
        record[0] = 1; record[3] = 42; gfx[0] = 1; gfx[4] = 1; gfx[8] = 1; gfx[23] = 99;
        byte[] originalRecord = (byte[])record.Clone(), originalGfx = (byte[])gfx.Clone();
        Assert.True(definitions.TryResolveSavedObjectMode(record, gfx, 42, 3, out var mode, out var reason));
        Assert.Null(reason); Assert.Equal(new NativeSavedObjectMode(42, 0, 99, 3, 0x4caf80), mode);
        Assert.False(definitions.TryResolveSavedObjectMode(record, gfx, 43, 3, out mode, out reason));
        Assert.Null(mode); Assert.Contains("UID", reason);
        Assert.False(definitions.TryResolveSavedObjectMode(record, gfx, 42, null, out mode, out reason));
        Assert.Null(mode); Assert.Contains("context", reason);
        Assert.True(definitions.TryResolveSavedObjectMode(record, gfx, 42, 99, out mode, out reason));
        Assert.Null(reason); Assert.Equal(99, mode!.ResolvedMode); Assert.Null(mode.LeafAddress);
        Assert.Equal(originalRecord, record); Assert.Equal(originalGfx, gfx);
        Assert.False(definitions.TryResolveSavedObjectMode(record, gfx, 42, 32768, out mode, out reason));
        Assert.Null(mode); Assert.Contains("signed16", reason);
        gfx[8] = 0;
        Assert.False(definitions.TryResolveSavedObjectMode(record, gfx, 42, 3, out mode, out reason));
        Assert.Null(mode); Assert.Contains("unresolved", reason); gfx[8] = 1;
        definitions.Dispose();
        Assert.Throws<ObjectDisposedException>(() => definitions.TryResolveSavedObjectMode(record, gfx, 42, 3, out _, out _));
        Assert.Throws<ObjectDisposedException>(() => definitions.TryResolvePrimaryMode(830, out _, out _));
    }
}
