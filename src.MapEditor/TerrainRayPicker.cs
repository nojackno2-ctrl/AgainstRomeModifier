using System.Numerics;

namespace AgainstRomeMapEditor;

internal readonly record struct TerrainRay(Vector3 Origin, Vector3 Direction);

internal static class TerrainRayPicker
{
    public static bool TryPick(TerrainHeightField field, TerrainRay ray, out int tileX, out int tileY)
    {
        tileX = tileY = -1;
        if (!TryPickPoint(field, ray, out Vector3 point)) return false;
        tileX = Math.Clamp((int)MathF.Floor(point.X), 0, (int)field.TileWidth - 1);
        tileY = Math.Clamp((int)MathF.Floor(point.Z), 0, (int)field.TileHeight - 1);
        return true;
    }

    /// <summary>Continuous terrain hit (tile units) for relative dragging.</summary>
    public static bool TryPickPoint(TerrainHeightField field, TerrainRay ray, out Vector3 point)
    {
        point = default;
        Vector3 direction = Vector3.Normalize(ray.Direction);
        // March only inside the map's bounding box: an orthographic camera starts rays far outside it.
        if (!ClipToMap(field, ray.Origin, direction, out float start, out float end)) return false;
        float previousDistance = start;
        float previousDelta = Delta(previousDistance);
        for (float distance = start + .25f; distance <= end + .25f; distance += .25f)
        {
            float delta = Delta(distance);
            if (previousDelta >= 0 && delta <= 0)
            {
                float low = previousDistance, high = distance;
                for (int i = 0; i < 12; i++)
                {
                    float middle = (low + high) / 2f;
                    if (Delta(middle) >= 0) low = middle; else high = middle;
                }
                point = ray.Origin + direction * ((low + high) / 2f);
                return point.X >= 0 && point.Z >= 0 && point.X < field.TileWidth && point.Z < field.TileHeight;
            }
            previousDistance = distance; previousDelta = delta;
        }
        return false;

        float Delta(float distance)
        {
            Vector3 point = ray.Origin + direction * distance;
            if (point.X < 0 || point.Z < 0 || point.X > field.TileWidth || point.Z > field.TileHeight) return float.PositiveInfinity;
            return point.Y - field.SampleHeight(point.X, point.Z);
        }
    }

    private static bool ClipToMap(TerrainHeightField field, Vector3 origin, Vector3 direction, out float start, out float end)
    {
        start = 0; end = float.MaxValue;
        // Generous vertical slab: terrain heights stay far inside it at any relief scale.
        (float Min, float Max, float Origin, float Direction)[] slabs =
        [
            (0, field.TileWidth, origin.X, direction.X),
            (-1000, 1000, origin.Y, direction.Y),
            (0, field.TileHeight, origin.Z, direction.Z),
        ];
        foreach ((float min, float max, float o, float d) in slabs)
        {
            if (MathF.Abs(d) < 1e-8f) { if (o < min || o > max) return false; continue; }
            float t0 = (min - o) / d, t1 = (max - o) / d;
            if (t0 > t1) (t0, t1) = (t1, t0);
            start = Math.Max(start, t0); end = Math.Min(end, t1);
        }
        return start <= end;
    }
}
