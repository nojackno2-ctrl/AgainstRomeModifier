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
    /// Index of the object visible under a screen point, or -1. Sprites hit on opaque pixels of
    /// their projected quad, and the nearest hit wins because sprites are painted far to near.
    /// Objects without a sprite hit within <paramref name="markerRadius"/> pixels of their marker.
    /// </summary>
    public static int PickObject(IReadOnlyList<MapSceneObject> objects, IReadOnlyList<NativeSprite?> sprites, Func<NativeSprite, bool> drawn,
        TerrainHeightField heights, Matrix4x4 view, Matrix4x4 projection, Vector2 viewport, Vector2 point, float markerRadius = 8,
        Func<MapSceneObject, bool>? markerVisible = null)
    {
        int best = -1;
        float bestDepth = float.MaxValue;
        for (int index = 0; index < objects.Count; index++)
        {
            NativeSprite? sprite = index < sprites.Count ? sprites[index] : null;
            bool hasSprite = sprite is not null && drawn(sprite);
            if (!hasSprite && markerVisible?.Invoke(objects[index]) == false) continue;
            Vector3 anchor = GroundPoint(objects[index], heights, hasSprite ? 0 : .25f);
            Vector4 eye = Vector4.Transform(new Vector4(anchor, 1), view);
            if (eye.Z >= 0) continue; // behind the camera (right-handed view looks down -Z)
            bool hit;
            if (hasSprite)
            {
                Vector2 bottomLeft = Project(eye + new Vector4(-sprite!.AnchorX * TilesPerSpritePixel, (sprite.AnchorY - sprite.Height) * TilesPerSpritePixel, 0, 0), projection, viewport);
                Vector2 topRight = Project(eye + new Vector4((sprite.Width - sprite.AnchorX) * TilesPerSpritePixel, sprite.AnchorY * TilesPerSpritePixel, 0, 0), projection, viewport);
                if (point.X < bottomLeft.X || point.X >= topRight.X || point.Y < topRight.Y || point.Y >= bottomLeft.Y) continue;
                int px = Math.Clamp((int)((point.X - bottomLeft.X) / (topRight.X - bottomLeft.X) * sprite.Width), 0, sprite.Width - 1);
                int py = Math.Clamp((int)((point.Y - topRight.Y) / (bottomLeft.Y - topRight.Y) * sprite.Height), 0, sprite.Height - 1);
                hit = sprite.ArgbPixels[py * sprite.Width + px] >> 24 != 0;
            }
            else hit = Vector2.Distance(Project(eye, projection, viewport), point) <= markerRadius;
            float distance = new Vector3(eye.X, eye.Y, eye.Z).Length(); // same order as BuildSpriteVertices
            if (hit && distance < bestDepth) { bestDepth = distance; best = index; }
        }
        return best;
    }

    private static Vector2 Project(Vector4 eye, Matrix4x4 projection, Vector2 viewport)
    {
        Vector4 clip = Vector4.Transform(eye, projection);
        return new Vector2((clip.X / clip.W * .5f + .5f) * viewport.X, (.5f - clip.Y / clip.W * .5f) * viewport.Y);
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
