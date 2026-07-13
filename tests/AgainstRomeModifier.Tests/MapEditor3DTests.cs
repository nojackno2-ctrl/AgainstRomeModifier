using System.Numerics;
using AgainstRomeMapEditor;

namespace AgainstRomeModifier.Tests;

public sealed class MapEditor3DTests
{
    [Fact]
    public void HeightField_Interpolates_and_clamps_samples()
    {
        var field = new TerrainHeightField(3, 3, new byte[] { 0, 100, 200, 30, 130, 230, 60, 160, 255 }, 10);

        Assert.Equal(130f / 255f * 10f, field.SampleHeight(1, 1), 4);
        Assert.Equal(65f / 255f * 10f, field.SampleHeight(.5f, .5f), 4);
        Assert.Equal(0, field.SampleHeight(-10, -3), 4);
        Assert.Equal(10, field.SampleHeight(9, 9), 4);
    }

    [Fact]
    public void MeshBuilder_uses_four_vertices_per_tile_for_independent_atlas_uvs()
    {
        var field = new TerrainHeightField(3, 3, new byte[9], 6);
        TerrainMeshData mesh = TerrainMeshBuilder.Build(field, 2, (_, _) => new[] { Vector2.Zero, Vector2.UnitX, Vector2.One, Vector2.UnitY });

        Assert.Equal(16, mesh.Vertices.Length);
        Assert.Equal(24, mesh.Indices.Length);
        Assert.All(mesh.Vertices, vertex => Assert.Equal(Vector3.UnitY, vertex.Normal));
        Assert.Equal(new uint[] { 0, 1, 2, 0, 2, 3 }, mesh.Indices.Take(6));
    }

    [Fact]
    public void RayPicker_returns_nearest_tile_and_rejects_outside_ray()
    {
        var field = new TerrainHeightField(5, 5, new byte[25]);

        Assert.True(TerrainRayPicker.TryPick(field, new TerrainRay(new Vector3(1.8f, 4, 2.2f), -Vector3.UnitY), out int x, out int y));
        Assert.Equal(1, x); Assert.Equal(2, y);
        Assert.False(TerrainRayPicker.TryPick(field, new TerrainRay(new Vector3(-2, 4, 2), -Vector3.UnitY), out _, out _));
    }

    [Fact]
    public void EditorCamera_clamps_pitch_and_distance()
    {
        var camera = new EditorCamera();
        camera.Rotate(0, 500); camera.Zoom(.001f);
        Assert.Equal(80, camera.PitchDegrees); Assert.Equal(camera.MinDistance, camera.Distance);
        camera.Rotate(0, -1000); camera.Zoom(1000);
        Assert.Equal(20, camera.PitchDegrees); Assert.Equal(camera.MaxDistance, camera.Distance);
        Assert.NotEqual(camera.Position, camera.Target);
    }

    [Fact]
    public void AtlasLayout_is_non_overlapping_and_stays_inside_bounds()
    {
        IReadOnlyDictionary<string, AtlasRect> layout = FloorTextureAtlas.Layout(new[] { "a", "b", "a", "c" }, 2048);

        Assert.Equal(3, layout.Count);
        Assert.All(layout.Values, rect => Assert.InRange(rect.X + rect.Width, 1, 2048));
        Assert.DoesNotContain(layout.Values, first => layout.Values.Any(second => first != second && first.X < second.X + second.Width && second.X < first.X + first.Width && first.Y < second.Y + second.Height && second.Y < first.Y + first.Height));
    }
}
