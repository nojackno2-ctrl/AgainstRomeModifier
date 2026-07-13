using System.Numerics;

namespace AgainstRomeMapEditor;

internal readonly record struct TerrainRay(Vector3 Origin, Vector3 Direction);

internal static class TerrainRayPicker
{
    public static bool TryPick(TerrainHeightField field, TerrainRay ray, out int tileX, out int tileY)
    {
        tileX = tileY = -1;
        Vector3 direction = Vector3.Normalize(ray.Direction);
        float previousDistance = 0;
        float previousDelta = Delta(previousDistance);
        for (float distance = .25f; distance <= 400f; distance += .25f)
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
                Vector3 point = ray.Origin + direction * ((low + high) / 2f);
                if (point.X < 0 || point.Z < 0 || point.X >= field.TileWidth || point.Z >= field.TileHeight) return false;
                tileX = Math.Clamp((int)MathF.Floor(point.X), 0, (int)field.TileWidth - 1);
                tileY = Math.Clamp((int)MathF.Floor(point.Z), 0, (int)field.TileHeight - 1);
                return true;
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
}
