using System.Buffers.Binary;
using System.Numerics;
using AgainstRomeMapEditor.NativeAssets;
using Xunit;

namespace AgainstRomeMapEditor.Modules.Tests;

public sealed class NativeAptAttachmentTests
{
    [Theory]
    [InlineData(2u)]
    [InlineData(3u)]
    public void Attachment_groups_select_stage_and_variant_without_using_global_extra_points(uint version)
    {
        NativeAptPoint[][][] lights = [[[new(5, 7)], [new(11, 13), new(17, 19)]], [[new(23, 29)], [new(31, 37)]]];
        byte[] source = Fixture(version, lights, [[new(41, 43)], [new(47, 53)]], [new(-1000000, 0), new(59, 61)]);
        var document = NativeAptDocument.Parse(source);
        Array.Fill(source, (byte)0);
        Assert.Equal(new NativeAptPoint(3, 5), document.ProjectionOrigin);
        Assert.Equal(new NativeAptPoint(23, 29), Assert.Single(document.GetAttachments(1, 0, 0).Lights));
        Assert.Equal(version == 2 ? lights[0][0] : lights[0][1], document.GetAttachments(0, 1, 0).Lights);
        Assert.Equal(new NativeAptPoint(47, 53), Assert.Single(document.GetAttachments(uint.MaxValue, 0, uint.MaxValue).Smoke));
        Assert.Equal(new NativeAptPoint(59, 61), document.GetAttachments(1, 0, 0).Marker);
        Assert.Null(document.GetAttachments(0, 0, 0).Marker);
        Assert.Throws<NotSupportedException>(() => ((IList<NativeAptPoint>)document.GetAttachments(1, 0, 0).Lights)[0] = default);
        Assert.Equal(23, Assert.Single(NativeAptLightPoints.ExtractLightPoints(Fixture(version, lights, [[], []], [default, default]))).ScreenX);
    }

    [Theory]
    [InlineData(0, 0, 0, 0)]
    [InlineData(63, 0, 0, -1)]
    [InlineData(64, 0, 1, -1)]
    [InlineData(-1, 0, -1, 0)]
    [InlineData(0, -1, -1, -1)]
    public void Default_projection_floors_signed_points_relative_to_first_anchor(int x, int y, int gridX, int gridZ)
    {
        var document = NativeAptDocument.Parse(SingleLights((0, 0)));
        Assert.Equal(new NativeAptPoint(gridX, gridZ), NativeAptAttachmentGeometry.Project(document, new(x, y)));
    }

    [Theory]
    [InlineData(0f, 0f)]
    [InlineData(63.9f, 0f)]
    [InlineData(64f, 64f)]
    [InlineData(-.5f, 0f)]
    [InlineData(-1f, -64f)]
    [InlineData(-64.5f, -64f)]
    public void Parent_world_position_uses_native_biased_conversion_then_signed_grid_shift(float world, float expected)
        => Assert.Equal(expected, NativeAptAttachmentGeometry.WorldGridOrigin(world));

    [Fact]
    public void Per_point_quantization_preserves_anchor_phase_and_native_capacity20()
    {
        var points = Enumerable.Range(0, 21).Select(_ => new NativeAptPoint(65, 0)).ToArray();
        var doc = NativeAptDocument.Parse(Fixture(3, [[points]], [[]], [default], new(0, 0), new(63, 0)));
        // Project65=(1,-2), Project63=(0,-1); projecting the delta2 would incorrectly give(0,-1).
        Assert.Equal(new Vector3(96, 80, -32), NativeAptAttachmentGeometry.Place(doc, new(65, 0), 0, 0, 80));
        Assert.Equal(20, NativeAptLightPoints.GetBuildingWorldLightPositions(
            Fixture(3, [[points]], [[]], [default], new(0, 0), new(63, 0)), 0, 0, 0, 80).Count);
    }

    [Theory]
    [InlineData(2u)]
    [InlineData(3u)]
    public void Every_truncated_prefix_and_missing_group_is_rejected(uint version)
    {
        byte[] bytes = Fixture(version, [[[new(5, 7)], [new(11, 13)]]], [[new(17, 19)]], [default]);
        for (int i = 0; i < bytes.Length; i++)
            Assert.Throws<InvalidDataException>(() => NativeAptDocument.Parse(bytes.AsSpan(0, i)));
        BinaryPrimitives.WriteUInt32LittleEndian(bytes.AsSpan(32), 2); // stage axis now references an absent group.
        Assert.Throws<InvalidDataException>(() => NativeAptDocument.Parse(bytes).GetAttachments(1, 0, 0));
    }

    internal static byte[] SingleLights(params (int X, int Y)[] deltas)
        => Fixture(3, [[deltas.Select(p => new NativeAptPoint(p.X + 569, p.Y + 405)).ToArray()]], [[]],
            [new(-1000000, -1000000)], new(0, 0), new(569, 405));

    internal static byte[] Fixture(uint version, NativeAptPoint[][][] lights, NativeAptPoint[][] smoke,
        NativeAptPoint[] markers, NativeAptPoint? origin = null, NativeAptPoint? anchor = null)
    {
        using var stream = new MemoryStream(); using var writer = new BinaryWriter(stream);
        int variants = lights[0].Length;
        uint[] header = new uint[28];
        header[0] = 0x54415041; header[1] = version; header[2] = (uint)(lights.Length * variants); header[3] = 112;
        header[6] = 64; header[7] = 31; header[8] = (uint)lights.Length; header[9] = (uint)variants; header[10] = header[11] = 1;
        header[13] = 8; header[20] = header[21] = 1; header[23] = 64; header[24] = 31;
        foreach (uint word in header) writer.Write(word);
        for (int i = 0; i < 62; i++) writer.Write(0);
        void Point(NativeAptPoint p) { writer.Write(p.X); writer.Write(p.Y); }
        Point(origin ?? new(3, 5)); Point(anchor ?? new(7, 11));
        writer.Write(1); writer.Write(0); writer.Write(lights.Length);
        for (int i = 0; i < lights.Length; i++)
        {
            Point(markers[i]); writer.Write(1); writer.Write(1);
            for (int v = 0; v < (version == 2 ? 1 : variants); v++) writer.Write(lights[i][v].Length);
            writer.Write(smoke[i].Length); writer.Write(1);
        }
        Point(new(999, 997)); // Decoy global extraA must not become a per-frame light.
        for (int i = 0; i < lights.Length; i++)
        {
            writer.Write(new byte[20]); // Other group arrays.
            for (int v = 0; v < (version == 2 ? 1 : variants); v++) foreach (var p in lights[i][v]) Point(p);
            foreach (var p in smoke[i]) Point(p);
            writer.Write(new byte[16]);
        }
        for (int i = 0; i < lights.Length * variants; i++) { writer.Write(0); writer.Write(0); }
        writer.Write(0); writer.Write(0x54414450u);
        return stream.ToArray();
    }
}
