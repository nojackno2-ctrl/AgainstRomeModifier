using AgainstRomeMapEditor.NativeAssets;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class RustSceneDrawOrderTests
{
    [Fact]
    public void Shared_order_interleaves_heterogeneous_depths_and_keeps_ties_stable()
    {
        // Object depths10,3; particle depths8,3,12 in a shared queue.
        float[] depths = [10, 3, 8, 3, 12];
        var order = RustAlrFrameDecoder.OrderSceneDraws(depths);
        Assert.Equal(new uint[] { 4, 0, 2, 1, 3 }, order);
        Assert.Equal(new float[] { 10, 3, 8, 3, 12 }, depths);
        Assert.Throws<NotSupportedException>(() => ((IList<uint>)order)[0] = 0);
        Assert.Equal(new uint[] { 2, 0, 1, 3 }, RustAlrFrameDecoder.OrderSceneDraws(new float[] { -0f, 0f, 1, -1 }));
        Assert.Empty(RustAlrFrameDecoder.OrderSceneDraws(Array.Empty<float>()));
    }

    [Theory]
    [InlineData(float.NaN)]
    [InlineData(float.PositiveInfinity)]
    [InlineData(float.NegativeInfinity)]
    public void Nonfinite_depth_is_rejected_before_a_partial_order_is_returned(float depth)
        => Assert.Throws<InvalidDataException>(() => RustAlrFrameDecoder.OrderSceneDraws(new float[] { 1, depth }));
}
