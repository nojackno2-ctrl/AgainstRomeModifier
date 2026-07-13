using System.Numerics;
using AgainstRomeModifier.Maps;

namespace AgainstRomeMapEditor;

internal static class SceneObjectRenderer
{
    public static Vector3[] BuildMarkerPoints(IEnumerable<MapSceneObject> objects, TerrainHeightField heights)
        => objects.Select(item =>
        {
            float x = item.WorldX / (SdlSceneCatalog.WorldUnitsPerMapPixel * 4f);
            float z = item.WorldZ / (SdlSceneCatalog.WorldUnitsPerMapPixel * 4f);
            return new Vector3(x, heights.SampleHeight(x, z) + .25f, z);
        }).ToArray();
}
