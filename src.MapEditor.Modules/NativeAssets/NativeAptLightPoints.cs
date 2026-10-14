using System.Numerics;

namespace AgainstRomeMapEditor.NativeAssets;

/// <summary>Light points for the same completed, axis1/2-zero static APT state as the editor sprite.</summary>
internal static class NativeAptLightPoints
{
    public sealed record AptLightPoint(int ScreenX, int ScreenY, int DeltaAnchorX, int DeltaAnchorY);

    public static IReadOnlyList<AptLightPoint> ExtractLightPoints(ReadOnlySpan<byte> aptBytes)
    {
        var document = NativeAptDocument.Parse(aptBytes);
        return CompletedLights(document).Select(point => new AptLightPoint(point.X, point.Y,
            unchecked(point.X - document.AnchorX), unchecked(point.Y - document.AnchorY))).ToArray();
    }

    private static IReadOnlyList<NativeAptPoint> CompletedLights(NativeAptDocument document)
        => document.GetAttachments(document.Layout[0] == 0 ? 0 : document.Layout[0] - 1, 0, 0).Lights;

    public static IReadOnlyList<Vector3> GetBuildingWorldLightPositions(ReadOnlySpan<byte> aptBytes,
        float buildingWorldX, float buildingWorldZ, float groundY, float aptHeightOffset)
    {
        var document = NativeAptDocument.Parse(aptBytes);
        // 4c82a0 is called with a capacity of20. Terrain is sampled at each returned X/Z by the host.
        return CompletedLights(document).Take(20).Select(point => NativeAptAttachmentGeometry.Place(document,
            point, buildingWorldX, buildingWorldZ, groundY + aptHeightOffset)).ToArray();
    }
}
