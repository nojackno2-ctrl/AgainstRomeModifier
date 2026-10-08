using AgainstRomeMapEditor.Modules.Fortification;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class PalisadeRunPlannerTests
{
    private const string Z = "AlongZ", X = "AlongX", C = "Corner";

    [Fact]
    public void Straight_run_uses_64_unit_spacing_and_axis_variant()
    {
        var pieces = PalisadeRunPlanner.Plan([(0, 0), (256, 0)], Z, X, C);
        Assert.Equal(5, pieces.Count); // 0,64,128,192,256
        Assert.All(pieces, piece => Assert.Equal(X, piece.Name));
        Assert.Equal([0f, 64f, 128f, 192f, 256f], pieces.Select(piece => piece.X).ToArray());
        var vertical = PalisadeRunPlanner.Plan([(0, 0), (0, 128)], Z, X, C);
        Assert.All(vertical, piece => Assert.Equal(Z, piece.Name));
    }

    [Fact]
    public void L_shaped_path_puts_one_corner_at_the_bend_and_matches_original_spacing()
    {
        var pieces = PalisadeRunPlanner.Plan([(0, 0), (128, 0), (128, 128)], Z, X, C);
        Assert.Single(pieces, piece => piece.Name == C);
        var corner = Assert.Single(pieces, piece => piece.Name == C);
        Assert.Equal((128f, 0f), (corner.X, corner.Z));
        Assert.Equal(5, pieces.Count); // 3 along X incl. corner + 2 more along Z
        Assert.Equal(pieces.Count, pieces.Select(piece => (piece.X, piece.Z)).Distinct().Count());
    }

    [Fact]
    public void Diagonal_segment_is_split_into_two_orthogonal_runs_and_degenerate_input_is_empty()
    {
        var pieces = PalisadeRunPlanner.Plan([(0, 0), (128, 128)], Z, X, C);
        Assert.Contains(pieces, piece => piece.Name == C);
        Assert.Empty(PalisadeRunPlanner.Plan([(10, 10)], Z, X, C));
    }
}
