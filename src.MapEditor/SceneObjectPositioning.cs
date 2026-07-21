using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor;

internal static class SceneObjectPositioning
{
    private const float MapWorldSize = SdlSceneCatalog.WorldUnitsPerMapPixel * SdlSceneCatalog.MapPixelSize;

    public static MapSceneObject MoveToWorldPosition(MapSceneObject source, float worldX, float worldZ)
    {
        if (!float.IsFinite(worldX) || !float.IsFinite(worldZ))
            throw new ArgumentOutOfRangeException(nameof(worldX), "Scene-object coordinates must be finite.");

        worldX = Math.Clamp(worldX, 0, MapWorldSize);
        worldZ = Math.Clamp(worldZ, 0, MapWorldSize);
        return source with
        {
            WorldX = worldX,
            WorldZ = worldZ,
            LocalX = source.LocalX + worldX - source.WorldX,
            LocalZ = source.LocalZ + worldZ - source.WorldZ,
        };
    }
}
