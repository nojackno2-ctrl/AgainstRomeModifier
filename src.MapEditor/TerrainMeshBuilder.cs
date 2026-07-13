using System.Numerics;

namespace AgainstRomeMapEditor;

internal readonly record struct TerrainVertex(Vector3 Position, Vector3 Normal, Vector2 TexCoord);
internal sealed record TerrainMeshData(TerrainVertex[] Vertices, uint[] Indices);

internal static class TerrainMeshBuilder
{
    // Tiles intentionally own their four vertices: a shared 257x257 grid cannot address a distinct atlas UV per tile.
    public static TerrainMeshData Build(TerrainHeightField field, int dimension, Func<int, int, Vector2[]> uvForTile)
    {
        if (dimension <= 0) throw new ArgumentOutOfRangeException(nameof(dimension));
        var vertices = new TerrainVertex[dimension * dimension * 4];
        var indices = new uint[dimension * dimension * 6];
        int vertex = 0, index = 0;
        for (int y = 0; y < dimension; y++)
        for (int x = 0; x < dimension; x++)
        {
            Vector2[] uv = uvForTile(x, y);
            if (uv.Length != 4) throw new ArgumentException("Each tile needs four UV coordinates.", nameof(uvForTile));
            int start = vertex;
            AddVertex(x, y, uv[0]); AddVertex(x + 1, y, uv[1]); AddVertex(x + 1, y + 1, uv[2]); AddVertex(x, y + 1, uv[3]);
            // Counter-clockwise when viewed from above (+Y), matching the generated normals and OpenGL front-face culling.
            indices[index++] = (uint)start; indices[index++] = (uint)(start + 2); indices[index++] = (uint)(start + 1);
            indices[index++] = (uint)start; indices[index++] = (uint)(start + 3); indices[index++] = (uint)(start + 2);
        }
        return new TerrainMeshData(vertices, indices);

        void AddVertex(float x, float y, Vector2 uv)
        {
            const float delta = .5f;
            float height = field.SampleHeight(x, y);
            float dx = field.SampleHeight(x + delta, y) - field.SampleHeight(x - delta, y);
            float dz = field.SampleHeight(x, y + delta) - field.SampleHeight(x, y - delta);
            Vector3 normal = Vector3.Normalize(new Vector3(-dx, 2 * delta, -dz));
            vertices[vertex++] = new TerrainVertex(new Vector3(x, height, y), normal, uv);
        }
    }
}
