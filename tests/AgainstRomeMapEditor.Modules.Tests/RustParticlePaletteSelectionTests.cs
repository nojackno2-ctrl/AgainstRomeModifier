using AgainstRomeMapEditor.NativeAssets;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class RustParticlePaletteSelectionTests
{
    [Theory]
    [InlineData(0, 1, 128, false, 0, 1, 128)]
    [InlineData(0, 1, 0, true, 0, -1, 0)]
    [InlineData(0, 1, 256, true, 1, -1, 256)]
    [InlineData(0, 1, 999, true, 1, -1, 256)]
    [InlineData(0, -1, 256, true, 0, -1, 256)]
    [InlineData(0, 1, -1, false, 0, 1, -1)]
    [InlineData(8, 9, -5, true, 1, -1, -5)]
    [InlineData(0, 0, 100, true, 0, -1, 100)]
    public void Actual_selector_preserves_native_endpoint_and_signed_weight_rules(int first, int second,
        int weight, bool endpoint, int a, int b, int w)
        => Assert.Equal(new NativePaletteSelection(endpoint, a, b, w), RustAlrFrameDecoder.SelectPalette(2, first, second, weight));

    [Fact]
    public void Actual_selector_rejects_unknown_variant_context()
    {
        foreach (var args in new[] { (0, 0, 0), (2, -1, 0), (2, 0, -2) })
            Assert.Throws<InvalidDataException>(() => RustAlrFrameDecoder.SelectPalette(args.Item1, args.Item2, args.Item3, 128));
    }
}
