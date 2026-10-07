using System.Globalization;
using System.Numerics;
using AgainstRomeMapEditor.Modules.Placement;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class FormationLayoutTests
{
    // 真實 id0 的有效幾何摘要；保留原表格 20 組結構，省略非幾何欄位。
    private const string NativeGeometry = "0,1,9,.05,-.05,.20,.05,-.20,.15,-.05,.05,.20,-.20,.05,-.15,-.10,-.05,-.20,-.20,.30,-.15,.40,.10,-.25,.25,-.40,.05,.05,.30,.30,.20,.34,-.25,.10,-.34,-.20,-.34,-.40,-.15";
    private static FormationLayoutDefinition ParsedNative() => FormationLayoutDefinition.FromFormDefText(
        "[FormationDefault]\r\n;idx,activ,keys,...\r\n " + NativeGeometry + string.Concat(Enumerable.Repeat(",0", 44)) + ",0,180,All_Haufen;geometry excerpt\r\n");

    [Fact]
    public void Real_default_structural_excerpt_matches_embedded_geometry()
        => Assert.Equal(FormationLayoutDefinition.NativeDefault.Segments, ParsedNative().Segments);

    public static IEnumerable<object[]> NativeCounts => Enumerable.Range(1, 20).Select(n => new object[] { n });

    [Theory]
    [MemberData(nameof(NativeCounts))]
    public void Native_counts_sample_the_correct_segment_stay_in_bounds_and_rotate_rigidly(int count)
    {
        var definition = ParsedNative();
        var zero = FormationLayout.Create(count, 0, definition);
        Assert.Equal(count, zero.Count);
        Assert.Equal(count, zero.Distinct().Count());
        for (int i = 0; i < count; i++)
        {
            double position = (i + .5) * 9 / count;
            int j = (int)Math.Floor(position);
            float t = (float)(position - j);
            var segment = definition.Segments[j];
            var point = Vector2.Lerp(segment.Start, segment.End, t);
            Near(new(-point.Y * 320, -point.X * 320), zero[i]);
            Assert.InRange(zero[i].X, -96, 108.81f);
            Assert.InRange(zero[i].Y, -128, 128);
        }
        foreach (float angle in new[] { 22.5f, 45, 90, 180, 270, -45, 405 })
        {
            var rotated = FormationLayout.Create(count, angle, definition);
            double theta = -angle * Math.PI / 180;
            for (int i = 0; i < count; i++)
            {
                Near(new((float)(zero[i].X * Math.Cos(theta) - zero[i].Y * Math.Sin(theta)),
                    (float)(zero[i].X * Math.Sin(theta) + zero[i].Y * Math.Cos(theta))), rotated[i]);
                Assert.InRange(Math.Abs(zero[i].Length() - rotated[i].Length()), 0, .001f);
            }
        }
    }

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
