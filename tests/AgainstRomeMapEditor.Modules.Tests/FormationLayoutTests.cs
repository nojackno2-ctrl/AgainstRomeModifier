using System.Globalization;
using System.Numerics;
using AgainstRomeMapEditor.Modules.Placement;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class FormationLayoutTests
{
    private static FormationLayoutDefinition Line() => new([new(new(-1, 0), new(1, 0))]);

    [Fact]
    public void Members_sample_interval_midpoints_and_banner_is_separate()
    {
        var offsets = FormationLayout.Create(4, 0, Line());
        Assert.Equal(4, offsets.Count);
        for (int i = 0; i < 4; i++) Near(new(0, 240 - 160 * i), offsets[i]);
        Assert.Equal(Vector2.Zero, FormationLayout.BannerOffset);
        Assert.Empty(FormationLayout.Create(0, 0, Line()));
        Near(Vector2.Zero, FormationLayout.Create(1, 0, Line())[0]);
    }

    [Theory]
    [InlineData(0, 0, -80)]
    [InlineData(90, -80, 0)]
    [InlineData(180, 0, 80)]
    [InlineData(270, 80, 0)]
    [InlineData(-90, 80, 0)]
    [InlineData(450, -80, 0)]
    public void Orientation_reflects_local_X_and_rotates_by_90_minus_angle(float angle, float x, float z)
    {
        var point = new FormationLayoutDefinition([new(new(.25f, 0), new(.25f, 0))]);
        Near(new(x, z), FormationLayout.Create(1, angle, point)[0]);
    }

    [Fact]
    public void Two_local_axes_have_native_handedness_and_spacing_is_a_scale()
    {
        var point = new FormationLayoutDefinition([new(new(.25f, .5f), new(.25f, .5f))]);
        Near(new(-160, -80), FormationLayout.Create(1, 0, point)[0]);
        Near(new(-80, 160), FormationLayout.Create(1, 90, point)[0]);
        Near(new(-320, -160), FormationLayout.Create(1, 0, point, 640)[0]);
    }

    [Fact]
    public void Count_changes_segment_occupancy_instead_of_using_a_sqrt_grid()
    {
        // Synthetic four-line definition, NOT asserted to be original formDef=0.
        var definition = new FormationLayoutDefinition(Enumerable.Range(0, 4)
            .Select(i => new FormationLayoutSegment(new(i, -1), new(i, 1))));
        var offsets = FormationLayout.Create(10, 0, definition, 10);
        Assert.Equal(new[] { 2, 3, 2, 3 }, Enumerable.Range(0, 4)
            .Select(column => offsets.Count(p => Math.Abs(p.Y + column * 10) < .001f)));
        Near(new(6, 0), offsets[0]);
        Near(new(-2, 0), offsets[1]);
        Near(new(10, -10), offsets[2]); // Exact segment boundary, fraction zero.
        Near(new(-6, -30), offsets[9]);
        Assert.Equal(20, FormationLayout.Create(20, 22.5f, definition).Count);
    }

    [Fact]
    public void Definition_and_output_are_owned_readonly_snapshots()
    {
        FormationLayoutSegment[] source = [new(new(1, 0), new(1, 0))];
        var definition = new FormationLayoutDefinition(source); source[0] = default;
        Near(new(0, -320), FormationLayout.Create(1, 0, definition)[0]);
        Assert.Throws<NotSupportedException>(() => ((IList<FormationLayoutSegment>)definition.Segments)[0] = default);
        var result = FormationLayout.Create(1, 0, definition);
        Assert.Throws<NotSupportedException>(() => ((IList<Vector2>)result)[0] = default);
    }

    [Fact]
    public void Geometry_fields_parse_invariantly_and_select_requested_definition()
    {
        string[] values = Enumerable.Repeat("0", 83).ToArray();
        values[0] = "0"; values[1] = "1"; values[2] = "2";
        values[3] = ".25"; values[4] = ".5"; values[5] = ".25"; values[6] = ".5";
        values[7] = "1"; values[8] = "2"; values[9] = "1"; values[10] = "2";
        string text = "[Other]\n" + string.Join(',', values) + "\n[FormationDefault]\n" + string.Join(',', values) + ";test\n";
        var before = CultureInfo.CurrentCulture;
        try {
            CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo("de-DE");
            var definition = FormationLayoutDefinition.FromFormDefText(text);
            Near(new(-160, -80), FormationLayout.Create(2, 0, definition)[0]);
            Near(new(-640, -320), FormationLayout.Create(2, 0, definition)[1]);
        } finally { CultureInfo.CurrentCulture = before; }
        Assert.Throws<InvalidDataException>(() => FormationLayoutDefinition.FromFormDefText(text, 1));
        Assert.Throws<InvalidDataException>(() => FormationLayoutDefinition.FromFormDefText(text + string.Join(',', values)));
        Assert.Throws<InvalidDataException>(() => FormationLayoutDefinition.FromFormDefText("[FormationDefault]\n0,1,2"));
        values[3] = "NaN";
        Assert.Throws<InvalidDataException>(() => FormationLayoutDefinition.FromFormDefText("[FormationDefault]\n" + string.Join(',', values)));
    }

    [Theory]
    [InlineData(-1, 0, 320)]
    [InlineData(21, 0, 320)]
    [InlineData(1, float.NaN, 320)]
    [InlineData(1, float.PositiveInfinity, 320)]
    [InlineData(1, 0, 0)]
    [InlineData(1, 0, -1)]
    [InlineData(1, 0, float.NaN)]
    [InlineData(1, 0, float.PositiveInfinity)]
    public void Invalid_inputs_fail_explicitly(int count, float angle, float spacing)
        => Assert.Throws<ArgumentOutOfRangeException>(() => FormationLayout.Create(count, angle, Line(), spacing));

    [Fact]
    public void Invalid_definitions_are_rejected()
    {
        Assert.Throws<ArgumentNullException>(() => FormationLayout.Create(1, 0, null!));
        Assert.Throws<ArgumentException>(() => new FormationLayoutDefinition([]));
        Assert.Throws<ArgumentException>(() => new FormationLayoutDefinition(Enumerable.Repeat(default(FormationLayoutSegment), 21)));
        Assert.Throws<ArgumentException>(() => new FormationLayoutDefinition([new(new(float.NaN, 0), default)]));
    }

    private static void Near(Vector2 expected, Vector2 actual)
    {
        Assert.InRange(Math.Abs(expected.X - actual.X), 0, .001f);
        Assert.InRange(Math.Abs(expected.Y - actual.Y), 0, .001f);
    }
}
