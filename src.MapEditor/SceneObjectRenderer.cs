using System.Numerics;
using AgainstRomeMapEditor.NativeAssets;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor;

internal static class SceneObjectRenderer
{
    /// <summary>
    /// Sprite pixels per tile unit. An APT diamond is 64 px wide and spans the diagonal of one
    /// map pixel (a quarter tile), matching the game's 2:1 isometric projection. Estimated from
    /// asset geometry; not yet measured against an in-game screenshot.
    /// </summary>
    public const float TilesPerSpritePixel = .25f * 1.41421356f / 64f;
    public const int FloatsPerSpriteVertex = 7;

    public static Vector3[] BuildMarkerPoints(IEnumerable<MapSceneObject> objects, TerrainHeightField heights)
        => objects.Select(item => GroundPoint(item, heights, .25f)).ToArray();

    public static Vector3 GroundPoint(MapSceneObject item, TerrainHeightField heights, float lift = 0)
    {
        float x = item.WorldX / (SdlSceneCatalog.WorldUnitsPerMapPixel * 4f);
        float z = item.WorldZ / (SdlSceneCatalog.WorldUnitsPerMapPixel * 4f);
        return new Vector3(x, heights.SampleHeight(x, z) + lift, z);
    }

    /// <summary>
    /// Screen-aligned sprite quads (anchor xyz, view-space offset xy, uv) ordered far to near,
    /// as the original 2.5D renderer paints. Objects without an atlas entry are skipped.
    /// </summary>
    public static float[] BuildSpriteVertices(IReadOnlyList<MapSceneObject> objects, IReadOnlyList<NativeSprite?> sprites,
        NativeSpriteAtlas atlas, TerrainHeightField heights, Vector3 cameraPosition)
    {
        var visible = new List<(float Distance, Vector3 Anchor, NativeSprite Sprite, NativeSpriteUv Uv)>();
        for (int index = 0; index < objects.Count && index < sprites.Count; index++)
        {
            if (sprites[index] is not { } sprite || !atlas.TryGetUv(sprite, out NativeSpriteUv uv)) continue;
            Vector3 anchor = GroundPoint(objects[index], heights);
            visible.Add((Vector3.DistanceSquared(anchor, cameraPosition), anchor, sprite, uv));
        }
        visible.Sort((a, b) => b.Distance.CompareTo(a.Distance));
        var vertices = new float[visible.Count * 6 * FloatsPerSpriteVertex];
        int offset = 0;
        foreach ((_, Vector3 anchor, NativeSprite sprite, NativeSpriteUv uv) in visible)
        {
            float left = -sprite.AnchorX * TilesPerSpritePixel, right = (sprite.Width - sprite.AnchorX) * TilesPerSpritePixel;
            float top = sprite.AnchorY * TilesPerSpritePixel, bottom = (sprite.AnchorY - sprite.Height) * TilesPerSpritePixel;
            void Vertex(float ox, float oy, float u, float v)
            {
                vertices[offset++] = anchor.X; vertices[offset++] = anchor.Y; vertices[offset++] = anchor.Z;
                vertices[offset++] = ox; vertices[offset++] = oy; vertices[offset++] = u; vertices[offset++] = v;
            }
            Vertex(left, bottom, uv.U0, uv.V1); Vertex(right, bottom, uv.U1, uv.V1); Vertex(right, top, uv.U1, uv.V0);
            Vertex(left, bottom, uv.U0, uv.V1); Vertex(right, top, uv.U1, uv.V0); Vertex(left, top, uv.U0, uv.V0);
        }
        return vertices;
    }
}
